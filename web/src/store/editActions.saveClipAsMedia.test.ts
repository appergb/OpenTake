import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  operationId: "save-as:test-request",
  refreshMedia: vi.fn<() => Promise<void>>(),
  saveClipAsMediaApi: vi.fn<(clipId: string, operationId: string) => Promise<unknown>>(),
  saveRangeAsMediaApi:
    vi.fn<(inFrame: number, outFrame: number, operationId: string) => Promise<unknown>>(),
  onExportProgress: vi.fn(),
  progressUnlisten: vi.fn(),
  cancelExport: vi.fn<(operationId: string) => Promise<void>>(),
}));

vi.mock("../lib/api", () => ({
  isTauri: true,
  createExportOperationId: () => mocks.operationId,
  saveClipAsMedia: mocks.saveClipAsMediaApi,
  saveRangeAsMedia: mocks.saveRangeAsMediaApi,
  onExportProgress: mocks.onExportProgress,
  cancelExport: mocks.cancelExport,
  EXPORT_CANCELLED_SENTINEL: "export cancelled",
}));

vi.mock("./mediaStore", () => ({
  refreshMedia: mocks.refreshMedia,
}));

import { cancelSaveAsMedia, saveClipAsMedia, saveMarkedRangeAsMedia } from "./editActions";
import { useEditorUiStore } from "./uiStore";

describe("saveClipAsMedia", () => {
  beforeEach(() => {
    mocks.saveClipAsMediaApi.mockReset();
    mocks.refreshMedia.mockReset();
    mocks.saveClipAsMediaApi.mockResolvedValue({});
    mocks.saveRangeAsMediaApi.mockReset();
    mocks.saveRangeAsMediaApi.mockResolvedValue({});
    mocks.refreshMedia.mockResolvedValue();
    mocks.progressUnlisten.mockReset();
    mocks.onExportProgress.mockReset().mockResolvedValue(mocks.progressUnlisten);
    mocks.cancelExport.mockReset().mockResolvedValue();
    useEditorUiStore.getState().setSaveAsProgress(null);
  });

  it("uses the single-clip backend command signature", async () => {
    await saveClipAsMedia("clip-123");

    expect(mocks.saveClipAsMediaApi).toHaveBeenCalledWith("clip-123", mocks.operationId);
    expect(mocks.saveClipAsMediaApi).toHaveBeenCalledTimes(1);
    expect(mocks.refreshMedia).toHaveBeenCalledTimes(1);
    expect(mocks.onExportProgress).toHaveBeenCalledTimes(1);
    expect(mocks.progressUnlisten).toHaveBeenCalledTimes(1);
    expect(useEditorUiStore.getState().saveAsProgress).toBeNull();
  });

  it("saveMarkedRangeAsMedia keeps the range and clip selection", async () => {
    const range = { startFrame: 12, endFrame: 48 };
    const selected = new Set(["clip-123"]);
    useEditorUiStore.setState({ selectedTimelineRange: range, selectedClipIds: selected });

    await saveMarkedRangeAsMedia(range);

    expect(mocks.saveRangeAsMediaApi).toHaveBeenCalledWith(12, 48, mocks.operationId);
    expect(mocks.refreshMedia).toHaveBeenCalledTimes(1);
    expect(useEditorUiStore.getState().selectedTimelineRange).toEqual(range);
    expect(useEditorUiStore.getState().selectedClipIds).toEqual(selected);
  });

  it("exposes save-as progress and routes the visible cancel action", async () => {
    let resolveSave: (() => void) | undefined;
    mocks.saveClipAsMediaApi.mockImplementation(
      () => new Promise<void>((resolve) => (resolveSave = resolve)),
    );
    const saving = saveClipAsMedia("clip-123");
    await vi.waitFor(() => expect(useEditorUiStore.getState().saveAsProgress?.cancellable).toBe(true));

    await cancelSaveAsMedia();

    expect(mocks.cancelExport).toHaveBeenCalledWith(mocks.operationId);
    expect(useEditorUiStore.getState().saveAsProgress?.cancelling).toBe(true);
    resolveSave?.();
    await saving;
    expect(mocks.progressUnlisten).toHaveBeenCalledTimes(1);
  });

  it("treats backend cancellation as a neutral terminal state", async () => {
    mocks.saveClipAsMediaApi.mockRejectedValueOnce(new Error("export cancelled"));

    await saveClipAsMedia("clip-123");

    expect(useEditorUiStore.getState().toast?.message).toContain("Save as media cancelled");
    expect(mocks.refreshMedia).not.toHaveBeenCalled();
    expect(mocks.progressUnlisten).toHaveBeenCalledTimes(1);
  });
});
