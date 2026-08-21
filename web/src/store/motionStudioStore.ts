import { create, type StoreApi, type UseBoundStore } from "zustand";
import {
  motionDocumentCreate,
  motionDocumentHash,
  motionDocumentList,
  motionDocumentPatch,
  motionDocumentRead,
  addMotion,
  cancelMotion,
  onMotionProgress,
  motionPreview,
  motionPreviewCancel,
  type MotionDocumentChangedEvent,
} from "../lib/api";
import { useProjectStore } from "./projectStore";
import type {
  MotionDocument,
  MotionDocumentCreateRequest,
  MotionDocumentFile,
  MotionDocumentHashRequest,
  MotionDocumentPatchRequest,
  MotionDocumentSummary,
  MotionPreviewDiagnostic,
  MotionPreviewRequest,
  MotionPreviewResponse,
  MotionPublishParameters,
} from "../lib/types";
import type {
  MotionAddRequest,
  MotionCommit,
  MotionProgressPhase,
  MotionProgressUpdate,
} from "../lib/api";

export const MOTION_SAVE_DEBOUNCE_MS = 300;
const DEFAULT_PARAMETERS: MotionPublishParameters = {
  width: 1920,
  height: 1080,
  fps: 30,
  durationFrames: 90,
};
const PARAMETER_BOUNDS = {
  width: [2, 4096],
  height: [2, 4096],
  fps: [1, 240],
  durationFrames: [1, 3600],
} as const;

export interface MotionStudioBackend {
  list: () => Promise<MotionDocumentSummary[]>;
  create: (request: MotionDocumentCreateRequest) => Promise<MotionDocument>;
  read: (documentId: string) => Promise<MotionDocument>;
  hash: (request: MotionDocumentHashRequest) => Promise<string>;
  patch: (request: MotionDocumentPatchRequest) => Promise<MotionDocument>;
  preview: (request: MotionPreviewRequest) => Promise<MotionPreviewResponse>;
  cancelPreview: () => Promise<boolean>;
  publish: (request: MotionAddRequest) => Promise<MotionCommit>;
  cancelPublish: () => Promise<boolean>;
  onProgress: (handler: (update: MotionProgressUpdate) => void) => Promise<() => void>;
}

const nativeBackend: MotionStudioBackend = {
  list: motionDocumentList,
  create: motionDocumentCreate,
  read: motionDocumentRead,
  hash: motionDocumentHash,
  patch: motionDocumentPatch,
  preview: motionPreview,
  cancelPreview: motionPreviewCancel,
  publish: (request) => addMotion(request),
  cancelPublish: () => cancelMotion(),
  onProgress: (handler) => onMotionProgress(handler),
};

export interface MotionConflict {
  file: MotionDocumentFile;
  localSource: string;
}

export type MotionStudioPhase = "idle" | "loading" | "ready" | "error";
export type MotionPreviewPhase = "idle" | "loading" | "ready" | "error";
export type MotionPublishPhase = "idle" | MotionProgressPhase | "error";

export interface MotionStudioState {
  phase: MotionStudioPhase;
  error: string | null;
  errorFile: MotionDocumentFile | null;
  documents: MotionDocumentSummary[];
  document: MotionDocument | null;
  activeFile: MotionDocumentFile;
  html: string;
  css: string;
  dirtyFiles: Record<MotionDocumentFile, boolean>;
  savingFile: MotionDocumentFile | null;
  conflict: MotionConflict | null;
  parameters: MotionPublishParameters;
  transparent: boolean;
  frame: number;
  playing: boolean;
  previewPhase: MotionPreviewPhase;
  previewError: string | null;
  lastGoodPreview: MotionPreviewResponse | null;
  diagnostics: MotionPreviewDiagnostic[];
  diagnosticFile: MotionDocumentFile | null;
  publishPhase: MotionPublishPhase;
  publishFrameProgress: { done: number; total: number } | null;
  publishError: string | null;
  publishCommit: MotionCommit | null;
  load: () => Promise<void>;
  selectDocument: (documentId: string) => Promise<void>;
  refreshExternalDocument: (change: MotionDocumentChangedEvent) => Promise<void>;
  setActiveFile: (file: MotionDocumentFile) => void;
  updateSource: (source: string) => void;
  flushSave: () => Promise<void>;
  reloadConflict: () => Promise<void>;
  reapplyConflict: () => Promise<void>;
  requestPreview: (sourceFile?: MotionDocumentFile) => Promise<void>;
  setFrame: (frame: number) => void;
  setParameter: (name: keyof MotionPublishParameters, value: number) => void;
  setTransparent: (value: boolean) => void;
  play: () => void;
  pause: () => void;
  suspend: () => Promise<void>;
  resume: () => Promise<void>;
  replay: () => void;
  publish: () => Promise<void>;
  cancelPublish: () => Promise<void>;
  resetProject: () => void;
  dispose: () => Promise<void>;
}

export type MotionStudioStore = UseBoundStore<StoreApi<MotionStudioState>>;

function normalizeSource(source: string): string {
  return source.replace(/\r\n?/g, "\n");
}

function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "object" && error !== null && "message" in error && typeof error.message === "string") {
    return error.message;
  }
  return String(error);
}

function errorDiagnostics(error: unknown): MotionPreviewDiagnostic[] {
  if (typeof error !== "object" || error === null || !("diagnostics" in error)) return [];
  if (!Array.isArray(error.diagnostics)) return [];
  return error.diagnostics.flatMap((diagnostic) => {
    if (
      typeof diagnostic !== "object" ||
      diagnostic === null ||
      !("message" in diagnostic) ||
      typeof diagnostic.message !== "string"
    ) {
      return [];
    }
    const severity = "severity" in diagnostic && diagnostic.severity === "warning" ? "warning" : "error";
    const line = "line" in diagnostic && Number.isSafeInteger(diagnostic.line) && Number(diagnostic.line) > 0
      ? Number(diagnostic.line)
      : undefined;
    const column = "column" in diagnostic && Number.isSafeInteger(diagnostic.column) && Number(diagnostic.column) > 0
      ? Number(diagnostic.column)
      : undefined;
    return [{ severity, message: diagnostic.message.slice(0, 2048), line, column }];
  });
}

function clampInteger(value: number, min: number, max: number): number {
  if (!Number.isFinite(value)) return min;
  return Math.min(max, Math.max(min, Math.round(value)));
}

function sourceFor(document: MotionDocument, file: MotionDocumentFile): string {
  return file === "index.html" ? document.html : document.css;
}

export function createMotionStudioStore(
  backend: MotionStudioBackend = nativeBackend,
): MotionStudioStore {
  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  let playbackTimer: ReturnType<typeof setInterval> | null = null;
  let loadGeneration = 0;
  let previewGeneration = 0;
  let saveOperation = 0;
  let conflictOperation = 0;
  let saveCompletion: Promise<void> | null = null;
  let disposing = false;
  let disposal: Promise<void> | null = null;
  let suspended = false;
  let visibilityOperation = 0;
  let previewCancellation: Promise<void> | null = null;
  let publishOperation = 0;
  let publishCompletion: Promise<MotionCommit> | null = null;
  let publishCancellationRequested = false;
  let externalRefreshOperation = 0;
  let pendingExternalChange: MotionDocumentChangedEvent | null = null;
  const sourceVersions: Record<MotionDocumentFile, number> = {
    "index.html": 0,
    "styles.css": 0,
  };

  const clearSaveTimer = () => {
    if (saveTimer !== null) clearTimeout(saveTimer);
    saveTimer = null;
  };
  const clearPlaybackTimer = () => {
    if (playbackTimer !== null) clearInterval(playbackTimer);
    playbackTimer = null;
  };
  const cancelPreviews = async () => {
    if (previewCancellation) await previewCancellation;
    const current = backend.cancelPreview().then(() => undefined, () => undefined);
    previewCancellation = current;
    await current;
    if (previewCancellation === current) previewCancellation = null;
  };

  const store = create<MotionStudioState>((set, get) => {
    const replayPendingExternalChange = () => {
      const pending = pendingExternalChange;
      pendingExternalChange = null;
      if (pending) void get().refreshExternalDocument(pending);
    };

    const scheduleSave = (delay = MOTION_SAVE_DEBOUNCE_MS) => {
      clearSaveTimer();
      saveTimer = setTimeout(() => {
        saveTimer = null;
        void get().flushSave();
      }, delay);
    };

    const installDocument = (document: MotionDocument, documents: MotionDocumentSummary[]) => {
      sourceVersions["index.html"] += 1;
      sourceVersions["styles.css"] += 1;
      set({
        phase: "ready",
        error: null,
        errorFile: null,
        documents,
        document,
        html: document.html,
        css: document.css,
        dirtyFiles: { "index.html": false, "styles.css": false },
        savingFile: null,
        conflict: null,
        diagnostics: [],
        diagnosticFile: null,
        previewError: null,
        previewPhase: "idle",
        lastGoodPreview: null,
        publishPhase: "idle",
        publishFrameProgress: null,
        publishError: null,
        publishCommit: null,
      });
      void get().requestPreview();
    };

    const beginPlayback = () => {
      clearPlaybackTimer();
      const fps = get().parameters.fps;
      playbackTimer = setInterval(() => {
        const current = get();
        if (!current.playing) return;
        if (current.previewPhase === "loading") return;
        const lastFrame = current.parameters.durationFrames - 1;
        const next = Math.min(lastFrame, current.frame + 1);
        set({ frame: next, playing: next < lastFrame });
        void get().requestPreview();
        if (next >= lastFrame) clearPlaybackTimer();
      }, 1000 / fps);
    };

    return {
      phase: "idle",
      error: null,
      errorFile: null,
      documents: [],
      document: null,
      activeFile: "index.html",
      html: "",
      css: "",
      dirtyFiles: { "index.html": false, "styles.css": false },
      savingFile: null,
      conflict: null,
      parameters: { ...DEFAULT_PARAMETERS },
      transparent: false,
      frame: 0,
      playing: false,
      previewPhase: "idle",
      previewError: null,
      lastGoodPreview: null,
      diagnostics: [],
      diagnosticFile: null,
      publishPhase: "idle",
      publishFrameProgress: null,
      publishError: null,
      publishCommit: null,

      load: async () => {
        const generation = ++loadGeneration;
        previewGeneration += 1;
        saveOperation += 1;
        clearSaveTimer();
        clearPlaybackTimer();
        set({ phase: "loading", error: null, errorFile: null, playing: false });
        if (previewCancellation) await previewCancellation;
        if (generation !== loadGeneration) return;
        disposing = false;
        suspended = false;
        try {
          const summaries = await backend.list();
          if (generation !== loadGeneration) return;
          const document = summaries.length > 0
            ? await backend.read(summaries[0]!.id)
            : await backend.create({ title: null });
          if (generation !== loadGeneration) return;
          const documents = summaries.length > 0 ? summaries : [document.summary];
          installDocument(document, documents);
        } catch (error) {
          if (generation !== loadGeneration) return;
          set({ phase: "error", error: errorMessage(error), errorFile: null });
        }
      },

      selectDocument: async (documentId) => {
        if (get().document?.summary.id === documentId) return;
        if (["validating", "rendering", "encoding", "committing"].includes(get().publishPhase)) {
          set({ error: "请等待 Motion Studio 发布完成后再切换文档 / Wait for publishing to finish before switching documents." });
          return;
        }
        clearSaveTimer();
        await get().flushSave();
        const pending = get();
        if (pending.dirtyFiles["index.html"] || pending.dirtyFiles["styles.css"] || pending.conflict) {
          set({ error: pending.error ?? "Save the current Motion Studio document before switching." });
          return;
        }
        const generation = ++loadGeneration;
        const sourceDocumentId = pending.document?.summary.id;
        const htmlVersion = sourceVersions["index.html"];
        const cssVersion = sourceVersions["styles.css"];
        previewGeneration += 1;
        saveOperation += 1;
        clearSaveTimer();
        set({ phase: "loading", error: null, errorFile: null });
        try {
          const document = await backend.read(documentId);
          if (generation !== loadGeneration) return;
          if (
            get().document?.summary.id !== sourceDocumentId ||
            sourceVersions["index.html"] !== htmlVersion ||
            sourceVersions["styles.css"] !== cssVersion
          ) {
            set({
              phase: "ready",
              error: "The editor changed while switching documents. Try again after it saves.",
            });
            return;
          }
          installDocument(document, get().documents);
        } catch (error) {
          if (generation === loadGeneration) {
            set({ phase: "error", error: errorMessage(error), errorFile: null });
          }
        }
      },

      refreshExternalDocument: async (change) => {
        const projectAtAdmission = useProjectStore.getState();
        if (
          projectAtAdmission.projectEpoch !== change.projectEpoch ||
          projectAtAdmission.projectPath !== change.projectPath
        ) return;
        const { summary } = change;
        set((state) => {
          const present = state.documents.some((item) => item.id === summary.id);
          return {
            documents: present
              ? state.documents.map((item) => item.id === summary.id ? summary : item)
              : [summary, ...state.documents],
          };
        });
        const current = get();
        if (
          current.document?.summary.id !== summary.id ||
          current.document.summary.revisionHash === summary.revisionHash
        ) return;
        if (
          current.savingFile ||
          current.conflict ||
          current.dirtyFiles["index.html"] ||
          current.dirtyFiles["styles.css"]
        ) {
          return;
        }
        if (["validating", "rendering", "encoding", "committing"].includes(current.publishPhase)) {
          pendingExternalChange = change;
          return;
        }
        const operation = ++externalRefreshOperation;
        const generation = loadGeneration;
        const expectedRevision = current.document.summary.revisionHash;
        const htmlVersion = sourceVersions["index.html"];
        const cssVersion = sourceVersions["styles.css"];
        try {
          const remote = await backend.read(summary.id);
          const latest = get();
          const currentProject = useProjectStore.getState();
          if (
            operation !== externalRefreshOperation ||
            generation !== loadGeneration ||
            currentProject.projectEpoch !== change.projectEpoch ||
            currentProject.projectPath !== change.projectPath ||
            latest.document?.summary.id !== summary.id ||
            latest.document.summary.revisionHash !== expectedRevision ||
            latest.savingFile ||
            latest.conflict ||
            latest.dirtyFiles["index.html"] ||
            latest.dirtyFiles["styles.css"] ||
            sourceVersions["index.html"] !== htmlVersion ||
            sourceVersions["styles.css"] !== cssVersion
          ) return;
          installDocument(remote, latest.documents.map((item) =>
            item.id === remote.summary.id ? remote.summary : item,
          ));
        } catch (error) {
          const currentProject = useProjectStore.getState();
          if (
            operation === externalRefreshOperation &&
            generation === loadGeneration &&
            currentProject.projectEpoch === change.projectEpoch &&
            currentProject.projectPath === change.projectPath
          ) {
            set({ error: errorMessage(error), errorFile: null });
          }
        }
      },

      setActiveFile: (activeFile) => set({ activeFile }),

      updateSource: (source) => {
        const file = get().activeFile;
        const normalized = normalizeSource(source);
        sourceVersions[file] += 1;
        set((state) => ({
          ...(file === "index.html" ? { html: normalized } : { css: normalized }),
          dirtyFiles: { ...state.dirtyFiles, [file]: true },
          conflict: null,
          error: null,
          errorFile: null,
          previewError: null,
          diagnostics: [],
          diagnosticFile: null,
        }));
        scheduleSave();
      },

      flushSave: async () => {
        clearSaveTimer();
        if (saveCompletion) {
          await saveCompletion;
          const pending = get();
          if (
            !pending.conflict &&
            (pending.dirtyFiles["index.html"] || pending.dirtyFiles["styles.css"])
          ) {
            await get().flushSave();
          }
          return;
        }
        const state = get();
        if (state.savingFile || !state.document || state.conflict) return;
        const file: MotionDocumentFile | undefined = state.dirtyFiles["index.html"]
          ? "index.html"
          : state.dirtyFiles["styles.css"]
            ? "styles.css"
            : undefined;
        if (!file) return;

        let finishSave!: () => void;
        const completion = new Promise<void>((resolve) => {
          finishSave = resolve;
        });
        saveCompletion = completion;

        try {
          const operation = ++saveOperation;
          const generation = loadGeneration;
          const document = state.document;
          const localSource = file === "index.html" ? state.html : state.css;
          const version = sourceVersions[file];
          const hashRequest: MotionDocumentHashRequest = {
            documentId: document.summary.id,
            file,
            baselineHash: document.summary.revisionHash,
            edits: [
              {
                start: 0,
                end: new TextEncoder().encode(sourceFor(document, file)).length,
                replacement: localSource,
              },
            ],
          };
          set({ savingFile: file, error: null, errorFile: null });

          try {
            const expectedResultHash = await backend.hash(hashRequest);
            if (operation !== saveOperation || generation !== loadGeneration) return;
            const result = await backend.patch({
              ...hashRequest,
              expectedResultHash,
            });
            if (operation !== saveOperation || generation !== loadGeneration) return;
            const unchanged = sourceVersions[file] === version &&
              (file === "index.html" ? get().html : get().css) === localSource;
            set((latest) => ({
              document: result,
              documents: latest.documents.map((summary) =>
                summary.id === result.summary.id ? result.summary : summary,
              ),
              ...(file === "index.html" && unchanged ? { html: result.html } : {}),
              ...(file === "styles.css" && unchanged ? { css: result.css } : {}),
              dirtyFiles: { ...latest.dirtyFiles, [file]: !unchanged },
              savingFile: null,
              conflict: null,
            }));
            void get().requestPreview(file);
            if (get().dirtyFiles["index.html"] || get().dirtyFiles["styles.css"]) scheduleSave(0);
          } catch (error) {
            if (operation !== saveOperation || generation !== loadGeneration) return;
            const message = errorMessage(error);
            set({
              savingFile: null,
              error: message,
              errorFile: file,
              conflict: /revision conflict/i.test(message) ? { file, localSource } : null,
            });
          }
        } finally {
          finishSave();
          if (saveCompletion === completion) saveCompletion = null;
        }
      },

      reloadConflict: async () => {
        const current = get();
        if (!current.document || !current.conflict) return;
        const operation = ++conflictOperation;
        const documentId = current.document.summary.id;
        const conflict = current.conflict;
        const htmlVersion = sourceVersions["index.html"];
        const cssVersion = sourceVersions["styles.css"];
        saveOperation += 1;
        clearSaveTimer();
        try {
          const remote = await backend.read(documentId);
          const latest = get();
          if (
            operation !== conflictOperation ||
            latest.document?.summary.id !== documentId ||
            latest.conflict?.file !== conflict.file ||
            latest.conflict.localSource !== conflict.localSource ||
            sourceVersions["index.html"] !== htmlVersion ||
            sourceVersions["styles.css"] !== cssVersion
          ) return;
          installDocument(remote, latest.documents.map((summary) =>
            summary.id === remote.summary.id ? remote.summary : summary,
          ));
        } catch (error) {
          if (operation === conflictOperation) {
            set({ error: errorMessage(error), errorFile: conflict.file });
          }
        }
      },

      reapplyConflict: async () => {
        const current = get();
        if (!current.document || !current.conflict) return;
        const conflict = current.conflict;
        const operation = ++conflictOperation;
        const documentId = current.document.summary.id;
        const htmlVersion = sourceVersions["index.html"];
        const cssVersion = sourceVersions["styles.css"];
        const local = {
          html: current.html,
          css: current.css,
          dirtyFiles: { ...current.dirtyFiles },
        };
        saveOperation += 1;
        clearSaveTimer();
        try {
          const remote = await backend.read(documentId);
          const latest = get();
          if (
            operation !== conflictOperation ||
            latest.document?.summary.id !== documentId ||
            latest.conflict?.file !== conflict.file ||
            latest.conflict.localSource !== conflict.localSource ||
            sourceVersions["index.html"] !== htmlVersion ||
            sourceVersions["styles.css"] !== cssVersion
          ) return;
          if (!local.dirtyFiles["index.html"]) sourceVersions["index.html"] += 1;
          if (!local.dirtyFiles["styles.css"]) sourceVersions["styles.css"] += 1;
          set({
            document: remote,
            documents: latest.documents.map((summary) =>
              summary.id === remote.summary.id ? remote.summary : summary,
            ),
            html: local.dirtyFiles["index.html"] ? local.html : remote.html,
            css: local.dirtyFiles["styles.css"] ? local.css : remote.css,
            dirtyFiles: local.dirtyFiles,
            conflict: null,
            error: null,
            errorFile: null,
          });
          scheduleSave(0);
        } catch (error) {
          if (operation === conflictOperation) {
            set({ error: errorMessage(error), errorFile: conflict.file });
          }
        }
      },

      requestPreview: async (sourceFile) => {
        if (disposing || suspended) return;
        const state = get();
        if (!state.document) return;
        const generation = ++previewGeneration;
        const request: MotionPreviewRequest = {
          documentId: state.document.summary.id,
          revisionHash: state.document.summary.revisionHash,
          ...state.parameters,
          frame: state.frame,
        };
        const diagnosticFile = sourceFile ?? state.activeFile;
        set({ previewPhase: "loading", previewError: null, diagnostics: [], diagnosticFile });
        try {
          const response = await backend.preview(request);
          const latest = get();
          if (
            generation !== previewGeneration ||
            latest.document?.summary.revisionHash !== response.revisionHash ||
            latest.frame !== response.frame
          ) {
            return;
          }
          set({
            previewPhase: "ready",
            previewError: null,
            lastGoodPreview: response,
            diagnostics: response.diagnostics,
            diagnosticFile,
          });
        } catch (error) {
          if (generation !== previewGeneration) return;
          set({
            previewPhase: "error",
            previewError: errorMessage(error),
            diagnostics: errorDiagnostics(error),
            diagnosticFile,
          });
        }
      },

      setFrame: (value) => {
        const frame = clampInteger(value, 0, get().parameters.durationFrames - 1);
        set({ frame });
        void get().requestPreview();
      },

      setParameter: (name, value) => {
        const [min, max] = PARAMETER_BOUNDS[name];
        let next = clampInteger(value, min, max);
        if ((name === "width" || name === "height") && next % 2 !== 0) {
          next = Math.min(max, next + 1);
        }
        set((state) => {
          const parameters = { ...state.parameters, [name]: next };
          return {
            parameters,
            frame: Math.min(state.frame, parameters.durationFrames - 1),
          };
        });
        if (get().playing && name === "fps") beginPlayback();
        void get().requestPreview();
      },

      setTransparent: (value) => set({ transparent: value }),

      play: () => {
        if (suspended || disposing) return;
        const state = get();
        if (state.parameters.durationFrames <= 1) return;
        if (state.frame >= state.parameters.durationFrames - 1) set({ frame: 0 });
        set({ playing: true });
        beginPlayback();
        const current = get();
        const hasCurrentFrame = current.lastGoodPreview?.frame === current.frame &&
          current.lastGoodPreview.revisionHash === current.document?.summary.revisionHash;
        if (current.previewPhase !== "loading" && !hasCurrentFrame) void current.requestPreview();
      },

      pause: () => {
        clearPlaybackTimer();
        set({ playing: false });
      },

      suspend: async () => {
        const operation = ++visibilityOperation;
        suspended = true;
        clearPlaybackTimer();
        clearSaveTimer();
        previewGeneration += 1;
        set((state) => ({
          playing: false,
          previewPhase: state.lastGoodPreview?.frame === state.frame ? "ready" : "idle",
          previewError: null,
          lastGoodPreview: state.lastGoodPreview?.frame === state.frame
            ? state.lastGoodPreview
            : null,
        }));
        await Promise.all([cancelPreviews(), get().flushSave()]);
        if (operation !== visibilityOperation) return;
      },

      resume: async () => {
        const operation = ++visibilityOperation;
        while (disposal) await disposal;
        if (operation !== visibilityOperation) return;
        while (saveCompletion) await saveCompletion;
        if (operation !== visibilityOperation) return;
        while (previewCancellation) await previewCancellation;
        if (operation !== visibilityOperation) return;
        disposing = false;
        suspended = false;
        if (get().phase === "idle") {
          await get().load();
        } else {
          await get().requestPreview();
        }
      },

      replay: () => {
        if (suspended || disposing) return;
        clearPlaybackTimer();
        set({ frame: 0, playing: true });
        beginPlayback();
        void get().requestPreview();
      },

      publish: async () => {
        const operation = ++publishOperation;
        if (
          !get().document ||
          ["validating", "rendering", "encoding", "committing"].includes(get().publishPhase)
        ) {
          return;
        }
        await get().flushSave();
        if (operation !== publishOperation) return;
        const state = get();
        const document = state.document;
        const previewMatches = Boolean(
          document &&
          state.previewPhase === "ready" &&
          state.lastGoodPreview?.revisionHash === document.summary.revisionHash &&
          !state.diagnostics.some((diagnostic) => diagnostic.severity === "error"),
        );
        if (
          !document ||
          state.savingFile ||
          state.conflict ||
          state.dirtyFiles["index.html"] ||
          state.dirtyFiles["styles.css"] ||
          !previewMatches
        ) {
          set({
            publishPhase: "error",
            publishFrameProgress: null,
            publishError: "请先保存文档并解决版本冲突或预览错误 / Save the document and resolve conflicts or preview errors before publishing.",
            publishCommit: null,
          });
          return;
        }
        const documentId = document.summary.id;
        const revisionHash = document.summary.revisionHash;
        publishCancellationRequested = false;
        set({
          publishPhase: "validating",
          publishFrameProgress: null,
          publishError: null,
          publishCommit: null,
        });
        let unlisten: (() => void) | null = null;
        let progressOpen = true;
        try {
          unlisten = await backend
            .onProgress((update) => {
              if (!progressOpen || operation !== publishOperation) return;
              set({
                publishPhase: update.phase,
                ...(update.phase === "rendering" &&
                update.doneFrames !== undefined &&
                update.totalFrames !== undefined
                  ? { publishFrameProgress: { done: update.doneFrames, total: update.totalFrames } }
                  : {}),
              });
            })
            .catch(() => null);
          if (operation !== publishOperation) return;
          const completion = backend.publish({
            documentId,
            revisionHash,
            ...state.parameters,
            transparent: state.transparent,
            startFrame: 0,
            trackIndex: undefined,
          });
          publishCompletion = completion;
          const commit = await completion;
          if (operation !== publishOperation) return;
          progressOpen = false;
          set({
            publishPhase: "complete",
            publishFrameProgress: {
              done: commit.output.durationFrames,
              total: commit.output.durationFrames,
            },
            publishError: null,
            publishCommit: commit,
          });
        } catch (error) {
          progressOpen = false;
          if (operation === publishOperation) {
            set(publishCancellationRequested
              ? {
                  publishPhase: "idle",
                  publishFrameProgress: null,
                  publishError: null,
                  publishCommit: null,
                }
              : {
                  publishPhase: "error",
                  publishFrameProgress: null,
                  publishError: errorMessage(error),
                  publishCommit: null,
                });
          }
        } finally {
          progressOpen = false;
          publishCompletion = null;
          publishCancellationRequested = false;
          unlisten?.();
          replayPendingExternalChange();
        }
      },

      cancelPublish: async () => {
        const active = publishCompletion;
        publishCancellationRequested = true;
        if (!active) publishOperation += 1;
        await backend.cancelPublish().catch(() => false);
        if (active) {
          await active.catch(() => undefined);
        } else {
          set({
            publishPhase: "idle",
            publishFrameProgress: null,
            publishError: null,
            publishCommit: null,
          });
        }
      },

      resetProject: () => {
        clearSaveTimer();
        clearPlaybackTimer();
        loadGeneration += 1;
        previewGeneration += 1;
        saveOperation += 1;
        conflictOperation += 1;
        publishOperation += 1;
        externalRefreshOperation += 1;
        pendingExternalChange = null;
        publishCancellationRequested = true;
        void backend.cancelPublish().catch(() => false);
        visibilityOperation += 1;
        disposing = false;
        suspended = true;
        void cancelPreviews();
        set({
          phase: "idle",
          error: null,
          errorFile: null,
          documents: [],
          document: null,
          html: "",
          css: "",
          dirtyFiles: { "index.html": false, "styles.css": false },
          savingFile: null,
          conflict: null,
          transparent: false,
          playing: false,
          previewPhase: "idle",
          previewError: null,
          lastGoodPreview: null,
          diagnostics: [],
          diagnosticFile: null,
          publishPhase: "idle",
          publishFrameProgress: null,
          publishError: null,
          publishCommit: null,
        });
      },

      dispose: () => {
        if (disposal) return disposal;
        let current!: Promise<void>;
        current = (async () => {
          visibilityOperation += 1;
          disposing = true;
          suspended = true;
          clearSaveTimer();
          clearPlaybackTimer();
          previewGeneration += 1;
          set({ playing: false, previewPhase: "idle", previewError: null });
          try {
            await cancelPreviews();
            await get().flushSave();
            loadGeneration += 1;
            previewGeneration += 1;
            saveOperation += 1;
            conflictOperation += 1;
            await cancelPreviews();
            set((state) => ({
              savingFile: null,
              ...(state.document ? {} : { phase: "idle" as const }),
            }));
          } finally {
            if (disposal === current) disposal = null;
          }
        })();
        disposal = current;
        return current;
      },
    };
  });

  return store;
}

export const useMotionStudioStore = createMotionStudioStore();
