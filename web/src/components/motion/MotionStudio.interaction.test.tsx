// @vitest-environment happy-dom

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { MotionDocument, MotionDocumentPatchRequest } from "../../lib/types";
import {
  createMotionStudioStore,
  type MotionStudioBackend,
} from "../../store/motionStudioStore";
import { useEditorUiStore } from "../../store/uiStore";
import { useProjectStore } from "../../store/projectStore";

vi.mock("../../i18n", () => ({
  useT: () => (key: string, values?: Record<string, unknown>) =>
    values ? `${key}:${JSON.stringify(values)}` : key,
}));

vi.mock("./MotionCodeEditor", () => ({
  MotionCodeEditor: ({ value, label, onChange }: { value: string; label: string; onChange: (value: string) => void }) => (
    <textarea aria-label={label} value={value} onChange={(event) => onChange(event.currentTarget.value)} />
  ),
}));

import { MotionStudio } from "./MotionStudio";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const documentFixture: MotionDocument = {
  summary: {
    id: "a3ae19c7-6644-46df-8520-66eecc2a499e",
    title: "Launch title",
    revisionHash: "d".repeat(64),
    updatedAt: 1,
  },
  html: "<main><h1>让创意动起来</h1></main>\n",
  css: "h1 { animation: enter 1s both; }\n@keyframes enter { from { opacity: 0; } }\n",
  parameters: {},
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

function backend(): MotionStudioBackend {
  return {
    list: vi.fn(async () => [documentFixture.summary]),
    create: vi.fn(async () => documentFixture),
    read: vi.fn(async () => documentFixture),
    patch: vi.fn(async (request: MotionDocumentPatchRequest) => ({
      ...documentFixture,
      summary: { ...documentFixture.summary, revisionHash: request.expectedResultHash },
      ...(request.file === "index.html"
        ? { html: request.edits[0]!.replacement }
        : { css: request.edits[0]!.replacement }),
    })),
    hash: vi.fn(async () => "e".repeat(64)),
    preview: vi.fn(async (request) => ({
      revisionHash: request.revisionHash,
      frame: request.frame,
      pngDataUrl: "data:image/png;base64,preview",
      diagnostics: [],
    })),
    cancelPreview: vi.fn(async () => true),
    publish: vi.fn(async () => ({
      clipId: "published-clip",
      assetId: "published-asset",
      contentHash: "f".repeat(64),
      actionName: "Add Motion Graphic",
      sourceDocument: {
        documentId: documentFixture.summary.id,
        revisionHash: documentFixture.summary.revisionHash,
      },
      output: {
        renderer: "opentake-motion-studio",
        rendererVersion: "1.0.0",
        outputFile: "output.mp4",
        fps: 30,
        width: 1920,
        height: 1080,
        durationFrames: 90,
        durationSeconds: 3,
        contentHash: "f".repeat(64),
      },
    })),
    cancelPublish: vi.fn(async () => true),
    onProgress: vi.fn(async () => () => {}),
  } as MotionStudioBackend;
}

describe("Motion Studio authoring workspace", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.useFakeTimers();
    useEditorUiStore.setState({ view: "motion", selectedClipIds: new Set() });
    useProjectStore.setState({ projectEpoch: 1, projectPath: "/tmp/A.opentake" });
    container = document.createElement("div");
    document.body.append(container);
    root = createRoot(container);
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    container.remove();
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it("shows real starter text, switches source tabs by keyboard, and debounces a save", async () => {
    const motionBackend = backend();
    const store = createMotionStudioStore(motionBackend);
    await act(async () => root.render(<MotionStudio store={store} />));
    await act(async () => vi.waitFor(() => expect(store.getState().phase).toBe("ready")));

    const editor = container.querySelector<HTMLTextAreaElement>(
      'textarea[aria-label="motionStudio.codeEditor"]',
    )!;
    expect(editor.value).toContain("让创意动起来");
    const htmlTab = container.querySelector<HTMLButtonElement>('[role="tab"][data-file="index.html"]')!;
    htmlTab.focus();
    await act(async () => htmlTab.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true })));
    const cssTab = container.querySelector<HTMLButtonElement>('[role="tab"][data-file="styles.css"]')!;
    expect(document.activeElement).toBe(cssTab);
    expect(editor.value).toContain("@keyframes enter");

    await act(async () => {
      const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value")!.set!;
      setter.call(editor, "h1 { color: #fff; }\n");
      editor.dispatchEvent(new Event("input", { bubbles: true }));
    });
    await act(async () => vi.advanceTimersByTimeAsync(300));
    await act(async () => vi.waitFor(() => expect(motionBackend.patch).toHaveBeenCalledOnce()));
  });

  it("renders the last-good 16:9 preview and exposes semantic playback, scrub, and parameter controls", async () => {
    const store = createMotionStudioStore(backend());
    await act(async () => root.render(<MotionStudio store={store} />));
    await act(async () => vi.waitFor(() => expect(store.getState().lastGoodPreview).not.toBeNull()));

    const preview = container.querySelector<HTMLImageElement>(
      'figure[role="region"] img[alt="motionStudio.previewFrame"]',
    );
    expect(preview?.src).toContain("data:image/png;base64,preview");
    for (const label of ["play", "pause", "replay"]) {
      expect(
        container.querySelector(`button[aria-label="motionStudio.${label}"]`),
        label,
      ).not.toBeNull();
    }
    expect(container.querySelector('input[type="range"][aria-label="motionStudio.scrub"]')).not.toBeNull();
    for (const name of ["width", "height", "fps", "durationFrames"]) {
      expect(container.querySelector(`input[name="${name}"]`), name).not.toBeNull();
    }
    expect(container.textContent).toContain("@keyframes enter");
  });

  it("keeps diagnostics adjacent to the active source and offers both conflict decisions", async () => {
    const motionBackend = backend();
    vi.mocked(motionBackend.preview).mockRejectedValueOnce({
      message: "compile failed",
      diagnostics: [{ severity: "error", message: "event handler", line: 4, column: 9 }],
    });
    vi.mocked(motionBackend.patch).mockRejectedValueOnce(new Error("motion document revision conflict"));
    const store = createMotionStudioStore(motionBackend);
    await act(async () => root.render(<MotionStudio store={store} />));
    await act(async () => vi.waitFor(() => expect(store.getState().phase).toBe("ready")));
    await act(async () => vi.waitFor(() => expect(container.textContent).toContain("4:9")));
    expect(container.textContent).toContain("4:9");

    await act(async () => store.getState().updateSource("<main>mine</main>\n"));
    await act(async () => vi.advanceTimersByTimeAsync(300));
    await act(async () => vi.waitFor(() => expect(store.getState().conflict).not.toBeNull()));
    expect(container.querySelector('button[data-conflict-action="reload"]')).not.toBeNull();
    expect(container.querySelector('button[data-conflict-action="reapply"]')).not.toBeNull();
  });

  it("does not display source-specific preview or save errors under another file tab", async () => {
    const motionBackend = backend();
    vi.mocked(motionBackend.preview).mockRejectedValueOnce({ message: "HTML preview failed", diagnostics: [] });
    vi.mocked(motionBackend.patch).mockRejectedValueOnce(new Error("HTML save failed"));
    const store = createMotionStudioStore(motionBackend);
    await act(async () => root.render(<MotionStudio store={store} />));
    await act(async () => vi.waitFor(() => expect(container.textContent).toContain("HTML preview failed")));

    const htmlEditor = container.querySelector<HTMLTextAreaElement>('textarea[aria-label="motionStudio.codeEditor"]')!;
    await act(async () => {
      const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value")!.set!;
      setter.call(htmlEditor, "<main>save failure</main>\n");
      htmlEditor.dispatchEvent(new Event("input", { bubbles: true }));
    });
    await act(async () => vi.advanceTimersByTimeAsync(300));
    await act(async () => vi.waitFor(() => expect(container.textContent).toContain("HTML save failed")));

    const cssTab = container.querySelector<HTMLButtonElement>('[role="tab"][data-file="styles.css"]')!;
    await act(async () => cssTab.click());
    expect(container.textContent).not.toContain("HTML preview failed");
    expect(container.textContent).not.toContain("HTML save failed");
  });

  it("defines a deterministic narrow-layout folding order and reduced-motion override", () => {
    const css = readFileSync(resolve(process.cwd(), "src/styles/components.css"), "utf8");
    expect(css).toMatch(/@media\s*\(max-width:\s*900px\)[\s\S]*motion-studio__files[\s\S]*motion-studio__authoring[\s\S]*motion-studio__inspector[\s\S]*motion-studio__timeline/);
    expect(css).toMatch(/@media\s*\(prefers-reduced-motion:\s*reduce\)[\s\S]*motion-studio/);
  });

  it("pauses hidden work, reloads on project identity changes, and disposes on unmount", async () => {
    const second: MotionDocument = {
      ...documentFixture,
      summary: {
        ...documentFixture.summary,
        id: "47e0a1b7-6e7e-4313-91ee-e0be2d0a6a30",
        title: "Project B",
      },
      html: "<main>Project B</main>\n",
    };
    const motionBackend = backend();
    vi.mocked(motionBackend.list)
      .mockResolvedValueOnce([documentFixture.summary])
      .mockResolvedValueOnce([second.summary]);
    vi.mocked(motionBackend.read)
      .mockResolvedValueOnce(documentFixture)
      .mockResolvedValueOnce(second);
    const store = createMotionStudioStore(motionBackend);
    const dispose = vi.fn(store.getState().dispose);
    store.setState({ dispose });

    await act(async () => root.render(<MotionStudio store={store} />));
    await act(async () => vi.waitFor(() => expect(store.getState().phase).toBe("ready")));
    act(() => store.getState().play());
    expect(store.getState().playing).toBe(true);

    await act(async () => useEditorUiStore.getState().setView("home"));
    expect(store.getState().playing).toBe(false);
    expect(motionBackend.cancelPreview).toHaveBeenCalled();

    await act(async () => useProjectStore.setState({ projectEpoch: 2, projectPath: "/tmp/B.opentake" }));
    expect(store.getState().document).toBeNull();
    await act(async () => useEditorUiStore.getState().setView("motion"));
    await act(async () => vi.waitFor(() => expect(store.getState().document?.summary.id).toBe(second.summary.id)));

    await act(async () => root.unmount());
    root = createRoot(container);
    await act(async () => Promise.resolve());
    expect(dispose).toHaveBeenCalledOnce();
  });

  it("waits for hidden-preview cancellation before requesting the visible frame again", async () => {
    const cancellation = deferred<boolean>();
    const motionBackend = backend();
    vi.mocked(motionBackend.cancelPreview).mockImplementation(() => cancellation.promise);
    const store = createMotionStudioStore(motionBackend);
    await act(async () => root.render(<MotionStudio store={store} />));
    await act(async () => vi.waitFor(() => expect(store.getState().previewPhase).toBe("ready")));
    expect(motionBackend.preview).toHaveBeenCalledOnce();

    await act(async () => useEditorUiStore.getState().setView("home"));
    await act(async () => useEditorUiStore.getState().setView("motion"));
    expect(motionBackend.preview).toHaveBeenCalledOnce();

    cancellation.resolve(true);
    await act(async () => vi.waitFor(() => expect(motionBackend.preview).toHaveBeenCalledTimes(2)));
  });

  it("publishes the exact revision then navigates to and selects the committed clip", async () => {
    const motionBackend = backend();
    const store = createMotionStudioStore(motionBackend);
    await act(async () => root.render(<MotionStudio store={store} />));
    await act(async () => vi.waitFor(() => expect(store.getState().previewPhase).toBe("ready")));

    const publish = container.querySelector<HTMLButtonElement>('[data-motion-publish="true"]');
    expect(publish).not.toBeNull();
    const transparent = container.querySelector<HTMLInputElement>('input[name="transparent"]');
    expect(transparent).not.toBeNull();
    expect(transparent!.checked).toBe(false);
    await act(async () => transparent!.click());
    expect(store.getState().transparent).toBe(true);
    await act(async () => publish!.click());
    await act(async () => vi.waitFor(() => expect(store.getState().publishPhase).toBe("complete")));

    expect(motionBackend.publish).toHaveBeenCalledOnce();
    expect(motionBackend.publish).toHaveBeenCalledWith(expect.objectContaining({ transparent: true }));
    expect(useEditorUiStore.getState().view).toBe("editor");
    expect(useEditorUiStore.getState().selectedClipIds).toEqual(new Set(["published-clip"]));
  });

  it("shows exact completed and total render frames while publishing", async () => {
    const motionBackend = backend();
    const store = createMotionStudioStore(motionBackend);
    await act(async () => root.render(<MotionStudio store={store} />));
    await act(async () => vi.waitFor(() => expect(store.getState().previewPhase).toBe("ready")));

    await act(async () => store.setState({
      publishPhase: "rendering",
      publishFrameProgress: { done: 27, total: 90 },
    }));

    expect(container.textContent).toContain("27");
    expect(container.textContent).toContain("90");
  });

  it("keeps a later Home navigation when publishing completes after Motion is hidden", async () => {
    const completion = deferred<Awaited<ReturnType<MotionStudioBackend["publish"]>>>();
    const motionBackend = backend();
    vi.mocked(motionBackend.publish).mockImplementation(() => completion.promise);
    const store = createMotionStudioStore(motionBackend);
    await act(async () => root.render(<MotionStudio store={store} />));
    await act(async () => vi.waitFor(() => expect(store.getState().previewPhase).toBe("ready")));

    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-motion-publish="true"]')!.click();
    });
    await act(async () => vi.waitFor(() => expect(motionBackend.publish).toHaveBeenCalledOnce()));
    await act(async () => useEditorUiStore.getState().setView("home"));
    completion.resolve((await backend().publish({} as never)));
    await act(async () => vi.waitFor(() => expect(store.getState().publishPhase).toBe("complete")));

    expect(useEditorUiStore.getState().view).toBe("home");
    expect(useEditorUiStore.getState().selectedClipIds).not.toContain("published-clip");
  });

  it("does not navigate after an unmounted Motion workspace receives a late publish commit", async () => {
    const completion = deferred<Awaited<ReturnType<MotionStudioBackend["publish"]>>>();
    const motionBackend = backend();
    vi.mocked(motionBackend.publish).mockImplementation(() => completion.promise);
    const store = createMotionStudioStore(motionBackend);
    await act(async () => root.render(<MotionStudio store={store} />));
    await act(async () => vi.waitFor(() => expect(store.getState().previewPhase).toBe("ready")));

    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-motion-publish="true"]')!.click();
    });
    await act(async () => vi.waitFor(() => expect(motionBackend.publish).toHaveBeenCalledOnce()));
    await act(async () => root.unmount());
    root = createRoot(container);
    completion.resolve((await backend().publish({} as never)));
    await act(async () => vi.waitFor(() => expect(store.getState().publishPhase).toBe("complete")));

    expect(useEditorUiStore.getState().view).toBe("motion");
    expect(useEditorUiStore.getState().selectedClipIds).not.toContain("published-clip");
  });
});
