//! Model catalog entry + capability matrices. A 1:1 port of upstream
//! `CatalogEntry` and the four `*Caps` structs (`ModelCatalog.swift:112-241`).
//! This is the single data-driven source for UI/agent adaptation (axiom A5) and
//! is identical between managed and BYOK modes. `uiCapabilities` is dispatched by
//! `kind` via a custom `Deserialize`, replicating `CatalogEntry.init`.

use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;

/// Top-level media kind. Port of `CatalogEntry.Kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelKind {
    Video,
    Image,
    Audio,
    Upscale,
}

/// Shape of the result payload. Port of `CatalogEntry.ResponseShape`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum ResponseShape {
    #[serde(rename = "video")]
    Video,
    #[serde(rename = "images")]
    Images,
    #[serde(rename = "audio")]
    Audio,
    #[serde(rename = "upscaledImage")]
    UpscaledImage,
}

/// Audio pricing model. Port of `CatalogEntry.AudioPricing`, internally-tagged
/// by `mode`.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(tag = "mode")]
pub enum AudioPricing {
    #[serde(rename = "perThousandChars")]
    PerThousandChars { rate: f64 },
    #[serde(rename = "perSecond")]
    PerSecond { rate: f64 },
    #[serde(rename = "flat")]
    Flat { price: f64 },
}

/// Video capability matrix. Port of `VideoCaps` (`ModelCatalog.swift:195-211`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoCaps {
    pub durations: Vec<u32>,
    #[serde(default)]
    pub resolutions: Option<Vec<String>>,
    pub aspect_ratios: Vec<String>,
    pub supports_first_frame: bool,
    pub supports_last_frame: bool,
    pub max_reference_images: u32,
    pub max_reference_videos: u32,
    pub max_reference_audios: u32,
    #[serde(default)]
    pub max_total_references: Option<u32>,
    #[serde(default)]
    pub max_combined_video_ref_seconds: Option<f64>,
    #[serde(default)]
    pub max_combined_audio_ref_seconds: Option<f64>,
    pub frames_and_references_exclusive: bool,
    pub reference_tag_noun: String,
    pub requires_source_video: bool,
    pub requires_reference_image: bool,
}

/// Image capability matrix. Port of `ImageCaps` (`ModelCatalog.swift:213-219`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageCaps {
    #[serde(default)]
    pub resolutions: Option<Vec<String>>,
    pub aspect_ratios: Vec<String>,
    #[serde(default)]
    pub qualities: Option<Vec<String>>,
    pub supports_image_reference: bool,
    pub max_images: u32,
}

/// Audio capability matrix. Port of `AudioCaps` (`ModelCatalog.swift:221-234`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioCaps {
    /// "tts" | "music" | "sfx".
    pub category: String,
    #[serde(default)]
    pub voices: Option<Vec<String>>,
    #[serde(default)]
    pub default_voice: Option<String>,
    pub supports_lyrics: bool,
    pub supports_instrumental: bool,
    pub supports_style_instructions: bool,
    #[serde(default)]
    pub durations: Option<Vec<u32>>,
    pub min_prompt_length: u32,
    /// "text" | "video".
    #[serde(default)]
    pub inputs: Option<Vec<String>>,
    #[serde(default)]
    pub prompt_label: Option<String>,
    #[serde(default)]
    pub min_seconds: Option<u32>,
    #[serde(default)]
    pub max_seconds: Option<u32>,
}

/// Upscale capability matrix. Port of `UpscaleCaps` (`ModelCatalog.swift:236-240`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpscaleCaps {
    /// "Fast" | "Medium" | "Slow".
    pub speed: String,
    pub p75_duration_seconds: u32,
    /// "video" | "image".
    pub supported_types: Vec<String>,
}

/// Capability matrix dispatched by kind. Port of `CatalogEntry.UICapabilities`.
#[derive(Debug, Clone, PartialEq)]
pub enum UiCapabilities {
    Video(VideoCaps),
    Image(ImageCaps),
    Audio(AudioCaps),
    Upscale(UpscaleCaps),
}

/// A single catalog entry. Port of `CatalogEntry`. `uiCapabilities` decodes per
/// `kind` (custom `Deserialize` below, replicating `CatalogEntry.init`).
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogEntry {
    pub id: String,
    pub kind: ModelKind,
    pub display_name: String,
    pub allowed_endpoints: Vec<String>,
    pub response_shape: ResponseShape,
    pub ui_capabilities: UiCapabilities,
    pub credits_per_second: Option<HashMap<String, f64>>,
    pub audio_discount_rate: Option<HashMap<String, f64>>,
    pub credits_per_image: Option<HashMap<String, f64>>,
    pub qualities: Option<Vec<String>>,
    pub audio_pricing: Option<AudioPricing>,
    pub credits_per_second_upscale: Option<f64>,
}

// Field names for the custom deserializer.
#[derive(Deserialize)]
#[serde(field_identifier)]
enum Field {
    #[serde(rename = "id")]
    Id,
    #[serde(rename = "kind")]
    Kind,
    #[serde(rename = "displayName")]
    DisplayName,
    #[serde(rename = "allowedEndpoints")]
    AllowedEndpoints,
    #[serde(rename = "responseShape")]
    ResponseShape,
    #[serde(rename = "uiCapabilities")]
    UiCapabilities,
    #[serde(rename = "creditsPerSecond")]
    CreditsPerSecond,
    #[serde(rename = "audioDiscountRate")]
    AudioDiscountRate,
    #[serde(rename = "creditsPerImage")]
    CreditsPerImage,
    #[serde(rename = "qualities")]
    Qualities,
    #[serde(rename = "audioPricing")]
    AudioPricing,
    #[serde(rename = "creditsPerSecondUpscale")]
    CreditsPerSecondUpscale,
    #[serde(other)]
    Ignore,
}

impl<'de> Deserialize<'de> for CatalogEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct EntryVisitor;

        impl<'de> Visitor<'de> for EntryVisitor {
            type Value = CatalogEntry;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a CatalogEntry object")
            }

            fn visit_map<M>(self, mut map: M) -> Result<CatalogEntry, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut id = None;
                let mut kind = None;
                let mut display_name = None;
                let mut allowed_endpoints: Option<Vec<String>> = None;
                let mut response_shape = None;
                // Capture the raw uiCapabilities value; decode after kind is known.
                let mut caps_raw: Option<serde_json::Value> = None;
                let mut credits_per_second = None;
                let mut audio_discount_rate = None;
                let mut credits_per_image = None;
                let mut qualities = None;
                let mut audio_pricing = None;
                let mut credits_per_second_upscale = None;

                while let Some(key) = map.next_key::<Field>()? {
                    match key {
                        Field::Id => id = Some(map.next_value()?),
                        Field::Kind => kind = Some(map.next_value()?),
                        Field::DisplayName => display_name = Some(map.next_value()?),
                        Field::AllowedEndpoints => allowed_endpoints = Some(map.next_value()?),
                        Field::ResponseShape => response_shape = Some(map.next_value()?),
                        Field::UiCapabilities => caps_raw = Some(map.next_value()?),
                        Field::CreditsPerSecond => credits_per_second = Some(map.next_value()?),
                        Field::AudioDiscountRate => audio_discount_rate = Some(map.next_value()?),
                        Field::CreditsPerImage => credits_per_image = Some(map.next_value()?),
                        Field::Qualities => qualities = Some(map.next_value()?),
                        Field::AudioPricing => audio_pricing = Some(map.next_value()?),
                        Field::CreditsPerSecondUpscale => {
                            credits_per_second_upscale = Some(map.next_value()?)
                        }
                        Field::Ignore => {
                            let _ = map.next_value::<de::IgnoredAny>()?;
                        }
                    }
                }

                let kind: ModelKind = kind.ok_or_else(|| de::Error::missing_field("kind"))?;
                let caps_raw =
                    caps_raw.ok_or_else(|| de::Error::missing_field("uiCapabilities"))?;
                let ui_capabilities = match kind {
                    ModelKind::Video => UiCapabilities::Video(
                        serde_json::from_value(caps_raw).map_err(de::Error::custom)?,
                    ),
                    ModelKind::Image => UiCapabilities::Image(
                        serde_json::from_value(caps_raw).map_err(de::Error::custom)?,
                    ),
                    ModelKind::Audio => UiCapabilities::Audio(
                        serde_json::from_value(caps_raw).map_err(de::Error::custom)?,
                    ),
                    ModelKind::Upscale => UiCapabilities::Upscale(
                        serde_json::from_value(caps_raw).map_err(de::Error::custom)?,
                    ),
                };

                Ok(CatalogEntry {
                    id: id.ok_or_else(|| de::Error::missing_field("id"))?,
                    kind,
                    display_name: display_name
                        .ok_or_else(|| de::Error::missing_field("displayName"))?,
                    allowed_endpoints: allowed_endpoints.unwrap_or_default(),
                    response_shape: response_shape
                        .ok_or_else(|| de::Error::missing_field("responseShape"))?,
                    ui_capabilities,
                    credits_per_second,
                    audio_discount_rate,
                    credits_per_image,
                    qualities,
                    audio_pricing,
                    credits_per_second_upscale,
                })
            }
        }

        deserializer.deserialize_map(EntryVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_video_entry_with_video_caps() {
        let json = r#"{
            "id": "fal:kling-video",
            "kind": "video",
            "displayName": "Kling Video",
            "allowedEndpoints": ["text-to-video"],
            "responseShape": "video",
            "creditsPerSecond": {"": 10.0, "1080p": 20.0},
            "audioDiscountRate": {"": 0.8},
            "uiCapabilities": {
                "durations": [5, 10],
                "resolutions": ["720p", "1080p"],
                "aspectRatios": ["16:9", "9:16"],
                "supportsFirstFrame": true,
                "supportsLastFrame": true,
                "maxReferenceImages": 4,
                "maxReferenceVideos": 0,
                "maxReferenceAudios": 0,
                "framesAndReferencesExclusive": false,
                "referenceTagNoun": "element",
                "requiresSourceVideo": false,
                "requiresReferenceImage": false
            }
        }"#;
        let e: CatalogEntry = serde_json::from_str(json).unwrap();
        assert_eq!(e.id, "fal:kling-video");
        assert_eq!(e.kind, ModelKind::Video);
        assert_eq!(e.response_shape, ResponseShape::Video);
        match e.ui_capabilities {
            UiCapabilities::Video(caps) => {
                assert_eq!(caps.durations, vec![5, 10]);
                assert!(caps.supports_first_frame);
                assert_eq!(caps.max_reference_images, 4);
                assert_eq!(caps.reference_tag_noun, "element");
            }
            other => panic!("expected Video caps, got {other:?}"),
        }
        assert_eq!(e.credits_per_second.unwrap().get("1080p"), Some(&20.0));
    }

    #[test]
    fn deserializes_image_entry_with_image_caps() {
        let json = r#"{
            "id": "openai:gpt-image-1",
            "kind": "image",
            "displayName": "GPT Image",
            "allowedEndpoints": [],
            "responseShape": "images",
            "creditsPerImage": {"1024x1024|high": 5.0},
            "qualities": ["low", "high"],
            "uiCapabilities": {
                "resolutions": ["1024x1024"],
                "aspectRatios": ["1:1"],
                "qualities": ["low", "high"],
                "supportsImageReference": true,
                "maxImages": 4
            }
        }"#;
        let e: CatalogEntry = serde_json::from_str(json).unwrap();
        assert_eq!(e.kind, ModelKind::Image);
        match e.ui_capabilities {
            UiCapabilities::Image(caps) => {
                assert_eq!(caps.max_images, 4);
                assert!(caps.supports_image_reference);
            }
            other => panic!("expected Image caps, got {other:?}"),
        }
    }

    #[test]
    fn deserializes_audio_entry_with_pricing() {
        let json = r#"{
            "id": "elevenlabs:tts",
            "kind": "audio",
            "displayName": "ElevenLabs TTS",
            "allowedEndpoints": [],
            "responseShape": "audio",
            "audioPricing": {"mode": "perThousandChars", "rate": 0.3},
            "uiCapabilities": {
                "category": "tts",
                "voices": ["rachel"],
                "defaultVoice": "rachel",
                "supportsLyrics": false,
                "supportsInstrumental": false,
                "supportsStyleInstructions": false,
                "minPromptLength": 1
            }
        }"#;
        let e: CatalogEntry = serde_json::from_str(json).unwrap();
        assert_eq!(e.kind, ModelKind::Audio);
        assert_eq!(
            e.audio_pricing,
            Some(AudioPricing::PerThousandChars { rate: 0.3 })
        );
        match e.ui_capabilities {
            UiCapabilities::Audio(caps) => {
                assert_eq!(caps.category, "tts");
                assert_eq!(caps.default_voice.as_deref(), Some("rachel"));
            }
            other => panic!("expected Audio caps, got {other:?}"),
        }
    }

    #[test]
    fn deserializes_upscale_entry() {
        let json = r#"{
            "id": "replicate:topaz",
            "kind": "upscale",
            "displayName": "Topaz",
            "allowedEndpoints": [],
            "responseShape": "upscaledImage",
            "creditsPerSecondUpscale": 2.5,
            "uiCapabilities": {
                "speed": "Medium",
                "p75DurationSeconds": 30,
                "supportedTypes": ["video", "image"]
            }
        }"#;
        let e: CatalogEntry = serde_json::from_str(json).unwrap();
        assert_eq!(e.kind, ModelKind::Upscale);
        assert_eq!(e.credits_per_second_upscale, Some(2.5));
        match e.ui_capabilities {
            UiCapabilities::Upscale(caps) => {
                assert_eq!(caps.speed, "Medium");
                assert_eq!(caps.supported_types, vec!["video", "image"]);
            }
            other => panic!("expected Upscale caps, got {other:?}"),
        }
    }

    #[test]
    fn audio_pricing_per_second_and_flat() {
        let ps: AudioPricing =
            serde_json::from_str(r#"{"mode":"perSecond","rate":1.5}"#).unwrap();
        assert_eq!(ps, AudioPricing::PerSecond { rate: 1.5 });
        let flat: AudioPricing = serde_json::from_str(r#"{"mode":"flat","price":7.0}"#).unwrap();
        assert_eq!(flat, AudioPricing::Flat { price: 7.0 });
    }

    #[test]
    fn unknown_top_level_fields_are_ignored() {
        let json = r#"{
            "id": "fal:x", "kind": "image", "displayName": "X",
            "allowedEndpoints": [], "responseShape": "images",
            "someFutureField": {"a": 1},
            "uiCapabilities": {
                "aspectRatios": ["1:1"], "supportsImageReference": false, "maxImages": 1
            }
        }"#;
        let e: CatalogEntry = serde_json::from_str(json).unwrap();
        assert_eq!(e.id, "fal:x");
    }

    #[test]
    fn missing_kind_is_error() {
        let json = r#"{"id":"x","displayName":"X","allowedEndpoints":[],"responseShape":"images","uiCapabilities":{}}"#;
        assert!(serde_json::from_str::<CatalogEntry>(json).is_err());
    }
}
