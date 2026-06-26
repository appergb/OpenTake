/**
 * Browser-only in-memory timeline fallback (used when not running inside Tauri).
 * Mirrors a subset of the Rust command behavior so the UI shell is explorable
 * in a plain browser. NOT an editing engine — the authoritative truth is always
 * the Rust core under Tauri. Kept deliberately small.
 */

import type {
  Clip,
  EditRequest,
  EditResult,
  Timeline,
  TimelineSnapshot,
  Track,
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

function isVisual(type: Clip["mediaType"]): boolean {
  return type !== "audio";
}

type AddClipEntry = Extract<EditRequest, { type: "addClips" }>["entries"][number];

function newClipFromEntry(id: string, entry: AddClipEntry): Clip {
  return {
    id,
    mediaRef: entry.mediaRef,
    mediaType: entry.mediaType,
    sourceClipType: entry.sourceClipType,
    startFrame: Math.max(0, entry.startFrame),
    durationFrames: Math.max(1, entry.durationFrames),
    trimStartFrame: entry.trimStartFrame ?? 0,
    trimEndFrame: entry.trimEndFrame ?? 0,
    speed: 1,
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

export function createFallbackStore() {
  let timeline: Timeline = demoTimeline();
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

  function insertionIndex(kind: Clip["mediaType"], requested = timeline.tracks.length): number {
    const firstAudio = timeline.tracks.findIndex((track) => track.type === "audio");
    const firstAudioIndex = firstAudio >= 0 ? firstAudio : timeline.tracks.length;
    const bounded = Math.max(0, Math.min(timeline.tracks.length, requested));
    if (kind === "audio") return Math.max(bounded, firstAudioIndex);
    return Math.min(bounded, firstAudioIndex);
  }

  function trackCompatible(track: Track, type: Clip["mediaType"]): boolean {
    return type === "audio" ? track.type === "audio" : isVisual(track.type);
  }

  function clearRegion(trackIndex: number, startFrame: number, durationFrames: number): void {
    const endFrame = startFrame + durationFrames;
    const next: Clip[] = [];
    for (const clip of timeline.tracks[trackIndex].clips) {
      const clipEnd = clip.startFrame + clip.durationFrames;
      if (clipEnd <= startFrame || clip.startFrame >= endFrame) {
        next.push(clip);
        continue;
      }
      if (clip.startFrame < startFrame) {
        const left = { ...structuredClone(clip), durationFrames: startFrame - clip.startFrame };
        left.trimEndFrame += Math.round((clipEnd - startFrame) * clip.speed);
        next.push(left);
      }
      if (clipEnd > endFrame) {
        const right = structuredClone(clip);
        right.id = nextId();
        right.startFrame = endFrame;
        right.durationFrames = clipEnd - endFrame;
        right.trimStartFrame += Math.round((endFrame - clip.startFrame) * clip.speed);
        next.push(right);
      }
    }
    timeline.tracks[trackIndex].clips = next.sort((a, b) => a.startFrame - b.startFrame);
  }

  function resolveOrCreateAudioTrack(startFrame: number, durationFrames: number): number {
    const endFrame = startFrame + durationFrames;
    for (let i = 0; i < timeline.tracks.length; i++) {
      const track = timeline.tracks[i];
      if (track.type !== "audio") continue;
      const overlaps = track.clips.some(
        (clip) => clip.startFrame < endFrame && clip.startFrame + clip.durationFrames > startFrame,
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

  return {
    getTimeline: (): TimelineSnapshot => snapshot(),
    reset: () => {
      timeline = { fps: 30, width: 1920, height: 1080, settingsConfigured: false, tracks: [] };
      bump();
    },
    noop: (name: string): EditResult => result(false, name, []),
    editApply: (cmd: EditRequest): EditResult => {
      switch (cmd.type) {
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
          const affected: string[] = [];
          for (const entry of cmd.entries) {
            const track = timeline.tracks[entry.trackIndex];
            if (!track || !trackCompatible(track, entry.mediaType)) continue;
            const id = nextId();
            const clip = newClipFromEntry(id, entry);
            const shouldLink =
              entry.addLinkedAudio === true &&
              entry.hasAudio === true &&
              track.type === "video" &&
              entry.sourceClipType === "video";
            const linkGroupId = shouldLink ? nextId() : undefined;
            clip.linkGroupId = linkGroupId;
            clearRegion(entry.trackIndex, clip.startFrame, clip.durationFrames);
            track.clips.push(clip);
            track.clips.sort((a, b) => a.startFrame - b.startFrame);
            affected.push(id);
            if (shouldLink && linkGroupId) {
              const audioTrackIndex = resolveOrCreateAudioTrack(clip.startFrame, clip.durationFrames);
              const audio: Clip = {
                ...newClipFromEntry(nextId(), { ...entry, mediaType: "audio" }),
                linkGroupId,
              };
              clearRegion(audioTrackIndex, audio.startFrame, audio.durationFrames);
              timeline.tracks[audioTrackIndex].clips.push(audio);
              timeline.tracks[audioTrackIndex].clips.sort((a, b) => a.startFrame - b.startFrame);
              affected.push(audio.id);
            }
          }
          return result(affected.length > 0, affected.length === 1 ? "Add Clip" : "Add Clips", affected);
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
        case "moveClips": {
          let changed = false;
          for (const m of cmd.moves) {
            const loc = findClip(m.clipId);
            if (!loc) continue;
            const [ti, ci] = loc;
            const clip = timeline.tracks[ti].clips[ci];
            if (m.toTrack >= 0 && m.toTrack < timeline.tracks.length) {
              timeline.tracks[ti].clips.splice(ci, 1);
              clip.startFrame = Math.max(0, m.toFrame);
              timeline.tracks[m.toTrack].clips.push(clip);
              timeline.tracks[m.toTrack].clips.sort((a, b) => a.startFrame - b.startFrame);
              changed = true;
            }
          }
          return result(changed, "Move Clip", cmd.moves.map((m) => m.clipId));
        }
        case "duplicateClips": {
          const plans: Array<{ copy: Clip; targetTrackIndex: number; startFrame: number }> = [];
          for (let i = 0; i < cmd.clipIds.length; i++) {
            const loc = findClip(cmd.clipIds[i]);
            const targetTrackIndex = cmd.targetTrackIndexes[i];
            const target = timeline.tracks[targetTrackIndex];
            if (!loc || !target) continue;
            const source = timeline.tracks[loc[0]].clips[loc[1]];
            if (!trackCompatible(target, source.mediaType)) continue;
            plans.push({
              copy: structuredClone(source),
              targetTrackIndex,
              startFrame: Math.max(0, source.startFrame + cmd.offsetFrames),
            });
          }
          const affected: string[] = [];
          for (const plan of plans) {
            clearRegion(plan.targetTrackIndex, plan.startFrame, plan.copy.durationFrames);
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
          for (const plan of plans) {
            const target = timeline.tracks[plan.targetTrackIndex];
            if (!target) continue;
            const copy = plan.copy;
            copy.id = nextId();
            copy.startFrame = plan.startFrame;
            copy.linkGroupId = copy.linkGroupId ? linkGroupRemap.get(copy.linkGroupId) : undefined;
            target.clips.push(copy);
            target.clips.sort((a, b) => a.startFrame - b.startFrame);
            affected.push(copy.id);
          }
          return result(
            affected.length > 0,
            affected.length === 1 ? "Duplicate Clip" : "Duplicate Clips",
            affected,
          );
        }
        case "splitClip": {
          const loc = findClip(cmd.clipId);
          if (!loc) return result(false, "Split Clip", []);
          const [ti, ci] = loc;
          const clip = timeline.tracks[ti].clips[ci];
          if (cmd.atFrame <= clip.startFrame || cmd.atFrame >= clip.startFrame + clip.durationFrames)
            return result(false, "Split Clip", []);
          const rightDur = clip.startFrame + clip.durationFrames - cmd.atFrame;
          clip.durationFrames = cmd.atFrame - clip.startFrame;
          const right = newClip(nextId(), clip.mediaRef, clip.mediaType, cmd.atFrame, rightDur);
          timeline.tracks[ti].clips.splice(ci + 1, 0, right);
          return result(true, "Split Clip", [right.id]);
        }
        case "setClipProperties": {
          let changed = false;
          for (const id of cmd.clipIds) {
            const loc = findClip(id);
            if (!loc) continue;
            const c = timeline.tracks[loc[0]].clips[loc[1]];
            const p = cmd.properties;
            if (p.opacity !== undefined) (c.opacity = p.opacity), (changed = true);
            if (p.volume !== undefined) (c.volume = p.volume), (changed = true);
            if (p.speed !== undefined) (c.speed = p.speed), (changed = true);
            if (p.transform !== undefined) (c.transform = p.transform), (changed = true);
            if (p.fadeInFrames !== undefined) (c.fadeInFrames = p.fadeInFrames), (changed = true);
            if (p.fadeOutFrames !== undefined) (c.fadeOutFrames = p.fadeOutFrames), (changed = true);
            if (p.fadeInInterpolation !== undefined)
              (c.fadeInInterpolation = p.fadeInInterpolation), (changed = true);
            if (p.fadeOutInterpolation !== undefined)
              (c.fadeOutInterpolation = p.fadeOutInterpolation), (changed = true);
          }
          return result(changed, "Set Clip Property", cmd.clipIds);
        }
        default:
          return result(false, cmd.type, []);
      }
    },
  };
}
