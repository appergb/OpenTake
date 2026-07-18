// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../i18n", () => ({
  useT: () => (key: string) => key,
}));

import { useEditorUiStore } from "../../store/uiStore";
import { TitleBar } from "./TitleBar";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

let root: Root | null = null;
let container: HTMLDivElement | null = null;

beforeEach(() => {
  localStorage.clear();
  useEditorUiStore.setState({ agentPanelVisible: false });
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
});

afterEach(async () => {
  if (root) await act(async () => root?.unmount());
  container?.remove();
  root = null;
  container = null;
  useEditorUiStore.setState({ agentPanelVisible: false });
});

describe("TitleBar Agent entry", () => {
  it("renders the specified visible toggle and drives the persisted panel state", async () => {
    await act(async () => root?.render(<TitleBar />));

    const button = container?.querySelector<HTMLButtonElement>(
      'button[aria-label="title.toggleAgent"]',
    );
    expect(button).not.toBeNull();
    expect(button?.getAttribute("aria-pressed")).toBe("false");
    expect(button?.style.opacity).toBe("0.55");
    expect(button?.querySelector("[data-agent-gradient-icon]")).not.toBeNull();

    await act(async () => button?.click());
    expect(useEditorUiStore.getState().agentPanelVisible).toBe(true);
    expect(button?.getAttribute("aria-pressed")).toBe("true");
    expect(button?.style.opacity).toBe("1");
    expect(localStorage.getItem("agentPanelVisible")).toBe("true");

    await act(async () => button?.click());
    expect(useEditorUiStore.getState().agentPanelVisible).toBe(false);
  });

  it("keeps the View menu as a second working mouse entry", async () => {
    await act(async () => root?.render(<TitleBar />));
    const menuButton = container?.querySelector<HTMLButtonElement>(
      'button[aria-label="view.menu"]',
    );
    expect(menuButton).not.toBeNull();

    await act(async () => menuButton?.click());
    const agentItem = [...(container?.querySelectorAll<HTMLButtonElement>('[role="menu"] button') ?? [])]
      .find((button) => button.textContent?.includes("view.agentPanel"));
    expect(agentItem).not.toBeUndefined();

    await act(async () => agentItem?.click());
    expect(useEditorUiStore.getState().agentPanelVisible).toBe(true);
  });
});
