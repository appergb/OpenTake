import {
  captureProjectEditIdentity,
  deleteFolder as deleteFolderAction,
  deleteMedia as deleteMediaAction,
} from "./editActions";
import {
  isCurrentMediaProject,
  type MediaProjectIdentity,
} from "./mediaStore";
import { useProjectStore } from "./projectStore";
import { previewMediaTabId, useEditorUiStore } from "./uiStore";

type MediaDeleteKind = "folder" | "media";

const pendingDeletes = new Map<string, Promise<void>>();

function uniqueIds(ids: Iterable<string>): string[] {
  return [...new Set(ids)].filter((id) => id !== "");
}

function transactionKey(
  project: ReturnType<typeof captureProjectEditIdentity>,
  kind: MediaDeleteKind,
  ids: readonly string[],
): string {
  return JSON.stringify([
    project.projectEpoch,
    project.projectPath,
    project.timelineVersion,
    kind,
    [...ids].sort(),
  ]);
}

async function runDeleteTransaction(
  kind: MediaDeleteKind,
  ids: Iterable<string>,
): Promise<void> {
  const targets = uniqueIds(ids);
  if (targets.length === 0) return;
  const expected = captureProjectEditIdentity();
  const project: MediaProjectIdentity = {
    projectEpoch: expected.projectEpoch,
    projectPath: expected.projectPath,
  };
  const key = transactionKey(expected, kind, targets);
  const existing = pendingDeletes.get(key);
  if (existing) return existing;

  const operation = (async () => {
    const result =
      kind === "media"
        ? await deleteMediaAction(targets, expected)
        : await deleteFolderAction(targets, expected);
    if (!isCurrentMediaProject(project)) return;
    if (result) {
      const currentVersion = useProjectStore.getState().timelineVersion;
      if (
        currentVersion !== expected.timelineVersion &&
        currentVersion !== result.timelineVersion
      ) {
        return;
      }
    }

    const deleted = new Set(targets);
    if (kind === "media") {
      useEditorUiStore.setState((latest) => ({
        selectedMediaAssetIds: new Set(
          [...latest.selectedMediaAssetIds].filter((id) => !deleted.has(id)),
        ),
      }));
      const ui = useEditorUiStore.getState();
      if (ui.previewTabIds.length === 0 && ui.previewMediaId && deleted.has(ui.previewMediaId)) {
        ui.setPreviewMedia(null);
      } else {
        for (const mediaId of ui.previewTabIds.filter((id) => deleted.has(id))) {
          ui.closePreviewTab(previewMediaTabId(mediaId));
        }
      }
    } else {
      useEditorUiStore.setState((latest) => ({
        selectedFolderIds: new Set(
          [...latest.selectedFolderIds].filter((id) => !deleted.has(id)),
        ),
      }));
    }
  })();
  pendingDeletes.set(key, operation);
  try {
    await operation;
  } finally {
    if (pendingDeletes.get(key) === operation) pendingDeletes.delete(key);
  }
}

/** Keyboard and application-menu deletion is selection-only. */
export function deleteSelectedMediaAssets(): Promise<void> {
  return runDeleteTransaction(
    "media",
    useEditorUiStore.getState().selectedMediaAssetIds,
  );
}

/** An unselected context-menu target deletes itself without replacing selection. */
export function deleteMediaFromContextMenu(mediaId: string): Promise<void> {
  const selected = useEditorUiStore.getState().selectedMediaAssetIds;
  return runDeleteTransaction("media", selected.has(mediaId) ? selected : [mediaId]);
}

export function deleteSelectedFolders(): Promise<void> {
  return runDeleteTransaction("folder", useEditorUiStore.getState().selectedFolderIds);
}

export function deleteFolderFromContextMenu(folderId: string): Promise<void> {
  const selected = useEditorUiStore.getState().selectedFolderIds;
  return runDeleteTransaction("folder", selected.has(folderId) ? selected : [folderId]);
}
