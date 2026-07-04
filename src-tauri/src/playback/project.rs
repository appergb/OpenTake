//! Timeline → render-side projections for the streaming playback engine (#53).
//!
//! This is the playback counterpart to the projection logic in [`crate::render`]
//! / [`crate::export`]: it turns the authoritative session (timeline + media
//! manifest + project dir) into the lookups the streaming resolver needs — a
//! per-asset path + intrinsic size, and a per-text-clip content + style + box.
//!
//! Kept as a self-contained copy (exactly like `export.rs` does) so the existing
//! preview/export paths are not disturbed by the playback work. A later refactor
//! can hoist the single shared projection into one `pub(crate)` helper once all
//! three paths are stable (tracked as a follow-up; see the export.rs header note).

use std::collections::HashMap;
use std::path::PathBuf;

use opentake_domain::{ClipType, MediaManifest, MediaSource, TextStyle, Timeline};
use opentake_render::SourceMetrics;

/// Resolvable info for one media asset, projected from the manifest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaInfo {
    /// Absolute, decode-ready path (project-relative entries already joined to
    /// the bundle dir).
    pub path: PathBuf,
}

/// A text clip projected from the timeline, keyed by clip id. The box's width /
/// height drive the rasterized texture size; position rides the layer affine, so
/// x/y are kept only for completeness (matching the preview/export projection).
#[derive(Clone, Debug, PartialEq)]
pub struct TextInfo {
    pub content: String,
    pub style: TextStyle,
    pub box_norm: (f64, f64, f64, f64),
}

/// [`SourceMetrics`] backed by the media manifest: only intrinsic size is known
/// here (orientation/alpha use the documented identity/false defaults; ffmpeg
/// auto-rotates on decode in this cut), mirroring the preview/export adapters.
pub struct ManifestMetrics {
    pub sizes: HashMap<String, (u32, u32)>,
}

impl SourceMetrics for ManifestMetrics {
    fn natural_size(&self, media_ref: &str) -> Option<(u32, u32)> {
        self.sizes.get(media_ref).copied()
    }
}

/// Project the timeline's text clips (content + style + box) into the per-clip
/// lookup the resolver rasterizes from. Keyed by clip id, matching
/// `TextureSource::Text { clip_id }`. Mirrors `render::composite_frame`'s and
/// `export::project_text`'s identical projection.
pub fn project_text(timeline: &Timeline) -> HashMap<String, TextInfo> {
    let mut text: HashMap<String, TextInfo> = HashMap::new();
    for track in &timeline.tracks {
        for clip in &track.clips {
            if clip.media_type != ClipType::Text {
                continue;
            }
            let (Some(content), Some(style)) = (&clip.text_content, &clip.text_style) else {
                continue;
            };
            let tl = clip.transform.top_left();
            text.insert(
                clip.id.clone(),
                TextInfo {
                    content: content.clone(),
                    style: style.clone(),
                    box_norm: (tl.x, tl.y, clip.transform.width, clip.transform.height),
                },
            );
        }
    }
    text
}

/// Project the media manifest into the render-side `(sizes, media)` lookups,
/// resolving project-relative paths against `project_dir`. A `Project` entry with
/// no bundle dir is skipped (its path is unresolvable), matching the preview /
/// export behavior. Mirrors `export::project_media`.
pub fn project_media(
    manifest: &MediaManifest,
    project_dir: &Option<PathBuf>,
) -> (HashMap<String, (u32, u32)>, HashMap<String, MediaInfo>) {
    let mut sizes: HashMap<String, (u32, u32)> = HashMap::new();
    let mut media: HashMap<String, MediaInfo> = HashMap::new();
    for entry in &manifest.entries {
        let path = match &entry.source {
            MediaSource::External { absolute_path } => PathBuf::from(absolute_path),
            MediaSource::Project { relative_path } => match project_dir {
                Some(base) => base.join(relative_path),
                None => continue,
            },
        };
        if let (Some(w), Some(h)) = (entry.source_width, entry.source_height) {
            if w > 0 && h > 0 {
                sizes.insert(entry.id.clone(), (w as u32, h as u32));
            }
        }
        media.insert(entry.id.clone(), MediaInfo { path });
    }
    (sizes, media)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_domain::{Clip, MediaManifestEntry, Timeline, Track};

    fn entry(id: &str, source: MediaSource, size: Option<(i32, i32)>) -> MediaManifestEntry {
        MediaManifestEntry {
            id: id.to_string(),
            name: format!("{id}.mp4"),
            kind: ClipType::Video,
            source,
            duration: 1.0,
            generation_input: None,
            source_width: size.map(|(w, _)| w),
            source_height: size.map(|(_, h)| h),
            source_fps: None,
            has_audio: None,
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        }
    }

    #[test]
    fn project_media_resolves_external_and_project_paths() {
        let mut manifest = MediaManifest::new();
        manifest.entries.push(entry(
            "ext",
            MediaSource::External {
                absolute_path: "/abs/a.mp4".into(),
            },
            Some((1920, 1080)),
        ));
        manifest.entries.push(entry(
            "proj",
            MediaSource::Project {
                relative_path: "media/b.mp4".into(),
            },
            Some((1280, 720)),
        ));

        let dir = Some(PathBuf::from("/bundle"));
        let (sizes, media) = project_media(&manifest, &dir);

        assert_eq!(media.get("ext").unwrap().path, PathBuf::from("/abs/a.mp4"));
        assert_eq!(
            media.get("proj").unwrap().path,
            PathBuf::from("/bundle/media/b.mp4")
        );
        assert_eq!(sizes.get("ext").copied(), Some((1920, 1080)));
        assert_eq!(sizes.get("proj").copied(), Some((1280, 720)));
    }

    #[test]
    fn project_media_skips_project_entry_without_bundle_dir() {
        let mut manifest = MediaManifest::new();
        manifest.entries.push(entry(
            "proj",
            MediaSource::Project {
                relative_path: "media/b.mp4".into(),
            },
            Some((1280, 720)),
        ));
        let (sizes, media) = project_media(&manifest, &None);
        assert!(
            media.is_empty(),
            "unresolvable project path must be skipped"
        );
        assert!(sizes.is_empty());
    }

    #[test]
    fn project_media_drops_nonpositive_sizes() {
        let mut manifest = MediaManifest::new();
        manifest.entries.push(entry(
            "zero",
            MediaSource::External {
                absolute_path: "/abs/z.mp4".into(),
            },
            Some((0, 1080)),
        ));
        manifest.entries.push(entry(
            "none",
            MediaSource::External {
                absolute_path: "/abs/n.mp4".into(),
            },
            None,
        ));
        let (sizes, media) = project_media(&manifest, &None);
        // Paths still resolve; sizes are just absent for degenerate/unknown dims.
        assert_eq!(media.len(), 2);
        assert!(sizes.is_empty());
    }

    #[test]
    fn project_text_collects_text_clips_only() {
        let mut tl = Timeline::new();
        let mut track = Track::new("t1", ClipType::Text);

        let mut text_clip = Clip::new("text-1", "asset-x", 0, 30);
        text_clip.media_type = ClipType::Text;
        text_clip.text_content = Some("hello".into());
        text_clip.text_style = Some(TextStyle::default());
        track.clips.push(text_clip);

        // A text-typed clip missing content is skipped (no panic, no entry).
        let mut empty = Clip::new("text-2", "asset-y", 30, 30);
        empty.media_type = ClipType::Text;
        track.clips.push(empty);

        tl.tracks.push(track);

        let text = project_text(&tl);
        assert_eq!(text.len(), 1);
        assert_eq!(text.get("text-1").unwrap().content, "hello");
        assert!(!text.contains_key("text-2"));
    }

    #[test]
    fn project_text_ignores_non_text_clips() {
        let mut tl = Timeline::new();
        let mut track = Track::new("v1", ClipType::Video);
        let mut clip = Clip::new("v-1", "asset-v", 0, 30);
        clip.media_type = ClipType::Video;
        clip.text_content = Some("ignored".into());
        track.clips.push(clip);
        tl.tracks.push(track);
        assert!(project_text(&tl).is_empty());
    }
}
