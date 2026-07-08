import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  refreshMedia: vi.fn<() => Promise<void>>(),
  saveClipAsMediaApi: vi.fn<(clipId: string) => Promise<unknown>>(),
}));

vi.mock("../lib/api", () => ({
  isTauri: true,
  saveClipAsMedia: mocks.saveClipAsMediaApi,
}));

vi.mock("./mediaStore", () => ({
  refreshMedia: mocks.refreshMedia,
}));

import { saveClipAsMedia } from "./editActions";

describe("saveClipAsMedia", () => {
  beforeEach(() => {
    mocks.saveClipAsMediaApi.mockReset();
    mocks.refreshMedia.mockReset();
    mocks.saveClipAsMediaApi.mockResolvedValue({});
    mocks.refreshMedia.mockResolvedValue();
  });

  it("uses the single-clip backend command signature", async () => {
    await saveClipAsMedia("clip-123");

    expect(mocks.saveClipAsMediaApi).toHaveBeenCalledWith("clip-123");
    expect(mocks.saveClipAsMediaApi).toHaveBeenCalledTimes(1);
    expect(mocks.refreshMedia).toHaveBeenCalledTimes(1);
  });
});
