// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  cancelExport: vi.fn(),
  exportBundle: vi.fn(),
  exportVideo: vi.fn(),
  getDefaultProjectDir: vi.fn(),
  onExportProgress: vi.fn(),
  save: vi.fn(),
  saveDialog: vi.fn(),
  unlisten: vi.fn(),
}));

vi.mock("../../i18n", () => ({
  useT: () => (key: string, vars?: Record<string, string | number>) =>
    key === "export.mode.video" ? `Video (.${vars?.ext ?? "mp4"})` : key,
}));

vi.mock("../../lib/api", () => ({
  EXPORT_CANCELLED_SENTINEL: "export cancelled",
  cancelExport: mocks.cancelExport,
  createExportOperationId: () => "video-operation-1",
  exportBundle: mocks.exportBundle,
  exportVideo: mocks.exportVideo,
  getDefaultProjectDir: mocks.getDefaultProjectDir,
  onExportProgress: mocks.onExportProgress,
}));

vi.mock("../../lib/dialog", () => ({
  saveDialog: mocks.saveDialog,
}));

import { useEditorUiStore } from "../../store/uiStore";
import { useProjectStore } from "../../store/projectStore";
import { ExportDialog } from "./ExportDialog";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

let root: Root | null = null;
let container: HTMLDivElement | null = null;
let returnFocus: HTMLButtonElement | null = null;

function buttonWithLabel(label: string): HTMLButtonElement {
  const button = container?.querySelector<HTMLButtonElement>(`button[aria-label="${label}"]`);
  expect(button, `button ${label}`).not.toBeNull();
  return button!;
}

function buttonWithText(text: string): HTMLButtonElement {
  const button = [...(container?.querySelectorAll<HTMLButtonElement>("button") ?? [])].find(
    (candidate) => candidate.textContent === text,
  );
  expect(button, `button text ${text}`).not.toBeUndefined();
  return button!;
}

async function chooseDropdown(label: string, optionText: string): Promise<void> {
  await act(async () => buttonWithLabel(label).click());
  const option = [...(container?.querySelectorAll<HTMLButtonElement>('button[role="option"]') ?? [])]
    .find((candidate) => candidate.textContent?.includes(optionText));
  expect(option, `option ${optionText}`).not.toBeUndefined();
  await act(async () => option?.click());
}

async function renderDialog(): Promise<void> {
  await act(async () => root?.render(<ExportDialog />));
  expect(container?.querySelector('[role="dialog"]')).not.toBeNull();
}

beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  useEditorUiStore.setState({
    exportDialogOpen: true,
    toast: null,
  });
  useProjectStore.setState({
    projectPath: "/tmp/My Film.opentake",
    timeline: {
      fps: 30,
      width: 1920,
      height: 1080,
      settingsConfigured: true,
      tracks: [],
    },
  });
  mocks.saveDialog.mockResolvedValue(mocks.save);
  mocks.save.mockResolvedValue("/tmp/render");
  mocks.getDefaultProjectDir.mockResolvedValue("/tmp");
  mocks.onExportProgress.mockResolvedValue(mocks.unlisten);
  mocks.exportVideo.mockResolvedValue({ width: 1920, height: 1080, frameCount: 30 });
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
  returnFocus = document.createElement("button");
  returnFocus.textContent = "Open export";
  document.body.append(returnFocus);
  returnFocus.focus();
});

afterEach(async () => {
  if (root) await act(async () => root?.unmount());
  container?.remove();
  returnFocus?.remove();
  root = null;
  container = null;
  returnFocus = null;
});

describe("ExportDialog control acceptance", () => {
  it("moves focus into the modal, contains both Tab directions, and restores its trigger", async () => {
    await renderDialog();
    const dialog = container?.querySelector<HTMLElement>('[role="dialog"]')!;
    const controls = [...dialog.querySelectorAll<HTMLButtonElement>("button")].filter(
      (button) => !button.disabled && button.tabIndex >= 0,
    );
    const first = controls[0]!;
    const last = controls.at(-1)!;
    expect(dialog.contains(document.activeElement)).toBe(true);

    await act(async () => {
      last.focus();
      document.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Tab", bubbles: true, cancelable: true }),
      );
    });
    expect(document.activeElement).toBe(first);

    await act(async () => {
      first.focus();
      document.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "Tab",
          shiftKey: true,
          bubbles: true,
          cancelable: true,
        }),
      );
    });
    expect(document.activeElement).toBe(last);

    await act(async () => buttonWithLabel("export.close").click());
    expect(document.activeElement).toBe(returnFocus);
  });

  it("control-580ab884755388a9 dismiss Export by clicking the backdrop", async () => {
    await renderDialog();
    const dialog = container?.querySelector<HTMLElement>('[role="dialog"]');
    const backdrop = dialog?.parentElement;
    expect(backdrop).not.toBeNull();

    await act(async () => backdrop?.click());
    expect(useEditorUiStore.getState().exportDialogOpen).toBe(false);

    await act(async () => useEditorUiStore.setState({ exportDialogOpen: true }));
    let finishExport: ((value: { width: number; height: number; frameCount: number }) => void) | null =
      null;
    mocks.exportVideo.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finishExport = resolve;
        }),
    );
    await act(async () => {
      buttonWithText("export.run").click();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(buttonWithLabel("export.close").disabled).toBe(true);

    const busyDialog = container?.querySelector<HTMLElement>('[role="dialog"]');
    await act(async () => busyDialog?.parentElement?.click());
    expect(useEditorUiStore.getState().exportDialogOpen).toBe(true);

    await act(async () => {
      finishExport?.({ width: 1920, height: 1080, frameCount: 30 });
      await Promise.resolve();
    });
  });

  it("control-6064916ed05a1362 close Export from its header", async () => {
    await renderDialog();
    const close = buttonWithLabel("export.close");
    expect(close.disabled).toBe(false);
    await act(async () => close.click());
    expect(useEditorUiStore.getState().exportDialogOpen).toBe(false);
  });

  it("control-34646794727cf515 choose export mode", async () => {
    mocks.exportVideo.mockRejectedValueOnce(new Error("stale export error"));
    await renderDialog();
    await act(async () => {
      buttonWithText("export.run").click();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(container?.textContent).toContain("stale export error");

    await chooseDropdown("export.mode: Video (.mp4)", "Video (.mp4)");
    expect(container?.textContent).not.toContain("stale export error");
  });

  it("labels the video export type with the selected codec container", async () => {
    await renderDialog();
    expect(buttonWithLabel("export.mode: Video (.mp4)").textContent).toContain(".mp4");

    await chooseDropdown("export.format", "export.codec.h265");
    expect(buttonWithLabel("export.mode: Video (.mp4)").textContent).not.toContain(".mov");

    await chooseDropdown("export.format", "export.codec.prores");

    expect(buttonWithLabel("export.mode: Video (.mov)").textContent).toContain(".mov");
    expect(buttonWithLabel("export.mode: Video (.mov)").textContent).not.toContain(".mp4");
  });

  it("control-6846958c0e19c8e9 choose export codec", async () => {
    await renderDialog();
    await chooseDropdown("export.format", "export.codec.prores");
    await act(async () => {
      buttonWithText("export.run").click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.save).toHaveBeenCalledWith(
      expect.objectContaining({
        defaultPath: "/tmp/My Film.mov",
      }),
    );
    expect(mocks.save.mock.calls[0]?.[0]).not.toHaveProperty("filters");
    expect(mocks.exportVideo).toHaveBeenCalledWith(
      { outPath: "/tmp/render.mov", codec: "prores", quality: "1080p" },
      "video-operation-1",
    );
  });

  it("control-30862b5deb972fcd choose export resolution", async () => {
    await renderDialog();
    await chooseDropdown("export.resolution", "export.quality.4k");
    await act(async () => {
      buttonWithText("export.run").click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.exportVideo).toHaveBeenCalledWith(
      { outPath: "/tmp/render.mp4", codec: "h264", quality: "4k" },
      "video-operation-1",
    );
  });

  it("control-af586bdec82ebcc7 cancel/close Export", async () => {
    await renderDialog();
    await act(async () => buttonWithText("export.cancel").click());
    expect(useEditorUiStore.getState().exportDialogOpen).toBe(false);
    expect(mocks.cancelExport).not.toHaveBeenCalled();

    await act(async () => useEditorUiStore.setState({ exportDialogOpen: true, toast: null }));
    mocks.exportVideo.mockImplementationOnce(() => new Promise(() => undefined));
    mocks.cancelExport.mockRejectedValueOnce(new Error("cancel channel unavailable"));
    await act(async () => {
      buttonWithText("export.run").click();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(buttonWithLabel("export.close").disabled).toBe(true);

    await act(async () => {
      buttonWithText("export.cancel").click();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(mocks.cancelExport).toHaveBeenCalledWith("video-operation-1");
    expect(useEditorUiStore.getState().exportDialogOpen).toBe(true);
    expect(container?.textContent).toContain("cancel channel unavailable");
    expect(useEditorUiStore.getState().toast?.message).toBe("export.failed");
  });

  it("control-543cacc54290eeba start video export", async () => {
    await act(async () => useProjectStore.setState({ projectPath: null }));
    mocks.getDefaultProjectDir.mockResolvedValue("/exports");
    mocks.save.mockResolvedValue("/exports/final-cut");
    let reportProgress: ((progress: { done: number; total: number }) => void) | undefined;
    mocks.onExportProgress.mockImplementationOnce(
      async (
        _operationId: string,
        callback: (progress: { done: number; total: number }) => void,
      ) => {
        reportProgress = callback;
        return mocks.unlisten;
      },
    );
    let finishExport: ((value: { width: number; height: number; frameCount: number }) => void) | null =
      null;
    mocks.exportVideo.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finishExport = resolve;
        }),
    );
    await renderDialog();

    await act(async () => {
      buttonWithText("export.run").click();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(mocks.getDefaultProjectDir).toHaveBeenCalledOnce();
    expect(mocks.save).toHaveBeenCalledWith(
      expect.objectContaining({
        defaultPath: "/exports/Timeline.mp4",
      }),
    );
    expect(mocks.save.mock.calls[0]?.[0]).not.toHaveProperty("filters");
    expect(mocks.onExportProgress).toHaveBeenCalledWith(
      "video-operation-1",
      expect.any(Function),
    );
    expect(mocks.exportVideo).toHaveBeenCalledWith(
      { outPath: "/exports/final-cut.mp4", codec: "h264", quality: "1080p" },
      "video-operation-1",
    );

    await act(async () => reportProgress?.({ done: 15, total: 30 }));
    expect(container?.querySelector('[role="progressbar"]')?.getAttribute("aria-valuenow")).toBe(
      "50",
    );
    await act(async () => {
      finishExport?.({ width: 1920, height: 1080, frameCount: 30 });
      await Promise.resolve();
    });
    expect(useEditorUiStore.getState().exportDialogOpen).toBe(false);
    expect(useEditorUiStore.getState().toast?.message).toBe("export.done");
    expect(mocks.unlisten).toHaveBeenCalledTimes(1);

    await act(async () => useEditorUiStore.setState({ exportDialogOpen: true, toast: null }));
    mocks.exportVideo.mockRejectedValueOnce(new Error("export cancelled"));
    await act(async () => {
      buttonWithText("export.run").click();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(useEditorUiStore.getState().exportDialogOpen).toBe(false);
    expect(useEditorUiStore.getState().toast?.message).toBe("export.cancelled");
    expect(mocks.unlisten).toHaveBeenCalledTimes(2);

    await act(async () => useEditorUiStore.setState({ exportDialogOpen: true, toast: null }));
    mocks.exportVideo.mockRejectedValueOnce(new Error("encoder unavailable"));
    await act(async () => {
      buttonWithText("export.run").click();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(useEditorUiStore.getState().exportDialogOpen).toBe(true);
    expect(container?.textContent).toContain("encoder unavailable");
    expect(buttonWithLabel("export.close").disabled).toBe(false);
    expect(container?.querySelector('[role="progressbar"]')).toBeNull();
    expect(useEditorUiStore.getState().toast?.message).toBe("export.failed");
    expect(mocks.unlisten).toHaveBeenCalledTimes(3);

    const exportCallCount = mocks.exportVideo.mock.calls.length;
    mocks.save.mockResolvedValueOnce(null);
    await act(async () => {
      buttonWithText("export.run").click();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(mocks.exportVideo).toHaveBeenCalledTimes(exportCallCount);
    expect(useEditorUiStore.getState().exportDialogOpen).toBe(true);
  });

  it("preserves codec-specific export extensions without constraining the native save dialog", async () => {
    await renderDialog();

    await chooseDropdown("export.format", "export.codec.h265");
    mocks.save.mockResolvedValueOnce("/tmp/h265-render");
    await act(async () => {
      buttonWithText("export.run").click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.save).toHaveBeenLastCalledWith(
      expect.objectContaining({
        defaultPath: "/tmp/My Film.mp4",
      }),
    );
    expect(mocks.save.mock.calls.at(-1)?.[0]).not.toHaveProperty("filters");
    expect(mocks.exportVideo).toHaveBeenLastCalledWith(
      { outPath: "/tmp/h265-render.mp4", codec: "h265", quality: "1080p" },
      "video-operation-1",
    );

    await act(async () => useEditorUiStore.setState({ exportDialogOpen: true, toast: null }));
    await chooseDropdown("export.format", "export.codec.prores");
    mocks.save.mockResolvedValueOnce("/tmp/prores-render");
    await act(async () => {
      buttonWithText("export.run").click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.save).toHaveBeenLastCalledWith(
      expect.objectContaining({
        defaultPath: "/tmp/My Film.mov",
      }),
    );
    expect(mocks.save.mock.calls.at(-1)?.[0]).not.toHaveProperty("filters");
    expect(mocks.exportVideo).toHaveBeenLastCalledWith(
      { outPath: "/tmp/prores-render.mov", codec: "prores", quality: "1080p" },
      "video-operation-1",
    );
  });

  it("announces an export failure as an atomic assertive live message", async () => {
    mocks.exportVideo.mockRejectedValueOnce(new Error("encoder unavailable"));
    await renderDialog();

    await act(async () => {
      buttonWithText("export.run").click();
      await Promise.resolve();
      await Promise.resolve();
    });

    const alert = container?.querySelector<HTMLElement>(
      '[role="alert"][aria-live="assertive"][aria-atomic="true"]',
    );
    expect(alert?.textContent).toBe("encoder unavailable");
  });
});
