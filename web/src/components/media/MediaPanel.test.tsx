// @vitest-environment happy-dom

import { readFileSync } from "node:fs";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { MediaItem, MediaList } from "../../lib/types";
import {
  applyMediaErrorForProject,
  applyMediaListForProject,
  useMediaStore,
} from "../../store/mediaStore";
import { useProjectStore } from "../../store/projectStore";
import { derivedResourceScheduler } from "../../lib/derivedResourceScheduler";
import { textContrastRatio } from "../../../test/contrast";

const dialogMocks = vi.hoisted(() => ({
  openDialog: vi.fn(),
  saveDialog: vi.fn(),
  save: vi.fn(),
}));

vi.mock("../../lib/api", () => ({
  isTauri: false,
  getWaveform: vi.fn().mockResolvedValue(null),
  preloadMedia: vi.fn().mockResolvedValue("cached"),
  generateThumbnail: vi.fn().mockResolvedValue(null),
  toggleFavorite: vi.fn(),
  extractAudio: vi.fn(),
  scriptToVideo: vi.fn(),
  generateAvatar: vi.fn(),
  cloneVoice: vi.fn(),
  cancelAdvancedWorkflow: vi.fn(),
  searchIndexStatus: vi.fn().mockResolvedValue({ modelInstalled: true, indexable: 0, indexed: 0 }),
  searchIndexStart: vi.fn().mockResolvedValue(undefined),
  downloadSearchModel: vi.fn().mockResolvedValue(undefined),
  onSearchModelProgress: vi.fn().mockResolvedValue(() => {}),
  onSearchIndexProgress: vi.fn().mockResolvedValue(() => {}),
  searchQuery: vi.fn().mockResolvedValue({ moments: [], spoken: [], files: [] }),
}));

vi.mock("../../lib/asset", () => ({
  assetUrl: (path: string | null | undefined) => (path ? `asset://${path}` : null),
}));
vi.mock("../../lib/dialog", () => ({
  openDialog: dialogMocks.openDialog,
  saveDialog: dialogMocks.saveDialog,
}));
vi.mock("../../store/editActions", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../store/editActions")>();
  return {
    ...actual,
    addMediaToTimeline: vi.fn(),
    deleteFolder: vi.fn(),
    deleteMedia: vi.fn(),
  };
});

import { useEditorUiStore } from "../../store/uiStore";
import * as api from "../../lib/api";
import * as editActions from "../../store/editActions";
import {
  deleteSelectedFolders,
  deleteSelectedMediaAssets,
} from "../../store/mediaDeleteActions";
import { useKeyboardShortcuts } from "../../hooks/useKeyboardShortcuts";
import {
  AudioWaveform,
  FolderTile,
  MediaCard,
  MediaFavoriteButton,
  MediaPanel,
  filterMediaByType,
  sortMediaItems,
} from "./MediaPanel";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const originalRequestMediaPreviewToggle =
  useEditorUiStore.getState().requestMediaPreviewToggle;

afterEach(() => {
  document.body.replaceChildren();
  vi.clearAllMocks();
  useMediaStore.setState({ items: [], folders: [], importing: false, error: null });
  useProjectStore.setState({ projectEpoch: 0, projectPath: null, timelineVersion: 0 });
  useEditorUiStore.setState({
    view: "home",
    settingsOpen: false,
    exportDialogOpen: false,
    saveAsProgress: null,
    projectSettingsPrompt: null,
    pendingSwapClipId: null,
    selectedMediaAssetIds: new Set(),
    selectedFolderIds: new Set(),
    previewMediaId: null,
    focusedPanel: null,
    requestMediaPreviewToggle: originalRequestMediaPreviewToggle,
  });
  dialogMocks.openDialog.mockResolvedValue(null);
  dialogMocks.saveDialog.mockResolvedValue(dialogMocks.save);
  dialogMocks.save.mockResolvedValue("/exports/audio.m4a");
});

function mediaItem(id: string): MediaItem {
  return {
    id,
    name: id,
    type: "video",
    duration: 1,
    hasAudio: false,
    favorite: false,
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function MediaKeyboardHarness() {
  useKeyboardShortcuts();
  return <MediaPanel />;
}

describe("AudioWaveform", () => {
  const fallback = <span data-testid="audio-fallback">fallback</span>;

  it("renders waveform bars when normalized buckets are available", () => {
    const html = renderToStaticMarkup(
      <AudioWaveform mediaRef="audio-1" projectEpoch={1} fallback={fallback} bucketsOverride={[0, 0.25, 0.5, 0.75, 1]} />,
    );

    expect(html).toContain('data-testid="audio-waveform"');
    expect(html).not.toContain('data-testid="audio-fallback"');
  });

  it("renders the fallback when waveform loading resolves to null", () => {
    const html = renderToStaticMarkup(
      <AudioWaveform mediaRef="audio-1" projectEpoch={1} fallback={fallback} bucketsOverride={null} />,
    );

    expect(html).not.toContain('data-testid="audio-waveform"');
    expect(html).toContain('data-testid="audio-fallback"');
  });

  it("renders the fallback when waveform loading resolves to an empty array", () => {
    const html = renderToStaticMarkup(
      <AudioWaveform mediaRef="audio-1" projectEpoch={1} fallback={fallback} bucketsOverride={[]} />,
    );

    expect(html).not.toContain('data-testid="audio-waveform"');
    expect(html).toContain('data-testid="audio-fallback"');
  });

  it("shares the derived-resource active limit and cancels queued waveform work on unmount", async () => {
    const jobs = Array.from({ length: 4 }, () => deferred<number[] | null>());
    let nextJob = 0;
    vi.mocked(api.getWaveform).mockImplementation(() => jobs[nextJob++]!.promise);
    useProjectStore.setState({ projectEpoch: 90, projectPath: "/waveforms.opentake" });
    derivedResourceScheduler.activateProject(90);
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    let mounted = true;
    try {
      await act(async () =>
        root.render(
          <>
            {Array.from({ length: 6 }, (_, index) => (
              <AudioWaveform
                key={index}
                mediaRef={`audio-${index}`}
                projectEpoch={90}
                sourceKey={`source-${index}`}
                fallback={fallback}
              />
            ))}
          </>,
        ),
      );

      expect(api.getWaveform).toHaveBeenCalledTimes(4);
      expect(derivedResourceScheduler.stats()).toEqual({
        active: 4,
        pending: 2,
        inFlight: 6,
        projectEpoch: 90,
      });

      await act(async () => root.unmount());
      mounted = false;
      expect(derivedResourceScheduler.stats()).toEqual({
        active: 4,
        pending: 0,
        inFlight: 0,
        projectEpoch: 90,
      });
    } finally {
      if (mounted) await act(async () => root.unmount());
      for (const job of jobs) job.resolve(null);
      await act(async () => {
        await Promise.resolve();
        await Promise.resolve();
      });
      vi.mocked(api.getWaveform).mockResolvedValue(null);
    }
    expect(derivedResourceScheduler.stats().active).toBe(0);
  });

  it("does not publish a waveform result from the previous project epoch", async () => {
    const stale = deferred<number[] | null>();
    const current = deferred<number[] | null>();
    vi.mocked(api.getWaveform)
      .mockReset()
      .mockReturnValueOnce(stale.promise)
      .mockReturnValueOnce(current.promise);
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    let mounted = true;
    try {
      await act(async () =>
        root.render(
          <AudioWaveform
            mediaRef="audio"
            projectEpoch={100}
            sourceKey="old-source"
            fallback={fallback}
          />,
        ),
      );
      await act(async () =>
        root.render(
          <AudioWaveform
            mediaRef="audio"
            projectEpoch={101}
            sourceKey="new-source"
            fallback={fallback}
          />,
        ),
      );
      expect(api.getWaveform).toHaveBeenCalledTimes(2);

      await act(async () => stale.resolve([0, 0, 0]));
      expect(container.querySelector('[data-testid="audio-waveform"]')).toBeNull();
      expect(container.querySelector('[data-testid="audio-fallback"]')).not.toBeNull();

      await act(async () => current.resolve([0, 0.5, 1]));
      expect(container.querySelector('[data-testid="audio-waveform"]')).not.toBeNull();
    } finally {
      if (mounted) {
        await act(async () => root.unmount());
        mounted = false;
      }
      stale.resolve(null);
      current.resolve(null);
      vi.mocked(api.getWaveform).mockResolvedValue(null);
    }
  });
});

describe("MediaCard derived-resource lifecycle", () => {
  it("ignores a disconnected thumbnail observer callback from an older project", async () => {
    const callbacks: IntersectionObserverCallback[] = [];
    class FakeIntersectionObserver {
      readonly root = null;
      readonly rootMargin = "0px";
      readonly thresholds = [0];

      constructor(callback: IntersectionObserverCallback) {
        callbacks.push(callback);
      }

      disconnect() {}
      observe() {}
      takeRecords(): IntersectionObserverEntry[] { return []; }
      unobserve() {}
    }
    vi.stubGlobal("IntersectionObserver", FakeIntersectionObserver);
    useProjectStore.setState({ projectEpoch: 400 });
    derivedResourceScheduler.activateProject(400);
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    try {
      await act(async () => root.render(<MediaCard item={mediaItem("old-project-video")} />));
      expect(callbacks.length).toBeGreaterThan(0);
      await act(async () => root.unmount());

      useProjectStore.setState({ projectEpoch: 401 });
      derivedResourceScheduler.activateProject(401);
      await act(async () => {
        for (const callback of callbacks) {
          callback(
            [{ isIntersecting: true } as IntersectionObserverEntry],
            {} as IntersectionObserver,
          );
        }
      });

      expect(derivedResourceScheduler.stats().projectEpoch).toBe(401);
      expect(api.generateThumbnail).not.toHaveBeenCalled();
    } finally {
      vi.unstubAllGlobals();
    }
  });
});

describe("MediaFavoriteButton", () => {
  it("disables while pending and keeps the rendered star unchanged after rejection", async () => {
    useProjectStore.setState({ projectEpoch: 7, projectPath: "/project-7.opentake" });
    let rejectToggle: (reason: unknown) => void = () => undefined;
    const pendingToggle = new Promise<never>((_resolve, reject) => {
      rejectToggle = reject;
    });
    const performToggle = vi.fn(() => pendingToggle);
    const onSuccess = vi.fn();

    function Harness() {
      const [feedback, setFeedback] = React.useState<string | null>(null);
      return (
        <>
          <MediaFavoriteButton
            assetId="asset-1"
            favorite
            title="Unfavorite"
            onSuccess={onSuccess}
            onError={setFeedback}
            performToggle={performToggle}
          />
          {feedback && <span role="alert">{feedback}</span>}
        </>
      );
    }

    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () => root.render(<Harness />));
    const button = container.querySelector<HTMLButtonElement>("button")!;

    await act(async () => button.dispatchEvent(new MouseEvent("click", { bubbles: true })));

    expect(performToggle).toHaveBeenCalledWith("asset-1", false, {
      projectEpoch: 7,
      projectPath: "/project-7.opentake",
    });
    expect(button.disabled).toBe(true);
    expect(button.getAttribute("aria-busy")).toBe("true");
    expect(button.getAttribute("aria-pressed")).toBe("true");

    await act(async () => rejectToggle(new Error("backend rejected")));

    expect(button.disabled).toBe(false);
    expect(button.getAttribute("aria-busy")).toBe("false");
    expect(button.getAttribute("aria-pressed")).toBe("true");
    expect(container.querySelector('[role="alert"]')?.textContent).toContain("backend rejected");
    expect(onSuccess).not.toHaveBeenCalled();
    await act(async () => root.unmount());
  });

  it("ignores a project A resolution after project B replaces the media mirror", async () => {
    const toggle = deferred<MediaList>();
    useProjectStore.setState({ projectEpoch: 1, projectPath: "/A.opentake" });
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () =>
      root.render(
        <MediaFavoriteButton
          assetId="asset-a"
          favorite={false}
          title="Favorite"
          performToggle={() => toggle.promise}
          onSuccess={(media, project) => {
            applyMediaListForProject(project, media);
          }}
          onError={(message, project) => {
            applyMediaErrorForProject(project, message);
          }}
        />,
      ),
    );
    const button = container.querySelector<HTMLButtonElement>("button")!;
    await act(async () => button.dispatchEvent(new MouseEvent("click", { bubbles: true })));
    const projectBItems = [mediaItem("asset-b")];
    const projectBFolders = [{ id: "folder-b", name: "B" }];
    useProjectStore.setState({ projectEpoch: 2, projectPath: "/B.opentake" });
    useMediaStore.setState({
      items: projectBItems,
      folders: projectBFolders,
      error: "B error",
    });

    await act(async () =>
      toggle.resolve({ items: [mediaItem("late-a")], folders: [{ id: "folder-a", name: "A" }] }),
    );

    expect(useMediaStore.getState().items).toEqual(projectBItems);
    expect(useMediaStore.getState().folders).toEqual(projectBFolders);
    expect(useMediaStore.getState().error).toBe("B error");
    await act(async () => root.unmount());
  });

  it("ignores a project A rejection after project B replaces the media mirror", async () => {
    const toggle = deferred<MediaList>();
    useProjectStore.setState({ projectEpoch: 10, projectPath: "/A.opentake" });
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () =>
      root.render(
        <MediaFavoriteButton
          assetId="asset-a"
          favorite
          title="Unfavorite"
          performToggle={() => toggle.promise}
          onSuccess={(media, project) => {
            applyMediaListForProject(project, media);
          }}
          onError={(message, project) => {
            applyMediaErrorForProject(project, message);
          }}
        />,
      ),
    );
    const button = container.querySelector<HTMLButtonElement>("button")!;
    await act(async () => button.dispatchEvent(new MouseEvent("click", { bubbles: true })));
    const projectBItems = [mediaItem("asset-b")];
    const projectBFolders = [{ id: "folder-b", name: "B" }];
    useProjectStore.setState({ projectEpoch: 11, projectPath: "/B.opentake" });
    useMediaStore.setState({ items: projectBItems, folders: projectBFolders, error: null });

    await act(async () => toggle.reject(new Error("late A rejection")));

    expect(useMediaStore.getState().items).toEqual(projectBItems);
    expect(useMediaStore.getState().folders).toEqual(projectBFolders);
    expect(useMediaStore.getState().error).toBeNull();
    await act(async () => root.unmount());
  });
});

describe("media grid interaction consistency", () => {
  it("consumes a rejected double-click placement and shows a visible toast", async () => {
    const item = mediaItem("placement-fails");
    const failure = new Error("timeline queue rejected");
    vi.mocked(editActions.addMediaToTimeline).mockRejectedValueOnce(failure);
    useEditorUiStore.setState({ toast: null });
    const unhandled = vi.fn();
    const onUnhandled = (event: PromiseRejectionEvent) => unhandled(event.reason);
    window.addEventListener("unhandledrejection", onUnhandled);
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);

    try {
      await act(async () => root.render(<MediaCard item={item} />));
      const card = container.querySelector<HTMLElement>('[role="gridcell"]')!;
      await act(async () => {
        card.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
        await Promise.resolve();
        await Promise.resolve();
      });

      expect(editActions.addMediaToTimeline).toHaveBeenCalledWith(item);
      expect(useEditorUiStore.getState().toast?.message).toContain("timeline queue rejected");
      expect(unhandled).not.toHaveBeenCalled();
    } finally {
      window.removeEventListener("unhandledrejection", onUnhandled);
      await act(async () => root.unmount());
    }
  });

  it("gives generation, retry, and relink actions a real 24px pointer target", async () => {
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);

    await act(async () =>
      root.render(
        <MediaCard
          item={{
            ...mediaItem("generating"),
            generationStatus: "generating",
            generationInput: {
              prompt: "mountains",
              model: "test",
              duration: 1,
              aspectRatio: "16:9",
              provider: "local",
              jobId: "job-1",
            },
          }}
        />,
      ),
    );
    const cancel = [...container.querySelectorAll<HTMLButtonElement>("button")].find(
      (button) => button.textContent === "取消",
    );
    expect(cancel?.style.minWidth).toBe("24px");
    expect(cancel?.style.minHeight).toBe("24px");

    await act(async () =>
      root.render(
        <MediaCard
          item={{
            ...mediaItem("failed"),
            generationStatus: "failed",
            generationInput: {
              prompt: "mountains",
              model: "test",
              duration: 1,
              aspectRatio: "16:9",
              provider: "local",
              jobId: "job-2",
            },
          }}
        />,
      ),
    );
    const retry = [...container.querySelectorAll<HTMLButtonElement>("button")].find(
      (button) => button.textContent === "重试",
    );
    expect(retry?.style.minWidth).toBe("24px");
    expect(retry?.style.minHeight).toBe("24px");

    await act(async () =>
      root.render(<MediaCard item={{ ...mediaItem("missing"), missing: true }} />),
    );
    const relink = [...container.querySelectorAll<HTMLButtonElement>("button")].find(
      (button) => button.textContent === "重新链接",
    );
    expect(relink?.style.minWidth).toBe("24px");
    expect(relink?.style.minHeight).toBe("24px");

    await act(async () => root.unmount());
  });

  it("keeps extract audio keyboard-reachable from zero hover without adding idle visual noise", async () => {
    dialogMocks.saveDialog.mockResolvedValue(dialogMocks.save);
    dialogMocks.save.mockResolvedValue("/exports/audio.m4a");
    vi.mocked(api.extractAudio).mockResolvedValue("/exports/audio.m4a");
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () =>
      root.render(<MediaCard item={{ ...mediaItem("talking-head"), hasAudio: true }} />),
    );

    const extract = container.querySelector<HTMLButtonElement>(
      'button[aria-label="提取音频"]',
    );
    expect(extract).not.toBeNull();
    expect(extract?.tabIndex).toBeGreaterThanOrEqual(0);
    expect(extract?.style.opacity).toBe("0");

    await act(async () => extract?.focus());
    expect(document.activeElement).toBe(extract);
    expect(extract?.style.opacity).toBe("1");

    await act(async () => {
      extract?.click();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(api.extractAudio).toHaveBeenCalledWith("talking-head", "/exports/audio.m4a");
    await act(async () => root.unmount());
  });

  it("keeps the normal informational empty state at WCAG AA text contrast", () => {
    const source = readFileSync(
      "src/components/media/MediaPanel.tsx",
      "utf8",
    );
    const emptyState = source.match(/function EmptyState[\s\S]*?const TYPE_ICON/)?.[0];
    const color = emptyState?.match(/\bcolor:\s*"([^"]+)"/)?.[1];

    expect(emptyState).toBeDefined();
    expect(color).toBeDefined();
    expect(textContrastRatio(color!, "var(--bg-surface)")).toBeGreaterThanOrEqual(4.5);
  });

  it("keeps mouse preview, command selection, and visible card state synchronized", async () => {
    const item = mediaItem("asset-1");
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () => root.render(<MediaCard item={item} />));
    const card = container.querySelector<HTMLElement>('[role="gridcell"]')!;

    await act(async () => card.dispatchEvent(new MouseEvent("click", { bubbles: true })));

    const ui = useEditorUiStore.getState();
    expect(ui.previewMediaId).toBe(item.id);
    expect([...ui.selectedMediaAssetIds]).toEqual([item.id]);
    expect(card.getAttribute("aria-selected")).toBe("true");
    expect(card.hasAttribute("aria-pressed")).toBe(false);
    expect(card.tabIndex).toBe(0);
    await act(async () => root.unmount());
  });

  it("routes Delete and the context-menu action to the mouse-selected asset", async () => {
    const item = mediaItem("asset-1");
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () => root.render(<MediaCard item={item} />));
    const card = container.querySelector<HTMLElement>('[role="gridcell"]')!;

    await act(async () => {
      card.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      card.dispatchEvent(new KeyboardEvent("keydown", { key: "Delete", bubbles: true }));
      await Promise.resolve();
    });
    expect(editActions.deleteMedia).toHaveBeenLastCalledWith([item.id], expect.any(Object));
    expect(useEditorUiStore.getState().previewMediaId).toBeNull();

    await act(async () => {
      card.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      card.dispatchEvent(
        new MouseEvent("contextmenu", { bubbles: true, clientX: 10, clientY: 20 }),
      );
    });
    const menuItem = container.querySelector<HTMLButtonElement>('[role="menuitem"]')!;
    expect(menuItem).not.toBeNull();
    await act(async () => {
      menuItem.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      await Promise.resolve();
    });
    expect(editActions.deleteMedia).toHaveBeenLastCalledWith([item.id], expect.any(Object));
    expect(editActions.deleteMedia).toHaveBeenCalledTimes(2);
    await act(async () => root.unmount());
  });

  it("replaces a hidden stale selection when a media tile receives direct focus", async () => {
    useEditorUiStore.setState({
      selectedMediaAssetIds: new Set(["asset-a"]),
      previewMediaId: "asset-a",
    });
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () => root.render(<MediaCard item={mediaItem("asset-b")} />));
    const card = container.querySelector<HTMLElement>('[data-media-asset-id="asset-b"]')!;

    await act(async () => card.focus());
    expect([...useEditorUiStore.getState().selectedMediaAssetIds]).toEqual(["asset-b"]);
    expect(useEditorUiStore.getState().previewMediaId).toBe("asset-b");
    await act(async () => {
      card.dispatchEvent(new KeyboardEvent("keydown", { key: "Delete", bubbles: true }));
      await Promise.resolve();
    });

    expect([...useEditorUiStore.getState().selectedMediaAssetIds]).toEqual([]);
    expect(editActions.deleteMedia).toHaveBeenCalledOnce();
    expect(editActions.deleteMedia).toHaveBeenCalledWith(["asset-b"], expect.any(Object));
    await act(async () => root.unmount());
  });

  it.each(["generating", "downloading"] as const)(
    "selects a %s tile on direct focus without previewing it, then deletes only that tile",
    async (generationStatus) => {
      useEditorUiStore.setState({
        focusedPanel: "timeline",
        selectedMediaAssetIds: new Set(["asset-a"]),
        selectedFolderIds: new Set(["stale-folder"]),
        previewMediaId: "asset-a",
      });
      const generating = {
        ...mediaItem("asset-b"),
        generationStatus,
      };
      const container = document.createElement("div");
      document.body.append(container);
      const root = createRoot(container);
      await act(async () => root.render(<MediaCard item={generating} />));
      const card = container.querySelector<HTMLElement>('[data-media-asset-id="asset-b"]')!;

      await act(async () => card.focus());

      expect(useEditorUiStore.getState().focusedPanel).toBe("media");
      expect([...useEditorUiStore.getState().selectedMediaAssetIds]).toEqual(["asset-b"]);
      expect([...useEditorUiStore.getState().selectedFolderIds]).toEqual([]);
      expect(useEditorUiStore.getState().previewMediaId).toBe("asset-a");

      await act(async () => {
        card.dispatchEvent(new KeyboardEvent("keydown", { key: "Delete", bubbles: true }));
        await Promise.resolve();
      });

      expect(editActions.deleteMedia).toHaveBeenCalledOnce();
      expect(editActions.deleteMedia).toHaveBeenCalledWith(["asset-b"], expect.any(Object));
      await act(async () => root.unmount());
    },
  );

  it("does not replace selection when focus bubbles from a nested card control", async () => {
    useEditorUiStore.setState({
      selectedMediaAssetIds: new Set(["asset-a"]),
      previewMediaId: "asset-a",
    });
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () => root.render(<MediaCard item={mediaItem("asset-b")} />));
    const nested = container.querySelector<HTMLButtonElement>('button[aria-pressed]')!;

    await act(async () => nested.focus());

    expect([...useEditorUiStore.getState().selectedMediaAssetIds]).toEqual(["asset-a"]);
    expect(useEditorUiStore.getState().previewMediaId).toBe("asset-a");
    await act(async () => root.unmount());
  });

  it("visibly selects a folder on click and opens it with Enter", async () => {
    const onOpen = vi.fn();
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () =>
      root.render(<FolderTile folder={{ id: "folder-1", name: "B-roll" }} onOpen={onOpen} />),
    );
    const tile = container.querySelector<HTMLElement>('[role="gridcell"]')!;

    await act(async () => tile.dispatchEvent(new MouseEvent("click", { bubbles: true })));
    expect([...useEditorUiStore.getState().selectedFolderIds]).toEqual(["folder-1"]);
    expect(tile.getAttribute("aria-selected")).toBe("true");
    expect(tile.hasAttribute("aria-pressed")).toBe(false);
    expect(tile.tabIndex).toBe(0);

    await act(async () =>
      tile.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true })),
    );
    expect(onOpen).toHaveBeenCalledWith("folder-1");
    expect([...useEditorUiStore.getState().selectedFolderIds]).toEqual([]);
    await act(async () => root.unmount());
  });

  it("moves A to B with ArrowRight, synchronizes focus/preview, then deletes only B", async () => {
    useEditorUiStore.setState({
      mediaTab: "material",
      mediaSubTab: "import",
      mediaPanelCurrentFolderId: null,
    });
    useMediaStore.setState({
      items: [mediaItem("asset-a"), mediaItem("asset-b")],
      folders: [],
      importing: false,
      error: null,
    });
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () => root.render(<MediaKeyboardHarness />));
    const first = container.querySelector<HTMLElement>('[data-media-asset-id="asset-a"]')!;
    const second = container.querySelector<HTMLElement>('[data-media-asset-id="asset-b"]')!;

    await act(async () => first.dispatchEvent(new MouseEvent("click", { bubbles: true })));
    useEditorUiStore.setState({ selectedFolderIds: new Set(["stale-folder"]) });
    await act(async () =>
      first.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "ArrowRight",
          code: "ArrowRight",
          bubbles: true,
          cancelable: true,
        }),
      ),
    );

    expect(document.activeElement).toBe(second);
    expect(second.tabIndex).toBe(0);
    expect(first.tabIndex).toBe(-1);
    expect(useEditorUiStore.getState().previewMediaId).toBe("asset-b");
    expect([...useEditorUiStore.getState().selectedMediaAssetIds]).toEqual(["asset-b"]);
    expect([...useEditorUiStore.getState().selectedFolderIds]).toEqual([]);

    await act(async () => {
      second.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "Delete",
          code: "Delete",
          bubbles: true,
          cancelable: true,
        }),
      );
      await Promise.resolve();
    });
    expect(editActions.deleteMedia).toHaveBeenCalledTimes(1);
    expect(editActions.deleteMedia).toHaveBeenCalledWith(["asset-b"], expect.any(Object));
    await act(async () => root.unmount());
  });

  it("lets a focused media card own Space instead of toggling transport", async () => {
    const requestMediaPreviewToggle = vi.fn();
    useEditorUiStore.setState({
      view: "editor",
      settingsOpen: false,
      exportDialogOpen: false,
      saveAsProgress: null,
      projectSettingsPrompt: null,
      pendingSwapClipId: null,
      mediaTab: "material",
      mediaSubTab: "import",
      mediaPanelCurrentFolderId: null,
      previewMediaId: "other-asset",
      selectedMediaAssetIds: new Set(["other-asset"]),
      requestMediaPreviewToggle,
    });
    useMediaStore.setState({
      items: [mediaItem("asset-a")],
      folders: [],
      importing: false,
      error: null,
    });
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () => root.render(<MediaKeyboardHarness />));
    const card = container.querySelector<HTMLElement>('[data-media-asset-id="asset-a"]')!;
    const nonMediaTarget = document.createElement("div");
    container.append(nonMediaTarget);

    await act(async () =>
      nonMediaTarget.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: " ",
          code: "Space",
          bubbles: true,
          cancelable: true,
        }),
      ),
    );
    expect(requestMediaPreviewToggle).toHaveBeenCalledOnce();
    requestMediaPreviewToggle.mockClear();
    card.focus();

    await act(async () =>
      card.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: " ",
          code: "Space",
          bubbles: true,
          cancelable: true,
        }),
      ),
    );

    expect(requestMediaPreviewToggle).not.toHaveBeenCalled();
    expect(useEditorUiStore.getState().previewMediaId).toBe("asset-a");
    expect([...useEditorUiStore.getState().selectedMediaAssetIds]).toEqual(["asset-a"]);
    await act(async () => root.unmount());
  });

  it("ignores held-key repeats and concurrent Delete requests while deletion is pending", async () => {
    const pending = deferred<void>();
    vi.mocked(editActions.deleteMedia).mockReturnValue(pending.promise);
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () => root.render(<MediaCard item={mediaItem("asset-a")} />));
    const card = container.querySelector<HTMLElement>('[data-media-asset-id="asset-a"]')!;
    await act(async () => card.dispatchEvent(new MouseEvent("click", { bubbles: true })));

    await act(async () => {
      card.dispatchEvent(new KeyboardEvent("keydown", { key: "Delete", bubbles: true }));
      card.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Delete", repeat: true, bubbles: true }),
      );
      card.dispatchEvent(new KeyboardEvent("keydown", { key: "Delete", bubbles: true }));
      await Promise.resolve();
    });
    expect(editActions.deleteMedia).toHaveBeenCalledTimes(1);

    await act(async () => pending.resolve(undefined));
    await act(async () => root.unmount());
  });

  it("does not coalesce different media targets in the same project", async () => {
    const firstDelete = deferred<void>();
    vi.mocked(editActions.deleteMedia)
      .mockReturnValueOnce(firstDelete.promise)
      .mockResolvedValueOnce(undefined);
    useProjectStore.setState({ projectEpoch: 20, projectPath: "/same.opentake" });
    useEditorUiStore.setState({ selectedMediaAssetIds: new Set(["asset-a"]) });

    const first = deleteSelectedMediaAssets();
    useEditorUiStore.setState({ selectedMediaAssetIds: new Set(["asset-b"]) });
    const second = deleteSelectedMediaAssets();
    await second;
    firstDelete.resolve(undefined);
    await first;

    expect(editActions.deleteMedia).toHaveBeenCalledTimes(2);
    expect(editActions.deleteMedia).toHaveBeenNthCalledWith(1, ["asset-a"], {
      projectEpoch: 20,
      projectPath: "/same.opentake",
      timelineVersion: 0,
    });
    expect(editActions.deleteMedia).toHaveBeenNthCalledWith(2, ["asset-b"], {
      projectEpoch: 20,
      projectPath: "/same.opentake",
      timelineVersion: 0,
    });
  });

  it("does not coalesce the same target across different timeline revisions", async () => {
    const firstDelete = deferred<void>();
    vi.mocked(editActions.deleteMedia)
      .mockReturnValueOnce(firstDelete.promise)
      .mockResolvedValueOnce(undefined);
    useProjectStore.setState({
      projectEpoch: 21,
      projectPath: "/same.opentake",
      timelineVersion: 4,
    });
    useEditorUiStore.setState({ selectedMediaAssetIds: new Set(["asset-a"]) });

    const first = deleteSelectedMediaAssets();
    useProjectStore.setState({ timelineVersion: 5 });
    const second = deleteSelectedMediaAssets();
    await second;
    firstDelete.resolve(undefined);
    await first;

    expect(editActions.deleteMedia).toHaveBeenNthCalledWith(1, ["asset-a"], {
      projectEpoch: 21,
      projectPath: "/same.opentake",
      timelineVersion: 4,
    });
    expect(editActions.deleteMedia).toHaveBeenNthCalledWith(2, ["asset-a"], {
      projectEpoch: 21,
      projectPath: "/same.opentake",
      timelineVersion: 5,
    });
  });

  it("clears media selection when completion advances to the returned revision", async () => {
    const completed = deferred<{
      changed: boolean;
      actionName: string;
      affectedClipIds: string[];
      timelineVersion: number;
      summary: string;
    }>();
    vi.mocked(editActions.deleteMedia).mockReturnValue(completed.promise);
    useProjectStore.setState({
      projectEpoch: 22,
      projectPath: "/same.opentake",
      timelineVersion: 8,
    });
    useEditorUiStore.setState({
      selectedMediaAssetIds: new Set(["asset-a"]),
      previewMediaId: "asset-a",
    });

    const deletion = deleteSelectedMediaAssets();
    useProjectStore.setState({ timelineVersion: 9 });
    completed.resolve({
      changed: true,
      actionName: "Delete Media",
      affectedClipIds: [],
      timelineVersion: 9,
      summary: "Deleted asset-a",
    });
    await deletion;

    expect([...useEditorUiStore.getState().selectedMediaAssetIds]).toEqual([]);
    expect(useEditorUiStore.getState().previewMediaId).toBeNull();
  });

  it("keeps folder selection when a newer revision supersedes completion", async () => {
    const completed = deferred<{
      changed: boolean;
      actionName: string;
      affectedClipIds: string[];
      timelineVersion: number;
      summary: string;
    }>();
    vi.mocked(editActions.deleteFolder).mockReturnValue(completed.promise);
    useProjectStore.setState({
      projectEpoch: 23,
      projectPath: "/same.opentake",
      timelineVersion: 12,
    });
    useEditorUiStore.setState({ selectedFolderIds: new Set(["folder-a"]) });

    const deletion = deleteSelectedFolders();
    useProjectStore.setState({ timelineVersion: 14 });
    completed.resolve({
      changed: true,
      actionName: "Delete Folder",
      affectedClipIds: [],
      timelineVersion: 13,
      summary: "Deleted folder-a",
    });
    await deletion;

    expect([...useEditorUiStore.getState().selectedFolderIds]).toEqual(["folder-a"]);
  });

  it("isolates pending media deletes and late completion by project identity", async () => {
    const projectADelete = deferred<void>();
    const projectBDelete = deferred<void>();
    vi.mocked(editActions.deleteMedia)
      .mockReturnValueOnce(projectADelete.promise)
      .mockReturnValueOnce(projectBDelete.promise);
    useProjectStore.setState({ projectEpoch: 30, projectPath: "/A.opentake" });
    useEditorUiStore.setState({
      selectedMediaAssetIds: new Set(["shared-asset"]),
      previewMediaId: "shared-asset",
    });

    const first = deleteSelectedMediaAssets();
    useProjectStore.setState({ projectEpoch: 31, projectPath: "/B.opentake" });
    useEditorUiStore.setState({
      selectedMediaAssetIds: new Set(["shared-asset"]),
      previewMediaId: "shared-asset",
    });
    const second = deleteSelectedMediaAssets();
    projectBDelete.resolve(undefined);
    await second;
    useEditorUiStore.setState({
      selectedMediaAssetIds: new Set(["shared-asset"]),
      previewMediaId: "shared-asset",
    });
    projectADelete.resolve(undefined);
    await first;

    expect(editActions.deleteMedia).toHaveBeenCalledTimes(2);
    expect(editActions.deleteMedia).toHaveBeenNthCalledWith(1, ["shared-asset"], {
      projectEpoch: 30,
      projectPath: "/A.opentake",
      timelineVersion: 0,
    });
    expect(editActions.deleteMedia).toHaveBeenNthCalledWith(2, ["shared-asset"], {
      projectEpoch: 31,
      projectPath: "/B.opentake",
      timelineVersion: 0,
    });
    expect([...useEditorUiStore.getState().selectedMediaAssetIds]).toEqual(["shared-asset"]);
    expect(useEditorUiStore.getState().previewMediaId).toBe("shared-asset");
  });

  it("isolates pending folder deletes and late completion by project identity", async () => {
    const projectADelete = deferred<void>();
    const projectBDelete = deferred<void>();
    vi.mocked(editActions.deleteFolder)
      .mockReturnValueOnce(projectADelete.promise)
      .mockReturnValueOnce(projectBDelete.promise);
    useProjectStore.setState({ projectEpoch: 40, projectPath: "/A.opentake" });
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () =>
      root.render(<FolderTile folder={{ id: "shared-folder", name: "Shared" }} onOpen={() => {}} />),
    );
    const folder = container.querySelector<HTMLElement>('[data-media-folder-id="shared-folder"]')!;
    await act(async () => folder.dispatchEvent(new MouseEvent("click", { bubbles: true })));
    await act(async () => {
      folder.dispatchEvent(new KeyboardEvent("keydown", { key: "Delete", bubbles: true }));
      await Promise.resolve();
    });

    useProjectStore.setState({ projectEpoch: 41, projectPath: "/B.opentake" });
    await act(async () => folder.dispatchEvent(new MouseEvent("click", { bubbles: true })));
    await act(async () => {
      folder.dispatchEvent(new KeyboardEvent("keydown", { key: "Delete", bubbles: true }));
      await Promise.resolve();
    });
    projectBDelete.resolve(undefined);
    await act(async () => {
      await projectBDelete.promise;
      await Promise.resolve();
    });
    useEditorUiStore.setState({ selectedFolderIds: new Set(["shared-folder"]) });
    projectADelete.resolve(undefined);
    await act(async () => {
      await projectADelete.promise;
      await Promise.resolve();
    });

    expect(editActions.deleteFolder).toHaveBeenCalledTimes(2);
    expect(editActions.deleteFolder).toHaveBeenNthCalledWith(1, ["shared-folder"], {
      projectEpoch: 40,
      projectPath: "/A.opentake",
      timelineVersion: 0,
    });
    expect(editActions.deleteFolder).toHaveBeenNthCalledWith(2, ["shared-folder"], {
      projectEpoch: 41,
      projectPath: "/B.opentake",
      timelineVersion: 0,
    });
    expect([...useEditorUiStore.getState().selectedFolderIds]).toEqual(["shared-folder"]);
    await act(async () => root.unmount());
  });

  it("keeps context-menu fallback independent from the current selection", async () => {
    useEditorUiStore.setState({
      focusedPanel: "media",
      selectedMediaAssetIds: new Set(["asset-a"]),
      previewMediaId: "asset-a",
    });
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () => root.render(<MediaCard item={mediaItem("asset-b")} />));
    const card = container.querySelector<HTMLElement>('[data-media-asset-id="asset-b"]')!;

    await act(async () =>
      card.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true })),
    );
    const deleteItem = container.querySelector<HTMLButtonElement>('[role="menuitem"]')!;
    await act(async () => {
      deleteItem.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      await Promise.resolve();
    });

    expect(editActions.deleteMedia).toHaveBeenCalledWith(["asset-b"], expect.any(Object));
    expect([...useEditorUiStore.getState().selectedMediaAssetIds]).toEqual(["asset-a"]);
    expect(useEditorUiStore.getState().previewMediaId).toBe("asset-a");
    await act(async () => root.unmount());
  });

  it("supports directional menu focus and closes the menu on Tab or focusout", async () => {
    const container = document.createElement("div");
    const outside = document.createElement("button");
    document.body.append(container, outside);
    const root = createRoot(container);
    await act(async () =>
      root.render(<FolderTile folder={{ id: "folder-a", name: "A" }} onOpen={() => {}} />),
    );
    const folder = container.querySelector<HTMLElement>('[data-media-folder-id="folder-a"]')!;

    await act(async () =>
      folder.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true })),
    );
    let menu = container.querySelector<HTMLElement>('[role="menu"]')!;
    await act(async () =>
      menu.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true })),
    );
    expect(document.activeElement).toBe(folder);

    await act(async () =>
      folder.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true })),
    );
    menu = container.querySelector<HTMLElement>('[role="menu"]')!;
    const items = [...menu.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')];
    expect(document.activeElement).toBe(items[0]);
    await act(async () =>
      items[0]!.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true })),
    );
    expect(document.activeElement).toBe(items[1]);
    await act(async () =>
      items[1]!.dispatchEvent(new KeyboardEvent("keydown", { key: "Home", bubbles: true })),
    );
    expect(document.activeElement).toBe(items[0]);
    await act(async () =>
      items[0]!.dispatchEvent(new KeyboardEvent("keydown", { key: "Tab", bubbles: true })),
    );
    expect(container.querySelector('[role="menu"]')).toBeNull();

    await act(async () =>
      folder.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true })),
    );
    menu = container.querySelector<HTMLElement>('[role="menu"]')!;
    await act(async () =>
      menu.dispatchEvent(
        new FocusEvent("focusout", { bubbles: true, relatedTarget: outside }),
      ),
    );
    expect(container.querySelector('[role="menu"]')).toBeNull();
    await act(async () => root.unmount());
  });
});

describe("media presentation controls (view mode / sort / filter)", () => {
  it("sorts locally by name, duration, and file size without mutating the list", () => {
    const items = [
      { ...mediaItem("b"), duration: 2, fileSize: 10 },
      { ...mediaItem("a"), duration: 5, fileSize: 40 },
      { ...mediaItem("c"), duration: 1, fileSize: 25 },
      { ...mediaItem("d"), duration: 3, fileSize: null }, // 离线素材
    ];

    expect(sortMediaItems(items, "default")).toBe(items);
    expect(sortMediaItems(items, "name").map((i) => i.id)).toEqual(["a", "b", "c", "d"]);
    expect(sortMediaItems(items, "duration").map((i) => i.id)).toEqual(["a", "d", "b", "c"]);
    expect(sortMediaItems(items, "fileSize").map((i) => i.id)).toEqual(["a", "c", "b", "d"]);
    // 输入数组未被改动（排序是副本上的操作）
    expect(items.map((i) => i.id)).toEqual(["b", "a", "c", "d"]);
  });

  it("filters locally by type without mutating the list", () => {
    const items = [
      mediaItem("v1"),
      { ...mediaItem("a1"), type: "audio" },
      { ...mediaItem("i1"), type: "image" },
      mediaItem("v2"),
    ];

    expect(filterMediaByType(items, "all")).toBe(items);
    expect(filterMediaByType(items, "video").map((i) => i.id)).toEqual(["v1", "v2"]);
    expect(filterMediaByType(items, "audio").map((i) => i.id)).toEqual(["a1"]);
    expect(filterMediaByType(items, "image").map((i) => i.id)).toEqual(["i1"]);
    expect(items).toHaveLength(4);
  });

  it("toggles grid/list view, sorts, and filters the loaded list locally", async () => {
    useEditorUiStore.setState({
      view: "editor",
      settingsOpen: false,
      exportDialogOpen: false,
      saveAsProgress: null,
      projectSettingsPrompt: null,
      pendingSwapClipId: null,
      mediaTab: "material",
      mediaSubTab: "import",
      mediaPanelCurrentFolderId: null,
    });
    useMediaStore.setState({
      items: [
        { ...mediaItem("zebra"), duration: 3, fileSize: 30 },
        { ...mediaItem("alpha"), duration: 9, fileSize: 90 },
        { ...mediaItem("mid-song"), type: "audio", duration: 1, fileSize: 10 },
      ],
      folders: [],
      importing: false,
      error: null,
    });
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () => root.render(<MediaPanel />));

    const assetIds = () =>
      [...container.querySelectorAll<HTMLElement>("[data-media-asset-id]")].map(
        (el) => el.dataset.mediaAssetId,
      );

    // 默认保持服务端顺序，网格视图
    expect(assetIds()).toEqual(["zebra", "alpha", "mid-song"]);
    expect(container.querySelector('[data-media-layout="grid"]')).not.toBeNull();

    // 排序：按名称（菜单选择后自动关闭）
    await act(async () =>
      container
        .querySelector<HTMLButtonElement>('button[title="排序"]')!
        .dispatchEvent(new MouseEvent("click", { bubbles: true })),
    );
    await act(async () =>
      [...container.querySelectorAll<HTMLButtonElement>('[role="menuitemradio"]')]
        .find((b) => b.textContent === "按名称")!
        .dispatchEvent(new MouseEvent("click", { bubbles: true })),
    );
    expect(assetIds()).toEqual(["alpha", "mid-song", "zebra"]);
    expect(container.querySelector('[role="menu"]')).toBeNull();

    // 筛选：仅视频
    await act(async () =>
      container
        .querySelector<HTMLButtonElement>('button[title="筛选"]')!
        .dispatchEvent(new MouseEvent("click", { bubbles: true })),
    );
    await act(async () =>
      [...container.querySelectorAll<HTMLButtonElement>('[role="menuitemradio"]')]
        .find((b) => b.textContent === "仅视频")!
        .dispatchEvent(new MouseEvent("click", { bubbles: true })),
    );
    expect(assetIds()).toEqual(["alpha", "zebra"]);

    // 视图模式：网格 → 列表 → 网格（排序/筛选在列表视图中同样生效）
    const viewButton = container.querySelector<HTMLButtonElement>('button[title="视图模式"]')!;
    await act(async () => viewButton.dispatchEvent(new MouseEvent("click", { bubbles: true })));
    expect(container.querySelector('[data-media-layout="list"]')).not.toBeNull();
    expect(assetIds()).toEqual(["alpha", "zebra"]);
    expect(
      container.querySelector<HTMLElement>("[data-media-asset-id]")?.getAttribute("role"),
    ).toBe("gridcell");
    await act(async () => viewButton.dispatchEvent(new MouseEvent("click", { bubbles: true })));
    expect(container.querySelector('[data-media-layout="grid"]')).not.toBeNull();
    await act(async () => root.unmount());
  });

  it("gives list rows the same selection/delete contract as grid cards", async () => {
    useEditorUiStore.setState({
      view: "editor",
      settingsOpen: false,
      exportDialogOpen: false,
      saveAsProgress: null,
      projectSettingsPrompt: null,
      pendingSwapClipId: null,
      mediaTab: "material",
      mediaSubTab: "import",
      mediaPanelCurrentFolderId: null,
      selectedMediaAssetIds: new Set(["asset-a"]),
      previewMediaId: "asset-a",
    });
    useMediaStore.setState({
      items: [mediaItem("asset-a"), mediaItem("asset-b")],
      folders: [],
      importing: false,
      error: null,
    });
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () => root.render(<MediaPanel />));
    await act(async () =>
      container
        .querySelector<HTMLButtonElement>('button[title="视图模式"]')!
        .dispatchEvent(new MouseEvent("click", { bubbles: true })),
    );
    const rowB = container.querySelector<HTMLElement>('[data-media-asset-id="asset-b"]')!;

    await act(async () => rowB.focus());
    expect([...useEditorUiStore.getState().selectedMediaAssetIds]).toEqual(["asset-b"]);
    expect(useEditorUiStore.getState().previewMediaId).toBe("asset-b");

    await act(async () => {
      rowB.dispatchEvent(new KeyboardEvent("keydown", { key: "Delete", bubbles: true }));
      await Promise.resolve();
    });
    expect(editActions.deleteMedia).toHaveBeenCalledOnce();
    expect(editActions.deleteMedia).toHaveBeenCalledWith(["asset-b"], expect.any(Object));
    await act(async () => root.unmount());
  });

  it("disables the view-mode toggle while a search flattens the list", async () => {
    useEditorUiStore.setState({
      view: "editor",
      settingsOpen: false,
      exportDialogOpen: false,
      saveAsProgress: null,
      projectSettingsPrompt: null,
      pendingSwapClipId: null,
      mediaTab: "material",
      mediaSubTab: "import",
      mediaPanelCurrentFolderId: null,
    });
    useMediaStore.setState({
      items: [mediaItem("asset-a"), mediaItem("asset-b")],
      folders: [],
      importing: false,
      error: null,
    });
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () => root.render(<MediaPanel />));

    const viewButton = () =>
      container.querySelector<HTMLButtonElement>('button[title="视图模式"]')!;
    expect(viewButton().disabled).toBe(false);

    const input = container.querySelector<HTMLInputElement>("input")!;
    const typeQuery = (value: string) => {
      const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
      setter?.call(input, value);
      input.dispatchEvent(new Event("input", { bubbles: true }));
    };
    await act(async () => typeQuery("asset"));
    // 搜索结果视图忽略网格/列表切换：视图按钮禁用，排序/筛选仍然可用
    expect(viewButton().disabled).toBe(true);
    const sortButton = container.querySelector<HTMLButtonElement>('button[title="排序"]')!;
    expect(sortButton.disabled).toBe(false);
    const filterButton = container.querySelector<HTMLButtonElement>('button[title="筛选"]')!;
    expect(filterButton.disabled).toBe(false);

    await act(async () => typeQuery(""));
    expect(viewButton().disabled).toBe(false);
    await act(async () => root.unmount());
  });

  it("hides the presentation controls on subtabs without the full media list", async () => {
    // 「我的」收藏库：数据源是全局库条目，不是项目媒体列表
    useEditorUiStore.setState({
      view: "editor",
      settingsOpen: false,
      exportDialogOpen: false,
      saveAsProgress: null,
      projectSettingsPrompt: null,
      pendingSwapClipId: null,
      mediaTab: "material",
      mediaSubTab: "mine",
      mediaPanelCurrentFolderId: null,
    });
    useMediaStore.setState({
      items: [mediaItem("asset-a")],
      folders: [],
      importing: false,
      error: null,
    });
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () => root.render(<MediaPanel />));
    expect(container.querySelector('button[title="视图模式"]')).toBeNull();
    expect(container.querySelector('button[title="排序"]')).toBeNull();
    expect(container.querySelector('button[title="筛选"]')).toBeNull();

    // 音效：任务型列表（SoundLibraryTab 自己按 query 过滤）
    useEditorUiStore.setState({ mediaTab: "audio", mediaSubTab: "sound" });
    await act(async () => Promise.resolve());
    expect(container.querySelector('button[title="排序"]')).toBeNull();
    expect(container.querySelector('button[title="筛选"]')).toBeNull();

    // 提取：任务型列表（视频-only）
    useEditorUiStore.setState({ mediaSubTab: "extract" });
    await act(async () => Promise.resolve());
    expect(container.querySelector('button[title="视图模式"]')).toBeNull();
    expect(container.querySelector('button[title="排序"]')).toBeNull();
    await act(async () => root.unmount());
  });
});

describe("MediaCard drag image", () => {
  it("uses a thumbnail-only drag image without filename characters", async () => {
    useEditorUiStore.setState({
      mediaTab: "material",
      mediaSubTab: "import",
      mediaPanelCurrentFolderId: null,
      previewMediaId: null,
    });
    useMediaStore.setState({
      items: [
        {
          id: "long-video",
          name: "第二节课超长素材.mov",
          type: "video",
          duration: 3_600,
          hasAudio: true,
          path: "/long.mov",
          thumbnail: "/long-thumb.jpg",
          favorite: false,
        },
      ],
      folders: [],
      importing: false,
      error: null,
    });

    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () => root.render(<MediaPanel />));
    const card = container.querySelector<HTMLElement>('[draggable="true"]');
    expect(card).not.toBeNull();

    const setDragImage = vi.fn();
    const drag = new Event("dragstart", { bubbles: true, cancelable: true });
    Object.defineProperty(drag, "dataTransfer", {
      value: {
        effectAllowed: "none",
        setData: vi.fn(),
        setDragImage,
      },
    });
    await act(async () => card!.dispatchEvent(drag));

    expect(setDragImage).toHaveBeenCalledTimes(1);
    const preview = setDragImage.mock.calls[0][0] as HTMLElement;
    expect(preview.textContent?.trim()).toBe("");
    expect(preview.style.width).toBe("80px");
    expect(preview.style.height).toBe("60px");
    await act(async () => root.unmount());
  });
});

describe("MediaPanel accessibility contracts", () => {
  it("labels the media search and keeps every tab control target mounted", () => {
    useEditorUiStore.setState({
      mediaTab: "material",
      mediaSubTab: "import",
      mediaPanelCurrentFolderId: null,
    });

    const container = document.createElement("div");
    container.innerHTML = renderToStaticMarkup(<MediaPanel />);

    const search = container.querySelector<HTMLInputElement>('input[type="search"]');
    expect(search?.getAttribute("aria-label")).toBeTruthy();

    const tablists = [...container.querySelectorAll<HTMLElement>('[role="tablist"]')];
    expect(tablists).toHaveLength(2);
    for (const tablist of tablists) {
      const tabs = [...tablist.querySelectorAll<HTMLButtonElement>('[role="tab"]')];
      expect(tabs.filter((tab) => tab.tabIndex === 0)).toHaveLength(1);
      for (const tab of tabs) {
        const panelId = tab.getAttribute("aria-controls");
        expect(panelId).toBeTruthy();
        const panel = container.querySelector<HTMLElement>(`#${panelId}`);
        expect(panel?.getAttribute("role")).toBe("tabpanel");
        expect(panel?.hidden).toBe(tab.getAttribute("aria-selected") !== "true");
      }
    }
  });

  it("exposes popup state and supports complete menu focus navigation", async () => {
    useEditorUiStore.setState({
      view: "editor",
      mediaTab: "material",
      mediaSubTab: "import",
      mediaPanelCurrentFolderId: null,
    });
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () => root.render(<MediaPanel />));

    const popupTriggers = () =>
      [...container.querySelectorAll<HTMLButtonElement>('button[aria-haspopup="menu"]')];
    expect(popupTriggers()).toHaveLength(3);

    for (let index = 0; index < popupTriggers().length; index += 1) {
      const trigger = popupTriggers()[index]!;
      expect(trigger.getAttribute("aria-expanded")).toBe("false");
      const menuId = trigger.getAttribute("aria-controls");
      expect(menuId).toBeTruthy();

      await act(async () => trigger.click());
      const menu = document.getElementById(menuId!);
      expect(trigger.getAttribute("aria-expanded")).toBe("true");
      expect(menu?.getAttribute("role")).toBe("menu");
      const options = [...menu!.querySelectorAll<HTMLButtonElement>('[role^="menuitem"]')];
      expect(document.activeElement).toBe(options[0]);
      expect(options.map((option) => option.tabIndex)).toEqual([
        0,
        ...options.slice(1).map(() => -1),
      ]);

      await act(async () =>
        document.activeElement?.dispatchEvent(
          new KeyboardEvent("keydown", { key: "End", bubbles: true }),
        ),
      );
      expect(document.activeElement).toBe(options.at(-1));
      expect(options.map((option) => option.tabIndex)).toEqual([
        ...options.slice(0, -1).map(() => -1),
        0,
      ]);
      await act(async () =>
        document.activeElement?.dispatchEvent(
          new KeyboardEvent("keydown", { key: "ArrowUp", bubbles: true }),
        ),
      );
      expect(document.activeElement).toBe(options[Math.max(0, options.length - 2)]);
      expect(options.filter((option) => option.tabIndex === 0)).toEqual([
        options[Math.max(0, options.length - 2)],
      ]);
      await act(async () =>
        document.activeElement?.dispatchEvent(
          new KeyboardEvent("keydown", { key: "Home", bubbles: true }),
        ),
      );
      expect(document.activeElement).toBe(options[0]);
      expect(options.filter((option) => option.tabIndex === 0)).toEqual([options[0]]);
      await act(async () =>
        document.activeElement?.dispatchEvent(
          new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }),
        ),
      );
      expect(document.activeElement).toBe(options[Math.min(1, options.length - 1)]);
      expect(options.filter((option) => option.tabIndex === 0)).toEqual([
        options[Math.min(1, options.length - 1)],
      ]);
      await act(async () =>
        document.activeElement?.dispatchEvent(
          new KeyboardEvent("keydown", { key: "Escape", bubbles: true }),
        ),
      );
      expect(document.getElementById(menuId!)).toBeNull();
      expect(document.activeElement).toBe(trigger);
    }

    const importTrigger = popupTriggers()[0]!;
    await act(async () =>
      importTrigger.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowUp", bubbles: true })),
    );
    const importMenu = document.getElementById(importTrigger.getAttribute("aria-controls")!);
    const importOptions = [
      ...importMenu!.querySelectorAll<HTMLButtonElement>('[role^="menuitem"]'),
    ];
    expect(document.activeElement).toBe(importOptions.at(-1));
    await act(async () =>
      document.activeElement?.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Escape", bubbles: true }),
      ),
    );

    await act(async () => root.unmount());
  });

  it("dismisses Import, Sort, and Filter on Tab or focusout without restoring the trigger", async () => {
    useEditorUiStore.setState({
      view: "editor",
      mediaTab: "material",
      mediaSubTab: "import",
      mediaPanelCurrentFolderId: null,
    });
    const container = document.createElement("div");
    const outside = document.createElement("button");
    document.body.append(container, outside);
    const root = createRoot(container);
    await act(async () => root.render(<MediaPanel />));

    const popupTriggers = () =>
      [...container.querySelectorAll<HTMLButtonElement>('button[aria-haspopup="menu"]')];
    expect(popupTriggers()).toHaveLength(3);

    await act(async () => popupTriggers()[0]?.click());
    await act(async () => popupTriggers()[1]?.click());
    expect(container.querySelectorAll('[role="menu"]')).toHaveLength(1);
    expect(popupTriggers()[0]?.getAttribute("aria-expanded")).toBe("false");
    expect(popupTriggers()[1]?.getAttribute("aria-expanded")).toBe("true");
    await act(async () =>
      document.activeElement?.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Escape", bubbles: true }),
      ),
    );

    for (let index = 0; index < 3; index += 1) {
      let trigger = popupTriggers()[index]!;
      const menuId = trigger.getAttribute("aria-controls")!;
      await act(async () => trigger.click());
      let option = document
        .getElementById(menuId)!
        .querySelector<HTMLButtonElement>('[role^="menuitem"]')!;
      expect(document.activeElement).toBe(option);

      await act(async () =>
        option.dispatchEvent(new KeyboardEvent("keydown", { key: "Tab", bubbles: true })),
      );
      trigger = popupTriggers()[index]!;
      expect(document.getElementById(menuId)).toBeNull();
      expect(trigger.getAttribute("aria-expanded")).toBe("false");
      expect(document.activeElement).not.toBe(trigger);

      await act(async () => trigger.click());
      option = document
        .getElementById(menuId)!
        .querySelector<HTMLButtonElement>('[role^="menuitem"]')!;
      expect(document.activeElement).toBe(option);
      await act(async () => outside.focus());
      trigger = popupTriggers()[index]!;
      expect(document.getElementById(menuId)).toBeNull();
      expect(trigger.getAttribute("aria-expanded")).toBe("false");
      expect(document.activeElement).toBe(outside);
    }

    await act(async () => root.unmount());
    outside.remove();
  });

  it("restores each popup trigger after Enter or Space selects an item", async () => {
    useEditorUiStore.setState({
      view: "editor",
      mediaTab: "material",
      mediaSubTab: "import",
      mediaPanelCurrentFolderId: null,
    });
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () => root.render(<MediaPanel />));

    const keys = ["Enter", " ", "Enter"] as const;
    for (let index = 0; index < keys.length; index += 1) {
      const trigger = [
        ...container.querySelectorAll<HTMLButtonElement>('button[aria-haspopup="menu"]'),
      ][index]!;
      const menuId = trigger.getAttribute("aria-controls")!;
      await act(async () => trigger.click());
      const option = document
        .getElementById(menuId)!
        .querySelector<HTMLButtonElement>('[role^="menuitem"]')!;
      expect(document.activeElement).toBe(option);

      await act(async () =>
        option.dispatchEvent(new KeyboardEvent("keydown", { key: keys[index], bubbles: true })),
      );
      expect(document.getElementById(menuId)).toBeNull();
      expect(document.activeElement).toBe(trigger);
    }

    await act(async () => root.unmount());
  });

  it("keeps every clickable breadcrumb target at least 24 by 24 CSS pixels", async () => {
    useEditorUiStore.setState({
      mediaTab: "material",
      mediaSubTab: "import",
      mediaPanelCurrentFolderId: "child",
    });
    useMediaStore.setState({
      items: [],
      folders: [
        { id: "parent", name: "Parent" },
        { id: "child", name: "Child", parentFolderId: "parent" },
      ],
      importing: false,
      error: null,
    });

    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () => root.render(<MediaPanel />));
    await act(async () =>
      useEditorUiStore.setState({ mediaPanelCurrentFolderId: "child" }),
    );
    const breadcrumbButtons = [
      ...container.querySelectorAll<HTMLButtonElement>("button[data-folder-breadcrumb-target]"),
    ];
    expect(breadcrumbButtons).toHaveLength(2);
    for (const button of breadcrumbButtons) {
      expect(Number.parseFloat(button.style.minWidth)).toBeGreaterThanOrEqual(24);
      expect(Number.parseFloat(button.style.minHeight)).toBeGreaterThanOrEqual(24);
    }
    await act(async () => root.unmount());
  });
});
