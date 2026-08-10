import type { PlaybackFrameEvent } from "../../lib/types";

export interface RustFrameIdentity {
  projectEpoch: number;
  timelineVersion: number;
  sessionId: string;
}

export interface RustFrameSlot {
  src: string | null;
  frame: PlaybackFrameEvent | null;
  visible: boolean;
  requestCount: number;
  retryCount: number;
}

export interface RustFrameBufferState {
  identity: RustFrameIdentity | null;
  slots: [RustFrameSlot, RustFrameSlot];
  activeSlot: 0 | 1 | null;
  pendingSlot: 0 | 1 | null;
}

export type RustFrameBufferEffect = "none" | "terminal-promoted" | "terminal-exhausted";

export interface RustFrameBufferTransition {
  state: RustFrameBufferState;
  effect: RustFrameBufferEffect;
}

function emptySlot(): RustFrameSlot {
  return { src: null, frame: null, visible: false, requestCount: 0, retryCount: 0 };
}

export function createRustFrameBufferState(
  identity: RustFrameIdentity | null = null,
): RustFrameBufferState {
  return {
    identity,
    slots: [emptySlot(), emptySlot()],
    activeSlot: null,
    pendingSlot: null,
  };
}

function validFrame(frame: PlaybackFrameEvent): boolean {
  return (
    Number.isSafeInteger(frame.projectEpoch) &&
    frame.projectEpoch >= 0 &&
    Number.isSafeInteger(frame.timelineVersion) &&
    frame.timelineVersion >= 0 &&
    /^[A-Za-z0-9-]{1,128}$/.test(frame.sessionId) &&
    Number.isSafeInteger(frame.frame) &&
    frame.frame >= 0 &&
    Number.isSafeInteger(frame.sequence) &&
    frame.sequence >= 0 &&
    typeof frame.terminal === "boolean"
  );
}

function sameIdentity(
  left: RustFrameIdentity | null,
  right: RustFrameIdentity,
): boolean {
  return (
    left !== null &&
    left.projectEpoch === right.projectEpoch &&
    left.timelineVersion === right.timelineVersion &&
    left.sessionId === right.sessionId
  );
}

function frameUrl(endpoint: string | null, frame: PlaybackFrameEvent): string | null {
  if (!endpoint || !validFrame(frame)) return null;
  const params = new URLSearchParams({
    projectEpoch: String(frame.projectEpoch),
    timelineVersion: String(frame.timelineVersion),
    sessionId: frame.sessionId,
    frame: String(frame.frame),
    sequence: String(frame.sequence),
  });
  return `${endpoint}?${params.toString()}`;
}

function retryUrl(src: string, retry: number): string {
  const separator = src.includes("?") ? "&" : "?";
  return `${src.replace(/([?&])retry=\d+$/, "")}${separator}retry=${retry}`;
}

function cloneSlots(state: RustFrameBufferState): [RustFrameSlot, RustFrameSlot] {
  return [{ ...state.slots[0] }, { ...state.slots[1] }];
}

export function syncRustFrameBufferIdentity(
  state: RustFrameBufferState,
  identity: RustFrameIdentity,
): RustFrameBufferState {
  return sameIdentity(state.identity, identity)
    ? state
    : createRustFrameBufferState({ ...identity });
}

/** Retain the last promoted frame while invalidating an image request that was
 * started by the running transport. A pause/resume cycle must not allow that
 * pre-pause decoder completion to paint after the new run begins. */
export function cancelPendingRustFrame(state: RustFrameBufferState): RustFrameBufferState {
  if (state.pendingSlot === null) return state;
  const slots = cloneSlots(state);
  slots[state.pendingSlot] = emptySlot();
  return { ...state, slots, pendingSlot: null };
}

export function requestRustFrame(
  current: RustFrameBufferState,
  frame: PlaybackFrameEvent,
  endpoint: string | null,
): RustFrameBufferTransition {
  const src = frameUrl(endpoint, frame);
  if (!src) return { state: current, effect: "none" };
  const identity = {
    projectEpoch: frame.projectEpoch,
    timelineVersion: frame.timelineVersion,
    sessionId: frame.sessionId,
  };
  const state = syncRustFrameBufferIdentity(current, identity);
  const activeFrame = state.activeSlot === null ? null : state.slots[state.activeSlot].frame;
  const pendingFrame = state.pendingSlot === null ? null : state.slots[state.pendingSlot].frame;
  if (
    (activeFrame && activeFrame.sequence >= frame.sequence) ||
    (pendingFrame && pendingFrame.sequence >= frame.sequence)
  ) {
    return { state, effect: "none" };
  }

  const pendingSlot: 0 | 1 = state.activeSlot === 0 ? 1 : 0;
  const slots = cloneSlots(state);
  slots[pendingSlot] = {
    src,
    frame: { ...frame },
    visible: false,
    requestCount: slots[pendingSlot].requestCount + 1,
    retryCount: 0,
  };
  return {
    state: { ...state, slots, pendingSlot },
    effect: "none",
  };
}

export function loadRustFrame(
  state: RustFrameBufferState,
  slot: 0 | 1,
  src: string,
): RustFrameBufferTransition {
  if (state.pendingSlot !== slot || state.slots[slot].src !== src) {
    return { state, effect: "none" };
  }
  const slots = cloneSlots(state);
  slots[0].visible = slot === 0;
  slots[1].visible = slot === 1;
  const terminal = slots[slot].frame?.terminal === true;
  return {
    state: { ...state, slots, activeSlot: slot, pendingSlot: null },
    effect: terminal ? "terminal-promoted" : "none",
  };
}

export function failRustFrame(
  state: RustFrameBufferState,
  slot: 0 | 1,
  src: string,
): RustFrameBufferTransition {
  if (state.pendingSlot !== slot || state.slots[slot].src !== src) {
    return { state, effect: "none" };
  }
  const pending = state.slots[slot];
  const slots = cloneSlots(state);
  // Live frames fail with 204 when the request arrives after the engine has
  // already published a newer frame (slow render / fast publish). A single
  // retry re-issues the same URL, which the backend now resolves to the newest
  // published frame, so transient races do not freeze the preview on the idle
  // still. Terminal frames keep their two retries.
  const retryable = pending.frame?.terminal
    ? pending.retryCount < 2
    : pending.retryCount < 1;
  if (retryable) {
    const retryCount = pending.retryCount + 1;
    slots[slot] = {
      ...pending,
      src: retryUrl(src, retryCount),
      requestCount: pending.requestCount + 1,
      retryCount,
      visible: false,
    };
    return { state: { ...state, slots }, effect: "none" };
  }
  slots[slot] = emptySlot();
  return {
    state: { ...state, slots, pendingSlot: null },
    effect: pending.frame?.terminal ? "terminal-exhausted" : "none",
  };
}

export function releaseRustFrameAfterComposite(
  state: RustFrameBufferState,
  handoff: RustFrameIdentity & {
    frame: number;
    engineDriving: boolean;
    compositeLoaded: boolean;
  },
): RustFrameBufferState {
  if (handoff.engineDriving || !handoff.compositeLoaded || state.activeSlot === null) return state;
  const active = state.slots[state.activeSlot].frame;
  if (
    !active?.terminal ||
    active.projectEpoch !== handoff.projectEpoch ||
    active.timelineVersion !== handoff.timelineVersion ||
    active.sessionId !== handoff.sessionId ||
    active.frame !== handoff.frame
  ) {
    return state;
  }
  return createRustFrameBufferState(state.identity);
}
