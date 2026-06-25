//! End-to-end pipeline test: source → request → cache (miss/hit) → rendered
//! frames → consumed through the `opentake-render` clip-source traits. Uses the
//! deterministic `StubRenderer`, so it runs offline with no Chromium.

use std::path::Path;

use opentake_motion::{
    content_hash, MotionCache, MotionClipSource, MotionRenderRequest, MotionRenderer, MotionSource,
    StubRenderer,
};
use opentake_render::{FrameProvider, SourceMetrics};

/// Decode the stub's PNGs back to RGBA via the `image` dev-dep.
fn image_decoder(path: &Path) -> Option<opentake_render::DecodedFrame> {
    let img = image::open(path).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Some(opentake_render::DecodedFrame::new(
        w,
        h,
        img.into_raw(),
        false,
    ))
}

#[test]
fn full_pipeline_render_cache_and_ingest() {
    let tmp = tempfile::tempdir().unwrap();
    let cache = MotionCache::new(tmp.path());
    let renderer = StubRenderer::new(cache.clone());

    let req = MotionRenderRequest::new(
        MotionSource::code("<style>body{background:transparent}</style><h1>Title</h1>"),
        30,
        10,
        320,
        180,
    );
    req.validate().unwrap();

    // First render is a cache miss.
    assert!(!cache.is_cached(&req));
    let clip = renderer.render(&req).unwrap();
    assert_eq!(clip.frame_count(), 10);
    assert_eq!(clip.content_hash, content_hash(&req));

    // Now the cache reports a hit (complete frame set on disk).
    assert!(cache.is_cached(&req));

    // Consume the clip through the render-crate clip-source traits.
    let source = MotionClipSource::new(clip, image_decoder);
    assert_eq!(source.natural_size("motion-ref"), Some((320, 180)));
    assert!(source.needs_premultiply("motion-ref")); // transparent overlay

    let frame3 = source
        .decoded_frame("motion-ref", 3)
        .expect("frame 3 decodes");
    assert_eq!((frame3.width, frame3.height), (320, 180));
    assert_eq!(frame3.rgba.len(), 320 * 180 * 4);
}

#[test]
fn changing_source_invalidates_cache_key() {
    let tmp = tempfile::tempdir().unwrap();
    let cache = MotionCache::new(tmp.path());
    let renderer = StubRenderer::new(cache.clone());

    let req_a = MotionRenderRequest::new(MotionSource::code("<h1>A</h1>"), 30, 4, 64, 64);
    let req_b = MotionRenderRequest::new(MotionSource::code("<h1>B</h1>"), 30, 4, 64, 64);

    let clip_a = renderer.render(&req_a).unwrap();
    // A is cached; B is not (different content hash).
    assert!(cache.is_cached(&req_a));
    assert!(!cache.is_cached(&req_b));

    let clip_b = renderer.render(&req_b).unwrap();
    assert_ne!(clip_a.content_hash, clip_b.content_hash);
    assert!(cache.is_cached(&req_b));
}

#[test]
fn rerender_identical_request_is_a_hit_and_reuses_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let cache = MotionCache::new(tmp.path());
    let renderer = StubRenderer::new(cache.clone());

    let req = MotionRenderRequest::new(MotionSource::code("<x/>"), 24, 3, 32, 32);
    let first = renderer.render(&req).unwrap();
    assert!(cache.is_cached(&req));

    // A second render of the exact same request lands in the same directory and
    // produces the same hash (the host would skip re-rendering on the hit).
    let second = renderer.render(&req).unwrap();
    assert_eq!(first.content_hash, second.content_hash);
    assert_eq!(first.frames, second.frames);
    assert_eq!(cache.dir_for(&req), cache.dir_for_hash(&first.content_hash));
}
