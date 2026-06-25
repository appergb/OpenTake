//! Thumbnails: a video thumbnail sequence (with sprite-grid disk cache) and a
//! single still per image. Port of `Timeline/MediaVisualCache.swift`.
//!
//! The time-point formula ([`video_thumbnail_times`]) is pure and unit-tested;
//! frame decode uses the system ffmpeg CLI; image thumbnails use the `image`
//! crate (with EXIF orientation handled by the decoder).

pub mod sprite;

pub use sprite::{load_sprite, save_sprite, ThumbnailCacheMeta, VideoThumb};

use std::path::Path;

use crate::cache_key::{file_identity_key, KEY_HEX_LEN};
use crate::decode::frame::{decode_frames_at, fit_within, FrameRequest};
use crate::error::{MediaError, Result};
use crate::frame::RgbaFrame;

/// Thumbnail max box (upstream `maximumSize = 120×68`).
pub const THUMB_MAX_SIZE: (u32, u32) = (120, 68);
/// Seek tolerance for thumbnail decoding (upstream 1.0 s).
pub const THUMB_TOLERANCE_SECS: f64 = 1.0;
/// Default max pixel for an image thumbnail's long edge (upstream 120).
pub const IMAGE_THUMB_MAX_PIXEL: u32 = 120;
/// Progressive publish stride (upstream publishes every 50 frames).
pub const PARTIAL_STRIDE: usize = 50;

/// Callback invoked with the partially-decoded thumbnail list for progressive UI
/// updates (upstream's every-50-frames publish).
pub type PartialThumbCallback<'a> = &'a dyn Fn(&[VideoThumb]);

/// Thumbnail sample times: `interval = duration < 10 ? 1.0 : 2.0`, then
/// `stride(from: 0, to: duration, by: interval)`. Verbatim port of
/// `videoThumbnailTimes` (`MediaVisualCache.swift:192-202`). Empty when
/// `duration <= 0`.
pub fn video_thumbnail_times(duration: f64) -> Vec<f64> {
    if !duration.is_finite() || duration <= 0.0 {
        return Vec::new();
    }
    let interval = if duration < 10.0 { 1.0 } else { 2.0 };
    let mut times = Vec::new();
    let mut t = 0.0;
    while t < duration {
        times.push(t);
        t += interval;
    }
    times
}

/// Generate a video thumbnail sequence. Returns the disk-cached sequence on a
/// hit; otherwise decodes, saves the sprite cache, and returns. `on_partial` is
/// invoked every [`PARTIAL_STRIDE`] frames for progressive UI updates.
pub fn video_thumbnails(
    cache_root: &Path,
    path: &Path,
    duration_secs: f64,
    on_partial: Option<PartialThumbCallback<'_>>,
) -> Result<Vec<VideoThumb>> {
    let key = file_identity_key(path, KEY_HEX_LEN);
    if let Some(ref key) = key {
        if let Some(cached) = sprite::load_sprite(cache_root, key) {
            return Ok(cached);
        }
    }

    let times = video_thumbnail_times(duration_secs);
    if times.is_empty() {
        return Ok(Vec::new());
    }
    let req = FrameRequest {
        time_secs: 0.0,
        max_size: THUMB_MAX_SIZE,
        tolerance_secs: THUMB_TOLERANCE_SECS,
        apply_rotation: true,
    };

    let mut thumbs = Vec::new();
    for (n, result) in decode_frames_at(path, &times, &req).into_iter().enumerate() {
        let (actual, frame) = result?;
        thumbs.push(VideoThumb {
            time_secs: actual,
            image: frame,
        });
        if (n + 1) % PARTIAL_STRIDE == 0 {
            if let Some(cb) = on_partial {
                cb(&thumbs);
            }
        }
    }

    if !thumbs.is_empty() {
        if let Some(ref key) = key {
            let _ = sprite::save_sprite(cache_root, key, &thumbs);
        }
    }
    Ok(thumbs)
}

/// Decode a single image to an RGBA thumbnail scaled so its long edge is
/// `<= max_pixel` (never enlarged). EXIF orientation is applied by the decoder.
/// Port of `makeImageThumbnail` (`:152-163`).
pub fn image_thumbnail(path: &Path, max_pixel: u32) -> Result<RgbaFrame> {
    let img = image::open(path).map_err(|e| MediaError::Decode(format!("image open: {e}")))?;
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    let (nw, nh) = fit_within(w, h, (max_pixel, max_pixel));
    let scaled = if (nw, nh) == (w, h) {
        rgba
    } else {
        image::imageops::resize(&rgba, nw, nh, image::imageops::FilterType::Triangle)
    };
    Ok(RgbaFrame::new(
        scaled.width(),
        scaled.height(),
        scaled.into_raw(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_short_clip_one_second_interval() {
        // duration 5 (<10) → interval 1: 0,1,2,3,4
        assert_eq!(video_thumbnail_times(5.0), vec![0.0, 1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn times_long_clip_two_second_interval() {
        // duration 10 (>=10) → interval 2: 0,2,4,6,8
        assert_eq!(video_thumbnail_times(10.0), vec![0.0, 2.0, 4.0, 6.0, 8.0]);
    }

    #[test]
    fn times_boundary_at_ten_uses_two() {
        // exactly 10 → interval 2.
        let t = video_thumbnail_times(10.0);
        assert_eq!(t[1], 2.0);
    }

    #[test]
    fn times_just_below_ten_uses_one() {
        let t = video_thumbnail_times(9.5);
        assert_eq!(t[1], 1.0);
        assert_eq!(*t.last().unwrap(), 9.0);
    }

    #[test]
    fn times_zero_or_invalid_is_empty() {
        assert!(video_thumbnail_times(0.0).is_empty());
        assert!(video_thumbnail_times(-3.0).is_empty());
        assert!(video_thumbnail_times(f64::NAN).is_empty());
    }

    #[test]
    fn times_stride_is_strictly_less_than_duration() {
        // duration exactly 4 with interval 1 → 0,1,2,3 (not 4).
        let t = video_thumbnail_times(4.0);
        assert_eq!(t, vec![0.0, 1.0, 2.0, 3.0]);
    }

    #[test]
    fn image_thumbnail_scales_down_long_edge() {
        // Build a 400x100 PNG in a temp file and thumbnail it.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("img.png");
        let img = image::RgbaImage::from_pixel(400, 100, image::Rgba([10, 20, 30, 255]));
        img.save(&path).unwrap();

        let thumb = image_thumbnail(&path, 120).unwrap();
        // long edge (400) → 120, short edge scales to 30.
        assert_eq!(thumb.width, 120);
        assert_eq!(thumb.height, 30);
    }

    #[test]
    fn image_thumbnail_does_not_enlarge_small_image() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("small.png");
        image::RgbaImage::from_pixel(40, 30, image::Rgba([0, 0, 0, 255]))
            .save(&path)
            .unwrap();
        let thumb = image_thumbnail(&path, 120).unwrap();
        assert_eq!(thumb.width, 40);
        assert_eq!(thumb.height, 30);
    }
}
