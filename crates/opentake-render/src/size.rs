//! Render-size even-ization and export scaling (SPEC §5.2). Pure functions for
//! the export backend / callers. Ports upstream `ExportResolution.renderSize`
//! (ExportService L39-46) and the `even` helper (TimelineRenderer L85).

use crate::plan::RenderSize;

/// Round to the nearest even integer, floored at 2. Port of upstream `even`
/// (TimelineRenderer L85) and the `Int(...) / 2 * 2` idiom in
/// `ExportResolution.renderSize` (ExportService L43-44): round, integer-divide
/// by 2, multiply by 2, clamp to >= 2.
pub fn even(v: f64) -> u32 {
    let rounded = v.round() as i64;
    ((rounded / 2) * 2).max(2) as u32
}

/// Standard export short-side targets (ExportService L31-37).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ExportResolution {
    R720p,
    R1080p,
    R4k,
}

impl ExportResolution {
    /// Short-side pixel target.
    pub fn short_side_pixels(self) -> i32 {
        match self {
            ExportResolution::R720p => 720,
            ExportResolution::R1080p => 1080,
            ExportResolution::R4k => 2160,
        }
    }
}

/// Export render size for a canvas at a chosen resolution. Port of upstream
/// `ExportResolution.renderSize(for:)` (ExportService L39-46): scale so the
/// canvas's short side hits `short_side_pixels`, then even-ize each axis (>= 2).
///
/// Note (SPEC §5.2): unlike `TimelineRenderer`, this does NOT clamp the scale to
/// 1.0 — exporting a small canvas at 4K upscales, matching the real export path.
pub fn export_render_size(canvas: (i32, i32), resolution: ExportResolution) -> RenderSize {
    let (cw, ch) = (canvas.0 as f64, canvas.1 as f64);
    let canvas_short = cw.min(ch);
    if canvas_short <= 0.0 {
        // Degenerate canvas: fall back to even-ized canvas (>= 2).
        return RenderSize::new(even(cw.max(2.0)), even(ch.max(2.0)));
    }
    let scale = resolution.short_side_pixels() as f64 / canvas_short;
    RenderSize::new(even(cw * scale), even(ch * scale))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn even_rounds_then_floors_at_two() {
        assert_eq!(even(0.0), 2);
        assert_eq!(even(1.0), 2); // round(1)=1 -> 0 -> max(2)
        assert_eq!(even(2.4), 2); // round=2
        assert_eq!(even(3.0), 2); // round=3 -> 3/2*2 = 2
        assert_eq!(even(4.0), 4);
        assert_eq!(even(5.0), 4); // 5/2*2 = 4
        assert_eq!(even(1919.5), 1920); // round=1920
        assert_eq!(even(1921.0), 1920); // 1921/2*2 = 1920
    }

    #[test]
    fn export_1080p_landscape() {
        // 1920x1080 canvas at 1080p: short side already 1080 -> scale 1.
        let rs = export_render_size((1920, 1080), ExportResolution::R1080p);
        assert_eq!(rs, RenderSize::new(1920, 1080));
    }

    #[test]
    fn export_720p_downscales() {
        // 1920x1080 -> short 1080, target 720 -> scale 720/1080 = 0.6667.
        // w = 1920 * 0.6667 = 1280, h = 720.
        let rs = export_render_size((1920, 1080), ExportResolution::R720p);
        assert_eq!(rs, RenderSize::new(1280, 720));
    }

    #[test]
    fn export_4k_upscales_no_clamp() {
        // 1920x1080 at 4K: short 1080 -> target 2160 -> scale 2.0.
        let rs = export_render_size((1920, 1080), ExportResolution::R4k);
        assert_eq!(rs, RenderSize::new(3840, 2160));
    }

    #[test]
    fn export_portrait_uses_short_side() {
        // 1080x1920 portrait at 1080p: short side is 1080 (width) -> scale 1.
        let rs = export_render_size((1080, 1920), ExportResolution::R1080p);
        assert_eq!(rs, RenderSize::new(1080, 1920));
        // at 720p: scale 720/1080 -> 720 x 1280.
        let rs = export_render_size((1080, 1920), ExportResolution::R720p);
        assert_eq!(rs, RenderSize::new(720, 1280));
    }

    #[test]
    fn export_degenerate_canvas_is_safe() {
        let rs = export_render_size((0, 0), ExportResolution::R1080p);
        assert!(rs.width >= 2 && rs.height >= 2);
    }
}
