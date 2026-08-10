import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }));

describe("manual update recovery IPC", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
    mocks.invoke.mockReset().mockResolvedValue(undefined);
    mocks.listen.mockReset().mockResolvedValue(vi.fn());
  });

  afterEach(() => vi.unstubAllGlobals());

  it("exposes no renderer-controlled URL argument", async () => {
    const { openUpdateReleases } = await import("./api");

    await openUpdateReleases();

    expect(mocks.invoke).toHaveBeenCalledWith("open_update_releases");
  });

  it("propagates a native browser launch error to the dialog", async () => {
    mocks.invoke.mockRejectedValueOnce(new Error("no default browser"));
    const { openUpdateReleases } = await import("./api");

    await expect(openUpdateReleases()).rejects.toThrow("no default browser");
  });
});
