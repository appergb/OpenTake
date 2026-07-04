/**
 * folderTree 单测：CapCut 式文件夹浏览的纯遍历逻辑。覆盖根/嵌套、parentFolderId
 * 的 null/缺省归一、面包屑链路，以及坏数据（环、悬挂父引用）的健壮性。
 */
import { describe, expect, it } from "vitest";
import type { MediaFolder } from "./types";
import { childFolders, folderTrail, normalizeFolderId } from "./folderTree";

const f = (id: string, parentFolderId: string | null | undefined): MediaFolder => ({
  id,
  name: id.toUpperCase(),
  parentFolderId,
});

describe("normalizeFolderId", () => {
  it("collapses null / undefined to null", () => {
    expect(normalizeFolderId(null)).toBeNull();
    expect(normalizeFolderId(undefined)).toBeNull();
  });

  it("passes a real id through unchanged", () => {
    expect(normalizeFolderId("abc")).toBe("abc");
  });
});

describe("childFolders", () => {
  // root: trip / lib ; trip > day1 ; day1 > morning
  const folders: MediaFolder[] = [
    f("trip", null),
    f("lib", undefined),
    f("day1", "trip"),
    f("morning", "day1"),
  ];

  it("returns top-level folders for the root (null) — null and undefined parents both count", () => {
    expect(childFolders(folders, null).map((x) => x.id)).toEqual(["trip", "lib"]);
  });

  it("returns only the direct children of a folder, not grandchildren", () => {
    expect(childFolders(folders, "trip").map((x) => x.id)).toEqual(["day1"]);
    expect(childFolders(folders, "day1").map((x) => x.id)).toEqual(["morning"]);
  });

  it("returns an empty array for a leaf folder", () => {
    expect(childFolders(folders, "morning")).toEqual([]);
  });

  it("preserves manifest order", () => {
    const reversed: MediaFolder[] = [f("b", null), f("a", null)];
    expect(childFolders(reversed, null).map((x) => x.id)).toEqual(["b", "a"]);
  });
});

describe("folderTrail", () => {
  const folders: MediaFolder[] = [
    f("trip", null),
    f("day1", "trip"),
    f("morning", "day1"),
  ];

  it("returns an empty trail at the root", () => {
    expect(folderTrail(folders, null)).toEqual([]);
  });

  it("walks ancestors-first from a nested folder to the root", () => {
    expect(folderTrail(folders, "morning").map((x) => x.id)).toEqual(["trip", "day1", "morning"]);
  });

  it("returns a single segment for a top-level folder", () => {
    expect(folderTrail(folders, "trip").map((x) => x.id)).toEqual(["trip"]);
  });

  it("stops at a dangling parent reference instead of throwing", () => {
    const orphan: MediaFolder[] = [f("child", "ghost")];
    expect(folderTrail(orphan, "child").map((x) => x.id)).toEqual(["child"]);
  });

  it("breaks a parent cycle instead of looping forever", () => {
    // a -> b -> a (corrupt). Starting at "a" must terminate.
    const cyclic: MediaFolder[] = [f("a", "b"), f("b", "a")];
    const trail = folderTrail(cyclic, "a");
    expect(trail.length).toBeLessThanOrEqual(2);
    expect(trail.some((x) => x.id === "a")).toBe(true);
  });

  it("returns an empty trail when the cursor id is unknown", () => {
    expect(folderTrail(folders, "nope")).toEqual([]);
  });
});
