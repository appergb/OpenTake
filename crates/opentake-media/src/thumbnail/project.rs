//! Project cover thumbnail — the representative-frame capture written into a
//! bundle's `thumbnail.jpg` on save.
//!
//! Upstream `VideoProject.captureThumbnail` walks
//! `timeline.tracks where track.type == .video`, then each clip in order, and
//! returns the first frame it can grab:
//! - an **image** clip → `ImageEncoder.thumbnail(url, maxPixelSize: 640)` →
//!   `encodeJPEG(quality: 0.7)`;
//! - a **video** clip → `AVAssetImageGenerator` (`maximumSize = 320×180`,
//!   `appliesPreferredTrackTransform`) seeked to
//!   `CMTime(value: clip.trimStartFrame, timescale: fps)` → JPEG `quality 0.7`.
//!
//! OpenTake retains that source-only path for compatibility, but Home covers use
//! [`capture_project_composite_thumbnail`]: the desktop renderer supplies the
//! same composited RGBA frame used by preview/export, and this module applies
//! the deterministic 16:9 cover policy plus JPEG encoding. The media layer does
//! not own a second renderer.

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

/// Default bounded 16:9 surface for an authoritative project composite.
pub const PROJECT_COMPOSITE_COVER_BOUNDS: (u32, u32) = (640, 360);

/// Seek tolerance (seconds) for the video cover grab. Upstream's
/// `AVAssetImageGenerator` uses its default tolerances (not zero); a modest
/// window keeps the grab cheap and reliably lands a decodable frame near the
/// clip's in-point.
pub const VIDEO_COVER_TOLERANCE_SECS: f64 = 1.0;

/// JPEG quality for the cover. Upstream encodes at `compressionFactor: 0.7`;
/// `image`'s `JpegEncoder` takes a 1–100 quality, so 72 ≈ 0.7. Named (not
/// hardcoded) per the media-layer "no magic thresholds" rule.
pub const PROJECT_THUMB_JPEG_QUALITY: u8 = 72;

/// Borrowed state needed to validate/select a representative composite without
/// coupling the media crate to the desktop renderer.
#[derive(Clone, Copy, Debug)]
pub struct ProjectCompositeThumbnailSnapshot<'a> {
    timeline: &'a Timeline,
    project_base: Option<&'a Path>,
}

impl<'a> ProjectCompositeThumbnailSnapshot<'a> {
    pub fn new(timeline: &'a Timeline, project_base: Option<&'a Path>) -> Self {
        Self {
            timeline,
            project_base,
        }
    }

    pub fn representative_frame(self, manifest: &MediaManifest) -> Option<i32> {
        representative_project_thumbnail_frame(self.timeline, manifest, self.project_base)
    }
}

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

/// Choose a deterministic frame inside the first visible, resolvable visual
/// clip. A valid outgoing cross-dissolve prefers its midpoint so a cover can
/// truthfully represent both sides of an authored transition; otherwise the
/// clip midpoint avoids unstable half-open boundaries.
pub fn representative_project_thumbnail_frame(
    timeline: &Timeline,
    manifest: &MediaManifest,
    project_base: Option<&Path>,
) -> Option<i32> {
    let resolver = MediaResolver::new(manifest, project_base);
    for track in timeline.tracks.iter().filter(|track| !track.hidden) {
        for (index, clip) in track.clips.iter().enumerate() {
            if !clip_is_compositable(clip, timeline, &resolver) || clip.duration_frames <= 0 {
                continue;
            }
            if let (Some(transition), Some(incoming)) =
                (clip.transition_out.as_ref(), track.clips.get(index + 1))
            {
                let transition_matches = transition.kind
                    == opentake_domain::TransitionKind::CrossDissolve
                    && (transition.from_clip_id.is_empty() || transition.from_clip_id == clip.id)
                    && transition.to_clip_id == incoming.id
                    && incoming.start_frame == clip.end_frame()
                    && incoming.duration_frames > 0
                    && clip_is_compositable(incoming, timeline, &resolver);
                if transition_matches {
                    let duration = transition
                        .duration_frames
                        .max(1)
                        .min(clip.duration_frames)
                        .min(incoming.duration_frames);
                    let transition_start = clip.end_frame() - duration;
                    return Some((transition_start + duration / 2).min(clip.end_frame() - 1));
                }
            }
            return Some(clip.start_frame + (clip.duration_frames - 1) / 2);
        }
    }
    None
}

fn clip_is_compositable(
    clip: &opentake_domain::Clip,
    timeline: &Timeline,
    resolver: &MediaResolver<'_>,
) -> bool {
    if !clip.media_type.is_visual() {
        return false;
    }
    if let Some(sequence_id) = clip.nested_sequence_id.as_deref() {
        return timeline
            .nested_sequences
            .iter()
            .any(|sequence| sequence.id == sequence_id && sequence.timeline.total_frames() > 0);
    }
    match clip.media_type {
        ClipType::Text => {
            clip.text_content
                .as_deref()
                .is_some_and(|content| !content.trim().is_empty())
                && clip.text_style.is_some()
        }
        ClipType::Video | ClipType::Image | ClipType::Lottie => resolver
            .expected_path(&clip.media_ref)
            .is_some_and(|path| path.is_file()),
        ClipType::Audio => false,
    }
}

/// JPEG-encode an already-authoritative project composite. `frame` must be the
/// RGBA result returned by the shared preview/export compositor for
/// `snapshot.representative_frame(manifest)`. This function deliberately owns
/// only cover geometry and encoding.
///
/// The output is the largest exact 16:9 raster that fits inside `bounds`.
/// The project canvas is resized to fit and centered over an opaque black
/// background. This keeps the bounded Home surface predictable without
/// cropping authored canvas content or relying on JPEG alpha handling.
pub fn capture_project_composite_thumbnail(
    snapshot: ProjectCompositeThumbnailSnapshot<'_>,
    manifest: &MediaManifest,
    frame: &crate::frame::RgbaFrame,
    bounds: (u32, u32),
) -> Option<Vec<u8>> {
    snapshot.representative_frame(manifest)?;
    encode_project_composite_thumbnail(frame, bounds)
}

/// Apply only the bounded cover-surface policy and JPEG encoding to a frame
/// already selected and produced by the authoritative render plan. This form is
/// used by strict retained-source capture so media paths are not reopened just
/// to repeat representative-frame selection.
pub fn encode_project_composite_thumbnail(
    frame: &crate::frame::RgbaFrame,
    bounds: (u32, u32),
) -> Option<Vec<u8>> {
    let (width, height) = bounded_sixteen_by_nine(bounds)?;
    let rgba = image::RgbaImage::from_raw(frame.width, frame.height, frame.rgba.clone())?;
    let contained = image::DynamicImage::ImageRgba8(rgba)
        .resize(width, height, image::imageops::FilterType::Lanczos3)
        .to_rgb8();
    let mut cover = image::RgbImage::from_pixel(width, height, image::Rgb([0, 0, 0]));
    let x = i64::from(width.saturating_sub(contained.width()) / 2);
    let y = i64::from(height.saturating_sub(contained.height()) / 2);
    image::imageops::overlay(&mut cover, &contained, x, y);
    encode_dynamic_jpeg(&image::DynamicImage::ImageRgb8(cover)).ok()
}

fn bounded_sixteen_by_nine(bounds: (u32, u32)) -> Option<(u32, u32)> {
    let scale = (bounds.0 / 16).min(bounds.1 / 9);
    (scale > 0).then_some((scale * 16, scale * 9))
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
    encode_dynamic_jpeg(&image::DynamicImage::ImageRgba8(rgba))
}

fn encode_dynamic_jpeg(image: &image::DynamicImage) -> Result<Vec<u8>> {
    let rgb = image.to_rgb8();
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
        Clip, ClipType, MediaManifest, MediaManifestEntry, MediaSource, Point, TextStyle, Timeline,
        Track, Transform, Transition, TransitionKind,
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
            color: None,
            proxy: None,
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

    fn touch_color_png(dir: &Path, name: &str, color: [u8; 4]) -> PathBuf {
        let p = dir.join(name);
        image::RgbaImage::from_pixel(320, 180, image::Rgba(color))
            .save(&p)
            .unwrap();
        p
    }

    fn composite_fixture(dir: &Path) -> (Timeline, MediaManifest, crate::frame::RgbaFrame) {
        let background = touch_color_png(dir, "background.png", [210, 20, 20, 255]);
        let incoming = touch_color_png(dir, "incoming.png", [20, 180, 20, 255]);
        let overlay = touch_color_png(dir, "overlay.png", [20, 40, 220, 255]);
        let mut manifest = MediaManifest::new();
        manifest
            .entries
            .push(entry("background", ClipType::Video, &background));
        manifest
            .entries
            .push(entry("incoming", ClipType::Video, &incoming));
        manifest
            .entries
            .push(entry("overlay", ClipType::Image, &overlay));

        let mut background_clip = clip("background-clip", "background", ClipType::Video, 0);
        background_clip.duration_frames = 30;
        background_clip.transition_out = Some(Transition {
            from_clip_id: "background-clip".into(),
            to_clip_id: "incoming-clip".into(),
            kind: TransitionKind::CrossDissolve,
            duration_frames: 10,
        });
        let mut incoming_clip = clip("incoming-clip", "incoming", ClipType::Video, 0);
        incoming_clip.start_frame = 30;
        incoming_clip.duration_frames = 30;
        let mut background_track = Track::new("background-track", ClipType::Video);
        background_track.clips = vec![background_clip, incoming_clip];

        let mut overlay_clip = clip("overlay-clip", "overlay", ClipType::Image, 0);
        overlay_clip.start_frame = 20;
        overlay_clip.duration_frames = 20;
        overlay_clip.transform = Transform::from_center(Point { x: 0.78, y: 0.5 }, 0.3, 0.6);
        overlay_clip.transform.rotation = 8.0;
        let mut overlay_track = Track::new("overlay-track", ClipType::Video);
        overlay_track.clips.push(overlay_clip);

        let mut text_clip = clip("text-clip", "", ClipType::Text, 0);
        text_clip.start_frame = 20;
        text_clip.duration_frames = 20;
        text_clip.text_content = Some("Composite".into());
        text_clip.text_style = Some(TextStyle::default());
        text_clip.transform = Transform::from_center(Point { x: 0.28, y: 0.5 }, 0.4, 0.2);
        let mut text_track = Track::new("text-track", ClipType::Text);
        text_track.clips.push(text_clip);

        let mut timeline = Timeline::new();
        timeline.width = 320;
        timeline.height = 180;
        timeline.tracks = vec![background_track, overlay_track, text_track];

        // A hand-derived stand-in for the authoritative renderer output at the
        // cross-dissolve midpoint: red background, green transition evidence,
        // transformed blue overlay, and white text evidence.
        let mut pixels = image::RgbaImage::from_pixel(320, 180, image::Rgba([210, 20, 20, 255]));
        for y in 150..180 {
            for x in 0..320 {
                pixels.put_pixel(x, y, image::Rgba([20, 180, 20, 255]));
            }
        }
        for y in 40..140 {
            for x in 210..300 {
                pixels.put_pixel(x, y, image::Rgba([20, 40, 220, 255]));
            }
        }
        for y in 76..96 {
            for x in 25..145 {
                pixels.put_pixel(x, y, image::Rgba([245, 245, 245, 255]));
            }
        }
        (
            timeline,
            manifest,
            crate::frame::RgbaFrame {
                width: 320,
                height: 180,
                rgba: pixels.into_raw(),
            },
        )
    }

    fn desired_composite_capture(
        timeline: &Timeline,
        manifest: &MediaManifest,
        frame: &crate::frame::RgbaFrame,
        bounds: (u32, u32),
    ) -> Option<Vec<u8>> {
        capture_project_composite_thumbnail(
            ProjectCompositeThumbnailSnapshot::new(timeline, None),
            manifest,
            frame,
            bounds,
        )
    }

    fn desired_representative_frame(
        timeline: &Timeline,
        manifest: &MediaManifest,
        project_base: Option<&Path>,
    ) -> Option<i32> {
        representative_project_thumbnail_frame(timeline, manifest, project_base)
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

    #[test]
    fn composite_thumbnail_contains_background_transition_overlay_transform_and_text_evidence() {
        let dir = TmpDir::new("composite-layers");
        let (timeline, manifest, composite) = composite_fixture(dir.path());

        let bytes =
            desired_composite_capture(&timeline, &manifest, &composite, VIDEO_COVER_MAX_SIZE)
                .expect("composite JPEG");
        let decoded = image::load_from_memory(&bytes)
            .expect("decode cover")
            .to_rgb8();

        let background = decoded.get_pixel(180, 20).0;
        let transition = decoded.get_pixel(180, 168).0;
        let overlay = decoded.get_pixel(250, 90).0;
        let text = decoded.get_pixel(80, 85).0;
        assert!(background[0] > 150 && background[1] < 80, "{background:?}");
        assert!(transition[1] > 110 && transition[0] < 100, "{transition:?}");
        assert!(overlay[2] > 140 && overlay[0] < 100, "{overlay:?}");
        assert!(text.iter().all(|channel| *channel > 190), "{text:?}");
    }

    #[test]
    fn composite_thumbnail_uses_transition_midpoint_as_stable_representative_frame() {
        let dir = TmpDir::new("composite-representative");
        let (timeline, manifest, _) = composite_fixture(dir.path());

        assert_eq!(
            desired_representative_frame(&timeline, &manifest, None),
            Some(25)
        );
    }

    #[test]
    fn composite_thumbnail_is_deterministically_bounded_to_sixteen_by_nine() {
        let dir = TmpDir::new("composite-bounds");
        let (timeline, manifest, composite) = composite_fixture(dir.path());

        let first = desired_composite_capture(&timeline, &manifest, &composite, (200, 200))
            .expect("first JPEG");
        let second = desired_composite_capture(&timeline, &manifest, &composite, (200, 200))
            .expect("second JPEG");
        let decoded = image::load_from_memory(&first).expect("decode bounded cover");

        assert_eq!((decoded.width(), decoded.height()), (192, 108));
        assert_eq!(first, second);
    }

    #[test]
    fn composite_thumbnail_letterboxes_portrait_canvas_without_cropping() {
        let dir = TmpDir::new("composite-portrait-letterbox");
        let source = touch_color_png(dir.path(), "portrait.png", [220, 30, 20, 255]);
        let mut manifest = MediaManifest::new();
        manifest
            .entries
            .push(entry("portrait", ClipType::Image, &source));
        let mut timeline = Timeline::new();
        timeline.width = 180;
        timeline.height = 320;
        let mut track = Track::new("video", ClipType::Video);
        track
            .clips
            .push(clip("portrait-clip", "portrait", ClipType::Image, 0));
        timeline.tracks.push(track);
        let portrait = crate::frame::RgbaFrame::new(180, 320, [220, 30, 20, 255].repeat(180 * 320));

        let bytes = desired_composite_capture(
            &timeline,
            &manifest,
            &portrait,
            PROJECT_COMPOSITE_COVER_BOUNDS,
        )
        .expect("portrait cover");
        let decoded = image::load_from_memory(&bytes).unwrap().to_rgb8();

        assert_eq!((decoded.width(), decoded.height()), (640, 360));
        assert!(decoded
            .get_pixel(0, 180)
            .0
            .iter()
            .all(|channel| *channel < 12));
        let center = decoded.get_pixel(320, 180).0;
        assert!(center[0] > 180 && center[1] < 70, "{center:?}");
        assert!(decoded
            .get_pixel(639, 180)
            .0
            .iter()
            .all(|channel| *channel < 12));
    }

    #[test]
    fn composite_thumbnail_letterboxes_four_by_three_canvas_at_exact_boundaries() {
        let dir = TmpDir::new("composite-four-three-letterbox");
        let source = touch_color_png(dir.path(), "four-three.png", [20, 80, 220, 255]);
        let mut manifest = MediaManifest::new();
        manifest
            .entries
            .push(entry("four-three", ClipType::Image, &source));
        let mut timeline = Timeline::new();
        timeline.width = 400;
        timeline.height = 300;
        let mut track = Track::new("video", ClipType::Video);
        track
            .clips
            .push(clip("four-three-clip", "four-three", ClipType::Image, 0));
        timeline.tracks.push(track);
        let frame = crate::frame::RgbaFrame::new(400, 300, [20, 80, 220, 255].repeat(400 * 300));

        let bytes =
            desired_composite_capture(&timeline, &manifest, &frame, PROJECT_COMPOSITE_COVER_BOUNDS)
                .expect("four-by-three cover");
        let decoded = image::load_from_memory(&bytes).unwrap().to_rgb8();

        assert!(decoded
            .get_pixel(79, 180)
            .0
            .iter()
            .all(|channel| *channel < 15));
        let first_content = decoded.get_pixel(82, 180).0;
        assert!(first_content[2] > 160, "{first_content:?}");
        let last_content = decoded.get_pixel(557, 180).0;
        assert!(last_content[2] > 160, "{last_content:?}");
        assert!(decoded
            .get_pixel(560, 180)
            .0
            .iter()
            .all(|channel| *channel < 15));
    }

    #[test]
    fn composite_thumbnail_returns_none_for_empty_project() {
        assert!(desired_composite_capture(
            &Timeline::new(),
            &MediaManifest::new(),
            &crate::frame::RgbaFrame {
                width: 2,
                height: 2,
                rgba: vec![0; 16],
            },
            VIDEO_COVER_MAX_SIZE,
        )
        .is_none());
    }

    #[test]
    fn composite_thumbnail_returns_none_when_every_visual_source_is_offline() {
        let dir = TmpDir::new("composite-offline");
        let missing = dir.path().join("missing.png");
        let mut manifest = MediaManifest::new();
        manifest
            .entries
            .push(entry("missing", ClipType::Image, &missing));
        let mut timeline = Timeline::new();
        let mut track = Track::new("video", ClipType::Video);
        track
            .clips
            .push(clip("missing-clip", "missing", ClipType::Image, 0));
        timeline.tracks.push(track);

        assert_eq!(
            desired_representative_frame(&timeline, &manifest, None),
            None
        );
    }
}
