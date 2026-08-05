import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }));

const USAGE_FIXTURE = {
  categories: [
    { id: "thumbnails", bytes: 150, path: "/cache/MediaVisualCache" },
    { id: "waveforms", bytes: 100, path: "/cache/MediaVisualCache" },
    { id: "searchIndex", bytes: 50, path: "/cache/Embeddings" },
    { id: "models", bytes: 500, path: "/data/models" },
    { id: "other", bytes: 210, path: "/cache" },
  ],
  totalBytes: 1010,
  cacheRoot: "/cache",
};

describe("settings storage IPC", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
    mocks.invoke.mockReset().mockResolvedValue(USAGE_FIXTURE);
    mocks.listen.mockReset().mockResolvedValue(vi.fn());
  });

  afterEach(() => vi.unstubAllGlobals());

  it("storageUsage reads the read-only usage command without arguments", async () => {
    const { storageUsage } = await import("./api");

    await expect(storageUsage()).resolves.toEqual(USAGE_FIXTURE);

    expect(mocks.invoke).toHaveBeenCalledWith("storage_usage");
  });

  it("storageClear forwards the requested categories with an unconfirmed models gate by default", async () => {
    const { storageClear } = await import("./api");

    await storageClear(["thumbnails", "searchIndex"]);

    expect(mocks.invoke).toHaveBeenCalledWith("storage_clear", {
      request: { categories: ["thumbnails", "searchIndex"], modelsConfirmed: false },
    });
  });

  it("storageClear forwards modelsConfirmed only when the caller passes it", async () => {
    const { storageClear } = await import("./api");

    await storageClear(["models"], true);

    expect(mocks.invoke).toHaveBeenCalledWith("storage_clear", {
      request: { categories: ["models"], modelsConfirmed: true },
    });
  });

  it("storageClear returns the fresh usage snapshot from the backend", async () => {
    const { storageClear } = await import("./api");
    const emptied = {
      ...USAGE_FIXTURE,
      categories: USAGE_FIXTURE.categories.map((category) =>
        category.id === "thumbnails" ? { ...category, bytes: 0 } : category,
      ),
      totalBytes: 860,
    };
    mocks.invoke.mockResolvedValueOnce(emptied);

    await expect(storageClear(["thumbnails"])).resolves.toEqual(emptied);
  });

  it("resolves to an honest empty report outside Tauri instead of fake data", async () => {
    vi.unstubAllGlobals();
    vi.stubGlobal("window", {});
    const { storageUsage, storageClear } = await import("./api");

    const usage = await storageUsage();
    expect(usage.totalBytes).toBe(0);
    expect(usage.cacheRoot).toBe("");
    expect(usage.categories).toHaveLength(5);
    expect(usage.categories.every((category) => category.bytes === 0)).toBe(true);

    const cleared = await storageClear(["thumbnails", "models"], true);
    expect(cleared.totalBytes).toBe(0);
    expect(mocks.invoke).not.toHaveBeenCalled();
  });
});
