/**
 * Home recent-project mirror. The native registry is authoritative in the
 * desktop app; localStorage remains a compatible startup cache for browser
 * previews and migrations from older OpenTake builds.
 */

import { create } from "zustand";
import { useProjectStore } from "./projectStore";

const LS_RECENTS = "recentProjects";
const MAX_RECENTS = 12;
const MAX_RECENT_PATH_CHARS = 32_768;
const MAX_RECENT_NAME_CHARS = 512;
let validationInFlight: Promise<void> | null = null;

export interface RecentProject {
  path: string;
  name: string;
  openedAt: number; // epoch ms
  createdAt?: number;
  modifiedAt?: number;
  thumbnailPath?: string | null;
  missing?: boolean;
  offline?: boolean;
}

function finiteTimestamp(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) && value >= 0
    ? value
    : undefined;
}

/** Decode the localStorage startup cache without trusting its size or fields. */
export function decodeRecentProjects(raw: string | null): RecentProject[] {
  try {
    if (!raw) return [];
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) return [];
    const recents: RecentProject[] = [];
    const seen = new Set<string>();
    for (const candidate of parsed) {
      if (recents.length >= MAX_RECENTS) break;
      if (!candidate || typeof candidate !== "object") continue;
      const source = candidate as Partial<RecentProject>;
      const path = source.path;
      if (
        typeof path !== "string"
        || path.length === 0
        || path.length > MAX_RECENT_PATH_CHARS
        || path.includes("\0")
        || !/\.opentake$/i.test(path)
        || seen.has(path)
      ) {
        continue;
      }
      seen.add(path);
      const expectedThumbnail = projectThumbnailPath(path);
      const thumbnailPath = typeof source.thumbnailPath === "string"
        && source.thumbnailPath.length <= MAX_RECENT_PATH_CHARS
        && source.thumbnailPath === expectedThumbnail
        ? source.thumbnailPath
        : null;
      const openedAt = finiteTimestamp(source.openedAt) ?? 0;
      recents.push({
        path,
        name: projectNameFromPath(path).slice(0, MAX_RECENT_NAME_CHARS),
        openedAt,
        createdAt: finiteTimestamp(source.createdAt),
        modifiedAt: finiteTimestamp(source.modifiedAt),
        thumbnailPath,
        missing: source.missing === true,
        offline: source.offline === true,
      });
    }
    return recents;
  } catch {
    return [];
  }
}

function load(): RecentProject[] {
  if (typeof localStorage === "undefined") return [];
  return decodeRecentProjects(localStorage.getItem(LS_RECENTS));
}

function persist(list: RecentProject[]) {
  if (typeof localStorage !== "undefined") {
    localStorage.setItem(LS_RECENTS, JSON.stringify(list));
  }
}

/** Derive a display name from a bundle path (its last path segment, minus the
 *  `.opentake` extension). */
export function projectNameFromPath(path: string): string {
  const segment = path.split(/[\\/]/).filter(Boolean).pop() ?? path;
  return segment.replace(/\.opentake$/i, "");
}

interface RecentState {
  recents: RecentProject[];
  /** In-memory user mutations made while a native snapshot is in flight. */
  mutationRevision: number;
  /** Native metadata has approved every thumbnail path for asset-protocol use. */
  thumbnailPathsValidated: boolean;
  add: (path: string) => void;
  markSaved: (path: string, modifiedAt?: number, thumbnailPath?: string | null) => void;
  remove: (path: string) => Promise<void>;
  reveal: (path: string) => Promise<void>;
  trash: (path: string) => Promise<void>;
  validateRecents: () => Promise<void>;
}

function removeLocal(path: string, recents: RecentProject[]): RecentProject[] {
  return recents.filter((entry) => entry.path !== path);
}

function hasTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export function projectThumbnailPath(path: string): string {
  const separator = path.includes("\\") && !path.includes("/") ? "\\" : "/";
  return `${path.replace(/[\\/]+$/, "")}${separator}thumbnail.jpg`;
}

export const useRecentStore = create<RecentState>((set, get) => ({
  recents: load(),
  mutationRevision: 0,
  // In the desktop shell, localStorage is only an untrusted startup cache. Its
  // thumbnail paths must never reach the main-thread asset protocol until the
  // native registry has classified them as local, resident regular files.
  thumbnailPathsValidated: !hasTauriRuntime(),
  add: (path) => {
    const openedAt = Date.now();
    const existing = get().recents.find((entry) => entry.path === path);
    const entry: RecentProject = {
      path,
      name: projectNameFromPath(path),
      openedAt,
      createdAt: existing?.createdAt ?? openedAt,
      modifiedAt: existing?.modifiedAt ?? openedAt,
      thumbnailPath: existing?.thumbnailPath ?? null,
      missing: false,
      offline: false,
    };
    const next = [entry, ...get().recents.filter((recent) => recent.path !== path)].slice(
      0,
      MAX_RECENTS,
    );
    persist(next);
    set({ recents: next, mutationRevision: get().mutationRevision + 1 });
    if (hasTauriRuntime()) {
      void import("../lib/api")
        .then(({ homeProjectRegister }) => homeProjectRegister(path, openedAt))
        .catch((error) => {
          console.error("Failed to persist recent project registration:", error);
        });
    }
  },
  markSaved: (path, modifiedAt = Date.now(), thumbnailPath = projectThumbnailPath(path)) => {
    const next = get().recents.map((entry) => (
      entry.path === path
        ? { ...entry, modifiedAt, thumbnailPath, missing: false, offline: false }
        : entry
    ));
    persist(next);
    set({ recents: next, mutationRevision: get().mutationRevision + 1 });
  },
  remove: async (path) => {
    if (hasTauriRuntime()) {
      const { homeProjectRemove } = await import("../lib/api");
      await homeProjectRemove(path);
    }
    const next = removeLocal(path, get().recents);
    persist(next);
    set({ recents: next, mutationRevision: get().mutationRevision + 1 });

    const project = useProjectStore.getState();
    if (project.projectPath === path) {
      project.clearProjectSnapshot();
    }
  },
  reveal: async (path) => {
    if (!hasTauriRuntime()) throw new Error("Reveal in file manager requires the desktop app");
    const { homeProjectReveal } = await import("../lib/api");
    await homeProjectReveal(path);
  },
  trash: async (path) => {
    if (!hasTauriRuntime()) throw new Error("Move to trash requires the desktop app");
    const { homeProjectTrash } = await import("../lib/api");
    await homeProjectTrash(path);
    const next = removeLocal(path, get().recents);
    persist(next);
    set({ recents: next, mutationRevision: get().mutationRevision + 1 });

    const project = useProjectStore.getState();
    if (project.projectPath === path) {
      project.clearProjectSnapshot();
    }
  },
  validateRecents: () => {
    if (!hasTauriRuntime()) return Promise.resolve();
    if (validationInFlight) return validationInFlight;

    const operation = (async () => {
      try {
        const { homeProjectsSync } = await import("../lib/api");
        for (;;) {
          const snapshot = get();
          const native = await homeProjectsSync(
            snapshot.recents.map(({ path, openedAt, createdAt, modifiedAt, thumbnailPath }) => ({
              path,
              openedAt,
              createdAt,
              modifiedAt,
              thumbnailPath,
            })),
          );
          // If the user changed the list while this request was in flight,
          // resynchronize the newest state instead of publishing stale data.
          if (snapshot.mutationRevision !== get().mutationRevision) continue;
          const recents = native.slice(0, MAX_RECENTS);
          persist(recents);
          set({ recents, thumbnailPathsValidated: true });
          return;
        }
      } catch (error) {
        // A registry read failure must never silently erase a user's Home list.
        console.error("Failed to synchronize recent projects:", error);
      }
    })();
    validationInFlight = operation.finally(() => {
      validationInFlight = null;
    });
    return validationInFlight;
  },
}));
