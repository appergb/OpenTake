use opentake_domain::{Clip, ClipType, Crop, Timeline, Track, Transform};
use opentake_ops::intent::{
    plan_auto_track_add, plan_beat_sync_placement, plan_ripple_delete_range, plan_smart_reframe,
    plan_trim_to_playhead, IntentClipEntry,
};
use opentake_ops::{EditCommand, TrimEdge};

fn clip(id: &str, start: i32, dur: i32) -> Clip {
    Clip::new(id, "asset", start, dur)
}

fn track(id: &str, kind: ClipType, clips: Vec<Clip>) -> Track {
    let mut t = Track::new(id, kind);
    t.clips = clips;
    t
}

fn intent_entry(track_index: Option<usize>, media_type: ClipType, start: i32) -> IntentClipEntry {
    IntentClipEntry {
        media_ref: "asset".into(),
        media_type,
        source_clip_type: media_type,
        track_index,
        start_frame: start,
        duration_frames: 30,
        trim_start_frame: None,
        trim_end_frame: None,
        has_audio: false,
        add_linked_audio: false,
        transform: None,
    }
}

#[test]
fn auto_track_add_on_empty_timeline_plans_insert_then_add_on_new_track() {
    let timeline = Timeline::new();

    let plan = plan_auto_track_add(&timeline, vec![intent_entry(None, ClipType::Video, 12)])
        .expect("auto-track add plan");

    assert_eq!(plan.label, "auto_track_add");
    assert!(plan.warnings.is_empty());
    assert_eq!(plan.commands.len(), 1);
    match plan.commands[0].clone() {
        EditCommand::AddClipsAutoTrack { entries } => {
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].track_index, 0);
            assert_eq!(entries[0].start_frame, 12);
        }
        other => panic!("expected AddClipsAutoTrack, got {other:?}"),
    }
}

#[test]
fn auto_track_add_with_explicit_track_index_uses_add_clips() {
    let mut timeline = Timeline::new();
    timeline.tracks = vec![
        track("v1", ClipType::Video, vec![]),
        track("a1", ClipType::Audio, vec![]),
    ];

    let plan = plan_auto_track_add(&timeline, vec![intent_entry(Some(0), ClipType::Image, 0)])
        .expect("auto-track add plan");

    assert_eq!(plan.commands.len(), 1);
    match plan.commands[0].clone() {
        EditCommand::AddClips { entries } => {
            assert_eq!(entries[0].track_index, 0);
            assert_eq!(entries[0].source_clip_type, ClipType::Image);
        }
        other => panic!("expected AddClips, got {other:?}"),
    }
}

#[test]
fn auto_track_add_mixed_audio_video_uses_atomic_command_without_precomputed_indexes() {
    let timeline = Timeline::new();

    let plan = plan_auto_track_add(
        &timeline,
        vec![
            intent_entry(None, ClipType::Audio, 0),
            intent_entry(None, ClipType::Video, 10),
        ],
    )
    .expect("auto-track add plan");

    assert_eq!(plan.commands.len(), 1);
    match plan.commands[0].clone() {
        EditCommand::AddClipsAutoTrack { entries } => {
            assert_eq!(entries.len(), 2);
            assert_eq!(entries[0].source_clip_type, ClipType::Audio);
            assert_eq!(entries[1].source_clip_type, ClipType::Video);
        }
        other => panic!("expected AddClipsAutoTrack, got {other:?}"),
    }
}

#[test]
fn trim_to_playhead_plans_source_frame_trim_for_left_edge() {
    let mut timeline = Timeline::new();
    let mut c = clip("c1", 100, 60);
    c.trim_start_frame = 5;
    timeline.tracks.push(track("v1", ClipType::Video, vec![c]));

    let plan = plan_trim_to_playhead(&timeline, &["c1".to_string()], 130, TrimEdge::Left)
        .expect("trim plan");

    match plan.commands[0].clone() {
        EditCommand::TrimClips { edits } => {
            assert_eq!(edits, vec![("c1".to_string(), 35, 0)]);
        }
        other => panic!("expected TrimClips, got {other:?}"),
    }
}

#[test]
fn ripple_delete_range_plans_half_open_range_command() {
    let plan = plan_ripple_delete_range(2, 10, 25).expect("ripple plan");

    match plan.commands[0].clone() {
        EditCommand::RippleDeleteRanges {
            track_index,
            ranges,
        } => {
            assert_eq!(track_index, 2);
            assert_eq!(ranges[0].start, 10);
            assert_eq!(ranges[0].end, 25);
        }
        other => panic!("expected RippleDeleteRanges, got {other:?}"),
    }
}

#[test]
fn beat_sync_placement_sets_entry_start_frames_from_beats_then_auto_tracks() {
    let timeline = Timeline::new();

    let plan = plan_beat_sync_placement(
        &timeline,
        vec![
            intent_entry(None, ClipType::Video, 999),
            intent_entry(None, ClipType::Video, 999),
        ],
        &[12, 42],
    )
    .expect("beat plan");

    assert_eq!(plan.commands.len(), 1);
    match plan.commands[0].clone() {
        EditCommand::AddClipsAutoTrack { entries } => {
            assert_eq!(entries[0].start_frame, 12);
            assert_eq!(entries[1].start_frame, 42);
            assert_eq!(entries[0].track_index, 0);
            assert_eq!(entries[1].track_index, 0);
        }
        other => panic!("expected AddClipsAutoTrack, got {other:?}"),
    }
}

#[test]
fn smart_reframe_plans_crop_and_transform_properties() {
    let crop = Crop {
        left: 0.1,
        top: 0.0,
        right: 0.1,
        bottom: 0.0,
    };
    let transform = Transform {
        center_x: 0.5,
        center_y: 0.5,
        width: 0.75,
        height: 1.0,
        rotation: 0.0,
        flip_horizontal: false,
        flip_vertical: false,
    };

    let plan =
        plan_smart_reframe(&["c1".to_string()], crop, Some(transform)).expect("reframe plan");

    match plan.commands[0].clone() {
        EditCommand::SetClipProperties {
            clip_ids,
            properties,
        } => {
            assert_eq!(clip_ids, vec!["c1".to_string()]);
            assert_eq!(properties.crop, Some(crop));
            assert_eq!(properties.transform, Some(transform));
        }
        other => panic!("expected SetClipProperties, got {other:?}"),
    }
}
