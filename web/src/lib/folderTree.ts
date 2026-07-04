/**
 * Pure helpers for the media panel's CapCut-style folder browser (#49/#58). The
 * backend returns a flat `MediaFolder[]` (each nesting via `parentFolderId`); the
 * panel walks it to render one level at a time plus a breadcrumb trail. Keeping
 * the traversal here (data-only, no React) makes it unit-testable and keeps
 * `MediaPanel` to rendering.
 */

import type { MediaFolder } from "./types";

/** Normalize a folder ref so absent / null / "" all collapse to `null` (= root). */
export function normalizeFolderId(id: string | null | undefined): string | null {
  return id ?? null;
}

/** Direct children of `parentId` (null = root), in their manifest order. */
export function childFolders(folders: MediaFolder[], parentId: string | null): MediaFolder[] {
  return folders.filter((f) => normalizeFolderId(f.parentFolderId) === parentId);
}

/**
 * Build the breadcrumb trail from the root down to `folderId` by walking the
 * `parentFolderId` chain. Returns ancestors-first; an empty array means root.
 * Guards against cycles and dangling parent refs so a corrupt manifest can never
 * loop or throw — it just stops at the first break.
 */
export function folderTrail(folders: MediaFolder[], folderId: string | null): MediaFolder[] {
  if (folderId === null) return [];
  const byId = new Map(folders.map((f) => [f.id, f]));
  const trail: MediaFolder[] = [];
  const seen = new Set<string>();
  let current: string | null = folderId;
  while (current !== null && !seen.has(current)) {
    seen.add(current);
    const folder = byId.get(current);
    if (!folder) break;
    trail.unshift(folder);
    current = normalizeFolderId(folder.parentFolderId);
  }
  return trail;
}
