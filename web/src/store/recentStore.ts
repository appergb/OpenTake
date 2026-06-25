/**
 * Recent-projects list for the Home view. Stores the absolute paths of recently
 * opened `.opentake` bundles (most-recent first, capped) in localStorage — a
 * front-end-only convenience that mirrors upstream `ProjectRegistry`'s recents,
 * without the on-disk thumbnail/metadata index (a later concern).
 */

import { create } from "zustand";
import { useProjectStore } from "./projectStore";

const LS_RECENTS = "recentProjects";
const MAX_RECENTS = 12;

export interface RecentProject {
  path: string;
  name: string;
  openedAt: number; // epoch ms
}

function load(): RecentProject[] {
  if (typeof localStorage === "undefined") return [];
  try {
    const raw = localStorage.getItem(LS_RECENTS);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(
      (e): e is RecentProject =>
        !!e && typeof e === "object" && typeof (e as RecentProject).path === "string",
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
  remove: (path: string) => void;
  validateRecents: () => Promise<void>;
}

export const useRecentStore = create<RecentState>((set, get) => ({
  recents: load(),
  add: (path) => {
    const entry: RecentProject = {
      path,
      name: projectNameFromPath(path),
      openedAt: Date.now(),
    };
    const next = [entry, ...get().recents.filter((r) => r.path !== path)].slice(0, MAX_RECENTS);
    persist(next);
    set({ recents: next });
  },
  remove: (path) => {
    const next = get().recents.filter((r) => r.path !== path);
    persist(next);
    set({ recents: next });

    // Clear active project from memory if it matches the removed path
    const projStore = useProjectStore.getState();
    if (projStore.projectPath === path) {
      projStore.setProjectPath(null);
      projStore.setMirror({
        fps: 30,
        width: 1920,
        height: 1080,
        settingsConfigured: false,
        tracks: [],
      }, 0);
    }
  },
  validateRecents: async () => {
    const list = get().recents;
    if (list.length === 0) return;

    const { isTauri } = await import("../lib/api");
    if (!isTauri) return;

    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const valid: RecentProject[] = [];
      for (const entry of list) {
        try {
          const exists = await invoke<boolean>("check_path_exists", { path: entry.path });
          if (exists) {
            valid.push(entry);
          }
        } catch {
          // If we fail to check, fallback to keeping the project
          valid.push(entry);
        }
      }
      if (valid.length !== list.length) {
        persist(valid);
        set({ recents: valid });
      }
    } catch (e) {
      console.error("Failed to validate recents:", e);
    }
  },
}));
