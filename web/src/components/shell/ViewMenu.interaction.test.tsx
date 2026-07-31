// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../i18n", () => ({
  useT: () => (key: string) => key,
}));

import { useEditorUiStore } from "../../store/uiStore";
import { ViewMenu } from "./ViewMenu";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

let root: Root | null = null;
let container: HTMLDivElement | null = null;

function menuButton(label: string): HTMLButtonElement | undefined {
  return [...(container?.querySelectorAll<HTMLButtonElement>('[role="menu"] button') ?? [])]
    .find((button) => button.textContent?.includes(label));
}

async function renderAndOpen() {
  await act(async () => root?.render(<ViewMenu />));
  const trigger = container?.querySelector<HTMLButtonElement>('button[aria-label="view.menu"]');
  expect(trigger).not.toBeNull();
  await act(async () => trigger?.click());
  expect(trigger?.getAttribute("aria-expanded")).toBe("true");
  expect(container?.querySelector('[role="menu"]')).not.toBeNull();
  return trigger;
}

beforeEach(() => {
  localStorage.clear();
  useEditorUiStore.setState({
    layoutPreset: "default",
    agentPanelVisible: false,
    mediaPanelVisible: true,
    inspectorPanelVisible: true,
  });
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

describe("ViewMenu planned control acceptance", () => {
  it("control-d826a0ad433703cb open/close the View menu", async () => {
    const trigger = await renderAndOpen();

    await act(async () => trigger?.click());
    expect(trigger?.getAttribute("aria-expanded")).toBe("false");

    await act(async () => trigger?.click());
    await act(async () => window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" })));
    expect(trigger?.getAttribute("aria-expanded")).toBe("false");

    await act(async () => trigger?.click());
    await act(async () => window.dispatchEvent(new MouseEvent("mousedown", { bubbles: true })));
    expect(trigger?.getAttribute("aria-expanded")).toBe("false");
  });

  it("control-a2d1b5cb37952878 select a layout preset", async () => {
    const trigger = await renderAndOpen();
    const mediaLayout = menuButton("view.layoutMedia");
    expect(mediaLayout?.getAttribute("aria-checked")).toBe("false");

    await act(async () => mediaLayout?.click());

    expect(useEditorUiStore.getState().layoutPreset).toBe("media");
    expect(localStorage.getItem("opentake.ui.v1.layoutPreset")).toBe("media");
    expect(trigger?.getAttribute("aria-expanded")).toBe("false");
    expect(container?.querySelector('[role="menu"]')).toBeNull();
  });

  it("control-d606f9c3adb8762a toggle the Agent panel", async () => {
    await renderAndOpen();
    const agent = menuButton("view.agentPanel");
    expect(agent?.getAttribute("aria-checked")).toBe("false");

    await act(async () => agent?.click());

    expect(useEditorUiStore.getState().agentPanelVisible).toBe(true);
    expect(localStorage.getItem("opentake.ui.v1.agentPanelVisible")).toBe("true");
    expect(agent?.getAttribute("aria-checked")).toBe("true");
    expect(container?.querySelector('[role="menu"]')).not.toBeNull();
  });

  it("control-acd9e30dacf466b6 toggle the Media panel", async () => {
    await renderAndOpen();
    const media = menuButton("view.mediaPanel");
    expect(media?.getAttribute("aria-checked")).toBe("true");

    await act(async () => media?.click());

    expect(useEditorUiStore.getState().mediaPanelVisible).toBe(false);
    expect(localStorage.getItem("opentake.ui.v1.mediaPanelVisible")).toBe("false");
    expect(media?.getAttribute("aria-checked")).toBe("false");
    expect(container?.querySelector('[role="menu"]')).not.toBeNull();
  });

  it("control-fb7422e825128973 toggle the Inspector panel", async () => {
    await renderAndOpen();
    const inspector = menuButton("view.inspector");
    expect(inspector?.getAttribute("aria-checked")).toBe("true");

    await act(async () => inspector?.click());

    expect(useEditorUiStore.getState().inspectorPanelVisible).toBe(false);
    expect(localStorage.getItem("opentake.ui.v1.inspectorPanelVisible")).toBe("false");
    expect(inspector?.getAttribute("aria-checked")).toBe("false");
    expect(container?.querySelector('[role="menu"]')).not.toBeNull();
  });
});
