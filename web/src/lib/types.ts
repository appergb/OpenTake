/**
 * TypeScript mirror of the Rust domain model (the read-only timeline mirror).
 * Field names match the Rust serde `camelCase` output verbatim
 * (`opentake-domain`), which is also the `project.json` schema. See
 * docs/specs/frontend-UI-1to1-SPEC.md §12.
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
  textStyle?: unknown;
  opacityTrack?: KeyframeTrack<number>;
  positionTrack?: KeyframeTrack<AnimPair>;
  scaleTrack?: KeyframeTrack<AnimPair>;
  rotationTrack?: KeyframeTrack<number>;
  cropTrack?: KeyframeTrack<Crop>;
  volumeTrack?: KeyframeTrack<number>;
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
  volume?: number;
  opacity?: number;
  transform?: Transform;
  textContent?: string;
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
  | { type: "trimClips"; edits: TrimEditReq[] }
  | { type: "setClipProperties"; clipIds: string[]; properties: ClipPropertiesReq }
  | { type: "setKeyframes"; clipId: string; property: KeyframeProperty; payload: KeyframePayloadReq }
  | { type: "stampKeyframe"; clipId: string; property: KeyframeProperty; frame: number }
  | { type: "removeKeyframe"; clipId: string; property: KeyframeProperty; frame: number }
  | { type: "moveKeyframe"; clipId: string; property: KeyframeProperty; fromFrame: number; toFrame: number }
  | { type: "setKeyframeInterpolation"; clipId: string; property: KeyframeProperty; frame: number; interpolation: Interpolation }
  | { type: "rippleDeleteRanges"; trackIndex: number; ranges: FrameRangeReq[] }
  | { type: "rippleDeleteClips"; clipIds: string[] }
  | { type: "addTexts"; entries: TextEntryReq[] }
  | { type: "link"; clipIds: string[] }
  | { type: "unlink"; clipIds: string[] }
  | { type: "removeTracks"; trackIndexes: number[] }
  | { type: "insertTrack"; kind: ClipType }
  | {
      type: "setTrackProps";
      trackIndex: number;
      muted?: boolean;
      hidden?: boolean;
      syncLocked?: boolean;
    }
  | { type: "createFolder"; name: string; parentFolderId?: string }
  | { type: "moveToFolder"; assetIds: string[]; folderId?: string }
  | { type: "swapMedia"; clipId: string; mediaRef: string };

export interface TextEntryReq {
  trackIndex: number;
  startFrame: number;
  durationFrames: number;
  content: string;
  textStyle: unknown;
  transform: Transform;
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
  version: number;
}

// MARK: - Media catalog (mirror of src-tauri MediaItemDto / MediaListDto)

/** One media-library item as returned by `get_media` / `import_*`. `type` is the
 *  serde-renamed `kind`; `duration` is in seconds; `path` is the resolvable
 *  source path; `thumbnail` is an on-disk thumbnail path (currently always
 *  null — the panel renders a type placeholder). */
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
  /** `true` when the source file is offline (moved/deleted). Derived from file
   *  existence on the backend; clears after a successful relink. */
  missing?: boolean;
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
}

// MARK: - BYOK secret store (mirror of src-tauri SecretStatus)

/** Masked status of a provider's stored API key. The plaintext key never
 *  crosses the Tauri boundary: `secret_load` / `secret_save` / `secret_delete`
 *  return only `hasKey` and a bullet-`masked` form (last 4 chars revealed). */
export interface SecretStatus {
  hasKey: boolean;
  masked: string;
}
