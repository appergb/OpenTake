export const INTERACTIVE_SEEK_INTERVAL_MS = 1000 / 30;

export interface InteractiveSeekRequest {
  frame: number;
  toleranceSec: number;
}

export interface InteractiveSeekQueue {
  pending: InteractiveSeekRequest | null;
  lastDispatchMs: number;
}

export function createInteractiveSeekQueue(): InteractiveSeekQueue {
  return { pending: null, lastDispatchMs: Number.NEGATIVE_INFINITY };
}

export function interactiveToleranceSec(activeLayerCount: number): number {
  return Math.min(0.75, 0.15 * Math.max(1, activeLayerCount));
}

export function enqueueInteractiveSeek(
  queue: InteractiveSeekQueue,
  request: InteractiveSeekRequest,
  nowMs: number,
  intervalMs = INTERACTIVE_SEEK_INTERVAL_MS,
): { kind: "flush"; request: InteractiveSeekRequest } | { kind: "schedule"; delayMs: number } {
  queue.pending = request;
  const elapsed = nowMs - queue.lastDispatchMs;
  const delayMs = Math.max(0, intervalMs - elapsed);
  if (delayMs > 0) return { kind: "schedule", delayMs };
  const flushed = flushPendingInteractiveSeek(queue, nowMs);
  return { kind: "flush", request: flushed ?? request };
}

export function flushPendingInteractiveSeek(
  queue: InteractiveSeekQueue,
  nowMs: number,
): InteractiveSeekRequest | null {
  const pending = queue.pending;
  if (!pending) return null;
  queue.pending = null;
  queue.lastDispatchMs = nowMs;
  return pending;
}

export function cancelInteractiveSeek(queue: InteractiveSeekQueue): void {
  queue.pending = null;
}
