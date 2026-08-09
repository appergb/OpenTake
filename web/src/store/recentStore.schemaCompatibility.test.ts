import { beforeEach, describe, expect, it } from "vitest";
import type { Timeline } from "../lib/types";
import { useProjectStore } from "./projectStore";
import { useRecentStore } from "./recentStore";

const ACTIVE_PATH = "/Volumes/QA/unknown.opentake";

const ACTIVE_TIMELINE: Timeline = {
  fps: 60,
  width: 1920,
  height: 1080,
  settingsConfigured: true,
  tracks: [
    {
      id: "track-1",
      type: "video",
      muted: false,
      hidden: false,
      syncLocked: true,
      clips: [],
    },
  ],
};

describe("recent project compatibility cleanup", () => {
  beforeEach(() => {
    useProjectStore.getState().clearProjectSnapshot();
    useRecentStore.setState({
      recents: [{ path: ACTIVE_PATH, name: "unknown", openedAt: 1 }],
    });
  });

  it("clears the whole active read-only snapshot when its recent entry is removed", async () => {
    useProjectStore.setState({
      projectEpoch: 17,
      timelineVersion: 12,
      timeline: ACTIVE_TIMELINE,
      projectPath: ACTIVE_PATH,
      lastSavedVersion: 12,
      canUndo: true,
      canRedo: true,
      compatibilityReadOnly: true,
      compatibilityBlockers: ["manifest.futureField"],
    });

    await useRecentStore.getState().remove(ACTIVE_PATH);

    const state = useProjectStore.getState();
    expect(useRecentStore.getState().recents).toEqual([]);
    expect(state.projectPath).toBeNull();
    expect(state.projectEpoch).toBe(0);
    expect(state.timelineVersion).toBe(0);
    expect(state.lastSavedVersion).toBe(0);
    expect(state.timeline.tracks).toEqual([]);
    expect(state.canUndo).toBe(false);
    expect(state.canRedo).toBe(false);
    expect(state.compatibilityReadOnly).toBe(false);
    expect(state.compatibilityBlockers).toEqual([]);
  });
});
