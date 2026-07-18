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
import { setDraggingMedia } from "../../lib/mediaDragState";
import { setDraggingMomentRange } from "../../lib/momentDragState";
import { MEDIA_DND_TYPE, setMediaThumbnailDragImage } from "./MediaPanel";
import { useMediaStore } from "../../store/mediaStore";
import { useProjectStore } from "../../store/projectStore";
import { useEditorUiStore } from "../../store/uiStore";
import { addMediaToTimeline } from "../../store/editActions";
import {
  generateThumbnail,
  preloadMedia,
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
/** Combined SigLIP2 model size shown before download when the manifest is a
 *  placeholder (bytes 0). ~380 MB is the two fp32 ONNX encoders + tokenizer. */
const FALLBACK_MODEL_MB = 380;

function formatModelBytes(bytes: number): string {
  const mb = bytes > 0 ? bytes / (1024 * 1024) : FALLBACK_MODEL_MB;
  return `${Math.round(mb)} MB`;
}

/** The visual-index lifecycle the affordance renders. */
type IndexPhase =
  | { kind: "hidden" }
  | { kind: "needsModel" }
  | { kind: "downloading"; fraction: number }
  | { kind: "readyToIndex" }
  | { kind: "indexing"; done: number; total: number; fraction: number }
  | { kind: "failed" };

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
  const [results, setResults] = useState<SearchResults>({ moments: [], spoken: [], files: [] });
  const phase = useSearchIndexPhase(hasIndexableAssets);

  // Debounced backend query for Moments + Spoken. Files come from `nameMatches`.
  const reqId = useRef(0);
  useEffect(() => {
    const q = query.trim();
    if (q === "") {
      setResults({ moments: [], spoken: [], files: [] });
      return;
    }
    const id = ++reqId.current;
    const handle = window.setTimeout(() => {
      void searchQueryApi(q).then((r) => {
        // Ignore a stale response (a newer query superseded this one).
        if (id === reqId.current) setResults(r);
      });
    }, SEARCH_DEBOUNCE_MS);
    return () => window.clearTimeout(handle);
  }, [query]);

  const { moments, spoken } = results;
  const isEmpty = moments.length === 0 && spoken.length === 0 && nameMatches.length === 0;

  return (
    <div style={{ flex: 1, overflowY: "auto", display: "flex", flexDirection: "column" }}>
      <SearchIndexAffordance phase={phase} />

      {moments.length > 0 && (
        <Group icon={Film} label={t("search.group.moments")} count={moments.length}>
          <ResultsGrid>
            {moments.map((hit, i) => (
              <MomentCard key={`${hit.mediaId}:${hit.frame}:${i}`} hit={hit} />
            ))}
          </ResultsGrid>
        </Group>
      )}

      {spoken.length > 0 && (
        <Group icon={Mic} label={t("search.group.spoken")} count={spoken.length}>
          <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-xs)", padding: "0 var(--space-sm) var(--space-sm)" }}>
            {spoken.map((hit, i) => (
              <SpokenRow key={`${hit.mediaId}:${hit.startSec}:${i}`} hit={hit} />
            ))}
          </div>
        </Group>
      )}

      {nameMatches.length > 0 && (
        <Group icon={FileText} label={t("search.group.files")} count={nameMatches.length}>
          <ResultsGrid>
            {nameMatches.map((item) => (
              <FileCard key={item.id} item={item} />
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
function useSearchIndexPhase(hasIndexableAssets: boolean): IndexPhase {
  const [phase, setPhase] = useState<IndexPhase>({ kind: "hidden" });
  const mediaCount = useMediaStore((s) => s.items.length);

  // Re-evaluate whenever the library changes (new assets → maybe need indexing).
  const refresh = useCallback(async () => {
    const status = await searchIndexStatus();
    if (!status.modelInstalled) {
      setPhase(hasIndexableAssets ? { kind: "needsModel" } : { kind: "hidden" });
      return;
    }
    if (status.indexable > 0 && status.indexed < status.indexable) {
      setPhase({ kind: "readyToIndex" });
    } else {
      setPhase({ kind: "hidden" });
    }
  }, [hasIndexableAssets]);

  useEffect(() => {
    let cancelled = false;
    void refresh().catch(() => {
      if (!cancelled) setPhase({ kind: "hidden" });
    });
    return () => {
      cancelled = true;
    };
  }, [refresh, mediaCount]);

  // Live download + indexing progress events keep the ring moving.
  useEffect(() => {
    let offDownload = () => {};
    let offIndex = () => {};
    void onSearchModelProgress((fraction) => {
      setPhase((p) =>
        p.kind === "downloading" || p.kind === "needsModel" ? { kind: "downloading", fraction } : p,
      );
    }).then((off) => (offDownload = off));
    void onSearchIndexProgress(({ completed, total, fraction }) => {
      if (total === 0) {
        void refresh();
        return;
      }
      setPhase({ kind: "indexing", done: completed, total, fraction });
      // On the final tick, settle back to the resting state.
      if (completed >= total) void refresh();
    }).then((off) => (offIndex = off));
    return () => {
      offDownload();
      offIndex();
    };
  }, [refresh]);

  // Expose setters through a module ref so the button handlers can drive it.
  phaseSetterRef.current = setPhase;
  return phase;
}

/** Lets the affordance's buttons flip the phase optimistically before the async
 *  command's first progress event lands (module ref — avoids prop threading). */
const phaseSetterRef: { current: ((p: IndexPhase) => void) | null } = { current: null };

/** The status affordance: a download/enable button (no model) or a progress ring
 *  (downloading / indexing). Hidden when nothing needs attention (upstream
 *  `MediaTab+IndexStatus.swift`). */
function SearchIndexAffordance({ phase }: { phase: IndexPhase }) {
  const t = useT();

  const onDownload = useCallback(() => {
    phaseSetterRef.current?.({ kind: "downloading", fraction: 0 });
    void downloadSearchModel()
      .then(() => phaseSetterRef.current?.({ kind: "readyToIndex" }))
      .catch(() => phaseSetterRef.current?.({ kind: "failed" }));
  }, []);

  const onIndex = useCallback(() => {
    phaseSetterRef.current?.({ kind: "indexing", done: 0, total: 1, fraction: 0 });
    void searchIndexStart()
      .then(() => phaseSetterRef.current?.({ kind: "hidden" }))
      .catch(() => phaseSetterRef.current?.({ kind: "hidden" }));
  }, []);

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
    return (
      <button
        type="button"
        onClick={onDownload}
        title={t("search.smartSearchHint", { size: formatModelBytes(0) })}
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
    return (
      <button
        type="button"
        onClick={onDownload}
        title={t("search.retryHint")}
        style={{ ...barStyle, cursor: "pointer", textAlign: "left", color: "var(--status-error)" }}
      >
        <Icon icon={AlertTriangle} size={13} />
        <span style={{ fontWeight: "var(--fw-medium)" }}>{t("search.retry")}</span>
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
  alt,
  thumbnailRef,
}: {
  mediaId: string;
  timeSec: number;
  alt: string;
  thumbnailRef?: React.Ref<HTMLDivElement>;
}) {
  const [path, setPath] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    void generateThumbnail(mediaId, { timeSecs: timeSec, includeSprite: false }).then((r) => {
      if (!cancelled) setPath(r?.thumbnailPath ?? null);
    });
    return () => {
      cancelled = true;
    };
  }, [mediaId, timeSec]);
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

/** A visual "Moments" card: frame thumb + name + timecode range, draggable to the
 *  timeline as a trimmed source-range clip (upstream `momentCard`). */
function MomentCard({ hit }: { hit: MomentHit }) {
  const t = useT();
  const item = useMediaItem(hit.mediaId);
  const fps = useProjectStore((s) => s.timeline.fps);
  const setPreviewMedia = useEditorUiStore((s) => s.setPreviewMedia);
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
      draggable
      onDragStart={onDragStart}
      onDragEnd={onDragEnd}
      onClick={() => {
        setPreviewMedia(item.id);
        void preloadMedia(item.id);
      }}
      title={t("search.dragToTimeline")}
      style={{ display: "flex", flexDirection: "column", gap: 3, cursor: "grab" }}
    >
      <HitThumbnail
        mediaId={hit.mediaId}
        timeSec={hit.startSec}
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
    </div>
  );
}

/** A "Spoken" transcript row: thumb + text + name·timecode, draggable as a
 *  trimmed range (upstream `spokenRow`). */
function SpokenRow({ hit }: { hit: SpokenHit }) {
  const t = useT();
  const item = useMediaItem(hit.mediaId);
  const fps = useProjectStore((s) => s.timeline.fps);
  const setPreviewMedia = useEditorUiStore((s) => s.setPreviewMedia);
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
      draggable
      onDragStart={onDragStart}
      onDragEnd={onDragEnd}
      onClick={() => {
        setPreviewMedia(item.id);
        void preloadMedia(item.id);
      }}
      title={t("search.dragToTimeline")}
      style={{
        display: "flex",
        gap: "var(--space-sm)",
        cursor: "grab",
        alignItems: "flex-start",
      }}
    >
      <div style={{ width: 96, flex: "0 0 auto" }}>
        <HitThumbnail
          mediaId={hit.mediaId}
          timeSec={hit.startSec}
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
    </div>
  );
}

/** A "Files" name-match card: thumb + name, draggable as the whole asset (the
 *  pre-existing behavior; upstream `fileCard`). */
function FileCard({ item }: { item: MediaItem }) {
  const setPreviewMedia = useEditorUiStore((s) => s.setPreviewMedia);
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
      draggable
      onDragStart={onDragStart}
      onDragEnd={onDragEnd}
      onClick={() => {
        setPreviewMedia(item.id);
        void preloadMedia(item.id);
      }}
      onDoubleClick={() => void addMediaToTimeline(item)}
      title={item.name}
      style={{ display: "flex", flexDirection: "column", gap: 3, cursor: "grab" }}
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
    </div>
  );
}
