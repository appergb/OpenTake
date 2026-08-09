// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { MediaItem } from "../../lib/types";
import { useEditorUiStore } from "../../store/uiStore";
import {
  BadgeMenu,
  exactTimelineFrame,
  NativeSourcePlaybackSurface,
  PreviewTabs,
  retireSourcePlaybackStart,
  ScrubBar,
  sourceDurationFrames,
  sourcePreviewFrame,
  sourcePlaybackStartFrame,
} from "./Preview";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

let container: HTMLDivElement;
let root: Root;
let onSeek: ReturnType<typeof vi.fn>;
let onExactSeek: ReturnType<typeof vi.fn>;
let onScrubbingChange: ReturnType<typeof vi.fn>;
let setPointerCapture: ReturnType<typeof vi.spyOn>;

it("normalizes fractional playback ticks before exact frame stepping", () => {
  expect(exactTimelineFrame(26.4, 100)).toBe(26);
  expect(exactTimelineFrame(-0.6, 100)).toBe(0);
  expect(exactTimelineFrame(100.6, 100)).toBe(100);
});

it("rewinds a completed source preview and otherwise resumes its exact frame", () => {
  expect(sourcePlaybackStartFrame(42, 100)).toBe(42);
  expect(sourcePlaybackStartFrame(99, 100)).toBe(0);
  expect(sourcePlaybackStartFrame(120, 100)).toBe(0);
});

it("keeps source still requests on the last decodable frame", () => {
  expect(sourcePreviewFrame(42, 100)).toBe(42);
  expect(sourcePreviewFrame(100, 100)).toBe(99);
  expect(sourcePreviewFrame(120, 100)).toBe(99);
});

it("truncates fractional source durations at the terminal frame", () => {
  expect(sourceDurationFrames(1.55, 30)).toBe(46);
  expect(sourceDurationFrames(Number.NaN, 30)).toBe(0);
});

it("invalidates a pending source start before its deferred completion", async () => {
  const generation = { current: 3 };
  const starting = { current: true };
  const pendingToken = generation.current;
  let release!: () => void;
  let committed = false;
  const completion = new Promise<void>((resolve) => {
    release = resolve;
  }).then(() => {
    if (pendingToken === generation.current) committed = true;
  });

  retireSourcePlaybackStart(generation, starting);
  release();
  await completion;

  expect(generation.current).toBe(4);
  expect(starting.current).toBe(false);
  expect(committed).toBe(false);
});

beforeEach(() => {
  onSeek = vi.fn();
  onExactSeek = vi.fn();
  onScrubbingChange = vi.fn();
  setPointerCapture = vi.spyOn(HTMLElement.prototype, "setPointerCapture")
    .mockImplementation(() => {});
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
  act(() => {
    root.render(
      <ScrubBar
        ariaLabel="Preview playhead"
        frame={25}
        total={100}
        onSeek={onSeek}
        onExactSeek={onExactSeek}
        onScrubbingChange={onScrubbingChange}
      />,
    );
  });
});

afterEach(async () => {
  await act(async () => root.unmount());
  container.remove();
  vi.restoreAllMocks();
});

it("control-200c9fd6ec3f0f35 pointer scrub preview playhead", async () => {
  const scrub = container.querySelector<HTMLElement>("[data-preview-scrub]");
  expect(scrub?.getAttribute("role")).toBe("slider");
  expect(scrub?.tabIndex).toBe(0);
  expect(scrub?.getAttribute("aria-label")).toBe("Preview playhead");
  expect(scrub?.getAttribute("aria-valuemin")).toBe("0");
  expect(scrub?.getAttribute("aria-valuemax")).toBe("100");
  expect(scrub?.getAttribute("aria-valuenow")).toBe("25");
  expect(scrub?.style.height).toBe("24px");
  scrub!.getBoundingClientRect = () =>
    ({ left: 100, width: 200, right: 300, top: 0, bottom: 18, height: 18 }) as DOMRect;

  const track = container.querySelector<HTMLElement>("[data-preview-scrub-track]");
  expect(track?.style.height).toBe("3px");
  await act(async () => scrub?.dispatchEvent(new MouseEvent("mouseover", { bubbles: true })));
  expect(track?.style.height).toBe("4px");
  expect(onSeek).not.toHaveBeenCalled();
  expect(onScrubbingChange).not.toHaveBeenCalled();

  await act(async () => {
    scrub?.dispatchEvent(new PointerEvent("pointerdown", {
      bubbles: true,
      pointerId: 7,
      clientX: 150,
    }));
  });
  expect(setPointerCapture).toHaveBeenCalledWith(7);
  expect(onScrubbingChange).toHaveBeenLastCalledWith(true);
  expect(onSeek).toHaveBeenLastCalledWith(25);

  onSeek.mockClear();
  await act(async () => {
    scrub?.dispatchEvent(new PointerEvent("pointermove", {
      bubbles: true,
      pointerId: 7,
      buttons: 0,
      clientX: 200,
    }));
  });
  expect(onSeek).not.toHaveBeenCalled();

  await act(async () => {
    scrub?.dispatchEvent(new PointerEvent("pointermove", {
      bubbles: true,
      pointerId: 7,
      buttons: 1,
      clientX: 200,
    }));
    scrub?.dispatchEvent(new PointerEvent("pointerup", {
      bubbles: true,
      pointerId: 7,
      clientX: 250,
    }));
  });
  expect(onSeek.mock.calls).toEqual([[50], [75]]);
  expect(onScrubbingChange).toHaveBeenLastCalledWith(false);

  onScrubbingChange.mockClear();
  await act(async () => {
    scrub?.dispatchEvent(new PointerEvent("pointerdown", {
      bubbles: true,
      pointerId: 8,
      clientX: 160,
    }));
  });
  await act(async () => scrub?.dispatchEvent(new Event("lostpointercapture", { bubbles: true })));
  expect(onScrubbingChange.mock.calls).toEqual([[true], [false]]);

  onSeek.mockClear();
  scrub?.focus();
  await act(async () => {
    scrub?.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }));
    scrub?.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowLeft", shiftKey: true, bubbles: true }));
    scrub?.dispatchEvent(new KeyboardEvent("keydown", { key: "Home", bubbles: true }));
    scrub?.dispatchEvent(new KeyboardEvent("keydown", { key: "End", bubbles: true }));
  });
  expect(onSeek).not.toHaveBeenCalled();
  expect(onExactSeek.mock.calls).toEqual([[26], [20], [0], [100]]);
  expect(document.activeElement).toBe(scrub);
});

it("gives preview tabs a connected tablist and roving keyboard behavior", async () => {
  const item: MediaItem = {
    id: "source",
    name: "Source clip",
    type: "video",
    duration: 1,
    hasAudio: false,
    favorite: false,
  };
  useEditorUiStore.setState({ previewMediaId: item.id });
  await act(async () => root.render(<PreviewTabs item={item} />));

  const tablist = container.querySelector('[role="tablist"]');
  const tabs = [...container.querySelectorAll<HTMLButtonElement>('[role="tab"]')];
  expect(tablist).not.toBeNull();
  expect(tabs).toHaveLength(2);
  expect(tabs.map((tab) => tab.getAttribute("aria-selected"))).toEqual(["false", "true"]);
  expect(tabs.map((tab) => tab.tabIndex)).toEqual([-1, 0]);
  expect(tabs[1]?.getAttribute("aria-controls")).toBe("preview-content-panel");

  tabs[1]?.focus();
  await act(async () =>
    tabs[1]?.dispatchEvent(new KeyboardEvent("keydown", { key: "Home", bubbles: true })),
  );
  expect(useEditorUiStore.getState().previewMediaId).toBeNull();
  expect(document.activeElement).toBe(tabs[0]);
});

it("requests latest-only FFmpeg source stills for paused source seek", async () => {
  const requestSourceFrame = vi.fn().mockResolvedValue({
    width: 1280,
    height: 720,
    dataUrl: "data:image/png;base64,source",
  });
  const item: MediaItem = {
    id: "main10",
    name: "Main10",
    type: "video",
    duration: 210.7105,
    width: 3840,
    height: 2160,
    hasAudio: true,
    favorite: false,
  };
  const renderFrame = (frame: number) => (
    <NativeSourcePlaybackSurface
      item={item}
      event={null}
      endpoint="http://127.0.0.1/frame"
      projectEpoch={3}
      timelineVersion={7}
      playing={false}
      frame={frame}
      previewQualityShortEdge={720}
      onPlayingChange={vi.fn()}
      onTerminalFailure={vi.fn()}
      requestSourceFrame={requestSourceFrame}
      cancelSourceFrame={vi.fn().mockResolvedValue(undefined)}
    />
  );

  await act(async () => {
    root.render(renderFrame(1_572));
    await Promise.resolve();
  });
  await act(async () => {
    root.render(renderFrame(1_600));
    await Promise.resolve();
  });

  expect(requestSourceFrame.mock.calls.map(([request]) => request)).toEqual([
    expect.objectContaining({ frame: 1_572, sourceMediaId: "main10" }),
    expect.objectContaining({ frame: 1_600, sourceMediaId: "main10" }),
  ]);
  expect(requestSourceFrame).toHaveBeenLastCalledWith(
    expect.objectContaining({ frame: 1_600, sourceMediaId: "main10" }),
    1280,
  );
});

it("drops the previous source still while the replacement source decodes", async () => {
  const item = (id: string): MediaItem => ({
    id,
    name: id,
    type: "video",
    duration: 10,
    width: 1920,
    height: 1080,
    hasAudio: true,
    favorite: false,
  });
  const requestSourceFrame = vi.fn(
    (request: { sourceMediaId?: string }) =>
      request.sourceMediaId === "source-a"
        ? Promise.resolve({
            width: 1280,
            height: 720,
            dataUrl: "data:image/png;base64,source-a",
          })
        : new Promise<never>(() => undefined),
  );
  const renderSource = (id: string) => (
    <NativeSourcePlaybackSurface
      item={item(id)}
      event={null}
      endpoint="http://127.0.0.1/frame"
      projectEpoch={3}
      timelineVersion={7}
      playing={false}
      frame={12}
      previewQualityShortEdge={720}
      onPlayingChange={vi.fn()}
      onTerminalFailure={vi.fn()}
      requestSourceFrame={requestSourceFrame}
      cancelSourceFrame={vi.fn().mockResolvedValue(undefined)}
    />
  );

  await act(async () => {
    root.render(renderSource("source-a"));
    await Promise.resolve();
    await Promise.resolve();
  });
  expect(
    container.querySelector<HTMLImageElement>("[data-testid='rust-idle-composite-still']")
      ?.src,
  ).toContain("source-a");

  await act(async () => {
    root.render(renderSource("source-b"));
    await Promise.resolve();
  });
  expect(
    container.querySelector<HTMLImageElement>("[data-testid='rust-idle-composite-still']"),
  ).toBeNull();
});

it("moves focus through BadgeMenu listbox options and restores it on Escape", async () => {
  const selected = vi.fn();
  await act(async () =>
    root.render(
      <BadgeMenu
        label="100%"
        ariaLabel="Canvas zoom"
        options={[
          { key: "fit", label: "Fit", active: true, onSelect: selected },
          { key: "100", label: "100%", active: false, onSelect: selected },
          { key: "200", label: "200%", active: false, onSelect: selected },
        ]}
      />,
    ),
  );

  const trigger = container.querySelector<HTMLButtonElement>('button[aria-haspopup="listbox"]')!;
  expect(trigger.getAttribute("aria-expanded")).toBe("false");
  const listboxId = trigger.getAttribute("aria-controls");
  expect(listboxId).toBeTruthy();
  await act(async () => trigger.click());

  const listbox = document.getElementById(listboxId!);
  const options = [...listbox!.querySelectorAll<HTMLButtonElement>('[role="option"]')];
  expect(listbox?.getAttribute("aria-label")).toBe("Canvas zoom");
  expect(document.activeElement).toBe(options[0]);
  expect(options.map((option) => option.tabIndex)).toEqual([0, -1, -1]);
  await act(async () =>
    document.activeElement?.dispatchEvent(
      new KeyboardEvent("keydown", { key: "End", bubbles: true }),
    ),
  );
  expect(document.activeElement).toBe(options[2]);
  expect(options.map((option) => option.tabIndex)).toEqual([-1, -1, 0]);
  await act(async () =>
    document.activeElement?.dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowUp", bubbles: true }),
    ),
  );
  expect(document.activeElement).toBe(options[1]);
  expect(options.map((option) => option.tabIndex)).toEqual([-1, 0, -1]);
  await act(async () =>
    document.activeElement?.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Home", bubbles: true }),
    ),
  );
  expect(document.activeElement).toBe(options[0]);
  expect(options.map((option) => option.tabIndex)).toEqual([0, -1, -1]);
  await act(async () =>
    document.activeElement?.dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }),
    ),
  );
  expect(document.activeElement).toBe(options[1]);
  expect(options.map((option) => option.tabIndex)).toEqual([-1, 0, -1]);
  await act(async () =>
    document.activeElement?.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Escape", bubbles: true }),
    ),
  );
  expect(document.getElementById(listboxId!)).toBeNull();
  expect(document.activeElement).toBe(trigger);
});

it("dismisses BadgeMenu on Tab or focusout without stealing focus and selects with Enter", async () => {
  const selected = vi.fn();
  const outside = document.createElement("button");
  document.body.append(outside);
  await act(async () =>
    root.render(
      <BadgeMenu
        label="100%"
        ariaLabel="Canvas zoom"
        options={[
          { key: "fit", label: "Fit", active: true, onSelect: selected },
          { key: "100", label: "100%", active: false, onSelect: selected },
        ]}
      />,
    ),
  );

  let trigger = container.querySelector<HTMLButtonElement>('button[aria-haspopup="listbox"]')!;
  const listboxId = trigger.getAttribute("aria-controls")!;
  await act(async () => trigger.click());
  let option = document
    .getElementById(listboxId)!
    .querySelector<HTMLButtonElement>('[role="option"]')!;
  await act(async () =>
    option.dispatchEvent(new KeyboardEvent("keydown", { key: "Tab", bubbles: true })),
  );
  trigger = container.querySelector<HTMLButtonElement>('button[aria-haspopup="listbox"]')!;
  expect(document.getElementById(listboxId)).toBeNull();
  expect(trigger.getAttribute("aria-expanded")).toBe("false");
  expect(document.activeElement).not.toBe(trigger);

  await act(async () => trigger.click());
  option = document
    .getElementById(listboxId)!
    .querySelector<HTMLButtonElement>('[role="option"]')!;
  expect(document.activeElement).toBe(option);
  await act(async () => outside.focus());
  trigger = container.querySelector<HTMLButtonElement>('button[aria-haspopup="listbox"]')!;
  expect(document.getElementById(listboxId)).toBeNull();
  expect(trigger.getAttribute("aria-expanded")).toBe("false");
  expect(document.activeElement).toBe(outside);

  await act(async () => trigger.click());
  const options = [
    ...document.getElementById(listboxId)!.querySelectorAll<HTMLButtonElement>('[role="option"]'),
  ];
  await act(async () =>
    options[0]?.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true })),
  );
  await act(async () =>
    options[1]?.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true })),
  );
  expect(selected).toHaveBeenCalledOnce();
  expect(document.getElementById(listboxId)).toBeNull();
  expect(document.activeElement).toBe(trigger);
  outside.remove();
});

it("keeps only one BadgeMenu listbox open across assistive click activation", async () => {
  await act(async () =>
    root.render(
      <div>
        <BadgeMenu
          label="100%"
          ariaLabel="Canvas zoom"
          options={[{ key: "100", label: "100%", active: true, onSelect: () => {} }]}
        />
        <BadgeMenu
          label="1080p"
          ariaLabel="Preview quality"
          options={[{ key: "1080", label: "1080p", active: true, onSelect: () => {} }]}
        />
      </div>,
    ),
  );
  const triggers = [
    ...container.querySelectorAll<HTMLButtonElement>('button[aria-haspopup="listbox"]'),
  ];

  await act(async () => triggers[0]?.click());
  await act(async () => triggers[1]?.click());

  expect(container.querySelectorAll('[role="listbox"]')).toHaveLength(1);
  expect(triggers[0]?.getAttribute("aria-expanded")).toBe("false");
  expect(triggers[1]?.getAttribute("aria-expanded")).toBe("true");
});
