import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

describe("saveRangeAsMedia IPC", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
    mocks.invoke.mockReset();
    mocks.invoke.mockResolvedValue({ items: [], folders: [] });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("sends inFrame and outFrame in a camelCase payload", async () => {
    const { saveRangeAsMedia } = await import("./api");

    await saveRangeAsMedia(12, 48);

    expect(mocks.invoke).toHaveBeenCalledWith("save_range_as_media", {
      inFrame: 12,
      outFrame: 48,
    });
  });
});
