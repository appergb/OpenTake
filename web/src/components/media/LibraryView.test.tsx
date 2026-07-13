import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { LibraryEntry } from "../../lib/libraryApi";
import { LibraryEntryGrid, libraryEntryPreviewSource } from "./LibraryView";

const entry: LibraryEntry = {
  id: "content-hash",
  type: "video",
  category: null,
  favoritedAt: 1,
  source: "/offline/original.mov",
  storedPath: "/global/library/content-hash.mov",
};

describe("LibraryEntryGrid", () => {
  it("renders global-library entries in the reusable Mine grid", () => {
    const html = renderToStaticMarkup(
      <LibraryEntryGrid entries={[entry]} loading={false} totalEmpty={false} />,
    );

    expect(html).toContain("original.mov");
    expect(html).toContain('title="original.mov"');
  });

  it("prefers the durable stored copy when no explicit thumbnail exists", () => {
    expect(libraryEntryPreviewSource(entry)).toBe("/global/library/content-hash.mov");
  });
});
