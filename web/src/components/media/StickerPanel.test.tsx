// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { beforeEach, afterEach, describe, it, expect, vi } from "vitest";
import type { MediaItem, MediaList } from "../../lib/types";
import { useMediaStore, resetProjectMediaState, beginMediaImport, endMediaImport } from "../../store/mediaStore";
import { useProjectStore } from "../../store/projectStore";
import { useEditorUiStore } from "../../store/uiStore";
import { t } from "../../i18n";

vi.mock("../../lib/api", async (original) => ({
  ...await original<typeof import("../../lib/api")>(),
  syncProjectFavorites: vi.fn(async () => ({ media: { items: useMediaStore.getState().items, folders: [] }, migratedLegacyAssetIds: [], failures: [] })),
  importMedia: vi.fn(),
  getMedia: vi.fn(),
  preloadMedia: vi.fn().mockResolvedValue(null),
  generateThumbnail: vi.fn().mockResolvedValue(null),
}));
vi.mock("../../lib/dialog", () => ({ openDialog: vi.fn(), saveDialog: vi.fn() }));
vi.mock("../../store/editActions", async (original) => ({
  ...await original<typeof import("../../store/editActions")>(),
  addMediaToTimeline: vi.fn().mockResolvedValue(undefined),
}));
import * as api from "../../lib/api";
import { openDialog } from "../../lib/dialog";
import { addMediaToTimeline } from "../../store/editActions";
import { MediaPanel, MEDIA_DND_TYPE } from "./MediaPanel";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
let root: Root;
let host: HTMLDivElement;
const open = vi.fn();
const sticker = (id: string, type: MediaItem["type"] = "image", extra: Partial<MediaItem> = {}): MediaItem => ({
  id, name: id, type, duration: 3, hasAudio: false, favorite: false, ...extra,
});
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
function button(key: string) {
  const found = [...host.querySelectorAll<HTMLButtonElement>("button")].find(el => el.textContent === t(key));
  expect(found, `button ${key}`).toBeDefined();
  return found!;
}
function card(id: string) { return host.querySelector<HTMLElement>(`[data-media-asset-id="${id}"]`)!; }
async function mount(items: MediaItem[] = []) {
  useMediaStore.setState({ items });
  await act(async () => root.render(<MediaPanel />));
}
async function click(el: HTMLElement) { await act(async () => el.click()); }

beforeEach(() => {
  vi.clearAllMocks();
  resetProjectMediaState();
  useProjectStore.setState({ projectEpoch: 1, projectPath: "/project-a", compatibilityReadOnly: false });
  useEditorUiStore.setState({ mediaTab: "sticker", selectedMediaAssetIds: new Set(), previewMediaId: null, toasts: [] });
  vi.mocked(openDialog).mockResolvedValue(open);
  open.mockResolvedValue(null);
  vi.mocked(api.importMedia).mockResolvedValue({ items: [], folders: [] });
  vi.mocked(api.getMedia).mockResolvedValue({ items: [], folders: [] });
  vi.mocked(addMediaToTimeline).mockReset().mockResolvedValue(undefined);
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});
afterEach(async () => {
  await act(async () => root.unmount());
  host.remove();
  resetProjectMediaState();
});

describe("Sticker panel", () => {
  it("opens from an enabled tab and gives an accessible empty state and explicit import", async () => {
    useEditorUiStore.setState({ mediaTab: "text" });
    await mount();
    const tab = host.querySelector<HTMLButtonElement>("#media-main-tab-sticker")!;
    expect(tab.disabled).toBe(false);
    await click(tab);
    expect(host.querySelector("#media-main-panel-sticker")?.getAttribute("hidden")).toBeNull();
    expect(host.querySelector('[role="status"]')?.textContent).toContain(t("sticker.empty"));
    expect(button("sticker.import").disabled).toBe(false);
    expect(button("sticker.add").disabled).toBe(true);
  });

  it("shows images and Lottie across folders, selects/previews real cards and places the selection once", async () => {
    const image = sticker("image", "image", { folderId: "nested" });
    const lottie = sticker("motion", "lottie");
    await mount([image, lottie, sticker("video", "video"), sticker("audio", "audio")]);
    expect([...host.querySelectorAll("[data-media-asset-id]")].map(el => el.getAttribute("data-media-asset-id"))).toEqual(["image", "motion"]);
    await click(card("motion"));
    expect(useEditorUiStore.getState().previewMediaId).toBe("motion");
    expect(card("motion").getAttribute("aria-selected")).toBe("true");
    await click(button("sticker.add"));
    expect(addMediaToTimeline).toHaveBeenCalledExactlyOnceWith(lottie);
    vi.mocked(addMediaToTimeline).mockClear();
    await act(async () => card("image").dispatchEvent(new MouseEvent("dblclick", { bubbles: true })));
    expect(addMediaToTimeline).toHaveBeenCalledExactlyOnceWith(image);
    const dataTransfer = { setData: vi.fn(), setDragImage: vi.fn(), effectAllowed: "" };
    const drag = new Event("dragstart", { bubbles: true });
    Object.defineProperty(drag, "dataTransfer", { value: dataTransfer });
    await act(async () => card("image").dispatchEvent(drag));
    expect(dataTransfer.setData).toHaveBeenCalledWith(MEDIA_DND_TYPE, "image");
    await act(async () => card("image").dispatchEvent(new Event("dragend", { bubbles: true })));
  });

  it("imports only supported sticker paths, refreshes Rust's catalog and reports skipped files", async () => {
    open.mockResolvedValue(["/a.PNG", "/b.json", "/c.lottie", "/movie.mp4"]);
    const list = { items: [sticker("a"), sticker("b", "lottie"), sticker("c", "lottie")], folders: [] };
    vi.mocked(api.importMedia).mockResolvedValue({ ...list, skipped: ["/bad.json"] });
    vi.mocked(api.getMedia).mockResolvedValue(list);
    await mount();
    await click(button("sticker.import"));
    expect(open).toHaveBeenCalledWith(expect.objectContaining({ multiple: true, directory: false, filters: [expect.objectContaining({ extensions: expect.arrayContaining(["png", "json", "lottie"]) })] }));
    expect(api.importMedia).toHaveBeenCalledExactlyOnceWith(["/a.PNG", "/b.json", "/c.lottie"]);
    expect(card("b")).not.toBeNull();
    expect(host.querySelector('[role="status"]')?.textContent).toContain(t("media.importSkipped", { count: 2 }));
  });

  it("cancels cleanly and reports an unavailable native dialog", async () => {
    await mount();
    await click(button("sticker.import"));
    expect(api.importMedia).not.toHaveBeenCalled();
    expect(button("sticker.import").disabled).toBe(false);
    vi.mocked(openDialog).mockResolvedValue(null);
    await click(button("sticker.import"));
    expect(host.querySelector('[role="alert"]')?.textContent).toContain(t("sticker.desktopRequired"));
  });

  it("blocks duplicate imports while the picker is open and while importing", async () => {
    const picked = deferred<string[]>();
    const imported = deferred<MediaList>();
    open.mockReturnValue(picked.promise);
    vi.mocked(api.importMedia).mockReturnValue(imported.promise);
    await mount();
    await click(button("sticker.import"));
    expect(button("sticker.import").disabled).toBe(true);
    await click(button("sticker.import"));
    expect(open).toHaveBeenCalledTimes(1);
    await act(async () => picked.resolve(["/a.png"]));
    expect(useMediaStore.getState().importing).toBe(true);
    expect(button("sticker.import").disabled).toBe(true);
    await act(async () => imported.resolve({ items: [], folders: [] }));
    expect(useMediaStore.getState().importing).toBe(false);
    expect(button("sticker.import").disabled).toBe(false);
  });

  it.each(["picker", "import"])("ignores stale %s completion after a project switch", async (stage) => {
    const picked = deferred<string[]>();
    const imported = deferred<MediaList>();
    open.mockReturnValue(stage === "picker" ? picked.promise : Promise.resolve(["/a.png"]));
    vi.mocked(api.importMedia).mockReturnValue(imported.promise);
    await mount();
    await click(button("sticker.import"));
    await act(async () => {
      useProjectStore.setState({ projectEpoch: 2, projectPath: "/project-b" });
      resetProjectMediaState();
      useMediaStore.setState({ items: [sticker("new-project")] });
    });
    let newImport!: ReturnType<typeof beginMediaImport>;
    await act(async () => { newImport = beginMediaImport(); });
    await act(async () => {
      picked.resolve(["/old.png"]);
      imported.resolve({ items: [sticker("old-project")], folders: [] });
    });
    expect(api.importMedia).toHaveBeenCalledTimes(stage === "picker" ? 0 : 1);
    expect(api.getMedia).not.toHaveBeenCalled();
    expect(card("new-project")).not.toBeNull();
    expect(card("old-project")).toBeNull();
    expect(useMediaStore.getState().importing).toBe(true);
    await act(async () => endMediaImport(newImport));
  });

  it("shows import failures and permits retry", async () => {
    open.mockResolvedValue(["/invalid.json"]);
    vi.mocked(api.importMedia).mockRejectedValue(new Error("Invalid Lottie"));
    await mount();
    await click(button("sticker.import"));
    expect(host.querySelector('[role="alert"]')?.textContent).toContain("Invalid Lottie");
    expect(button("sticker.import").disabled).toBe(false);
    expect(useMediaStore.getState().importing).toBe(false);
  });

  it("disables missing and unfinished stickers with an explanation and no placement or drag", async () => {
    await mount([sticker("offline", "image", { missing: true }), sticker("busy", "lottie", { generationStatus: "generating" }), sticker("failed", "image", { generationStatus: "failed" })]);
    for (const id of ["offline", "busy", "failed"]) {
      await click(card(id));
      expect(button("sticker.add").disabled).toBe(true);
      expect(card(id).getAttribute("draggable")).toBe("false");
      expect(card(id).getAttribute("aria-disabled")).toBe("true");
      await act(async () => card(id).dispatchEvent(new MouseEvent("dblclick", { bubbles: true })));
    }
    expect(addMediaToTimeline).not.toHaveBeenCalled();
    expect(useEditorUiStore.getState().previewMediaId).toBeNull();
    expect(host.textContent).toContain(t("sticker.failed"));
  });

  it("keeps a placement pending once, displays its failure, then enables retry", async () => {
    const placement = deferred<void>();
    vi.mocked(addMediaToTimeline).mockReturnValue(placement.promise);
    await mount([sticker("one")]);
    await click(card("one"));
    await click(button("sticker.add"));
    expect(button("sticker.add").disabled).toBe(true);
    await act(async () => card("one").dispatchEvent(new MouseEvent("dblclick", { bubbles: true })));
    expect(addMediaToTimeline).toHaveBeenCalledTimes(1);
    await act(async () => placement.reject(new Error("Track locked")));
    expect(host.querySelector('[role="alert"]')?.textContent).toContain("Track locked");
    expect(button("sticker.add").disabled).toBe(false);
  });

  it.each(["visible", "hidden"])("retains pending placement across tabs and lets a %s failure be retried once", async (surface) => {
    const placement = deferred<void>();
    const retry = deferred<void>();
    vi.mocked(addMediaToTimeline)
      .mockReturnValueOnce(placement.promise)
      .mockReturnValueOnce(retry.promise);
    await mount([sticker("one")]);
    await click(card("one"));
    await click(button("sticker.add"));
    await click(host.querySelector<HTMLButtonElement>("#media-main-tab-text")!);
    expect(host.querySelector("#media-main-panel-sticker")?.hasAttribute("hidden")).toBe(true);
    await click(host.querySelector<HTMLButtonElement>("#media-main-tab-sticker")!);
    expect(button("sticker.add").disabled).toBe(true);
    expect(button("sticker.import").disabled).toBe(true);
    await click(button("sticker.add"));
    await act(async () => card("one").dispatchEvent(new MouseEvent("dblclick", { bubbles: true })));
    expect(addMediaToTimeline).toHaveBeenCalledTimes(1);

    if (surface === "hidden") await click(host.querySelector<HTMLButtonElement>("#media-main-tab-text")!);
    await act(async () => placement.reject(new Error("Track locked after tab switch")));
    if (surface === "hidden") await click(host.querySelector<HTMLButtonElement>("#media-main-tab-sticker")!);
    expect(host.querySelector('[role="alert"]')?.textContent).toContain("Track locked after tab switch");
    expect(button("sticker.add").disabled).toBe(false);
    await click(button("sticker.add"));
    await click(button("sticker.add"));
    expect(addMediaToTimeline).toHaveBeenCalledTimes(2);
    expect(button("sticker.add").disabled).toBe(true);
    expect(host.querySelector('[role="alert"]')).toBeNull();
    await act(async () => retry.resolve());
    expect(button("sticker.add").disabled).toBe(false);
  });

  it("isolates an old hidden placement from the new project's pending placement", async () => {
    const oldPlacement = deferred<void>();
    const newPlacement = deferred<void>();
    vi.mocked(addMediaToTimeline)
      .mockReturnValueOnce(oldPlacement.promise)
      .mockReturnValueOnce(newPlacement.promise);
    await mount([sticker("old")]);
    await click(card("old"));
    await click(button("sticker.add"));
    await click(host.querySelector<HTMLButtonElement>("#media-main-tab-text")!);
    await act(async () => {
      useProjectStore.setState({ projectEpoch: 4, projectPath: "/project-d" });
      resetProjectMediaState();
      useMediaStore.setState({ items: [sticker("current")] });
    });
    await click(host.querySelector<HTMLButtonElement>("#media-main-tab-sticker")!);
    await click(card("current"));
    expect(button("sticker.add").disabled).toBe(false);
    await click(button("sticker.add"));
    await act(async () => oldPlacement.reject(new Error("Old hidden placement failed")));
    expect(host.querySelector('[role="alert"]')).toBeNull();
    expect(button("sticker.add").disabled).toBe(true);
    await click(button("sticker.add"));
    expect(addMediaToTimeline).toHaveBeenCalledTimes(2);
    await act(async () => newPlacement.resolve());
    expect(button("sticker.add").disabled).toBe(false);
  });

  it("shows a placement failure even when a previous import failed", async () => {
    await mount([sticker("one")]);
    await act(async () => useMediaStore.getState().setError("Invalid Lottie"));
    vi.mocked(addMediaToTimeline).mockRejectedValue(new Error("Track locked"));
    await click(card("one"));
    await click(button("sticker.add"));
    expect(host.querySelector('[role="alert"]')?.textContent).toContain("Track locked");
  });

  it("roves sticker cards by keyboard without previewing an unavailable sticker", async () => {
    await mount([sticker("ready"), sticker("offline", "image", { missing: true }), sticker("motion", "lottie")]);
    await act(async () => card("ready").focus());
    await act(async () => card("ready").dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true })));
    expect(document.activeElement).toBe(card("offline"));
    expect(useEditorUiStore.getState().previewMediaId).toBe("ready");
    expect(button("sticker.add").disabled).toBe(true);
    await act(async () => card("offline").dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true })));
    expect(document.activeElement).toBe(card("motion"));
    expect(useEditorUiStore.getState().previewMediaId).toBe("motion");
    expect(button("sticker.add").disabled).toBe(false);
  });

  it.each(["import", "placement"])("does not leak an old %s failure into the new project", async (stage) => {
    const operation = deferred<never>();
    open.mockResolvedValue(["/a.png"]);
    vi.mocked(api.importMedia).mockReturnValue(operation.promise);
    vi.mocked(addMediaToTimeline).mockReturnValue(operation.promise);
    await mount([sticker("old")]);
    await click(card("old"));
    await click(button(stage === "import" ? "sticker.import" : "sticker.add"));
    await act(async () => {
      useProjectStore.setState({ projectEpoch: 3, projectPath: "/project-c" });
      resetProjectMediaState();
      useMediaStore.setState({ items: [sticker("current")] });
    });
    await act(async () => operation.reject(new Error("Old project failed")));
    expect(host.querySelector('[role="alert"]')).toBeNull();
    expect(card("current")).not.toBeNull();
    expect(button("sticker.import").disabled).toBe(false);
  });

  it("disables import when there is no project", async () => {
    useProjectStore.setState({ projectPath: null });
    await mount();
    expect(button("sticker.import").disabled).toBe(true);
    expect(host.textContent).toContain(t("sticker.noProject"));
  });

  it("explains why import and placement are disabled for a read-only project", async () => {
    useProjectStore.setState({ compatibilityReadOnly: true });
    await mount([sticker("one")]);
    await click(card("one"));
    expect(button("sticker.import").disabled).toBe(true);
    expect(button("sticker.add").disabled).toBe(true);
    expect(host.textContent).toContain(t("sticker.readOnly"));
    await act(async () => card("one").dispatchEvent(new MouseEvent("dblclick", { bubbles: true })));
    expect(addMediaToTimeline).not.toHaveBeenCalled();
  });
});
