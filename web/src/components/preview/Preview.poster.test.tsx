// @vitest-environment happy-dom

import React, { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { MediaItem } from "../../lib/types";

vi.mock("../../lib/asset", () => ({
  assetUrl: (path: string | null | undefined) => (path ? `asset://${path}` : null),
}));

vi.mock("../../lib/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../lib/api")>()),
  previewPoster: vi.fn(),
}));

import * as api from "../../lib/api";
import { derivedResourceScheduler } from "../../lib/derivedResourceScheduler";
import { MediaPreview } from "./Preview";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((res) => {
    resolve = res;
  });
  return { promise, resolve };
}

function videoItem(id: string, path: string): MediaItem {
  return {
    id,
    name: id,
    type: "video",
    duration: 1,
    hasAudio: true,
    favorite: false,
    path,
  };
}

afterEach(() => {
  document.body.replaceChildren();
  vi.mocked(api.previewPoster).mockReset();
});

describe("MediaPreview poster scheduling", () => {
  it("keeps a superseded poster from publishing after the selected item changes", async () => {
    const stale = deferred<string | null>();
    const current = deferred<string | null>();
    vi.mocked(api.previewPoster)
      .mockReturnValueOnce(stale.promise)
      .mockReturnValueOnce(current.promise);
    derivedResourceScheduler.activateProject(300);
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    const mediaRef = { current: null as HTMLMediaElement | null };
    const callbacks = {
      onTime: vi.fn(),
      onDuration: vi.fn(),
      onPlayingChange: vi.fn(),
    };
    let mounted = true;
    try {
      await act(async () =>
        root.render(
          <MediaPreview
            item={videoItem("old", "/old.mov")}
            projectEpoch={300}
            mediaRef={mediaRef}
            {...callbacks}
          />,
        ),
      );
      await act(async () =>
        root.render(
          <MediaPreview
            item={videoItem("current", "/current.mov")}
            projectEpoch={300}
            mediaRef={mediaRef}
            {...callbacks}
          />,
        ),
      );
      expect(api.previewPoster).toHaveBeenNthCalledWith(1, "old");
      expect(api.previewPoster).toHaveBeenNthCalledWith(2, "current");

      await act(async () => stale.resolve("/stale.png"));
      expect(container.querySelector("video")?.getAttribute("poster")).toBeNull();

      await act(async () => current.resolve("/current.png"));
      expect(container.querySelector("video")?.getAttribute("poster")).toBe(
        "asset:///current.png",
      );
    } finally {
      if (mounted) {
        await act(async () => root.unmount());
        mounted = false;
      }
      stale.resolve(null);
      current.resolve(null);
    }
  });
});
