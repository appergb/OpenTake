/**
 * Clip-derived UI helpers (pure). Track color, display name, link flag.
 * See SPEC §5.4 (label = name + double-space + duration, underline when linked)
 * and §1.5 (track colors by ClipType).
 */

import { TRACK_COLOR } from "./theme";
import { formatClipDuration } from "./geometry";
import type { Clip, ClipType, TrimEditReq } from "./types";

export function trackColor(type: ClipType): string {
  return TRACK_COLOR[type] ?? TRACK_COLOR.video;
}

/** First non-empty line of textContent, else a friendly fallback from mediaRef. */
export function clipDisplayName(clip: Clip): string {
  if (clip.textContent) {
    const firstLine = clip.textContent.split("\n").find((l) => l.trim().length > 0);
    if (firstLine) return firstLine.trim();
  }
  if (clip.mediaRef) return clip.mediaRef;
  return clip.mediaType;
}

/** Clip label-bar text: "<name>  <duration timecode>" (ClipRenderer:598-609). */
export function clipLabel(clip: Clip, fps: number): string {
  return `${clipDisplayName(clip)}  ${formatClipDuration(clip.durationFrames, fps)}`;
}

export function isLinked(clip: Clip): boolean {
  return clip.linkGroupId != null;
}

/** Which edge a trim drag grabs. */
export type TrimEdge = "left" | "right";

type TrimClip = Pick<Clip, "durationFrames" | "speed" | "trimStartFrame" | "trimEndFrame" | "mediaType">;

function isUnbounded(clip: TrimClip): boolean {
  return clip.mediaType === "image" || clip.mediaType === "text";
}

/**
 * Clamp a trim-edge drag (`delta` in TIMELINE frames) so the clip keeps a ≥1
 * frame duration and — for bounded media (video/audio) — can't extend past the
 * available leading/trailing source. Mirrors upstream's `mouseDragged` trim
 * clamp; the unbounded source clamp for image/text is left to `trimSourceValues`.
 */
export function clampTrimDeltaFrames(clip: TrimClip, edge: TrimEdge, delta: number): number {
  const speed = clip.speed > 0 ? clip.speed : 1;
  if (edge === "left") {
    // Positive delta shrinks duration (left edge moves right): keep ≥1 frame.
    let d = Math.min(delta, clip.durationFrames - 1);
    if (!isUnbounded(clip)) {
      // Negative delta extends into leading source; bounded by what's trimmed.
      d = Math.max(d, -Math.floor(clip.trimStartFrame / speed));
    }
    return d;
  }
  // Right: negative delta shrinks duration (right edge moves left): keep ≥1 frame.
  let d = Math.max(delta, -(clip.durationFrames - 1));
  if (!isUnbounded(clip)) {
    d = Math.min(d, Math.floor(clip.trimEndFrame / speed));
  }
  return d;
}

/**
 * New SOURCE-frame `(trimStartFrame, trimEndFrame)` for an edge drag of `delta`
 * TIMELINE frames. 1:1 with opentake-ops `trim_values`: source delta =
 * round(delta * speed); video/audio clamp the moved edge at 0, image/text are
 * unbounded.
 */
export function trimSourceValues(
  clip: TrimClip,
  edge: TrimEdge,
  delta: number,
): { trimStartFrame: number; trimEndFrame: number } {
  const speed = clip.speed > 0 ? clip.speed : 1;
  const sourceDelta = Math.round(delta * speed);
  if (edge === "left") {
    const ns = clip.trimStartFrame + sourceDelta;
    return {
      trimStartFrame: isUnbounded(clip) ? ns : Math.max(0, ns),
      trimEndFrame: clip.trimEndFrame,
    };
  }
  const ne = clip.trimEndFrame - sourceDelta;
  return {
    trimStartFrame: clip.trimStartFrame,
    trimEndFrame: isUnbounded(clip) ? ne : Math.max(0, ne),
  };
}

/**
 * Trim-edit reqs that move each clip's IN (`edge:"left"`) or OUT (`edge:"right"`)
 * point to `frame` — 剪映's Q / W ("删除播放头左/右"). Only clips the playhead is
 * strictly inside are affected (a clip whose edge already sits at the playhead,
 * or that the playhead misses, is skipped). The delta is the same TIMELINE-frame
 * edge move the trim drag computes, so the source conversion + clamps match it.
 */
export function trimToPlayheadEdits(clips: Clip[], frame: number, edge: TrimEdge): TrimEditReq[] {
  const edits: TrimEditReq[] = [];
  for (const c of clips) {
    const end = c.startFrame + c.durationFrames;
    if (frame <= c.startFrame || frame >= end) continue; // playhead must be strictly inside
    const rawDelta = edge === "left" ? frame - c.startFrame : frame - end;
    const delta = clampTrimDeltaFrames(c, edge, rawDelta);
    if (delta === 0) continue;
    const { trimStartFrame, trimEndFrame } = trimSourceValues(c, edge, delta);
    edits.push({ clipId: c.id, trimStartFrame, trimEndFrame });
  }
  return edits;
}
