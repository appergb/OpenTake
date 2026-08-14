import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type {
  MotionDocument,
  MotionDocumentPatchRequest,
  MotionPreviewRequest,
} from "../lib/types";
import {
  createMotionStudioStore,
  type MotionStudioBackend,
} from "./motionStudioStore";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

const starter: MotionDocument = {
  summary: {
    id: "2ca6a727-e513-4d08-99f6-63d762fa48f0",
    title: "Starter",
    revisionHash: "a".repeat(64),
    updatedAt: 1,
  },
  html: '<main class="motion-stage">\n  <h1>让创意动起来</h1>\n</main>\n',
  css: ".motion-stage { display: grid; }\n@keyframes enter { from { opacity: 0; } }\n",
  parameters: {},
};

function backend(overrides: Partial<MotionStudioBackend> = {}): MotionStudioBackend {
  return {
    list: vi.fn(async () => [starter.summary]),
    create: vi.fn(async () => starter),
    read: vi.fn(async () => starter),
    patch: vi.fn(async (request: MotionDocumentPatchRequest) => ({
      ...starter,
      summary: { ...starter.summary, revisionHash: request.expectedResultHash, updatedAt: 2 },
      ...(request.file === "index.html"
        ? { html: request.edits[0]?.replacement ?? starter.html }
        : { css: request.edits[0]?.replacement ?? starter.css }),
    })),
    hash: vi.fn(async () => "e".repeat(64)),
    preview: vi.fn(async (request: MotionPreviewRequest) => ({
      revisionHash: request.revisionHash,
      frame: request.frame,
      pngDataUrl: `data:image/png;base64,frame-${request.frame}`,
      diagnostics: [],
    })),
    cancelPreview: vi.fn(async () => true),
    publish: vi.fn(async () => ({
      clipId: "motion-clip",
      assetId: "motion-asset",
      contentHash: "f".repeat(64),
      actionName: "Add Motion Graphic",
      sourceDocument: {
        documentId: starter.summary.id,
        revisionHash: starter.summary.revisionHash,
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
    ...overrides,
  } as MotionStudioBackend;
}

describe("Motion Studio store", () => {
  beforeEach(() => vi.useFakeTimers());

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it("loads the project document and creates the visible starter only when the catalog is empty", async () => {
    const existingBackend = backend();
    const existing = createMotionStudioStore(existingBackend);
    await existing.getState().load();
    expect(existing.getState()).toMatchObject({
      phase: "ready",
      activeFile: "index.html",
      html: starter.html,
      css: starter.css,
    });
    expect(existingBackend.create).not.toHaveBeenCalled();

    const emptyBackend = backend({ list: vi.fn(async () => []) });
    const empty = createMotionStudioStore(emptyBackend);
    await empty.getState().load();
    expect(emptyBackend.create).toHaveBeenCalledWith({ title: null });
    expect(empty.getState().html).toContain("让创意动起来");
  });

  it("debounces a full-source atomic patch and converts CodeMirror text to UTF-8 byte offsets", async () => {
    const motionBackend = backend();
    const store = createMotionStudioStore(motionBackend);
    await store.getState().load();
    const next = '<main class="motion-stage">你好 👋</main>\n';

    store.getState().updateSource(next);
    await vi.advanceTimersByTimeAsync(299);
    expect(motionBackend.patch).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1);

    expect(store.getState().error).toBeNull();
    await vi.waitFor(() => expect(motionBackend.patch).toHaveBeenCalledOnce());
    const request = vi.mocked(motionBackend.patch).mock.calls[0]![0];
    expect(request).toMatchObject({
      documentId: starter.summary.id,
      file: "index.html",
      baselineHash: starter.summary.revisionHash,
      edits: [{ start: 0, end: new TextEncoder().encode(starter.html).length, replacement: next }],
    });
    expect(motionBackend.hash).toHaveBeenCalledWith({
      documentId: starter.summary.id,
      file: "index.html",
      baselineHash: starter.summary.revisionHash,
      edits: [{ start: 0, end: new TextEncoder().encode(starter.html).length, replacement: next }],
    });
    expect(request.expectedResultHash).toBe("e".repeat(64));
  });

  it("serializes saves without letting an older completion overwrite the newest editor text", async () => {
    const first = deferred<MotionDocument>();
    const motionBackend = backend({
      patch: vi
        .fn()
        .mockImplementationOnce(() => first.promise)
        .mockImplementation(async (request: MotionDocumentPatchRequest) => ({
          ...starter,
          summary: { ...starter.summary, revisionHash: request.expectedResultHash, updatedAt: 3 },
          html: request.edits[0]!.replacement,
        })),
    });
    const store = createMotionStudioStore(motionBackend);
    await store.getState().load();

    store.getState().updateSource("<main>first</main>\n");
    await vi.advanceTimersByTimeAsync(300);
    store.getState().updateSource("<main>second</main>\n");
    first.resolve({
      ...starter,
      summary: { ...starter.summary, revisionHash: "b".repeat(64), updatedAt: 2 },
      html: "<main>first</main>\n",
    });
    await Promise.resolve();
    await vi.runOnlyPendingTimersAsync();
    await vi.waitFor(() => expect(motionBackend.patch).toHaveBeenCalledTimes(2));

    expect(store.getState().html).toBe("<main>second</main>\n");
    expect(vi.mocked(motionBackend.patch).mock.calls[1]![0].baselineHash).toBe("b".repeat(64));
  });

  it("offers an explicit remote reload or local reapply after a revision conflict", async () => {
    const remote = {
      ...starter,
      summary: { ...starter.summary, revisionHash: "c".repeat(64), updatedAt: 4 },
      html: "<main>remote</main>\n",
    };
    const patch = vi
      .fn()
      .mockRejectedValueOnce(new Error("motion document revision conflict"))
      .mockImplementation(async (request: MotionDocumentPatchRequest) => ({
        ...remote,
        summary: { ...remote.summary, revisionHash: request.expectedResultHash, updatedAt: 5 },
        html: request.edits[0]!.replacement,
      }));
    const motionBackend = backend({ patch, read: vi.fn(async () => remote) });
    const store = createMotionStudioStore(motionBackend);
    await store.getState().load();
    store.getState().updateSource("<main>mine</main>\n");
    await vi.advanceTimersByTimeAsync(300);
    await vi.waitFor(() => expect(store.getState().conflict).not.toBeNull());
    expect(store.getState().conflict).toMatchObject({ file: "index.html" });

    await store.getState().reloadConflict();
    expect(store.getState().html).toBe(remote.html);
    expect(store.getState().conflict).toBeNull();

    store.getState().updateSource("<main>mine again</main>\n");
    patch.mockRejectedValueOnce(new Error("motion document revision conflict"));
    await vi.advanceTimersByTimeAsync(300);
    await store.getState().reapplyConflict();
    await vi.runOnlyPendingTimersAsync();
    expect(patch).toHaveBeenLastCalledWith(
      expect.objectContaining({ baselineHash: remote.summary.revisionHash }),
    );
    expect(store.getState().html).toBe("<main>mine again</main>\n");
  });

  it("does not let a late conflict decision overwrite typing that happened after the click", async () => {
    const remoteRead = deferred<MotionDocument>();
    const remote = {
      ...starter,
      summary: { ...starter.summary, revisionHash: "c".repeat(64), updatedAt: 4 },
      html: "<main>remote</main>\n",
    };
    const motionBackend = backend({
      patch: vi.fn().mockRejectedValueOnce(new Error("motion document revision conflict")),
      read: vi
        .fn()
        .mockResolvedValueOnce(starter)
        .mockImplementationOnce(() => remoteRead.promise),
    });
    const store = createMotionStudioStore(motionBackend);
    await store.getState().load();
    store.getState().updateSource("<main>conflicted</main>\n");
    await vi.advanceTimersByTimeAsync(300);
    await vi.waitFor(() => expect(store.getState().conflict).not.toBeNull());

    const reload = store.getState().reloadConflict();
    store.getState().updateSource("<main>typed after reload click</main>\n");
    remoteRead.resolve(remote);
    await reload;

    expect(store.getState().html).toBe("<main>typed after reload click</main>\n");
    expect(store.getState().dirtyFiles["index.html"]).toBe(true);
  });

  it("reapplies every dirty file instead of discarding a second pending source", async () => {
    const remote = {
      ...starter,
      summary: { ...starter.summary, revisionHash: "c".repeat(64), updatedAt: 4 },
      html: "<main>remote</main>\n",
      css: "main { color: remote; }\n",
    };
    const motionBackend = backend({
      patch: vi.fn().mockRejectedValueOnce(new Error("motion document revision conflict")),
      read: vi.fn().mockResolvedValueOnce(starter).mockResolvedValueOnce(remote),
    });
    const store = createMotionStudioStore(motionBackend);
    await store.getState().load();
    store.getState().updateSource("<main>local html</main>\n");
    store.getState().setActiveFile("styles.css");
    store.getState().updateSource("main { color: local; }\n");
    await vi.advanceTimersByTimeAsync(300);
    await vi.waitFor(() => expect(store.getState().conflict?.file).toBe("index.html"));

    await store.getState().reapplyConflict();

    expect(store.getState()).toMatchObject({
      html: "<main>local html</main>\n",
      css: "main { color: local; }\n",
      dirtyFiles: { "index.html": true, "styles.css": true },
    });
  });

  it("ignores stale preview replies and retains the last-good frame with line/column diagnostics", async () => {
    const initial = deferred<Awaited<ReturnType<MotionStudioBackend["preview"]>>>();
    const preview = vi
      .fn()
      .mockImplementationOnce(() => initial.promise)
      .mockImplementationOnce(async (request: MotionPreviewRequest) => ({
        revisionHash: request.revisionHash,
        frame: request.frame,
        pngDataUrl: "data:image/png;base64,newest",
        diagnostics: [],
      }))
      .mockRejectedValueOnce({
        message: "Unsupported active content",
        diagnostics: [{ severity: "error", message: "event handlers are not allowed", line: 7, column: 12 }],
      });
    const store = createMotionStudioStore(backend({ preview }));
    await store.getState().load();
    store.getState().setFrame(1);
    await Promise.resolve();
    initial.resolve({
      revisionHash: starter.summary.revisionHash,
      frame: 0,
      pngDataUrl: "data:image/png;base64,stale",
      diagnostics: [],
    });
    await Promise.resolve();
    expect(store.getState().lastGoodPreview?.frame).toBe(1);

    store.getState().setFrame(2);
    await Promise.resolve();
    expect(store.getState().lastGoodPreview?.pngDataUrl).toBe("data:image/png;base64,newest");
    expect(store.getState().diagnostics).toEqual([
      expect.objectContaining({ line: 7, column: 12 }),
    ]);
  });

  it("never carries a last-good frame across a document boundary", async () => {
    const other = {
      ...starter,
      summary: { ...starter.summary, id: "f1148cf4-789b-4eea-9fb8-7d277914d4d7", title: "Other" },
    };
    const preview = vi
      .fn()
      .mockResolvedValueOnce({
        revisionHash: starter.summary.revisionHash,
        frame: 0,
        pngDataUrl: "data:image/png;base64,document-a",
        diagnostics: [],
      })
      .mockRejectedValueOnce(new Error("document B cannot render"));
    const store = createMotionStudioStore(backend({
      list: vi.fn(async () => [starter.summary, other.summary]),
      read: vi.fn(async (id) => id === other.summary.id ? other : starter),
      preview,
    }));
    await store.getState().load();
    await vi.waitFor(() => expect(store.getState().lastGoodPreview).not.toBeNull());

    await store.getState().selectDocument(other.summary.id);
    await vi.waitFor(() => expect(store.getState().previewPhase).toBe("error"));

    expect(store.getState().document?.summary.id).toBe(other.summary.id);
    expect(store.getState().lastGoodPreview).toBeNull();
  });

  it("clamps shared publish parameters and advances playback on integer frames", async () => {
    const store = createMotionStudioStore(backend());
    await store.getState().load();
    store.getState().setParameter("width", 99_999);
    store.getState().setParameter("height", -4);
    store.getState().setParameter("fps", 2);
    store.getState().setParameter("durationFrames", 3);
    expect(store.getState().parameters).toEqual({
      width: 4096,
      height: 2,
      fps: 2,
      durationFrames: 3,
    });
    store.getState().setParameter("width", 101);
    expect(store.getState().parameters.width).toBe(102);

    store.getState().play();
    await vi.advanceTimersByTimeAsync(500);
    expect(store.getState().frame).toBe(1);
    await vi.advanceTimersByTimeAsync(1_000);
    expect(store.getState()).toMatchObject({ frame: 2, playing: false });

    store.getState().replay();
    expect(store.getState()).toMatchObject({ frame: 0, playing: true });
    store.getState().pause();
    expect(store.getState().playing).toBe(false);
  });

  it("does not cancel every slow Chromium frame while playback is advancing", async () => {
    const firstFrame = deferred<Awaited<ReturnType<MotionStudioBackend["preview"]>>>();
    const preview = vi
      .fn()
      .mockImplementationOnce(() => firstFrame.promise)
      .mockImplementation(async (request: MotionPreviewRequest) => ({
        revisionHash: request.revisionHash,
        frame: request.frame,
        pngDataUrl: `data:image/png;base64,${request.frame}`,
        diagnostics: [],
      }));
    const store = createMotionStudioStore(backend({ preview }));
    store.getState().setParameter("fps", 2);
    await store.getState().load();
    store.getState().play();

    await vi.advanceTimersByTimeAsync(500);
    expect(store.getState().frame).toBe(0);
    expect(preview).toHaveBeenCalledOnce();

    firstFrame.resolve({
      revisionHash: starter.summary.revisionHash,
      frame: 0,
      pngDataUrl: "data:image/png;base64,0",
      diagnostics: [],
    });
    await Promise.resolve();
    await vi.advanceTimersByTimeAsync(500);
    expect(store.getState().frame).toBe(1);
    expect(preview).toHaveBeenCalledTimes(2);
  });

  it("lets the current frame finish when the user pauses playback", async () => {
    const nextFrame = deferred<Awaited<ReturnType<MotionStudioBackend["preview"]>>>();
    const preview = vi
      .fn()
      .mockImplementationOnce(async (request: MotionPreviewRequest) => ({
        revisionHash: request.revisionHash,
        frame: request.frame,
        pngDataUrl: "data:image/png;base64,frame-0",
        diagnostics: [],
      }))
      .mockImplementationOnce(() => nextFrame.promise);
    const motionBackend = backend({ preview });
    const store = createMotionStudioStore(motionBackend);
    store.getState().setParameter("fps", 2);
    store.getState().setParameter("durationFrames", 3);
    await store.getState().load();
    await vi.waitFor(() => expect(store.getState().previewPhase).toBe("ready"));
    store.getState().play();
    await vi.advanceTimersByTimeAsync(500);
    expect(store.getState().frame).toBe(1);

    store.getState().pause();
    nextFrame.resolve({
      revisionHash: starter.summary.revisionHash,
      frame: 1,
      pngDataUrl: "data:image/png;base64,frame-1",
      diagnostics: [],
    });
    await Promise.resolve();

    expect(store.getState().playing).toBe(false);
    expect(store.getState().lastGoodPreview?.frame).toBe(1);
  });

  it("flushes a dirty document before switching and clears a recovered preview error", async () => {
    const other = {
      ...starter,
      summary: { ...starter.summary, id: "f1148cf4-789b-4eea-9fb8-7d277914d4d7", title: "Other" },
    };
    const preview = vi
      .fn()
      .mockRejectedValueOnce({ message: "renderer busy", diagnostics: [] })
      .mockImplementation(async (request: MotionPreviewRequest) => ({
        revisionHash: request.revisionHash,
        frame: request.frame,
        pngDataUrl: "data:image/png;base64,recovered",
        diagnostics: [],
      }));
    const motionBackend = backend({
      list: vi.fn(async () => [starter.summary, other.summary]),
      read: vi.fn(async (id) => id === other.summary.id ? other : starter),
      preview,
    });
    const store = createMotionStudioStore(motionBackend);
    await store.getState().load();
    await vi.waitFor(() => expect(store.getState().previewPhase).toBe("error"));
    expect(store.getState().previewError).toContain("renderer busy");

    store.getState().updateSource("<main>saved before switch</main>\n");
    const selecting = store.getState().selectDocument(other.summary.id);
    await vi.waitFor(() => expect(motionBackend.patch).toHaveBeenCalledOnce());
    await selecting;
    expect(store.getState().document?.summary.id).toBe(other.summary.id);
    expect(store.getState().error).toBeNull();
    await vi.waitFor(() => expect(store.getState().lastGoodPreview?.pngDataUrl).toContain("recovered"));
  });

  it("cancels a late document switch when the editor changes during its read", async () => {
    const readOther = deferred<MotionDocument>();
    const other = {
      ...starter,
      summary: { ...starter.summary, id: "f1148cf4-789b-4eea-9fb8-7d277914d4d7", title: "Other" },
    };
    const motionBackend = backend({
      list: vi.fn(async () => [starter.summary, other.summary]),
      read: vi
        .fn()
        .mockResolvedValueOnce(starter)
        .mockImplementationOnce(() => readOther.promise),
    });
    const store = createMotionStudioStore(motionBackend);
    await store.getState().load();

    const selecting = store.getState().selectDocument(other.summary.id);
    store.getState().updateSource("<main>typed during switch</main>\n");
    readOther.resolve(other);
    await selecting;

    expect(store.getState().document?.summary.id).toBe(starter.summary.id);
    expect(store.getState().html).toBe("<main>typed during switch</main>\n");
    expect(store.getState().dirtyFiles["index.html"]).toBe(true);
  });

  it("cancels native preview work and flushes dirty source before disposal", async () => {
    const saved = deferred<MotionDocument>();
    const motionBackend = backend({ patch: vi.fn(() => saved.promise) });
    const store = createMotionStudioStore(motionBackend);
    await store.getState().load();
    store.getState().updateSource("<main>saved on dispose</main>\n");

    const disposing = store.getState().dispose();
    await vi.waitFor(() => expect(motionBackend.patch).toHaveBeenCalledOnce());
    expect(motionBackend.cancelPreview).toHaveBeenCalled();
    saved.resolve({
      ...starter,
      summary: { ...starter.summary, revisionHash: "e".repeat(64), updatedAt: 2 },
      html: "<main>saved on dispose</main>\n",
    });
    await disposing;

    expect(store.getState()).toMatchObject({ phase: "ready", playing: false, savingFile: null });
    expect(store.getState().dirtyFiles["index.html"]).toBe(false);
  });

  it("re-activates after an immediate remount waits for disposal autosave", async () => {
    const saved = deferred<MotionDocument>();
    const motionBackend = backend({ patch: vi.fn(() => saved.promise) });
    const store = createMotionStudioStore(motionBackend);
    await store.getState().load();
    await vi.waitFor(() => expect(store.getState().previewPhase).toBe("ready"));
    store.getState().updateSource("<main>saved while remounting</main>\n");

    const disposing = store.getState().dispose();
    await vi.waitFor(() => expect(motionBackend.patch).toHaveBeenCalledOnce());
    const resuming = store.getState().resume();
    saved.resolve({
      ...starter,
      summary: { ...starter.summary, revisionHash: "e".repeat(64), updatedAt: 2 },
      html: "<main>saved while remounting</main>\n",
    });
    await disposing;
    await resuming;

    await vi.waitFor(() => expect(motionBackend.preview).toHaveBeenCalledTimes(2));
    expect(store.getState()).toMatchObject({ phase: "ready", playing: false });
  });

  it("restarts an initial load that was invalidated by unmount and immediate remount", async () => {
    const firstList = deferred<MotionDocument["summary"][]>();
    const motionBackend = backend({
      list: vi.fn().mockImplementationOnce(() => firstList.promise).mockResolvedValueOnce([starter.summary]),
    });
    const store = createMotionStudioStore(motionBackend);

    const loading = store.getState().load();
    await store.getState().dispose();
    const resuming = store.getState().resume();
    firstList.resolve([starter.summary]);
    await loading;
    await resuming;

    await vi.waitFor(() => expect(store.getState().phase).toBe("ready"));
    expect(motionBackend.list).toHaveBeenCalledTimes(2);
    expect(store.getState().document?.summary.id).toBe(starter.summary.id);
  });

  it("keeps the last hidden intent during suspend-resume-suspend cancellation races", async () => {
    const firstCancel = deferred<boolean>();
    const motionBackend = backend();
    vi.mocked(motionBackend.cancelPreview)
      .mockImplementationOnce(() => firstCancel.promise)
      .mockResolvedValue(true);
    const store = createMotionStudioStore(motionBackend);
    await store.getState().load();
    await vi.waitFor(() => expect(store.getState().previewPhase).toBe("ready"));
    expect(motionBackend.preview).toHaveBeenCalledOnce();

    const firstSuspend = store.getState().suspend();
    const resume = store.getState().resume();
    const finalSuspend = store.getState().suspend();
    firstCancel.resolve(true);
    await Promise.all([firstSuspend, resume, finalSuspend]);

    expect(motionBackend.preview).toHaveBeenCalledOnce();
    store.getState().play();
    await vi.advanceTimersByTimeAsync(1_000);
    expect(store.getState().playing).toBe(false);
  });

  it("publishes only the saved previewed revision and exposes progress and cancellation", async () => {
    let progress: ((update: { phase: "rendering"; doneFrames: number; totalFrames: number }) => void) | undefined;
    const completion = deferred<Awaited<ReturnType<MotionStudioBackend["publish"]>>>();
    const committed = await backend().publish({} as never);
    const motionBackend = backend({
      publish: vi.fn(() => completion.promise),
      onProgress: vi.fn(async (handler) => {
        progress = handler as typeof progress;
        return () => {};
      }),
    });
    const store = createMotionStudioStore(motionBackend);
    await store.getState().load();
    await vi.waitFor(() => expect(store.getState().previewPhase).toBe("ready"));

    const publishing = store.getState().publish();
    await vi.waitFor(() => expect(progress).toBeDefined());
    progress?.({ phase: "rendering", doneFrames: 27, totalFrames: 90 });
    expect(store.getState().publishFrameProgress).toEqual({ done: 27, total: 90 });
    completion.resolve(committed);
    await publishing;

    expect(motionBackend.publish).toHaveBeenCalledWith({
      documentId: starter.summary.id,
      revisionHash: starter.summary.revisionHash,
      startFrame: 0,
      durationFrames: 90,
      width: 1920,
      height: 1080,
      fps: 30,
      trackIndex: undefined,
    });
    expect(store.getState()).toMatchObject({
      publishPhase: "complete",
      publishError: null,
      publishCommit: { clipId: "motion-clip" },
      publishFrameProgress: { done: 90, total: 90 },
    });

    await store.getState().cancelPublish();
    expect(motionBackend.cancelPublish).toHaveBeenCalledOnce();
  });

  it("refuses to publish dirty, conflicting, or preview-invalid source", async () => {
    const motionBackend = backend();
    const store = createMotionStudioStore(motionBackend);
    await store.getState().load();
    await vi.waitFor(() => expect(store.getState().previewPhase).toBe("ready"));
    store.getState().updateSource("<main>not saved yet</main>\n");
    store.setState({ conflict: { file: "index.html", localSource: "local" } });

    await store.getState().publish();

    expect(motionBackend.publish).not.toHaveBeenCalled();
    expect(store.getState().publishError).toMatch(/saved|conflict|preview/i);
  });

  it("does not discard a commit that wins the race with cancellation", async () => {
    const committed = deferred<Awaited<ReturnType<MotionStudioBackend["publish"]>>>();
    const motionBackend = backend({ publish: vi.fn(() => committed.promise) });
    const store = createMotionStudioStore(motionBackend);
    await store.getState().load();
    await vi.waitFor(() => expect(store.getState().previewPhase).toBe("ready"));

    const publishing = store.getState().publish();
    await vi.waitFor(() => expect(motionBackend.publish).toHaveBeenCalledOnce());
    const cancelling = store.getState().cancelPublish();
    committed.resolve({
      clipId: "late-commit",
      assetId: "asset",
      contentHash: "f".repeat(64),
      actionName: "Add Motion Graphic",
      sourceDocument: {
        documentId: starter.summary.id,
        revisionHash: starter.summary.revisionHash,
      },
      output: {
        renderer: "opentake-motion-studio",
        rendererVersion: "1",
        outputFile: "output.mp4",
        fps: 30,
        width: 1920,
        height: 1080,
        durationFrames: 90,
        durationSeconds: 3,
        contentHash: "f".repeat(64),
      },
    });
    await Promise.all([publishing, cancelling]);

    expect(store.getState()).toMatchObject({
      publishPhase: "complete",
      publishCommit: { clipId: "late-commit" },
    });
  });
});
