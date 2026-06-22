//! `build_render_plan` (SPEC §2.3) + `RenderPlan::frame` (SPEC §2.4) +
//! `source_frame_index` (SPEC §2.5).
//!
//! This is the port of upstream `CompositionBuilder.buildVisuals` — but it
//! emits per-frame property VALUES, not AVFoundation ramp instructions. Every
//! keyframe / fade / dB sample goes through the domain `*_at` methods (SPEC §0
//! iron rule); this module only adds geometry projection + frame scheduling.

use opentake_domain::{Clip, ClipType, Timeline};

use super::affine::{affine_transform, compose, crop_to_uv};
use super::types::{ClipPlan, FramePlan, LayerDraw, RenderPlan, RenderSize, TextureSource};
use crate::source::SourceMetrics;

/// Half-away-from-zero round, matching the domain convention (`clip.rs` L7).
#[inline]
fn round_haz(v: f64) -> i64 {
    v.round() as i64
}

/// Absolute value of the bounding box of the rect `(0, 0, w, h)` transformed by
/// `pt`, plus the translation that re-origins that box to (0,0). Port of upstream
/// CompositionBuilder L170-172:
///
/// ```text
/// box = CGRect(origin: .zero, size: natSize).applying(pt)
/// natSize = (|box.width|, |box.height|)
/// preferredTransform = pt.concatenating(translate(-box.minX, -box.minY))
/// ```
fn normalize_box(nat0: (f64, f64), pt: [f64; 6]) -> ((f64, f64), [f64; 6]) {
    let corners = [(0.0, 0.0), (nat0.0, 0.0), (0.0, nat0.1), (nat0.0, nat0.1)];
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for (x, y) in corners {
        let tx = x * pt[0] + y * pt[2] + pt[4];
        let ty = x * pt[1] + y * pt[3] + pt[5];
        min_x = min_x.min(tx);
        min_y = min_y.min(ty);
        max_x = max_x.max(tx);
        max_y = max_y.max(ty);
    }
    let nat = ((max_x - min_x).abs(), (max_y - min_y).abs());
    let reorigined = compose(pt, [1.0, 0.0, 0.0, 1.0, -min_x, -min_y]);
    (nat, reorigined)
}

/// Pick the [`TextureSource`] for a clip's media type.
fn texture_source_for(clip: &Clip) -> TextureSource {
    match clip.media_type {
        ClipType::Image => TextureSource::Image {
            media_ref: clip.media_ref.clone(),
        },
        ClipType::Lottie => TextureSource::Lottie {
            media_ref: clip.media_ref.clone(),
        },
        ClipType::Text => TextureSource::Text {
            clip_id: clip.id.clone(),
        },
        // Video (and any audio that slipped through — guarded by the caller).
        ClipType::Video | ClipType::Audio => TextureSource::Decoded {
            media_ref: clip.media_ref.clone(),
        },
    }
}

/// Build a [`ClipPlan`] for one selected clip.
fn make_clip_plan(
    clip: &Clip,
    track_index: usize,
    clip_index: usize,
    sources: &dyn SourceMetrics,
    render_size: RenderSize,
) -> ClipPlan {
    let is_text = clip.media_type == ClipType::Text;

    // natSize / preferredTransform. Text uses its layout box (preferred =
    // identity); other sources use the metrics + box normalization (L166-172).
    let (nat_size, preferred_transform) = if is_text {
        // Text rasterizes to its BOX (not the whole canvas): the texture covers
        // `clip.transform`'s box in pixels. Setting nat_size to that box size
        // makes `affine_transform(clip.transform, nat=box, render)` collapse to
        // sx=sy=1 placed at the box top-left — the box texture maps 1:1 and the
        // existing affine carries position / rotation / flip / opacity, exactly
        // like video/image layers. (A full-canvas text clip — the add_texts
        // default transform — yields the render size, unchanged from before.)
        let bw = (clip.transform.width * render_size.width_f()).max(1.0);
        let bh = (clip.transform.height * render_size.height_f()).max(1.0);
        ((bw, bh), [1.0, 0.0, 0.0, 1.0, 0.0, 0.0])
    } else {
        let nat0 = sources
            .natural_size(&clip.media_ref)
            .map(|(w, h)| (w as f64, h as f64))
            .filter(|&(w, h)| w > 0.0 && h > 0.0)
            .unwrap_or((render_size.width_f(), render_size.height_f()));
        let pt = sources.preferred_transform(&clip.media_ref);
        normalize_box(nat0, pt)
    };

    let needs_premultiply = match clip.media_type {
        ClipType::Video => sources.needs_premultiply(&clip.media_ref),
        // Image / Text / Lottie are authored premultiplied.
        _ => false,
    };

    let lottie_frame_count = if clip.media_type == ClipType::Lottie {
        sources.lottie_frame_count(&clip.media_ref)
    } else {
        None
    };

    ClipPlan {
        clip_id: clip.id.clone(),
        track_index,
        clip_index,
        source: texture_source_for(clip),
        start_frame: clip.start_frame,
        end_frame: clip.end_frame(),
        nat_size,
        preferred_transform,
        needs_premultiply,
        speed: clip.speed,
        trim_start_frame: clip.trim_start_frame,
        media_type: clip.media_type,
        lottie_frame_count,
        // Advanced pixel-effect inputs, copied verbatim from the clip (frame-
        // independent this round). Drop a color grade that is the identity so the
        // compositor can skip it cheaply.
        color_grade: clip.color_grade.filter(|g| !g.is_identity()),
        chroma_key: clip.chroma_key,
        masks: clip.masks.clone(),
        effects: clip.effects.clone(),
    }
}

/// Parse a [`Timeline`] into a static [`RenderPlan`] (SPEC §2.3).
///
/// Mirrors `CompositionBuilder.build` L53-216 + the visible-clip selection in
/// `buildVisuals` L405-445:
/// - skip hidden tracks,
/// - per video track, sort clips by `start_frame` and drop overlaps
///   (`duration > 0 && start >= prev_end`, L152/L424),
/// - text clips bypass the per-track skip and collect into `text_plans`
///   (upstream L57/L422 + the CoreAnimationTool overlay),
/// - audio tracks contribute NO video clip plan (audio mixing lives elsewhere,
///   SPEC §3.8).
pub fn build_render_plan(
    timeline: &Timeline,
    render_size: RenderSize,
    sources: &dyn SourceMetrics,
) -> RenderPlan {
    let total_frames = timeline.total_frames();
    let mut clip_plans: Vec<ClipPlan> = Vec::new();
    let mut text_plans: Vec<ClipPlan> = Vec::new();

    for (track_index, track) in timeline.tracks.iter().enumerate() {
        if track.hidden {
            continue;
        }
        let is_audio = track.kind == ClipType::Audio;

        // Sort clip *indices* by start frame so we keep the original
        // `clip_index` for `frame()` lookups while applying the upstream order.
        let mut order: Vec<usize> = (0..track.clips.len()).collect();
        order.sort_by_key(|&i| track.clips[i].start_frame);

        let mut prev_end_frame = i32::MIN;
        for &clip_index in &order {
            let clip = &track.clips[clip_index];

            if clip.media_type == ClipType::Text {
                // Text: no overlap skip, no audio gate; each text clip stands
                // alone (SPEC §4.2). Defensive: require a positive span.
                if clip.duration_frames > 0 {
                    text_plans.push(make_clip_plan(
                        clip,
                        track_index,
                        clip_index,
                        sources,
                        render_size,
                    ));
                }
                continue;
            }

            // Audio tracks: no video texture (SPEC §3.8).
            if is_audio {
                continue;
            }

            // Video-track de-dup (upstream L152 / L424).
            if clip.duration_frames <= 0 || clip.start_frame < prev_end_frame {
                continue;
            }
            clip_plans.push(make_clip_plan(
                clip,
                track_index,
                clip_index,
                sources,
                render_size,
            ));
            prev_end_frame = clip.end_frame();
        }
    }

    // Final blend order: (track_index, start_frame). Within a track there are no
    // overlaps, so this fully determines the alpha-over stacking (SPEC §1.5).
    clip_plans.sort_by(|a, b| {
        a.track_index
            .cmp(&b.track_index)
            .then(a.start_frame.cmp(&b.start_frame))
    });

    RenderPlan {
        fps: timeline.fps,
        render_size,
        total_frames,
        clip_plans,
        text_plans,
    }
}

/// The source frame a clip references at timeline frame `f` (SPEC §2.5).
/// `f` is assumed inside `[start_frame, end_frame)`.
///
/// Port of upstream `insertClip` trim+speed handling (L301-343): the source
/// cursor advances by `rel * speed`; the image trim floor is `max(0, trim)`.
pub fn source_frame_index(plan: &ClipPlan, f: i32) -> i64 {
    let rel = (f - plan.start_frame) as f64;
    let trim = if plan.media_type == ClipType::Image {
        plan.trim_start_frame.max(0) as i64
    } else {
        plan.trim_start_frame as i64
    };

    match (&plan.source, plan.media_type) {
        // Image / Text: single static texture.
        (TextureSource::Image { .. }, _) | (TextureSource::Text { .. }, _) => 0,
        (_, ClipType::Image) | (_, ClipType::Text) => 0,
        (TextureSource::Lottie { .. }, _) => {
            let raw = trim + round_haz(rel * plan.speed);
            match plan.lottie_frame_count {
                Some(n) if n > 0 => raw.rem_euclid(n),
                // Unknown frame count: clamp at 0 lower bound, no wrap.
                _ => raw.max(0),
            }
        }
        // Decoded video/audio: source frame number; the decoder maps it to PTS.
        _ => trim + round_haz(rel * plan.speed),
    }
}

/// Evaluate a single layer's [`LayerDraw`] at frame `f`, or `None` when the clip
/// is outside its span or fully transparent. `render_size` is the canvas size
/// (passed in from [`RenderPlan::frame`]).
fn eval_layer<'a>(
    plan: &'a ClipPlan,
    clip: &Clip,
    f: i32,
    render_size: RenderSize,
) -> Option<LayerDraw<'a>> {
    // Hit test: outside [start, end) contributes nothing (opacity 0 upstream,
    // L407/L431).
    if f < plan.start_frame || f >= plan.end_frame {
        return None;
    }
    let opacity = clip.opacity_at(f);
    if opacity <= 0.0 {
        return None; // behavior-equivalent skip (SPEC §2.4 step 3).
    }
    // Upstream `emitTransform` (CompositionBuilder L631-632) branches: the STATIC
    // path uses `clip.transform` (which carries flip flags), while the ANIMATED
    // path uses `clip.transformAt(frame)` (which rebuilds top-left/size/rotation
    // and intentionally drops flip — matching domain `transform_at`). Replicate
    // that split so flip behaves exactly as upstream.
    let transform = if clip.has_transform_animation() {
        clip.transform_at(f)
    } else {
        clip.transform
    };
    let affine = compose(
        plan.preferred_transform,
        affine_transform(&transform, plan.nat_size, render_size),
    );
    let crop_uv = crop_to_uv(clip.crop_at(f));
    let source_frame = source_frame_index(plan, f);

    Some(LayerDraw {
        source: &plan.source,
        source_frame,
        affine,
        crop_uv,
        opacity,
        needs_premultiply: plan.needs_premultiply,
        clip_id: &plan.clip_id,
        color_grade: plan.color_grade.as_ref(),
        chroma_key: plan.chroma_key.as_ref(),
        masks: &plan.masks,
        effects: &plan.effects,
    })
}

impl RenderPlan {
    /// Evaluate the ordered draw list for frame `f` (SPEC §2.4).
    ///
    /// `timeline` must be the same one the plan was built from (they share clip
    /// indices). Video clips composite first; text clips composite last (on
    /// top), matching upstream's text-over-video layering (SPEC §4.2).
    pub fn frame<'a>(&'a self, timeline: &'a Timeline, f: i32) -> FramePlan<'a> {
        let mut draws: Vec<LayerDraw<'a>> = Vec::new();

        for plan in &self.clip_plans {
            let Some(clip) = clip_for(timeline, plan) else {
                continue;
            };
            if let Some(d) = eval_layer(plan, clip, f, self.render_size) {
                draws.push(d);
            }
        }
        for plan in &self.text_plans {
            let Some(clip) = clip_for(timeline, plan) else {
                continue;
            };
            if let Some(d) = eval_layer(plan, clip, f, self.render_size) {
                draws.push(d);
            }
        }

        FramePlan {
            clear_rgba: [0.0, 0.0, 0.0, 1.0],
            draws,
        }
    }
}

/// Resolve the `&Clip` for a plan via its stored indices, falling back to an id
/// search if the indices no longer line up (defensive; the indexed path is the
/// fast one per SPEC §2.4).
fn clip_for<'a>(timeline: &'a Timeline, plan: &ClipPlan) -> Option<&'a Clip> {
    if let Some(track) = timeline.tracks.get(plan.track_index) {
        if let Some(clip) = track.clips.get(plan.clip_index) {
            if clip.id == plan.clip_id {
                return Some(clip);
            }
        }
        // Indices drifted: fall back to id lookup within the track.
        return track.clips.iter().find(|c| c.id == plan.clip_id);
    }
    None
}
