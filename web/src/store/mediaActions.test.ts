import { beforeEach, describe, expect, it, vi } from "vitest";
import type { MediaList } from "../lib/types";

const srv = vi.hoisted(() => ({
  selected: ["/tmp/new.mov"],
  imported: {
    items: [
      { id: "old", name: "old", type: "video", duration: 10, hasAudio: true, path: "/tmp/old.mov" },
      { id: "fresh-video", name: "fresh", type: "video", duration: 10, hasAudio: true, path: "/tmp/new.mov" },
      { id: "fresh-audio", name: "audio", type: "audio", duration: 10, hasAudio: false, path: "/tmp/a.wav" },
      { id: "fresh-image", name: "still", type: "image", duration: 1, hasAudio: false, path: "/tmp/i.png" },
    ],
    folders: [],
  } as MediaList,
  importMedia: vi.fn(),
  getMedia: vi.fn(),
  preloadMedia: vi.fn(),
  open: vi.fn(),
}));

vi.mock("../lib/api", () => ({
  importMedia: srv.importMedia,
  getMedia: srv.getMedia,
  preloadMedia: srv.preloadMedia,
}));

vi.mock("../lib/dialog", () => ({
  openDialog: async () => srv.open,
}));

import { importFilesViaDialog } from "./mediaActions";
import { useMediaStore } from "./mediaStore";

describe("mediaActions import warmup", () => {
  beforeEach(() => {
    srv.importMedia.mockReset();
    srv.getMedia.mockReset();
    srv.preloadMedia.mockReset();
    srv.open.mockReset();
    srv.open.mockResolvedValue(srv.selected);
    srv.importMedia.mockResolvedValue(srv.imported);
    srv.getMedia.mockResolvedValue(srv.imported);
    useMediaStore.setState({
      items: [
        { id: "old", name: "old", type: "video", duration: 10, hasAudio: true, path: "/tmp/old.mov" },
      ],
      folders: [],
      importing: false,
      error: null,
    });
  });

  it("preloads newly imported timeline-capable media after file import", async () => {
    await importFilesViaDialog();

    expect(srv.preloadMedia).toHaveBeenCalledTimes(2);
    expect(srv.preloadMedia).toHaveBeenNthCalledWith(1, "fresh-video");
    expect(srv.preloadMedia).toHaveBeenNthCalledWith(2, "fresh-audio");
    expect(srv.preloadMedia).not.toHaveBeenCalledWith("old");
    expect(srv.preloadMedia).not.toHaveBeenCalledWith("fresh-image");
  });
});
