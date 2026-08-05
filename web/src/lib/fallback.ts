/**
 * Browser-only in-memory timeline fallback (used when not running inside Tauri).
 * Mirrors a subset of the Rust command behavior so the UI shell is explorable
 * in a plain browser. NOT an editing engine — the authoritative truth is always
 * the Rust core under Tauri. Kept deliberately small.
 */

import type {
  AnimPair,
  Clip,
  Crop,
  EditRequest,
  EditResult,
  KeyframeTrack,
  Timeline,
  TimelineSnapshot,
  Track,
  Transform,
} from "./types";

function defaultTransform() {
  return {
    centerX: 0.5,
    centerY: 0.5,
    width: 1,
    height: 1,
    rotation: 0,
    flipHorizontal: false,
    flipVertical: false,
  };
}
function defaultCrop() {
  return { left: 0, top: 0, right: 0, bottom: 0 };
}

function defaultRgb() {
  return { r: 1, g: 1, b: 1 };
}

function normalizeRgb(input: Partial<ReturnType<typeof defaultRgb>> | undefined, fallback = defaultRgb()) {
  return {
    r: input?.r ?? fallback.r,
    g: input?.g ?? fallback.g,
    b: input?.b ?? fallback.b,
  };
}

function normalizeColorGrade(
  grade: Extract<EditRequest, { type: "setColorGrade" }>["grade"],
): NonNullable<Clip["colorGrade"]> | undefined {
  if (grade == null) return undefined;
  return {
    exposure: grade.exposure ?? 0,
    temperature: grade.temperature ?? 0,
    tint: grade.tint ?? 0,
    liftGammaGain: {
      lift: normalizeRgb(grade.liftGammaGain?.lift, { r: 0, g: 0, b: 0 }),
      gamma: normalizeRgb(grade.liftGammaGain?.gamma),
      gain: normalizeRgb(grade.liftGammaGain?.gain),
    },
    contrast: grade.contrast ?? 0,
    saturation: grade.saturation ?? 1,
    hslSecondary: grade.hslSecondary
      ? {
          hueCenter: grade.hslSecondary.hueCenter ?? 0,
          hueWidth: grade.hslSecondary.hueWidth ?? 0.24,
          feather: grade.hslSecondary.feather ?? 0.08,
          hueShift: grade.hslSecondary.hueShift ?? 0,
          saturation: grade.hslSecondary.saturation ?? 0,
          lightness: grade.hslSecondary.lightness ?? 0,
        }
      : undefined,
  };
}

function isValidColorGrade(grade: NonNullable<Clip["colorGrade"]>): boolean {
  const finiteRange = (value: number, min: number, max: number) =>
    Number.isFinite(value) && value >= min && value <= max;
  const { lift, gamma, gain } = grade.liftGammaGain;
  const secondary = grade.hslSecondary;
  return (
    finiteRange(grade.exposure, -5, 5) &&
    finiteRange(grade.temperature, -1, 1) &&
    finiteRange(grade.tint, -1, 1) &&
    [lift.r, lift.g, lift.b].every((value) => finiteRange(value, -1, 1)) &&
    [gamma.r, gamma.g, gamma.b].every((value) => Number.isFinite(value) && value > 0 && value <= 4) &&
    [gain.r, gain.g, gain.b].every((value) => finiteRange(value, 0, 4)) &&
    finiteRange(grade.contrast, -1, 2) &&
    finiteRange(grade.saturation, 0, 3) &&
    (secondary == null ||
      (finiteRange(secondary.hueCenter, 0, 1) &&
        Number.isFinite(secondary.hueWidth) && secondary.hueWidth > 0 && secondary.hueWidth <= 1 &&
        finiteRange(secondary.feather, 0, 0.5) &&
        finiteRange(secondary.hueShift, -0.5, 0.5) &&
        finiteRange(secondary.saturation, -1, 1) &&
        finiteRange(secondary.lightness, -1, 1)))
  );
}

function normalizeChromaKey(
  chromaKey: Extract<EditRequest, { type: "setChromaKey" }>["chromaKey"],
): NonNullable<Clip["chromaKey"]> | undefined {
  if (chromaKey == null) return undefined;
  return {
    keyColor: normalizeRgb(chromaKey.keyColor, { r: 0, g: 1, b: 0 }),
    similarity: chromaKey.similarity ?? 0.15,
    smoothness: chromaKey.smoothness ?? 0.35,
    spill: chromaKey.spill ?? 0.5,
  };
}

function normalizeMask(mask: Extract<EditRequest, { type: "setMasks" }>["masks"][number]): NonNullable<Clip["masks"]>[number] {
  return {
    shape: mask.shape ?? {
      kind: "circle",
      center: { x: 0.5, y: 0.5 },
      radius: { x: 1.5, y: 1.5 },
    },
    feather: mask.feather ?? 0,
    invert: mask.invert ?? false,
    transform: mask.transform ?? {
      offset: { x: 0, y: 0 },
      scale: { x: 1, y: 1 },
      rotationDegrees: 0,
    },
  };
}

function normalizeEffect(effect: Extract<EditRequest, { type: "setEffects" }>["effects"][number]): NonNullable<Clip["effects"]>[number] {
  return {
    name: effect.name,
    params: { ...(effect.params ?? {}) },
    enabled: effect.enabled ?? true,
  };
}

function isVisual(type: Clip["mediaType"]): boolean {
  return type !== "audio";
}

function trackTypeCompatible(
  trackType: Track["type"],
  mediaType: Clip["mediaType"],
): boolean {
  return mediaType === "audio" ? trackType === "audio" : isVisual(trackType);
}

const I32_MIN = -2_147_483_648;
const I32_MAX = 2_147_483_647;

function isI32(value: number): boolean {
  return Number.isInteger(value) && value >= I32_MIN && value <= I32_MAX;
}

function checkedI32Add(left: number, right: number): number | null {
  if (!isI32(left) || !isI32(right)) return null;
  const value = left + right;
  return isI32(value) ? value : null;
}

function checkedScaledFrame(value: number, speed: number): number | null {
  if (!isI32(value) || !Number.isFinite(speed) || speed <= 0) return null;
  const scaled = Math.round(value * speed);
  return isI32(scaled) ? scaled : null;
}

function checkedFrameEnd(
  startFrame: number,
  durationFrames: number,
  trimStartFrame = 0,
  trimEndFrame = 0,
  speed = 1,
  allowNegativeTrims = false,
): number | null {
  if (
    !isI32(startFrame) ||
    !isI32(durationFrames) ||
    !isI32(trimStartFrame) ||
    !isI32(trimEndFrame) ||
    startFrame < 0 ||
    durationFrames < 1 ||
    (!allowNegativeTrims && (trimStartFrame < 0 || trimEndFrame < 0)) ||
    !Number.isFinite(speed) ||
    speed <= 0
  ) {
    return null;
  }
  const endFrame = startFrame + durationFrames;
  const rawSourceExtent = durationFrames + trimStartFrame + trimEndFrame;
  const consumed = Math.round(durationFrames * speed);
  const sourceExtent = trimStartFrame + consumed + trimEndFrame;
  const trimStartExtent = trimStartFrame + consumed;
  const trimEndExtent = trimEndFrame + consumed;
  if (
    !isI32(endFrame) ||
    !isI32(rawSourceExtent) ||
    !isI32(consumed) ||
    consumed < 0 ||
    !isI32(trimStartExtent) ||
    !isI32(trimEndExtent) ||
    !isI32(sourceExtent)
  ) {
    return null;
  }
  return endFrame;
}

function checkedClipEnd(clip: Clip, startFrame = clip.startFrame): number | null {
  return checkedFrameEnd(
    startFrame,
    clip.durationFrames,
    clip.trimStartFrame,
    clip.trimEndFrame,
    clip.speed,
    clip.mediaType === "image" || clip.mediaType === "text",
  );
}

function timelineFrameArithmeticIsSafe(target: Timeline): boolean {
  return (
    target.tracks.every((track) => track.clips.every((clip) => checkedClipEnd(clip) !== null)) &&
    (target.nestedSequences ?? []).every((sequence) =>
      timelineFrameArithmeticIsSafe(sequence.timeline)
    )
  );
}

function settingsFrameProjectionIsSafe(target: Timeline, fps: number): boolean {
  if (
    !(target.nestedSequences ?? []).every((sequence) =>
      settingsFrameProjectionIsSafe(sequence.timeline, fps)
    )
  ) {
    return false;
  }
  if (target.fps <= 0 || target.fps === fps) return true;
  const scale = fps / target.fps;
  return target.tracks.every((track) => {
    const ordered = [...track.clips].sort((left, right) => left.startFrame - right.startFrame);
    let previousEnd: number | undefined;
    for (const clip of ordered) {
      const sourceEnd = checkedClipEnd(clip);
      if (sourceEnd === null) return false;
      const scaledStart = Math.round(clip.startFrame * scale);
      const scaledEnd = Math.round(sourceEnd * scale);
      const startFrame = Math.max(scaledStart, previousEnd ?? scaledStart);
      const durationFrames = Math.max(1, scaledEnd - startFrame);
      const trimStartFrame = Math.round(clip.trimStartFrame * scale);
      const trimEndFrame = Math.round(clip.trimEndFrame * scale);
      const endFrame = checkedFrameEnd(
        startFrame,
        durationFrames,
        trimStartFrame,
        trimEndFrame,
        clip.speed,
        clip.mediaType === "image" || clip.mediaType === "text",
      );
      if (endFrame === null) return false;
      previousEnd = endFrame;
    }
    return true;
  });
}

function interpolateNumber(from: number, to: number, amount: number): number {
  return from + (to - from) * amount;
}

function sampleKeyframeTrack<Value>(
  track: KeyframeTrack<Value>,
  frame: number,
  fallback: Value,
  interpolate: (from: Value, to: Value, amount: number) => Value,
): Value {
  const keyframes = track.keyframes;
  if (keyframes.length === 0) return structuredClone(fallback);
  if (keyframes.length === 1 || frame <= keyframes[0].frame) {
    return structuredClone(keyframes[0].value);
  }
  const last = keyframes[keyframes.length - 1];
  if (frame >= last.frame) return structuredClone(last.value);
  const rightIndex = keyframes.findIndex((keyframe) => keyframe.frame > frame);
  const left = keyframes[rightIndex - 1];
  const right = keyframes[rightIndex];
  const raw = (frame - left.frame) / (right.frame - left.frame);
  const amount = left.interpolationOut === "hold"
    ? 0
    : left.interpolationOut === "smooth"
      ? raw * raw * (3 - 2 * raw)
      : raw;
  return interpolate(left.value, right.value, amount);
}

function splitKeyframeTrack<Value>(
  track: KeyframeTrack<Value> | undefined,
  splitOffset: number,
  fallback: Value,
  interpolate: (from: Value, to: Value, amount: number) => Value,
): [KeyframeTrack<Value> | undefined, KeyframeTrack<Value> | undefined] {
  if (!track) return [undefined, undefined];
  if (track.keyframes.length === 0) {
    return [structuredClone(track), structuredClone(track)];
  }
  const boundary = sampleKeyframeTrack(track, splitOffset, fallback, interpolate);
  const left = track.keyframes
    .filter((keyframe) => keyframe.frame <= splitOffset)
    .map((keyframe) => structuredClone(keyframe));
  if (left[left.length - 1]?.frame !== splitOffset) {
    left.push({
      frame: splitOffset,
      value: structuredClone(boundary),
      interpolationOut: "smooth",
    });
  }
  const right = track.keyframes
    .filter((keyframe) => keyframe.frame >= splitOffset)
    .map((keyframe) => ({
      ...structuredClone(keyframe),
      frame: keyframe.frame - splitOffset,
    }));
  if (right[0]?.frame !== 0) {
    right.unshift({
      frame: 0,
      value: structuredClone(boundary),
      interpolationOut: "smooth",
    });
  }
  return [
    left.length > 0 ? { keyframes: left } : undefined,
    right.length > 0 ? { keyframes: right } : undefined,
  ];
}

function interpolatePair(from: AnimPair, to: AnimPair, amount: number): AnimPair {
  return {
    a: interpolateNumber(from.a, to.a, amount),
    b: interpolateNumber(from.b, to.b, amount),
  };
}

function interpolateCrop(from: Crop, to: Crop, amount: number): Crop {
  return {
    left: interpolateNumber(from.left, to.left, amount),
    top: interpolateNumber(from.top, to.top, amount),
    right: interpolateNumber(from.right, to.right, amount),
    bottom: interpolateNumber(from.bottom, to.bottom, amount),
  };
}

function pruneInvalidTransitions(target: Timeline): void {
  for (const sequence of target.nestedSequences ?? []) {
    pruneInvalidTransitions(sequence.timeline);
  }
  for (const track of target.tracks) {
    if (track.type === "audio") {
      for (const clip of track.clips) clip.transitionOut = undefined;
      continue;
    }
    const ordered = [...track.clips].sort((left, right) => {
      if (left.startFrame !== right.startFrame) return left.startFrame - right.startFrame;
      return left.id === right.id ? 0 : left.id < right.id ? -1 : 1;
    });
    const valid = new Map<string, { toId: string; maximum: number }>();
    for (let index = 0; index + 1 < ordered.length; index++) {
      const from = ordered[index];
      const to = ordered[index + 1];
      if (
        checkedClipEnd(from) !== to.startFrame ||
        from.mediaType === "audio" ||
        from.mediaType === "text" ||
        to.mediaType === "audio" ||
        to.mediaType === "text"
      ) {
        continue;
      }
      valid.set(from.id, {
        toId: to.id,
        maximum: Math.max(1, Math.floor(Math.min(from.durationFrames, to.durationFrames) / 2)),
      });
    }
    for (const clip of track.clips) {
      const transition = clip.transitionOut;
      if (!transition) continue;
      const boundary = valid.get(clip.id);
      if (
        !boundary ||
        (transition.fromClipId !== "" && transition.fromClipId !== clip.id) ||
        transition.toClipId !== boundary.toId
      ) {
        clip.transitionOut = undefined;
        continue;
      }
      transition.fromClipId = clip.id;
      transition.durationFrames = Math.min(
        boundary.maximum,
        Math.max(1, transition.durationFrames),
      );
    }
  }
}

function planSplitHalves(clip: Clip, atFrame: number): [Clip, Clip] | null {
  const clipEnd = checkedClipEnd(clip);
  if (
    clipEnd === null ||
    !isI32(atFrame) ||
    atFrame <= clip.startFrame ||
    atFrame >= clipEnd
  ) {
    return null;
  }
  const leftDuration = atFrame - clip.startFrame;
  const rightDuration = clipEnd - atFrame;
  const leftSource = checkedScaledFrame(leftDuration, clip.speed);
  const rightSource = checkedScaledFrame(rightDuration, clip.speed);
  if (leftSource === null || rightSource === null) return null;

  const left = structuredClone(clip);
  const right = structuredClone(clip);
  left.durationFrames = leftDuration;
  right.startFrame = atFrame;
  right.durationFrames = rightDuration;
  if (clip.reversed) {
    const leftTrimStart = checkedI32Add(clip.trimStartFrame, rightSource);
    const rightTrimEnd = checkedI32Add(clip.trimEndFrame, leftSource);
    if (leftTrimStart === null || rightTrimEnd === null) return null;
    left.trimStartFrame = leftTrimStart;
    right.trimEndFrame = rightTrimEnd;
  } else {
    const leftTrimEnd = checkedI32Add(clip.trimEndFrame, rightSource);
    const rightTrimStart = checkedI32Add(clip.trimStartFrame, leftSource);
    if (leftTrimEnd === null || rightTrimStart === null) return null;
    left.trimEndFrame = leftTrimEnd;
    right.trimStartFrame = rightTrimStart;
  }

  left.fadeOutFrames = 0;
  left.fadeInFrames = Math.min(Math.max(0, left.fadeInFrames), leftDuration);
  right.fadeInFrames = 0;
  right.fadeOutFrames = Math.min(Math.max(0, right.fadeOutFrames), rightDuration);
  left.loudnessNormalization = undefined;
  right.loudnessNormalization = undefined;

  [left.opacityTrack, right.opacityTrack] = splitKeyframeTrack(
    clip.opacityTrack,
    leftDuration,
    clip.opacity,
    interpolateNumber,
  );
  [left.volumeTrack, right.volumeTrack] = splitKeyframeTrack(
    clip.volumeTrack,
    leftDuration,
    clip.volume,
    interpolateNumber,
  );
  [left.positionTrack, right.positionTrack] = splitKeyframeTrack(
    clip.positionTrack,
    leftDuration,
    { a: 0, b: 0 },
    interpolatePair,
  );
  [left.scaleTrack, right.scaleTrack] = splitKeyframeTrack(
    clip.scaleTrack,
    leftDuration,
    { a: 1, b: 1 },
    interpolatePair,
  );
  [left.rotationTrack, right.rotationTrack] = splitKeyframeTrack(
    clip.rotationTrack,
    leftDuration,
    0,
    interpolateNumber,
  );
  [left.cropTrack, right.cropTrack] = splitKeyframeTrack(
    clip.cropTrack,
    leftDuration,
    clip.crop,
    interpolateCrop,
  );

  return checkedClipEnd(left) !== null && checkedClipEnd(right) !== null
    ? [left, right]
    : null;
}

function upsertSmoothKeyframe<Value>(
  track: KeyframeTrack<Value>,
  frame: number,
  value: Value,
): void {
  const keyframe = { frame, value, interpolationOut: "smooth" as const };
  const existing = track.keyframes.findIndex((candidate) => candidate.frame === frame);
  if (existing >= 0) {
    track.keyframes[existing] = keyframe;
  } else {
    const index = track.keyframes.findIndex((candidate) => candidate.frame > frame);
    track.keyframes.splice(index < 0 ? track.keyframes.length : index, 0, keyframe);
  }
}

type AddClipEntry = Extract<EditRequest, { type: "addClips" }>["entries"][number];

function newClipFromEntry(id: string, entry: AddClipEntry): Clip {
  return {
    id,
    mediaRef: entry.mediaRef,
    mediaType: entry.mediaType,
    sourceClipType: entry.sourceClipType,
    startFrame: entry.startFrame,
    durationFrames: entry.durationFrames,
    trimStartFrame: entry.trimStartFrame ?? 0,
    trimEndFrame: entry.trimEndFrame ?? 0,
    speed: 1,
    reversed: false,
    volume: 1,
    fadeInFrames: 0,
    fadeOutFrames: 0,
    fadeInInterpolation: "linear",
    fadeOutInterpolation: "linear",
    opacity: 1,
    transform: entry.transform ?? defaultTransform(),
    crop: defaultCrop(),
  };
}

function newClip(
  id: string,
  mediaRef: string,
  type: Clip["mediaType"],
  startFrame: number,
  durationFrames: number,
): Clip {
  return {
    id,
    mediaRef,
    mediaType: type,
    sourceClipType: type,
    startFrame,
    durationFrames,
    trimStartFrame: 0,
    trimEndFrame: 0,
    speed: 1,
    reversed: false,
    volume: 1,
    fadeInFrames: 0,
    fadeOutFrames: 0,
    fadeInInterpolation: "linear",
    fadeOutInterpolation: "linear",
    opacity: 1,
    transform: defaultTransform(),
    crop: defaultCrop(),
  };
}

/** A small demo timeline so the canvas shows something in a browser preview. */
function demoTimeline(): Timeline {
  const v1: Track = {
    id: "t-v1",
    type: "video",
    muted: false,
    hidden: false,
    syncLocked: true,
    clips: [
      newClip("c1", "demo-video", "video", 0, 90),
      newClip("c2", "demo-image", "image", 110, 60),
    ],
  };
  const a1: Track = {
    id: "t-a1",
    type: "audio",
    muted: false,
    hidden: false,
    syncLocked: true,
    clips: [newClip("c3", "demo-audio", "audio", 0, 150)],
  };
  return {
    fps: 30,
    width: 1920,
    height: 1080,
    settingsConfigured: true,
    tracks: [v1, a1],
  };
}

export function createFallbackStore(initialTimeline?: Timeline) {
  let timeline: Timeline = initialTimeline ? structuredClone(initialTimeline) : demoTimeline();
  let version = 0;
  let idSeq = 100;
  const nextId = () => `c${idSeq++}`;
  const nextTrackId = () => `t${idSeq++}`;

  function snapshot(): TimelineSnapshot {
    return { timeline: structuredClone(timeline), version };
  }

  function bump() {
    version += 1;
  }

  function findClip(id: string): [number, number] | null {
    for (let ti = 0; ti < timeline.tracks.length; ti++) {
      const ci = timeline.tracks[ti].clips.findIndex((c) => c.id === id);
      if (ci >= 0) return [ti, ci];
    }
    return null;
  }

  function findAllClips(ids: string[]): Array<[number, number]> | null {
    const locations: Array<[number, number]> = [];
    for (const id of ids) {
      const loc = findClip(id);
      if (!loc) return null;
      locations.push(loc);
    }
    return locations;
  }

  function splitClipsAtomically(
    clipIds: string[],
    atFrame: number,
    actionName: "Split Clip" | "Split Clips",
  ): EditResult {
    if (clipIds.length === 0 || !isI32(atFrame)) {
      return result(false, actionName, []);
    }

    const requested: string[] = [];
    const seenIds = new Set<string>();
    for (const clipId of clipIds) {
      if (seenIds.has(clipId)) continue;
      seenIds.add(clipId);
      const location = findClip(clipId);
      if (!location) return result(false, actionName, []);
      const clip = timeline.tracks[location[0]].clips[location[1]];
      if (!planSplitHalves(clip, atFrame)) return result(false, actionName, []);
      requested.push(clipId);
    }

    const seeds: string[] = [];
    const seenGroups = new Set<string>();
    for (const clipId of requested) {
      const location = findClip(clipId)!;
      const group = timeline.tracks[location[0]].clips[location[1]].linkGroupId;
      if (group && seenGroups.has(group)) continue;
      if (group) seenGroups.add(group);
      seeds.push(clipId);
    }

    type MemberPlan = { clipId: string; left: Clip; right: Clip };
    const groups: Array<{ totalMembers: number; members: MemberPlan[] }> = [];
    for (const seedId of seeds) {
      const seedLocation = findClip(seedId)!;
      const seed = timeline.tracks[seedLocation[0]].clips[seedLocation[1]];
      const groupMembers = seed.linkGroupId
        ? [
            seed,
            ...timeline.tracks
              .flatMap((track) => track.clips)
              .filter(
                (clip) =>
                  clip.id !== seed.id && clip.linkGroupId === seed.linkGroupId,
              ),
          ]
        : [seed];
      const members: MemberPlan[] = [];
      for (const clip of groupMembers) {
        const clipEnd = checkedClipEnd(clip);
        if (
          clipEnd === null ||
          atFrame <= clip.startFrame ||
          atFrame >= clipEnd
        ) {
          continue;
        }
        const halves = planSplitHalves(clip, atFrame);
        if (!halves) return result(false, actionName, []);
        members.push({ clipId: clip.id, left: halves[0], right: halves[1] });
      }
      groups.push({ totalMembers: groupMembers.length, members });
    }

    const beforeTimeline = structuredClone(timeline);
    const beforeIdSeq = idSeq;
    const refuse = (): EditResult => {
      Object.assign(timeline, beforeTimeline);
      idSeq = beforeIdSeq;
      return result(false, actionName, []);
    };
    const affected: string[] = [];
    for (const group of groups) {
      const groupRights: string[] = [];
      for (const member of group.members) {
        const location = findClip(member.clipId);
        if (!location) return refuse();
        const left = structuredClone(member.left);
        const right = structuredClone(member.right);
        right.id = nextId();
        timeline.tracks[location[0]].clips.splice(location[1], 1, left, right);
        timeline.tracks[location[0]].clips.sort(
          (first, second) => first.startFrame - second.startFrame,
        );
        groupRights.push(right.id);
        affected.push(right.id);
      }
      if (group.totalMembers > 1 && groupRights.length > 0) {
        const rightGroup = nextId();
        for (const rightId of groupRights) {
          const location = findClip(rightId);
          if (!location) return refuse();
          timeline.tracks[location[0]].clips[location[1]].linkGroupId = rightGroup;
        }
      }
    }
    pruneInvalidTransitions(timeline);
    if (affected.length === 0 || !timelineFrameArithmeticIsSafe(timeline)) return refuse();
    return result(true, actionName, affected);
  }

  function insertionIndex(kind: Clip["mediaType"], requested = timeline.tracks.length): number {
    const firstAudio = timeline.tracks.findIndex((track) => track.type === "audio");
    const firstAudioIndex = firstAudio >= 0 ? firstAudio : timeline.tracks.length;
    const bounded = Math.max(0, Math.min(timeline.tracks.length, requested));
    if (kind === "audio") return Math.max(bounded, firstAudioIndex);
    return Math.min(bounded, firstAudioIndex);
  }

  function trackCompatible(track: Track, type: Clip["mediaType"]): boolean {
    return trackTypeCompatible(track.type, type);
  }

  function clearRegionSpan(trackIndex: number, startFrame: number, endFrame: number): boolean {
    const track = timeline.tracks[trackIndex];
    if (!track || !isI32(startFrame) || !isI32(endFrame) || startFrame < 0 || endFrame <= startFrame) {
      return false;
    }
    const planned: Array<{ clip: Clip; freshId: boolean }> = [];
    for (const clip of track.clips) {
      const clipEnd = checkedClipEnd(clip);
      if (clipEnd === null) return false;
      if (clipEnd <= startFrame || clip.startFrame >= endFrame) {
        planned.push({ clip, freshId: false });
        continue;
      }
      if (clip.startFrame < startFrame) {
        const durationFrames = startFrame - clip.startFrame;
        const removedFrames = clipEnd - startFrame;
        const sourceDelta = checkedScaledFrame(removedFrames, clip.speed);
        const trimEndFrame = sourceDelta === null
          ? null
          : checkedI32Add(clip.trimEndFrame, sourceDelta);
        if (!isI32(durationFrames) || durationFrames < 1 || trimEndFrame === null) return false;
        const left = {
          ...structuredClone(clip),
          durationFrames,
          trimEndFrame,
          fadeOutFrames: 0,
          loudnessNormalization: undefined,
        };
        if (checkedClipEnd(left) === null) return false;
        planned.push({ clip: left, freshId: false });
      }
      if (clipEnd > endFrame) {
        const durationFrames = clipEnd - endFrame;
        const removedFrames = endFrame - clip.startFrame;
        const sourceDelta = checkedScaledFrame(removedFrames, clip.speed);
        const trimStartFrame = sourceDelta === null
          ? null
          : checkedI32Add(clip.trimStartFrame, sourceDelta);
        if (!isI32(durationFrames) || durationFrames < 1 || trimStartFrame === null) return false;
        const right = structuredClone(clip);
        right.startFrame = endFrame;
        right.durationFrames = durationFrames;
        right.trimStartFrame = trimStartFrame;
        right.fadeInFrames = 0;
        right.loudnessNormalization = undefined;
        if (checkedClipEnd(right) === null) return false;
        planned.push({ clip: right, freshId: true });
      }
    }
    for (const item of planned) {
      if (item.freshId) item.clip.id = nextId();
    }
    track.clips = planned.map((item) => item.clip).sort((a, b) => a.startFrame - b.startFrame);
    return true;
  }

  function resolveOrCreateAudioTrack(
    startFrame: number,
    validatedEndFrame: number,
  ): number {
    const endFrame = validatedEndFrame;
    for (let i = 0; i < timeline.tracks.length; i++) {
      const track = timeline.tracks[i];
      if (track.type !== "audio") continue;
      const overlaps = track.clips.some(
        (clip) => clip.startFrame < endFrame && checkedClipEnd(clip)! > startFrame,
      );
      if (!overlaps) return i;
    }
    const index = insertionIndex("audio");
    timeline.tracks.splice(index, 0, {
      id: nextTrackId(),
      type: "audio",
      muted: false,
      hidden: false,
      syncLocked: true,
      clips: [],
    });
    return index;
  }

  function result(changed: boolean, actionName: string, affected: string[]): EditResult {
    if (changed) bump();
    return {
      changed,
      actionName,
      affectedClipIds: affected,
      timelineVersion: version,
      summary: actionName,
    };
  }

  let dispatchFallback: (command: EditRequest) => EditResult;
  const store = {
    getTimeline: (): TimelineSnapshot => snapshot(),
    reset: () => {
      timeline = { fps: 30, width: 1920, height: 1080, settingsConfigured: false, tracks: [] };
      bump();
    },
    noop: (name: string): EditResult => result(false, name, []),
    editApply: (cmd: EditRequest): EditResult => {
      if (!timelineFrameArithmeticIsSafe(timeline)) {
        return result(false, cmd.type, []);
      }
      switch (cmd.type) {
        case "editNestedSequence": {
          const root = timeline;
          const child = root.nestedSequences?.find((sequence) => sequence.id === cmd.sequenceId);
          if (!child) return result(false, "Edit Compound Clip", []);
          timeline = child.timeline;
          try {
            return dispatchFallback(cmd.command);
          } finally {
            timeline = root;
          }
        }
        case "placeMedia": {
          const root = timeline;
          const beforeRoot = structuredClone(root);
          const beforeIdSeq = idSeq;
          const refuseAfterMutation = (): EditResult => {
            Object.assign(root, beforeRoot);
            idSeq = beforeIdSeq;
            return result(false, "Place Media", []);
          };
          const targetTimeline = cmd.sequenceId
            ? root.nestedSequences?.find((sequence) => sequence.id === cmd.sequenceId)?.timeline
            : root;
          const expectedTrackType: Clip["mediaType"] =
            cmd.entry.mediaType === "audio" ? "audio" : "video";
          const existingTrackId = cmd.target.kind === "existingTrack" ? cmd.target.trackId : undefined;
          const existingTarget = existingTrackId !== undefined
            ? targetTimeline?.tracks.find((track) => track.id === existingTrackId)
            : undefined;
          const entryEnd = checkedFrameEnd(
            cmd.entry.startFrame,
            cmd.entry.durationFrames,
            cmd.entry.trimStartFrame ?? 0,
            cmd.entry.trimEndFrame ?? 0,
            1,
            cmd.entry.mediaType === "image" || cmd.entry.mediaType === "text",
          );
          const invalidEntry =
            entryEnd === null ||
            cmd.entry.mediaType !== cmd.entry.sourceClipType ||
            (cmd.entry.addLinkedAudio === true &&
              !(cmd.entry.mediaType === "video" && cmd.entry.hasAudio === true));
          const invalidSettings =
            cmd.settings !== undefined &&
            (!isI32(cmd.settings.fps) ||
              !isI32(cmd.settings.width) ||
              !isI32(cmd.settings.height) ||
              cmd.settings.fps <= 0 ||
              cmd.settings.width <= 0 ||
              cmd.settings.height <= 0);
          const invalidTarget =
            !targetTimeline ||
            (cmd.target.kind === "existingTrack" &&
              (!existingTarget || !trackCompatible(existingTarget, cmd.entry.mediaType))) ||
            (cmd.target.kind === "newTrack" && cmd.target.trackType !== expectedTrackType);
          const invalidTimeline = !timelineFrameArithmeticIsSafe(root);
          const invalidProjection = cmd.settings !== undefined &&
            !settingsFrameProjectionIsSafe(root, cmd.settings.fps);
          if (
            invalidEntry ||
            invalidSettings ||
            invalidTarget ||
            invalidTimeline ||
            invalidProjection
          ) {
            return result(false, "Place Media", []);
          }
          if (cmd.settings) {
            const applySettings = (target: Timeline) => {
              for (const sequence of target.nestedSequences ?? []) applySettings(sequence.timeline);
              if (target.fps > 0 && target.fps !== cmd.settings!.fps) {
                const scale = cmd.settings!.fps / target.fps;
                for (const track of target.tracks) {
                  const ordered = [...track.clips].sort((a, b) => a.startFrame - b.startFrame);
                  let previousEnd: number | undefined;
                  for (const clip of ordered) {
                    const sourceEnd = checkedClipEnd(clip)!;
                    const start = Math.round(clip.startFrame * scale);
                    const end = Math.round(sourceEnd * scale);
                    clip.startFrame = Math.max(start, previousEnd ?? start);
                    clip.durationFrames = Math.max(1, end - clip.startFrame);
                    clip.trimStartFrame = Math.round(clip.trimStartFrame * scale);
                    clip.trimEndFrame = Math.round(clip.trimEndFrame * scale);
                    previousEnd = checkedClipEnd(clip)!;
                  }
                }
              }
              target.fps = cmd.settings!.fps;
              target.width = cmd.settings!.width;
              target.height = cmd.settings!.height;
              target.settingsConfigured = true;
            };
            applySettings(root);
          }
          // The target was resolved and validated before any settings or track
          // mutation, preserving the composite command's failure atomicity.
          if (!targetTimeline) return result(false, "Place Media", []);
          timeline = targetTimeline;
          try {
            let trackIndex: number;
            if (cmd.target.kind === "existingTrack") {
              const trackId = cmd.target.trackId;
              trackIndex = timeline.tracks.findIndex((track) => track.id === trackId);
              if (trackIndex < 0) return refuseAfterMutation();
            } else {
              trackIndex = insertionIndex(cmd.target.trackType, cmd.target.at);
              timeline.tracks.splice(trackIndex, 0, {
                id: nextTrackId(),
                type: cmd.target.trackType === "audio" ? "audio" : "video",
                muted: false,
                hidden: false,
                syncLocked: true,
                clips: [],
              });
            }
            const track = timeline.tracks[trackIndex];
            if (!track || !trackCompatible(track, cmd.entry.mediaType)) {
              return refuseAfterMutation();
            }
            const entry: AddClipEntry = { ...cmd.entry, trackIndex };
            const clip = newClipFromEntry(nextId(), entry);
            const shouldLink = entry.addLinkedAudio === true
              && entry.hasAudio === true
              && track.type === "video"
              && entry.sourceClipType === "video";
            const group = shouldLink ? nextId() : undefined;
            clip.linkGroupId = group;
            if (!clearRegionSpan(trackIndex, clip.startFrame, entryEnd!)) {
              return refuseAfterMutation();
            }
            track.clips.push(clip);
            track.clips.sort((left, right) => left.startFrame - right.startFrame);
            const affected = [clip.id];
            if (group) {
              const audioTrackIndex = resolveOrCreateAudioTrack(
                clip.startFrame,
                entryEnd!,
              );
              const audio = newClipFromEntry(nextId(), { ...entry, mediaType: "audio" });
              audio.linkGroupId = group;
              if (!clearRegionSpan(audioTrackIndex, audio.startFrame, entryEnd!)) {
                return refuseAfterMutation();
              }
              timeline.tracks[audioTrackIndex].clips.push(audio);
              timeline.tracks[audioTrackIndex].clips.sort((left, right) => left.startFrame - right.startFrame);
              affected.push(audio.id);
            }
            if (!timelineFrameArithmeticIsSafe(root)) return refuseAfterMutation();
            return result(true, "Place Media", affected);
          } finally {
            timeline = root;
          }
        }
        case "insertTrack": {
          const index = insertionIndex(cmd.kind, cmd.at);
          const trackId = nextTrackId();
          timeline.tracks.splice(index, 0, {
            id: trackId,
            type: cmd.kind === "audio" ? "audio" : "video",
            muted: false,
            hidden: false,
            syncLocked: true,
            clips: [],
          });
          return result(true, "Insert Track", [trackId]);
        }
        case "addClips": {
          if (cmd.entries.length === 0) return result(false, "Add Clips", []);
          const beforeTimeline = structuredClone(timeline);
          const beforeIdSeq = idSeq;
          const refuse = (): EditResult => {
            Object.assign(timeline, beforeTimeline);
            idSeq = beforeIdSeq;
            return result(false, "Add Clips", []);
          };
          const plans: Array<{ entry: AddClipEntry; endFrame: number }> = [];
          for (const entry of cmd.entries) {
            const track = timeline.tracks[entry.trackIndex];
            const endFrame = checkedFrameEnd(
              entry.startFrame,
              entry.durationFrames,
              entry.trimStartFrame ?? 0,
              entry.trimEndFrame ?? 0,
              1,
              entry.mediaType === "image" || entry.mediaType === "text",
            );
            if (
              !Number.isInteger(entry.trackIndex) ||
              !track ||
              !trackCompatible(track, entry.mediaType) ||
              endFrame === null ||
              (entry.addLinkedAudio === true &&
                !(entry.mediaType === "video" &&
                  entry.sourceClipType === "video" &&
                  entry.hasAudio === true))
            ) {
              return result(false, "Add Clips", []);
            }
            plans.push({ entry, endFrame });
          }

          const affected: string[] = [];
          for (const { entry, endFrame } of plans) {
            const track = timeline.tracks[entry.trackIndex];
            const id = nextId();
            const clip = newClipFromEntry(id, entry);
            const shouldLink =
              entry.addLinkedAudio === true &&
              entry.hasAudio === true &&
              track.type === "video" &&
              entry.sourceClipType === "video";
            const linkGroupId = shouldLink ? nextId() : undefined;
            clip.linkGroupId = linkGroupId;
            if (!clearRegionSpan(entry.trackIndex, clip.startFrame, endFrame)) return refuse();
            track.clips.push(clip);
            track.clips.sort((a, b) => a.startFrame - b.startFrame);
            affected.push(id);
            if (shouldLink && linkGroupId) {
              const audioTrackIndex = resolveOrCreateAudioTrack(clip.startFrame, endFrame);
              const audio: Clip = {
                ...newClipFromEntry(nextId(), { ...entry, mediaType: "audio" }),
                linkGroupId,
              };
              if (!clearRegionSpan(audioTrackIndex, audio.startFrame, endFrame)) return refuse();
              timeline.tracks[audioTrackIndex].clips.push(audio);
              timeline.tracks[audioTrackIndex].clips.sort((a, b) => a.startFrame - b.startFrame);
              affected.push(audio.id);
            }
          }
          if (!timelineFrameArithmeticIsSafe(timeline)) return refuse();
          return result(affected.length > 0, affected.length === 1 ? "Add Clip" : "Add Clips", affected);
        }
        case "insertClips": {
          // Minimal ripple insert for the browser shell: push clips at/after
          // atFrame right by the total inserted duration on the target track,
          // then place the new clips. (Sync-lock / linked-audio ripple is a Rust
          // concern; the shell only needs the visible push on the target track.)
          const track = timeline.tracks[cmd.trackIndex];
          if (
            !Number.isInteger(cmd.trackIndex) ||
            !track ||
            cmd.entries.length === 0 ||
            !isI32(cmd.atFrame) ||
            cmd.atFrame < 0
          ) {
            return result(false, "Insert Clips", []);
          }
          let totalPush = 0;
          for (const entry of cmd.entries) {
            const nextTotal = checkedI32Add(totalPush, entry.durationFrames);
            if (
              !trackCompatible(track, entry.mediaType) ||
              checkedFrameEnd(
                0,
                entry.durationFrames,
                entry.trimStartFrame ?? 0,
                entry.trimEndFrame ?? 0,
                1,
                entry.mediaType === "image" || entry.mediaType === "text",
              ) === null ||
              nextTotal === null
            ) {
              return result(false, "Insert Clips", []);
            }
            totalPush = nextTotal;
          }

          const shiftedStarts = new Map<string, number>();
          for (const clip of track.clips) {
            if (clip.startFrame < cmd.atFrame) continue;
            const startFrame = checkedI32Add(clip.startFrame, totalPush);
            if (startFrame === null || checkedClipEnd(clip, startFrame) === null) {
              return result(false, "Insert Clips", []);
            }
            shiftedStarts.set(clip.id, startFrame);
          }

          const starts: number[] = [];
          let plannedCursor = cmd.atFrame;
          for (const entry of cmd.entries) {
            const endFrame = checkedFrameEnd(
              plannedCursor,
              entry.durationFrames,
              entry.trimStartFrame ?? 0,
              entry.trimEndFrame ?? 0,
              1,
              entry.mediaType === "image" || entry.mediaType === "text",
            );
            if (endFrame === null) return result(false, "Insert Clips", []);
            starts.push(plannedCursor);
            plannedCursor = endFrame;
          }

          const beforeTimeline = structuredClone(timeline);
          const beforeIdSeq = idSeq;
          for (const clip of track.clips) {
            const shifted = shiftedStarts.get(clip.id);
            if (shifted !== undefined) clip.startFrame = shifted;
          }
          const affected: string[] = [];
          for (let index = 0; index < cmd.entries.length; index++) {
            const entry = cmd.entries[index];
            const id = nextId();
            const clip = newClipFromEntry(id, { ...entry, startFrame: starts[index] });
            track.clips.push(clip);
            affected.push(id);
          }
          track.clips.sort((a, b) => a.startFrame - b.startFrame);
          if (!timelineFrameArithmeticIsSafe(timeline)) {
            // Every destination was prevalidated, so this is a defensive guard;
            // restore the source graph without consuming a visible version.
            Object.assign(timeline, beforeTimeline);
            idSeq = beforeIdSeq;
            return result(false, "Insert Clips", []);
          }
          return result(affected.length > 0, affected.length === 1 ? "Insert Clip" : "Insert Clips", affected);
        }
        case "removeClips": {
          let changed = false;
          for (const track of timeline.tracks) {
            const before = track.clips.length;
            track.clips = track.clips.filter((c) => !cmd.clipIds.includes(c.id));
            if (track.clips.length !== before) changed = true;
          }
          return result(changed, "Remove Clip", cmd.clipIds);
        }
        case "swapTracks": {
          const first = timeline.tracks[cmd.a];
          const second = timeline.tracks[cmd.b];
          if (!first || !second || first.type !== second.type || cmd.a === cmd.b) {
            return result(false, "Swap Tracks", []);
          }
          [timeline.tracks[cmd.a], timeline.tracks[cmd.b]] = [second, first];
          return result(true, "Swap Tracks", []);
        }
        case "swapClips": {
          const locA = findClip(cmd.clipA);
          const locB = findClip(cmd.clipB);
          if (!locA || !locB || cmd.clipA === cmd.clipB) return result(false, "Swap Clips", []);
          const [ta, ca] = locA;
          const [tb, cb] = locB;
          const clipA = timeline.tracks[ta].clips[ca];
          const clipB = timeline.tracks[tb].clips[cb];
          if (
            !trackCompatible(timeline.tracks[tb], clipA.mediaType) ||
            !trackCompatible(timeline.tracks[ta], clipB.mediaType)
          ) {
            return result(false, "Swap Clips", []);
          }
          const aStart = clipA.startFrame;
          const bStart = clipB.startFrame;
          const aDestinationEnd = checkedClipEnd(clipA, bStart);
          const bDestinationEnd = checkedClipEnd(clipB, aStart);
          if (aDestinationEnd === null || bDestinationEnd === null) {
            return result(false, "Swap Clips", []);
          }
          // Both clips vacate, so they never block each other; only OTHER clips
          // on each destination track can refuse the swap (keeps it lossless).
          const free = (track: Track, start: number, end: number, exclude: string[]) =>
            !track.clips.some(
              (c) =>
                !exclude.includes(c.id) &&
                c.startFrame < end &&
                checkedClipEnd(c)! > start,
            );
          const exclude = [cmd.clipA, cmd.clipB];
          if (
            !free(timeline.tracks[tb], bStart, aDestinationEnd, exclude) ||
            !free(timeline.tracks[ta], aStart, bDestinationEnd, exclude) ||
            (ta === tb && aStart < aDestinationEnd && bStart < bDestinationEnd)
          ) {
            return result(false, "Swap Clips", []);
          }
          timeline.tracks[ta].clips = timeline.tracks[ta].clips.filter((c) => c.id !== cmd.clipA);
          timeline.tracks[tb].clips = timeline.tracks[tb].clips.filter((c) => c.id !== cmd.clipB);
          clipA.startFrame = bStart;
          clipB.startFrame = aStart;
          timeline.tracks[tb].clips.push(clipA);
          timeline.tracks[ta].clips.push(clipB);
          timeline.tracks[ta].clips.sort((a, b) => a.startFrame - b.startFrame);
          if (ta !== tb) timeline.tracks[tb].clips.sort((a, b) => a.startFrame - b.startFrame);
          return result(true, "Swap Clips", [cmd.clipA, cmd.clipB]);
        }
        case "moveClips": {
          if (cmd.moves.length === 0 || new Set(cmd.moves.map((move) => move.clipId)).size !== cmd.moves.length) {
            return result(false, "Move Clip", []);
          }
          const plans: Array<{
            clip: Clip;
            sourceTrackIndex: number;
            targetTrackIndex: number;
            startFrame: number;
            endFrame: number;
          }> = [];
          for (const m of cmd.moves) {
            const loc = findClip(m.clipId);
            if (!loc) return result(false, "Move Clip", []);
            const [ti, ci] = loc;
            const clip = structuredClone(timeline.tracks[ti].clips[ci]);
            const target = timeline.tracks[m.toTrack];
            const endFrame = checkedClipEnd(clip, m.toFrame);
            if (
              !Number.isInteger(m.toTrack) ||
              !target ||
              !trackCompatible(target, clip.mediaType) ||
              endFrame === null
            ) {
              return result(false, "Move Clip", []);
            }
            plans.push({
              clip,
              sourceTrackIndex: ti,
              targetTrackIndex: m.toTrack,
              startFrame: m.toFrame,
              endFrame,
            });
          }
          const changed = plans.some(
            (plan) =>
              plan.sourceTrackIndex !== plan.targetTrackIndex ||
              plan.clip.startFrame !== plan.startFrame,
          );
          if (!changed) return result(false, "Move Clip", []);

          const beforeTimeline = structuredClone(timeline);
          const beforeIdSeq = idSeq;
          const refuse = (): EditResult => {
            Object.assign(timeline, beforeTimeline);
            idSeq = beforeIdSeq;
            return result(false, "Move Clip", []);
          };
          const selected = new Set(cmd.moves.map((move) => move.clipId));
          for (const track of timeline.tracks) {
            track.clips = track.clips.filter((clip) => !selected.has(clip.id));
          }
          for (const plan of plans) {
            if (!clearRegionSpan(plan.targetTrackIndex, plan.startFrame, plan.endFrame)) {
              return refuse();
            }
          }
          for (const plan of plans) {
            plan.clip.startFrame = plan.startFrame;
            timeline.tracks[plan.targetTrackIndex].clips.push(plan.clip);
          }
          for (const track of timeline.tracks) {
            track.clips.sort((left, right) => left.startFrame - right.startFrame);
          }
          if (!timelineFrameArithmeticIsSafe(timeline)) return refuse();
          return result(true, "Move Clip", cmd.moves.map((m) => m.clipId));
        }
        case "duplicateClips": {
          if (
            cmd.clipIds.length === 0 ||
            cmd.targetTrackIndexes.length !== cmd.clipIds.length ||
            new Set(cmd.clipIds).size !== cmd.clipIds.length ||
            !isI32(cmd.offsetFrames)
          ) {
            return result(false, "Duplicate Clips", []);
          }
          const plans: Array<{
            copy: Clip;
            targetTrackIndex: number;
            startFrame: number;
            endFrame: number;
          }> = [];
          for (let i = 0; i < cmd.clipIds.length; i++) {
            const loc = findClip(cmd.clipIds[i]);
            const targetTrackIndex = cmd.targetTrackIndexes[i];
            const target = timeline.tracks[targetTrackIndex];
            if (!loc || !target) return result(false, "Duplicate Clips", []);
            const source = timeline.tracks[loc[0]].clips[loc[1]];
            if (!trackCompatible(target, source.mediaType)) {
              return result(false, "Duplicate Clips", []);
            }
            const shifted = source.startFrame + cmd.offsetFrames;
            if (!isI32(shifted)) return result(false, "Duplicate Clips", []);
            const startFrame = Math.max(0, shifted);
            const endFrame = checkedClipEnd(source, startFrame);
            if (endFrame === null) return result(false, "Duplicate Clips", []);
            plans.push({
              copy: structuredClone(source),
              targetTrackIndex,
              startFrame,
              endFrame,
            });
          }
          const beforeTimeline = structuredClone(timeline);
          const beforeIdSeq = idSeq;
          const refuse = (): EditResult => {
            Object.assign(timeline, beforeTimeline);
            idSeq = beforeIdSeq;
            return result(false, "Duplicate Clips", []);
          };
          const affected: string[] = [];
          for (const plan of plans) {
            if (!clearRegionSpan(plan.targetTrackIndex, plan.startFrame, plan.endFrame)) {
              return refuse();
            }
          }
          const linkGroupCounts = new Map<string, number>();
          for (const plan of plans) {
            if (!plan.copy.linkGroupId) continue;
            linkGroupCounts.set(plan.copy.linkGroupId, (linkGroupCounts.get(plan.copy.linkGroupId) ?? 0) + 1);
          }
          const linkGroupRemap = new Map<string, string | undefined>();
          for (const [groupId, count] of linkGroupCounts) {
            linkGroupRemap.set(groupId, count > 1 ? nextId() : undefined);
          }
          const newIds = plans.map(() => nextId());
          const idMap = new Map(
            plans.map((plan, index) => [plan.copy.id, newIds[index]] as const),
          );
          for (let index = 0; index < plans.length; index++) {
            const plan = plans[index];
            const target = timeline.tracks[plan.targetTrackIndex];
            if (!target) continue;
            const copy = plan.copy;
            const oldId = copy.id;
            copy.id = newIds[index];
            copy.startFrame = plan.startFrame;
            copy.linkGroupId = copy.linkGroupId ? linkGroupRemap.get(copy.linkGroupId) : undefined;
            if (copy.transitionOut) {
              const toClipId = idMap.get(copy.transitionOut.toClipId);
              const fromMatches =
                copy.transitionOut.fromClipId === "" || copy.transitionOut.fromClipId === oldId;
              copy.transitionOut = toClipId && fromMatches
                ? { ...copy.transitionOut, fromClipId: copy.id, toClipId }
                : undefined;
            }
            target.clips.push(copy);
            target.clips.sort((a, b) => a.startFrame - b.startFrame);
            affected.push(copy.id);
          }
          if (affected.length !== plans.length || !timelineFrameArithmeticIsSafe(timeline)) {
            return refuse();
          }
          return result(
            affected.length > 0,
            affected.length === 1 ? "Duplicate Clip" : "Duplicate Clips",
            affected,
          );
        }
        case "moveOrDuplicateClipsToNewTrack": {
          const beforeTimeline = structuredClone(timeline);
          const beforeIdSeq = idSeq;
          const actionName = cmd.mode === "duplicate"
            ? "Duplicate Clips To New Track"
            : "Move Clips To New Track";
          const refuseAfterPlanning = (): EditResult => {
            Object.assign(timeline, beforeTimeline);
            idSeq = beforeIdSeq;
            return result(false, actionName, []);
          };
          if (
            cmd.clipIds.length === 0 ||
            new Set(cmd.clipIds).size !== cmd.clipIds.length ||
            !cmd.clipIds.includes(cmd.leadClipId) ||
            !isI32(cmd.requestedFrameDelta)
          ) {
            return result(false, actionName, []);
          }

          const sources: Array<{
            clip: Clip;
            trackId: string;
            trackType: Clip["mediaType"];
            sourceEnd: number;
            destinationStart?: number;
            destinationEnd?: number;
          }> = [];
          for (const clipId of cmd.clipIds) {
            const location = findClip(clipId);
            if (!location) return result(false, actionName, []);
            const track = timeline.tracks[location[0]];
            const clip = structuredClone(track.clips[location[1]]);
            const sourceEnd = checkedClipEnd(clip);
            if (sourceEnd === null) return result(false, actionName, []);
            sources.push({
              clip,
              trackId: track.id,
              trackType: track.type,
              sourceEnd,
            });
          }
          const lead = sources.find((source) => source.clip.id === cmd.leadClipId);
          if (!lead) return result(false, actionName, []);

          const minStart = Math.min(...sources.map((source) => source.clip.startFrame));
          const frameDelta = Math.max(cmd.requestedFrameDelta, -minStart);
          if (!isI32(frameDelta)) return result(false, actionName, []);
          for (const source of sources) {
            const destinationStart = source.clip.startFrame + frameDelta;
            if (!isI32(destinationStart) || destinationStart < 0) {
              return result(false, actionName, []);
            }
            const destinationEnd = checkedClipEnd(source.clip, destinationStart);
            if (destinationEnd === null) return result(false, actionName, []);
            source.destinationStart = destinationStart;
            source.destinationEnd = destinationEnd;
          }
          const newTrackType: Clip["mediaType"] = lead.clip.mediaType === "audio" ? "audio" : "video";
          const insertedIndex = insertionIndex(newTrackType, cmd.insertAt);
          const insertedTrackId = nextTrackId();
          timeline.tracks.splice(insertedIndex, 0, {
            id: insertedTrackId,
            type: newTrackType,
            muted: false,
            hidden: false,
            syncLocked: true,
            clips: [],
          });

          // A duplicate must leave every source clip untouched. Linked A/V
          // companions cannot share the lead's new lane, and overwrite
          // placement on a retained source lane would trim/delete an
          // overlapping original. Pin each affected source lane to one fresh,
          // compatible destination before any regions are cleared.
          const duplicateTrackIds = new Map<string, string>();
          if (cmd.mode === "duplicate") {
            const needsFreshTrack = new Set<string>();
            for (const source of sources) {
              const linkedToLead =
                source.clip.id !== lead.clip.id &&
                lead.clip.linkGroupId !== undefined &&
                source.clip.linkGroupId === lead.clip.linkGroupId;
              const incompatibleCompanion = !trackTypeCompatible(
                lead.trackType,
                source.clip.mediaType,
              );
              const pinned = linkedToLead || incompatibleCompanion;
              if (!pinned && source.trackId === lead.trackId) continue;

              const duplicateStart = source.destinationStart!;
              const duplicateEnd = source.destinationEnd!;
              const overlapsPreservedSource = sources.some(
                (preserved) =>
                  preserved.trackId === source.trackId &&
                  preserved.clip.startFrame < duplicateEnd &&
                  preserved.sourceEnd > duplicateStart,
              );
              if (pinned || overlapsPreservedSource) needsFreshTrack.add(source.trackId);
            }

            for (const source of sources) {
              if (
                !needsFreshTrack.has(source.trackId) ||
                duplicateTrackIds.has(source.trackId)
              ) {
                continue;
              }
              const sourceTrack = timeline.tracks.find((track) => track.id === source.trackId);
              if (!sourceTrack) return refuseAfterPlanning();
              const freshTrackType: Clip["mediaType"] = sourceTrack.type === "audio"
                ? "audio"
                : "video";
              const freshTrackIndex = insertionIndex(freshTrackType, timeline.tracks.length);
              const freshTrackId = nextTrackId();
              timeline.tracks.splice(freshTrackIndex, 0, {
                id: freshTrackId,
                type: freshTrackType,
                muted: false,
                hidden: false,
                syncLocked: true,
                clips: [],
              });
              duplicateTrackIds.set(source.trackId, freshTrackId);
            }
          }

          const plans = sources.map((source) => {
            const linkedToLead =
              source.clip.id !== lead.clip.id &&
              lead.clip.linkGroupId !== undefined &&
              source.clip.linkGroupId === lead.clip.linkGroupId;
            const incompatibleCompanion = !trackTypeCompatible(
              lead.trackType,
              source.clip.mediaType,
            );
            const pinned = linkedToLead || incompatibleCompanion;
            const targetTrackId = duplicateTrackIds.get(source.trackId) ?? (
              !pinned && source.trackId === lead.trackId
                ? insertedTrackId
                : source.trackId
            );
            return {
              ...source,
              targetTrackId,
              startFrame: source.destinationStart!,
              endFrame: source.destinationEnd!,
            };
          });

          if (cmd.mode === "move") {
            const selected = new Set(cmd.clipIds);
            for (const track of timeline.tracks) {
              track.clips = track.clips.filter((clip) => !selected.has(clip.id));
            }
          }
          for (const plan of plans) {
            const targetIndex = timeline.tracks.findIndex((track) => track.id === plan.targetTrackId);
            if (targetIndex < 0 || !trackCompatible(timeline.tracks[targetIndex], plan.clip.mediaType)) {
              // Defensive invariant guard: restore in place so this also works
              // while `timeline` is a nested sequence owned by the root object.
              return refuseAfterPlanning();
            }
            if (!clearRegionSpan(targetIndex, plan.startFrame, plan.endFrame)) {
              return refuseAfterPlanning();
            }
          }

          const affected: string[] = [];
          const duplicateLinkCounts = new Map<string, number>();
          if (cmd.mode === "duplicate") {
            for (const plan of plans) {
              const group = plan.clip.linkGroupId;
              if (group) duplicateLinkCounts.set(group, (duplicateLinkCounts.get(group) ?? 0) + 1);
            }
          }
          const duplicateLinkMap = new Map<string, string | undefined>();
          for (const [group, count] of duplicateLinkCounts) {
            duplicateLinkMap.set(group, count > 1 ? nextId() : undefined);
          }

          const duplicateIds = cmd.mode === "duplicate" ? plans.map(() => nextId()) : [];
          const duplicateIdMap = new Map(
            plans.map((plan, index) => [plan.clip.id, duplicateIds[index]] as const),
          );

          for (let index = 0; index < plans.length; index++) {
            const plan = plans[index];
            const target = timeline.tracks.find((track) => track.id === plan.targetTrackId);
            if (!target) continue;
            const clip = structuredClone(plan.clip);
            clip.startFrame = plan.startFrame;
            if (cmd.mode === "duplicate") {
              const oldId = clip.id;
              clip.id = duplicateIds[index];
              clip.linkGroupId = clip.linkGroupId
                ? duplicateLinkMap.get(clip.linkGroupId)
                : undefined;
              if (clip.transitionOut) {
                const toClipId = duplicateIdMap.get(clip.transitionOut.toClipId);
                const fromMatches =
                  clip.transitionOut.fromClipId === "" || clip.transitionOut.fromClipId === oldId;
                clip.transitionOut = toClipId && fromMatches
                  ? { ...clip.transitionOut, fromClipId: clip.id, toClipId }
                  : undefined;
              }
            }
            target.clips.push(clip);
            target.clips.sort((left, right) => left.startFrame - right.startFrame);
            affected.push(clip.id);
          }
          if (
            affected.length !== plans.length ||
            !timelineFrameArithmeticIsSafe(timeline)
          ) return refuseAfterPlanning();
          timeline.tracks = timeline.tracks.filter((track) => track.clips.length > 0);
          return result(
            true,
            actionName,
            affected,
          );
        }
        case "pasteClips": {
          if (cmd.entries.length === 0) return result(false, "Paste Clips", []);
          const beforeTimeline = structuredClone(timeline);
          const beforeIdSeq = idSeq;
          const refuse = (): EditResult => {
            Object.assign(timeline, beforeTimeline);
            idSeq = beforeIdSeq;
            return result(false, "Paste Clips", []);
          };
          const sourceIds = new Set<string>();
          const destinationEnds: number[] = [];
          for (const entry of cmd.entries) {
            const clip = entry.clip;
            const target = timeline.tracks.find((track) => track.id === entry.targetTrackId);
            const sourceEnd = checkedClipEnd(clip);
            const destinationEnd = checkedClipEnd(clip, entry.startFrame);
            if (
              clip.id.trim() === "" ||
              sourceIds.has(clip.id) ||
              sourceEnd === null ||
              destinationEnd === null ||
              !target ||
              !trackCompatible(target, clip.mediaType) ||
              (clip.mediaType === "text" && clip.sourceClipType !== "text") ||
              (clip.nestedSequenceId !== undefined && clip.mediaRef !== "") ||
              (clip.mediaType !== "text" && clip.nestedSequenceId === undefined && clip.mediaRef === "")
            ) {
              return result(false, "Paste Clips", []);
            }
            sourceIds.add(clip.id);
            destinationEnds.push(destinationEnd);
          }

          const newIds = cmd.entries.map(() => nextId());
          const idMap = new Map(
            cmd.entries.map((entry, index) => [entry.clip.id, newIds[index]] as const),
          );
          const linkMap = new Map<string, string>();
          const captionMap = new Map<string, string>();
          for (const entry of cmd.entries) {
            const link = entry.clip.linkGroupId;
            if (link && !linkMap.has(link)) linkMap.set(link, nextId());
            const caption = entry.clip.captionGroupId;
            if (caption && !captionMap.has(caption)) captionMap.set(caption, nextId());
          }

          // Clear the entire batch before inserting any copy, so later entries
          // cannot overwrite an earlier entry from this same paste gesture.
          for (let index = 0; index < cmd.entries.length; index++) {
            const entry = cmd.entries[index];
            const targetIndex = timeline.tracks.findIndex((track) => track.id === entry.targetTrackId);
            if (!clearRegionSpan(targetIndex, entry.startFrame, destinationEnds[index])) {
              return refuse();
            }
          }
          for (let index = 0; index < cmd.entries.length; index++) {
            const entry = cmd.entries[index];
            const clip = structuredClone(entry.clip);
            const oldId = clip.id;
            clip.id = newIds[index];
            clip.startFrame = entry.startFrame;
            clip.linkGroupId = clip.linkGroupId ? linkMap.get(clip.linkGroupId) : undefined;
            clip.captionGroupId = clip.captionGroupId
              ? captionMap.get(clip.captionGroupId)
              : undefined;
            if (clip.transitionOut) {
              const toClipId = idMap.get(clip.transitionOut.toClipId);
              const fromMatches =
                clip.transitionOut.fromClipId === "" || clip.transitionOut.fromClipId === oldId;
              clip.transitionOut = toClipId && fromMatches
                ? { ...clip.transitionOut, fromClipId: clip.id, toClipId }
                : undefined;
            }
            const target = timeline.tracks.find((track) => track.id === entry.targetTrackId);
            target?.clips.push(clip);
          }
          for (const track of timeline.tracks) {
            track.clips.sort((left, right) => left.startFrame - right.startFrame);
          }
          if (!timelineFrameArithmeticIsSafe(timeline)) return refuse();
          timeline.tracks = timeline.tracks.filter((track) => track.clips.length > 0);
          return result(true, "Paste Clips", newIds);
        }
        case "splitClip": {
          return splitClipsAtomically([cmd.clipId], cmd.atFrame, "Split Clip");
        }
        case "splitClips": {
          return splitClipsAtomically(cmd.clipIds, cmd.atFrame, "Split Clips");
        }
        case "setClipProperties": {
          if (cmd.clipIds.length === 0 || new Set(cmd.clipIds).size !== cmd.clipIds.length) {
            return result(false, "Set Clip Property", []);
          }
          const locations = findAllClips(cmd.clipIds);
          if (!locations) return result(false, "Set Clip Property", []);
          const updates: Array<{ location: [number, number]; clip: Clip }> = [];
          for (const location of locations) {
            const c = timeline.tracks[location[0]].clips[location[1]];
            const p = cmd.properties;
            const speed = p.speed ?? c.speed;
            let durationFrames = p.durationFrames ?? c.durationFrames;
            if (p.speed !== undefined && p.durationFrames === undefined) {
              const projectedDuration = Math.round((c.durationFrames * c.speed) / speed);
              if (!isI32(projectedDuration) || projectedDuration < 1) {
                return result(false, "Set Clip Property", []);
              }
              durationFrames = projectedDuration;
            }
            const trimStartFrame = p.trimStartFrame ?? c.trimStartFrame;
            const trimEndFrame = p.trimEndFrame ?? c.trimEndFrame;
            if (
              checkedFrameEnd(
                c.startFrame,
                durationFrames,
                trimStartFrame,
                trimEndFrame,
                speed,
                c.mediaType === "image" || c.mediaType === "text",
              ) === null
            ) {
              return result(false, "Set Clip Property", []);
            }

            const next = structuredClone(c);
            if (
              p.durationFrames !== undefined ||
              p.trimStartFrame !== undefined ||
              p.trimEndFrame !== undefined ||
              p.speed !== undefined ||
              p.reversed !== undefined
            ) {
              delete next.loudnessNormalization;
            }
            next.durationFrames = durationFrames;
            next.trimStartFrame = trimStartFrame;
            next.trimEndFrame = trimEndFrame;
            next.speed = speed;
            if (p.opacity !== undefined) next.opacity = p.opacity;
            if (p.volume !== undefined) next.volume = p.volume;
            if (p.reversed !== undefined) next.reversed = p.reversed;
            if (p.transform !== undefined) next.transform = structuredClone(p.transform);
            if (p.crop !== undefined) {
              next.crop = structuredClone(p.crop);
              next.cropTrack = undefined;
            }
            if (p.textContent !== undefined) next.textContent = p.textContent;
            if (p.textStyle !== undefined) next.textStyle = structuredClone(p.textStyle);
            if (p.flipHorizontal !== undefined) next.transform.flipHorizontal = p.flipHorizontal;
            if (p.flipVertical !== undefined) next.transform.flipVertical = p.flipVertical;
            next.fadeInFrames = Math.min(
              durationFrames,
              Math.max(0, p.fadeInFrames ?? next.fadeInFrames),
            );
            next.fadeOutFrames = Math.min(
              durationFrames,
              Math.max(0, p.fadeOutFrames ?? next.fadeOutFrames),
            );
            if (p.fadeInInterpolation !== undefined) {
              next.fadeInInterpolation = p.fadeInInterpolation;
            }
            if (p.fadeOutInterpolation !== undefined) {
              next.fadeOutInterpolation = p.fadeOutInterpolation;
            }
            if (checkedClipEnd(next) === null) return result(false, "Set Clip Property", []);
            updates.push({ location, clip: next });
          }
          const changed = updates.some(({ location, clip }) =>
            JSON.stringify(timeline.tracks[location[0]].clips[location[1]]) !== JSON.stringify(clip)
          );
          if (!changed) return result(false, "Set Clip Property", []);
          for (const { location, clip } of updates) {
            timeline.tracks[location[0]].clips[location[1]] = clip;
          }
          return result(changed, "Set Clip Property", cmd.clipIds);
        }
        case "setTransformAtFrame": {
          const transform: Transform = cmd.transform;
          if (
            !isI32(cmd.frame) ||
            ![
              transform.centerX,
              transform.centerY,
              transform.width,
              transform.height,
              transform.rotation,
            ].every(Number.isFinite)
          ) {
            return result(false, "Change Transform", []);
          }
          const targetLeft = transform.centerX - transform.width / 2;
          const targetTop = transform.centerY - transform.height / 2;
          if (!Number.isFinite(targetLeft) || !Number.isFinite(targetTop)) {
            return result(false, "Change Transform", []);
          }
          const location = findClip(cmd.clipId);
          if (!location) return result(false, "Change Transform", []);
          const current = timeline.tracks[location[0]].clips[location[1]];
          const positionActive = (current.positionTrack?.keyframes.length ?? 0) > 0;
          const scaleActive = (current.scaleTrack?.keyframes.length ?? 0) > 0;
          const rotationActive = (current.rotationTrack?.keyframes.length ?? 0) > 0;
          const hasActiveTrack = positionActive || scaleActive || rotationActive;
          const clipEnd = checkedClipEnd(current);
          if (
            clipEnd === null ||
            (hasActiveTrack &&
              (cmd.frame < current.startFrame || cmd.frame >= clipEnd))
          ) {
            return result(false, "Change Transform", []);
          }
          const relativeFrame = hasActiveTrack ? cmd.frame - current.startFrame : 0;
          if (!isI32(relativeFrame)) return result(false, "Change Transform", []);

          const next = structuredClone(current);
          if (positionActive) {
            upsertSmoothKeyframe(next.positionTrack!, relativeFrame, {
              a: targetLeft,
              b: targetTop,
            });
          } else {
            next.transform.centerX = transform.centerX;
            next.transform.centerY = transform.centerY;
          }
          if (scaleActive) {
            upsertSmoothKeyframe(next.scaleTrack!, relativeFrame, {
              a: transform.width,
              b: transform.height,
            });
          } else {
            next.transform.width = transform.width;
            next.transform.height = transform.height;
          }
          if (rotationActive) {
            upsertSmoothKeyframe(next.rotationTrack!, relativeFrame, transform.rotation);
          } else {
            next.transform.rotation = transform.rotation;
          }
          next.transform.flipHorizontal = transform.flipHorizontal;
          next.transform.flipVertical = transform.flipVertical;

          const changed = JSON.stringify(current) !== JSON.stringify(next);
          if (changed) timeline.tracks[location[0]].clips[location[1]] = next;
          return result(changed, "Change Transform", [cmd.clipId]);
        }
        case "setColorGrade": {
          const locations = findAllClips(cmd.clipIds);
          if (!locations) return result(false, "Set Color Grade", []);
          const next = normalizeColorGrade(cmd.grade);
          if (next && !isValidColorGrade(next)) {
            return result(false, "Set Color Grade", []);
          }
          let changed = false;
          for (const loc of locations) {
            const clip = timeline.tracks[loc[0]].clips[loc[1]];
            if (JSON.stringify(clip.colorGrade) !== JSON.stringify(next)) {
              clip.colorGrade = next;
              changed = true;
            }
          }
          return result(changed, "Set Color Grade", cmd.clipIds);
        }
        case "setLut": {
          const locations = findAllClips(cmd.clipIds);
          if (!locations) return result(false, "Set LUT", []);
          const next = cmd.lut ?? undefined;
          const nameBytes = next ? new TextEncoder().encode(next.name).length : 0;
          if (
            next &&
            (!/^[0-9a-f]{64}$/.test(next.id) ||
              nameBytes < 1 ||
              nameBytes > 128 ||
              /\p{Cc}/u.test(next.name) ||
              !Number.isFinite(next.intensity) ||
              next.intensity < 0 ||
              next.intensity > 1)
          ) {
            return result(false, "Set LUT", []);
          }
          let changed = false;
          for (const loc of locations) {
            const clip = timeline.tracks[loc[0]].clips[loc[1]];
            if (JSON.stringify(clip.lut) !== JSON.stringify(next)) {
              clip.lut = next ? { ...next } : undefined;
              changed = true;
            }
          }
          return result(changed, "Set LUT", cmd.clipIds);
        }
        case "setChromaKey": {
          const locations = findAllClips(cmd.clipIds);
          if (!locations) return result(false, "Set Chroma Key", []);
          const next = normalizeChromaKey(cmd.chromaKey);
          let changed = false;
          for (const loc of locations) {
            const clip = timeline.tracks[loc[0]].clips[loc[1]];
            if (JSON.stringify(clip.chromaKey) !== JSON.stringify(next)) {
              clip.chromaKey = next;
              changed = true;
            }
          }
          return result(changed, "Set Chroma Key", cmd.clipIds);
        }
        case "setMasks": {
          const locations = findAllClips(cmd.clipIds);
          if (!locations) return result(false, "Set Masks", []);
          const next = cmd.masks.map(normalizeMask);
          let changed = false;
          for (const loc of locations) {
            const clip = timeline.tracks[loc[0]].clips[loc[1]];
            if (JSON.stringify(clip.masks ?? []) !== JSON.stringify(next)) {
              clip.masks = structuredClone(next);
              changed = true;
            }
          }
          return result(changed, "Set Masks", cmd.clipIds);
        }
        case "setEffects": {
          const locations = findAllClips(cmd.clipIds);
          if (!locations) return result(false, "Set Effects", []);
          const next = cmd.effects.map(normalizeEffect);
          let changed = false;
          for (const loc of locations) {
            const clip = timeline.tracks[loc[0]].clips[loc[1]];
            if (JSON.stringify(clip.effects ?? []) !== JSON.stringify(next)) {
              clip.effects = structuredClone(next);
              changed = true;
            }
          }
          return result(changed, "Set Effects", cmd.clipIds);
        }
        case "setLoudnessNormalization": {
          const loc = findClip(cmd.clipId);
          if (!loc) return result(false, "Normalize Loudness", []);
          const clip = timeline.tracks[loc[0]].clips[loc[1]];
          const next = cmd.normalization ?? undefined;
          if (JSON.stringify(clip.loudnessNormalization) === JSON.stringify(next)) {
            return result(false, next ? "Normalize Loudness" : "Reset Loudness", []);
          }
          clip.loudnessNormalization = next;
          return result(true, next ? "Normalize Loudness" : "Reset Loudness", [cmd.clipId]);
        }
        case "setAudioDenoise": {
          const loc = findClip(cmd.clipId);
          if (!loc) return result(false, "Apply Audio Denoise", []);
          const clip = timeline.tracks[loc[0]].clips[loc[1]];
          const next = cmd.denoise ?? undefined;
          if (JSON.stringify(clip.audioDenoise) === JSON.stringify(next)) {
            return result(false, next ? "Apply Audio Denoise" : "Reset Audio Denoise", []);
          }
          clip.audioDenoise = next ? structuredClone(next) : undefined;
          return result(
            true,
            next ? "Apply Audio Denoise" : "Reset Audio Denoise",
            [cmd.clipId],
          );
        }
        case "applyStabilization": {
          const loc = findClip(cmd.clipId);
          if (!loc) return result(false, "Apply Stabilization", []);
          const clip = timeline.tracks[loc[0]].clips[loc[1]];
          if (clip.mediaType !== "video" || cmd.solution.sourceIdentity !== clip.mediaRef) {
            return result(false, "Apply Stabilization", []);
          }
          clip.stabilization = structuredClone(cmd.solution);
          return result(true, "Apply Stabilization", [cmd.clipId]);
        }
        case "adjustStabilization": {
          const loc = findClip(cmd.clipId);
          if (!loc) return result(false, "Adjust Stabilization", []);
          const clip = timeline.tracks[loc[0]].clips[loc[1]];
          if (!clip.stabilization) return result(false, "Adjust Stabilization", []);
          if (cmd.strength !== undefined) clip.stabilization.strength = cmd.strength;
          if (cmd.cropMargin !== undefined) clip.stabilization.cropMargin = cmd.cropMargin;
          return result(true, "Adjust Stabilization", [cmd.clipId]);
        }
        case "resetStabilization": {
          const loc = findClip(cmd.clipId);
          if (!loc) return result(false, "Reset Stabilization", []);
          const clip = timeline.tracks[loc[0]].clips[loc[1]];
          if (!clip.stabilization) return result(false, "Reset Stabilization", []);
          delete clip.stabilization;
          return result(true, "Reset Stabilization", [cmd.clipId]);
        }
        case "setTransition": {
          const fromLocation = findClip(cmd.fromClipId);
          if (!fromLocation) return result(false, "Set Transition", []);
          const from = timeline.tracks[fromLocation[0]].clips[fromLocation[1]];
          if (cmd.kind == null) {
            if (
              from.transitionOut?.toClipId !== cmd.toClipId ||
              (from.transitionOut.fromClipId !== undefined &&
                from.transitionOut.fromClipId !== cmd.fromClipId)
            ) {
              return result(false, "Remove Transition", []);
            }
            delete from.transitionOut;
            return result(true, "Remove Transition", [cmd.fromClipId, cmd.toClipId]);
          }
          const toLocation = findClip(cmd.toClipId);
          if (!toLocation || toLocation[0] !== fromLocation[0]) {
            return result(false, "Set Transition", []);
          }
          const track = timeline.tracks[fromLocation[0]];
          const to = timeline.tracks[toLocation[0]].clips[toLocation[1]];
          const ordered = track.clips
            .slice()
            .sort((left, right) => left.startFrame - right.startFrame || left.id.localeCompare(right.id));
          const fromIndex = ordered.findIndex((clip) => clip.id === from.id);
          if (
            cmd.durationFrames < 1 ||
            track.type === "audio" ||
            from.mediaType === "audio" ||
            from.mediaType === "text" ||
            to.mediaType === "audio" ||
            to.mediaType === "text" ||
            from.startFrame + from.durationFrames !== to.startFrame ||
            ordered[fromIndex + 1]?.id !== to.id
          ) {
            return result(false, "Set Transition", []);
          }
          const maximum = Math.max(1, Math.floor(Math.min(from.durationFrames, to.durationFrames) / 2));
          if (cmd.durationFrames > maximum) {
            return result(false, "Set Transition", []);
          }
          const next = {
            fromClipId: from.id,
            toClipId: to.id,
            kind: cmd.kind,
            durationFrames: cmd.durationFrames,
          };
          if (JSON.stringify(from.transitionOut) === JSON.stringify(next)) {
            return result(false, "Set Transition", []);
          }
          from.transitionOut = next;
          return result(true, "Set Transition", [cmd.fromClipId, cmd.toClipId]);
        }
        case "swapMedia": {
          return result(false, "Swap Media", []);
        }
        case "setTimelineSettings": {
          if (
            !isI32(cmd.fps) ||
            !isI32(cmd.width) ||
            !isI32(cmd.height) ||
            cmd.fps <= 0 ||
            cmd.width <= 0 ||
            cmd.height <= 0 ||
            !settingsFrameProjectionIsSafe(timeline, cmd.fps)
          ) {
            return result(false, "Change Project Settings", []);
          }
          const same =
            timeline.fps === cmd.fps &&
            timeline.width === cmd.width &&
            timeline.height === cmd.height &&
            timeline.settingsConfigured;
          if (same) return result(false, "Change Project Settings", []);
          const before = structuredClone(timeline);
          const applySettings = (target: Timeline): void => {
            for (const sequence of target.nestedSequences ?? []) {
              applySettings(sequence.timeline);
            }
            if (target.fps > 0 && target.fps !== cmd.fps) {
              const scale = cmd.fps / target.fps;
              for (const track of target.tracks) {
                const ordered = [...track.clips].sort((a, b) => a.startFrame - b.startFrame);
                let previousEnd: number | undefined;
                for (const clip of ordered) {
                  const sourceEnd = checkedClipEnd(clip)!;
                  const scaledStart = Math.round(clip.startFrame * scale);
                  const scaledEnd = Math.round(sourceEnd * scale);
                  clip.startFrame = Math.max(scaledStart, previousEnd ?? scaledStart);
                  clip.durationFrames = Math.max(1, scaledEnd - clip.startFrame);
                  clip.trimStartFrame = Math.round(clip.trimStartFrame * scale);
                  clip.trimEndFrame = Math.round(clip.trimEndFrame * scale);
                  previousEnd = checkedClipEnd(clip)!;
                }
              }
            }
            target.fps = cmd.fps;
            target.width = cmd.width;
            target.height = cmd.height;
            target.settingsConfigured = true;
          };
          applySettings(timeline);
          if (!timelineFrameArithmeticIsSafe(timeline)) {
            Object.assign(timeline, before);
            return result(false, "Change Project Settings", []);
          }
          return result(true, "Change Project Settings", []);
        }
        case "renameMedia":
        case "renameFolder":
        case "deleteMedia":
        case "deleteFolder":
          return result(false, cmd.type, []);
        default:
          return result(false, cmd.type, []);
      }
    },
  };
  dispatchFallback = store.editApply;
  return store;
}
