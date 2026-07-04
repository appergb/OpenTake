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
import { useEditorUiStore } from "./uiStore";
import { openDialog } from "../lib/dialog";
import { t } from "../i18n";
import type { MediaList } from "../lib/types";

/** Extensions the Rust importer accepts (mirrors the `session.rs` white-lists).
 *  These populate the native file-picker filter so the dialog surfaces the same
 *  formats the backend can decode; keep in sync with
 *  `crates/opentake-core/src/session.rs`. */
const VIDEO_EXTS = [
  "mov", "mp4", "m4v", "mkv", "webm", "avi", "mts", "m2ts", "mpg", "mpeg", "3gp", "wmv", "flv", "ts",
];
const AUDIO_EXTS = [
  "mp3", "wav", "aac", "m4a", "flac", "ogg", "opus", "aiff", "aif", "wma", "caf",
];
const IMAGE_EXTS = ["png", "jpg", "jpeg", "tiff", "heic", "webp", "bmp", "gif", "avif"];

function getErrorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}

/** Toast the count of files an import skipped as unsupported, if any (mirrors
 *  upstream `mediaPanelToast`). A no-op when nothing was skipped so a clean
 *  import stays quiet. */
function reportSkipped(list: MediaList): void {
  const skipped = list.skipped ?? [];
  if (skipped.length === 0) return;
  useEditorUiStore.getState().pushToast(t("media.importSkipped", { count: skipped.length }));
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
    const list = await api.importFolder(selected, true);
    await refreshMedia();
    reportSkipped(list);
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
    const list = await api.importMedia(paths);
    await refreshMedia();
    reportSkipped(list);
  } catch (error: unknown) {
    store.setError(getErrorMessage(error));
  } finally {
    store.setImporting(false);
  }
}
