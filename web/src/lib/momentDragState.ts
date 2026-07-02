/**
 * Shared drag state for "search Moments/Spoken hit → timeline" drags. A search
 * hit drags onto the timeline as a *trimmed* source-range clip (only the shot /
 * spoken segment lands), mirroring upstream's `assetDragString(forAssetId:
 * segment:)`. The hit still uses {@link MEDIA_DND_TYPE} so the existing timeline
 * drop machinery (ghost sizing, track resolution) works unchanged; this module
 * stashes the source-second range the drop reads to place a trimmed clip instead
 * of the whole asset. Module-level (not a store) so reads/writes never re-render.
 *
 * Cleared whenever the gesture ends (drop or a plain media-card drag starting).
 */

import type { SourceRange } from "../store/editActions";

let range: SourceRange | null = null;

/** Record the source-second range being dragged from a search hit (or clear
 *  with `null`). A still image (no range) simply never sets this. */
export function setDraggingMomentRange(next: SourceRange | null): void {
  range = next;
}

/** The source-second range of the search hit currently dragged, or `null` when
 *  the active drag is a plain full-asset drag (or none). */
export function getDraggingMomentRange(): SourceRange | null {
  return range;
}
