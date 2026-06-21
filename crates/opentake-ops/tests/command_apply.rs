//! End-to-end command-transaction tests ("对拍"): each exercises a full
//! [`EditCommand`] through [`apply`] against an [`EditorState`], asserting the
//! resulting `Timeline` / `MediaManifest`, undo/redo behavior, versioning, and
//! the refusal path — the behaviors the port must match upstream.

use opentake_domain::{AnimPair, Interpolation, Keyframe, KeyframeTrack};
use opentake_domain::{Clip, ClipType, MediaManifest, Timeline, Track, Transform};
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
