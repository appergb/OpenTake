// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  exportSubtitles: vi.fn(),
  getDefaultProjectDir: vi.fn(),
  save: vi.fn(),
  saveDialog: vi.fn(),
}));

vi.mock("../../i18n", () => ({
  useT: () => (key: string) => key,
}));

vi.mock("../../lib/api", () => ({
  exportEdl: vi.fn(),
  exportFcpxmlModern: vi.fn(),
  exportOtio: vi.fn(),
  exportSubtitles: mocks.exportSubtitles,
  exportXmeml: vi.fn(),
  getDefaultProjectDir: mocks.getDefaultProjectDir,
}));

vi.mock("../../lib/dialog", () => ({
  saveDialog: mocks.saveDialog,
}));

import { useEditorUiStore } from "../../store/uiStore";
import { useProjectStore } from "../../store/projectStore";
import type { Clip } from "../../lib/types";
import { TitleBar } from "./TitleBar";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

let root: Root | null = null;
let container: HTMLDivElement | null = null;

beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  useEditorUiStore.setState({
    agentPanelVisible: false,
    exportDialogOpen: false,
    settingsOpen: false,
    toast: null,
    view: "editor",
  });
  useProjectStore.setState({
    projectPath: "/tmp/TalkingHeadQA.opentake",
    timeline: {
      fps: 30,
      width: 1920,
      height: 1080,
      settingsConfigured: true,
      tracks: [],
    },
  });
  mocks.saveDialog.mockResolvedValue(mocks.save);
  mocks.getDefaultProjectDir.mockResolvedValue("/tmp");
  mocks.exportSubtitles.mockResolvedValue({ outPath: "/tmp/captions.srt", cueCount: 4 });
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
});

afterEach(async () => {
  if (root) await act(async () => root?.unmount());
  container?.remove();
  root = null;
  container = null;
  useEditorUiStore.setState({ agentPanelVisible: false });
});

describe("TitleBar Agent entry", () => {
  it("renders the specified visible toggle and drives the persisted panel state", async () => {
    await act(async () => root?.render(<TitleBar />));

    const button = container?.querySelector<HTMLButtonElement>(
      'button[aria-label="title.toggleAgent"]',
    );
    expect(button).not.toBeNull();
    expect(button?.getAttribute("aria-pressed")).toBe("false");
    expect(button?.style.opacity).toBe("0.55");
    expect(button?.querySelector("[data-agent-gradient-icon]")).not.toBeNull();

    await act(async () => button?.click());
    expect(useEditorUiStore.getState().agentPanelVisible).toBe(true);
    expect(button?.getAttribute("aria-pressed")).toBe("true");
    expect(button?.style.opacity).toBe("1");
    expect(localStorage.getItem("agentPanelVisible")).toBe("true");

    await act(async () => button?.click());
    expect(useEditorUiStore.getState().agentPanelVisible).toBe(false);
  });

  it("keeps the View menu as a second working mouse entry", async () => {
    await act(async () => root?.render(<TitleBar />));
    const menuButton = container?.querySelector<HTMLButtonElement>(
      'button[aria-label="view.menu"]',
    );
    expect(menuButton).not.toBeNull();

    await act(async () => menuButton?.click());
    const agentItem = [...(container?.querySelectorAll<HTMLButtonElement>('[role="menu"] button') ?? [])]
      .find((button) => button.textContent?.includes("view.agentPanel"));
    expect(agentItem).not.toBeUndefined();

    await act(async () => agentItem?.click());
    expect(useEditorUiStore.getState().agentPanelVisible).toBe(true);
  });
});

describe("TitleBar navigation and video export controls", () => {
  it("control-f52cc89817361a19 return from editor to Home", async () => {
    await act(async () => root?.render(<TitleBar />));
    const home = container?.querySelector<HTMLButtonElement>('button[aria-label="title.backHome"]');
    expect(home).not.toBeNull();

    await act(async () => home?.click());
    expect(useEditorUiStore.getState().view).toBe("home");
  });

  it("control-4bda8f075e1f3a14 open the global Library", async () => {
    await act(async () => root?.render(<TitleBar />));
    const library = container?.querySelector<HTMLButtonElement>('button[aria-label="library.entry"]');
    expect(library).not.toBeNull();

    await act(async () => library?.click());
    expect(useEditorUiStore.getState().view).toBe("library");
  });

  it("control-ff132f94a8c87906 open Settings from the editor", async () => {
    await act(async () => root?.render(<TitleBar />));
    const settings = container?.querySelector<HTMLButtonElement>('button[aria-label="title.settings"]');
    expect(settings).not.toBeNull();

    await act(async () => settings?.click());
    expect(useEditorUiStore.getState().settingsOpen).toBe(true);
  });

  it("control-d7ba227c6447e43e open Video Export", async () => {
    await act(async () => root?.render(<TitleBar />));
    const emptyExport = container?.querySelector<HTMLButtonElement>(
      'button[aria-label="title.exportVideo"]',
    );
    expect(emptyExport?.disabled).toBe(true);
    await act(async () => emptyExport?.click());
    expect(useEditorUiStore.getState().exportDialogOpen).toBe(false);

    await act(async () => useProjectStore.setState({
      timeline: {
        ...useProjectStore.getState().timeline,
        tracks: [{
          id: "v1",
          type: "video",
          muted: false,
          hidden: false,
          syncLocked: true,
          clips: [{} as Clip],
        }],
      },
    }));
    const populatedExport = container?.querySelector<HTMLButtonElement>(
      'button[aria-label="title.exportVideo"]',
    );
    expect(populatedExport?.disabled).toBe(false);
    await act(async () => populatedExport?.click());
    expect(useEditorUiStore.getState().exportDialogOpen).toBe(true);
  });

  it("control-229710d0115f07bc open/close interchange export menu", async () => {
    await act(async () => root?.render(<TitleBar />));
    const trigger = container?.querySelector<HTMLButtonElement>('button[aria-label="title.export"]');
    expect(trigger?.getAttribute("aria-expanded")).toBe("false");

    await act(async () => trigger?.click());
    expect(trigger?.getAttribute("aria-expanded")).toBe("true");
    await act(async () => window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" })));
    expect(trigger?.getAttribute("aria-expanded")).toBe("false");

    await act(async () => trigger?.click());
    await act(async () => window.dispatchEvent(new MouseEvent("mousedown", { bubbles: true })));
    expect(trigger?.getAttribute("aria-expanded")).toBe("false");
  });

  it("control-02d1bf7fff7c1e3a open Video Export from the interchange menu", async () => {
    useProjectStore.setState({
      timeline: {
        ...useProjectStore.getState().timeline,
        tracks: [{
          id: "v1",
          type: "video",
          muted: false,
          hidden: false,
          syncLocked: true,
          clips: [{} as Clip],
        }],
      },
    });
    await act(async () => root?.render(<TitleBar />));
    const trigger = container?.querySelector<HTMLButtonElement>('button[aria-label="title.export"]');
    await act(async () => trigger?.click());
    const renderVideo = [...(container?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]') ?? [])]
      .find((button) => button.textContent === "title.exportRenderVideo");
    expect(renderVideo?.disabled).toBe(false);

    await act(async () => renderVideo?.click());
    expect(trigger?.getAttribute("aria-expanded")).toBe("false");
    expect(useEditorUiStore.getState().exportDialogOpen).toBe(true);
  });
});

describe("TitleBar subtitle export", () => {
  it("control-c035467e6746e570 open/close subtitle export formats", async () => {
    await act(async () => root?.render(<TitleBar />));

    const trigger = container?.querySelector<HTMLButtonElement>(
      'button[aria-label="title.exportSubtitles"]',
    );
    expect(trigger).not.toBeNull();
    expect(trigger?.getAttribute("aria-expanded")).toBe("false");

    await act(async () => trigger?.click());
    expect(trigger?.getAttribute("aria-expanded")).toBe("true");
    expect(container?.querySelector('[role="menu"]')).not.toBeNull();

    await act(async () => window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" })));
    expect(trigger?.getAttribute("aria-expanded")).toBe("false");
    expect(container?.querySelector('[role="menu"]')).toBeNull();

    await act(async () => trigger?.click());
    await act(async () => window.dispatchEvent(new MouseEvent("mousedown", { bubbles: true })));
    expect(trigger?.getAttribute("aria-expanded")).toBe("false");
    expect(container?.querySelector('[role="menu"]')).toBeNull();
  });

  it.each([
    ["srt", "title.exportSrt", "/tmp/captions.srt"],
    ["vtt", "title.exportVtt", "/tmp/captions.vtt"],
  ] as const)("control-f54f4037ab7bffbe export SRT or VTT subtitles (%s)", async (format, label, path) => {
    mocks.save.mockResolvedValue("/tmp/captions");
    mocks.exportSubtitles.mockResolvedValue({ outPath: path, cueCount: 4 });
    await act(async () => root?.render(<TitleBar />));

    const trigger = container?.querySelector<HTMLButtonElement>(
      'button[aria-label="title.exportSubtitles"]',
    );
    expect(trigger).not.toBeNull();
    await act(async () => trigger?.click());

    const menuItem = [...(container?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]') ?? [])]
      .find((button) => button.textContent === label);
    expect(menuItem).not.toBeUndefined();
    await act(async () => {
      menuItem?.click();
      await Promise.resolve();
    });

    expect(mocks.save).toHaveBeenCalledWith(
      expect.objectContaining({
        defaultPath: `/tmp/TalkingHeadQA.${format}`,
        filters: [expect.objectContaining({ extensions: [format] })],
      }),
    );
    expect(mocks.exportSubtitles).toHaveBeenCalledTimes(1);
    expect(mocks.exportSubtitles).toHaveBeenCalledWith(path, format);
    expect(useEditorUiStore.getState().toast?.message).toBe("title.exportSubtitlesDone");
  });

  it("control-f54f4037ab7bffbe export SRT or VTT subtitles reports empty and failure", async () => {
    await act(async () => root?.render(<TitleBar />));
    const trigger = container?.querySelector<HTMLButtonElement>(
      'button[aria-label="title.exportSubtitles"]',
    );

    mocks.save.mockResolvedValueOnce("/tmp/empty.srt");
    mocks.exportSubtitles.mockResolvedValueOnce({ outPath: "/tmp/empty.srt", cueCount: 0 });
    await act(async () => trigger?.click());
    const srt = [...(container?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]') ?? [])]
      .find((button) => button.textContent === "title.exportSrt");
    await act(async () => {
      srt?.click();
      await Promise.resolve();
    });
    expect(useEditorUiStore.getState().toast?.message).toBe("title.exportSubtitlesEmpty");

    useEditorUiStore.setState({ toast: null });
    mocks.save.mockResolvedValueOnce("/tmp/failed.vtt");
    mocks.exportSubtitles.mockRejectedValueOnce(new Error("write failed"));
    await act(async () => trigger?.click());
    const vtt = [...(container?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]') ?? [])]
      .find((button) => button.textContent === "title.exportVtt");
    await act(async () => {
      vtt?.click();
      await Promise.resolve();
    });
    expect(useEditorUiStore.getState().toast?.message).toBe("title.exportSubtitlesFailed");
  });

  it("control-f54f4037ab7bffbe export SRT or VTT subtitles preserves cancel and default-directory behavior", async () => {
    useProjectStore.setState({ projectPath: null });
    await act(async () => root?.render(<TitleBar />));
    const trigger = container?.querySelector<HTMLButtonElement>(
      'button[aria-label="title.exportSubtitles"]',
    );

    mocks.save.mockResolvedValueOnce(null);
    await act(async () => trigger?.click());
    const srt = [...(container?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]') ?? [])]
      .find((button) => button.textContent === "title.exportSrt");
    await act(async () => {
      srt?.click();
      await Promise.resolve();
    });
    expect(mocks.getDefaultProjectDir).toHaveBeenCalledTimes(1);
    expect(mocks.save).toHaveBeenCalledWith(expect.objectContaining({ defaultPath: "/tmp/Timeline.srt" }));
    expect(mocks.exportSubtitles).not.toHaveBeenCalled();
    expect(useEditorUiStore.getState().toast).toBeNull();
  });
});
