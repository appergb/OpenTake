import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }));

describe("path existence IPC", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
    mocks.invoke.mockReset().mockResolvedValue(false);
    mocks.listen.mockReset().mockResolvedValue(vi.fn());
  });

  afterEach(() => vi.unstubAllGlobals());

  it("forwards the exact candidate path to the native existence command", async () => {
    mocks.invoke.mockResolvedValueOnce(true);
    const { checkPathExists } = await import("./api");

    await expect(checkPathExists("C:\\Projects\\Untitled 2.opentake")).resolves.toBe(true);
    expect(mocks.invoke).toHaveBeenCalledWith("check_path_exists", {
      path: "C:\\Projects\\Untitled 2.opentake",
    });
  });
});
