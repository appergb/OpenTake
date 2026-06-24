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
import { t } from "../i18n";
import * as edit from "../store/editActions";
import { saveCurrentProject } from "../store/projectActions";
import { ZOOM } from "../lib/theme";

/** Per-keypress zoom step for ⌘+ / ⌘- (剪映: Cmd + +/-). */
const ZOOM_KEY_STEP = 1.3;

function isTextEntry(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || target.isContentEditable;
}

export function useKeyboardShortcuts() {
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (isTextEntry(e.target)) return;
      const ui = useEditorUiStore.getState();
      // Editor-only shortcuts: ignore while the Home / Settings views are shown.
      if (ui.view !== "editor") return;
      const mod = e.metaKey || e.ctrlKey;
      const total = (() => {
        const tl = useProjectStore.getState().timeline;
        let m = 0;
        for (const t of tl.tracks)
          for (const c of t.clips) m = Math.max(m, c.startFrame + c.durationFrames);
        return m;
      })();

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

      // Cmd-modified actions.
      if (mod) {
        switch (e.code) {
          case "KeyZ":
            e.preventDefault();
            if (e.shiftKey) edit.redo();
            else edit.undo();
            return;
          // ⌘+ / ⌘- zoom in/out (剪映 Cmd + +/-). "Equal" is the +/= key.
          case "Equal":
          case "NumpadAdd":
            e.preventDefault();
            zoomBy(ZOOM_KEY_STEP);
            return;
          case "Minus":
          case "NumpadSubtract":
            e.preventDefault();
            zoomBy(1 / ZOOM_KEY_STEP);
            return;
          case "KeyK":
          case "KeyB":
            // ⌘K (existing) and ⌘B (剪映 split-at-playhead) both split.
            e.preventDefault();
            edit.splitAtPlayhead();
            return;
          case "KeyS":
            e.preventDefault();
            void saveCurrentProject();
            return;
          case "Digit1":
            e.preventDefault();
            ui.setLayoutPreset("default");
            return;
          case "Digit2":
            e.preventDefault();
            ui.setLayoutPreset("media");
            return;
          case "Digit3":
            e.preventDefault();
            ui.setLayoutPreset("vertical");
            return;
          case "Digit0":
            e.preventDefault();
            if (e.altKey) ui.toggleInspectorPanel();
            else ui.toggleMediaPanel();
            return;
          case "KeyA":
            if (e.altKey) {
              e.preventDefault();
              ui.toggleAgentPanel();
              return;
            }
            return;
          case "KeyC":
            e.preventDefault();
            edit.copyClips();
            return;
          case "KeyX":
            e.preventDefault();
            void edit.cutClips();
            return;
          case "KeyV":
            e.preventDefault();
            if (!useClipboardStore.getState().hasContent) {
              useEditorUiStore.getState().pushToast(t("edit.clipboardEmpty"));
              return;
            }
            void edit.pasteClipsAtPlayhead();
            return;
        }
        return;
      }

      // Unmodified keys.
      switch (e.code) {
        case "Space":
          e.preventDefault();
          if (ui.previewMediaId) {
            ui.requestMediaPreviewToggle();
          } else {
            ui.togglePlay(); // rewinds from the parked end frame on replay
          }
          return;
        case "ArrowLeft":
          e.preventDefault();
          ui.setCurrentFrame(Math.max(0, ui.activeFrame - (e.shiftKey ? 5 : 1)));
          return;
        case "ArrowRight":
          e.preventDefault();
          ui.setCurrentFrame(Math.min(total, ui.activeFrame + (e.shiftKey ? 5 : 1)));
          return;
        case "Backspace":
        case "Delete":
          e.preventDefault();
          // ⇧⌫ ripple-deletes (closes the gap); plain ⌫ lifts out (leaves a gap).
          if (e.shiftKey) edit.rippleDeleteSelectedClips();
          else edit.deleteSelectedClips();
          return;
        case "KeyQ":
          // 剪映 Q：删除播放头左侧（修剪入点到播放头）。
          e.preventDefault();
          edit.trimStartToPlayhead();
          return;
        case "KeyW":
          // 剪映 W：删除播放头右侧（修剪出点到播放头）。
          e.preventDefault();
          edit.trimEndToPlayhead();
          return;
        case "KeyC":
        case "KeyB":
          // C (existing) and B (剪映 切割模式) both enter the razor/blade tool.
          ui.setToolMode("razor");
          return;
        case "KeyV":
        case "KeyA":
          // V (existing) and A (剪映 选择模式) both return to the pointer tool.
          ui.setToolMode("pointer");
          return;
        case "KeyZ":
          // ⇧Z fits the whole timeline to the window (剪映 Shift+Z 适配窗口).
          if (e.shiftKey) {
            e.preventDefault();
            ui.setZoomScale(ui.minZoomScale);
            ui.setScroll(0, ui.scrollTop);
          }
          return;
        case "Backquote": {
          e.preventDefault();
          const focused = ui.focusedPanel;
          ui.setMaximizedPanel(ui.maximizedPanel ? null : focused);
          return;
        }
        case "Escape":
          if (ui.maximizedPanel) ui.setMaximizedPanel(null);
          else {
            ui.clearSelection();
            ui.setToolMode("pointer");
          }
          return;
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);
}
