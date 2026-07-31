import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }));

describe("audio denoise IPC", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
    mocks.invoke.mockReset().mockResolvedValue({});
    mocks.listen.mockReset().mockResolvedValue(vi.fn());
  });

  afterEach(() => vi.unstubAllGlobals());

  it("passes all persisted processing parameters to the native validator", async () => {
    const { prepareDenoise } = await import("./api");
    await prepareDenoise("clip-a", "voice", 0.85, false);
    expect(mocks.invoke).toHaveBeenCalledWith("prepare_denoise", {
      clipId: "clip-a",
      mode: "voice",
      strength: 0.85,
      previewEnabled: false,
    });
  });

  it("filters progress by clip identity", async () => {
    const { onDenoiseProgress } = await import("./api");
    const handler = vi.fn();
    await onDenoiseProgress("clip-a", handler);
    const listener = mocks.listen.mock.calls[0]?.[1] as
      | ((event: { payload: unknown }) => void)
      | undefined;
    listener?.({ payload: { clipId: "clip-b", done: 50, total: 100 } });
    listener?.({ payload: { clipId: "clip-a", done: 75, total: 100 } });
    expect(handler).toHaveBeenCalledOnce();
    expect(handler).toHaveBeenCalledWith({ clipId: "clip-a", done: 75, total: 100 });
  });

  it("routes cooperative cancellation to the native analysis state", async () => {
    const { cancelDenoiseAnalysis } = await import("./api");
    await cancelDenoiseAnalysis();
    expect(mocks.invoke).toHaveBeenCalledWith("cancel_denoise_analysis");
  });
});
