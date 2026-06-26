/**
 * Pure helpers for real-time timeline playback (#53). These resolve, for a given
 * playhead frame, which media should be playing and where each element's
 * `currentTime` must sit — the data the `<TimelinePlayback>` component feeds to
 * `<video>`/`<audio>` elements.
 *
 * Faithful to upstream's model: one composition clock plays the timeline
 * (VideoEngine.swift). In the WebView the DOM media elements are followers of
 * that clock, not authorities over it; pausing keeps them frozen on the current
 * frame instead of switching to a separate ffmpeg/PNG render path.
 */

import type { Clip, Timeline, Track } from "../../lib/types";

/** A clip selected for playback at a frame, with its track context. */
export interface ActiveMedia {
  clip: Clip;
  track: Track;
  trackIndex: number;
}

/** Whether a clip covers `frame` on its track ([start, start+duration)). */
export function clipCoversFrame(clip: Clip, frame: number): boolean {
  return frame >= clip.startFrame && frame < clip.startFrame + clip.durationFrames;
}

/** The clip on `track` under `frame`, or null (tracks have no overlap). */
function clipAt(track: Track, frame: number): Clip | null {
  for (const c of track.clips) {
    if (clipCoversFrame(c, frame)) return c;
  }
  return null;
}

/**
 * Top-most VISUAL clip (video or image) at `frame`. Higher track index draws on
 * top (matches the render plan's ascending-track blend order), so the last
 * matching track wins. Kept for master-clock selection.
 */
export function activeVisualClip(timeline: Timeline, frame: number): ActiveMedia | null {
  const clips = activeVisualClips(timeline, frame);
  return clips.length > 0 ? clips[clips.length - 1] : null;
}

/**
 * Every visible VISUAL clip at `frame`, ordered bottom -> top by track index.
 * This is the DOM equivalent of upstream's video-composition layer list.
 */
export function activeVisualClips(timeline: Timeline, frame: number): ActiveMedia[] {
  const out: ActiveMedia[] = [];
  timeline.tracks.forEach((track, trackIndex) => {
    if (track.hidden || track.type === "audio") return;
    const clip = clipAt(track, frame);
    if (!clip) return;
    if (clip.mediaType !== "video" && clip.mediaType !== "image") return;
    out.push({ clip, track, trackIndex });
  });
  return out;
}

/**
 * Audio sources at `frame`: every clip on a non-muted AUDIO track. A video
 * clip's own sound is played by its visual `<video>` element (see
 * `activeVisualClip`), so it is not duplicated here.
 */
export function activeAudioClips(timeline: Timeline, frame: number): ActiveMedia[] {
  const out: ActiveMedia[] = [];
  timeline.tracks.forEach((track, trackIndex) => {
    if (track.type !== "audio" || track.muted) return;
    const clip = clipAt(track, frame);
    if (clip) out.push({ clip, track, trackIndex });
  });
  return out;
}

/**
 * The source-media time (seconds) a clip plays at timeline `frame`:
 * `(trimStart + (frame - start) * speed) / fps`. Mirrors `source_frame_index`
 * (opentake-render plan/build.rs) in seconds. Clamped at 0.
 */
export function sourceTimeSec(clip: Clip, frame: number, fps: number): number {
  const speed = clip.speed > 0 ? clip.speed : 1;
  const safeFps = fps > 0 ? fps : 30;
  const srcFrame = clip.trimStartFrame + (frame - clip.startFrame) * speed;
  return Math.max(0, srcFrame / safeFps);
}

/**
 * Inverse of {@link sourceTimeSec}: the timeline frame a clip's element
 * `currentTime` (seconds) corresponds to. Used to drive the playhead from the
 * master media element's clock (upstream's periodic time observer).
 */
export function frameForSourceTime(clip: Clip, timeSec: number, fps: number): number {
  const speed = clip.speed > 0 ? clip.speed : 1;
  const safeFps = fps > 0 ? fps : 30;
  const srcFrame = timeSec * safeFps;
  return clip.startFrame + (srcFrame - clip.trimStartFrame) / speed;
}

/** Effective 0–1 playback volume for a clip: clip volume, 0 when track muted. */
export function clipVolume(track: Track, clip: Clip): number {
  if (track.muted) return 0;
  const v = clip.volume;
  return Math.max(0, Math.min(1, Number.isFinite(v) ? v : 1));
}

/** Effective 0–1 opacity for a clip (clamped; defaults to 1). */
export function clipOpacity(clip: Clip): number {
  const o = clip.opacity;
  return Math.max(0, Math.min(1, Number.isFinite(o) ? o : 1));
}

/**
 * Whether a visual video clip's own audio should be silenced because an audio
 * track is already playing the SAME source (its sound was extracted to a linked
 * audio clip). Prevents the double-audio that linked-audio splitting would
 * otherwise cause.
 */
export function visualAudioIsDuplicated(
  visual: ActiveMedia | null,
  audios: ActiveMedia[],
): boolean {
  if (!visual || visual.clip.mediaType !== "video") return false;
  return audios.some((a) => a.clip.mediaRef === visual.clip.mediaRef);
}

/**
 * Next playhead frame for one engine tick (pure; the seam upstream calls the
 * periodic time observer, VideoEngine.swift). The browser has separate source
 * elements rather than a single AVPlayer composition, so DOM media clocks are
 * treated as followers; a stale source clock must never pull the timeline
 * backward or across clip boundaries. The result is pre-clamp — the caller
 * clamps to the last drawable frame and stops at the end.
 */
export function advancePlayhead(args: {
  currentFrame: number;
  dtSec: number;
  fps: number;
}): number {
  const { currentFrame, dtSec } = args;
  const safeFps = args.fps > 0 ? args.fps : 30;
  return currentFrame + dtSec * safeFps;
}
