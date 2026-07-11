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
//! per-frame lookup keyed `v:{media_ref}:{source_frame}`. Multiple clips may
//! share that key; when one decoder is behind, the exact-frame candidate wins
//! over its stale fallback regardless of draw order. `resolve()` then degrades
//! to a table lookup for video, and the usual static cache for image / text.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::mpsc::TryRecvError;

use opentake_media::decode::{
    spawn_video_stream, StreamVideoFrame, VideoStream, VideoStreamRequest,
};
use opentake_media::{decode_frame_at_cancellable, FrameRequest, MediaCancelToken, MediaError};
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
    cached_tex: Rc<GpuTexture>,
    /// Source-frame identity of `cached_tex`. This may trail the requested
    /// target while the forward decoder catches up.
    cached_source_frame: i64,
}

impl ClipStream {
    fn new(stream: VideoStream, cached_tex: Rc<GpuTexture>, cached_source_frame: i64) -> Self {
        ClipStream {
            stream,
            pending: None,
            cached_tex,
            cached_source_frame,
        }
    }

    /// Advance this clip's single decoder stream to `target`, uploading and
    /// caching the matched frame. Cold bootstrap is completed synchronously
    /// before construction; subsequent calls are non-blocking and retain
    /// `cached_tex` when decode falls behind.
    fn advance(
        &mut self,
        target: i64,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<(), String> {
        let cached_source_frame = self.cached_source_frame;
        let next = {
            let rx = self.stream.receiver();
            drain_to_target(
                &mut self.pending,
                || classify_stream_pull(rx.try_recv(), cached_source_frame, target),
                target,
            )?
        };
        if let Some(vf) = next {
            self.cached_source_frame = vf.source_frame;
            let decoded = DecodedFrame::new(vf.frame.width, vf.frame.height, vf.frame.rgba, false);
            let tex = upload_rgba(device, queue, &decoded, false, Some("playback-src"));
            self.cached_tex = Rc::new(tex);
        }
        Ok(())
    }
}

fn classify_stream_pull(
    result: Result<Result<StreamVideoFrame, MediaError>, TryRecvError>,
    cached_source_frame: i64,
    target: i64,
) -> Result<Option<StreamVideoFrame>, String> {
    match result {
        Ok(Ok(frame)) => Ok(Some(frame)),
        Ok(Err(error)) => Err(format!(
            "playback continuous decode failed before source frame {target}: {error}"
        )),
        Err(TryRecvError::Empty) => Ok(None),
        Err(TryRecvError::Disconnected) if cached_source_frame >= target => Ok(None),
        Err(TryRecvError::Disconnected) => Err(format!(
            "playback continuous decode ended at source frame {cached_source_frame} before requested source frame {target}"
        )),
    }
}

fn ensure_stream_with<'a, S, E>(
    streams: &'a mut HashMap<String, S>,
    clip_id: &str,
    create: impl FnOnce() -> Result<S, E>,
) -> Result<&'a mut S, E> {
    use std::collections::hash_map::Entry;

    match streams.entry(clip_id.to_string()) {
        Entry::Occupied(entry) => Ok(entry.into_mut()),
        Entry::Vacant(entry) => Ok(entry.insert(create()?)),
    }
}

fn bootstrap_frame_request(
    source_frame: i64,
    timeline_fps: i32,
    render_box: (u32, u32),
) -> FrameRequest {
    FrameRequest {
        time_secs: source_frame.max(0) as f64 / timeline_fps.max(1) as f64,
        max_size: render_box,
        tolerance_secs: 0.0,
        apply_rotation: true,
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
fn drain_to_target<E>(
    pending: &mut Option<StreamVideoFrame>,
    mut pull: impl FnMut() -> Result<Option<StreamVideoFrame>, E>,
    target: i64,
) -> Result<Option<StreamVideoFrame>, E> {
    // A frame stashed on a previous tick takes priority over the live queue.
    if let Some(p) = pending.take() {
        if p.source_frame == target {
            return Ok(Some(p));
        }
        if p.source_frame > target {
            *pending = Some(p); // still ahead: keep it, reuse cache this tick
            return Ok(None);
        }
        // p.source_frame < target: stale, drop and fall through to the queue.
    }
    while let Some(f) = pull()? {
        if f.source_frame < target {
            continue; // behind target: discard (fast-forward / normal advance)
        }
        if f.source_frame == target {
            return Ok(Some(f));
        }
        // Over-pulled past the target: stash for a later tick, reuse cache now.
        *pending = Some(f);
        return Ok(None);
    }
    Ok(None)
}

fn should_replace_frame_texture(current_exact: bool, candidate_exact: bool) -> bool {
    candidate_exact && !current_exact
}

struct FrameTexture {
    texture: Rc<GpuTexture>,
    exact: bool,
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
    cancel: MediaCancelToken,
}

impl PlaybackResolverState {
    pub fn new(
        media: HashMap<String, MediaInfo>,
        text: HashMap<String, TextInfo>,
        timeline_fps: i32,
        render_box: (u32, u32),
        cancel: MediaCancelToken,
    ) -> Self {
        PlaybackResolverState {
            streams: HashMap::new(),
            static_cache: TextureCache::new(STATIC_CACHE_CAP),
            text_rasterizer: CosmicTextRasterizer::new(),
            media,
            text,
            timeline_fps,
            render_box,
            cancel,
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
    frame_tex: HashMap<String, FrameTexture>,
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
    /// 2. Decode each newly-visible clip's exact target synchronously, then
    ///    spawn its forward stream from the following source frame.
    /// 3. Advance every active stream to its target and stash the resulting
    ///    texture in the per-frame lookup.
    pub fn sync_active(&mut self, plan: &FramePlan) -> Result<(), String> {
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
        let mut uploaded: Vec<(String, Rc<GpuTexture>, bool)> = Vec::with_capacity(targets.len());
        for t in &targets {
            let media_path = self
                .state
                .media
                .get(&t.media_ref)
                .map(|info| info.path.clone())
                .ok_or_else(|| format!("playback bootstrap media not found: {}", t.media_ref))?;
            let timeline_fps = self.state.timeline_fps;
            let render_box = self.state.render_box;
            ensure_stream_with(&mut self.state.streams, &t.clip_id, || {
                let request = bootstrap_frame_request(t.source_frame, timeline_fps, render_box);
                let (_, frame) =
                    decode_frame_at_cancellable(&media_path, &request, &self.state.cancel)
                        .map_err(|error| {
                            format!(
                        "playback bootstrap decode failed for {} at source frame {}: {error}",
                        t.media_ref, t.source_frame
                    )
                        })?;
                let decoded = DecodedFrame::new(frame.width, frame.height, frame.rgba, false);
                let texture = Rc::new(upload_rgba(
                    self.device,
                    self.queue,
                    &decoded,
                    false,
                    Some("playback-bootstrap"),
                ));

                let mut req = VideoStreamRequest::new(media_path, timeline_fps);
                req.start_frame = t.source_frame.max(0).saturating_add(1);
                req.timeline_fps = timeline_fps;
                req.max_size = render_box;
                let stream = spawn_video_stream(req).map_err(|error| {
                    format!(
                        "playback bootstrap stream failed for {} at source frame {}: {error}",
                        t.media_ref, t.source_frame
                    )
                })?;
                Ok::<_, String>(ClipStream::new(stream, texture, t.source_frame))
            })?;
            if let Some(cs) = self.state.streams.get_mut(&t.clip_id) {
                cs.advance(t.source_frame, self.device, self.queue)?;
                uploaded.push((
                    format!("v:{}:{}", t.media_ref, t.source_frame),
                    cs.cached_tex.clone(),
                    cs.cached_source_frame == t.source_frame,
                ));
            }
        }

        self.frame_tex.clear();
        for (key, texture, exact) in uploaded {
            use std::collections::hash_map::Entry;
            match self.frame_tex.entry(key) {
                Entry::Vacant(entry) => {
                    entry.insert(FrameTexture { texture, exact });
                }
                Entry::Occupied(mut entry)
                    if should_replace_frame_texture(entry.get().exact, exact) =>
                {
                    entry.insert(FrameTexture { texture, exact });
                }
                Entry::Occupied(_) => {}
            }
        }
        Ok(())
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
        let (_actual, frame) =
            decode_frame_at_cancellable(&info.path, &req, &self.state.cancel).ok()?;
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
            // Video: pre-uploaded by `sync_active`. The compositor does not pass
            // clip id here, so duplicate media/source keys share the best
            // candidate selected above (exact beats stale). A miss returns None
            // and the compositor skips the layer for this frame.
            TextureSource::Decoded { media_ref } => self
                .frame_tex
                .get(&format!("v:{media_ref}:{source_frame}"))
                .map(|candidate| candidate.texture.clone()),
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

    #[test]
    fn bootstrap_request_has_zero_tolerance_at_exact_source_frame() {
        let request = bootstrap_frame_request(17, 25, (640, 360));

        assert_eq!(request.time_secs, 17.0 / 25.0);
        assert_eq!(request.tolerance_secs, 0.0);
        assert_eq!(request.max_size, (640, 360));
        assert!(request.apply_rotation);
    }

    fn vf(source_frame: i64) -> StreamVideoFrame {
        StreamVideoFrame {
            source_frame,
            pts_secs: source_frame as f64 / 30.0,
            frame: RgbaFrame::new(1, 1, vec![0, 0, 0, 255]),
        }
    }

    /// A `pull` closure draining a fixed queue in order.
    fn queue_pull(
        frames: Vec<StreamVideoFrame>,
    ) -> impl FnMut() -> Result<Option<StreamVideoFrame>, ()> {
        let mut it = frames.into_iter();
        move || Ok(it.next())
    }

    #[test]
    fn drain_exact_hit_returns_target_frame() {
        let mut pending = None;
        let got = drain_to_target(&mut pending, queue_pull(vec![vf(5)]), 5).unwrap();
        assert_eq!(got.map(|f| f.source_frame), Some(5));
        assert!(pending.is_none());
    }

    #[test]
    fn drain_discards_frames_behind_target() {
        let mut pending = None;
        // Normal forward advance: 3 and 4 are stale, 5 is the target.
        let got = drain_to_target(&mut pending, queue_pull(vec![vf(3), vf(4), vf(5)]), 5).unwrap();
        assert_eq!(got.map(|f| f.source_frame), Some(5));
        assert!(pending.is_none());
    }

    #[test]
    fn drain_stashes_ahead_frame_and_reuses_cache() {
        let mut pending = None;
        // Only a future frame is available (slow-mo / dup): reuse cache now, keep 7.
        let got = drain_to_target(&mut pending, queue_pull(vec![vf(7)]), 5).unwrap();
        assert!(got.is_none());
        assert_eq!(pending.as_ref().map(|f| f.source_frame), Some(7));
    }

    #[test]
    fn drain_consumes_pending_when_target_catches_up() {
        let mut pending = Some(vf(7));
        // Queue empty; target now equals the stashed frame -> use it.
        let got = drain_to_target(&mut pending, queue_pull(vec![]), 7).unwrap();
        assert_eq!(got.map(|f| f.source_frame), Some(7));
        assert!(pending.is_none());
    }

    #[test]
    fn drain_keeps_pending_while_still_ahead() {
        let mut pending = Some(vf(8));
        let got = drain_to_target(&mut pending, queue_pull(vec![]), 5).unwrap();
        assert!(got.is_none());
        assert_eq!(pending.as_ref().map(|f| f.source_frame), Some(8));
    }

    #[test]
    fn drain_drops_stale_pending_then_pulls_target() {
        let mut pending = Some(vf(2));
        let got = drain_to_target(&mut pending, queue_pull(vec![vf(5)]), 5).unwrap();
        assert_eq!(got.map(|f| f.source_frame), Some(5));
        assert!(pending.is_none());
    }

    #[test]
    fn drain_empty_queue_reuses_cache() {
        let mut pending = None;
        let got = drain_to_target(&mut pending, queue_pull(vec![]), 5).unwrap();
        assert!(got.is_none());
        assert!(pending.is_none());
    }

    #[test]
    fn one_clip_uses_one_decoder_source() {
        let mut streams = HashMap::new();
        let mut decoder_invocations = 0;
        let stream = ensure_stream_with(&mut streams, "clip-1", || {
            decoder_invocations += 1;
            Ok::<_, ()>("decoder-1")
        })
        .expect("create cold decoder");
        assert_eq!(*stream, "decoder-1");

        let same = ensure_stream_with(&mut streams, "clip-1", || {
            decoder_invocations += 1;
            Ok::<_, ()>("duplicate-decoder")
        })
        .expect("reuse decoder");
        assert_eq!(*same, "decoder-1");
        assert_eq!(
            decoder_invocations, 1,
            "one clip must have one cold decoder source"
        );
    }

    #[test]
    fn exact_frame_texture_wins_same_key_collision_in_either_draw_order() {
        assert!(should_replace_frame_texture(false, true));
        assert!(!should_replace_frame_texture(true, false));
        assert!(!should_replace_frame_texture(true, true));
        assert!(!should_replace_frame_texture(false, false));
    }

    #[test]
    fn drain_propagates_stream_failure_instead_of_freezing_cache() {
        let mut pending = None;
        let error = drain_to_target(
            &mut pending,
            || Err::<Option<StreamVideoFrame>, _>("decoder failed"),
            9,
        )
        .expect_err("continuous decoder failure must propagate");

        assert_eq!(error, "decoder failed");
    }

    #[test]
    fn stream_pull_surfaces_worker_error_and_premature_disconnect() {
        let decode_error = classify_stream_pull(
            Ok(Err(MediaError::Decode("broken stream".to_string()))),
            4,
            5,
        )
        .expect_err("worker error must propagate");
        assert!(decode_error.contains("broken stream"));

        let disconnect_error = classify_stream_pull(Err(TryRecvError::Disconnected), 4, 5)
            .expect_err("disconnect before the target must propagate");
        assert!(disconnect_error.contains("ended at source frame 4"));

        assert!(classify_stream_pull(Err(TryRecvError::Disconnected), 5, 5)
            .expect("normal EOF may retain the exact final frame")
            .is_none());
        assert!(classify_stream_pull(Err(TryRecvError::Empty), 4, 5)
            .expect("an empty live queue may temporarily reuse cache")
            .is_none());
    }
}
