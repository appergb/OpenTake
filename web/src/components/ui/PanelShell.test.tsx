import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useEditorUiStore } from "../../store/uiStore";

vi.mock("../../lib/api", () => ({ isTauri: true }));
// SSR reads Zustand's initial snapshot; select the current real store state so
// the rendered component sees the requested PLAY/PAUSE transport state.
vi.mock("../../store/uiStore", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../store/uiStore")>();
  const store = actual.useEditorUiStore;
  const directStore = Object.assign(
    <T,>(selector: (state: ReturnType<typeof store.getState>) => T) =>
      selector(store.getState()),
    store,
  );
  return { ...actual, useEditorUiStore: directStore };
});

import { PanelShell } from "./PanelShell";

function renderPreviewShell(isPlaying: boolean): string {
  useEditorUiStore.setState({
    focusedPanel: "timeline",
    isPlaying,
    isScrubbing: false,
    previewMediaId: null,
    rustEngineFailed: false,
  });
  return renderToStaticMarkup(
    <PanelShell panel="preview">
      <div>preview</div>
    </PanelShell>,
  );
}

describe("PanelShell preview surface", () => {
  beforeEach(() => {
    vi.stubGlobal("localStorage", {
      getItem: () => null,
    } as unknown as Storage);
    useEditorUiStore.setState({
      focusedPanel: "timeline",
      isPlaying: false,
      isScrubbing: false,
      previewMediaId: null,
      rustEngineFailed: false,
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it.each([
    ["PAUSE", false],
    ["PLAY", true],
  ])("keeps both preview shell layers opaque during %s", (_transport, isPlaying) => {
    const html = renderPreviewShell(isPlaying);

    expect(html).toContain("background:var(--bg-base)");
    expect(html).toContain("background:var(--bg-surface)");
    expect(html).not.toContain("background:transparent");
  });
});
