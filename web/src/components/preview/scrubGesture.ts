export type ScrubGesturePhase = "down" | "move" | "up" | "cancel";
export type ScrubGestureEffect = "interactive-seek" | "exact-seek" | "none";

export interface ScrubGestureState {
  active: boolean;
}

export interface ScrubGestureTransition {
  state: ScrubGestureState;
  effect: ScrubGestureEffect;
  scrubbing: boolean;
}

export function createScrubGesture(): ScrubGestureState {
  return { active: false };
}

/**
 * Pure pointer lifecycle for the preview scrub bar. Pointer-up is the only
 * phase that publishes an exact seek; a following lost-capture/cancel event is
 * deliberately a no-op so one gesture cannot commit twice.
 */
export function transitionScrubGesture(
  state: ScrubGestureState,
  phase: ScrubGesturePhase,
): ScrubGestureTransition {
  if (phase === "down") {
    return { state: { active: true }, effect: "interactive-seek", scrubbing: true };
  }
  if (!state.active) {
    return { state, effect: "none", scrubbing: false };
  }
  if (phase === "move") {
    return { state, effect: "interactive-seek", scrubbing: true };
  }
  if (phase === "up") {
    return { state: { active: false }, effect: "exact-seek", scrubbing: false };
  }
  return { state: { active: false }, effect: "none", scrubbing: false };
}
