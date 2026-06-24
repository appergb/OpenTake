/**
 * Clip renderer — port of `Timeline/ClipRenderer.draw` (SPEC §5.4). Draws one
 * clip into its rect following the exact upstream order: base fill, content
 * placeholder, fade wedges, left color strip, border, missing wash, label bar,
 * keyframe diamonds, trim handles. Thumbnail/waveform content is a placeholder
 * here (Rust media cache, SPEC §11.3) — drawn as a tinted band + type hint.
 */

import { ACCENT, CLIP, TEXT, TRIM, BORDER } from "../../lib/theme";
import { trackColor, clipLabel, isLinked } from "../../lib/clip";
import type { ClipRect } from "../../lib/geometry";
import type { Clip } from "../../lib/types";

interface DrawOpts {
  isSelected: boolean;
  fps: number;
  /** Normalized waveform buckets (`0 = loud, 1 = silence`) spanning the WHOLE
   *  source media, or undefined until the Rust `get_waveform` cache resolves. */
  waveform?: number[];
  /** The clip's source media file is offline (moved/deleted). Draws the error
   *  wash (port of `ClipRenderer` missing state). */
  missing?: boolean;
  /** This clip is being dragged (move/trim ghost): drawn semi-transparent at its
   *  live position so it follows the cursor. */
  ghost?: boolean;
}

/** Linear amplitude → dB, clamped to the volume slider range. 1:1 port of
 *  `VolumeScale.dbFromLinear` (opentake-domain clip.rs). */
export function dbFromLinear(linear: number): number {
  const FLOOR = -60;
  const CEIL = 15;
  if (linear > 0) return Math.min(CEIL, Math.max(FLOOR, 20 * Math.log10(linear)));
  return FLOOR;
}

/**
 * The `[start, end)` sample indices of `clip`'s trimmed source sub-range within a
 * `sampleCount`-long waveform that spans the WHOLE source. Port of the index math
 * in `ClipRenderer.drawWaveform` (Swift 207-213): `source_duration_frames =
 * round(duration*speed) + trim_start + trim_end`. Returns an empty range when the
 * clip has no positive source span.
 */
export function waveformSampleRange(
  clip: Pick<Clip, "durationFrames" | "speed" | "trimStartFrame" | "trimEndFrame">,
  sampleCount: number,
): { start: number; end: number } {
  const speed = clip.speed > 0 ? clip.speed : 1;
  const consumed = Math.round(clip.durationFrames * speed);
  const totalSource = consumed + clip.trimStartFrame + clip.trimEndFrame;
  if (totalSource <= 0 || sampleCount <= 0) return { start: 0, end: 0 };
  const startFrac = clip.trimStartFrame / totalSource;
  const endFrac = (clip.trimStartFrame + consumed) / totalSource;
  const start = Math.max(0, Math.min(sampleCount, Math.floor(startFrac * sampleCount)));
  const end = Math.max(start, Math.min(sampleCount, Math.floor(endFrac * sampleCount)));
  return { start, end };
}

/** Blend an "rgb(r,g,b)" string `frac` of the way toward white (upstream
 *  `themeColor.blended(withFraction:of:.white)`). */
function blendWhite(rgb: string, frac: number): string {
  const m = rgb.match(/rgb\((\d+),\s*(\d+),\s*(\d+)\)/);
  if (!m) return rgb;
  const mix = (c: number) => Math.round(c + (255 - c) * frac);
  return `rgb(${mix(+m[1])},${mix(+m[2])},${mix(+m[3])})`;
}

function roundRectPath(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  w: number,
  h: number,
  r: number,
) {
  const radius = Math.min(r, w / 2, h / 2);
  ctx.beginPath();
  ctx.moveTo(x + radius, y);
  ctx.arcTo(x + w, y, x + w, y + h, radius);
  ctx.arcTo(x + w, y + h, x, y + h, radius);
  ctx.arcTo(x, y + h, x, y, radius);
  ctx.arcTo(x, y, x + w, y, radius);
  ctx.closePath();
}

/** Parse "rgb(r,g,b)" -> "rgba(r,g,b,a)". */
function withAlpha(rgb: string, a: number): string {
  const m = rgb.match(/rgb\((\d+),\s*(\d+),\s*(\d+)\)/);
  if (!m) return rgb;
  return `rgba(${m[1]},${m[2]},${m[3]},${a})`;
}

export function drawClip(
  ctx: CanvasRenderingContext2D,
  clip: Clip,
  rect: ClipRect,
  opts: DrawOpts,
) {
  const { x, y, width, height } = rect;
  if (width <= 0 || height <= 0) return;
  const color = trackColor(clip.sourceClipType);
  const r = TRIM.clipCornerRadius;

  ctx.save();
  // Ghost (active move/trim): drawn semi-transparent so the user sees it follow
  // the cursor while the originals stay put underneath.
  if (opts.ghost) ctx.globalAlpha = 0.6;

  // 1. Base fill (ClipRenderer:74-81): selected 0.45 else 0.30.
  roundRectPath(ctx, x, y, width, height, r);
  ctx.fillStyle = withAlpha(color, opts.isSelected ? 0.45 : 0.3);
  ctx.fill();

  // 2. Content band placeholder (real thumbnails/waveform come from Rust cache).
  ctx.save();
  roundRectPath(ctx, x, y, width, height, r);
  ctx.clip();
  const contentX = x + CLIP.stripWidth + 1;
  const contentY = y + CLIP.labelBarHeight;
  const contentW = width - CLIP.stripWidth - 1 - TRIM.handleWidth;
  const contentH = height - CLIP.labelBarHeight;
  if (contentW > 2 && contentH > 2) {
    if (clip.mediaType === "audio") {
      if (opts.waveform && opts.waveform.length > 0) {
        drawWaveform(ctx, clip, contentX, contentY, contentW, contentH, color, opts.waveform);
      } else {
        // No samples yet (cache still resolving): a faint band, not a fake wave.
        ctx.fillStyle = withAlpha(color, 0.12);
        ctx.fillRect(contentX, contentY, contentW, contentH);
      }
    } else {
      ctx.fillStyle = withAlpha(color, 0.12);
      ctx.fillRect(contentX, contentY, contentW, contentH);
    }
  }
  ctx.restore();

  // 3. Opacity fade wedges (non-audio) — smoothstep curves with a knee near top.
  if (clip.mediaType !== "audio") {
    drawFades(ctx, clip, rect, opts.isSelected);
  }

  // 4. Left color strip (ClipRenderer:114-119): solid, more saturated.
  ctx.save();
  roundRectPath(ctx, x, y, width, height, r);
  ctx.clip();
  ctx.fillStyle = color;
  ctx.fillRect(x, y, CLIP.stripWidth, height);
  ctx.restore();

  // 5. Border (ClipRenderer:121-132). Selected = a clear blue 2px outline (the
  //    old near-white border read as grey on the clip body and was easy to miss).
  roundRectPath(ctx, x, y, width, height, r);
  if (opts.isSelected) {
    ctx.strokeStyle = "rgba(56,139,253,1)";
    ctx.lineWidth = 2;
  } else {
    ctx.strokeStyle = BORDER.primary;
    ctx.lineWidth = 0.5;
  }
  ctx.stroke();

  // 6. Missing-media wash (ClipRenderer:134-143): a translucent red fill + red
  //    border when the clip's source file is offline, so a "lost media" clip
  //    reads as broken. Clears automatically once the asset is relinked (the
  //    `missing` flag is derived from file existence on each refresh).
  if (opts.missing) {
    roundRectPath(ctx, x, y, width, height, r);
    ctx.fillStyle = withAlpha(ACCENT.systemRed, 0.35);
    ctx.fill();
    ctx.strokeStyle = ACCENT.systemRed;
    ctx.lineWidth = 1;
    ctx.stroke();
  }

  // 7. Label bar (ClipRenderer:594-621): clip wider than 20px.
  if (width > CLIP.minWidthForLabel) {
    ctx.save();
    ctx.beginPath();
    ctx.rect(x + CLIP.stripWidth + 3, y, width - CLIP.stripWidth - 3, CLIP.labelBarHeight);
    ctx.clip();
    ctx.fillStyle = TEXT.primary;
    ctx.font = `500 10px ${cssFontStack()}`;
    ctx.textBaseline = "middle";
    const label = clipLabel(clip, opts.fps);
    const tx = x + CLIP.stripWidth + 6;
    const ty = y + CLIP.labelBarHeight / 2;
    ctx.fillText(label, tx, ty);
    if (isLinked(clip)) {
      // Underline the name portion (before the double-space).
      const name = label.split("  ")[0];
      const nameW = ctx.measureText(name).width;
      ctx.strokeStyle = TEXT.primary;
      ctx.lineWidth = 0.5;
      ctx.beginPath();
      ctx.moveTo(tx, ty + 6);
      ctx.lineTo(tx + nameW, ty + 6);
      ctx.stroke();
    }
    ctx.restore();
  }

  // 9. Keyframe diamonds along the bottom (ClipRenderer:163-191), y = maxY-5.
  drawKeyframeMarkers(ctx, clip, rect);

  // 10. Trim handles (ClipRenderer:659-666): 4px muted bars on each edge.
  ctx.fillStyle = TEXT.muted;
  ctx.fillRect(x, y, TRIM.handleWidth, height);
  ctx.fillRect(x + width - TRIM.handleWidth, y, TRIM.handleWidth, height);

  ctx.restore();
}

/**
 * Real waveform render — port of `ClipRenderer.drawWaveform` (Swift 195-263).
 * `samples` are normalized buckets (`0 = loud, 1 = silence`) over the WHOLE
 * source. Maps the clip's trimmed source sub-range into the samples, peak-detects
 * (min, since 0 = loud) per output bar, shifts the dB axis by the clip volume,
 * and draws bottom-anchored 1px bars. Per-bar volume (keyframed/faded audio) is
 * a follow-up; the static-volume path matches the common case exactly.
 */
function drawWaveform(
  ctx: CanvasRenderingContext2D,
  clip: Clip,
  x: number,
  y: number,
  w: number,
  h: number,
  color: string,
  samples: number[],
) {
  if (w <= 2 || h <= 2 || samples.length === 0) return;

  // Visible source range → sample indices.
  const { start: sampleStart, end: sampleEnd } = waveformSampleRange(clip, samples.length);
  if (sampleEnd <= sampleStart) return;

  const barCount = Math.floor(w);
  if (barCount <= 0) return;
  const visCount = sampleEnd - sampleStart;

  // Samples are dB-normalized, so volume shifts the dB axis (not multiplies).
  const dbRange = 50;
  const staticShift = dbFromLinear(clip.volume) / dbRange;

  ctx.fillStyle = withAlpha(blendWhite(color, 0.3), 0.85);
  for (let i = 0; i < barCount; i++) {
    const sStart = sampleStart + Math.floor((i * visCount) / barCount);
    const sEnd = Math.max(sStart + 1, sampleStart + Math.floor(((i + 1) * visCount) / barCount));
    const end = Math.min(sEnd, sampleEnd);
    let loudest = 1; // 0 = loud, so the loudest sample is the MIN
    for (let j = sStart; j < end; j++) {
      const s = samples[j];
      if (s < loudest) loudest = s;
    }
    const dbAmp = Math.max(0, 1 - loudest + staticShift);
    const amplitude = Math.min(1, dbAmp);
    const barHeight = Math.max(1, amplitude * (h - 2));
    ctx.fillRect(x + i, y + h - barHeight - 1, 1, barHeight);
  }
}

const FADE_KNEE_TOP_INSET = 4;
const FADE_EDGE_INSET = 6;
const FADE_KNEE_SIZE = 7;

/** Standard smoothstep (matches the shader + upstream `smoothstep`). */
function smoothstep(t: number): number {
  return t * t * (3 - 2 * t);
}

/** Sample points along a fade ramp (1:1 with `fadeCurvePoints`): a straight line
 *  for linear/hold, a 12-step smoothstep curve for smooth. */
function fadeCurvePoints(
  sx: number,
  sy: number,
  ex: number,
  ey: number,
  interp: string,
): Array<[number, number]> {
  if (interp !== "smooth") return [[ex, ey]];
  const steps = 12;
  const out: Array<[number, number]> = [];
  for (let s = 1; s <= steps; s++) {
    const t = s / steps;
    out.push([sx + (ex - sx) * t, sy + (ey - sy) * smoothstep(t)]);
  }
  return out;
}

/** One fade wedge — dark fill above the curve + a stroked curve (port of
 *  `ClipRenderer.drawFadeWedge`). `silent` is the silent (bottom) corner, `knee`
 *  the top control point; the fill rises to `fillTopY`. */
function drawFadeWedge(
  ctx: CanvasRenderingContext2D,
  silent: [number, number],
  knee: [number, number],
  interp: string,
  fillTopY: number,
  fillAlpha: number,
  strokeColor: string,
) {
  const curve = fadeCurvePoints(silent[0], silent[1], knee[0], knee[1], interp);
  // Fill the wedge above the curve.
  ctx.beginPath();
  ctx.moveTo(silent[0], silent[1]);
  ctx.lineTo(silent[0], fillTopY);
  ctx.lineTo(knee[0], fillTopY);
  if (fillTopY !== knee[1]) ctx.lineTo(knee[0], knee[1]);
  for (let i = curve.length - 2; i >= 0; i--) ctx.lineTo(curve[i][0], curve[i][1]);
  ctx.closePath();
  ctx.fillStyle = `rgba(0,0,0,${fillAlpha})`;
  ctx.fill();
  // Stroke the curve.
  ctx.beginPath();
  ctx.moveTo(silent[0], silent[1]);
  for (const [px, py] of curve) ctx.lineTo(px, py);
  ctx.strokeStyle = strokeColor;
  ctx.lineWidth = 1.5;
  ctx.stroke();
}

/** Opacity fade wedges for visual (non-audio) clips — smoothstep curves rising
 *  to a knee near the top of the body, dark fill above (port of the video-fade
 *  block in ClipRenderer.swift:386-435). */
function drawFades(ctx: CanvasRenderingContext2D, clip: Clip, rect: ClipRect, isSelected: boolean) {
  const { x, y, width, height } = rect;
  if (clip.durationFrames <= 0) return;
  const ppf = width / clip.durationFrames;
  const bodyMinY = y + CLIP.labelBarHeight;
  const bodyMaxY = y + height - 1;
  const kneeY = bodyMinY + FADE_KNEE_TOP_INSET;
  const alpha = isSelected ? 0.95 : 0.75;
  const fadeColor = `rgba(255,255,255,${alpha * 0.7})`;
  const kneeX = (offsetFrames: number) =>
    Math.max(x + FADE_EDGE_INSET, Math.min(x + width - FADE_EDGE_INSET, x + offsetFrames * ppf));

  ctx.save();
  if (clip.fadeInFrames > 0) {
    const lx = kneeX(Math.min(clip.fadeInFrames, clip.durationFrames));
    drawFadeWedge(ctx, [x, bodyMaxY], [lx, kneeY], clip.fadeInInterpolation, bodyMinY, 0.6, fadeColor);
  }
  if (clip.fadeOutFrames > 0) {
    const rx = kneeX(Math.max(0, clip.durationFrames - clip.fadeOutFrames));
    drawFadeWedge(ctx, [x + width, bodyMaxY], [rx, kneeY], clip.fadeOutInterpolation, bodyMinY, 0.6, fadeColor);
  }
  // Draggable knee handles (visual indicators) when selected.
  if (isSelected) {
    ctx.fillStyle = `rgba(255,255,255,${alpha})`;
    ctx.strokeStyle = "rgba(0,0,0,0.5)";
    ctx.lineWidth = 0.5;
    const half = FADE_KNEE_SIZE / 2;
    if (clip.fadeInFrames > 0) {
      const lx = kneeX(Math.min(clip.fadeInFrames, clip.durationFrames));
      ctx.fillRect(lx - half, kneeY - half, FADE_KNEE_SIZE, FADE_KNEE_SIZE);
      ctx.strokeRect(lx - half, kneeY - half, FADE_KNEE_SIZE, FADE_KNEE_SIZE);
    }
    if (clip.fadeOutFrames > 0) {
      const rx = kneeX(Math.max(0, clip.durationFrames - clip.fadeOutFrames));
      ctx.fillRect(rx - half, kneeY - half, FADE_KNEE_SIZE, FADE_KNEE_SIZE);
      ctx.strokeRect(rx - half, kneeY - half, FADE_KNEE_SIZE, FADE_KNEE_SIZE);
    }
  }
  ctx.restore();
}

function drawKeyframeMarkers(ctx: CanvasRenderingContext2D, clip: Clip, rect: ClipRect) {
  const tracks = [
    clip.opacityTrack,
    clip.positionTrack,
    clip.scaleTrack,
    clip.rotationTrack,
    clip.cropTrack,
    clip.volumeTrack,
  ];
  const frames = new Set<number>();
  for (const t of tracks) {
    if (!t) continue;
    for (const kf of t.keyframes) frames.add(kf.frame);
  }
  if (frames.size === 0 || clip.durationFrames <= 0) return;
  // Markers live INSIDE the trim handles, so the diamond at frame 0 isn't hidden
  // under the left handle (ClipRenderer.swift:172-181).
  const ppf = (rect.width - 2 * TRIM.handleWidth) / clip.durationFrames;
  if (ppf <= 0) return;
  const baseX = rect.x + TRIM.handleWidth;
  const cy = rect.y + rect.height - 5;
  const radius = CLIP.keyframeDiamondRadius;
  ctx.save();
  for (const f of frames) {
    if (f < 0 || f > clip.durationFrames) continue; // clip.contains(timelineFrame:)
    const cx = baseX + f * ppf;
    ctx.beginPath();
    ctx.moveTo(cx, cy - radius);
    ctx.lineTo(cx + radius, cy);
    ctx.lineTo(cx, cy + radius);
    ctx.lineTo(cx - radius, cy);
    ctx.closePath();
    ctx.fillStyle = withAlpha(ACCENT.systemYellow, 0.95);
    ctx.fill();
    ctx.strokeStyle = "rgba(0,0,0,0.5)";
    ctx.lineWidth = 0.5;
    ctx.stroke();
  }
  ctx.restore();
}

function cssFontStack(): string {
  return '-apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC", system-ui, sans-serif';
}
