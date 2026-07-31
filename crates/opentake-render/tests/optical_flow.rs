use std::rc::Rc;

use opentake_media::{
    convert_frame_rate, interpolate_frame_pair, FrameInterpolationFallback, FrameInterpolationMode,
    RgbaFrame,
};
use opentake_render::gpu::compositor::{
    TextureInterpolationConfig, TextureInterpolationFallback, TextureInterpolationMode,
    TextureResolveRequest,
};
use opentake_render::{GpuTexture, TextureResolver, TextureSource};

fn moving_square(x: u32) -> RgbaFrame {
    let mut frame = RgbaFrame::black(8, 4);
    for y in 1..=2 {
        for px_x in x..x + 2 {
            let offset = ((y * frame.width + px_x) * 4) as usize;
            frame.rgba[offset..offset + 4].copy_from_slice(&[255, 255, 255, 255]);
        }
    }
    frame
}

fn light_centroid_x(frame: &RgbaFrame) -> f64 {
    let mut weighted_x = 0.0;
    let mut weight = 0.0;
    for y in 0..frame.height {
        for x in 0..frame.width {
            let offset = ((y * frame.width + x) * 4) as usize;
            let value = frame.rgba[offset] as f64;
            weighted_x += x as f64 * value;
            weight += value;
        }
    }
    weighted_x / weight
}

#[derive(Default)]
struct RecordingResolver {
    requests: Vec<TextureInterpolationConfig>,
}

impl TextureResolver for RecordingResolver {
    fn resolve(&mut self, _source: &TextureSource, _source_frame: i64) -> Option<Rc<GpuTexture>> {
        None
    }

    fn resolve_with_interpolation(
        &mut self,
        request: TextureResolveRequest<'_>,
    ) -> Option<Rc<GpuTexture>> {
        self.requests.push(request.interpolation);
        None
    }
}

#[test]
fn two_frame_fixture_is_deterministic_and_matches_preview_export() {
    let first = moving_square(1);
    let last = moving_square(5);
    let conversion = convert_frame_rate(2, 24.0, 60.0).expect("valid 24 to 60 conversion");

    assert_eq!(conversion.len(), 4);
    assert_eq!(conversion.first().unwrap().timestamp_secs, 0.0);
    assert_eq!(conversion.last().unwrap().timestamp_secs, 1.0 / 24.0);
    assert_eq!(conversion[1].source_frame, 0);
    assert_eq!(conversion[1].next_source_frame, 1);
    assert!((conversion[1].source_alpha - 0.4).abs() < 1e-12);
    assert!((conversion[2].source_alpha - 0.8).abs() < 1e-12);

    let render = |optical_flow_available| {
        let source = [&first, &last];
        conversion
            .iter()
            .map(|sample| {
                interpolate_frame_pair(
                    source[sample.source_frame as usize],
                    source[sample.next_source_frame as usize],
                    sample.source_alpha,
                    FrameInterpolationMode::OpticalFlow,
                    FrameInterpolationFallback::Blend,
                    optical_flow_available,
                )
                .expect("configured blend fallback is infallible")
            })
            .collect::<Vec<_>>()
    };
    let preview = render(true);
    let export = render(true);

    assert_eq!(preview, export);
    assert_eq!(preview.first().unwrap().frame, first);
    assert_eq!(preview.last().unwrap().frame, last);
    let centroids = preview
        .iter()
        .map(|result| light_centroid_x(&result.frame))
        .collect::<Vec<_>>();
    assert!(centroids.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(preview
        .iter()
        .all(|result| result.mode_used == FrameInterpolationMode::OpticalFlow));

    let fallback = render(false);
    assert!(fallback
        .iter()
        .all(|result| result.mode_used == FrameInterpolationMode::Blend));
    assert_ne!(fallback[1].frame, preview[1].frame);
    assert!(interpolate_frame_pair(
        &first,
        &last,
        0.5,
        FrameInterpolationMode::OpticalFlow,
        FrameInterpolationFallback::Error,
        false,
    )
    .is_err());

    let interpolation = TextureInterpolationConfig::new(
        24.0,
        60.0,
        TextureInterpolationMode::OpticalFlow,
        TextureInterpolationFallback::Blend,
    )
    .expect("valid render interpolation config");
    assert!(TextureInterpolationConfig::new(
        0.0,
        60.0,
        TextureInterpolationMode::OpticalFlow,
        TextureInterpolationFallback::Blend,
    )
    .is_err());
    let source = TextureSource::Decoded {
        media_ref: "motion-24fps".to_string(),
    };
    let mut preview_resolver = RecordingResolver::default();
    let mut export_resolver = RecordingResolver::default();
    let request = TextureResolveRequest {
        source: &source,
        source_frame: 1,
        interpolation,
    };
    preview_resolver.resolve_with_interpolation(request);
    export_resolver.resolve_with_interpolation(request);

    assert_eq!(preview_resolver.requests, export_resolver.requests);
    assert_eq!(preview_resolver.requests, vec![interpolation]);
}
