/**
 * Pure value-mapping helpers for editing an animated (keyframe-track-active)
 * Inspector property. Each function takes the sampled-at-playhead value plus
 * the user's raw field input and returns the `KeyframeValueReq` to pass to
 * `upsertKeyframe`. These mirror the SAME semantic value the static
 * (non-keyframed) field commits via `setClipProperties` today — see
 * `Inspector.tsx`'s static `onCommit` handlers for each property, and
 * `crates/opentake-ops/src/command.rs::upsert_keyframe` for the Rust-side
 * property/value-kind pairing (Opacity/Volume/Rotation=Scalar,
 * Position/Scale=Pair, Crop=Crop; Volume is stored in dB).
 */

import { dbFromLinear, resizeTransformKeepingSourceAspect } from "./clip";
import type { Crop, KeyframeValueReq, Transform } from "./types";

/** Opacity field shows a 0–100 percentage; the track stores 0–1. */
export function opacityKeyframeValue(percent: number): KeyframeValueReq {
  return { kind: "scalar", value: percent / 100 };
}

/** Rotation field and track both use degrees — no conversion. */
export function rotationKeyframeValue(degrees: number): KeyframeValueReq {
  return { kind: "scalar", value: degrees };
}

/** Volume field displays dB (via `ScrubbableNumberField`'s `format`), but its
 *  underlying control value is LINEAR amplitude (matching the static field's
 *  `value={clip.volume}` / `onCommit={(v) => commit({ volume: v })}`). The
 *  track stores dB, so convert linear -> dB with the same `dbFromLinear` the
 *  static ReadOnly display already uses for `sampledVolume`. */
export function volumeKeyframeValue(linear: number): KeyframeValueReq {
  return { kind: "scalar", value: dbFromLinear(linear) };
}

/** Scale field commits the normalized canvas WIDTH; height is re-derived by
 *  the SAME `resizeTransformKeepingSourceAspect` the static commit uses (it
 *  is reused here rather than re-derived, so the aspect-fallback behavior —
 *  falling back to `clip.transform`'s own aspect when `aspect` is null —
 *  stays byte-for-byte identical to the static path). The scale track stores
 *  an `AnimPair` where `a` = width, `b` = height (see `sizeAt` in clip.ts). */
export function scaleKeyframeValue(
  clipTransform: Transform,
  width: number,
  aspect: number | null,
): KeyframeValueReq {
  const next = resizeTransformKeepingSourceAspect(clipTransform, width, aspect);
  return { kind: "pair", value: { a: next.width, b: next.height } };
}

/** Position X: preserve the sampled Y (other axis) from `topLeftAt`. The
 *  position track stores an `AnimPair` where `a` = x, `b` = y. */
export function positionXKeyframeValue(newX: number, sampledY: number): KeyframeValueReq {
  return { kind: "pair", value: { a: newX, b: sampledY } };
}

/** Position Y: preserve the sampled X (other axis). */
export function positionYKeyframeValue(sampledX: number, newY: number): KeyframeValueReq {
  return { kind: "pair", value: { a: sampledX, b: newY } };
}

/** Crop edge: one edge changes, the other three are carried over from the
 *  sampled crop (immutable spread — never mutate `sampledCrop`). */
export function cropEdgeKeyframeValue(sampledCrop: Crop, edge: keyof Crop, value: number): KeyframeValueReq {
  return { kind: "crop", value: { ...sampledCrop, [edge]: value } };
}
