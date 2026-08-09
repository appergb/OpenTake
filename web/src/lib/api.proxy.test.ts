import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }));

describe("media proxy IPC", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
    mocks.invoke.mockReset().mockResolvedValue({});
    mocks.listen.mockReset().mockResolvedValue(vi.fn());
  });

  afterEach(() => vi.unstubAllGlobals());

  it("keeps proxy creation bounded and playback preference separate", async () => {
    const { createMediaProxy, setProxyPlaybackEnabled } = await import("./api");
    await createMediaProxy("asset-a", 960, 540);
    expect(mocks.invoke).toHaveBeenCalledWith("create_media_proxy", {
      assetId: "asset-a",
      maxWidth: 960,
      maxHeight: 540,
    });
    await setProxyPlaybackEnabled(true);
    expect(mocks.invoke).toHaveBeenCalledWith("set_proxy_playback_enabled", { enabled: true });
  });

  it("filters progress by asset and exposes cancel/remove", async () => {
    const { cancelMediaProxy, onMediaProxyProgress, removeMediaProxy } = await import("./api");
    const handler = vi.fn();
    await onMediaProxyProgress("asset-a", handler);
    const listener = mocks.listen.mock.calls[0]?.[1] as
      | ((event: { payload: unknown }) => void)
      | undefined;
    listener?.({ payload: { assetId: "asset-b", done: 100, total: 1000 } });
    listener?.({ payload: { assetId: "asset-a", done: 500, total: 1000 } });
    expect(handler).toHaveBeenCalledOnce();
    expect(handler).toHaveBeenCalledWith({ assetId: "asset-a", done: 500, total: 1000 });
    await cancelMediaProxy();
    await removeMediaProxy("asset-a");
    expect(mocks.invoke).toHaveBeenCalledWith("cancel_media_proxy");
    expect(mocks.invoke).toHaveBeenCalledWith("remove_media_proxy", { assetId: "asset-a" });
  });
});
