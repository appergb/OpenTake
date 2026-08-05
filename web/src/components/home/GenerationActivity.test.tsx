// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { GenerationLog } from "../../lib/types";

const mocks = vi.hoisted(() => ({
  generationLog: vi.fn<() => Promise<GenerationLog>>(),
}));
const i18nMocks = vi.hoisted(() => ({
  t: vi.fn((key: string) => key),
}));

vi.mock("../../i18n", () => ({ useT: () => i18nMocks.t }));
vi.mock("../../lib/api", () => ({
  isTauri: false,
  generationLog: mocks.generationLog,
}));
vi.mock("../../lib/asset", () => ({ assetUrl: (path: string | null) => path }));
vi.mock("../../store/projectActions", () => ({
  newProjectAndEnter: vi.fn(),
  openProjectPath: vi.fn(),
  openProjectViaDialog: vi.fn(),
  openSampleProject: vi.fn(),
}));

import { useRecentStore } from "../../store/recentStore";
import { useEditorUiStore } from "../../store/uiStore";
import { HomeView } from "./HomeView";
import { DICTS } from "../../i18n/dict";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

let root: Root;
let container: HTMLDivElement;

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  useRecentStore.setState({ recents: [] });
  useEditorUiStore.setState({ view: "home" });
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
});

afterEach(async () => {
  await act(async () => root.unmount());
  container.remove();
});

describe("Home generation activity (read-only mirror of generation_log)", () => {
  it("shows the loading state while the read is in flight", async () => {
    mocks.generationLog.mockReturnValueOnce(new Promise<GenerationLog>(() => {}));
    await act(async () => root.render(<HomeView />));

    expect(container.querySelector('[role="status"]')?.textContent).toBe(
      "home.generationActivityLoading",
    );
  });

  it("shows the honest empty state for a session with no generations", async () => {
    mocks.generationLog.mockResolvedValueOnce({ version: 1, entries: [] });
    await act(async () => root.render(<HomeView />));
    await vi.waitFor(() => {
      expect(container.textContent).toContain("home.generationActivityEmpty");
    });
    expect(container.textContent).not.toContain("home.generationActivityTotal");
  });

  it("renders real rows newest-first with model, relative time and credits, plus total spend", async () => {
    mocks.generationLog.mockResolvedValueOnce({
      version: 1,
      entries: [
        {
          id: "row-old",
          model: "veo-3",
          costCredits: 250,
          createdAt: 700_000_000, // Apple-reference seconds
        },
        {
          id: "row-new",
          model: "gpt-4o",
          // costCredits omitted on the wire -> unknown cost row
          createdAt: 700_000_100,
        },
      ],
    });
    await act(async () => root.render(<HomeView />));
    await vi.waitFor(() => {
      expect(container.textContent).toContain("gpt-4o");
    });

    // Newest first: the row without costCredits comes before the veo-3 row.
    const rows = [...container.querySelectorAll("li")].map((li) => li.textContent);
    expect(rows[0]).toContain("gpt-4o");
    expect(rows[0]).toContain("home.generationActivityCostUnknown");
    expect(rows[1]).toContain("veo-3");
    expect(rows[1]).toContain("250 home.generationActivityCreditsUnit");
    // Relative time is derived from the Apple-reference epoch.
    expect(rows[1]).toContain("home.relative.");

    // Total spend sums present credits, treating missing as 0 (Rust
    // `total_credits` semantics).
    expect(i18nMocks.t).toHaveBeenCalledWith("home.generationActivityTotal", {
      count: 2,
      credits: 250,
    });
  });

  it("renders the error state when the read fails", async () => {
    mocks.generationLog.mockRejectedValueOnce(new Error("ipc down"));
    await act(async () => root.render(<HomeView />));

    await vi.waitFor(() => {
      expect(container.querySelector('[role="alert"]')?.textContent).toBe(
        "home.generationActivityFailed",
      );
    });
    expect(container.textContent).not.toContain("home.generationActivityEmpty");
  });

  it("does not fetch while the Home view is inactive", async () => {
    useEditorUiStore.setState({ view: "editor" });
    await act(async () => root.render(<HomeView />));
    expect(mocks.generationLog).not.toHaveBeenCalled();
  });

  it("refetches when Home becomes active again", async () => {
    mocks.generationLog.mockResolvedValue({ version: 1, entries: [] });
    useEditorUiStore.setState({ view: "editor" });
    await act(async () => root.render(<HomeView />));
    expect(mocks.generationLog).not.toHaveBeenCalled();

    await act(async () => useEditorUiStore.getState().setView("home"));
    await vi.waitFor(() => expect(mocks.generationLog).toHaveBeenCalledTimes(1));
  });

  it("localizes every new key in both supported locales", () => {
    for (const key of [
      "home.generationActivity",
      "home.generationActivityLoading",
      "home.generationActivityFailed",
      "home.generationActivityEmpty",
      "home.generationActivityTotal",
      "home.generationActivityCreditsUnit",
      "home.generationActivityCostUnknown",
    ]) {
      expect(DICTS["zh-CN"][key], `zh ${key}`).toBeTruthy();
      expect(DICTS.en[key], `en ${key}`).toBeTruthy();
    }
    expect(DICTS["zh-CN"]["home.generationActivityTotal"]).toContain("{count}");
    expect(DICTS["zh-CN"]["home.generationActivityTotal"]).toContain("{credits}");
    expect(DICTS.en["home.generationActivityTotal"]).toContain("{count}");
    expect(DICTS.en["home.generationActivityTotal"]).toContain("{credits}");
  });
});
