import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }));

describe("save-as media IPC operation identity", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
    mocks.invoke.mockReset();
    mocks.invoke.mockResolvedValue({ items: [], folders: [] });
    mocks.listen.mockReset().mockResolvedValue(vi.fn());
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("sends the operation id with range coordinates", async () => {
    const { saveRangeAsMedia } = await import("./api");

    await saveRangeAsMedia(12, 48, "save-range-request");

    expect(mocks.invoke).toHaveBeenCalledWith("save_range_as_media", {
      request: {
        inFrame: 12,
        outFrame: 48,
        operationId: "save-range-request",
      },
    });
  });

  it("uses the same explicit identity shape for clip save and cancel", async () => {
    const { cancelExport, saveClipAsMedia } = await import("./api");

    await saveClipAsMedia("clip-123", "save-clip-request");
    await cancelExport("save-clip-request");

    expect(mocks.invoke).toHaveBeenNthCalledWith(1, "save_clip_as_media", {
      clipId: "clip-123",
      operationId: "save-clip-request",
    });
    expect(mocks.invoke).toHaveBeenNthCalledWith(2, "cancel_export", {
      operationId: "save-clip-request",
    });
  });

  it("delivers progress only to the matching operation", async () => {
    const { onExportProgress } = await import("./api");
    const onProgress = vi.fn();
    await onExportProgress("save-current", onProgress);
    const listener = mocks.listen.mock.calls[0]?.[1] as
      | ((event: { payload: unknown }) => void)
      | undefined;

    listener?.({ payload: { operationId: "save-old", done: 9, total: 10 } });
    expect(onProgress).not.toHaveBeenCalled();

    listener?.({ payload: { operationId: "save-current", done: 4, total: 10 } });
    expect(onProgress).toHaveBeenCalledWith({
      operationId: "save-current",
      done: 4,
      total: 10,
    });
  });

  it("binds a normal video export start to the same operation-id contract", async () => {
    const { exportVideo } = await import("./api");
    const req = { outPath: "/tmp/out.mp4", codec: "h264" as const, quality: "720p" as const };

    await exportVideo(req, "video-request");

    expect(mocks.invoke).toHaveBeenCalledWith("export_video", {
      req,
      operationId: "video-request",
    });
  });
});
