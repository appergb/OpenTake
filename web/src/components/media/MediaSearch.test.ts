// @vitest-environment happy-dom

import React, { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { MediaItem, SearchIndexStatus, SearchResults } from "../../lib/types";
import { t } from "../../i18n";
import { useEditorUiStore } from "../../store/uiStore";
import { useMediaStore } from "../../store/mediaStore";
import { useProjectStore } from "../../store/projectStore";

vi.mock("../../lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../lib/api")>();
  return {
    ...actual,
    generateThumbnail: vi.fn().mockResolvedValue(null),
    preloadMedia: vi.fn().mockResolvedValue("cached"),
    searchIndexStatus: vi
      .fn()
      .mockResolvedValue({ modelInstalled: false, indexable: 0, indexed: 0 }),
    searchQuery: vi.fn().mockResolvedValue({ moments: [], spoken: [], files: [] }),
    searchIndexStart: vi.fn().mockResolvedValue(undefined),
    downloadSearchModel: vi.fn().mockResolvedValue(undefined),
    onSearchModelProgress: vi.fn().mockResolvedValue(() => {}),
    onSearchIndexProgress: vi.fn().mockResolvedValue(() => {}),
  };
});
vi.mock("../../store/editActions", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../store/editActions")>();
  return { ...actual, deleteMedia: vi.fn() };
});

import * as api from "../../lib/api";
import * as editActions from "../../store/editActions";
import {
  MediaSearchResults,
  beginMediaSearchRequest,
  subscribeWithAsyncCleanup,
} from "./MediaSearch";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

const emptyResults = (): SearchResults => ({ moments: [], spoken: [], files: [] });
const immediateSchedule = (task: () => void) => {
  task();
  return 1;
};

afterEach(() => {
  vi.useRealTimers();
  document.body.replaceChildren();
  vi.clearAllMocks();
  vi.mocked(api.searchIndexStatus)
    .mockReset()
    .mockResolvedValue({ modelInstalled: false, indexable: 0, indexed: 0 });
  useMediaStore.setState({ items: [], folders: [], importing: false, error: null });
  useProjectStore.setState({ projectEpoch: 0, projectPath: null });
  useEditorUiStore.setState({
    selectedMediaAssetIds: new Set(),
    selectedFolderIds: new Set(),
    previewMediaId: null,
    focusedPanel: null,
  });
});

describe("MediaSearch request lifecycle", () => {
  it("invalidates an in-flight response when the query is cleared", async () => {
    const oldRequest = deferred<SearchResults>();
    const sequence = { current: 0 };
    let results = emptyResults();
    let error: string | null = null;
    const request = beginMediaSearchRequest({
      query: "interview",
      requestSequence: sequence,
      search: () => oldRequest.promise,
      onResults: (next) => {
        results = next;
      },
      onError: (next) => {
        error = next;
      },
      schedule: immediateSchedule,
      cancelScheduled: () => {},
    });

    request.cancel();
    beginMediaSearchRequest({
      query: "",
      requestSequence: sequence,
      search: () => Promise.resolve(emptyResults()),
      onResults: (next) => {
        results = next;
      },
      onError: (next) => {
        error = next;
      },
      schedule: immediateSchedule,
      cancelScheduled: () => {},
    });
    oldRequest.resolve({ moments: [], spoken: [], files: [{ mediaId: "old", score: 1 }] });
    await request.pending();

    expect(results).toEqual(emptyResults());
    expect(error).toBeNull();
  });

  it("clears stale results and reports the current request failure", async () => {
    const failedRequest = deferred<SearchResults>();
    const sequence = { current: 0 };
    let results: SearchResults = {
      moments: [],
      spoken: [],
      files: [{ mediaId: "stale", score: 1 }],
    };
    let error: string | null = null;
    const request = beginMediaSearchRequest({
      query: "missing",
      requestSequence: sequence,
      search: () => failedRequest.promise,
      onResults: (next) => {
        results = next;
      },
      onError: (next) => {
        error = next;
      },
      schedule: immediateSchedule,
      cancelScheduled: () => {},
    });

    failedRequest.reject(new Error("backend unavailable"));
    await request.pending();

    expect(results).toEqual(emptyResults());
    expect(error).toBe("backend unavailable");
  });

  it("invalidates an in-flight same-query response when the project changes", async () => {
    vi.useFakeTimers();
    const projectA = deferred<SearchResults>();
    const projectB = deferred<SearchResults>();
    vi.mocked(api.searchQuery)
      .mockReset()
      .mockReturnValueOnce(projectA.promise)
      .mockReturnValueOnce(projectB.promise);
    useProjectStore.setState({ projectEpoch: 1, projectPath: "/A.opentake" });
    useMediaStore.setState({ items: [searchItem], folders: [], importing: false, error: null });
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    let calls = 0;
    let text = "";
    try {
      await act(async () =>
        root.render(
          React.createElement(MediaSearchResults, {
            query: "same query",
            nameMatches: [],
            hasIndexableAssets: false,
          }),
        ),
      );
      await act(async () => vi.advanceTimersByTime(250));
      await act(async () =>
        useProjectStore.setState({ projectEpoch: 2, projectPath: "/B.opentake" }),
      );
      await act(async () => vi.advanceTimersByTime(250));
      calls = vi.mocked(api.searchQuery).mock.calls.length;

      await act(async () =>
        projectB.resolve({
          moments: [],
          spoken: [
            { mediaId: searchItem.id, startSec: 0, endSec: 1, text: "project-b", score: 1 },
          ],
          files: [],
        }),
      );
      await act(async () =>
        projectA.resolve({
          moments: [],
          spoken: [
            { mediaId: searchItem.id, startSec: 0, endSec: 1, text: "project-a", score: 1 },
          ],
          files: [],
        }),
      );
      text = container.textContent ?? "";
    } finally {
      await act(async () => root.unmount());
      vi.useRealTimers();
    }

    expect(calls).toBe(2);
    expect(text).toContain("project-b");
    expect(text).not.toContain("project-a");
  });
});

describe("MediaSearch async listener cleanup", () => {
  it("guards disposed callbacks and unlistens if subscribe resolves after cleanup", async () => {
    const ready = deferred<() => void>();
    const unlisten = vi.fn();
    const events: number[] = [];
    let emit: ((value: number) => void) | null = null;
    const cleanup = subscribeWithAsyncCleanup<number>(
      (listener) => {
        emit = listener;
        return ready.promise;
      },
      (value) => events.push(value),
    );

    cleanup();
    emit?.(0.5);
    ready.resolve(unlisten);
    await ready.promise;
    await Promise.resolve();

    expect(events).toEqual([]);
    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it("ignores an older status failure after a newer refresh succeeds", async () => {
    const older = deferred<SearchIndexStatus>();
    const newer = deferred<SearchIndexStatus>();
    vi.mocked(api.searchIndexStatus)
      .mockReset()
      .mockResolvedValueOnce({ modelInstalled: false, indexable: 0, indexed: 0 })
      .mockReturnValueOnce(older.promise)
      .mockReturnValueOnce(newer.promise);
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () =>
      root.render(
        React.createElement(MediaSearchResults, {
          query: "",
          nameMatches: [],
          hasIndexableAssets: true,
        }),
      ),
    );
    const emitIndex = vi.mocked(api.onSearchIndexProgress).mock.calls[0]?.[0];
    expect(emitIndex).toBeTypeOf("function");

    await act(async () => {
      emitIndex!({ completed: 0, total: 0, fraction: 0 });
      emitIndex!({ completed: 0, total: 0, fraction: 0 });
    });
    await act(async () =>
      newer.resolve({ modelInstalled: true, indexable: 1, indexed: 0 }),
    );
    expect(container.querySelector("button")).not.toBeNull();

    await act(async () => older.reject(new Error("stale status failure")));
    expect(container.querySelector("button")).not.toBeNull();
    await act(async () => root.unmount());
  });

  it("invalidates index status when project identity changes at the same media count", async () => {
    const projectA = deferred<SearchIndexStatus>();
    const projectB = deferred<SearchIndexStatus>();
    vi.mocked(api.searchIndexStatus)
      .mockReset()
      .mockReturnValueOnce(projectA.promise)
      .mockReturnValueOnce(projectB.promise);
    useProjectStore.setState({ projectEpoch: 10, projectPath: "/A.opentake" });
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () =>
      root.render(
        React.createElement(MediaSearchResults, {
          query: "",
          nameMatches: [],
          hasIndexableAssets: true,
        }),
      ),
    );
    await act(async () =>
      useProjectStore.setState({ projectEpoch: 11, projectPath: "/B.opentake" }),
    );
    const calls = vi.mocked(api.searchIndexStatus).mock.calls.length;

    await act(async () =>
      projectB.resolve({ modelInstalled: false, indexable: 1, indexed: 0 }),
    );
    await act(async () =>
      projectA.resolve({ modelInstalled: true, indexable: 0, indexed: 0 }),
    );
    const hasProjectBAction = container.querySelector("button") !== null;
    await act(async () => root.unmount());

    expect(calls).toBe(2);
    expect(hasProjectBAction).toBe(true);
  });

  it("ignores project A index progress and completion after project B becomes current", async () => {
    const projectAIndex = deferred<void>();
    vi.mocked(api.searchIndexStatus)
      .mockReset()
      .mockResolvedValueOnce({ modelInstalled: true, indexable: 1, indexed: 0 })
      .mockResolvedValueOnce({ modelInstalled: false, indexable: 1, indexed: 0 })
      .mockResolvedValueOnce({ modelInstalled: false, indexable: 1, indexed: 0 });
    vi.mocked(api.searchIndexStart).mockReset().mockReturnValue(projectAIndex.promise);
    useProjectStore.setState({ projectEpoch: 20, projectPath: "/A.opentake" });
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    let before = "";
    let after = "";
    try {
      await act(async () =>
        root.render(
          React.createElement(MediaSearchResults, {
            query: "",
            nameMatches: [],
            hasIndexableAssets: true,
          }),
        ),
      );
      await vi.waitFor(() => expect(container.querySelector("button")).not.toBeNull());
      await act(async () => container.querySelector<HTMLButtonElement>("button")!.click());

      await act(async () =>
        useProjectStore.setState({ projectEpoch: 21, projectPath: "/B.opentake" }),
      );
      await vi.waitFor(() => expect(api.searchIndexStatus).toHaveBeenCalledTimes(2));
      await vi.waitFor(() => expect(container.querySelector("button")).not.toBeNull());
      before = container.querySelector("button")?.textContent ?? "";
      const currentProjectListener = vi.mocked(api.onSearchIndexProgress).mock.calls.at(-1)?.[0];
      await act(async () =>
        currentProjectListener?.({ completed: 1, total: 2, fraction: 0.5 }),
      );
      await act(async () => projectAIndex.resolve(undefined));
      await vi.waitFor(() => expect(api.searchIndexStatus).toHaveBeenCalledTimes(3));
      after = container.querySelector("button")?.textContent ?? "";
    } finally {
      await act(async () => root.unmount());
    }

    expect(before).not.toBe("");
    expect(after).toBe(before);
  });

  it("refreshes project B authoritatively when project A's model download settles", async () => {
    const projectADownload = deferred<void>();
    vi.mocked(api.searchIndexStatus)
      .mockReset()
      .mockResolvedValueOnce({ modelInstalled: false, indexable: 1, indexed: 0 })
      .mockResolvedValueOnce({ modelInstalled: false, indexable: 1, indexed: 0 })
      .mockResolvedValueOnce({ modelInstalled: true, indexable: 1, indexed: 0 });
    vi.mocked(api.downloadSearchModel)
      .mockReset()
      .mockReturnValue(projectADownload.promise);
    useProjectStore.setState({ projectEpoch: 30, projectPath: "/A.opentake" });
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    try {
      await act(async () =>
        root.render(
          React.createElement(MediaSearchResults, {
            query: "",
            nameMatches: [],
            hasIndexableAssets: true,
          }),
        ),
      );
      await vi.waitFor(() => expect(container.querySelector("button")).not.toBeNull());
      await act(async () => container.querySelector<HTMLButtonElement>("button")!.click());
      await act(async () =>
        useProjectStore.setState({ projectEpoch: 31, projectPath: "/B.opentake" }),
      );
      await vi.waitFor(() => expect(api.searchIndexStatus).toHaveBeenCalledTimes(2));
      await vi.waitFor(() => expect(container.querySelector("button")).not.toBeNull());
      await act(async () => projectADownload.resolve(undefined));
      await vi.waitFor(() => expect(api.searchIndexStatus).toHaveBeenCalledTimes(3));
      await vi.waitFor(() =>
        expect(container.querySelector("button")?.textContent).toContain(t("search.index")),
      );
    } finally {
      await act(async () => root.unmount());
    }
  });

  it("refreshes the remounted current hook when an earlier model download settles", async () => {
    const download = deferred<void>();
    vi.mocked(api.searchIndexStatus)
      .mockReset()
      .mockResolvedValueOnce({ modelInstalled: false, indexable: 1, indexed: 0 })
      .mockResolvedValueOnce({ modelInstalled: false, indexable: 1, indexed: 0 })
      .mockResolvedValueOnce({ modelInstalled: true, indexable: 1, indexed: 0 });
    vi.mocked(api.downloadSearchModel).mockReset().mockReturnValue(download.promise);
    useProjectStore.setState({ projectEpoch: 35, projectPath: "/remount.opentake" });
    const firstContainer = document.createElement("div");
    const secondContainer = document.createElement("div");
    document.body.append(firstContainer, secondContainer);
    const firstRoot = createRoot(firstContainer);
    const secondRoot = createRoot(secondContainer);
    try {
      await act(async () =>
        firstRoot.render(
          React.createElement(MediaSearchResults, {
            query: "",
            nameMatches: [],
            hasIndexableAssets: true,
          }),
        ),
      );
      await vi.waitFor(() => expect(firstContainer.querySelector("button")).not.toBeNull());
      await act(async () => firstContainer.querySelector<HTMLButtonElement>("button")!.click());
      await act(async () => firstRoot.unmount());

      await act(async () =>
        secondRoot.render(
          React.createElement(MediaSearchResults, {
            query: "",
            nameMatches: [],
            hasIndexableAssets: true,
          }),
        ),
      );
      await vi.waitFor(() => expect(api.searchIndexStatus).toHaveBeenCalledTimes(2));
      await act(async () => download.resolve(undefined));
      await vi.waitFor(() => expect(api.searchIndexStatus).toHaveBeenCalledTimes(3));
      await vi.waitFor(() =>
        expect(secondContainer.querySelector("button")?.textContent).toContain(t("search.index")),
      );
    } finally {
      await act(async () => secondRoot.unmount());
    }
  });

  it("refreshes the remounted current hook when an earlier index build settles", async () => {
    const indexBuild = deferred<void>();
    vi.mocked(api.searchIndexStatus)
      .mockReset()
      .mockResolvedValueOnce({ modelInstalled: true, indexable: 1, indexed: 0 })
      .mockResolvedValueOnce({ modelInstalled: true, indexable: 1, indexed: 0 })
      .mockResolvedValueOnce({ modelInstalled: true, indexable: 1, indexed: 1 });
    vi.mocked(api.searchIndexStart).mockReset().mockReturnValue(indexBuild.promise);
    useProjectStore.setState({ projectEpoch: 36, projectPath: "/index-remount.opentake" });
    const firstContainer = document.createElement("div");
    const secondContainer = document.createElement("div");
    document.body.append(firstContainer, secondContainer);
    const firstRoot = createRoot(firstContainer);
    const secondRoot = createRoot(secondContainer);
    try {
      await act(async () =>
        firstRoot.render(
          React.createElement(MediaSearchResults, {
            query: "",
            nameMatches: [],
            hasIndexableAssets: true,
          }),
        ),
      );
      await vi.waitFor(() => expect(firstContainer.querySelector("button")).not.toBeNull());
      await act(async () => firstContainer.querySelector<HTMLButtonElement>("button")!.click());
      expect(api.searchIndexStart).toHaveBeenCalledWith(36, "/index-remount.opentake");
      await act(async () => firstRoot.unmount());

      await act(async () =>
        secondRoot.render(
          React.createElement(MediaSearchResults, {
            query: "",
            nameMatches: [],
            hasIndexableAssets: true,
          }),
        ),
      );
      await vi.waitFor(() => expect(api.searchIndexStatus).toHaveBeenCalledTimes(2));
      await act(async () => indexBuild.resolve(undefined));
      await vi.waitFor(() => expect(api.searchIndexStatus).toHaveBeenCalledTimes(3));
      await vi.waitFor(() => expect(secondContainer.querySelector("button")).toBeNull());
    } finally {
      await act(async () => secondRoot.unmount());
    }
  });

  it("keeps an index failure visible and retries indexing instead of downloading", async () => {
    vi.mocked(api.searchIndexStatus)
      .mockReset()
      .mockResolvedValue({ modelInstalled: true, indexable: 1, indexed: 0 });
    vi.mocked(api.searchIndexStart)
      .mockReset()
      .mockRejectedValueOnce(new Error("index failed"))
      .mockResolvedValueOnce(undefined);
    vi.mocked(api.downloadSearchModel).mockReset().mockResolvedValue(undefined);
    useProjectStore.setState({ projectEpoch: 40, projectPath: "/index.opentake" });
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    let retryVisible = false;
    try {
      await act(async () =>
        root.render(
          React.createElement(MediaSearchResults, {
            query: "",
            nameMatches: [],
            hasIndexableAssets: true,
          }),
        ),
      );
      await vi.waitFor(() => expect(container.querySelector("button")).not.toBeNull());
      await act(async () => container.querySelector<HTMLButtonElement>("button")!.click());
      await act(async () => {
        await Promise.resolve();
        await Promise.resolve();
      });
      const retry = container.querySelector<HTMLButtonElement>("button");
      retryVisible = retry !== null;
      if (retry) await act(async () => retry.click());
    } finally {
      await act(async () => root.unmount());
    }

    expect(retryVisible).toBe(true);
    expect(api.searchIndexStart).toHaveBeenCalledTimes(2);
    expect(api.searchIndexStart).toHaveBeenNthCalledWith(1, 40, "/index.opentake");
    expect(api.searchIndexStart).toHaveBeenNthCalledWith(2, 40, "/index.opentake");
    expect(api.downloadSearchModel).not.toHaveBeenCalled();
  });
});

const searchItem: MediaItem = {
  id: "asset-b",
  name: "asset-b",
  type: "video",
  duration: 1,
  hasAudio: false,
  favorite: false,
};

describe("MediaSearch result context menu", () => {
  it("replaces a hidden stale selection when a search result receives direct focus", async () => {
    useMediaStore.setState({ items: [searchItem], folders: [], importing: false, error: null });
    useEditorUiStore.setState({
      focusedPanel: "media",
      selectedMediaAssetIds: new Set(["asset-a"]),
      previewMediaId: "asset-a",
    });
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () =>
      root.render(
        React.createElement(MediaSearchResults, {
          query: "",
          nameMatches: [searchItem],
          hasIndexableAssets: false,
        }),
      ),
    );
    const card = container.querySelector<HTMLElement>('[title="asset-b"]')!;

    await act(async () => card.focus());
    expect([...useEditorUiStore.getState().selectedMediaAssetIds]).toEqual(["asset-b"]);
    expect(useEditorUiStore.getState().previewMediaId).toBe("asset-b");
    await act(async () => {
      card.dispatchEvent(new KeyboardEvent("keydown", { key: "Delete", bubbles: true }));
      await Promise.resolve();
    });

    expect(editActions.deleteMedia).toHaveBeenCalledOnce();
    expect(editActions.deleteMedia).toHaveBeenCalledWith(["asset-b"], expect.any(Object));
    await act(async () => root.unmount());
  });
  it("opens on right-click and deletes the target without replacing another selection", async () => {
    useMediaStore.setState({ items: [searchItem], folders: [], importing: false, error: null });
    useEditorUiStore.setState({
      focusedPanel: "media",
      selectedMediaAssetIds: new Set(["asset-a"]),
      previewMediaId: "asset-a",
    });
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () =>
      root.render(
        React.createElement(MediaSearchResults, {
          query: "",
          nameMatches: [searchItem],
          hasIndexableAssets: false,
        }),
      ),
    );
    const card = container.querySelector<HTMLElement>('[title="asset-b"]')!;

    await act(async () =>
      card.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true })),
    );
    const menuItem = container.querySelector<HTMLButtonElement>('[role="menuitem"]')!;
    expect(menuItem).not.toBeNull();
    await act(async () => {
      menuItem.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      await Promise.resolve();
    });

    expect(editActions.deleteMedia).toHaveBeenCalledWith(["asset-b"], expect.any(Object));
    expect([...useEditorUiStore.getState().selectedMediaAssetIds]).toEqual(["asset-a"]);
    expect(useEditorUiStore.getState().previewMediaId).toBe("asset-a");
    await act(async () => root.unmount());
  });

  it("opens the same menu from Shift+F10", async () => {
    useMediaStore.setState({ items: [searchItem], folders: [], importing: false, error: null });
    useEditorUiStore.setState({
      focusedPanel: "media",
      selectedMediaAssetIds: new Set([searchItem.id]),
      previewMediaId: searchItem.id,
    });
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () =>
      root.render(
        React.createElement(MediaSearchResults, {
          query: "",
          nameMatches: [searchItem],
          hasIndexableAssets: false,
        }),
      ),
    );
    const card = container.querySelector<HTMLElement>('[title="asset-b"]')!;
    card.focus();

    await act(async () =>
      card.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "F10",
          shiftKey: true,
          bubbles: true,
          cancelable: true,
        }),
      ),
    );

    expect(container.querySelector('[role="menu"]')).not.toBeNull();
    await act(async () => root.unmount());
  });
});
