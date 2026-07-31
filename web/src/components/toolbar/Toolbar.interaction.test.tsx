// @vitest-environment happy-dom

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const edit = vi.hoisted(() => ({
  undo: vi.fn<() => Promise<void>>(),
  redo: vi.fn(),
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

beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  useEditorUiStore.setState({ toast: null });
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
});
