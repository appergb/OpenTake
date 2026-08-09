// @vitest-environment happy-dom

import React from "react";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { MediaItem } from "../../lib/types";
import { useChatStore } from "../../store/chatStore";
import { useEditorUiStore } from "../../store/uiStore";
import { useLibraryStore } from "../../store/libraryStore";
import { useMediaStore } from "../../store/mediaStore";
import { MusicTab } from "./MusicTab";

function audio(id: string, name = id): MediaItem {
  return {
    id,
    name,
    type: "audio",
    duration: 12,
    hasAudio: true,
    favorite: false,
  };
}

afterEach(() => {
  document.body.replaceChildren();
  useMediaStore.setState({ items: [], folders: [], importing: false, error: null });
  useLibraryStore.setState({ entries: [], loading: false, error: null });
  useEditorUiStore.setState({ agentPanelVisible: false });
  useChatStore.setState({ composerDraft: null });
});

describe("MusicTab", () => {
  it("browses_imports_places_and_prepares_generation", async () => {
    const projectTrack = audio("project-music", "Project Theme");
    const importedTrack = audio("imported-music", "Library Theme");
    useMediaStore.setState({ items: [projectTrack, importedTrack] });
    useLibraryStore.setState({
      entries: [
        {
          id: "library-music",
          type: "audio",
          category: "music",
          favoritedAt: 1,
          source: "/tmp/Library Theme.wav",
        },
      ],
      refresh: vi.fn().mockResolvedValue(undefined),
      importToProject: vi.fn().mockResolvedValue({
        id: importedTrack.id,
        name: importedTrack.name,
        path: "/tmp/Library Theme.wav",
      }),
    });
    const place = vi.fn().mockResolvedValue(undefined);
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () => root.render(<MusicTab onPlace={place} />));

    expect(container.textContent).toContain("Project Theme");
    expect(container.textContent).toContain("Library Theme.wav");

    await act(async () =>
      container
        .querySelector<HTMLButtonElement>('[data-place-media="project-music"]')!
        .dispatchEvent(new MouseEvent("click", { bubbles: true })),
    );
    expect(place).toHaveBeenCalledWith(projectTrack);

    await act(async () =>
      container
        .querySelector<HTMLButtonElement>('[data-import-library="library-music"]')!
        .dispatchEvent(new MouseEvent("click", { bubbles: true })),
    );
    expect(useLibraryStore.getState().importToProject).toHaveBeenCalledWith("library-music");
    expect(place).toHaveBeenCalledWith(importedTrack);

    await act(async () =>
      container
        .querySelector<HTMLButtonElement>('[data-action="generate-music"]')!
        .dispatchEvent(new MouseEvent("click", { bubbles: true })),
    );
    expect(useEditorUiStore.getState().agentPanelVisible).toBe(true);
    expect(useChatStore.getState().composerDraft).toContain("音乐");
    await act(async () => root.unmount());
  });

  it("shows_a_typed_failure_when_timeline_placement_is_rejected", async () => {
    useMediaStore.setState({ items: [audio("broken", "Broken Theme")] });
    useLibraryStore.setState({ refresh: vi.fn().mockResolvedValue(undefined) });
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () =>
      root.render(<MusicTab onPlace={vi.fn().mockRejectedValue(new Error("timeline locked"))} />),
    );

    await act(async () =>
      container
        .querySelector<HTMLButtonElement>('[data-place-media="broken"]')!
        .dispatchEvent(new MouseEvent("click", { bubbles: true })),
    );
    expect(container.querySelector('[role="alert"]')?.textContent).toContain("timeline locked");
    await act(async () => root.unmount());
  });
});
