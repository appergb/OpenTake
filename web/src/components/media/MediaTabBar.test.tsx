// @vitest-environment happy-dom

import { act, useState } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { MediaSubTabId, MediaTabId } from "../../store/uiStore";
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
