// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { ScrubBar } from "./Preview";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

let container: HTMLDivElement;
let root: Root;
let onSeek: ReturnType<typeof vi.fn>;
let onScrubbingChange: ReturnType<typeof vi.fn>;
let setPointerCapture: ReturnType<typeof vi.spyOn>;

beforeEach(() => {
  onSeek = vi.fn();
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
    scrub?.dispatchEvent(new KeyboardEvent("keydown", { key: "Home", bubbles: true }));
    scrub?.dispatchEvent(new KeyboardEvent("keydown", { key: "End", bubbles: true }));
  });
  expect(onSeek.mock.calls).toEqual([[26], [0], [100]]);
  expect(document.activeElement).toBe(scrub);
});
