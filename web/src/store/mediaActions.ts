/**
 * Media import gestures (CapCut-style). The Import button opens a native dialog
 * (tauri-plugin-dialog) to pick either a folder (`directory: true`) or one/many
 * files (`multiple: true`), then routes the selection to the Rust import
 * commands. Rust emits `media_changed`, which the media mirror listens for and
 * re-fetches — so these actions only need to start the import and surface
 * progress / errors; they never mutate the catalog directly.
 *
 * Outside Tauri the dialog plugin is unavailable; the actions degrade to no-ops
 * so the browser shell never throws.
 */

import * as api from "../lib/api";
import { useMediaStore, refreshMedia } from "./mediaStore";
import { useSettingsStore } from "./settingsStore";
import { openDialog } from "../lib/dialog";

/** Extensions the Rust importer accepts (mirrors `session.rs` white-lists). */
const VIDEO_EXTS = ["mov", "mp4", "m4v"];
const AUDIO_EXTS = ["mp3", "wav", "aac", "m4a"];
const IMAGE_EXTS = ["png", "jpg", "jpeg", "tiff", "heic", "webp"];

function getErrorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}

/** Pick a folder and import every supported file inside it. */
export async function importFolderViaDialog(): Promise<void> {
  const open = await openDialog();
  if (!open) return;
  const store = useMediaStore.getState();
  store.setError(null);
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      defaultPath: useSettingsStore.getState().defaultImportFolder ?? undefined,
    });
    if (typeof selected !== "string") return; // cancelled
    store.setImporting(true);
    await api.importFolder(selected, false);
    await refreshMedia();
  } catch (error: unknown) {
    store.setError(getErrorMessage(error));
  } finally {
    store.setImporting(false);
  }
}

/**
 * Relink an offline asset: pick the file it should now point at and hand it to
 * the Rust `relink_media` command, which keeps the SAME asset id so every clip
 * referencing it recovers (re-importing would mint a new id and strand them).
 * Rust emits `media_changed`; we also refresh so the offline wash clears at once.
 */
export async function relinkMediaViaDialog(mediaRef: string): Promise<void> {
  const open = await openDialog();
  if (!open) return;
  const store = useMediaStore.getState();
  store.setError(null);
  try {
    const selected = await open({
      directory: false,
      multiple: false,
      defaultPath: useSettingsStore.getState().defaultImportFolder ?? undefined,
      filters: [
        { name: "Media", extensions: [...VIDEO_EXTS, ...AUDIO_EXTS, ...IMAGE_EXTS] },
      ],
    });
    if (typeof selected !== "string") return; // cancelled
    await api.relinkMedia(mediaRef, selected);
    await refreshMedia();
  } catch (error: unknown) {
    store.setError(getErrorMessage(error));
  }
}

/** Pick one or more media files and import them. */
export async function importFilesViaDialog(): Promise<void> {
  const open = await openDialog();
  if (!open) return;
  const store = useMediaStore.getState();
  store.setError(null);
  try {
    const selected = await open({
      directory: false,
      multiple: true,
      defaultPath: useSettingsStore.getState().defaultImportFolder ?? undefined,
      filters: [
        { name: "Media", extensions: [...VIDEO_EXTS, ...AUDIO_EXTS, ...IMAGE_EXTS] },
      ],
    });
    const paths = Array.isArray(selected) ? selected : selected ? [selected] : [];
    if (paths.length === 0) return; // cancelled
    store.setImporting(true);
    await api.importMedia(paths);
    await refreshMedia();
  } catch (error: unknown) {
    store.setError(getErrorMessage(error));
  } finally {
    store.setImporting(false);
  }
}
