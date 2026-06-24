//! `list_models` catalog → JSON projection.
//!
//! The first real bridge between `opentake-agent` and `opentake-gen`: under BYOK
//! the model catalog is a static asset compiled into `opentake-gen`
//! ([`opentake_gen::builtin_catalog`]). This module reads that catalog, applies
//! the optional `?type=` filter ([`opentake_gen::filter_by_kind`], mirroring the
//! proxy / upstream `ToolExecutor+Generate.swift:374-387`), and projects the
//! typed [`CatalogEntry`] into the `{ models, loaded }` JSON the agent contract
//! expects (`docs/_analysis/04-MCP与Agent工具.md:86`).
//!
//! The catalog is embedded in the binary, so `loaded` is always `true` here —
//! there is no async sync step to fail. This is purely local, needs no network
//! and no BYOK key, so the dispatcher can run it synchronously and tests cover it
//! offline.
//!
//! Why a hand-written projection instead of `serde_json::to_value(entry)`:
//! [`CatalogEntry`] derives a custom `Deserialize` only (it is a 1:1 port of an
//! upstream wire type and is not meant to be re-serialized inside `opentake-gen`).
//! Rather than widen that crate's public surface, the agent owns this read-side
//! projection. The field names mirror the embedded `builtin_catalog.json` so the
//! UI/agent see the same shape both modes produce.

use opentake_gen::{
    AudioCaps, AudioPricing, CatalogEntry, ImageCaps, ModelKind, ResponseShape, UiCapabilities,
    UpscaleCaps, VideoCaps,
};
use serde_json::{json, Value};

use crate::tools::errors::ToolError;

/// Parse the optional `type` arg into a [`ModelKind`]. `None` means "no filter".
/// An unrecognised value is a precise-path tool error (mirrors the decode-error
/// contract the rest of the tool layer relies on for agent self-correction).
pub fn parse_kind(raw: Option<&str>) -> Result<Option<ModelKind>, ToolError> {
    match raw {
        None => Ok(None),
        Some("video") => Ok(Some(ModelKind::Video)),
        Some("image") => Ok(Some(ModelKind::Image)),
        Some("audio") => Ok(Some(ModelKind::Audio)),
        Some("upscale") => Ok(Some(ModelKind::Upscale)),
        Some(other) => Err(ToolError::new(format!(
            "type: unknown value '{other}'. Allowed: audio, image, upscale, video."
        ))),
    }
}

/// Build the `{ models, loaded }` payload for `list_models`, optionally filtered
/// by `kind`. Reads the built-in static catalog from `opentake-gen`.
pub fn list_models_payload(kind: Option<ModelKind>) -> Value {
    let entries = opentake_gen::builtin_catalog();
    let selected = match kind {
        Some(k) => opentake_gen::filter_by_kind(&entries, k),
        None => entries,
    };
    let models: Vec<Value> = selected.iter().map(entry_to_json).collect();
    json!({ "models": models, "loaded": true })
}

/// Project one typed [`CatalogEntry`] into its JSON wire form. Field names match
/// the embedded `builtin_catalog.json` (camelCase). `Option` fields are emitted
/// as JSON `null` to match the round-trip shape of the source asset.
fn entry_to_json(e: &CatalogEntry) -> Value {
    let mut obj = json!({
        "id": e.id,
        "kind": kind_str(e.kind),
        "displayName": e.display_name,
        "allowedEndpoints": e.allowed_endpoints,
        "responseShape": response_shape_str(e.response_shape),
        "uiCapabilities": ui_capabilities_to_json(&e.ui_capabilities),
    });
    let map = obj.as_object_mut().expect("json! object");
    map.insert("creditsPerSecond".into(), opt_map(&e.credits_per_second));
    map.insert("audioDiscountRate".into(), opt_map(&e.audio_discount_rate));
    map.insert("creditsPerImage".into(), opt_map(&e.credits_per_image));
    map.insert("qualities".into(), opt_str_vec(&e.qualities));
    map.insert(
        "audioPricing".into(),
        e.audio_pricing
            .map(audio_pricing_to_json)
            .unwrap_or(Value::Null),
    );
    map.insert(
        "creditsPerSecondUpscale".into(),
        e.credits_per_second_upscale
            .map(|v| json!(v))
            .unwrap_or(Value::Null),
    );
    obj
}

fn kind_str(k: ModelKind) -> &'static str {
    match k {
        ModelKind::Video => "video",
        ModelKind::Image => "image",
        ModelKind::Audio => "audio",
        ModelKind::Upscale => "upscale",
    }
}

fn response_shape_str(s: ResponseShape) -> &'static str {
    match s {
        ResponseShape::Video => "video",
        ResponseShape::Images => "images",
        ResponseShape::Audio => "audio",
        ResponseShape::UpscaledImage => "upscaledImage",
    }
}

fn ui_capabilities_to_json(caps: &UiCapabilities) -> Value {
    match caps {
        UiCapabilities::Video(v) => video_caps_to_json(v),
        UiCapabilities::Image(i) => image_caps_to_json(i),
        UiCapabilities::Audio(a) => audio_caps_to_json(a),
        UiCapabilities::Upscale(u) => upscale_caps_to_json(u),
    }
}

fn video_caps_to_json(c: &VideoCaps) -> Value {
    json!({
        "durations": c.durations,
        "resolutions": c.resolutions,
        "aspectRatios": c.aspect_ratios,
        "supportsFirstFrame": c.supports_first_frame,
        "supportsLastFrame": c.supports_last_frame,
        "maxReferenceImages": c.max_reference_images,
        "maxReferenceVideos": c.max_reference_videos,
        "maxReferenceAudios": c.max_reference_audios,
        "maxTotalReferences": c.max_total_references,
        "maxCombinedVideoRefSeconds": c.max_combined_video_ref_seconds,
        "maxCombinedAudioRefSeconds": c.max_combined_audio_ref_seconds,
        "framesAndReferencesExclusive": c.frames_and_references_exclusive,
        "referenceTagNoun": c.reference_tag_noun,
        "requiresSourceVideo": c.requires_source_video,
        "requiresReferenceImage": c.requires_reference_image,
    })
}

fn image_caps_to_json(c: &ImageCaps) -> Value {
    json!({
        "resolutions": c.resolutions,
        "aspectRatios": c.aspect_ratios,
        "qualities": c.qualities,
        "supportsImageReference": c.supports_image_reference,
        "maxImages": c.max_images,
    })
}

fn audio_caps_to_json(c: &AudioCaps) -> Value {
    json!({
        "category": c.category,
        "voices": c.voices,
        "defaultVoice": c.default_voice,
        "supportsLyrics": c.supports_lyrics,
        "supportsInstrumental": c.supports_instrumental,
        "supportsStyleInstructions": c.supports_style_instructions,
        "durations": c.durations,
        "minPromptLength": c.min_prompt_length,
        "inputs": c.inputs,
        "promptLabel": c.prompt_label,
        "minSeconds": c.min_seconds,
        "maxSeconds": c.max_seconds,
    })
}

fn upscale_caps_to_json(c: &UpscaleCaps) -> Value {
    json!({
        "speed": c.speed,
        "p75DurationSeconds": c.p75_duration_seconds,
        "supportedTypes": c.supported_types,
    })
}

fn audio_pricing_to_json(p: AudioPricing) -> Value {
    match p {
        AudioPricing::PerThousandChars { rate } => {
            json!({ "mode": "perThousandChars", "rate": rate })
        }
        AudioPricing::PerSecond { rate } => json!({ "mode": "perSecond", "rate": rate }),
        AudioPricing::Flat { price } => json!({ "mode": "flat", "price": price }),
    }
}

/// `Option<HashMap<..>>` → JSON object or `null`.
fn opt_map(m: &Option<std::collections::HashMap<String, f64>>) -> Value {
    match m {
        Some(map) => json!(map),
        None => Value::Null,
    }
}

/// `Option<Vec<String>>` → JSON array or `null`.
fn opt_str_vec(v: &Option<Vec<String>>) -> Value {
    match v {
        Some(list) => json!(list),
        None => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_kind_maps_known_values() {
        assert_eq!(parse_kind(None).unwrap(), None);
        assert_eq!(parse_kind(Some("video")).unwrap(), Some(ModelKind::Video));
        assert_eq!(parse_kind(Some("image")).unwrap(), Some(ModelKind::Image));
        assert_eq!(parse_kind(Some("audio")).unwrap(), Some(ModelKind::Audio));
        assert_eq!(
            parse_kind(Some("upscale")).unwrap(),
            Some(ModelKind::Upscale)
        );
    }

    #[test]
    fn parse_kind_rejects_unknown_with_precise_message() {
        let err = parse_kind(Some("gif")).unwrap_err();
        assert!(
            err.message.starts_with("type: unknown value 'gif'"),
            "{}",
            err.message
        );
        // Allowed list is enumerated so the agent can self-correct.
        assert!(err.message.contains("audio, image, upscale, video"));
    }

    #[test]
    fn payload_lists_full_catalog_when_unfiltered() {
        let v = list_models_payload(None);
        assert_eq!(v["loaded"], json!(true));
        let models = v["models"].as_array().expect("models array");
        assert_eq!(models.len(), opentake_gen::builtin_catalog().len());
        // Spot-check a known entry shape.
        let flux = models
            .iter()
            .find(|m| m["id"] == json!("fal:flux-pro"))
            .expect("fal:flux-pro present");
        assert_eq!(flux["kind"], json!("image"));
        assert_eq!(flux["displayName"], json!("FLUX.1 [pro]"));
        assert!(flux["uiCapabilities"]["maxImages"].is_number());
    }

    #[test]
    fn payload_filters_by_kind() {
        let v = list_models_payload(Some(ModelKind::Video));
        let models = v["models"].as_array().expect("models array");
        assert!(!models.is_empty(), "catalog must have video models");
        assert!(models.iter().all(|m| m["kind"] == json!("video")));
        // Every video entry carries video-specific caps.
        assert!(models
            .iter()
            .all(|m| m["uiCapabilities"]["durations"].is_array()));
    }

    #[test]
    fn audio_entries_carry_audio_caps() {
        let v = list_models_payload(Some(ModelKind::Audio));
        let models = v["models"].as_array().expect("models array");
        assert!(!models.is_empty());
        assert!(models
            .iter()
            .all(|m| m["uiCapabilities"]["category"].is_string()));
    }
}
