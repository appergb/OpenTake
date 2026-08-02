use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use opentake_agent::mcp::core_handle::CoreHandle;
use opentake_agent::mcp::dispatch::Dispatcher;
use opentake_agent::plugin::registry::PluginRegistry;
use opentake_domain::{Clip, ClipType, MediaManifest, Timeline, Track};
use opentake_ops::{apply as ops_apply, EditCommand, EditResult, EditorState, SeqIdGen};

struct RecordingCore {
    state: Mutex<EditorState>,
    apply_calls: AtomicUsize,
}

impl RecordingCore {
    fn new() -> Self {
        let mut timeline = Timeline::new();
        let mut track = Track::new("video-track", ClipType::Video);
        track.clips.push(Clip::new("clip-a", "asset-a", 0, 30));
        timeline.tracks.push(track);
        Self {
            state: Mutex::new(EditorState::new(timeline, MediaManifest::new())),
            apply_calls: AtomicUsize::new(0),
        }
    }
}

impl CoreHandle for RecordingCore {
    fn timeline(&self) -> Timeline {
        self.state.lock().expect("state lock").timeline.clone()
    }

    fn media(&self) -> MediaManifest {
        self.state.lock().expect("state lock").manifest.clone()
    }

    fn apply(&self, command: EditCommand) -> anyhow::Result<EditResult> {
        self.apply_calls.fetch_add(1, Ordering::SeqCst);
        ops_apply(
            &mut self.state.lock().expect("state lock"),
            command,
            &SeqIdGen::new("contract-"),
        )
        .map_err(|error| anyhow::anyhow!(error.to_string()))
    }

    fn project_dir(&self) -> Option<PathBuf> {
        None
    }
}

#[test]
fn completion_43312d5e9f613913_tauri_exposes_typed_core_edit_commands_with_stab() {
    let core = Arc::new(RecordingCore::new());
    let dispatcher = Dispatcher::new(core.clone(), Arc::new(RwLock::new(PluginRegistry::new())));
    let before = core.timeline();

    let malformed = dispatcher.dispatch(
        "remove_clips",
        serde_json::json!({"clipIds": "not-an-array", "hostPath": "/private/secret"}),
    );
    assert!(malformed.is_error);
    assert_eq!(core.apply_calls.load(Ordering::SeqCst), 0);
    assert_eq!(core.timeline(), before);
    assert!(!malformed.text_joined().contains("/private/secret"));

    let valid = dispatcher.dispatch("remove_clips", serde_json::json!({"clipIds": ["clip-a"]}));
    assert!(!valid.is_error, "{}", valid.text_joined());
    assert_eq!(core.apply_calls.load(Ordering::SeqCst), 1);
    assert!(core
        .timeline()
        .tracks
        .iter()
        .all(|track| track.clips.is_empty()));
}
