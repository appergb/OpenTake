import { beforeEach, describe, expect, it, vi } from "vitest";

const { syncProjectFavorites } = vi.hoisted(() => ({
  syncProjectFavorites: vi.fn(),
}));
vi.mock("../../lib/api", () => ({ syncProjectFavorites }));

function makeLocalStorage(): Storage {
  const map = new Map<string, string>();
  return {
    getItem: (key) => map.get(key) ?? null,
    setItem: (key, value) => void map.set(key, String(value)),
    removeItem: (key) => void map.delete(key),
    clear: () => map.clear(),
    key: (index) => [...map.keys()][index] ?? null,
    get length() {
      return map.size;
    },
  } as Storage;
}

const KEY = "opentake.favorites";

describe("migrateLocalFavorites", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.stubGlobal("localStorage", makeLocalStorage());
    syncProjectFavorites.mockReset().mockResolvedValue({
      media: { items: [], folders: [] },
      migratedLegacyAssetIds: [],
      failures: [],
    });
  });

  it("sends only current-project legacy ids and removes only confirmed migrations", async () => {
    localStorage.setItem(KEY, JSON.stringify(["a", "b", "other-project"]));
    syncProjectFavorites.mockResolvedValueOnce({
      media: { items: [], folders: [] },
      migratedLegacyAssetIds: ["a"],
      failures: [{ assetId: "b", message: "offline" }],
    });
    const { migrateLocalFavorites } = await import("./favorites");

    const outcome = await migrateLocalFavorites([{ id: "a" }, { id: "b" }], 4);

    expect(syncProjectFavorites).toHaveBeenCalledWith(["a", "b"]);
    expect(outcome.failures).toEqual([{ assetId: "b", message: "offline" }]);
    expect(JSON.parse(localStorage.getItem(KEY) as string)).toEqual(["b", "other-project"]);
  });

  it("reconciles manifest favorites once per project epoch even without local data", async () => {
    const { migrateLocalFavorites } = await import("./favorites");

    expect((await migrateLocalFavorites([{ id: "a" }], 10)).synced).toBe(true);
    expect((await migrateLocalFavorites([{ id: "a" }], 10)).synced).toBe(false);
    expect((await migrateLocalFavorites([{ id: "a" }], 11)).synced).toBe(true);

    expect(syncProjectFavorites).toHaveBeenNthCalledWith(1, []);
    expect(syncProjectFavorites).toHaveBeenNthCalledWith(2, []);
  });

  it("retains local ids and allows retry when synchronization rejects", async () => {
    localStorage.setItem(KEY, JSON.stringify(["a"]));
    syncProjectFavorites.mockRejectedValueOnce(new Error("library unavailable"));
    const { migrateLocalFavorites } = await import("./favorites");

    await expect(migrateLocalFavorites([{ id: "a" }], 7)).rejects.toThrow(
      "library unavailable",
    );
    await migrateLocalFavorites([{ id: "a" }], 7);

    expect(syncProjectFavorites).toHaveBeenCalledTimes(2);
    expect(JSON.parse(localStorage.getItem(KEY) as string)).toEqual(["a"]);
  });

  it("waits for the project media mirror before consuming an epoch with legacy ids", async () => {
    localStorage.setItem(KEY, JSON.stringify(["a"]));
    const { migrateLocalFavorites } = await import("./favorites");

    expect((await migrateLocalFavorites([], 12)).synced).toBe(false);
    expect(syncProjectFavorites).not.toHaveBeenCalled();

    await migrateLocalFavorites([{ id: "a" }], 12);
    expect(syncProjectFavorites).toHaveBeenCalledWith(["a"]);
  });
});
