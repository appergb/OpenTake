import { afterEach, describe, expect, it, vi } from "vitest";
import {
  INITIAL_UPDATE_CHECK_DELAY_MS,
  UPDATE_CHECK_INTERVAL_MS,
  startUpdateScheduler,
} from "./updateScheduler";

afterEach(() => {
  vi.useRealTimers();
});

describe("update scheduler", () => {
  it("checks after the startup delay, repeats hourly, and cleans up both timers", async () => {
    vi.useFakeTimers();
    const check = vi.fn().mockResolvedValue(undefined);
    const stop = startUpdateScheduler(check);

    expect(check).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(INITIAL_UPDATE_CHECK_DELAY_MS - 1);
    expect(check).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1);
    expect(check).toHaveBeenCalledOnce();

    await vi.advanceTimersByTimeAsync(UPDATE_CHECK_INTERVAL_MS);
    expect(check).toHaveBeenCalledTimes(2);

    stop();
    await vi.advanceTimersByTimeAsync(UPDATE_CHECK_INTERVAL_MS * 2);
    expect(check).toHaveBeenCalledTimes(2);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("can be stopped before the delayed startup check", async () => {
    vi.useFakeTimers();
    const check = vi.fn().mockResolvedValue(undefined);
    const stop = startUpdateScheduler(check);
    stop();

    await vi.runAllTimersAsync();
    expect(check).not.toHaveBeenCalled();
    expect(vi.getTimerCount()).toBe(0);
  });
});
