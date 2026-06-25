//! End-to-end command-transaction tests ("对拍"): each exercises a full
//! [`EditCommand`] through [`apply`] against an [`EditorState`], asserting the
//! resulting `Timeline` / `MediaManifest`, undo/redo behavior, versioning, and
//! the refusal path — the behaviors the port must match upstream.

use opentake_domain::{AnimPair, Interpolation, Keyframe, KeyframeTrack};
use opentake_domain::{ChromaKey, ColorGrade, Effect, Mask, MaskShape, Point2};
use opentake_domain::{
    Clip, ClipType, MediaManifest, MediaManifestEntry, MediaSource, Timeline, Track, Transform,
};
use opentake_ops::{
    apply, ClipEntry, ClipMove, ClipProperties, EditCommand, EditError, EditorState, FrameRange,
    KeyframePayload, KeyframeProperty, SeqIdGen, TextEntry,
};

// ---- builders -------------------------------------------------------------

fn clip(id: &str, start: i32, dur: i32) -> Clip {
    Clip::new(id, "asset", start, dur)
}

fn video_track(id: &str, sync: bool, clips: Vec<Clip>) -> Track {
    let mut t = Track::new(id, ClipType::Video);
    t.sync_locked = sync;
    t.clips = clips;
    t
}

fn audio_track(id: &str, sync: bool, clips: Vec<Clip>) -> Track {
    let mut t = Track::new(id, ClipType::Audio);
    t.sync_locked = sync;
    t.clips = clips;
    t
}

fn state(tracks: Vec<Track>) -> EditorState {
    let mut tl = Timeline::new();
    tl.tracks = tracks;
    EditorState::new(tl, MediaManifest::new())
}

fn entry(track_index: usize, media_type: ClipType, start: i32, dur: i32) -> ClipEntry {
    ClipEntry {
        media_ref: "m".into(),
        media_type,
        source_clip_type: media_type,
        track_index,
        start_frame: start,
        duration_frames: dur,
        trim_start_frame: None,
        trim_end_frame: None,
        has_audio: false,
        add_linked_audio: false,
    }
}

// ---- add_clips + overwrite ------------------------------------------------

#[test]
fn add_clips_overwrites_overlapping_clip() {
    // Existing clip [0,100) on a video track; add a new clip at [40,80) ->
    // overwrite splits the existing clip into [0,40) and [80,100).
    let mut st = state(vec![video_track("v", true, vec![clip("old", 0, 100)])]);
    let g = SeqIdGen::new("n-");
    let res = apply(
        &mut st,
        EditCommand::AddClips {
            entries: vec![entry(0, ClipType::Video, 40, 40)],
        },
        &g,
    )
    .unwrap();

    assert!(res.changed);
    assert_eq!(res.action_name, "Add Clip");
    assert_eq!(res.timeline_version, 1);
    assert_eq!(res.affected_clip_ids.len(), 1);

    // Track now holds: old-left [0,40), new [40,80), old-right [80,100).
    let mut spans: Vec<(i32, i32)> = st.timeline.tracks[0]
        .clips
        .iter()
        .map(|c| (c.start_frame, c.end_frame()))
        .collect();
    spans.sort();
    assert_eq!(spans, vec![(0, 40), (40, 80), (80, 100)]);
}

#[test]
fn add_clips_rejects_out_of_range_track() {
    let mut st = state(vec![video_track("v", true, vec![])]);
    let g = SeqIdGen::default();
    let err = apply(
        &mut st,
        EditCommand::AddClips {
            entries: vec![entry(9, ClipType::Video, 0, 30)],
        },
        &g,
    )
    .unwrap_err();
    assert!(matches!(err, EditError::Invalid(_)));
    assert_eq!(st.version(), 0); // unchanged
}

#[test]
fn add_clips_rejects_incompatible_type() {
    // audio asset onto a video track -> incompatible.
    let mut st = state(vec![video_track("v", true, vec![])]);
    let g = SeqIdGen::default();
    let err = apply(
        &mut st,
        EditCommand::AddClips {
            entries: vec![entry(0, ClipType::Audio, 0, 30)],
        },
        &g,
    )
    .unwrap_err();
    assert!(matches!(err, EditError::Invalid(_)));
}

// ---- split + keyframes ----------------------------------------------------

#[test]
fn split_clip_distributes_keyframes_at_cut() {
    // opacity 0->1 over [0,60] (linear); split at frame 130 (offset 30).
    let mut c = clip("c", 100, 60);
    c.opacity_track = Some(KeyframeTrack::from_keyframes(vec![
        Keyframe::with_interpolation(0, 0.0, Interpolation::Linear),
        Keyframe::new(60, 1.0),
    ]));
    let mut st = state(vec![video_track("v", true, vec![c])]);
    let g = SeqIdGen::new("r-");

    let res = apply(
        &mut st,
        EditCommand::SplitClip {
            clip_id: "c".into(),
            at_frame: 130,
        },
        &g,
    )
    .unwrap();
    assert!(res.changed);
    assert_eq!(res.action_name, "Split Clip");
    assert_eq!(res.affected_clip_ids, vec!["r-1".to_string()]);

    let left = st.timeline.tracks[0]
        .clips
        .iter()
        .find(|c| c.id == "c")
        .unwrap();
    let right = st.timeline.tracks[0]
        .clips
        .iter()
        .find(|c| c.id == "r-1")
        .unwrap();
    let lk = left.opacity_track.as_ref().unwrap();
    let rk = right.opacity_track.as_ref().unwrap();
    // left ends with a boundary kf at offset 30 (value 0.5); right starts with it rebased to 0.
    assert_eq!(lk.keyframes.last().unwrap().frame, 30);
    assert!((lk.keyframes.last().unwrap().value - 0.5).abs() < 1e-9);
    assert_eq!(rk.keyframes.first().unwrap().frame, 0);
    assert!((rk.keyframes.first().unwrap().value - 0.5).abs() < 1e-9);
}

#[test]
fn split_outside_range_is_a_no_op_command() {
    let mut st = state(vec![video_track("v", true, vec![clip("c", 100, 60)])]);
    let g = SeqIdGen::default();
    // at_frame == start is exclusive -> rejected (outside range).
    let err = apply(
        &mut st,
        EditCommand::SplitClip {
            clip_id: "c".into(),
            at_frame: 100,
        },
        &g,
    )
    .unwrap_err();
    assert!(matches!(err, EditError::Invalid(_)));
}

// ---- linking: A/V move/split/delete as a unit -----------------------------

fn linked_av_state() -> EditorState {
    let mut vc = clip("v1", 100, 60);
    vc.link_group_id = Some("g1".into());
    let mut ac = clip("a1", 100, 60);
    ac.media_type = ClipType::Audio;
    ac.link_group_id = Some("g1".into());
    state(vec![
        video_track("v", true, vec![vc]),
        audio_track("a", true, vec![ac]),
    ])
}

#[test]
fn split_linked_pair_splits_partner_and_regroups() {
    let mut st = linked_av_state();
    let g = SeqIdGen::new("n-");
    let res = apply(
        &mut st,
        EditCommand::SplitClip {
            clip_id: "v1".into(),
            at_frame: 130,
        },
        &g,
    )
    .unwrap();
    // both partners split -> two right halves; action name pluralized.
    assert_eq!(res.action_name, "Split Clips");
    assert_eq!(res.affected_clip_ids.len(), 2);

    // each track now has two clips.
    assert_eq!(st.timeline.tracks[0].clips.len(), 2);
    assert_eq!(st.timeline.tracks[1].clips.len(), 2);
    // right halves share a new group, distinct from g1.
    let rights: Vec<&Clip> = st
        .timeline
        .tracks
        .iter()
        .flat_map(|t| &t.clips)
        .filter(|c| c.start_frame == 130)
        .collect();
    assert_eq!(rights.len(), 2);
    assert_eq!(rights[0].link_group_id, rights[1].link_group_id);
    assert_ne!(rights[0].link_group_id.as_deref(), Some("g1"));
}

#[test]
fn duplicate_linked_pair_keeps_copies_linked() {
    // Option/Alt-drag duplicating an A/V linked pair must keep the copies linked
    // to each other under a fresh group id (groupCounts/groupRemap semantics).
    let mut st = linked_av_state();
    let g = SeqIdGen::default();
    let res = apply(
        &mut st,
        EditCommand::DuplicateClips {
            clip_ids: vec!["v1".into(), "a1".into()],
            offset_frames: 200,
            target_track_indexes: vec![0, 1],
        },
        &g,
    )
    .unwrap();
    assert_eq!(res.affected_clip_ids.len(), 2);

    // The copies share a NEW link_group_id (same as each other, different from
    // the source "g1") — the A/V link survives the duplicate.
    let vc = find_clip(&st, &res.affected_clip_ids[0]);
    let ac = find_clip(&st, &res.affected_clip_ids[1]);
    assert_eq!(vc.link_group_id, ac.link_group_id);
    assert_ne!(vc.link_group_id.as_deref(), Some("g1"));
    assert!(
        vc.link_group_id.is_some(),
        "linked pair copies must stay linked"
    );

    // Originals keep "g1".
    assert_eq!(find_clip(&st, "v1").link_group_id.as_deref(), Some("g1"));
    assert_eq!(find_clip(&st, "a1").link_group_id.as_deref(), Some("g1"));
}

#[test]
fn remove_clips_expands_to_linked_partner() {
    let mut st = linked_av_state();
    let g = SeqIdGen::default();
    // removing just v1 should also remove its linked a1.
    let res = apply(
        &mut st,
        EditCommand::RemoveClips {
            clip_ids: vec!["v1".into()],
        },
        &g,
    )
    .unwrap();
    assert!(res.changed);
    assert_eq!(res.action_name, "Remove Clips"); // 2 clips after expansion
                                                 // both tracks emptied and pruned.
    assert!(st.timeline.tracks.is_empty());
}

#[test]
fn link_then_unlink_round_trips() {
    let mut st = state(vec![
        video_track("v", true, vec![clip("a", 0, 30)]),
        audio_track("au", true, vec![clip("b", 0, 30)]),
    ]);
    let g = SeqIdGen::new("g-");
    apply(
        &mut st,
        EditCommand::Link {
            clip_ids: vec!["a".into(), "b".into()],
        },
        &g,
    )
    .unwrap();
    let ga = st.find_clip("a").unwrap();
    let gid = st.timeline.tracks[ga.track_index].clips[ga.clip_index]
        .link_group_id
        .clone();
    assert!(gid.is_some());
    // both share the same fresh group.
    let gb = st.find_clip("b").unwrap();
    assert_eq!(
        st.timeline.tracks[gb.track_index].clips[gb.clip_index].link_group_id,
        gid
    );

    apply(
        &mut st,
        EditCommand::Unlink {
            clip_ids: vec!["a".into()],
        },
        &g,
    )
    .unwrap();
    // unlink expands to the whole group -> both cleared.
    for t in &st.timeline.tracks {
        for c in &t.clips {
            assert!(c.link_group_id.is_none());
        }
    }
}

// ---- ripple delete refusal ------------------------------------------------

#[test]
fn ripple_delete_ranges_refuses_when_sync_follower_collides() {
    // Anchor video track [0,200). A sync-locked follower has two clips that
    // would collide once shifted left to close a 60-frame gap -> refuse.
    let anchor = video_track("v", true, vec![clip("a", 0, 200)]);
    let follower = audio_track(
        "f",
        true,
        vec![clip("fixed", 0, 50), clip("mover", 100, 50)],
    );
    let mut st = state(vec![anchor, follower]);
    let before_version = st.version();
    let g = SeqIdGen::default();

    let err = apply(
        &mut st,
        EditCommand::RippleDeleteRanges {
            track_index: 0,
            ranges: vec![FrameRange::new(0, 60)],
        },
        &g,
    )
    .unwrap_err();
    assert!(matches!(err, EditError::Refused(_)));
    // Document completely untouched: anchor clip full length, follower unchanged, version same.
    assert_eq!(st.timeline.tracks[0].clips[0].duration_frames, 200);
    assert_eq!(
        st.timeline.tracks[1]
            .clips
            .iter()
            .find(|c| c.id == "mover")
            .unwrap()
            .start_frame,
        100
    );
    assert_eq!(st.version(), before_version);
    assert!(!st.can_undo());
}

#[test]
fn ripple_delete_ranges_succeeds_and_shifts_follower() {
    // Same shape but the follower can absorb the shift.
    let anchor = video_track("v", true, vec![clip("a", 0, 200)]);
    let follower = audio_track("f", true, vec![clip("x", 120, 40)]);
    let mut st = state(vec![anchor, follower]);
    let g = SeqIdGen::new("r-");

    let res = apply(
        &mut st,
        EditCommand::RippleDeleteRanges {
            track_index: 0,
            ranges: vec![FrameRange::new(40, 60)], // remove 20 frames inside anchor
        },
        &g,
    )
    .unwrap();
    assert!(res.changed);
    assert_eq!(res.action_name, "Ripple Delete");
    // anchor span shrinks by 20: max end 200 -> 180.
    let max_end = st.timeline.tracks[0]
        .clips
        .iter()
        .map(|c| c.end_frame())
        .max()
        .unwrap();
    assert_eq!(max_end, 180);
    // follower x at 120 shifts left by 20 -> 100.
    assert_eq!(st.timeline.tracks[1].clips[0].start_frame, 100);
}

// ---- undo / redo ----------------------------------------------------------

#[test]
fn undo_redo_restores_and_versions() {
    let mut st = state(vec![video_track("v", true, vec![clip("old", 0, 100)])]);
    let g = SeqIdGen::new("n-");

    // add a clip (overwrite splits old)
    apply(
        &mut st,
        EditCommand::AddClips {
            entries: vec![entry(0, ClipType::Video, 40, 40)],
        },
        &g,
    )
    .unwrap();
    let after_add = st.timeline.clone();
    assert_eq!(st.version(), 1);
    assert_eq!(st.timeline.tracks[0].clips.len(), 3);

    // undo -> back to single clip
    let r = apply(&mut st, EditCommand::Undo, &g).unwrap();
    assert!(r.changed);
    assert_eq!(r.action_name, "Undo");
    assert_eq!(st.timeline.tracks[0].clips.len(), 1);
    assert_eq!(st.timeline.tracks[0].clips[0].id, "old");
    assert_eq!(st.version(), 2);

    // redo -> back to three clips, identical to after_add
    let r = apply(&mut st, EditCommand::Redo, &g).unwrap();
    assert!(r.changed);
    assert_eq!(st.timeline, after_add);
    assert_eq!(st.version(), 3);
}

#[test]
fn undo_with_empty_history_is_no_op() {
    let mut st = state(vec![video_track("v", true, vec![])]);
    let g = SeqIdGen::default();
    let r = apply(&mut st, EditCommand::Undo, &g).unwrap();
    assert!(!r.changed);
    assert_eq!(r.summary, "Nothing to undo");
    assert_eq!(st.version(), 0);
}

#[test]
fn new_edit_after_undo_clears_redo() {
    let mut st = state(vec![video_track("v", true, vec![clip("old", 0, 100)])]);
    let g = SeqIdGen::new("n-");
    apply(
        &mut st,
        EditCommand::AddClips {
            entries: vec![entry(0, ClipType::Video, 40, 40)],
        },
        &g,
    )
    .unwrap();
    apply(&mut st, EditCommand::Undo, &g).unwrap();
    assert!(st.can_redo());
    // a fresh edit invalidates redo.
    apply(
        &mut st,
        EditCommand::AddClips {
            entries: vec![entry(0, ClipType::Video, 0, 10)],
        },
        &g,
    )
    .unwrap();
    let r = apply(&mut st, EditCommand::Redo, &g).unwrap();
    assert!(!r.changed);
}

// ---- trim / set properties ------------------------------------------------

#[test]
fn trim_clips_resizes_in_place_overwrite_style() {
    // Two adjacent clips; trimming the first must NOT move the second (overwrite).
    let mut st = state(vec![video_track(
        "v",
        true,
        vec![clip("a", 0, 100), clip("b", 100, 50)],
    )]);
    let g = SeqIdGen::default();
    // trim a's end by 30 source frames (speed 1.0) -> a becomes [0,70), b unmoved.
    let res = apply(
        &mut st,
        EditCommand::TrimClips {
            edits: vec![("a".into(), 0, 30)],
        },
        &g,
    )
    .unwrap();
    assert!(res.changed);
    let a = st.timeline.tracks[0]
        .clips
        .iter()
        .find(|c| c.id == "a")
        .unwrap();
    let b = st.timeline.tracks[0]
        .clips
        .iter()
        .find(|c| c.id == "b")
        .unwrap();
    assert_eq!((a.start_frame, a.end_frame()), (0, 70));
    assert_eq!(b.start_frame, 100); // unchanged
}

#[test]
fn set_clip_properties_propagates_timing_to_linked_partner() {
    let mut st = linked_av_state(); // v1 + a1 linked, both [100,160)
    let g = SeqIdGen::default();
    // set durationFrames=40 on v1 -> partner a1 also gets duration 40.
    let res = apply(
        &mut st,
        EditCommand::SetClipProperties {
            clip_ids: vec!["v1".into()],
            properties: ClipProperties {
                duration_frames: Some(40),
                ..Default::default()
            },
        },
        &g,
    )
    .unwrap();
    assert!(res.changed);
    let v1 = st.find_clip("v1").unwrap();
    let a1 = st.find_clip("a1").unwrap();
    assert_eq!(
        st.timeline.tracks[v1.track_index].clips[v1.clip_index].duration_frames,
        40
    );
    assert_eq!(
        st.timeline.tracks[a1.track_index].clips[a1.clip_index].duration_frames,
        40
    );
}

#[test]
fn set_clip_properties_scalar_clears_keyframe_track() {
    let mut c = clip("c", 0, 60);
    c.opacity_track = Some(KeyframeTrack::from_keyframes(vec![Keyframe::new(0, 0.0)]));
    let mut st = state(vec![video_track("v", true, vec![c])]);
    let g = SeqIdGen::default();
    apply(
        &mut st,
        EditCommand::SetClipProperties {
            clip_ids: vec!["c".into()],
            properties: ClipProperties {
                opacity: Some(0.5),
                ..Default::default()
            },
        },
        &g,
    )
    .unwrap();
    let c = &st.timeline.tracks[0].clips[0];
    assert!((c.opacity - 0.5).abs() < 1e-9);
    assert!(c.opacity_track.is_none()); // cleared by setting the scalar
}

#[test]
fn set_clip_properties_crop_sets_and_clears_track() {
    let mut st = state(vec![video_track("v", true, vec![clip("c", 0, 60)])]);
    let g = SeqIdGen::default();
    // Pre-existing crop track should be cleared when a static crop is set.
    let mut existing = st.timeline.tracks[0].clips[0].clone();
    existing.crop_track = Some(KeyframeTrack::from_keyframes(vec![Keyframe::new(
        0,
        opentake_domain::Crop {
            left: 0.1,
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
        },
    )]));
    st.timeline.tracks[0].clips[0] = existing;

    apply(
        &mut st,
        EditCommand::SetClipProperties {
            clip_ids: vec!["c".into()],
            properties: ClipProperties {
                crop: Some(opentake_domain::Crop {
                    left: 0.2,
                    top: 0.1,
                    right: 0.0,
                    bottom: 0.0,
                }),
                ..Default::default()
            },
        },
        &g,
    )
    .unwrap();

    let c = &st.timeline.tracks[0].clips[0];
    assert!((c.crop.left - 0.2).abs() < 1e-9);
    assert!((c.crop.top - 0.1).abs() < 1e-9);
    assert!(c.crop_track.is_none()); // cleared by setting the static value
}

#[test]
fn set_clip_properties_fade_sets_frames_and_interpolation() {
    let mut st = state(vec![video_track("v", true, vec![clip("c", 0, 60)])]);
    let g = SeqIdGen::default();
    apply(
        &mut st,
        EditCommand::SetClipProperties {
            clip_ids: vec!["c".into()],
            properties: ClipProperties {
                fade_in_frames: Some(10),
                fade_out_frames: Some(15),
                fade_in_interpolation: Some(Interpolation::Smooth),
                fade_out_interpolation: Some(Interpolation::Hold),
                ..Default::default()
            },
        },
        &g,
    )
    .unwrap();

    let c = &st.timeline.tracks[0].clips[0];
    assert_eq!(c.fade_in_frames, 10);
    assert_eq!(c.fade_out_frames, 15);
    assert_eq!(c.fade_in_interpolation, Interpolation::Smooth);
    assert_eq!(c.fade_out_interpolation, Interpolation::Hold);
}

#[test]
fn set_clip_properties_fade_clamps_to_duration() {
    let mut st = state(vec![video_track("v", true, vec![clip("c", 0, 30)])]);
    let g = SeqIdGen::default();
    // fade_in 100 on a 30-frame clip should clamp to 30, fade_out to 0.
    apply(
        &mut st,
        EditCommand::SetClipProperties {
            clip_ids: vec!["c".into()],
            properties: ClipProperties {
                fade_in_frames: Some(100),
                ..Default::default()
            },
        },
        &g,
    )
    .unwrap();
    let c = &st.timeline.tracks[0].clips[0];
    assert_eq!(c.fade_in_frames, 30);
    assert_eq!(c.fade_out_frames, 0);
}

#[test]
fn set_clip_properties_flip_writes_to_transform() {
    let mut st = state(vec![video_track("v", true, vec![clip("c", 0, 60)])]);
    let g = SeqIdGen::default();
    apply(
        &mut st,
        EditCommand::SetClipProperties {
            clip_ids: vec!["c".into()],
            properties: ClipProperties {
                flip_horizontal: Some(true),
                flip_vertical: Some(true),
                ..Default::default()
            },
        },
        &g,
    )
    .unwrap();
    let c = &st.timeline.tracks[0].clips[0];
    assert!(c.transform.flip_horizontal);
    assert!(c.transform.flip_vertical);
}

#[test]
fn set_clip_properties_multiple_fields_at_once() {
    let mut st = state(vec![video_track("v", true, vec![clip("c", 0, 60)])]);
    let g = SeqIdGen::default();
    apply(
        &mut st,
        EditCommand::SetClipProperties {
            clip_ids: vec!["c".into()],
            properties: ClipProperties {
                crop: Some(opentake_domain::Crop {
                    left: 0.1,
                    top: 0.2,
                    right: 0.3,
                    bottom: 0.4,
                }),
                fade_in_frames: Some(5),
                fade_in_interpolation: Some(Interpolation::Smooth),
                flip_horizontal: Some(true),
                opacity: Some(0.8),
                ..Default::default()
            },
        },
        &g,
    )
    .unwrap();
    let c = &st.timeline.tracks[0].clips[0];
    assert!((c.crop.left - 0.1).abs() < 1e-9);
    assert!((c.crop.bottom - 0.4).abs() < 1e-9);
    assert_eq!(c.fade_in_frames, 5);
    assert_eq!(c.fade_in_interpolation, Interpolation::Smooth);
    assert!(c.transform.flip_horizontal);
    assert!((c.opacity - 0.8).abs() < 1e-9);
    assert!(c.opacity_track.is_none()); // opacity scalar cleared its track
}

// ---- set_keyframes --------------------------------------------------------

#[test]
fn set_keyframes_installs_position_track() {
    let mut st = state(vec![video_track("v", true, vec![clip("c", 0, 60)])]);
    let g = SeqIdGen::default();
    let track = KeyframeTrack::from_keyframes(vec![
        Keyframe::new(0, AnimPair::new(0.0, 0.0)),
        Keyframe::new(30, AnimPair::new(0.5, 0.5)),
    ]);
    let res = apply(
        &mut st,
        EditCommand::SetKeyframes {
            clip_id: "c".into(),
            property: KeyframeProperty::Position,
            payload: KeyframePayload::Pair(track),
        },
        &g,
    )
    .unwrap();
    assert!(res.changed);
    assert!(st.timeline.tracks[0].clips[0].position_track.is_some());
}

#[test]
fn set_keyframes_rejects_type_mismatch() {
    let mut st = state(vec![video_track("v", true, vec![clip("c", 0, 60)])]);
    let g = SeqIdGen::default();
    // opacity is a scalar property; passing a Pair payload is a mismatch.
    let err = apply(
        &mut st,
        EditCommand::SetKeyframes {
            clip_id: "c".into(),
            property: KeyframeProperty::Opacity,
            payload: KeyframePayload::Pair(KeyframeTrack::new()),
        },
        &g,
    )
    .unwrap_err();
    assert!(matches!(err, EditError::Invalid(_)));
}

// ---- insert (ripple) ------------------------------------------------------

#[test]
fn insert_clips_pushes_later_clips() {
    let mut st = state(vec![video_track(
        "v",
        true,
        vec![clip("a", 0, 30), clip("b", 30, 30)],
    )]);
    let g = SeqIdGen::new("n-");
    // insert a 20-frame clip at frame 30 -> b pushed to 50.
    let res = apply(
        &mut st,
        EditCommand::InsertClips {
            track_index: 0,
            at_frame: 30,
            entries: vec![entry(0, ClipType::Video, 0, 20)],
        },
        &g,
    )
    .unwrap();
    assert!(res.changed);
    assert_eq!(res.action_name, "Ripple Insert Clip");
    let b = st.timeline.tracks[0]
        .clips
        .iter()
        .find(|c| c.id == "b")
        .unwrap();
    assert_eq!(b.start_frame, 50);
}

// ---- folders --------------------------------------------------------------

#[test]
fn create_folder_and_move_asset_into_it() {
    use opentake_domain::{MediaManifestEntry, MediaSource};
    let mut tl = Timeline::new();
    tl.tracks.push(video_track("v", true, vec![]));
    let mut manifest = MediaManifest::new();
    manifest.entries.push(MediaManifestEntry {
        id: "asset1".into(),
        name: "Clip".into(),
        kind: ClipType::Video,
        source: MediaSource::External {
            absolute_path: "/x.mp4".into(),
        },
        duration: 1.0,
        generation_input: None,
        source_width: None,
        source_height: None,
        source_fps: None,
        has_audio: None,
        folder_id: None,
        cached_remote_url: None,
        cached_remote_url_expires_at: None,
    });
    let mut st = EditorState::new(tl, manifest);
    let g = SeqIdGen::new("f-");

    let res = apply(
        &mut st,
        EditCommand::CreateFolder {
            name: "B-Roll".into(),
            parent_folder_id: None,
        },
        &g,
    )
    .unwrap();
    assert!(res.changed);
    let folder_id = res.affected_clip_ids[0].clone();
    assert_eq!(st.manifest.folders.len(), 1);

    let res = apply(
        &mut st,
        EditCommand::MoveToFolder {
            asset_ids: vec!["asset1".into()],
            folder_id: Some(folder_id.clone()),
        },
        &g,
    )
    .unwrap();
    assert!(res.changed);
    assert_eq!(st.manifest.entries[0].folder_id, Some(folder_id));

    // undo move -> asset back to root; folder still present.
    apply(&mut st, EditCommand::Undo, &g).unwrap();
    assert!(st.manifest.entries[0].folder_id.is_none());
    assert_eq!(st.manifest.folders.len(), 1);
}

// ---- remove tracks --------------------------------------------------------

#[test]
fn remove_tracks_resolves_indexes_before_removal() {
    let mut st = state(vec![
        video_track("v0", true, vec![clip("a", 0, 30)]),
        video_track("v1", true, vec![clip("b", 0, 30)]),
        audio_track("au", true, vec![clip("c", 0, 30)]),
    ]);
    let g = SeqIdGen::default();
    // remove indexes 0 and 2 -> ids resolved up front so the shift is correct.
    let res = apply(
        &mut st,
        EditCommand::RemoveTracks {
            track_indexes: vec![0, 2],
        },
        &g,
    )
    .unwrap();
    assert!(res.changed);
    assert_eq!(res.action_name, "Remove Tracks");
    assert_eq!(st.timeline.tracks.len(), 1);
    assert_eq!(st.timeline.tracks[0].id, "v1");
}

// ---- no-change command ----------------------------------------------------

#[test]
fn unchanged_command_does_not_push_undo_or_bump_version() {
    // Moving a clip to its current location yields no diff.
    let mut st = state(vec![video_track("v", true, vec![clip("a", 0, 30)])]);
    let g = SeqIdGen::default();
    let res = apply(
        &mut st,
        EditCommand::MoveClips {
            moves: vec![ClipMove {
                clip_id: "a".into(),
                to_track: 0,
                to_frame: 0,
            }],
        },
        &g,
    )
    .unwrap();
    assert!(!res.changed);
    assert_eq!(st.version(), 0);
    assert!(!st.can_undo());
}

// ---- add_texts ------------------------------------------------------------

#[test]
fn add_texts_places_text_clip_with_style() {
    let mut st = state(vec![video_track("v", true, vec![])]);
    let g = SeqIdGen::new("t-");
    let res = apply(
        &mut st,
        EditCommand::AddTexts {
            entries: vec![TextEntry {
                track_index: 0,
                start_frame: 0,
                duration_frames: 90,
                content: "Hello".into(),
                text_style: opentake_domain::TextStyle::default(),
                transform: Transform::default(),
            }],
        },
        &g,
    )
    .unwrap();
    assert!(res.changed);
    assert_eq!(res.affected_clip_ids.len(), 1);
    let c = &st.timeline.tracks[0].clips[0];
    assert_eq!(c.media_type, ClipType::Text);
    assert_eq!(c.text_content.as_deref(), Some("Hello"));
    assert!(c.text_style.is_some());
}

#[test]
fn add_texts_rejects_audio_track() {
    let mut st = state(vec![audio_track("a", true, vec![])]);
    let g = SeqIdGen::default();
    let err = apply(
        &mut st,
        EditCommand::AddTexts {
            entries: vec![TextEntry {
                track_index: 0,
                start_frame: 0,
                duration_frames: 90,
                content: "Hi".into(),
                text_style: opentake_domain::TextStyle::default(),
                transform: Transform::default(),
            }],
        },
        &g,
    )
    .unwrap_err();
    assert!(matches!(err, EditError::Invalid(_)));
}

// ---- defensive: empty payloads -------------------------------------------

#[test]
fn empty_payloads_are_rejected() {
    let mut st = state(vec![video_track("v", true, vec![])]);
    let g = SeqIdGen::default();
    assert!(matches!(
        apply(&mut st, EditCommand::AddClips { entries: vec![] }, &g),
        Err(EditError::Invalid(_))
    ));
    assert!(matches!(
        apply(&mut st, EditCommand::MoveClips { moves: vec![] }, &g),
        Err(EditError::Invalid(_))
    ));
    assert!(matches!(
        apply(&mut st, EditCommand::RemoveClips { clip_ids: vec![] }, &g),
        Err(EditError::Invalid(_))
    ));
}

// ---- advanced pixel-effect commands (A-tier) ------------------------------

fn one_clip_state() -> EditorState {
    state(vec![video_track("v", true, vec![clip("c", 0, 30)])])
}

fn find_clip<'a>(st: &'a EditorState, id: &str) -> &'a Clip {
    st.timeline
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter())
        .find(|c| c.id == id)
        .expect("clip exists")
}

#[test]
fn set_color_grade_applies_and_undoes() {
    let mut st = one_clip_state();
    let g = SeqIdGen::default();
    let grade = ColorGrade {
        exposure: 0.5,
        saturation: 1.2,
        ..Default::default()
    };
    let res = apply(
        &mut st,
        EditCommand::SetColorGrade {
            clip_ids: vec!["c".into()],
            grade: Some(grade),
        },
        &g,
    )
    .unwrap();
    assert!(res.changed);
    assert_eq!(res.action_name, "Set Color Grade");
    assert_eq!(res.timeline_version, 1);
    assert_eq!(find_clip(&st, "c").color_grade, Some(grade));

    // Undo restores the cleared grade.
    apply(&mut st, EditCommand::Undo, &g).unwrap();
    assert_eq!(find_clip(&st, "c").color_grade, None);
}

#[test]
fn set_color_grade_none_clears() {
    let mut st = one_clip_state();
    let g = SeqIdGen::default();
    // First set a grade...
    apply(
        &mut st,
        EditCommand::SetColorGrade {
            clip_ids: vec!["c".into()],
            grade: Some(ColorGrade {
                exposure: 1.0,
                ..Default::default()
            }),
        },
        &g,
    )
    .unwrap();
    // ...then clear it.
    let res = apply(
        &mut st,
        EditCommand::SetColorGrade {
            clip_ids: vec!["c".into()],
            grade: None,
        },
        &g,
    )
    .unwrap();
    assert!(res.changed);
    assert_eq!(find_clip(&st, "c").color_grade, None);
}

#[test]
fn set_color_grade_no_op_when_unchanged() {
    let mut st = one_clip_state();
    let g = SeqIdGen::default();
    // Setting None on a clip with no grade is a no-op (no version bump).
    let res = apply(
        &mut st,
        EditCommand::SetColorGrade {
            clip_ids: vec!["c".into()],
            grade: None,
        },
        &g,
    )
    .unwrap();
    assert!(!res.changed);
    assert_eq!(st.version(), 0);
}

#[test]
fn set_color_grade_batches_multiple_clips() {
    let mut st = state(vec![video_track(
        "v",
        true,
        vec![clip("a", 0, 30), clip("b", 30, 30)],
    )]);
    let g = SeqIdGen::default();
    let grade = ColorGrade {
        contrast: 0.3,
        ..Default::default()
    };
    let res = apply(
        &mut st,
        EditCommand::SetColorGrade {
            clip_ids: vec!["a".into(), "b".into()],
            grade: Some(grade),
        },
        &g,
    )
    .unwrap();
    assert!(res.changed);
    assert_eq!(find_clip(&st, "a").color_grade, Some(grade));
    assert_eq!(find_clip(&st, "b").color_grade, Some(grade));
}

#[test]
fn set_chroma_key_applies_and_clears() {
    let mut st = one_clip_state();
    let g = SeqIdGen::default();
    let key = ChromaKey::default();
    let res = apply(
        &mut st,
        EditCommand::SetChromaKey {
            clip_ids: vec!["c".into()],
            chroma_key: Some(key),
        },
        &g,
    )
    .unwrap();
    assert!(res.changed);
    assert_eq!(res.action_name, "Set Chroma Key");
    assert_eq!(find_clip(&st, "c").chroma_key, Some(key));

    let res2 = apply(
        &mut st,
        EditCommand::SetChromaKey {
            clip_ids: vec!["c".into()],
            chroma_key: None,
        },
        &g,
    )
    .unwrap();
    assert!(res2.changed);
    assert_eq!(find_clip(&st, "c").chroma_key, None);
}

#[test]
fn set_masks_replaces_list() {
    let mut st = one_clip_state();
    let g = SeqIdGen::default();
    let masks = vec![Mask {
        shape: MaskShape::Circle {
            center: Point2::new(0.5, 0.5),
            radius: Point2::new(0.3, 0.3),
        },
        feather: 0.05,
        invert: false,
    }];
    let res = apply(
        &mut st,
        EditCommand::SetMasks {
            clip_ids: vec!["c".into()],
            masks: masks.clone(),
        },
        &g,
    )
    .unwrap();
    assert!(res.changed);
    assert_eq!(res.action_name, "Set Masks");
    assert_eq!(find_clip(&st, "c").masks, masks);

    // Replacing with an empty list clears all masks.
    let res2 = apply(
        &mut st,
        EditCommand::SetMasks {
            clip_ids: vec!["c".into()],
            masks: vec![],
        },
        &g,
    )
    .unwrap();
    assert!(res2.changed);
    assert!(find_clip(&st, "c").masks.is_empty());
}

#[test]
fn set_effects_replaces_chain() {
    let mut st = one_clip_state();
    let g = SeqIdGen::default();
    let effects = vec![
        Effect::new("gaussianBlur").with_param("radius", 4.0),
        Effect::new("glow").with_param("intensity", 0.6),
    ];
    let res = apply(
        &mut st,
        EditCommand::SetEffects {
            clip_ids: vec!["c".into()],
            effects: effects.clone(),
        },
        &g,
    )
    .unwrap();
    assert!(res.changed);
    assert_eq!(res.action_name, "Set Effects");
    assert_eq!(find_clip(&st, "c").effects, effects);
}

#[test]
fn advanced_effect_commands_reject_empty_and_missing() {
    let mut st = one_clip_state();
    let g = SeqIdGen::default();
    // Empty clip_ids -> Invalid.
    assert!(matches!(
        apply(
            &mut st,
            EditCommand::SetColorGrade {
                clip_ids: vec![],
                grade: None
            },
            &g
        ),
        Err(EditError::Invalid(_))
    ));
    // Unknown clip id -> Invalid, no version bump.
    assert!(matches!(
        apply(
            &mut st,
            EditCommand::SetEffects {
                clip_ids: vec!["nope".into()],
                effects: vec![Effect::new("blur")]
            },
            &g
        ),
        Err(EditError::Invalid(_))
    ));
    assert_eq!(st.version(), 0);
}

// ---- ripple_delete_clips --------------------------------------------------

#[test]
fn ripple_delete_clips_closes_the_gap() {
    // Two back-to-back clips; deleting the first ripples the second to frame 0.
    let v = video_track("v", false, vec![clip("a", 0, 50), clip("b", 50, 50)]);
    let mut st = state(vec![v]);
    let g = SeqIdGen::default();

    let res = apply(
        &mut st,
        EditCommand::RippleDeleteClips {
            clip_ids: vec!["a".into()],
        },
        &g,
    )
    .unwrap();
    assert!(res.changed);
    let clips = &st.timeline.tracks[0].clips;
    assert_eq!(clips.len(), 1);
    assert_eq!(clips[0].id, "b");
    assert_eq!(clips[0].start_frame, 0); // gap closed
    assert!(st.can_undo());
}

#[test]
fn ripple_delete_clips_rejects_unknown_clip() {
    let v = video_track("v", false, vec![clip("a", 0, 50)]);
    let mut st = state(vec![v]);
    let g = SeqIdGen::default();
    assert!(matches!(
        apply(
            &mut st,
            EditCommand::RippleDeleteClips {
                clip_ids: vec!["missing".into()],
            },
            &g,
        ),
        Err(EditError::Invalid(_))
    ));
    assert_eq!(st.version(), 0);
}

// ---- swap_media ------------------------------------------------------------

/// Build a manifest entry with `duration` in seconds and an External source.
fn media_entry(id: &str, kind: ClipType, duration_secs: f64) -> MediaManifestEntry {
    MediaManifestEntry {
        id: id.into(),
        name: id.into(),
        kind,
        source: MediaSource::External {
            absolute_path: format!("/abs/{id}"),
        },
        duration: duration_secs,
        generation_input: None,
        source_width: None,
        source_height: None,
        source_fps: None,
        has_audio: None,
        folder_id: None,
        cached_remote_url: None,
        cached_remote_url_expires_at: None,
    }
}

/// Build a state with the given tracks and manifest entries (fps defaults to 30).
fn state_with_media(tracks: Vec<Track>, entries: Vec<MediaManifestEntry>) -> EditorState {
    let mut tl = Timeline::new();
    tl.tracks = tracks;
    let mut manifest = MediaManifest::new();
    manifest.entries = entries;
    EditorState::new(tl, manifest)
}

#[test]
fn swap_media_replaces_ref_and_preserves_attributes() {
    // Clip duration 100 frames (fps=30 -> 100/30 secs). New media same length.
    let mut c = clip("c", 0, 100);
    c.opacity = 0.7;
    c.transform = Transform {
        center_x: 0.3,
        center_y: 0.4,
        width: 0.5,
        height: 0.6,
        rotation: 15.0,
        flip_horizontal: true,
        flip_vertical: false,
    };
    c.trim_start_frame = 5;
    c.trim_end_frame = 7;
    c.speed = 1.5;
    let v = video_track("v", true, vec![c]);
    let entries = vec![
        media_entry("old", ClipType::Video, 100.0 / 30.0),
        media_entry("new", ClipType::Video, 100.0 / 30.0),
    ];
    let mut st = state_with_media(vec![v], entries);
    let g = SeqIdGen::default();

    let res = apply(
        &mut st,
        EditCommand::SwapMedia {
            clip_id: "c".into(),
            media_ref: "new".into(),
        },
        &g,
    )
    .unwrap();

    assert!(res.changed);
    assert_eq!(res.action_name, "Swap Media");
    assert_eq!(res.affected_clip_ids, vec!["c".to_string()]);
    let clip = &st.timeline.tracks[0].clips[0];
    assert_eq!(clip.media_ref, "new");
    assert_eq!(clip.duration_frames, 100); // unchanged
                                           // Preserved editing attributes
    assert!((clip.opacity - 0.7).abs() < 1e-9);
    assert!((clip.transform.center_x - 0.3).abs() < 1e-9);
    assert!((clip.transform.rotation - 15.0).abs() < 1e-9);
    assert!(clip.transform.flip_horizontal);
    // trim / speed untouched (resetTrim=false)
    assert_eq!(clip.trim_start_frame, 5);
    assert_eq!(clip.trim_end_frame, 7);
    assert!((clip.speed - 1.5).abs() < 1e-9);
}

#[test]
fn swap_media_does_not_truncate_when_new_media_shorter() {
    // resetTrim=false: clip duration is preserved even when the new media is
    // shorter. The render layer is responsible for any overshoot sampling.
    let mut c = clip("c", 0, 100);
    c.start_frame = 20;
    c.trim_start_frame = 2;
    c.trim_end_frame = 3;
    let v = video_track("v", true, vec![c]);
    let entries = vec![
        media_entry("old", ClipType::Video, 100.0 / 30.0),
        media_entry("short", ClipType::Video, 50.0 / 30.0),
    ];
    let mut st = state_with_media(vec![v], entries);
    let g = SeqIdGen::default();

    let res = apply(
        &mut st,
        EditCommand::SwapMedia {
            clip_id: "c".into(),
            media_ref: "short".into(),
        },
        &g,
    )
    .unwrap();

    assert!(res.changed);
    let clip = &st.timeline.tracks[0].clips[0];
    assert_eq!(clip.media_ref, "short");
    // Start / duration / trim all untouched.
    assert_eq!(clip.start_frame, 20);
    assert_eq!(clip.duration_frames, 100);
    assert_eq!(clip.trim_start_frame, 2);
    assert_eq!(clip.trim_end_frame, 3);
}

#[test]
fn swap_media_rejects_missing_media_ref() {
    let v = video_track("v", true, vec![clip("c", 0, 100)]);
    let entries = vec![media_entry("old", ClipType::Video, 100.0 / 30.0)];
    let mut st = state_with_media(vec![v], entries);
    let g = SeqIdGen::default();

    let err = apply(
        &mut st,
        EditCommand::SwapMedia {
            clip_id: "c".into(),
            media_ref: "nonexistent".into(),
        },
        &g,
    )
    .unwrap_err();

    assert!(matches!(err, EditError::Invalid(_)));
    assert_eq!(st.version(), 0); // unchanged
                                 // Original media_ref preserved.
    assert_eq!(st.timeline.tracks[0].clips[0].media_ref, "asset");
}

#[test]
fn swap_media_rejects_type_mismatch() {
    // Clip is video; asset is audio. Must refuse (no isVisual leniency).
    let mut c = clip("c", 0, 100);
    c.media_type = ClipType::Video;
    c.source_clip_type = ClipType::Video;
    let v = video_track("v", true, vec![c]);
    let entries = vec![
        media_entry("old", ClipType::Video, 100.0 / 30.0),
        media_entry("audio1", ClipType::Audio, 100.0 / 30.0),
    ];
    let mut st = state_with_media(vec![v], entries);
    let g = SeqIdGen::default();

    let err = apply(
        &mut st,
        EditCommand::SwapMedia {
            clip_id: "c".into(),
            media_ref: "audio1".into(),
        },
        &g,
    )
    .unwrap_err();

    assert!(matches!(err, EditError::Refused(_)));
    assert_eq!(st.version(), 0); // unchanged
                                 // Original media_ref preserved.
    assert_eq!(st.timeline.tracks[0].clips[0].media_ref, "asset");
    assert_eq!(st.timeline.tracks[0].clips[0].media_type, ClipType::Video);
}

#[test]
fn swap_media_rejects_missing_clip() {
    let v = video_track("v", true, vec![]);
    let entries = vec![media_entry("new", ClipType::Video, 100.0 / 30.0)];
    let mut st = state_with_media(vec![v], entries);
    let g = SeqIdGen::default();

    let err = apply(
        &mut st,
        EditCommand::SwapMedia {
            clip_id: "missing".into(),
            media_ref: "new".into(),
        },
        &g,
    )
    .unwrap_err();

    assert!(matches!(err, EditError::Invalid(_)));
    assert_eq!(st.version(), 0);
}

#[test]
fn swap_media_no_op_on_same_ref() {
    // Seed clip references "asset" (builder default); swapping to "asset" must
    // be a no-op (no undo entry, no version bump).
    let v = video_track("v", true, vec![clip("c", 0, 100)]);
    let entries = vec![media_entry("asset", ClipType::Video, 100.0 / 30.0)];
    let mut st = state_with_media(vec![v], entries);
    let g = SeqIdGen::default();
    let version_before = st.version();

    let res = apply(
        &mut st,
        EditCommand::SwapMedia {
            clip_id: "c".into(),
            media_ref: "asset".into(),
        },
        &g,
    )
    .unwrap();

    assert!(!res.changed);
    assert_eq!(st.version(), version_before);
    assert!(!st.can_undo());
    assert_eq!(st.timeline.tracks[0].clips[0].media_ref, "asset");
}

#[test]
fn swap_media_is_undoable() {
    let v = video_track("v", true, vec![clip("c", 0, 100)]);
    let entries = vec![
        media_entry("old", ClipType::Video, 100.0 / 30.0),
        media_entry("new", ClipType::Video, 100.0 / 30.0),
    ];
    let mut st = state_with_media(vec![v], entries);
    let g = SeqIdGen::default();

    apply(
        &mut st,
        EditCommand::SwapMedia {
            clip_id: "c".into(),
            media_ref: "new".into(),
        },
        &g,
    )
    .unwrap();
    assert_eq!(st.timeline.tracks[0].clips[0].media_ref, "new");
    assert!(st.can_undo());

    // Undo via the command (undo() is pub(crate), so we route through apply).
    apply(&mut st, EditCommand::Undo, &g).unwrap();
    assert_eq!(st.timeline.tracks[0].clips[0].media_ref, "asset"); // restored
}

#[test]
fn swap_media_cascades_to_link_group_with_same_ref() {
    // A linked V1/A1 pair both reference "old". Swapping the video clip must
    // also swap the audio clip's ref so the pair stays in sync.
    let mut vc = clip("v", 0, 100);
    vc.media_type = ClipType::Video;
    vc.source_clip_type = ClipType::Video;
    vc.link_group_id = Some("g1".into());
    let mut ac = clip("a", 0, 100);
    ac.media_type = ClipType::Audio;
    ac.source_clip_type = ClipType::Audio;
    ac.link_group_id = Some("g1".into());
    let v = video_track("v", true, vec![vc]);
    let a = audio_track("a", true, vec![ac]);
    let entries = vec![
        media_entry("old", ClipType::Video, 100.0 / 30.0),
        media_entry("new_v", ClipType::Video, 100.0 / 30.0),
    ];
    let mut st = state_with_media(vec![v, a], entries);
    let g = SeqIdGen::default();

    let res = apply(
        &mut st,
        EditCommand::SwapMedia {
            clip_id: "v".into(),
            media_ref: "new_v".into(),
        },
        &g,
    )
    .unwrap();

    assert!(res.changed);
    // Both V1 and A1 updated.
    let v_clip = st
        .find_clip("v")
        .map(|l| &st.timeline.tracks[l.track_index].clips[l.clip_index])
        .unwrap();
    let a_clip = st
        .find_clip("a")
        .map(|l| &st.timeline.tracks[l.track_index].clips[l.clip_index])
        .unwrap();
    assert_eq!(v_clip.media_ref, "new_v");
    assert_eq!(a_clip.media_ref, "new_v");

    // Undo restores both.
    apply(&mut st, EditCommand::Undo, &g).unwrap();
    let v_clip = st
        .find_clip("v")
        .map(|l| &st.timeline.tracks[l.track_index].clips[l.clip_index])
        .unwrap();
    let a_clip = st
        .find_clip("a")
        .map(|l| &st.timeline.tracks[l.track_index].clips[l.clip_index])
        .unwrap();
    assert_eq!(v_clip.media_ref, "asset");
    assert_eq!(a_clip.media_ref, "asset");
}

#[test]
fn swap_media_does_not_cascade_to_link_group_with_different_ref() {
    // V1 references "old", A1 (its linked partner) references a DIFFERENT
    // asset. Swapping V1 must NOT touch A1 — the swap is only meant to
    // update clips that share the old ref.
    let mut vc = clip("v", 0, 100);
    vc.media_type = ClipType::Video;
    vc.source_clip_type = ClipType::Video;
    vc.link_group_id = Some("g1".into());
    let mut ac = clip("a", 0, 100);
    ac.media_type = ClipType::Audio;
    ac.source_clip_type = ClipType::Audio;
    ac.link_group_id = Some("g1".into());
    ac.media_ref = "other".into();
    let v = video_track("v", true, vec![vc]);
    let a = audio_track("a", true, vec![ac]);
    let entries = vec![
        media_entry("old", ClipType::Video, 100.0 / 30.0),
        media_entry("other", ClipType::Audio, 100.0 / 30.0),
        media_entry("new_v", ClipType::Video, 100.0 / 30.0),
    ];
    let mut st = state_with_media(vec![v, a], entries);
    let g = SeqIdGen::default();

    apply(
        &mut st,
        EditCommand::SwapMedia {
            clip_id: "v".into(),
            media_ref: "new_v".into(),
        },
        &g,
    )
    .unwrap();

    let v_clip = st
        .find_clip("v")
        .map(|l| &st.timeline.tracks[l.track_index].clips[l.clip_index])
        .unwrap();
    let a_clip = st
        .find_clip("a")
        .map(|l| &st.timeline.tracks[l.track_index].clips[l.clip_index])
        .unwrap();
    assert_eq!(v_clip.media_ref, "new_v");
    assert_eq!(a_clip.media_ref, "other"); // untouched
}
