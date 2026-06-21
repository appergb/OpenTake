//! Client-side cost estimation (display only). A 1:1 port of upstream
//! `CostEstimator` (`CostEstimator.swift:3-108`). Real charging is settled by the
//! proxy on job completion (managed) and is absent under BYOK. Pure functions,
//! fully unit-testable.

use super::entry::{AudioPricing, CatalogEntry, ModelKind, UiCapabilities};
use std::collections::HashMap;

/// `dict[key] ?? dict[""]` — resolved rate with empty-string default tier.
/// Replicates `resolvedRate` (`CostEstimator.swift:99-102`).
fn resolved_rate(dict: &HashMap<String, f64>, key: Option<&str>) -> Option<f64> {
    if let Some(k) = key {
        if let Some(v) = dict.get(k) {
            return Some(*v);
        }
    }
    dict.get("").copied()
}

/// `credits <= 0 -> 0`, else ceil. Replicates `ceilCredits`
/// (`CostEstimator.swift:104-107`).
fn ceil_credits(credits: f64) -> i64 {
    if credits <= 0.0 {
        0
    } else {
        credits.ceil() as i64
    }
}

/// `dict[key] ?? dict[""]` for the audio discount rate. Replicates
/// `audioDiscount(for:)` (`VideoModelConfig.swift:44-48`).
fn audio_discount(rate: &Option<HashMap<String, f64>>, resolution: Option<&str>) -> Option<f64> {
    let dict = rate.as_ref()?;
    if let Some(k) = resolution {
        if let Some(v) = dict.get(k) {
            return Some(*v);
        }
    }
    dict.get("").copied()
}

/// Video cost: `ceil(rate * duration)`, `rate = creditsPerSecond[res] ?? [""]`;
/// when `!generate_audio`, multiply by `audioDiscountRate[res] ?? [""]`.
/// Replicates `videoCost` (`CostEstimator.swift:5-17`).
pub fn video_cost(
    entry: &CatalogEntry,
    duration_seconds: i64,
    resolution: Option<&str>,
    generate_audio: bool,
) -> Option<i64> {
    let cps = entry.credits_per_second.as_ref()?;
    if cps.is_empty() || duration_seconds <= 0 {
        return None;
    }
    let mut rate = resolved_rate(cps, resolution)?;
    if !generate_audio {
        if let Some(discount) = audio_discount(&entry.audio_discount_rate, resolution) {
            rate *= discount;
        }
    }
    Some(ceil_credits(rate * duration_seconds as f64))
}

/// Image cost: 2D `"<res>|<quality>"` lookup, then quality-only, then
/// `creditsPerImage[res] ?? [""]`; multiplied by `max(1, num_images)`.
/// Replicates `imageCost` (`CostEstimator.swift:19-37`).
pub fn image_cost(
    entry: &CatalogEntry,
    resolution: Option<&str>,
    quality: Option<&str>,
    num_images: i64,
) -> Option<i64> {
    let cpi = entry.credits_per_image.as_ref()?;
    if cpi.is_empty() {
        return None;
    }
    let count = num_images.max(1) as f64;

    // 2D matrix lookup first.
    if let (Some(r), Some(q)) = (resolution, quality) {
        if let Some(price) = cpi.get(&format!("{r}|{q}")) {
            return Some(ceil_credits(price * count));
        }
    }
    // Quality-only lookup when the model varies on quality but not resolution.
    if entry.qualities.is_some() {
        if let Some(q) = quality {
            if let Some(price) = cpi.get(q) {
                return Some(ceil_credits(price * count));
            }
        }
    }
    let rate = resolved_rate(cpi, resolution)?;
    Some(ceil_credits(rate * count))
}

/// Audio cost by pricing mode. Replicates `audioCost`
/// (`CostEstimator.swift:39-57`).
pub fn audio_cost(
    entry: &CatalogEntry,
    prompt: &str,
    duration_seconds: Option<i64>,
) -> Option<i64> {
    match entry.audio_pricing? {
        AudioPricing::PerThousandChars { rate } => {
            let chars = prompt.chars().count();
            if chars == 0 {
                return None;
            }
            Some(ceil_credits(rate * (chars as f64 / 1000.0)))
        }
        AudioPricing::PerSecond { rate } => {
            let secs = duration_seconds?;
            if secs <= 0 {
                return None;
            }
            Some(ceil_credits(rate * secs as f64))
        }
        AudioPricing::Flat { price } => Some(ceil_credits(price)),
    }
}

/// Upscale cost: `ceil(creditsPerSecondUpscale * max(1, duration))`.
/// Replicates `upscaleCost` (`CostEstimator.swift:59-62`).
pub fn upscale_cost(entry: &CatalogEntry, duration_seconds: i64) -> Option<i64> {
    let rate = entry.credits_per_second_upscale?;
    let d = duration_seconds.max(1);
    Some(ceil_credits(rate * d as f64))
}

/// Format credits for display. Replicates `format` (`CostEstimator.swift:90-95`).
pub fn format_credits(credits: Option<i64>) -> String {
    match credits {
        None => "—".to_string(),
        Some(c) if c <= 0 => "0 credits".to_string(),
        Some(1) => "1 credit".to_string(),
        Some(c) => format!("{c} credits"),
    }
}

/// Dispatch cost computation from a `CatalogEntry` + a `GenerationInput`.
/// Mirrors `CostEstimator.cost(for:)` (`CostEstimator.swift:65-88`). The
/// audio-duration gating (`durations != nil || inputs contains video`) is
/// replicated from upstream.
pub fn cost_for_input(
    entry: &CatalogEntry,
    input: &opentake_domain::GenerationInput,
) -> Option<i64> {
    match entry.kind {
        ModelKind::Video => video_cost(
            entry,
            input.duration as i64,
            input.resolution.as_deref(),
            input.generate_audio.unwrap_or(true),
        ),
        ModelKind::Image => image_cost(
            entry,
            input.resolution.as_deref(),
            input.quality.as_deref(),
            input.num_images.unwrap_or(1) as i64,
        ),
        ModelKind::Audio => {
            let gate_on_duration = match &entry.ui_capabilities {
                UiCapabilities::Audio(caps) => {
                    caps.durations.is_some()
                        || caps
                            .inputs
                            .as_ref()
                            .map(|v| v.iter().any(|s| s == "video"))
                            .unwrap_or(false)
                }
                _ => false,
            };
            let duration = if gate_on_duration {
                Some(input.duration as i64)
            } else {
                None
            };
            audio_cost(entry, &input.prompt, duration)
        }
        ModelKind::Upscale => upscale_cost(entry, input.duration as i64),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn video_entry() -> CatalogEntry {
        let v = json!({
            "id": "fal:vid", "kind": "video", "displayName": "Vid",
            "allowedEndpoints": [], "responseShape": "video",
            "creditsPerSecond": {"": 10.0, "1080p": 20.0},
            "audioDiscountRate": {"": 0.5},
            "uiCapabilities": {
                "durations": [5], "aspectRatios": ["16:9"],
                "supportsFirstFrame": false, "supportsLastFrame": false,
                "maxReferenceImages": 0, "maxReferenceVideos": 0, "maxReferenceAudios": 0,
                "framesAndReferencesExclusive": false, "referenceTagNoun": "x",
                "requiresSourceVideo": false, "requiresReferenceImage": false
            }
        });
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn video_cost_uses_resolution_rate_and_ceils() {
        let e = video_entry();
        // 20.0 * 3 = 60
        assert_eq!(video_cost(&e, 3, Some("1080p"), true), Some(60));
    }

    #[test]
    fn video_cost_falls_back_to_default_tier() {
        let e = video_entry();
        // unknown res -> "" tier = 10.0; 10 * 2 = 20
        assert_eq!(video_cost(&e, 2, Some("4k"), true), Some(20));
        // None res -> "" tier
        assert_eq!(video_cost(&e, 2, None, true), Some(20));
    }

    #[test]
    fn video_cost_applies_audio_discount_when_no_audio() {
        let e = video_entry();
        // 20.0 * 0.5 = 10.0 per sec; * 4 = 40
        assert_eq!(video_cost(&e, 4, Some("1080p"), false), Some(40));
    }

    #[test]
    fn video_cost_none_when_zero_duration() {
        assert_eq!(video_cost(&video_entry(), 0, Some("1080p"), true), None);
    }

    fn image_entry_2d() -> CatalogEntry {
        let v = json!({
            "id": "openai:img", "kind": "image", "displayName": "Img",
            "allowedEndpoints": [], "responseShape": "images",
            "creditsPerImage": {"1024x1024|high": 5.0, "high": 4.0, "": 2.0},
            "qualities": ["low", "high"],
            "uiCapabilities": {
                "aspectRatios": ["1:1"], "qualities": ["low","high"],
                "supportsImageReference": false, "maxImages": 4
            }
        });
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn image_cost_prefers_2d_matrix() {
        let e = image_entry_2d();
        // "1024x1024|high" = 5.0 * 2 images = 10
        assert_eq!(image_cost(&e, Some("1024x1024"), Some("high"), 2), Some(10));
    }

    #[test]
    fn image_cost_quality_only_lookup() {
        let e = image_entry_2d();
        // res missing in 2D -> quality-only "high" = 4.0 * 1 = 4
        assert_eq!(image_cost(&e, Some("512x512"), Some("high"), 1), Some(4));
    }

    #[test]
    fn image_cost_default_tier_lookup() {
        let e = image_entry_2d();
        // no quality match -> resolved_rate falls to "" = 2.0 * 3 = 6
        assert_eq!(image_cost(&e, Some("999"), Some("ultra"), 3), Some(6));
    }

    #[test]
    fn image_cost_clamps_count_to_one() {
        let e = image_entry_2d();
        // num_images 0 -> max(1) -> "" tier 2.0 * 1 = 2
        assert_eq!(image_cost(&e, None, None, 0), Some(2));
    }

    fn audio_entry(pricing: serde_json::Value) -> CatalogEntry {
        let v = json!({
            "id": "x:aud", "kind": "audio", "displayName": "Aud",
            "allowedEndpoints": [], "responseShape": "audio",
            "audioPricing": pricing,
            "uiCapabilities": {
                "category": "tts", "supportsLyrics": false,
                "supportsInstrumental": false, "supportsStyleInstructions": false,
                "minPromptLength": 1
            }
        });
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn audio_cost_per_thousand_chars() {
        let e = audio_entry(json!({"mode":"perThousandChars","rate":10.0}));
        // 250 chars * 10 / 1000 = 2.5 -> ceil 3
        let prompt = "a".repeat(250);
        assert_eq!(audio_cost(&e, &prompt, None), Some(3));
    }

    #[test]
    fn audio_cost_per_thousand_chars_none_when_empty() {
        let e = audio_entry(json!({"mode":"perThousandChars","rate":10.0}));
        assert_eq!(audio_cost(&e, "", None), None);
    }

    #[test]
    fn audio_cost_per_second() {
        let e = audio_entry(json!({"mode":"perSecond","rate":1.5}));
        // 1.5 * 10 = 15
        assert_eq!(audio_cost(&e, "hi", Some(10)), Some(15));
        assert_eq!(audio_cost(&e, "hi", None), None);
    }

    #[test]
    fn audio_cost_flat() {
        let e = audio_entry(json!({"mode":"flat","price":7.2}));
        assert_eq!(audio_cost(&e, "anything", None), Some(8)); // ceil(7.2)
    }

    fn upscale_entry() -> CatalogEntry {
        let v = json!({
            "id": "r:up", "kind": "upscale", "displayName": "Up",
            "allowedEndpoints": [], "responseShape": "upscaledImage",
            "creditsPerSecondUpscale": 2.5,
            "uiCapabilities": {"speed":"Fast","p75DurationSeconds":1,"supportedTypes":["image"]}
        });
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn upscale_cost_ceils_and_clamps_duration() {
        let e = upscale_entry();
        // 2.5 * 4 = 10
        assert_eq!(upscale_cost(&e, 4), Some(10));
        // duration 0 -> max(1) -> 2.5 -> ceil 3
        assert_eq!(upscale_cost(&e, 0), Some(3));
    }

    #[test]
    fn format_credits_variants() {
        assert_eq!(format_credits(None), "—");
        assert_eq!(format_credits(Some(0)), "0 credits");
        assert_eq!(format_credits(Some(-5)), "0 credits");
        assert_eq!(format_credits(Some(1)), "1 credit");
        assert_eq!(format_credits(Some(42)), "42 credits");
    }

    #[test]
    fn cost_for_input_dispatches_by_kind() {
        let e = video_entry();
        let input = opentake_domain::GenerationInput {
            prompt: "p".into(),
            model: "fal:vid".into(),
            duration: 3,
            aspect_ratio: "16:9".into(),
            resolution: Some("1080p".into()),
            generate_audio: Some(true),
            ..Default::default()
        };
        assert_eq!(cost_for_input(&e, &input), Some(60));
    }

    #[test]
    fn cost_for_input_audio_gates_duration_on_caps() {
        // category tts, no durations, inputs none -> duration NOT used.
        let e = audio_entry(json!({"mode":"perSecond","rate":1.0}));
        let input = opentake_domain::GenerationInput {
            prompt: "hi".into(),
            model: "x:aud".into(),
            duration: 30,
            aspect_ratio: String::new(),
            ..Default::default()
        };
        // perSecond with duration gated off -> None
        assert_eq!(cost_for_input(&e, &input), None);
    }
}
