//! Clip / media type discriminant. 1:1 port of upstream `ClipType.swift`.
//!
//! Swift: `enum ClipType: String, Codable, CaseIterable`. The JSON wire form is
//! the lowercase case name (`"video"`, `"audio"`, ...), matching Swift's default
//! `RawRepresentable` Codable. SF Symbol names and track labels are pure-UI
//! mappings rebuilt in the frontend/render layer and are intentionally omitted.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClipType {
    Video,
    Audio,
    Image,
    Text,
    Lottie,
}

impl ClipType {
    /// All cases, in declaration order (mirrors Swift `CaseIterable.allCases`).
    pub const ALL: [ClipType; 5] = [
        ClipType::Video,
        ClipType::Audio,
        ClipType::Image,
        ClipType::Text,
        ClipType::Lottie,
    ];

    /// Visual types occupy video tracks and contribute pixels to the canvas.
    pub fn is_visual(self) -> bool {
        matches!(
            self,
            ClipType::Video | ClipType::Image | ClipType::Text | ClipType::Lottie
        )
    }

    /// Two types are track-compatible when identical, or both visual.
    pub fn is_compatible(self, other: ClipType) -> bool {
        self == other || (self.is_visual() && other.is_visual())
    }

    /// Infer a clip type from a lowercase file extension. Returns `None` for
    /// unknown extensions (mirrors Swift's failable `init?(fileExtension:)`).
    pub fn from_file_extension(ext: &str) -> Option<ClipType> {
        match ext {
            "mov" | "mp4" | "m4v" => Some(ClipType::Video),
            "mp3" | "wav" | "aac" | "m4a" => Some(ClipType::Audio),
            "png" | "jpg" | "jpeg" | "tiff" | "heic" | "webp" => Some(ClipType::Image),
            "json" | "lottie" => Some(ClipType::Lottie),
            _ => None,
        }
    }
}

impl Default for ClipType {
    /// Upstream defaults `mediaType` / `sourceClipType` to `.video`.
    fn default() -> Self {
        ClipType::Video
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_to_lowercase_case_name() {
        assert_eq!(
            serde_json::to_string(&ClipType::Video).unwrap(),
            "\"video\""
        );
        assert_eq!(
            serde_json::to_string(&ClipType::Lottie).unwrap(),
            "\"lottie\""
        );
    }

    #[test]
    fn roundtrips_through_json() {
        for t in ClipType::ALL {
            let json = serde_json::to_string(&t).unwrap();
            let back: ClipType = serde_json::from_str(&json).unwrap();
            assert_eq!(t, back);
        }
    }

    #[test]
    fn visual_classification_matches_upstream() {
        assert!(ClipType::Video.is_visual());
        assert!(ClipType::Image.is_visual());
        assert!(ClipType::Text.is_visual());
        assert!(ClipType::Lottie.is_visual());
        assert!(!ClipType::Audio.is_visual());
    }

    #[test]
    fn compatibility_rules() {
        // identical
        assert!(ClipType::Audio.is_compatible(ClipType::Audio));
        // both visual
        assert!(ClipType::Video.is_compatible(ClipType::Text));
        assert!(ClipType::Image.is_compatible(ClipType::Lottie));
        // visual vs audio
        assert!(!ClipType::Video.is_compatible(ClipType::Audio));
        assert!(!ClipType::Audio.is_compatible(ClipType::Image));
    }

    #[test]
    fn file_extension_mapping() {
        assert_eq!(ClipType::from_file_extension("mp4"), Some(ClipType::Video));
        assert_eq!(ClipType::from_file_extension("m4a"), Some(ClipType::Audio));
        assert_eq!(ClipType::from_file_extension("heic"), Some(ClipType::Image));
        assert_eq!(
            ClipType::from_file_extension("lottie"),
            Some(ClipType::Lottie)
        );
        assert_eq!(
            ClipType::from_file_extension("json"),
            Some(ClipType::Lottie)
        );
        assert_eq!(ClipType::from_file_extension("xyz"), None);
    }
}
