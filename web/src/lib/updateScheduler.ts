export const INITIAL_UPDATE_CHECK_DELAY_MS = 4_000;
export const UPDATE_CHECK_INTERVAL_MS = 60 * 60 * 1_000;

/**
 * Start the desktop update cadence. The caller owns the Tauri-only guard; this
 * helper only owns timers and intentionally swallows rejected background checks
 * because the update store already keeps background failures non-disruptive.
 */
export function startUpdateScheduler(check: () => Promise<void>): () => void {
  let interval: ReturnType<typeof setInterval> | null = null;
  let stopped = false;
  const run = () => {
    if (stopped) return;
    void check().catch(() => {});
  };
  const initial = setTimeout(() => {
    run();
    if (!stopped) interval = setInterval(run, UPDATE_CHECK_INTERVAL_MS);
  }, INITIAL_UPDATE_CHECK_DELAY_MS);

  return () => {
    stopped = true;
    clearTimeout(initial);
    if (interval !== null) clearInterval(interval);
  };
}
