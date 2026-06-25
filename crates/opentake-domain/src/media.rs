//! Media manifest, sources, folders, generation input, and the runtime
//! `MediaAsset`. Ports `MediaManifest.swift`, `MediaFolder.swift`,
//! `MediaResolver.swift`, and the data/derived parts of `MediaAsset.swift`.
//!
//! Field names are matched to Swift's default `JSONEncoder` output (property
//! names verbatim), so abbreviation casings like `imageURLs`, `sourceFPS`, and
//! `cachedRemoteURL` use explicit `#[serde(rename = ...)]` rather than the
//! container's `camelCase` (which would lowercase the abbreviations).
//!
//! Dates: upstream `JSONEncoder` emits `Date` as seconds since the Apple
//! reference date (2001-01-01). To keep this crate zero-dependency and
//! byte-compatible with existing project files, dates are `f64` (those same
//! seconds). The render/project layer converts to/from wall-clock time.
//!
//! Zero-IO rule: `MediaResolver` here only computes expected paths and queries
//! the manifest. Filesystem existence checks (`resolveURL` / `isMissing`) belong
//! to the project/media layer and are intentionally NOT ported here.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::clip_type::ClipType;

/// Where a media file lives. Encoded externally-tagged to match Swift's
/// synthesized `Codable` for an enum with associated values:
/// `{"external":{"absolutePath":"..."}}` / `{"project":{"relativePath":"..."}}`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MediaSource {
    #[serde(rename_all = "camelCase")]
    External { absolute_path: String },
    #[serde(rename_all = "camelCase")]
    Project { relative_path: String },
}

/// Full serializable input snapshot for a generated asset. 1:1 port of
/// `GenerationInput`. `prompt` / `model` / `duration` / `aspect_ratio` are
/// required upstream; everything else is optional.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationInput {
    pub prompt: String,
    pub model: String,
    pub duration: i32,
    pub aspect_ratio: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<String>,
    #[serde(rename = "imageURLs", default, skip_serializing_if = "Option::is_none")]
    pub image_urls: Option<Vec<String>>,
    /// Image-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_images: Option<i32>,
    /// Audio-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lyrics: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style_instructions: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instrumental: Option<bool>,
    /// Video-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generate_audio: Option<bool>,
    #[serde(
        rename = "referenceImageURLs",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub reference_image_urls: Option<Vec<String>>,
    #[serde(
        rename = "referenceVideoURLs",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub reference_video_urls: Option<Vec<String>>,
    #[serde(
        rename = "referenceAudioURLs",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub reference_audio_urls: Option<Vec<String>>,
    /// Asset IDs for the references.
    #[serde(
        rename = "imageURLAssetIds",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub image_url_asset_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_image_asset_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_video_asset_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_audio_asset_ids: Option<Vec<String>>,
    /// Apple-reference-date seconds (see module note on dates).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<f64>,
}

/// Serializable manifest entry. 1:1 port of `MediaManifestEntry`.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaManifestEntry {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: ClipType,
    pub source: MediaSource,
    pub duration: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_input: Option<GenerationInput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_width: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_height: Option<i32>,
    #[serde(rename = "sourceFPS", default, skip_serializing_if = "Option::is_none")]
    pub source_fps: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_audio: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<String>,
    #[serde(
        rename = "cachedRemoteURL",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub cached_remote_url: Option<String>,
    #[serde(
        rename = "cachedRemoteURLExpiresAt",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub cached_remote_url_expires_at: Option<f64>,
}

/// A media library folder. 1:1 port of `MediaFolder`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaFolder {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_folder_id: Option<String>,
}

impl MediaFolder {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        MediaFolder {
            id: id.into(),
            name: name.into(),
            parent_folder_id: None,
        }
    }
}

/// The media manifest. 1:1 port of `MediaManifest`. The default `version` is 2,
/// but a *missing* `version` on decode falls back to 1 (matching upstream's
/// custom decoder), so the custom `Deserialize` below is required.
#[derive(Clone, PartialEq, Debug, Serialize)]
pub struct MediaManifest {
    pub version: i64,
    pub entries: Vec<MediaManifestEntry>,
    pub folders: Vec<MediaFolder>,
}

impl Default for MediaManifest {
    fn default() -> Self {
        MediaManifest {
            version: 2,
            entries: Vec::new(),
            folders: Vec::new(),
        }
    }
}

impl MediaManifest {
    pub fn new() -> Self {
        MediaManifest::default()
    }
}

impl<'de> Deserialize<'de> for MediaManifest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Raw {
            // Missing version => 1 (NOT the struct default of 2), per upstream.
            version: Option<i64>,
            entries: Option<Vec<MediaManifestEntry>>,
            folders: Option<Vec<MediaFolder>>,
        }
        let raw = Raw::deserialize(deserializer)?;
        Ok(MediaManifest {
            version: raw.version.unwrap_or(1),
            entries: raw.entries.unwrap_or_default(),
            folders: raw.folders.unwrap_or_default(),
        })
    }
}

/// Resolves asset IDs against a manifest. Zero-IO: this computes *expected*
/// paths only. Filesystem existence checks live in the project/media layer.
/// 1:1 port of the pure parts of `MediaResolver`.
pub struct MediaResolver<'a> {
    manifest: &'a MediaManifest,
    project_base: Option<&'a Path>,
}

impl<'a> MediaResolver<'a> {
    pub fn new(manifest: &'a MediaManifest, project_base: Option<&'a Path>) -> Self {
        MediaResolver {
            manifest,
            project_base,
        }
    }

    /// The manifest entry for `asset_id`, if present.
    pub fn entry(&self, asset_id: &str) -> Option<&MediaManifestEntry> {
        self.manifest.entries.iter().find(|e| e.id == asset_id)
    }

    /// Display name for `asset_id`, or `"Offline"` when unknown (upstream string).
    pub fn display_name(&self, asset_id: &str) -> String {
        self.entry(asset_id)
            .map(|e| e.name.clone())
            .unwrap_or_else(|| "Offline".to_string())
    }

    /// Expected on-disk path for `asset_id`. `External` returns its absolute
    /// path; `Project` joins the relative path onto `project_base`. Returns
    /// `None` when the asset is unknown, or when a project asset has no base.
    pub fn expected_path(&self, asset_id: &str) -> Option<PathBuf> {
        let entry = self.entry(asset_id)?;
        match &entry.source {
            MediaSource::External { absolute_path } => Some(PathBuf::from(absolute_path)),
            MediaSource::Project { relative_path } => {
                self.project_base.map(|base| base.join(relative_path))
            }
        }
    }
}

/// Generation lifecycle state. 1:1 port of `MediaAsset.GenerationStatus`. Not
/// persisted upstream; serde here is for in-process use only.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GenerationStatus {
    #[default]
    None,
    Generating,
    Downloading,
    Rendering,
    Failed(String),
}

/// Runtime media object. 1:1 port of the *data* on `MediaAsset` plus its derived
/// helpers. AppKit/AVFoundation members (`thumbnail: NSImage`, `loadMetadata`)
/// are platform-bound and rebuilt in the media layer (FFmpeg); they are omitted.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaAsset {
    pub id: String,
    pub url: PathBuf,
    #[serde(rename = "type")]
    pub kind: ClipType,
    pub name: String,
    #[serde(default)]
    pub duration: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_width: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_height: Option<i32>,
    #[serde(rename = "sourceFPS", default, skip_serializing_if = "Option::is_none")]
    pub source_fps: Option<f64>,
    #[serde(default)]
    pub has_audio: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_input: Option<GenerationInput>,
    #[serde(default)]
    pub generation_status: GenerationStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_download_url: Option<PathBuf>,
    #[serde(
        rename = "cachedRemoteURL",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub cached_remote_url: Option<String>,
    #[serde(
        rename = "cachedRemoteURLExpiresAt",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub cached_remote_url_expires_at: Option<f64>,
}

impl MediaAsset {
    /// Construct a fresh asset. `has_audio` mirrors upstream: video defaults to
    /// having audio until metadata says otherwise.
    pub fn new(
        id: impl Into<String>,
        url: impl Into<PathBuf>,
        kind: ClipType,
        name: impl Into<String>,
        duration: f64,
    ) -> Self {
        MediaAsset {
            id: id.into(),
            url: url.into(),
            kind,
            name: name.into(),
            duration,
            source_width: None,
            source_height: None,
            source_fps: None,
            has_audio: kind == ClipType::Video,
            generation_input: None,
            generation_status: GenerationStatus::None,
            folder_id: None,
            pending_download_url: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        }
    }

    /// Reconstruct a runtime asset from a manifest entry and a resolved path.
    /// 1:1 port of `init(entry:resolvedURL:)`.
    pub fn from_entry(entry: &MediaManifestEntry, resolved_url: impl Into<PathBuf>) -> Self {
        MediaAsset {
            id: entry.id.clone(),
            url: resolved_url.into(),
            kind: entry.kind,
            name: entry.name.clone(),
            duration: entry.duration,
            source_width: entry.source_width,
            source_height: entry.source_height,
            source_fps: entry.source_fps,
            has_audio: entry.has_audio.unwrap_or(false),
            generation_input: entry.generation_input.clone(),
            generation_status: GenerationStatus::None,
            folder_id: entry.folder_id.clone(),
            pending_download_url: None,
            cached_remote_url: entry.cached_remote_url.clone(),
            cached_remote_url_expires_at: entry.cached_remote_url_expires_at,
        }
    }

    pub fn is_generated(&self) -> bool {
        self.generation_input.is_some()
    }

    pub fn is_generating(&self) -> bool {
        matches!(
            self.generation_status,
            GenerationStatus::Generating
                | GenerationStatus::Downloading
                | GenerationStatus::Rendering
        )
    }

    /// The cached remote URL iff it is set AND not expired at `now` (both in
    /// Apple-reference-date seconds). 1:1 port of `freshRemoteURL`, with `now`
    /// injected to keep this crate clock-free.
    pub fn fresh_remote_url(&self, now: f64) -> Option<&str> {
        let url = self.cached_remote_url.as_deref()?;
        let expires_at = self.cached_remote_url_expires_at?;
        if expires_at > now {
            Some(url)
        } else {
            None
        }
    }

    /// Produce a serializable manifest entry. `Project` when `url` is inside
    /// `project_base`, else `External`. Expired cached URLs are dropped (their
    /// expiry too). 1:1 port of `toManifestEntry(projectURL:)`, with `now`
    /// injected for the freshness check.
    pub fn to_manifest_entry(&self, project_base: Option<&Path>, now: f64) -> MediaManifestEntry {
        let source = match project_base {
            Some(base) if self.url.starts_with(base) => {
                let relative = self
                    .url
                    .strip_prefix(base)
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default();
                MediaSource::Project {
                    relative_path: relative,
                }
            }
            _ => MediaSource::External {
                absolute_path: self.url.to_string_lossy().into_owned(),
            },
        };
        let fresh = self.fresh_remote_url(now).map(|s| s.to_string());
        let expires = if fresh.is_none() {
            None
        } else {
            self.cached_remote_url_expires_at
        };
        MediaManifestEntry {
            id: self.id.clone(),
            name: self.name.clone(),
            kind: self.kind,
            source,
            duration: self.duration,
            generation_input: self.generation_input.clone(),
            source_width: self.source_width,
            source_height: self.source_height,
            source_fps: self.source_fps,
            has_audio: Some(self.has_audio),
            folder_id: self.folder_id.clone(),
            cached_remote_url: fresh,
            cached_remote_url_expires_at: expires,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- MediaSource ---

    #[test]
    fn media_source_external_wire_format() {
        let s = MediaSource::External {
            absolute_path: "/abs/x.mp4".to_string(),
        };
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, r#"{"external":{"absolutePath":"/abs/x.mp4"}}"#);
        let back: MediaSource = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn media_source_project_wire_format() {
        let s = MediaSource::Project {
            relative_path: "media/clip.mov".to_string(),
        };
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, r#"{"project":{"relativePath":"media/clip.mov"}}"#);
        let back: MediaSource = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    // --- MediaManifest version fallback ---

    #[test]
    fn manifest_missing_version_falls_back_to_one() {
        let m: MediaManifest = serde_json::from_str(r#"{"entries":[],"folders":[]}"#).unwrap();
        assert_eq!(m.version, 1);
    }

    #[test]
    fn manifest_empty_object_decodes() {
        let m: MediaManifest = serde_json::from_str("{}").unwrap();
        assert_eq!(m.version, 1);
        assert!(m.entries.is_empty());
        assert!(m.folders.is_empty());
    }

    #[test]
    fn manifest_default_version_is_two() {
        assert_eq!(MediaManifest::default().version, 2);
    }

    #[test]
    fn manifest_explicit_version_preserved() {
        let m: MediaManifest =
            serde_json::from_str(r#"{"version":2,"entries":[],"folders":[]}"#).unwrap();
        assert_eq!(m.version, 2);
    }

    // --- GenerationInput abbreviation casing ---

    #[test]
    fn generation_input_preserves_abbreviation_casing() {
        let gi = GenerationInput {
            prompt: "p".into(),
            model: "m".into(),
            duration: 5,
            aspect_ratio: "16:9".into(),
            image_urls: Some(vec!["u1".into()]),
            reference_image_urls: Some(vec!["r1".into()]),
            image_url_asset_ids: Some(vec!["a1".into()]),
            ..Default::default()
        };
        let json = serde_json::to_string(&gi).unwrap();
        assert!(json.contains("\"imageURLs\":[\"u1\"]"));
        assert!(json.contains("\"referenceImageURLs\":[\"r1\"]"));
        assert!(json.contains("\"imageURLAssetIds\":[\"a1\"]"));
        assert!(json.contains("\"aspectRatio\":\"16:9\""));
        let back: GenerationInput = serde_json::from_str(&json).unwrap();
        assert_eq!(gi, back);
    }

    #[test]
    fn generation_input_omits_none_fields() {
        let gi = GenerationInput {
            prompt: "p".into(),
            model: "m".into(),
            duration: 5,
            aspect_ratio: "1:1".into(),
            ..Default::default()
        };
        let json = serde_json::to_string(&gi).unwrap();
        assert!(!json.contains("imageURLs"));
        assert!(!json.contains("voice"));
    }

    // --- MediaManifestEntry ---

    #[test]
    fn manifest_entry_source_fps_casing_and_roundtrip() {
        let e = MediaManifestEntry {
            id: "id1".into(),
            name: "Clip".into(),
            kind: ClipType::Video,
            source: MediaSource::Project {
                relative_path: "media/a.mov".into(),
            },
            duration: 3.5,
            generation_input: None,
            source_width: Some(1920),
            source_height: Some(1080),
            source_fps: Some(30.0),
            has_audio: Some(true),
            folder_id: None,
            cached_remote_url: Some("https://x".into()),
            cached_remote_url_expires_at: Some(700_000_000.0),
        };
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("\"sourceFPS\":30.0"));
        assert!(json.contains("\"cachedRemoteURL\":\"https://x\""));
        assert!(json.contains("\"cachedRemoteURLExpiresAt\":700000000.0"));
        assert!(json.contains("\"type\":\"video\""));
        let back: MediaManifestEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(e, back);
    }

    // --- MediaFolder ---

    #[test]
    fn folder_roundtrip() {
        let mut f = MediaFolder::new("f1", "B-Roll");
        f.parent_folder_id = Some("root".into());
        let json = serde_json::to_string(&f).unwrap();
        assert!(json.contains("\"parentFolderId\":\"root\""));
        let back: MediaFolder = serde_json::from_str(&json).unwrap();
        assert_eq!(f, back);
    }

    // --- MediaResolver ---

    fn sample_manifest() -> MediaManifest {
        let mut m = MediaManifest::new();
        m.entries.push(MediaManifestEntry {
            id: "ext".into(),
            name: "External".into(),
            kind: ClipType::Video,
            source: MediaSource::External {
                absolute_path: "/abs/ext.mp4".into(),
            },
            duration: 1.0,
            generation_input: None,
            source_width: None,
            source_height: None,
            source_fps: None,
            has_audio: None,
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        });
        m.entries.push(MediaManifestEntry {
            id: "proj".into(),
            name: "Project".into(),
            kind: ClipType::Image,
            source: MediaSource::Project {
                relative_path: "media/p.png".into(),
            },
            duration: 5.0,
            generation_input: None,
            source_width: None,
            source_height: None,
            source_fps: None,
            has_audio: None,
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        });
        m
    }

    #[test]
    fn resolver_expected_path_external() {
        let m = sample_manifest();
        let r = MediaResolver::new(&m, Some(Path::new("/proj")));
        assert_eq!(r.expected_path("ext"), Some(PathBuf::from("/abs/ext.mp4")));
    }

    #[test]
    fn resolver_expected_path_project_joins_base() {
        let m = sample_manifest();
        let r = MediaResolver::new(&m, Some(Path::new("/proj")));
        assert_eq!(
            r.expected_path("proj"),
            Some(PathBuf::from("/proj/media/p.png"))
        );
    }

    #[test]
    fn resolver_project_without_base_is_none() {
        let m = sample_manifest();
        let r = MediaResolver::new(&m, None);
        assert_eq!(r.expected_path("proj"), None);
        // external still resolves without a base
        assert_eq!(r.expected_path("ext"), Some(PathBuf::from("/abs/ext.mp4")));
    }

    #[test]
    fn resolver_unknown_id() {
        let m = sample_manifest();
        let r = MediaResolver::new(&m, Some(Path::new("/proj")));
        assert_eq!(r.expected_path("nope"), None);
        assert_eq!(r.display_name("nope"), "Offline");
        assert_eq!(r.display_name("proj"), "Project");
    }

    // --- MediaAsset ---

    #[test]
    fn asset_video_has_audio_by_default() {
        let a = MediaAsset::new("a", "/x.mp4", ClipType::Video, "X", 1.0);
        assert!(a.has_audio);
        let b = MediaAsset::new("b", "/x.png", ClipType::Image, "Y", 5.0);
        assert!(!b.has_audio);
    }

    #[test]
    fn asset_is_generated_and_generating() {
        let mut a = MediaAsset::new("a", "/x.mp4", ClipType::Video, "X", 1.0);
        assert!(!a.is_generated());
        assert!(!a.is_generating());
        a.generation_input = Some(GenerationInput {
            prompt: "p".into(),
            model: "m".into(),
            duration: 5,
            aspect_ratio: "16:9".into(),
            ..Default::default()
        });
        assert!(a.is_generated());
        a.generation_status = GenerationStatus::Downloading;
        assert!(a.is_generating());
        a.generation_status = GenerationStatus::Failed("boom".into());
        assert!(!a.is_generating());
    }

    #[test]
    fn fresh_remote_url_respects_expiry() {
        let mut a = MediaAsset::new("a", "/x.mp4", ClipType::Video, "X", 1.0);
        a.cached_remote_url = Some("https://cdn/x".into());
        a.cached_remote_url_expires_at = Some(1000.0);
        assert_eq!(a.fresh_remote_url(999.0), Some("https://cdn/x")); // not expired
        assert_eq!(a.fresh_remote_url(1000.0), None); // expiry is exclusive (>)
        assert_eq!(a.fresh_remote_url(1001.0), None);
        // No cached url -> None regardless
        a.cached_remote_url = None;
        assert_eq!(a.fresh_remote_url(0.0), None);
    }

    #[test]
    fn to_manifest_entry_project_when_inside_base() {
        let a = MediaAsset::new("a", "/proj/media/x.mp4", ClipType::Video, "X", 2.0);
        let e = a.to_manifest_entry(Some(Path::new("/proj")), 0.0);
        assert_eq!(
            e.source,
            MediaSource::Project {
                relative_path: "media/x.mp4".into()
            }
        );
        assert_eq!(e.has_audio, Some(true));
    }

    #[test]
    fn to_manifest_entry_external_when_outside_base() {
        let a = MediaAsset::new("a", "/elsewhere/x.mp4", ClipType::Video, "X", 2.0);
        let e = a.to_manifest_entry(Some(Path::new("/proj")), 0.0);
        assert_eq!(
            e.source,
            MediaSource::External {
                absolute_path: "/elsewhere/x.mp4".into()
            }
        );
    }

    #[test]
    fn to_manifest_entry_drops_expired_cached_url() {
        let mut a = MediaAsset::new("a", "/elsewhere/x.mp4", ClipType::Video, "X", 2.0);
        a.cached_remote_url = Some("https://cdn/x".into());
        a.cached_remote_url_expires_at = Some(500.0);
        // now=600 -> expired -> both dropped
        let e = a.to_manifest_entry(None, 600.0);
        assert_eq!(e.cached_remote_url, None);
        assert_eq!(e.cached_remote_url_expires_at, None);
        // now=400 -> fresh -> kept
        let e2 = a.to_manifest_entry(None, 400.0);
        assert_eq!(e2.cached_remote_url.as_deref(), Some("https://cdn/x"));
        assert_eq!(e2.cached_remote_url_expires_at, Some(500.0));
    }

    #[test]
    fn from_entry_reconstructs_asset() {
        let e = MediaManifestEntry {
            id: "id1".into(),
            name: "Clip".into(),
            kind: ClipType::Video,
            source: MediaSource::Project {
                relative_path: "media/a.mov".into(),
            },
            duration: 3.0,
            generation_input: None,
            source_width: Some(1280),
            source_height: Some(720),
            source_fps: Some(24.0),
            has_audio: Some(true),
            folder_id: Some("f1".into()),
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        };
        let a = MediaAsset::from_entry(&e, "/proj/media/a.mov");
        assert_eq!(a.id, "id1");
        assert_eq!(a.url, PathBuf::from("/proj/media/a.mov"));
        assert_eq!(a.source_fps, Some(24.0));
        assert!(a.has_audio);
        assert_eq!(a.folder_id.as_deref(), Some("f1"));
    }

    #[test]
    fn manifest_full_roundtrip() {
        let mut m = sample_manifest();
        m.folders.push(MediaFolder::new("f1", "Folder"));
        let json = serde_json::to_string(&m).unwrap();
        let back: MediaManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(m, back);
    }
}
