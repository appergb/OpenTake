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

interface MediaState {
  items: MediaItem[];
  /** All folders in the manifest. Empty when the project has no folders. */
  folders: MediaFolder[];
  importing: boolean;
  error: string | null;
  setItems: (items: MediaItem[]) => void;
  setFolders: (folders: MediaFolder[]) => void;
  setCatalog: (items: MediaItem[], folders: MediaFolder[]) => void;
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
  setCatalog: (items, folders) => set({ items, folders }),
  setImporting: (importing) => set({ importing }),
  setError: (error) => set({ error }),
}));

let started = false;
let unlisten: (() => void) | null = null;

/** Fetch the current catalog into the store. */
export async function refreshMedia(): Promise<void> {
  const list = await api.getMedia();
  useMediaStore.getState().setCatalog(list.items, list.folders ?? []);
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
  unlisten?.();
  unlisten = null;
  started = false;
}
