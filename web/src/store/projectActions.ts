/**
 * Project lifecycle gestures driven from the Home view. "New" starts a fresh
 * session and enters the editor; "Open" picks an `.opentake` bundle (a directory
 * on disk) via the native dialog, opens it in the core, records it in recents,
 * and enters the editor. All paths degrade gracefully outside Tauri so the
 * browser shell can still navigate into the editor.
 */

import * as api from "../lib/api";
import { forceRefresh } from "./sync";
import { useEditorUiStore } from "./uiStore";
import { useProjectStore } from "./projectStore";
import { useRecentStore } from "./recentStore";
import { refreshMedia, resetProjectMediaState } from "./mediaStore";
import { openDialog, saveDialog } from "../lib/dialog";
import { t } from "../i18n";
import { stopNativePlaybackForProjectBoundary } from "../components/preview/nativePlaybackSession";

const PROJECT_EXT = "opentake";

/** Ensure a chosen path carries the `.opentake` bundle extension. */
function withExt(path: string): string {
  return path.endsWith(`.${PROJECT_EXT}`) ? path : `${path}.${PROJECT_EXT}`;
}

/**
 * New project. Mirrors upstream `AppState.createNewProject` (`NSSavePanel`):
 * prompt for a save location + name (default `~/Documents/OpenTake`), then
 * create the session and **immediately write the `.opentake` bundle to disk** so
 * the project has a real location (the user's complaint was "new project can't
 * choose where it saves"). Records it in recents and enters the editor.
 *
 * Outside Tauri (browser shell) there is no save panel — fall back to a fresh
 * in-memory session so the UI is still explorable.
 */
export async function newProjectAndEnter(): Promise<void> {
  try {
    const save = await saveDialog();
    if (!save) {
      await stopNativePlaybackForProjectBoundary();
      const snapshot = await api.projectNew(null);
      useProjectStore.getState().replaceProjectSnapshot(snapshot);
      resetProjectMediaState();
      await forceRefresh();
      useEditorUiStore.getState().resetProjectRuntimeState();
      useEditorUiStore.getState().setView("editor");
      return;
    }

    const defaultDir = await api.getDefaultProjectDir().catch(() => "");
    const sep = defaultDir && !defaultDir.endsWith("/") ? "/" : "";
    const defaultPath = defaultDir
      ? `${defaultDir}${sep}${t("home.untitled")}.${PROJECT_EXT}`
      : undefined;

    const chosen = await save({
      title: t("home.newProject"),
      defaultPath,
      filters: [{ name: "OpenTake", extensions: [PROJECT_EXT] }],
    });
    if (typeof chosen !== "string") return; // cancelled

    const requestedPath = withExt(chosen);
    await stopNativePlaybackForProjectBoundary();
    // The desktop command persists a separate fresh session first and only
    // replaces the live project after that bundle can be reopened. A failed
    // initial save therefore leaves the current project and UI untouched.
    const snapshot = await api.projectNew(requestedPath);
    useProjectStore.getState().replaceProjectSnapshot(snapshot);
    resetProjectMediaState();
    await forceRefresh();
    const committedPath = snapshot.projectPath ?? requestedPath;
    useProjectStore.getState().markSaved();
    useRecentStore.getState().add(committedPath);
    useEditorUiStore.getState().resetProjectRuntimeState();
    useEditorUiStore.getState().setView("editor");
  } catch (error) {
    useEditorUiStore
      .getState()
      .pushToast(t("project.createFailed", { error: projectLifecycleErrorMessage(error) }));
    throw error;
  }
}

interface SaveSnapshot {
  snapshotMutationRevision: number;
  projectEpoch: number;
  projectPath: string;
  timelineVersion: number;
}

let saveInFlight: Promise<void> | null = null;
let activeSaveSnapshot: SaveSnapshot | null = null;
let queuedExplicitSave: SaveSnapshot | null = null;

function captureSaveSnapshot(): SaveSnapshot | null {
  const current = useProjectStore.getState();
  if (!current.projectPath) return null;
  return {
    snapshotMutationRevision: current.snapshotMutationRevision,
    projectEpoch: current.projectEpoch,
    projectPath: current.projectPath,
    timelineVersion: current.timelineVersion,
  };
}

function sameSnapshot(left: SaveSnapshot, right: SaveSnapshot): boolean {
  return (
    left.snapshotMutationRevision === right.snapshotMutationRevision &&
    left.projectEpoch === right.projectEpoch &&
    left.projectPath === right.projectPath &&
    left.timelineVersion === right.timelineVersion
  );
}

function sameProject(snapshot: SaveSnapshot): boolean {
  const current = useProjectStore.getState();
  return (
    current.projectEpoch === snapshot.projectEpoch && current.projectPath === snapshot.projectPath
  );
}

function currentProjectNeedsSave(): boolean {
  const current = useProjectStore.getState();
  return Boolean(current.projectPath) && current.timelineVersion !== current.lastSavedVersion;
}

async function runSaveCoordinator(): Promise<void> {
  while (true) {
    const explicitRequest = queuedExplicitSave;
    queuedExplicitSave = null;
    const snapshot = captureSaveSnapshot();
    if (!snapshot) return;
    if (explicitRequest && !sameProject(explicitRequest)) return;
    activeSaveSnapshot = snapshot;

    try {
      await api.projectSave(null);
    } catch (error) {
      activeSaveSnapshot = null;
      const afterFailure = captureSaveSnapshot();
      const failureIsCurrent = Boolean(afterFailure && sameSnapshot(snapshot, afterFailure));
      if (failureIsCurrent) {
        const message = error instanceof Error ? error.message : String(error);
        useEditorUiStore.getState().pushToast(t("project.saveFailed", { error: message }));
      }
      if (queuedExplicitSave) continue;
      if (failureIsCurrent) return;
      if (sameProject(snapshot) && currentProjectNeedsSave()) continue;
      return;
    }

    activeSaveSnapshot = null;
    const after = useProjectStore.getState();
    if (
      sameProject(snapshot) &&
      after.snapshotMutationRevision === snapshot.snapshotMutationRevision &&
      after.timelineVersion === snapshot.timelineVersion
    ) {
      after.markSaved(snapshot.timelineVersion);
    }
    if (queuedExplicitSave) continue;
    if (sameProject(snapshot) && currentProjectNeedsSave()) continue;
    return;
  }
}

/**
 * Save the open project back to its bundle (`project_save(None)`). Used by the
 * Cmd/Ctrl+S shortcut and the debounced autosave. Concurrent triggers share one
 * coordinator; if the document advances while a save is in flight, one fresh
 * save follows before the new version can be marked persisted. Completions are
 * bound to the initiating project identity so an old project cannot mark or
 * toast a newly opened project.
 */
export function saveCurrentProject(): Promise<void> {
  const request = captureSaveSnapshot();
  if (!request) return Promise.resolve();
  if (saveInFlight) {
    if (!activeSaveSnapshot || !sameSnapshot(request, activeSaveSnapshot)) {
      queuedExplicitSave = request;
    }
    return saveInFlight;
  }
  queuedExplicitSave = request;
  const run = runSaveCoordinator();
  const tracked = run.finally(() => {
    if (saveInFlight === tracked) saveInFlight = null;
  });
  saveInFlight = tracked;
  return tracked;
}

/** Save the current project to a newly chosen `.opentake` bundle and adopt that
 *  path as the live session. The core performs an atomic Save As (including
 *  project-local media) and only changes its retained root after publication
 *  succeeds; the front-end mirrors the returned canonical path afterwards. */
export async function saveCurrentProjectAs(): Promise<void> {
  const project = useProjectStore.getState();
  if (!project.projectPath || project.compatibilityReadOnly) return;
  try {
    const save = await saveDialog();
    if (!save) return;
    const selected = await save({
      title: t("menu.saveAs"),
      defaultPath: project.projectPath,
      filters: [{ name: "OpenTake", extensions: [PROJECT_EXT] }],
    });
    if (typeof selected !== "string") return;
    const committedPath = await api.projectSave(withExt(selected));
    const current = useProjectStore.getState();
    current.setProjectPath(committedPath);
    useProjectStore.getState().markSaved();
    useRecentStore.getState().add(committedPath);
  } catch (error) {
    useEditorUiStore.getState().pushToast(
      t("project.saveFailed", { error: projectLifecycleErrorMessage(error) }),
    );
    throw error;
  }
}

/** Open `path` (a `.opentake` bundle), refresh the mirror, record it, and enter
 *  the editor. Used by both the dialog flow and the recents list. */
function projectLifecycleErrorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "object" && error !== null && "message" in error) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === "string") return message;
  }
  return String(error);
}

export async function openProjectPath(path: string): Promise<void> {
  await stopNativePlaybackForProjectBoundary();
  let snap: Awaited<ReturnType<typeof api.projectOpen>>;
  try {
    snap = await api.projectOpen(path);
  } catch (error) {
    const message = projectLifecycleErrorMessage(error);
    useEditorUiStore.getState().pushToast(t("project.openFailed", { error: message }));
    throw error;
  }
  useProjectStore.getState().replaceProjectSnapshot(snap);
  resetProjectMediaState();
  useProjectStore.getState().markSaved();
  if (snap.projectPath) useRecentStore.getState().add(snap.projectPath);
  await refreshMedia();
  useEditorUiStore.getState().resetProjectRuntimeState();
  useEditorUiStore.getState().setView("editor");
}

/** Pick a project bundle with the native dialog, then open it. `.opentake`
 *  bundles are directories, so the picker is a directory chooser (mirrors
 *  upstream's package-as-folder open panel). */
export async function openProjectViaDialog(): Promise<void> {
  let delegatedToProjectOpen = false;
  try {
    const open = await openDialog();
    if (!open) {
      // Browser shell: no file system. Just enter the editor on the demo mirror.
      useEditorUiStore.getState().setView("editor");
      return;
    }
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected !== "string") return; // cancelled
    delegatedToProjectOpen = true;
    await openProjectPath(selected);
  } catch (error) {
    // openProjectPath reports its own downstream failures. Dialog acquisition
    // and picker failures happen before delegation, so report them here.
    if (!delegatedToProjectOpen) {
      useEditorUiStore.getState().pushToast(
        t("project.openFailed", { error: projectLifecycleErrorMessage(error) }),
      );
    }
    throw error;
  }
}
