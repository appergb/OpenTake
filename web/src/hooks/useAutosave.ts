/**
 * Debounced autosave (issue #38). Whenever the document version advances past
 * the last persisted version, schedule a save-back to the open bundle
 * (`project_save(None)`). This is the safety net; Cmd/Ctrl+S forces an immediate
 * save and the window-close handler flushes a final write in Rust.
 *
 * No-op outside Tauri (no bundle on disk) or when no project is open (Home).
 */

import { useEffect, useRef } from "react";
import { isTauri } from "../lib/api";
import { useProjectStore } from "../store/projectStore";
import { saveCurrentProject } from "../store/projectActions";

const AUTOSAVE_DEBOUNCE_MS = 1500;

export function useAutosave() {
  const version = useProjectStore((s) => s.timelineVersion);
  const projectPath = useProjectStore((s) => s.projectPath);
  const timer = useRef<number | null>(null);

  useEffect(() => {
    if (!isTauri || !projectPath) return;
    if (version === useProjectStore.getState().lastSavedVersion) return;
    if (timer.current !== null) window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => {
      void saveCurrentProject();
    }, AUTOSAVE_DEBOUNCE_MS);
    return () => {
      if (timer.current !== null) window.clearTimeout(timer.current);
    };
  }, [version, projectPath]);
}
