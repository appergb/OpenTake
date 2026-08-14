// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  generationLog: vi.fn(async () => ({ version: 1, entries: [] })),
  newProjectAndEnter: vi.fn(),
  openProjectPath: vi.fn(),
  openProjectViaDialog: vi.fn(),
}));

vi.mock("../../i18n", () => ({
  useT: () => (key: string) => key,
}));

vi.mock("../../lib/api", () => ({
  isTauri: false,
  generationLog: mocks.generationLog,
}));

vi.mock("../../store/projectActions", () => ({
  newProjectAndEnter: mocks.newProjectAndEnter,
  openProjectPath: mocks.openProjectPath,
  openProjectViaDialog: mocks.openProjectViaDialog,
}));

import { useRecentStore } from "../../store/recentStore";
import { useEditorUiStore } from "../../store/uiStore";
import {
  HOME_NOTICE_STORAGE_KEY,
  HOME_NOTICE_VERSION,
  HomeView,
} from "./HomeView";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const PROJECT_PATH = "/tmp/Recent Demo.opentake";

let root: Root | null = null;
let container: HTMLDivElement | null = null;

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

function buttonsNamed(label: string): HTMLButtonElement[] {
  return [...(container?.querySelectorAll<HTMLButtonElement>("button") ?? [])]
    .filter((button) => button.textContent === label);
}

beforeEach(() => {
  vi.clearAllMocks();
  mocks.generationLog.mockResolvedValue({ version: 1, entries: [] });
  mocks.newProjectAndEnter.mockResolvedValue(undefined);
  mocks.openProjectPath.mockResolvedValue(undefined);
  mocks.openProjectViaDialog.mockResolvedValue(undefined);
  localStorage.clear();
  localStorage.setItem(HOME_NOTICE_STORAGE_KEY, HOME_NOTICE_VERSION);
  useRecentStore.setState({
    recents: [{ path: PROJECT_PATH, name: "Recent Demo", openedAt: 1 }],
    thumbnailPathsValidated: true,
  });
  useEditorUiStore.setState({ view: "home", settingsOpen: false });
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
  it("control-e2d0f1ed3415ea45 create a new project from the Home sidebar", async () => {
    const pending = deferred<void>();
    mocks.newProjectAndEnter.mockReturnValueOnce(pending.promise);
    await act(async () => root?.render(<HomeView />));
    const sidebarNew = buttonsNamed("home.newProject")[0];

    await act(async () => sidebarNew?.click());

    expect(mocks.newProjectAndEnter).toHaveBeenCalledTimes(1);
    expect(buttonsNamed("home.creating")).toHaveLength(2);
    expect(buttonsNamed("home.creating").every((button) => button.disabled)).toBe(true);
    await act(async () => pending.resolve());
    await vi.waitFor(() => expect(buttonsNamed("home.newProject")).toHaveLength(2));
  });

  it("control-575978f9bced5959 create a new project from the empty launcher", async () => {
    const pending = deferred<void>();
    mocks.newProjectAndEnter.mockReturnValueOnce(pending.promise);
    useRecentStore.setState({ recents: [] });
    await act(async () => root?.render(<HomeView />));
    const emptyNew = buttonsNamed("home.newProject")[1];

    await act(async () => emptyNew?.click());

    expect(mocks.newProjectAndEnter).toHaveBeenCalledTimes(1);
    expect(buttonsNamed("home.creating")).toHaveLength(2);
    expect(buttonsNamed("home.openProject").every((button) => button.disabled)).toBe(true);
    await act(async () => pending.resolve());
    await vi.waitFor(() => expect(buttonsNamed("home.newProject")).toHaveLength(2));
  });

  it("control-74f414d717baaed3 create a new project from the populated launcher", async () => {
    const pending = deferred<void>();
    mocks.newProjectAndEnter.mockReturnValueOnce(pending.promise);
    await act(async () => root?.render(<HomeView />));
    const populatedNew = buttonsNamed("home.newProject")[1];

    await act(async () => populatedNew?.click());

    expect(mocks.newProjectAndEnter).toHaveBeenCalledTimes(1);
    expect(buttonsNamed("home.creating")).toHaveLength(2);
    expect(container?.querySelector<HTMLButtonElement>("button.home-project-card")?.disabled).toBe(
      true,
    );
    await act(async () => pending.resolve());
    await vi.waitFor(() => expect(buttonsNamed("home.newProject")).toHaveLength(2));
  });

  it("restores all project controls after new-project creation rejects", async () => {
    mocks.newProjectAndEnter.mockRejectedValueOnce(new Error("project create failed"));
    await act(async () => root?.render(<HomeView />));

    await act(async () => buttonsNamed("home.newProject")[0]?.click());

    expect(mocks.newProjectAndEnter).toHaveBeenCalledTimes(1);
    expect(buttonsNamed("home.newProject")).toHaveLength(2);
    expect(buttonsNamed("home.newProject").every((button) => !button.disabled)).toBe(true);
    expect(buttonsNamed("home.openProject").every((button) => !button.disabled)).toBe(true);
    expect(container?.querySelector<HTMLButtonElement>("button.home-project-card")?.disabled).toBe(
      false,
    );
  });

  it("control-ef78873f98fcab84 open a project from the Home sidebar", async () => {
    const pending = deferred<void>();
    mocks.openProjectViaDialog.mockReturnValueOnce(pending.promise);
    await act(async () => root?.render(<HomeView />));
    const sidebarOpen = buttonsNamed("home.openProject")[0];

    await act(async () => sidebarOpen?.click());
    expect(mocks.openProjectViaDialog).toHaveBeenCalledTimes(1);
    expect(sidebarOpen?.textContent).toBe("home.opening");
    expect(sidebarOpen?.disabled).toBe(true);
    expect(buttonsNamed("home.opening")).toHaveLength(2);
    expect(buttonsNamed("home.opening").every((button) => button.disabled)).toBe(true);
    expect(buttonsNamed("home.newProject").every((button) => button.disabled)).toBe(true);
    const recent = container?.querySelector<HTMLButtonElement>("button.home-project-card");
    expect(recent?.disabled).toBe(true);

    await act(async () => buttonsNamed("home.opening")[1]?.click());
    expect(mocks.openProjectViaDialog).toHaveBeenCalledTimes(1);

    await act(async () => pending.resolve());
    await vi.waitFor(() => expect(sidebarOpen?.textContent).toBe("home.openProject"));
    expect(sidebarOpen?.disabled).toBe(false);
    expect(buttonsNamed("home.newProject").every((button) => !button.disabled)).toBe(true);
    expect(recent?.disabled).toBe(false);
  });

  it("control-2121d7b9fdc279b9 open a project from the empty launcher", async () => {
    const pending = deferred<void>();
    mocks.openProjectViaDialog.mockReturnValueOnce(pending.promise);
    useRecentStore.setState({ recents: [] });
    await act(async () => root?.render(<HomeView />));
    const emptyOpen = buttonsNamed("home.openProject")[1];

    await act(async () => emptyOpen?.click());
    expect(mocks.openProjectViaDialog).toHaveBeenCalledTimes(1);
    expect(emptyOpen?.textContent).toBe("home.opening");
    expect(emptyOpen?.disabled).toBe(true);

    await act(async () => pending.resolve());
    await vi.waitFor(() => expect(emptyOpen?.textContent).toBe("home.openProject"));
    expect(emptyOpen?.disabled).toBe(false);
  });

  it("control-ab109c708bb0efbf open a project from the populated launcher", async () => {
    const pending = deferred<void>();
    mocks.openProjectViaDialog.mockReturnValueOnce(pending.promise);
    await act(async () => root?.render(<HomeView />));
    const populatedOpen = buttonsNamed("home.openProject")[1];

    await act(async () => populatedOpen?.click());
    expect(mocks.openProjectViaDialog).toHaveBeenCalledTimes(1);
    expect(populatedOpen?.textContent).toBe("home.opening");
    expect(populatedOpen?.disabled).toBe(true);

    await act(async () => pending.resolve());
    await vi.waitFor(() => expect(populatedOpen?.textContent).toBe("home.openProject"));
    expect(populatedOpen?.disabled).toBe(false);
  });

  it("restores an open control after the native project command rejects", async () => {
    const failure = new Error("project open timed out");
    mocks.openProjectViaDialog.mockRejectedValueOnce(failure);
    await act(async () => root?.render(<HomeView />));
    const sidebarOpen = buttonsNamed("home.openProject")[0];

    await act(async () => sidebarOpen?.click());

    expect(mocks.openProjectViaDialog).toHaveBeenCalledTimes(1);
    expect(sidebarOpen?.textContent).toBe("home.openProject");
    expect(sidebarOpen?.disabled).toBe(false);
  });

  it("serializes a recent-card open through the shared Home pending state", async () => {
    const pending = deferred<void>();
    mocks.openProjectPath.mockReturnValueOnce(pending.promise);
    await act(async () => root?.render(<HomeView />));
    const card = container?.querySelector<HTMLButtonElement>("button.home-project-card");

    await act(async () =>
      card?.dispatchEvent(new MouseEvent("dblclick", { bubbles: true })),
    );

    expect(mocks.openProjectPath).toHaveBeenCalledTimes(1);
    expect(buttonsNamed("home.opening")).toHaveLength(2);
    expect(card?.disabled).toBe(true);

    await act(async () =>
      card?.dispatchEvent(new MouseEvent("dblclick", { bubbles: true })),
    );
    expect(mocks.openProjectPath).toHaveBeenCalledTimes(1);

    await act(async () => pending.resolve());
    await vi.waitFor(() => expect(card?.disabled).toBe(false));
    expect(buttonsNamed("home.openProject")).toHaveLength(2);
  });

  it("control-f4a6b4f8789ea013 open the global Library from Home", async () => {
    await act(async () => root?.render(<HomeView />));
    const library = [...(container?.querySelectorAll<HTMLButtonElement>("button") ?? [])]
      .find((button) => button.textContent === "library.entry");

    expect(library).not.toBeUndefined();
    await act(async () => library?.click());
    expect(useEditorUiStore.getState().view).toBe("library");
  });

  it("control-810a7d793fcd8323 open Settings from Home", async () => {
    await act(async () => root?.render(<HomeView />));
    const settings = [...(container?.querySelectorAll<HTMLButtonElement>("button") ?? [])]
      .find((button) => button.textContent === "home.settings");

    expect(settings).not.toBeUndefined();
    await act(async () => settings?.click());
    expect(useEditorUiStore.getState().settingsOpen).toBe(true);
  });

  it("control-acd6238c08e790cc clear recent-project card selection", async () => {
    await act(async () => root?.render(<HomeView />));
    const card = container?.querySelector<HTMLButtonElement>(
      'button.home-project-card[aria-label="Recent Demo"]',
    );
    const launcher = card?.parentElement?.parentElement?.parentElement;

    expect(launcher).not.toBeNull();
    await act(async () => card?.click());
    expect(card?.getAttribute("aria-pressed")).toBe("true");
    await act(async () => launcher?.click());
    expect(card?.getAttribute("aria-pressed")).toBe("false");
  });

  it("keeps the project context menu available from the semantic preview card", async () => {
    await act(async () => root?.render(<HomeView />));
    const card = container?.querySelector<HTMLButtonElement>(
      'button.home-project-card[aria-label="Recent Demo"]',
    );
    const preview = card?.querySelector<HTMLElement>("figure.home-project-preview");

    expect(preview?.tagName).toBe("FIGURE");
    await act(async () => preview?.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true })));

    expect(container?.querySelector('[role="dialog"][aria-label="home.projectActions"]')).not.toBeNull();
  });

  it("control-ec1cd7a2d49bb97a remove a recent project entry", async () => {
    await act(async () => root?.render(<HomeView />));
    const card = container?.querySelector<HTMLButtonElement>(
      'button.home-project-card[aria-label="Recent Demo"]',
    );
    const wrapper = card?.parentElement;

    await act(async () => wrapper?.dispatchEvent(new MouseEvent("mouseover", { bubbles: true })));
    const remove = container?.querySelector<HTMLButtonElement>(
      'button[aria-label="home.remove"]',
    );
    expect(remove).not.toBeNull();

    await act(async () => remove?.click());
    expect(useRecentStore.getState().recents).toEqual([]);
    expect(container?.querySelector("button.home-project-card")).toBeNull();
    expect(mocks.openProjectPath).not.toHaveBeenCalled();
  });

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
