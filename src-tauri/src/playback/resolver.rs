//! Streaming texture resolver for continuous playback (#53).
//!
//! Where the preview's [`crate::render::composite_frame`] resolves each video
//! layer with a fresh seek-per-frame `decode_frame_at` (correct but far too slow
//! for real-time multi-track playback), this resolver keeps **one forward
//! [`VideoStream`] per active clip** and pulls frames out of each clip's bounded
//! queue to match the frame the compositor is asking for. Sequential decode (no
//! per-frame seek) is the whole point — that is what makes high-bitrate / ProRes
//! playback smooth.
//!
//! ## Two-part shape (why a persistent state + a transient resolver)
//! The compositor's [`TextureResolver`] trait hands `resolve()` only a
//! `(&TextureSource, source_frame)` — **no `clip_id`**. But stream *lifecycle*
//! must be keyed by clip id (a split clip, or the same asset reused twice, needs
//! its own decode position). So lifecycle can't live inside `resolve()`.
//!
//! Instead the render thread owns the persistent [`PlaybackResolverState`]
//! (the per-clip streams + the static image/text caches), and each frame wraps it
//! in a transient [`StreamingResolver`] that borrows the wgpu device + the state.
//! Before compositing, the thread calls [`StreamingResolver::sync_active`] with
//! the frame's [`FramePlan`]: that adds/stops streams by `clip_id`, advances each
//! to its target `source_frame`, and pre-uploads the matching textures into a
//! per-frame lookup keyed `v:{media_ref}:{source_frame}` (the *content* is fully
//! determined by media + source frame, so the key needs no clip id). `resolve()`
//! then degrades to a table lookup for video, and the usual static cache for
//! image / text.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use opentake_media::decode::{
    spawn_video_stream, StreamVideoFrame, VideoStream, VideoStreamRequest,
};
use opentake_media::{decode_frame_at, FrameRequest};
use opentake_render::gpu::texture::upload_rgba;
use opentake_render::wgpu;
use opentake_render::{
    CosmicTextRasterizer, DecodedFrame, FramePlan, GpuTexture, TextRasterRequest, TextRasterizer,
    TextureCache, TextureResolver, TextureSource,
};

use super::project::{MediaInfo, TextInfo};

/// Per-frame texture cache size for static (image / text) layers. Video frames
/// are NOT cached here — they live in each clip's stream and are uploaded per
/// frame. Bounds VRAM for the static layers.
const STATIC_CACHE_CAP: usize = 64;

/// One active video clip's continuous-decode state. Created when a clip first
/// appears in a frame plan, dropped (after a cooperative stop) when it leaves.
struct ClipStream {
    /// The forward ffmpeg decode worker for this clip's source.
    stream: VideoStream,
    /// A frame pulled off the queue that is *ahead* of the current target, held
    /// for a future tick instead of being discarded (slow-motion / dup frames).
    pending: Option<StreamVideoFrame>,
    /// Most recently uploaded texture, reused when decode falls behind the
    /// target ("drop video, keep the clock moving").
    cached_tex: Option<Rc<GpuTexture>>,
}

impl ClipStream {
    fn new(stream: VideoStream) -> Self {
        ClipStream {
            stream,
            pending: None,
            cached_tex: None,
        }
    }

    /// Advance this clip's stream to `target`, uploading the matched frame to the
    /// GPU and caching it. When decode is behind (no frame at/after target yet),
    /// the previous `cached_tex` is retained so the layer holds its last frame
    /// rather than flickering to black.
    fn advance(&mut self, target: i64, device: &wgpu::Device, queue: &wgpu::Queue) {
        let next = {
            let rx = self.stream.receiver();
            // Decode errors are skipped (treated as "no frame this pull"): a
            // transient error reuses the last frame; a dead stream simply holds
            // the last frame. Surfacing decode errors to the UI is a PR2 concern.
            drain_to_target(
                &mut self.pending,
                || rx.try_recv().ok().and_then(|r| r.ok()),
                target,
            )
        };
        if let Some(vf) = next {
            let decoded = DecodedFrame::new(vf.frame.width, vf.frame.height, vf.frame.rgba, false);
            let tex = upload_rgba(device, queue, &decoded, false, Some("playback-src"));
            self.cached_tex = Some(Rc::new(tex));
        }
    }
}

/// Pure drain decision: pick the queued frame to display at `target`, discarding
/// stale (behind-target) frames and stashing an ahead-of-target frame in
/// `pending` for a later tick. Returns `Some(frame)` when a frame *at* `target`
/// is available (caller uploads it), or `None` to reuse the cached texture
/// (decode is behind, or the only available frame is still ahead).
///
/// `pull` is the non-blocking queue read (`try_recv`); it returns `None` when the
/// queue is momentarily empty — the render loop never blocks on decode.
fn drain_to_target(
    pending: &mut Option<StreamVideoFrame>,
    mut pull: impl FnMut() -> Option<StreamVideoFrame>,
    target: i64,
) -> Option<StreamVideoFrame> {
    // A frame stashed on a previous tick takes priority over the live queue.
    if let Some(p) = pending.take() {
        if p.source_frame == target {
            return Some(p);
        }
        if p.source_frame > target {
            *pending = Some(p); // still ahead: keep it, reuse cache this tick
            return None;
        }
        // p.source_frame < target: stale, drop and fall through to the queue.
    }
    while let Some(f) = pull() {
        if f.source_frame < target {
            continue; // behind target: discard (fast-forward / normal advance)
        }
        if f.source_frame == target {
            return Some(f);
        }
        // Over-pulled past the target: stash for a later tick, reuse cache now.
        *pending = Some(f);
        return None;
    }
    None
}

/// The render-thread-owned persistent state behind the streaming resolver: the
/// per-clip decode streams plus the static (image / text) texture cache. Lives
/// for the whole playback session and is wrapped in a transient
/// [`StreamingResolver`] each frame.
pub struct PlaybackResolverState {
    /// Active video streams, keyed by **clip id** (NOT media_ref): a split clip
    /// or a reused asset needs an independent decode position.
    streams: HashMap<String, ClipStream>,
    /// Image + text textures (persistent across frames).
    static_cache: TextureCache,
    text_rasterizer: CosmicTextRasterizer,
    media: HashMap<String, MediaInfo>,
    text: HashMap<String, TextInfo>,
    timeline_fps: i32,
    /// Decode / raster downscale box (matches the playback render size).
    render_box: (u32, u32),
}

impl PlaybackResolverState {
    pub fn new(
        media: HashMap<String, MediaInfo>,
        text: HashMap<String, TextInfo>,
        timeline_fps: i32,
        render_box: (u32, u32),
    ) -> Self {
        PlaybackResolverState {
            streams: HashMap::new(),
            static_cache: TextureCache::new(STATIC_CACHE_CAP),
            text_rasterizer: CosmicTextRasterizer::new(),
            media,
            text,
            timeline_fps,
            render_box,
        }
    }

    /// Stop and drop every active stream (used on seek: streams restart at the
    /// new position on the next `sync_active`). Cooperative stop is requested;
    /// the worker threads are reaped in the background via `Drop`, never joined
    /// on the render thread.
    pub fn clear_streams(&mut self) {
        for (_, cs) in self.streams.drain() {
            cs.stream.request_stop();
        }
    }
}

/// One video layer's decode target for a frame: which clip, which asset, and the
/// integer source frame the plan asked for.
struct VideoTarget {
    clip_id: String,
    media_ref: String,
    source_frame: i64,
}

/// Extract the per-clip video decode targets from a frame plan (the `Decoded`
/// layers). Image / text / Lottie layers carry no stream.
fn video_targets(plan: &FramePlan) -> Vec<VideoTarget> {
    plan.draws
        .iter()
        .filter_map(|d| match d.source {
            TextureSource::Decoded { media_ref } => Some(VideoTarget {
                clip_id: d.clip_id.to_string(),
                media_ref: media_ref.clone(),
                source_frame: d.source_frame,
            }),
            _ => None,
        })
        .collect()
}

/// A transient, per-frame [`TextureResolver`] over the persistent
/// [`PlaybackResolverState`] and the render thread's wgpu device. Built fresh
/// each frame; `sync_active` must be called before handing it to the compositor.
pub struct StreamingResolver<'d, 's> {
    device: &'d wgpu::Device,
    queue: &'d wgpu::Queue,
    state: &'s mut PlaybackResolverState,
    /// Per-frame video lookup, keyed `v:{media_ref}:{source_frame}`. Filled by
    /// `sync_active`, read by `resolve`.
    frame_tex: HashMap<String, Rc<GpuTexture>>,
}

impl<'d, 's> StreamingResolver<'d, 's> {
    pub fn new(
        device: &'d wgpu::Device,
        queue: &'d wgpu::Queue,
        state: &'s mut PlaybackResolverState,
    ) -> Self {
        StreamingResolver {
            device,
            queue,
            state,
            frame_tex: HashMap::new(),
        }
    }

    /// Reconcile the active video streams with this frame's plan and pre-upload
    /// each clip's current texture. Must run before `render_to_rgba`.
    ///
    /// 1. Stop streams whose clip is no longer on screen.
    /// 2. Spawn a stream for each newly-visible clip (decoding from its target
    ///    source frame — the stream's first output frame lands exactly there).
    /// 3. Advance every active stream to its target and stash the resulting
    ///    texture in the per-frame lookup.
    pub fn sync_active(&mut self, plan: &FramePlan) {
        let targets = video_targets(plan);
        let active_ids: HashSet<&str> = targets.iter().map(|t| t.clip_id.as_str()).collect();

        // 1. Drop streams for clips that left the frame.
        self.state.streams.retain(|id, cs| {
            if active_ids.contains(id.as_str()) {
                true
            } else {
                cs.stream.request_stop();
                false
            }
        });

        // 2 + 3. Spawn missing streams, advance all, collect textures. Textures
        // are gathered into a local Vec first so `frame_tex` is not borrowed
        // while `state.streams` is.
        let mut uploaded: Vec<(String, Rc<GpuTexture>)> = Vec::with_capacity(targets.len());
        for t in &targets {
            if !self.state.streams.contains_key(&t.clip_id) {
                if let Some(info) = self.state.media.get(&t.media_ref) {
                    let mut req =
                        VideoStreamRequest::new(info.path.clone(), self.state.timeline_fps);
                    req.start_frame = t.source_frame.max(0);
                    req.max_size = self.state.render_box;
                    if let Ok(stream) = spawn_video_stream(req) {
                        self.state
                            .streams
                            .insert(t.clip_id.clone(), ClipStream::new(stream));
                    }
                }
            }
            if let Some(cs) = self.state.streams.get_mut(&t.clip_id) {
                cs.advance(t.source_frame, self.device, self.queue);
                if let Some(tex) = &cs.cached_tex {
                    uploaded.push((format!("v:{}:{}", t.media_ref, t.source_frame), tex.clone()));
                }
            }
        }

        self.frame_tex.clear();
        for (key, tex) in uploaded {
            self.frame_tex.insert(key, tex);
        }
    }

    /// Decode (once) and cache a static image layer, mirroring the preview
    /// resolver's image path.
    fn resolve_image(&mut self, media_ref: &str) -> Option<Rc<GpuTexture>> {
        let key = format!("i:{media_ref}");
        if let Some(tex) = self.state.static_cache.get(&key) {
            return Some(tex);
        }
        let info = self.state.media.get(media_ref)?;
        let req = FrameRequest {
            time_secs: 0.0,
            max_size: self.state.render_box,
            tolerance_secs: 0.0,
            apply_rotation: true,
        };
        let (_actual, frame) = decode_frame_at(&info.path, &req).ok()?;
        let decoded = DecodedFrame::new(frame.width, frame.height, frame.rgba, false);
        let tex = upload_rgba(
            self.device,
            self.queue,
            &decoded,
            false,
            Some("playback-image"),
        );
        Some(self.state.static_cache.insert(key, tex))
    }

    /// Rasterize (once) and cache a text layer, mirroring the preview resolver's
    /// text path (premultiplied RGBA box, composited above video).
    fn resolve_text(&mut self, clip_id: &str) -> Option<Rc<GpuTexture>> {
        let key = format!("t:{clip_id}");
        if let Some(tex) = self.state.static_cache.get(&key) {
            return Some(tex);
        }
        let info = self.state.text.get(clip_id)?;
        let req = TextRasterRequest {
            clip_id,
            content: &info.content,
            style: &info.style,
            box_norm: info.box_norm,
            canvas: self.state.render_box,
        };
        let frame = self.state.text_rasterizer.rasterize(&req)?;
        let tex = upload_rgba(
            self.device,
            self.queue,
            &frame,
            false,
            Some("playback-text"),
        );
        Some(self.state.static_cache.insert(key, tex))
    }
}

impl TextureResolver for StreamingResolver<'_, '_> {
    fn resolve(&mut self, source: &TextureSource, source_frame: i64) -> Option<Rc<GpuTexture>> {
        match source {
            // Video: pre-uploaded by `sync_active`; the content is fully keyed by
            // (media_ref, source_frame), so no clip id is needed here. A miss
            // (decode not warmed up) returns None and the compositor skips the
            // layer for this frame.
            TextureSource::Decoded { media_ref } => self
                .frame_tex
                .get(&format!("v:{media_ref}:{source_frame}"))
                .cloned(),
            TextureSource::Image { media_ref } => self.resolve_image(media_ref),
            TextureSource::Text { clip_id } => self.resolve_text(clip_id),
            // Lottie bake wiring lands with #65 (PR3); skipped for now, matching
            // the preview resolver (`render.rs`).
            TextureSource::Lottie { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_media::RgbaFrame;

    fn vf(source_frame: i64) -> StreamVideoFrame {
        StreamVideoFrame {
            source_frame,
            pts_secs: source_frame as f64 / 30.0,
            frame: RgbaFrame::new(1, 1, vec![0, 0, 0, 255]),
        }
    }

    /// A `pull` closure draining a fixed queue in order.
    fn queue_pull(frames: Vec<StreamVideoFrame>) -> impl FnMut() -> Option<StreamVideoFrame> {
        let mut it = frames.into_iter();
        move || it.next()
    }

    #[test]
    fn drain_exact_hit_returns_target_frame() {
        let mut pending = None;
        let got = drain_to_target(&mut pending, queue_pull(vec![vf(5)]), 5);
        assert_eq!(got.map(|f| f.source_frame), Some(5));
        assert!(pending.is_none());
    }

    #[test]
    fn drain_discards_frames_behind_target() {
        let mut pending = None;
        // Normal forward advance: 3 and 4 are stale, 5 is the target.
        let got = drain_to_target(&mut pending, queue_pull(vec![vf(3), vf(4), vf(5)]), 5);
        assert_eq!(got.map(|f| f.source_frame), Some(5));
        assert!(pending.is_none());
    }

    #[test]
    fn drain_stashes_ahead_frame_and_reuses_cache() {
        let mut pending = None;
        // Only a future frame is available (slow-mo / dup): reuse cache now, keep 7.
        let got = drain_to_target(&mut pending, queue_pull(vec![vf(7)]), 5);
        assert!(got.is_none());
        assert_eq!(pending.as_ref().map(|f| f.source_frame), Some(7));
    }

    #[test]
    fn drain_consumes_pending_when_target_catches_up() {
        let mut pending = Some(vf(7));
        // Queue empty; target now equals the stashed frame -> use it.
        let got = drain_to_target(&mut pending, queue_pull(vec![]), 7);
        assert_eq!(got.map(|f| f.source_frame), Some(7));
        assert!(pending.is_none());
    }

    #[test]
    fn drain_keeps_pending_while_still_ahead() {
        let mut pending = Some(vf(8));
        let got = drain_to_target(&mut pending, queue_pull(vec![]), 5);
        assert!(got.is_none());
        assert_eq!(pending.as_ref().map(|f| f.source_frame), Some(8));
    }

    #[test]
    fn drain_drops_stale_pending_then_pulls_target() {
        let mut pending = Some(vf(2));
        let got = drain_to_target(&mut pending, queue_pull(vec![vf(5)]), 5);
        assert_eq!(got.map(|f| f.source_frame), Some(5));
        assert!(pending.is_none());
    }

    #[test]
    fn drain_empty_queue_reuses_cache() {
        let mut pending = None;
        let got = drain_to_target(&mut pending, queue_pull(vec![]), 5);
        assert!(got.is_none());
        assert!(pending.is_none());
    }
}
