// @vitest-environment happy-dom

import React from "react";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({ cancelSaveAsMedia: vi.fn() }));
vi.mock("../../store/editActions", () => ({ cancelSaveAsMedia: mocks.cancelSaveAsMedia }));

import { SaveAsProgressView } from "./SaveAsProgress";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

afterEach(() => document.body.replaceChildren());

describe("SaveAsProgress", () => {
  it("renders visible progress and an enabled cancel button", async () => {
    const progress = {
      operationId: "save-as:test",
      label: "Saving clip",
      done: 25,
      total: 100,
      cancellable: true,
      cancelling: false,
    };
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () => root.render(<SaveAsProgressView progress={progress} />));

    expect(container.textContent).toContain("Saving clip");
    expect(container.textContent).toContain("25%");
    expect(container.textContent).toContain("取消");
    const button = container.querySelector<HTMLButtonElement>("button")!;
    expect(button.disabled).toBe(false);

    await act(async () => button.dispatchEvent(new MouseEvent("click", { bubbles: true })));
    expect(mocks.cancelSaveAsMedia).toHaveBeenCalledTimes(1);
    await act(async () => root.unmount());
  });
});
