// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { ScrubbableNumberField } from "./ScrubbableNumberField";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

let container: HTMLDivElement;
let root: Root;
let onChange: ReturnType<typeof vi.fn>;
let onCommit: ReturnType<typeof vi.fn>;

function renderField(): void {
  root.render(
    <ScrubbableNumberField
      ariaLabel="Opacity"
      value={5}
      min={0}
      max={10}
      sensitivity={1}
      format={(value) => value.toFixed(1)}
      suffix="%"
      onChange={onChange}
      onCommit={onCommit}
    />,
  );
}

async function enterTextMode(): Promise<HTMLInputElement> {
  const display = container.querySelector<HTMLElement>("[role='spinbutton']")!;
  await act(async () => {
    display.dispatchEvent(new PointerEvent("pointerdown", {
      bubbles: true,
      pointerId: 1,
      clientX: 10,
    }));
    display.dispatchEvent(new PointerEvent("pointerup", {
      bubbles: true,
      pointerId: 1,
      clientX: 10,
    }));
  });
  return container.querySelector<HTMLInputElement>("input[aria-label='Opacity']")!;
}

async function setInput(input: HTMLInputElement, value: string): Promise<void> {
  await act(async () => {
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set?.call(input, value);
    input.dispatchEvent(new InputEvent("input", { bubbles: true }));
  });
}

beforeEach(() => {
  onChange = vi.fn();
  onCommit = vi.fn();
  vi.spyOn(HTMLElement.prototype, "setPointerCapture").mockImplementation(() => {});
  vi.spyOn(HTMLElement.prototype, "releasePointerCapture").mockImplementation(() => {});
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
    callback(0);
    return 1;
  });
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
  act(() => renderField());
});

afterEach(async () => {
  await act(async () => root.unmount());
  container.remove();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

it("control-481c7d66573516a6 numeric text-entry mode", async () => {
  let input = await enterTextMode();
  expect(document.activeElement).toBe(input);
  await setInput(input, "12,5%");
  await act(async () => {
    input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
  });
  expect(onCommit).toHaveBeenCalledTimes(1);
  expect(onCommit).toHaveBeenCalledWith(10);
  expect(container.querySelector("input")).toBeNull();
  expect(document.activeElement).toBe(container.querySelector("[role='spinbutton']"));

  onCommit.mockClear();
  input = await enterTextMode();
  await setInput(input, "not-a-number");
  await act(async () => input.dispatchEvent(new FocusEvent("focusout", { bubbles: true })));
  expect(onCommit).not.toHaveBeenCalled();
  expect(container.querySelector("input")).toBeNull();

  input = await enterTextMode();
  await setInput(input, "7");
  await act(async () => {
    input.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
  });
  expect(onCommit).not.toHaveBeenCalled();
  expect(document.activeElement).toBe(container.querySelector("[role='spinbutton']"));
});

it("control-3e4fc80f4dde046e pointer-scrubbable numeric value", async () => {
  const display = container.querySelector<HTMLElement>("[role='spinbutton']")!;
  await act(async () => {
    display.dispatchEvent(new PointerEvent("pointerdown", {
      bubbles: true,
      pointerId: 2,
      clientX: 10,
    }));
    display.dispatchEvent(new PointerEvent("pointermove", {
      bubbles: true,
      pointerId: 2,
      clientX: 14,
    }));
    display.dispatchEvent(new PointerEvent("pointerup", {
      bubbles: true,
      pointerId: 2,
      clientX: 14,
    }));
  });
  expect(onChange).toHaveBeenCalledWith(9);
  expect(onCommit).toHaveBeenCalledTimes(1);
  expect(onCommit).toHaveBeenCalledWith(9);

  onChange.mockClear();
  onCommit.mockClear();
  await act(async () => {
    display.dispatchEvent(new PointerEvent("pointerdown", {
      bubbles: true,
      pointerId: 3,
      clientX: 10,
    }));
    display.dispatchEvent(new PointerEvent("pointermove", {
      bubbles: true,
      pointerId: 3,
      clientX: 14,
      shiftKey: true,
    }));
    display.dispatchEvent(new PointerEvent("pointercancel", {
      bubbles: true,
      pointerId: 3,
    }));
    display.dispatchEvent(new PointerEvent("pointerup", {
      bubbles: true,
      pointerId: 3,
      clientX: 14,
    }));
  });
  expect(onChange).toHaveBeenCalledWith(10);
  expect(onCommit).not.toHaveBeenCalled();
  expect(container.querySelector("input")).toBeNull();

  onChange.mockClear();
  await act(async () => {
    display.dispatchEvent(new PointerEvent("pointerdown", {
      bubbles: true,
      pointerId: 4,
      clientX: 10,
    }));
    display.dispatchEvent(new PointerEvent("pointermove", {
      bubbles: true,
      pointerId: 4,
      clientX: 20,
      metaKey: true,
    }));
    display.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
  });
  expect(onChange).toHaveBeenCalledWith(6);
  expect(onCommit).not.toHaveBeenCalled();
  expect(document.activeElement).toBe(display);
});
