// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  usage: vi.fn(),
  clear: vi.fn(),
}));

vi.mock("../../lib/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../lib/api")>()),
  storageUsage: api.usage,
  storageClear: api.clear,
}));

import { t } from "../../i18n";
import { StoragePane } from "./StoragePane";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

interface UsageFixture {
  categories: Array<{ id: string; bytes: number; path: string }>;
  totalBytes: number;
  cacheRoot: string;
}

function usageFixture(): UsageFixture {
  return {
    categories: [
      { id: "thumbnails", bytes: 150, path: "/cache/MediaVisualCache" },
      { id: "waveforms", bytes: 100, path: "/cache/MediaVisualCache" },
      { id: "searchIndex", bytes: 0, path: "/cache/Embeddings" },
      { id: "models", bytes: 500, path: "/data/models" },
      { id: "other", bytes: 210, path: "/cache" },
    ],
    totalBytes: 960,
    cacheRoot: "/cache",
  };
}

let container: HTMLDivElement;
let root: Root;
let rootMounted: boolean;

beforeEach(() => {
  vi.clearAllMocks();
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
  rootMounted = true;
});

afterEach(async () => {
  if (rootMounted) await act(async () => root.unmount());
  container.remove();
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

const render = async () => {
  await act(async () => root.render(<StoragePane />));
};

const clearButton = (category: string) =>
  container.querySelector<HTMLButtonElement>(`[data-category="${category}"][data-action="clear"]`)!;

const modelRow = () => container.querySelector<HTMLElement>('[data-storage-row="models"]')!;

const reveal = () => modelRow().querySelector<HTMLElement>(".reveal");

const confirmationButton = (action: "confirm-remove" | "confirm-cancel") =>
  container.querySelector<HTMLButtonElement>(`[data-category="models"][data-action="${action}"]`)!;

const openModelConfirmation = async () => {
  await act(async () => clearButton("models").click());
};

const deferred = <T,>() => {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
};

it("shows a loading state while the usage report is in flight", async () => {
  api.usage.mockReturnValue(new Promise(() => undefined));
  await render();
  expect(container.textContent).toContain(t("storage.loading"));
  expect(container.querySelector('[role="status"]')).not.toBeNull();
});

it("renders real per-category sizes and disables zero-byte clear buttons", async () => {
  api.usage.mockResolvedValue(usageFixture());
  await render();
  await act(async () => undefined);

  expect(container.textContent).toContain("960 B");
  expect(container.textContent).toContain("150 B");
  expect(container.textContent).toContain("500 B");
  expect(container.textContent).toContain("/cache");

  expect(clearButton("thumbnails").disabled).toBe(false);
  expect(clearButton("models").disabled).toBe(false);
  expect(clearButton("searchIndex").disabled).toBe(true); // 0 bytes
});

it("clears a non-model category immediately and adopts the fresh snapshot", async () => {
  api.usage.mockResolvedValue(usageFixture());
  api.clear.mockResolvedValue({
    ...usageFixture(),
    categories: usageFixture().categories.map((category) =>
      category.id === "thumbnails" ? { ...category, bytes: 0 } : category,
    ),
    totalBytes: 810,
  });
  await render();
  await act(async () => undefined);

  await act(async () => clearButton("thumbnails").click());
  expect(api.clear).toHaveBeenCalledWith(["thumbnails"], false);
  expect(container.textContent).toContain("810 B");
  expect(clearButton("thumbnails").disabled).toBe(true);
});

it("reveals one model confirmation inside its row so sibling layout follows the disclosure", async () => {
  api.usage.mockResolvedValue(usageFixture());
  await render();
  await act(async () => undefined);

  const siblingRow = modelRow().nextElementSibling;
  await openModelConfirmation();

  expect(api.clear).not.toHaveBeenCalled();
  expect(reveal()?.textContent).toContain(t("storage.clearConfirmTitle"));
  expect(reveal()?.textContent).toContain(t("storage.clearConfirmBody"));
  expect(modelRow().contains(reveal())).toBe(true);
  expect(modelRow().nextElementSibling).toBe(siblingRow);
  expect(siblingRow?.getAttribute("data-storage-row")).toBe("other");

  await act(async () => clearButton("models").click());
  expect(modelRow().querySelectorAll('[data-action="confirm-remove"]')).toHaveLength(1);
});

it("keeps confirmation copy mounted through the cancel exit and restores focus to clear", async () => {
  vi.useFakeTimers();
  api.usage.mockResolvedValue(usageFixture());
  await render();
  await act(async () => undefined);

  await openModelConfirmation();
  const cancel = confirmationButton("confirm-cancel");
  cancel.focus();
  await act(async () => cancel.click());

  expect(document.activeElement).toBe(clearButton("models"));
  expect(reveal()?.dataset.state).toBe("closed");
  expect(reveal()?.textContent).toContain(t("storage.clearConfirmBody"));

  await act(async () => vi.advanceTimersByTimeAsync(179));
  expect(reveal()).not.toBeNull();
  await act(async () => vi.advanceTimersByTimeAsync(1));
  expect(reveal()).toBeNull();
});

it("closes after successful model deletion and moves focus to the next available clear action", async () => {
  vi.useFakeTimers();
  api.usage.mockResolvedValue(usageFixture());
  let resolveClear: (usage: UsageFixture) => void;
  api.clear.mockReturnValue(new Promise<UsageFixture>((resolve) => {
    resolveClear = resolve;
  }));
  await render();
  await act(async () => undefined);

  await openModelConfirmation();
  await act(async () => confirmationButton("confirm-remove").click());
  expect(confirmationButton("confirm-remove").disabled).toBe(true);

  await act(async () => {
    resolveClear({
      ...usageFixture(),
      categories: usageFixture().categories.map((category) =>
        category.id === "models" ? { ...category, bytes: 0 } : category,
      ),
      totalBytes: 460,
    });
  });

  expect(api.clear).toHaveBeenCalledWith(["models"], true);
  expect(reveal()?.dataset.state).toBe("closed");
  expect(document.activeElement).toBe(clearButton("other"));
  await act(async () => vi.advanceTimersByTimeAsync(180));
  expect(reveal()).toBeNull();
});

it("falls back to the previous enabled clear action when no later model sibling is available", async () => {
  vi.useFakeTimers();
  const initial = usageFixture();
  initial.categories = initial.categories.map((category) =>
    category.id === "other" ? { ...category, bytes: 0 } : category,
  );
  initial.totalBytes = 750;
  api.usage.mockResolvedValue(initial);
  api.clear.mockResolvedValue({
    ...initial,
    categories: initial.categories.map((category) =>
      category.id === "models" ? { ...category, bytes: 0 } : category,
    ),
    totalBytes: 250,
  });
  await render();
  await act(async () => undefined);

  await openModelConfirmation();
  await act(async () => confirmationButton("confirm-remove").click());

  expect(document.activeElement).toBe(clearButton("waveforms"));
});

it("focuses the pane when model deletion leaves no enabled clear action", async () => {
  vi.useFakeTimers();
  const initial = usageFixture();
  initial.categories = initial.categories.map((category) => ({
    ...category,
    bytes: category.id === "models" ? 500 : 0,
  }));
  initial.totalBytes = 500;
  api.usage.mockResolvedValue(initial);
  api.clear.mockResolvedValue({
    ...initial,
    categories: initial.categories.map((category) => ({ ...category, bytes: 0 })),
    totalBytes: 0,
  });
  await render();
  await act(async () => undefined);

  const pane = container.querySelector("section");
  await openModelConfirmation();
  await act(async () => confirmationButton("confirm-remove").click());

  expect(document.activeElement).toBe(pane);
});

it.each(["resolve", "reject"] as const)(
  "ignores a model clear %s after the pane unmounts",
  async (outcome) => {
    const operation = deferred<UsageFixture>();
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => undefined);
    api.usage.mockResolvedValue(usageFixture());
    api.clear.mockReturnValue(operation.promise);
    await render();
    await act(async () => undefined);

    await openModelConfirmation();
    await act(async () => confirmationButton("confirm-remove").click());
    await act(async () => root.unmount());
    rootMounted = false;

    const outside = document.createElement("button");
    document.body.append(outside);
    outside.focus();
    await act(async () => {
      if (outcome === "resolve") operation.resolve(usageFixture());
      else operation.reject(new Error("late failure"));
    });

    expect(container.childElementCount).toBe(0);
    expect(document.activeElement).toBe(outside);
    expect(consoleError).not.toHaveBeenCalled();
    outside.remove();
  },
);

it("keeps a model-clear backend failure visible in the open confirmation", async () => {
  api.usage.mockResolvedValue(usageFixture());
  api.clear.mockRejectedValue("model removal failed");
  await render();
  await act(async () => undefined);

  await openModelConfirmation();
  await act(async () => confirmationButton("confirm-remove").click());

  expect(reveal()?.dataset.state).toBe("open");
  expect(reveal()?.querySelector('[role="alert"]')?.textContent).toContain("model removal failed");
});

it("removes the confirmation synchronously when reduced motion is requested", async () => {
  vi.stubGlobal(
    "matchMedia",
    vi.fn().mockImplementation(() => ({ matches: true })),
  );
  api.usage.mockResolvedValue(usageFixture());
  await render();
  await act(async () => undefined);

  await openModelConfirmation();
  expect(reveal()?.dataset.state).toBe("open");
  await act(async () => confirmationButton("confirm-cancel").click());
  expect(reveal()).toBeNull();
});

it("requires an explicit confirm step before clearing models", async () => {
  api.usage.mockResolvedValue(usageFixture());
  api.clear.mockResolvedValue({ ...usageFixture(), totalBytes: 460 });
  await render();
  await act(async () => undefined);

  await openModelConfirmation();
  expect(api.clear).not.toHaveBeenCalled();

  // Cancelling leaves the models untouched.
  await act(async () => {
    confirmationButton("confirm-cancel").click();
  });
  expect(api.clear).not.toHaveBeenCalled();

  // Confirming removes them with the gate flag set.
  await openModelConfirmation();
  await act(async () => {
    confirmationButton("confirm-remove").click();
  });
  expect(api.clear).toHaveBeenCalledWith(["models"], true);
});

it("surfaces a load failure without rendering fake statistics", async () => {
  api.usage.mockRejectedValue(new Error("boom"));
  await render();
  await act(async () => undefined);

  const alert = container.querySelector('[role="alert"]');
  expect(alert?.textContent).toContain(t("storage.error", { error: "boom" }));
  expect(container.querySelector('[data-action="clear"]')).toBeNull();
});

it("surfaces a clear failure and keeps the previous snapshot", async () => {
  api.usage.mockResolvedValue(usageFixture());
  api.clear.mockRejectedValue("storage_clear failed");
  await render();
  await act(async () => undefined);

  await act(async () => clearButton("other").click());
  expect(container.querySelector('[role="alert"]')?.textContent).toContain("storage_clear failed");
  expect(container.textContent).toContain("210 B"); // snapshot unchanged
});

it("renders the honest unsupported state outside Tauri (no cache root)", async () => {
  api.usage.mockResolvedValue({
    categories: [
      { id: "thumbnails", bytes: 0, path: "" },
      { id: "waveforms", bytes: 0, path: "" },
      { id: "searchIndex", bytes: 0, path: "" },
      { id: "models", bytes: 0, path: "" },
      { id: "other", bytes: 0, path: "" },
    ],
    totalBytes: 0,
    cacheRoot: "",
  });
  await render();
  await act(async () => undefined);

  expect(container.textContent).toContain(t("storage.unsupported"));
  expect(container.querySelector('[data-action="clear"]')).toBeNull();
});

it("reports an empty state when every cache is already empty", async () => {
  api.usage.mockResolvedValue({
    ...usageFixture(),
    categories: usageFixture().categories.map((category) => ({ ...category, bytes: 0 })),
    totalBytes: 0,
  });
  await render();
  await act(async () => undefined);

  expect(container.textContent).toContain(t("storage.empty"));
  expect(clearButton("thumbnails").disabled).toBe(true);
});
