/**
 * Media-library mirror. Like the timeline mirror, the authoritative manifest
 * lives in Rust; the front end holds a read-only copy of the catalog returned by
 * `get_media` / `import_*` and re-fetches on the `media_changed` event. The
 * store also tracks an in-flight import flag and the last import error so the
 * panel can show progress / failure without each caller re-implementing it.
 */

import { create } from "zustand";
import * as api from "../lib/api";
import type { MediaFolder, MediaItem } from "../lib/types";
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

interface ProjectIdentity {
  projectEpoch: number;
  projectPath: string | null;
}

export interface MediaImportOperation {
  id: number;
  project: ProjectIdentity;
}

const activeImportOperations = new Map<number, MediaImportOperation>();

function captureProjectIdentity(): ProjectIdentity {
  const { projectEpoch, projectPath } = useProjectStore.getState();
  return { projectEpoch, projectPath };
}

function sameProject(left: ProjectIdentity, right: ProjectIdentity): boolean {
  return left.projectEpoch === right.projectEpoch && left.projectPath === right.projectPath;
}

function currentProjectHasActiveImport(): boolean {
  const current = captureProjectIdentity();
  return [...activeImportOperations.values()].some((operation) =>
    sameProject(operation.project, current),
  );
}

export function beginMediaImport(): MediaImportOperation {
  const operation = {
    id: ++nextImportOperationId,
    project: captureProjectIdentity(),
  };
  activeImportOperations.set(operation.id, operation);
  useMediaStore.getState().setImporting(true);
  return operation;
}

export function endMediaImport(operation: MediaImportOperation): void {
  if (!activeImportOperations.delete(operation.id)) return;
  useMediaStore.getState().setImporting(currentProjectHasActiveImport());
}

export function resetProjectMediaTransientState(): void {
  refreshGeneration += 1;
  activeImportOperations.clear();
  useMediaStore.setState({ importing: false, error: null });
}

/** Fetch the current catalog into the store (items + folder tree). */
export async function refreshMedia(): Promise<boolean> {
  const generation = ++refreshGeneration;
  const project = captureProjectIdentity();
  const list = await api.getMedia();
  if (generation !== refreshGeneration || !sameProject(project, captureProjectIdentity())) {
    return false;
  }
  // Dedup by id (#91-A4): a concurrent re-fetch can briefly surface duplicate
  // assets from overlapping snapshots; collapse by the authoritative item id so
  // the grid never renders the same asset twice (last wins, backend order kept).
  const byId = new Map(list.items.map((i) => [i.id, i] as const));
  useMediaStore.setState({ items: [...byId.values()], folders: list.folders });
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
