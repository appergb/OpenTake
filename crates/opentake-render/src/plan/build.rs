//! `build_render_plan` (SPEC §2.3) + `RenderPlan::frame` (SPEC §2.4) +
//! `source_frame_index` (SPEC §2.5).
//!
//! This is the port of upstream `CompositionBuilder.buildVisuals` — but it
//! emits per-frame property VALUES, not AVFoundation ramp instructions. Every
//! keyframe / fade / dB sample goes through the domain `*_at` methods (SPEC §0
//! iron rule); this module only adds geometry projection + frame scheduling.

use std::collections::{HashMap, HashSet};

use opentake_domain::{Clip, ClipType, NestedSequence, Timeline, TransitionKind};

use super::affine::{affine_transform, compose, crop_to_uv};
use super::types::{
    AudioClipPlan, ClipPlan, CompoundAncestor, FramePlan, LayerDraw, RenderPlan, RenderSize,
    TextureSource,
};
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
#[allow(clippy::too_many_arguments)]
fn make_clip_plan(
    clip: &Clip,
    track_index: usize,
    clip_index: usize,
    blend_path: Vec<usize>,
    compound_ancestors: Vec<CompoundAncestor>,
    visible_start: i32,
    visible_end: i32,
    effective_trim_start: i32,
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
        clip: clip.clone(),
        clip_id: clip.id.clone(),
        track_index,
        clip_index,
        blend_path,
        compound_ancestors,
        source: texture_source_for(clip),
        start_frame: visible_start,
        end_frame: visible_end,
        nat_size,
        preferred_transform,
        needs_premultiply,
        speed: clip.speed,
        reversed: clip.reversed,
        trim_start_frame: effective_trim_start,
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
    try_build_render_plan(timeline, render_size, sources)
        .expect("timeline graph must be valid before building a render plan")
}

/// Fail-closed render-plan construction used by preview, export, and agents.
pub fn try_build_render_plan(
    timeline: &Timeline,
    render_size: RenderSize,
    sources: &dyn SourceMetrics,
) -> Result<RenderPlan, String> {
    timeline.validate_nested_sequences()?;
    validate_nested_render_constraints(timeline)?;
    let total_frames = timeline.total_frames();
    let mut clip_plans: Vec<ClipPlan> = Vec::new();
    let mut text_plans: Vec<ClipPlan> = Vec::new();
    let mut audio_clips: Vec<AudioClipPlan> = Vec::new();

    let registry: HashMap<&str, &NestedSequence> = timeline
        .nested_sequences
        .iter()
        .map(|sequence| (sequence.id.as_str(), sequence))
        .collect();
    collect_timeline_plans(
        timeline,
        &registry,
        0,
        None,
        None,
        &[],
        &[],
        render_size,
        sources,
        &mut clip_plans,
        &mut text_plans,
    );
    collect_nested_audio(
        timeline,
        &registry,
        0,
        None,
        None,
        false,
        &[],
        &mut audio_clips,
    );

    // Final blend order: bottom-to-top. Upstream keeps visual track 0 topmost,
    // so higher track indexes draw first and lower indexes draw last.
    clip_plans.sort_by(|a, b| {
        b.blend_path
            .cmp(&a.blend_path)
            .then(a.start_frame.cmp(&b.start_frame))
    });

    Ok(RenderPlan {
        fps: timeline.fps,
        render_size,
        total_frames,
        clip_plans,
        text_plans,
        audio_clips,
    })
}

#[allow(clippy::too_many_arguments)]
fn collect_timeline_plans(
    timeline: &Timeline,
    registry: &HashMap<&str, &NestedSequence>,
    frame_offset: i32,
    parent_start: Option<i32>,
    parent_end: Option<i32>,
    parent_blend_path: &[usize],
    compound_ancestors: &[CompoundAncestor],
    render_size: RenderSize,
    sources: &dyn SourceMetrics,
    clip_plans: &mut Vec<ClipPlan>,
    text_plans: &mut Vec<ClipPlan>,
) {
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
        let mut blend_path = parent_blend_path.to_vec();
        blend_path.push(track_index);
        for &clip_index in &order {
            let clip = &track.clips[clip_index];

            let unclamped_start = frame_offset.saturating_add(clip.start_frame);
            let absolute_start = unclamped_start.max(parent_start.unwrap_or(i32::MIN));
            let absolute_end = frame_offset
                .saturating_add(clip.end_frame())
                .min(parent_end.unwrap_or(i32::MAX));
            if absolute_end <= absolute_start {
                continue;
            }

            let clipped_left = absolute_start - unclamped_start;
            let effective_trim_start = clip
                .trim_start_frame
                .saturating_add((clipped_left as f64 * clip.speed).round() as i32);
            let mut mapped_clip = clip.clone();
            mapped_clip.start_frame = unclamped_start;

            // Apply the same overlap policy to compound and ordinary visual
            // clips before recursively expanding the selected compound.
            if clip.media_type != ClipType::Text {
                if clip.duration_frames <= 0 || clip.start_frame < prev_end_frame {
                    continue;
                }
                prev_end_frame = clip.end_frame();
            }

            if let Some(sequence_id) = clip.nested_sequence_id.as_deref() {
                let sequence = registry
                    .get(sequence_id)
                    .expect("validated nested reference must exist");
                let mut child_ancestors = compound_ancestors.to_vec();
                child_ancestors.push(CompoundAncestor {
                    clip: mapped_clip,
                    // Flattened leaves are already projected into the output
                    // canvas' normalized coordinate space. Compound transforms
                    // therefore operate on that output canvas as their source;
                    // using the authored child pixel size here would scale the
                    // same normalization a second time.
                    canvas_size: (render_size.width_f(), render_size.height_f()),
                });
                collect_timeline_plans(
                    &sequence.timeline,
                    registry,
                    absolute_start.saturating_sub(effective_trim_start),
                    Some(absolute_start),
                    Some(absolute_end),
                    &blend_path,
                    &child_ancestors,
                    render_size,
                    sources,
                    clip_plans,
                    text_plans,
                );
                continue;
            }

            if clip.media_type == ClipType::Text {
                // Text: no overlap skip, no audio gate; each text clip stands
                // alone (SPEC §4.2). Defensive: require a positive span.
                if clip.duration_frames > 0 {
                    text_plans.push(make_clip_plan(
                        &mapped_clip,
                        track_index,
                        clip_index,
                        blend_path.clone(),
                        compound_ancestors.to_vec(),
                        absolute_start,
                        absolute_end,
                        effective_trim_start,
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
            clip_plans.push(make_clip_plan(
                &mapped_clip,
                track_index,
                clip_index,
                blend_path.clone(),
                compound_ancestors.to_vec(),
                absolute_start,
                absolute_end,
                effective_trim_start,
                sources,
                render_size,
            ));
        }
    }
}

fn validate_nested_render_constraints(timeline: &Timeline) -> Result<(), String> {
    let mut timelines = Vec::with_capacity(timeline.nested_sequences.len() + 1);
    timelines.push(timeline);
    timelines.extend(
        timeline
            .nested_sequences
            .iter()
            .map(|sequence| &sequence.timeline),
    );
    let compound_ids = timelines
        .iter()
        .flat_map(|candidate| candidate.tracks.iter())
        .flat_map(|track| &track.clips)
        .filter(|clip| clip.nested_sequence_id.is_some())
        .map(|clip| clip.id.as_str())
        .collect::<HashSet<_>>();
    for candidate in timelines {
        for clip in candidate.tracks.iter().flat_map(|track| &track.clips) {
            if clip
                .transition_out
                .as_ref()
                .is_some_and(|transition| compound_ids.contains(transition.to_clip_id.as_str()))
            {
                return Err(format!(
                    "transition into compound clip {} requires offscreen nesting",
                    clip.transition_out
                        .as_ref()
                        .expect("transition was matched")
                        .to_clip_id
                ));
            }
        }
        for clip in candidate
            .tracks
            .iter()
            .flat_map(|track| track.clips.iter())
            .filter(|clip| clip.nested_sequence_id.is_some())
        {
            if (clip.speed - 1.0).abs() > f64::EPSILON || clip.reversed {
                return Err(format!(
                    "compound clip {} must use forward 1x playback",
                    clip.id
                ));
            }
            if clip.crop != Default::default()
                || clip.crop_track.is_some()
                || clip.color_grade.is_some()
                || clip.chroma_key.is_some()
                || !clip.masks.is_empty()
                || !clip.effects.is_empty()
                || clip.transition_out.is_some()
            {
                return Err(format!(
                    "compound clip {} uses effects that require offscreen nesting",
                    clip.id
                ));
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_nested_audio(
    timeline: &Timeline,
    registry: &HashMap<&str, &NestedSequence>,
    frame_offset: i32,
    parent_start: Option<i32>,
    parent_end: Option<i32>,
    parent_muted: bool,
    compound_ancestors: &[Clip],
    audio_clips: &mut Vec<AudioClipPlan>,
) {
    for track in &timeline.tracks {
        let muted = parent_muted || track.muted;
        for clip in &track.clips {
            let unclamped_start = frame_offset.saturating_add(clip.start_frame);
            let absolute_start = unclamped_start.max(parent_start.unwrap_or(i32::MIN));
            let absolute_end = frame_offset
                .saturating_add(clip.end_frame())
                .min(parent_end.unwrap_or(i32::MAX));
            if absolute_end <= absolute_start {
                continue;
            }
            let clipped_left = absolute_start - unclamped_start;
            let effective_trim_start = clip
                .trim_start_frame
                .saturating_add((clipped_left as f64 * clip.speed).round() as i32);
            let mut mapped_gain_clip = clip.clone();
            mapped_gain_clip.start_frame = unclamped_start;
            if let Some(sequence_id) = clip.nested_sequence_id.as_deref() {
                let sequence = registry
                    .get(sequence_id)
                    .expect("validated nested reference must exist");
                let mut child_ancestors = compound_ancestors.to_vec();
                child_ancestors.push(mapped_gain_clip);
                collect_nested_audio(
                    &sequence.timeline,
                    registry,
                    absolute_start.saturating_sub(effective_trim_start),
                    Some(absolute_start),
                    Some(absolute_end),
                    muted,
                    &child_ancestors,
                    audio_clips,
                );
                continue;
            }
            if muted || !matches!(clip.media_type, ClipType::Audio | ClipType::Video) {
                continue;
            }
            let mut flattened = clip.clone();
            flattened.start_frame = absolute_start;
            flattened.duration_frames = absolute_end - absolute_start;
            flattened.trim_start_frame = effective_trim_start;
            audio_clips.push(AudioClipPlan {
                clip: flattened,
                gain_clip: mapped_gain_clip,
                compound_ancestors: compound_ancestors.to_vec(),
            });
        }
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
    let duration_frames = (plan.end_frame - plan.start_frame).max(1) as f64;
    let source_frames_consumed = round_haz(duration_frames * plan.speed).max(1);
    let last = trim + source_frames_consumed - 1;

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
        _ => {
            let offset = round_haz(rel * plan.speed);
            if plan.reversed {
                (last - offset).clamp(trim, last)
            } else {
                (trim + offset).clamp(trim, last)
            }
        }
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
    let mut opacity = clip.opacity_at(f);
    for ancestor in plan.compound_ancestors.iter().rev() {
        opacity *= ancestor.clip.opacity_at(f);
    }
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
    let mut affine = compose(
        plan.preferred_transform,
        affine_transform(&transform, plan.nat_size, render_size),
    );
    for ancestor in plan.compound_ancestors.iter().rev() {
        let transform = if ancestor.clip.has_transform_animation() {
            ancestor.clip.transform_at(f)
        } else {
            ancestor.clip.transform
        };
        affine = compose(
            affine,
            affine_transform(&transform, ancestor.canvas_size, render_size),
        );
    }
    let crop_uv = crop_to_uv(clip.crop_at(f));
    let source_frame = source_frame_index(plan, f);

    Some(LayerDraw {
        source: &plan.source,
        source_frame,
        affine,
        // Carry the SAME natural size the affine was built with (above) so the
        // shader's quad lands in the right place regardless of the (possibly
        // downscaled) decoded texture resolution (#125).
        nat_size: plan.nat_size,
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

/// Evaluate the incoming side of a cross dissolve before its nominal timeline
/// start. The first source frame is held during the dissolve, then regular
/// playback begins at the cut; this avoids reading outside the clip's source
/// window while preserving timeline duration and adjacency.
fn eval_transition_incoming<'a>(
    plan: &'a ClipPlan,
    clip: &Clip,
    progress: f64,
    render_size: RenderSize,
) -> Option<LayerDraw<'a>> {
    let sample_frame = plan.start_frame;
    let mut opacity = clip.raw_opacity_at(sample_frame) * progress.clamp(0.0, 1.0);
    for ancestor in plan.compound_ancestors.iter().rev() {
        opacity *= ancestor.clip.opacity_at(sample_frame);
    }
    if opacity <= 0.0 {
        return None;
    }
    let transform = if clip.has_transform_animation() {
        clip.transform_at(sample_frame)
    } else {
        clip.transform
    };
    let mut affine = compose(
        plan.preferred_transform,
        affine_transform(&transform, plan.nat_size, render_size),
    );
    for ancestor in plan.compound_ancestors.iter().rev() {
        let transform = if ancestor.clip.has_transform_animation() {
            ancestor.clip.transform_at(sample_frame)
        } else {
            ancestor.clip.transform
        };
        affine = compose(
            affine,
            affine_transform(&transform, ancestor.canvas_size, render_size),
        );
    }
    Some(LayerDraw {
        source: &plan.source,
        source_frame: source_frame_index(plan, sample_frame),
        affine,
        nat_size: plan.nat_size,
        crop_uv: crop_to_uv(clip.crop_at(sample_frame)),
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
    pub fn frame<'a>(&'a self, _timeline: &'a Timeline, f: i32) -> FramePlan<'a> {
        let mut draws: Vec<LayerDraw<'a>> = Vec::new();

        for (index, plan) in self.clip_plans.iter().enumerate() {
            let clip = &plan.clip;
            let transition = clip.transition_out.as_ref().and_then(|transition| {
                let incoming_plan = self.clip_plans.get(index + 1)?;
                if transition.kind != TransitionKind::CrossDissolve
                    || incoming_plan.track_index != plan.track_index
                    || incoming_plan.clip_id != transition.to_clip_id
                    || incoming_plan.start_frame != plan.end_frame
                {
                    return None;
                }
                let incoming = &incoming_plan.clip;
                let duration = transition
                    .duration_frames
                    .max(1)
                    .min(clip.duration_frames.max(1))
                    .min(incoming.duration_frames.max(1));
                let start = plan.end_frame - duration;
                if f < start || f >= plan.end_frame {
                    return None;
                }
                let progress = (f - start) as f64 / duration as f64;
                Some((incoming_plan, incoming, progress))
            });

            if let Some(mut d) = eval_layer(plan, clip, f, self.render_size) {
                if let Some((_, _, progress)) = transition {
                    d.opacity *= 1.0 - progress;
                }
                if d.opacity > 0.0 {
                    draws.push(d);
                }
            }
            if let Some((incoming_plan, incoming, progress)) = transition {
                if let Some(d) =
                    eval_transition_incoming(incoming_plan, incoming, progress, self.render_size)
                {
                    draws.push(d);
                }
            }
        }
        for plan in &self.text_plans {
            let clip = &plan.clip;
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
