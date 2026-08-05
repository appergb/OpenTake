// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { SplitPane } from "./SplitPane";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

let container: HTMLDivElement;
let root: Root;
let setPointerCapture: ReturnType<typeof vi.spyOn>;
let releasePointerCapture: ReturnType<typeof vi.spyOn>;

beforeEach(() => {
  vi.stubGlobal("ResizeObserver", undefined);
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(600);
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(400);
  setPointerCapture = vi.spyOn(HTMLElement.prototype, "setPointerCapture")
    .mockImplementation(() => {});
  releasePointerCapture = vi.spyOn(HTMLElement.prototype, "releasePointerCapture")
    .mockImplementation(() => {});
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
});

afterEach(async () => {
  await act(async () => root.unmount());
  container.remove();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

it("control-d88c7103e09bb382 resize two editor panes", async () => {
  await act(async () => {
    root.render(
      <SplitPane
        mode="horizontal"
        initial={200}
        min={100}
        secondMin={150}
        first={(
          <>
            <button data-pane="first" />
            <div role="gridcell" data-pane="gridcell" />
            <div role="spinbutton" data-pane="spinbutton" />
            <div draggable data-pane="draggable" />
          </>
        )}
        second={<div data-pane="second" />}
      />,
    );
  });
  const separator = container.querySelector<HTMLElement>("[role='separator']")!;
  const split = separator.parentElement?.parentElement as HTMLElement;
  split.getBoundingClientRect = () =>
    ({ left: 100, width: 600, right: 700, top: 50, height: 400, bottom: 450 }) as DOMRect;
  expect(separator.tabIndex).toBe(0);
  expect(separator.getAttribute("aria-orientation")).toBe("vertical");
  expect(separator.getAttribute("aria-valuemin")).toBe("100");
  expect(separator.getAttribute("aria-valuemax")).toBe("450");
  expect(separator.style.width).toBe("24px");
  expect(separator.style.pointerEvents).toBe("none");

  const seamControl = container.querySelector<HTMLButtonElement>("[data-pane='first']")!;
  await act(async () => {
    seamControl.dispatchEvent(new PointerEvent("pointerdown", {
      bubbles: true,
      pointerId: 2,
      clientX: 300,
    }));
  });
  expect(setPointerCapture).not.toHaveBeenCalled();
  expect(separator.getAttribute("aria-valuenow")).toBe("200");

  for (const kind of ["gridcell", "spinbutton", "draggable"]) {
    const customControl = container.querySelector<HTMLElement>(`[data-pane='${kind}']`)!;
    await act(async () => {
      customControl.dispatchEvent(new PointerEvent("pointerdown", {
        bubbles: true,
        pointerId: 2,
        clientX: 300,
      }));
    });
  }
  expect(setPointerCapture).not.toHaveBeenCalled();
  expect(separator.getAttribute("aria-valuenow")).toBe("200");

  await act(async () => {
    separator.dispatchEvent(new PointerEvent("pointerdown", {
      bubbles: true,
      pointerId: 3,
      clientX: 300,
    }));
    separator.dispatchEvent(new PointerEvent("pointermove", {
      bubbles: true,
      pointerId: 3,
      clientX: 0,
    }));
  });
  expect(setPointerCapture).toHaveBeenCalledWith(3);
  expect(separator.dataset.interactionState).toBe("dragging");
  expect(separator.getAttribute("aria-valuenow")).toBe("100");

  await act(async () => {
    separator.dispatchEvent(new PointerEvent("pointermove", {
      bubbles: true,
      pointerId: 3,
      clientX: 1000,
    }));
  });
  expect(separator.getAttribute("aria-valuenow")).toBe("450");
  await act(async () => {
    separator.dispatchEvent(new PointerEvent("pointercancel", {
      bubbles: true,
      pointerId: 3,
    }));
  });
  expect(separator.dataset.interactionState).toBe("enabled");
  expect(releasePointerCapture).toHaveBeenCalledWith(3);

  separator.focus();
  await act(async () => {
    separator.dispatchEvent(new KeyboardEvent("keydown", { key: "Home", bubbles: true }));
  });
  expect(separator.getAttribute("aria-valuenow")).toBe("100");
  await act(async () => {
    separator.dispatchEvent(new KeyboardEvent("keydown", { key: "End", bubbles: true }));
  });
  expect(separator.getAttribute("aria-valuenow")).toBe("450");
  expect(document.activeElement).toBe(separator);

  await act(async () => {
    root.render(
      <SplitPane
        mode="vertical"
        initial={160}
        min={80}
        secondMin={120}
        first={<div />}
        second={<div />}
      />,
    );
  });
  const vertical = container.querySelector<HTMLElement>("[role='separator']")!;
  expect(vertical.getAttribute("aria-orientation")).toBe("horizontal");
  expect(vertical.getAttribute("aria-valuemax")).toBe("280");
  expect(vertical.style.height).toBe("24px");
});
