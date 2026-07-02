//! Project cover thumbnail — the representative-frame capture written into a
//! bundle's `thumbnail.jpg` on save. 1:1 port of upstream
//! `VideoProject.captureThumbnail` (`Project/VideoProject.swift:261-300`).
//!
//! Upstream walks `timeline.tracks where track.type == .video`, then each clip
//! in order, and returns the first frame it can grab:
//! - an **image** clip → `ImageEncoder.thumbnail(url, maxPixelSize: 640)` →
//!   `encodeJPEG(quality: 0.7)`;
//! - a **video** clip → `AVAssetImageGenerator` (`maximumSize = 320×180`,
//!   `appliesPreferredTrackTransform`) seeked to
//!   `CMTime(value: clip.trimStartFrame, timescale: fps)` → JPEG `quality 0.7`.
//!
//! The **pick** ([`pick_thumbnail_source`]) is a pure function over the timeline
//! and manifest (resolvable-file filter lives here because the media layer,
//! unlike `opentake-domain`, may touch the filesystem), so the track/clip
//! selection rule is unit-testable without ffmpeg. The **capture**
//! ([`capture_project_thumbnail`]) decodes and JPEG-encodes and therefore needs
//! ffmpeg / a real image file.

use std::path::{Path, PathBuf};

use opentake_domain::{ClipType, MediaManifest, MediaResolver, Timeline};

use crate::decode::frame::{decode_frame_at, FrameRequest};
use crate::error::Result;
use crate::thumbnail::image_thumbnail;

/// Long-edge cap for an **image** clip's cover, matching upstream
/// `ImageEncoder.thumbnail(url:, maxPixelSize: 640)`.
pub const IMAGE_COVER_MAX_PIXEL: u32 = 640;

/// Box a **video** clip's cover is fit within, matching upstream
/// `generator.maximumSize = CGSize(width: 320, height: 180)`.
pub const VIDEO_COVER_MAX_SIZE: (u32, u32) = (320, 180);

/// Seek tolerance (seconds) for the video cover grab. Upstream's
/// `AVAssetImageGenerator` uses its default tolerances (not zero); a modest
/// window keeps the grab cheap and reliably lands a decodable frame near the
/// clip's in-point.
pub const VIDEO_COVER_TOLERANCE_SECS: f64 = 1.0;

/// JPEG quality for the cover. Upstream encodes at `compressionFactor: 0.7`;
/// `image`'s `JpegEncoder` takes a 1–100 quality, so 72 ≈ 0.7. Named (not
/// hardcoded) per the media-layer "no magic thresholds" rule.
pub const PROJECT_THUMB_JPEG_QUALITY: u8 = 72;

/// Which decode path a picked clip needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThumbnailKind {
    /// Still image: decode the whole file, fit to [`IMAGE_COVER_MAX_PIXEL`].
    Image,
    /// Video: seek to the in-point and grab one frame in [`VIDEO_COVER_MAX_SIZE`].
    Video,
}

/// The representative clip chosen for the cover: the on-disk source, whether it
/// is an image or video, and (for video) the source frame to seek to (the clip's
/// `trim_start_frame`, i.e. its in-point — exactly upstream's
/// `CMTime(value: clip.trimStartFrame, …)`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThumbnailSource {
    /// Resolved, existing path to the source media file.
    pub path: PathBuf,
    /// Image vs video decode path.
    pub kind: ThumbnailKind,
    /// Source frame to seek to for video (0 for images / no trim). Absolute
    /// source-frame offset like upstream's `clip.trimStartFrame`.
    pub seek_frame: i32,
}

/// Pick the representative clip for the project cover, or `None` when no video
/// track carries a resolvable image/video clip. Pure over `(timeline, manifest,
/// project_base)`; only reads the filesystem to confirm the picked file exists
/// (upstream `resolveURL` yields nothing for an unresolved ref, so an unresolved
/// clip is skipped just the same).
///
/// Order mirrors upstream exactly: iterate tracks whose kind is `Video`
/// (**not** every visual track — audio/text/lottie tracks are skipped), then the
/// clips in stored order; the first clip that is an image or a video **and**
/// whose media resolves to an existing file wins.
pub fn pick_thumbnail_source(
    timeline: &Timeline,
    manifest: &MediaManifest,
    project_base: Option<&Path>,
) -> Option<ThumbnailSource> {
    let resolver = MediaResolver::new(manifest, project_base);
    for track in timeline.tracks.iter().filter(|t| t.kind == ClipType::Video) {
        for clip in &track.clips {
            let kind = match clip.media_type {
                ClipType::Image => ThumbnailKind::Image,
                ClipType::Video => ThumbnailKind::Video,
                // Non-visual / text / lottie clips on a video track are not
                // frame-grabbable cover sources (upstream only handles .image
                // and .video), so skip them.
                _ => continue,
            };
            let Some(path) = resolver.expected_path(&clip.media_ref) else {
                continue; // unresolved ref (no manifest entry / no project base)
            };
            if !path.is_file() {
                continue; // offline media — upstream's generator would fail too
            }
            return Some(ThumbnailSource {
                path,
                kind,
                // Images ignore the seek; keep the clip's own value for video.
                seek_frame: clip.trim_start_frame.max(0),
            });
        }
    }
    None
}

/// Capture the project cover as JPEG bytes, or `None` when there is no
/// representative clip (empty project / all-offline media) or the single grab
/// fails. Mirrors upstream `captureThumbnail`: pick → decode one frame → encode
/// JPEG at [`PROJECT_THUMB_JPEG_QUALITY`]. `fps` is the timeline frame rate, used
/// to convert a video clip's `seek_frame` to a seek time.
///
/// A decode/encode failure returns `None` (not `Err`): upstream's capture is
/// best-effort and simply omits `thumbnail.jpg` on failure — the save itself must
/// never fail because a cover could not be produced.
pub fn capture_project_thumbnail(
    timeline: &Timeline,
    manifest: &MediaManifest,
    project_base: Option<&Path>,
) -> Option<Vec<u8>> {
    let source = pick_thumbnail_source(timeline, manifest, project_base)?;
    let fps = if timeline.fps > 0 { timeline.fps } else { 30 };
    encode_source(&source, fps).ok()
}

/// Decode the picked clip's cover frame and JPEG-encode it. Split out so the
/// (ffmpeg-dependent) capture is a single fallible step the caller degrades to
/// `None`.
fn encode_source(source: &ThumbnailSource, fps: i32) -> Result<Vec<u8>> {
    let frame = match source.kind {
        ThumbnailKind::Image => image_thumbnail(&source.path, IMAGE_COVER_MAX_PIXEL)?,
        ThumbnailKind::Video => {
            let time_secs = (source.seek_frame.max(0) as f64) / fps as f64;
            let req = FrameRequest {
                time_secs,
                max_size: VIDEO_COVER_MAX_SIZE,
                tolerance_secs: VIDEO_COVER_TOLERANCE_SECS,
                apply_rotation: true, // upstream appliesPreferredTrackTransform
            };
            let (_actual, frame) = decode_frame_at(&source.path, &req)?;
            frame
        }
    };
    encode_jpeg(&frame)
}

/// Encode an [`RgbaFrame`](crate::frame::RgbaFrame) as JPEG (alpha dropped → RGB)
/// at [`PROJECT_THUMB_JPEG_QUALITY`]. Mirrors the sprite cache's JPEG path.
fn encode_jpeg(frame: &crate::frame::RgbaFrame) -> Result<Vec<u8>> {
    let rgba = image::RgbaImage::from_raw(frame.width, frame.height, frame.rgba.clone())
        .ok_or_else(|| crate::error::MediaError::Encode("thumbnail: bad rgba buffer".into()))?;
    let rgb = image::DynamicImage::ImageRgba8(rgba).to_rgb8();
    let mut jpg_bytes = Vec::new();
    {
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
            &mut jpg_bytes,
            PROJECT_THUMB_JPEG_QUALITY,
        );
        encoder
            .encode(
                rgb.as_raw(),
                rgb.width(),
                rgb.height(),
                image::ExtendedColorType::Rgb8,
            )
            .map_err(|e| crate::error::MediaError::Encode(format!("thumbnail jpeg: {e}")))?;
    }
    Ok(jpg_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_domain::{
        Clip, ClipType, MediaManifest, MediaManifestEntry, MediaSource, Timeline, Track,
    };
    use std::fs;
    use std::path::PathBuf;

    /// A per-call-unique scratch dir under the system temp dir, removed on drop.
    struct TmpDir(PathBuf);
    impl TmpDir {
        fn new(tag: &str) -> Self {
            use std::sync::atomic::{AtomicU64, Ordering};
            static N: AtomicU64 = AtomicU64::new(0);
            let n = N.fetch_add(1, Ordering::Relaxed);
            let p = std::env::temp_dir().join(format!(
                "opentake-projthumb-{tag}-{}-{n}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            TmpDir(p)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TmpDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn entry(id: &str, kind: ClipType, abs_path: &Path) -> MediaManifestEntry {
        MediaManifestEntry {
            id: id.into(),
            name: id.into(),
            kind,
            source: MediaSource::External {
                absolute_path: abs_path.to_string_lossy().into_owned(),
            },
            duration: 1.0,
            generation_input: None,
            source_width: Some(4),
            source_height: Some(4),
            source_fps: None,
            has_audio: None,
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        }
    }

    fn clip(id: &str, media_ref: &str, media_type: ClipType, trim_start: i32) -> Clip {
        let mut c = Clip::new(id, media_ref, 0, 30);
        c.media_type = media_type;
        c.trim_start_frame = trim_start;
        c
    }

    /// Write a real PNG so `expected_path().is_file()` passes (the pick filters
    /// on existence, matching upstream `resolveURL` returning nothing offline).
    fn touch_png(dir: &Path, name: &str) -> PathBuf {
        let p = dir.join(name);
        image::RgbaImage::from_pixel(4, 4, image::Rgba([10, 20, 30, 255]))
            .save(&p)
            .unwrap();
        p
    }

    #[test]
    fn pick_returns_none_for_empty_timeline() {
        let tl = Timeline::new();
        let manifest = MediaManifest::new();
        assert_eq!(pick_thumbnail_source(&tl, &manifest, None), None);
    }

    #[test]
    fn pick_takes_first_resolvable_image_clip_on_video_track() {
        let dir = TmpDir::new("pick-image");
        let img = touch_png(dir.path(), "pic.png");
        let mut manifest = MediaManifest::new();
        manifest.entries.push(entry("a1", ClipType::Image, &img));

        let mut tl = Timeline::new();
        let mut vt = Track::new("vt", ClipType::Video);
        vt.clips.push(clip("c1", "a1", ClipType::Image, 0));
        tl.tracks.push(vt);

        let picked = pick_thumbnail_source(&tl, &manifest, None).expect("picked");
        assert_eq!(picked.kind, ThumbnailKind::Image);
        assert_eq!(picked.path, img);
        assert_eq!(picked.seek_frame, 0);
    }

    #[test]
    fn pick_carries_video_trim_start_as_seek_frame() {
        let dir = TmpDir::new("pick-video");
        // A video clip whose source "file" merely needs to exist for the pick
        // (the pick never decodes; it only checks `is_file()`). Plain bytes are
        // enough and avoid the image crate rejecting a `.mp4` extension.
        let vid = dir.path().join("shot.mp4");
        fs::write(&vid, b"not-real-video").unwrap();
        let mut manifest = MediaManifest::new();
        manifest.entries.push(entry("v1", ClipType::Video, &vid));

        let mut tl = Timeline::new();
        let mut vt = Track::new("vt", ClipType::Video);
        vt.clips.push(clip("c1", "v1", ClipType::Video, 45));
        tl.tracks.push(vt);

        let picked = pick_thumbnail_source(&tl, &manifest, None).expect("picked");
        assert_eq!(picked.kind, ThumbnailKind::Video);
        assert_eq!(picked.seek_frame, 45);
    }

    #[test]
    fn pick_skips_audio_tracks_and_offline_clips() {
        let dir = TmpDir::new("pick-skip");
        // Present on disk, but on an AUDIO track → must be skipped (upstream only
        // scans `.video` tracks).
        let on_audio = touch_png(dir.path(), "onaudio.png");
        // On a video track but its file does NOT exist → offline, skipped.
        let missing_path = dir.path().join("gone.png");
        // The one that should win: second clip on the video track, present.
        let good = touch_png(dir.path(), "good.png");

        let mut manifest = MediaManifest::new();
        manifest
            .entries
            .push(entry("aud", ClipType::Image, &on_audio));
        manifest
            .entries
            .push(entry("missing", ClipType::Image, &missing_path));
        manifest.entries.push(entry("good", ClipType::Image, &good));

        let mut tl = Timeline::new();
        let mut at = Track::new("at", ClipType::Audio);
        at.clips.push(clip("ca", "aud", ClipType::Image, 0)); // ignored: audio track
        let mut vt = Track::new("vt", ClipType::Video);
        vt.clips
            .push(clip("c-missing", "missing", ClipType::Image, 0)); // offline
        vt.clips.push(clip("c-good", "good", ClipType::Image, 0)); // winner
        tl.tracks.push(at);
        tl.tracks.push(vt);

        let picked = pick_thumbnail_source(&tl, &manifest, None).expect("picked");
        assert_eq!(picked.path, good);
    }

    #[test]
    fn pick_skips_text_clips_on_video_track() {
        let dir = TmpDir::new("pick-text");
        let img = touch_png(dir.path(), "real.png");
        let mut manifest = MediaManifest::new();
        // Text clips carry no manifest media entry; only the image does.
        manifest.entries.push(entry("img", ClipType::Image, &img));

        let mut tl = Timeline::new();
        let mut vt = Track::new("vt", ClipType::Video);
        vt.clips.push(clip("t1", "text-ref", ClipType::Text, 0)); // skipped
        vt.clips.push(clip("i1", "img", ClipType::Image, 0)); // winner
        tl.tracks.push(vt);

        let picked = pick_thumbnail_source(&tl, &manifest, None).expect("picked");
        assert_eq!(picked.kind, ThumbnailKind::Image);
        assert_eq!(picked.path, img);
    }

    #[test]
    fn capture_encodes_jpeg_for_image_clip() {
        let dir = TmpDir::new("cap-image");
        let img = touch_png(dir.path(), "cover.png");
        let mut manifest = MediaManifest::new();
        manifest.entries.push(entry("a1", ClipType::Image, &img));
        let mut tl = Timeline::new();
        let mut vt = Track::new("vt", ClipType::Video);
        vt.clips.push(clip("c1", "a1", ClipType::Image, 0));
        tl.tracks.push(vt);

        let bytes = capture_project_thumbnail(&tl, &manifest, None).expect("jpeg bytes");
        assert!(!bytes.is_empty());
        // JPEG SOI marker.
        assert_eq!(&bytes[..2], &[0xFF, 0xD8]);
        // Decodable back to an image.
        let decoded = image::load_from_memory(&bytes).expect("decode jpeg");
        assert!(decoded.width() > 0 && decoded.height() > 0);
    }

    #[test]
    fn capture_returns_none_without_representative_clip() {
        let tl = Timeline::new();
        let manifest = MediaManifest::new();
        assert!(capture_project_thumbnail(&tl, &manifest, None).is_none());
    }
}
