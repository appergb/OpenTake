// @vitest-environment happy-dom

import React from "react";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { LibraryEntry } from "../../lib/libraryApi";
import { useLibraryStore } from "../../store/libraryStore";
import { textContrastRatio } from "../../../test/contrast";
import {
  LibraryEntryCard,
  LibraryEntryGrid,
  LibrarySearchBox,
  libraryEntryPreviewSource,
} from "./LibraryView";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

let root: Root | null = null;
let container: HTMLDivElement | null = null;

const originalActions = {
  importToProject: useLibraryStore.getState().importToProject,
  categorize: useLibraryStore.getState().categorize,
  unfavorite: useLibraryStore.getState().unfavorite,
};

afterEach(async () => {
  if (root) await act(async () => root?.unmount());
  container?.remove();
  root = null;
  container = null;
  useLibraryStore.setState(originalActions);
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

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

  it("keeps loading, empty, and no-match information at WCAG AA text contrast", async () => {
    container = document.createElement("div");
    document.body.append(container);
    root = createRoot(container);

    for (const props of [
      { loading: true, totalEmpty: true },
      { loading: false, totalEmpty: true },
      { loading: false, totalEmpty: false },
    ]) {
      await act(async () =>
        root?.render(<LibraryEntryGrid entries={[]} {...props} />),
      );
      const empty = container.firstElementChild as HTMLElement | null;
      expect(empty).not.toBeNull();
      expect(textContrastRatio(empty!.style.color, "var(--bg-surface)")).toBeGreaterThanOrEqual(4.5);
    }
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

describe("LibraryEntryCard keyboard actions", () => {
  it("keeps core actions focusable without hover, reveals them on focus, and executes them", async () => {
    const importToProject = vi.fn().mockResolvedValue(null);
    const categorize = vi.fn().mockResolvedValue(undefined);
    const unfavorite = vi.fn().mockResolvedValue(undefined);
    useLibraryStore.setState({ importToProject, categorize, unfavorite });
    vi.stubGlobal("prompt", vi.fn().mockReturnValue("Review"));

    container = document.createElement("div");
    document.body.append(container);
    root = createRoot(container);
    await act(async () => root?.render(<LibraryEntryCard entry={entry} />));

    const actions = [...container.querySelectorAll<HTMLButtonElement>("button")];
    expect(actions).toHaveLength(3);
    expect(actions.every((button) => button.tabIndex >= 0 && !button.disabled)).toBe(true);

    const actionRow = actions[0]?.parentElement;
    expect(actionRow?.style.opacity).toBe("0");
    await act(async () => actions[0]?.focus());
    expect(actionRow?.style.opacity).toBe("1");

    await act(async () => actions[0]?.click());
    expect(importToProject).toHaveBeenCalledWith(entry.id);

    await act(async () => actions[1]?.click());
    expect(categorize).toHaveBeenCalledWith(entry.id, "Review");

    await act(async () => actions[2]?.click());
    expect(unfavorite).toHaveBeenCalledWith(entry.id);
  });
});
