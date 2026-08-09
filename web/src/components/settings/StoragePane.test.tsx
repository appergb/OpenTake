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

beforeEach(() => {
  vi.clearAllMocks();
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
});

afterEach(async () => {
  await act(async () => root.unmount());
  container.remove();
});

const render = async () => {
  await act(async () => root.render(<StoragePane />));
};

const clearButton = (category: string) =>
  container.querySelector<HTMLButtonElement>(`[data-category="${category}"][data-action="clear"]`)!;

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

it("requires an explicit confirm step before clearing models", async () => {
  api.usage.mockResolvedValue(usageFixture());
  api.clear.mockResolvedValue({ ...usageFixture(), totalBytes: 460 });
  await render();
  await act(async () => undefined);

  await act(async () => clearButton("models").click());
  expect(api.clear).not.toHaveBeenCalled();
  expect(container.textContent).toContain(t("storage.clearConfirmTitle"));
  expect(container.textContent).toContain(t("storage.clearConfirmBody"));

  // Cancelling leaves the models untouched.
  await act(async () => {
    container
      .querySelector<HTMLButtonElement>('[data-category="models"][data-action="confirm-cancel"]')!
      .click();
  });
  expect(api.clear).not.toHaveBeenCalled();
  expect(container.textContent).not.toContain(t("storage.clearConfirmTitle"));

  // Confirming removes them with the gate flag set.
  await act(async () => clearButton("models").click());
  await act(async () => {
    container
      .querySelector<HTMLButtonElement>('[data-category="models"][data-action="confirm-remove"]')!
      .click();
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
