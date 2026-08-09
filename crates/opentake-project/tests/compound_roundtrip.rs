//! Persistence and compatibility boundaries for editable compound clips.

mod common;

use opentake_domain::{Clip, ClipType, NestedSequence, Timeline, Track};
use opentake_project::{Project, ProjectError};

use common::{write_file, TempDir};

fn nested_timeline() -> Timeline {
    let mut child = Timeline::new();
    let mut child_track = Track::new("child-track", ClipType::Video);
    child_track
        .clips
        .push(Clip::new("child-clip", "asset-a", 2, 12));
    child.tracks.push(child_track);

    let mut root = Timeline::new();
    root.nested_sequences
        .push(NestedSequence::new("sequence-a", "Scene A", child));
    let mut root_track = Track::new("root-track", ClipType::Video);
    root_track
        .clips
        .push(Clip::new_nested("compound-a", "sequence-a", 10, 20));
    root.tracks.push(root_track);
    root
}

#[test]
fn compound_clip_roundtrips_nested_timeline() {
    let temp = TempDir::new("compound-roundtrip");
    let bundle = temp.child("Compound.opentake");
    let mut project = Project::new(&bundle);
    project.timeline = nested_timeline();

    project.save().expect("save nested timeline");
    let reopened = Project::open(&bundle).expect("open nested timeline");

    assert_eq!(reopened.timeline, project.timeline);
    assert_eq!(
        reopened.timeline.tracks[0].clips[0]
            .nested_sequence_id
            .as_deref(),
        Some("sequence-a")
    );
    reopened
        .timeline
        .validate_nested_sequences()
        .expect("reopened graph stays valid");
}

#[test]
fn nested_future_fields_make_project_read_only() {
    let temp = TempDir::new("compound-future-field");
    let bundle = temp.child("Future.opentake");
    std::fs::create_dir_all(&bundle).unwrap();
    write_file(
        &bundle.join("project.json"),
        br#"{
          "nestedSequences": [{
            "id": "sequence-a",
            "name": "A",
            "timeline": {
              "tracks": [{
                "id": "child-track",
                "type": "video",
                "futureTrackFlag": true,
                "clips": []
              }]
            }
          }],
          "tracks": []
        }"#,
    );

    let project = Project::open(&bundle).expect("unknown field opens read-only");
    assert!(project.compatibility().is_read_only());
    assert!(project.compatibility().blockers().iter().any(|blocker| {
        blocker == "project.json:nestedSequences.0.timeline.tracks.0.futureTrackFlag"
    }));
}

#[test]
fn recursive_nested_graph_fails_open_and_save() {
    let temp = TempDir::new("compound-cycle");
    let bundle = temp.child("Cycle.opentake");
    let mut a = Timeline::new();
    let mut a_track = Track::new("a-track", ClipType::Video);
    a_track.clips.push(Clip::new_nested("a-to-b", "b", 0, 10));
    a.tracks.push(a_track);
    let mut b = Timeline::new();
    let mut b_track = Track::new("b-track", ClipType::Video);
    b_track.clips.push(Clip::new_nested("b-to-a", "a", 0, 10));
    b.tracks.push(b_track);

    let mut project = Project::new(&bundle);
    project.timeline.nested_sequences = vec![
        NestedSequence::new("a", "A", a),
        NestedSequence::new("b", "B", b),
    ];
    let error = project.save().expect_err("cycle must not be persisted");
    assert!(matches!(error, ProjectError::InvalidTimeline { .. }));
    assert!(
        !bundle.exists(),
        "failed preflight must not create a bundle"
    );

    std::fs::create_dir_all(&bundle).unwrap();
    write_file(
        &bundle.join("project.json"),
        serde_json::to_string(&project.timeline).unwrap().as_bytes(),
    );
    let error = Project::open(&bundle).expect_err("cycle must fail open");
    assert!(matches!(error, ProjectError::InvalidTimeline { .. }));
    assert!(error.to_string().contains("a -> b -> a"));
}
