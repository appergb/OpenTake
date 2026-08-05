import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { LibraryEntry } from "../../lib/libraryApi";
import {
  LibraryEntryGrid,
  LibrarySearchBox,
  libraryEntryPreviewSource,
} from "./LibraryView";

const entry: LibraryEntry = {
  id: "content-hash",
  type: "video",
  category: null,
  favoritedAt: 1,
  source: "/offline/original.mov",
  storedPath: "/global/library/content-hash.mov",
};

describe("LibraryEntryGrid", () => {
  it("gives the library search input a real target and accessible search semantics", () => {
    const html = renderToStaticMarkup(
      <LibrarySearchBox value="" onChange={() => {}} placeholder="Search assets" />,
    );

    expect(html).toContain('type="search"');
    expect(html).toContain('aria-label="Search assets"');
    expect(html).toContain("height:100%");
    expect(html).toContain("min-height:24px");
  });

  it("renders global-library entries in the reusable Mine grid", () => {
    const html = renderToStaticMarkup(
      <LibraryEntryGrid entries={[entry]} loading={false} totalEmpty={false} />,
    );

    expect(html).toContain("original.mov");
    expect(html).toContain('title="original.mov"');
  });

  it("never turns ambient library or source paths into preview authority", () => {
    expect(libraryEntryPreviewSource(entry)).toBeUndefined();
    expect(
      libraryEntryPreviewSource({ ...entry, thumb: "/global/library/thumb.jpg" }),
    ).toBeUndefined();
  });

  it("accepts only self-contained or browser-owned thumbnail URLs", () => {
    expect(libraryEntryPreviewSource({ ...entry, thumb: "data:image/png;base64,AA==" })).toBe(
      "data:image/png;base64,AA==",
    );
    expect(libraryEntryPreviewSource({ ...entry, thumb: "blob:https://example.test/id" })).toBe(
      "blob:https://example.test/id",
    );
  });
});
