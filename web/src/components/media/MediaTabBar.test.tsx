// @vitest-environment happy-dom

import { act, useState } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { MediaSubTabId, MediaTabId } from "../../store/uiStore";
import { useEditorUiStore } from "../../store/uiStore";
import { PanelShell } from "../ui/PanelShell";
import {
  MATERIAL_SUB_TABS,
  MediaSubTabBar,
  MediaTabBar,
} from "./MediaTabBar";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

let container: HTMLDivElement;
let root: Root;

beforeEach(() => {
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
});

afterEach(async () => {
  await act(async () => root.unmount());
  container.remove();
  useEditorUiStore.setState({ selectedClipIds: new Set(), focusedPanel: null });
});

function MainHarness({ onSelect }: { onSelect: (tab: MediaTabId) => void }) {
  const [active, setActive] = useState<MediaTabId>("material");
  return (
    <MediaTabBar
      active={active}
      onSelect={(tab) => {
        setActive(tab);
        onSelect(tab);
      }}
    />
  );
}

function SubHarness({ onSelect }: { onSelect: (tab: MediaSubTabId) => void }) {
  const [active, setActive] = useState<MediaSubTabId>("import");
  return (
    <MediaSubTabBar
      active={active}
      tabs={MATERIAL_SUB_TABS}
      idPrefix="media-material-subtab"
      onSelect={(tab) => {
        setActive(tab);
        onSelect(tab);
      }}
    />
  );
}

describe("MediaTabBar keyboard semantics", () => {
  it("keeps_the_timeline_cut_selected_when_opening_the_transition_tab", async () => {
    const onSelect = vi.fn();
    useEditorUiStore.setState({
      selectedClipIds: new Set(["outgoing-clip"]),
      focusedPanel: "timeline",
    });
    await act(async () =>
      root.render(
        <PanelShell panel="media">
          <MainHarness onSelect={onSelect} />
        </PanelShell>,
      ),
    );
    const transitionTab = container.querySelector<HTMLButtonElement>(
      "#media-main-tab-transition",
    )!;

    await act(async () => {
      transitionTab.dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
      transitionTab.focus();
      transitionTab.dispatchEvent(new MouseEvent("mouseup", { bubbles: true }));
      transitionTab.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });

    expect(onSelect).toHaveBeenCalledWith("transition");
    expect(useEditorUiStore.getState().focusedPanel).toBe("media");
    expect(useEditorUiStore.getState().selectedClipIds).toEqual(new Set(["outgoing-clip"]));
  });

  it("keeps_the_cut_selected_when_keyboard_focus_roves_to_the_transition_tab", async () => {
    const onSelect = vi.fn();
    useEditorUiStore.setState({
      selectedClipIds: new Set(["outgoing-clip"]),
      focusedPanel: "timeline",
    });
    await act(async () =>
      root.render(
        <PanelShell panel="media">
          <MainHarness onSelect={onSelect} />
        </PanelShell>,
      ),
    );
    const panel = container.querySelector<HTMLElement>('[data-editor-panel="media"]')!;
    const materialTab = container.querySelector<HTMLButtonElement>(
      "#media-main-tab-material",
    )!;

    expect(materialTab.tabIndex).toBe(0);
    await act(async () => materialTab.focus());
    for (let step = 0; step < 6; step += 1) {
      await act(async () =>
        document.activeElement!.dispatchEvent(
          new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }),
        ),
      );
    }

    expect(document.activeElement?.id).toBe("media-main-tab-transition");
    expect(onSelect).toHaveBeenLastCalledWith("transition");
    expect(useEditorUiStore.getState().focusedPanel).toBe("media");
    expect(useEditorUiStore.getState().selectedClipIds).toEqual(new Set(["outgoing-clip"]));
    expect(panel.tabIndex).toBe(-1);
  });

  it("still_clears_timeline_selection_when_clicking_an_asset_tab", async () => {
    useEditorUiStore.setState({
      selectedClipIds: new Set(["timeline-clip"]),
      focusedPanel: "timeline",
    });
    await act(async () =>
      root.render(
        <PanelShell panel="media">
          <MainHarness onSelect={vi.fn()} />
        </PanelShell>,
      ),
    );
    const audioTab = container.querySelector<HTMLButtonElement>("#media-main-tab-audio")!;

    await act(async () => {
      audioTab.dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
      audioTab.focus();
      audioTab.dispatchEvent(new MouseEvent("mouseup", { bubbles: true }));
      audioTab.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });

    expect(useEditorUiStore.getState().selectedClipIds).toEqual(new Set());
  });

  it("still_clears_timeline_selection_when_non_tab_media_content_receives_focus", async () => {
    useEditorUiStore.setState({
      selectedClipIds: new Set(["timeline-clip"]),
      focusedPanel: "timeline",
    });
    await act(async () =>
      root.render(
        <PanelShell panel="media">
          <input aria-label="media search" />
        </PanelShell>,
      ),
    );

    await act(async () =>
      container.querySelector<HTMLInputElement>('input[aria-label="media search"]')!.focus(),
    );

    expect(useEditorUiStore.getState().selectedClipIds).toEqual(new Set());
  });

  it("clears_timeline_selection_when_accessibility_focus_lands_on_a_non_transition_media_panel", async () => {
    useEditorUiStore.setState({
      selectedClipIds: new Set(["timeline-clip"]),
      focusedPanel: "timeline",
    });
    await act(async () =>
      root.render(
        <PanelShell panel="media">
          <MainHarness onSelect={vi.fn()} />
        </PanelShell>,
      ),
    );

    await act(async () =>
      container.querySelector<HTMLElement>('[data-editor-panel="media"]')!.focus(),
    );

    expect(useEditorUiStore.getState().selectedClipIds).toEqual(new Set());
  });

  it("roves across enabled main tabs and skips disabled tabs", async () => {
    const onSelect = vi.fn();
    await act(async () => root.render(<MainHarness onSelect={onSelect} />));
    const tabs = [...container.querySelectorAll<HTMLButtonElement>('[role="tab"]')];
    expect(tabs[0]?.tabIndex).toBe(0);
    expect(tabs.slice(1).every((tab) => tab.tabIndex === -1)).toBe(true);
    expect(tabs[0]?.getAttribute("aria-controls")).toBe("media-main-panel-material");

    await act(async () =>
      tabs[0]?.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true })),
    );
    expect(onSelect).toHaveBeenLastCalledWith("audio");
    expect(document.activeElement?.textContent).toBe(tabs[1]?.textContent);

    await act(async () =>
      document.activeElement?.dispatchEvent(
        new KeyboardEvent("keydown", { key: "End", bubbles: true }),
      ),
    );
    expect(onSelect).toHaveBeenLastCalledWith("smartPack");
    expect(document.activeElement?.textContent).toBe(tabs.at(-1)?.textContent);

    await act(async () =>
      document.activeElement?.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Home", bubbles: true }),
      ),
    );
    expect(onSelect).toHaveBeenLastCalledWith("material");
  });

  it("exposes the text tab as an enabled editing surface", async () => {
    await act(async () => root.render(<MainHarness onSelect={vi.fn()} />));
    const textTab = container.querySelector<HTMLButtonElement>("#media-main-tab-text")!;

    expect(textTab.disabled).toBe(false);
    expect(textTab.getAttribute("aria-disabled")).toBe("false");
  });

  it("exposes the effect tab as an enabled editing surface", async () => {
    await act(async () => root.render(<MainHarness onSelect={vi.fn()} />));
    const effectTab = container.querySelector<HTMLButtonElement>("#media-main-tab-effect")!;

    expect(effectTab.disabled).toBe(false);
    expect(effectTab.getAttribute("aria-disabled")).toBe("false");
  });

  it("roves secondary tabs and keeps exactly one tab in the tab order", async () => {
    const onSelect = vi.fn();
    await act(async () => root.render(<SubHarness onSelect={onSelect} />));
    const tabs = [...container.querySelectorAll<HTMLButtonElement>('[role="tab"]')];
    expect(tabs.map((tab) => tab.tabIndex)).toEqual([0, -1]);
    expect(tabs[0]?.getAttribute("aria-controls")).toBe(
      "media-material-subtab-panel-import",
    );

    await act(async () =>
      tabs[0]?.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true })),
    );
    expect(onSelect).toHaveBeenLastCalledWith("mine");
    expect(document.activeElement?.textContent).toBe(tabs[1]?.textContent);
    expect(
      [...container.querySelectorAll<HTMLButtonElement>('[role="tab"]')].map(
        (tab) => tab.tabIndex,
      ),
    ).toEqual([-1, 0]);
  });
});
