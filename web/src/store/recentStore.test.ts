// @vitest-environment happy-dom

import { beforeEach, expect, it, vi } from "vitest";
import type { Timeline } from "../lib/types";

const mocks = vi.hoisted(() => ({
  projectSave: vi.fn(async () => "/tmp/Metadata.opentake"),
  homeProjectsSync: vi.fn(async (legacy: unknown) => legacy),
  homeProjectRegister: vi.fn(async () => undefined),
}));

vi.mock("../lib/api", () => ({
  isTauri: false,
  projectSave: mocks.projectSave,
  homeProjectsSync: mocks.homeProjectsSync,
  homeProjectRegister: mocks.homeProjectRegister,
  motionDocumentList: vi.fn().mockResolvedValue([]),
  motionDocumentCreate: vi.fn(),
  motionDocumentRead: vi.fn(),
  motionDocumentHash: vi.fn(),
  motionDocumentPatch: vi.fn(),
  motionPreview: vi.fn(),
  motionPreviewCancel: vi.fn().mockResolvedValue(false),
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
import { decodeRecentProjects, projectThumbnailPath, useRecentStore } from "./recentStore";

const defaultValidateRecents = useRecentStore.getState().validateRecents;

const TIMELINE: Timeline = {
  fps: 30,
  width: 1920,
  height: 1080,
  settingsConfigured: true,
  tracks: [],
};

beforeEach(() => {
  delete (window as Window & { __TAURI_INTERNALS__?: object }).__TAURI_INTERNALS__;
  localStorage.clear();
  mocks.projectSave.mockClear();
  mocks.homeProjectsSync.mockReset();
  mocks.homeProjectsSync.mockImplementation(async (legacy: unknown) => legacy);
  mocks.homeProjectRegister.mockClear();
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
    mutationRevision: 0,
    validateRecents: defaultValidateRecents,
  });
});

it("autosave_and_home_metadata_have_separate_owners", async () => {
  vi.setSystemTime(6_000);
  await saveCurrentProject();

  expect(mocks.projectSave).toHaveBeenCalledWith(
    null,
    7,
    "/tmp/Metadata.opentake",
  );
  expect(useRecentStore.getState().recents[0]).toMatchObject({
    openedAt: 1_000,
    modifiedAt: 6_000,
    thumbnailPath: "/tmp/Metadata.opentake/thumbnail.jpg",
    preview: {
      canvasWidth: 1920,
      canvasHeight: 1080,
      trackKinds: [],
    },
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

it("bounds_deduplicates_and_sanitizes_the_untrusted_startup_cache", () => {
  const entries = Array.from({ length: 40 }, (_, index) => ({
    path: `/tmp/Project ${index}.opentake`,
    name: "attacker-controlled-name",
    openedAt: index,
    thumbnailPath: index === 0
      ? "/dev/zero"
      : projectThumbnailPath(`/tmp/Project ${index}.opentake`),
    preview: index === 1
      ? { canvasWidth: 1080, canvasHeight: 1920, trackKinds: ["video", "audio"] }
      : { canvasWidth: 0, canvasHeight: 1080, trackKinds: ["invented"] },
  }));
  entries.splice(1, 0, { ...entries[0] });
  entries.splice(2, 0, {
    ...entries[0],
    path: `${"x".repeat(32_769)}.opentake`,
  });

  const decoded = decodeRecentProjects(JSON.stringify(entries));

  expect(decoded).toHaveLength(12);
  expect(new Set(decoded.map((entry) => entry.path)).size).toBe(12);
  expect(decoded[0]).toMatchObject({
    path: "/tmp/Project 0.opentake",
    name: "Project 0",
    thumbnailPath: null,
  });
  expect(decoded[0]?.preview).toBeUndefined();
  expect(decoded[1]?.thumbnailPath).toBe("/tmp/Project 1.opentake/thumbnail.jpg");
  expect(decoded[1]?.preview).toEqual({
    canvasWidth: 1080,
    canvasHeight: 1920,
    trackKinds: ["video", "audio"],
  });
});

it("rejects_cached_preview_dimensions_outside_the_native_i32_contract", () => {
  const [decoded] = decodeRecentProjects(JSON.stringify([{
    path: "/tmp/Huge.opentake",
    openedAt: 1,
    preview: {
      canvasWidth: Number.MAX_VALUE,
      canvasHeight: 1080,
      trackKinds: [],
    },
  }]));

  expect(decoded?.preview).toBeUndefined();
});

it("coalesces_concurrent_recent_validation_into_one_native_sync", async () => {
  (window as Window & { __TAURI_INTERNALS__?: object }).__TAURI_INTERNALS__ = {};
  let resolveSync!: (value: unknown) => void;
  const response = new Promise<unknown>((resolve) => {
    resolveSync = resolve;
  });
  mocks.homeProjectsSync.mockImplementationOnce(async () => response);

  const first = useRecentStore.getState().validateRecents();
  const second = useRecentStore.getState().validateRecents();

  expect(second).toBe(first);
  await vi.waitFor(() => expect(mocks.homeProjectsSync).toHaveBeenCalledTimes(1));
  resolveSync([{
    path: "/tmp/Metadata.opentake",
    name: "Metadata",
    createdAt: 500,
    openedAt: 1_000,
    modifiedAt: 750,
    thumbnailPath: null,
    missing: false,
    offline: false,
  }]);
  await Promise.all([first, second]);

  expect(mocks.homeProjectsSync).toHaveBeenCalledTimes(1);
});

it("retries_native_sync_instead_of_overwriting_a_concurrent_local_mutation", async () => {
  (window as Window & { __TAURI_INTERNALS__?: object }).__TAURI_INTERNALS__ = {};
  let resolveFirst!: (value: unknown) => void;
  const firstResponse = new Promise<unknown>((resolve) => {
    resolveFirst = resolve;
  });
  mocks.homeProjectsSync
    .mockImplementationOnce(async () => firstResponse)
    .mockImplementationOnce(async (legacy: unknown) => legacy);

  const validation = useRecentStore.getState().validateRecents();
  await vi.waitFor(() => expect(mocks.homeProjectsSync).toHaveBeenCalledTimes(1));

  useRecentStore.getState().add("/tmp/Added-During-Sync.opentake");
  resolveFirst([{
    path: "/tmp/Metadata.opentake",
    name: "Metadata",
    openedAt: 1_000,
  }]);

  await validation;

  expect(mocks.homeProjectsSync).toHaveBeenCalledTimes(2);
  expect(mocks.homeProjectsSync.mock.calls[1]?.[0]).toEqual(expect.arrayContaining([
    expect.objectContaining({ path: "/tmp/Added-During-Sync.opentake" }),
  ]));
  expect(useRecentStore.getState().recents.map(({ path }) => path)).toContain(
    "/tmp/Added-During-Sync.opentake",
  );
});
