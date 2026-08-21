/**
 * Gesture -> EditCommand mapping (SPEC §11.1). Every editing action funnels
 * through `editApply`; after a successful change we force a mirror refresh so
 * the browser fallback (which emits no event) and Tauri behave identically.
 */

import * as api from "../lib/api";
import { isTauri } from "../lib/api";
import { forceRefresh } from "./sync";
import { useEditorUiStore } from "./uiStore";
import { useProjectStore } from "./projectStore";
import { refreshMedia } from "./mediaStore";
import { fitTransformForMedia, trimToPlayheadEdits } from "../lib/clip";
import type { TrackDropTarget } from "../lib/geometry";
import { validRange, type TimelineRange } from "../lib/timelineRange";
import { planNudge } from "../lib/timelineNudge";
import { buildInsertPlan, type InsertPlan } from "../lib/timelineInsert";
import { checkProjectSettings } from "../lib/projectSettings";
import { expandLinkGroup } from "../components/timeline/hitTest";
import { useClipboardStore } from "./clipboardStore";
import { t } from "../i18n";
import type {
  CaptionEntryReq,
  CaptionRequest,
  AudioDenoise,
  Clip,
  ClipEntryReq,
  ClipMoveReq,
  ClipPropertiesReq,
  ClipType,
  ChromaKeyInput,
  ColorGradeInput,
  Crop,
  EffectInput,
  EditResult,
  EditRequest,
  FrameRangeReq,
  Interpolation,
  KeyframePayloadReq,
  KeyframeProperty,
  KeyframeValueReq,
  MaskInput,
  LutReference,
  LoudnessNormalization,
  DenoiseMode,
  MediaItem,
  PasteClipEntryReq,
  PlaceMediaTargetReq,
  ProjectTimelineSettingsReq,
  ProjectEditIdentity,
  StabilizationTrack,
  RenameEntryReq,
  TextEntryReq,
  TextAutoTrackEntryReq,
  TextStyle,
  Timeline,
  Transform,
  TransitionKind,
  TrimEditReq,
  UnplacedClipEntryReq,
} from "../lib/types";

/**
 * Maintained UI-gesture → wire DTO → Rust command inventory. The exhaustiveness
 * sentinel below makes a newly-added `EditRequest` variant a compile failure
 * until its production gesture route is recorded here.
 */
export const EDIT_GESTURE_COMMAND_MATRIX = [
  { gesture: "group selected clips", action: "createNestedSequence", requestType: "createNestedSequence", backend: "CreateNestedSequenceFromClips" },
  { gesture: "edit inside compound", action: "editNestedSequence", requestType: "editNestedSequence", backend: "EditNestedSequence" },
  { gesture: "rename compound", action: "renameNestedSequence", requestType: "renameNestedSequence", backend: "RenameNestedSequence" },
  { gesture: "dissolve compound", action: "dissolveNestedSequence", requestType: "dissolveNestedSequence", backend: "DissolveNestedSequence" },
  { gesture: "atomic media placement", action: "placeMedia", requestType: "placeMedia", backend: "PlaceMedia" },
  { gesture: "media overwrite drop", action: "addClips", requestType: "addClips", backend: "AddClips" },
  { gesture: "media ripple insert", action: "insertClips", requestType: "insertClips", backend: "InsertClips" },
  { gesture: "clip drag commit", action: "moveClips", requestType: "moveClips", backend: "MoveClips" },
  { gesture: "option drag copy", action: "duplicateClips", requestType: "duplicateClips", backend: "DuplicateClips" },
  { gesture: "new-track move or copy", action: "moveOrDuplicateClipsToNewTrack", requestType: "moveOrDuplicateClipsToNewTrack", backend: "MoveOrDuplicateClipsToNewTrack" },
  { gesture: "clipboard deep paste", action: "pasteClipsAtPlayhead", requestType: "pasteClips", backend: "PasteClips" },
  { gesture: "delete selection", action: "removeClips", requestType: "removeClips", backend: "RemoveClips" },
  { gesture: "single razor split", action: "splitClip", requestType: "splitClip", backend: "SplitClip" },
  { gesture: "playhead multi-split", action: "splitAtPlayhead", requestType: "splitClips", backend: "SplitClips" },
  { gesture: "freeze frame", action: "freezeFrame", requestType: "freezeFrame", backend: "FreezeFrame" },
  { gesture: "trim handle commit", action: "trimClips", requestType: "trimClips", backend: "TrimClips" },
  { gesture: "inspector property commit", action: "setClipProperties", requestType: "setClipProperties", backend: "SetClipProperties" },
  { gesture: "canvas transform commit", action: "setTransformAtFrame", requestType: "setTransformAtFrame", backend: "SetTransformAtFrame" },
  { gesture: "replace keyframe lane", action: "setKeyframes", requestType: "setKeyframes", backend: "SetKeyframes" },
  { gesture: "stamp keyframe", action: "stampKeyframe", requestType: "stampKeyframe", backend: "StampKeyframe" },
  { gesture: "write animated value", action: "upsertKeyframe", requestType: "upsertKeyframe", backend: "UpsertKeyframe" },
  { gesture: "delete keyframe", action: "removeKeyframe", requestType: "removeKeyframe", backend: "RemoveKeyframe" },
  { gesture: "drag keyframe", action: "moveKeyframe", requestType: "moveKeyframe", backend: "MoveKeyframe" },
  { gesture: "keyframe interpolation menu", action: "setKeyframeInterpolation", requestType: "setKeyframeInterpolation", backend: "SetKeyframeInterpolation" },
  { gesture: "color grade commit", action: "setColorGrade", requestType: "setColorGrade", backend: "SetColorGrade" },
  { gesture: "LUT select, intensity, or remove", action: "setLut", requestType: "setLut", backend: "SetLut" },
  { gesture: "chroma key commit", action: "setChromaKey", requestType: "setChromaKey", backend: "SetChromaKey" },
  { gesture: "mask commit", action: "setMasks", requestType: "setMasks", backend: "SetMasks" },
  { gesture: "effect chain commit", action: "setEffects", requestType: "setEffects", backend: "SetEffects" },
  { gesture: "loudness analysis apply or reset", action: "setLoudnessNormalization", requestType: "setLoudnessNormalization", backend: "SetLoudnessNormalization" },
  { gesture: "denoise apply, preview toggle, or reset", action: "setAudioDenoise", requestType: "setAudioDenoise", backend: "SetAudioDenoise" },
  { gesture: "stabilization analysis apply", action: "analyzeAndApplyStabilization", requestType: "applyStabilization", backend: "ApplyStabilization" },
  { gesture: "stabilization strength or crop", action: "adjustStabilization", requestType: "adjustStabilization", backend: "AdjustStabilization" },
  { gesture: "stabilization reset", action: "resetStabilization", requestType: "resetStabilization", backend: "ResetStabilization" },
  { gesture: "transition commit", action: "setTransition", requestType: "setTransition", backend: "SetTransition" },
  { gesture: "ripple delete range", action: "rippleDeleteRanges", requestType: "rippleDeleteRanges", backend: "RippleDeleteRanges" },
  { gesture: "shift-delete clips", action: "rippleDeleteClips", requestType: "rippleDeleteClips", backend: "RippleDeleteClips" },
  { gesture: "place text batch", action: "addTexts", requestType: "addTexts", backend: "AddTexts" },
  { gesture: "toolbar text", action: "addTextsAutoTrack", requestType: "addTextsAutoTrack", backend: "AddTextsAutoTrack" },
  { gesture: "generate captions", action: "addCaptions", requestType: "addCaptions", backend: "AddCaptions" },
  { gesture: "link selection", action: "linkClips", requestType: "link", backend: "Link" },
  { gesture: "unlink selection", action: "unlinkClips", requestType: "unlink", backend: "Unlink" },
  { gesture: "delete tracks", action: "removeTracks", requestType: "removeTracks", backend: "RemoveTracks" },
  { gesture: "move track row", action: "swapTracks", requestType: "swapTracks", backend: "SwapTracks" },
  { gesture: "exchange clip positions", action: "swapClips", requestType: "swapClips", backend: "SwapClips" },
  { gesture: "insert track drop zone", action: "insertTrack", requestType: "insertTrack", backend: "InsertTrack" },
  { gesture: "track header toggle", action: "setTrackProps", requestType: "setTrackProps", backend: "SetTrackProps" },
  { gesture: "create media folder", action: "createFolder", requestType: "createFolder", backend: "CreateFolder" },
  { gesture: "move media to folder", action: "moveToFolder", requestType: "moveToFolder", backend: "MoveToFolder" },
  { gesture: "rename media", action: "renameMedia", requestType: "renameMedia", backend: "RenameMedia" },
  { gesture: "rename folder", action: "renameFolder", requestType: "renameFolder", backend: "RenameFolder" },
  { gesture: "delete media", action: "deleteMedia", requestType: "deleteMedia", backend: "DeleteMedia" },
  { gesture: "delete media folder", action: "deleteFolder", requestType: "deleteFolder", backend: "DeleteFolder" },
  { gesture: "swap source media", action: "swapMedia", requestType: "swapMedia", backend: "SwapMedia" },
  { gesture: "reset transform", action: "resetTransform", requestType: "resetTransform", backend: "ResetTransform" },
  { gesture: "project settings commit", action: "setTimelineSettings", requestType: "setTimelineSettings", backend: "SetTimelineSettings" },
] as const satisfies ReadonlyArray<{
  gesture: string;
  action: string;
  requestType: EditRequest["type"];
  backend: string;
}>;

type RoutedEditRequestType = (typeof EDIT_GESTURE_COMMAND_MATRIX)[number]["requestType"];
type MissingEditRequestType = Exclude<EditRequest["type"], RoutedEditRequestType>;
export const EDIT_GESTURE_COMMAND_MATRIX_IS_EXHAUSTIVE: [MissingEditRequestType] extends [never]
  ? true
  : never = true;

export function currentTimeline(): Timeline {
  const root = useProjectStore.getState().timeline;
  const sequenceId = useEditorUiStore.getState().activeNestedSequenceId;
  return root.nestedSequences?.find((sequence) => sequence.id === sequenceId)?.timeline ?? root;
}

/** End frame of the timeline currently shown in the editor. Nested timeline
 *  transport and paste placement must not inherit the root sequence duration. */
export function currentTimelineEndFrame(): number {
  let end = 0;
  for (const track of currentTimeline().tracks) {
    for (const clip of track.clips) end = Math.max(end, clip.startFrame + clip.durationFrames);
  }
  return end;
}

const ROOT_SCOPED_EDIT_REQUEST_TYPES = new Set<EditRequest["type"]>([
  "createNestedSequence",
  "editNestedSequence",
  "renameNestedSequence",
  "dissolveNestedSequence",
  "placeMedia",
  "createFolder",
  "moveToFolder",
  "renameMedia",
  "renameFolder",
  "deleteMedia",
  "deleteFolder",
  "setTimelineSettings",
]);

async function applyAndRefresh(
  cmd: Parameters<typeof api.editApply>[0],
  options: {
    root?: boolean;
    expected?: ProjectEditIdentity;
    sequenceId?: string | null;
  } = {},
) {
  const expected = options.expected ?? captureProjectEditIdentity();
  const sequenceId =
    options.sequenceId === undefined
      ? useEditorUiStore.getState().activeNestedSequenceId
      : options.sequenceId;
  const targetsRoot = options.root || ROOT_SCOPED_EDIT_REQUEST_TYPES.has(cmd.type);
  const request =
    sequenceId && !targetsRoot
      ? ({ type: "editNestedSequence", sequenceId, command: cmd } as const)
      : cmd;
  const res = await api.editApply(request, expected);
  // Tauri pushes timeline_changed -> sync re-fetches. The browser fallback has
  // no event channel, so refresh explicitly there.
  if (!isTauri && res.changed) await forceRefresh();
  return res;
}

/** Capture the complete authority token at gesture time. Rust rechecks all
 * three fields under the same lock as the edit transaction. */
export function captureProjectEditIdentity(): ProjectEditIdentity {
  const { projectEpoch, projectPath, timelineVersion } = useProjectStore.getState();
  return { projectEpoch, projectPath, timelineVersion };
}

/** Long-running preparation must retain both backend authority and the editor
 * scope selected when the gesture began. Project identity alone cannot prevent
 * a late result from being redirected to another nested sequence. */
export interface ProjectEditContext {
  expected: ProjectEditIdentity;
  sequenceId: string | null;
}

export function captureProjectEditContext(): ProjectEditContext {
  return {
    expected: captureProjectEditIdentity(),
    sequenceId: useEditorUiStore.getState().activeNestedSequenceId,
  };
}

export async function createNestedSequence(name = "Compound clip") {
  const clipIds = Array.from(useEditorUiStore.getState().selectedClipIds);
  if (clipIds.length === 0) return;
  const result = await applyAndRefresh({ type: "createNestedSequence", name, clipIds }, { root: true });
  if (result.affectedClipIds[0]) {
    useEditorUiStore.getState().selectClips(new Set([result.affectedClipIds[0]]));
  }
  return result;
}

export async function renameNestedSequence(sequenceId: string, name: string) {
  return applyAndRefresh({ type: "renameNestedSequence", sequenceId, name }, { root: true });
}

export async function editNestedSequence(sequenceId: string, command: EditRequest) {
  return applyAndRefresh({ type: "editNestedSequence", sequenceId, command }, { root: true });
}

export async function dissolveNestedSequence(clipId: string) {
  return applyAndRefresh({ type: "dissolveNestedSequence", clipId }, { root: true });
}

/** Root-scoped atomic media placement. `sequenceId` is carried inside the
 * command because optional project settings always apply to the root while the
 * clip itself may target a nested timeline. */
export async function placeMedia(
  request: Omit<Extract<EditRequest, { type: "placeMedia" }>, "type">,
  expected?: ProjectEditIdentity,
) {
  return applyAndRefresh(
    { type: "placeMedia", ...request },
    { root: true, expected, sequenceId: null },
  );
}

export async function addClips(entries: ClipEntryReq[]) {
  if (entries.length === 0) return;
  return applyAndRefresh({ type: "addClips", entries });
}

/** Place prepared text clips on explicit tracks as one edit transaction. */
export async function addTexts(entries: TextEntryReq[]) {
  if (entries.length === 0) return;
  return applyAndRefresh({ type: "addTexts", entries });
}

/** Place prepared text clips on one fresh top track as one edit transaction. */
export async function addTextsAutoTrack(entries: TextAutoTrackEntryReq[]) {
  if (entries.length === 0) return;
  return applyAndRefresh({ type: "addTextsAutoTrack", entries });
}

/** Ripple-insert clips at `atFrame` on `trackIndex`: place the entries and push
 *  everything past the insert point right by their total duration on the target
 *  track + every sync-locked track (backend `InsertClips` / upstream
 *  `rippleInsertClips`). The overwrite-drop counterpart is {@link addClips}. */
export async function insertClips(trackIndex: number, atFrame: number, entries: ClipEntryReq[]) {
  if (entries.length === 0) return;
  const res = await applyAndRefresh({ type: "insertClips", trackIndex, atFrame, entries });
  if (isTauri && res.changed) await forceRefresh();
  if (res.affectedClipIds.length > 0) {
    const ui = useEditorUiStore.getState();
    ui.selectClips(new Set(res.affectedClipIds));
    ui.setPreviewMedia(null);
    ui.setCurrentFrame(Math.max(0, atFrame));
  }
  return res;
}

/** Build the ripple-insert plan for a media item dropped at `atFrame` over
 *  `preferredTrackIndex` (wires the pure `buildInsertPlan` with the store's
 *  transform-fit + still-image default). Returns null when no compatible target
 *  track exists — the caller then falls back to the overwrite add. */
export function buildMediaInsertPlan(
  timeline: Timeline,
  item: MediaItem,
  atFrame: number,
  preferredTrackIndex: number | null,
): InsertPlan | null {
  return buildInsertPlan(
    timeline,
    item,
    atFrame,
    preferredTrackIndex,
    fitTransformForMedia,
    DEFAULT_IMAGE_SECONDS,
  );
}

export async function moveClips(moves: ClipMoveReq[]) {
  if (moves.length === 0) return;
  await applyAndRefresh({ type: "moveClips", moves });
}

/** Swap the positions of two clips (the cross-track "exchange places" gesture)
 *  so neither overwrites the other. The backend refuses (no change) if the swap
 *  would overlap a third clip, keeping it lossless. */
export async function swapClips(clipA: string, clipB: string) {
  if (clipA === clipB) return;
  await applyAndRefresh({ type: "swapClips", clipA, clipB });
}

/** Option/Alt-drag duplicate: deep-copy each clip to a new position. The
 *  backend clones every field (keyframe tracks / grade / masks / effects /
 *  text / transform / crop / fades), mints a fresh id, shifts `startFrame` by
 *  `offsetFrames`, lands on `targetTrackIndexes[i]`, and clears the link group
 *  (a copy is not linked to the original's partners). Returns the new clip ids
 *  via the EditResult so the caller can select them. */
export async function duplicateClips(
  clipIds: string[],
  offsetFrames: number,
  targetTrackIndexes: number[],
) {
  if (clipIds.length === 0) return;
  return applyAndRefresh({
    type: "duplicateClips",
    clipIds,
    offsetFrames,
    targetTrackIndexes,
  });
}

export async function moveOrDuplicateClipsToNewTrack(
  clipIds: string[],
  leadClipId: string,
  requestedFrameDelta: number,
  insertAt: number,
  mode: "move" | "duplicate",
) {
  if (clipIds.length === 0) return;
  return applyAndRefresh({
    type: "moveOrDuplicateClipsToNewTrack",
    clipIds,
    leadClipId,
    requestedFrameDelta,
    insertAt,
    mode,
  });
}

/** Deep-paste complete clipboard snapshots as one command. The caller plans
 * stable destination track ids before crossing the command boundary. */
export async function pasteClips(entries: PasteClipEntryReq[]) {
  if (entries.length === 0) return;
  return applyAndRefresh({ type: "pasteClips", entries });
}

export async function removeClips(clipIds: string[]) {
  if (clipIds.length === 0) return;
  await applyAndRefresh({ type: "removeClips", clipIds });
}

export async function splitClip(clipId: string, atFrame: number) {
  await applyAndRefresh({ type: "splitClip", clipId, atFrame });
}

/** Split every requested target in one backend transaction and one Undo step. */
export async function splitClips(clipIds: string[], atFrame: number) {
  if (clipIds.length === 0) return;
  await applyAndRefresh({ type: "splitClips", clipIds, atFrame });
}

export const DEFAULT_FREEZE_FRAMES = 30;

export async function freezeFrame(
  clipId: string,
  atFrame: number,
  durationFrames: number = DEFAULT_FREEZE_FRAMES,
) {
  return applyAndRefresh({ type: "freezeFrame", clipId, atFrame, durationFrames });
}

export async function freezeClipAtPlayhead(
  clip: Clip,
  durationFrames: number = DEFAULT_FREEZE_FRAMES,
) {
  const playhead = Math.round(useEditorUiStore.getState().activeFrame);
  const inside = playhead > clip.startFrame && playhead < clip.startFrame + clip.durationFrames;
  const atFrame = inside ? playhead : clip.startFrame + Math.floor(clip.durationFrames / 2);
  return freezeFrame(clip.id, atFrame, durationFrames);
}

export async function trimClips(edits: TrimEditReq[]) {
  if (edits.length === 0) return;
  await applyAndRefresh({ type: "trimClips", edits });
}

export async function setClipProperties(
  clipIds: string[],
  properties: ClipPropertiesReq,
  context?: ProjectEditContext,
) {
  if (clipIds.length === 0) return;
  await applyAndRefresh({ type: "setClipProperties", clipIds, properties }, context);
}

/** Commit one on-canvas transform gesture at an absolute timeline frame.
 *  The backend chooses keyframed versus static storage per transform component
 *  and applies the whole gesture atomically. */
export async function setTransformAtFrame(
  clipId: string,
  frame: number,
  transform: Transform,
  context?: ProjectEditContext,
) {
  await applyAndRefresh({ type: "setTransformAtFrame", clipId, frame, transform }, context);
}

export async function setColorGrade(clipIds: string[], grade: ColorGradeInput | null) {
  if (clipIds.length === 0) return;
  await applyAndRefresh({ type: "setColorGrade", clipIds, grade });
}

export async function setLut(clipIds: string[], lut: LutReference | null) {
  if (clipIds.length === 0) return;
  await applyAndRefresh({ type: "setLut", clipIds, lut });
}

export async function setChromaKey(clipIds: string[], chromaKey: ChromaKeyInput | null) {
  if (clipIds.length === 0) return;
  await applyAndRefresh({ type: "setChromaKey", clipIds, chromaKey });
}

export async function setMasks(clipIds: string[], masks: MaskInput[]) {
  if (clipIds.length === 0) return;
  await applyAndRefresh({ type: "setMasks", clipIds, masks });
}

export async function setEffects(clipIds: string[], effects: EffectInput[]) {
  if (clipIds.length === 0) return;
  await applyAndRefresh({ type: "setEffects", clipIds, effects });
}

export async function setLoudnessNormalization(
  clipId: string,
  normalization: LoudnessNormalization | null,
) {
  await applyAndRefresh({ type: "setLoudnessNormalization", clipId, normalization });
}

export async function analyzeAndApplyLoudness(
  clipId: string,
  targetLufs: number,
  truePeakCeilingDbtp: number,
) {
  const context = captureProjectEditContext();
  const normalization = await api.analyzeLoudness(clipId, targetLufs, truePeakCeilingDbtp);
  await applyAndRefresh(
    { type: "setLoudnessNormalization", clipId, normalization },
    context,
  );
  return normalization;
}

export async function cancelLoudnessAnalysis() {
  return api.cancelLoudnessAnalysis();
}

export async function setAudioDenoise(clipId: string, denoise: AudioDenoise | null) {
  await applyAndRefresh({ type: "setAudioDenoise", clipId, denoise });
}

export async function prepareAndApplyAudioDenoise(
  clipId: string,
  mode: DenoiseMode,
  strength: number,
  previewEnabled: boolean,
) {
  const context = captureProjectEditContext();
  const denoise = await api.prepareDenoise(clipId, mode, strength, previewEnabled);
  await applyAndRefresh({ type: "setAudioDenoise", clipId, denoise }, context);
  return denoise;
}

export async function cancelDenoiseAnalysis() {
  return api.cancelDenoiseAnalysis();
}

export async function applyStabilization(clipId: string, solution: StabilizationTrack) {
  await applyAndRefresh({ type: "applyStabilization", clipId, solution });
}

export async function analyzeAndApplyStabilization(clipId: string) {
  const context = captureProjectEditContext();
  const solution = await api.analyzeStabilization(clipId);
  await applyAndRefresh({ type: "applyStabilization", clipId, solution }, context);
  return solution;
}

export async function cancelStabilizationAnalysis() {
  return api.cancelStabilizationAnalysis();
}

export async function adjustStabilization(
  clipId: string,
  adjustment: { strength?: number; cropMargin?: number },
) {
  await applyAndRefresh({ type: "adjustStabilization", clipId, ...adjustment });
}

export async function resetStabilization(clipId: string) {
  await applyAndRefresh({ type: "resetStabilization", clipId });
}

export async function setTransition(
  fromClipId: string,
  toClipId: string,
  kind: TransitionKind | null,
  durationFrames: number,
) {
  await applyAndRefresh({
    type: "setTransition",
    fromClipId,
    toClipId,
    kind,
    durationFrames,
  });
}

/** Inspector Transform section "Reset" button (upstream `transformHeader`
 *  onReset). Resets transform to identity, opacity to full, clears the
 *  opacity/position/scale/rotation keyframe tracks, and zeroes both fades.
 *  Crop is untouched (a separate Inspector section upstream). */
export async function resetTransform(clipIds: string[]) {
  if (clipIds.length === 0) return;
  await applyAndRefresh({ type: "resetTransform", clipIds });
}

/** Change project timeline settings (FPS / resolution) — the Preview Aspect and
 *  Quality badge menus (upstream `applyTimelineSettings`). When FPS changes the
 *  Rust engine rescales all clip frames; width/height set the canvas. */
export async function setTimelineSettings(fps: number, width: number, height: number) {
  return applyAndRefresh({ type: "setTimelineSettings", fps, width, height });
}

export async function linkClips(clipIds: string[]) {
  await applyAndRefresh({ type: "link", clipIds });
}

/** Insert a new empty track of `kind` (clamped into its zone by the core). Used
 *  by the drop flow to create a track on demand when the timeline has none
 *  compatible. */
export async function insertTrack(kind: ClipType, at?: number) {
  return applyAndRefresh({ type: "insertTrack", kind, at });
}

export async function unlinkClips(clipIds: string[]) {
  await applyAndRefresh({ type: "unlink", clipIds });
}

/** Remove one or more tracks in a single undoable edit. */
export async function removeTracks(trackIndexes: number[]) {
  if (trackIndexes.length === 0) return;
  await applyAndRefresh({ type: "removeTracks", trackIndexes });
}

/** Toggle a track head's mute / hide / sync-lock. Omitted fields are unchanged. */
export async function setTrackProps(
  trackIndex: number,
  props: { muted?: boolean; hidden?: boolean; syncLocked?: boolean },
) {
  await applyAndRefresh({ type: "setTrackProps", trackIndex, ...props });
}

/** Swap two same-kind tracks as whole rows; cross-kind swaps are a no-op in core. */
export async function swapTracks(a: number, b: number) {
  if (a === b) return;
  await applyAndRefresh({ type: "swapTracks", a, b });
}

/** Replace (or clear) a clip's keyframe track for one property. */
export async function setKeyframes(
  clipId: string,
  property: KeyframeProperty,
  payload: KeyframePayloadReq,
) {
  await applyAndRefresh({ type: "setKeyframes", clipId, property, payload });
}

/** Stamp a keyframe at `frame` using the clip's current sampled value. */
export async function stampKeyframe(
  clipId: string,
  property: KeyframeProperty,
  frame: number,
) {
  await applyAndRefresh({ type: "stampKeyframe", clipId, property, frame });
}

/** Upsert a keyframe at `frame` with an EXPLICIT value (not the sampled value —
 *  that's `stampKeyframe`). The write path for editing an animated property's
 *  value at the playhead, mirroring upstream `write<Property>`'s upsert branch. */
export async function upsertKeyframe(
  clipId: string,
  property: KeyframeProperty,
  frame: number,
  value: KeyframeValueReq,
  context?: ProjectEditContext,
) {
  await applyAndRefresh({ type: "upsertKeyframe", clipId, property, frame, value }, context);
}

/** Remove the keyframe at `frame`. */
export async function removeKeyframe(
  clipId: string,
  property: KeyframeProperty,
  frame: number,
) {
  await applyAndRefresh({ type: "removeKeyframe", clipId, property, frame });
}

/** Move a keyframe from `fromFrame` to `toFrame`. */
export async function moveKeyframe(
  clipId: string,
  property: KeyframeProperty,
  fromFrame: number,
  toFrame: number,
  context?: ProjectEditContext,
) {
  await applyAndRefresh({ type: "moveKeyframe", clipId, property, fromFrame, toFrame }, context);
}

/** Change the interpolation mode of the keyframe at `frame`. */
export async function setKeyframeInterpolation(
  clipId: string,
  property: KeyframeProperty,
  frame: number,
  interpolation: Interpolation,
) {
  await applyAndRefresh({ type: "setKeyframeInterpolation", clipId, property, frame, interpolation });
}

/** Ripple-delete project-frame ranges on a track, closing the gaps. */
export async function rippleDeleteRanges(trackIndex: number, ranges: FrameRangeReq[]) {
  if (ranges.length === 0) return;
  await applyAndRefresh({ type: "rippleDeleteRanges", trackIndex, ranges });
}

/** Ripple-delete selected clips as one atomic/undoable request. */
export async function rippleDeleteClips(clipIds: string[]) {
  if (clipIds.length === 0) return;
  await applyAndRefresh({ type: "rippleDeleteClips", clipIds });
}

/** Create a media-library folder (optionally nested under `parentFolderId`). */
export async function createFolder(name: string, parentFolderId?: string) {
  await applyAndRefresh({ type: "createFolder", name, parentFolderId });
}

/** Move media assets into a folder (or to root with no `folderId`). */
export async function moveToFolder(assetIds: string[], folderId?: string) {
  if (assetIds.length === 0) return;
  await applyAndRefresh({ type: "moveToFolder", assetIds, folderId });
}

export async function renameMedia(entries: RenameEntryReq[]) {
  if (entries.length === 0) return;
  await applyAndRefresh({ type: "renameMedia", entries });
}

export async function renameFolder(entries: RenameEntryReq[]) {
  if (entries.length === 0) return;
  await applyAndRefresh({ type: "renameFolder", entries });
}

export async function deleteMedia(assetIds: string[], expected?: ProjectEditIdentity) {
  if (assetIds.length === 0) return;
  return applyAndRefresh({ type: "deleteMedia", assetIds }, { expected });
}

export async function deleteFolder(folderIds: string[], expected?: ProjectEditIdentity) {
  if (folderIds.length === 0) return;
  return applyAndRefresh({ type: "deleteFolder", folderIds }, { expected });
}

/** Replace a clip's media source in place, preserving all editing attributes.
 *  The backend intentionally consumes only `clipId` + `mediaRef`; it does not
 *  rewrite trim, duration, or type metadata. */
export async function swapMedia(clipId: string, mediaRef: string) {
  await applyAndRefresh({ type: "swapMedia", clipId, mediaRef });
}

export async function applyAutomationCommands(commands: EditRequest[]) {
  if (commands.length === 0) return [];
  if (commands.length !== 1) {
    throw new Error(
      "Automation writes must arrive as one atomic EditRequest; multi-command batches are refused",
    );
  }
  return [await applyAndRefresh(commands[0])];
}

export async function applySmartReframe(clipIds: string[], crop: Crop, transform?: Transform) {
  if (clipIds.length === 0) return;
  await setClipProperties(clipIds, { crop, transform });
}

export async function addClipsToBeatFrames(entries: ClipEntryReq[], beatFrames: number[]) {
  if (entries.length === 0) return;
  const placed = entries.map((entry, index) => ({
    ...entry,
    startFrame: beatFrames[index] ?? entry.startFrame,
  }));
  await addClips(placed);
}

export async function tightenSilenceRanges(trackIndex: number, ranges: FrameRangeReq[]) {
  await rippleDeleteRanges(trackIndex, ranges);
}

export async function undo() {
  await api.undo(captureProjectEditIdentity());
  if (!isTauri) await forceRefresh();
}

export async function redo() {
  await api.redo(captureProjectEditIdentity());
  if (!isTauri) await forceRefresh();
}

/** Split at the current playhead (Toolbar / ⌘K). Upstream only acts on the
 *  selected clips; with no selection this is an intentional no-op. A selected
 *  clip the playhead doesn't intersect is a no-op in the core. */
export async function splitAtPlayhead() {
  const ui = useEditorUiStore.getState();
  const frame = Math.round(ui.activeFrame);
  if (ui.selectedClipIds.size === 0) return;
  const ids = clipsUnderPlayhead().map((clip) => clip.id);
  await splitClips(ids, frame);
}

/** Selected clips the playhead is strictly inside. Upstream playhead-relative
 *  edits are selection-driven; an empty selection intentionally yields none. */
function clipsUnderPlayhead(): Clip[] {
  const ui = useEditorUiStore.getState();
  const frame = Math.round(ui.activeFrame);
  const selected = new Set(ui.selectedClipIds);
  if (selected.size === 0) return [];
  const out: Clip[] = [];
  for (const track of currentTimeline().tracks) {
    for (const c of track.clips) {
      if (frame <= c.startFrame || frame >= c.startFrame + c.durationFrames) continue;
      if (!selected.has(c.id)) continue;
      out.push(c);
    }
  }
  return out;
}

/** Trim each target clip's IN point to the playhead (Q / Toolbar `[` — 剪映
 *  "删除播放头左侧"). The right edge stays put; the left part is removed. */
export async function trimStartToPlayhead() {
  const frame = Math.round(useEditorUiStore.getState().activeFrame);
  await trimClips(trimToPlayheadEdits(clipsUnderPlayhead(), frame, "left"));
}

/** Trim each target clip's OUT point to the playhead (W / Toolbar `]` — 剪映
 *  "删除播放头右侧"). The left edge stays put; the right part is removed. */
export async function trimEndToPlayhead() {
  const frame = Math.round(useEditorUiStore.getState().activeFrame);
  await trimClips(trimToPlayheadEdits(clipsUnderPlayhead(), frame, "right"));
}

/** The subset of `selected` that still exists as a clip in the current timeline.
 *  A stale id (a clip already removed/replaced/split by a prior edit, left behind
 *  in the selection set) makes the core's RemoveClips/RippleDelete reject the
 *  WHOLE batch — so one orphan silently blocks deletion of everything. Filtering
 *  to live ids first is what makes ⌫ reliably delete. */
function liveSelectedClipIds(): string[] {
  const live = new Set<string>();
  for (const track of currentTimeline().tracks) {
    for (const clip of track.clips) live.add(clip.id);
  }
  return [...useEditorUiStore.getState().selectedClipIds].filter((id) => live.has(id));
}

/** Delete selected clips (⌫ / menu). Wrapped in try/catch so that even if the
 *  backend RemoveClips rejects (IPC error, an edge the live-id filter missed),
 *  the selection is still cleared and the failure is surfaced as a toast instead
 *  of silently doing nothing (the reported "delete does nothing"). */
export async function deleteSelectedClips() {
  const ui = useEditorUiStore.getState();
  const ids = liveSelectedClipIds();
  if (ids.length > 0) {
    try {
      await removeClips(ids);
      // Tauri normally refreshes via the timeline_changed event; force it too so
      // a missed/raced event can't leave the just-deleted clip painted on screen.
      if (isTauri) await forceRefresh();
    } catch (err) {
      ui.pushToast(`删除失败 / Delete failed: ${err instanceof Error ? err.message : String(err)}`);
    }
  }
  ui.clearSelection();
}

async function runSaveAsMedia(
  label: string,
  successMessage: string,
  failurePrefix: string,
  command: (operationId: string) => Promise<unknown>,
): Promise<void> {
  const ui = useEditorUiStore.getState();
  if (ui.saveAsProgress) {
    ui.pushToast("已有另存任务正在进行 / Another save-as operation is already running");
    return;
  }
  const operationId = api.createExportOperationId("save-as");
  ui.setSaveAsProgress({
    operationId,
    label,
    done: 0,
    total: 1,
    cancellable: false,
    cancelling: false,
  });
  let unlisten: (() => void) | undefined;
  try {
    unlisten = await api.onExportProgress(operationId, ({ done, total }) => {
      const current = useEditorUiStore.getState().saveAsProgress;
      if (!current || current.operationId !== operationId) return;
      useEditorUiStore.getState().setSaveAsProgress({
        ...current,
        done: Math.max(0, done),
        total: Math.max(1, total),
      });
    });
    // Dispatch the IPC command before exposing Cancel. Tauri has then queued
    // the backend operation before React can render the enabled button.
    const operation = command(operationId);
    const current = useEditorUiStore.getState().saveAsProgress;
    if (current?.operationId === operationId) {
      ui.setSaveAsProgress({ ...current, cancellable: true });
    }
    await operation;
    await refreshMedia();
    ui.pushToast(successMessage);
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    if (message === api.EXPORT_CANCELLED_SENTINEL) {
      ui.pushToast("已取消另存 / Save as media cancelled");
    } else {
      ui.pushToast(`${failurePrefix}: ${message}`);
    }
  } finally {
    unlisten?.();
    if (useEditorUiStore.getState().saveAsProgress?.operationId === operationId) {
      ui.setSaveAsProgress(null);
    }
  }
}

/** Save a clip as a reusable project media asset with visible progress. */
export async function saveClipAsMedia(clipId: string) {
  await runSaveAsMedia(
    "正在另存片段 / Saving clip",
    "已另存为媒体 / Saved as media",
    "另存失败 / Save as media failed",
    (operationId) => api.saveClipAsMedia(clipId, operationId),
  );
}

export async function saveMarkedRangeAsMedia(range: TimelineRange) {
  const normalized = validRange(range);
  if (!normalized) return;
  await runSaveAsMedia(
    "正在另存范围 / Saving range",
    "范围已另存为媒体 / Range saved as media",
    "范围另存失败 / Save range as media failed",
    (operationId) =>
      api.saveRangeAsMedia(normalized.startFrame, normalized.endFrame, operationId),
  );
}

export async function cancelSaveAsMedia(): Promise<void> {
  const ui = useEditorUiStore.getState();
  const current = ui.saveAsProgress;
  if (!current || !current.cancellable || current.cancelling) return;
  ui.setSaveAsProgress({ ...current, cancelling: true });
  try {
    await api.cancelExport(current.operationId);
  } catch (error) {
    const latest = useEditorUiStore.getState().saveAsProgress;
    if (latest?.operationId === current.operationId) {
      ui.setSaveAsProgress({ ...latest, cancelling: false });
    }
    ui.pushToast(
      `取消失败 / Cancel failed: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
}

/** Ripple-delete selected clips (⇧⌫): remove and close the gaps, shifting
 *  sync-locked followers (the core refuses if a follower would collide). */
export async function rippleDeleteSelectedClips() {
  const ui = useEditorUiStore.getState();
  const ids = liveSelectedClipIds();
  if (ids.length > 0) {
    try {
      await rippleDeleteClips(ids);
      if (isTauri) await forceRefresh();
    } catch (err) {
      ui.pushToast(`删除失败 / Delete failed: ${err instanceof Error ? err.message : String(err)}`);
    }
  }
  ui.clearSelection();
}

/** Ripple-delete the marked in/out range on the anchor clip's track (⇧⌫ when a
 *  valid range is marked AND a clip is selected). Mirrors upstream
 *  `rippleDeleteRanges(anchorClipId:)` → `rippleDeleteRangesOnTrack(trackIndex:
 *  anchorLoc.trackIndex, …)`: the range is cut from the selected clip's track,
 *  gaps close, linked A/V partners are cut, and sync-locked followers shift
 *  (the core refuses if a follower would collide). No-op without a valid range
 *  or a selected clip. Returns true when a delete was issued. */
export async function rippleDeleteMarkedRange(): Promise<boolean> {
  const ui = useEditorUiStore.getState();
  const range = validRange(ui.selectedTimelineRange);
  if (!range) return false;
  // Anchor = the selected clip's track (upstream `rippleDeleteRanges(anchorClipId:)`).
  const anchorId = liveSelectedClipIds()[0];
  if (!anchorId) return false;
  const timeline = currentTimeline();
  let trackIndex = -1;
  for (let ti = 0; ti < timeline.tracks.length && trackIndex < 0; ti++) {
    if (timeline.tracks[ti].clips.some((c) => c.id === anchorId)) trackIndex = ti;
  }
  if (trackIndex < 0) return false;
  try {
    await rippleDeleteRanges(trackIndex, [{ start: range.startFrame, end: range.endFrame }]);
    if (isTauri) await forceRefresh();
  } catch (err) {
    ui.pushToast(`删除失败 / Delete failed: ${err instanceof Error ? err.message : String(err)}`);
  }
  ui.clearTimelineRange();
  ui.clearSelection();
  return true;
}

/** Ripple-close the selected gap (⇧⌫ on a selected gap): remove the empty span
 *  and shift the gap track's later clips + every sync-locked follower left by the
 *  gap length (backend `RippleDeleteRanges` on the gap's track; upstream
 *  `rippleDeleteSelectedGap`). No-op without a selected gap. Returns true when a
 *  delete was issued. */
export async function rippleDeleteSelectedGap(): Promise<boolean> {
  const ui = useEditorUiStore.getState();
  const gap = ui.selectedGap;
  if (!gap || gap.endFrame <= gap.startFrame) return false;
  // Guard: an out-of-band edit may have filled the gap (upstream re-checks).
  const timeline = currentTimeline();
  const track = timeline.tracks[gap.trackIndex];
  if (
    !track ||
    track.clips.some((c) => c.startFrame < gap.endFrame && c.startFrame + c.durationFrames > gap.startFrame)
  ) {
    ui.selectGap(null);
    return false;
  }
  try {
    await rippleDeleteRanges(gap.trackIndex, [{ start: gap.startFrame, end: gap.endFrame }]);
    if (isTauri) await forceRefresh();
  } catch (err) {
    ui.pushToast(`删除失败 / Delete failed: ${err instanceof Error ? err.message : String(err)}`);
  }
  ui.selectGap(null);
  return true;
}

/** Nudge the selected clips by `deltaFrames` along the timeline (OpenTake
 *  extension: , / . keys, ±5 with Shift). Preserves each clip's track, floors
 *  the group at frame 0, and moves linked partners together (the selection is
 *  expanded to full link groups, then routed through `moveClips`). No-op when
 *  nothing is selected or the floored delta is zero. */
export async function nudgeSelectedClips(deltaFrames: number) {
  const ui = useEditorUiStore.getState();
  if (ui.selectedClipIds.size === 0) return;
  const timeline = currentTimeline();
  // Linked partners travel together (the backend moveClips does NOT auto-expand
  // link groups — the drag path expands them too).
  const expanded = expandLinkGroup(timeline, ui.selectedClipIds);
  const moves = planNudge(timeline, expanded, deltaFrames);
  if (moves.length === 0) return;
  await moveClips(moves);
}

// MARK: - Media -> timeline (drag and drop)

/** Stills get a fixed default duration (upstream `Constants.defaultImageDuration`
 *  ≈ 5s) since they have no intrinsic length. */
const DEFAULT_IMAGE_SECONDS = 5;

/** Match Rust's positive `f64 as i32` frame conversion: truncate toward zero,
 *  return a finite integer, and keep duration-bearing UI objects visible for at
 *  least one frame. Callers choose any source-duration fallback first. */
function durationSecondsToFrames(seconds: number, fps: number): number {
  const frames = seconds * fps;
  return Number.isFinite(frames) ? Math.max(1, Math.trunc(frames)) : 1;
}

/** Same conversion for offsets, where frame zero is valid. */
function offsetSecondsToFrames(seconds: number, fps: number): number {
  const frames = seconds * fps;
  return Number.isFinite(frames) ? Math.trunc(frames) : 0;
}

/** Frame length a media item occupies when placed on the timeline: its source
 *  duration (seconds → frames), or the still-image default when it has none.
 *  Shared by the clip-entry builders and the drop-ghost preview so the ghost
 *  width matches the clip that actually lands. */
export function mediaDurationFrames(item: MediaItem, fps: number): number {
  const seconds = Number.isFinite(item.duration) && item.duration > 0
    ? item.duration
    : DEFAULT_IMAGE_SECONDS;
  return durationSecondsToFrames(seconds, fps);
}

function isVisual(type: MediaItem["type"]): boolean {
  return type === "video" || type === "image" || type === "text" || type === "lottie";
}

/** First existing track whose kind is compatible with `type`, else null. When
 *  none exists, the drop flow ([`addMediaToTimeline`]) creates one on demand
 *  (`insertTrack`) — mirroring upstream `placeClip` auto-track-creation — so a
 *  drop onto an empty timeline still produces a clip. */
function firstCompatibleTrackIndex(timeline: Timeline, type: MediaItem["type"]): number | null {
  const wantAudio = type === "audio";
  for (let i = 0; i < timeline.tracks.length; i++) {
    const trackIsAudio = timeline.tracks[i].type === "audio";
    if (wantAudio ? trackIsAudio : !trackIsAudio && isVisual(timeline.tracks[i].type)) {
      return i;
    }
  }
  return null;
}

function trackIsCompatible(timeline: Timeline, trackIndex: number, type: MediaItem["type"]): boolean {
  const track = timeline.tracks[trackIndex];
  if (!track) return false;
  const wantAudio = type === "audio";
  const trackIsAudio = track.type === "audio";
  return wantAudio ? trackIsAudio : !trackIsAudio && isVisual(track.type);
}

/** Append position on a track: just past its last clip (clamped to >= 0). */
function appendStartFrame(timeline: Timeline, trackIndex: number): number {
  return timeline.tracks[trackIndex].clips.reduce(
    (max, c) => Math.max(max, c.startFrame + c.durationFrames),
    0,
  );
}

function trackOverlaps(timeline: Timeline, trackIndex: number, startFrame: number, durationFrames: number): boolean {
  const endFrame = startFrame + durationFrames;
  return timeline.tracks[trackIndex].clips.some((c) => c.startFrame < endFrame && c.startFrame + c.durationFrames > startFrame);
}

export function firstOpenCompatibleTrackIndex(
  timeline: Timeline,
  type: MediaItem["type"],
  startFrame: number,
  durationFrames: number,
  preferredTrackIndex: number | null,
): number | null {
  const candidates: number[] = [];
  if (preferredTrackIndex !== null && trackIsCompatible(timeline, preferredTrackIndex, type)) {
    candidates.push(preferredTrackIndex);
  }
  for (let i = 0; i < timeline.tracks.length; i++) {
    if (i !== preferredTrackIndex && trackIsCompatible(timeline, i, type)) candidates.push(i);
  }
  for (const trackIndex of candidates) {
    if (!trackOverlaps(timeline, trackIndex, startFrame, durationFrames)) return trackIndex;
  }
  return null;
}

/** Where a media item dropped at `startFrame` over `dropTarget` will actually
 *  land — the truthful target for the drop-ghost preview. Pure mirror of
 *  [`addMediaToTimelineAtInner`]'s resolution: an insert-zone hover makes a new
 *  track; an over-a-track hover lands on the first open compatible track (the
 *  hovered one when free), and falls back to a fresh track when none is open. */
export function resolveMediaDropTrack(
  timeline: Timeline,
  item: MediaItem,
  startFrame: number,
  dropTarget: TrackDropTarget,
): { trackIndex: number | null; newTrack: { index: number; type: ClipType } | null } {
  const newType: ClipType = item.type === "audio" ? "audio" : "video";
  if (dropTarget.kind === "newTrack") {
    return { trackIndex: null, newTrack: { index: dropTarget.index, type: newType } };
  }
  const durationFrames = mediaDurationFrames(item, timeline.fps);
  const landed = firstOpenCompatibleTrackIndex(
    timeline,
    item.type,
    Math.max(0, startFrame),
    durationFrames,
    dropTarget.trackIndex,
  );
  if (landed !== null) return { trackIndex: landed, newTrack: null };
  // No open compatible track under/near the hover: the drop inserts a fresh one
  // at the hovered index (fallback in `addMediaToTimelineAtInner`).
  return { trackIndex: null, newTrack: { index: dropTarget.trackIndex, type: newType } };
}

interface MediaGestureContext {
  expected: ProjectEditIdentity;
  sequenceId: string | null;
}

interface MediaQueueOutcome {
  generation: number;
  ok: boolean;
  anchor: MediaGestureContext;
  next?: MediaGestureContext;
}

interface MediaQueueRunContext extends MediaGestureContext {
  timeline: Timeline;
}

let mediaQueueGeneration = 0;
let mediaQueuePending = 0;
let mediaAddQueue: Promise<MediaQueueOutcome | null> = Promise.resolve(null);

function sameProjectIdentity(left: ProjectEditIdentity, right: ProjectEditIdentity): boolean {
  return left.projectEpoch === right.projectEpoch
    && left.projectPath === right.projectPath
    && left.timelineVersion === right.timelineVersion;
}

function sameMediaContext(left: MediaGestureContext, right: MediaGestureContext): boolean {
  return sameProjectIdentity(left.expected, right.expected) && left.sequenceId === right.sequenceId;
}

function currentMediaContext(): MediaGestureContext {
  return {
    expected: captureProjectEditIdentity(),
    sequenceId: useEditorUiStore.getState().activeNestedSequenceId,
  };
}

function assertMediaContext(expected: MediaGestureContext, stage: string): void {
  const current = currentMediaContext();
  if (!sameMediaContext(current, expected)) {
    throw new Error(`media placement authority changed ${stage}`);
  }
}

/** Serialize media-placement gestures while retaining the authority captured at
 * enqueue time. A clean predecessor may advance a rapid-click chain by exactly
 * one committed revision; every other version/path/epoch/sequence change
 * breaks all already-queued gestures in that generation. */
function enqueueMediaAdd(
  run: (context: MediaQueueRunContext) => Promise<EditResult>,
): Promise<void> {
  const captured = currentMediaContext();
  if (mediaQueuePending === 0) {
    mediaQueueGeneration += 1;
    mediaAddQueue = Promise.resolve(null);
  }
  const generation = mediaQueueGeneration;
  mediaQueuePending += 1;
  const previous = mediaAddQueue;
  let anchor = captured;

  const execution = previous.then(async (outcome): Promise<MediaQueueOutcome> => {
    if (outcome?.generation === generation) {
      anchor = outcome.anchor;
      if (!outcome.ok || !outcome.next) {
        throw new Error("an earlier queued media placement did not commit cleanly");
      }
      if (!sameMediaContext(captured, outcome.anchor)) {
        throw new Error("queued media placement was captured from another authority");
      }
      assertMediaContext(outcome.next, "before the next queued plan");
    } else {
      assertMediaContext(captured, "before planning");
    }

    const context = outcome?.generation === generation && outcome.next
      ? outcome.next
      : captured;
    const result = await run({ ...context, timeline: currentTimeline() });
    if (!result.changed || result.timelineVersion !== context.expected.timelineVersion + 1) {
      throw new Error("media placement did not produce exactly one committed revision");
    }
    if (isTauri) await forceRefresh();
    const next: MediaGestureContext = {
      expected: {
        projectEpoch: context.expected.projectEpoch,
        projectPath: context.expected.projectPath,
        timelineVersion: result.timelineVersion,
      },
      sequenceId: context.sequenceId,
    };
    assertMediaContext(next, "after authoritative refresh");
    return { generation, ok: true, anchor, next };
  });

  mediaAddQueue = execution.then(
    (outcome) => outcome,
    () => ({ generation, ok: false, anchor }),
  );
  return execution.then(() => undefined).finally(() => {
    mediaQueuePending -= 1;
  });
}

async function chooseProjectSettings(
  item: MediaItem,
  timeline: Timeline,
): Promise<ProjectTimelineSettingsReq | undefined> {
  const decision = checkProjectSettings(timeline, [item]);
  if (decision.kind === "proceed") return undefined;
  if (decision.kind === "apply") return decision.settings;
  const applySuggested = await useEditorUiStore.getState().requestProjectSettingsPrompt({
    current: { fps: timeline.fps, width: timeline.width, height: timeline.height },
    suggested: decision.settings,
  });
  return applySuggested ? decision.settings : undefined;
}

/** Placement-only projection of the root settings transaction. The backend is
 * authoritative, but planning must use the post-settings fps/canvas and clip
 * ranges before that single compound command is sent. */
function timelineAfterSettings(
  timeline: Timeline,
  settings: ProjectTimelineSettingsReq | undefined,
): Timeline {
  if (!settings) return timeline;
  const scale = timeline.fps > 0 ? settings.fps / timeline.fps : 1;
  const tracks = timeline.tracks.map((track) => {
    let previousEnd: number | undefined;
    const clips = [...track.clips]
      .sort((left, right) => left.startFrame - right.startFrame)
      .map((clip) => {
        const scaledStart = Math.round(clip.startFrame * scale);
        const scaledEnd = Math.round((clip.startFrame + clip.durationFrames) * scale);
        const startFrame = Math.max(scaledStart, previousEnd ?? scaledStart);
        const durationFrames = Math.max(1, scaledEnd - startFrame);
        previousEnd = startFrame + durationFrames;
        return { ...structuredClone(clip), startFrame, durationFrames };
      });
    return { ...track, clips };
  });
  return {
    ...timeline,
    fps: settings.fps,
    width: settings.width,
    height: settings.height,
    settingsConfigured: true,
    tracks,
  };
}

/** Add a media-library item to the timeline (drag-drop / double-click from the
 *  media panel). Resolves the target track and append position from the current
 *  mirror; if the timeline has no compatible track (e.g. a brand-new empty
 *  project), creates one on demand first (upstream `placeClip` auto-creates),
 *  then places the clip.
 *
 *  Adds are **serialized**: a rapid second drop/double-click would otherwise
 *  start while the first is still in flight, read a stale (clip-less) mirror,
 *  compute `startFrame` 0 again, and have the core's overwrite-on-place drop the
 *  first clip. The queue makes each add observe the previous one's result. */
export function addMediaToTimeline(item: MediaItem): Promise<void> {
  return enqueueMediaAdd((context) => addMediaToTimelineInner(item, context));
}

/** Consume a media-placement rejection at a user-event boundary while keeping
 * the placement action itself rejectable for callers that await it. */
export function reportMediaPlacementFailure(error: unknown): void {
  const message = error instanceof Error ? error.message : String(error);
  useEditorUiStore.getState().pushToast(t("media.placeFailed", { error: message }));
}

export function addMediaToTimelineAt(
  item: MediaItem,
  startFrame: number,
  preferredTrackIndex: number | null,
  insertTrackAt?: number,
): Promise<void> {
  const preferredTrackId = preferredTrackIndex === null
    ? null
    : currentTimeline().tracks[preferredTrackIndex]?.id ?? null;
  return enqueueMediaAdd((context) =>
    addMediaToTimelineAtInner(
      item,
      startFrame,
      preferredTrackId,
      preferredTrackIndex,
      insertTrackAt,
      context,
    ),
  );
}

/** A source-media sub-range (seconds) to place from a search "Moments"/"Spoken"
 *  hit: only `[startSec, endSec)` of the asset lands on the timeline as a trimmed
 *  clip, mirroring upstream's `assetDragString(forAssetId:segment:)`. */
export interface SourceRange {
  startSec: number;
  endSec: number;
}

/** Frames a moment clip occupies on the timeline for a source `[startSec,endSec)`
 *  range: the range length in frames, clamped to at least one frame. */
export function momentDurationFrames(range: SourceRange, fps: number): number {
  return durationSecondsToFrames(range.endSec - range.startSec, fps);
}

/** Place only `range` of `item` on the timeline at `startFrame` — a trimmed clip
 *  (drag from a visual/spoken search hit). Reuses the same track resolution as a
 *  full-asset drop, then overrides the entry's trim/duration from the range.
 *  A still image (or a range that covers the whole/none of the source) falls back
 *  to the plain full-asset placement. */
export function addMomentToTimelineAt(
  item: MediaItem,
  startFrame: number,
  preferredTrackIndex: number | null,
  range: SourceRange,
  insertTrackAt?: number,
): Promise<void> {
  const preferredTrackId = preferredTrackIndex === null
    ? null
    : currentTimeline().tracks[preferredTrackIndex]?.id ?? null;
  return enqueueMediaAdd((context) =>
    addMomentToTimelineAtInner(
      item,
      startFrame,
      preferredTrackId,
      preferredTrackIndex,
      range,
      insertTrackAt,
      context,
    ),
  );
}

function unplacedForMedia(
  timeline: Timeline,
  item: MediaItem,
  startFrame: number,
): UnplacedClipEntryReq {
  return {
    mediaRef: item.id,
    mediaType: item.type,
    sourceClipType: item.type,
    startFrame: Math.max(0, startFrame),
    durationFrames: mediaDurationFrames(item, timeline.fps),
    hasAudio: item.hasAudio,
    addLinkedAudio: item.type === "video" && item.hasAudio,
    transform: fitTransformForMedia(item.width, item.height, timeline.width, timeline.height),
  };
}

async function commitMediaPlacement(
  item: MediaItem,
  context: MediaQueueRunContext,
  plan: (timeline: Timeline) => { target: PlaceMediaTargetReq; entry: UnplacedClipEntryReq },
): Promise<EditResult> {
  const settings = await chooseProjectSettings(item, context.timeline);
  assertMediaContext(context, "after project-settings choice");
  const placement = plan(timelineAfterSettings(context.timeline, settings));
  const result = await placeMedia(
    {
      sequenceId: context.sequenceId ?? undefined,
      settings,
      target: placement.target,
      entry: placement.entry,
    },
    context.expected,
  );
  if (result.affectedClipIds.length > 0) {
    const ui = useEditorUiStore.getState();
    ui.selectClips(new Set(result.affectedClipIds));
    ui.setPreviewMedia(null);
    ui.setCurrentFrame(placement.entry.startFrame);
  }
  return result;
}

async function addMediaToTimelineInner(
  item: MediaItem,
  context: MediaQueueRunContext,
): Promise<EditResult> {
  return commitMediaPlacement(item, context, (timeline) => {
    const trackIndex = firstCompatibleTrackIndex(timeline, item.type);
    const startFrame = trackIndex === null ? 0 : appendStartFrame(timeline, trackIndex);
    return {
      target: trackIndex === null
        ? {
            kind: "newTrack",
            trackType: item.type === "audio" ? "audio" : "video",
            at: timeline.tracks.length,
          }
        : { kind: "existingTrack", trackId: timeline.tracks[trackIndex].id },
      entry: unplacedForMedia(timeline, item, startFrame),
    };
  });
}

async function addMediaToTimelineAtInner(
  item: MediaItem,
  startFrame: number,
  preferredTrackId: string | null,
  preferredTrackIndexAtGesture: number | null,
  insertTrackAt: number | undefined,
  context: MediaQueueRunContext,
): Promise<EditResult> {
  return commitMediaPlacement(item, context, (timeline) => {
    const preferredTrackIndex = preferredTrackId === null
      ? null
      : timeline.tracks.findIndex((track) => track.id === preferredTrackId);
    const normalizedPreferred = preferredTrackIndex !== null && preferredTrackIndex >= 0
      ? preferredTrackIndex
      : null;
    const entry = unplacedForMedia(timeline, item, startFrame);
    if (insertTrackAt !== undefined) {
      return {
        target: {
          kind: "newTrack",
          trackType: item.type === "audio" ? "audio" : "video",
          at: insertTrackAt,
        },
        entry,
      };
    }
    const existing = firstOpenCompatibleTrackIndex(
      timeline,
      item.type,
      entry.startFrame,
      entry.durationFrames,
      normalizedPreferred,
    );
    return existing === null
      ? {
          target: {
            kind: "newTrack",
            trackType: item.type === "audio" ? "audio" : "video",
            at: normalizedPreferred ?? preferredTrackIndexAtGesture ?? timeline.tracks.length,
          },
          entry,
        }
      : {
          target: { kind: "existingTrack", trackId: timeline.tracks[existing].id },
          entry,
        };
  });
}

async function addMomentToTimelineAtInner(
  item: MediaItem,
  startFrame: number,
  preferredTrackId: string | null,
  preferredTrackIndexAtGesture: number | null,
  range: SourceRange,
  insertTrackAt: number | undefined,
  context: MediaQueueRunContext,
): Promise<EditResult> {
  // Stills (or a degenerate range) have no meaningful sub-range → full asset.
  const spanSec = range.endSec - range.startSec;
  if (item.type === "image" || spanSec <= 0 || item.duration <= 0) {
    return addMediaToTimelineAtInner(
      item,
      startFrame,
      preferredTrackId,
      preferredTrackIndexAtGesture,
      insertTrackAt,
      context,
    );
  }
  return commitMediaPlacement(item, context, (timeline) => {
    const preferredTrackIndex = preferredTrackId === null
      ? null
      : timeline.tracks.findIndex((track) => track.id === preferredTrackId);
    const normalizedPreferred = preferredTrackIndex !== null && preferredTrackIndex >= 0
      ? preferredTrackIndex
      : null;
    const totalSource = mediaDurationFrames(item, timeline.fps);
    const trimStartFrame = Math.max(
      0,
      Math.min(totalSource, offsetSecondsToFrames(range.startSec, timeline.fps)),
    );
    const durationFrames = Math.max(
      1,
      Math.min(momentDurationFrames(range, timeline.fps), totalSource - trimStartFrame),
    );
    const entry: UnplacedClipEntryReq = {
      ...unplacedForMedia(timeline, item, startFrame),
      durationFrames,
      trimStartFrame,
      trimEndFrame: Math.max(0, totalSource - trimStartFrame - durationFrames),
    };
    if (insertTrackAt !== undefined) {
      return {
        target: {
          kind: "newTrack",
          trackType: item.type === "audio" ? "audio" : "video",
          at: insertTrackAt,
        },
        entry,
      };
    }
    const existing = firstOpenCompatibleTrackIndex(
      timeline,
      item.type,
      entry.startFrame,
      entry.durationFrames,
      normalizedPreferred,
    );
    return existing === null
      ? {
          target: {
            kind: "newTrack",
            trackType: item.type === "audio" ? "audio" : "video",
            at: normalizedPreferred ?? preferredTrackIndexAtGesture ?? timeline.tracks.length,
          },
          entry,
        }
      : {
          target: { kind: "existingTrack", trackId: timeline.tracks[existing].id },
          entry,
        };
  });
}

// MARK: - Text tool (Toolbar "T" button, SPEC §4)

/** Default text clip duration: 3 seconds at the timeline's fps. */
const DEFAULT_TEXT_SECONDS = 3;

/** Default transform for a newly created text clip (centered, unit size). */
const DEFAULT_TEXT_TRANSFORM: Transform = {
  centerX: 0.5,
  centerY: 0.5,
  width: 1,
  height: 1,
  rotation: 0,
  flipHorizontal: false,
  flipVertical: false,
};

const DEFAULT_TEXT_STYLE: TextStyle = {
  fontName: "Helvetica-Bold",
  fontSize: 96,
  fontScale: 1,
  color: { r: 1, g: 1, b: 1, a: 1 },
  alignment: "center",
  shadow: {
    enabled: true,
    color: { r: 0, g: 0, b: 0, a: 0.6 },
    offsetX: 0,
    offsetY: -2,
    blur: 6,
  },
  background: {
    enabled: false,
    color: { r: 0, g: 0, b: 0, a: 0.6 },
  },
  border: {
    enabled: false,
    color: { r: 0, g: 0, b: 0, a: 1 },
  },
};

/** Add a text clip at the playhead on a fresh top track. Selects the new clip
 *  afterwards so the Inspector opens its Text tab. Used by the Toolbar "T"
 *  button.
 *
 *  Always targets a brand-new track via `addTextsAutoTrack`, mirroring
 *  upstream `addTextClip` (`EditorViewModel+MediaLibrary.swift:519-558`),
 *  which unconditionally does `insertTrack(at: 0, type: .video)`. This used
 *  to reuse the first existing visual track instead — a top video/image
 *  track could hold unrelated content that `addTexts`' overwrite semantics
 *  would clear to make room for the new text clip (#194). */
export async function addTextClip() {
  const ui = useEditorUiStore.getState();
  const startFrame = ui.activeFrame;
  const timeline = currentTimeline();

  const durationFrames = durationSecondsToFrames(DEFAULT_TEXT_SECONDS, timeline.fps);
  const entry: TextAutoTrackEntryReq = {
    startFrame,
    durationFrames,
    content: "",
    textStyle: DEFAULT_TEXT_STYLE,
    transform: DEFAULT_TEXT_TRANSFORM,
  };

  const res = await addTextsAutoTrack([entry]);
  // Tauri's timeline_changed event refreshes the mirror asynchronously; force
  // it synchronously so the mirror (and any caller reading it right after,
  // e.g. selection logic elsewhere) reflects the new track immediately —
  // same pattern as addMediaToTimeline.
  if (isTauri) await forceRefresh();
  if (res && res.affectedClipIds.length > 0) {
    ui.selectClips(new Set(res.affectedClipIds));
  }
}

// MARK: - Captions (Captions tab / add_captions)

/** Place a batch of pre-built caption entries on one fresh track as a single
 *  undoable action (`AddCaptions`). Thin wrapper mirroring the other editActions;
 *  the Captions tab normally calls {@link generateCaptions} (which builds + places
 *  in Rust), but this exists for callers holding ready-made caption entries. */
export async function addCaptions(entries: CaptionEntryReq[]) {
  if (entries.length === 0) return;
  return applyAndRefresh({ type: "addCaptions", entries });
}

/** Run the full caption pipeline for `request` (transcribe + build + place in
 *  Rust) and refresh the mirror. Returns the caption count so the UI can report
 *  "no speech detected" (count 0) distinctly from a placed batch. */
export async function generateCaptions(request: CaptionRequest) {
  const result = await api.generateCaptions(request);
  // A placed batch changed the timeline. Force a mirror refresh so it appears
  // even if Tauri's timeline_changed event races (and the browser has no event
  // channel at all), matching the other editActions' refresh discipline.
  if (result.edit.changed) await forceRefresh();
  return result;
}

// MARK: - Clipboard (copy / cut / paste, Issue #94)
//
// Front-end paste buffer: copy snapshots the selected clips; paste re-places
// them at the playhead with a fresh `linkGroupId` (cleared so the backend
// re-assigns, mirroring upstream `pasteClipsAtPlayhead` link re-reflection).
// Track placement is preserved (clip stays on its original track index); if
// the target track no longer exists the clip is skipped.

/** Collect selected clips with their track index into the clipboard store.
 *  If any selected clip belongs to a link group, the entire group is copied
 *  (mirrors upstream `copyClips` which expands the selection to include
 *  linked companions, so a paste reproduces the video+audio pair). */
export function copyClips() {
  const ui = useEditorUiStore.getState();
  const tl = currentTimeline();
  const ids = ui.selectedClipIds;
  if (ids.size === 0) return;
  // Expand selection to include linked companions.
  const expanded = new Set<string>(ids);
  for (let ti = 0; ti < tl.tracks.length; ti++) {
    for (const clip of tl.tracks[ti].clips) {
      if (ids.has(clip.id) && clip.linkGroupId) {
        for (let tj = 0; tj < tl.tracks.length; tj++) {
          for (const c2 of tl.tracks[tj].clips) {
            if (c2.linkGroupId === clip.linkGroupId) expanded.add(c2.id);
          }
        }
      }
    }
  }
  const entries: { clip: Clip; sourceTrackIndex: number }[] = [];
  for (let ti = 0; ti < tl.tracks.length; ti++) {
    for (const clip of tl.tracks[ti].clips) {
      if (expanded.has(clip.id)) entries.push({ clip, sourceTrackIndex: ti });
    }
  }
  if (entries.length === 0) return;
  const sourceFirstFrame = entries.reduce(
    (min, e) => Math.min(min, e.clip.startFrame),
    Number.POSITIVE_INFINITY,
  );
  useClipboardStore.getState().set(entries, sourceFirstFrame);
}

/** Copy then delete — the standard cut semantics. */
export async function cutClips() {
  copyClips();
  await deleteSelectedClips();
}

/** Paste clipboard entries at the current playhead. Each clip's `startFrame`
 *  is offset by `activeFrame - sourceFirstFrame`. After the clips are created,
 *  link groups are re-established: clips that shared a `linkGroupId` in the
 *  clipboard are re-linked via `linkClips` so the paste preserves video+audio
 *  linkage. Clips whose source track no longer exists are silently skipped
 *  (upstream drops them too). */
export async function pasteClipsAtPlayhead() {
  const cb = useClipboardStore.getState();
  if (!cb.hasContent || cb.entries.length === 0) return;
  const ui = useEditorUiStore.getState();
  const tl = currentTimeline();
  const offset = ui.activeFrame - cb.sourceFirstFrame;
  const entries: PasteClipEntryReq[] = [];
  for (const e of cb.entries) {
    if (e.sourceTrackIndex >= tl.tracks.length) continue;
    entries.push({
      clip: structuredClone(e.clip),
      targetTrackId: tl.tracks[e.sourceTrackIndex].id,
      startFrame: Math.max(0, e.clip.startFrame + offset),
    });
  }
  if (entries.length === 0) return;
  try {
    const res = await pasteClips(entries);
    if (!res || res.affectedClipIds.length === 0) return;
    // Select the freshly pasted clips so the user can immediately move/trim them.
    ui.selectClips(new Set(res.affectedClipIds));
    if (isTauri) await forceRefresh();
  } catch (error) {
    ui.pushToast(t("edit.pasteFailed", { error: String(error) }));
  }
}
