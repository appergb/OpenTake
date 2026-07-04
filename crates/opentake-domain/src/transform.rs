//! Geometric transform and crop for a clip. 1:1 port of upstream `Transform`
//! and `Crop` from `Timeline.swift`.
//!
//! Coordinates are normalized canvas space (0–1). `center_x` / `center_y` are the
//! clip center; `width` / `height` are normalized size; `rotation` is in degrees,
//! positive = clockwise. The custom `Deserialize` migrates legacy `x` / `y`
//! (top-left-ish) keys to centers using the exact upstream formula
//! `center_x = old_x + width - 0.5` (likewise for y).

use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;

/// Normalized 2D point `(x, y)` in canvas space.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, PartialEq, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transform {
    pub center_x: f64,
    pub center_y: f64,
    pub width: f64,
    pub height: f64,
    /// Degrees, positive = clockwise.
    pub rotation: f64,
    pub flip_horizontal: bool,
    pub flip_vertical: bool,
}

impl Default for Transform {
    fn default() -> Self {
        Transform {
            center_x: 0.5,
            center_y: 0.5,
            width: 1.0,
            height: 1.0,
            rotation: 0.0,
            flip_horizontal: false,
            flip_vertical: false,
        }
    }
}

impl Transform {
    /// Construct from a top-left origin and size (centers are derived).
    pub fn from_top_left(top_left: Point, width: f64, height: f64) -> Self {
        Transform {
            center_x: top_left.x + width / 2.0,
            center_y: top_left.y + height / 2.0,
            width,
            height,
            ..Transform::default()
        }
    }

    /// Construct from a center and size.
    pub fn from_center(center: Point, width: f64, height: f64) -> Self {
        Transform {
            center_x: center.x,
            center_y: center.y,
            width,
            height,
            ..Transform::default()
        }
    }

    /// Top-left corner in normalized canvas space.
    pub fn top_left(&self) -> Point {
        Point {
            x: self.center_x - self.width / 2.0,
            y: self.center_y - self.height / 2.0,
        }
    }

    /// Center in normalized canvas space.
    pub fn center(&self) -> Point {
        Point {
            x: self.center_x,
            y: self.center_y,
        }
    }

    /// Snap a value to canvas boundaries (0 or 1) when within `threshold`.
    pub fn snap_to_boundary(value: f64, threshold: f64) -> f64 {
        if value.abs() < threshold {
            return 0.0;
        }
        if (value - 1.0).abs() < threshold {
            return 1.0;
        }
        value
    }

    /// Snap clip edges to canvas boundaries (0 or 1), preserving size.
    pub fn snap_to_canvas_edges(&mut self, threshold: f64) {
        let tl = self.top_left();
        let snapped_left = Self::snap_to_boundary(tl.x, threshold);
        let snapped_right = Self::snap_to_boundary(tl.x + self.width, threshold);
        if snapped_left != tl.x {
            self.center_x -= tl.x - snapped_left;
        } else if snapped_right != tl.x + self.width {
            self.center_x -= tl.x + self.width - snapped_right;
        }

        let tl2 = self.top_left();
        let snapped_top = Self::snap_to_boundary(tl2.y, threshold);
        let snapped_bottom = Self::snap_to_boundary(tl2.y + self.height, threshold);
        if snapped_top != tl2.y {
            self.center_y -= tl2.y - snapped_top;
        } else if snapped_bottom != tl2.y + self.height {
            self.center_y -= tl2.y + self.height - snapped_bottom;
        }
    }

    /// Snap the center to the canvas center per-axis within thresholds.
    /// Returns `(snapped_x, snapped_y)` so callers can draw guide indicators.
    pub fn snap_center_to_canvas_center(
        &mut self,
        threshold_h: f64,
        threshold_v: f64,
    ) -> (bool, bool) {
        let mut snapped_x = false;
        let mut snapped_y = false;
        if (self.center_x - 0.5).abs() < threshold_h {
            self.center_x = 0.5;
            snapped_x = true;
        }
        if (self.center_y - 0.5).abs() < threshold_v {
            self.center_y = 0.5;
            snapped_y = true;
        }
        (snapped_x, snapped_y)
    }
}

// Custom Deserialize: tolerate missing keys (fall back to defaults) and migrate
// legacy `x` / `y` keys exactly as upstream does:
//   center_x = old_x + width - 0.5   (and same for y)
// Modern `centerX` / `centerY` take precedence over legacy keys when present.
impl<'de> Deserialize<'de> for Transform {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "camelCase")]
        enum Field {
            CenterX,
            CenterY,
            Width,
            Height,
            Rotation,
            FlipHorizontal,
            FlipVertical,
            // Legacy keys
            X,
            Y,
            #[serde(other)]
            Ignore,
        }

        struct TransformVisitor;

        impl<'de> Visitor<'de> for TransformVisitor {
            type Value = Transform;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a Transform object")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Transform, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut center_x: Option<f64> = None;
                let mut center_y: Option<f64> = None;
                let mut width: Option<f64> = None;
                let mut height: Option<f64> = None;
                let mut rotation: Option<f64> = None;
                let mut flip_horizontal: Option<bool> = None;
                let mut flip_vertical: Option<bool> = None;
                let mut old_x: Option<f64> = None;
                let mut old_y: Option<f64> = None;

                while let Some(key) = map.next_key::<Field>()? {
                    match key {
                        Field::CenterX => center_x = Some(map.next_value()?),
                        Field::CenterY => center_y = Some(map.next_value()?),
                        Field::Width => width = Some(map.next_value()?),
                        Field::Height => height = Some(map.next_value()?),
                        Field::Rotation => rotation = Some(map.next_value()?),
                        Field::FlipHorizontal => flip_horizontal = Some(map.next_value()?),
                        Field::FlipVertical => flip_vertical = Some(map.next_value()?),
                        Field::X => old_x = Some(map.next_value()?),
                        Field::Y => old_y = Some(map.next_value()?),
                        Field::Ignore => {
                            let _ = map.next_value::<de::IgnoredAny>()?;
                        }
                    }
                }

                let w = width.unwrap_or(1.0);
                let h = height.unwrap_or(1.0);
                // centerX precedence: modern key, else legacy migration, else default.
                let cx = match (center_x, old_x) {
                    (Some(cx), _) => cx,
                    (None, Some(ox)) => ox + w - 0.5,
                    (None, None) => 0.5,
                };
                let cy = match (center_y, old_y) {
                    (Some(cy), _) => cy,
                    (None, Some(oy)) => oy + h - 0.5,
                    (None, None) => 0.5,
                };

                Ok(Transform {
                    center_x: cx,
                    center_y: cy,
                    width: w,
                    height: h,
                    rotation: rotation.unwrap_or(0.0),
                    flip_horizontal: flip_horizontal.unwrap_or(false),
                    flip_vertical: flip_vertical.unwrap_or(false),
                })
            }
        }

        deserializer.deserialize_map(TransformVisitor)
    }
}

/// Per-clip crop as edge insets in normalized (0–1) source coordinates.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Crop {
    #[serde(default)]
    pub left: f64,
    #[serde(default)]
    pub top: f64,
    #[serde(default)]
    pub right: f64,
    #[serde(default)]
    pub bottom: f64,
}

impl Default for Crop {
    fn default() -> Self {
        Crop {
            left: 0.0,
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
        }
    }
}

impl Crop {
    pub fn is_identity(&self) -> bool {
        self.left == 0.0 && self.top == 0.0 && self.right == 0.0 && self.bottom == 0.0
    }

    pub fn visible_width_fraction(&self) -> f64 {
        (1.0 - self.left - self.right).max(0.0)
    }

    pub fn visible_height_fraction(&self) -> f64 {
        (1.0 - self.top - self.bottom).max(0.0)
    }
}

/// Aspect-ratio constraint for the Crop overlay. 1:1 port of `CropAspectLock`.
/// `label` is a pure-UI string; only `pixel_aspect` (numeric logic) is ported.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CropAspectLock {
    Free,
    Original,
    R16x9,
    R9x16,
    R1x1,
    R4x3,
    R3x4,
    R21x9,
}

impl CropAspectLock {
    pub const ALL: [CropAspectLock; 8] = [
        CropAspectLock::Free,
        CropAspectLock::Original,
        CropAspectLock::R16x9,
        CropAspectLock::R9x16,
        CropAspectLock::R1x1,
        CropAspectLock::R4x3,
        CropAspectLock::R3x4,
        CropAspectLock::R21x9,
    ];

    /// Target pixel aspect ratio, or `None` for free / source-derived locks.
    pub fn pixel_aspect(self) -> Option<f64> {
        match self {
            CropAspectLock::Free | CropAspectLock::Original => None,
            CropAspectLock::R16x9 => Some(16.0 / 9.0),
            CropAspectLock::R9x16 => Some(9.0 / 16.0),
            CropAspectLock::R1x1 => Some(1.0),
            CropAspectLock::R4x3 => Some(4.0 / 3.0),
            CropAspectLock::R3x4 => Some(3.0 / 4.0),
            CropAspectLock::R21x9 => Some(21.0 / 9.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-12, "{a} != {b}");
    }

    #[test]
    fn default_transform_is_centered_full_canvas() {
        let t = Transform::default();
        approx(t.center_x, 0.5);
        approx(t.center_y, 0.5);
        approx(t.width, 1.0);
        approx(t.height, 1.0);
        assert_eq!(t.rotation, 0.0);
        assert!(!t.flip_horizontal && !t.flip_vertical);
    }

    #[test]
    fn top_left_and_center_round_trip() {
        let t = Transform::from_top_left(Point { x: 0.1, y: 0.2 }, 0.4, 0.6);
        approx(t.center_x, 0.1 + 0.2);
        approx(t.center_y, 0.2 + 0.3);
        let tl = t.top_left();
        approx(tl.x, 0.1);
        approx(tl.y, 0.2);
        let c = Transform::from_center(Point { x: 0.5, y: 0.5 }, 0.4, 0.6);
        approx(c.center_x, 0.5);
        approx(c.center_y, 0.5);
    }

    #[test]
    fn legacy_x_y_migration() {
        // old_x=0.0, width=1.0 => center_x = 0.0 + 1.0 - 0.5 = 0.5
        let json = r#"{"x":0.0,"y":0.0,"width":1.0,"height":1.0}"#;
        let t: Transform = serde_json::from_str(json).unwrap();
        approx(t.center_x, 0.5);
        approx(t.center_y, 0.5);

        // old_x=0.25, width=0.5 => center_x = 0.25 + 0.5 - 0.5 = 0.25
        let json2 = r#"{"x":0.25,"y":0.1,"width":0.5,"height":0.5}"#;
        let t2: Transform = serde_json::from_str(json2).unwrap();
        approx(t2.center_x, 0.25);
        approx(t2.center_y, 0.1);
    }

    #[test]
    fn modern_center_keys_take_precedence_over_legacy() {
        let json = r#"{"centerX":0.7,"centerY":0.8,"x":0.0,"y":0.0,"width":0.5,"height":0.5}"#;
        let t: Transform = serde_json::from_str(json).unwrap();
        approx(t.center_x, 0.7);
        approx(t.center_y, 0.8);
    }

    #[test]
    fn missing_keys_fall_back_to_defaults() {
        let t: Transform = serde_json::from_str("{}").unwrap();
        approx(t.center_x, 0.5);
        approx(t.center_y, 0.5);
        approx(t.width, 1.0);
        approx(t.height, 1.0);
    }

    #[test]
    fn serialize_emits_camel_case_keys() {
        let t = Transform::default();
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains("\"centerX\":0.5"));
        assert!(json.contains("\"flipHorizontal\":false"));
        // No legacy keys on the way out.
        assert!(!json.contains("\"x\":"));
    }

    #[test]
    fn full_roundtrip_modern() {
        let t = Transform {
            center_x: 0.3,
            center_y: 0.4,
            width: 0.5,
            height: 0.6,
            rotation: 45.0,
            flip_horizontal: true,
            flip_vertical: false,
        };
        let json = serde_json::to_string(&t).unwrap();
        let back: Transform = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn snap_to_boundary_rules() {
        approx(Transform::snap_to_boundary(0.02, 0.05), 0.0);
        approx(Transform::snap_to_boundary(0.98, 0.05), 1.0);
        approx(Transform::snap_to_boundary(0.5, 0.05), 0.5);
    }

    #[test]
    fn snap_edges_left() {
        // top-left at (0.02, 0.5), width 0.4 -> left edge snaps to 0, center shifts by 0.02
        let mut t = Transform::from_top_left(Point { x: 0.02, y: 0.5 }, 0.4, 0.2);
        t.snap_to_canvas_edges(0.05);
        approx(t.top_left().x, 0.0);
    }

    #[test]
    fn snap_center_returns_axis_flags() {
        let mut t = Transform {
            center_x: 0.51,
            center_y: 0.9,
            ..Transform::default()
        };
        let (sx, sy) = t.snap_center_to_canvas_center(0.05, 0.05);
        assert!(sx);
        assert!(!sy);
        approx(t.center_x, 0.5);
        approx(t.center_y, 0.9);
    }

    #[test]
    fn crop_identity_and_visible_fractions() {
        let c = Crop::default();
        assert!(c.is_identity());
        approx(c.visible_width_fraction(), 1.0);
        approx(c.visible_height_fraction(), 1.0);

        let inset = Crop {
            left: 0.1,
            top: 0.2,
            right: 0.1,
            bottom: 0.2,
        };
        assert!(!inset.is_identity());
        approx(inset.visible_width_fraction(), 0.8);
        approx(inset.visible_height_fraction(), 0.6);

        // Over-inset clamps to 0, never negative.
        let over = Crop {
            left: 0.7,
            top: 0.0,
            right: 0.7,
            bottom: 0.0,
        };
        approx(over.visible_width_fraction(), 0.0);
    }

    #[test]
    fn crop_missing_keys_default_zero() {
        let c: Crop = serde_json::from_str(r#"{"left":0.3}"#).unwrap();
        approx(c.left, 0.3);
        approx(c.top, 0.0);
        approx(c.right, 0.0);
        approx(c.bottom, 0.0);
    }

    #[test]
    fn crop_aspect_pixel_ratios() {
        assert_eq!(CropAspectLock::Free.pixel_aspect(), None);
        assert_eq!(CropAspectLock::Original.pixel_aspect(), None);
        assert_eq!(CropAspectLock::R1x1.pixel_aspect(), Some(1.0));
        approx(CropAspectLock::R16x9.pixel_aspect().unwrap(), 16.0 / 9.0);
        approx(CropAspectLock::R21x9.pixel_aspect().unwrap(), 21.0 / 9.0);
    }
}
