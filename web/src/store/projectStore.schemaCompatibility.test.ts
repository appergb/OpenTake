import { beforeEach, describe, expect, it } from "vitest";
import type { RuntimeTimelineSnapshot, Timeline } from "../lib/types";
import { useProjectStore } from "./projectStore";

const UNKNOWN_TIMELINE: Timeline = {
  fps: 24,
  width: 3840,
  height: 2160,
  settingsConfigured: true,
  tracks: [],
};

function snapshot(
  overrides: Partial<RuntimeTimelineSnapshot> = {},
): RuntimeTimelineSnapshot {
  return {
    timeline: UNKNOWN_TIMELINE,
    projectEpoch: 8,
    version: 13,
    projectPath: "/Volumes/QA/unknown.opentake",
    compatibilityReadOnly: true,
    compatibilityBlockers: ["timeline.tracks[0].futureField"],
    ...overrides,
  };
}

describe("project snapshot schema compatibility", () => {
  beforeEach(() => {
    useProjectStore.getState().clearProjectSnapshot();
  });

  it("replaces project identity, mirror, path, and compatibility in one update", () => {
    const updates: number[] = [];
    const unsubscribe = useProjectStore.subscribe((state) => {
      updates.push(state.timelineVersion);
    });

    useProjectStore.getState().replaceProjectSnapshot(snapshot());
    unsubscribe();

    const state = useProjectStore.getState();
    expect(updates).toEqual([13]);
    expect(state.projectEpoch).toBe(8);
    expect(state.timelineVersion).toBe(13);
    expect(state.timeline).toBe(UNKNOWN_TIMELINE);
    expect(state.projectPath).toBe("/Volumes/QA/unknown.opentake");
    expect(state.compatibilityReadOnly).toBe(true);
    expect(state.compatibilityBlockers).toEqual(["timeline.tracks[0].futureField"]);
  });

  it("clears compatibility when replacing the mirror with a known project", () => {
    useProjectStore.getState().replaceProjectSnapshot(snapshot());

    useProjectStore.getState().replaceProjectSnapshot(
      snapshot({
        projectEpoch: 9,
        version: 0,
        projectPath: "/tmp/known.opentake",
        compatibilityReadOnly: false,
        compatibilityBlockers: [],
      }),
    );

    const state = useProjectStore.getState();
    expect(state.projectEpoch).toBe(9);
    expect(state.projectPath).toBe("/tmp/known.opentake");
    expect(state.compatibilityReadOnly).toBe(false);
    expect(state.compatibilityBlockers).toEqual([]);
  });
});
