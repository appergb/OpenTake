//! JPEG sprite-grid thumbnail cache. Port of the sprite logic in
//! `Timeline/MediaVisualCache.swift` (`saveThumbnails`/`loadThumbnails`).
//!
//! Thumbnails persist as one JPEG sprite (`<key>.thumbs.jpg`) plus a JSON
//! sidecar (`<key>.thumbs.json`). The sidecar is written **last** and is the
//! marker of a complete entry. Layout: `columns = min(50, count)`, tile size =
//! the first frame's pixel size, rows packed top-to-bottom (origin top-left,
//! matching the `image` crate). Field names in the sidecar are camelCase
//! (`tileWidth`/`tileHeight`/`columns`/`times`) so the cache is interchangeable
//! with upstream.

use std::path::{Path, PathBuf};

use image::{ImageBuffer, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::frame::RgbaFrame;
use crate::waveform::store::CACHE_SUBDIR;

/// Max sprite columns (upstream `min(50, count)`).
pub const MAX_COLUMNS: u32 = 50;
/// JPEG quality used when encoding the sprite (upstream 0.75 → 75/100).
pub const JPEG_QUALITY: u8 = 75;

/// A video thumbnail at a source time.
#[derive(Clone)]
pub struct VideoThumb {
    pub time_secs: f64,
    pub image: RgbaFrame,
}

/// Sidecar metadata (camelCase to match upstream `ThumbnailCacheMeta`).
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ThumbnailCacheMeta {
    pub tile_width: u32,
    pub tile_height: u32,
    pub columns: u32,
    pub times: Vec<f64>,
}

fn jpg_path(cache_root: &Path, key: &str) -> PathBuf {
    cache_root
        .join(CACHE_SUBDIR)
        .join(format!("{key}.thumbs.jpg"))
}

fn json_path(cache_root: &Path, key: &str) -> PathBuf {
    cache_root
        .join(CACHE_SUBDIR)
        .join(format!("{key}.thumbs.json"))
}

/// Pure: sprite grid geometry for `count` tiles. Returns `(columns, rows)`.
pub fn grid_geometry(count: usize) -> (u32, u32) {
    if count == 0 {
        return (0, 0);
    }
    let columns = MAX_COLUMNS.min(count as u32).max(1);
    let rows = (count as u32).div_ceil(columns);
    (columns, rows)
}

/// Pure: the (col, row) tile position for sprite index `i` given `columns`.
pub fn tile_position(i: usize, columns: u32) -> (u32, u32) {
    let columns = columns.max(1);
    ((i as u32) % columns, (i as u32) / columns)
}

/// Compose `thumbs` into a single RGBA sprite image (top-left origin, row-major).
/// Returns `None` if there are no thumbs or the first tile is empty.
fn compose_sprite(thumbs: &[VideoThumb]) -> Option<(RgbaImage, ThumbnailCacheMeta)> {
    let first = thumbs.first()?;
    if first.image.width == 0 || first.image.height == 0 {
        return None;
    }
    let tile_w = first.image.width;
    let tile_h = first.image.height;
    let (columns, rows) = grid_geometry(thumbs.len());

    let mut sprite: RgbaImage =
        ImageBuffer::from_pixel(tile_w * columns, tile_h * rows, Rgba([0, 0, 0, 255]));
    for (i, thumb) in thumbs.iter().enumerate() {
        // Only place tiles matching the canonical tile size (defensive).
        if thumb.image.width != tile_w || thumb.image.height != tile_h {
            continue;
        }
        let (col, row) = tile_position(i, columns);
        let ox = col * tile_w;
        let oy = row * tile_h;
        for y in 0..tile_h {
            for x in 0..tile_w {
                let base = (y as usize * tile_w as usize + x as usize) * 4;
                let px = Rgba([
                    thumb.image.rgba[base],
                    thumb.image.rgba[base + 1],
                    thumb.image.rgba[base + 2],
                    thumb.image.rgba[base + 3],
                ]);
                sprite.put_pixel(ox + x, oy + y, px);
            }
        }
    }
    let meta = ThumbnailCacheMeta {
        tile_width: tile_w,
        tile_height: tile_h,
        columns,
        times: thumbs.iter().map(|t| t.time_secs).collect(),
    };
    Some((sprite, meta))
}

/// Save the sprite + sidecar under `<cache_root>/MediaVisualCache/`. The JSON
/// sidecar is written last (completeness marker).
pub fn save_sprite(cache_root: &Path, key: &str, thumbs: &[VideoThumb]) -> Result<()> {
    let Some((sprite, meta)) = compose_sprite(thumbs) else {
        return Ok(());
    };
    let dir = cache_root.join(CACHE_SUBDIR);
    std::fs::create_dir_all(&dir)?;

    // Encode JPEG (drop alpha → RGB) at the configured quality.
    let rgb = image::DynamicImage::ImageRgba8(sprite).to_rgb8();
    let mut jpg_bytes = Vec::new();
    {
        let mut encoder =
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpg_bytes, JPEG_QUALITY);
        encoder
            .encode(
                rgb.as_raw(),
                rgb.width(),
                rgb.height(),
                image::ExtendedColorType::Rgb8,
            )
            .map_err(|e| crate::error::MediaError::Encode(format!("jpeg: {e}")))?;
    }
    std::fs::write(jpg_path(cache_root, key), &jpg_bytes)?;

    // Sidecar last.
    let json =
        serde_json::to_vec(&meta).map_err(|e| crate::error::MediaError::Encode(e.to_string()))?;
    std::fs::write(json_path(cache_root, key), json)?;
    Ok(())
}

/// Load the sprite + sidecar, slicing tiles back out. Returns `None` if either
/// file is missing/unparsable or the sprite is too small for the declared grid
/// (upstream's size validation).
pub fn load_sprite(cache_root: &Path, key: &str) -> Option<Vec<VideoThumb>> {
    let meta_bytes = std::fs::read(json_path(cache_root, key)).ok()?;
    let meta: ThumbnailCacheMeta = serde_json::from_slice(&meta_bytes).ok()?;
    if meta.tile_width == 0 || meta.tile_height == 0 || meta.columns == 0 || meta.times.is_empty() {
        return None;
    }
    let sprite = image::open(jpg_path(cache_root, key)).ok()?.to_rgba8();

    let count = meta.times.len();
    let rows = (count as u32).div_ceil(meta.columns);
    let need_w = meta.tile_width * (meta.columns.min(count as u32));
    let need_h = meta.tile_height * rows;
    if sprite.width() < need_w || sprite.height() < need_h {
        return None;
    }

    let mut out = Vec::with_capacity(count);
    for (i, &t) in meta.times.iter().enumerate() {
        let (col, row) = tile_position(i, meta.columns);
        let ox = col * meta.tile_width;
        let oy = row * meta.tile_height;
        let mut rgba = Vec::with_capacity((meta.tile_width * meta.tile_height * 4) as usize);
        for y in 0..meta.tile_height {
            for x in 0..meta.tile_width {
                let px = sprite.get_pixel(ox + x, oy + y);
                rgba.extend_from_slice(&px.0);
            }
        }
        out.push(VideoThumb {
            time_secs: t,
            image: RgbaFrame::new(meta.tile_width, meta.tile_height, rgba),
        });
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thumb(t: f64, w: u32, h: u32, fill: u8) -> VideoThumb {
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for px in rgba.chunks_exact_mut(4) {
            px[0] = fill;
            px[1] = fill;
            px[2] = fill;
            px[3] = 255;
        }
        VideoThumb {
            time_secs: t,
            image: RgbaFrame::new(w, h, rgba),
        }
    }

    // --- pure geometry ---

    #[test]
    fn grid_geometry_caps_columns_at_50() {
        assert_eq!(grid_geometry(10), (10, 1));
        assert_eq!(grid_geometry(50), (50, 1));
        assert_eq!(grid_geometry(51), (50, 2));
        assert_eq!(grid_geometry(100), (50, 2));
        assert_eq!(grid_geometry(120), (50, 3));
        assert_eq!(grid_geometry(0), (0, 0));
    }

    #[test]
    fn tile_position_row_major() {
        assert_eq!(tile_position(0, 50), (0, 0));
        assert_eq!(tile_position(49, 50), (49, 0));
        assert_eq!(tile_position(50, 50), (0, 1));
        assert_eq!(tile_position(75, 50), (25, 1));
    }

    // --- round-trip through disk ---

    #[test]
    fn sprite_roundtrip_preserves_times_and_pixels() {
        let dir = tempfile::tempdir().unwrap();
        let thumbs = vec![
            thumb(0.0, 4, 3, 10),
            thumb(1.0, 4, 3, 120),
            thumb(2.0, 4, 3, 250),
        ];
        save_sprite(dir.path(), "k", &thumbs).unwrap();
        let back = load_sprite(dir.path(), "k").unwrap();
        assert_eq!(back.len(), 3);
        assert_eq!(back[0].time_secs, 0.0);
        assert_eq!(back[1].time_secs, 1.0);
        assert_eq!(back[2].time_secs, 2.0);
        // Tile dims preserved.
        assert_eq!(back[0].image.width, 4);
        assert_eq!(back[0].image.height, 3);
        // Pixel values approximately preserved (JPEG is lossy) — check the
        // bright tile is bright and the dark tile is dark.
        let dark_avg = avg(&back[0].image);
        let bright_avg = avg(&back[2].image);
        assert!(dark_avg < 60.0, "dark={dark_avg}");
        assert!(bright_avg > 200.0, "bright={bright_avg}");
    }

    #[test]
    fn sidecar_uses_camel_case_field_names() {
        let dir = tempfile::tempdir().unwrap();
        save_sprite(dir.path(), "k", &[thumb(0.0, 2, 2, 50)]).unwrap();
        let json = std::fs::read_to_string(json_path(dir.path(), "k")).unwrap();
        assert!(json.contains("\"tileWidth\""));
        assert!(json.contains("\"tileHeight\""));
        assert!(json.contains("\"columns\""));
        assert!(json.contains("\"times\""));
    }

    #[test]
    fn sidecar_written_after_jpg() {
        // Completeness marker: both files exist after save; absence of sidecar
        // means incomplete.
        let dir = tempfile::tempdir().unwrap();
        save_sprite(dir.path(), "k", &[thumb(0.0, 2, 2, 50)]).unwrap();
        assert!(jpg_path(dir.path(), "k").exists());
        assert!(json_path(dir.path(), "k").exists());
    }

    #[test]
    fn load_missing_sidecar_is_none() {
        let dir = tempfile::tempdir().unwrap();
        // write only a jpg, no sidecar → incomplete → None.
        let d = dir.path().join(CACHE_SUBDIR);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("k.thumbs.jpg"), b"not a real jpg").unwrap();
        assert!(load_sprite(dir.path(), "k").is_none());
    }

    #[test]
    fn load_corrupt_sidecar_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path().join(CACHE_SUBDIR);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("k.thumbs.json"), b"{ not json").unwrap();
        std::fs::write(d.join("k.thumbs.jpg"), b"xx").unwrap();
        assert!(load_sprite(dir.path(), "k").is_none());
    }

    #[test]
    fn empty_thumbs_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        save_sprite(dir.path(), "k", &[]).unwrap();
        assert!(!jpg_path(dir.path(), "k").exists());
        assert!(!json_path(dir.path(), "k").exists());
    }

    #[test]
    fn multi_row_sprite_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        // 60 tiles → 50 cols, 2 rows.
        let thumbs: Vec<_> = (0..60)
            .map(|i| thumb(i as f64, 2, 2, (i * 4) as u8))
            .collect();
        save_sprite(dir.path(), "multi", &thumbs).unwrap();
        let back = load_sprite(dir.path(), "multi").unwrap();
        assert_eq!(back.len(), 60);
        assert_eq!(back[59].time_secs, 59.0);
    }

    fn avg(f: &RgbaFrame) -> f64 {
        let mut sum = 0.0f64;
        let mut n = 0.0f64;
        for px in f.rgba.chunks_exact(4) {
            sum += (px[0] as f64 + px[1] as f64 + px[2] as f64) / 3.0;
            n += 1.0;
        }
        sum / n.max(1.0)
    }
}
