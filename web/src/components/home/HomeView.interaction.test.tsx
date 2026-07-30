// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  newProjectAndEnter: vi.fn(),
  openProjectPath: vi.fn(),
  openProjectViaDialog: vi.fn(),
}));

vi.mock("../../i18n", () => ({
  useT: () => (key: string) => key,
}));

vi.mock("../../lib/api", () => ({ isTauri: false }));

vi.mock("../../store/projectActions", () => ({
  newProjectAndEnter: mocks.newProjectAndEnter,
  openProjectPath: mocks.openProjectPath,
  openProjectViaDialog: mocks.openProjectViaDialog,
}));

import { useRecentStore } from "../../store/recentStore";
import { HomeView } from "./HomeView";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const PROJECT_PATH = "/tmp/Recent Demo.opentake";

let root: Root | null = null;
let container: HTMLDivElement | null = null;

beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  useRecentStore.setState({
    recents: [{ path: PROJECT_PATH, name: "Recent Demo", openedAt: 1 }],
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

describe("Home recent-project controls", () => {
  it("control-9697b53d4d2cf1ca select or open a recent project card", async () => {
    await act(async () => root?.render(<HomeView />));
    const card = container?.querySelector<HTMLButtonElement>(
      'button.home-project-card[aria-label="Recent Demo"]',
    );

    expect(card).not.toBeNull();
    expect(card?.tabIndex).toBe(0);
    expect(card?.getAttribute("aria-label")).toBe("Recent Demo");
    expect(card?.getAttribute("aria-pressed")).toBe("false");

    await act(async () => card?.focus());
    expect(card?.getAttribute("aria-pressed")).toBe("true");
    const sidebarNew = [...(container?.querySelectorAll<HTMLButtonElement>("button") ?? [])]
      .find((button) => button.textContent === "home.newProject");
    await act(async () => sidebarNew?.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Enter", bubbles: true }),
    ));
    expect(mocks.openProjectPath).not.toHaveBeenCalled();

    await act(async () => card?.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Enter", bubbles: true }),
    ));
    expect(mocks.openProjectPath).toHaveBeenCalledTimes(1);
    expect(mocks.openProjectPath).toHaveBeenLastCalledWith(PROJECT_PATH);

    mocks.openProjectPath.mockClear();
    await act(async () => card?.click());
    expect(card?.getAttribute("aria-pressed")).toBe("true");
    expect(mocks.openProjectPath).not.toHaveBeenCalled();

    await act(async () => card?.dispatchEvent(new MouseEvent("dblclick", { bubbles: true })));
    expect(mocks.openProjectPath).toHaveBeenCalledTimes(1);
    expect(mocks.openProjectPath).toHaveBeenLastCalledWith(PROJECT_PATH);
  });
});
