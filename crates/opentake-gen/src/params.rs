//! `GenerationParams` — the tagged union submitted to the backend. A 1:1 wire
//! port of upstream `BackendGenerationParams` (`GenerationBackend.swift:95-110`)
//! and the four `*GenerationParams` `Encodable` structs. JSON field names match
//! upstream verbatim (note the all-caps `URL`/`URLs` keys); `kind` is the
//! internally-tagged discriminant. Optional + empty-collection fields are
//! omitted to match upstream `encodeIfPresent` / `if !x.isEmpty` behavior.

use serde::Serialize;

/// Tagged union of generation parameters, one variant per media kind.
/// `#[serde(tag = "kind")]` produces the same wire shape as the upstream
/// `singleValueContainer` encode (a flat object with a top-level `kind`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum GenerationParams {
    Image(ImageParams),
    Video(VideoParams),
    Audio(AudioParams),
    Upscale(UpscaleParams),
}

impl GenerationParams {
    /// The `kind` discriminant as it appears on the wire.
    pub fn kind_str(&self) -> &'static str {
        match self {
            GenerationParams::Image(_) => "image",
            GenerationParams::Video(_) => "video",
            GenerationParams::Audio(_) => "audio",
            GenerationParams::Upscale(_) => "upscale",
        }
    }
}

/// `kind="image"` — port of `ImageGenerationParams` (`ImageModelConfig.swift:3-25`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageParams {
    pub prompt: String,
    pub aspect_ratio: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<String>,
    /// Upstream wire key is `imageURLs` (all-caps URL); only written when non-empty.
    #[serde(rename = "imageURLs", skip_serializing_if = "Vec::is_empty")]
    pub image_urls: Vec<String>,
    pub num_images: u8,
}

impl ImageParams {
    /// Build with the upstream `numImages` clamp `max(1, min(4, n))`
    /// (`GenerationService.swift:41` + `ImageModelConfig.swift:47`).
    pub fn new(prompt: impl Into<String>, aspect_ratio: impl Into<String>, num_images: u8) -> Self {
        Self {
            prompt: prompt.into(),
            aspect_ratio: aspect_ratio.into(),
            resolution: None,
            quality: None,
            image_urls: Vec::new(),
            num_images: clamp_num_images(num_images),
        }
    }
}

/// Clamp to the upstream-supported 1..=4 image range.
pub fn clamp_num_images(n: u8) -> u8 {
    n.clamp(1, 4)
}

/// `kind="video"` — port of `VideoGenerationParams` (`VideoModelConfig.swift:67-124`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoParams {
    pub prompt: String,
    pub duration: u32,
    pub aspect_ratio: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    #[serde(rename = "sourceVideoURL", skip_serializing_if = "Option::is_none")]
    pub source_video_url: Option<String>,
    #[serde(rename = "startFrameURL", skip_serializing_if = "Option::is_none")]
    pub start_frame_url: Option<String>,
    #[serde(rename = "endFrameURL", skip_serializing_if = "Option::is_none")]
    pub end_frame_url: Option<String>,
    #[serde(rename = "referenceImageURLs", skip_serializing_if = "Vec::is_empty")]
    pub reference_image_urls: Vec<String>,
    #[serde(rename = "referenceVideoURLs", skip_serializing_if = "Vec::is_empty")]
    pub reference_video_urls: Vec<String>,
    #[serde(rename = "referenceAudioURLs", skip_serializing_if = "Vec::is_empty")]
    pub reference_audio_urls: Vec<String>,
    pub generate_audio: bool,
}

impl Default for VideoParams {
    /// Upstream init defaults `generateAudio = true` (`VideoModelConfig.swift:87`).
    fn default() -> Self {
        Self {
            prompt: String::new(),
            duration: 0,
            aspect_ratio: String::new(),
            resolution: None,
            source_video_url: None,
            start_frame_url: None,
            end_frame_url: None,
            reference_image_urls: Vec::new(),
            reference_video_urls: Vec::new(),
            reference_audio_urls: Vec::new(),
            generate_audio: true,
        }
    }
}

/// `kind="audio"` — port of `AudioGenerationParams` (`AudioModelConfig.swift:3-27`).
/// Covers TTS / music / sfx.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioParams {
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lyrics: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style_instructions: Option<String>,
    pub instrumental: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<u32>,
    #[serde(rename = "videoURL", skip_serializing_if = "Option::is_none")]
    pub video_url: Option<String>,
}

impl AudioParams {
    pub fn new(prompt: impl Into<String>, instrumental: bool) -> Self {
        Self {
            prompt: prompt.into(),
            voice: None,
            lyrics: None,
            style_instructions: None,
            instrumental,
            duration_seconds: None,
            video_url: None,
        }
    }
}

/// `kind="upscale"` — port of `UpscaleGenerationParams` (`UpscaleModelConfig.swift:3-15`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpscaleParams {
    #[serde(rename = "sourceURL")]
    pub source_url: String,
    pub duration_seconds: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn image_params_wire_shape_matches_upstream() {
        let p = GenerationParams::Image(ImageParams {
            prompt: "a cat".into(),
            aspect_ratio: "16:9".into(),
            resolution: Some("1024x1024".into()),
            quality: Some("high".into()),
            image_urls: vec!["https://x/a.png".into()],
            num_images: 2,
        });
        assert_eq!(
            serde_json::to_value(&p).unwrap(),
            json!({
                "kind": "image",
                "prompt": "a cat",
                "aspectRatio": "16:9",
                "resolution": "1024x1024",
                "quality": "high",
                "imageURLs": ["https://x/a.png"],
                "numImages": 2
            })
        );
    }

    #[test]
    fn image_params_omits_none_and_empty() {
        let p = GenerationParams::Image(ImageParams::new("hi", "1:1", 1));
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(
            v,
            json!({"kind":"image","prompt":"hi","aspectRatio":"1:1","numImages":1})
        );
        // resolution/quality/imageURLs must be absent, not null/empty.
        assert!(v.get("resolution").is_none());
        assert!(v.get("quality").is_none());
        assert!(v.get("imageURLs").is_none());
    }

    #[test]
    fn num_images_clamped_to_1_4() {
        assert_eq!(ImageParams::new("x", "1:1", 0).num_images, 1);
        assert_eq!(ImageParams::new("x", "1:1", 9).num_images, 4);
        assert_eq!(clamp_num_images(3), 3);
    }

    #[test]
    fn video_params_wire_keys_are_all_caps_url() {
        let p = GenerationParams::Video(VideoParams {
            prompt: "scene".into(),
            duration: 5,
            aspect_ratio: "16:9".into(),
            resolution: Some("1080p".into()),
            source_video_url: Some("https://x/src.mp4".into()),
            start_frame_url: Some("https://x/start.png".into()),
            end_frame_url: Some("https://x/end.png".into()),
            reference_image_urls: vec!["https://x/r1.png".into()],
            reference_video_urls: vec!["https://x/rv.mp4".into()],
            reference_audio_urls: vec!["https://x/ra.mp3".into()],
            generate_audio: false,
        });
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["kind"], "video");
        assert_eq!(v["sourceVideoURL"], "https://x/src.mp4");
        assert_eq!(v["startFrameURL"], "https://x/start.png");
        assert_eq!(v["endFrameURL"], "https://x/end.png");
        assert_eq!(v["referenceImageURLs"], json!(["https://x/r1.png"]));
        assert_eq!(v["referenceVideoURLs"], json!(["https://x/rv.mp4"]));
        assert_eq!(v["referenceAudioURLs"], json!(["https://x/ra.mp3"]));
        assert_eq!(v["generateAudio"], false);
        assert_eq!(v["duration"], 5);
    }

    #[test]
    fn video_params_default_generate_audio_true_and_omits_empties() {
        let p = VideoParams {
            prompt: "p".into(),
            duration: 3,
            aspect_ratio: "9:16".into(),
            ..Default::default()
        };
        assert!(p.generate_audio);
        let v = serde_json::to_value(GenerationParams::Video(p)).unwrap();
        assert!(v.get("sourceVideoURL").is_none());
        assert!(v.get("startFrameURL").is_none());
        assert!(v.get("referenceImageURLs").is_none());
        assert!(v.get("resolution").is_none());
        assert_eq!(v["generateAudio"], true);
    }

    #[test]
    fn audio_params_wire_shape() {
        let p = GenerationParams::Audio(AudioParams {
            prompt: "song".into(),
            voice: Some("alloy".into()),
            lyrics: Some("la la".into()),
            style_instructions: Some("upbeat".into()),
            instrumental: true,
            duration_seconds: Some(30),
            video_url: Some("https://x/v.mp4".into()),
        });
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(
            v,
            json!({
                "kind": "audio",
                "prompt": "song",
                "voice": "alloy",
                "lyrics": "la la",
                "styleInstructions": "upbeat",
                "instrumental": true,
                "durationSeconds": 30,
                "videoURL": "https://x/v.mp4"
            })
        );
    }

    #[test]
    fn audio_params_omits_optionals() {
        let p = GenerationParams::Audio(AudioParams::new("hello", false));
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(
            v,
            json!({"kind":"audio","prompt":"hello","instrumental":false})
        );
        assert!(v.get("voice").is_none());
        assert!(v.get("videoURL").is_none());
    }

    #[test]
    fn upscale_params_wire_shape() {
        let p = GenerationParams::Upscale(UpscaleParams {
            source_url: "https://x/in.mp4".into(),
            duration_seconds: 10,
        });
        assert_eq!(
            serde_json::to_value(&p).unwrap(),
            json!({"kind":"upscale","sourceURL":"https://x/in.mp4","durationSeconds":10})
        );
    }

    #[test]
    fn kind_str_matches_variant() {
        assert_eq!(
            GenerationParams::Image(ImageParams::new("a", "1:1", 1)).kind_str(),
            "image"
        );
        assert_eq!(
            GenerationParams::Video(VideoParams::default()).kind_str(),
            "video"
        );
        assert_eq!(
            GenerationParams::Audio(AudioParams::new("a", false)).kind_str(),
            "audio"
        );
        assert_eq!(
            GenerationParams::Upscale(UpscaleParams {
                source_url: "u".into(),
                duration_seconds: 1
            })
            .kind_str(),
            "upscale"
        );
    }
}
