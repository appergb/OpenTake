// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  newProjectAndEnter: vi.fn(),
  openProjectPath: vi.fn(),
  openProjectViaDialog: vi.fn(),
  openSampleProject: vi.fn(),
}));
const i18nMocks = vi.hoisted(() => ({
  t: vi.fn((key: string) => key),
}));

vi.mock("../../i18n", () => ({ useT: () => i18nMocks.t }));
vi.mock("../../lib/api", () => ({
  isTauri: false,
  generationLog: async () => ({ version: 1, entries: [] }),
}));
vi.mock("../../lib/asset", () => ({ assetUrl: (path: string | null) => path }));
vi.mock("../../store/projectActions", () => mocks);

import { useRecentStore } from "../../store/recentStore";
import { useEditorUiStore } from "../../store/uiStore";
import {
  HOME_NOTICE_STORAGE_KEY,
  HOME_NOTICE_VERSION,
  HomeView,
} from "./HomeView";
import { DICTS } from "../../i18n/dict";

const defaultValidateRecents = useRecentStore.getState().validateRecents;

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

let root: Root;
let container: HTMLDivElement;

beforeEach(() => {
  vi.clearAllMocks();
  for (const mock of Object.values(mocks)) mock.mockResolvedValue(undefined);
  localStorage.clear();
  localStorage.setItem(HOME_NOTICE_STORAGE_KEY, HOME_NOTICE_VERSION);
  useRecentStore.setState({
    recents: [{ path: "/tmp/Existing.opentake", name: "Existing", openedAt: 1 }],
    thumbnailPathsValidated: true,
    validateRecents: defaultValidateRecents,
  });
  useEditorUiStore.setState({ view: "home" });
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
});

afterEach(async () => {
  await act(async () => root.unmount());
  container.remove();
});

it("localizes the complete recent-project count in both supported locales", async () => {
  await act(async () => root.render(<HomeView />));

  expect(i18nMocks.t).toHaveBeenCalledWith("home.recentCount", { count: 1 });
  expect(container.textContent).not.toContain("1 recent");
  expect(DICTS["zh-CN"]["home.recentCount"]).toBe("{count} 个最近项目");
  expect(DICTS.en["home.recentCount"]).toBe("{count} recent");
});

it("does not let a kept-alive hidden Home handle Enter or Escape", async () => {
  localStorage.removeItem(HOME_NOTICE_STORAGE_KEY);
  await act(async () => root.render(<HomeView />));
  const card = container.querySelector<HTMLButtonElement>('button[aria-label="Existing"]')!;
  await act(async () => card.focus());

  await act(async () => useEditorUiStore.getState().setView("library"));
  await act(async () => window.dispatchEvent(
    new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true }),
  ));
  await act(async () => window.dispatchEvent(
    new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }),
  ));

  expect(mocks.openProjectPath).not.toHaveBeenCalled();
  expect(localStorage.getItem(HOME_NOTICE_STORAGE_KEY)).toBeNull();
});

it("new_open_sample_register_only_after_success_and_route_tutorial", async () => {
  let finish!: () => void;
  mocks.openSampleProject.mockReturnValueOnce(new Promise<void>((resolve) => {
    finish = resolve;
  }));
  await act(async () => root.render(<HomeView />));

  const tutorial = [...container.querySelectorAll<HTMLButtonElement>("button")]
    .find((button) => button.textContent === "home.sampleTutorial");
  expect(tutorial).toBeDefined();

  await act(async () => tutorial?.click());
  expect(mocks.openSampleProject).toHaveBeenCalledWith("quick-tutorial", true);
  expect(useRecentStore.getState().recents.map(({ name }) => name)).toEqual(["Existing"]);
  expect(tutorial?.disabled).toBe(true);

  await act(async () => finish());
  await vi.waitFor(() => expect(tutorial?.disabled).toBe(false));
  expect(useRecentStore.getState().recents.map(({ name }) => name)).toEqual(["Existing"]);
});

it("project_card_renders_thumbnail_relative_time_and_missing_state", async () => {
  useRecentStore.setState({
    recents: [{
      path: "/tmp/Metadata.opentake",
      name: "Metadata",
      openedAt: Date.now() - 86_400_000,
      modifiedAt: Date.now(),
      thumbnailPath: "/tmp/Metadata.opentake/thumbnail.jpg",
      missing: false,
    }],
  });
  await act(async () => root.render(<HomeView />));

  expect(container.querySelector<HTMLImageElement>("img")?.src).toContain("thumbnail.jpg");
  expect(container.textContent).toContain("home.relative.today");

  act(() => useRecentStore.setState({
    recents: [{
      path: "/tmp/Metadata.opentake",
      name: "Metadata",
      openedAt: Date.now() - 86_400_000,
      modifiedAt: Date.now(),
      thumbnailPath: "/tmp/Metadata.opentake/thumbnail.jpg",
      missing: true,
    }],
  }));
  await vi.waitFor(() => expect(container.textContent).toContain("home.fileMissing"));
  expect(container.querySelector("img")).toBeNull();
});

it("never_mounts_a_cached_thumbnail_before_native_path_validation", async () => {
  useRecentStore.setState({
    recents: [{
      path: "/tmp/Cloud.opentake",
      name: "Cloud",
      openedAt: 1,
      thumbnailPath: "/tmp/Cloud.opentake/thumbnail.jpg",
      missing: false,
    }],
    thumbnailPathsValidated: false,
  });
  await act(async () => root.render(<HomeView />));

  expect(container.querySelector("img")).toBeNull();

  act(() => useRecentStore.setState({ thumbnailPathsValidated: true }));
  await vi.waitFor(() => expect(container.querySelector("img")?.getAttribute("src"))
    .toContain("thumbnail.jpg"));
});

it("cloud_only_project_is_accessible_but_cannot_open_or_mount_its_thumbnail", async () => {
  useRecentStore.setState({
    recents: [{
      path: "/tmp/Cloud.opentake",
      name: "Cloud",
      openedAt: 1,
      thumbnailPath: "/tmp/Cloud.opentake/thumbnail.jpg",
      missing: false,
      offline: true,
    }],
    thumbnailPathsValidated: true,
  });
  await act(async () => root.render(<HomeView />));

  const card = container.querySelector<HTMLButtonElement>(
    'button[aria-label="Cloud · home.fileOffline"]',
  );
  expect(card).not.toBeNull();
  expect(container.querySelector("img")).toBeNull();
  await act(async () => card?.dispatchEvent(new MouseEvent("dblclick", { bubbles: true })));
  await act(async () => card?.focus());
  await act(async () => window.dispatchEvent(
    new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true }),
  ));
  expect(mocks.openProjectPath).not.toHaveBeenCalled();
});

it("revalidates_an_offline_project_when_the_window_regains_focus", async () => {
  const validateRecents = vi
    .fn()
    .mockResolvedValueOnce(undefined)
    .mockImplementationOnce(async () => {
      useRecentStore.setState({
        recents: [{
          path: "/tmp/Downloaded.opentake",
          name: "Downloaded",
          openedAt: 1,
          missing: false,
          offline: false,
        }],
      });
    });
  useRecentStore.setState({
    recents: [{
      path: "/tmp/Downloaded.opentake",
      name: "Downloaded",
      openedAt: 1,
      missing: false,
      offline: true,
    }],
    validateRecents,
  });
  await act(async () => root.render(<HomeView />));
  await vi.waitFor(() => expect(validateRecents).toHaveBeenCalledTimes(1));

  await act(async () => window.dispatchEvent(new Event("focus")));
  await vi.waitFor(() => expect(validateRecents).toHaveBeenCalledTimes(2));
  const card = container.querySelector<HTMLButtonElement>('button[aria-label="Downloaded"]');
  expect(card).not.toBeNull();
  await act(async () => card?.dispatchEvent(new MouseEvent("dblclick", { bubbles: true })));

  expect(mocks.openProjectPath).toHaveBeenCalledWith("/tmp/Downloaded.opentake");
});

it("missing_card_reveal_remove_and_trash_states", async () => {
  const reveal = vi.fn().mockResolvedValue(undefined);
  const trash = vi
    .fn()
    .mockRejectedValueOnce(new Error("permission denied"))
    .mockImplementationOnce(async () => {
      useRecentStore.setState({ recents: [] });
    });
  const remove = vi
    .fn()
    .mockRejectedValueOnce(new Error("registry is read-only"))
    .mockImplementationOnce(async () => {
      useRecentStore.setState({ recents: [] });
    });
  useRecentStore.setState({
    recents: [{
      path: "/tmp/Missing.opentake",
      name: "Missing",
      openedAt: 1,
      missing: true,
    }],
    reveal,
    trash,
    remove,
  });
  await act(async () => root.render(<HomeView />));

  expect(container.textContent).toContain("home.fileMissing");
  await act(async () => container.querySelector<HTMLButtonElement>(
    "button[aria-label='home.projectActions']",
  )?.click());
  expect(container.textContent).toContain("home.revealInFinder");
  expect(container.textContent).toContain("home.removeFromRecents");
  expect(container.textContent).toContain("home.moveToTrash");
  const actionDialog = container.querySelector<HTMLElement>('[role="dialog"]');
  expect(
    [...(actionDialog?.querySelectorAll<HTMLButtonElement>("button") ?? [])].every(
      (button) => Number.parseInt(button.style.minHeight, 10) >= 24,
    ),
  ).toBe(true);

  await act(async () => [...container.querySelectorAll<HTMLButtonElement>("button")]
    .find((button) => button.textContent?.includes("home.revealInFinder"))?.click());
  expect(reveal).toHaveBeenCalledWith("/tmp/Missing.opentake");

  await act(async () => container.querySelector<HTMLButtonElement>(
    "button[aria-label='home.projectActions']",
  )?.click());
  await act(async () => [...container.querySelectorAll<HTMLButtonElement>("button")]
    .find((button) => button.textContent?.includes("home.moveToTrash"))?.click());
  expect(container.textContent).toContain("home.confirmTrashBody");
  await act(async () => [...container.querySelectorAll<HTMLButtonElement>("button")]
    .find((button) => button.textContent?.includes("home.moveToTrash"))?.click());
  await vi.waitFor(() => expect(container.textContent).toContain("home.trashFailed"));
  expect(useRecentStore.getState().recents).toHaveLength(1);

  await act(async () => [...container.querySelectorAll<HTMLButtonElement>("button")]
    .find((button) => button.textContent?.includes("home.moveToTrash"))?.click());
  await vi.waitFor(() => expect(useRecentStore.getState().recents).toEqual([]));

  act(() => useRecentStore.setState({
    recents: [{
      path: "/tmp/Missing.opentake",
      name: "Missing",
      openedAt: 1,
      missing: true,
    }],
  }));
  await vi.waitFor(() => expect(container.textContent).toContain("home.fileMissing"));
  await act(async () => container.querySelector<HTMLButtonElement>(
    "button[aria-label='home.projectActions']",
  )?.click());
  await act(async () => [...container.querySelectorAll<HTMLButtonElement>("button")]
    .find((button) => button.textContent === "home.removeFromRecents")?.click());
  await vi.waitFor(() => expect(container.textContent).toContain("home.removeFailed"));
  expect(useRecentStore.getState().recents).toHaveLength(1);
  await act(async () => [...container.querySelectorAll<HTMLButtonElement>("button")]
    .find((button) => button.textContent === "home.removeFromRecents")?.click());
  await vi.waitFor(() => expect(remove).toHaveBeenCalledTimes(2));
  expect(useRecentStore.getState().recents).toEqual([]);
});

it("upstream_home_children_close_one_composite_acceptance", async () => {
  localStorage.removeItem(HOME_NOTICE_STORAGE_KEY);
  useRecentStore.setState({ recents: [] });
  await act(async () => root.render(<HomeView />));

  expect(container.querySelector("aside")).not.toBeNull();
  expect(container.textContent).toContain("home.samples");
  expect(container.textContent).toContain("home.sampleDemo");
  expect(container.textContent).toContain("home.welcome");
  expect(container.querySelector('[role="dialog"][aria-modal="true"]')?.textContent).toContain(
    "home.welcomeOverlayTitle",
  );
  const welcomeDismiss = [...container.querySelectorAll<HTMLButtonElement>("button")]
    .find((button) => button.textContent === "home.welcomeOverlayStart");
  expect(document.activeElement).toBe(welcomeDismiss);
  await act(async () => window.dispatchEvent(
    new KeyboardEvent("keydown", { key: "Escape", bubbles: true }),
  ));
  expect(localStorage.getItem(HOME_NOTICE_STORAGE_KEY)).toBe(HOME_NOTICE_VERSION);
  expect(container.querySelector('[role="dialog"][aria-modal="true"]')).toBeNull();

  await act(async () => root.unmount());
  container.replaceChildren();
  root = createRoot(container);
  localStorage.setItem(HOME_NOTICE_STORAGE_KEY, "0.9.0");
  const reveal = vi.fn().mockResolvedValue(undefined);
  const trash = vi.fn().mockResolvedValue(undefined);
  useRecentStore.setState({
    recents: [
      { path: "/tmp/Existing.opentake", name: "Existing", openedAt: Date.now() },
      { path: "/tmp/Missing.opentake", name: "Missing", openedAt: 1, missing: true },
    ],
    reveal,
    trash,
  });
  await act(async () => root.render(<HomeView />));

  const updateDialog = container.querySelector('[role="dialog"][aria-modal="true"]');
  expect(updateDialog?.textContent).toContain("home.newInVersion");
  expect(updateDialog?.textContent).toContain("home.updateOverlayBody");
  await act(async () => [...container.querySelectorAll<HTMLButtonElement>("button")]
    .find((button) => button.textContent === "home.updateOverlayDismiss")?.click());

  expect(container.querySelector<HTMLButtonElement>('button[aria-label="Existing"]')).not.toBeNull();
  const missingCard = container.querySelector<HTMLButtonElement>(
    'button[aria-label="Missing · home.fileMissing"]',
  );
  expect(missingCard).not.toBeNull();
  await act(async () => missingCard?.parentElement?.dispatchEvent(
    new MouseEvent("contextmenu", { bubbles: true }),
  ));
  expect(container.textContent).toContain("home.revealInFinder");
  expect(container.textContent).toContain("home.moveToTrash");
  await act(async () => [...container.querySelectorAll<HTMLButtonElement>("button")]
    .find((button) => button.textContent?.includes("home.moveToTrash"))?.click());
  expect(container.textContent).toContain("home.confirmTrashBody");
  await act(async () => [...container.querySelectorAll<HTMLButtonElement>("button")]
    .find((button) => button.textContent === "common.cancel")?.click());
  expect(trash).not.toHaveBeenCalled();

  const existingCard = container.querySelector<HTMLButtonElement>('button[aria-label="Existing"]')!;
  await act(async () => existingCard.focus());
  await act(async () => existingCard.dispatchEvent(
    new KeyboardEvent("keydown", { key: "Enter", bubbles: true }),
  ));
  expect(mocks.openProjectPath).toHaveBeenCalledWith("/tmp/Existing.opentake");

  await act(async () => [...container.querySelectorAll<HTMLButtonElement>("button")]
    .find((button) => button.textContent === "home.sampleDemo")?.click());
  expect(mocks.openSampleProject).toHaveBeenCalledWith("product-demo", false);
});
