import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }));

describe("stem separation IPC", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
    mocks.invoke.mockReset().mockResolvedValue({});
    mocks.listen.mockReset().mockResolvedValue(vi.fn());
  });

  afterEach(() => vi.unstubAllGlobals());

  it("passes explicit local/hosted privacy routing fields", async () => {
    const { separateAudioStems } = await import("./api");
    await separateAudioStems("asset-a", "hosted", "stemhost", "stemhost:v1", true);
    expect(mocks.invoke).toHaveBeenCalledWith("separate_audio_stems", {
      sourceAssetId: "asset-a",
      execution: "hosted",
      provider: "stemhost",
      model: "stemhost:v1",
      uploadConfirmed: true,
    });
  });

  it("filters progress by source asset and routes cancellation", async () => {
    const { cancelStemSeparation, onStemSeparationProgress } = await import("./api");
    const handler = vi.fn();
    await onStemSeparationProgress("asset-a", handler);
    const listener = mocks.listen.mock.calls[0]?.[1] as
      | ((event: { payload: unknown }) => void)
      | undefined;
    listener?.({ payload: { sourceAssetId: "asset-b", done: 2, total: 10 } });
    listener?.({ payload: { sourceAssetId: "asset-a", done: 7, total: 10 } });
    expect(handler).toHaveBeenCalledOnce();
    expect(handler).toHaveBeenCalledWith({ sourceAssetId: "asset-a", done: 7, total: 10 });
    await cancelStemSeparation();
    expect(mocks.invoke).toHaveBeenCalledWith("cancel_stem_separation");
  });

  it("imports_reviewed_stems_to_aligned_tracks", async () => {
    const { importStemsToTracks } = await import("./api");
    await importStemsToTracks("vocals", "music", 42);
    expect(mocks.invoke).toHaveBeenCalledWith("import_stems_to_tracks", {
      vocalsAssetId: "vocals",
      accompanimentAssetId: "music",
      startFrame: 42,
    });
  });
});
