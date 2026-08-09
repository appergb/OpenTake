//! Media probing — the ffprobe equivalent of upstream `MediaAsset.loadMetadata`
//! (`MediaAsset.swift:96-162`): duration, rotation-corrected pixel dimensions,
//! frame rate, and audio presence. Header/stream parameters only, no decode.
//!
//! The JSON→`MediaProbe` mapping is a pure function ([`parse_probe`]) so the
//! rotation/duration/fps rules are unit-testable from fixtures without invoking
//! ffprobe.

use std::path::Path;
use std::time::Duration;

use opentake_domain::MediaColorMetadata;

use crate::error::{MediaError, Result};
use crate::ff;

/// Probed media facts. Time in seconds; dimensions already rotation-corrected.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct MediaProbe {
    /// Prefer the video stream duration, falling back to the container duration.
    pub duration_secs: f64,
    /// Display width after applying rotation side-data / display matrix.
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// `avg_frame_rate` (falling back to `r_frame_rate`), matching
    /// `nominalFrameRate` semantics.
    pub fps: Option<f64>,
    pub has_audio: bool,
    pub has_video: bool,
    /// ffprobe `codec_name` of the primary (non-cover-art) video stream.
    /// Post-encode verification compares this against the requested encoder.
    pub video_codec: Option<String>,
    /// ffprobe `codec_name` of the first audio stream.
    pub audio_codec: Option<String>,
    /// ffprobe's comma-separated demuxer names (for example
    /// `mov,mp4,m4a,3gp,3g2,mj2`). Security-sensitive import boundaries use
    /// this to verify that downloaded bytes match their declared container.
    pub format_name: Option<String>,
    /// Source video color signalling retained for HDR-aware decode and durable
    /// project metadata. Absent only when the stream reports no color fields.
    pub color: Option<MediaColorMetadata>,
}

/// Open the container and read the first video stream + audio presence.
pub fn probe(path: &Path) -> Result<MediaProbe> {
    if !path.exists() {
        return Err(MediaError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            path.display().to_string(),
        )));
    }
    let json = ff::ffprobe_json(path)?;
    Ok(parse_probe(&json))
}

/// Probe an already-open regular file. No path fallback is attempted: callers
/// use this when the open handle is the authority that must survive namespace
/// rebinding.
pub fn probe_file(file: &std::fs::File) -> Result<MediaProbe> {
    let json = ff::ffprobe_json_file(file)?;
    Ok(parse_probe(&json))
}

/// Probe a retained regular file with cooperative cancellation and a hard
/// helper-process deadline.
pub fn probe_file_cancellable(
    file: &std::fs::File,
    cancel: &crate::MediaCancelToken,
    timeout: Duration,
) -> Result<MediaProbe> {
    let json = ff::ffprobe_json_file_cancellable(file, cancel, timeout)?;
    Ok(parse_probe(&json))
}

/// Parse the rate string ffprobe emits, e.g. `"30000/1001"` or `"25/1"`.
/// `"0/0"` (unknown) → `None`.
fn parse_rate(s: &str) -> Option<f64> {
    let (num, den) = s.split_once('/')?;
    let num: f64 = num.trim().parse().ok()?;
    let den: f64 = den.trim().parse().ok()?;
    if den == 0.0 || num == 0.0 {
        return None;
    }
    Some(num / den)
}

/// Extract a rotation in degrees from a stream's `tags.rotate` or
/// `side_data_list[*].rotation`. ffprobe reports display-matrix rotation as a
/// (often negative) angle; we fold to a non-negative multiple of 90.
fn stream_rotation(stream: &serde_json::Value) -> i64 {
    // tags.rotate (string)
    if let Some(r) = stream
        .get("tags")
        .and_then(|t| t.get("rotate"))
        .and_then(|v| v.as_str())
        .and_then(|s| s.trim().parse::<i64>().ok())
    {
        return ((r % 360) + 360) % 360;
    }
    // side_data_list[*].rotation (number)
    if let Some(list) = stream.get("side_data_list").and_then(|v| v.as_array()) {
        for sd in list {
            if let Some(rot) = sd.get("rotation").and_then(|v| v.as_f64()) {
                let r = rot.round() as i64;
                return ((r % 360) + 360) % 360;
            }
        }
    }
    0
}

/// Pure JSON → `MediaProbe`. Implements upstream's rules:
/// rotation 90/270 swaps W/H; duration prefers the video stream then container;
/// fps uses `avg_frame_rate` then `r_frame_rate`.
pub fn parse_probe(json: &serde_json::Value) -> MediaProbe {
    let streams = json
        .get("streams")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    // Cover art in an audio container is reported as a video stream with
    // `disposition.attached_pic = 1`. It is metadata, not playable video:
    // treating it as the primary video stream makes valid MP3/M4A imports look
    // like videos and gives them the cover's dimensions and frame rate.
    let video = streams.iter().find(|stream| {
        stream.get("codec_type").and_then(|value| value.as_str()) == Some("video")
            && stream
                .pointer("/disposition/attached_pic")
                .and_then(|value| value.as_i64())
                != Some(1)
    });
    let has_video = video.is_some();
    // An audio stream that reports zero channels carries no real sound (an
    // empty/placeholder track some exporters add). Treating it as "has audio"
    // makes a dropped video spawn a phantom linked audio clip (the user's "no
    // audio but it split" report), so require channels > 0 when reported. Streams
    // that don't report `channels` are kept as audio (conservative default).
    let mut audio_codec = None;
    let has_audio = streams.iter().any(|s| {
        if s.get("codec_type").and_then(|v| v.as_str()) != Some("audio") {
            return false;
        }
        if audio_codec.is_none() {
            audio_codec = s
                .get("codec_name")
                .and_then(|value| value.as_str())
                .map(str::to_owned);
        }
        s.get("channels").and_then(|v| v.as_u64()) != Some(0)
    });

    let mut width = None;
    let mut height = None;
    let mut fps = None;
    let mut video_duration = None;
    let mut color = None;
    let mut video_codec = None;

    if let Some(v) = video {
        video_codec = v
            .get("codec_name")
            .and_then(|value| value.as_str())
            .map(str::to_owned);
        let w = v.get("width").and_then(|x| x.as_u64()).map(|x| x as u32);
        let h = v.get("height").and_then(|x| x.as_u64()).map(|x| x as u32);
        let rot = stream_rotation(v);
        if rot == 90 || rot == 270 {
            width = h;
            height = w;
        } else {
            width = w;
            height = h;
        }

        fps = v
            .get("avg_frame_rate")
            .and_then(|x| x.as_str())
            .and_then(parse_rate)
            .or_else(|| {
                v.get("r_frame_rate")
                    .and_then(|x| x.as_str())
                    .and_then(parse_rate)
            });

        video_duration = v
            .get("duration")
            .and_then(|x| x.as_str())
            .and_then(|s| s.parse::<f64>().ok());

        let metadata = MediaColorMetadata {
            primaries: v
                .get("color_primaries")
                .and_then(|value| value.as_str())
                .map(str::to_owned),
            transfer: v
                .get("color_transfer")
                .and_then(|value| value.as_str())
                .map(str::to_owned),
            matrix: v
                .get("color_space")
                .and_then(|value| value.as_str())
                .map(str::to_owned),
            range: v
                .get("color_range")
                .and_then(|value| value.as_str())
                .map(str::to_owned),
        };
        if !metadata.is_empty() {
            color = Some(metadata);
        }
    }

    let container_duration = json
        .get("format")
        .and_then(|f| f.get("duration"))
        .and_then(|x| x.as_str())
        .and_then(|s| s.parse::<f64>().ok());
    let format_name = json
        .get("format")
        .and_then(|format| format.get("format_name"))
        .and_then(|value| value.as_str())
        .map(str::to_owned);

    let duration_secs = video_duration.or(container_duration).unwrap_or(0.0);

    MediaProbe {
        duration_secs,
        width,
        height,
        fps,
        has_audio,
        has_video,
        video_codec,
        audio_codec,
        format_name,
        color,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn retained_regular_file_probe_rewinds_and_uses_fd_protocol() {
        if !crate::ff::ffmpeg_available() || !crate::ff::ffprobe_available() {
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let media = tmp.path().join("retained.mp4");
        let generated = std::process::Command::new(crate::ff::ffmpeg_path())
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=32x18:r=1",
                "-t",
                "1",
                "-c:v",
                "mpeg4",
                "-y",
            ])
            .arg(&media)
            .status()
            .unwrap();
        assert!(generated.success());
        let mut file = std::fs::File::open(&media).unwrap();
        use std::io::{Seek, SeekFrom};
        file.seek(SeekFrom::End(0)).unwrap();

        let probed = probe_file(&file).expect("ffprobe retained regular file through fd:");

        assert_eq!(probed.width, Some(32));
        assert_eq!(probed.height, Some(18));
        assert!(probed.has_video);
    }

    #[test]
    fn parse_rate_handles_fractions_and_unknown() {
        assert_eq!(parse_rate("30/1"), Some(30.0));
        assert!((parse_rate("30000/1001").unwrap() - 29.970).abs() < 0.001);
        assert_eq!(parse_rate("0/0"), None);
        assert_eq!(parse_rate("25"), None); // no slash
    }

    #[test]
    fn landscape_video_dimensions_unchanged() {
        let j = json!({
            "streams": [{
                "codec_type": "video", "width": 1920, "height": 1080,
                "avg_frame_rate": "30/1", "duration": "12.5"
            }],
            "format": {"duration": "12.6"}
        });
        let p = parse_probe(&j);
        assert_eq!(p.width, Some(1920));
        assert_eq!(p.height, Some(1080));
        assert_eq!(p.fps, Some(30.0));
        assert!(p.has_video && !p.has_audio);
        // video stream duration wins over container.
        assert_eq!(p.duration_secs, 12.5);
    }

    #[test]
    fn codec_names_carried_for_post_encode_verification() {
        let p = parse_probe(&json!({
            "streams": [
                {"codec_type": "video", "codec_name": "h264", "width": 640, "height": 360,
                 "avg_frame_rate": "30/1", "duration": "2.0"},
                {"codec_type": "audio", "codec_name": "aac", "channels": 2}
            ],
            "format": {"duration": "2.0"}
        }));
        assert_eq!(p.video_codec.as_deref(), Some("h264"));
        assert_eq!(p.audio_codec.as_deref(), Some("aac"));
        assert!(p.has_video && p.has_audio);
    }

    #[test]
    fn no_streams_means_no_codec_names() {
        let p = parse_probe(&json!({"streams": [], "format": {}}));
        assert_eq!(p.video_codec, None);
        assert_eq!(p.audio_codec, None);
    }

    #[test]
    fn attached_picture_is_not_a_playable_video_stream() {
        let probe = parse_probe(&json!({
            "streams": [
                {
                    "codec_type": "audio",
                    "channels": 2
                },
                {
                    "codec_type": "video",
                    "width": 600,
                    "height": 600,
                    "disposition": {"attached_pic": 1}
                }
            ],
            "format": {
                "duration": "3.0",
                "format_name": "mp3"
            }
        }));
        assert!(probe.has_audio);
        assert!(!probe.has_video);
        assert_eq!(probe.width, None);
        assert_eq!(probe.height, None);
    }

    #[test]
    fn rotated_90_swaps_dimensions_via_tags() {
        let j = json!({
            "streams": [{
                "codec_type": "video", "width": 1920, "height": 1080,
                "tags": {"rotate": "90"}, "avg_frame_rate": "30/1"
            }],
            "format": {}
        });
        let p = parse_probe(&j);
        assert_eq!(p.width, Some(1080));
        assert_eq!(p.height, Some(1920));
    }

    #[test]
    fn rotated_270_via_side_data_swaps() {
        let j = json!({
            "streams": [{
                "codec_type": "video", "width": 1920, "height": 1080,
                "side_data_list": [{"rotation": -90.0}],
                "avg_frame_rate": "24/1"
            }],
            "format": {}
        });
        // -90 folds to 270 → swap.
        let p = parse_probe(&j);
        assert_eq!(p.width, Some(1080));
        assert_eq!(p.height, Some(1920));
    }

    #[test]
    fn rotated_180_does_not_swap() {
        let j = json!({
            "streams": [{
                "codec_type": "video", "width": 1920, "height": 1080,
                "tags": {"rotate": "180"}, "avg_frame_rate": "30/1"
            }],
            "format": {}
        });
        let p = parse_probe(&j);
        assert_eq!(p.width, Some(1920));
        assert_eq!(p.height, Some(1080));
    }

    #[test]
    fn fps_falls_back_to_r_frame_rate() {
        let j = json!({
            "streams": [{
                "codec_type": "video", "width": 100, "height": 100,
                "avg_frame_rate": "0/0", "r_frame_rate": "25/1"
            }],
            "format": {}
        });
        assert_eq!(parse_probe(&j).fps, Some(25.0));
    }

    #[test]
    fn audio_only_has_no_video_dimensions() {
        let j = json!({
            "streams": [{"codec_type": "audio", "sample_rate": "48000"}],
            "format": {"duration": "60.0"}
        });
        let p = parse_probe(&j);
        assert!(!p.has_video);
        assert!(p.has_audio);
        assert_eq!(p.width, None);
        assert_eq!(p.duration_secs, 60.0);
    }

    #[test]
    fn duration_falls_back_to_container() {
        let j = json!({
            "streams": [{"codec_type": "video", "width": 10, "height": 10, "avg_frame_rate": "30/1"}],
            "format": {"duration": "7.0"}
        });
        // video stream has no duration → container.
        assert_eq!(parse_probe(&j).duration_secs, 7.0);
    }

    #[test]
    fn no_duration_anywhere_is_zero() {
        let j = json!({"streams": [], "format": {}});
        let p = parse_probe(&j);
        assert_eq!(p.duration_secs, 0.0);
        assert!(!p.has_video && !p.has_audio);
    }

    #[test]
    fn video_with_audio_track_flags_both() {
        let j = json!({
            "streams": [
                {"codec_type": "video", "width": 640, "height": 480, "avg_frame_rate": "30/1"},
                {"codec_type": "audio", "sample_rate": "44100"}
            ],
            "format": {"duration": "5.0"}
        });
        let p = parse_probe(&j);
        assert!(p.has_video && p.has_audio);
    }

    #[test]
    fn video_with_zero_channel_audio_has_no_audio() {
        // An empty/placeholder audio stream (0 channels) must not count as audio,
        // so a dropped video does not spawn a phantom linked audio clip.
        let j = json!({
            "streams": [
                {"codec_type": "video", "width": 640, "height": 480, "avg_frame_rate": "30/1"},
                {"codec_type": "audio", "channels": 0}
            ],
            "format": {"duration": "5.0"}
        });
        let p = parse_probe(&j);
        assert!(p.has_video && !p.has_audio);
    }

    #[test]
    fn video_with_multichannel_audio_flags_audio() {
        let j = json!({
            "streams": [
                {"codec_type": "video", "width": 640, "height": 480, "avg_frame_rate": "30/1"},
                {"codec_type": "audio", "channels": 2, "sample_rate": "48000"}
            ],
            "format": {"duration": "5.0"}
        });
        assert!(parse_probe(&j).has_audio);
    }
}
