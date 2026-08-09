import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }));

describe("playback capability IPC", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
    mocks.invoke.mockReset();
    mocks.listen.mockReset().mockResolvedValue(vi.fn());
  });

  afterEach(() => vi.unstubAllGlobals());

  it("treats a missing preview endpoint command as an unavailable capability", async () => {
    mocks.invoke.mockRejectedValue("Command get_preview_endpoint not found");
    const { getPreviewEndpoint } = await import("./api");

    await expect(getPreviewEndpoint()).resolves.toBeNull();
    await expect(getPreviewEndpoint()).resolves.toBeNull();
    expect(mocks.invoke).toHaveBeenCalledOnce();
    expect(mocks.invoke).toHaveBeenCalledWith("get_preview_endpoint");
  });

  it("rethrows a transient probe failure and allows the next call to retry", async () => {
    const transient = { code: "engine", message: "preview server is starting" };
    mocks.invoke
      .mockRejectedValueOnce(transient)
      .mockResolvedValueOnce("http://127.0.0.1:43123/frame");
    const { getPreviewEndpoint } = await import("./api");

    await expect(getPreviewEndpoint()).rejects.toBe(transient);
    await expect(getPreviewEndpoint()).resolves.toBe("http://127.0.0.1:43123/frame");
    expect(mocks.invoke).toHaveBeenCalledTimes(2);
  });

  it("does not treat another missing IPC command as an absent endpoint capability", async () => {
    const unrelatedMissingCommand = "Command playback_start not found";
    mocks.invoke
      .mockRejectedValueOnce(unrelatedMissingCommand)
      .mockResolvedValueOnce("http://127.0.0.1:43123/frame");
    const { getPreviewEndpoint } = await import("./api");

    await expect(getPreviewEndpoint()).rejects.toBe(unrelatedMissingCommand);
    await expect(getPreviewEndpoint()).resolves.toBe("http://127.0.0.1:43123/frame");
    expect(mocks.invoke).toHaveBeenCalledTimes(2);
  });

  it("recognizes only missing playback commands as fallback-safe plain strings", async () => {
    const { isPlaybackCommandUnavailable } = await import("./api");

    expect(isPlaybackCommandUnavailable("Command playback_start not found")).toBe(true);
    expect(isPlaybackCommandUnavailable("Command get_preview_endpoint not found")).toBe(true);
    expect(isPlaybackCommandUnavailable("Command project_save not found")).toBe(false);
    expect(isPlaybackCommandUnavailable("decoder crashed")).toBe(false);
  });
});
