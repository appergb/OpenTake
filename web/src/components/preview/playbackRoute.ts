import type { Clip, Timeline } from "../../lib/types";
import { isAdvertisedEffectName } from "../../lib/effects";

export interface PlaybackRouteRuntime {
  rustAvailable: boolean;
  rustEnabled: boolean;
  /** WebKit failed to decode a visual clip in this exact project revision. */
  forceRust?: boolean;
}

export type UnsupportedPlaybackReason =
  | { code: "lottie"; clipId: string }
  | { code: "unknown-effect"; clipId: string; effect: string }
  | { code: "mask-overflow"; clipId: string; count: number; limit: 4 }
  | { code: "composited-reverse"; clipId: string }
  | { code: "composited-speed"; clipId: string; speed: number }
  | { code: "rust-unavailable" }
  | { code: "rust-disabled" };

export type TimelinePlaybackRoute =
  | { kind: "webkit"; reasons: [] }
  | { kind: "rust"; reasons: [] }
  | { kind: "unsupported"; reasons: UnsupportedPlaybackReason[] };

export function isRetryableRustPlaybackFailure(
  route: TimelinePlaybackRoute,
  rustEngineFailed: boolean,
): boolean {
  return (
    rustEngineFailed &&
    route.kind === "unsupported" &&
    route.reasons.length === 1 &&
    route.reasons[0]?.code === "rust-disabled"
  );
}

interface ClipCapabilities {
  clip: Clip;
  needsRust: boolean;
  reversed: boolean;
  speedChanged: boolean;
}

function inspectClip(
  clip: Clip,
  reasons: UnsupportedPlaybackReason[],
): ClipCapabilities {
  const masks = clip.masks ?? [];
  const effects = clip.effects ?? [];
  const enabledEffects = effects.filter((effect) => effect.enabled);
  const isLottie = clip.mediaType === "lottie" || clip.sourceClipType === "lottie";
  const needsRust =
    clip.mediaType === "text" ||
    clip.sourceClipType === "text" ||
    clip.colorGrade !== undefined ||
    clip.chromaKey !== undefined ||
    clip.stabilization !== undefined ||
    masks.length > 0 ||
    enabledEffects.length > 0;

  if (isLottie) reasons.push({ code: "lottie", clipId: clip.id });
  for (const effect of effects) {
    if (!isAdvertisedEffectName(effect.name)) {
      reasons.push({ code: "unknown-effect", clipId: clip.id, effect: effect.name });
    }
  }
  if (masks.length > 4) {
    reasons.push({ code: "mask-overflow", clipId: clip.id, count: masks.length, limit: 4 });
  }
  return {
    clip,
    needsRust,
    reversed: clip.reversed === true,
    speedChanged: clip.speed !== 1,
  };
}

/**
 * Choose only a renderer that can preserve every authored playback property.
 * Runtime preference is consulted after the capability matrix, so it cannot
 * force Rust for temporal remapping or WebKit for compositor-only content.
 */
export function resolveTimelinePlaybackRoute(
  timeline: Timeline,
  runtime: PlaybackRouteRuntime,
): TimelinePlaybackRoute {
  const reasons: UnsupportedPlaybackReason[] = [];
  const capabilities = timeline.tracks
    .filter((track) => !track.hidden)
    .flatMap((track) => track.clips.map((clip) => inspectClip(clip, reasons)));
  const needsRust = capabilities.some((item) => item.needsRust);
  const hasVideo = capabilities.some((item) => item.clip.mediaType === "video");
  const requiresNativeVideoStack =
    timeline.tracks.filter(
      (track) =>
        !track.hidden && track.clips.some((clip) => clip.mediaType === "video"),
    ).length > 1;
  const hasTemporalRemapping = capabilities.some(
    (item) => item.reversed || item.speedChanged,
  );

  if (needsRust) {
    for (const item of capabilities) {
      if (item.reversed) {
        reasons.push({ code: "composited-reverse", clipId: item.clip.id });
      }
      if (item.speedChanged) {
        reasons.push({
          code: "composited-speed",
          clipId: item.clip.id,
          speed: item.clip.speed,
        });
      }
    }
  }

  if (reasons.length > 0) return { kind: "unsupported", reasons };
  if (!needsRust) {
    // A single ordinary video track stays on the low-overhead WebKit route.
    // Multiple video tracks need the native compositor for deterministic
    // decode/layer parity; an explicit WebKit decode error also retries the
    // exact revision through FFmpeg. Temporal remapping stays on WebKit until
    // native reverse/speed parity exists.
    if (
      (requiresNativeVideoStack || runtime.forceRust === true) &&
      hasVideo &&
      !hasTemporalRemapping &&
      runtime.rustAvailable &&
      runtime.rustEnabled
    ) {
      return { kind: "rust", reasons: [] };
    }
    return { kind: "webkit", reasons: [] };
  }
  if (!runtime.rustAvailable) {
    return { kind: "unsupported", reasons: [{ code: "rust-unavailable" }] };
  }
  if (!runtime.rustEnabled) {
    return { kind: "unsupported", reasons: [{ code: "rust-disabled" }] };
  }
  return { kind: "rust", reasons: [] };
}
