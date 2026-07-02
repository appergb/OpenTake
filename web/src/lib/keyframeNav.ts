/**
 * Pure keyframe-navigation helpers for the per-row Inspector keyframe cluster
 * (the diamond stamp toggle + prev/next chevrons — upstream
 * `InspectorView.keyframeControls`, Inspector/InspectorView.swift:511-563).
 *
 * All frames here are ABSOLUTE timeline frames, matching the playhead
 * (`activeFrame`) and the values the diamond/chevrons act on. Keyframe tracks
 * store CLIP-RELATIVE offsets (`Keyframe.frame`), so every read converts via
 * `kf.frame + clip.startFrame` — the same absolute mapping upstream's
 * `Clip.keyframeFrames(for:)` does with `toAbs` (Models/Keyframe.swift:100-116)
 * and `KeyframesLaneRow` already does inline.
 *
 * 1:1 with upstream `EditorViewModel+Keyframes.swift:7-21`:
 *   keyframeFrames        -> clip offsets mapped to absolute
 *   hasKeyframe(at:)      -> frames.contains(frame)
 *   previousKeyframeFrame -> frames.filter { $0 < frame }.max()
 *   nextKeyframeFrame     -> frames.filter { $0 > frame }.min()
 * and `Clip.contains(timelineFrame:)` (Models/Keyframe.swift:95-97):
 *   clipContainsFrame     -> frame >= startFrame && frame < endFrame
 */

import type { Clip, KeyframeProperty } from "./types";

/** Resolve the clip's keyframe track for a property, or undefined when the clip
 *  has no track for it. Mirrors `KeyframesLaneRow.getTrack`. */
function trackFrames(clip: Clip, property: KeyframeProperty): number[] {
  const track = (() => {
    switch (property) {
      case "opacity":
        return clip.opacityTrack;
      case "volume":
        return clip.volumeTrack;
      case "rotation":
        return clip.rotationTrack;
      case "position":
        return clip.positionTrack;
      case "scale":
        return clip.scaleTrack;
      case "crop":
        return clip.cropTrack;
    }
  })();
  return track ? track.keyframes.map((kf) => kf.frame + clip.startFrame) : [];
}

/** Absolute timeline frames of every keyframe on `property` (empty when none). */
export function keyframeFrames(clip: Clip, property: KeyframeProperty): number[] {
  return trackFrames(clip, property);
}

/** Whether a keyframe exists exactly AT the (absolute) playhead frame. Drives
 *  the diamond fill (filled = a keyframe is at the playhead). */
export function hasKeyframeAt(clip: Clip, property: KeyframeProperty, frame: number): boolean {
  return trackFrames(clip, property).includes(frame);
}

/** Nearest keyframe strictly BEFORE `frame`, or null when none — the target the
 *  left chevron jumps the playhead to (chevron disabled when null). */
export function previousKeyframeFrame(
  clip: Clip,
  property: KeyframeProperty,
  frame: number,
): number | null {
  const before = trackFrames(clip, property).filter((f) => f < frame);
  return before.length > 0 ? Math.max(...before) : null;
}

/** Nearest keyframe strictly AFTER `frame`, or null when none — the target the
 *  right chevron jumps to (chevron disabled when null). */
export function nextKeyframeFrame(
  clip: Clip,
  property: KeyframeProperty,
  frame: number,
): number | null {
  const after = trackFrames(clip, property).filter((f) => f > frame);
  return after.length > 0 ? Math.min(...after) : null;
}

/** Whether the (absolute) playhead frame is inside the clip's half-open span
 *  `[startFrame, endFrame)`. The stamp toggle is disabled when this is false
 *  (upstream `keyframeControls`'s `inRange` gate). */
export function clipContainsFrame(clip: Clip, frame: number): boolean {
  return frame >= clip.startFrame && frame < clip.startFrame + clip.durationFrames;
}
