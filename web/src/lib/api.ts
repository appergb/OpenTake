/**
 * Tauri bridge. All editing goes through `edit_apply`; the mirror is fetched via
 * `get_timeline` and refreshed on the `timeline_changed` event (SPEC §11).
 *
 * Degrades gracefully when not running inside Tauri (plain `vite dev` /
 * `vite preview` in a browser): `isTauri` is false and commands resolve against
 * a local in-memory fallback so the UI shell is still explorable. The real
 * editing truth always lives in Rust when running under Tauri.
 */

import type {
  ClipType,
  EditRequest,
  EditResult,
  MediaList,
  SecretStatus,
  TimelineSnapshot,
} from "./types";

// Tauri injects `__TAURI_INTERNALS__` on the window when running in the shell.
export const isTauri =
  typeof window !== "undefined" &&
  "__TAURI_INTERNALS__" in (window as unknown as Record<string, unknown>);

type InvokeFn = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;
type ListenFn = (
  event: string,
  handler: (e: { payload: unknown }) => void,
) => Promise<() => void>;

let invokeImpl: InvokeFn | null = null;
let listenImpl: ListenFn | null = null;

async function ensureTauri(): Promise<void> {
  if (!isTauri || invokeImpl) return;
  const core = await import("@tauri-apps/api/core");
  const ev = await import("@tauri-apps/api/event");
  invokeImpl = core.invoke as InvokeFn;
  listenImpl = ev.listen as unknown as ListenFn;
}

// MARK: - Commands

export async function getTimeline(): Promise<TimelineSnapshot> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<TimelineSnapshot>("get_timeline");
  return fallback.getTimeline();
}

export async function editApply(command: EditRequest): Promise<EditResult> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<EditResult>("edit_apply", { command });
  return fallback.editApply(command);
}

/** Sequential automation wrapper: each command still goes through the single
 * Rust `EditCommand` authority via `edit_apply`. */
export async function editApplyMany(commands: EditRequest[]): Promise<EditResult[]> {
  const results: EditResult[] = [];
  for (const command of commands) {
    results.push(await editApply(command));
  }
  return results;
}

export async function undo(): Promise<EditResult> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<EditResult>("undo");
  return fallback.noop("Undo");
}

export async function redo(): Promise<EditResult> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<EditResult>("redo");
  return fallback.noop("Redo");
}

export async function canUndo(): Promise<boolean> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<boolean>("can_undo");
  return false;
}

export async function canRedo(): Promise<boolean> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<boolean>("can_redo");
  return false;
}

export async function projectNew(): Promise<void> {
  await ensureTauri();
  if (invokeImpl) {
    await invokeImpl<void>("project_new");
    return;
  }
  fallback.reset();
}

export async function projectOpen(path: string): Promise<TimelineSnapshot> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<TimelineSnapshot>("project_open", { path });
  return fallback.getTimeline();
}

export async function projectSave(path: string | null): Promise<string> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<string>("project_save", { path });
  return path ?? "";
}

/** The default folder new projects save into (`~/Documents/OpenTake`). Empty
 *  string outside Tauri (where the save dialog is unavailable anyway). */
export async function getDefaultProjectDir(): Promise<string> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<string>("get_default_project_dir");
  return "";
}

// MARK: - Timeline interchange export (XMEML / EDL / OTIO / FCPXML)
//
// Four standard editorial-interchange formats, each a thin path-only command
// that writes the live timeline to disk and returns nothing (or rejects). All
// no-op outside Tauri (no Rust core / no file system). Pick the format per the
// target NLE — see each wrapper.

/**
 * Export the current timeline as XMEML 4 (Final Cut Pro 7 XML, `.xml`). This is
 * the Premiere / DaVinci Resolve / 剪映-importable interchange format (Premiere
 * does NOT read modern FCPXML; DaVinci/FCP still import FCP7 XML).
 */
export async function exportXmeml(path: string): Promise<void> {
  await ensureTauri();
  if (invokeImpl) {
    await invokeImpl<void>("export_xmeml", { path });
  }
}

/**
 * @deprecated Use {@link exportXmeml}. Historically named "fcpxml" but always
 * produced XMEML 4 (FCP7 XML). Kept so older callers keep working; for native
 * Final Cut Pro X FCPXML use {@link exportFcpxmlModern}.
 */
export async function exportFcpxml(path: string): Promise<void> {
  return exportXmeml(path);
}

/**
 * Export the current timeline as a CMX3600 EDL (`.edl`) — the classic edit
 * decision list Premiere / DaVinci / Avid / 剪映 import. Video track only;
 * effects/transforms/audio are dropped (a CMX3600 limitation).
 */
export async function exportEdl(path: string): Promise<void> {
  await ensureTauri();
  if (invokeImpl) {
    await invokeImpl<void>("export_edl", { path });
  }
}

/**
 * Export the current timeline as OpenTimelineIO JSON (`.otio`) — the industry
 * interchange standard `otioview` / DaVinci / Blender read. Preserves track
 * order/kind, clip placement, source ranges, gaps, and media references.
 */
export async function exportOtio(path: string): Promise<void> {
  await ensureTauri();
  if (invokeImpl) {
    await invokeImpl<void>("export_otio", { path });
  }
}

/**
 * Export the current timeline as native Final Cut Pro X FCPXML 1.10
 * (`.fcpxml`). Carries text overlays (`<title>`), transforms, and volume that
 * XMEML can't. NOTE: Premiere does NOT import FCPXML — use {@link exportXmeml}
 * for Premiere / DaVinci / 剪映.
 */
export async function exportFcpxmlModern(path: string): Promise<void> {
  await ensureTauri();
  if (invokeImpl) {
    await invokeImpl<void>("export_fcpxml_modern", { path });
  }
}

// MARK: - Subtitle export (#29)
//
// `export_subtitles` collects the timeline's caption clips (any clip with a
// caption group + text) and writes them as a SubRip (`.srt`) or WebVTT (`.vtt`)
// file. It mirrors the Rust DTO verbatim (lower-case `format` tag matching the
// extension) and returns the cue count so the caller can tell "wrote N cues"
// from "timeline has no captions". No-op outside Tauri (no Rust core / no FS) —
// the summary then reports zero cues so the caller can surface an unavailable
// state without throwing.

/** Subtitle container. Lower-case tags match the chosen file extension. */
export type SubtitleFormat = "srt" | "vtt";

/** Summary of a completed subtitle export (mirror of Rust `SubtitleExportSummary`). */
export interface SubtitleExportSummary {
  outPath: string;
  cueCount: number;
}

export async function exportSubtitles(
  path: string,
  format: SubtitleFormat,
): Promise<SubtitleExportSummary> {
  await ensureTauri();
  if (invokeImpl) {
    return invokeImpl<SubtitleExportSummary>("export_subtitles", { path, format });
  }
  return { outPath: path, cueCount: 0 };
}

// MARK: - Video export (#112)
//
// `export_video` composites every timeline frame on the GPU and encodes it to a
// real file on disk (H.264 / .mp4 in this cut; H.265 / ProRes are accepted by
// the type but rejected by the backend until wired). The request mirrors the
// Rust `ExportRequest` DTO verbatim (camelCase `outPath`; lowercase enum tags).
// Outside Tauri there is no GPU/ffmpeg, so the wrapper rejects with a friendly
// error rather than silently no-op'ing (an export the user asked for must not
// quietly do nothing).

/** Output codec. Only `h264` is fully wired; the others are reserved. */
export type ExportCodec = "h264" | "h265" | "prores";

/** Output short-edge resolution selector. */
export type ExportQuality = "720p" | "1080p" | "4k";

/** Parameters for a video export (mirror of Rust `ExportRequest`). */
export interface ExportRequest {
  outPath: string;
  codec: ExportCodec;
  quality: ExportQuality;
}

/** Summary of a completed export (mirror of Rust `ExportSummary`). */
export interface ExportSummary {
  outPath: string;
  width: number;
  height: number;
  fps: number;
  frameCount: number;
}

export async function exportVideo(req: ExportRequest): Promise<ExportSummary> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<ExportSummary>("export_video", { req });
  throw new Error("video export requires the desktop app (GPU + ffmpeg)");
}

// MARK: - Media commands
//
// `import_folder` scans a directory for white-listed media and imports each;
// `import_media` imports an explicit file list; `get_media` returns the current
// catalog. All three are no-ops outside Tauri (no Rust core / no file system),
// returning an empty catalog so the browser shell degrades gracefully.

export async function importFolder(
  path: string,
  recursive = false,
): Promise<MediaList> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<MediaList>("import_folder", { path, recursive });
  return { items: [], folders: [] };
}

export async function importMedia(paths: string[]): Promise<MediaList> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<MediaList>("import_media", { paths });
  return { items: [], folders: [] };
}

export async function getMedia(): Promise<MediaList> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<MediaList>("get_media");
  return { items: [], folders: [] };
}

/**
 * `extract_audio`: extract the audio track from a media asset into a
 * self-contained audio file. `outPath`'s extension picks the codec
 * (`.m4a` -> AAC, `.mp3` -> libmp3lame, `.wav` -> PCM s16le). Returns the
 * output path on success. Outside Tauri there is no ffmpeg, so this rejects
 * with a friendly error.
 */
export async function extractAudio(mediaId: string, outPath: string): Promise<string> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<string>("extract_audio", { mediaId, outPath });
  throw new Error("audio extraction requires the desktop app (ffmpeg)");
}

/**
 * Relink an offline asset to a newly chosen file, KEEPING its id so every clip
 * that references it recovers in place (the fix for "lost media stays red after
 * re-selecting the path" — re-importing would mint a new id and strand the old
 * clips). The new file's type must match the original. Returns the refreshed
 * catalog (the asset's `missing` is recomputed → `false`).
 */
export async function relinkMedia(mediaRef: string, newPath: string): Promise<MediaList> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<MediaList>("relink_media", { mediaRef, newPath });
  return { items: [], folders: [] };
}

export interface ThumbnailResult {
  mediaRef: string;
  type: ClipType;
  thumbnailPath?: string | null;
  spritePath?: string | null;
  tileWidth?: number | null;
  tileHeight?: number | null;
  columns?: number | null;
  times: number[];
}

export async function generateThumbnail(
  mediaRef: string,
  opts?: { timeSecs?: number; maxFrames?: number; includeSprite?: boolean },
): Promise<ThumbnailResult | null> {
  await ensureTauri();
  if (invokeImpl) {
    const args: Record<string, unknown> = { mediaRef };
    if (opts?.timeSecs != null) args.timeSecs = opts.timeSecs;
    if (opts?.maxFrames != null) args.maxFrames = opts.maxFrames;
    if (opts?.includeSprite != null) args.includeSprite = opts.includeSprite;
    try {
      return await invokeImpl<ThumbnailResult>("generate_thumbnail", args);
    } catch (e) {
      console.warn(`generate_thumbnail failed for ${mediaRef}:`, e);
      return null;
    }
  }
  return null;
}

/**
 * Decode (and disk-cache) a HI-RES first-frame poster for a VIDEO asset and
 * return its on-disk path (run it through {@link assetUrl} to display). This is
 * the instant, sharp placeholder painted behind the preview `<video>` so a cold
 * click shows its first frame immediately instead of a blank/spinner — the asset
 * protocol then streams the real video progressively (it honors HTTP Range, so
 * `<video preload="metadata">` never downloads the whole file). Returns null for
 * non-video assets (images render straight from disk; audio has no frame) and
 * outside Tauri; decode errors are swallowed (best-effort) so the preview just
 * has no poster rather than throwing. */
export async function previewPoster(
  mediaRef: string,
  timeSecs?: number,
): Promise<string | null> {
  await ensureTauri();
  if (!invokeImpl) return null;
  try {
    const args: Record<string, unknown> = { mediaRef };
    if (timeSecs != null) args.timeSecs = timeSecs;
    return await invokeImpl<string | null>("preview_poster", args);
  } catch (e) {
    console.warn(`preview_poster failed for ${mediaRef}:`, e);
    return null;
  }
}

/** Fire-and-forget preview warm-up for a media asset when it's selected or drag
 *  starts: the backend decodes its hi-res first-frame poster into the on-disk
 *  cache on a worker thread, so a subsequent preview shows a sharp first frame
 *  with no decode on the interaction path. Deliberately light — it no longer
 *  warms the heavy 240-frame filmstrip sprite or waveform (which never sped
 *  actual `<video>` playback). No-op in the browser fallback / for non-video;
 *  errors are swallowed (best-effort). */
export async function preloadMedia(mediaRef: string): Promise<void> {
  await ensureTauri();
  if (!invokeImpl) return;
  try {
    await invokeImpl<void>("preload_media", { mediaRef });
  } catch (e) {
    console.warn(`preload_media failed for ${mediaRef}:`, e);
  }
}

// MARK: - Timeline composite preview (#47)
//
// `composite_frame` renders the timeline at a frame on the GPU (wgpu compositor)
// and returns a PNG data URL the Preview paints onto a <canvas>. `maxSize` caps
// the longest side (px); omit for the backend default. Outside Tauri there is no
// GPU/core, so this returns null and the Preview keeps its placeholder.

/** One composited timeline frame: a PNG data URL plus its pixel size. */
export interface CompositeFrame {
  width: number;
  height: number;
  dataUrl: string;
}

export async function compositeFrame(
  frame: number,
  maxSize?: number,
): Promise<CompositeFrame | null> {
  await ensureTauri();
  // The backend command takes an `i32`; the playhead accumulates as a float
  // during playback, so floor to the current frame before invoking (a
  // non-integer is rejected/coerced inconsistently by Tauri's deserializer).
  if (invokeImpl)
    return invokeImpl<CompositeFrame>("composite_frame", {
      frame: Math.floor(frame),
      maxSize,
    });
  return null;
}

/**
 * Normalized waveform buckets (`0 = loud, 1 = silence`) for a media asset,
 * computed/cached by the Rust media engine (`get_waveform`). The array spans the
 * WHOLE source; the timeline renderer maps the clip's trimmed sub-range into it.
 * Returns null outside Tauri (no media engine).
 */
export async function getWaveform(mediaRef: string): Promise<number[] | null> {
  await ensureTauri();
  if (invokeImpl) {
    try {
      return await invokeImpl<number[]>("get_waveform", { mediaRef });
    } catch (e) {
      // No audio track / decode failure: the caller renders nothing. Surface
      // the reason — a silent swallow here is what masked the waveform decode
      // backend failing for whole categories of source files.
      console.warn(`get_waveform failed for ${mediaRef}:`, e);
      return null;
    }
  }
  return null;
}

// MARK: - BYOK secret store
//
// API keys are stored in the OS keychain by the Rust backend (`secret_*`
// commands wrapping `opentake-gen`'s `KeyringStore`). The plaintext key is sent
// only on save; every command returns a masked `SecretStatus`, so the key never
// lives in JS memory or localStorage. Outside Tauri there is no keychain, so the
// fallback reports "no key" — the form renders but cannot persist.

const NO_SECRET: SecretStatus = { hasKey: false, masked: "" };

export async function secretSave(
  provider: string,
  key: string,
): Promise<SecretStatus> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<SecretStatus>("secret_save", { provider, key });
  return NO_SECRET;
}

export async function secretLoad(provider: string): Promise<SecretStatus> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<SecretStatus>("secret_load", { provider });
  return NO_SECRET;
}

export async function secretDelete(provider: string): Promise<SecretStatus> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<SecretStatus>("secret_delete", { provider });
  return NO_SECRET;
}

// MARK: - Events

/** Subscribe to `timeline_changed`. Returns an unlisten function. No-op (and a
 *  no-op unlisten) when not in Tauri. */
export async function onTimelineChanged(
  handler: (version: number) => void,
): Promise<() => void> {
  await ensureTauri();
  if (!listenImpl) return () => {};
  return listenImpl("timeline_changed", (e) => {
    const payload = e.payload as { version?: number } | undefined;
    if (payload && typeof payload.version === "number") handler(payload.version);
  });
}

export async function onProjectOpened(
  handler: (path: string, version: number) => void,
): Promise<() => void> {
  await ensureTauri();
  if (!listenImpl) return () => {};
  return listenImpl("project_opened", (e) => {
    const p = e.payload as { path?: string; version?: number } | undefined;
    if (p) handler(p.path ?? "", p.version ?? 0);
  });
}

/** Subscribe to `media_changed` (manifest mutated by an import). The payload
 *  carries a version; the handler just needs to know it fired so it can re-fetch
 *  `get_media`. No-op outside Tauri. */
export async function onMediaChanged(handler: () => void): Promise<() => void> {
  await ensureTauri();
  if (!listenImpl) return () => {};
  return listenImpl("media_changed", () => handler());
}

/** Subscribe to `go_home` (emitted when the window is closed/hidden so the app
 *  keeps running in the background — the front end returns to the launcher so a
 *  Dock-reopen shows Home, mirroring upstream "close window → Home"). No-op
 *  outside Tauri. */
export async function onGoHome(handler: () => void): Promise<() => void> {
  await ensureTauri();
  if (!listenImpl) return () => {};
  return listenImpl("go_home", () => handler());
}

// MARK: - Streaming playback engine (#53)
//
// Continuous playback runs in Rust (decode → wgpu composite → MJPEG stream) with
// a cpal audio master clock. During PLAY the front end points an <img> at
// `get_preview_endpoint` and moves its playhead from `playback_frame` events;
// scrub/pause stay on the existing <video> path. All no-ops outside Tauri (the
// browser shell keeps the <video> playback path), and gated behind a runtime flag
// in the preview engine until verified on a real machine.

/** Start (or restart) Rust streaming playback from `fromFrame` (the current
 *  playhead). No-op outside Tauri. */
export async function playbackStart(fromFrame: number): Promise<void> {
  await ensureTauri();
  if (invokeImpl)
    await invokeImpl<void>("playback_start", { fromFrame: Math.floor(fromFrame) });
}

/** Pause Rust playback: the render thread stops (the front end then freezes the
 *  <video> on the last frame). No-op outside Tauri. */
export async function playbackPause(): Promise<void> {
  await ensureTauri();
  if (invokeImpl) await invokeImpl<void>("playback_pause");
}

/** Stop Rust playback and tear down the engine. No-op outside Tauri. */
export async function playbackStop(): Promise<void> {
  await ensureTauri();
  if (invokeImpl) await invokeImpl<void>("playback_stop");
}

/** Seek the running Rust engine to `frame` (no-op when not playing / outside Tauri). */
export async function playbackSeek(frame: number): Promise<void> {
  await ensureTauri();
  if (invokeImpl) await invokeImpl<void>("playback_seek", { frame: Math.floor(frame) });
}

/** The MJPEG stream URL to point a playback <img> at, or null outside Tauri. */
export async function getPreviewEndpoint(): Promise<string | null> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<string>("get_preview_endpoint");
  return null;
}

/** Subscribe to `playback_frame` (the Rust master clock's current frame). Returns
 *  an unlisten function; no-op (no-op unlisten) outside Tauri. */
export async function onPlaybackFrame(
  handler: (frame: number) => void,
): Promise<() => void> {
  await ensureTauri();
  if (!listenImpl) return () => {};
  return listenImpl("playback_frame", (e) => {
    const payload = e.payload as { frame?: number } | undefined;
    if (payload && typeof payload.frame === "number") handler(payload.frame);
  });
}

// MARK: - Browser fallback (mirror, not authoritative)
//
// When running outside Tauri there is no Rust core; provide a small in-memory
// timeline so the shell renders something. This is intentionally minimal — it
// is a preview aid, not a second editing engine.

import { createFallbackStore } from "./fallback";
const fallback = createFallbackStore();
