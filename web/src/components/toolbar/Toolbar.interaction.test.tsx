// @vitest-environment happy-dom

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const edit = vi.hoisted(() => ({
  undo: vi.fn<() => Promise<void>>(),
  redo: vi.fn<() => Promise<void>>(),
  splitAtPlayhead: vi.fn(),
  trimStartToPlayhead: vi.fn(),
  trimEndToPlayhead: vi.fn(),
  addTextClip: vi.fn(),
}));

vi.mock("../../i18n", () => ({
  useT: () => (key: string) => key,
}));

vi.mock("../../store/editActions", () => edit);

import { useEditorUiStore } from "../../store/uiStore";
import { useProjectStore } from "../../store/projectStore";
import { ZOOM } from "../../lib/theme";
import { Toolbar } from "./Toolbar";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

let root: Root | null = null;
let container: HTMLDivElement | null = null;

function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

function undoButton(): HTMLButtonElement {
  const button = container?.querySelector<HTMLButtonElement>(
    'button[aria-label="toolbar.undo"]',
  );
  if (!button) throw new Error("undo control was not rendered");
  return button;
}

function redoButton(): HTMLButtonElement {
  const button = container?.querySelector<HTMLButtonElement>(
    'button[aria-label="toolbar.redo"]',
  );
  if (!button) throw new Error("redo control was not rendered");
  return button;
}

beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  useEditorUiStore.setState({
    toast: null,
    toolMode: "pointer",
    minZoomScale: 0.05,
    zoomScale: ZOOM.default,
  });
  useProjectStore.setState({ canUndo: false, canRedo: false });
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
});

afterEach(async () => {
  if (root) await act(async () => root?.unmount());
  container?.remove();
  root = null;
  container = null;
});

describe("Toolbar command controls", () => {
  it("control-3be71cae61006e08 undo the last edit", async () => {
    await act(async () => root?.render(<Toolbar />));

    expect(undoButton().title).toBe("toolbar.undo");
    expect(undoButton().disabled).toBe(true);
    undoButton().click();
    undoButton().dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    expect(edit.undo).not.toHaveBeenCalled();

    await act(async () => useProjectStore.setState({ canUndo: true, canRedo: false }));
    undoButton().focus();
    const first = deferred();
    edit.undo.mockImplementationOnce(async () => {
      await first.promise;
      useProjectStore.setState({ canUndo: false, canRedo: true });
    });

    await act(async () => undoButton().click());
    expect(edit.undo).toHaveBeenCalledTimes(1);
    expect(undoButton().disabled).toBe(true);
    expect(undoButton().parentElement?.getAttribute("aria-busy")).toBe("true");
    undoButton().click();
    expect(edit.undo).toHaveBeenCalledTimes(1);
    expect(document.activeElement).toBe(undoButton());

    await act(async () => first.resolve());
    expect(useProjectStore.getState()).toMatchObject({ canUndo: false, canRedo: true });
    expect(undoButton().disabled).toBe(true);
    expect(undoButton().parentElement?.hasAttribute("aria-busy")).toBe(false);

    await act(async () => useProjectStore.setState({ canUndo: true }));
    edit.undo.mockRejectedValueOnce(new Error("history locked"));
    await act(async () => undoButton().click());
    expect(useEditorUiStore.getState().toast?.message).toContain("history locked");
    expect(undoButton().disabled).toBe(false);

    edit.undo.mockResolvedValueOnce();
    await act(async () => undoButton().click());
    expect(edit.undo).toHaveBeenCalledTimes(3);

    const webRoot = process.cwd();
    const toolbarSource = readFileSync(join(webRoot, "src/components/toolbar/Toolbar.tsx"), "utf8");
    const actionSource = readFileSync(join(webRoot, "src/store/editActions.ts"), "utf8");
    const apiSource = readFileSync(join(webRoot, "src/lib/api.ts"), "utf8");
    const rustSource = readFileSync(join(webRoot, "../src-tauri/src/commands.rs"), "utf8");
    expect(toolbarSource).toMatch(/await edit\.undo\(\)/);
    expect(actionSource).toMatch(/export async function undo\(\)[\s\S]*?await api\.undo\(\)/);
    expect(apiSource).toMatch(/export async function undo\(\)[\s\S]*?invokeImpl<EditResult>\("undo"\)/);
    expect(rustSource).toMatch(/pub fn undo[\s\S]*?handle_undo\(&core\)/);
  });

  it("control-b001ac6b21c97ad0 redo the last undone edit", async () => {
    await act(async () => root?.render(<Toolbar />));

    expect(redoButton().title).toBe("toolbar.redo");
    expect(redoButton().disabled).toBe(true);
    redoButton().click();
    redoButton().dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    expect(edit.redo).not.toHaveBeenCalled();

    await act(async () => useProjectStore.setState({ canUndo: false, canRedo: true }));
    redoButton().focus();
    const first = deferred();
    edit.redo.mockImplementationOnce(async () => {
      await first.promise;
      useProjectStore.setState({ canUndo: true, canRedo: false });
    });

    await act(async () => redoButton().click());
    expect(edit.redo).toHaveBeenCalledTimes(1);
    expect(redoButton().disabled).toBe(true);
    expect(redoButton().parentElement?.getAttribute("aria-busy")).toBe("true");
    redoButton().click();
    expect(edit.redo).toHaveBeenCalledTimes(1);
    expect(document.activeElement).toBe(redoButton());

    await act(async () => first.resolve());
    expect(useProjectStore.getState()).toMatchObject({ canUndo: true, canRedo: false });
    expect(redoButton().disabled).toBe(true);
    expect(redoButton().parentElement?.hasAttribute("aria-busy")).toBe(false);

    await act(async () => useProjectStore.setState({ canRedo: true }));
    edit.redo.mockRejectedValueOnce(new Error("redo history locked"));
    await act(async () => redoButton().click());
    expect(useEditorUiStore.getState().toast?.message).toContain("redo history locked");
    expect(redoButton().disabled).toBe(false);

    edit.redo.mockResolvedValueOnce();
    await act(async () => redoButton().click());
    expect(edit.redo).toHaveBeenCalledTimes(3);

    const webRoot = process.cwd();
    const toolbarSource = readFileSync(join(webRoot, "src/components/toolbar/Toolbar.tsx"), "utf8");
    const actionSource = readFileSync(join(webRoot, "src/store/editActions.ts"), "utf8");
    const apiSource = readFileSync(join(webRoot, "src/lib/api.ts"), "utf8");
    const rustSource = readFileSync(join(webRoot, "../src-tauri/src/commands.rs"), "utf8");
    expect(toolbarSource).toMatch(/await edit\.redo\(\)/);
    expect(actionSource).toMatch(/export async function redo\(\)[\s\S]*?await api\.redo\(\)/);
    expect(apiSource).toMatch(/export async function redo\(\)[\s\S]*?invokeImpl<EditResult>\("redo"\)/);
    expect(rustSource).toMatch(/pub fn redo[\s\S]*?handle_redo\(&core\)/);
  });

  it("control-9d69468ce3479312 switch to Pointer tool", async () => {
    useEditorUiStore.setState({ toolMode: "razor" });
    await act(async () => root?.render(<Toolbar />));

    const pointer = container?.querySelector<HTMLButtonElement>(
      'button[aria-label="toolbar.pointer"]',
    );
    expect(pointer).not.toBeNull();
    expect(pointer?.disabled).toBe(false);
    expect(pointer?.classList.contains("is-active")).toBe(false);

    pointer?.focus();
    await act(async () => pointer?.click());
    expect(useEditorUiStore.getState().toolMode).toBe("pointer");
    expect(pointer?.classList.contains("is-active")).toBe(true);
    expect(document.activeElement).toBe(pointer);
  });

  it("control-8105812f9d07bc93 switch to Razor tool", async () => {
    await act(async () => root?.render(<Toolbar />));

    const razor = container?.querySelector<HTMLButtonElement>(
      'button[aria-label="toolbar.razor"]',
    );
    expect(razor).not.toBeNull();
    expect(razor?.disabled).toBe(false);
    expect(razor?.classList.contains("is-active")).toBe(false);

    razor?.focus();
    await act(async () => razor?.click());
    expect(useEditorUiStore.getState().toolMode).toBe("razor");
    expect(razor?.classList.contains("is-active")).toBe(true);
    expect(document.activeElement).toBe(razor);
  });

  it("control-582e8fdf1d3d9e7e change timeline zoom", async () => {
    await act(async () => root?.render(<Toolbar />));

    const slider = container?.querySelector<HTMLInputElement>(
      'input[type="range"][aria-label="toolbar.zoom"]',
    );
    if (!slider) throw new Error("timeline zoom control was not rendered");
    expect(slider.disabled).toBe(false);

    slider.focus();
    await act(async () => {
      const valueSetter = Object.getOwnPropertyDescriptor(
        HTMLInputElement.prototype,
        "value",
      )?.set;
      if (!valueSetter) throw new Error("native range value setter is unavailable");
      valueSetter.call(slider, "0.75");
      slider.dispatchEvent(new Event("input", { bubbles: true }));
    });

    const expected = Math.exp(
      Math.log(0.05) + 0.75 * (Math.log(ZOOM.max) - Math.log(0.05)),
    );
    expect(useEditorUiStore.getState().zoomScale).toBeCloseTo(expected, 10);
    expect(Number(localStorage.getItem("opentake.ui.v1.zoomScale"))).toBeCloseTo(expected, 10);
    expect(document.activeElement).toBe(slider);
  });
});
