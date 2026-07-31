use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use opentake_agent::mcp::core_handle::CoreHandle;
use opentake_agent::mcp::dispatch::Dispatcher;
use opentake_agent::plugin::registry::PluginRegistry;
use opentake_domain::{
    Clip, ClipType, Crop, MediaManifest, MediaManifestEntry, MediaSource, Timeline, Track,
};
use opentake_media::analysis::{
    detect_autocrop, detect_beats, AutocropConfig, BeatDetectionConfig, FrameBuffer, PixelFormat,
};
use opentake_media::{PcmBuffer, PcmFormat, PcmSpec};
use opentake_ops::intent::{plan_beat_sync_placement, plan_smart_reframe, IntentClipEntry};
use opentake_ops::{apply as ops_apply, EditCommand, EditResult, EditorState, SeqIdGen};

struct AutomationHandle {
    state: Mutex<EditorState>,
    pcm: PcmBuffer,
    apply_calls: AtomicUsize,
}

impl AutomationHandle {
    fn new() -> Self {
        let mut timeline = Timeline::new();
        timeline.fps = 10;
        let mut track = Track::new("video-track", ClipType::Video);
        track.clips.push(Clip::new("clip-a", "asset-1", 0, 10));
        timeline.tracks.push(track);

        let mut manifest = MediaManifest::new();
        manifest.entries.push(MediaManifestEntry {
            id: "asset-1".into(),
            name: "Source.mov".into(),
            kind: ClipType::Video,
            source: MediaSource::External {
                absolute_path: "/fixture/Source.mov".into(),
            },
            duration: 1.0,
            generation_input: None,
            source_width: Some(1920),
            source_height: Some(1080),
            source_fps: Some(10.0),
            has_audio: Some(true),
            color: None,
            proxy: None,
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        });

        let mut samples = vec![0.5; 300];
        samples.extend(std::iter::repeat_n(0.0, 400));
        samples.extend(std::iter::repeat_n(0.5, 300));
        Self {
            state: Mutex::new(EditorState::new(timeline, manifest)),
            pcm: PcmBuffer {
                spec: PcmSpec {
                    sample_rate: 1_000,
                    channels: 1,
                    format: PcmFormat::F32,
                },
                samples_f32: samples,
            },
            apply_calls: AtomicUsize::new(0),
        }
    }
}

impl CoreHandle for AutomationHandle {
    fn timeline(&self) -> Timeline {
        self.state.lock().expect("state lock").timeline.clone()
    }

    fn media(&self) -> MediaManifest {
        self.state.lock().expect("state lock").manifest.clone()
    }

    fn apply(&self, command: EditCommand) -> anyhow::Result<EditResult> {
        self.apply_calls.fetch_add(1, Ordering::AcqRel);
        let ids = SeqIdGen::new("automation-");
        ops_apply(&mut self.state.lock().expect("state lock"), command, &ids)
            .map_err(|error| anyhow::anyhow!(error.to_string()))
    }

    fn project_dir(&self) -> Option<PathBuf> {
        None
    }

    fn extract_analysis_pcm(
        &self,
        _media_ref: &str,
        _spec: PcmSpec,
        _range: Option<(f64, f64)>,
    ) -> anyhow::Result<PcmBuffer> {
        Ok(self.pcm.clone())
    }
}

fn first_json(result: &opentake_agent::tools::result::ToolResult) -> serde_json::Value {
    let text = match &result.content[0] {
        opentake_agent::tools::result::Block::Text { text } => text,
        other => panic!("expected JSON text block, got {other:?}"),
    };
    serde_json::from_str(text).expect("valid tool JSON")
}

#[test]
fn automation_children_are_atomic_reviewable_and_command_routed() {
    // The media children are deterministic and reject malformed input without
    // producing a partial proposal.
    let beat_config = BeatDetectionConfig {
        sample_rate: 1_000,
        fps: 10.0,
        window_size_samples: 100,
        hop_size_samples: 100,
        min_onset_strength: 0.05,
        min_gap_frames: 1,
    };
    let mut pulse = vec![0.0; 1_000];
    pulse[500..530].fill(1.0);
    assert_eq!(
        detect_beats(&pulse, beat_config),
        detect_beats(&pulse, beat_config)
    );
    assert!(detect_beats(
        &pulse,
        BeatDetectionConfig {
            sample_rate: 0,
            ..beat_config
        }
    )
    .is_empty());

    let pixels = [0_u8, 0, 0, 255, 255, 255];
    let valid_frame = FrameBuffer {
        width: 2,
        height: 1,
        data: &pixels,
        pixel_format: PixelFormat::Rgb,
    };
    assert_eq!(
        detect_autocrop(&valid_frame, AutocropConfig::default()),
        detect_autocrop(&valid_frame, AutocropConfig::default())
    );
    let truncated = FrameBuffer {
        data: &pixels[..3],
        ..valid_frame
    };
    assert_eq!(detect_autocrop(&truncated, AutocropConfig::default()), None);

    let handle = Arc::new(AutomationHandle::new());
    let dispatcher = Dispatcher::new(handle.clone(), Arc::new(RwLock::new(PluginRegistry::new())));
    let before = handle.timeline();

    // Analysis surfaces return reviewable proposals or typed diagnostics. They
    // never reach the edit boundary themselves.
    let beats = dispatcher.dispatch("detect_beats", serde_json::json!({"mediaRef": "asset-1"}));
    assert!(!beats.is_error, "{}", beats.text_joined());
    assert_eq!(first_json(&beats)["applied"], false);

    let silences = dispatcher.dispatch(
        "tighten_silences",
        serde_json::json!({
            "clipIds": ["clip-a"],
            "thresholdDb": -40.0,
            "minSilenceFrames": 2,
            "paddingFrames": 0
        }),
    );
    assert!(!silences.is_error, "{}", silences.text_joined());
    let silence_json = first_json(&silences);
    assert_eq!(silence_json["applied"], false);
    assert_eq!(silence_json["commands"][0]["tool"], "ripple_delete_ranges");

    let unavailable = dispatcher.dispatch(
        "smart_reframe",
        serde_json::json!({"clipIds": ["clip-a"], "aspectRatio": "9:16"}),
    );
    assert!(unavailable.is_error);
    assert!(unavailable.text_joined().contains("needs vision"));
    assert_eq!(handle.apply_calls.load(Ordering::Acquire), 0);
    assert_eq!(handle.timeline(), before);

    // A valid write is normalized to exactly one existing EditCommand. The
    // command boundary owns the mutation and its single undo restores the exact
    // prior timeline.
    let crop = Crop {
        left: 0.1,
        top: 0.0,
        right: 0.1,
        bottom: 0.0,
    };
    let plan = plan_smart_reframe(&["clip-a".into()], crop, None).expect("reframe plan");
    assert_eq!(plan.label, "smart_reframe");
    assert_eq!(plan.commands.len(), 1);
    let applied = handle
        .apply(plan.commands[0].clone())
        .expect("atomic command");
    assert!(applied.changed);
    assert_eq!(handle.apply_calls.load(Ordering::Acquire), 1);
    assert_eq!(handle.timeline().tracks[0].clips[0].crop, crop);
    handle.apply(EditCommand::Undo).expect("single undo");
    assert_eq!(handle.timeline(), before);

    // Rejected plans are typed, remain command-free, and preserve state.
    let rejected = plan_smart_reframe(&[], crop, None).expect_err("empty clip ids must fail");
    assert!(rejected.to_string().contains("empty clipIds"));
    assert_eq!(handle.timeline(), before);

    let entry = IntentClipEntry {
        media_ref: "asset-1".into(),
        media_type: ClipType::Video,
        source_clip_type: ClipType::Video,
        track_index: None,
        start_frame: 0,
        duration_frames: 5,
        trim_start_frame: None,
        trim_end_frame: None,
        has_audio: true,
        add_linked_audio: true,
        transform: None,
    };
    let beat_plan =
        plan_beat_sync_placement(&before, vec![entry.clone()], &[3]).expect("beat placement plan");
    assert_eq!(beat_plan.commands.len(), 1);
    assert!(matches!(
        beat_plan.commands[0],
        EditCommand::AddClipsAutoTrack { .. }
    ));
    let bad_beats =
        plan_beat_sync_placement(&before, vec![entry], &[]).expect_err("missing beat must fail");
    assert!(bad_beats.to_string().contains("Need at least 1 beat"));
    assert_eq!(handle.timeline(), before);
}
