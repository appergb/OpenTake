import { describe, expect, it } from "vitest";
import type { MediaFolder, MediaItem } from "./types";
import {
  projectMediaView,
  type MediaOrganizationMode,
} from "./mediaViewModes";

function mediaItem(
  id: string,
  overrides: Partial<MediaItem> = {},
): MediaItem {
  return {
    id,
    name: id,
    type: "video",
    duration: 1,
    hasAudio: false,
    favorite: false,
    ...overrides,
  };
}

function project(
  mode: MediaOrganizationMode,
  overrides: Partial<Parameters<typeof projectMediaView>[0]> = {},
) {
  const folders: MediaFolder[] = [
    { id: "folder-interviews", name: "Interviews" },
    { id: "folder-broll", name: "B-roll" },
    { id: "folder-guest", name: "Guest", parentFolderId: "folder-interviews" },
    { id: "folder-loop", name: "Loop", parentFolderId: "folder-loop" },
    { id: "folder-empty", name: "Empty" },
  ];
  const items: MediaItem[] = [
    mediaItem("root-a-roll", { folderId: null }),
    mediaItem("interview-host", { folderId: "folder-interviews" }),
    mediaItem("guest-closeup", { folderId: "folder-guest", favorite: true }),
    mediaItem("broll-city", { folderId: "folder-broll", type: "image" }),
    mediaItem("loop-asset", { folderId: "folder-loop", type: "audio", favorite: true }),
  ];
  return projectMediaView({
    mode,
    items,
    folders,
    currentFolderId: null,
    query: "",
    typeFilter: "all",
    favoriteOnly: false,
    ...overrides,
  });
}

describe("projectMediaView", () => {
  it("projects folder mode to direct child folders and direct child items only", () => {
    const rootProjection = project("folder");
    expect(rootProjection.folders.map((folder) => folder.id)).toEqual([
      "folder-interviews",
      "folder-broll",
      "folder-empty",
    ]);
    expect(rootProjection.items.map((item) => item.id)).toEqual(["root-a-roll"]);
    expect(rootProjection.groups).toEqual([]);

    const nestedProjection = project("folder", { currentFolderId: "folder-interviews" });
    expect(nestedProjection.folders.map((folder) => folder.id)).toEqual(["folder-guest"]);
    expect(nestedProjection.items.map((item) => item.id)).toEqual(["interview-host"]);
    expect(nestedProjection.groups).toEqual([]);
  });

  it("projects flat mode to every filtered asset regardless of folder depth", () => {
    const projection = project("flat");
    expect(projection.folders).toEqual([]);
    expect(projection.groups).toEqual([]);
    expect(projection.items.map((item) => item.id)).toEqual([
      "root-a-roll",
      "interview-host",
      "guest-closeup",
      "broll-city",
      "loop-asset",
    ]);
  });

  it("projects grouped mode with root and nested path labels while omitting empty groups", () => {
    const projection = project("grouped");
    expect(projection.folders).toEqual([]);
    expect(projection.items).toEqual([]);
    expect(
      projection.groups.map((group) => ({
        folderId: group.folderId,
        label: group.label,
        items: group.items.map((item) => item.id),
      })),
    ).toEqual([
      {
        folderId: null,
        label: "All",
        items: ["root-a-roll"],
      },
      {
        folderId: "folder-interviews",
        label: "Interviews",
        items: ["interview-host"],
      },
      {
        folderId: "folder-broll",
        label: "B-roll",
        items: ["broll-city"],
      },
      {
        folderId: "folder-guest",
        label: "Interviews / Guest",
        items: ["guest-closeup"],
      },
      {
        folderId: "folder-loop",
        label: "Loop",
        items: ["loop-asset"],
      },
    ]);
  });

  it("applies search, type, and favorite filters consistently across every mode", () => {
    for (const mode of ["folder", "flat", "grouped"] as const) {
      const searchProjection = project(mode, { query: "guest" });
      expect(searchProjection.folders).toEqual([]);
      expect(searchProjection.items.map((item) => item.id)).toEqual(
        mode === "grouped" ? [] : ["guest-closeup"],
      );
      expect(searchProjection.groups.map((group) => group.items.map((item) => item.id))).toEqual(
        mode === "grouped" ? [["guest-closeup"]] : [],
      );

      const typeProjection = project(mode, { typeFilter: "audio" });
      expect(typeProjection.folders.map((folder) => folder.id)).toEqual(
        mode === "folder" ? ["folder-interviews", "folder-broll", "folder-empty"] : [],
      );
      expect(typeProjection.items.map((item) => item.id)).toEqual(
        mode === "flat" ? ["loop-asset"] : [],
      );
      expect(typeProjection.groups.map((group) => group.items.map((item) => item.id))).toEqual(
        mode === "grouped" ? [["loop-asset"]] : [],
      );

      const favoriteProjection = project(mode, { favoriteOnly: true });
      expect(favoriteProjection.folders.map((folder) => folder.id)).toEqual(
        mode === "folder" ? ["folder-interviews", "folder-broll", "folder-empty"] : [],
      );
      expect(favoriteProjection.items.map((item) => item.id)).toEqual(
        mode === "flat" ? ["guest-closeup", "loop-asset"] : [],
      );
      expect(
        favoriteProjection.groups.map((group) => ({
          folderId: group.folderId,
          items: group.items.map((item) => item.id),
        })),
      ).toEqual(
        mode === "grouped"
          ? [
              { folderId: "folder-guest", items: ["guest-closeup"] },
              { folderId: "folder-loop", items: ["loop-asset"] },
            ]
          : [],
      );
    }
  });

  it("never mutates the input arrays while projecting", () => {
    const folders: MediaFolder[] = [
      { id: "folder-a", name: "A" },
      { id: "folder-b", name: "B", parentFolderId: "folder-a" },
    ];
    const items: MediaItem[] = [
      mediaItem("clip-a", { folderId: null }),
      mediaItem("clip-b", { folderId: "folder-b", favorite: true }),
    ];
    const folderSnapshot = folders.map((folder) => ({ ...folder }));
    const itemSnapshot = items.map((item) => ({ ...item }));

    projectMediaView({
      mode: "grouped",
      items,
      folders,
      currentFolderId: "folder-a",
      query: "clip",
      typeFilter: "all",
      favoriteOnly: false,
    });

    expect(folders).toEqual(folderSnapshot);
    expect(items).toEqual(itemSnapshot);
  });
});
