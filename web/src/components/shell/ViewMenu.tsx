/**
 * View menu overlay (SPEC §2.9). Hosts the Layout-preset switch and panel
 * visibility toggles that were previously inlined into the title bar, so the
 * §2.8 title bar can stay a 1:1 copy of the upstream (Agent toggle + Export
 * only). A click-out / Escape dismisses it. Every action reuses the existing
 * uiStore mutators and matches the §9.6 keyboard shortcuts (⌘1–3, ⌘0, ⌘⌥0).
 */

import { useEffect, useRef, useState } from "react";
import {
  Bot,
  Menu,
  PanelLeft,
  PanelRight,
  Columns3,
  Check,
  Maximize2,
  Fullscreen,
  type LucideIcon,
} from "lucide-react";
import { Icon } from "../ui/Icon";
import { useEditorUiStore, type LayoutPreset } from "../../store/uiStore";
import { t, useI18nStore, useT } from "../../i18n";
import { isTauri } from "../../lib/api";
import { useProjectStore } from "../../store/projectStore";
import { useClipboardStore } from "../../store/clipboardStore";
import { useMediaStore } from "../../store/mediaStore";
import {
  isUpdateInstallationBlocking,
  useUpdateStore,
} from "../../store/updateStore";
import * as edit from "../../store/editActions";
import {
  deleteSelectedFolders,
  deleteSelectedMediaAssets,
} from "../../store/mediaDeleteActions";
import {
  newProjectAndEnter,
  openProjectViaDialog,
  saveCurrentProject,
  saveCurrentProjectAs,
} from "../../store/projectActions";
import { importFilesViaDialog } from "../../store/mediaActions";

export type ApplicationMenuKind = "action" | "check" | "predefined" | "disabled";
export type ApplicationMenuGroup = "app" | "file" | "edit" | "view" | "help";

export interface ApplicationMenuSpecEntry {
  group: ApplicationMenuGroup;
  id: string;
  labelKey: string;
  accelerator?: string;
  kind: ApplicationMenuKind;
}

/** Auditable §2.9 command matrix. This drives the packaged native menu rather
 *  than maintaining a second, implicit list that can drift from the in-app
 *  View menu or its keyboard handlers. */
export const APPLICATION_MENU_SPEC: readonly ApplicationMenuSpecEntry[] = [
  { group: "app", id: "about", labelKey: "menu.about", kind: "predefined" },
  { group: "app", id: "checkUpdates", labelKey: "menu.checkUpdates", kind: "action" },
  { group: "app", id: "settings", labelKey: "settings.title", accelerator: "CmdOrCtrl+,", kind: "action" },
  { group: "app", id: "quit", labelKey: "menu.quit", accelerator: "CmdOrCtrl+Q", kind: "predefined" },
  { group: "file", id: "new", labelKey: "menu.new", accelerator: "CmdOrCtrl+N", kind: "action" },
  { group: "file", id: "open", labelKey: "menu.open", accelerator: "CmdOrCtrl+O", kind: "action" },
  { group: "file", id: "save", labelKey: "menu.save", accelerator: "CmdOrCtrl+S", kind: "action" },
  { group: "file", id: "saveAs", labelKey: "menu.saveAs", accelerator: "CmdOrCtrl+Shift+S", kind: "action" },
  { group: "file", id: "importMedia", labelKey: "menu.importMedia", accelerator: "CmdOrCtrl+I", kind: "action" },
  { group: "file", id: "export", labelKey: "menu.export", accelerator: "CmdOrCtrl+E", kind: "action" },
  { group: "edit", id: "undo", labelKey: "menu.undo", accelerator: "CmdOrCtrl+Z", kind: "action" },
  { group: "edit", id: "redo", labelKey: "menu.redo", accelerator: "CmdOrCtrl+Shift+Z", kind: "action" },
  { group: "edit", id: "cut", labelKey: "menu.cut", accelerator: "CmdOrCtrl+X", kind: "action" },
  { group: "edit", id: "copy", labelKey: "menu.copy", accelerator: "CmdOrCtrl+C", kind: "action" },
  { group: "edit", id: "paste", labelKey: "menu.paste", accelerator: "CmdOrCtrl+V", kind: "action" },
  { group: "edit", id: "selectAll", labelKey: "menu.selectAll", accelerator: "CmdOrCtrl+A", kind: "action" },
  { group: "edit", id: "split", labelKey: "menu.split", accelerator: "CmdOrCtrl+K", kind: "action" },
  { group: "edit", id: "trimStart", labelKey: "menu.trimStart", accelerator: "Q", kind: "action" },
  { group: "edit", id: "trimEnd", labelKey: "menu.trimEnd", accelerator: "W", kind: "action" },
  { group: "edit", id: "delete", labelKey: "menu.delete", accelerator: "Backspace", kind: "action" },
  { group: "view", id: "mediaPanel", labelKey: "view.mediaPanel", accelerator: "CmdOrCtrl+0", kind: "check" },
  { group: "view", id: "inspector", labelKey: "view.inspector", accelerator: "CmdOrCtrl+Alt+0", kind: "check" },
  { group: "view", id: "agentPanel", labelKey: "view.agentPanel", accelerator: "CmdOrCtrl+Alt+A", kind: "check" },
  { group: "view", id: "maximizeFocused", labelKey: "view.maximizeFocused", accelerator: "`", kind: "check" },
  { group: "view", id: "layoutDefault", labelKey: "view.layoutDefault", accelerator: "CmdOrCtrl+1", kind: "check" },
  { group: "view", id: "layoutMedia", labelKey: "view.layoutMedia", accelerator: "CmdOrCtrl+2", kind: "check" },
  { group: "view", id: "layoutVertical", labelKey: "view.layoutVertical", accelerator: "CmdOrCtrl+3", kind: "check" },
  { group: "view", id: "fullscreen", labelKey: "view.enterFullScreen", accelerator: "CmdOrCtrl+F", kind: "check" },
  { group: "help", id: "tutorial", labelKey: "menu.tutorial", kind: "disabled" },
  { group: "help", id: "shortcuts", labelKey: "menu.shortcuts", accelerator: "CmdOrCtrl+Shift+/", kind: "action" },
  { group: "help", id: "mcp", labelKey: "settings.section.mcp", kind: "action" },
  { group: "help", id: "feedback", labelKey: "menu.feedback", kind: "disabled" },
] as const;

const PRESETS: Array<{ id: LayoutPreset; icon: typeof PanelLeft; labelKey: string; key: string }> = [
  { id: "default", icon: Columns3, labelKey: "view.layoutDefault", key: "⌘1" },
  { id: "media", icon: PanelLeft, labelKey: "view.layoutMedia", key: "⌘2" },
  { id: "vertical", icon: PanelRight, labelKey: "view.layoutVertical", key: "⌘3" },
];

function ignoreRejected(operation: Promise<unknown>): void {
  void operation.catch(() => {
    // Owning actions surface their own localized error/recovery state.
  });
}

function reportMediaDeleteFailure(operation: Promise<void>): void {
  void operation.catch((error: unknown) => {
    const message = error instanceof Error ? error.message : String(error);
    useEditorUiStore.getState().pushToast(message);
  });
}

/** Route native-menu activation through the same actions as the visible UI and
 *  global shortcut layer. Exported so the command boundary remains directly
 *  testable without requiring an OS menu server. */
export function runApplicationMenuCommand(id: string): void {
  if (isUpdateInstallationBlocking(useUpdateStore.getState().phase)) return;
  const ui = useEditorUiStore.getState();
  switch (id) {
    case "checkUpdates":
      ignoreRejected(useUpdateStore.getState().check("manual"));
      return;
    case "settings":
      ui.openSettingsPane("general");
      return;
    case "new":
      ignoreRejected(newProjectAndEnter());
      return;
    case "open":
      ignoreRejected(openProjectViaDialog());
      return;
    case "save":
      ignoreRejected(saveCurrentProject());
      return;
    case "saveAs":
      ignoreRejected(saveCurrentProjectAs());
      return;
    case "importMedia":
      ignoreRejected(importFilesViaDialog());
      return;
    case "export":
      // Native menu accelerators can still dispatch while an asynchronously
      // synchronized menu item is stale. Re-check the same state contract at
      // the command boundary so the empty-timeline path remains fail-closed.
      if (applicationMenuStateSnapshot().enabled.export) ui.setExportDialogOpen(true);
      return;
    case "undo":
      ignoreRejected(edit.undo());
      return;
    case "redo":
      ignoreRejected(edit.redo());
      return;
    case "cut":
      ignoreRejected(edit.cutClips());
      return;
    case "copy":
      edit.copyClips();
      return;
    case "paste":
      ignoreRejected(edit.pasteClipsAtPlayhead());
      return;
    case "selectAll": {
      if (ui.focusedPanel === "media") {
        ui.selectMediaAssets(new Set(useMediaStore.getState().items.map((item) => item.id)));
        useEditorUiStore.setState({ selectedFolderIds: new Set() });
      } else {
        const ids = useProjectStore
          .getState()
          .timeline.tracks.flatMap((track) => track.clips.map((clip) => clip.id));
        ui.selectClips(new Set(ids));
      }
      return;
    }
    case "split":
      ignoreRejected(edit.splitAtPlayhead());
      return;
    case "trimStart":
      ignoreRejected(edit.trimStartToPlayhead());
      return;
    case "trimEnd":
      ignoreRejected(edit.trimEndToPlayhead());
      return;
    case "delete":
      if (ui.focusedPanel === "media") {
        reportMediaDeleteFailure(
          ui.selectedFolderIds.size > 0
            ? deleteSelectedFolders()
            : deleteSelectedMediaAssets(),
        );
      } else {
        ignoreRejected(edit.deleteSelectedClips());
      }
      return;
    case "mediaPanel":
      ui.toggleMediaPanel();
      return;
    case "inspector":
      ui.toggleInspectorPanel();
      return;
    case "agentPanel":
      ui.toggleAgentPanel();
      return;
    case "maximizeFocused":
      ui.toggleMaximizedFocusedPanel();
      return;
    case "layoutDefault":
      ui.setLayoutPreset("default");
      return;
    case "layoutMedia":
      ui.setLayoutPreset("media");
      return;
    case "layoutVertical":
      ui.setLayoutPreset("vertical");
      return;
    case "fullscreen":
      ignoreRejected(ui.toggleFullscreen());
      return;
    case "shortcuts":
      ui.openSettingsPane("shortcuts");
      return;
    case "mcp":
      ui.openSettingsPane("mcp");
      return;
  }
}

interface NativeMenuItemHandle {
  setEnabled: (enabled: boolean) => Promise<void>;
  setText: (text: string) => Promise<void>;
}

interface NativeCheckMenuItemHandle extends NativeMenuItemHandle {
  setChecked: (checked: boolean) => Promise<void>;
}

interface NativeApplicationMenuHandles {
  items: Map<string, NativeMenuItemHandle>;
  checks: Map<string, NativeCheckMenuItemHandle>;
  texts: Map<string, { item: Pick<NativeMenuItemHandle, "setText">; labelKey: string }>;
}

export interface ApplicationMenuStateSnapshot {
  enabled: Readonly<Record<string, boolean>>;
  checked: Readonly<Record<string, boolean>>;
}

function nativeAccelerator(entry: ApplicationMenuSpecEntry): string | undefined {
  if (entry.id === "settings") return "CmdOrCtrl+Comma";
  if (entry.id === "maximizeFocused") return "Backquote";
  if (entry.id === "shortcuts") return "CmdOrCtrl+Shift+Slash";
  return entry.accelerator;
}

export function applicationMenuStateSnapshot(): ApplicationMenuStateSnapshot {
  const ui = useEditorUiStore.getState();
  const project = useProjectStore.getState();
  const clipboard = useClipboardStore.getState();
  const media = useMediaStore.getState();
  const editor = ui.view === "editor";
  const hasClips = project.timeline.tracks.some((track) => track.clips.length > 0);
  const clipSelection = ui.selectedClipIds.size > 0;
  const mediaSelection = ui.selectedMediaAssetIds.size > 0;
  const folderSelection = ui.selectedFolderIds.size > 0;
  const selection = ui.focusedPanel === "media" ? mediaSelection : clipSelection;
  const deleteSelection =
    ui.focusedPanel === "media" ? mediaSelection || folderSelection : clipSelection;
  const anySelectable = ui.focusedPanel === "media" ? media.items.length > 0 : hasClips;
  const mutableProject = editor && Boolean(project.projectPath) && !project.compatibilityReadOnly;
  const actionsEnabled = !isUpdateInstallationBlocking(useUpdateStore.getState().phase);

  const enabled: Record<string, boolean> = {
    quit: actionsEnabled,
    checkUpdates: actionsEnabled,
    settings: actionsEnabled,
    new: actionsEnabled,
    open: actionsEnabled,
    save: actionsEnabled && mutableProject,
    saveAs: actionsEnabled && mutableProject,
    importMedia: actionsEnabled && mutableProject,
    export: actionsEnabled && editor && hasClips,
    undo: actionsEnabled && editor && project.canUndo,
    redo: actionsEnabled && editor && project.canRedo,
    cut: actionsEnabled && editor && selection,
    copy: actionsEnabled && editor && selection,
    paste: actionsEnabled && editor && clipboard.hasContent,
    selectAll: actionsEnabled && editor && anySelectable,
    split: actionsEnabled && editor && hasClips,
    trimStart: actionsEnabled && editor && clipSelection,
    trimEnd: actionsEnabled && editor && clipSelection,
    delete: actionsEnabled && editor && deleteSelection,
    mediaPanel: actionsEnabled && editor,
    inspector: actionsEnabled && editor,
    agentPanel: actionsEnabled && editor,
    maximizeFocused: actionsEnabled && editor && ui.focusedPanel !== null,
    layoutDefault: actionsEnabled && editor,
    layoutMedia: actionsEnabled && editor,
    layoutVertical: actionsEnabled && editor,
    fullscreen: actionsEnabled && editor,
    tutorial: false,
    shortcuts: actionsEnabled,
    mcp: actionsEnabled,
    feedback: false,
  };
  const checked: Record<string, boolean> = {
    mediaPanel: ui.mediaPanelVisible,
    inspector: ui.inspectorPanelVisible,
    agentPanel: ui.agentPanelVisible,
    maximizeFocused: ui.maximizedPanel !== null,
    layoutDefault: ui.layoutPreset === "default",
    layoutMedia: ui.layoutPreset === "media",
    layoutVertical: ui.layoutPreset === "vertical",
    fullscreen: ui.fullscreen,
  };
  return { enabled, checked };
}

async function applyNativeApplicationMenuState(
  handles: NativeApplicationMenuHandles,
  { enabled, checked }: ApplicationMenuStateSnapshot,
): Promise<void> {
  await Promise.all(
    [...handles.items].map(([id, item]) =>
      id in enabled ? item.setEnabled(enabled[id]!) : Promise.resolve(),
    ),
  );
  await Promise.all(
    [...handles.checks].map(([id, item]) => item.setChecked(Boolean(checked[id]))),
  );
}

async function applyNativeApplicationMenuText(
  handles: NativeApplicationMenuHandles,
): Promise<void> {
  await Promise.all(
    [...handles.texts.values()].map(({ item, labelKey }) => item.setText(t(labelKey))),
  );
}

async function installNativeApplicationMenu(): Promise<() => void> {
  const { Menu, Submenu, MenuItem, CheckMenuItem, PredefinedMenuItem } =
    await import("@tauri-apps/api/menu");
  const handles: NativeApplicationMenuHandles = {
    items: new Map(),
    checks: new Map(),
    texts: new Map(),
  };
  const entries = new Map(APPLICATION_MENU_SPEC.map((entry) => [entry.id, entry]));

  const actionItem = async (id: string) => {
    const entry = entries.get(id)!;
    const item = await MenuItem.new({
      id,
      text: t(entry.labelKey),
      enabled: entry.kind !== "disabled",
      accelerator: nativeAccelerator(entry),
      action: () => runApplicationMenuCommand(id),
    });
    handles.items.set(id, item);
    handles.texts.set(id, { item, labelKey: entry.labelKey });
    return item;
  };
  const checkItem = async (id: string) => {
    const entry = entries.get(id)!;
    const item = await CheckMenuItem.new({
      id,
      text: t(entry.labelKey),
      checked: false,
      accelerator: nativeAccelerator(entry),
      action: () => runApplicationMenuCommand(id),
    });
    handles.items.set(id, item);
    handles.checks.set(id, item);
    handles.texts.set(id, { item, labelKey: entry.labelKey });
    return item;
  };
  const separator = () => PredefinedMenuItem.new({ item: "Separator" });

  const aboutItem = await PredefinedMenuItem.new({
    item: { About: { name: t("app.name"), version: __APP_VERSION__ } },
    text: t("menu.about"),
  });
  handles.texts.set("about", { item: aboutItem, labelKey: "menu.about" });
  const quitItem = await PredefinedMenuItem.new({ item: "Quit", text: t("menu.quit") });
  handles.texts.set("quit", { item: quitItem, labelKey: "menu.quit" });

  const appMenu = await Submenu.new({
    id: "app",
    text: t("app.name"),
    items: [
      aboutItem,
      await actionItem("checkUpdates"),
      await actionItem("settings"),
      await separator(),
      quitItem,
    ],
  });
  handles.texts.set("group:app", { item: appMenu, labelKey: "app.name" });
  const fileMenu = await Submenu.new({
    id: "file",
    text: t("menu.file"),
    items: [
      await actionItem("new"),
      await actionItem("open"),
      await separator(),
      await actionItem("save"),
      await actionItem("saveAs"),
      await separator(),
      await actionItem("importMedia"),
      await actionItem("export"),
    ],
  });
  handles.texts.set("group:file", { item: fileMenu, labelKey: "menu.file" });
  const editMenu = await Submenu.new({
    id: "edit",
    text: t("menu.edit"),
    items: [
      await actionItem("undo"),
      await actionItem("redo"),
      await separator(),
      await actionItem("cut"),
      await actionItem("copy"),
      await actionItem("paste"),
      await actionItem("selectAll"),
      await separator(),
      await actionItem("split"),
      await actionItem("trimStart"),
      await actionItem("trimEnd"),
      await actionItem("delete"),
    ],
  });
  handles.texts.set("group:edit", { item: editMenu, labelKey: "menu.edit" });
  const layoutMenu = await Submenu.new({
    id: "layout",
    text: t("view.layout"),
    items: [
      await checkItem("layoutDefault"),
      await checkItem("layoutMedia"),
      await checkItem("layoutVertical"),
    ],
  });
  handles.texts.set("group:layout", { item: layoutMenu, labelKey: "view.layout" });
  const viewMenu = await Submenu.new({
    id: "view",
    text: t("view.menu"),
    items: [
      await checkItem("mediaPanel"),
      await checkItem("inspector"),
      await checkItem("agentPanel"),
      await checkItem("maximizeFocused"),
      await separator(),
      layoutMenu,
      await checkItem("fullscreen"),
    ],
  });
  handles.texts.set("group:view", { item: viewMenu, labelKey: "view.menu" });
  const helpMenu = await Submenu.new({
    id: "help",
    text: t("menu.help"),
    items: [
      await actionItem("tutorial"),
      await actionItem("shortcuts"),
      await actionItem("mcp"),
      await separator(),
      await actionItem("feedback"),
    ],
  });
  handles.texts.set("group:help", { item: helpMenu, labelKey: "menu.help" });
  const menu = await Menu.new({ items: [appMenu, fileMenu, editMenu, viewMenu, helpMenu] });
  await menu.setAsAppMenu();

  let syncQueue = Promise.resolve();
  let stateSignature = "";
  let localeSignature = "";
  const syncState = () => {
    const snapshot = applicationMenuStateSnapshot();
    const nextSignature = JSON.stringify(snapshot);
    if (nextSignature === stateSignature) return;
    stateSignature = nextSignature;
    syncQueue = syncQueue
      .then(() => applyNativeApplicationMenuState(handles, snapshot))
      .catch(() => {
        stateSignature = "";
      });
  };
  const syncText = () => {
    const nextLocale = useI18nStore.getState().locale;
    if (nextLocale === localeSignature) return;
    localeSignature = nextLocale;
    syncQueue = syncQueue.then(() => applyNativeApplicationMenuText(handles)).catch(() => {
      localeSignature = "";
    });
  };
  const unsubs = [
    useEditorUiStore.subscribe(syncState),
    useProjectStore.subscribe(syncState),
    useClipboardStore.subscribe(syncState),
    useMediaStore.subscribe(syncState),
    useUpdateStore.subscribe(syncState),
    useI18nStore.subscribe(syncText),
  ];
  syncState();
  syncText();
  return () => unsubs.forEach((unsubscribe) => unsubscribe());
}

/** Installs the full native application menu once for packaged Tauri builds.
 *  The visual component is intentionally empty; ownership stays beside
 *  ViewMenu because both surfaces share one command/check-state contract. */
export function ApplicationMenuBridge() {
  useEffect(() => {
    if (!isTauri) return;
    let disposed = false;
    let cleanup: (() => void) | undefined;
    void installNativeApplicationMenu()
      .then((installedCleanup) => {
        if (disposed) installedCleanup();
        else cleanup = installedCleanup;
      })
      .catch(() => {
        useEditorUiStore.getState().pushToast(t("menu.installFailed"));
      });
    return () => {
      disposed = true;
      cleanup?.();
    };
  }, []);
  return null;
}

export function ViewMenu() {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const t = useT();

  const layoutPreset = useEditorUiStore((s) => s.layoutPreset);
  const setLayoutPreset = useEditorUiStore((s) => s.setLayoutPreset);
  const mediaVisible = useEditorUiStore((s) => s.mediaPanelVisible);
  const toggleMedia = useEditorUiStore((s) => s.toggleMediaPanel);
  const inspectorVisible = useEditorUiStore((s) => s.inspectorPanelVisible);
  const toggleInspector = useEditorUiStore((s) => s.toggleInspectorPanel);
  const agentVisible = useEditorUiStore((s) => s.agentPanelVisible);
  const toggleAgent = useEditorUiStore((s) => s.toggleAgentPanel);
  const focusedPanel = useEditorUiStore((s) => s.focusedPanel);
  const maximizedPanel = useEditorUiStore((s) => s.maximizedPanel);
  const toggleMaximizedFocusedPanel = useEditorUiStore(
    (s) => s.toggleMaximizedFocusedPanel,
  );
  const fullscreen = useEditorUiStore((s) => s.fullscreen);
  const syncFullscreen = useEditorUiStore((s) => s.syncFullscreen);
  const toggleFullscreen = useEditorUiStore((s) => s.toggleFullscreen);

  const closeAndRestoreFocus = () => {
    setOpen(false);
    queueMicrotask(() => triggerRef.current?.focus());
  };

  useEffect(() => {
    if (!open) return;
    void syncFullscreen().catch(() => {
      // A rejected native window query must not make the rest of the View menu
      // unusable. Keep the last known checked state and leave every command live.
    });
    menuRef.current
      ?.querySelector<HTMLButtonElement>('button:not(:disabled)')
      ?.focus();
  }, [open, syncFullscreen]);

  // Dismiss on outside click or Escape.
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        // Consume Escape before the editor-wide handler sees it. Otherwise one
        // keypress both dismisses this menu and exits a maximized panel.
        e.stopImmediatePropagation();
        closeAndRestoreFocus();
      }
    };
    window.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey, true);
    return () => {
      window.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey, true);
    };
  }, [open]);

  return (
    <div ref={rootRef} style={{ position: "relative", display: "inline-flex" }}>
      <button
        ref={triggerRef}
        title={t("view.menu")}
        aria-label={t("view.menu")}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
        className="hover-area"
        style={{
          width: 26,
          height: 26,
          display: "inline-flex",
          alignItems: "center",
          justifyContent: "center",
          color: "var(--text-secondary)",
          opacity: open ? 1 : 0.7,
        }}
      >
        <Icon icon={Menu} size={13} />
      </button>

      {open && (
        <div
          ref={menuRef}
          role="menu"
          aria-label={t("view.menu")}
          onKeyDown={(event) => {
            if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;
            event.preventDefault();
            const items = [
              ...(menuRef.current?.querySelectorAll<HTMLButtonElement>(
                'button:not(:disabled)',
              ) ?? []),
            ];
            if (items.length === 0) return;
            const current = items.indexOf(document.activeElement as HTMLButtonElement);
            const next =
              event.key === "Home"
                ? 0
                : event.key === "End"
                  ? items.length - 1
                  : event.key === "ArrowDown"
                    ? (Math.max(current, -1) + 1) % items.length
                    : (current <= 0 ? items.length : current) - 1;
            items[next]?.focus();
          }}
          style={{
            position: "absolute",
            top: "calc(100% + 6px)",
            left: 0,
            minWidth: 210,
            padding: "var(--space-xs)",
            background: "var(--bg-raised)",
            border: "var(--bw-thin) solid var(--border-primary)",
            borderRadius: "var(--radius-md)",
            boxShadow: "var(--shadow-lg)",
            zIndex: 200,
          }}
        >
          <MenuSectionLabel>{t("view.panels")}</MenuSectionLabel>
          <MenuItem
            icon={PanelLeft}
            label={t("view.mediaPanel")}
            shortcut="⌘0"
            checked={mediaVisible}
            onClick={toggleMedia}
          />
          <MenuItem
            icon={PanelRight}
            label={t("view.inspector")}
            shortcut="⌘⌥0"
            checked={inspectorVisible}
            onClick={toggleInspector}
          />
          <MenuItem
            icon={Bot}
            label={t("view.agentPanel")}
            shortcut="⌘⌥A"
            checked={agentVisible}
            onClick={toggleAgent}
          />
          <MenuItem
            icon={Maximize2}
            label={t("view.maximizeFocused")}
            shortcut="`"
            checked={maximizedPanel !== null}
            disabled={focusedPanel === null}
            onClick={toggleMaximizedFocusedPanel}
          />

          <MenuDivider />

          <MenuSectionLabel>{t("view.layout")}</MenuSectionLabel>
          {PRESETS.map((p) => (
            <MenuItem
              key={p.id}
              icon={p.icon}
              label={t(p.labelKey)}
              shortcut={p.key}
              checked={layoutPreset === p.id}
              onClick={() => {
                setLayoutPreset(p.id);
                setOpen(false);
              }}
            />
          ))}

          <MenuDivider />

          <MenuItem
            icon={Fullscreen}
            label={t("view.enterFullScreen")}
            shortcut="⌘F"
            checked={fullscreen}
            onClick={() => {
              void toggleFullscreen().catch(() => {
                // Keep the menu usable if the host rejects a fullscreen request.
              });
            }}
          />
        </div>
      )}
    </div>
  );
}

function MenuSectionLabel({ children }: { children: string }) {
  return (
    <div
      style={{
        padding: "var(--space-xxs) var(--space-sm)",
        fontSize: "var(--fs-xs)",
        fontWeight: "var(--fw-semibold)",
        color: "var(--text-tertiary)",
        textTransform: "uppercase",
        letterSpacing: "0.04em",
      }}
    >
      {children}
    </div>
  );
}

function MenuDivider() {
  return (
    <div
      style={{
        height: "var(--bw-thin)",
        background: "var(--border-primary)",
        margin: "var(--space-xs) var(--space-xs)",
      }}
    />
  );
}

function MenuItem({
  icon,
  label,
  shortcut,
  checked,
  disabled = false,
  onClick,
}: {
  icon: LucideIcon;
  label: string;
  shortcut: string;
  checked: boolean;
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      role="menuitemcheckbox"
      aria-checked={checked}
      disabled={disabled}
      onClick={onClick}
      className={disabled ? undefined : "hover-area"}
      style={{
        width: "100%",
        display: "flex",
        alignItems: "center",
        gap: "var(--space-sm)",
        height: 28,
        padding: "0 var(--space-sm)",
        borderRadius: "var(--radius-sm)",
        color: disabled
          ? "var(--text-muted)"
          : checked
            ? "var(--text-primary)"
            : "var(--text-secondary)",
        fontSize: "var(--fs-sm)",
        fontWeight: "var(--fw-medium)",
        textAlign: "left",
        cursor: disabled ? "default" : "pointer",
        opacity: disabled ? 0.45 : 1,
      }}
    >
      <span style={{ display: "inline-flex", width: 14, justifyContent: "center" }}>
        <Icon icon={icon} size={13} />
      </span>
      <span style={{ flex: 1 }}>{label}</span>
      <span
        style={{
          display: "inline-flex",
          width: 14,
          justifyContent: "center",
          color: "var(--text-primary)",
          opacity: checked ? 1 : 0,
        }}
      >
        <Icon icon={Check} size={13} />
      </span>
      <span style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)", minWidth: 28, textAlign: "right" }}>
        {shortcut}
      </span>
    </button>
  );
}
