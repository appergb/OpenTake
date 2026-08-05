import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/libraryApi", () => ({
  libraryCategorize: vi.fn(),
  libraryDelete: vi.fn(),
  libraryImportToProject: vi.fn(),
  libraryList: vi.fn(async () => []),
  libraryRename: vi.fn(),
  libraryUnfavorite: vi.fn(),
}));
vi.mock("./mediaStore", () => ({ refreshMedia: vi.fn() }));

import * as libraryApi from "../lib/libraryApi";
import type { LibraryEntry } from "../lib/libraryApi";
import { t, useI18nStore } from "../i18n";
import { refreshMedia } from "./mediaStore";
import { startLibrarySync, stopLibrarySync, useLibraryStore } from "./libraryStore";
import { useEditorUiStore } from "./uiStore";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

type OwnedMutation =
  | "unfavorite"
  | "categorize"
  | "renameCategory"
  | "remove"
  | "importToProject";

const ownedMutations: OwnedMutation[] = [
  "unfavorite",
  "categorize",
  "renameCategory",
  "remove",
  "importToProject",
];

const currentWarning = {
  kind: "postconditionRollbackFailed" as const,
  postcondition: "current postcondition",
  rollback: "current rollback",
};

function queueMutation(
  mutation: OwnedMutation,
  response: Promise<unknown>,
): void {
  switch (mutation) {
    case "unfavorite":
      vi.mocked(libraryApi.libraryUnfavorite).mockReturnValueOnce(
        response as ReturnType<typeof libraryApi.libraryUnfavorite>,
      );
      return;
    case "categorize":
      vi.mocked(libraryApi.libraryCategorize).mockReturnValueOnce(
        response as ReturnType<typeof libraryApi.libraryCategorize>,
      );
      return;
    case "renameCategory":
      vi.mocked(libraryApi.libraryRename).mockReturnValueOnce(
        response as ReturnType<typeof libraryApi.libraryRename>,
      );
      return;
    case "remove":
      vi.mocked(libraryApi.libraryDelete).mockReturnValueOnce(
        response as ReturnType<typeof libraryApi.libraryDelete>,
      );
      return;
    case "importToProject":
      vi.mocked(libraryApi.libraryImportToProject).mockReturnValueOnce(
        response as ReturnType<typeof libraryApi.libraryImportToProject>,
      );
  }
}

function rejectNextMutation(mutation: OwnedMutation, error: Error): void {
  switch (mutation) {
    case "unfavorite":
      vi.mocked(libraryApi.libraryUnfavorite).mockRejectedValueOnce(error);
      return;
    case "categorize":
      vi.mocked(libraryApi.libraryCategorize).mockRejectedValueOnce(error);
      return;
    case "renameCategory":
      vi.mocked(libraryApi.libraryRename).mockRejectedValueOnce(error);
      return;
    case "remove":
      vi.mocked(libraryApi.libraryDelete).mockRejectedValueOnce(error);
      return;
    case "importToProject":
      vi.mocked(libraryApi.libraryImportToProject).mockRejectedValueOnce(error);
  }
}

function invokeMutation(mutation: OwnedMutation): Promise<unknown> {
  switch (mutation) {
    case "unfavorite":
      return useLibraryStore.getState().unfavorite("library-old");
    case "categorize":
      return useLibraryStore
        .getState()
        .categorize("library-old", "old-category");
    case "renameCategory":
      return useLibraryStore
        .getState()
        .renameCategory("old-category", "renamed-category");
    case "remove":
      return useLibraryStore.getState().remove("library-old");
    case "importToProject":
      return useLibraryStore.getState().importToProject("library-old");
  }
}

function mutationSuccess(mutation: OwnedMutation): unknown {
  switch (mutation) {
    case "unfavorite":
    case "remove":
      return true;
    case "categorize":
      return {
        id: "library-old",
        type: "video",
        category: "old-category",
        favoritedAt: 1,
        source: "/old.mp4",
      };
    case "renameCategory":
      return 1;
    case "importToProject":
      return {
        id: "asset-old",
        name: "old.mp4",
        path: "/project/media/old.mp4",
        warning: {
          kind: "postconditionRollbackFailed" as const,
          postcondition: "stale postcondition",
          rollback: "stale rollback",
        },
      };
  }
}

beforeEach(() => {
  stopLibrarySync();
  vi.mocked(libraryApi.libraryCategorize).mockReset().mockResolvedValue({
    id: "library-current",
    type: "video",
    favoritedAt: 2,
    source: "/current.mp4",
  });
  vi.mocked(libraryApi.libraryDelete).mockReset().mockResolvedValue(true);
  vi.mocked(libraryApi.libraryImportToProject).mockReset();
  vi.mocked(libraryApi.libraryList).mockReset().mockResolvedValue([]);
  vi.mocked(libraryApi.libraryRename).mockReset().mockResolvedValue(1);
  vi.mocked(libraryApi.libraryUnfavorite).mockReset().mockResolvedValue(true);
  vi.mocked(refreshMedia).mockReset().mockResolvedValue(true);
  useI18nStore.setState({ locale: "zh-CN" });
  useLibraryStore.setState({
    entries: [],
    loading: false,
    error: null,
    lastImportWarning: null,
  });
  useEditorUiStore.setState({ toast: null });
});

afterEach(() => {
  stopLibrarySync();
});

describe("library import warnings", () => {
  it("preserves a committed rollback warning and surfaces it through the existing toast", async () => {
    const warning = {
      kind: "postconditionRollbackFailed" as const,
      postcondition: "project import leaf identity changed during commit",
      rollback: "injected manifest rollback failure",
    };
    vi.mocked(libraryApi.libraryImportToProject).mockResolvedValue({
      id: "asset-1",
      name: "clip.mp4",
      path: "/project/media/clip.mp4",
      warning,
    });

    const imported = await useLibraryStore.getState().importToProject("library-1");

    expect(imported).toMatchObject({ id: "asset-1", name: "clip.mp4" });
    expect(useLibraryStore.getState().lastImportWarning).toEqual(warning);
    expect(refreshMedia).toHaveBeenCalledOnce();
    expect(useEditorUiStore.getState().toast?.message).toBe(
      t("library.importCommittedWarning"),
    );
  });

  it("returns the imported asset identity and emits no warning toast", async () => {
    vi.mocked(libraryApi.libraryImportToProject).mockResolvedValue({
      id: "asset-2",
      name: "clean.mp4",
      path: "/project/media/clean.mp4",
    });

    const imported = await useLibraryStore.getState().importToProject("library-2");

    expect(imported).toMatchObject({ id: "asset-2", name: "clean.mp4" });
    expect(useLibraryStore.getState().lastImportWarning).toBeNull();
    expect(useEditorUiStore.getState().toast).toBeNull();
  });

  it("does not reverse a committed warning when the project mirror refresh fails", async () => {
    const warning = {
      kind: "postconditionRollbackFailed" as const,
      postcondition: "postcondition failed",
      rollback: "rollback failed",
    };
    vi.mocked(libraryApi.libraryImportToProject).mockResolvedValue({
      id: "asset-3",
      name: "committed.mp4",
      path: "/project/media/committed.mp4",
      warning,
    });
    vi.mocked(refreshMedia).mockRejectedValueOnce(new Error("mirror refresh failed"));

    const imported = await useLibraryStore.getState().importToProject("library-3");

    expect(imported).toMatchObject({ id: "asset-3", name: "committed.mp4" });
    expect(useLibraryStore.getState().lastImportWarning).toEqual(warning);
    expect(useLibraryStore.getState().error).toBe("mirror refresh failed");
    expect(useEditorUiStore.getState().toast?.message).toBe(
      t("library.importCommittedWarning"),
    );
  });
});

describe("library startup refresh", () => {
  it("retries after the first swallowed refresh failure", async () => {
    vi.mocked(libraryApi.libraryList)
      .mockRejectedValueOnce(new Error("library unavailable"))
      .mockResolvedValueOnce([]);

    await startLibrarySync();
    expect(useLibraryStore.getState().error).toBe("library unavailable");

    await startLibrarySync();
    expect(libraryApi.libraryList).toHaveBeenCalledTimes(2);
    expect(useLibraryStore.getState().error).toBeNull();
  });

  it("can start again after an explicit lifecycle stop", async () => {
    await startLibrarySync();
    stopLibrarySync();
    await startLibrarySync();

    expect(libraryApi.libraryList).toHaveBeenCalledTimes(2);
  });

  it("ignores an old lifecycle success that resolves after the replacement load", async () => {
    const stale = deferred<Awaited<ReturnType<typeof libraryApi.libraryList>>>();
    const currentEntries = [
      { id: "current", type: "video", favoritedAt: 2, source: "/current.mp4" },
    ];
    vi.mocked(libraryApi.libraryList)
      .mockReturnValueOnce(stale.promise)
      .mockResolvedValueOnce(currentEntries);

    const oldStart = startLibrarySync();
    await vi.waitFor(() => expect(libraryApi.libraryList).toHaveBeenCalledTimes(1));
    stopLibrarySync();
    await startLibrarySync();

    stale.resolve([
      { id: "stale", type: "video", favoritedAt: 1, source: "/stale.mp4" },
    ]);
    await oldStart;

    expect(useLibraryStore.getState().entries).toEqual(currentEntries);
    expect(useLibraryStore.getState().error).toBeNull();
  });

  it("ignores an old lifecycle rejection that arrives after the replacement load", async () => {
    const stale = deferred<Awaited<ReturnType<typeof libraryApi.libraryList>>>();
    const currentEntries = [
      { id: "current", type: "audio", favoritedAt: 2, source: "/current.wav" },
    ];
    vi.mocked(libraryApi.libraryList)
      .mockReturnValueOnce(stale.promise)
      .mockResolvedValueOnce(currentEntries);

    const oldStart = startLibrarySync();
    await vi.waitFor(() => expect(libraryApi.libraryList).toHaveBeenCalledTimes(1));
    stopLibrarySync();
    await startLibrarySync();

    stale.reject(new Error("stale library failure"));
    await oldStart;

    expect(useLibraryStore.getState().entries).toEqual(currentEntries);
    expect(useLibraryStore.getState().error).toBeNull();
  });
});

describe("library mutation lifecycle ownership", () => {
  it.each(ownedMutations)(
    "ignores an old %s rejection after stop, re-entry, and the current load",
    async (mutation) => {
      const stale = deferred<unknown>();
      queueMutation(mutation, stale.promise);
      const oldMutation = invokeMutation(mutation);

      stopLibrarySync();
      await startLibrarySync();
      useLibraryStore.setState({
        error: "current lifecycle error",
        lastImportWarning: currentWarning,
      });

      stale.reject(new Error(`stale ${mutation} failure`));
      await oldMutation;

      expect(useLibraryStore.getState().error).toBe("current lifecycle error");
      expect(useLibraryStore.getState().lastImportWarning).toEqual(currentWarning);
    },
  );

  it.each(ownedMutations)(
    "ignores an old %s success after stop, re-entry, and the current load",
    async (mutation) => {
      const stale = deferred<unknown>();
      queueMutation(mutation, stale.promise);
      const oldMutation = invokeMutation(mutation);

      stopLibrarySync();
      await startLibrarySync();
      const currentListCalls = vi.mocked(libraryApi.libraryList).mock.calls.length;
      vi.mocked(refreshMedia).mockClear();
      useLibraryStore.setState({
        error: "current lifecycle error",
        lastImportWarning: currentWarning,
      });
      useEditorUiStore.getState().pushToast("current lifecycle toast");

      stale.resolve(mutationSuccess(mutation));
      await oldMutation;

      expect(useLibraryStore.getState().error).toBe("current lifecycle error");
      expect(useLibraryStore.getState().lastImportWarning).toEqual(currentWarning);
      expect(libraryApi.libraryList).toHaveBeenCalledTimes(
        currentListCalls + (mutation === "importToProject" ? 0 : 1),
      );
      expect(refreshMedia).not.toHaveBeenCalled();
      expect(useEditorUiStore.getState().toast?.message).toBe(
        "current lifecycle toast",
      );
    },
  );

  it.each(ownedMutations)(
    "keeps the newer %s failure when an older same-action success settles last",
    async (mutation) => {
      const stale = deferred<unknown>();
      queueMutation(mutation, stale.promise);
      const oldMutation = invokeMutation(mutation);

      rejectNextMutation(mutation, new Error(`current ${mutation} failure`));
      await invokeMutation(mutation);
      expect(useLibraryStore.getState().error).toBe(
        `current ${mutation} failure`,
      );

      stale.resolve(mutationSuccess(mutation));
      await oldMutation;

      expect(useLibraryStore.getState().error).toBe(
        `current ${mutation} failure`,
      );
    },
  );

  it("reconciles two different categorize commits that finish in reverse order", async () => {
    const first = deferred<Awaited<ReturnType<typeof libraryApi.libraryCategorize>>>();
    const second = deferred<Awaited<ReturnType<typeof libraryApi.libraryCategorize>>>();
    vi.mocked(libraryApi.libraryCategorize)
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    let committed: LibraryEntry[] = [
      { id: "base", type: "video", favoritedAt: 3, source: "/base.mp4" },
    ];
    vi.mocked(libraryApi.libraryList).mockImplementation(async () => [...committed]);

    const older = useLibraryStore.getState().categorize("asset-a", "A");
    const newer = useLibraryStore.getState().categorize("asset-b", "B");
    committed = [
      ...committed,
      { id: "asset-b", type: "video", category: "B", favoritedAt: 2, source: "/b.mp4" },
    ];
    second.resolve(committed[1]!);
    await newer;
    expect(useLibraryStore.getState().entries.map(({ id }) => id)).toEqual([
      "base",
      "asset-b",
    ]);

    committed = [
      ...committed,
      { id: "asset-a", type: "video", category: "A", favoritedAt: 1, source: "/a.mp4" },
    ];
    first.resolve(committed[2]!);
    await older;

    expect(useLibraryStore.getState().entries.map(({ id }) => id)).toEqual([
      "base",
      "asset-b",
      "asset-a",
    ]);
    expect(libraryApi.libraryList).toHaveBeenCalledTimes(2);
  });

  it("releases an older refresh loading lease when a committed removal reconciliation wins", async () => {
    const staleRefresh = deferred<LibraryEntry[]>();
    vi.mocked(libraryApi.libraryList)
      .mockReturnValueOnce(staleRefresh.promise)
      .mockResolvedValueOnce([]);
    useLibraryStore.setState({
      entries: [
        {
          id: "last-entry",
          type: "video",
          favoritedAt: 1,
          source: "/last.mp4",
        },
      ],
    });

    const refresh = useLibraryStore.getState().refresh();
    expect(useLibraryStore.getState().loading).toBe(true);

    await useLibraryStore.getState().remove("last-entry");
    expect(useLibraryStore.getState()).toMatchObject({
      entries: [],
      loading: false,
    });

    staleRefresh.resolve([
      {
        id: "stale-entry",
        type: "video",
        favoritedAt: 0,
        source: "/stale.mp4",
      },
    ]);
    await refresh;

    expect(useLibraryStore.getState()).toMatchObject({
      entries: [],
      loading: false,
    });
  });
});
