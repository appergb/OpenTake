import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }));

describe("search index project identity IPC", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
    mocks.invoke.mockReset().mockResolvedValue({
      modelInstalled: true,
      indexable: 1,
      indexed: 1,
    });
    mocks.listen.mockReset().mockResolvedValue(vi.fn());
  });

  afterEach(() => vi.unstubAllGlobals());

  it("forwards the captured project epoch and path when indexing starts", async () => {
    const { searchIndexStart } = await import("./api");

    await searchIndexStart(42, "/Projects/A.opentake");

    expect(mocks.invoke).toHaveBeenCalledWith("search_index_start", {
      expectedProjectEpoch: 42,
      expectedProjectPath: "/Projects/A.opentake",
    });
  });
});
