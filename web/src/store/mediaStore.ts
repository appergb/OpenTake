/**
 * Media-library mirror. Like the timeline mirror, the authoritative manifest
 * lives in Rust; the front end holds a read-only copy of the catalog returned by
 * `get_media` / `import_*` and re-fetches on the `media_changed` event. The
 * store also tracks an in-flight import flag and the last import error so the
 * panel can show progress / failure without each caller re-implementing it.
 */

import { create } from "zustand";
import * as api from "../lib/api";
import type { MediaFolder, MediaItem, MediaList } from "../lib/types";
import { useProjectStore } from "./projectStore";
import { useEditorUiStore } from "./uiStore";

interface MediaState {
  items: MediaItem[];
  /** Library folders (flat list; nest via `parentFolderId`). Drives the
   *  CapCut-style folder browser in the media panel. */
  folders: MediaFolder[];
  importing: boolean;
  error: string | null;
  setItems: (items: MediaItem[]) => void;
  setFolders: (folders: MediaFolder[]) => void;
  setImporting: (importing: boolean) => void;
  setError: (error: string | null) => void;
}

type MediaErrorOwner =
  | { channel: "operation"; token: number }
  | { channel: "sync"; requestToken: number; refreshGeneration: number };

let mediaErrorOwner: MediaErrorOwner | null = null;
let nextOperationErrorToken = 0;
let nextSyncRequestToken = 0;
let latestSyncRequestToken = 0;

export const useMediaStore = create<MediaState>((set) => ({
  items: [],
  folders: [],
  importing: false,
  error: null,
  setItems: (items) => set({ items }),
  setFolders: (folders) => set({ folders }),
  setImporting: (importing) => set({ importing }),
  setError: (error) => {
    mediaErrorOwner =
      error === null
        ? null
        : { channel: "operation", token: ++nextOperationErrorToken };
    set({ error });
  },
}));

let started = false;
let unlisten: (() => void) | null = null;
let refreshGeneration = 0;
let lifecycleGeneration = 0;
let nextImportOperationId = 0;
const MAX_EVENT_REFRESH_ATTEMPTS = 2;

export interface MediaProjectIdentity {
  projectEpoch: number;
  projectPath: string | null;
}

export interface MediaImportOperation {
  id: number;
  project: MediaProjectIdentity;
}

const activeImportOperations = new Map<number, MediaImportOperation>();

function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  return String(error);
}

async function convergeMediaEvent(lifecycleActive: () => boolean): Promise<void> {
  const requestToken = ++nextSyncRequestToken;
  latestSyncRequestToken = requestToken;
  let lastError: unknown;
  for (let attempt = 0; attempt < MAX_EVENT_REFRESH_ATTEMPTS; attempt += 1) {
    if (!lifecycleActive() || requestToken !== latestSyncRequestToken) return;
    try {
      const applied = await refreshMedia();
      if (!lifecycleActive() || requestToken !== latestSyncRequestToken) return;
      if (applied) return;
      lastError = new Error(
        "media mirror refresh was superseded before convergence",
      );
    } catch (error) {
      lastError = error;
    }
  }
  if (!lifecycleActive() || requestToken !== latestSyncRequestToken) return;
  const message = errorMessage(lastError);
  if (mediaErrorOwner?.channel !== "operation") {
    mediaErrorOwner = {
      channel: "sync",
      requestToken,
      refreshGeneration,
    };
    useMediaStore.setState({ error: message });
  }
  useEditorUiStore
    .getState()
    .pushToast(`媒体事件同步失败 / Media event sync failed: ${message}`);
}

export function captureMediaProjectIdentity(): MediaProjectIdentity {
  const { projectEpoch, projectPath } = useProjectStore.getState();
  return { projectEpoch, projectPath };
}

function sameProject(left: MediaProjectIdentity, right: MediaProjectIdentity): boolean {
  return left.projectEpoch === right.projectEpoch && left.projectPath === right.projectPath;
}

export function isCurrentMediaProject(project: MediaProjectIdentity): boolean {
  return sameProject(project, captureMediaProjectIdentity());
}

function catalogState(list: MediaList): Pick<MediaState, "items" | "folders"> {
  const byId = new Map(list.items.map((item) => [item.id, item] as const));
  return { items: [...byId.values()], folders: list.folders };
}

function clearOlderSyncError(refreshToken: number): void {
  if (
    mediaErrorOwner?.channel === "sync" &&
    mediaErrorOwner.refreshGeneration <= refreshToken
  ) {
    mediaErrorOwner = null;
    useMediaStore.setState({ error: null });
  }
}

/** Apply a command-returned catalog only to the project that started it. Also
 * invalidates older in-flight refreshes so they cannot overwrite this result. */
export function applyMediaListForProject(
  project: MediaProjectIdentity,
  list: MediaList,
): boolean {
  if (!isCurrentMediaProject(project)) return false;
  const generation = ++refreshGeneration;
  useMediaStore.setState(catalogState(list));
  clearOlderSyncError(generation);
  return true;
}

export function applyMediaErrorForProject(
  project: MediaProjectIdentity,
  error: string,
): boolean {
  if (!isCurrentMediaProject(project)) return false;
  useMediaStore.getState().setError(error);
  return true;
}

function currentProjectHasActiveImport(): boolean {
  const current = captureMediaProjectIdentity();
  return [...activeImportOperations.values()].some((operation) =>
    sameProject(operation.project, current),
  );
}

export function beginMediaImport(): MediaImportOperation {
  const operation = {
    id: ++nextImportOperationId,
    project: captureMediaProjectIdentity(),
  };
  activeImportOperations.set(operation.id, operation);
  useMediaStore.getState().setImporting(true);
  return operation;
}

export function endMediaImport(operation: MediaImportOperation): void {
  if (!activeImportOperations.delete(operation.id)) return;
  useMediaStore.getState().setImporting(currentProjectHasActiveImport());
}

export function resetProjectMediaState(): void {
  refreshGeneration += 1;
  latestSyncRequestToken = ++nextSyncRequestToken;
  mediaErrorOwner = null;
  activeImportOperations.clear();
  useMediaStore.setState({ items: [], folders: [], importing: false, error: null });
}

/** Fetch the current catalog into the store (items + folder tree). */
export async function refreshMedia(): Promise<boolean> {
  const generation = ++refreshGeneration;
  const project = captureMediaProjectIdentity();
  const list = await api.getMedia();
  if (generation !== refreshGeneration || !isCurrentMediaProject(project)) {
    return false;
  }
  // Dedup by id (#91-A4): a concurrent re-fetch can briefly surface duplicate
  // assets from overlapping snapshots; collapse by the authoritative item id so
  // the grid never renders the same asset twice (last wins, backend order kept).
  useMediaStore.setState(catalogState(list));
  clearOlderSyncError(generation);
  return true;
}

/** Idempotent bootstrap: initial fetch + subscribe to `media_changed`. */
export async function startMediaSync(): Promise<void> {
  if (started) return;
  started = true;
  const generation = ++lifecycleGeneration;
  const lifecycleActive = () => started && generation === lifecycleGeneration;
  try {
    await refreshMedia();
    if (!lifecycleActive()) return;
    const registeredUnlisten = await api.onMediaChanged(() => {
      if (!lifecycleActive()) return;
      return convergeMediaEvent(lifecycleActive);
    });
    if (!lifecycleActive()) {
      registeredUnlisten();
      return;
    }
    unlisten = registeredUnlisten;
    // Close the fetch-before-subscribe window so a media mutation that occurred
    // during listener registration cannot leave the catalog permanently stale.
    await refreshMedia();
    if (!lifecycleActive()) return;
  } catch (error) {
    if (generation === lifecycleGeneration) {
      lifecycleGeneration += 1;
      refreshGeneration += 1;
      unlisten?.();
      unlisten = null;
      started = false;
    }
    throw error;
  }
}

export function stopMediaSync(): void {
  lifecycleGeneration += 1;
  refreshGeneration += 1;
  latestSyncRequestToken = ++nextSyncRequestToken;
  unlisten?.();
  unlisten = null;
  started = false;
}
