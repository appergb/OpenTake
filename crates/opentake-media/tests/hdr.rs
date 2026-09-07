use opentake_domain::MediaColorMetadata;
use opentake_media::{
    decode_frame_at, hdr_tonemap_filter, parse_probe, probe, ExportPreset, ExportResolution,
    FrameRequest, VideoCodec, VideoEncoder,
};
use serde_json::json;

#[test]
fn hdr_probe_and_sdr_delivery_policy_preserve_source_metadata() {
    for (transfer, expected_token) in [("smpte2084", "smpte2084"), ("arib-std-b67", "arib-std-b67")]
    {
        let probe = parse_probe(&json!({
            "streams": [{
                "codec_type": "video",
                "width": 3840,
                "height": 2160,
                "avg_frame_rate": "30/1",
                "color_primaries": "bt2020",
                "color_transfer": transfer,
                "color_space": "bt2020nc",
                "color_range": "tv"
            }],
            "format": {"duration": "5.0"}
        }));

        let color = probe.color.expect("HDR metadata must survive probing");
        assert_eq!(
            color,
            MediaColorMetadata {
                primaries: Some("bt2020".into()),
                transfer: Some(transfer.into()),
                matrix: Some("bt2020nc".into()),
                range: Some("tv".into()),
            }
        );
        assert!(color.is_hdr());
        let filter = hdr_tonemap_filter(&color).expect("PQ/HLG must choose an explicit tonemap");
        if cfg!(target_os = "macos") {
            assert!(filter.contains("scale_vt="));
            assert!(filter.contains("color_transfer=bt709"));
            assert!(filter.contains("hwdownload,format=p010le"));
        } else {
            assert!(filter.contains(expected_token));
            assert!(filter.contains("tonemap="));
            assert!(filter.contains("p=bt709:t=bt709:m=bt709"));
        }
    }

    let preset = ExportPreset::new(VideoCodec::H265, ExportResolution::P1080);
    let args = preset.color_args();
    assert!(args
        .windows(2)
        .any(|pair| pair == ["-color_primaries", "bt709"]));
    assert!(args.windows(2).any(|pair| pair == ["-color_trc", "bt709"]));
    assert!(args.windows(2).any(|pair| pair == ["-colorspace", "bt709"]));
}

#[test]
fn sdr_or_unknown_transfer_does_not_apply_hdr_tonemapping() {
    let sdr = MediaColorMetadata {
        primaries: Some("bt709".into()),
        transfer: Some("bt709".into()),
        matrix: Some("bt709".into()),
        range: Some("tv".into()),
    };
    assert!(!sdr.is_hdr());
    assert_eq!(hdr_tonemap_filter(&sdr), None);
    assert_eq!(hdr_tonemap_filter(&MediaColorMetadata::default()), None);
}

#[test]
fn packaged_hdr_decode_path_materializes_bt709_rgba_pixels() {
    if !opentake_media::ffmpeg_status::ffmpeg_available()
        || !opentake_media::ffmpeg_status::ffprobe_available()
    {
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("pq.mp4");
    let generated = std::process::Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=160x90:rate=24",
            "-frames:v",
            "1",
            "-vf",
            "format=yuv420p10le",
            "-c:v",
            "libx265",
            "-preset",
            "ultrafast",
            "-x265-params",
            "log-level=error:hdr-opt=1:repeat-headers=1:colorprim=bt2020:transfer=smpte2084:colormatrix=bt2020nc",
            "-color_primaries",
            "bt2020",
            "-color_trc",
            "smpte2084",
            "-colorspace",
            "bt2020nc",
        ])
        .arg(&source)
        .output()
        .unwrap();
    assert!(
        generated.status.success(),
        "generate HDR fixture: {}",
        String::from_utf8_lossy(&generated.stderr)
    );
    let metadata = probe(&source).unwrap();
    assert!(metadata.color.as_ref().is_some_and(|color| color.is_hdr()));

    let (_, frame) = decode_frame_at(
        &source,
        &FrameRequest {
            time_secs: 0.0,
            max_size: (160, 90),
            ..FrameRequest::default()
        },
    )
    .expect("platform HDR conversion must decode to RGBA");
    assert_eq!((frame.width, frame.height), (160, 90));
    let (min, max) = frame
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|pixel| pixel[..3].iter().copied())
        .fold((u8::MAX, u8::MIN), |(min, max), value| {
            (min.min(value), max.max(value))
        });
    assert!(
        max.saturating_sub(min) > 32,
        "tone-mapped frame must retain contrast"
    );

    let delivered = temp.path().join("delivery.mp4");
    let preset = ExportPreset::new(VideoCodec::H264, ExportResolution::P720);
    let mut encoder = VideoEncoder::new(&delivered, frame.width, frame.height, 1, &preset).unwrap();
    encoder.push_frame(&frame).unwrap();
    encoder.finish().unwrap();
    let tags = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=color_primaries,color_transfer,color_space",
            "-of",
            "json",
        ])
        .arg(&delivered)
        .output()
        .unwrap();
    assert!(tags.status.success());
    let tags: serde_json::Value = serde_json::from_slice(&tags.stdout).unwrap();
    assert_eq!(
        tags.pointer("/streams/0/color_primaries"),
        Some(&json!("bt709"))
    );
    assert_eq!(
        tags.pointer("/streams/0/color_transfer"),
        Some(&json!("bt709"))
    );
    assert_eq!(
        tags.pointer("/streams/0/color_space"),
        Some(&json!("bt709"))
    );
}
