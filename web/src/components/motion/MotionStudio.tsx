import { useEffect, useRef } from "react";
import { Clock3, FileCode2, Layers3, Sparkles } from "lucide-react";
import { useT } from "../../i18n";
import type { MotionDocumentFile, MotionPublishParameters } from "../../lib/types";
import {
  useMotionStudioStore,
  type MotionStudioStore,
} from "../../store/motionStudioStore";
import { useEditorUiStore } from "../../store/uiStore";
import { useProjectStore } from "../../store/projectStore";
import { forceRefresh } from "../../store/sync";
import { onMotionDocumentChanged } from "../../lib/api";
import { Icon } from "../ui/Icon";
import { MotionCodeEditor } from "./MotionCodeEditor";
import { MotionPreview } from "./MotionPreview";
import { MotionTimeline } from "./MotionTimeline";

const FILES: MotionDocumentFile[] = ["index.html", "styles.css"];

export function MotionStudio({ store = useMotionStudioStore }: { store?: MotionStudioStore }) {
  const t = useT();
  const phase = store((state) => state.phase);
  const error = store((state) => state.error);
  const errorFile = store((state) => state.errorFile);
  const documents = store((state) => state.documents);
  const document = store((state) => state.document);
  const activeFile = store((state) => state.activeFile);
  const html = store((state) => state.html);
  const css = store((state) => state.css);
  const dirtyFiles = store((state) => state.dirtyFiles);
  const savingFile = store((state) => state.savingFile);
  const conflict = store((state) => state.conflict);
  const diagnostics = store((state) => state.diagnostics);
  const diagnosticFile = store((state) => state.diagnosticFile);
  const previewError = store((state) => state.previewError);
  const previewPhase = store((state) => state.previewPhase);
  const lastGoodPreview = store((state) => state.lastGoodPreview);
  const parameters = store((state) => state.parameters);
  const transparent = store((state) => state.transparent);
  const load = store((state) => state.load);
  const suspend = store((state) => state.suspend);
  const resume = store((state) => state.resume);
  const resetProject = store((state) => state.resetProject);
  const selectDocument = store((state) => state.selectDocument);
  const refreshExternalDocument = store((state) => state.refreshExternalDocument);
  const setActiveFile = store((state) => state.setActiveFile);
  const updateSource = store((state) => state.updateSource);
  const reloadConflict = store((state) => state.reloadConflict);
  const reapplyConflict = store((state) => state.reapplyConflict);
  const setParameter = store((state) => state.setParameter);
  const setTransparent = store((state) => state.setTransparent);
  const publishPhase = store((state) => state.publishPhase);
  const publishFrameProgress = store((state) => state.publishFrameProgress);
  const publishError = store((state) => state.publishError);
  const publish = store((state) => state.publish);
  const cancelPublish = store((state) => state.cancelPublish);
  const htmlTabRef = useRef<HTMLButtonElement>(null);
  const cssTabRef = useRef<HTMLButtonElement>(null);
  const lifecycleGenerationRef = useRef(0);
  const appView = useEditorUiStore((state) => state.view);
  const projectEpoch = useProjectStore((state) => state.projectEpoch);
  const projectPath = useProjectStore((state) => state.projectPath);
  const projectIdentityRef = useRef({ projectEpoch, projectPath });

  useEffect(() => {
    ++lifecycleGenerationRef.current;
    return () => {
      const disposedGeneration = ++lifecycleGenerationRef.current;
      queueMicrotask(() => {
        if (lifecycleGenerationRef.current === disposedGeneration) {
          void store.getState().dispose();
        }
      });
    };
  }, [store]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void onMotionDocumentChanged((change) => {
      if (!disposed) void refreshExternalDocument(change);
    }).then((cleanup) => {
      if (disposed) cleanup();
      else unlisten = cleanup;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [refreshExternalDocument]);

  useEffect(() => {
    if (appView !== "motion") {
      void suspend();
    } else {
      void resume();
    }
  }, [appView, resume, suspend]);

  useEffect(() => {
    const previous = projectIdentityRef.current;
    if (previous.projectEpoch === projectEpoch && previous.projectPath === projectPath) return;
    projectIdentityRef.current = { projectEpoch, projectPath };
    resetProject();
    if (appView === "motion") void resume();
  }, [appView, projectEpoch, projectPath, resetProject, resume]);

  const activeSource = activeFile === "index.html" ? html : css;
  const tabRef = (file: MotionDocumentFile) => file === "index.html" ? htmlTabRef : cssTabRef;
  const selectSource = (file: MotionDocumentFile, focus = false) => {
    setActiveFile(file);
    if (focus) queueMicrotask(() => tabRef(file).current?.focus());
  };
  const onTabKeyDown = (event: React.KeyboardEvent, file: MotionDocumentFile) => {
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
    event.preventDefault();
    const index = FILES.indexOf(file);
    const next = event.key === "Home"
      ? FILES[0]!
      : event.key === "End"
        ? FILES[FILES.length - 1]!
        : FILES[(index + (event.key === "ArrowRight" ? 1 : -1) + FILES.length) % FILES.length]!;
    selectSource(next, true);
  };
  const adjacentError = error && (errorFile === null || errorFile === activeFile) ? error : null;
  const adjacentPreviewError = previewError && diagnosticFile === activeFile ? previewError : null;
  const publishActive = ["validating", "rendering", "encoding", "committing"].includes(publishPhase);
  const publishDisabled = publishActive || !document || Boolean(
    savingFile ||
    conflict ||
    dirtyFiles["index.html"] ||
    dirtyFiles["styles.css"] ||
    previewError ||
    previewPhase !== "ready" ||
    lastGoodPreview?.revisionHash !== document?.summary.revisionHash,
  );
  const publishDocument = async () => {
    const lifecycleGeneration = lifecycleGenerationRef.current;
    const projectIdentity = useProjectStore.getState();
    await publish();
    const commit = store.getState().publishCommit;
    if (!commit) return;
    await forceRefresh().catch(() => undefined);
    const currentProject = useProjectStore.getState();
    const ui = useEditorUiStore.getState();
    if (
      lifecycleGenerationRef.current !== lifecycleGeneration ||
      ui.view !== "motion" ||
      currentProject.projectEpoch !== projectIdentity.projectEpoch ||
      currentProject.projectPath !== projectIdentity.projectPath ||
      store.getState().publishCommit?.clipId !== commit.clipId
    ) return;
    ui.selectClips(new Set([commit.clipId]));
    ui.setView("editor");
  };

  return (
    <main aria-label={t("motionStudio.workspace")} className="motion-studio">
      <aside aria-label={t("motionStudio.files")} className="motion-studio__files motion-panel">
        <header className="motion-panel__header">
          <span>{t("motionStudio.files")}</span>
          <Icon icon={FileCode2} size={13} />
        </header>
        <div className="motion-files__section">
          <p className="motion-files__eyebrow">{t("motionStudio.documents")}</p>
          {documents.map((summary) => (
            <button
              type="button"
              key={summary.id}
              className="motion-files__item"
              aria-current={document?.summary.id === summary.id ? "page" : undefined}
              disabled={publishActive}
              onClick={() => void selectDocument(summary.id)}
            >
              <span>{summary.title}</span>
              <small>{new Date(summary.updatedAt).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</small>
            </button>
          ))}
          <label className="motion-inspector__toggle">
            <input
              name="transparent"
              type="checkbox"
              checked={transparent}
              aria-label={t("motionStudio.transparentOutput")}
              onChange={(event) => setTransparent(event.currentTarget.checked)}
            />
            <span>{t("motionStudio.transparentOutput")}</span>
          </label>
        </div>
        <div className="motion-files__section">
          <p className="motion-files__eyebrow"><Icon icon={Sparkles} size={11} /> {t("motionStudio.templates")}</p>
          <span className="motion-files__static">{t("motionStudio.starterTemplate")}</span>
        </div>
        <div className="motion-files__section motion-files__history">
          <p className="motion-files__eyebrow"><Icon icon={Clock3} size={11} /> {t("motionStudio.history")}</p>
          <span className="motion-files__static">
            {document ? t("motionStudio.savedRevision", { revision: document.summary.revisionHash.slice(0, 8) }) : "—"}
          </span>
        </div>
      </aside>

      <section aria-label={t("motionStudio.editor")} className="motion-studio__authoring">
        <div className="motion-studio__editor motion-panel">
          <div className="motion-editor__tabs" role="tablist" aria-label={t("motionStudio.sourceFiles")}>
            {FILES.map((file) => {
              const selected = file === activeFile;
              const hasDiagnostic = file === diagnosticFile && diagnostics.length > 0;
              return (
                <button
                  type="button"
                  key={file}
                  ref={tabRef(file)}
                  role="tab"
                  data-file={file}
                  aria-selected={selected}
                  aria-controls="motion-source-panel"
                  tabIndex={selected ? 0 : -1}
                  onClick={() => selectSource(file)}
                  onKeyDown={(event) => onTabKeyDown(event, file)}
                >
                  {file}
                  {dirtyFiles[file] && <span aria-label={t("motionStudio.unsaved")}>●</span>}
                  {savingFile === file && <span className="motion-editor__saving">{t("motionStudio.saving")}</span>}
                  {hasDiagnostic && <span className="motion-editor__error-dot" aria-label={t("motionStudio.hasErrors")} />}
                </button>
              );
            })}
          </div>
          <div id="motion-source-panel" role="tabpanel" aria-label={activeFile} className="motion-editor__surface">
            {phase === "loading" && <p role="status">{t("motionStudio.loading")}</p>}
            {phase === "error" && !document && (
              <div className="motion-editor__empty" role="alert">
                <p>{error}</p>
                <button type="button" onClick={() => void load()}>{t("motionStudio.retry")}</button>
              </div>
            )}
            {document && (
              <MotionCodeEditor
                file={activeFile}
                value={activeSource}
                label={t("motionStudio.codeEditor")}
                onChange={updateSource}
              />
            )}
          </div>
          {((diagnosticFile === activeFile && diagnostics.length > 0) || ((adjacentError || adjacentPreviewError) && document)) && (
            <div className="motion-editor__diagnostics" role="status" aria-live="polite">
              {(diagnosticFile === activeFile ? diagnostics : []).map((diagnostic, index) => (
                <p key={`${diagnostic.message}-${index}`}>
                  {diagnostic.line && diagnostic.column ? `${diagnostic.line}:${diagnostic.column} ` : ""}
                  {diagnostic.message}
                </p>
              ))}
              {(diagnosticFile !== activeFile || diagnostics.length === 0) && (adjacentError || adjacentPreviewError) && (
                <p>{adjacentError || adjacentPreviewError}</p>
              )}
            </div>
          )}
          {conflict && (
            <div className="motion-editor__conflict" role="alert">
              <p>{t("motionStudio.conflict")}</p>
              <button type="button" data-conflict-action="reload" onClick={() => void reloadConflict()}>
                {t("motionStudio.reloadRemote")}
              </button>
              <button type="button" data-conflict-action="reapply" onClick={() => void reapplyConflict()}>
                {t("motionStudio.reapplyMine")}
              </button>
            </div>
          )}
        </div>
        <MotionPreview store={store} />
      </section>

      <aside aria-label={t("motionStudio.inspector")} className="motion-studio__inspector motion-panel">
        <header className="motion-panel__header">
          <span>{t("motionStudio.inspector")}</span>
          <Icon icon={Layers3} size={13} />
        </header>
        <div className="motion-inspector__fields">
          {(["width", "height", "fps", "durationFrames"] as Array<keyof MotionPublishParameters>).map((name) => (
            <label key={name}>
              <span>{t(`motionStudio.parameter.${name}`)}</span>
              <input
                name={name}
                type="number"
                inputMode="numeric"
                step={name === "width" || name === "height" ? 2 : 1}
                value={parameters[name]}
                min={name === "width" || name === "height" ? 2 : 1}
                max={name === "fps" ? 240 : name === "durationFrames" ? 3600 : 4096}
                onChange={(event) => setParameter(name, Number(event.currentTarget.value))}
              />
            </label>
          ))}
        </div>
        <dl className="motion-inspector__summary">
          <div><dt>{t("motionStudio.aspect")}</dt><dd>{parameters.width} × {parameters.height}</dd></div>
          <div><dt>{t("motionStudio.duration")}</dt><dd>{(parameters.durationFrames / parameters.fps).toFixed(2)}s</dd></div>
        </dl>
        <div className="motion-inspector__publish">
          <button
            type="button"
            data-motion-publish="true"
            disabled={publishDisabled}
            onClick={() => void publishDocument()}
          >
            {publishActive
              ? t(
                  `motionStudio.publishPhase.${publishPhase}`,
                  publishPhase === "rendering" && publishFrameProgress
                    ? { done: publishFrameProgress.done, total: publishFrameProgress.total }
                    : undefined,
                )
              : t("motionStudio.publish")}
          </button>
          {publishActive && (
            <button type="button" onClick={() => void cancelPublish()}>
              {t("motionStudio.cancelPublish")}
            </button>
          )}
          {publishPhase === "rendering" && publishFrameProgress && (
            <p role="status" aria-live="polite">
              {t("motionStudio.renderProgress", {
                done: publishFrameProgress.done,
                total: publishFrameProgress.total,
              })}
            </p>
          )}
          {publishError && <p role="alert">{publishError}</p>}
        </div>
      </aside>

      <MotionTimeline store={store} />
    </main>
  );
}
