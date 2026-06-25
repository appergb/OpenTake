//! The motion-graphic value types: what to render (`MotionSource`), how to render
//! it (`MotionRenderRequest`), and the result handle (`RenderedClip`).
//!
//! These are pure, serializable, and fully unit-testable — no renderer, no IO.
//! `MotionSource` is what the domain's `MediaSource` gains a `Motion { .. }` arm
//! for (docs/MOTION-GRAPHICS-PLUGIN.md §2); keeping it here lets the domain crate
//! stay dependency-free while this crate owns the web-engine specifics.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::{MotionError, MotionResult};

/// Hard caps on a render request, expressed as types so out-of-range inputs are
/// rejected at the boundary rather than melting an offscreen engine. These mirror
/// the "资源/时长上限" requirement in docs/MOTION-GRAPHICS-PLUGIN.md §5.
pub mod limits {
    /// Max frames in one render (e.g. 60 fps × 60 s). A motion graphic is an
    /// overlay/title, not a feature film — long durations belong on the timeline
    /// as repeated/looped clips, not a single mega-render.
    pub const MAX_FRAMES: u32 = 3600;
    /// Max canvas edge in pixels (covers 4K either orientation, with headroom).
    pub const MAX_DIMENSION: u32 = 4096;
    /// Min canvas edge — the compositor's even-ization floors at 2; we keep 2.
    pub const MIN_DIMENSION: u32 = 2;
    /// Max fps. Above this is never useful for video and only inflates work.
    pub const MAX_FPS: u32 = 240;
}

/// A motion-graphic parameter value. Templates declare a typed schema (see
/// [`crate::manifest`]); instances bind concrete values. Kept deliberately small
/// — strings/numbers/bools/colors cover lower-thirds, data callouts, titles.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "value")]
pub enum ParamValue {
    /// Free text (titles, subtitles, labels).
    String(String),
    /// Any finite number (counts, durations, sizes the template interprets).
    Number(f64),
    /// Toggle (show/hide a sub-element).
    Bool(bool),
    /// A color as `#RRGGBB` / `#RRGGBBAA` (validated on bind, see
    /// [`MotionSource::validate`]).
    Color(String),
}

impl ParamValue {
    /// `true` when this value satisfies a declared param `type` string from a
    /// template manifest (`"string" | "number" | "bool" | "color"`). Unknown
    /// declared types are treated as "accept anything" so a newer manifest does
    /// not hard-fail an older host.
    pub fn matches_declared(&self, declared: &str) -> bool {
        match declared {
            "string" => matches!(self, ParamValue::String(_)),
            "number" => matches!(self, ParamValue::Number(_)),
            "bool" | "boolean" => matches!(self, ParamValue::Bool(_)),
            "color" => matches!(self, ParamValue::Color(_)),
            _ => true,
        }
    }
}

/// Where a motion graphic's animation comes from. The two arms map 1:1 to the
/// two usage levels in docs/MOTION-GRAPHICS-PLUGIN.md §4:
/// - `Code` — the agent writes a self-contained HTML/CSS/JS animation inline
///   ("即兴模式").
/// - `Template` — instantiate a registered plugin by id with bound params
///   ("模板/插件模式").
///
/// Externally tagged so it round-trips compactly and unambiguously.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MotionSource {
    /// Inline, self-contained web document. The string is the full HTML (it may
    /// embed `<style>` / `<script>`). The deterministic clock is injected by the
    /// renderer (see [`crate::renderer`]); authors animate against
    /// `OpenTake.seek(seconds)` / `document.timeline.currentTime`.
    Code {
        /// The HTML/CSS/JS document text.
        html_css_js: String,
    },
    /// Instantiate a registered template by id with concrete parameter bindings.
    Template {
        /// Registered template id (matches a [`crate::manifest::MotionPlugin`] id).
        id: String,
        /// Parameter bindings (`name -> value`). `BTreeMap` for a stable,
        /// hash-stable iteration order (the cache key depends on it).
        #[serde(default)]
        params: BTreeMap<String, ParamValue>,
    },
}

impl MotionSource {
    /// Convenience constructor for an inline document.
    pub fn code(html_css_js: impl Into<String>) -> Self {
        MotionSource::Code {
            html_css_js: html_css_js.into(),
        }
    }

    /// Convenience constructor for a template instance with no params.
    pub fn template(id: impl Into<String>) -> Self {
        MotionSource::Template {
            id: id.into(),
            params: BTreeMap::new(),
        }
    }

    /// Validate the source in isolation (no template-schema cross-check — that is
    /// [`crate::manifest::MotionPlugin::validate_params`]'s job once the template
    /// is resolved). Catches empty code and malformed color params early.
    pub fn validate(&self) -> MotionResult<()> {
        match self {
            MotionSource::Code { html_css_js } => {
                if html_css_js.trim().is_empty() {
                    return Err(MotionError::invalid_source("inline code is empty"));
                }
                Ok(())
            }
            MotionSource::Template { id, params } => {
                if id.trim().is_empty() {
                    return Err(MotionError::invalid_source("template id is empty"));
                }
                for (name, value) in params {
                    if let ParamValue::Color(c) = value {
                        if !is_hex_color(c) {
                            return Err(MotionError::invalid_source(format!(
                                "param '{name}' is not a valid hex color: {c:?}"
                            )));
                        }
                    }
                }
                Ok(())
            }
        }
    }
}

/// `true` for `#RGB`-style hex colors: `#RRGGBB` or `#RRGGBBAA` (case-insensitive).
pub(crate) fn is_hex_color(s: &str) -> bool {
    let Some(hexpart) = s.strip_prefix('#') else {
        return false;
    };
    matches!(hexpart.len(), 6 | 8) && hexpart.bytes().all(|b| b.is_ascii_hexdigit())
}

/// A deterministic render request: the source plus the exact frame grid and
/// canvas to rasterize. Every field here participates in the content-hash cache
/// key (see [`crate::cache`]) — same request ⇒ same bytes ⇒ cache hit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionRenderRequest {
    /// What to render.
    pub source: MotionSource,
    /// Timeline frames per second (the project fps the clip is composited at).
    pub fps: u32,
    /// Number of frames to produce. Frame `i` is captured at `t = i / fps`
    /// seconds of virtual time (docs §3).
    pub duration_frames: u32,
    /// Canvas width in pixels.
    pub width: u32,
    /// Canvas height in pixels.
    pub height: u32,
    /// Whether to capture a straight-alpha RGBA overlay (transparent body) so the
    /// result composites over other layers. `false` renders an opaque clip.
    pub transparent: bool,
}

impl MotionRenderRequest {
    /// Build a request, defaulting `transparent` to `true` (the overlay case is
    /// the dominant one for motion graphics).
    pub fn new(
        source: MotionSource,
        fps: u32,
        duration_frames: u32,
        width: u32,
        height: u32,
    ) -> Self {
        MotionRenderRequest {
            source,
            fps,
            duration_frames,
            width,
            height,
            transparent: true,
        }
    }

    /// Builder-style override of the transparency flag.
    pub fn with_transparent(mut self, transparent: bool) -> Self {
        self.transparent = transparent;
        self
    }

    /// Validate ranges against [`limits`] and the source. Pure; call before
    /// handing the request to any renderer.
    pub fn validate(&self) -> MotionResult<()> {
        self.source.validate()?;
        if self.fps == 0 || self.fps > limits::MAX_FPS {
            return Err(MotionError::invalid_request(format!(
                "fps {} out of range 1..={}",
                self.fps,
                limits::MAX_FPS
            )));
        }
        if self.duration_frames == 0 || self.duration_frames > limits::MAX_FRAMES {
            return Err(MotionError::invalid_request(format!(
                "durationFrames {} out of range 1..={}",
                self.duration_frames,
                limits::MAX_FRAMES
            )));
        }
        for (label, dim) in [("width", self.width), ("height", self.height)] {
            if !(limits::MIN_DIMENSION..=limits::MAX_DIMENSION).contains(&dim) {
                return Err(MotionError::invalid_request(format!(
                    "{label} {dim} out of range {}..={}",
                    limits::MIN_DIMENSION,
                    limits::MAX_DIMENSION
                )));
            }
        }
        Ok(())
    }

    /// Total virtual-time duration in seconds (`duration_frames / fps`).
    pub fn duration_seconds(&self) -> f64 {
        self.duration_frames as f64 / self.fps as f64
    }
}

/// The product of a successful render: a sequence of on-disk RGBA frame files plus
/// the metadata the compositor needs to treat the clip as a texture source.
///
/// Frames live on disk (not in memory) because a motion clip can be thousands of
/// 4K RGBA frames; the compositor pulls them lazily via [`crate::integration`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderedClip {
    /// The content-hash key this clip was rendered under (cache directory name).
    pub content_hash: String,
    /// Absolute paths to each frame file, in playback order (`frames[i]` is the
    /// frame at `t = i / fps`). Format is PNG (RGBA, straight alpha when
    /// `transparent`).
    pub frames: Vec<std::path::PathBuf>,
    /// Frames per second the clip was rendered at (== request fps).
    pub fps: u32,
    /// Canvas width in pixels.
    pub width: u32,
    /// Canvas height in pixels.
    pub height: u32,
    /// Whether the frames carry straight alpha (overlay) vs. opaque.
    pub transparent: bool,
}

impl RenderedClip {
    /// Number of frames (== `duration_frames` of the request that produced it).
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Playback duration in seconds.
    pub fn duration_seconds(&self) -> f64 {
        if self.fps == 0 {
            return 0.0;
        }
        self.frames.len() as f64 / self.fps as f64
    }

    /// The frame path for a 0-based frame index, clamped to the last frame so a
    /// timeline clip held past its natural end freezes on the final frame rather
    /// than failing (mirrors the Lottie/image hold behavior elsewhere).
    pub fn frame_path(&self, frame: usize) -> Option<&std::path::Path> {
        if self.frames.is_empty() {
            return None;
        }
        let idx = frame.min(self.frames.len() - 1);
        Some(self.frames[idx].as_path())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_color_recognizes_rgb_and_rgba() {
        assert!(is_hex_color("#00FF00"));
        assert!(is_hex_color("#00ff00aa"));
        assert!(!is_hex_color("00FF00")); // no hash
        assert!(!is_hex_color("#fff")); // 3-digit not accepted
        assert!(!is_hex_color("#GGGGGG")); // non-hex
        assert!(!is_hex_color("#00FF0")); // 5 digits
    }

    #[test]
    fn source_code_roundtrips_and_validates() {
        let s = MotionSource::code("<div>hi</div>");
        let json = serde_json::to_string(&s).unwrap();
        let back: MotionSource = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
        assert!(s.validate().is_ok());
    }

    #[test]
    fn empty_code_is_rejected() {
        let s = MotionSource::code("   \n  ");
        let err = s.validate().unwrap_err();
        assert!(err.to_string().contains("inline code is empty"));
    }

    #[test]
    fn template_with_params_roundtrips() {
        let mut params = BTreeMap::new();
        params.insert("title".to_string(), ParamValue::String("Hello".into()));
        params.insert("accent".to_string(), ParamValue::Color("#FF0066".into()));
        params.insert("count".to_string(), ParamValue::Number(42.0));
        let s = MotionSource::Template {
            id: "lower-third.glass".into(),
            params,
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: MotionSource = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
        assert!(s.validate().is_ok());
    }

    #[test]
    fn template_bad_color_param_is_rejected() {
        let mut params = BTreeMap::new();
        params.insert("accent".to_string(), ParamValue::Color("red".into()));
        let s = MotionSource::Template {
            id: "x".into(),
            params,
        };
        let err = s.validate().unwrap_err();
        assert!(err.to_string().contains("not a valid hex color"));
    }

    #[test]
    fn empty_template_id_is_rejected() {
        let s = MotionSource::template("");
        assert!(s.validate().is_err());
    }

    #[test]
    fn param_value_matches_declared_type() {
        assert!(ParamValue::String("x".into()).matches_declared("string"));
        assert!(ParamValue::Number(1.0).matches_declared("number"));
        assert!(ParamValue::Bool(true).matches_declared("bool"));
        assert!(ParamValue::Bool(true).matches_declared("boolean"));
        assert!(ParamValue::Color("#fff000".into()).matches_declared("color"));
        // wrong type
        assert!(!ParamValue::String("x".into()).matches_declared("number"));
        // unknown declared type accepts anything (forward-compat)
        assert!(ParamValue::Number(1.0).matches_declared("future_type"));
    }

    #[test]
    fn request_validate_accepts_reasonable_values() {
        let req = MotionRenderRequest::new(MotionSource::code("<b>x</b>"), 30, 150, 1920, 1080);
        assert!(req.validate().is_ok());
        assert!(req.transparent);
        assert!((req.duration_seconds() - 5.0).abs() < 1e-9);
    }

    #[test]
    fn request_rejects_zero_fps_and_overlong_duration() {
        let bad_fps = MotionRenderRequest::new(MotionSource::code("x"), 0, 10, 100, 100);
        assert!(bad_fps.validate().is_err());

        let bad_dur = MotionRenderRequest::new(
            MotionSource::code("x"),
            30,
            limits::MAX_FRAMES + 1,
            100,
            100,
        );
        assert!(bad_dur.validate().is_err());
    }

    #[test]
    fn request_rejects_out_of_range_dimensions() {
        let too_big = MotionRenderRequest::new(
            MotionSource::code("x"),
            30,
            10,
            limits::MAX_DIMENSION + 1,
            100,
        );
        assert!(too_big.validate().is_err());
        let too_small = MotionRenderRequest::new(MotionSource::code("x"), 30, 10, 1, 100);
        assert!(too_small.validate().is_err());
    }

    #[test]
    fn rendered_clip_frame_path_clamps_and_reports_duration() {
        let clip = RenderedClip {
            content_hash: "abc".into(),
            frames: vec!["/c/0.png".into(), "/c/1.png".into(), "/c/2.png".into()],
            fps: 3,
            width: 10,
            height: 10,
            transparent: true,
        };
        assert_eq!(clip.frame_count(), 3);
        assert!((clip.duration_seconds() - 1.0).abs() < 1e-9);
        assert_eq!(clip.frame_path(0).unwrap().to_str(), Some("/c/0.png"));
        // past the end clamps to the last frame (freeze-frame hold)
        assert_eq!(clip.frame_path(99).unwrap().to_str(), Some("/c/2.png"));
    }

    #[test]
    fn rendered_clip_empty_frames_has_no_path() {
        let clip = RenderedClip {
            content_hash: "x".into(),
            frames: vec![],
            fps: 30,
            width: 10,
            height: 10,
            transparent: false,
        };
        assert!(clip.frame_path(0).is_none());
        assert_eq!(clip.duration_seconds(), 0.0);
    }
}
