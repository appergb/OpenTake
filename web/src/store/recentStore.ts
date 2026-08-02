/**
 * Home recent-project mirror. The native registry is authoritative in the
 * desktop app; localStorage remains a compatible startup cache for browser
 * previews and migrations from older OpenTake builds.
 */

import { create } from "zustand";
import { useProjectStore } from "./projectStore";

const LS_RECENTS = "recentProjects";
const MAX_RECENTS = 12;

export interface RecentProject {
  path: string;
  name: string;
  openedAt: number; // epoch ms
  createdAt?: number;
  modifiedAt?: number;
  thumbnailPath?: string | null;
  missing?: boolean;
}

function load(): RecentProject[] {
  if (typeof localStorage === "undefined") return [];
  try {
    const raw = localStorage.getItem(LS_RECENTS);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(
      (entry): entry is RecentProject =>
        !!entry &&
        typeof entry === "object" &&
        typeof (entry as RecentProject).path === "string",
    );
  } catch {
    return [];
  }
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
    };
    const next = [entry, ...get().recents.filter((recent) => recent.path !== path)].slice(
      0,
      MAX_RECENTS,
    );
    persist(next);
    set({ recents: next });
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
      entry.path === path ? { ...entry, modifiedAt, thumbnailPath } : entry
    ));
    persist(next);
    set({ recents: next });
  },
  remove: async (path) => {
    if (hasTauriRuntime()) {
      const { homeProjectRemove } = await import("../lib/api");
      await homeProjectRemove(path);
    }
    const next = removeLocal(path, get().recents);
    persist(next);
    set({ recents: next });

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
    set({ recents: next });

    const project = useProjectStore.getState();
    if (project.projectPath === path) {
      project.clearProjectSnapshot();
    }
  },
  validateRecents: async () => {
    const legacy = get().recents;
    if (!hasTauriRuntime()) return;
    try {
      const { homeProjectsSync } = await import("../lib/api");
      const native = await homeProjectsSync(
        legacy.map(({ path, openedAt, createdAt, modifiedAt, thumbnailPath }) => ({
          path,
          openedAt,
          createdAt,
          modifiedAt,
          thumbnailPath,
        })),
      );
      const recents = native.slice(0, MAX_RECENTS);
      persist(recents);
      set({ recents });
    } catch (error) {
      // A registry read failure must never silently erase a user's Home list.
      console.error("Failed to synchronize recent projects:", error);
    }
  },
}));
