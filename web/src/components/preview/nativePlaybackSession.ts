import { useSyncExternalStore } from "react";
import * as api from "../../lib/api";
import type {
  PlaybackCommandError,
  PlaybackFrameEvent,
  PlaybackIdentity,
  ProjectRevision,
} from "../../lib/types";

let publication: PlaybackFrameEvent | null = null;
const publicationListeners = new Set<() => void>();

function notifyPublication(): void {
  for (const listener of publicationListeners) listener();
}

export function publishNativePlaybackFrame(event: PlaybackFrameEvent): void {
  publication = event;
  notifyPublication();
}

export function clearNativePlaybackPublication(): void {
  if (publication === null) return;
  publication = null;
  notifyPublication();
}

export function getNativePlaybackPublication(): PlaybackFrameEvent | null {
  return publication;
}

export function subscribeNativePlaybackPublication(listener: () => void): () => void {
  publicationListeners.add(listener);
  return () => publicationListeners.delete(listener);
}

export function useNativePlaybackPublication(): PlaybackFrameEvent | null {
  return useSyncExternalStore(
    subscribeNativePlaybackPublication,
    getNativePlaybackPublication,
    getNativePlaybackPublication,
  );
}

export function samePlaybackIdentity(
  left: PlaybackIdentity | null,
  right: PlaybackIdentity | null,
): boolean {
  return (
    left !== null &&
    right !== null &&
    left.projectEpoch === right.projectEpoch &&
    left.timelineVersion === right.timelineVersion &&
    left.sessionId === right.sessionId
  );
}

function sameRevision(identity: PlaybackIdentity | null, revision: ProjectRevision): boolean {
  return (
    identity !== null &&
    identity.projectEpoch === revision.projectEpoch &&
    identity.timelineVersion === revision.timelineVersion
  );
}

function validFrameEvent(event: PlaybackFrameEvent): boolean {
  return (
    Number.isSafeInteger(event.projectEpoch) &&
    event.projectEpoch >= 0 &&
    Number.isSafeInteger(event.timelineVersion) &&
    event.timelineVersion >= 0 &&
    /^[A-Za-z0-9-]{1,128}$/.test(event.sessionId) &&
    Number.isSafeInteger(event.frame) &&
    event.frame >= 0 &&
    Number.isSafeInteger(event.sequence) &&
    event.sequence >= 0 &&
    typeof event.terminal === "boolean"
  );
}

export interface NativePlaybackApi {
  playbackStart(frame: number, identity: PlaybackIdentity): Promise<void>;
  playbackPause(identity: PlaybackIdentity, frame: number): Promise<void>;
  playbackSeek(identity: PlaybackIdentity, frame: number): Promise<void>;
  playbackStop(identity: PlaybackIdentity): Promise<void>;
}

export interface NativePlaybackController {
  start(
    revision: ProjectRevision,
    frame: number,
    options?: {
      forceNewSession?: boolean;
      onIdentity?: (identity: PlaybackIdentity) => void;
    },
  ): Promise<PlaybackIdentity>;
  pause(identity: PlaybackIdentity, frame: number): Promise<void>;
  seek(identity: PlaybackIdentity, frame: number): Promise<void>;
  stop(identity: PlaybackIdentity): Promise<void>;
  cleanup(identity: PlaybackIdentity, action: "pause" | "stop", frame: number): Promise<void>;
  stopCurrent(): Promise<void>;
  currentIdentity(): PlaybackIdentity | null;
  acceptFrame(event: PlaybackFrameEvent): void;
  shouldFallback(error: unknown): boolean;
}

export function createNativePlaybackController(
  playbackApi: NativePlaybackApi,
  mintSessionId: () => string = createSessionId,
): NativePlaybackController {
  let current: PlaybackIdentity | null = null;
  let paused = false;
  let lastSequence = -1;

  const stopIdentity = async (identity: PlaybackIdentity) => {
    if (!samePlaybackIdentity(current, identity)) return;
    // Retire locally before awaiting IPC. A final backend frame can race the
    // stop command across the WebView bridge; keeping `current` alive until the
    // promise resolves lets that stale frame overwrite a newer scrub result.
    current = null;
    paused = false;
    lastSequence = -1;
    clearNativePlaybackPublication();
    try {
      await playbackApi.playbackStop(identity);
    } catch (error) {
      const decoded = api.decodePlaybackCommandError(error);
      if (!decoded || decoded.code === "engine") throw error;
    }
  };

  return {
    async start(revision, frame, options) {
      const reusable = sameRevision(current, revision) && paused && !options?.forceNewSession;
      if (!reusable && current) await stopIdentity(current);
      if (!reusable) {
        current = {
          ...revision,
          sessionId: mintSessionId(),
        };
        lastSequence = -1;
        clearNativePlaybackPublication();
      }
      const identity = current;
      if (!identity) throw new Error("native playback identity was not created");
      options?.onIdentity?.({ ...identity });
      await playbackApi.playbackStart(Math.max(0, Math.floor(frame)), identity);
      paused = false;
      return identity;
    },
    async pause(identity, frame) {
      if (!samePlaybackIdentity(current, identity)) return;
      try {
        await playbackApi.playbackPause(identity, Math.max(0, Math.floor(frame)));
      } catch (error) {
        const decoded = api.decodePlaybackCommandError(error);
        if (!decoded || decoded.code === "engine") throw error;
      }
      if (samePlaybackIdentity(current, identity)) paused = true;
    },
    async seek(identity, frame) {
      if (!samePlaybackIdentity(current, identity)) return;
      try {
        await playbackApi.playbackSeek(identity, Math.max(0, Math.floor(frame)));
      } catch (error) {
        const decoded = api.decodePlaybackCommandError(error);
        if (!decoded || decoded.code === "engine") throw error;
      }
    },
    stop: stopIdentity,
    async cleanup(identity, action, frame) {
      if (!samePlaybackIdentity(current, identity)) return;
      if (action === "pause") {
        try {
          await playbackApi.playbackPause(identity, Math.max(0, Math.floor(frame)));
        } catch (error) {
          const decoded = api.decodePlaybackCommandError(error);
          if (!decoded || decoded.code === "engine") throw error;
        }
        if (samePlaybackIdentity(current, identity)) paused = true;
      } else {
        await stopIdentity(identity);
      }
    },
    async stopCurrent() {
      if (current) await stopIdentity(current);
    },
    currentIdentity() {
      return current ? { ...current } : null;
    },
    acceptFrame(event) {
      if (!validFrameEvent(event) || !samePlaybackIdentity(current, event)) return;
      if (event.sequence <= lastSequence) return;
      lastSequence = event.sequence;
      publishNativePlaybackFrame(event);
    },
    shouldFallback(error) {
      return (
        api.decodePlaybackCommandError(error)?.code === "engine" ||
        api.isPlaybackCommandUnavailable(error)
      );
    },
  };
}

function createSessionId(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  return `session-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

export const nativePlaybackController = createNativePlaybackController({
  playbackStart: (frame, identity) => api.playbackStart(frame, identity),
  playbackPause: (identity, frame) => api.playbackPause(identity, frame),
  playbackSeek: (identity, frame) => api.playbackSeek(identity, frame),
  playbackStop: (identity) => api.playbackStop(identity),
});

export async function stopNativePlaybackForProjectBoundary(): Promise<void> {
  await nativePlaybackController.stopCurrent();
}

export function isPlaybackCommandError(error: unknown): error is PlaybackCommandError {
  return api.decodePlaybackCommandError(error) !== null;
}
