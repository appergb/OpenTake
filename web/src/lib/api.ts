/**
 * Tauri bridge. All editing goes through `edit_apply`; the mirror is fetched via
 * `get_timeline` and refreshed on the `timeline_changed` event (SPEC §11).
 *
 * Degrades gracefully when not running inside Tauri (plain `vite dev` /
 * `vite preview` in a browser): `isTauri` is false and commands resolve against
 * a local in-memory fallback so the UI shell is still explorable. The real
 * editing truth always lives in Rust when running under Tauri.
 */

import type {
  EditRequest,
  EditResult,
  MediaList,
  SecretStatus,
  TimelineSnapshot,
} from "./types";

// Tauri injects `__TAURI_INTERNALS__` on the window when running in the shell.
export const isTauri =
  typeof window !== "undefined" &&
  "__TAURI_INTERNALS__" in (window as unknown as Record<string, unknown>);

type InvokeFn = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;
type ListenFn = (
  event: string,
  handler: (e: { payload: unknown }) => void,
) => Promise<() => void>;

let invokeImpl: InvokeFn | null = null;
let listenImpl: ListenFn | null = null;

async function ensureTauri(): Promise<void> {
  if (!isTauri || invokeImpl) return;
  const core = await import("@tauri-apps/api/core");
  const ev = await import("@tauri-apps/api/event");
  invokeImpl = core.invoke as InvokeFn;
  listenImpl = ev.listen as unknown as ListenFn;
}

// MARK: - Commands

export async function getTimeline(): Promise<TimelineSnapshot> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<TimelineSnapshot>("get_timeline");
  return fallback.getTimeline();
}

export async function editApply(command: EditRequest): Promise<EditResult> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<EditResult>("edit_apply", { command });
  return fallback.editApply(command);
}

export async function undo(): Promise<EditResult> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<EditResult>("undo");
  return fallback.noop("Undo");
}

export async function redo(): Promise<EditResult> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<EditResult>("redo");
  return fallback.noop("Redo");
}

export async function canUndo(): Promise<boolean> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<boolean>("can_undo");
  return false;
}

export async function canRedo(): Promise<boolean> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<boolean>("can_redo");
  return false;
}

export async function projectNew(): Promise<void> {
  await ensureTauri();
  if (invokeImpl) {
    await invokeImpl<void>("project_new");
    return;
  }
  fallback.reset();
}

export async function projectOpen(path: string): Promise<TimelineSnapshot> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<TimelineSnapshot>("project_open", { path });
  return fallback.getTimeline();
}

export async function projectSave(path: string | null): Promise<string> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<string>("project_save", { path });
  return path ?? "";
}

// MARK: - Media commands
//
// `import_folder` scans a directory for white-listed media and imports each;
// `import_media` imports an explicit file list; `get_media` returns the current
// catalog. All three are no-ops outside Tauri (no Rust core / no file system),
// returning an empty catalog so the browser shell degrades gracefully.

export async function importFolder(
  path: string,
  recursive = false,
): Promise<MediaList> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<MediaList>("import_folder", { path, recursive });
  return { items: [] };
}

export async function importMedia(paths: string[]): Promise<MediaList> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<MediaList>("import_media", { paths });
  return { items: [] };
}

export async function getMedia(): Promise<MediaList> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<MediaList>("get_media");
  return { items: [] };
}

// MARK: - BYOK secret store
//
// API keys are stored in the OS keychain by the Rust backend (`secret_*`
// commands wrapping `opentake-gen`'s `KeyringStore`). The plaintext key is sent
// only on save; every command returns a masked `SecretStatus`, so the key never
// lives in JS memory or localStorage. Outside Tauri there is no keychain, so the
// fallback reports "no key" — the form renders but cannot persist.

const NO_SECRET: SecretStatus = { hasKey: false, masked: "" };

export async function secretSave(
  provider: string,
  key: string,
): Promise<SecretStatus> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<SecretStatus>("secret_save", { provider, key });
  return NO_SECRET;
}

export async function secretLoad(provider: string): Promise<SecretStatus> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<SecretStatus>("secret_load", { provider });
  return NO_SECRET;
}

export async function secretDelete(provider: string): Promise<SecretStatus> {
  await ensureTauri();
  if (invokeImpl) return invokeImpl<SecretStatus>("secret_delete", { provider });
  return NO_SECRET;
}

// MARK: - Events

/** Subscribe to `timeline_changed`. Returns an unlisten function. No-op (and a
 *  no-op unlisten) when not in Tauri. */
export async function onTimelineChanged(
  handler: (version: number) => void,
): Promise<() => void> {
  await ensureTauri();
  if (!listenImpl) return () => {};
  return listenImpl("timeline_changed", (e) => {
    const payload = e.payload as { version?: number } | undefined;
    if (payload && typeof payload.version === "number") handler(payload.version);
  });
}

export async function onProjectOpened(
  handler: (path: string, version: number) => void,
): Promise<() => void> {
  await ensureTauri();
  if (!listenImpl) return () => {};
  return listenImpl("project_opened", (e) => {
    const p = e.payload as { path?: string; version?: number } | undefined;
    if (p) handler(p.path ?? "", p.version ?? 0);
  });
}

/** Subscribe to `media_changed` (manifest mutated by an import). The payload
 *  carries a version; the handler just needs to know it fired so it can re-fetch
 *  `get_media`. No-op outside Tauri. */
export async function onMediaChanged(handler: () => void): Promise<() => void> {
  await ensureTauri();
  if (!listenImpl) return () => {};
  return listenImpl("media_changed", () => handler());
}

// MARK: - Browser fallback (mirror, not authoritative)
//
// When running outside Tauri there is no Rust core; provide a small in-memory
// timeline so the shell renders something. This is intentionally minimal — it
// is a preview aid, not a second editing engine.

import { createFallbackStore } from "./fallback";
const fallback = createFallbackStore();
