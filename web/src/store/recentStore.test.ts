// @vitest-environment happy-dom

import { beforeEach, expect, it, vi } from "vitest";
import type { Timeline } from "../lib/types";

const mocks = vi.hoisted(() => ({
  projectSave: vi.fn(async () => "/tmp/Metadata.opentake"),
}));

vi.mock("../lib/api", () => ({
  isTauri: false,
  projectSave: mocks.projectSave,
}));

vi.mock("../components/preview/nativePlaybackSession", () => ({
  stopNativePlaybackForProjectBoundary: vi.fn(),
}));

vi.mock("../lib/dialog", () => ({
  openDialog: vi.fn(),
  saveDialog: vi.fn(),
}));

import { saveCurrentProject } from "./projectActions";
import { useProjectStore } from "./projectStore";
import { useRecentStore } from "./recentStore";

const TIMELINE: Timeline = {
  fps: 30,
  width: 1920,
  height: 1080,
  settingsConfigured: true,
  tracks: [],
};

beforeEach(() => {
  localStorage.clear();
  mocks.projectSave.mockClear();
  useProjectStore.setState({
    snapshotMutationRevision: 1,
    projectEpoch: 7,
    projectPath: "/tmp/Metadata.opentake",
    timeline: TIMELINE,
    timelineVersion: 2,
    lastSavedVersion: 1,
  });
  useRecentStore.setState({
    recents: [{
      path: "/tmp/Metadata.opentake",
      name: "Metadata",
      openedAt: 1_000,
      createdAt: 500,
      modifiedAt: 750,
      thumbnailPath: null,
    }],
  });
});

it("autosave_and_home_metadata_have_separate_owners", async () => {
  vi.setSystemTime(6_000);
  await saveCurrentProject();

  expect(mocks.projectSave).toHaveBeenCalledWith(null);
  expect(useRecentStore.getState().recents[0]).toMatchObject({
    openedAt: 1_000,
    modifiedAt: 6_000,
    thumbnailPath: "/tmp/Metadata.opentake/thumbnail.jpg",
  });
  expect(JSON.parse(localStorage.getItem("recentProjects") ?? "[]")[0]).toMatchObject({
    modifiedAt: 6_000,
    thumbnailPath: "/tmp/Metadata.opentake/thumbnail.jpg",
  });

  vi.setSystemTime(7_000);
  useRecentStore.getState().add("/tmp/Metadata.opentake");
  expect(useRecentStore.getState().recents[0]).toMatchObject({
    openedAt: 7_000,
    modifiedAt: 6_000,
    thumbnailPath: "/tmp/Metadata.opentake/thumbnail.jpg",
  });
});
