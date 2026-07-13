import * as api from "../../lib/api";
import type { FavoriteSyncFailure, MediaList } from "../../lib/types";

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

const completedEpochs = new Set<number>();
const inFlight = new Map<number, Promise<FavoriteMigrationOutcome>>();

/** Reconcile the current project's persisted favorite mirrors with the global
 * library once per project epoch. Only localStorage ids belonging to the
 * current project are sent, and only backend-confirmed ids are removed. */
export function migrateLocalFavorites(
  items: ReadonlyArray<{ id: string }>,
  projectEpoch: number,
): Promise<FavoriteMigrationOutcome> {
  if (completedEpochs.has(projectEpoch)) {
    return Promise.resolve({ synced: false, failures: [] });
  }
  const existing = inFlight.get(projectEpoch);
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
    .syncProjectFavorites(matchingLegacyIds)
    .then((result): FavoriteMigrationOutcome => {
      removeMigratedLegacyIds(result.migratedLegacyAssetIds);
      completedEpochs.add(projectEpoch);
      return { synced: true, media: result.media, failures: result.failures };
    })
    .finally(() => {
      inFlight.delete(projectEpoch);
    });
  inFlight.set(projectEpoch, operation);
  return operation;
}
