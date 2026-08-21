use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use opentake_media::{
    AssetIndex, ExportPreset, ExportResolution, FrameRequest, Header, MediaEngine, PcmBuffer,
    PcmFormat, PcmSpec, Row, TranscribeOptions, Transcriber, TranscriptionResult, VideoCodec,
};

struct FixtureTranscriber;

impl Transcriber for FixtureTranscriber {
    fn transcribe_pcm(
        &self,
        pcm: &PcmBuffer,
        _opts: &TranscribeOptions,
    ) -> opentake_media::Result<TranscriptionResult> {
        Ok(TranscriptionResult {
            text: format!("{} samples", pcm.samples_f32.len()),
            language: Some("en".into()),
            words: vec![],
            segments: vec![],
        })
    }
}

fn manifest(crate_name: &str) -> String {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("media crate belongs to the workspace");
    std::fs::read_to_string(workspace.join("crates").join(crate_name).join("Cargo.toml"))
        .expect("read workspace crate manifest")
}

fn make_av_fixture(path: &Path) -> bool {
    Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=0x336699:s=32x18:r=4",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=16000",
            "-t",
            "1",
            "-c:v",
            "mpeg4",
            "-c:a",
            "aac",
            "-y",
        ])
        .arg(path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[test]
fn all_services_are_reachable_only_through_facade_and_dependencies_stay_acyclic() {
    // The production dependency direction is domain <- media. Neither the
    // zero-IO domain leaf nor media imports core/render back upward; render may
    // consume media at its adapter/test boundary without creating a cycle.
    let media_manifest = manifest("opentake-media");
    let domain_manifest = manifest("opentake-domain");
    let render_manifest = manifest("opentake-render");
    let core_manifest = manifest("opentake-core");
    assert!(media_manifest.contains("opentake-domain = { workspace = true }"));
    assert!(!media_manifest.contains("opentake-core"));
    assert!(!media_manifest.contains("opentake-render"));
    assert!(!domain_manifest.contains("opentake-media"));
    assert!(render_manifest.contains("opentake-media = { workspace = true }"));
    assert!(!core_manifest.contains("opentake-render"));

    let temp = tempfile::tempdir().unwrap();
    let engine = MediaEngine::new(temp.path().join("cache"), temp.path().join("models"));

    // Search is a real facade operation over the persisted index value model.
    let indexes = vec![(
        "asset-a".to_string(),
        AssetIndex {
            header: Header {
                model: "fixture".into(),
                model_version: 1,
                sampler_version: 1,
                dim: 2,
                count: 1,
            },
            rows: vec![Row {
                time: 0.25,
                shot_start: 0.0,
                shot_end: 1.0,
            }],
            vectors: vec![1.0, 0.0],
        },
    )];
    let hits = engine.search_visual(&[1.0, 0.0], &indexes, 20, 0.85, Some(0.05));
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].asset_id, "asset-a");

    // The exact IO methods must compile on every platform. Environments without
    // ffmpeg stop after the pure dependency/search assertions.
    let source = temp.path().join("facade-source.mp4");
    if !make_av_fixture(&source) {
        return;
    }

    let probe = engine.probe(&source).unwrap();
    assert_eq!((probe.width, probe.height), (Some(32), Some(18)));
    assert!(probe.has_audio && probe.has_video);

    let frame = engine
        .decode_frame(
            &source,
            &FrameRequest {
                time_secs: 0.25,
                max_size: (32, 18),
                tolerance_secs: 0.25,
                apply_rotation: true,
            },
        )
        .unwrap()
        .1;
    assert_eq!((frame.width, frame.height), (32, 18));

    let pcm_spec = PcmSpec {
        sample_rate: 16_000,
        channels: 1,
        format: PcmFormat::F32,
    };
    let pcm = engine.extract_pcm(&source, &pcm_spec, None).unwrap();
    assert_eq!(pcm.spec, pcm_spec);
    assert!(
        (15_000..=16_500).contains(&pcm.samples_f32.len()),
        "expected ~1s of 16k mono, got {} samples ({:.6}s)",
        pcm.samples_f32.len(),
        pcm.duration_secs()
    );

    let transcript_cache = opentake_media::TranscriptCache::new(temp.path().join("cache"));
    let transcript = engine
        .transcribe(&source, true, None, &FixtureTranscriber, &transcript_cache)
        .unwrap();
    assert!(transcript.text.ends_with(" samples"));

    let encoded = temp.path().join("facade-encoded.mp4");
    let mut encoder = engine
        .video_encoder(
            &encoded,
            32,
            18,
            4,
            &ExportPreset::new(VideoCodec::H264, ExportResolution::P720),
        )
        .unwrap();
    encoder.push_frame(&frame).unwrap();
    encoder.finish().unwrap();
    assert!(encoded.is_file());
    assert!(engine.probe(&encoded).unwrap().has_video);

    let _: PathBuf = engine.cache_root().to_path_buf();
}
