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
  AccountInfo,
  AccountStatus,
  AudioDenoise,
  CaptionRequest,
  ChatMessage,
  ChatSession,
  ChatToolCall,
  ClipType,
  EditRequest,
  EditResult,
  GenerateCaptionsResult,
  GenerationLog,
  MediaList,
  MattingModelStatus,
  MotionTrackingRegion,
  MotionTrackingResult,
  GenerateMatteResult,
  RemoveObjectResult,
  MatchColorResult,
  CaptionTranslationResult,
  CaptionTranslationReviewChange,
  ScriptToVideoResult,
  ScriptToVideoSegmentInput,
  AvatarGenerationResult,
  VoiceCloneResult,
  LutReference,
  LoudnessNormalization,
  DenoiseMode,
  ModelStatus,
  PlaybackCommandError,
  PlaybackFrameEvent,
  PlaybackIdentity,
  ProjectEditIdentity,
  RuntimeTimelineSnapshot,
  SearchIndexStatus,
  SearchModelStatus,
  SearchResults,
  SecretStatus,
  StabilizationTrack,
  StorageCategoryId,
  StorageUsage,
  Transcript,
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

/** Stable machine-readable error returned by typed core/edit Tauri commands. */
export class TauriCommandError extends Error {
  readonly code: string;

  constructor(code: string, message: string) {
    super(message);
    this.name = "TauriCommandError";
    this.code = code;
  }
}

function asTauriCommandError(error: unknown): Error {
  if (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    typeof error.code === "string" &&
    "message" in error &&
    typeof error.message === "string"
  ) {
    return new TauriCommandError(error.code, error.message);
  }
  return error instanceof Error ? error : new Error(String(error));
}

let invokeImpl: InvokeFn | null = null;
let listenImpl: ListenFn | null = null;
let previewEndpointProbe: Promise<string | null> | null = null;

async function ensureTauri(): Promise<void> {
  if (!isTauri || invokeImpl) return;
  const core = await import("@tauri-apps/api/core");
  const ev = await import("@tauri-apps/api/event");
  invokeImpl = core.invoke as InvokeFn;
  listenImpl = ev.listen as unknown as ListenFn;
}

// MARK: - Commands

export async function getTimeline(): Promise<RuntimeTimelineSnapshot> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<RuntimeTimelineSnapshot>("get_timeline");
  return {
    ...fallback.getTimeline(),
    projectEpoch: 0,
    projectPath: null,
    compatibilityReadOnly: false,
    compatibilityBlockers: [],
  };
}

/** The current session's append-only AI generation audit log (rows + credits
 *  math, persisted as `generation-log.json`). Read-only: the UI never mutates
 *  the log; only the core's generation lifecycle appends. Infallible — a
 *  session with no project yields the empty log. Outside Tauri it resolves to
 *  the honest empty log (no fake data). */
export async function generationLog(): Promise<GenerationLog> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<GenerationLog>("generation_log");
  return { version: 1, entries: [] };
}

function editIdentityArgs(expected: ProjectEditIdentity): Record<string, unknown> {
  return {
    expectedProjectEpoch: expected.projectEpoch,
    expectedTimelineVersion: expected.timelineVersion,
    expectedProjectPath: expected.projectPath,
  };
}

export async function editApply(
  command: EditRequest,
  expected: ProjectEditIdentity,
): Promise<EditResult> {
  await ensureTauri();
  if (invokeImpl) {
    return invokeImpl<EditResult>("edit_apply", {
      command,
      ...editIdentityArgs(expected),
    }).catch((error: unknown) => {
      throw asTauriCommandError(error);
    });
  }
  return fallback.editApply(command);
}

export async function undo(expected: ProjectEditIdentity): Promise<EditResult> {
  await ensureTauri();
  if (invokeImpl) {
    return invokeImpl<EditResult>("undo", editIdentityArgs(expected)).catch((error: unknown) => {
      throw asTauriCommandError(error);
    });
  }
  return fallback.noop("Undo");
}

export async function redo(expected: ProjectEditIdentity): Promise<EditResult> {
  await ensureTauri();
  if (invokeImpl) {
    return invokeImpl<EditResult>("redo", editIdentityArgs(expected)).catch((error: unknown) => {
      throw asTauriCommandError(error);
    });
  }
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

export async function projectNew(path: string | null = null): Promise<RuntimeTimelineSnapshot> {
  await ensureTauri();
  if (invokeImpl) {
    return invokeImpl<RuntimeTimelineSnapshot>("project_new", { path });
  }
  fallback.reset();
  return {
    ...fallback.getTimeline(),
    projectEpoch: 0,
    projectPath: null,
    compatibilityReadOnly: false,
    compatibilityBlockers: [],
  };
}

export async function projectOpen(path: string): Promise<RuntimeTimelineSnapshot> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<RuntimeTimelineSnapshot>("project_open", { path });
  return {
    ...fallback.getTimeline(),
    projectEpoch: 0,
    projectPath: null,
    compatibilityReadOnly: false,
    compatibilityBlockers: [],
  };
}

export async function projectSave(
  path: string | null,
  expectedProjectEpoch: number,
  expectedProjectPath: string,
): Promise<string> {
  await ensureTauri();
  if (invokeImpl) {
    return invokeImpl<string>("project_save", {
      path,
      expectedProjectEpoch,
      expectedProjectPath,
    }).catch((error: unknown) => {
      throw asTauriCommandError(error);
    });
  }
  return path ?? "";
}

/** The default folder new projects save into (`~/Documents/OpenTake`). Empty
 *  string outside Tauri (where the save dialog is unavailable anyway). */
export async function getDefaultProjectDir(): Promise<string> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<string>("get_default_project_dir");
  return "";
}

/** Whether an exact filesystem path already exists in the desktop shell. */
export async function checkPathExists(path: string): Promise<boolean> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<boolean>("check_path_exists", { path });
  return false;
}

export async function sampleProjectMaterialize(slug: string): Promise<string> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<string>("sample_project_materialize", { slug });
  throw new Error("Sample projects require the OpenTake desktop app");
}

export interface HomeProjectEntry {
  path: string;
  name: string;
  createdAt: number;
  openedAt: number;
  modifiedAt: number;
  thumbnailPath?: string | null;
  missing: boolean;
  offline: boolean;
}

export interface LegacyRecentProject {
  path: string;
  openedAt: number;
  createdAt?: number;
  modifiedAt?: number;
  thumbnailPath?: string | null;
}

export async function homeProjectsSync(
  entries: LegacyRecentProject[],
): Promise<HomeProjectEntry[]> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<HomeProjectEntry[]>("home_projects_sync", { entries });
  throw new Error("Recent project sync requires the OpenTake desktop app");
}

export async function homeProjectRegister(path: string, openedAt: number): Promise<void> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<void>("home_project_register", { path, openedAt });
  throw new Error("Recent project registration requires the OpenTake desktop app");
}

export async function homeProjectRemove(path: string): Promise<void> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<void>("home_project_remove", { path });
  throw new Error("Recent project removal requires the OpenTake desktop app");
}

export async function homeProjectTrash(path: string): Promise<void> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<void>("home_project_trash", { path });
  throw new Error("Moving a project to trash requires the OpenTake desktop app");
}

export async function homeProjectReveal(path: string): Promise<void> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<void>("home_project_reveal", { path });
  throw new Error("Revealing a project requires the OpenTake desktop app");
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
// real file on disk (H.264 / H.265 in an .mp4 container, or ProRes 422 in a
// .mov container). The request mirrors the Rust `ExportRequest` DTO verbatim
// (camelCase `outPath`; lowercase enum tags). Outside Tauri there is no
// GPU/ffmpeg, so the wrapper rejects with a friendly error rather than
// silently no-op'ing (an export the user asked for must not quietly do
// nothing). Progress streams via the `"export://progress"` event
// (`onExportProgress`); `cancelExport` requests a mid-encode stop, which
// surfaces back through `exportVideo`'s rejection as `EXPORT_CANCELLED_SENTINEL`.

/** Output codec. `h264`/`h265` require an `.mp4` output path; `prores` requires `.mov`. */
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
  hasAudio: boolean;
}

let nextExportOperationId = 0;

/** Mint a process-local request identity that is carried by start, progress,
 * and cancel IPC. The random UUID is preferred; the sequence fallback remains
 * collision-free for this WebView lifetime. */
export function createExportOperationId(scope: "video" | "save-as"): string {
  const randomId = globalThis.crypto?.randomUUID?.();
  const suffix = randomId ?? `${Date.now().toString(36)}-${++nextExportOperationId}`;
  return `${scope}:${suffix}`;
}

export async function exportVideo(
  req: ExportRequest,
  operationId: string,
): Promise<ExportSummary> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<ExportSummary>("export_video", { req, operationId });
  throw new Error("video export requires the desktop app (GPU + ffmpeg)");
}

/** Stable `Err` string `export_video` rejects with when cancelled mid-encode
 *  (mirror of the Rust `CANCELLED_SENTINEL`). Callers match this exact string
 *  to show a neutral "cancelled" state instead of the failure toast. */
export const EXPORT_CANCELLED_SENTINEL = "export cancelled";

/** Request that the in-flight `export_video` stop at the next frame boundary.
 *  No-op outside Tauri (there is no export to cancel) and a no-op backend-side
 *  when nothing is exporting. */
export async function cancelExport(operationId: string): Promise<void> {
  await ensureTauri();
  if (invokeImpl) await invokeImpl<void>("cancel_export", { operationId });
}

export async function cancelGeneration(jobId: string): Promise<boolean> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<boolean>("generation_cancel", { jobId });
  return false;
}

export async function retryGeneration(
  jobId: string,
  costAuthorized: boolean,
): Promise<{ jobId: string; placeholderAssetIds: string[]; status: string }> {
  await ensureTauri();
  if (invokeImpl) {
    return invokeImpl("generation_retry", { jobId, costAuthorized });
  }
  throw new Error("Generation retry is available only in the desktop app");
}

export type MotionProgressPhase =
  | "validating"
  | "rendering"
  | "encoding"
  | "committing"
  | "complete";

export interface MotionAddRequest {
  code?: string;
  templateId?: "title-card" | "lower-third.glass";
  params?: Record<string, string>;
  startFrame: number;
  durationFrames: number;
  transparent?: boolean;
  trackIndex?: number;
}

export interface MotionCommit {
  clipId: string;
  assetId: string;
  contentHash: string;
  actionName: string;
  output: {
    renderer: string;
    rendererVersion: string;
    outputFile: string;
    fps: number;
    width: number;
    height: number;
    durationFrames: number;
    durationSeconds: number;
    contentHash: string;
  };
}

export async function motionCapability(): Promise<boolean> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<boolean>("motion_capability");
  return false;
}

export async function addMotion(request: MotionAddRequest): Promise<MotionCommit> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<MotionCommit>("motion_add", { request });
  throw new Error("Motion graphics require the desktop app");
}

export async function cancelMotion(): Promise<boolean> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<boolean>("motion_cancel");
  return false;
}

export async function onMotionProgress(
  handler: (phase: MotionProgressPhase) => void,
): Promise<() => void> {
  await ensureTauri();
  if (!listenImpl) return () => {};
  return listenImpl("motion_progress", (event) => {
    if (
      typeof event.payload === "string" &&
      ["validating", "rendering", "encoding", "committing", "complete"].includes(
        event.payload,
      )
    ) {
      handler(event.payload as MotionProgressPhase);
    }
  });
}

/** Progress payload for `"export://progress"`: `done` of `total` frames
 *  composited so far. */
export interface ExportProgress {
  operationId: string;
  done: number;
  total: number;
}

/** Subscribe to the throttled `"export://progress"` event fired by `export_video`
 *  (at most every ~200ms, plus a final 100% emit). Returns an unlisten function;
 *  no-op (no-op unlisten) outside Tauri. */
export async function onExportProgress(
  operationId: string,
  handler: (progress: ExportProgress) => void,
): Promise<() => void> {
  await ensureTauri();
  if (!listenImpl) return () => {};
  return listenImpl("export://progress", (e) => {
    const p = e.payload as
      | { operationId?: string; done?: number; total?: number }
      | undefined;
    if (
      p &&
      p.operationId === operationId &&
      typeof p.done === "number" &&
      typeof p.total === "number"
    ) {
      handler({ operationId: p.operationId, done: p.done, total: p.total });
    }
  });
}

// MARK: - Self-contained `.opentake` bundle export (#29 / upstream `.palmier`)
//
// The self-contained `.opentake` bundle capability is intentionally withdrawn:
// its Rust-owned destination/disclosure workflow is not integrated yet. The
// compatibility types and fail-closed function stay available to the disabled
// UI branch, but no renderer call is sent to an unregistered Tauri command.
// A future secure command may write a bundle containing every
// resolvable media reference is copied inside and the manifest is rewritten to
// bundle-relative paths, so the project opens on any machine (port of upstream
// `ExportService.exportPalmierProject` / `PalmierProjectExporter`). It carries
// the live in-memory timeline / manifest / generation log with no save-first,
// matching upstream. Both interfaces mirror the Rust DTOs verbatim — camelCase
// `outPath` / `copiedInternal` / `totalBytes` (IPC camelCase drift is this
// repo's #1 historical bug). Outside Tauri there is no Rust core / file system,
// so the wrapper rejects with a friendly error rather than silently no-op'ing.

/** One media entry that could not be bundled because its source file was not
 *  found on disk (mirror of Rust `MissingMediaDto`). Kept as a dangling
 *  reference in the exported bundle, exactly as upstream does. */
export interface MissingMedia {
  id: string;
  name: string;
}

/** Summary of a completed `.opentake` bundle export (mirror of Rust
 *  `BundleReportDto`). `missing` lists entries whose source file couldn't be
 *  found so the dialog can surface them while still reporting success. */
export interface BundleReport {
  outPath: string;
  collected: string[];
  copiedInternal: number;
  missing: MissingMedia[];
  totalBytes: number;
}

/** Fail-closed capability gate for the unavailable secure bundle workflow. */
export async function exportBundle(outPath: string): Promise<BundleReport> {
  void outPath;
  throw new Error("secure bundle export is not available in this build");
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

/** Validate and copy one `.cube` into the active project's managed storage. */
export async function importLut(path: string): Promise<LutReference> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<LutReference>("import_lut", { path });
  throw new Error("3D LUT import requires the desktop app");
}

export async function getMedia(): Promise<MediaList> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<MediaList>("get_media");
  return { items: [], folders: [] };
}

/**
 * `toggle_favorite`: copy or remove one project asset in the durable global
 * library and persist its content hash in the project's compatibility mirror.
 * Outside Tauri there is no project, so this resolves to an empty catalog.
 */
interface FavoriteProjectIdentity {
  projectEpoch: number;
  projectPath: string | null;
}

function favoriteProjectArgs(project: FavoriteProjectIdentity): {
  expectedProjectEpoch: number;
  expectedProjectPath: string;
} {
  if (project.projectPath === null) {
    throw new Error("save the project before changing global favorites");
  }
  return {
    expectedProjectEpoch: project.projectEpoch,
    expectedProjectPath: project.projectPath,
  };
}

export async function toggleFavorite(
  assetId: string,
  favorite: boolean,
  project: FavoriteProjectIdentity,
): Promise<MediaList> {
  await ensureTauri();
  if (invokeImpl) {
    return invokeImpl<MediaList>("toggle_favorite", {
      assetId,
      favorite,
      ...favoriteProjectArgs(project),
    });
  }
  return { items: [], folders: [] };
}

export async function syncProjectFavorites(
  legacyAssetIds: string[],
  project: FavoriteProjectIdentity,
): Promise<import("./types").FavoriteSyncResult> {
  await ensureTauri();
  if (invokeImpl) {
    return invokeImpl<import("./types").FavoriteSyncResult>("sync_project_favorites", {
      legacyAssetIds,
      ...favoriteProjectArgs(project),
    });
  }
  return {
    media: { items: [], folders: [] },
    migratedLegacyAssetIds: [],
    failures: [],
  };
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
 * `save_clip_as_media` (#91 §3.5): render one timeline clip — effects, color,
 * text, speed baked in — to a new .mp4 in the project bundle's media/ dir and
 * import it as a fresh asset. This intentionally keeps the original single-clip
 * semantics; marked ranges use the separate `save_range_as_media` contract.
 */
export async function saveClipAsMedia(
  clipId: string,
  operationId: string,
): Promise<MediaList> {
  await ensureTauri();
  if (invokeImpl) {
    return invokeImpl<MediaList>("save_clip_as_media", { clipId, operationId });
  }
  throw new Error("saving a clip as media requires the desktop app");
}

export async function saveRangeAsMedia(
  inFrame: number,
  outFrame: number,
  operationId: string,
): Promise<MediaList> {
  await ensureTauri();
  if (invokeImpl) {
    return invokeImpl<MediaList>("save_range_as_media", {
      request: { inFrame, outFrame, operationId },
    });
  }
  throw new Error("saving a range as media requires the desktop app");
}

// MARK: - Transcription (whisper model + on-device transcribe, #183 + captions)

/** Whether the whisper model is installed. Never downloads. The Captions tab
 *  calls this to decide whether to prompt for a one-time model download.
 *  Outside Tauri there is no backend, so report "not installed". */
export async function transcribeModelStatus(): Promise<ModelStatus> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<ModelStatus>("transcribe_model_status");
  return { installed: false, model: "", bytes: 0 };
}

/** Download the whisper model (idempotent), emitting `transcribe://progress`
 *  events as bytes arrive. Rejects outside Tauri (no backend). */
export async function downloadTranscribeModel(): Promise<ModelStatus> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<ModelStatus>("download_transcribe_model");
  throw new Error("transcription model download requires the desktop app");
}

/** Subscribe to model-download progress (`fraction` in 0..=1). No-op outside Tauri. */
export async function onTranscribeProgress(
  handler: (fraction: number) => void,
): Promise<() => void> {
  await ensureTauri();
  if (!listenImpl) return () => {};
  return listenImpl("transcribe://progress", (e) => {
    const p = e.payload as { fraction?: number } | undefined;
    if (p && typeof p.fraction === "number") handler(p.fraction);
  });
}

// MARK: - On-device AI matting

export async function mattingModelStatus(): Promise<MattingModelStatus> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<MattingModelStatus>("matting_model_status");
  return { installed: false, model: "", bytes: 0, sha256: "" };
}

export async function downloadMattingModel(): Promise<MattingModelStatus> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<MattingModelStatus>("download_matting_model");
  throw new Error("matting model download requires the desktop app");
}

export async function cancelMattingModelDownload(): Promise<boolean> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<boolean>("cancel_matting_model_download");
  return false;
}

export async function onMattingProgress(
  handler: (progress: { fraction: number; downloadedBytes: number; totalBytes: number }) => void,
): Promise<() => void> {
  await ensureTauri();
  if (!listenImpl) return () => {};
  return listenImpl("matting://progress", (event) => {
    const value = event.payload as
      | { fraction?: number; downloadedBytes?: number; totalBytes?: number }
      | undefined;
    if (
      value &&
      typeof value.fraction === "number" &&
      typeof value.downloadedBytes === "number" &&
      typeof value.totalBytes === "number"
    ) {
      handler({
        fraction: value.fraction,
        downloadedBytes: value.downloadedBytes,
        totalBytes: value.totalBytes,
      });
    }
  });
}

export async function generateMatte(
  clipId: string,
  apply: boolean,
  range?: { startFrame?: number; endFrame?: number },
): Promise<GenerateMatteResult> {
  await ensureTauri();
  if (invokeImpl) {
    return invokeImpl<GenerateMatteResult>("advanced_generate_matte", {
      request: {
        clipId,
        apply,
        ...(range?.startFrame == null ? {} : { startFrame: range.startFrame }),
        ...(range?.endFrame == null ? {} : { endFrame: range.endFrame }),
      },
    });
  }
  throw new Error("AI matting requires the desktop app");
}

export async function trackMotion(
  clipId: string,
  region: MotionTrackingRegion,
  range: { startFrame: number; endFrame: number },
  apply: boolean,
): Promise<MotionTrackingResult> {
  await ensureTauri();
  if (invokeImpl) {
    return invokeImpl<MotionTrackingResult>("advanced_track_motion", {
      request: {
        clipId,
        region,
        startFrame: range.startFrame,
        endFrame: range.endFrame,
        apply,
      },
    });
  }
  throw new Error("motion tracking requires the desktop app");
}

export async function removeObject(
  clipId: string,
  apply: boolean,
  range: { startFrame: number; endFrame: number },
): Promise<RemoveObjectResult> {
  await ensureTauri();
  if (invokeImpl) {
    return invokeImpl<RemoveObjectResult>("advanced_remove_object", {
      request: {
        clipId,
        maskId: "primary",
        provider: "local",
        model: "opentake-boundary-fill-v1",
        startFrame: range.startFrame,
        endFrame: range.endFrame,
        apply,
      },
    });
  }
  throw new Error("object removal requires the desktop app");
}

export async function matchColor(
  clipId: string,
  referenceMediaRef: string,
  referenceFrame: number,
  targetFrame: number,
  apply: boolean,
): Promise<MatchColorResult> {
  await ensureTauri();
  if (invokeImpl) {
    return invokeImpl<MatchColorResult>("advanced_match_color", {
      request: {
        clipId,
        referenceMediaRef,
        referenceFrame,
        targetFrame,
        apply,
      },
    });
  }
  throw new Error("color match requires the desktop app");
}

export async function translateCaptions(
  captionClipIds: string[],
  sourceLocale: string,
  targetLocale: string,
  provider: "openai" | "anthropic",
  costAuthorized: boolean,
): Promise<CaptionTranslationResult> {
  await ensureTauri();
  if (invokeImpl) {
    return invokeImpl<CaptionTranslationResult>("advanced_translate_captions", {
      request: {
        captionClipIds,
        sourceLocale: sourceLocale.trim() || "auto",
        targetLocale: targetLocale.trim(),
        provider,
        costAuthorized,
        apply: false,
      },
    });
  }
  throw new Error("caption translation requires the desktop app");
}

export async function applyCaptionTranslationReview(
  result: CaptionTranslationResult["result"],
  changes: CaptionTranslationReviewChange[],
): Promise<GenerateMatteResult> {
  await ensureTauri();
  if (invokeImpl) {
    return invokeImpl<GenerateMatteResult>("advanced_apply_caption_translation_review", {
      request: {
        projectEpoch: result.projectEpoch,
        version: result.version,
        sourceLocale: result.sourceLocale,
        targetLocale: result.targetLocale,
        provider: result.provider,
        model: result.model,
        changes,
      },
    });
  }
  throw new Error("caption translation review requires the desktop app");
}

export async function scriptToVideo(
  segments: ScriptToVideoSegmentInput[],
  apply: boolean,
): Promise<ScriptToVideoResult> {
  await ensureTauri();
  if (invokeImpl) {
    return invokeImpl<ScriptToVideoResult>("advanced_script_to_video", {
      request: { segments, apply },
    });
  }
  throw new Error("script-to-video requires the desktop app");
}

export async function generateAvatar(request: {
  portraitMediaRef: string;
  audioMediaRef: string;
  consentId: string;
  costAuthorized: boolean;
  startFrame?: number;
}): Promise<AvatarGenerationResult> {
  await ensureTauri();
  if (invokeImpl) {
    return invokeImpl<AvatarGenerationResult>("advanced_generate_avatar", { request });
  }
  throw new Error("avatar generation requires the desktop app");
}

export async function cloneVoice(request: {
  action: "enroll" | "generate" | "revoke";
  consentId: string;
  referenceAudioMediaRef?: string;
  voiceId?: string;
  voiceName?: string;
  prompt?: string;
  costAuthorized?: boolean;
}): Promise<VoiceCloneResult> {
  await ensureTauri();
  if (invokeImpl) {
    return invokeImpl<VoiceCloneResult>("advanced_clone_voice", { request });
  }
  throw new Error("voice cloning requires the desktop app");
}

export async function cancelAdvancedWorkflow(): Promise<boolean> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<boolean>("cancel_advanced_workflow");
  return false;
}

/** Transcribe one asset (cached, so repeats are instant). `language` is an
 *  optional BCP-47/ISO-639 hint; omit for auto-detect. Rejects outside Tauri. */
export async function transcribeMedia(
  mediaId: string,
  language?: string,
): Promise<Transcript> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<Transcript>("transcribe_media", { mediaId, language });
  throw new Error("transcription requires the desktop app (whisper)");
}

/** Generate captions for the requested source: transcribe on-device and place
 *  styled caption clips on a fresh top track, as one undoable action. The whole
 *  build (packing/timing/placement) runs in Rust — the SAME pipeline as the
 *  `add_captions` agent tool. Rejects outside Tauri (no whisper backend). */
export async function generateCaptions(
  request: CaptionRequest,
): Promise<GenerateCaptionsResult> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<GenerateCaptionsResult>("generate_captions", { request });
  throw new Error("caption generation requires the desktop app (whisper)");
}

// MARK: - Semantic search (SigLIP2 visual model + index + query, search-wiring)

/** Whether the SigLIP2 visual-search model is installed. Never downloads. The
 *  media panel calls this to decide whether to show the "Smart search" download
 *  affordance. Outside Tauri there is no backend, so report "not installed". */
export async function searchModelStatus(): Promise<SearchModelStatus> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<SearchModelStatus>("search_model_status");
  return { installed: false, model: "", bytes: 0 };
}

/** Download the SigLIP2 model (idempotent), emitting `search://progress` events
 *  as bytes arrive, SHA-256-verified. Rejects outside Tauri (no backend). */
export async function downloadSearchModel(): Promise<SearchModelStatus> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<SearchModelStatus>("download_search_model");
  throw new Error("search model download requires the desktop app");
}

/** Subscribe to search-model-download progress (`fraction` in 0..=1). No-op
 *  outside Tauri. */
export async function onSearchModelProgress(
  handler: (fraction: number) => void,
): Promise<() => void> {
  await ensureTauri();
  if (!listenImpl) return () => {};
  return listenImpl("search://progress", (e) => {
    const p = e.payload as { fraction?: number } | undefined;
    if (p && typeof p.fraction === "number") handler(p.fraction);
  });
}

/** Snapshot how much of the project's video/image media is indexed. Never
 *  indexes. Outside Tauri report an empty/uninstalled state. */
export async function searchIndexStatus(): Promise<SearchIndexStatus> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<SearchIndexStatus>("search_index_status");
  return { modelInstalled: false, indexable: 0, indexed: 0 };
}

/** Index every not-yet-current video/image asset (sampled frames → SigLIP2
 *  embeddings), emitting `search://index` progress. Idempotent. Rejects outside
 *  Tauri or when the model isn't installed. */
export async function searchIndexStart(
  expectedProjectEpoch: number,
  expectedProjectPath: string,
): Promise<SearchIndexStatus> {
  await ensureTauri();
  if (invokeImpl)
    return invokeImpl<SearchIndexStatus>("search_index_start", {
      expectedProjectEpoch,
      expectedProjectPath,
    });
  throw new Error("visual indexing requires the desktop app");
}

/** Subscribe to indexing progress: `completed`/`total` assets + overall
 *  `fraction` (0..=1). No-op outside Tauri. */
export async function onSearchIndexProgress(
  handler: (progress: { completed: number; total: number; fraction: number }) => void,
): Promise<() => void> {
  await ensureTauri();
  if (!listenImpl) return () => {};
  return listenImpl("search://index", (e) => {
    const p = e.payload as
      | { completed?: number; total?: number; fraction?: number }
      | undefined;
    if (
      p &&
      typeof p.completed === "number" &&
      typeof p.total === "number" &&
      typeof p.fraction === "number"
    ) {
      handler({ completed: p.completed, total: p.total, fraction: p.fraction });
    }
  });
}

/** Run the three-group content query — Moments (visual), Spoken (transcript),
 *  Files (name). Visual is best-effort (empty without a model); Spoken + Files
 *  always work, so plain filename filtering is the zero-setup fallback. Outside
 *  Tauri returns empty groups (the panel falls back to its in-memory name filter). */
export async function searchQuery(query: string): Promise<SearchResults> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<SearchResults>("search_query", { query });
  return { moments: [], spoken: [], files: [] };
}

// MARK: - Settings Storage pane (storage_usage / storage_clear)

/** The honest empty usage report used outside Tauri (there is no backend file
 *  system): every category at zero bytes, no fake data. */
const EMPTY_STORAGE_USAGE: StorageUsage = {
  categories: [
    { id: "thumbnails", bytes: 0, path: "" },
    { id: "waveforms", bytes: 0, path: "" },
    { id: "searchIndex", bytes: 0, path: "" },
    { id: "models", bytes: 0, path: "" },
    { id: "other", bytes: 0, path: "" },
  ],
  totalBytes: 0,
  cacheRoot: "",
};

/** Real per-category byte usage for the derived caches (thumbnails, waveforms,
 *  search index, downloaded models, other) plus the cache root path. Read-only,
 *  never mutates anything. Outside Tauri resolves to the honest empty report —
 *  the pane then renders its unsupported state. */
export async function storageUsage(): Promise<StorageUsage> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<StorageUsage>("storage_usage");
  return EMPTY_STORAGE_USAGE;
}

/** Delete ONLY the requested derived caches and return the fresh usage
 *  snapshot. `modelsConfirmed` is the destructive gate for the `models`
 *  category (weights are re-downloads, not lazily-rebuilt caches): the pane
 *  only passes `true` after its explicit confirm step. Project files, the
 *  global media library, user media and credentials are never touched (the
 *  Rust command only operates on the engine-owned cache/models roots).
 *  Outside Tauri there is nothing to clear — resolves to the empty report. */
export async function storageClear(
  categories: StorageCategoryId[],
  modelsConfirmed = false,
): Promise<StorageUsage> {
  await ensureTauri();
  if (invokeImpl) {
    return invokeImpl<StorageUsage>("storage_clear", {
      request: { categories, modelsConfirmed },
    });
  }
  return EMPTY_STORAGE_USAGE;
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

export type PrewarmResult =
  | "queued"
  | "duplicate"
  | "cached"
  | "busy"
  | "staleProject"
  | "cancelled";

const PREWARM_RESULTS = new Set<PrewarmResult>([
  "queued",
  "duplicate",
  "cached",
  "busy",
  "staleProject",
  "cancelled",
]);

export function decodePrewarmResult(value: unknown): PrewarmResult | null {
  return typeof value === "string" && PREWARM_RESULTS.has(value as PrewarmResult)
    ? (value as PrewarmResult)
    : null;
}

/** Queue the project-scoped poster/waveform warm-up owned by the bounded Rust
 * scheduler. The structured admission result lets callers distinguish a cache
 * hit from queued/duplicate/busy work without starting a synchronous decoder.
 * Browser fallback is already cache-safe; transport errors remain best-effort. */
export async function preloadMedia(mediaRef: string): Promise<PrewarmResult | null> {
  await ensureTauri();
  if (!invokeImpl) return "cached";
  try {
    return decodePrewarmResult(await invokeImpl<unknown>("preload_media", { mediaRef }));
  } catch (e) {
    console.warn(`preload_media failed for ${mediaRef}:`, e);
    return null;
  }
}

export type TimelineSpriteStatus =
  | "queued"
  | "running"
  | "partial"
  | "cached"
  | "cancelled"
  | "failed"
  | "busy"
  | "staleProject";

export interface TimelineSpriteResult {
  status: TimelineSpriteStatus;
  thumbnail: ThumbnailResult | null;
}

export async function requestTimelineSprite(
  mediaRef: string,
  opts?: { maxFrames?: number },
): Promise<TimelineSpriteResult | null> {
  await ensureTauri();
  if (!invokeImpl) return null;
  try {
    const args: Record<string, unknown> = { mediaRef };
    if (opts?.maxFrames != null) args.maxFrames = opts.maxFrames;
    return await invokeImpl<TimelineSpriteResult>("request_timeline_sprite", args);
  } catch (error) {
    console.warn(`request_timeline_sprite failed for ${mediaRef}:`, error);
    return null;
  }
}

export async function setTimelineSpriteInteractive(active: boolean): Promise<void> {
  await ensureTauri();
  if (invokeImpl) await invokeImpl<void>("set_timeline_sprite_interactive", { active });
}

// MARK: - Timeline composite preview (#47)
//
// `composite_frame` renders the timeline at a frame on the GPU (wgpu compositor)
// and returns a PNG data URL the Preview paints onto a <canvas>. `maxSize` caps
// the longest side (px); omit for the backend default. Outside Tauri there is no
// GPU/core, so this returns null and the Preview keeps its placeholder.
//
// To honor the Preview Quality badge, callers pass
// `previewQualityMaxSize(previewQualityShortEdge, timeline.width, timeline.height)`
// (lib/previewPresets) as `maxSize`. NOTE (honest scope): the interactive
// timeline preview today is the DOM `<video>` path (TimelinePlaybackLayer),
// which cannot decode-downscale, so the Quality cap only affects composite-based
// paths (this command + capture) and the flagged streaming engine.

/** One composited timeline frame: a PNG data URL plus its pixel size. */
export interface CompositeFrame {
  width: number;
  height: number;
  dataUrl: string;
}

export interface CompositeStillRequest {
  frame: number;
  projectEpoch: number;
  timelineVersion: number;
  sessionId: string;
  sessionGeneration: number;
  seekGeneration: number;
  sequenceId?: string;
}

export interface CancelCompositeStillRequest {
  projectEpoch: number;
  timelineVersion: number;
  sessionId: string;
  sessionGeneration: number;
  minimumSeekGeneration: number;
}

export async function compositeFrame(
  request: CompositeStillRequest,
  maxSize?: number,
): Promise<CompositeFrame | null> {
  await ensureTauri();
  // The backend command takes an `i32`; the playhead accumulates as a float
  // during playback, so floor to the current frame before invoking (a
  // non-integer is rejected/coerced inconsistently by Tauri's deserializer).
  if (invokeImpl)
    return invokeImpl<CompositeFrame>("composite_frame", {
      request: {
        ...request,
        frame: Math.floor(request.frame),
      },
      maxSize,
    });
  return null;
}

export async function cancelCompositeFrame(
  request: CancelCompositeStillRequest,
): Promise<void> {
  await ensureTauri();
  if (invokeImpl) await invokeImpl<void>("cancel_composite_frame", { ...request });
}

/**
 * `capture_frame_to_media`: composite the timeline (or decode a single video
 * asset) at `frame` and import the result as a NEW still into the media library,
 * named `"{nameBase} {frame}"` and placed in `folderId` (current media-panel
 * folder). Pass `sourceMediaId` for the single-clip video preview tab (decodes
 * that asset's own frame); omit it for the timeline tab (composites). Returns the
 * updated media catalog, or null outside Tauri (no compositor). Mirrors upstream
 * `captureCurrentFrameToMedia`.
 */
export async function captureFrameToMedia(
  frame: number,
  nameBase: string,
  folderId: string | null,
  sourceMediaId?: string | null,
): Promise<MediaList | null> {
  await ensureTauri();
  if (invokeImpl)
    return invokeImpl<MediaList>("capture_frame_to_media", {
      frame: Math.floor(frame),
      nameBase,
      folderId: folderId ?? null,
      sourceMediaId: sourceMediaId ?? null,
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

export async function analyzeStabilization(clipId: string): Promise<StabilizationTrack> {
  await ensureTauri();
  if (invokeImpl) {
    return invokeImpl<StabilizationTrack>("analyze_stabilization", { clipId });
  }
  throw new Error("stabilization analysis requires the desktop app");
}

export async function cancelStabilizationAnalysis(): Promise<boolean> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<boolean>("cancel_stabilization_analysis");
  return false;
}

export interface LoudnessProgress {
  clipId: string;
  done: number;
  total: number;
}

export async function analyzeLoudness(
  clipId: string,
  targetLufs: number,
  truePeakCeilingDbtp: number,
): Promise<LoudnessNormalization> {
  await ensureTauri();
  if (invokeImpl) {
    return invokeImpl<LoudnessNormalization>("analyze_loudness", {
      clipId,
      targetLufs,
      truePeakCeilingDbtp,
    });
  }
  throw new Error("loudness analysis requires the desktop app");
}

export async function cancelLoudnessAnalysis(): Promise<boolean> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<boolean>("cancel_loudness_analysis");
  return false;
}

export async function onLoudnessProgress(
  clipId: string,
  handler: (progress: LoudnessProgress) => void,
): Promise<() => void> {
  await ensureTauri();
  if (!listenImpl) return () => {};
  return listenImpl("loudness://progress", (event) => {
    const progress = event.payload as Partial<LoudnessProgress> | undefined;
    if (
      progress?.clipId === clipId &&
      typeof progress.done === "number" &&
      typeof progress.total === "number"
    ) {
      handler(progress as LoudnessProgress);
    }
  });
}

export interface DenoiseProgress {
  clipId: string;
  done: number;
  total: number;
}

export async function prepareDenoise(
  clipId: string,
  mode: DenoiseMode,
  strength: number,
  previewEnabled: boolean,
): Promise<AudioDenoise> {
  await ensureTauri();
  if (invokeImpl) {
    return invokeImpl<AudioDenoise>("prepare_denoise", {
      clipId,
      mode,
      strength,
      previewEnabled,
    });
  }
  throw new Error("audio denoise requires the desktop app");
}

export async function cancelDenoiseAnalysis(): Promise<boolean> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<boolean>("cancel_denoise_analysis");
  return false;
}

export async function onDenoiseProgress(
  clipId: string,
  handler: (progress: DenoiseProgress) => void,
): Promise<() => void> {
  await ensureTauri();
  if (!listenImpl) return () => {};
  return listenImpl("denoise://progress", (event) => {
    const progress = event.payload as Partial<DenoiseProgress> | undefined;
    if (
      progress?.clipId === clipId &&
      typeof progress.done === "number" &&
      typeof progress.total === "number"
    ) {
      handler(progress as DenoiseProgress);
    }
  });
}

export interface StemSeparationProgress {
  sourceAssetId: string;
  done: number;
  total: number;
}

export interface StemSeparationResult {
  vocalsAssetId: string;
  accompanimentAssetId: string;
  sourceSha256: string;
  execution: string;
  modelSha256?: string | null;
  vocalSdrImprovementDb: number;
}

export async function separateAudioStems(
  sourceAssetId: string,
  execution: "local" | "hosted",
  provider: string | null = null,
  model: string | null = null,
  uploadConfirmed = false,
): Promise<StemSeparationResult> {
  await ensureTauri();
  if (invokeImpl) {
    return invokeImpl<StemSeparationResult>("separate_audio_stems", {
      sourceAssetId,
      execution,
      provider,
      model,
      uploadConfirmed,
    });
  }
  throw new Error("stem separation requires the desktop app");
}

export async function cancelStemSeparation(): Promise<boolean> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<boolean>("cancel_stem_separation");
  return false;
}

export interface ImportStemsToTracksResult {
  clipIds: string[];
  actionName: string;
}

export async function importStemsToTracks(
  vocalsAssetId: string,
  accompanimentAssetId: string,
  startFrame: number,
): Promise<ImportStemsToTracksResult> {
  await ensureTauri();
  if (invokeImpl) {
    return invokeImpl<ImportStemsToTracksResult>("import_stems_to_tracks", {
      vocalsAssetId,
      accompanimentAssetId,
      startFrame,
    });
  }
  throw new Error("stem track import requires the desktop app");
}

export interface MediaProxyResult {
  assetId: string;
  path: string;
  sourceSha256: string;
  width: number;
  height: number;
}

export interface MediaProxyProgress {
  assetId: string;
  done: number;
  total: number;
}

export async function createMediaProxy(
  assetId: string,
  maxWidth = 1280,
  maxHeight = 720,
): Promise<MediaProxyResult> {
  await ensureTauri();
  if (invokeImpl) {
    return invokeImpl<MediaProxyResult>("create_media_proxy", { assetId, maxWidth, maxHeight });
  }
  throw new Error("proxy creation requires the desktop app");
}

export async function cancelMediaProxy(): Promise<boolean> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<boolean>("cancel_media_proxy");
  return false;
}

export async function removeMediaProxy(assetId: string): Promise<boolean> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<boolean>("remove_media_proxy", { assetId });
  return false;
}

export async function setProxyPlaybackEnabled(enabled: boolean): Promise<boolean> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<boolean>("set_proxy_playback_enabled", { enabled });
  return enabled;
}

export async function getProxyPlaybackEnabled(): Promise<boolean> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<boolean>("get_proxy_playback_enabled");
  return false;
}

export async function onMediaProxyProgress(
  assetId: string,
  handler: (progress: MediaProxyProgress) => void,
): Promise<() => void> {
  await ensureTauri();
  if (!listenImpl) return () => {};
  return listenImpl("proxy://progress", (event) => {
    const progress = event.payload as Partial<MediaProxyProgress> | undefined;
    if (
      progress?.assetId === assetId &&
      typeof progress.done === "number" &&
      typeof progress.total === "number"
    ) {
      handler(progress as MediaProxyProgress);
    }
  });
}

export async function onStemSeparationProgress(
  sourceAssetId: string,
  handler: (progress: StemSeparationProgress) => void,
): Promise<() => void> {
  await ensureTauri();
  if (!listenImpl) return () => {};
  return listenImpl("stems://progress", (event) => {
    const progress = event.payload as Partial<StemSeparationProgress> | undefined;
    if (
      progress?.sourceAssetId === sourceAssetId &&
      typeof progress.done === "number" &&
      typeof progress.total === "number"
    ) {
      handler(progress as StemSeparationProgress);
    }
  });
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

// MARK: - Official Codex / ChatGPT authentication
//
// OpenTake never receives a ChatGPT token. These commands only ask the
// user-installed official Codex CLI to report, start, cancel, or clear its own
// login session.

export interface CodexAuthStatus {
  available: boolean;
  authenticated: boolean;
  authMethod: string | null;
  version: string | null;
  loginInProgress: boolean;
  message: string;
}

const NO_CODEX: CodexAuthStatus = {
  available: false,
  authenticated: false,
  authMethod: null,
  version: null,
  loginInProgress: false,
  message: "Official Codex CLI is available only in the desktop app",
};

export async function codexAuthStatus(): Promise<CodexAuthStatus> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<CodexAuthStatus>("codex_auth_status");
  return NO_CODEX;
}

export async function codexLoginStart(): Promise<CodexAuthStatus> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<CodexAuthStatus>("codex_login_start");
  return NO_CODEX;
}

export async function codexLoginCancel(): Promise<CodexAuthStatus> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<CodexAuthStatus>("codex_login_cancel");
  return NO_CODEX;
}

export async function codexLogout(): Promise<CodexAuthStatus> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<CodexAuthStatus>("codex_logout");
  return NO_CODEX;
}

// MARK: - Optional account backend
//
// OpenTake has no official backend. Outside Tauri, and before a user stores a
// custom backend origin, this surface remains offline and performs no network
// activity. The plaintext token only crosses the boundary on login and is never
// returned to JavaScript after verification.

export async function accountSetBackendUrl(url: string | null): Promise<void> {
  await ensureTauri();
  if (invokeImpl) await invokeImpl<void>("account_set_backend_url", { url });
}

export async function accountGetBackendUrl(): Promise<string | null> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<string | null>("account_get_backend_url");
  return null;
}

export async function accountLogin(token: string): Promise<AccountInfo> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<AccountInfo>("account_login", { token });
  throw new Error("account login requires the desktop app");
}

export async function accountLogout(): Promise<void> {
  await ensureTauri();
  if (invokeImpl) await invokeImpl<void>("account_logout");
}

export async function accountGetStatus(): Promise<AccountStatus> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<AccountStatus>("account_get_status");
  return { type: "offline" };
}

// MARK: - In-app chat (#HANDOFF-3.3)

export interface ChatDelta {
  projectEpoch: number;
  projectPath: string;
  sessionId: string;
  delta: string;
}

export interface ChatToolCallEvent {
  projectEpoch: number;
  projectPath: string;
  sessionId: string;
  toolCall: ChatToolCall;
}

export interface ChatDoneEvent {
  projectEpoch: number;
  projectPath: string;
  sessionId: string;
  message: ChatMessage;
}

export async function chatSend(
  sessionId: string,
  text: string,
  chatProvider: string,
  expectedProjectEpoch: number,
  expectedProjectPath: string,
): Promise<void> {
  await ensureTauri();
  if (invokeImpl)
    return invokeImpl<void>("chat_send", {
      sessionId,
      text,
      chatProvider,
      expectedProjectEpoch,
      expectedProjectPath,
    });
  throw new Error("chat requires the desktop app (LLM + tool dispatch)");
}

export async function chatHistory(
  sessionId: string,
  expectedProjectEpoch: number,
  expectedProjectPath: string,
): Promise<ChatMessage[]> {
  await ensureTauri();
  if (invokeImpl)
    return invokeImpl<ChatMessage[]>("chat_history", {
      sessionId,
      expectedProjectEpoch,
      expectedProjectPath,
    });
  return [];
}

export async function chatSessions(
  expectedProjectEpoch: number,
  expectedProjectPath: string,
): Promise<ChatSession[]> {
  await ensureTauri();
  if (invokeImpl)
    return invokeImpl<ChatSession[]>("chat_sessions", {
      expectedProjectEpoch,
      expectedProjectPath,
    });
  return [];
}

export async function chatSessionSetOpen(
  sessionId: string,
  isOpen: boolean,
  expectedProjectEpoch: number,
  expectedProjectPath: string,
): Promise<ChatSession> {
  await ensureTauri();
  if (invokeImpl)
    return invokeImpl<ChatSession>("chat_session_set_open", {
      sessionId,
      isOpen,
      expectedProjectEpoch,
      expectedProjectPath,
    });
  throw new Error("chat tabs require the desktop app");
}

export async function chatCancel(
  sessionId: string,
  expectedProjectEpoch: number,
  expectedProjectPath: string,
): Promise<void> {
  await ensureTauri();
  if (invokeImpl)
    await invokeImpl<void>("chat_cancel", {
      sessionId,
      expectedProjectEpoch,
      expectedProjectPath,
    });
}

// MARK: - Events

/** Subscribe to `timeline_changed`. Returns an unlisten function. No-op (and a
 *  no-op unlisten) when not in Tauri. */
export async function onTimelineChanged(
  handler: (projectEpoch: number, version: number) => void,
): Promise<() => void> {
  await ensureTauri();
  if (!listenImpl) return () => {};
  return listenImpl("timeline_changed", (e) => {
    const payload = e.payload as { projectEpoch?: number; version?: number } | undefined;
    if (
      payload &&
      typeof payload.projectEpoch === "number" &&
      typeof payload.version === "number"
    ) {
      handler(payload.projectEpoch, payload.version);
    }
  });
}

export async function onProjectOpened(
  handler: (path: string, projectEpoch: number, version: number) => void,
): Promise<() => void> {
  await ensureTauri();
  if (!listenImpl) return () => {};
  return listenImpl("project_opened", (e) => {
    const p = e.payload as
      | { path?: string; projectEpoch?: number; version?: number }
      | undefined;
    if (
      p &&
      typeof p.path === "string" &&
      typeof p.projectEpoch === "number" &&
      typeof p.version === "number"
    ) {
      handler(p.path, p.projectEpoch, p.version);
    }
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

/** Subscribe to `project_saved` — fired on EVERY bundle write, including
 *  core-internal saves (e.g. the media manifest) that never flow through the
 *  explicit save promises in `projectActions`. The payload carries the written
 *  path and project session, but no document version, so consumers treat it as
 *  a save-completion signal for that session. No-op outside Tauri. */
export async function onProjectSaved(
  handler: (path: string, projectEpoch: number) => void,
): Promise<() => void> {
  await ensureTauri();
  if (!listenImpl) return () => {};
  return listenImpl("project_saved", (e) => {
    const p = e.payload as { path?: string; projectEpoch?: number } | undefined;
    if (p && typeof p.path === "string" && typeof p.projectEpoch === "number") {
      handler(p.path, p.projectEpoch);
    }
  });
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

export async function onChatDelta(
  handler: (event: ChatDelta) => void,
): Promise<() => void> {
  await ensureTauri();
  if (!listenImpl) return () => {};
  return listenImpl("chat_delta", (e) => {
    const payload = e.payload as
      | { projectEpoch?: number; projectPath?: string; sessionId?: string; delta?: string }
      | undefined;
    if (
      payload &&
      typeof payload.projectEpoch === "number" &&
      typeof payload.projectPath === "string" &&
      typeof payload.sessionId === "string" &&
      typeof payload.delta === "string"
    ) {
      handler({
        projectEpoch: payload.projectEpoch,
        projectPath: payload.projectPath,
        sessionId: payload.sessionId,
        delta: payload.delta,
      });
    }
  });
}

export async function onChatToolCall(
  handler: (event: ChatToolCallEvent) => void,
): Promise<() => void> {
  await ensureTauri();
  if (!listenImpl) return () => {};
  return listenImpl("chat_tool_call", (e) => {
    const payload = e.payload as
      | {
          projectEpoch?: number;
          projectPath?: string;
          sessionId?: string;
          toolCall?: ChatToolCall;
        }
      | undefined;
    if (
      payload &&
      typeof payload.projectEpoch === "number" &&
      typeof payload.projectPath === "string" &&
      typeof payload.sessionId === "string" &&
      payload.toolCall
    ) {
      handler({
        projectEpoch: payload.projectEpoch,
        projectPath: payload.projectPath,
        sessionId: payload.sessionId,
        toolCall: payload.toolCall,
      });
    }
  });
}

export async function onChatDone(
  handler: (event: ChatDoneEvent) => void,
): Promise<() => void> {
  await ensureTauri();
  if (!listenImpl) return () => {};
  return listenImpl("chat_done", (e) => {
    const payload = e.payload as
      | {
          projectEpoch?: number;
          projectPath?: string;
          sessionId?: string;
          message?: ChatMessage;
        }
      | undefined;
    if (
      payload &&
      typeof payload.projectEpoch === "number" &&
      typeof payload.projectPath === "string" &&
      typeof payload.sessionId === "string" &&
      payload.message
    ) {
      handler({
        projectEpoch: payload.projectEpoch,
        projectPath: payload.projectPath,
        sessionId: payload.sessionId,
        message: payload.message,
      });
    }
  });
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
export async function playbackStart(
  fromFrame: number,
  identity: PlaybackIdentity,
): Promise<void> {
  await ensureTauri();
  if (invokeImpl)
    await invokeImpl<void>("playback_start", {
      fromFrame: Math.floor(fromFrame),
      identity,
    });
}

/** Pause the matching retained Rust playback session. No-op outside Tauri. */
export async function playbackPause(identity: PlaybackIdentity, frame: number): Promise<void> {
  await ensureTauri();
  if (invokeImpl)
    await invokeImpl<void>("playback_pause", { identity, frame: Math.floor(frame) });
}

/** Stop Rust playback and tear down the engine. No-op outside Tauri. */
export async function playbackStop(identity: PlaybackIdentity): Promise<void> {
  await ensureTauri();
  if (invokeImpl) await invokeImpl<void>("playback_stop", { identity });
}

/** Seek the running Rust engine to `frame` (no-op when not playing / outside Tauri). */
export async function playbackSeek(identity: PlaybackIdentity, frame: number): Promise<void> {
  await ensureTauri();
  if (invokeImpl)
    await invokeImpl<void>("playback_seek", { identity, frame: Math.floor(frame) });
}

/** The loopback `/frame` endpoint used for identity-scoped JPEG requests. */
export async function getPreviewEndpoint(): Promise<string | null> {
  await ensureTauri();
  if (invokeImpl) {
    if (!previewEndpointProbe) {
      let probe: Promise<string | null>;
      probe = invokeImpl<string>("get_preview_endpoint").catch((error: unknown) => {
        if (isTauriCommandNotFound(error, "get_preview_endpoint")) return null;
        if (previewEndpointProbe === probe) previewEndpointProbe = null;
        throw error;
      });
      previewEndpointProbe = probe;
    }
    return previewEndpointProbe;
  }
  return null;
}

/** Subscribe to `playback_frame` (the Rust master clock's current frame). Returns
 *  an unlisten function; no-op (no-op unlisten) outside Tauri. */
export async function onPlaybackFrame(
  handler: (event: PlaybackFrameEvent) => void,
): Promise<() => void> {
  await ensureTauri();
  if (!listenImpl) return () => {};
  return listenImpl("playback_frame", (e) => {
    const event = decodePlaybackFrameEvent(e.payload);
    if (event) handler(event);
  });
}

export function decodePlaybackFrameEvent(payload: unknown): PlaybackFrameEvent | null {
  if (!payload || typeof payload !== "object") return null;
  const event = payload as Partial<PlaybackFrameEvent>;
  if (
    !Number.isSafeInteger(event.projectEpoch) ||
    (event.projectEpoch ?? -1) < 0 ||
    !Number.isSafeInteger(event.timelineVersion) ||
    (event.timelineVersion ?? -1) < 0 ||
    typeof event.sessionId !== "string" ||
    !/^[A-Za-z0-9-]{1,128}$/.test(event.sessionId) ||
    !Number.isSafeInteger(event.frame) ||
    (event.frame ?? -1) < 0 ||
    !Number.isSafeInteger(event.sequence) ||
    (event.sequence ?? -1) < 0 ||
    typeof event.terminal !== "boolean"
  ) {
    return null;
  }
  return event as PlaybackFrameEvent;
}

export function decodePlaybackCommandError(error: unknown): PlaybackCommandError | null {
  if (!error || typeof error !== "object") return null;
  const candidate = error as Partial<PlaybackCommandError>;
  if (
    !["superseded", "cancelled", "busy", "engine"].includes(candidate.code ?? "") ||
    typeof candidate.message !== "string"
  ) {
    return null;
  }
  return candidate as PlaybackCommandError;
}

/** Tauri rejects commands removed by a Cargo feature as plain strings. Keep
 * this narrow so unrelated missing IPC commands cannot silently change the
 * playback route. */
export function isPlaybackCommandUnavailable(error: unknown): boolean {
  const message = tauriErrorMessage(error);
  return (
    message !== null &&
    /^Command (?:get_preview_endpoint|playback_(?:start|pause|stop|seek)) not found$/.test(
      message,
    )
  );
}

function tauriErrorMessage(error: unknown): string | null {
  return typeof error === "string" ? error : error instanceof Error ? error.message : null;
}

function isTauriCommandNotFound(error: unknown, command: string): boolean {
  return tauriErrorMessage(error) === `Command ${command} not found`;
}

// MARK: - Browser fallback (mirror, not authoritative)
//
// When running outside Tauri there is no Rust core; provide a small in-memory
// timeline so the shell renders something. This is intentionally minimal — it
// is a preview aid, not a second editing engine.

import { createFallbackStore } from "./fallback";
const fallback = createFallbackStore();
