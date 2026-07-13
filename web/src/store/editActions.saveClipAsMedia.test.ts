import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  refreshMedia: vi.fn<() => Promise<void>>(),
  saveClipAsMediaApi: vi.fn<(clipId: string) => Promise<unknown>>(),
  saveRangeAsMediaApi: vi.fn<(inFrame: number, outFrame: number) => Promise<unknown>>(),
}));

vi.mock("../lib/api", () => ({
  isTauri: true,
  saveClipAsMedia: mocks.saveClipAsMediaApi,
  saveRangeAsMedia: mocks.saveRangeAsMediaApi,
}));

vi.mock("./mediaStore", () => ({
  refreshMedia: mocks.refreshMedia,
}));

import { saveClipAsMedia, saveMarkedRangeAsMedia } from "./editActions";
import { useEditorUiStore } from "./uiStore";

describe("saveClipAsMedia", () => {
  beforeEach(() => {
    mocks.saveClipAsMediaApi.mockReset();
    mocks.refreshMedia.mockReset();
    mocks.saveClipAsMediaApi.mockResolvedValue({});
    mocks.saveRangeAsMediaApi.mockReset();
    mocks.saveRangeAsMediaApi.mockResolvedValue({});
    mocks.refreshMedia.mockResolvedValue();
  });

  it("uses the single-clip backend command signature", async () => {
    await saveClipAsMedia("clip-123");

    expect(mocks.saveClipAsMediaApi).toHaveBeenCalledWith("clip-123");
    expect(mocks.saveClipAsMediaApi).toHaveBeenCalledTimes(1);
    expect(mocks.refreshMedia).toHaveBeenCalledTimes(1);
  });

  it("saveMarkedRangeAsMedia keeps the range and clip selection", async () => {
    const range = { startFrame: 12, endFrame: 48 };
    const selected = new Set(["clip-123"]);
    useEditorUiStore.setState({ selectedTimelineRange: range, selectedClipIds: selected });

    await saveMarkedRangeAsMedia(range);

    expect(mocks.saveRangeAsMediaApi).toHaveBeenCalledWith(12, 48);
    expect(mocks.refreshMedia).toHaveBeenCalledTimes(1);
    expect(useEditorUiStore.getState().selectedTimelineRange).toEqual(range);
    expect(useEditorUiStore.getState().selectedClipIds).toEqual(selected);
  });
});
