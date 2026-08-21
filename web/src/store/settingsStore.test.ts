// @vitest-environment happy-dom

import { afterEach, beforeEach, expect, it, vi } from "vitest";

const native = vi.hoisted(() => ({
  currentMonitor: vi.fn(),
  primaryMonitor: vi.fn(),
  scaleFactor: vi.fn(),
  innerSize: vi.fn(),
  outerPosition: vi.fn(),
  setPosition: vi.fn(),
  setSize: vi.fn(),
  setProxyPlaybackEnabled: vi.fn(),
}));

vi.mock("../lib/api", () => ({
  isTauri: true,
  setProxyPlaybackEnabled: native.setProxyPlaybackEnabled,
}));

vi.mock("@tauri-apps/api/window", () => ({
  currentMonitor: native.currentMonitor,
  primaryMonitor: native.primaryMonitor,
  getCurrentWindow: () => ({
    scaleFactor: native.scaleFactor,
    innerSize: native.innerSize,
    outerPosition: native.outerPosition,
    setPosition: native.setPosition,
    setSize: native.setSize,
  }),
}));

vi.mock("@tauri-apps/api/dpi", () => ({
  LogicalSize: class LogicalSize {
    constructor(public width: number, public height: number) {}
  },
  LogicalPosition: class LogicalPosition {
    constructor(public x: number, public y: number) {}
  },
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

async function loadStores() {
  vi.resetModules();
  const settings = await import("./settingsStore");
  const ui = await import("./uiStore");
  return { ...settings, ...ui };
}

beforeEach(() => {
  localStorage.clear();
  document.documentElement.removeAttribute("data-theme");
  native.currentMonitor.mockReset().mockResolvedValue(null);
  native.primaryMonitor.mockReset().mockResolvedValue(null);
  native.scaleFactor.mockReset().mockResolvedValue(1);
  native.innerSize.mockReset().mockResolvedValue({
    toLogical: () => ({ width: 1600, height: 1000 }),
  });
  native.outerPosition.mockReset().mockResolvedValue({
    toLogical: () => ({ x: 40, y: 60 }),
  });
  native.setPosition.mockReset().mockResolvedValue(undefined);
  native.setSize.mockReset().mockResolvedValue(undefined);
  native.setProxyPlaybackEnabled.mockReset().mockResolvedValue(undefined);
});

afterEach(() => {
  vi.restoreAllMocks();
});

it("removes legacy and versioned theme preferences without a document theme marker", async () => {
  localStorage.setItem("theme", "light");
  localStorage.setItem("theme:v1", "dark");
  localStorage.setItem("opentake.theme.v2", "light");
  document.documentElement.dataset.theme = "light";

  const { useSettingsStore } = await loadStores();

  expect(localStorage.getItem("theme")).toBeNull();
  expect(localStorage.getItem("theme:v1")).toBeNull();
  expect(localStorage.getItem("opentake.theme.v2")).toBeNull();
  expect(useSettingsStore.getState()).not.toHaveProperty("theme");
  expect(document.documentElement.dataset.theme).toBeUndefined();
});

it("awaits a native resize before persisting the selected dark compact layout", async () => {
  const resize = deferred<void>();
  native.setSize.mockReturnValueOnce(resize.promise);
  const { useSettingsStore } = await loadStores();

  const operation = useSettingsStore.getState().setWindowSize("compact");

  expect(operation).toBeInstanceOf(Promise);
  expect(useSettingsStore.getState().windowSize).toBe("compact");
  expect(localStorage.getItem("windowSize")).toBeNull();
  await vi.waitFor(() => expect(native.setSize).toHaveBeenCalledOnce());
  expect(native.setSize).toHaveBeenCalledWith(expect.objectContaining({ width: 1066, height: 666 }));

  resize.resolve();
  await operation;

  expect(localStorage.getItem("windowSize")).toBe("compact");
});

it("restores the previous layout and reports a resize failure", async () => {
  localStorage.setItem("windowSize", "standard");
  native.setSize.mockRejectedValueOnce(new Error("resize denied"));
  const { useSettingsStore, useEditorUiStore } = await loadStores();

  const operation = useSettingsStore.getState().setWindowSize("compact");

  expect(useSettingsStore.getState().windowSize).toBe("compact");
  await operation;

  expect(useSettingsStore.getState().windowSize).toBe("standard");
  expect(localStorage.getItem("windowSize")).toBe("standard");
  expect(useEditorUiStore.getState().toast?.message).toContain("resize denied");
});

it("does not move the native window when its size is rejected", async () => {
  native.setSize.mockRejectedValueOnce(new Error("resize denied"));
  const { useSettingsStore } = await loadStores();

  await useSettingsStore.getState().setWindowSize("compact");

  expect(native.setPosition).not.toHaveBeenCalled();
});

it("restores native geometry when positioning a resized window fails", async () => {
  native.setPosition
    .mockRejectedValueOnce(new Error("position denied"))
    .mockResolvedValueOnce(undefined);
  const { useSettingsStore } = await loadStores();

  await useSettingsStore.getState().setWindowSize("compact");

  expect(native.setSize).toHaveBeenNthCalledWith(
    1,
    expect.objectContaining({ width: 1066, height: 666 }),
  );
  expect(native.setSize).toHaveBeenNthCalledWith(
    2,
    expect.objectContaining({ width: 1600, height: 1000 }),
  );
  expect(native.setPosition).toHaveBeenLastCalledWith(expect.objectContaining({ x: 40, y: 60 }));
  expect(useSettingsStore.getState().windowSize).toBe("standard");
});

it("clamps the standard layout to the current monitor work area and recenters it", async () => {
  native.scaleFactor.mockResolvedValue(2);
  native.currentMonitor.mockResolvedValue({
    scaleFactor: 2,
    workArea: {
      position: {
        toLogical: () => ({ x: 0, y: 24 }),
      },
      size: {
        toLogical: () => ({ width: 1331, height: 768 }),
      },
    },
  });
  native.innerSize.mockResolvedValue({
    toLogical: () => ({ width: 1066, height: 666 }),
  });
  native.outerPosition.mockResolvedValue({
    toLogical: () => ({ x: 120, y: 90 }),
  });
  const { useSettingsStore } = await loadStores();

  await useSettingsStore.getState().setWindowSize("standard");

  expect(native.currentMonitor).toHaveBeenCalledOnce();
  expect(native.setSize).toHaveBeenCalledWith(
    expect.objectContaining({ width: 1331, height: 768 }),
  );
  expect(native.setPosition).toHaveBeenCalledWith(
    expect.objectContaining({ x: 0, y: 24 }),
  );
  expect(useSettingsStore.getState().windowSize).toBe("standard");
});

it("serializes a later layout choice after an earlier native resize", async () => {
  const firstResize = deferred<void>();
  native.setSize
    .mockReturnValueOnce(firstResize.promise)
    .mockResolvedValueOnce(undefined);
  const { useSettingsStore, useEditorUiStore } = await loadStores();

  const first = useSettingsStore.getState().setWindowSize("compact");
  expect(first).toBeInstanceOf(Promise);
  await vi.waitFor(() => expect(native.setSize).toHaveBeenCalledOnce());

  const second = useSettingsStore.getState().setWindowSize("standard");
  expect(native.setSize).toHaveBeenCalledOnce();
  firstResize.resolve();
  await first;
  await second;

  expect(useSettingsStore.getState().windowSize).toBe("standard");
  expect(localStorage.getItem("windowSize")).toBe("standard");
  expect(useEditorUiStore.getState().toast).toBeNull();
  expect(native.setSize).toHaveBeenNthCalledWith(
    1,
    expect.objectContaining({ width: 1066, height: 666 }),
  );
  expect(native.setSize).toHaveBeenNthCalledWith(
    2,
    expect.objectContaining({ width: 1600, height: 1000 }),
  );
});

it("serializes a user layout selection after startup resize", async () => {
  const startupResize = deferred<void>();
  const startupPosition = deferred<void>();
  native.setSize
    .mockReturnValueOnce(startupResize.promise)
    .mockResolvedValueOnce(undefined);
  native.setPosition
    .mockReturnValueOnce(startupPosition.promise)
    .mockResolvedValueOnce(undefined);
  const { initWindowSize, useSettingsStore } = await loadStores();

  initWindowSize();
  await vi.waitFor(() => expect(native.setSize).toHaveBeenCalledOnce());
  const selection = useSettingsStore.getState().setWindowSize("compact");

  expect(native.setSize).toHaveBeenCalledOnce();
  startupResize.resolve();
  await vi.waitFor(() => expect(native.setPosition).toHaveBeenCalledOnce());
  expect(native.setSize).toHaveBeenCalledOnce();
  startupPosition.resolve();
  await selection;

  expect(native.setSize).toHaveBeenNthCalledWith(
    2,
    expect.objectContaining({ width: 1066, height: 666 }),
  );
});

it("ignores a stale resize failure while applying the later layout choice", async () => {
  const firstResize = deferred<void>();
  native.setSize
    .mockReturnValueOnce(firstResize.promise)
    .mockResolvedValueOnce(undefined);
  const { useSettingsStore, useEditorUiStore } = await loadStores();

  const first = useSettingsStore.getState().setWindowSize("compact");
  await vi.waitFor(() => expect(native.setSize).toHaveBeenCalledOnce());
  const second = useSettingsStore.getState().setWindowSize("standard");
  firstResize.reject(new Error("stale resize denied"));
  await first;
  await second;

  expect(useSettingsStore.getState().windowSize).toBe("standard");
  expect(localStorage.getItem("windowSize")).toBe("standard");
  expect(useEditorUiStore.getState().toast).toBeNull();
});
