//! `GenerationInput` -> `GenerationParams` assembly. A 1:1 port of the upstream
//! `*GenerationSubmission.buildParams` closures, including the exact URL slicing
//! order. `GenerationInput` lives in `opentake-domain` (it is persisted to the
//! project file); the assembly logic lives here (gen -> domain, one-way).

use crate::catalog::ModelKind;
use crate::params::{
    clamp_num_images, AudioParams, GenerationParams, ImageParams, UpscaleParams, VideoParams,
};
use opentake_domain::GenerationInput;

/// How the video reference uploads are laid out, when not an edit model.
/// Replicates `videoInputURLs` (`VideoGenerationSubmission.swift:289-305`):
/// `frames` first, then image refs, then video refs, then audio refs.
struct VideoSlices {
    frames: Vec<String>,
    image_refs: Vec<String>,
    video_refs: Vec<String>,
    audio_refs: Vec<String>,
}

fn slice_video_uploads(
    uploaded: &[String],
    frame_count: usize,
    image_ref_count: usize,
    video_ref_count: usize,
    audio_ref_count: usize,
) -> VideoSlices {
    let frames: Vec<String> = uploaded.iter().take(frame_count).cloned().collect();
    let rest: Vec<String> = uploaded.iter().skip(frame_count).cloned().collect();
    let image_refs: Vec<String> = rest.iter().take(image_ref_count).cloned().collect();
    let video_refs: Vec<String> = rest
        .iter()
        .skip(image_ref_count)
        .take(video_ref_count)
        .cloned()
        .collect();
    let audio_refs: Vec<String> = rest
        .iter()
        .skip(image_ref_count + video_ref_count)
        .take(audio_ref_count)
        .cloned()
        .collect();
    VideoSlices {
        frames,
        image_refs,
        video_refs,
        audio_refs,
    }
}

/// Build params for a text/image-to-video submission (non-edit). Replicates the
/// `params(...)` builder in `VideoGenerationSubmission.swift:264-284`:
/// `startFrameURL = frames.first`, `endFrameURL = frames[1]` (if present).
pub fn build_video_params(
    input: &GenerationInput,
    uploaded: &[String],
    frame_count: usize,
    image_ref_count: usize,
    video_ref_count: usize,
    audio_ref_count: usize,
) -> VideoParams {
    let s = slice_video_uploads(
        uploaded,
        frame_count,
        image_ref_count,
        video_ref_count,
        audio_ref_count,
    );
    VideoParams {
        prompt: input.prompt.clone(),
        duration: input.duration.max(0) as u32,
        aspect_ratio: input.aspect_ratio.clone(),
        resolution: input.resolution.clone(),
        source_video_url: None,
        start_frame_url: s.frames.first().cloned(),
        end_frame_url: s.frames.get(1).cloned(),
        reference_image_urls: s.image_refs,
        reference_video_urls: s.video_refs,
        reference_audio_urls: s.audio_refs,
        generate_audio: input.generate_audio.unwrap_or(true),
    }
}

/// Build params for a video-edit submission (`requiresSourceVideo`). Replicates
/// `VideoGenerationSubmission.swift:66-77`: `sourceVideoURL = uploaded.first`,
/// `referenceImageURLs = uploaded.dropFirst()`, frames all nil.
pub fn build_video_edit_params(input: &GenerationInput, uploaded: &[String]) -> VideoParams {
    VideoParams {
        prompt: input.prompt.clone(),
        duration: input.duration.max(0) as u32,
        aspect_ratio: input.aspect_ratio.clone(),
        resolution: input.resolution.clone(),
        source_video_url: uploaded.first().cloned(),
        start_frame_url: None,
        end_frame_url: None,
        reference_image_urls: uploaded.iter().skip(1).cloned().collect(),
        reference_video_urls: Vec::new(),
        reference_audio_urls: Vec::new(),
        generate_audio: input.generate_audio.unwrap_or(true),
    }
}

/// Build params for an image submission. Replicates
/// `ImageGenerationSubmission.swift:54-63`: `imageURLs = uploaded`.
pub fn build_image_params(input: &GenerationInput, uploaded: &[String]) -> ImageParams {
    ImageParams {
        prompt: input.prompt.clone(),
        aspect_ratio: input.aspect_ratio.clone(),
        resolution: input.resolution.clone(),
        quality: input.quality.clone(),
        image_urls: uploaded.to_vec(),
        num_images: clamp_num_images(input.num_images.unwrap_or(1).max(0) as u8),
    }
}

/// Build params for an audio submission. Replicates
/// `AudioGenerationSubmission.swift:31-34`: `videoURL` falls back to
/// `uploaded.first` when not already set.
pub fn build_audio_params(input: &GenerationInput, uploaded: &[String]) -> AudioParams {
    let video_url = input
        .reference_video_urls
        .as_ref()
        .and_then(|v| v.first().cloned())
        .or_else(|| uploaded.first().cloned());
    AudioParams {
        prompt: input.prompt.clone(),
        voice: input.voice.clone(),
        lyrics: input.lyrics.clone(),
        style_instructions: input.style_instructions.clone(),
        instrumental: input.instrumental.unwrap_or(false),
        duration_seconds: if input.duration > 0 {
            Some(input.duration as u32)
        } else {
            None
        },
        video_url,
    }
}

/// Build params for an upscale submission. `sourceURL` is the first uploaded URL.
pub fn build_upscale_params(input: &GenerationInput, uploaded: &[String]) -> UpscaleParams {
    UpscaleParams {
        source_url: uploaded
            .first()
            .cloned()
            .or_else(|| input.image_urls.as_ref().and_then(|v| v.first().cloned()))
            .unwrap_or_default(),
        duration_seconds: input.duration.max(0) as u32,
    }
}

/// Dispatch by model kind. For video, edit-model detection is the caller's
/// responsibility (via `requires_source_video`); the simple text/i2v path uses
/// frame/ref counts derived from the input.
pub fn build_params(
    input: &GenerationInput,
    uploaded: &[String],
    kind: ModelKind,
    requires_source_video: bool,
) -> GenerationParams {
    match kind {
        ModelKind::Image => GenerationParams::Image(build_image_params(input, uploaded)),
        ModelKind::Audio => GenerationParams::Audio(build_audio_params(input, uploaded)),
        ModelKind::Upscale => GenerationParams::Upscale(build_upscale_params(input, uploaded)),
        ModelKind::Video => {
            if requires_source_video {
                GenerationParams::Video(build_video_edit_params(input, uploaded))
            } else {
                // Frame count: derive from start/end presence via image_urls slot.
                let frame_count = input.image_urls.as_ref().map(|v| v.len()).unwrap_or(0);
                let image_ref_count = input
                    .reference_image_urls
                    .as_ref()
                    .map(|v| v.len())
                    .unwrap_or(0);
                let video_ref_count = input
                    .reference_video_urls
                    .as_ref()
                    .map(|v| v.len())
                    .unwrap_or(0);
                let audio_ref_count = input
                    .reference_audio_urls
                    .as_ref()
                    .map(|v| v.len())
                    .unwrap_or(0);
                GenerationParams::Video(build_video_params(
                    input,
                    uploaded,
                    frame_count,
                    image_ref_count,
                    video_ref_count,
                    audio_ref_count,
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_input() -> GenerationInput {
        GenerationInput {
            prompt: "p".into(),
            model: "m".into(),
            duration: 5,
            aspect_ratio: "16:9".into(),
            ..Default::default()
        }
    }

    #[test]
    fn video_slicing_order_frames_image_video_audio() {
        // uploaded: [frame0, frame1, img0, vid0, aud0]
        let uploaded = vec![
            "frame0".to_string(),
            "frame1".to_string(),
            "img0".to_string(),
            "vid0".to_string(),
            "aud0".to_string(),
        ];
        let p = build_video_params(&base_input(), &uploaded, 2, 1, 1, 1);
        assert_eq!(p.start_frame_url.as_deref(), Some("frame0"));
        assert_eq!(p.end_frame_url.as_deref(), Some("frame1"));
        assert_eq!(p.reference_image_urls, vec!["img0"]);
        assert_eq!(p.reference_video_urls, vec!["vid0"]);
        assert_eq!(p.reference_audio_urls, vec!["aud0"]);
        assert!(p.source_video_url.is_none());
    }

    #[test]
    fn video_single_frame_has_no_end_frame() {
        let uploaded = vec!["frame0".to_string()];
        let p = build_video_params(&base_input(), &uploaded, 1, 0, 0, 0);
        assert_eq!(p.start_frame_url.as_deref(), Some("frame0"));
        assert!(p.end_frame_url.is_none());
    }

    #[test]
    fn video_no_uploads_has_no_frames() {
        let p = build_video_params(&base_input(), &[], 0, 0, 0, 0);
        assert!(p.start_frame_url.is_none());
        assert!(p.end_frame_url.is_none());
        assert!(p.reference_image_urls.is_empty());
        assert!(p.generate_audio); // default true
    }

    #[test]
    fn video_edit_uses_source_then_image_refs() {
        let uploaded = vec![
            "src.mp4".to_string(),
            "ref0.png".to_string(),
            "ref1.png".to_string(),
        ];
        let p = build_video_edit_params(&base_input(), &uploaded);
        assert_eq!(p.source_video_url.as_deref(), Some("src.mp4"));
        assert_eq!(p.reference_image_urls, vec!["ref0.png", "ref1.png"]);
        assert!(p.start_frame_url.is_none());
        assert!(p.end_frame_url.is_none());
    }

    #[test]
    fn image_uses_all_uploads_and_clamps_num_images() {
        let mut input = base_input();
        input.num_images = Some(10);
        input.quality = Some("high".into());
        input.resolution = Some("1024x1024".into());
        let uploaded = vec!["a.png".to_string(), "b.png".to_string()];
        let p = build_image_params(&input, &uploaded);
        assert_eq!(p.image_urls, vec!["a.png", "b.png"]);
        assert_eq!(p.num_images, 4); // clamped
        assert_eq!(p.quality.as_deref(), Some("high"));
    }

    #[test]
    fn audio_video_url_falls_back_to_first_upload() {
        let mut input = base_input();
        input.voice = Some("alloy".into());
        let uploaded = vec!["clip.mp4".to_string()];
        let p = build_audio_params(&input, &uploaded);
        assert_eq!(p.video_url.as_deref(), Some("clip.mp4"));
        assert_eq!(p.voice.as_deref(), Some("alloy"));
        assert_eq!(p.duration_seconds, Some(5));
    }

    #[test]
    fn audio_prefers_explicit_reference_video_url() {
        let mut input = base_input();
        input.reference_video_urls = Some(vec!["explicit.mp4".into()]);
        let uploaded = vec!["fallback.mp4".to_string()];
        let p = build_audio_params(&input, &uploaded);
        assert_eq!(p.video_url.as_deref(), Some("explicit.mp4"));
    }

    #[test]
    fn audio_zero_duration_omits_duration_seconds() {
        let mut input = base_input();
        input.duration = 0;
        let p = build_audio_params(&input, &[]);
        assert_eq!(p.duration_seconds, None);
    }

    #[test]
    fn upscale_uses_first_upload_as_source() {
        let uploaded = vec!["in.mp4".to_string()];
        let p = build_upscale_params(&base_input(), &uploaded);
        assert_eq!(p.source_url, "in.mp4");
        assert_eq!(p.duration_seconds, 5);
    }

    #[test]
    fn dispatch_video_edit_vs_simple() {
        let uploaded = vec!["src.mp4".to_string(), "ref.png".to_string()];
        let edit = build_params(&base_input(), &uploaded, ModelKind::Video, true);
        match edit {
            GenerationParams::Video(v) => {
                assert_eq!(v.source_video_url.as_deref(), Some("src.mp4"))
            }
            _ => panic!("expected video"),
        }
        let mut input = base_input();
        input.image_urls = Some(vec!["frame.png".into()]);
        let simple = build_params(&input, &["frame.png".to_string()], ModelKind::Video, false);
        match simple {
            GenerationParams::Video(v) => {
                assert_eq!(v.start_frame_url.as_deref(), Some("frame.png"));
                assert!(v.source_video_url.is_none());
            }
            _ => panic!("expected video"),
        }
    }
}
