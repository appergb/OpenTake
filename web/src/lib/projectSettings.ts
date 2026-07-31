import type { MediaItem, Timeline } from "./types";

export interface ProjectSettingsTarget {
  fps: number;
  width: number;
  height: number;
}

export type ProjectSettingsDecision =
  | { kind: "proceed" }
  | { kind: "apply"; settings: ProjectSettingsTarget }
  | { kind: "prompt"; settings: ProjectSettingsTarget };

function positiveFinite(value: number | null | undefined): value is number {
  return typeof value === "number" && Number.isFinite(value) && value > 0;
}

function targetForVideo(timeline: Timeline, video: MediaItem): ProjectSettingsTarget {
  return {
    fps: positiveFinite(video.sourceFps) ? Math.max(1, Math.round(video.sourceFps)) : timeline.fps,
    width: positiveFinite(video.width) ? Math.round(video.width) : timeline.width,
    height: positiveFinite(video.height) ? Math.round(video.height) : timeline.height,
  };
}

/** Pure port of the upstream first-video settings decision. It intentionally
 * does not prompt once timeline content exists: changing FPS then would rescale
 * edits and interrupt normal imports. */
export function checkProjectSettings(
  timeline: Timeline,
  assets: readonly MediaItem[],
): ProjectSettingsDecision {
  const video = assets.find((asset) => asset.type === "video");
  if (!video) return { kind: "proceed" };

  const settings = targetForVideo(timeline, video);
  if (!timeline.settingsConfigured) return { kind: "apply", settings };

  const hasTimelineContent = timeline.tracks.some((track) => track.clips.length > 0);
  if (hasTimelineContent) return { kind: "proceed" };

  const fpsMismatch = positiveFinite(video.sourceFps) && settings.fps !== timeline.fps;
  const resolutionMismatch =
    (positiveFinite(video.width) && settings.width !== timeline.width) ||
    (positiveFinite(video.height) && settings.height !== timeline.height);
  return fpsMismatch || resolutionMismatch ? { kind: "prompt", settings } : { kind: "proceed" };
}
