/**
 * Global keyboard shortcuts (SPEC §9.6). Uses `event.code` for physical-key
 * parity with the upstream keyCodes. Skipped while a text input/textarea is
 * focused (SPEC §9.6 / trap #14). Cross-platform: ⌘ on macOS maps to Ctrl
 * elsewhere (metaKey || ctrlKey).
 */

import { useEffect } from "react";
import { useEditorUiStore } from "../store/uiStore";
import { useProjectStore } from "../store/projectStore";
import { useClipboardStore } from "../store/clipboardStore";
import { useMediaStore } from "../store/mediaStore";
import { t } from "../i18n";
import * as edit from "../store/editActions";
import { saveCurrentProject } from "../store/projectActions";
import { ZOOM } from "../lib/theme";
import type { AppView } from "../store/uiStore";
import { isTauri } from "../lib/api";
import { resolveTimelinePlaybackRoute } from "../components/preview/playbackRoute";
import { rustEngineEnabled } from "../components/preview/rustEngine";
import { runApplicationMenuCommand } from "../components/shell/ViewMenu";
import {
  isUpdateInstallationBlocking,
  useUpdateStore,
} from "../store/updateStore";

/** Per-keypress zoom step for ⌘+ / ⌘- (剪映: Cmd + +/-). */
const ZOOM_KEY_STEP = 1.3;
const TEXT_INPUT_TYPES: ReadonlySet<string> = new Set([
  "email",
  "password",
  "search",
  "tel",
  "text",
  "url",
]);

function isTextEntry(target: EventTarget | null): boolean {
  if (typeof Element === "undefined" || !(target instanceof Element)) return false;
  const editable = target.closest<HTMLElement>(
    "input, textarea, [contenteditable]",
  );
  if (!editable) return false;
  if (editable.isContentEditable || editable.matches("textarea")) return true;
  return (
    editable.matches("input") &&
    TEXT_INPUT_TYPES.has((editable as HTMLInputElement).type)
  );
}

function isNativeInteractionControl(target: EventTarget | null): boolean {
  if (typeof Element === "undefined" || !(target instanceof Element)) return false;
  return Boolean(
    target.closest(
      'input, button, a[href], select, option, summary, [role="button"], [role="menuitem"], [role="menuitemcheckbox"], [role="menuitemradio"], [role="checkbox"], [role="radio"], [role="switch"], [role="slider"], [role="spinbutton"], [role="combobox"], [role="listbox"], [role="separator"]',
    ),
  );
}

function nativeControlOwnsUnmodifiedKey(e: KeyboardEvent): boolean {
  if (!isNativeInteractionControl(e.target)) return false;
  return [
    "Enter",
    "NumpadEnter",
    "ArrowLeft",
    "ArrowRight",
    "ArrowUp",
    "ArrowDown",
  ].includes(e.code);
}

export const DOCUMENTED_SHORTCUT_ROWS = [
  "transport",
  "timeline-arrows",
  "media-arrows",
  "delete",
  "tools",
  "range",
  "trim",
  "maximize",
  "return-escape",
  "undo-redo",
  "clipboard",
  "split",
  "file",
  "panels",
  "layouts",
  "fullscreen",
  "help",
  "settings",
] as const;

export interface DocumentedShortcutContext {
  view: AppView;
  blocked: boolean;
  focusedPanel: "agent" | "media" | "preview" | "inspector" | "timeline" | null;
  compatibilityReadOnly: boolean;
  cropEditingActive: boolean;
}

export type ResolvedShortcut =
  | { type: "transport" }
  | { type: "stepFrame"; delta: number }
  | { type: "moveMediaSelection"; delta: number }
  | { type: "delete"; ripple: boolean }
  | { type: "setTool"; tool: "pointer" | "razor" }
  | { type: "markRange"; edge: "start" | "end" }
  | { type: "trimStart" }
  | { type: "trimEnd" }
  | { type: "maximize" }
  | { type: "mediaEnter" }
  | { type: "exitCrop" }
  | { type: "escape" }
  | { type: "history"; redo: boolean }
  | { type: "clipboard"; action: "copy" | "cut" | "paste" | "selectAll" }
  | { type: "split" }
  | {
      type: "application";
      id:
        | "settings"
        | "new"
        | "open"
        | "save"
        | "saveAs"
        | "importMedia"
        | "export"
        | "mediaPanel"
        | "inspector"
        | "agentPanel"
        | "layoutDefault"
        | "layoutMedia"
        | "layoutVertical"
        | "fullscreen"
        | "shortcuts";
    };

function isMutationShortcut(command: ResolvedShortcut): boolean {
  if (
    command.type === "delete" ||
    command.type === "trimStart" ||
    command.type === "trimEnd" ||
    command.type === "split" ||
    command.type === "history"
  ) {
    return true;
  }
  if (command.type === "clipboard") {
    return command.action === "cut" || command.action === "paste";
  }
  return (
    command.type === "application" &&
    (command.id === "save" || command.id === "saveAs" || command.id === "importMedia")
  );
}

/** Pure, table-driven §9.6 keyboard boundary. It resolves one physical key to
 * one semantic command only after view, modal, editable-target, platform
 * modifier, and compatibility-read-only rules have all been applied. */
export function resolveDocumentedShortcut(
  e: KeyboardEvent,
  context: DocumentedShortcutContext,
): ResolvedShortcut | null {
  if (context.view !== "editor" || context.blocked || e.isComposing || isTextEntry(e.target)) {
    return null;
  }
  const mod = e.metaKey || e.ctrlKey;
  let command: ResolvedShortcut | null = null;

  if (mod) {
    if (e.altKey) {
      if (!e.shiftKey && e.code === "Digit0") command = { type: "application", id: "inspector" };
      else if (!e.shiftKey && e.code === "KeyA") command = { type: "application", id: "agentPanel" };
    } else {
      switch (e.code) {
        case "KeyZ":
          command = { type: "history", redo: e.shiftKey };
          break;
        case "KeyC":
          if (!e.shiftKey) command = { type: "clipboard", action: "copy" };
          break;
        case "KeyX":
          if (!e.shiftKey) command = { type: "clipboard", action: "cut" };
          break;
        case "KeyV":
          if (!e.shiftKey) command = { type: "clipboard", action: "paste" };
          break;
        case "KeyA":
          if (!e.shiftKey) command = { type: "clipboard", action: "selectAll" };
          break;
        case "KeyK":
        case "KeyB":
          if (!e.shiftKey) command = { type: "split" };
          break;
        case "KeyN":
          if (!e.shiftKey) command = { type: "application", id: "new" };
          break;
        case "KeyO":
          if (!e.shiftKey) command = { type: "application", id: "open" };
          break;
        case "KeyS":
          command = { type: "application", id: e.shiftKey ? "saveAs" : "save" };
          break;
        case "KeyI":
          if (!e.shiftKey) command = { type: "application", id: "importMedia" };
          break;
        case "KeyE":
          if (!e.shiftKey) command = { type: "application", id: "export" };
          break;
        case "Digit0":
          if (!e.shiftKey) command = { type: "application", id: "mediaPanel" };
          break;
        case "Digit1":
          if (!e.shiftKey) command = { type: "application", id: "layoutDefault" };
          break;
        case "Digit2":
          if (!e.shiftKey) command = { type: "application", id: "layoutMedia" };
          break;
        case "Digit3":
          if (!e.shiftKey) command = { type: "application", id: "layoutVertical" };
          break;
        case "KeyF":
          if (!e.shiftKey) command = { type: "application", id: "fullscreen" };
          break;
        case "Slash":
          if (e.shiftKey) command = { type: "application", id: "shortcuts" };
          break;
        case "Comma":
          if (!e.shiftKey) command = { type: "application", id: "settings" };
          break;
      }
    }
  } else if (!e.altKey && !nativeControlOwnsUnmodifiedKey(e)) {
    switch (e.code) {
      case "Space":
        if (!e.shiftKey) {
          command = { type: "transport" };
        }
        break;
      case "ArrowLeft":
      case "ArrowRight":
        if (context.focusedPanel === "media") {
          if (!e.shiftKey) {
            command = { type: "moveMediaSelection", delta: e.code === "ArrowLeft" ? -1 : 1 };
          }
        } else {
          command = { type: "stepFrame", delta: (e.code === "ArrowLeft" ? -1 : 1) * (e.shiftKey ? 5 : 1) };
        }
        break;
      case "ArrowUp":
      case "ArrowDown":
        if (context.focusedPanel === "media" && !e.shiftKey) {
          command = { type: "moveMediaSelection", delta: e.code === "ArrowUp" ? -1 : 1 };
        }
        break;
      case "Backspace":
      case "Delete":
        command = { type: "delete", ripple: e.shiftKey };
        break;
      case "KeyC":
      case "KeyB":
        if (!e.shiftKey) command = { type: "setTool", tool: "razor" };
        break;
      case "KeyV":
      case "KeyA":
        if (!e.shiftKey) command = { type: "setTool", tool: "pointer" };
        break;
      case "KeyI":
        if (!e.shiftKey) command = { type: "markRange", edge: "start" };
        break;
      case "KeyO":
        if (!e.shiftKey) command = { type: "markRange", edge: "end" };
        break;
      case "BracketLeft":
      case "KeyQ":
        if (!e.shiftKey) command = { type: "trimStart" };
        break;
      case "BracketRight":
      case "KeyW":
        if (!e.shiftKey) command = { type: "trimEnd" };
        break;
      case "Backquote":
        if (!e.shiftKey) command = { type: "maximize" };
        break;
      case "Enter":
      case "NumpadEnter":
        if (!e.shiftKey) {
          if (context.cropEditingActive) command = { type: "exitCrop" };
          else if (context.focusedPanel === "media") command = { type: "mediaEnter" };
        }
        break;
      case "Escape":
        if (!e.shiftKey) command = { type: "escape" };
        break;
    }
  }

  if (command && context.compatibilityReadOnly && isMutationShortcut(command)) return null;
  return command;
}

function shortcutSurfaceBlocked(ui: ReturnType<typeof useEditorUiStore.getState>): boolean {
  return Boolean(
    ui.settingsOpen ||
      ui.exportDialogOpen ||
      ui.saveAsProgress ||
      ui.projectSettingsPrompt ||
      ui.pendingSwapClipId,
  );
}

export function shouldHandleTransportSpaceKey(e: KeyboardEvent, view: AppView): boolean {
  return (
    view === "editor" &&
    e.code === "Space" &&
    !e.metaKey &&
    !e.ctrlKey &&
    !e.altKey &&
    !e.shiftKey &&
    !e.isComposing &&
    !isTextEntry(e.target)
  );
}

interface TransportSpaceUi {
  view: AppView;
  previewMediaId: string | null;
  timelinePlaybackAllowed: boolean;
  requestMediaPreviewToggle: () => void;
  togglePlay: () => void;
}

export function handleTransportSpaceKeyDown(
  e: KeyboardEvent,
  ui: TransportSpaceUi,
): boolean {
  if (!shouldHandleTransportSpaceKey(e, ui.view)) return false;
  e.preventDefault();
  e.stopPropagation();
  if (e.repeat) return true;
  if (ui.previewMediaId) {
    ui.requestMediaPreviewToggle();
  } else if (ui.timelinePlaybackAllowed) {
    ui.togglePlay(); // rewinds from the parked end frame on replay
  }
  return true;
}

export function handleProjectSaveKeyDown(
  e: KeyboardEvent,
  save: () => void = () => {
    void saveCurrentProject();
  },
): boolean {
  if (
    e.code !== "KeyS" ||
    (!e.metaKey && !e.ctrlKey) ||
    e.altKey ||
    e.shiftKey ||
    isTextEntry(e.target)
  ) {
    return false;
  }
  e.preventDefault();
  if (!e.repeat) save();
  return true;
}

/** Handle the upstream Agent-panel accelerator on macOS and Windows/Linux. */
export function handleAgentPanelKeyDown(
  e: KeyboardEvent,
  view: AppView,
  toggle: () => void,
): boolean {
  if (
    view !== "editor" ||
    e.code !== "KeyA" ||
    !e.altKey ||
    e.shiftKey ||
    (!e.metaKey && !e.ctrlKey) ||
    isTextEntry(e.target)
  ) {
    return false;
  }
  e.preventDefault();
  if (!e.repeat) toggle();
  return true;
}

interface ViewShortcutUi {
  view: AppView;
  setLayoutPreset: (preset: "default" | "media" | "vertical") => void;
  toggleAgentPanel: () => void;
  toggleMediaPanel: () => void;
  toggleInspectorPanel: () => void;
  toggleMaximizedFocusedPanel: () => void;
  toggleFullscreen: () => Promise<void>;
}

/** Shared View-command accelerator boundary used by the app menu and the
 *  global key listener. Returning true means the native/browser shortcut was
 *  consumed, including held-key repeats (which are intentionally no-ops). */
export function handleViewShortcutKeyDown(e: KeyboardEvent, ui: ViewShortcutUi): boolean {
  if (ui.view !== "editor" || isTextEntry(e.target)) return false;
  const mod = e.metaKey || e.ctrlKey;
  let action: (() => void) | undefined;

  if (mod) {
    if (e.altKey && !e.shiftKey && e.code === "KeyA") action = ui.toggleAgentPanel;
    else if (!e.shiftKey && e.code === "Digit0") {
      action = e.altKey ? ui.toggleInspectorPanel : ui.toggleMediaPanel;
    } else if (!e.altKey && !e.shiftKey && e.code === "Digit1") {
      action = () => ui.setLayoutPreset("default");
    } else if (!e.altKey && !e.shiftKey && e.code === "Digit2") {
      action = () => ui.setLayoutPreset("media");
    } else if (!e.altKey && !e.shiftKey && e.code === "Digit3") {
      action = () => ui.setLayoutPreset("vertical");
    } else if (!e.altKey && !e.shiftKey && e.code === "KeyF") {
      action = () => void ui.toggleFullscreen();
    }
  } else if (!e.altKey && !e.shiftKey && e.code === "Backquote") {
    action = ui.toggleMaximizedFocusedPanel;
  }

  if (!action) return false;
  e.preventDefault();
  if (!e.repeat) action();
  return true;
}

export function useKeyboardShortcuts() {
  useEffect(() => {
    const blockDuringUpdateInstallation = (e: KeyboardEvent): boolean => {
      if (!isUpdateInstallationBlocking(useUpdateStore.getState().phase)) return false;
      e.preventDefault();
      e.stopImmediatePropagation();
      return true;
    };

    const context = (): DocumentedShortcutContext => {
      const ui = useEditorUiStore.getState();
      return {
        view: ui.view,
        blocked: shortcutSurfaceBlocked(ui),
        focusedPanel: ui.focusedPanel,
        compatibilityReadOnly: useProjectStore.getState().compatibilityReadOnly,
        cropEditingActive: ui.cropEditingActive,
      };
    };

    const handleSpaceKeyDown = (e: KeyboardEvent) => {
      if (blockDuringUpdateInstallation(e)) return;
      const ui = useEditorUiStore.getState();
      if (resolveDocumentedShortcut(e, context())?.type !== "transport") return;
      const timeline = useProjectStore.getState().timeline;
      const route = resolveTimelinePlaybackRoute(timeline, {
        rustAvailable: isTauri,
        rustEnabled: rustEngineEnabled(),
      });
      handleTransportSpaceKeyDown(e, {
        ...ui,
        timelinePlaybackAllowed: route.kind !== "unsupported",
      });
    };

    const moveMediaSelection = (delta: number) => {
      const ui = useEditorUiStore.getState();
      const renderedTiles =
        typeof document === "undefined"
          ? []
          : [
              ...document.querySelectorAll<HTMLElement>(
                '[data-media-tile="true"]',
              ),
            ];
      if (renderedTiles.length > 0) {
        const activeTile =
          document.activeElement instanceof Element
            ? document.activeElement.closest<HTMLElement>('[data-media-tile="true"]')
            : null;
        let current = activeTile ? renderedTiles.indexOf(activeTile) : -1;
        if (current < 0) {
          current = renderedTiles.findIndex((tile) =>
            tile.dataset.mediaFolderId
              ? ui.selectedFolderIds.has(tile.dataset.mediaFolderId)
              : Boolean(
                  tile.dataset.mediaAssetId &&
                    ui.selectedMediaAssetIds.has(tile.dataset.mediaAssetId),
                ),
          );
        }
        const origin = current >= 0 ? current : delta < 0 ? renderedTiles.length : -1;
        const nextIndex = Math.max(
          0,
          Math.min(renderedTiles.length - 1, origin + delta),
        );
        const next = renderedTiles[nextIndex]!;
        ui.focusPanel("media");
        if (next.dataset.mediaFolderId) {
          ui.selectMediaAssets(new Set());
          ui.setPreviewMedia(null);
          useEditorUiStore.setState({
            selectedFolderIds: new Set([next.dataset.mediaFolderId]),
          });
        } else if (next.dataset.mediaAssetId) {
          ui.selectMediaAssets(new Set([next.dataset.mediaAssetId]));
          useEditorUiStore.setState({ selectedFolderIds: new Set() });
          ui.setPreviewMedia(next.dataset.mediaAssetId);
        }
        next.focus();
        next.scrollIntoView?.({ block: "nearest", inline: "nearest" });
        return;
      }
      const items = useMediaStore.getState().items;
      if (items.length === 0) return;
      const selected = [...ui.selectedMediaAssetIds][0];
      const found = items.findIndex((item) => item.id === selected);
      const current = found >= 0 ? found : delta < 0 ? items.length : -1;
      const next = Math.max(0, Math.min(items.length - 1, current + delta));
      const mediaId = items[next]!.id;
      ui.focusPanel("media");
      ui.selectMediaAssets(new Set([mediaId]));
      useEditorUiStore.setState({ selectedFolderIds: new Set() });
      ui.setPreviewMedia(mediaId);
    };

    const handler = (e: KeyboardEvent) => {
      if (blockDuringUpdateInstallation(e)) return;
      const ui = useEditorUiStore.getState();
      const command = resolveDocumentedShortcut(e, context());
      const mod = e.metaKey || e.ctrlKey;
      const total = edit.currentTimelineEndFrame();

      // Zoom the timeline by `factor`, keeping the playhead stationary on screen
      // (剪映 zooms around the current position). Uses existing store actions.
      const zoomBy = (factor: number) => {
        const old = ui.zoomScale;
        const next = Math.max(ui.minZoomScale, Math.min(ZOOM.max, old * factor));
        if (next === old) return;
        ui.setZoomScale(next);
        // newScrollLeft keeps the playhead's screen x fixed: f*next - (f*old - left).
        const f = ui.activeFrame;
        ui.setScroll(Math.max(0, f * (next - old) + ui.scrollLeft), ui.scrollTop);
      };

      if (command) {
        e.preventDefault();
        // Frame/media navigation intentionally follows held-key repeats. Every
        // other command is a one-shot and is consumed without re-running.
        if (e.repeat && command.type !== "stepFrame" && command.type !== "moveMediaSelection") {
          return;
        }
        switch (command.type) {
          case "transport":
            // The capture listener owns Space so media/timeline playback toggles
            // before nested controls can consume it.
            return;
          case "stepFrame":
            ui.setCurrentFrame(Math.max(0, Math.min(total, ui.activeFrame + command.delta)));
            return;
          case "moveMediaSelection":
            moveMediaSelection(command.delta);
            return;
          case "delete":
            if (ui.focusedPanel === "media") {
              runApplicationMenuCommand("delete");
              return;
            }
            if (command.ripple) {
            // ⇧⌫ ripple-deletes (closes the gap). Route like upstream's
            // EditorWindowController: a selected gap closes first; else a marked
            // range on the selected clip's track; else the selected clips.
              if (ui.selectedGap) {
                void edit.rippleDeleteSelectedGap();
              } else {
                void (async () => {
                  if (!(await edit.rippleDeleteMarkedRange())) {
                    await edit.rippleDeleteSelectedClips();
                  }
                })();
              }
            } else {
              void edit.deleteSelectedClips();
            }
            return;
          case "setTool":
            ui.setToolMode(command.tool);
            return;
          case "markRange":
            if (command.edge === "start") ui.markRangeStart(Math.round(ui.activeFrame));
            else ui.markRangeEnd(Math.round(ui.activeFrame));
            return;
          case "trimStart":
            void edit.trimStartToPlayhead();
            return;
          case "trimEnd":
            void edit.trimEndToPlayhead();
            return;
          case "maximize":
            ui.toggleMaximizedFocusedPanel();
            return;
          case "mediaEnter": {
            const folder = [...ui.selectedFolderIds][0];
            if (folder) ui.setMediaPanelCurrentFolderId(folder);
            else {
              const media = [...ui.selectedMediaAssetIds][0];
              if (media) ui.setPreviewMedia(media);
            }
            return;
          }
          case "exitCrop":
            ui.setCropEditingActive(false);
            return;
          case "escape":
            if (ui.cropEditingActive) ui.setCropEditingActive(false);
            else if (ui.maximizedPanel) ui.setMaximizedPanel(null);
            else {
              ui.clearSelection();
              ui.clearTimelineRange();
              ui.setToolMode("pointer");
            }
            return;
          case "history":
            void (command.redo ? edit.redo() : edit.undo());
            return;
          case "clipboard":
            if (command.action === "copy") edit.copyClips();
            else if (command.action === "cut") void edit.cutClips();
            else if (command.action === "selectAll") runApplicationMenuCommand("selectAll");
            else if (!useClipboardStore.getState().hasContent) {
              ui.pushToast(t("edit.clipboardEmpty"));
            } else {
              void edit.pasteClipsAtPlayhead();
            }
            return;
          case "split":
            void edit.splitAtPlayhead().catch((error: unknown) => {
              const message = error instanceof Error ? error.message : String(error);
              ui.pushToast(`${t("toolbar.split")}: ${message}`);
            });
            return;
          case "application":
            runApplicationMenuCommand(command.id);
            return;
        }
      }

      // OpenTake extensions outside the pinned upstream table. These use the
      // same modal/editable/view boundary but stay visibly separate in code.
      if (
        context().view !== "editor" ||
        context().blocked ||
        isTextEntry(e.target) ||
        (!mod && isNativeInteractionControl(e.target))
      ) {
        return;
      }
      if (mod && !e.altKey && !e.shiftKey) {
        if (e.code === "Equal" || e.code === "NumpadAdd") {
          e.preventDefault();
          if (!e.repeat) zoomBy(ZOOM_KEY_STEP);
        } else if (e.code === "Minus" || e.code === "NumpadSubtract") {
          e.preventDefault();
          if (!e.repeat) zoomBy(1 / ZOOM_KEY_STEP);
        }
        return;
      }
      if (!mod && !e.altKey && (e.code === "Comma" || e.code === "Period")) {
        e.preventDefault();
        if (!e.repeat && !useProjectStore.getState().compatibilityReadOnly) {
          void edit.nudgeSelectedClips((e.code === "Comma" ? -1 : 1) * (e.shiftKey ? 5 : 1));
        }
      } else if (!mod && !e.altKey && e.shiftKey && e.code === "KeyZ") {
        e.preventDefault();
        if (!e.repeat) {
          ui.setZoomScale(ui.minZoomScale);
          ui.setScroll(0, ui.scrollTop);
        }
      }
    };

    window.addEventListener("keydown", handleSpaceKeyDown, true);
    window.addEventListener("keydown", handler);
    return () => {
      window.removeEventListener("keydown", handleSpaceKeyDown, true);
      window.removeEventListener("keydown", handler);
    };
  }, []);
}
