// @vitest-environment happy-dom

import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { MediaItem, MediaList } from "../../lib/types";
import {
  applyMediaErrorForProject,
  applyMediaListForProject,
  useMediaStore,
} from "../../store/mediaStore";
import { useProjectStore } from "../../store/projectStore";

vi.mock("../../lib/api", () => ({
  getWaveform: vi.fn(),
}));

import { AudioWaveform, MediaFavoriteButton } from "./MediaPanel";

afterEach(() => {
  document.body.replaceChildren();
  useMediaStore.setState({ items: [], folders: [], importing: false, error: null });
  useProjectStore.setState({ projectEpoch: 0, projectPath: null });
});

function mediaItem(id: string): MediaItem {
  return {
    id,
    name: id,
    type: "video",
    duration: 1,
    hasAudio: false,
    favorite: false,
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

describe("AudioWaveform", () => {
  const fallback = <span data-testid="audio-fallback">fallback</span>;

  it("renders waveform bars when normalized buckets are available", () => {
    const html = renderToStaticMarkup(
      <AudioWaveform mediaRef="audio-1" fallback={fallback} bucketsOverride={[0, 0.25, 0.5, 0.75, 1]} />,
    );

    expect(html).toContain('data-testid="audio-waveform"');
    expect(html).not.toContain('data-testid="audio-fallback"');
  });

  it("renders the fallback when waveform loading resolves to null", () => {
    const html = renderToStaticMarkup(
      <AudioWaveform mediaRef="audio-1" fallback={fallback} bucketsOverride={null} />,
    );

    expect(html).not.toContain('data-testid="audio-waveform"');
    expect(html).toContain('data-testid="audio-fallback"');
  });

  it("renders the fallback when waveform loading resolves to an empty array", () => {
    const html = renderToStaticMarkup(
      <AudioWaveform mediaRef="audio-1" fallback={fallback} bucketsOverride={[]} />,
    );

    expect(html).not.toContain('data-testid="audio-waveform"');
    expect(html).toContain('data-testid="audio-fallback"');
  });
});

describe("MediaFavoriteButton", () => {
  it("disables while pending and keeps the rendered star unchanged after rejection", async () => {
    useProjectStore.setState({ projectEpoch: 7, projectPath: "/project-7.opentake" });
    let rejectToggle: (reason: unknown) => void = () => undefined;
    const pendingToggle = new Promise<never>((_resolve, reject) => {
      rejectToggle = reject;
    });
    const performToggle = vi.fn(() => pendingToggle);
    const onSuccess = vi.fn();

    function Harness() {
      const [feedback, setFeedback] = React.useState<string | null>(null);
      return (
        <>
          <MediaFavoriteButton
            assetId="asset-1"
            favorite
            title="Unfavorite"
            onSuccess={onSuccess}
            onError={setFeedback}
            performToggle={performToggle}
          />
          {feedback && <span role="alert">{feedback}</span>}
        </>
      );
    }

    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () => root.render(<Harness />));
    const button = container.querySelector<HTMLButtonElement>("button")!;

    await act(async () => button.dispatchEvent(new MouseEvent("click", { bubbles: true })));

    expect(performToggle).toHaveBeenCalledWith("asset-1", false, {
      projectEpoch: 7,
      projectPath: "/project-7.opentake",
    });
    expect(button.disabled).toBe(true);
    expect(button.getAttribute("aria-busy")).toBe("true");
    expect(button.getAttribute("aria-pressed")).toBe("true");

    await act(async () => rejectToggle(new Error("backend rejected")));

    expect(button.disabled).toBe(false);
    expect(button.getAttribute("aria-busy")).toBe("false");
    expect(button.getAttribute("aria-pressed")).toBe("true");
    expect(container.querySelector('[role="alert"]')?.textContent).toContain("backend rejected");
    expect(onSuccess).not.toHaveBeenCalled();
    await act(async () => root.unmount());
  });

  it("ignores a project A resolution after project B replaces the media mirror", async () => {
    const toggle = deferred<MediaList>();
    useProjectStore.setState({ projectEpoch: 1, projectPath: "/A.opentake" });
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () =>
      root.render(
        <MediaFavoriteButton
          assetId="asset-a"
          favorite={false}
          title="Favorite"
          performToggle={() => toggle.promise}
          onSuccess={(media, project) => {
            applyMediaListForProject(project, media);
          }}
          onError={(message, project) => {
            applyMediaErrorForProject(project, message);
          }}
        />,
      ),
    );
    const button = container.querySelector<HTMLButtonElement>("button")!;
    await act(async () => button.dispatchEvent(new MouseEvent("click", { bubbles: true })));
    const projectBItems = [mediaItem("asset-b")];
    const projectBFolders = [{ id: "folder-b", name: "B" }];
    useProjectStore.setState({ projectEpoch: 2, projectPath: "/B.opentake" });
    useMediaStore.setState({
      items: projectBItems,
      folders: projectBFolders,
      error: "B error",
    });

    await act(async () =>
      toggle.resolve({ items: [mediaItem("late-a")], folders: [{ id: "folder-a", name: "A" }] }),
    );

    expect(useMediaStore.getState().items).toEqual(projectBItems);
    expect(useMediaStore.getState().folders).toEqual(projectBFolders);
    expect(useMediaStore.getState().error).toBe("B error");
    await act(async () => root.unmount());
  });

  it("ignores a project A rejection after project B replaces the media mirror", async () => {
    const toggle = deferred<MediaList>();
    useProjectStore.setState({ projectEpoch: 10, projectPath: "/A.opentake" });
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () =>
      root.render(
        <MediaFavoriteButton
          assetId="asset-a"
          favorite
          title="Unfavorite"
          performToggle={() => toggle.promise}
          onSuccess={(media, project) => {
            applyMediaListForProject(project, media);
          }}
          onError={(message, project) => {
            applyMediaErrorForProject(project, message);
          }}
        />,
      ),
    );
    const button = container.querySelector<HTMLButtonElement>("button")!;
    await act(async () => button.dispatchEvent(new MouseEvent("click", { bubbles: true })));
    const projectBItems = [mediaItem("asset-b")];
    const projectBFolders = [{ id: "folder-b", name: "B" }];
    useProjectStore.setState({ projectEpoch: 11, projectPath: "/B.opentake" });
    useMediaStore.setState({ items: projectBItems, folders: projectBFolders, error: null });

    await act(async () => toggle.reject(new Error("late A rejection")));

    expect(useMediaStore.getState().items).toEqual(projectBItems);
    expect(useMediaStore.getState().folders).toEqual(projectBFolders);
    expect(useMediaStore.getState().error).toBeNull();
    await act(async () => root.unmount());
  });
});
