// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it } from "vitest";
import { useEditorUiStore } from "../../store/uiStore";
import { PanelShell } from "./PanelShell";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

let container: HTMLDivElement;
let root: Root;

beforeEach(() => {
  useEditorUiStore.setState({
    focusedPanel: "timeline",
    selectedClipIds: new Set(["clip-1"]),
    selectedMediaAssetIds: new Set(["asset-1"]),
  });
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
  act(() => {
    root.render(
      <PanelShell panel="media">
        <button type="button">Media child</button>
      </PanelShell>,
    );
  });
});

afterEach(async () => {
  await act(async () => root.unmount());
  container.remove();
});

it("control-bbc125bbbf2275f2 focus an editor panel", async () => {
  const panel = container.querySelector<HTMLElement>("[data-editor-panel='media']")!;
  const ring = panel.querySelector<HTMLElement>("[data-panel-focus-ring]")!;
  expect(panel.getAttribute("role")).toBe("region");
  expect(panel.getAttribute("aria-label")).not.toBe("");
  expect(panel.tabIndex).toBe(-1);
  expect(panel.dataset.focused).toBe("false");
  expect(ring.style.opacity).toBe("0");

  await act(async () => {
    panel.dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
  });
  expect(useEditorUiStore.getState().focusedPanel).toBe("media");
  expect(useEditorUiStore.getState().selectedClipIds.size).toBe(0);
  expect(useEditorUiStore.getState().selectedMediaAssetIds).toEqual(new Set(["asset-1"]));
  expect(panel.dataset.focused).toBe("true");
  expect(ring.style.opacity).toBe("0.6");

  await act(async () => useEditorUiStore.setState({ focusedPanel: "timeline" }));
  // The named region remains programmatically focusable without adding a
  // redundant stop before its native buttons in the keyboard Tab order.
  await act(async () => panel.focus());
  expect(document.activeElement).toBe(panel);
  expect(useEditorUiStore.getState().focusedPanel).toBe("media");
  expect(panel.dataset.focused).toBe("true");
  expect(ring.style.opacity).toBe("0.6");
});
