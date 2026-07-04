/**
 * Shared drag state for "media panel → timeline" drags. HTML5 DnD forbids
 * reading `dataTransfer.getData()` during `dragover` (only on `drop`), so the
 * timeline cannot recover the dragged asset's duration/type from the event while
 * painting the drop ghost. The media card stashes the item here on `dragStart`;
 * the timeline reads it on `dragOver` to size the ghost, and both clear it when
 * the gesture ends. Module-level (not a store) so reads/writes never re-render.
 */

import type { MediaItem } from "./types";

let dragging: MediaItem | null = null;

/** Record the item being dragged from the media panel (or clear with `null`). */
export function setDraggingMedia(item: MediaItem | null): void {
  dragging = item;
}

/** The media item currently dragged from the panel, or `null` when none. */
export function getDraggingMedia(): MediaItem | null {
  return dragging;
}
