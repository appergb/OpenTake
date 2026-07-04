#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    Rgb,
    Rgba,
}

impl PixelFormat {
    fn channels(self) -> usize {
        match self {
            PixelFormat::Rgb => 3,
            PixelFormat::Rgba => 4,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FrameBuffer<'a> {
    pub width: u32,
    pub height: u32,
    pub data: &'a [u8],
    pub pixel_format: PixelFormat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CropRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CropTransform {
    pub scale_x: f32,
    pub scale_y: f32,
    pub translate_x: f32,
    pub translate_y: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AutocropPlan {
    pub crop: CropRect,
    pub transform: CropTransform,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AutocropConfig {
    pub black_threshold: u8,
    pub min_alpha: u8,
    pub sample_step: u32,
    pub target_aspect_ratio: Option<f32>,
}

impl Default for AutocropConfig {
    fn default() -> Self {
        AutocropConfig {
            black_threshold: 16,
            min_alpha: 16,
            sample_step: 1,
            target_aspect_ratio: None,
        }
    }
}

pub fn detect_autocrop(frame: &FrameBuffer<'_>, config: AutocropConfig) -> Option<AutocropPlan> {
    let channels = frame.pixel_format.channels();
    let width = frame.width as usize;
    let height = frame.height as usize;
    let expected_len = width.checked_mul(height)?.checked_mul(channels)?;
    if width == 0 || height == 0 || frame.data.len() < expected_len {
        return None;
    }

    let step = config.sample_step.max(1) as usize;
    let mut bounds = ContentBounds::empty();
    for y in (0..height).step_by(step) {
        for x in (0..width).step_by(step) {
            if is_content(frame, x, y, config) {
                bounds.include(x as u32, y as u32);
            }
        }
    }

    let mut crop = bounds.to_crop_rect().unwrap_or(CropRect {
        x: 0,
        y: 0,
        width: frame.width,
        height: frame.height,
    });

    if let Some(aspect) = config.target_aspect_ratio.filter(|aspect| *aspect > 0.0) {
        crop = expand_to_aspect(crop, frame.width, frame.height, aspect);
    }

    Some(AutocropPlan {
        crop,
        transform: crop_transform(crop, frame.width, frame.height),
    })
}

fn is_content(frame: &FrameBuffer<'_>, x: usize, y: usize, config: AutocropConfig) -> bool {
    let channels = frame.pixel_format.channels();
    let base = (y * frame.width as usize + x) * channels;
    let r = frame.data[base];
    let g = frame.data[base + 1];
    let b = frame.data[base + 2];
    let alpha_ok =
        frame.pixel_format == PixelFormat::Rgb || frame.data[base + 3] >= config.min_alpha;
    alpha_ok && r.max(g).max(b) > config.black_threshold
}

#[derive(Clone, Copy)]
struct ContentBounds {
    min_x: u32,
    min_y: u32,
    max_x: u32,
    max_y: u32,
    found: bool,
}

impl ContentBounds {
    fn empty() -> Self {
        ContentBounds {
            min_x: u32::MAX,
            min_y: u32::MAX,
            max_x: 0,
            max_y: 0,
            found: false,
        }
    }

    fn include(&mut self, x: u32, y: u32) {
        self.min_x = self.min_x.min(x);
        self.min_y = self.min_y.min(y);
        self.max_x = self.max_x.max(x);
        self.max_y = self.max_y.max(y);
        self.found = true;
    }

    fn to_crop_rect(self) -> Option<CropRect> {
        self.found.then_some(CropRect {
            x: self.min_x,
            y: self.min_y,
            width: self.max_x - self.min_x + 1,
            height: self.max_y - self.min_y + 1,
        })
    }
}

fn expand_to_aspect(rect: CropRect, frame_width: u32, frame_height: u32, target: f32) -> CropRect {
    let current = rect.width as f32 / rect.height as f32;
    if (current - target).abs() <= f32::EPSILON {
        return rect;
    }

    if current < target {
        let desired_width = ((rect.height as f32 * target).ceil() as u32).min(frame_width);
        expand_width(rect, desired_width.max(rect.width), frame_width)
    } else {
        let desired_height = ((rect.width as f32 / target).ceil() as u32).min(frame_height);
        expand_height(rect, desired_height.max(rect.height), frame_height)
    }
}

fn expand_width(rect: CropRect, width: u32, frame_width: u32) -> CropRect {
    let center = rect.x as i64 + rect.width as i64 / 2;
    let mut x = center - width as i64 / 2;
    x = x.clamp(0, (frame_width - width) as i64);
    CropRect {
        x: x as u32,
        width,
        ..rect
    }
}

fn expand_height(rect: CropRect, height: u32, frame_height: u32) -> CropRect {
    let center = rect.y as i64 + rect.height as i64 / 2;
    let mut y = center - height as i64 / 2;
    y = y.clamp(0, (frame_height - height) as i64);
    CropRect {
        y: y as u32,
        height,
        ..rect
    }
}

fn crop_transform(crop: CropRect, frame_width: u32, frame_height: u32) -> CropTransform {
    let crop_center_x = crop.x as f32 + crop.width as f32 / 2.0;
    let crop_center_y = crop.y as f32 + crop.height as f32 / 2.0;
    let frame_center_x = frame_width as f32 / 2.0;
    let frame_center_y = frame_height as f32 / 2.0;
    CropTransform {
        scale_x: frame_width as f32 / crop.width as f32,
        scale_y: frame_height as f32 / crop.height as f32,
        translate_x: (frame_center_x - crop_center_x) / frame_width as f32 * 2.0,
        translate_y: (frame_center_y - crop_center_y) / frame_height as f32 * 2.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn black_bars_generate_crop_rect_and_transform() {
        let width = 8;
        let height = 6;
        let mut rgb = vec![0u8; width * height * 3];
        for y in 1..5 {
            for x in 2..6 {
                let base = (y * width + x) * 3;
                rgb[base] = 240;
                rgb[base + 1] = 240;
                rgb[base + 2] = 240;
            }
        }

        let frame = FrameBuffer {
            width: width as u32,
            height: height as u32,
            data: &rgb,
            pixel_format: PixelFormat::Rgb,
        };
        let plan = detect_autocrop(&frame, AutocropConfig::default())
            .expect("valid RGB frame should produce a plan");

        assert_eq!(
            plan.crop,
            CropRect {
                x: 2,
                y: 1,
                width: 4,
                height: 4,
            }
        );
        assert!((plan.transform.scale_x - 2.0).abs() < f32::EPSILON);
        assert!((plan.transform.scale_y - 1.5).abs() < f32::EPSILON);
        assert!(plan.transform.translate_x.abs() < f32::EPSILON);
        assert!(plan.transform.translate_y.abs() < f32::EPSILON);
    }
}
