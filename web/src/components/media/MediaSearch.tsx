/**
 * Smart media search (1:1 with upstream `MediaTab+Search.swift` +
 * `MediaTab+IndexStatus.swift`). Upgrades the media panel's plain filename filter
 * into three result groups that rank independently and are never blended:
 *
 *  - **Moments** — visual (SigLIP2 semantic) hits with a frame thumbnail,
 *    DRAGGABLE onto the timeline as a trimmed source-range clip.
 *  - **Spoken** — transcript keyword hits with a thumbnail + timecode, also
 *    draggable as a trimmed range.
 *  - **Files** — filename matches (the pre-existing behavior), the zero-setup
 *    fallback that works with no model.
 *
 * When the visual index is unavailable, an index-status affordance appears
 * (download the on-device model → build the index → progress ring), mirroring
 * upstream's `searchIndexStatus`. Moments/Spoken degrade gracefully to empty
 * while Files keeps working, so plain name filtering never needs setup.
 *
 * The visual/spoken groups come from the Rust `search_query` command (best-effort
 * — empty outside Tauri / without a model); Files reuses the caller's already
 * name-filtered item list so it stays instant and offline.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { Sparkles, AlertTriangle, Mic, Film, FileText } from "lucide-react";
import { Icon } from "../ui/Icon";
import { useT } from "../../i18n";
import { formatTimecode } from "../../lib/geometry";
import { assetUrl } from "../../lib/asset";
import {
  derivedResourceKinds,
  derivedResourceScheduler,
} from "../../lib/derivedResourceScheduler";
import { setDraggingMedia } from "../../lib/mediaDragState";
import { setDraggingMomentRange } from "../../lib/momentDragState";
import {
  MEDIA_DND_TYPE,
  TileContextMenu,
  handleMediaTileArrowNavigation,
  keyboardMenuPoint,
  selectMediaForPreview,
  setMediaThumbnailDragImage,
  type TileMenuPoint,
} from "./MediaPanel";
import {
  isCurrentMediaProject,
  useMediaStore,
  type MediaProjectIdentity,
} from "../../store/mediaStore";
import {
  deleteMediaFromContextMenu,
  deleteSelectedMediaAssets,
} from "../../store/mediaDeleteActions";
import { useProjectStore } from "../../store/projectStore";
import { useEditorUiStore } from "../../store/uiStore";
import {
  addMediaToTimeline,
  reportMediaPlacementFailure,
} from "../../store/editActions";
import {
  generateThumbnail,
  preloadMedia,
  searchModelStatus,
  searchIndexStatus,
  searchIndexStart,
  downloadSearchModel,
  onSearchModelProgress,
  onSearchIndexProgress,
  searchQuery as searchQueryApi,
} from "../../lib/api";
import type { MediaItem, MomentHit, SpokenHit, SearchResults } from "../../lib/types";

/** Debounce before firing the backend visual/spoken query (upstream 250ms). */
const SEARCH_DEBOUNCE_MS = 250;
const MODEL_REPAIR_MARKER = "SEARCH_MODEL_REPAIR_REQUIRED:";

function requiresModelRepair(error: string | null | undefined): boolean {
  // Index worker errors wrap the marker (for example, "model error: …").
  return error?.includes(MODEL_REPAIR_MARKER) ?? false;
}

function emptySearchResults(): SearchResults {
  return { moments: [], spoken: [], files: [] };
}

interface MediaSearchRequestHandle {
  cancel: () => void;
  pending: () => Promise<void> | null;
}

/** Request lifecycle shared by the effect and race regression tests. Every
 * query transition, including clearing the field, advances the sequence. */
export function beginMediaSearchRequest({
  query,
  requestSequence,
  search,
  onResults,
  onError,
  schedule = (task, delay) => window.setTimeout(task, delay),
  cancelScheduled = (handle) => window.clearTimeout(handle),
}: {
  query: string;
  requestSequence: { current: number };
  search: (query: string) => Promise<SearchResults>;
  onResults: (results: SearchResults) => void;
  onError: (error: string | null) => void;
  schedule?: (task: () => void, delay: number) => number;
  cancelScheduled?: (handle: number) => void;
}): MediaSearchRequestHandle {
  const id = ++requestSequence.current;
  const q = query.trim();
  let disposed = false;
  let scheduled: number | null = null;
  let pendingRequest: Promise<void> | null = null;

  // Do not display a previous query's semantic hits during debounce or after a
  // rejection. Filename matches are supplied independently by nameMatches.
  onResults(emptySearchResults());
  onError(null);

  if (q === "") {
    return {
      cancel: () => {
        disposed = true;
      },
      pending: () => null,
    };
  }

  scheduled = schedule(() => {
    pendingRequest = search(q).then(
      (results) => {
        if (!disposed && id === requestSequence.current) onResults(results);
      },
      (error: unknown) => {
        if (disposed || id !== requestSequence.current) return;
        onResults(emptySearchResults());
        onError(searchErrorMessage(error));
      },
    );
  }, SEARCH_DEBOUNCE_MS);

  return {
    cancel: () => {
      disposed = true;
      if (scheduled !== null) cancelScheduled(scheduled);
    },
    pending: () => pendingRequest,
  };
}

function searchErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message.trim() !== "") return error.message;
  const message = String(error);
  return message === "" ? "Search failed" : message;
}

/** Bridge an asynchronously-created Tauri listener into synchronous React
 * cleanup. Late callbacks are ignored and a late unlisten handle is invoked. */
export function subscribeWithAsyncCleanup<T>(
  subscribe: (listener: (payload: T) => void) => Promise<() => void>,
  listener: (payload: T) => void,
): () => void {
  let disposed = false;
  let unlisten: (() => void) | null = null;
  void subscribe((payload) => {
    if (!disposed) listener(payload);
  }).then(
    (off) => {
      if (disposed) off();
      else unlisten = off;
    },
    () => {
      // Progress listeners are best-effort outside Tauri. Polling and action
      // buttons remain usable when registration is unavailable.
    },
  );
  return () => {
    if (disposed) return;
    disposed = true;
    unlisten?.();
    unlisten = null;
  };
}

function formatModelBytes(bytes: number | null): string | null {
  if (bytes === null || !Number.isFinite(bytes) || bytes <= 0) return null;
  return bytes >= 1_000_000_000
    ? `${(bytes / 1_000_000_000).toFixed(2)} GB`
    : `${Math.ceil(bytes / 1_000_000)} MB`;
}

/** The visual-index lifecycle the affordance renders. */
type IndexPhase =
  | { kind: "hidden" }
  | { kind: "needsModel" }
  | { kind: "downloading"; fraction: number }
  | { kind: "readyToIndex" }
  | { kind: "indexing"; done: number; total: number; fraction: number }
  | { kind: "failed"; action: "download" | "index"; error?: string };

interface SearchProjectOperation {
  project: MediaProjectIdentity;
  token: symbol;
  phase: Extract<IndexPhase, { kind: "downloading" | "indexing" }>;
}

type SearchOperationAction = "download" | "index";

interface SearchOperationEvent {
  action: SearchOperationAction;
  operation: SearchProjectOperation;
  state: "started" | "settled";
  succeeded?: boolean;
  error?: string;
}

interface SearchIndexController {
  phase: IndexPhase;
  modelBytes: number | null;
  queryRevision: number;
  startDownload: () => void;
  startIndex: () => void;
}

/** Backend progress events do not carry project identity. Keep the one global
 * backend job associated with the project that started it, and do not start a
 * second project's job until the first settles. */
let activeModelDownload: SearchProjectOperation | null = null;
let activeIndexBuild: SearchProjectOperation | null = null;
const searchOperationListeners = new Set<(event: SearchOperationEvent) => void>();

function publishSearchOperation(event: SearchOperationEvent): void {
  for (const listener of [...searchOperationListeners]) listener(event);
}

function subscribeSearchOperations(listener: (event: SearchOperationEvent) => void): () => void {
  searchOperationListeners.add(listener);
  return () => searchOperationListeners.delete(listener);
}

function settleSearchOperation(
  action: SearchOperationAction,
  operation: SearchProjectOperation,
  succeeded: boolean,
  error?: string,
): void {
  const active = action === "download" ? activeModelDownload : activeIndexBuild;
  if (active?.token !== operation.token) return;
  if (action === "download") activeModelDownload = null;
  else activeIndexBuild = null;
  publishSearchOperation({ action, operation, state: "settled", succeeded, error });
}

function sameSearchProject(
  left: MediaProjectIdentity,
  right: MediaProjectIdentity,
): boolean {
  return left.projectEpoch === right.projectEpoch && left.projectPath === right.projectPath;
}

/**
 * The full search view: index-status affordance + the three result groups.
 * `nameMatches` is the caller's already name-filtered items (the Files group).
 */
export function MediaSearchResults({
  query,
  nameMatches,
  hasIndexableAssets,
}: {
  query: string;
  nameMatches: MediaItem[];
  hasIndexableAssets: boolean;
}) {
  const t = useT();
  const [results, setResults] = useState<SearchResults>(emptySearchResults);
  const [searchError, setSearchError] = useState<string | null>(null);
  const projectEpoch = useProjectStore((s) => s.projectEpoch);
  const projectPath = useProjectStore((s) => s.projectPath);
  const indexController = useSearchIndexPhase(hasIndexableAssets, projectEpoch, projectPath, results.visualError);

  // Debounced backend query for Moments + Spoken. Files come from `nameMatches`.
  const reqId = useRef(0);
  useEffect(() => {
    const project = { projectEpoch, projectPath };
    const request = beginMediaSearchRequest({
      query,
      requestSequence: reqId,
      search: searchQueryApi,
      onResults: (next) => {
        if (isCurrentMediaProject(project)) setResults(next);
      },
      onError: (next) => {
        if (isCurrentMediaProject(project)) setSearchError(next);
      },
    });
    return request.cancel;
  }, [projectEpoch, projectPath, query, indexController.queryRevision]);

  const { moments, spoken } = results;
  const selectedMediaAssetIds = useEditorUiStore((s) => s.selectedMediaAssetIds);
  const activeMomentIndex = Math.max(
    0,
    moments.findIndex((hit) => selectedMediaAssetIds.has(hit.mediaId)),
  );
  const activeSpokenIndex = Math.max(
    0,
    spoken.findIndex((hit) => selectedMediaAssetIds.has(hit.mediaId)),
  );
  const activeFileIndex = Math.max(
    0,
    nameMatches.findIndex((item) => selectedMediaAssetIds.has(item.id)),
  );
  const visualError = results.visualError;
  const displayedError = searchError ?? (visualError
    ? requiresModelRepair(visualError)
      ? t("search.repairModelHint")
      : visualError.includes("SEARCH_VISUAL_BUSY:")
        ? t("search.visualBusy")
        : t("search.visualFailed", { error: visualError })
    : null);
  const isEmpty =
    displayedError === null &&
    moments.length === 0 &&
    spoken.length === 0 &&
    nameMatches.length === 0;

  return (
    <div style={{ flex: 1, overflowY: "auto", display: "flex", flexDirection: "column" }}>
      <SearchIndexAffordance
        phase={indexController.phase}
        modelBytes={indexController.modelBytes}
        onDownload={indexController.startDownload}
        onIndex={indexController.startIndex}
      />

      {displayedError && (
        <div
          role="alert"
          aria-live="polite"
          style={{
            display: "flex",
            alignItems: "center",
            gap: "var(--space-xs)",
            margin: "var(--space-xs) var(--space-sm) 0",
            padding: "var(--space-xs) var(--space-sm)",
            borderRadius: "var(--radius-sm)",
            border: "var(--bw-thin) solid var(--status-error)",
            color: "var(--status-error)",
            fontSize: "var(--fs-xs)",
          }}
        >
          <Icon icon={AlertTriangle} size={13} />
          <span>{displayedError}</span>
        </div>
      )}

      {moments.length > 0 && (
        <Group icon={Film} label={t("search.group.moments")} count={moments.length}>
          <ResultsGrid>
            {moments.map((hit, i) => (
              <div key={`${hit.mediaId}:${hit.frame}:${i}`} role="row" style={{ minWidth: 0 }}>
                <MomentCard
                  hit={hit}
                  projectEpoch={projectEpoch}
                  rovingTabIndex={i === activeMomentIndex ? 0 : -1}
                />
              </div>
            ))}
          </ResultsGrid>
        </Group>
      )}

      {spoken.length > 0 && (
        <Group icon={Mic} label={t("search.group.spoken")} count={spoken.length}>
          <div
            role="grid"
            data-media-roving-container="true"
            style={{ display: "flex", flexDirection: "column", gap: "var(--space-xs)", padding: "0 var(--space-sm) var(--space-sm)" }}
          >
            {spoken.map((hit, i) => (
              <div key={`${hit.mediaId}:${hit.startSec}:${i}`} role="row" style={{ minWidth: 0 }}>
                <SpokenRow
                  hit={hit}
                  projectEpoch={projectEpoch}
                  rovingTabIndex={i === activeSpokenIndex ? 0 : -1}
                />
              </div>
            ))}
          </div>
        </Group>
      )}

      {nameMatches.length > 0 && (
        <Group icon={FileText} label={t("search.group.files")} count={nameMatches.length}>
          <ResultsGrid>
            {nameMatches.map((item, index) => (
              <div key={item.id} role="row" style={{ minWidth: 0 }}>
                <FileCard
                  item={item}
                  rovingTabIndex={index === activeFileIndex ? 0 : -1}
                />
              </div>
            ))}
          </ResultsGrid>
        </Group>
      )}

      {isEmpty && (
        <div
          style={{
            flex: 1,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            color: "var(--text-tertiary)",
            fontSize: "var(--fs-sm)",
            padding: "var(--space-xl)",
            textAlign: "center",
          }}
        >
          {t("search.noMatches", { query: query.trim() })}
        </div>
      )}
    </div>
  );
}

/** Poll model + index status and subscribe to progress, deriving the affordance
 *  phase. Mirrors upstream `searchIndexStatus`'s state machine. */
function useSearchIndexPhase(
  hasIndexableAssets: boolean,
  projectEpoch: number,
  projectPath: string | null,
  visualError?: string | null,
): SearchIndexController {
  const [phase, setPhase] = useState<IndexPhase>({ kind: "hidden" });
  const [modelBytes, setModelBytes] = useState<number | null>(null);
  const [queryRevision, setQueryRevision] = useState(0);
  const mediaCount = useMediaStore((s) => s.items.length);
  const mounted = useRef(false);
  const statusGeneration = useRef(0);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      statusGeneration.current += 1;
    };
  }, []);

  useEffect(() => {
    statusGeneration.current += 1;
    setPhase({ kind: "hidden" });
  }, [projectEpoch, projectPath]);

  useEffect(() => {
    let disposed = false;
    const project = { projectEpoch, projectPath };
    setModelBytes(null);
    void (async () => {
      try {
        const status = await searchModelStatus();
        if (!disposed && isCurrentMediaProject(project)) setModelBytes(status.bytes);
      } catch {
        // A missing manifest size must not block search or invent a download size.
      }
    })();
    return () => { disposed = true; };
  }, [projectEpoch, projectPath]);

  // Re-evaluate whenever the library changes (new assets → maybe need indexing).
  const refresh = useCallback(async (failure?: Extract<IndexPhase, { kind: "failed" }>) => {
    const project = { projectEpoch, projectPath };
    const generation = ++statusGeneration.current;
    let status: Awaited<ReturnType<typeof searchIndexStatus>>;
    try {
      status = await searchIndexStatus();
    } catch {
      if (
        mounted.current &&
        generation === statusGeneration.current &&
        isCurrentMediaProject(project)
      ) {
        setPhase(failure ?? { kind: "hidden" });
      }
      return;
    }
    if (
      !mounted.current ||
      generation !== statusGeneration.current ||
      !isCurrentMediaProject(project)
    ) return;
    const running = [activeModelDownload, activeIndexBuild].find(
      (operation) => operation && sameSearchProject(operation.project, project),
    );
    if (running) {
      setPhase(running.phase);
      return;
    }
    if (failure) {
      setPhase(failure);
      return;
    }
    if (!status.modelInstalled) {
      setPhase(hasIndexableAssets ? { kind: "needsModel" } : { kind: "hidden" });
      return;
    }
    if (status.indexable > 0 && status.indexed < status.indexable) {
      setPhase({ kind: "readyToIndex" });
    } else {
      setPhase({ kind: "hidden" });
    }
  }, [hasIndexableAssets, projectEpoch, projectPath]);

  useEffect(() => {
    void refresh();
  }, [refresh, mediaCount]);

  useEffect(
    () =>
      subscribeSearchOperations((event) => {
        const project = { projectEpoch, projectPath };
        if (event.state === "started") {
          if (
            mounted.current &&
            isCurrentMediaProject(project) &&
            sameSearchProject(event.operation.project, project)
          ) {
            setPhase(event.operation.phase);
          }
          return;
        }
        const sameProject = sameSearchProject(event.operation.project, project);
        const failure: Extract<IndexPhase, { kind: "failed" }> | undefined =
          event.succeeded === false && sameProject
            ? {
                kind: "failed",
                action: requiresModelRepair(event.error) ? "download" : event.action,
                error: event.error,
              }
            : undefined;
        void refresh(failure).then(() => {
          if (event.succeeded && sameProject && mounted.current && isCurrentMediaProject(project)) {
            setQueryRevision((revision) => revision + 1);
          }
        });
      }),
    [projectEpoch, projectPath, refresh],
  );

  // Live download + indexing progress events keep the ring moving.
  useEffect(() => {
    const offDownload = subscribeWithAsyncCleanup(onSearchModelProgress, (fraction) => {
      const operation = activeModelDownload;
      if (!operation || !isCurrentMediaProject(operation.project)) return;
      operation.phase = { kind: "downloading", fraction };
      setPhase(operation.phase);
    });
    const offIndex = subscribeWithAsyncCleanup(
      onSearchIndexProgress,
      ({ completed, total, fraction }) => {
        if (total === 0) {
          void refresh();
          return;
        }
        const operation = activeIndexBuild;
        if (!operation || !isCurrentMediaProject(operation.project)) return;
        operation.phase = { kind: "indexing", done: completed, total, fraction };
        setPhase(operation.phase);
        // On the final tick, settle back to the resting state.
        if (completed >= total) {
          void refresh();
        }
      },
    );
    return () => {
      offDownload();
      offIndex();
    };
  }, [refresh]);

  const startDownload = useCallback(() => {
    const project = { projectEpoch, projectPath };
    const active = activeModelDownload;
    if (active) {
      setPhase(
        sameSearchProject(active.project, project)
          ? { kind: "downloading", fraction: 0 }
          : { kind: "failed", action: "download" },
      );
      return;
    }
    const operation: SearchProjectOperation = {
      project,
      token: Symbol("search-model-download"),
      phase: { kind: "downloading", fraction: 0 },
    };
    activeModelDownload = operation;
    publishSearchOperation({ action: "download", operation, state: "started" });
    void downloadSearchModel().then(
      () => {
        settleSearchOperation("download", operation, true);
      },
      (error: unknown) => {
        settleSearchOperation("download", operation, false, searchErrorMessage(error));
      },
    );
  }, [projectEpoch, projectPath]);

  const startIndex = useCallback(() => {
    const project = { projectEpoch, projectPath };
    if (projectPath === null) {
      setPhase({ kind: "failed", action: "index" });
      return;
    }
    const active = activeIndexBuild;
    if (active) {
      setPhase(
        sameSearchProject(active.project, project)
          ? { kind: "indexing", done: 0, total: 1, fraction: 0 }
          : { kind: "failed", action: "index" },
      );
      return;
    }
    const operation: SearchProjectOperation = {
      project,
      token: Symbol("search-index-build"),
      phase: { kind: "indexing", done: 0, total: 1, fraction: 0 },
    };
    activeIndexBuild = operation;
    publishSearchOperation({ action: "index", operation, state: "started" });
    void searchIndexStart(projectEpoch, projectPath).then(
      () => {
        settleSearchOperation("index", operation, true);
      },
      (error: unknown) => {
        settleSearchOperation("index", operation, false, searchErrorMessage(error));
      },
    );
  }, [projectEpoch, projectPath]);

  // A complete index is not proof that its model can execute. Query verification
  // failures must still offer repair, without replacing any Files/Spoken hits.
  const effectivePhase: IndexPhase = requiresModelRepair(visualError) &&
    phase.kind !== "downloading" && phase.kind !== "indexing" &&
    !(phase.kind === "failed" && phase.action === "download")
    ? { kind: "failed", action: "download", error: visualError ?? undefined }
    : phase;
  return { phase: effectivePhase, modelBytes, queryRevision, startDownload, startIndex };
}

/** The status affordance: a download/enable button (no model) or a progress ring
 *  (downloading / indexing). Hidden when nothing needs attention (upstream
 *  `MediaTab+IndexStatus.swift`). */
function SearchIndexAffordance({
  phase,
  modelBytes,
  onDownload,
  onIndex,
}: {
  phase: IndexPhase;
  modelBytes: number | null;
  onDownload: () => void;
  onIndex: () => void;
}) {
  const t = useT();

  if (phase.kind === "hidden") return null;

  const barStyle: React.CSSProperties = {
    display: "flex",
    alignItems: "center",
    gap: "var(--space-xs)",
    padding: "var(--space-xs) var(--space-sm)",
    margin: "var(--space-xs) var(--space-sm) 0",
    borderRadius: "var(--radius-sm)",
    background: "var(--bg-raised)",
    border: "var(--bw-thin) solid var(--border-subtle)",
    fontSize: "var(--fs-xs)",
    color: "var(--text-secondary)",
  };

  if (phase.kind === "needsModel") {
    const size = formatModelBytes(modelBytes);
    return (
      <button
        type="button"
        onClick={onDownload}
        title={size ? t("search.smartSearchHint", { size }) : t("search.smartSearchUnknownSizeHint")}
        style={{ ...barStyle, cursor: "pointer", textAlign: "left" }}
      >
        <Icon icon={Sparkles} size={13} />
        <span style={{ fontWeight: "var(--fw-medium)" }}>{t("search.smartSearch")}</span>
      </button>
    );
  }
  if (phase.kind === "readyToIndex") {
    return (
      <button
        type="button"
        onClick={onIndex}
        title={t("search.indexHint")}
        style={{ ...barStyle, cursor: "pointer", textAlign: "left" }}
      >
        <Icon icon={Sparkles} size={13} />
        <span style={{ fontWeight: "var(--fw-medium)" }}>{t("search.index")}</span>
      </button>
    );
  }
  if (phase.kind === "failed") {
    const repair = requiresModelRepair(phase.error);
    return (
      <button
        type="button"
        onClick={phase.action === "download" ? onDownload : onIndex}
        title={t(repair ? "search.repairModelHint" : phase.action === "index" ? "search.indexRetryHint" : "search.retryHint")}
        style={{ ...barStyle, cursor: "pointer", textAlign: "left", color: "var(--status-error)" }}
      >
        <Icon icon={AlertTriangle} size={13} />
        <span>
          <span style={{ fontWeight: "var(--fw-medium)" }}>{t(repair ? "search.repairModel" : "search.retry")}</span>
          {phase.error && <span style={{ display: "block" }}>{repair ? t("search.repairModelHint") : phase.error}</span>}
        </span>
      </button>
    );
  }
  // downloading | indexing → progress ring + label.
  const fraction = phase.fraction;
  const label =
    phase.kind === "downloading"
      ? t("search.downloading", { percent: Math.round(phase.fraction * 100) })
      : t("search.indexing", { done: Math.min(phase.done + 1, phase.total), total: phase.total });
  return (
    <div style={barStyle} title={phase.kind === "downloading" ? t("search.downloadingHint") : t("search.indexingHint")}>
      <ProgressRing value={fraction} />
      <span style={{ color: "var(--text-tertiary)" }}>{label}</span>
    </div>
  );
}

/** A small SVG progress ring (upstream `progressRing`). */
function ProgressRing({ value }: { value: number }) {
  const v = Math.max(0.03, Math.min(1, value));
  const size = 14;
  const stroke = 2;
  const r = (size - stroke) / 2;
  const c = 2 * Math.PI * r;
  return (
    <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} style={{ flex: "0 0 auto" }}>
      <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke="var(--border-subtle)" strokeWidth={stroke} />
      <circle
        cx={size / 2}
        cy={size / 2}
        r={r}
        fill="none"
        stroke="var(--text-secondary)"
        strokeWidth={stroke}
        strokeLinecap="round"
        strokeDasharray={c}
        strokeDashoffset={c * (1 - v)}
        transform={`rotate(-90 ${size / 2} ${size / 2})`}
      />
    </svg>
  );
}

/** A collapsible-looking group header + body (upstream `momentHeader`). */
function Group({
  icon,
  label,
  count,
  children,
}: {
  icon: typeof Film;
  label: string;
  count: number;
  children: React.ReactNode;
}) {
  return (
    <div style={{ display: "flex", flexDirection: "column" }}>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: "var(--space-xs)",
          padding: "var(--space-sm) var(--space-md)",
          color: "var(--text-secondary)",
        }}
      >
        <Icon icon={icon} size={12} />
        <span style={{ fontSize: "var(--fs-xs)", fontWeight: "var(--fw-semibold)" }}>{label}</span>
        <span className="tabular" style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)" }}>
          {count}
        </span>
      </div>
      {children}
    </div>
  );
}

/** The adaptive grid the Moments + Files groups use (upstream `resultsGrid`). */
function ResultsGrid({ children }: { children: React.ReactNode }) {
  return (
    <div
      role="grid"
      data-media-roving-container="true"
      style={{
        display: "grid",
        gridTemplateColumns: "repeat(auto-fill, minmax(112px, 1fr))",
        gap: "var(--space-sm)",
        padding: "0 var(--space-sm) var(--space-md)",
      }}
    >
      {children}
    </div>
  );
}

/** Async frame thumbnail for a search hit at a specific source-second time. */
function HitThumbnail({
  mediaId,
  timeSec,
  projectEpoch,
  sourceKey,
  alt,
  thumbnailRef,
}: {
  mediaId: string;
  timeSec: number;
  projectEpoch: number;
  sourceKey?: string;
  alt: string;
  thumbnailRef?: React.Ref<HTMLDivElement>;
}) {
  const [path, setPath] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    setPath(null);
    derivedResourceScheduler.activateProject(projectEpoch);
    const handle = derivedResourceScheduler.request<string | null>({
      projectEpoch,
      kind: derivedResourceKinds.searchThumbnail,
      key: `thumbnail:search:${mediaId}:${sourceKey ?? ""}:${timeSec}`,
      priority: "visible",
      run: async () => {
        const result = await generateThumbnail(mediaId, {
          timeSecs: timeSec,
          includeSprite: false,
        });
        return result?.thumbnailPath ?? null;
      },
    });
    void handle.promise
      .then((next) => {
        if (!cancelled) setPath(next);
      })
      .catch(() => {
        if (!cancelled) setPath(null);
      });
    return () => {
      cancelled = true;
      handle.cancel();
    };
  }, [mediaId, timeSec, projectEpoch, sourceKey]);
  const src = assetUrl(path);
  return (
    <div
      ref={thumbnailRef}
      style={{
        position: "relative",
        aspectRatio: "16 / 9",
        background: "var(--bg-placeholder)",
        borderRadius: "var(--radius-sm)",
        overflow: "hidden",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      {src ? (
        <img src={src} alt={alt} draggable={false} style={{ width: "100%", height: "100%", objectFit: "cover" }} />
      ) : (
        <Icon icon={Film} size={18} strokeWidth={1.5} />
      )}
    </div>
  );
}

/** Look up a media item by id (for the name + drag payload). */
function useMediaItem(mediaId: string): MediaItem | undefined {
  return useMediaStore((s) => s.items.find((m) => m.id === mediaId));
}

/** All search result variants participate in the same media selection contract
 * as the main grid, including keyboard activation and deletion. */
function useSearchResultInteraction(item: MediaItem | undefined) {
  const t = useT();
  const tileRef = useRef<HTMLDivElement | null>(null);
  const selected = useEditorUiStore((s) =>
    item ? s.selectedMediaAssetIds.has(item.id) : false,
  );
  const [focused, setFocused] = useState(false);
  const [menuPoint, setMenuPoint] = useState<
    (TileMenuPoint & { restoreFocus: boolean }) | null
  >(null);
  const activate = useCallback(() => {
    if (!item) return;
    selectMediaForPreview(item.id);
    void preloadMedia(item.id);
  }, [item]);
  const onClick = useCallback(
    (event: React.MouseEvent<HTMLDivElement>) => {
      event.currentTarget.focus();
      activate();
    },
    [activate],
  );
  const onContextMenu = useCallback((event: React.MouseEvent<HTMLDivElement>) => {
    if (!item) return;
    event.preventDefault();
    event.stopPropagation();
    setMenuPoint({
      x: event.clientX,
      y: event.clientY,
      restoreFocus: false,
    });
  }, [item]);
  const onKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLDivElement>) => {
      if (!item || event.target !== event.currentTarget) return;
      if (handleMediaTileArrowNavigation(event)) return;
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        event.stopPropagation();
        activate();
      } else if (event.key === "Delete" || event.key === "Backspace") {
        event.preventDefault();
        event.stopPropagation();
        if (!event.repeat) {
          void deleteSelectedMediaAssets().catch((error) => {
            useEditorUiStore.getState().pushToast(String(error));
          });
        }
      } else if (event.key === "ContextMenu" || (event.shiftKey && event.key === "F10")) {
        event.preventDefault();
        event.stopPropagation();
        setMenuPoint({
          ...keyboardMenuPoint(event.currentTarget),
          restoreFocus: true,
        });
      }
    },
    [activate, item],
  );
  const onFocus = useCallback((event: React.FocusEvent<HTMLDivElement>) => {
    setFocused(true);
    if (event.target === event.currentTarget) activate();
  }, [activate]);
  const onBlur = useCallback((event: React.FocusEvent<HTMLDivElement>) => {
    if (!event.currentTarget.contains(event.relatedTarget)) setFocused(false);
  }, []);
  const menu =
    item && menuPoint ? (
      <TileContextMenu
        point={menuPoint}
        onClose={() => setMenuPoint(null)}
        returnFocus={tileRef}
        restoreFocus={menuPoint.restoreFocus}
        actions={[
          {
            label: t("contextMenu.delete"),
            destructive: true,
            onSelect: () => {
              void deleteMediaFromContextMenu(item.id).catch((error) => {
                useEditorUiStore.getState().pushToast(String(error));
              });
            },
          },
        ]}
      />
    ) : null;
  return {
    tileRef,
    selected,
    focused,
    menu,
    onClick,
    onContextMenu,
    onKeyDown,
    onFocus,
    onBlur,
  };
}

/** A visual "Moments" card: frame thumb + name + timecode range, draggable to the
 *  timeline as a trimmed source-range clip (upstream `momentCard`). */
function MomentCard({
  hit,
  projectEpoch,
  rovingTabIndex,
}: {
  hit: MomentHit;
  projectEpoch: number;
  rovingTabIndex: number;
}) {
  const t = useT();
  const item = useMediaItem(hit.mediaId);
  const fps = useProjectStore((s) => s.timeline.fps);
  const interaction = useSearchResultInteraction(item);
  const thumbnailRef = useRef<HTMLDivElement | null>(null);
  if (!item) return null;

  const onDragStart = (e: React.DragEvent) => {
    e.dataTransfer.setData(MEDIA_DND_TYPE, item.id);
    e.dataTransfer.effectAllowed = "copy";
    if (thumbnailRef.current) {
      setMediaThumbnailDragImage(e.dataTransfer, thumbnailRef.current);
    }
    setDraggingMedia(item);
    void preloadMedia(item.id);
    // Stills drag as the whole asset (no meaningful range).
    if (!hit.isImage) setDraggingMomentRange({ startSec: hit.startSec, endSec: hit.endSec });
    else setDraggingMomentRange(null);
  };
  const onDragEnd = () => {
    setDraggingMedia(null);
    setDraggingMomentRange(null);
  };

  const startFrames = Math.round(hit.startSec * fps);
  const endFrames = Math.round(hit.endSec * fps);

  return (
    <div
      ref={interaction.tileRef}
      draggable
      onDragStart={onDragStart}
      onDragEnd={onDragEnd}
      onClick={interaction.onClick}
      onContextMenu={interaction.onContextMenu}
      onKeyDown={interaction.onKeyDown}
      onFocus={interaction.onFocus}
      onBlur={interaction.onBlur}
      title={t("search.dragToTimeline")}
      role="gridcell"
      tabIndex={rovingTabIndex}
      aria-selected={interaction.selected}
      aria-haspopup="menu"
      data-media-tile="true"
      data-media-asset-id={item.id}
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 3,
        cursor: "grab",
        borderRadius: "var(--radius-sm)",
        outline:
          interaction.selected || interaction.focused
            ? "2px solid var(--accent-primary)"
            : "none",
        outlineOffset: 2,
      }}
    >
      <HitThumbnail
        mediaId={hit.mediaId}
        timeSec={hit.startSec}
        projectEpoch={projectEpoch}
        sourceKey={item.path ?? undefined}
        alt={item.name}
        thumbnailRef={thumbnailRef}
      />
      <span
        style={{
          fontSize: "var(--fs-xs)",
          color: "var(--text-secondary)",
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}
      >
        {item.name}
      </span>
      {!hit.isImage && (
        <span className="tabular" style={{ fontSize: "var(--fs-micro)", color: "var(--text-tertiary)" }}>
          {formatTimecode(startFrames, fps)}–{formatTimecode(endFrames, fps)}
        </span>
      )}
      {interaction.menu}
    </div>
  );
}

/** A "Spoken" transcript row: thumb + text + name·timecode, draggable as a
 *  trimmed range (upstream `spokenRow`). */
function SpokenRow({
  hit,
  projectEpoch,
  rovingTabIndex,
}: {
  hit: SpokenHit;
  projectEpoch: number;
  rovingTabIndex: number;
}) {
  const t = useT();
  const item = useMediaItem(hit.mediaId);
  const fps = useProjectStore((s) => s.timeline.fps);
  const interaction = useSearchResultInteraction(item);
  const thumbnailRef = useRef<HTMLDivElement | null>(null);
  if (!item) return null;

  const onDragStart = (e: React.DragEvent) => {
    e.dataTransfer.setData(MEDIA_DND_TYPE, item.id);
    e.dataTransfer.effectAllowed = "copy";
    if (thumbnailRef.current) {
      setMediaThumbnailDragImage(e.dataTransfer, thumbnailRef.current);
    }
    setDraggingMedia(item);
    void preloadMedia(item.id);
    setDraggingMomentRange({ startSec: hit.startSec, endSec: hit.endSec });
  };
  const onDragEnd = () => {
    setDraggingMedia(null);
    setDraggingMomentRange(null);
  };

  return (
    <div
      ref={interaction.tileRef}
      draggable
      onDragStart={onDragStart}
      onDragEnd={onDragEnd}
      onClick={interaction.onClick}
      onContextMenu={interaction.onContextMenu}
      onKeyDown={interaction.onKeyDown}
      onFocus={interaction.onFocus}
      onBlur={interaction.onBlur}
      title={t("search.dragToTimeline")}
      role="gridcell"
      tabIndex={rovingTabIndex}
      aria-selected={interaction.selected}
      aria-haspopup="menu"
      data-media-tile="true"
      data-media-asset-id={item.id}
      style={{
        display: "flex",
        gap: "var(--space-sm)",
        cursor: "grab",
        alignItems: "flex-start",
        borderRadius: "var(--radius-sm)",
        outline:
          interaction.selected || interaction.focused
            ? "2px solid var(--accent-primary)"
            : "none",
        outlineOffset: 2,
      }}
    >
      <div style={{ width: 96, flex: "0 0 auto" }}>
        <HitThumbnail
          mediaId={hit.mediaId}
          timeSec={hit.startSec}
          projectEpoch={projectEpoch}
          sourceKey={item.path ?? undefined}
          alt={item.name}
          thumbnailRef={thumbnailRef}
        />
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 2, minWidth: 0 }}>
        <span
          style={{
            fontSize: "var(--fs-xs)",
            color: "var(--text-primary)",
            display: "-webkit-box",
            WebkitLineClamp: 3,
            WebkitBoxOrient: "vertical",
            overflow: "hidden",
          }}
        >
          {hit.text}
        </span>
        <span
          className="tabular"
          style={{
            fontSize: "var(--fs-micro)",
            color: "var(--text-tertiary)",
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {item.name} · {formatTimecode(Math.round(hit.startSec * fps), fps)}
        </span>
      </div>
      {interaction.menu}
    </div>
  );
}

/** A "Files" name-match card: thumb + name, draggable as the whole asset (the
 *  pre-existing behavior; upstream `fileCard`). */
function FileCard({
  item,
  rovingTabIndex,
}: {
  item: MediaItem;
  rovingTabIndex: number;
}) {
  const interaction = useSearchResultInteraction(item);
  const thumb = item.missing ? null : assetUrl(item.thumbnail);
  const thumbnailRef = useRef<HTMLDivElement | null>(null);

  const onDragStart = (e: React.DragEvent) => {
    e.dataTransfer.setData(MEDIA_DND_TYPE, item.id);
    e.dataTransfer.effectAllowed = "copy";
    if (thumbnailRef.current) {
      setMediaThumbnailDragImage(e.dataTransfer, thumbnailRef.current);
    }
    setDraggingMedia(item);
    void preloadMedia(item.id);
    setDraggingMomentRange(null); // whole asset
  };
  const onDragEnd = () => setDraggingMedia(null);

  return (
    <div
      ref={interaction.tileRef}
      draggable
      onDragStart={onDragStart}
      onDragEnd={onDragEnd}
      onClick={interaction.onClick}
      onContextMenu={interaction.onContextMenu}
      onKeyDown={interaction.onKeyDown}
      onFocus={interaction.onFocus}
      onBlur={interaction.onBlur}
      onDoubleClick={() => void addMediaToTimeline(item).catch(reportMediaPlacementFailure)}
      title={item.name}
      role="gridcell"
      tabIndex={rovingTabIndex}
      aria-selected={interaction.selected}
      aria-haspopup="menu"
      data-media-tile="true"
      data-media-asset-id={item.id}
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 3,
        cursor: "grab",
        borderRadius: "var(--radius-sm)",
        outline:
          interaction.selected || interaction.focused
            ? "2px solid var(--accent-primary)"
            : "none",
        outlineOffset: 2,
      }}
    >
      <div
        ref={thumbnailRef}
        style={{
          aspectRatio: "16 / 9",
          background: "var(--bg-placeholder)",
          borderRadius: "var(--radius-sm)",
          overflow: "hidden",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
        }}
      >
        {thumb ? (
          <img src={thumb} alt={item.name} draggable={false} style={{ width: "100%", height: "100%", objectFit: "cover" }} />
        ) : (
          <Icon icon={Film} size={18} strokeWidth={1.5} />
        )}
      </div>
      <span
        style={{
          fontSize: "var(--fs-xs)",
          color: "var(--text-secondary)",
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}
      >
        {item.name}
      </span>
      {interaction.menu}
    </div>
  );
}
