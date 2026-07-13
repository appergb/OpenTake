import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/libraryApi", () => ({
  libraryImportToProject: vi.fn(),
  libraryList: vi.fn(async () => []),
}));
vi.mock("./mediaStore", () => ({ refreshMedia: vi.fn() }));

import * as libraryApi from "../lib/libraryApi";
import { t, useI18nStore } from "../i18n";
import { refreshMedia } from "./mediaStore";
import { useLibraryStore } from "./libraryStore";
import { useEditorUiStore } from "./uiStore";

describe("library import warnings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useI18nStore.setState({ locale: "zh-CN" });
    useLibraryStore.setState({ error: null, lastImportWarning: null });
    useEditorUiStore.setState({ toast: null });
  });

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

    expect(imported).toBe("clip.mp4");
    expect(useLibraryStore.getState().lastImportWarning).toEqual(warning);
    expect(refreshMedia).toHaveBeenCalledOnce();
    expect(useEditorUiStore.getState().toast?.message).toBe(
      t("library.importCommittedWarning"),
    );
  });

  it("keeps the legacy successful return and emits no warning toast", async () => {
    vi.mocked(libraryApi.libraryImportToProject).mockResolvedValue({
      id: "asset-2",
      name: "clean.mp4",
      path: "/project/media/clean.mp4",
    });

    const imported = await useLibraryStore.getState().importToProject("library-2");

    expect(imported).toBe("clean.mp4");
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

    expect(imported).toBe("committed.mp4");
    expect(useLibraryStore.getState().lastImportWarning).toEqual(warning);
    expect(useLibraryStore.getState().error).toBe("mirror refresh failed");
    expect(useEditorUiStore.getState().toast?.message).toBe(
      t("library.importCommittedWarning"),
    );
  });
});
