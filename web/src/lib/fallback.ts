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
          }
          return result(changed, "Set Clip Property", cmd.clipIds);
        }
        default:
          return result(false, cmd.type, []);
      }
    },
  };
}
