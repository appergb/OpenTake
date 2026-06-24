/**
 * 全局库纯派生函数的回归测试(#56)。覆盖关键路径:
 * - 跨视图聚合:`all` 看到全部收藏。
 * - 音效/音频分流:同为 type==='audio',靠 category==='sound' 区分音效库与音频库。
 * - 内置分类按类型过滤、自建分类按 category 精确匹配。
 * - 搜索对文件名/分类大小写无关子串匹配。
 * - 三种排序(recent/oldest/type)。
 * 这些都是纯函数,不触 Tauri,直接断言。
 */
import { describe, expect, it } from "vitest";
import type { LibraryEntry } from "../lib/libraryApi";
import { deriveCustomCategories, selectEntries, sourceName } from "./libraryStore";

function entry(p: Partial<LibraryEntry> & Pick<LibraryEntry, "id" | "type" | "favoritedAt">): LibraryEntry {
  return { ...p };
}

const sample: LibraryEntry[] = [
  entry({ id: "v1", type: "video", favoritedAt: 30, source: "/x/intro.mp4" }),
  entry({ id: "a1", type: "audio", favoritedAt: 20, source: "/x/song.mp3" }),
  entry({ id: "s1", type: "audio", category: "sound", favoritedAt: 40, source: "/x/click.wav" }),
  entry({ id: "i1", type: "image", category: "Posters", favoritedAt: 10, source: "/x/poster.png" }),
  entry({ id: "e1", type: "effect", favoritedAt: 50, source: "/x/glow.json" }),
];

describe("selectEntries", () => {
  it("聚合视图 all 跨子库可见全部收藏", () => {
    const out = selectEntries(sample, "all", "", "recent");
    expect(out.map((e) => e.id).sort()).toEqual(["a1", "e1", "i1", "s1", "v1"]);
  });

  it("内置分类按素材类型过滤", () => {
    expect(selectEntries(sample, "video", "", "recent").map((e) => e.id)).toEqual(["v1"]);
    expect(selectEntries(sample, "image", "", "recent").map((e) => e.id)).toEqual(["i1"]);
    expect(selectEntries(sample, "effect", "", "recent").map((e) => e.id)).toEqual(["e1"]);
  });

  it("音效库仅含 category==='sound' 的音频,音频库排除它", () => {
    expect(selectEntries(sample, "sound", "", "recent").map((e) => e.id)).toEqual(["s1"]);
    expect(selectEntries(sample, "audio", "", "recent").map((e) => e.id)).toEqual(["a1"]);
  });

  it("自建分类按 category 精确匹配", () => {
    expect(selectEntries(sample, "Posters", "", "recent").map((e) => e.id)).toEqual(["i1"]);
  });

  it("搜索对文件名/分类大小写无关子串匹配", () => {
    expect(selectEntries(sample, "all", "SONG", "recent").map((e) => e.id)).toEqual(["a1"]);
    expect(selectEntries(sample, "all", "poster", "recent").map((e) => e.id)).toEqual(["i1"]);
  });

  it("排序 recent 降序、oldest 升序、type 字典序", () => {
    expect(selectEntries(sample, "all", "", "recent").map((e) => e.favoritedAt)).toEqual([
      50, 40, 30, 20, 10,
    ]);
    expect(selectEntries(sample, "all", "", "oldest").map((e) => e.favoritedAt)).toEqual([
      10, 20, 30, 40, 50,
    ]);
    expect(selectEntries(sample, "all", "", "type").map((e) => e.type)).toEqual([
      "audio",
      "audio",
      "effect",
      "image",
      "video",
    ]);
  });
});

describe("deriveCustomCategories", () => {
  it("聚合自建分类、去重、排除内置名", () => {
    const extra = [...sample, entry({ id: "i2", type: "image", category: "Posters", favoritedAt: 5 })];
    expect(deriveCustomCategories(extra)).toEqual(["Posters"]);
  });

  it("无自建分类时返回空", () => {
    const onlyBuiltin = sample.filter((e) => e.category !== "Posters");
    expect(deriveCustomCategories(onlyBuiltin)).toEqual([]);
  });
});

describe("sourceName", () => {
  it("取绝对路径文件名", () => {
    expect(sourceName("/a/b/c.mp4")).toBe("c.mp4");
    expect(sourceName(undefined)).toBe("");
  });
});
