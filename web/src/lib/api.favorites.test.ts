import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({ invoke: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

describe("global favorite IPC", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
    mocks.invoke.mockReset().mockResolvedValue({ items: [], folders: [] });
  });

  afterEach(() => vi.unstubAllGlobals());

  it("sends a singular assetId payload", async () => {
    const { toggleFavorite } = await import("./api");

    await toggleFavorite("asset-1", true);

    expect(mocks.invoke).toHaveBeenCalledWith("toggle_favorite", {
      assetId: "asset-1",
      favorite: true,
    });
  });

  it("sends legacy ids to the reconciliation command", async () => {
    const { syncProjectFavorites } = await import("./api");

    await syncProjectFavorites(["asset-1"]);

    expect(mocks.invoke).toHaveBeenCalledWith("sync_project_favorites", {
      legacyAssetIds: ["asset-1"],
    });
  });
});
