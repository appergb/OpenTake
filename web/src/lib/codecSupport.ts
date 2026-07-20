/**
 * Browser codec support detection for video preview fallback (#131).
 *
 * The Tauri WebView (WebKit/WebView2) plays source media directly via
 * `<video src={assetUrl(path)}>`. Most H.264/AAC MP4s play natively, but VP9/
 * AV1 WebM and HEVC MOV files can produce a black frame or an `onError` when
 * the system codec isn't available. This module probes `canPlayType` once and
 * exposes a `bestCodec` helper so the preview layer can decide whether to
 * attempt a transcode fallback or surface a "codec unsupported" affordance
 * instead of silently showing black.
 *
 * The detection is cached: `document.createElement("video")` is cheap but
 * repeated calls during rapid clip switching are wasteful, and the result
 * never changes within a page session.
 */

export interface CodecSupport {
  h264: boolean;
  vp9: boolean;
  av1: boolean;
  hevc: boolean;
}

let cached: CodecSupport | null = null;

/**
 * Detect the WebView's video codec support via `HTMLMediaElement.canPlayType`.
 * The result is cached for the page lifetime — codec support doesn't change
 * without a browser/OS update, which requires an app restart.
 *
 * @returns A {@link CodecSupport} object with per-codec booleans.
 */
export function detectCodecSupport(): CodecSupport {
  if (cached) return cached;
  const v = document.createElement("video");
  cached = {
    h264: v.canPlayType('video/mp4; codecs="avc1.42E01E"') !== "",
    vp9: v.canPlayType('video/webm; codecs="vp9"') !== "",
    av1: v.canPlayType('video/mp4; codecs="av01.0.05M.08"') !== "",
    hevc: v.canPlayType('video/mp4; codecs="hev1.1.6.L93.B0"') !== "",
  };
  return cached;
}

/**
 * Returns the best playable codec for a given media file based on its
 * extension and the WebView's codec support. Returns `"unknown"` when no
 * supported codec matches — the caller should then surface a fallback
 * affordance (e.g. an "unsupported codec" overlay or a transcode request)
 * instead of showing a black `<video>`.
 *
 * @param fileName - The source file name (extension is inspected).
 * @returns The best supported codec, or `"unknown"`.
 */
export function bestCodec(
  fileName: string,
): "h264" | "vp9" | "av1" | "hevc" | "unknown" {
  const support = detectCodecSupport();
  const ext = fileName.toLowerCase().split(".").pop() ?? "";
  // WebM files typically use VP9/AV1.
  if (ext === "webm") {
    if (support.av1) return "av1";
    if (support.vp9) return "vp9";
  }
  // MP4/MOV typically H.264/HEVC.
  if (ext === "mp4" || ext === "mov") {
    if (support.h264) return "h264";
    if (support.hevc) return "hevc";
  }
  return "unknown";
}
