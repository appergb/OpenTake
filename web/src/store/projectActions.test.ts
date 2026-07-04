import { beforeEach, describe, expect, it, vi } from "vitest";
import type { MediaList, Timeline } from "../lib/types";

const srv = vi.hoisted(() => {
  const timeline: Timeline = {
    fps: 30,
    width: 1920,
    height: 1080,
    settingsConfigured: true,
    tracks: [],
  };
  const media: MediaList = {
    items: [
      {
        id: "m1",
        name: "clip",
        type: "video",
        duration: 10,
        hasAudio: true,
        path: "/tmp/clip.mov",
      },
    ],
    folders: [],
  };
  return { timeline, media };
});

vi.mock("../lib/api", () => ({
  projectOpen: async () => ({ timeline: srv.timeline, version: 7 }),
  getMedia: async () => srv.media,
}));

import { openProjectPath } from "./projectActions";
import { useEditorUiStore } from "./uiStore";
import { useMediaStore } from "./mediaStore";
import { useProjectStore } from "./projectStore";

describe("openProjectPath", () => {
  beforeEach(() => {
    useMediaStore.getState().setItems([]);
    useProjectStore.setState({ projectPath: null, timelineVersion: 0 });
    useEditorUiStore.setState({ view: "home" });
  });

  it("refreshes the media mirror after opening a project", async () => {
    await openProjectPath("/tmp/demo.opentake");

    expect(useProjectStore.getState().projectPath).toBe("/tmp/demo.opentake");
    expect(useMediaStore.getState().items.map((item) => item.id)).toEqual(["m1"]);
    expect(useEditorUiStore.getState().view).toBe("editor");
  });
});
