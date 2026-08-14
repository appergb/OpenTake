/**
 * TypeScript mirror of the Rust domain model (the read-only timeline mirror).
 * Field names match the Rust serde `camelCase` output verbatim
 * (`opentake-domain`), which is also the `project.json` schema. See
 * docs/modules/web/SPEC.md §12.
 */

export type ClipType = "video" | "audio" | "image" | "text" | "lottie";
export type Interpolation = "linear" | "hold" | "smooth";
export type TransitionKind = "crossDissolve";

export type ExternalMcpListenerState =
  | "disabled"
  | "starting"
  | "listening"
  | "portConflict"
  | "authFailure"
  | "paused";

export interface ExternalMcpClientSummary {
  id: string;
  name: string;
  tokenDigest: string;
  createdAt: number;
  lastUsedAt: number | null;
  revokedAt: number | null;
}

export interface ExternalMcpStatus {
  revision: number;
  enabled: boolean;
  state: ExternalMcpListenerState;
  endpoint: string;
  clients: ExternalMcpClientSummary[];
  error: string | null;
}

export interface ExternalMcpPairingReceipt {
  client: ExternalMcpClientSummary;
  endpoint: string;
  bearerToken: string;
}

export type MotionDocumentFile = "index.html" | "styles.css";

export interface MotionDocumentSummary {
  id: string;
  title: string;
  revisionHash: string;
  updatedAt: number;
}

export interface MotionDocument {
  summary: MotionDocumentSummary;
  html: string;
  css: string;
  parameters: Record<string, unknown>;
}

export interface MotionDocumentCreateRequest {
  title: string | null;
}

export interface MotionTextReplacement {
  /** UTF-8 byte offset; CodeMirror positions are converted before IPC. */
  start: number;
  /** UTF-8 byte offset; CodeMirror positions are converted before IPC. */
  end: number;
  replacement: string;
}

export interface MotionDocumentPatchRequest {
  documentId: string;
  file: MotionDocumentFile;
  baselineHash: string;
  edits: MotionTextReplacement[];
  expectedResultHash: string;
}

export type MotionDocumentHashRequest = Omit<MotionDocumentPatchRequest, "expectedResultHash">;

export interface MotionPublishParameters {
  width: number;
  height: number;
  fps: number;
  durationFrames: number;
}

export interface MotionPreviewRequest extends MotionPublishParameters {
  documentId: string;
  revisionHash: string;
  frame: number;
}

export interface MotionPreviewDiagnostic {
  severity: "error" | "warning";
  message: string;
  line?: number;
  column?: number;
}

export interface MotionPreviewResponse {
  revisionHash: string;
  frame: number;
  pngDataUrl: string;
  diagnostics: MotionPreviewDiagnostic[];
}

export interface Transition {
  fromClipId: string;
  toClipId: string;
  kind: TransitionKind;
  durationFrames: number;
}

export interface Timeline {
  fps: number; // default 30
  width: number; // default 1920
  height: number; // default 1080
  settingsConfigured: boolean;
  nestedSequences?: NestedSequence[];
  scriptAssemblyPlans?: ScriptAssemblyPlan[];
  voiceModels?: VoiceModelRecord[];
  tracks: Track[];
}

export interface VoiceModelRecord {
  id: string;
  provider: string;
  providerVoiceId: string;
  model: string;
  consentId: string;
  sourceAudioAssetId: string;
  sourceAudioSha256: string;
  requestHash: string;
  voiceName: string;
  revoked: boolean;
}

export interface ScriptAssemblySegment {
  script: string;
  mediaRef: string;
  narrationMediaRef?: string;
  durationFrames: number;
  transition?: TransitionKind;
}

export interface ScriptAssemblyPlan {
  id: string;
  planHash: string;
  planner: string;
  plannerVersion: number;
  startFrame: number;
  segments: ScriptAssemblySegment[];
}

export interface NestedSequence {
  id: string;
  name: string;
  timeline: Timeline;
}

export interface Track {
  id: string;
  type: ClipType; // serde rename = "type"
  muted: boolean;
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

export interface HslSecondary {
  hueCenter: number;
  hueWidth: number;
  feather: number;
  hueShift: number;
  saturation: number;
  lightness: number;
}

export interface ColorGrade {
  exposure: number;
  temperature: number;
  tint: number;
  liftGammaGain: LiftGammaGain;
  contrast: number;
  saturation: number;
  hslSecondary?: HslSecondary;
}

export interface LutReference {
  id: string;
  name: string;
  intensity: number;
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

export interface MaskTransform {
  offset: Point2;
  scale: Point2;
  rotationDegrees: number;
}

export interface Mask {
  shape: MaskShape;
  feather: number;
  invert: boolean;
  transform?: MaskTransform;
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
  hslSecondary?: Partial<HslSecondary>;
}

export interface ColorMatchInput {
  referenceMediaRef: string;
  referenceFrame: number;
  targetFrame: number;
  algorithm: string;
  algorithmVersion: number;
  targetMeanLinear: Rgb;
  referenceMeanLinear: Rgb;
  deltaEBefore: number;
  deltaEAfter: number;
  targetLumaBefore: number;
  targetLumaAfter: number;
}

export interface CaptionTranslationInput {
  sourceText: string;
  sourceLocale: string;
  targetLocale: string;
  provider: string;
  model: string;
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
  transform?: MaskTransform;
}

export interface EffectInput {
  name: string;
  params?: Record<string, number>;
  enabled?: boolean;
}

export interface StabilizationKeyframe {
  frame: number;
  translationX: number;
  translationY: number;
  rotationDegrees: number;
}

export interface StabilizationTrack {
  model: string;
  modelVersion: number;
  sourceIdentity: string;
  strength: number;
  cropMargin: number;
  keyframes: StabilizationKeyframe[];
}

export interface LoudnessNormalization {
  targetLufs: number;
  truePeakCeilingDbtp: number;
  inputIntegratedLufs: number;
  inputTruePeakDbtp: number;
  gainDb: number;
  outputIntegratedLufs: number;
  outputTruePeakDbtp: number;
}

export type DenoiseMode = "adaptive" | "voice";

export interface AudioDenoise {
  mode: DenoiseMode;
  strength: number;
  previewEnabled: boolean;
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
  nestedSequenceId?: string;
  textContent?: string;
  textStyle?: TextStyle;
  captionTranslationInput?: CaptionTranslationInput;
  opacityTrack?: KeyframeTrack<number>;
  positionTrack?: KeyframeTrack<AnimPair>;
  scaleTrack?: KeyframeTrack<AnimPair>;
  rotationTrack?: KeyframeTrack<number>;
  cropTrack?: KeyframeTrack<Crop>;
  volumeTrack?: KeyframeTrack<number>;
  loudnessNormalization?: LoudnessNormalization;
  audioDenoise?: AudioDenoise;
  colorGrade?: ColorGrade;
  colorMatchInput?: ColorMatchInput;
  lut?: LutReference;
  chromaKey?: ChromaKey;
  masks?: Mask[];
  effects?: Effect[];
  stabilization?: StabilizationTrack;
  transitionOut?: Transition;
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

export type UnplacedClipEntryReq = Omit<ClipEntryReq, "trackIndex">;

export interface ProjectTimelineSettingsReq {
  fps: number;
  width: number;
  height: number;
}

export type PlaceMediaTargetReq =
  | { kind: "existingTrack"; trackId: string }
  | { kind: "newTrack"; trackType: ClipType; at?: number };

export interface PasteClipEntryReq {
  /** Complete clipboard snapshot. Rust replaces only identity/group mappings,
   * transition endpoints, and the requested timeline start. */
  clip: Clip;
  targetTrackId: string;
  startFrame: number;
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
  | { type: "createNestedSequence"; name: string; clipIds: string[] }
  | { type: "editNestedSequence"; sequenceId: string; command: EditRequest }
  | { type: "renameNestedSequence"; sequenceId: string; name: string }
  | { type: "dissolveNestedSequence"; clipId: string }
  | {
      type: "placeMedia";
      sequenceId?: string;
      settings?: ProjectTimelineSettingsReq;
      target: PlaceMediaTargetReq;
      entry: UnplacedClipEntryReq;
    }
  | { type: "addClips"; entries: ClipEntryReq[] }
  | { type: "insertClips"; trackIndex: number; atFrame: number; entries: ClipEntryReq[] }
  | { type: "moveClips"; moves: ClipMoveReq[] }
  | {
      type: "duplicateClips";
      clipIds: string[];
      offsetFrames: number;
      targetTrackIndexes: number[];
    }
  | {
      type: "moveOrDuplicateClipsToNewTrack";
      clipIds: string[];
      leadClipId: string;
      requestedFrameDelta: number;
      insertAt: number;
      mode: "move" | "duplicate";
    }
  | { type: "pasteClips"; entries: PasteClipEntryReq[] }
  | { type: "removeClips"; clipIds: string[] }
  | { type: "splitClip"; clipId: string; atFrame: number }
  | { type: "splitClips"; clipIds: string[]; atFrame: number }
  | { type: "freezeFrame"; clipId: string; atFrame: number; durationFrames: number }
  | { type: "trimClips"; edits: TrimEditReq[] }
  | { type: "setClipProperties"; clipIds: string[]; properties: ClipPropertiesReq }
  | { type: "setTransformAtFrame"; clipId: string; frame: number; transform: Transform }
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
  | { type: "setLut"; clipIds: string[]; lut?: LutReference | null }
  | { type: "setChromaKey"; clipIds: string[]; chromaKey?: ChromaKeyInput | null }
  | { type: "setMasks"; clipIds: string[]; masks: MaskInput[] }
  | { type: "setEffects"; clipIds: string[]; effects: EffectInput[] }
  | {
      type: "setLoudnessNormalization";
      clipId: string;
      normalization?: LoudnessNormalization | null;
    }
  | {
      type: "setAudioDenoise";
      clipId: string;
      denoise?: AudioDenoise | null;
    }
  | { type: "applyStabilization"; clipId: string; solution: StabilizationTrack }
  | {
      type: "adjustStabilization";
      clipId: string;
      strength?: number;
      cropMargin?: number;
    }
  | { type: "resetStabilization"; clipId: string }
  | {
      type: "setTransition";
      fromClipId: string;
      toClipId: string;
      kind?: TransitionKind | null;
      durationFrames: number;
    }
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

/** Complete optimistic-authority token for an edit/undo/redo IPC request. */
export interface ProjectEditIdentity extends ProjectRevision {
  projectPath: string | null;
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

export interface MattingModelStatus {
  installed: boolean;
  model: string;
  bytes: number;
  sha256: string;
}

export interface MotionTrackingRegion {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface MotionTrackingResult {
  result: {
    clipId: string;
    applied: boolean;
    algorithm: string;
    algorithmVersion: number;
    minimumConfidence: number;
    region: MotionTrackingRegion;
    keyframes: Array<{
      frame: number;
      position: { x: number; y: number };
      interpolation: "linear";
    }>;
  };
  actionName?: string | null;
}

export interface GenerateMatteResult {
  result: {
    clipId: string;
    sourceMediaRef: string;
    assetId?: string | null;
    applied: boolean;
    cacheKey: string;
    previewPath: string;
    frameCount: number;
    width?: number | null;
    height?: number | null;
    fps?: number | null;
    model: string;
    modelSha256: string;
    sourceSha256: string;
    startFrame: number;
    endFrame: number;
  };
  actionName?: string | null;
}

export interface RemoveObjectResult {
  result: {
    clipId: string;
    sourceMediaRef: string;
    assetId?: string | null;
    applied: boolean;
    cacheKey: string;
    previewPath: string;
    frameCount: number;
    width?: number | null;
    height?: number | null;
    fps?: number | null;
    provider: string;
    model: string;
    sourceSha256: string;
    maskIndex: number;
    startFrame: number;
    endFrame: number;
  };
  actionName?: string | null;
}

export interface MatchColorResult {
  result: {
    clipId: string;
    referenceMediaRef: string;
    referenceFrame: number;
    targetFrame: number;
    algorithm: string;
    algorithmVersion: number;
    grade: ColorGrade;
    targetMeanLinear: Rgb;
    referenceMeanLinear: Rgb;
    matchedMeanLinear: Rgb;
    deltaEBefore: number;
    deltaEAfter: number;
    targetLumaBefore: number;
    targetLumaAfter: number;
    applied: boolean;
  };
  actionName?: string | null;
}

export interface CaptionTranslationReviewChange {
  id: string;
  sourceText: string;
  translatedText: string;
}

export interface CaptionTranslationResult {
  result: {
    projectEpoch: number;
    version: number;
    sourceLocale: string;
    targetLocale: string;
    provider: string;
    model: string;
    review: CaptionTranslationReviewChange[];
    errors: Array<{ id: string; message: string }>;
    captionCount: number;
    translatedCount: number;
    applied: boolean;
  };
  actionName?: string | null;
}

export interface ScriptToVideoSegmentInput {
  script: string;
  mediaRef: string;
  narrationMediaRef?: string;
  durationFrames: number;
  transition?: "crossDissolve";
}

export interface ScriptToVideoResult {
  result: {
    projectEpoch: number;
    version: number;
    planId: string;
    planHash: string;
    planner: string;
    plannerVersion: number;
    startFrame: number;
    endFrame: number;
    segments: Array<ScriptToVideoSegmentInput & { startFrame: number }>;
    applied: boolean;
  };
  actionName?: string | null;
}

export interface AvatarGenerationResult {
  result: {
    assetId: string;
    clipIds: string[];
    previewPath: string;
    provider: string;
    model: string;
    providerRequestId: string;
    requestHash: string;
    consentId: string;
    portraitMediaRef: string;
    audioMediaRef: string;
    durationFrames: number;
    mediaType: string;
    imported: boolean;
  };
  actionName?: string | null;
}

export interface VoiceCloneResult {
  result: {
    action: "enroll" | "generate" | "revoke";
    voiceId: string;
    voiceName?: string;
    assetId?: string;
    clipIds?: string[];
    previewPath?: string;
    provider: string;
    model?: string;
    providerRequestId?: string;
    requestHash?: string;
    consentId: string;
    sourceAudioMediaRef?: string;
    sourceAudioSha256?: string;
    durationFrames?: number;
    mediaType?: string;
    imported?: boolean;
    revoked?: boolean;
  };
  actionName?: string | null;
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

// MARK: - Settings Storage pane (mirror of src-tauri storage.rs DTOs)

/** One clearable derived-cache category. The ids match the Rust
 *  `StorageCategoryId` serde tags verbatim: `thumbnails` and `waveforms` split
 *  the shared `MediaVisualCache` dir by file extension, `searchIndex` is the
 *  embedding store, `models` are the downloaded ONNX/ggml weights (re-downloads,
 *  not lazily-rebuilt caches), `other` are the remaining known cache subdirs. */
export type StorageCategoryId =
  | "thumbnails"
  | "waveforms"
  | "searchIndex"
  | "models"
  | "other";

/** Byte usage for one category plus its on-disk root (display only — mirrors
 *  Rust `StorageCategoryUsageDto`). */
export interface StorageCategoryUsage {
  id: StorageCategoryId;
  bytes: number;
  path: string;
}

/** `storage_usage` result (mirror of Rust `StorageUsageDto`): every category is
 *  always present (zero bytes included — the pane needs stable rows), plus the
 *  total and the cache root shown in the pane. */
export interface StorageUsage {
  categories: StorageCategoryUsage[];
  totalBytes: number;
  cacheRoot: string;
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
  /** Probed source frame rate for first-video project configuration. */
  sourceFps?: number | null;
  hasAudio: boolean;
  /** Original video stream color signalling. PQ/HLG is delivered as SDR BT.709
   *  by the current 8-bit preview/export compositor. */
  color?: {
    primaries?: string | null;
    transfer?: string | null;
    matrix?: string | null;
    range?: string | null;
  } | null;
  isHdr?: boolean;
  path?: string | null;
  /** Project-local low-resolution playback media; never used for export. */
  proxyPath?: string | null;
  proxyWidth?: number | null;
  proxyHeight?: number | null;
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
  generationStatus?: "none" | "generating" | "downloading" | "failed" | "cancelled";
  generationProgress?: number | null;
  generationErrorCode?: string | null;
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
  jobId?: string | null;
  provider?: string | null;
  providerJobId?: string | null;
  status?: "queued" | "generating" | "downloading" | "finalizing" | "ready" | "failed" | "cancelled";
  progress?: number | null;
  errorCode?: string | null;
  outputIndex?: number | null;
  sourceAssetId?: string | null;
  sourceClipId?: string | null;
  sourceStartFrame?: number | null;
  sourceEndFrame?: number | null;
  estimatedCostCredits?: number | null;
  consentId?: string | null;
  requestHash?: string | null;
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

/** Stream transport limits. The Rust sender only emits a small ordered block
 * list; reject oversized or malformed addresses before they can retain an
 * unbounded browser-side draft. */
export const MAX_CHAT_STREAM_ID_LENGTH = 512;
export const MAX_CHAT_BLOCK_INDEX = 511;
export const MAX_CHAT_DELTA_CHARS = 32_768;
export const MAX_CHAT_EVENT_SEQUENCE = 1_000_000;
export const MAX_CHAT_EVENT_BYTES = 1_048_576;
export const MAX_CHAT_MESSAGE_CHARS = 1_048_576;
export const MAX_CHAT_IMAGE_BASE64_CHARS = MAX_CHAT_EVENT_BYTES;
export const MAX_CHAT_JSON_DEPTH = 24;
export const MAX_CHAT_JSON_NODES = 8_192;
export const MAX_CHAT_SESSION_SNAPSHOT_BYTES = 8 * 1_048_576;
export const MAX_CHAT_PROJECT_SNAPSHOT_BYTES = 32 * 1_048_576;
export const MAX_CHAT_SESSION_SNAPSHOT_COUNT = 256;

export interface ChatToolCall {
  id: string;
  name: string;
  args: unknown;
  result?: unknown;
  isError?: boolean;
}

export type AgentToolResultContentBlock =
  | { kind: "text"; text: string }
  | { kind: "image"; base64: string; mediaType: string };

export type AgentContentBlock =
  | { type: "text"; text: string }
  | {
      type: "toolUse";
      id: string;
      name: string;
      input: unknown;
      result?: unknown;
      isError?: boolean;
    }
  | {
      type: "toolResult";
      toolUseId: string;
      content: AgentToolResultContentBlock[];
      isError?: boolean;
    };

export interface ChatMessage {
  id: string;
  role: ChatRole;
  content: string;
  toolCalls: ChatToolCall[];
  blocks?: AgentContentBlock[];
  createdAt: number;
  toolCallId?: string;
  toolIsError?: boolean;
}

export interface ChatSession {
  id: string;
  messages: ChatMessage[];
  createdAt: number;
  isOpen: boolean;
  provider?: string;
  model?: string;
}

function hasOwn(value: object, property: string): boolean {
  return Object.prototype.hasOwnProperty.call(value, property);
}

function isPlainDataRecord(value: unknown): value is Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  try {
    const prototype = Object.getPrototypeOf(value);
    if (prototype !== Object.prototype && prototype !== null) return false;
    return Reflect.ownKeys(value).every((key) => {
      if (typeof key !== "string") return false;
      const descriptor = Object.getOwnPropertyDescriptor(value, key);
      return descriptor?.enumerable === true && hasOwn(descriptor, "value");
    });
  } catch {
    return false;
  }
}

function isDenseDataArray(value: unknown): value is unknown[] {
  if (!Array.isArray(value)) return false;
  try {
    if (Reflect.ownKeys(value).length !== value.length + 1) return false;
    for (let index = 0; index < value.length; index += 1) {
      const key = String(index);
      const descriptor = Object.getOwnPropertyDescriptor(value, key);
      if (!descriptor || descriptor.enumerable !== true || !hasOwn(descriptor, "value")) return false;
    }
    return true;
  } catch {
    return false;
  }
}

function codeUnitCompare(left: string, right: string): number {
  return left < right ? -1 : left > right ? 1 : 0;
}

function boundedUtf8ByteLength(value: string, remaining: number): number | null {
  let bytes = 0;
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index);
    if (code <= 0x7f) bytes += 1;
    else if (code <= 0x7ff) bytes += 2;
    else if (code >= 0xd800 && code <= 0xdbff && index + 1 < value.length) {
      const next = value.charCodeAt(index + 1);
      if (next >= 0xdc00 && next <= 0xdfff) {
        bytes += 4;
        index += 1;
      } else {
        bytes += 3;
      }
    } else {
      bytes += 3;
    }
    if (bytes > remaining) return null;
  }
  return bytes;
}

class BoundedCanonicalWriter {
  private readonly chunks: string[] = [];
  private bytes = 0;

  append(value: string): boolean {
    const byteLength = boundedUtf8ByteLength(value, MAX_CHAT_EVENT_BYTES - this.bytes);
    if (byteLength === null) return false;
    this.bytes += byteLength;
    this.chunks.push(value);
    return true;
  }

  finish(): string {
    return this.chunks.join("");
  }
}

function appendJsonString(writer: BoundedCanonicalWriter, value: string): boolean {
  if (!writer.append("\"")) return false;
  let chunk = "";
  const flush = (): boolean => {
    if (chunk.length === 0) return true;
    const accepted = writer.append(chunk);
    chunk = "";
    return accepted;
  };
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index);
    let encoded: string;
    if (code === 0x22) encoded = "\\\"";
    else if (code === 0x5c) encoded = "\\\\";
    else if (code === 0x08) encoded = "\\b";
    else if (code === 0x09) encoded = "\\t";
    else if (code === 0x0a) encoded = "\\n";
    else if (code === 0x0c) encoded = "\\f";
    else if (code === 0x0d) encoded = "\\r";
    else if (code < 0x20 || (code >= 0xd800 && code <= 0xdfff)) {
      const next = index + 1 < value.length ? value.charCodeAt(index + 1) : -1;
      if (code >= 0xd800 && code <= 0xdbff && next >= 0xdc00 && next <= 0xdfff) {
        encoded = value.slice(index, index + 2);
        index += 1;
      } else {
        encoded = `\\u${code.toString(16).padStart(4, "0")}`;
      }
    } else encoded = value[index];
    if (chunk.length + encoded.length > 4_096 && !flush()) return false;
    chunk += encoded;
  }
  return flush() && writer.append("\"");
}

function canonicalizeBoundedChatValue(
  value: unknown,
  allowUndefinedProperties: boolean,
): string | null {
  const writer = new BoundedCanonicalWriter();
  let remainingNodes = MAX_CHAT_JSON_NODES;
  const ancestors = new WeakSet<object>();

  const visit = (candidate: unknown, depth: number): boolean => {
    remainingNodes -= 1;
    if (remainingNodes < 0 || depth > MAX_CHAT_JSON_DEPTH) return false;
    if (candidate === null) return writer.append("null");
    if (typeof candidate === "string") return appendJsonString(writer, candidate);
    if (typeof candidate === "boolean") return writer.append(candidate ? "true" : "false");
    if (typeof candidate === "number") {
      return Number.isFinite(candidate) && writer.append(JSON.stringify(candidate));
    }
    if (Array.isArray(candidate)) {
      if (!isDenseDataArray(candidate) || ancestors.has(candidate) || !writer.append("[")) return false;
      ancestors.add(candidate);
      for (let index = 0; index < candidate.length; index += 1) {
        if ((index > 0 && !writer.append(",")) || !visit(candidate[index], depth + 1)) {
          ancestors.delete(candidate);
          return false;
        }
      }
      ancestors.delete(candidate);
      return writer.append("]");
    }
    if (!isPlainDataRecord(candidate) || ancestors.has(candidate) || !writer.append("{")) return false;
    ancestors.add(candidate);
    const entries = Object.entries(candidate)
      .filter(([, item]) => item !== undefined || !allowUndefinedProperties)
      .sort(([left], [right]) => codeUnitCompare(left, right));
    let emitted = 0;
    for (const [key, item] of entries) {
      if (item === undefined) {
        ancestors.delete(candidate);
        return false;
      }
      if (
        (emitted > 0 && !writer.append(",")) ||
        !appendJsonString(writer, key) ||
        !writer.append(":") ||
        !visit(item, depth + 1)
      ) {
        ancestors.delete(candidate);
        return false;
      }
      emitted += 1;
    }
    ancestors.delete(candidate);
    return writer.append("}");
  };

  try {
    return visit(value, 0) ? writer.finish() : null;
  } catch {
    return null;
  }
}

/** Collision-free canonical JSON retained for exact retry comparison. */
export function canonicalizeBoundedChatEvent(value: unknown): string | null {
  return canonicalizeBoundedChatValue(value, true);
}

export function isBoundedChatId(value: unknown): value is string {
  return typeof value === "string" && value.length > 0 && value.length <= MAX_CHAT_STREAM_ID_LENGTH;
}

interface ChatShapeBudget {
  remaining: number;
}

function takeShapeNode(budget: ChatShapeBudget, depth: number): boolean {
  budget.remaining -= 1;
  return budget.remaining >= 0 && depth <= MAX_CHAT_JSON_DEPTH;
}

function isChatJsonShape(value: unknown, budget: ChatShapeBudget, depth: number): boolean {
  if (!takeShapeNode(budget, depth)) return false;
  if (value === null || typeof value === "boolean") return true;
  if (typeof value === "string") return value.length <= MAX_CHAT_MESSAGE_CHARS;
  if (typeof value === "number") return Number.isFinite(value);
  if (Array.isArray(value)) {
    return isDenseDataArray(value) &&
      value.length <= MAX_CHAT_JSON_NODES &&
      value.every((item) => isChatJsonShape(item, budget, depth + 1));
  }
  if (!isPlainDataRecord(value)) return false;
  const entries = Object.entries(value);
  return entries.length <= MAX_CHAT_JSON_NODES && entries.every(
    ([key, item]) => key.length <= MAX_CHAT_STREAM_ID_LENGTH && isChatJsonShape(item, budget, depth + 1),
  );
}

function isChatToolCallShape(
  value: unknown,
  budget: ChatShapeBudget,
  depth: number,
): value is ChatToolCall {
  if (!takeShapeNode(budget, depth) || !isPlainDataRecord(value)) return false;
  return isBoundedChatId(value.id) &&
    isBoundedChatId(value.name) &&
    hasOwn(value, "args") &&
    isChatJsonShape(value.args, budget, depth + 1) &&
    (value.result === undefined || isChatJsonShape(value.result, budget, depth + 1)) &&
    (value.isError === undefined || typeof value.isError === "boolean");
}

function isAgentContentBlockShape(
  value: unknown,
  budget: ChatShapeBudget,
  depth: number,
): value is AgentContentBlock {
  if (!takeShapeNode(budget, depth) || !isPlainDataRecord(value)) return false;
  if (value.type === "text") {
    return typeof value.text === "string" && value.text.length <= MAX_CHAT_MESSAGE_CHARS;
  }
  if (value.type === "toolUse") {
    return isBoundedChatId(value.id) &&
      isBoundedChatId(value.name) &&
      hasOwn(value, "input") &&
      isChatJsonShape(value.input, budget, depth + 1) &&
      (value.result === undefined || isChatJsonShape(value.result, budget, depth + 1)) &&
      (value.isError === undefined || typeof value.isError === "boolean");
  }
  if (
    value.type !== "toolResult" ||
    !isBoundedChatId(value.toolUseId) ||
    !isDenseDataArray(value.content) ||
    value.content.length > 64 ||
    (value.isError !== undefined && typeof value.isError !== "boolean")
  ) {
    return false;
  }
  return value.content.every((content) => {
    if (!takeShapeNode(budget, depth + 1) || !isPlainDataRecord(content)) return false;
    return (content.kind === "text" &&
      typeof content.text === "string" &&
      content.text.length <= MAX_CHAT_MESSAGE_CHARS) ||
      (content.kind === "image" &&
        typeof content.base64 === "string" &&
        content.base64.length <= MAX_CHAT_IMAGE_BASE64_CHARS &&
        typeof content.mediaType === "string" &&
        content.mediaType.length > 0 &&
        content.mediaType.length <= 128);
  });
}

export function isBoundedChatJson(value: unknown): boolean {
  return isChatJsonShape(value, { remaining: MAX_CHAT_JSON_NODES }, 0) &&
    canonicalizeBoundedChatValue(value, false) !== null;
}

export function isBoundedChatToolCall(value: unknown): value is ChatToolCall {
  return isChatToolCallShape(value, { remaining: MAX_CHAT_JSON_NODES }, 0) &&
    canonicalizeBoundedChatEvent(value) !== null;
}

export function isBoundedAgentContentBlock(value: unknown): value is AgentContentBlock {
  return isAgentContentBlockShape(value, { remaining: MAX_CHAT_JSON_NODES }, 0) &&
    canonicalizeBoundedChatEvent(value) !== null;
}

export function isBoundedAssistantChatMessage(
  value: unknown,
  expectedId: string,
): value is ChatMessage {
  if (!isPlainDataRecord(value)) return false;
  const message = value;
  const budget = { remaining: MAX_CHAT_JSON_NODES };
  if (
    !takeShapeNode(budget, 0) ||
    message.id !== expectedId ||
    !isBoundedChatId(message.id) ||
    message.role !== "assistant" ||
    typeof message.content !== "string" ||
    message.content.length > MAX_CHAT_MESSAGE_CHARS ||
    !Array.isArray(message.toolCalls) ||
    !isDenseDataArray(message.toolCalls) ||
    message.toolCalls.length > MAX_CHAT_BLOCK_INDEX + 1 ||
    !message.toolCalls.every((toolCall) => isChatToolCallShape(toolCall, budget, 1)) ||
    !isDenseDataArray(message.blocks) ||
    message.blocks.length > MAX_CHAT_BLOCK_INDEX + 1 ||
    !message.blocks.every((block) => isAgentContentBlockShape(block, budget, 1)) ||
    typeof message.createdAt !== "number" ||
    !Number.isSafeInteger(message.createdAt) ||
    message.createdAt < 0 ||
    message.toolCallId !== undefined ||
    message.toolIsError !== undefined ||
    canonicalizeBoundedChatEvent(message) === null
  ) {
    return false;
  }
  const blocks = message.blocks as AgentContentBlock[];
  if (blocks.some((block) => block.type === "toolResult")) return false;
  const content = blocks.flatMap((block) => block.type === "text" ? [block.text] : []).join("");
  const toolBlocks = blocks.filter(
    (block): block is Extract<AgentContentBlock, { type: "toolUse" }> => block.type === "toolUse",
  );
  const toolCalls = message.toolCalls as ChatToolCall[];
  return message.content === content &&
    toolBlocks.length === toolCalls.length &&
    toolBlocks.every((block, index) =>
      block.id === toolCalls[index].id && block.name === toolCalls[index].name,
    );
}

export function isBoundedToolChatMessage(
  value: unknown,
  expectedId: string,
): value is ChatMessage {
  if (!isPlainDataRecord(value)) return false;
  const message = value;
  const budget = { remaining: MAX_CHAT_JSON_NODES };
  if (
    !takeShapeNode(budget, 0) ||
    message.id !== expectedId ||
    !isBoundedChatId(message.id) ||
    message.role !== "tool" ||
    typeof message.content !== "string" ||
    message.content.length > MAX_CHAT_MESSAGE_CHARS ||
    !isDenseDataArray(message.toolCalls) ||
    message.toolCalls.length !== 0 ||
    !isDenseDataArray(message.blocks) ||
    message.blocks.length === 0 ||
    message.blocks.length > MAX_CHAT_BLOCK_INDEX + 1 ||
    !message.blocks.every((block) => isAgentContentBlockShape(block, budget, 1)) ||
    typeof message.createdAt !== "number" ||
    !Number.isSafeInteger(message.createdAt) ||
    message.createdAt < 0 ||
    !isBoundedChatId(message.toolCallId) ||
    (message.toolIsError !== undefined && typeof message.toolIsError !== "boolean") ||
    canonicalizeBoundedChatEvent(message) === null
  ) {
    return false;
  }
  const toolCallId = message.toolCallId;
  return (message.blocks as AgentContentBlock[]).every((block) =>
    block.type === "toolResult" &&
    block.toolUseId === toolCallId &&
    block.isError === message.toolIsError,
  );
}

export function isBoundedTerminalChatMessage(
  value: unknown,
  expectedId: string,
): value is ChatMessage {
  return isBoundedAssistantChatMessage(value, expectedId) || isBoundedToolChatMessage(value, expectedId);
}

function isBoundedTextChatMessage(value: unknown): value is ChatMessage {
  if (!isPlainDataRecord(value)) return false;
  const message = value;
  const budget = { remaining: MAX_CHAT_JSON_NODES };
  if (
    !takeShapeNode(budget, 0) ||
    !isBoundedChatId(message.id) ||
    (message.role !== "user" && message.role !== "system") ||
    typeof message.content !== "string" ||
    message.content.length > MAX_CHAT_MESSAGE_CHARS ||
    !isDenseDataArray(message.toolCalls) ||
    message.toolCalls.length !== 0 ||
    !isDenseDataArray(message.blocks) ||
    message.blocks.length > MAX_CHAT_BLOCK_INDEX + 1 ||
    !message.blocks.every((block) => isAgentContentBlockShape(block, budget, 1)) ||
    typeof message.createdAt !== "number" ||
    !Number.isSafeInteger(message.createdAt) ||
    message.createdAt < 0 ||
    message.toolCallId !== undefined ||
    message.toolIsError !== undefined ||
    canonicalizeBoundedChatEvent(message) === null
  ) {
    return false;
  }
  const blocks = message.blocks as AgentContentBlock[];
  return blocks.every((block) => block.type === "text") &&
    message.content === blocks.map((block) => block.type === "text" ? block.text : "").join("");
}

export function isBoundedStoredChatMessage(value: unknown): value is ChatMessage {
  if (!isPlainDataRecord(value) || !isBoundedChatId(value.id)) return false;
  return isBoundedTerminalChatMessage(value, value.id) || isBoundedTextChatMessage(value);
}

function boundedStoredMessageBytes(value: unknown): number | null {
  if (!isBoundedStoredChatMessage(value)) return null;
  const canonical = canonicalizeBoundedChatEvent(value);
  if (canonical === null) return null;
  return boundedUtf8ByteLength(canonical, MAX_CHAT_EVENT_BYTES);
}

function boundedChatHistorySnapshotBytes(value: unknown, limit: number): number | null {
  if (!isDenseDataArray(value) || value.length > MAX_CHAT_JSON_NODES) return null;
  let bytes = 2;
  for (let index = 0; index < value.length; index += 1) {
    const messageBytes = boundedStoredMessageBytes(value[index]);
    if (messageBytes === null) return null;
    bytes += messageBytes + (index > 0 ? 1 : 0);
    if (bytes > limit) return null;
  }
  return bytes;
}

export function isBoundedChatHistorySnapshot(value: unknown): value is ChatMessage[] {
  return boundedChatHistorySnapshotBytes(value, MAX_CHAT_SESSION_SNAPSHOT_BYTES) !== null;
}

function boundedChatSessionSnapshotBytes(value: unknown): number | null {
  if (!isPlainDataRecord(value)) return null;
  if (
    !isBoundedChatId(value.id) ||
    typeof value.createdAt !== "number" ||
    !Number.isSafeInteger(value.createdAt) ||
    value.createdAt < 0 ||
    typeof value.isOpen !== "boolean" ||
    (value.provider !== undefined &&
      (typeof value.provider !== "string" || value.provider.length > MAX_CHAT_STREAM_ID_LENGTH)) ||
    (value.model !== undefined &&
      (typeof value.model !== "string" || value.model.length > MAX_CHAT_STREAM_ID_LENGTH))
  ) {
    return null;
  }
  const historyBytes = boundedChatHistorySnapshotBytes(
    value.messages,
    MAX_CHAT_SESSION_SNAPSHOT_BYTES,
  );
  if (historyBytes === null) return null;
  const envelope = {
    id: value.id,
    messages: [],
    createdAt: value.createdAt,
    isOpen: value.isOpen,
    ...(value.provider === undefined ? {} : { provider: value.provider }),
    ...(value.model === undefined ? {} : { model: value.model }),
  };
  const canonicalEnvelope = canonicalizeBoundedChatEvent(envelope);
  if (canonicalEnvelope === null) return null;
  const envelopeBytes = boundedUtf8ByteLength(canonicalEnvelope, MAX_CHAT_EVENT_BYTES);
  if (envelopeBytes === null) return null;
  const bytes = envelopeBytes - 2 + historyBytes;
  return bytes <= MAX_CHAT_SESSION_SNAPSHOT_BYTES ? bytes : null;
}

export function isBoundedChatSessionsSnapshot(value: unknown): value is ChatSession[] {
  if (!isDenseDataArray(value) || value.length > MAX_CHAT_SESSION_SNAPSHOT_COUNT) return false;
  let bytes = 2;
  for (let index = 0; index < value.length; index += 1) {
    const sessionBytes = boundedChatSessionSnapshotBytes(value[index]);
    if (sessionBytes === null) return false;
    bytes += sessionBytes + (index > 0 ? 1 : 0);
    if (bytes > MAX_CHAT_PROJECT_SNAPSHOT_BYTES) return false;
  }
  return true;
}

// MARK: - AI generation audit log (mirror of opentake_project::gen_log,
// camelCase; optional fields are omitted on the wire when absent)

/** Provider-neutral lifecycle tag of a generation job row. Serialized by
 *  serde's camelCase rename of `opentake_domain::GenerationJobStatus`, which
 *  lower-cases each single-word variant (`Ready` -> `ready`). */
export type GenerationJobStatus =
  | "queued"
  | "generating"
  | "downloading"
  | "finalizing"
  | "ready"
  | "failed"
  | "cancelled";

/** One row in the project AI generation audit log. `createdAt` is
 *  Apple-reference-date seconds (2001-01-01 epoch, upstream Swift `Date`
 *  encoding) — convert with `(createdAt + 978_307_200) * 1000` for a JS
 *  `Date`. `costCredits` is the billed cost in credits; `None` means unknown. */
export interface GenerationLogEntry {
  id: string;
  model: string;
  costCredits?: number;
  createdAt?: number;
  jobId?: string;
  provider?: string;
  providerJobId?: string;
  assetId?: string;
  status?: GenerationJobStatus;
  progress?: number;
  errorCode?: string;
  sourceAssetId?: string;
  sourceClipId?: string;
}

/** The whole log: schema `version` + append-ordered rows. Read-only mirror of
 *  the `generation_log` command; the UI never mutates it. */
export interface GenerationLog {
  version: number;
  entries: GenerationLogEntry[];
}
