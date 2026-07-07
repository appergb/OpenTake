// Timeline -> mpv EDL translation for community-engine playback (libmpv).
//
// mpv's edl:// protocol concatenates source ranges into one seamless virtual
// stream — decoding, A/V sync, and presentation are all mpv's (ffmpeg-backed)
// problem, which is exactly the part WKWebView's <video> can't do for HEVC
// .mov and the retired in-house streaming path couldn't display. See
// DOCS/edl-mpv.rst in the mpv tree for the syntax.
//
// Scope (MVP, documented limits):
// - Plays the PRIMARY video track: the lowest-index video track (bottom
//   render layer — the main content line). Overlay tracks (PIP/text) and
//   independent audio tracks are not part of play-time output; the paused
//   composite remains pixel-exact.
// - A clip's embedded audio plays naturally (same source file).
// - speed != 1 clips: the segment keeps TIMELINE duration (playhead stays
//   aligned); content beyond/short of the source window is mpv's estimate.
// - Gaps are filled with lavfi black so timeline time == playback time.

import type { Timeline } from "./types";

/** EDL-escape one value: %<utf8-byte-length>%<value> (comma/semicolon safe). */
function edlEscape(value: string): string {
  const bytes = new TextEncoder().encode(value).length;
  return `%${bytes}%${value}`;
}

function seconds(frames: number, fps: number): number {
  return Math.max(0, frames) / fps;
}

/** Format seconds for EDL params: fixed micro precision, no exponent. */
function secStr(s: number): string {
  return s.toFixed(6);
}

/** The main-content video track: lowest index whose type is a visual kind. */
export function primaryVideoTrack(timeline: Timeline) {
  return timeline.tracks.find((t) => t.type === "video" || t.type === "image");
}

/**
 * Build an edl:// URL playing the primary video track from frame 0, or null
 * when there is nothing playable (no visual track / no media-backed clips).
 * `pathOf` maps a clip's mediaRef to an absolute source path (null = skip,
 * e.g. text clips have no source media).
 */
export function timelineToEdl(
  timeline: Timeline,
  pathOf: (mediaRef: string) => string | null,
): string | null {
  const fps = timeline.fps > 0 ? timeline.fps : 30;
  const track = primaryVideoTrack(timeline);
  if (!track) return null;

  const clips = [...track.clips]
    .filter((c) => (c.mediaType ?? "video") !== "text")
    .sort((a, b) => a.startFrame - b.startFrame);

  const segments: string[] = [];
  let cursorFrame = 0;

  const pushGap = (frames: number) => {
    if (frames <= 0) return;
    const d = seconds(frames, fps);
    // lavfi virtual input: black at project size/fps for the gap duration.
    const spec = `av://lavfi:[color=c=black:s=${timeline.width}x${timeline.height}:r=${fps}:d=${secStr(d)}]`;
    segments.push(`${edlEscape(spec)},length=${secStr(d)}`);
  };

  for (const clip of clips) {
    const path = pathOf(clip.mediaRef);
    if (!path) continue;
    pushGap(clip.startFrame - cursorFrame);
    const start = seconds(clip.trimStartFrame ?? 0, fps);
    const length = seconds(clip.durationFrames, fps);
    if (length <= 0) continue;
    segments.push(
      `${edlEscape(path)},start=${secStr(start)},length=${secStr(length)}`,
    );
    cursorFrame = clip.startFrame + clip.durationFrames;
  }

  if (!segments.some((s) => !s.includes("av://lavfi"))) return null;
  return `edl://${segments.join(";")}`;
}
