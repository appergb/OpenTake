// @vitest-environment happy-dom

import { act } from "react";
import { EditorSelection } from "@codemirror/state";
import { EditorView } from "codemirror";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MotionCodeEditor } from "./MotionCodeEditor";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

describe("MotionCodeEditor lifecycle", () => {
  let cleanup: (() => Promise<void>) | undefined;

  afterEach(async () => cleanup?.());

  it("keeps one controlled editor while swapping HTML/CSS language and disposes on unmount", async () => {
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    const onChange = vi.fn();
    cleanup = async () => {
      await act(async () => root.unmount());
      container.remove();
    };

    await act(async () => root.render(
      <MotionCodeEditor
        file="index.html"
        value="<main>Visible</main>"
        label="HTML and CSS editor"
        onChange={onChange}
      />,
    ));
    const editor = container.querySelector(".cm-editor");
    expect(editor).not.toBeNull();
    expect(container.querySelectorAll(".cm-editor")).toHaveLength(1);
    expect(container.querySelector(".cm-content")?.textContent).toBe("<main>Visible</main>");
    expect(container.querySelector(".cm-content")?.getAttribute("aria-label")).toBe("HTML and CSS editor");
    const view = EditorView.findFromDOM(container.querySelector(".cm-content")!);
    view.dispatch({ selection: EditorSelection.single(13, 6) });

    await act(async () => root.render(
      <MotionCodeEditor
        file="styles.css"
        value="main { color: white; }"
        label="Styles editor"
        onChange={onChange}
      />,
    ));
    expect(container.querySelectorAll(".cm-editor")).toHaveLength(1);
    expect(container.querySelector(".cm-content")?.textContent).toBe("main { color: white; }");
    expect(container.querySelector(".cm-content")?.getAttribute("aria-label")).toBe("Styles editor");
    expect(onChange).not.toHaveBeenCalled();

    view.dispatch({ selection: EditorSelection.single(2, 8) });
    await act(async () => root.render(
      <MotionCodeEditor
        file="index.html"
        value="<main>Visible again</main>"
        label="HTML and CSS editor"
        onChange={onChange}
      />,
    ));
    expect(view.state.selection.main).toMatchObject({ anchor: 13, head: 6 });

    await act(async () => root.unmount());
    cleanup = async () => container.remove();
    expect(container.querySelector(".cm-editor")).toBeNull();
  });
});
