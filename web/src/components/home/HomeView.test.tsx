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

vi.mock("../../i18n", () => ({ useT: () => (key: string) => key }));
vi.mock("../../lib/api", () => ({ isTauri: false }));
vi.mock("../../store/projectActions", () => mocks);

import { useRecentStore } from "../../store/recentStore";
import { HomeView } from "./HomeView";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

let root: Root;
let container: HTMLDivElement;

beforeEach(() => {
  vi.clearAllMocks();
  for (const mock of Object.values(mocks)) mock.mockResolvedValue(undefined);
  localStorage.clear();
  useRecentStore.setState({
    recents: [{ path: "/tmp/Existing.opentake", name: "Existing", openedAt: 1 }],
  });
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
});

afterEach(async () => {
  await act(async () => root.unmount());
  container.remove();
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
