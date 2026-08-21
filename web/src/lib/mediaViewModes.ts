import type { MediaFolder, MediaItem } from "./types";

export type MediaOrganizationMode = "folder" | "flat" | "grouped";

export interface MediaViewGroup {
  folderId: string | null;
  label: string;
  items: MediaItem[];
}

export interface MediaViewProjection {
  folders: MediaFolder[];
  items: MediaItem[];
  groups: MediaViewGroup[];
}

export interface ProjectMediaViewOptions {
  mode: MediaOrganizationMode;
  items: MediaItem[];
  folders: MediaFolder[];
  currentFolderId: string | null;
  query: string;
  typeFilter: MediaItem["type"] | "all";
  favoriteOnly: boolean;
}

const ROOT_GROUP_LABEL = "All";

export function normalizeFolderId(id: string | null | undefined): string | null {
  return id == null || id === "" ? null : id;
}

function matchesFilters(
  item: MediaItem,
  query: string,
  typeFilter: MediaItem["type"] | "all",
  favoriteOnly: boolean,
): boolean {
  if (favoriteOnly && !item.favorite) return false;
  if (typeFilter !== "all" && item.type !== typeFilter) return false;
  if (query === "") return true;
  return item.name.toLowerCase().includes(query);
}

function folderLabel(
  folderId: string,
  byId: ReadonlyMap<string, MediaFolder>,
): string {
  const parts: string[] = [];
  const visited = new Set<string>();
  let current: string | null = folderId;
  while (current !== null && !visited.has(current)) {
    visited.add(current);
    const folder = byId.get(current);
    if (!folder) break;
    parts.unshift(folder.name);
    current = normalizeFolderId(folder.parentFolderId);
  }
  return parts.join(" / ");
}

function groupedFolderId(
  item: MediaItem,
  byId: ReadonlyMap<string, MediaFolder>,
): string | null {
  const folderId = normalizeFolderId(item.folderId);
  if (folderId === null) return null;
  return byId.has(folderId) ? folderId : null;
}

export function projectMediaView({
  mode,
  items,
  folders,
  currentFolderId,
  query,
  typeFilter,
  favoriteOnly,
}: ProjectMediaViewOptions): MediaViewProjection {
  const trimmedQuery = query.trim().toLowerCase();
  const activeFolderId = normalizeFolderId(currentFolderId);
  const filteredItems = items.filter((item) =>
    matchesFilters(item, trimmedQuery, typeFilter, favoriteOnly),
  );

  if (mode === "flat") {
    return { folders: [], items: filteredItems, groups: [] };
  }

  if (mode === "grouped") {
    const folderById = new Map(folders.map((folder) => [folder.id, folder]));
    const groupsByFolder = new Map<string | null, MediaItem[]>();
    for (const item of filteredItems) {
      const folderId = groupedFolderId(item, folderById);
      const groupItems = groupsByFolder.get(folderId);
      if (groupItems) groupItems.push(item);
      else groupsByFolder.set(folderId, [item]);
    }
    const groups: MediaViewGroup[] = [];
    if (groupsByFolder.has(null)) {
      groups.push({
        folderId: null,
        label: ROOT_GROUP_LABEL,
        items: groupsByFolder.get(null) ?? [],
      });
    }
    for (const folder of folders) {
      const groupItems = groupsByFolder.get(folder.id);
      if (!groupItems || groupItems.length === 0) continue;
      groups.push({
        folderId: folder.id,
        label: folderLabel(folder.id, folderById) || folder.name,
        items: groupItems,
      });
    }
    return { folders: [], items: [], groups };
  }

  if (trimmedQuery !== "") {
    return { folders: [], items: filteredItems, groups: [] };
  }

  return {
    folders: folders.filter(
      (folder) => normalizeFolderId(folder.parentFolderId) === activeFolderId,
    ),
    items: filteredItems.filter(
      (item) => normalizeFolderId(item.folderId) === activeFolderId,
    ),
    groups: [],
  };
}
