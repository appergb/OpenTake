// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../i18n", () => ({
  useT: () => (key: string) => key,
  t: (key: string) => key,
  useI18nStore: { subscribe: () => () => {} },
}));
vi.mock("../../store/editActions", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../store/editActions")>();
  return { ...actual, deleteFolder: vi.fn(), deleteMedia: vi.fn() };
});

import { useEditorUiStore } from "../../store/uiStore";
import { useProjectStore } from "../../store/projectStore";
import { useClipboardStore } from "../../store/clipboardStore";
import { useMediaStore } from "../../store/mediaStore";
import { useUpdateStore } from "../../store/updateStore";
import type { Clip } from "../../lib/types";
import * as editActions from "../../store/editActions";
import { handleViewShortcutKeyDown } from "../../hooks/useKeyboardShortcuts";
import {
  APPLICATION_MENU_SPEC,
  applicationMenuStateSnapshot,
  runApplicationMenuCommand,
  ViewMenu,
} from "./ViewMenu";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

let root: Root | null = null;
let container: HTMLDivElement | null = null;

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

function menuItems(): HTMLButtonElement[] {
  return [...(container?.querySelectorAll<HTMLButtonElement>('[role="menu"] button') ?? [])];
}

function videoClip(): Clip {
  return {
    id: "clip-1",
    mediaRef: "media-1",
    mediaType: "video",
    sourceClipType: "video",
    startFrame: 0,
    durationFrames: 30,
    trimStartFrame: 0,
    trimEndFrame: 0,
    speed: 1,
    volume: 1,
    fadeInFrames: 0,
    fadeOutFrames: 0,
    fadeInInterpolation: "linear",
    fadeOutInterpolation: "linear",
    opacity: 1,
    transform: {
      centerX: 0.5,
      centerY: 0.5,
      width: 1,
      height: 1,
      rotation: 0,
      flipHorizontal: false,
      flipVertical: false,
    },
    crop: { left: 0, top: 0, right: 0, bottom: 0 },
  };
}

function menuItem(label: string): HTMLButtonElement {
  const item = menuItems().find((button) => button.textContent?.includes(label));
  expect(item, `missing menu item ${label}`).toBeDefined();
  return item!;
}

async function openMenu(): Promise<HTMLButtonElement> {
  const trigger = container?.querySelector<HTMLButtonElement>('button[aria-label="view.menu"]');
  expect(trigger).not.toBeNull();
  await act(async () => trigger?.click());
  expect(trigger?.getAttribute("aria-expanded")).toBe("true");
  return trigger!;
}

beforeEach(async () => {
  localStorage.clear();
  useEditorUiStore.setState({
    view: "editor",
    layoutPreset: "default",
    focusedPanel: "timeline",
    maximizedPanel: null,
    agentPanelVisible: false,
    mediaPanelVisible: true,
    inspectorPanelVisible: true,
    fullscreen: false,
    selectedMediaAssetIds: new Set(),
    selectedFolderIds: new Set(),
    previewMediaId: null,
  });
  useProjectStore.setState({ projectEpoch: 0, projectPath: null, timelineVersion: 0 });
  useUpdateStore.setState({
    phase: "idle",
    dialogOpen: false,
    source: null,
    update: null,
    progress: null,
    error: null,
  });
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
  await act(async () => root?.render(<ViewMenu />));
});

afterEach(async () => {
  if (root) await act(async () => root?.unmount());
  container?.remove();
  root = null;
  container = null;
  vi.clearAllMocks();
  vi.restoreAllMocks();
});

describe("ViewMenu aggregate command contract", () => {
  it("blocks native menu mutations and stale accelerators throughout installation", async () => {
    await act(async () => {
      useProjectStore.setState({
        projectPath: "/tmp/update-lock.opentake",
        compatibilityReadOnly: false,
        canUndo: true,
        timeline: {
          ...useProjectStore.getState().timeline,
          tracks: [
            {
              id: "v1",
              type: "video",
              muted: false,
              hidden: false,
              syncLocked: true,
              clips: [videoClip()],
            },
          ],
        },
      });
      useEditorUiStore.setState({ exportDialogOpen: false, layoutPreset: "default" });
      useUpdateStore.setState({ phase: "installing", dialogOpen: true });
    });

    expect(applicationMenuStateSnapshot().enabled).toMatchObject({
      quit: false,
      checkUpdates: false,
      new: false,
      open: false,
      save: false,
      importMedia: false,
      export: false,
      undo: false,
      split: false,
      layoutDefault: false,
      layoutMedia: false,
      fullscreen: false,
    });

    await act(async () => {
      runApplicationMenuCommand("export");
      runApplicationMenuCommand("layoutMedia");
    });
    expect(useEditorUiStore.getState()).toMatchObject({
      exportDialogOpen: false,
      layoutPreset: "default",
    });
  });

  it("keeps native Export closed when the timeline has no renderable clips", async () => {
    await act(async () => {
      useEditorUiStore.setState({ exportDialogOpen: false, view: "editor" });
      useProjectStore.setState({
        timeline: {
          ...useProjectStore.getState().timeline,
          tracks: [],
        },
      });
      runApplicationMenuCommand("export");
    });
    expect(useEditorUiStore.getState().exportDialogOpen).toBe(false);

    await act(async () => {
      useProjectStore.setState({
        timeline: {
          ...useProjectStore.getState().timeline,
          tracks: [
            {
              id: "empty-v1",
              type: "video",
              muted: false,
              hidden: false,
              syncLocked: true,
              clips: [],
            },
          ],
        },
      });
      runApplicationMenuCommand("export");
    });
    expect(useEditorUiStore.getState().exportDialogOpen).toBe(false);

    await act(async () => {
      useProjectStore.setState({
        timeline: {
          ...useProjectStore.getState().timeline,
          tracks: [
            {
              id: "v1",
              type: "video",
              muted: false,
              hidden: false,
              syncLocked: true,
              clips: [videoClip()],
            },
          ],
        },
      });
      runApplicationMenuCommand("export");
    });
    expect(useEditorUiStore.getState().exportDialogOpen).toBe(true);
  });

  it("routes application-menu media Delete through the shared pending transaction", async () => {
    const pending = deferred<void>();
    vi.mocked(editActions.deleteMedia).mockReturnValue(pending.promise);
    await act(async () => {
      useProjectStore.setState({ projectEpoch: 9, projectPath: "/menu-delete.opentake" });
      useEditorUiStore.setState({
        focusedPanel: "media",
        selectedMediaAssetIds: new Set(["media-1"]),
        previewMediaId: "media-1",
      });
      runApplicationMenuCommand("delete");
      runApplicationMenuCommand("delete");
      await Promise.resolve();
    });
    await act(async () => {
      pending.resolve(undefined);
      await pending.promise;
      await Promise.resolve();
    });
    const calls = vi.mocked(editActions.deleteMedia).mock.calls;
    const ui = useEditorUiStore.getState();

    expect(calls).toEqual([
      [
        ["media-1"],
        {
          projectEpoch: 9,
          projectPath: "/menu-delete.opentake",
          timelineVersion: 0,
        },
      ],
    ]);
    expect([...ui.selectedMediaAssetIds]).toEqual([]);
    expect(ui.previewMediaId).toBeNull();
  });

  it("enables and prioritizes folder Delete through the same application-menu boundary", async () => {
    vi.mocked(editActions.deleteFolder).mockResolvedValue(undefined);
    await act(async () => {
      useProjectStore.setState({ projectEpoch: 10, projectPath: "/folder-delete.opentake" });
      useEditorUiStore.setState({
        view: "editor",
        focusedPanel: "media",
        selectedFolderIds: new Set(["folder-1"]),
        selectedMediaAssetIds: new Set(),
      });
    });
    const enabled = applicationMenuStateSnapshot().enabled.delete;

    await act(async () => {
      useEditorUiStore.setState({ selectedMediaAssetIds: new Set(["stale-media"]) });
      runApplicationMenuCommand("delete");
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(enabled).toBe(true);
    expect(editActions.deleteFolder).toHaveBeenCalledOnce();
    expect(editActions.deleteFolder).toHaveBeenCalledWith(["folder-1"], expect.any(Object));
    expect(editActions.deleteMedia).not.toHaveBeenCalled();
    expect([...useEditorUiStore.getState().selectedFolderIds]).toEqual([]);
  });

  it("clears a folder selection when media Select All runs so Delete removes assets", async () => {
    vi.mocked(editActions.deleteMedia).mockResolvedValue(undefined);
    await act(async () => {
      useProjectStore.setState({ projectEpoch: 12, projectPath: "/select-all.opentake" });
      useMediaStore.setState({
        items: [
          {
            id: "media-1",
            name: "Media 1",
            type: "video",
            duration: 1,
            hasAudio: false,
            path: "/tmp/media-1.mp4",
          },
          {
            id: "media-2",
            name: "Media 2",
            type: "image",
            duration: 0,
            hasAudio: false,
            path: "/tmp/media-2.png",
          },
        ],
      });
      useEditorUiStore.setState({
        focusedPanel: "media",
        selectedFolderIds: new Set(["folder-1"]),
        selectedMediaAssetIds: new Set(),
      });
      runApplicationMenuCommand("selectAll");
    });

    expect([...useEditorUiStore.getState().selectedMediaAssetIds]).toEqual([
      "media-1",
      "media-2",
    ]);
    expect([...useEditorUiStore.getState().selectedFolderIds]).toEqual([]);

    await act(async () => {
      runApplicationMenuCommand("delete");
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(editActions.deleteFolder).not.toHaveBeenCalled();
    expect(editActions.deleteMedia).toHaveBeenCalledOnce();
    expect(editActions.deleteMedia).toHaveBeenCalledWith(
      ["media-1", "media-2"],
      expect.any(Object),
    );
  });

  it("surfaces an application-menu media Delete rejection", async () => {
    vi.mocked(editActions.deleteMedia).mockRejectedValue(new Error("delete rejected"));
    await act(async () => {
      useProjectStore.setState({ projectEpoch: 11, projectPath: "/delete-error.opentake" });
      useEditorUiStore.setState({
        view: "editor",
        focusedPanel: "media",
        selectedFolderIds: new Set(),
        selectedMediaAssetIds: new Set(["media-1"]),
        toast: null,
      });
      runApplicationMenuCommand("delete");
      await Promise.resolve();
      await Promise.resolve();
    });

    await vi.waitFor(() =>
      expect(useEditorUiStore.getState().toast?.message).toContain("delete rejected"),
    );
  });

  it("commands_shortcuts_checked_state_and_disabled_rules", async () => {
    expect(
      APPLICATION_MENU_SPEC.map(({ group, id, accelerator, kind }) => [
        group,
        id,
        accelerator ?? null,
        kind,
      ]),
    ).toEqual([
      ["app", "about", null, "predefined"],
      ["app", "checkUpdates", null, "action"],
      ["app", "settings", "CmdOrCtrl+,", "action"],
      ["app", "quit", "CmdOrCtrl+Q", "predefined"],
      ["file", "new", "CmdOrCtrl+N", "action"],
      ["file", "open", "CmdOrCtrl+O", "action"],
      ["file", "save", "CmdOrCtrl+S", "action"],
      ["file", "saveAs", "CmdOrCtrl+Shift+S", "action"],
      ["file", "importMedia", "CmdOrCtrl+I", "action"],
      ["file", "export", "CmdOrCtrl+E", "action"],
      ["edit", "undo", "CmdOrCtrl+Z", "action"],
      ["edit", "redo", "CmdOrCtrl+Shift+Z", "action"],
      ["edit", "cut", "CmdOrCtrl+X", "action"],
      ["edit", "copy", "CmdOrCtrl+C", "action"],
      ["edit", "paste", "CmdOrCtrl+V", "action"],
      ["edit", "selectAll", "CmdOrCtrl+A", "action"],
      ["edit", "split", "CmdOrCtrl+K", "action"],
      ["edit", "trimStart", "Q", "action"],
      ["edit", "trimEnd", "W", "action"],
      ["edit", "delete", "Backspace", "action"],
      ["view", "mediaPanel", "CmdOrCtrl+0", "check"],
      ["view", "inspector", "CmdOrCtrl+Alt+0", "check"],
      ["view", "agentPanel", "CmdOrCtrl+Alt+A", "check"],
      ["view", "maximizeFocused", "`", "check"],
      ["view", "layoutDefault", "CmdOrCtrl+1", "check"],
      ["view", "layoutMedia", "CmdOrCtrl+2", "check"],
      ["view", "layoutVertical", "CmdOrCtrl+3", "check"],
      ["view", "fullscreen", "CmdOrCtrl+F", "check"],
      ["help", "tutorial", null, "disabled"],
      ["help", "shortcuts", "CmdOrCtrl+Shift+/", "action"],
      ["help", "mcp", null, "action"],
      ["help", "feedback", null, "disabled"],
    ]);
    expect(new Set(APPLICATION_MENU_SPEC.map((entry) => entry.id)).size).toBe(
      APPLICATION_MENU_SPEC.length,
    );

    await act(async () => {
      useProjectStore.setState({
        projectPath: "/tmp/menu-contract.opentake",
        compatibilityReadOnly: false,
        canUndo: true,
        canRedo: false,
        timeline: {
          fps: 30,
          width: 1920,
          height: 1080,
          settingsConfigured: true,
          tracks: [],
        },
      });
      useClipboardStore.setState({ hasContent: true });
      useMediaStore.setState({
        items: [
          {
            id: "media-1",
            name: "Media 1",
            type: "video",
            duration: 1,
            hasAudio: false,
            path: "/tmp/media-1.mp4",
          },
        ],
      });
      useEditorUiStore.setState({
        focusedPanel: "media",
        selectedMediaAssetIds: new Set(["media-1"]),
        fullscreen: true,
      });
    });
    const nativeState = applicationMenuStateSnapshot();
    expect(nativeState.enabled).toMatchObject({
      checkUpdates: true,
      save: true,
      saveAs: true,
      importMedia: true,
      export: false,
      undo: true,
      redo: false,
      cut: true,
      copy: true,
      paste: true,
      selectAll: true,
      split: false,
      trimStart: false,
      trimEnd: false,
      delete: true,
      tutorial: false,
      feedback: false,
    });
    expect(nativeState.checked).toMatchObject({
      layoutDefault: true,
      mediaPanel: true,
      inspector: true,
      agentPanel: false,
      fullscreen: true,
    });
    const checkUpdates = vi.spyOn(useUpdateStore.getState(), "check").mockResolvedValue();
    await act(async () => runApplicationMenuCommand("checkUpdates"));
    expect(checkUpdates).toHaveBeenCalledWith("manual");
    await act(async () => runApplicationMenuCommand("shortcuts"));
    expect(useEditorUiStore.getState()).toMatchObject({
      settingsOpen: true,
      settingsPane: "shortcuts",
    });
    await act(async () => runApplicationMenuCommand("mcp"));
    expect(useEditorUiStore.getState().settingsPane).toBe("mcp");
    await act(async () =>
      useEditorUiStore.setState({
        settingsOpen: false,
        settingsPane: "general",
        focusedPanel: "timeline",
        selectedMediaAssetIds: new Set(),
        fullscreen: false,
      }),
    );

    const trigger = await openMenu();

    const expected = [
      ["view.mediaPanel", "⌘0", "true"],
      ["view.inspector", "⌘⌥0", "true"],
      ["view.agentPanel", "⌘⌥A", "false"],
      ["view.maximizeFocused", "`", "false"],
      ["view.layoutDefault", "⌘1", "true"],
      ["view.layoutMedia", "⌘2", "false"],
      ["view.layoutVertical", "⌘3", "false"],
      ["view.enterFullScreen", "⌘F", "false"],
    ] as const;

    for (const [label, shortcut, checked] of expected) {
      const item = menuItem(label);
      expect(item.textContent).toContain(shortcut);
      expect(item.getAttribute("aria-checked")).toBe(checked);
    }

    // Opening the menu moves keyboard focus into the command list. Arrow keys
    // navigate enabled entries and Escape restores focus to the trigger.
    expect(document.activeElement).toBe(menuItem("view.mediaPanel"));
    await act(async () =>
      document.activeElement?.dispatchEvent(
        new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }),
      ),
    );
    expect(document.activeElement).toBe(menuItem("view.inspector"));

    await act(async () => menuItem("view.maximizeFocused").click());
    expect(useEditorUiStore.getState().maximizedPanel).toBe("timeline");
    expect(menuItem("view.maximizeFocused").getAttribute("aria-checked")).toBe("true");

    const editorEscape = vi.fn();
    window.addEventListener("keydown", editorEscape);
    await act(async () => window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" })));
    window.removeEventListener("keydown", editorEscape);
    expect(trigger.getAttribute("aria-expanded")).toBe("false");
    expect(document.activeElement).toBe(trigger);
    expect(useEditorUiStore.getState().maximizedPanel).toBe("timeline");
    expect(editorEscape).not.toHaveBeenCalled();

    // There is no legal maximize target without a focused panel. The item must
    // be explicitly disabled and activation must be a no-op.
    await act(async () => useEditorUiStore.setState({ focusedPanel: null, maximizedPanel: null }));
    await openMenu();
    const maximize = menuItem("view.maximizeFocused");
    expect(maximize.disabled).toBe(true);
    await act(async () => maximize.click());
    expect(useEditorUiStore.getState().maximizedPanel).toBeNull();

    const calls: string[] = [];
    const shortcutUi = {
      view: "editor" as const,
      setLayoutPreset: (preset: "default" | "media" | "vertical") =>
        calls.push(`layout:${preset}`),
      toggleAgentPanel: () => calls.push("agent"),
      toggleMediaPanel: () => calls.push("media"),
      toggleInspectorPanel: () => calls.push("inspector"),
      toggleMaximizedFocusedPanel: () => calls.push("maximize"),
      toggleFullscreen: async () => {
        calls.push("fullscreen");
      },
    };
    const shortcuts = [
      new KeyboardEvent("keydown", { code: "Digit0", metaKey: true, cancelable: true }),
      new KeyboardEvent("keydown", {
        code: "Digit0",
        metaKey: true,
        altKey: true,
        cancelable: true,
      }),
      new KeyboardEvent("keydown", {
        code: "KeyA",
        metaKey: true,
        altKey: true,
        cancelable: true,
      }),
      new KeyboardEvent("keydown", { code: "Backquote", cancelable: true }),
      new KeyboardEvent("keydown", { code: "Digit1", ctrlKey: true, cancelable: true }),
      new KeyboardEvent("keydown", { code: "Digit2", metaKey: true, cancelable: true }),
      new KeyboardEvent("keydown", { code: "Digit3", metaKey: true, cancelable: true }),
      new KeyboardEvent("keydown", { code: "KeyF", metaKey: true, cancelable: true }),
    ];

    for (const event of shortcuts) {
      expect(handleViewShortcutKeyDown(event, shortcutUi)).toBe(true);
      expect(event.defaultPrevented).toBe(true);
    }
    expect(calls).toEqual([
      "media",
      "inspector",
      "agent",
      "maximize",
      "layout:default",
      "layout:media",
      "layout:vertical",
      "fullscreen",
    ]);

    // A repeat is consumed but must not oscillate state; text-entry and Home
    // events remain available to their native owners.
    expect(
      handleViewShortcutKeyDown(
        new KeyboardEvent("keydown", { code: "Digit0", metaKey: true, repeat: true }),
        shortcutUi,
      ),
    ).toBe(true);
    expect(calls).toHaveLength(8);
    expect(
      handleViewShortcutKeyDown(
        {
          code: "Digit0",
          metaKey: true,
          ctrlKey: false,
          altKey: false,
          shiftKey: false,
          repeat: false,
          target: document.createElement("input"),
          preventDefault: vi.fn(),
        } as unknown as KeyboardEvent,
        shortcutUi,
      ),
    ).toBe(false);

    useEditorUiStore.setState({
      focusedPanel: "media",
      maximizedPanel: "media",
      mediaPanelVisible: true,
    });
    useEditorUiStore.getState().toggleMediaPanel();
    expect(useEditorUiStore.getState()).toMatchObject({
      focusedPanel: "timeline",
      maximizedPanel: null,
      mediaPanelVisible: false,
    });

    const originalRequestFullscreen = document.documentElement.requestFullscreen;
    Object.defineProperty(document.documentElement, "requestFullscreen", {
      configurable: true,
      value: vi.fn().mockRejectedValue(new Error("host rejected fullscreen")),
    });
    useEditorUiStore.setState({ fullscreen: false, toast: null });
    await useEditorUiStore.getState().toggleFullscreen();
    expect(useEditorUiStore.getState().toast?.message).toBe("view.fullscreenFailed");
    Object.defineProperty(document.documentElement, "requestFullscreen", {
      configurable: true,
      value: originalRequestFullscreen,
    });
  });
});
