// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { Dropdown } from "./Dropdown";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

let container: HTMLDivElement;
let outside: HTMLButtonElement;
let root: Root;
let onChange: ReturnType<typeof vi.fn>;

beforeEach(() => {
  container = document.createElement("div");
  outside = document.createElement("button");
  document.body.append(container, outside);
  root = createRoot(container);
  onChange = vi.fn();
  act(() => {
    root.render(
      <Dropdown
        value="one"
        options={[
          { id: "one", label: "One" },
          { id: "two", label: "Two" },
          { id: "disabled", label: "Disabled", disabled: true },
        ]}
        onChange={onChange}
        ariaLabel="Mode"
      />,
    );
  });
});

afterEach(async () => {
  await act(async () => root.unmount());
  container.remove();
  outside.remove();
  vi.restoreAllMocks();
});

it("control-f1370db38b24cf33 open/close a reusable enum Dropdown", async () => {
  const trigger = container.querySelector<HTMLButtonElement>("button[aria-haspopup='listbox']")!;
  expect(trigger.getAttribute("aria-label")).toBe("Mode");
  expect(trigger.getAttribute("aria-expanded")).toBe("false");

  trigger.focus();
  await act(async () => trigger.click());
  expect(trigger.getAttribute("aria-expanded")).toBe("true");
  expect(trigger.getAttribute("aria-controls")).not.toBeNull();
  expect(document.activeElement?.getAttribute("role")).toBe("option");
  expect(document.activeElement?.textContent).toContain("One");

  await act(async () => {
    outside.dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
  });
  expect(container.querySelector("[role='listbox']")).toBeNull();
  expect(trigger.getAttribute("aria-expanded")).toBe("false");
  expect(document.activeElement).toBe(trigger);

  await act(async () => trigger.click());
  const escape = new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true });
  await act(async () => document.dispatchEvent(escape));
  expect(escape.defaultPrevented).toBe(true);
  expect(container.querySelector("[role='listbox']")).toBeNull();
  expect(document.activeElement).toBe(trigger);

  await act(async () => trigger.click());
  expect(container.querySelector("[role='listbox']")).not.toBeNull();
  await act(async () => trigger.click());
  expect(container.querySelector("[role='listbox']")).toBeNull();
  expect(document.activeElement).toBe(trigger);
  expect(onChange).not.toHaveBeenCalled();
});
