import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }));

describe("loudness analysis IPC", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
    mocks.invoke.mockReset().mockResolvedValue({});
    mocks.listen.mockReset().mockResolvedValue(vi.fn());
  });

  afterEach(() => vi.unstubAllGlobals());

  it("passes the target and true-peak ceiling to the native analyzer", async () => {
    const { analyzeLoudness } = await import("./api");
    await analyzeLoudness("clip-a", -16, -1);
    expect(mocks.invoke).toHaveBeenCalledWith("analyze_loudness", {
      clipId: "clip-a",
      targetLufs: -16,
      truePeakCeilingDbtp: -1,
    });
  });

  it("filters progress by clip identity", async () => {
    const { onLoudnessProgress } = await import("./api");
    const handler = vi.fn();
    await onLoudnessProgress("clip-a", handler);
    const listener = mocks.listen.mock.calls[0]?.[1] as
      | ((event: { payload: unknown }) => void)
      | undefined;
    listener?.({ payload: { clipId: "clip-b", done: 50, total: 100 } });
    listener?.({ payload: { clipId: "clip-a", done: 75, total: 100 } });
    expect(handler).toHaveBeenCalledOnce();
    expect(handler).toHaveBeenCalledWith({ clipId: "clip-a", done: 75, total: 100 });
  });
});
