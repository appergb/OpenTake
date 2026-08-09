//! Real GPU acceptance for project-managed 3D `.cube` LUTs.

use std::rc::Rc;

use opentake_domain::{Clip, ClipType, CubeLut, LutReference, Point, Timeline, Track, Transform};
use opentake_render::gpu::texture::{upload_lut_3d, upload_rgba};
use opentake_render::source::DecodedFrame;
use opentake_render::wgpu;
use opentake_render::{
    build_render_plan, Compositor, GpuLutTexture, GpuTexture, RenderDevice, RenderError,
    RenderSize, SourceMetrics, TextureResolver, TextureSource,
};

const RS: RenderSize = RenderSize {
    width: 16,
    height: 16,
};

struct Metrics;
impl SourceMetrics for Metrics {
    fn natural_size(&self, _media_ref: &str) -> Option<(u32, u32)> {
        Some((16, 16))
    }
}

struct LutResolver<'d> {
    device: &'d wgpu::Device,
    queue: &'d wgpu::Queue,
    source: Option<Rc<GpuTexture>>,
    lut: CubeLut,
    uploaded_lut: Option<Rc<GpuLutTexture>>,
}

impl TextureResolver for LutResolver<'_> {
    fn resolve(&mut self, _source: &TextureSource, _frame: i64) -> Option<Rc<GpuTexture>> {
        if self.source.is_none() {
            let frame = DecodedFrame::new(16, 16, [64, 128, 192, 255].repeat(16 * 16), true);
            self.source = Some(Rc::new(upload_rgba(
                self.device,
                self.queue,
                &frame,
                false,
                Some("lut-source"),
            )));
        }
        self.source.clone()
    }

    fn resolve_lut(
        &mut self,
        _reference: &LutReference,
    ) -> Result<Option<Rc<GpuLutTexture>>, RenderError> {
        if self.uploaded_lut.is_none() {
            self.uploaded_lut = Some(Rc::new(upload_lut_3d(
                self.device,
                self.queue,
                &self.lut,
                Some("known-transform-lut"),
            )));
        }
        Ok(self.uploaded_lut.clone())
    }
}

fn cube(size: usize, transform: impl Fn(f32, f32, f32) -> [f32; 3]) -> Vec<u8> {
    let mut text =
        format!("TITLE \"acceptance\"\nLUT_3D_SIZE {size}\nDOMAIN_MIN 0 0 0\nDOMAIN_MAX 1 1 1\n");
    let last = (size - 1) as f32;
    for b in 0..size {
        for g in 0..size {
            for r in 0..size {
                let [r, g, b] = transform(r as f32 / last, g as f32 / last, b as f32 / last);
                text.push_str(&format!("{r:.7} {g:.7} {b:.7}\n"));
            }
        }
    }
    text.into_bytes()
}

fn timeline(reference: LutReference) -> Timeline {
    let mut timeline = Timeline::new();
    timeline.width = 16;
    timeline.height = 16;
    timeline.fps = 30;
    let mut clip = Clip::new("clip", "asset", 0, 30);
    clip.transform = Transform::from_top_left(Point { x: 0.0, y: 0.0 }, 1.0, 1.0);
    clip.lut = Some(reference);
    let mut track = Track::new("track", ClipType::Video);
    track.clips.push(clip);
    timeline.tracks.push(track);
    timeline
}

fn render(dev: &RenderDevice, timeline: &Timeline, lut: CubeLut) -> DecodedFrame {
    let plan = build_render_plan(timeline, RS, &Metrics);
    let frame = plan.frame(timeline, 0);
    let compositor = Compositor::new(&dev.device);
    let mut resolver = LutResolver {
        device: &dev.device,
        queue: &dev.queue,
        source: None,
        lut,
        uploaded_lut: None,
    };
    compositor
        .render_to_rgba(&dev.device, &dev.queue, RS, &frame, &mut resolver)
        .expect("render with a valid LUT")
}

#[test]
fn malformed_and_oversized_luts_fail_closed_and_valid_lut_matches_preview_export() {
    let malformed = b"LUT_3D_SIZE 17\n0 0 0\n";
    assert!(CubeLut::parse(malformed).is_err(), "short table must fail");
    assert!(
        CubeLut::parse(&vec![b' '; CubeLut::MAX_BYTES + 1]).is_err(),
        "oversized input must fail before parsing"
    );
    assert!(
        CubeLut::parse(&cube(16, |r, g, b| [r, g, b])).is_err(),
        "only planned 17- and 33-point tables are accepted"
    );

    let identity = CubeLut::parse(&cube(17, |r, g, b| [r, g, b])).expect("17-point identity");
    let transform =
        CubeLut::parse(&cube(33, |r, g, b| [b, g * 0.5, r])).expect("33-point transform");
    assert_eq!(identity.size(), 17);
    assert_eq!(transform.size(), 33);

    let reference = LutReference::new(
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "Known Transform",
        0.75,
    )
    .expect("managed reference");
    assert_eq!(
        reference.relative_path(),
        "media/luts/0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef.cube"
    );

    let authored = timeline(reference.clone());
    let json = serde_json::to_vec(&authored).expect("save project timeline");
    let reopened: Timeline = serde_json::from_slice(&json).expect("reopen project timeline");
    assert_eq!(reopened.tracks[0].clips[0].lut.as_ref(), Some(&reference));

    let Ok(dev) = RenderDevice::try_new() else {
        eprintln!("[skip] LUT GPU acceptance: no GPU device");
        return;
    };
    let preview = render(&dev, &reopened, transform.clone());
    let export = render(&dev, &reopened, transform);
    assert_eq!(preview.rgba, export.rgba, "preview/export LUT drift");

    let center = (8 * 16 + 8) * 4;
    let pixel = &preview.rgba[center..center + 4];
    assert!(pixel[0] > 140, "blue-to-red transform missing: {pixel:?}");
    assert!(pixel[1] < 100, "green attenuation missing: {pixel:?}");
    assert!(pixel[2] < 100, "red-to-blue transform missing: {pixel:?}");

    let identity_timeline = timeline(LutReference::new(reference.id, "Identity", 1.0).unwrap());
    let identity_frame = render(&dev, &identity_timeline, identity);
    let identity_pixel = &identity_frame.rgba[center..center + 4];
    for (actual, expected) in identity_pixel.iter().zip([64_u8, 128, 192, 255]) {
        assert!((i16::from(*actual) - i16::from(expected)).abs() <= 2);
    }
}
