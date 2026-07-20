/**
 * TypeScript mirror of the Rust domain model (the read-only timeline mirror).
 * Field names match the Rust serde `camelCase` output verbatim
 * (`opentake-domain`), which is also the `project.json` schema. See
 * docs/modules/web/SPEC.md §12.
 */

export type ClipType = "video" | "audio" | "image" | "text" | "lottie";
export type Interpolation = "linear" | "hold" | "smooth";

export interface Timeline {
  fps: number; // default 30
  width: number; // default 1920
  height: number; // default 1080
  settingsConfigured: boolean;
  tracks: Track[];
}

export interface Track {
  id: string;
  type: ClipType; // serde rename = "type"
  muted: boolean;
  soloed: boolean;
  hidden: boolean;
  syncLocked: boolean; // default true
  clips: Clip[];
  // displayHeight is NOT in JSON — it's a UI-only field (default 50, 32..200).
}

export interface Keyframe<V> {
  frame: number; // clip-relative offset in storage
  value: V;
  interpolationOut: Interpolation; // default smooth
}
export interface KeyframeTrack<V> {
  keyframes: Keyframe<V>[];
}
/** Position (x,y) and scale (w,h) two-component keyframe value. */
export interface AnimPair {
  a: number;
  b: number;
}

export interface Rgba {
  r: number;
  g: number;
  b: number;
  a: number;
}

export type TextAlignment = "left" | "center" | "right";

export interface Shadow {
  enabled: boolean;
  color: Rgba;
  offsetX: number;
  offsetY: number;
  blur: number;
}

export interface Fill {
  enabled: boolean;
  color: Rgba;
}

export interface TextStyle {
  fontName: string;
  fontSize: number;
  fontScale: number;
  color: Rgba;
  alignment: TextAlignment;
  shadow: Shadow;
  background: Fill;
  border: Fill;
}

export interface Transform {
  centerX: number; // default 0.5
  centerY: number; // default 0.5
  width: number; // default 1
  height: number; // default 1
  rotation: number; // degrees, clockwise positive
  flipHorizontal: boolean;
  flipVertical: boolean;
}

export interface Crop {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

export interface Rgb {
  r: number;
  g: number;
  b: number;
}

export interface LiftGammaGain {
  lift: Rgb;
  gamma: Rgb;
  gain: Rgb;
}

export interface ColorGrade {
  exposure: number;
  temperature: number;
  tint: number;
  liftGammaGain: LiftGammaGain;
  contrast: number;
  saturation: number;
}

export interface ChromaKey {
  keyColor: Rgb;
  similarity: number;
  smoothness: number;
  spill: number;
}

export interface Point2 {
  x: number;
  y: number;
}

export type MaskShape =
  | { kind: "linear"; point: Point2; normal: Point2 }
  | { kind: "circle"; center: Point2; radius: Point2 }
  | { kind: "poly"; points: Point2[] };

export interface Mask {
  shape: MaskShape;
  feather: number;
  invert: boolean;
}

export interface Effect {
  name: string;
  params: Record<string, number>;
  enabled: boolean;
}

export interface LiftGammaGainInput {
  lift?: Partial<Rgb>;
  gamma?: Partial<Rgb>;
  gain?: Partial<Rgb>;
}

export interface ColorGradeInput {
  exposure?: number;
  temperature?: number;
  tint?: number;
  liftGammaGain?: LiftGammaGainInput;
  contrast?: number;
  saturation?: number;
}

export interface ChromaKeyInput {
  keyColor?: Partial<Rgb>;
  similarity?: number;
  smoothness?: number;
  spill?: number;
}

export interface MaskInput {
  shape?: MaskShape;
  feather?: number;
  invert?: boolean;
}

export interface EffectInput {
  name: string;
  params?: Record<string, number>;
  enabled?: boolean;
}

export interface Clip {
  id: string;
  mediaRef: string;
  mediaType: ClipType;
  sourceClipType: ClipType;
  startFrame: number;
  durationFrames: number;
  trimStartFrame: number;
  trimEndFrame: number;
  speed: number;
  reversed?: boolean;
  volume: number;
  fadeInFrames: number;
  fadeOutFrames: number;
  fadeInInterpolation: Interpolation;
  fadeOutInterpolation: Interpolation;
  opacity: number;
  transform: Transform;
  crop: Crop;
  linkGroupId?: string;
  captionGroupId?: string;
  textContent?: string;
  textStyle?: TextStyle;
  opacityTrack?: KeyframeTrack<number>;
  positionTrack?: KeyframeTrack<AnimPair>;
  scaleTrack?: KeyframeTrack<AnimPair>;
  rotationTrack?: KeyframeTrack<number>;
  cropTrack?: KeyframeTrack<Crop>;
  volumeTrack?: KeyframeTrack<number>;
  colorGrade?: ColorGrade;
  chromaKey?: ChromaKey;
  masks?: Mask[];
  effects?: Effect[];
}

// MARK: - Command DTOs (mirror src-tauri EditRequest)

export interface ClipEntryReq {
  mediaRef: string;
  mediaType: ClipType;
  sourceClipType: ClipType;
  trackIndex: number;
  startFrame: number;
  durationFrames: number;
  trimStartFrame?: number;
  trimEndFrame?: number;
  hasAudio?: boolean;
  addLinkedAudio?: boolean;
  transform?: Transform;
}

export interface ClipMoveReq {
  clipId: string;
  toTrack: number;
  toFrame: number;
}

export interface TrimEditReq {
  clipId: string;
  trimStartFrame: number;
  trimEndFrame: number;
}

export interface ClipPropertiesReq {
  durationFrames?: number;
  trimStartFrame?: number;
  trimEndFrame?: number;
  speed?: number;
  reversed?: boolean;
  volume?: number;
  opacity?: number;
  transform?: Transform;
  textContent?: string;
  /** Text style for a text clip (font / size / color / alignment / shadow /
   *  background / border). Replaces the clip's whole `textStyle`. */
  textStyle?: TextStyle;
  /** Per-clip crop insets (normalized 0–1). Clears `cropTrack` on the backend. */
  crop?: Crop;
  /** Fade-in length in frames. Clamped to clip duration on the backend. */
  fadeInFrames?: number;
  /** Fade-out length in frames. Clamped to clip duration on the backend. */
  fadeOutFrames?: number;
  fadeInInterpolation?: Interpolation;
  fadeOutInterpolation?: Interpolation;
  /** Writes to `transform.flipHorizontal` on the backend. */
  flipHorizontal?: boolean;
  /** Writes to `transform.flipVertical` on the backend. */
  flipVertical?: boolean;
}

export interface RenameEntryReq {
  id: string;
  name: string;
}

/** Which property a keyframe track targets (mirror of `KeyframeProperty`). */
export type KeyframeProperty =
  | "opacity"
  | "volume"
  | "rotation"
  | "position"
  | "scale"
  | "crop";

/** Keyframe payload, tagged by `kind` (mirror of `KeyframePayloadDto`). Reuses
 *  the shared `Keyframe<V>` / `AnimPair` / `Crop` types above. */
export type KeyframePayloadReq =
  | { kind: "scalar"; keyframes: Keyframe<number>[] }
  | { kind: "pair"; keyframes: Keyframe<AnimPair>[] }
  | { kind: "crop"; keyframes: Keyframe<Crop>[] };

/** An explicit single-value keyframe payload, tagged by `kind` (mirror of
 *  `KeyframeValueDto`). Unlike `KeyframePayloadReq` (a whole replacement
 *  track), this carries just the value to upsert at a given frame. */
export type KeyframeValueReq =
  | { kind: "scalar"; value: number }
  | { kind: "pair"; value: AnimPair }
  | { kind: "crop"; value: Crop };

/** A project-frame range `[start, end)` for ripple delete. */
export interface FrameRangeReq {
  start: number;
  end: number;
}

/** The discriminated union mapped to Rust `EditRequest` (tag = "type"). */
export type EditRequest =
  | { type: "addClips"; entries: ClipEntryReq[] }
  | { type: "insertClips"; trackIndex: number; atFrame: number; entries: ClipEntryReq[] }
  | { type: "moveClips"; moves: ClipMoveReq[] }
  | {
      type: "duplicateClips";
      clipIds: string[];
      offsetFrames: number;
      targetTrackIndexes: number[];
    }
  | { type: "removeClips"; clipIds: string[] }
  | { type: "splitClip"; clipId: string; atFrame: number }
  | { type: "freezeFrame"; clipId: string; atFrame: number; durationFrames: number }
  | { type: "trimClips"; edits: TrimEditReq[] }
  | { type: "setClipProperties"; clipIds: string[]; properties: ClipPropertiesReq }
  | { type: "setKeyframes"; clipId: string; property: KeyframeProperty; payload: KeyframePayloadReq }
  | { type: "stampKeyframe"; clipId: string; property: KeyframeProperty; frame: number }
  | {
      type: "upsertKeyframe";
      clipId: string;
      property: KeyframeProperty;
      frame: number;
      value: KeyframeValueReq;
    }
  | { type: "removeKeyframe"; clipId: string; property: KeyframeProperty; frame: number }
  | { type: "moveKeyframe"; clipId: string; property: KeyframeProperty; fromFrame: number; toFrame: number }
  | { type: "setKeyframeInterpolation"; clipId: string; property: KeyframeProperty; frame: number; interpolation: Interpolation }
  | { type: "setColorGrade"; clipIds: string[]; grade?: ColorGradeInput | null }
  | { type: "setChromaKey"; clipIds: string[]; chromaKey?: ChromaKeyInput | null }
  | { type: "setMasks"; clipIds: string[]; masks: MaskInput[] }
  | { type: "setEffects"; clipIds: string[]; effects: EffectInput[] }
  | { type: "rippleDeleteRanges"; trackIndex: number; ranges: FrameRangeReq[] }
  | { type: "rippleDeleteClips"; clipIds: string[] }
  | { type: "addTexts"; entries: TextEntryReq[] }
  | { type: "addTextsAutoTrack"; entries: TextAutoTrackEntryReq[] }
  | { type: "addCaptions"; entries: CaptionEntryReq[] }
  | { type: "link"; clipIds: string[] }
  | { type: "unlink"; clipIds: string[] }
  | { type: "removeTracks"; trackIndexes: number[] }
  | { type: "swapTracks"; a: number; b: number }
  | { type: "swapClips"; clipA: string; clipB: string }
  | { type: "insertTrack"; kind: ClipType; at?: number }
  | {
      type: "setTrackProps";
      trackIndex: number;
      muted?: boolean;
      hidden?: boolean;
      syncLocked?: boolean;
     }
  | { type: "toggleSolo"; trackIndex: number }
  | { type: "createFolder"; name: string; parentFolderId?: string }
  | { type: "moveToFolder"; assetIds: string[]; folderId?: string }
  | { type: "renameMedia"; entries: RenameEntryReq[] }
  | { type: "renameFolder"; entries: RenameEntryReq[] }
  | { type: "deleteMedia"; assetIds: string[] }
  | { type: "deleteFolder"; folderIds: string[] }
  | { type: "swapMedia"; clipId: string; mediaRef: string }
  | { type: "resetTransform"; clipIds: string[] }
  | { type: "setTimelineSettings"; fps: number; width: number; height: number };

export interface TextEntryReq {
  trackIndex: number;
  startFrame: number;
  durationFrames: number;
  content: string;
  textStyle: TextStyle;
  transform: Transform;
}

/** Like {@link TextEntryReq} minus `trackIndex` — every entry in an
 *  `addTextsAutoTrack` batch lands on the single fresh track the command
 *  creates, so there's nothing to target (mirror of Rust
 *  `TextAutoTrackEntryDto`). */
export interface TextAutoTrackEntryReq {
  startFrame: number;
  durationFrames: number;
  content: string;
  textStyle: TextStyle;
  transform: Transform;
}

/** One built caption clip (mirror of Rust `CaptionEntryDto`). Every caption in a
 *  Generate shares one `captionGroupId`; the whole batch lands on a single fresh
 *  track via `addCaptions`. Multi-word fields MUST be camelCase (the repo's #1
 *  IPC bug class). */
export interface CaptionEntryReq {
  startFrame: number;
  durationFrames: number;
  content: string;
  textStyle: TextStyle;
  transform: Transform;
  captionGroupId: string;
}

export interface EditResult {
  changed: boolean;
  actionName: string;
  affectedClipIds: string[];
  timelineVersion: number;
  summary: string;
}

export interface TimelineSnapshot {
  timeline: Timeline;
  projectEpoch?: number;
  version: number;
}

/** Runtime IPC snapshots always carry project identity. `TimelineSnapshot`
 * keeps the optional field only for the browser fallback fixture schema. */
export interface RuntimeTimelineSnapshot extends TimelineSnapshot {
  projectEpoch: number;
  projectPath: string | null;
  compatibilityReadOnly: boolean;
  compatibilityBlockers: string[];
}

export interface ProjectRevision {
  projectEpoch: number;
  timelineVersion: number;
}

export interface PlaybackIdentity extends ProjectRevision {
  sessionId: string;
}

export interface PlaybackFrameEvent extends PlaybackIdentity {
  frame: number;
  sequence: number;
  terminal: boolean;
}

export type PlaybackCommandErrorCode = "superseded" | "cancelled" | "busy" | "engine";

export interface PlaybackCommandError {
  code: PlaybackCommandErrorCode;
  message: string;
}

// MARK: - Transcription (mirror of src-tauri transcribe.rs DTOs)

/** Whether the whisper transcription model is installed, plus enough to prompt a
 *  one-time download (mirror of Rust `ModelStatusDto`). */
export interface ModelStatus {
  installed: boolean;
  /** Human label, e.g. "base (multilingual)". */
  model: string;
  /** Approximate download size in bytes. */
  bytes: number;
}

/** One transcript word/token with optional source-seconds timing. */
export interface TranscriptWord {
  text: string;
  start?: number;
  end?: number;
}

/** One endpointed transcript segment (sentence/pause boundary), source seconds. */
export interface TranscriptSegment {
  text: string;
  start: number;
  end: number;
}

/** A full transcript for one asset (mirror of Rust `TranscriptDto`). */
export interface Transcript {
  mediaId: string;
  text: string;
  language?: string;
  segments: TranscriptSegment[];
  words: TranscriptWord[];
}

/** Which clips a caption Generate targets (mirror of Rust `CaptionSource`). */
export type CaptionSource =
  | { kind: "auto" }
  | { kind: "track"; trackId: string }
  | { kind: "clips"; clipIds: string[] };

/** Letter case for captions (mirror of Rust `CaptionCaseDto`). */
export type CaptionCase = "auto" | "upper" | "lower";

/** The Captions-tab request (mirror of Rust `CaptionRequestDto`). All fields
 *  optional except `source`; style is the full text style, placement is a
 *  normalized canvas center, language is an optional BCP-47/ISO-639 hint. */
export interface CaptionRequest {
  source: CaptionSource;
  style?: TextStyle;
  centerX?: number;
  centerY?: number;
  textCase?: CaptionCase;
  censorProfanity?: boolean;
  language?: string;
}

/** Outcome of `generate_captions` (mirror of Rust `GenerateCaptionsResult`). */
export interface GenerateCaptionsResult {
  edit: EditResult;
  captionCount: number;
}

// MARK: - Semantic search (mirror of src-tauri search.rs DTOs)

/** Whether the SigLIP2 visual-search model is installed, plus enough to prompt a
 *  one-time download (mirror of Rust `SearchModelStatusDto`). */
export interface SearchModelStatus {
  installed: boolean;
  /** Model identity, e.g. "siglip2-base-patch16-256". */
  model: string;
  /** Approximate combined download size in bytes (image + text encoder + tokenizer). */
  bytes: number;
}

/** Visual-index coverage for the project's video/image assets (mirror of Rust
 *  `SearchIndexStatusDto`). Drives the panel's "index now" affordance + progress. */
export interface SearchIndexStatus {
  /** The model must be installed before anything can be indexed. */
  modelInstalled: boolean;
  /** Count of video/image assets in the project. */
  indexable: number;
  /** How many already have a current on-disk embedding index. */
  indexed: number;
}

/** One visual ("Moments") hit. `frame` is the shot-start in **source frames**
 *  (thumb + preview anchor); `startSec`/`endSec` are the source-second range used
 *  to drag a trimmed clip onto the timeline (mirror of Rust `MomentHitDto`). */
export interface MomentHit {
  mediaId: string;
  frame: number;
  startSec: number;
  endSec: number;
  score: number;
  /** True for still images (no time range → drag as a plain asset). */
  isImage: boolean;
}

/** One spoken ("Spoken") transcript hit (mirror of Rust `SpokenHitDto`). */
export interface SpokenHit {
  mediaId: string;
  startSec: number;
  endSec: number;
  text: string;
  score: number;
}

/** One filename ("Files") match (mirror of Rust `FileHitDto`). */
export interface FileHit {
  mediaId: string;
  score: number;
}

/** The three-group query result: Moments (visual), Spoken (transcript), Files
 *  (name), ranked independently and never blended (mirror of Rust
 *  `SearchResultsDto`). */
export interface SearchResults {
  moments: MomentHit[];
  spoken: SpokenHit[];
  files: FileHit[];
}

// MARK: - Media catalog (mirror of src-tauri MediaItemDto / MediaListDto)

/** One media-library item as returned by `get_media` / `import_*`. `type` is the
 *  serde-renamed `kind`; `duration` is in seconds; `path` is the resolvable
 *  source path; `thumbnail` is an on-disk generated thumbnail path when
 *  available. */
export interface MediaItem {
  id: string;
  name: string;
  type: ClipType;
  duration: number;
  width?: number | null;
  height?: number | null;
  hasAudio: boolean;
  path?: string | null;
  thumbnail?: string | null;
  /** Library folder this asset lives in (`null`/absent = root). */
  folderId?: string | null;
  /** Source file size in bytes when the file resolves on disk (Inspector Source
   *  → File "Size" row). `null`/absent for missing/unresolvable sources. */
  fileSize?: number | null;
  /** Generation snapshot for an AI-generated asset; `null`/absent for imported
   *  or user assets. Drives the Inspector Source → Generated/Prompt/References
   *  sections (mirror of `MediaItemDto.generationInput` / upstream
   *  `MediaAsset.generationInput`). */
  generationInput?: GenerationInput | null;
  /** `true` when the source file is offline (moved/deleted). Derived from file
   *  existence on the backend; clears after a successful relink. */
  missing?: boolean;
  /** Project-side compatibility mirror for the asset's global favorite mapping.
   *  The Mine grid reads the global library; cards receive this state from
   *  `get_media` / `toggle_favorite`. */
  favorite: boolean;
}

/** Generation input snapshot carried by an AI-generated media asset (mirror of
 *  the Rust `GenerationInput` DTO). Only the fields the Inspector reads are
 *  typed; the backend may carry more. The `assetId` reference arrays resolve to
 *  library items for the References section. */
export interface GenerationInput {
  prompt: string;
  model: string;
  duration: number;
  aspectRatio: string;
  resolution?: string | null;
  imageURLAssetIds?: string[] | null;
  referenceImageAssetIds?: string[] | null;
  referenceVideoAssetIds?: string[] | null;
  referenceAudioAssetIds?: string[] | null;
}

/** A media-library folder (flat list; nest via `parentFolderId`). */
export interface MediaFolder {
  id: string;
  name: string;
  parentFolderId?: string | null;
}

export interface MediaList {
  items: MediaItem[];
  folders: MediaFolder[];
  /** File names dropped during the import that produced this list because their
   *  type is not importable. Empty for plain listing / relink; only `import_*`
   *  populates it so the panel can toast the skips (mirrors upstream
   *  `mediaPanelToast`) instead of dropping them silently. Optional because the
   *  browser-fallback catalogs omit it. */
  skipped?: string[];
}

export interface FavoriteSyncFailure {
  assetId: string;
  message: string;
}

export interface FavoriteSyncResult {
  media: MediaList;
  migratedLegacyAssetIds: string[];
  failures: FavoriteSyncFailure[];
}

// MARK: - Folder browser (剪映-style nested folder browser, #49)

/** One visible entry in a browsed directory, returned by `list_folder`. `isDir`
 *  is `true` for directories; `mediaType` is `"video" | "audio" | "image"` for
 *  importable media files and `undefined` for directories (non-media files are
 *  filtered out by the backend). */
export interface FolderEntry {
  name: string;
  path: string;
  isDir: boolean;
  mediaType?: string;
}

// MARK: - BYOK secret store (mirror of src-tauri SecretStatus)

/** Masked status of a provider's stored API key. The plaintext key never
 *  crosses the Tauri boundary: `secret_load` / `secret_save` / `secret_delete`
 *  return only `hasKey` and a bullet-`masked` form (last 4 chars revealed). */
export interface SecretStatus {
  hasKey: boolean;
  masked: string;
}

// MARK: - Optional account backend (mirror of src-tauri account.rs)

/** Identity returned by a configured backend's `/api/auth/verify` endpoint. */
export interface AccountInfo {
  userId: string;
  email?: string | null;
  plan?: string | null;
}

/** Live, informational login state. `stored` means a credential exists but this
 * process has not made an automatic network request to restore identity. */
export type AccountStatus =
  | { type: "offline" }
  | { type: "stored" }
  | { type: "connecting" }
  | { type: "online"; info: AccountInfo }
  | { type: "error"; message: string };

// MARK: - In-app chat (mirror of opentake-agent::chat::session, camelCase)

export type ChatRole = "system" | "user" | "assistant" | "tool";

export interface ChatToolCall {
  id: string;
  name: string;
  args: unknown;
  result?: unknown;
  isError?: boolean;
}

export interface ChatMessage {
  id: string;
  role: ChatRole;
  content: string;
  toolCalls: ChatToolCall[];
  createdAt: number;
  toolCallId?: string;
}

// MARK: - Streaming PCM chunk (mirror of src-tauri PcmChunkDto, #160)

/** One chunk of decoded PCM audio for streaming playback (#160). Mirrors the
 *  Rust `PcmChunkDto` (camelCase serde). `samples` is interleaved f32 —
 *  `samples[f * channels + c]` — so stereo stays stereo (no mono averaging,
 *  unlike the waveform pipeline). Outside Tauri `decodePcmChunk` returns an
 *  empty chunk (no Rust core to decode with). */
export interface PcmChunkData {
  /** Interleaved f32 samples (length = `frameCount * channels`). */
  samples: number[];
  sampleRate: number;
  channels: number;
  /** Per-channel sample count (independent of channel count). */
  frameCount: number;
}

// MARK: - Motion graphics (mirror of opentake-motion types, Issue #34)
//
// These mirror the Rust `opentake-motion` serde output. `MotionSource` and
// `MotionParamValue` use snake_case (matching the motion crate's serde attrs);
// `MotionRenderParams` / `MotionRenderResult` use camelCase (matching the
// src-tauri DTO's `rename_all = "camelCase"`).

/** A typed parameter value for a motion template (mirror of Rust `ParamValue`).
 *  Internally tagged: `tag = "type"`, `content = "value"`. */
export type MotionParamValue =
  | { type: "string"; value: string }
  | { type: "number"; value: number }
  | { type: "bool"; value: boolean }
  | { type: "color"; value: string };

/** Where a motion graphic's animation comes from (mirror of Rust
 *  `MotionSource`). Externally-tagged enum: `{"code": {...}}` or
 *  `{"template": {...}}`.
 *
 *  - `code` — inline, self-contained HTML/CSS/JS document. The renderer injects
 *    a deterministic clock; authors animate against `OpenTake.seek(seconds)`.
 *  - `template` — instantiate a registered template by id with bound params. */
export type MotionSource =
  | { code: { html_css_js: string } }
  | { template: { id: string; params?: Record<string, MotionParamValue> } };

/** Render parameters for a motion clip (mirror of `MotionRenderParamsDto`).
 *  All fields map 1:1 to the Rust DTO (camelCase serde). */
export interface MotionRenderParams {
  /** Timeline frames per second. */
  fps: number;
  /** Number of frames to produce. */
  durationFrames: number;
  /** Canvas width in pixels. */
  width: number;
  /** Canvas height in pixels. */
  height: number;
  /** Whether to capture a straight-alpha overlay (transparent body). Defaults
 *  to `false` when omitted (the backend treats absent as `false`). */
  transparent?: boolean;
}

/** The full result of `render_motion_clip` (mirror of `MotionRenderResultDto`).
 *  `renderMotion` returns just the `mediaRef` string; this type is exported for
 *  callers that need the metadata (e.g. to check which renderer was used). */
export interface MotionRenderResult {
  /** The `motion://<contentHash>` media_ref to assign to a clip's `mediaRef`. */
  mediaRef: string;
  /** The content hash (SHA-256 hex) naming the cache directory. */
  contentHash: string;
  /** Number of frames rendered (== `durationFrames`). */
  frameCount: number;
  /** Canvas width in pixels. */
  width: number;
  /** Canvas height in pixels. */
  height: number;
  /** Whether the frames carry straight alpha. */
  transparent: boolean;
  /** Which renderer produced the frames: `"cache"` (hit), `"stub"`, or
   *  `"chromium"`. */
  renderer: string;
}
