//! Append-only AI generation audit log. Port of upstream `GenerationLog` /
//! `GenerationLogEntry` (`Editor/ViewModel/EditorViewModel+Cost.swift`),
//! persisted as `generation-log.json`.
//!
//! Two upstream tolerances are preserved verbatim:
//! - `version` defaults to `1` (the struct default and the missing-key
//!   fallback are both `1`, unlike `MediaManifest` whose default is 2 but
//!   fallback is 1).
//! - A row's cost migrates from the legacy dollar field: when `costCredits` is
//!   absent but `cost` (USD, a float) is present,
//!   `costCredits = ceil(cost * 100)` (Swift `(dollars * 100).rounded(.up)`).
//!
//! Dates: like the domain crate, `created_at` is Apple-reference-date seconds
//! (`f64`) — upstream's `JSONEncoder` default `Date` encoding. The
//! project/render layer converts to/from wall-clock time.

use serde::{Deserialize, Serialize};

fn default_version() -> i64 {
    1
}

/// The whole log. 1:1 with upstream `GenerationLog`.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct GenerationLog {
    /// Schema version. Defaults to 1 and falls back to 1 when missing. `i64` to
    /// match the width of upstream's Swift `Int` (64-bit on arm64).
    #[serde(default = "default_version")]
    pub version: i64,
    /// One row per AI generation, in append order.
    #[serde(default)]
    pub entries: Vec<GenerationLogEntry>,
}

impl Default for GenerationLog {
    fn default() -> Self {
        GenerationLog {
            version: 1,
            entries: Vec::new(),
        }
    }
}

impl GenerationLog {
    /// An empty log (`version = 1`).
    pub fn new() -> Self {
        GenerationLog::default()
    }

    /// Sum of `cost_credits` across rows (treating `None` as 0). Mirrors
    /// upstream `totalGenerationCost`.
    pub fn total_credits(&self) -> i64 {
        self.entries
            .iter()
            .map(|e| e.cost_credits.unwrap_or(0))
            .sum()
    }
}

/// One row in the project activity log. 1:1 with upstream `GenerationLogEntry`.
///
/// `id` is required on the wire when written by OpenTake, but tolerated as
/// missing on read (upstream synthesizes a UUID; here it decodes to an empty
/// string and the bundle layer leaves it untouched — rows are append-only and
/// not referenced by id elsewhere). `model` is required; `cost_credits` and
/// `created_at` are optional.
#[derive(Clone, PartialEq, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationLogEntry {
    /// Stable row id. Empty string when an old file omitted it.
    pub id: String,
    /// Model identifier used for the generation.
    pub model: String,
    /// Cost in credits. `None` when unknown. `i64` to match the width of
    /// upstream's Swift `Int` (64-bit on arm64).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_credits: Option<i64>,
    /// Apple-reference-date seconds. `None` when unknown.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<f64>,
}

impl GenerationLogEntry {
    /// Construct a row.
    pub fn new(
        id: impl Into<String>,
        model: impl Into<String>,
        cost_credits: Option<i64>,
        created_at: Option<f64>,
    ) -> Self {
        GenerationLogEntry {
            id: id.into(),
            model: model.into(),
            cost_credits,
            created_at,
        }
    }
}

impl<'de> Deserialize<'de> for GenerationLogEntry {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Capture both the new `costCredits` and the legacy `cost` (USD float),
        // matching upstream's hand-written decoder.
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Raw {
            id: Option<String>,
            model: String,
            cost_credits: Option<i64>,
            created_at: Option<f64>,
            // Legacy: dollars as a float. Only consulted when costCredits is absent.
            cost: Option<f64>,
        }
        let raw = Raw::deserialize(deserializer)?;
        let cost_credits = match raw.cost_credits {
            Some(c) => Some(c),
            None => raw
                .cost
                // Swift: Int((dollars * 100).rounded(.up)) — ceil toward +inf.
                .map(|dollars| (dollars * 100.0).ceil() as i64),
        };
        Ok(GenerationLogEntry {
            id: raw.id.unwrap_or_default(),
            model: raw.model,
            cost_credits,
            created_at: raw.created_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_and_new_version_is_one() {
        assert_eq!(GenerationLog::default().version, 1);
        assert_eq!(GenerationLog::new().version, 1);
        assert!(GenerationLog::new().entries.is_empty());
    }

    #[test]
    fn missing_version_falls_back_to_one() {
        let log: GenerationLog = serde_json::from_str(r#"{"entries":[]}"#).unwrap();
        assert_eq!(log.version, 1);
        let log2: GenerationLog = serde_json::from_str("{}").unwrap();
        assert_eq!(log2.version, 1);
        assert!(log2.entries.is_empty());
    }

    #[test]
    fn entry_roundtrip_camel_case() {
        let e = GenerationLogEntry::new("row-1", "veo-3", Some(250), Some(700_000_000.0));
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("\"costCredits\":250"));
        assert!(json.contains("\"createdAt\":700000000.0"));
        assert!(json.contains("\"model\":\"veo-3\""));
        let back: GenerationLogEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(e, back);
    }

    #[test]
    fn entry_omits_none_fields() {
        let e = GenerationLogEntry::new("row-2", "m", None, None);
        let json = serde_json::to_string(&e).unwrap();
        assert!(!json.contains("costCredits"));
        assert!(!json.contains("createdAt"));
        // id and model always present
        assert!(json.contains("\"id\":\"row-2\""));
    }

    #[test]
    fn legacy_cost_dollars_migrates_to_credits_ceil() {
        // 1.23 USD -> ceil(123.0) = 123
        let e: GenerationLogEntry =
            serde_json::from_str(r#"{"id":"a","model":"m","cost":1.23}"#).unwrap();
        assert_eq!(e.cost_credits, Some(123));
        // 0.005 USD -> ceil(0.5) = 1 (rounds up, never truncates)
        let e2: GenerationLogEntry =
            serde_json::from_str(r#"{"id":"b","model":"m","cost":0.005}"#).unwrap();
        assert_eq!(e2.cost_credits, Some(1));
        // exact: 2.00 USD -> 200
        let e3: GenerationLogEntry = serde_json::from_str(r#"{"model":"m","cost":2.0}"#).unwrap();
        assert_eq!(e3.cost_credits, Some(200));
    }

    #[test]
    fn cost_credits_wins_over_legacy_cost() {
        // When both present, costCredits is authoritative (upstream consults
        // legacy `cost` only when costCredits is absent).
        let e: GenerationLogEntry =
            serde_json::from_str(r#"{"model":"m","costCredits":7,"cost":99.0}"#).unwrap();
        assert_eq!(e.cost_credits, Some(7));
    }

    #[test]
    fn missing_id_decodes_to_empty_string() {
        let e: GenerationLogEntry = serde_json::from_str(r#"{"model":"m"}"#).unwrap();
        assert_eq!(e.id, "");
        assert_eq!(e.cost_credits, None);
        assert_eq!(e.created_at, None);
    }

    #[test]
    fn total_credits_sums_treating_none_as_zero() {
        let log = GenerationLog {
            version: 1,
            entries: vec![
                GenerationLogEntry::new("a", "m", Some(100), None),
                GenerationLogEntry::new("b", "m", None, None),
                GenerationLogEntry::new("c", "m", Some(50), None),
            ],
        };
        assert_eq!(log.total_credits(), 150);
    }

    #[test]
    fn full_log_roundtrip() {
        let log = GenerationLog {
            version: 1,
            entries: vec![GenerationLogEntry::new("a", "veo-3", Some(250), Some(1.0))],
        };
        let json = serde_json::to_string(&log).unwrap();
        let back: GenerationLog = serde_json::from_str(&json).unwrap();
        assert_eq!(log, back);
    }
}
