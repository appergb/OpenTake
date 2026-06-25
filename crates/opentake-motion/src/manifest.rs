//! The motion-template plugin manifest (`plugin.json`), per docs/
//! MOTION-GRAPHICS-PLUGIN.md §4. A template package = a web bundle + this
//! manifest declaring its name, typed parameter schema, duration model, fps
//! policy, and transparency.
//!
//! Style follows the existing workflow-plugin manifest in `opentake-agent`
//! (`plugin/model.rs`): pure JSON, `#[serde(rename_all = "snake_case")]`, every
//! field `#[serde(default)]`-tolerant so a partial/legacy manifest still decodes;
//! validation is a separate, explicit pass ([`MotionPlugin::validate`]).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::{MotionError, MotionResult};
use crate::source::ParamValue;

/// How a template's duration is determined.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DurationMode {
    /// The template has an intrinsic, fixed length (e.g. a 5 s title sting). The
    /// host uses `default_seconds`.
    #[default]
    Fixed,
    /// The length is driven by the host — the caller picks `duration_frames` and
    /// the template animates to fill it (e.g. a progress bar, a hold-able card).
    Driven,
}

/// The duration block of a manifest.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DurationSpec {
    #[serde(default)]
    pub mode: DurationMode,
    /// Default length in seconds (used directly for `fixed`, as a suggestion for
    /// `driven`).
    #[serde(default = "default_duration_seconds")]
    pub default_seconds: f64,
}

fn default_duration_seconds() -> f64 {
    5.0
}

impl Default for DurationSpec {
    fn default() -> Self {
        DurationSpec {
            mode: DurationMode::Fixed,
            default_seconds: default_duration_seconds(),
        }
    }
}

/// The fps policy of a manifest.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FpsPolicy {
    /// Render at the project's fps (the usual case — the clip matches the
    /// timeline). Serializes as `"inherit"`.
    #[default]
    Inherit,
    /// Render at a template-fixed fps regardless of the project. Serializes as
    /// `{"fixed": 30}`.
    Fixed(u32),
}

/// One declared parameter in a template's schema.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ParamSpec {
    /// Declared type: `"string" | "number" | "bool" | "color"` (see
    /// [`ParamValue::matches_declared`]).
    #[serde(rename = "type", default)]
    pub kind: String,
    /// Whether a binding is required. Defaults to `false` (optional).
    #[serde(default)]
    pub required: bool,
    /// Optional human label for UIs.
    #[serde(default)]
    pub label: Option<String>,
}

/// Plugin author block (mirrors the workflow-plugin manifest).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MotionPluginAuthor {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub url: Option<String>,
}

/// The whole motion-template `plugin.json`. All fields tolerant; call
/// [`MotionPlugin::validate`] for the strict pass.
///
/// `Default` is hand-written (not derived) so it matches the serde field
/// defaults exactly: `entry = "index.html"` and `transparent = true`. A derived
/// `Default` would use `String::default()` / `bool::default()` and diverge from
/// what decoding `{}` produces.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MotionPlugin {
    /// Manifest schema version (e.g. `"1.0"`).
    #[serde(default)]
    pub schema_version: String,
    /// Unique template id (e.g. `"lower-third.glass"`). Matches the id in a
    /// [`crate::source::MotionSource::Template`].
    #[serde(default)]
    pub id: String,
    /// Human-readable name.
    #[serde(default)]
    pub name: String,
    /// One-line description.
    #[serde(default)]
    pub description: String,
    /// Entry HTML file inside the bundle (e.g. `"index.html"`).
    #[serde(default = "default_entry")]
    pub entry: String,
    #[serde(default)]
    pub author: MotionPluginAuthor,
    #[serde(default)]
    pub license: String,
    /// Typed parameter schema (`name -> spec`). `BTreeMap` for stable order.
    #[serde(default)]
    pub params: BTreeMap<String, ParamSpec>,
    /// Duration model.
    #[serde(default)]
    pub duration: DurationSpec,
    /// fps policy (inherit project fps by default).
    #[serde(default)]
    pub fps: FpsPolicy,
    /// Whether the template renders a transparent overlay.
    #[serde(default = "default_true")]
    pub transparent: bool,
}

fn default_entry() -> String {
    "index.html".to_string()
}

fn default_true() -> bool {
    true
}

impl Default for MotionPlugin {
    fn default() -> Self {
        MotionPlugin {
            schema_version: String::new(),
            id: String::new(),
            name: String::new(),
            description: String::new(),
            entry: default_entry(),
            author: MotionPluginAuthor::default(),
            license: String::new(),
            params: BTreeMap::new(),
            duration: DurationSpec::default(),
            fps: FpsPolicy::default(),
            transparent: default_true(),
        }
    }
}

impl MotionPlugin {
    /// Strict validation of the manifest's own fields (independent of any
    /// instance). Returns the first problem found.
    pub fn validate(&self) -> MotionResult<()> {
        if self.id.trim().is_empty() {
            return Err(MotionError::manifest("manifest has no id"));
        }
        if self.name.trim().is_empty() {
            return Err(MotionError::manifest(format!(
                "template {:?} has no name",
                self.id
            )));
        }
        if self.entry.trim().is_empty() {
            return Err(MotionError::manifest(format!(
                "template {:?} has no entry file",
                self.id
            )));
        }
        if !(self.duration.default_seconds.is_finite() && self.duration.default_seconds > 0.0) {
            return Err(MotionError::manifest(format!(
                "template {:?} default_seconds must be a positive finite number",
                self.id
            )));
        }
        if let FpsPolicy::Fixed(fps) = self.fps {
            if fps == 0 || fps > crate::source::limits::MAX_FPS {
                return Err(MotionError::manifest(format!(
                    "template {:?} fixed fps {} out of range",
                    self.id, fps
                )));
            }
        }
        for (name, spec) in &self.params {
            if name.trim().is_empty() {
                return Err(MotionError::manifest("a param has an empty name"));
            }
            // An empty/absent type means "untyped" (accept anything) — allowed.
            if !spec.kind.is_empty()
                && !matches!(
                    spec.kind.as_str(),
                    "string" | "number" | "bool" | "boolean" | "color"
                )
            {
                return Err(MotionError::manifest(format!(
                    "param {name:?} has unknown type {:?}",
                    spec.kind
                )));
            }
        }
        Ok(())
    }

    /// Check that a set of bound params satisfies this template's schema:
    /// every `required` param is present, and present params match their declared
    /// type. Unknown params (not in the schema) are rejected so typos surface.
    pub fn validate_params(&self, bound: &BTreeMap<String, ParamValue>) -> MotionResult<()> {
        for (name, spec) in &self.params {
            if spec.required && !bound.contains_key(name) {
                return Err(MotionError::manifest(format!(
                    "template {:?} missing required param {name:?}",
                    self.id
                )));
            }
        }
        for (name, value) in bound {
            match self.params.get(name) {
                None => {
                    return Err(MotionError::manifest(format!(
                        "template {:?} got unknown param {name:?}",
                        self.id
                    )));
                }
                Some(spec) => {
                    if !spec.kind.is_empty() && !value.matches_declared(&spec.kind) {
                        return Err(MotionError::manifest(format!(
                            "param {name:?} expects type {:?}",
                            spec.kind
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    /// The effective fps for this template given the project's fps.
    pub fn effective_fps(&self, project_fps: u32) -> u32 {
        match self.fps {
            FpsPolicy::Inherit => project_fps,
            FpsPolicy::Fixed(fps) => fps,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "schema_version": "1.0",
        "id": "lower-third.glass",
        "name": "玻璃拟态下三分之一",
        "description": "A glassmorphism lower-third.",
        "entry": "index.html",
        "author": {"name": "OpenTake"},
        "license": "GPL-3.0-or-later",
        "params": {
            "title": {"type": "string", "required": true, "label": "Title"},
            "subtitle": {"type": "string"},
            "accent": {"type": "color"}
        },
        "duration": {"mode": "fixed", "default_seconds": 5.0},
        "fps": "inherit",
        "transparent": true
    }"#;

    #[test]
    fn full_manifest_decodes_and_validates() {
        let m: MotionPlugin = serde_json::from_str(SAMPLE).unwrap();
        assert_eq!(m.id, "lower-third.glass");
        assert_eq!(m.schema_version, "1.0");
        assert_eq!(m.entry, "index.html");
        assert_eq!(m.params.len(), 3);
        assert!(m.params["title"].required);
        assert_eq!(m.duration.mode, DurationMode::Fixed);
        assert_eq!(m.fps, FpsPolicy::Inherit);
        assert!(m.transparent);
        m.validate().unwrap();
    }

    #[test]
    fn partial_manifest_is_tolerant() {
        let m: MotionPlugin = serde_json::from_str(r#"{"id":"x","name":"X"}"#).unwrap();
        assert_eq!(m.id, "x");
        // defaults filled
        assert_eq!(m.entry, "index.html");
        assert_eq!(m.duration.default_seconds, 5.0);
        assert_eq!(m.fps, FpsPolicy::Inherit);
        assert!(m.transparent);
        assert!(m.params.is_empty());
        m.validate().unwrap();
    }

    #[test]
    fn empty_object_decodes_to_default() {
        let m: MotionPlugin = serde_json::from_str("{}").unwrap();
        assert_eq!(m, MotionPlugin::default());
        // ...but default has no id, so validation fails.
        assert!(m.validate().is_err());
    }

    #[test]
    fn fixed_fps_decodes_and_resolves() {
        let m: MotionPlugin =
            serde_json::from_str(r#"{"id":"a","name":"A","fps":{"fixed":30}}"#).unwrap();
        assert_eq!(m.fps, FpsPolicy::Fixed(30));
        assert_eq!(m.effective_fps(24), 30); // fixed overrides project
        let inherit: MotionPlugin = serde_json::from_str(r#"{"id":"b","name":"B"}"#).unwrap();
        assert_eq!(inherit.effective_fps(24), 24); // inherit follows project
    }

    #[test]
    fn validate_rejects_bad_fps_and_duration() {
        let bad_fps: MotionPlugin =
            serde_json::from_str(r#"{"id":"a","name":"A","fps":{"fixed":0}}"#).unwrap();
        assert!(bad_fps.validate().is_err());

        let bad_dur: MotionPlugin = serde_json::from_str(
            r#"{"id":"a","name":"A","duration":{"mode":"fixed","default_seconds":0}}"#,
        )
        .unwrap();
        assert!(bad_dur.validate().is_err());
    }

    #[test]
    fn validate_rejects_unknown_param_type() {
        let m: MotionPlugin =
            serde_json::from_str(r#"{"id":"a","name":"A","params":{"x":{"type":"vector3"}}}"#)
                .unwrap();
        let err = m.validate().unwrap_err();
        assert!(err.to_string().contains("unknown type"));
    }

    #[test]
    fn validate_params_enforces_required_and_types() {
        let m: MotionPlugin = serde_json::from_str(SAMPLE).unwrap();

        // Missing required `title`.
        let mut bound = BTreeMap::new();
        bound.insert("subtitle".to_string(), ParamValue::String("s".into()));
        assert!(m.validate_params(&bound).is_err());

        // Present + correctly typed.
        bound.insert("title".to_string(), ParamValue::String("T".into()));
        bound.insert("accent".to_string(), ParamValue::Color("#ABCDEF".into()));
        m.validate_params(&bound).unwrap();

        // Wrong type for accent.
        bound.insert("accent".to_string(), ParamValue::Number(1.0));
        assert!(m.validate_params(&bound).is_err());
    }

    #[test]
    fn validate_params_rejects_unknown_param() {
        let m: MotionPlugin = serde_json::from_str(SAMPLE).unwrap();
        let mut bound = BTreeMap::new();
        bound.insert("title".to_string(), ParamValue::String("T".into()));
        bound.insert("nope".to_string(), ParamValue::Bool(true));
        let err = m.validate_params(&bound).unwrap_err();
        assert!(err.to_string().contains("unknown param"));
    }

    #[test]
    fn driven_duration_mode_decodes() {
        let m: MotionPlugin = serde_json::from_str(
            r#"{"id":"bar","name":"Progress","duration":{"mode":"driven","default_seconds":3}}"#,
        )
        .unwrap();
        assert_eq!(m.duration.mode, DurationMode::Driven);
        assert_eq!(m.duration.default_seconds, 3.0);
        m.validate().unwrap();
    }
}
