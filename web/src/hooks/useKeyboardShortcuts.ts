/**
 * Global keyboard shortcuts (SPEC §9.6). Uses `event.code` for physical-key
 * parity with the upstream keyCodes. Skipped while a text input/textarea is
 * focused (SPEC §9.6 / trap #14). Cross-platform: ⌘ on macOS maps to Ctrl
 * elsewhere (metaKey || ctrlKey).
 */

import { useEffect } from "react";
import { useEditorUiStore } from "../store/uiStore";
import { useProjectStore } from "../store/projectStore";
import * as edit from "../store/editActions";
import { saveCurrentProject } from "../store/projectActions";

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

      // Cmd-modified actions.
      if (mod) {
        switch (e.code) {
          case "KeyZ":
            e.preventDefault();
            if (e.shiftKey) edit.redo();
            else edit.undo();
            return;
          case "KeyK":
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
        }
        return;
      }

      // Unmodified keys.
      switch (e.code) {
        case "Space":
          e.preventDefault();
          ui.setPlaying(!ui.isPlaying);
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
          edit.deleteSelectedClips();
          return;
        case "KeyC":
          ui.setToolMode("razor");
          return;
        case "KeyV":
          ui.setToolMode("pointer");
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
