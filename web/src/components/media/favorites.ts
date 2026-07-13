import * as api from "../../lib/api";
import type { FavoriteSyncFailure, MediaList } from "../../lib/types";
import {
  applyMediaErrorForProject,
  applyMediaListForProject,
  isCurrentMediaProject,
  type MediaProjectIdentity,
} from "../../store/mediaStore";

const LS_FAVORITES = "opentake.favorites";

function loadLegacyFavorites(): Set<string> {
  if (typeof localStorage === "undefined") return new Set();
  try {
    const raw = localStorage.getItem(LS_FAVORITES);
    if (!raw) return new Set();
    const parsed: unknown = JSON.parse(raw);
    return Array.isArray(parsed)
      ? new Set(parsed.filter((value): value is string => typeof value === "string"))
      : new Set();
  } catch {
    return new Set();
  }
}

function removeMigratedLegacyIds(ids: ReadonlyArray<string>): void {
  if (ids.length === 0 || typeof localStorage === "undefined") return;
  const migrated = new Set(ids);
  const remaining = [...loadLegacyFavorites()].filter((id) => !migrated.has(id));
  if (remaining.length === 0) localStorage.removeItem(LS_FAVORITES);
  else localStorage.setItem(LS_FAVORITES, JSON.stringify(remaining));
}

export interface FavoriteMigrationOutcome {
  synced: boolean;
  media?: MediaList;
  failures: FavoriteSyncFailure[];
}

const completedProjects = new Set<string>();
const inFlight = new Map<string, Promise<FavoriteMigrationOutcome>>();
const retryAfter = new Map<string, number>();
const RETRY_BACKOFF_MS = 250;

function projectKey(project: MediaProjectIdentity): string {
  return JSON.stringify([project.projectEpoch, project.projectPath]);
}

export function applyFavoriteMigrationOutcome(
  project: MediaProjectIdentity,
  outcome: FavoriteMigrationOutcome,
): boolean {
  if (!isCurrentMediaProject(project)) return false;
  if (outcome.media && !applyMediaListForProject(project, outcome.media)) return false;
  if (outcome.failures.length > 0) {
    applyMediaErrorForProject(
      project,
      outcome.failures.map((failure) => `${failure.assetId}: ${failure.message}`).join("; "),
    );
  }
  return true;
}

/** Reconcile the current project's persisted favorite mirrors with the global
 * library once per project identity. Only localStorage ids belonging to the
 * current project are sent, and only backend-confirmed ids are removed. */
export function migrateLocalFavorites(
  items: ReadonlyArray<{ id: string }>,
  project: MediaProjectIdentity,
): Promise<FavoriteMigrationOutcome> {
  if (!isCurrentMediaProject(project)) {
    return Promise.resolve({ synced: false, failures: [] });
  }
  const key = projectKey(project);
  if (completedProjects.has(key)) {
    return Promise.resolve({ synced: false, failures: [] });
  }
  if ((retryAfter.get(key) ?? 0) > Date.now()) {
    return Promise.resolve({ synced: false, failures: [] });
  }
  const existing = inFlight.get(key);
  if (existing) return existing;

  const legacyIds = loadLegacyFavorites();
  // Project state is published before its media mirror finishes loading. Do
  // not mark this epoch complete from that transient empty mirror, otherwise
  // legacy ids belonging to the project would never be offered to Rust.
  if (items.length === 0 && legacyIds.size > 0) {
    return Promise.resolve({ synced: false, failures: [] });
  }
  const projectIds = new Set(items.map((item) => item.id));
  const matchingLegacyIds = [...legacyIds].filter((id) => projectIds.has(id));
  const operation = api
    .syncProjectFavorites(matchingLegacyIds, project)
    .then((result): FavoriteMigrationOutcome => {
      if (!isCurrentMediaProject(project)) {
        return { synced: false, failures: [] };
      }
      removeMigratedLegacyIds(result.migratedLegacyAssetIds);
      if (result.failures.length === 0) {
        retryAfter.delete(key);
        completedProjects.add(key);
      } else {
        retryAfter.set(key, Date.now() + RETRY_BACKOFF_MS);
      }
      return { synced: true, media: result.media, failures: result.failures };
    })
    .finally(() => {
      inFlight.delete(key);
    });
  inFlight.set(key, operation);
  return operation;
}
