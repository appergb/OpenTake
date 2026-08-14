// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Reveal } from "./Reveal";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

type ObserverRecord = {
  callback: ResizeObserverCallback;
  target: Element | null;
};

const observerRecords = new Set<ObserverRecord>();
let animationFrames: FrameRequestCallback[] = [];
let container: HTMLDivElement;
let root: Root;

class TestResizeObserver implements ResizeObserver {
  private readonly record: ObserverRecord;

  constructor(callback: ResizeObserverCallback) {
    this.record = { callback, target: null };
    observerRecords.add(this.record);
  }

  observe(target: Element) {
    this.record.target = target;
    this.emit();
  }

  unobserve() {
    this.record.target = null;
  }

  disconnect() {
    observerRecords.delete(this.record);
    this.record.target = null;
  }

  emit() {
    const target = this.record.target;
    if (!target) return;
    const height = (target as HTMLElement).scrollHeight;
    this.record.callback(
      [
        {
          target,
          contentRect: { height } as DOMRectReadOnly,
        } as ResizeObserverEntry,
      ],
      this,
    );
  }
}

function renderReveal(
  open: boolean,
  options: { height?: number; onExited?: () => void } = {},
) {
  const height = options.height ?? 48;
  act(() => {
    root.render(
      <Reveal open={open} id="details" role="status" onExited={options.onExited}>
        <button data-height={height}>More details</button>
      </Reveal>,
    );
  });
}

function flushAnimationFrame() {
  const callbacks = animationFrames;
  animationFrames = [];
  act(() => callbacks.forEach((callback) => callback(0)));
}

function setReducedMotion(matches: boolean) {
  vi.stubGlobal(
    "matchMedia",
    vi.fn().mockImplementation(() => ({
      matches,
      media: "(prefers-reduced-motion: reduce)",
      onchange: null,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      addListener: vi.fn(),
      removeListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })),
  );
}

beforeEach(() => {
  vi.useFakeTimers();
  setReducedMotion(false);
  vi.stubGlobal("ResizeObserver", TestResizeObserver);
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
    animationFrames.push(callback);
    return animationFrames.length;
  });
  vi.stubGlobal("cancelAnimationFrame", vi.fn());
  vi.spyOn(HTMLElement.prototype, "scrollHeight", "get").mockImplementation(function () {
    return Number(this.firstElementChild?.getAttribute("data-height") ?? 0);
  });
  document.documentElement.style.setProperty("--motion-disclosure-duration", "180ms");

  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
  observerRecords.clear();
  animationFrames = [];
  document.documentElement.style.removeProperty("--motion-disclosure-duration");
  vi.useRealTimers();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("Reveal", () => {
  it("mounts open content with the requested semantics", () => {
    renderReveal(true);

    const disclosure = container.querySelector<HTMLElement>(".reveal");
    expect(disclosure?.id).toBe("details");
    expect(disclosure?.getAttribute("role")).toBe("status");
    expect(disclosure?.dataset.state).toBe("open");
    expect(disclosure?.textContent).toContain("More details");
  });

  it("retains measured content for the exit and unmounts after the duration", async () => {
    const onExited = vi.fn();
    renderReveal(true, { onExited });

    renderReveal(false, { onExited });
    const closing = container.querySelector<HTMLElement>(".reveal");
    expect(closing?.dataset.state).toBe("closed");
    expect(closing?.style.getPropertyValue("--reveal-block-size")).toBe("48px");
    expect(closing?.textContent).toContain("More details");

    await act(async () => vi.advanceTimersByTimeAsync(179));
    expect(container.querySelector(".reveal")).not.toBeNull();

    await act(async () => vi.advanceTimersByTimeAsync(1));
    expect(container.querySelector(".reveal")).toBeNull();
    expect(onExited).toHaveBeenCalledTimes(1);
  });

  it("cancels a pending unmount when reopened rapidly", async () => {
    const onExited = vi.fn();
    renderReveal(true, { onExited });
    renderReveal(false, { onExited });

    await act(async () => vi.advanceTimersByTimeAsync(90));
    renderReveal(true, { onExited });
    flushAnimationFrame();
    await act(async () => vi.advanceTimersByTimeAsync(180));

    expect(container.querySelector<HTMLElement>(".reveal")?.dataset.state).toBe("open");
    expect(container.textContent).toContain("More details");
    expect(onExited).not.toHaveBeenCalled();
  });

  it("keeps a numeric measured block size and updates it when content resizes", () => {
    renderReveal(true, { height: 48 });
    const disclosure = container.querySelector<HTMLElement>(".reveal")!;
    const content = disclosure.querySelector<HTMLElement>(".reveal__content")!;

    expect(disclosure.style.getPropertyValue("--reveal-block-size")).toBe("48px");
    expect(disclosure.style.getPropertyValue("--reveal-block-size")).not.toBe("auto");
    expect(disclosure.style.getPropertyValue("--reveal-block-size")).not.toBe("0px");

    content.firstElementChild?.setAttribute("data-height", "76");
    act(() => observerRecords.forEach((record) => record.target === content && record.callback(
      [{ target: content, contentRect: { height: 76 } as DOMRectReadOnly } as ResizeObserverEntry],
      {} as ResizeObserver,
    )));

    expect(disclosure.style.getPropertyValue("--reveal-block-size")).toBe("76px");
  });

  it("removes focus and interaction from content as soon as it is hidden", () => {
    renderReveal(true);
    const button = container.querySelector<HTMLButtonElement>("button")!;
    button.focus();
    expect(document.activeElement).toBe(button);

    renderReveal(false);

    const disclosure = container.querySelector<HTMLElement>(".reveal")!;
    expect(document.activeElement).not.toBe(button);
    expect(disclosure.getAttribute("aria-hidden")).toBe("true");
    expect(disclosure.hasAttribute("inert")).toBe(true);
  });

  it("unmounts synchronously when reduced motion is requested", () => {
    setReducedMotion(true);
    const onExited = vi.fn();
    renderReveal(true, { onExited });

    renderReveal(false, { onExited });

    expect(container.querySelector(".reveal")).toBeNull();
    expect(onExited).toHaveBeenCalledTimes(1);
  });

  it("opens synchronously when reduced motion is requested", () => {
    setReducedMotion(true);
    renderReveal(false);

    renderReveal(true);

    expect(container.querySelector<HTMLElement>(".reveal")?.dataset.state).toBe("open");
    expect(animationFrames).toHaveLength(0);
  });
});
