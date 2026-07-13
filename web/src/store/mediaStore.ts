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

export const useMediaStore = create<MediaState>((set) => ({
  items: [],
  folders: [],
  importing: false,
  error: null,
  setItems: (items) => set({ items }),
  setFolders: (folders) => set({ folders }),
  setImporting: (importing) => set({ importing }),
  setError: (error) => set({ error }),
}));

let started = false;
let unlisten: (() => void) | null = null;
let refreshGeneration = 0;
let nextImportOperationId = 0;

export interface MediaProjectIdentity {
  projectEpoch: number;
  projectPath: string | null;
}

export interface MediaImportOperation {
  id: number;
  project: MediaProjectIdentity;
}

const activeImportOperations = new Map<number, MediaImportOperation>();

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

/** Apply a command-returned catalog only to the project that started it. Also
 * invalidates older in-flight refreshes so they cannot overwrite this result. */
export function applyMediaListForProject(
  project: MediaProjectIdentity,
  list: MediaList,
): boolean {
  if (!isCurrentMediaProject(project)) return false;
  refreshGeneration += 1;
  useMediaStore.setState(catalogState(list));
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
  return true;
}

/** Idempotent bootstrap: initial fetch + subscribe to `media_changed`. */
export async function startMediaSync(): Promise<void> {
  if (started) return;
  started = true;
  await refreshMedia();
  unlisten = await api.onMediaChanged(() => {
    void refreshMedia();
  });
}

export function stopMediaSync(): void {
  refreshGeneration += 1;
  unlisten?.();
  unlisten = null;
  started = false;
}
