// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  cancelExport: vi.fn<(operationId: string) => Promise<void>>(),
  refreshMedia: vi.fn<() => Promise<void>>(),
}));

vi.mock("../../lib/api", () => ({
  EXPORT_CANCELLED_SENTINEL: "export cancelled",
  cancelExport: mocks.cancelExport,
  createExportOperationId: () => "save-as:unused",
  isTauri: true,
  onExportProgress: vi.fn(),
  saveClipAsMedia: vi.fn(),
  saveRangeAsMedia: vi.fn(),
}));

vi.mock("../../store/mediaStore", () => ({
  refreshMedia: mocks.refreshMedia,
}));

import { useEditorUiStore, type SaveAsProgressState } from "../../store/uiStore";
import { SaveAsProgress } from "./SaveAsProgress";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

let root: Root | null = null;
let container: HTMLDivElement | null = null;

const progress = (overrides: Partial<SaveAsProgressState> = {}): SaveAsProgressState => ({
  operationId: "save-as:clip-1",
  label: "Saving clip",
  done: 25,
  total: 100,
  cancellable: true,
  cancelling: false,
  ...overrides,
});

function cancelButton(): HTMLButtonElement {
  const button = container?.querySelector<HTMLButtonElement>("button");
  expect(button).not.toBeNull();
  return button!;
}

beforeEach(() => {
  vi.clearAllMocks();
  useEditorUiStore.setState({ saveAsProgress: progress(), toast: null });
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
});

afterEach(async () => {
  if (root) await act(async () => root?.unmount());
  container?.remove();
  root = null;
  container = null;
  useEditorUiStore.setState({ saveAsProgress: null, toast: null });
});

describe("SaveAsProgress control acceptance", () => {
  it("control-b0b920085e77d039 cancel Save Clip as Media", async () => {
    let finishCancel: (() => void) | undefined;
    mocks.cancelExport.mockImplementationOnce(
      () => new Promise<void>((resolve) => (finishCancel = resolve)),
    );
    await act(async () => root?.render(<SaveAsProgress />));

    expect(container?.querySelector('[role="status"]')?.getAttribute("aria-label")).toBe(
      "Saving clip",
    );
    expect(container?.textContent).toContain("25%");
    expect(cancelButton().disabled).toBe(false);

    await act(async () => {
      cancelButton().click();
      await Promise.resolve();
    });
    expect(mocks.cancelExport).toHaveBeenCalledWith("save-as:clip-1");
    expect(useEditorUiStore.getState().saveAsProgress?.cancelling).toBe(true);
    expect(cancelButton().disabled).toBe(true);
    expect(cancelButton().textContent).toBe("正在取消…");
    await act(async () => finishCancel?.());

    await act(async () =>
      useEditorUiStore.setState({
        saveAsProgress: progress({ operationId: "save-as:clip-2" }),
        toast: null,
      }),
    );
    mocks.cancelExport.mockRejectedValueOnce(new Error("cancel transport failed"));
    await act(async () => {
      cancelButton().click();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(mocks.cancelExport).toHaveBeenLastCalledWith("save-as:clip-2");
    expect(useEditorUiStore.getState().saveAsProgress?.cancelling).toBe(false);
    expect(cancelButton().disabled).toBe(false);
    expect(useEditorUiStore.getState().toast?.message).toContain("cancel transport failed");

    const callCount = mocks.cancelExport.mock.calls.length;
    await act(async () =>
      useEditorUiStore.setState({
        saveAsProgress: progress({ operationId: "save-as:preparing", cancellable: false }),
      }),
    );
    expect(cancelButton().disabled).toBe(true);
    await act(async () => cancelButton().click());
    expect(mocks.cancelExport).toHaveBeenCalledTimes(callCount);
  });
});
