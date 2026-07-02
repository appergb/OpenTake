/**
 * MediaPanel (SPEC §7 + 剪映式顶栏改造)。顶部横排主标签（素材/音频/文本/贴纸/
 * 特效/转场/字幕/智能包裹，仅素材/音频可用，其余置灰占位）取代了原左侧竖排
 * Media/Captions/Music 标签条。素材/音频下再分「导入 / 我的」二级标签：导入=全部
 * （音频标签仅 type==='audio'），我的=星标收藏（localStorage 持久化，见 favorites.ts）。
 * 内容区仍是 actions/search/context 工具栏 + 资产网格；网格项 HTML5-draggable 到
 * 时间线（见 `MediaGrid` / `TimelineRegion`）。
 */

import { useEffect, useMemo, useRef, useState } from "react";
import {
  Plus,
  Sparkles,
  Filter,
  ArrowUpDown,
  LayoutGrid,
  FolderOpen,
  Folder as FolderIcon,
  ChevronRight,
  ChevronLeft,
  FileVideo,
  FileAudio,
  Image as ImageIcon,
  Type as TypeIcon,
  AlertTriangle,
  Star,
} from "lucide-react";
import { Icon } from "../ui/Icon";
import { HoverButton } from "../ui/HoverButton";
import { useEditorUiStore, type MediaSubTabId } from "../../store/uiStore";
import { useMediaStore } from "../../store/mediaStore";
import {
  importFolderViaDialog,
  importFilesViaDialog,
  relinkMediaViaDialog,
} from "../../store/mediaActions";
import { useT } from "../../i18n";
import { formatTimecode } from "../../lib/geometry";
import { setDraggingMedia } from "../../lib/mediaDragState";
import { assetUrl } from "../../lib/asset";
import { BoundedCache } from "../../lib/lru";
import { childFolders, folderTrail, normalizeFolderId } from "../../lib/folderTree";
import { useProjectStore } from "../../store/projectStore";
import { addMediaToTimeline } from "../../store/editActions";
import { extractAudio, generateThumbnail, preloadMedia } from "../../lib/api";
import { saveDialog } from "../../lib/dialog";
import type { MediaFolder, MediaItem } from "../../lib/types";
import { MediaTabBar, MediaSubTabBar } from "./MediaTabBar";
import { useFavoritesStore, useIsFavorite } from "./favorites";

/** MIME-ish type used on dataTransfer when dragging a media item to the timeline. */
export const MEDIA_DND_TYPE = "application/x-opentake-media";
const MEDIA_THUMBNAIL_CONCURRENCY = 4;
/** Bound for the in-memory thumbnail-path cache. A long library scrolled top to
 *  bottom would otherwise grow this Map without limit; cap it (LRU) so memory
 *  stays bounded — evicted keys just re-request a (disk-cached) path later. */
const MEDIA_THUMBNAIL_CACHE_MAX = 256;

let activeThumbnailRequests = 0;
const pendingThumbnailRequests: Array<() => void> = [];
const mediaThumbnailInFlight = new Map<string, Promise<string | null>>();

/** Bounded LRU over the resolved thumbnail paths, so a long library scrolled top
 *  to bottom can't grow memory without limit (see {@link BoundedCache}). */
const mediaThumbnailCache = new BoundedCache<string | null>(MEDIA_THUMBNAIL_CACHE_MAX);

function runNextThumbnailRequest(): void {
  if (activeThumbnailRequests >= MEDIA_THUMBNAIL_CONCURRENCY) return;
  const next = pendingThumbnailRequests.shift();
  if (!next) return;
  activeThumbnailRequests += 1;
  next();
}

function enqueueThumbnailRequest(task: () => Promise<string | null>): Promise<string | null> {
  return new Promise((resolve, reject) => {
    pendingThumbnailRequests.push(() => {
      task()
        .then(resolve, reject)
        .finally(() => {
          activeThumbnailRequests = Math.max(0, activeThumbnailRequests - 1);
          runNextThumbnailRequest();
        });
    });
    runNextThumbnailRequest();
  });
}

function mediaThumbnailKey(item: MediaItem): string {
  return `${item.id}|${item.path ?? ""}|${item.thumbnail ?? ""}|${item.missing ? "missing" : "online"}`;
}

function requestMediaCardThumbnail(item: MediaItem): Promise<string | null> {
  const key = mediaThumbnailKey(item);
  if (mediaThumbnailCache.has(key)) return Promise.resolve(mediaThumbnailCache.get(key) ?? null);
  const inFlight = mediaThumbnailInFlight.get(key);
  if (inFlight) return inFlight;
  const promise = enqueueThumbnailRequest(async () => {
    const result = await generateThumbnail(item.id, { includeSprite: false });
    return result?.thumbnailPath ?? null;
  })
    .then((path) => {
      mediaThumbnailCache.set(key, path);
      return path;
    })
    .finally(() => {
      mediaThumbnailInFlight.delete(key);
    });
  mediaThumbnailInFlight.set(key, promise);
  return promise;
}

/** 当前已实现内容的两个主标签；其余标签在 MediaTabBar 中置灰、点不到。 */
type MediaTabKind = "material" | "audio";

export function MediaPanel() {
  const mediaTab = useEditorUiStore((s) => s.mediaTab);
  const setMediaTab = useEditorUiStore((s) => s.setMediaTab);
  const t = useT();

  // 仅 material/audio 渲染素材库内容；其余禁用标签理论上点不到，兜底显示占位。
  const isLibraryTab = mediaTab === "material" || mediaTab === "audio";

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%", width: "100%" }}>
      <MediaTabBar active={mediaTab} onSelect={setMediaTab} />
      {/* minHeight:0 lets the inner grid actually scroll instead of overflowing
          and pushing the whole panel (which hid the tab bar + killed scroll). */}
      <div style={{ flex: 1, minWidth: 0, minHeight: 0, display: "flex", flexDirection: "column" }}>
        {isLibraryTab ? (
          <MediaTab kind={mediaTab as MediaTabKind} />
        ) : (
          <Placeholder label={t(`media.tab.${mediaTab}`)} />
        )}
      </div>
    </div>
  );
}

function MediaTab({ kind }: { kind: MediaTabKind }) {
  const t = useT();
  const items = useMediaStore((s) => s.items);
  const folders = useMediaStore((s) => s.folders);
  const importing = useMediaStore((s) => s.importing);
  const error = useMediaStore((s) => s.error);
  const subTab = useEditorUiStore((s) => s.mediaSubTab);
  const setSubTab = useEditorUiStore((s) => s.setMediaSubTab);
  const favoriteIds = useFavoritesStore((s) => s.ids);
  const currentFolderId = useEditorUiStore((s) => s.mediaPanelCurrentFolderId);
  const setCurrentFolderId = useEditorUiStore((s) => s.setMediaPanelCurrentFolderId);
  const [search, setSearch] = useState("");

  // Folder navigation only applies to the "import" view (the full library tree).
  // "我的/favorites" is a flat cross-folder collection, so it ignores folders.
  const browsing = subTab === "import";

  // Switching the main tab (material↔audio) or to the favorites subtab resets the
  // folder cursor to root so we never sit inside a folder that the new view hides.
  // Depends only on kind/subTab on purpose; the setter is store-stable and
  // currentFolderId must not retrigger this (it would fight manual navigation).
  const resetFolder = useRef(setCurrentFolderId);
  resetFolder.current = setCurrentFolderId;
  useEffect(() => {
    resetFolder.current(null);
  }, [kind, subTab]);

  // Effective cursor: favorites view is always flat (root).
  const folderId = browsing ? currentFolderId : null;
  const query = search.trim().toLowerCase();

  // Sub-folders shown as tiles in the current level (only while browsing, and
  // not while a search is active — search flattens to matching files).
  const visibleFolders = useMemo(
    () => (browsing && query === "" ? childFolders(folders, folderId) : []),
    [browsing, query, folders, folderId],
  );

  // File filter pipeline (all immutable filters; never mutates the store):
  // 1) main tab — "音频" keeps only pure audio (strict type==='audio', no
  //    audio-bearing video, matching CapCut). "素材" shows every type.
  // 2) subtab — "我的" keeps only starred favorites; "导入" shows all.
  // 3) folder — while browsing without a search, only this folder's direct
  //    files. A search ignores folder scope and matches names library-wide
  //    (within the current main/subtab filter).
  const filteredItems = useMemo(
    () =>
      items.filter((item) => {
        if (kind === "audio" && item.type !== "audio") return false;
        if (subTab === "mine" && !favoriteIds.has(item.id)) return false;
        if (query !== "") return item.name.toLowerCase().includes(query);
        if (browsing && normalizeFolderId(item.folderId) !== folderId) return false;
        return true;
      }),
    [items, kind, subTab, favoriteIds, query, browsing, folderId],
  );

  const trail = useMemo(() => folderTrail(folders, folderId), [folders, folderId]);
  const totalCount = visibleFolders.length + filteredItems.length;
  const isEmpty = totalCount === 0;

  return (
    <>
      {/* Toolbar (fixed height; only the grid below scrolls) */}
      <div
        style={{
          flex: "0 0 auto",
          display: "flex",
          flexDirection: "column",
          gap: "var(--space-xs)",
          padding: "var(--space-sm) var(--space-sm) var(--space-xs)",
          background: "var(--bg-surface)",
        }}
      >
        {/* actionsRow */}
        <div style={{ height: 28, display: "flex", alignItems: "center", gap: "var(--space-xs)" }}>
          <ImportMenu />
          <button
            title={t("media.generate")}
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: 4,
              height: 24,
              padding: "0 8px",
              borderRadius: "var(--radius-sm)",
              background: "var(--ai-gradient)",
              color: "#111",
              fontSize: "var(--fs-sm)",
              fontWeight: "var(--fw-medium)",
            }}
          >
            <Icon icon={Sparkles} size={12} />
            {t("media.generate")}
          </button>
          <div style={{ flex: 1 }} />
          {/* 二级标签：导入 / 我的（星标收藏）。 */}
          <MediaSubTabBar active={subTab} onSelect={setSubTab} />
        </div>
        {/* searchControlsRow */}
        <div style={{ height: 28, display: "flex", alignItems: "center", gap: "var(--space-xs)" }}>
          <input
            placeholder={t("media.search")}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            style={{
              flex: 1,
              height: 22,
              background: "var(--bg-raised)",
              border: "var(--bw-thin) solid var(--border-primary)",
              borderRadius: "var(--radius-sm)",
              color: "var(--text-primary)",
              fontSize: "var(--fs-sm)",
              padding: "0 8px",
            }}
          />
          <HoverButton title={t("media.viewMode")}>
            <Icon icon={LayoutGrid} size={13} />
          </HoverButton>
          <HoverButton title={t("media.sort")}>
            <Icon icon={ArrowUpDown} size={13} />
          </HoverButton>
          <HoverButton title={t("media.filter")}>
            <Icon icon={Filter} size={13} />
          </HoverButton>
        </div>
        {/* Breadcrumb / 返回上级 — only while browsing the library tree and not
            searching. Root is always clickable; the current folder is plain text. */}
        {browsing && query === "" && (
          <FolderBreadcrumb trail={trail} onNavigate={setCurrentFolderId} />
        )}
        {/* contextBar */}
        <div
          style={{
            height: "var(--context-row-height)",
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            color: "var(--text-tertiary)",
            fontSize: "var(--fs-xs)",
          }}
        >
          <span>{t("media.library")}</span>
          <span>{importing ? t("media.importing") : t("media.itemCount", { count: totalCount })}</span>
        </div>
        {error && (
          <div style={{ color: "var(--status-error)", fontSize: "var(--fs-xs)" }}>
            {t("media.importFailed", { error })}
          </div>
        )}
      </div>

      {isEmpty ? (
        <EmptyState subTab={subTab} insideFolder={browsing && folderId !== null} />
      ) : (
        <MediaGrid
          folders={visibleFolders}
          items={filteredItems}
          onOpenFolder={setCurrentFolderId}
        />
      )}
    </>
  );
}

/** Breadcrumb row: 全部 / 子文件夹… / 当前。 Every segment except the last is a
 *  button that jumps to that level; a back chevron pops up one level. */
function FolderBreadcrumb({
  trail,
  onNavigate,
}: {
  trail: MediaFolder[];
  onNavigate: (id: string | null) => void;
}) {
  const t = useT();
  const atRoot = trail.length === 0;
  const parentId = trail.length >= 2 ? trail[trail.length - 2].id : null;

  const crumbButton = (label: string, target: string | null, isLast: boolean) =>
    isLast ? (
      <span
        key={target ?? "__root__"}
        style={{
          color: "var(--text-primary)",
          fontWeight: "var(--fw-medium)",
          whiteSpace: "nowrap",
          overflow: "hidden",
          textOverflow: "ellipsis",
        }}
      >
        {label}
      </span>
    ) : (
      <button
        key={target ?? "__root__"}
        type="button"
        onClick={() => onNavigate(target)}
        className="hover-area"
        style={{
          background: "transparent",
          border: "none",
          padding: "0 2px",
          color: "var(--text-secondary)",
          fontSize: "var(--fs-xs)",
          cursor: "pointer",
          whiteSpace: "nowrap",
        }}
      >
        {label}
      </button>
    );

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 2,
        minHeight: 22,
        overflowX: "auto",
        overflowY: "hidden",
      }}
    >
      {/* 返回上级（仅非根时）。 */}
      {!atRoot && (
        <button
          type="button"
          title={t("media.folderBack")}
          aria-label={t("media.folderBack")}
          onClick={() => onNavigate(parentId)}
          className="hover-area"
          style={{
            display: "inline-flex",
            alignItems: "center",
            justifyContent: "center",
            width: 20,
            height: 20,
            marginRight: 2,
            borderRadius: "var(--radius-xs)",
            background: "transparent",
            border: "none",
            color: "var(--text-secondary)",
            cursor: "pointer",
            flex: "0 0 auto",
          }}
        >
          <Icon icon={ChevronLeft} size={14} />
        </button>
      )}
      {crumbButton(t("media.folderRoot"), null, atRoot)}
      {trail.map((folder, i) => {
        const isLast = i === trail.length - 1;
        return (
          <span
            key={folder.id}
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: 2,
              color: "var(--text-tertiary)",
              flex: "0 0 auto",
            }}
          >
            <Icon icon={ChevronRight} size={12} />
            {crumbButton(folder.name, folder.id, isLast)}
          </span>
        );
      })}
    </div>
  );
}

/** Import button with a small folder/files menu (CapCut-style folder import). */
function ImportMenu() {
  const t = useT();
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) setOpen(false);
    };
    window.addEventListener("mousedown", onDown);
    return () => window.removeEventListener("mousedown", onDown);
  }, [open]);

  return (
    <div ref={rootRef} style={{ position: "relative", display: "inline-flex" }}>
      <HoverButton title={t("media.importHint")} active={open} onClick={() => setOpen((v) => !v)}>
        <Icon icon={Plus} size={13} />
      </HoverButton>
      {open && (
        <div
          role="menu"
          style={{
            position: "absolute",
            top: "calc(100% + 6px)",
            left: 0,
            minWidth: 168,
            padding: "var(--space-xs)",
            background: "var(--bg-raised)",
            border: "var(--bw-thin) solid var(--border-primary)",
            borderRadius: "var(--radius-md)",
            boxShadow: "var(--shadow-lg)",
            zIndex: 200,
          }}
        >
          <ImportMenuItem
            icon={FolderOpen}
            label={t("media.importFolder")}
            onClick={() => {
              setOpen(false);
              void importFolderViaDialog();
            }}
          />
          <ImportMenuItem
            icon={Plus}
            label={t("media.importFiles")}
            onClick={() => {
              setOpen(false);
              void importFilesViaDialog();
            }}
          />
        </div>
      )}
    </div>
  );
}

function ImportMenuItem({
  icon,
  label,
  onClick,
}: {
  icon: typeof Plus;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      role="menuitem"
      onClick={onClick}
      className="hover-area"
      style={{
        width: "100%",
        display: "flex",
        alignItems: "center",
        gap: "var(--space-sm)",
        height: 28,
        padding: "0 var(--space-sm)",
        borderRadius: "var(--radius-sm)",
        color: "var(--text-secondary)",
        fontSize: "var(--fs-sm)",
        fontWeight: "var(--fw-medium)",
        textAlign: "left",
      }}
    >
      <Icon icon={icon} size={13} />
      <span style={{ flex: 1 }}>{label}</span>
    </button>
  );
}

function EmptyState({ subTab, insideFolder }: { subTab: MediaSubTabId; insideFolder: boolean }) {
  const t = useT();
  const message = insideFolder
    ? t("media.folderEmpty")
    : subTab === "mine"
      ? t("media.empty.mine")
      : t("media.empty");
  return (
    <div
      style={{
        flex: 1,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        color: "var(--text-muted)",
        fontSize: "var(--fs-sm-md)",
        padding: "var(--space-xl)",
        textAlign: "center",
      }}
    >
      {message}
    </div>
  );
}

const TYPE_ICON: Record<MediaItem["type"], typeof FileVideo> = {
  video: FileVideo,
  audio: FileAudio,
  image: ImageIcon,
  text: TypeIcon,
  lottie: Sparkles,
};

function MediaGrid({
  folders,
  items,
  onOpenFolder,
}: {
  folders: MediaFolder[];
  items: MediaItem[];
  onOpenFolder: (id: string) => void;
}) {
  return (
    <div
      style={{
        flex: 1,
        overflowY: "auto",
        display: "grid",
        gridTemplateColumns: "repeat(auto-fill, minmax(96px, 1fr))",
        gap: "var(--space-sm)",
        padding: "var(--space-sm)",
        alignContent: "start",
      }}
    >
      {/* Folders first (双击进入), then files. */}
      {folders.map((folder) => (
        <FolderTile key={folder.id} folder={folder} onOpen={onOpenFolder} />
      ))}
      {items.map((item) => (
        <MediaCard key={item.id} item={item} />
      ))}
    </div>
  );
}

/** A folder shown in the grid (剪映式). Single click selects/enters on
 *  double-click — keeping it consistent with media cards (click=preview,
 *  double-click=add). Not draggable (folders aren't dropped on the timeline). */
function FolderTile({
  folder,
  onOpen,
}: {
  folder: MediaFolder;
  onOpen: (id: string) => void;
}) {
  const [hovered, setHovered] = useState(false);
  return (
    <div
      onDoubleClick={() => onOpen(folder.id)}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      title={folder.name}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onOpen(folder.id);
        }
      }}
      style={{ display: "flex", flexDirection: "column", gap: 4, cursor: "pointer" }}
    >
      <div
        style={{
          aspectRatio: "5 / 4",
          background: hovered ? "var(--bg-raised)" : "var(--bg-placeholder)",
          border: `var(--bw-thin) solid ${hovered ? "var(--accent-primary)" : "var(--border-primary)"}`,
          borderRadius: "var(--radius-sm)",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: hovered ? "var(--accent-primary)" : "var(--text-secondary)",
        }}
      >
        <Icon icon={FolderIcon} size={30} strokeWidth={1.5} />
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
        {folder.name}
      </span>
    </div>
  );
}

function MediaCard({ item }: { item: MediaItem }) {
  const t = useT();
  const cardRef = useRef<HTMLDivElement | null>(null);
  const fps = useProjectStore((s) => s.timeline.fps);
  const setPreviewMedia = useEditorUiStore((s) => s.setPreviewMedia);
  const previewMediaId = useEditorUiStore((s) => s.previewMediaId);
  const durationFrames = Math.round(item.duration * fps);
  const selected = previewMediaId === item.id;
  const favorite = useIsFavorite(item.id);
  const toggleFavorite = useFavoritesStore((s) => s.toggle);
  const thumbnailKey = mediaThumbnailKey(item);
  const [lazyThumbnail, setLazyThumbnail] = useState<string | null>(
    item.thumbnail ?? mediaThumbnailCache.get(thumbnailKey) ?? null,
  );
  // Offline assets shouldn't try to load a (now-missing) thumbnail.
  const thumb = item.missing ? null : assetUrl(lazyThumbnail);
  const [hovered, setHovered] = useState(false);
  const [feedback, setFeedback] = useState<string | null>(null);

  useEffect(() => {
    setLazyThumbnail(item.thumbnail ?? mediaThumbnailCache.get(thumbnailKey) ?? null);
  }, [item.thumbnail, thumbnailKey]);

  useEffect(() => {
    if (item.missing || item.thumbnail || (item.type !== "video" && item.type !== "image")) {
      return;
    }
    let cancelled = false;
    const request = () => {
      void requestMediaCardThumbnail(item).then((path) => {
        if (!cancelled && path) setLazyThumbnail(path);
      });
    };
    const el = cardRef.current;
    if (!el || typeof IntersectionObserver === "undefined") {
      request();
      return () => {
        cancelled = true;
      };
    }
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (!entry?.isIntersecting) return;
        observer.disconnect();
        request();
      },
      { root: null, rootMargin: "160px" },
    );
    observer.observe(el);
    return () => {
      cancelled = true;
      observer.disconnect();
    };
  }, [item, thumbnailKey]);

  // Page-aware preview pre-warm: when a VIDEO card scrolls into view, warm its
  // hi-res first-frame poster so a click previews near-instantly. Gated by the
  // same IntersectionObserver as the thumbnail, so cards scrolled far out of
  // view are never warmed (and we don't warm images/audio — nothing to decode).
  useEffect(() => {
    if (item.missing || item.type !== "video") return;
    const el = cardRef.current;
    if (!el || typeof IntersectionObserver === "undefined") return;
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (!entry?.isIntersecting) return;
        observer.disconnect();
        void preloadMedia(item.id);
      },
      { root: null, rootMargin: "160px" },
    );
    observer.observe(el);
    return () => observer.disconnect();
  }, [item.id, item.type, item.missing]);

  const onDragStart = (e: React.DragEvent) => {
    e.dataTransfer.setData(MEDIA_DND_TYPE, item.id);
    e.dataTransfer.effectAllowed = "copy";
    // Stash the item so the timeline can size its drop ghost during dragover
    // (dataTransfer payloads are unreadable until drop). Cleared on dragEnd.
    setDraggingMedia(item);
    // Warm caches for a dragged-but-not-clicked asset too (best-effort).
    void preloadMedia(item.id);
  };

  const onDragEnd = () => {
    setDraggingMedia(null);
  };

  /** Extract the audio track into a standalone file via ffmpeg. Opens a native
   *  save dialog (m4a/mp3/wav), then calls the `extract_audio` Tauri command.
   *  Only shown for video assets that carry audio (Issue #39). */
  const onExtractAudio = async (e: React.MouseEvent) => {
    e.stopPropagation();
    e.preventDefault();
    const save = await saveDialog();
    if (!save) return; // non-Tauri / dialog unavailable
    const chosen = await save({
      title: t("media.extractAudio"),
      defaultPath: `${item.name}.m4a`,
      filters: [
        { name: "Audio (M4A)", extensions: ["m4a"] },
        { name: "Audio (MP3)", extensions: ["mp3"] },
        { name: "Audio (WAV)", extensions: ["wav"] },
      ],
    });
    if (typeof chosen !== "string") return; // user cancelled
    setFeedback(null);
    try {
      const out = await extractAudio(item.id, chosen);
      setFeedback(t("media.extractAudioSuccess", { path: out }));
    } catch (err) {
      setFeedback(t("media.extractAudioFailed", { error: String(err) }));
    }
    setTimeout(() => setFeedback(null), 4000);
  };

  // Only local, present video assets with an audio track can be extracted.
  const canExtractAudio = item.type === "video" && item.hasAudio && !item.missing;

  return (
    <div
      ref={cardRef}
      draggable
      onDragStart={onDragStart}
      onDragEnd={onDragEnd}
      onClick={() => {
        setPreviewMedia(item.id);
        // Warm poster/sprite/waveform caches so preview + a later timeline drop
        // are instant instead of decoding on the interaction path.
        void preloadMedia(item.id);
      }}
      onDoubleClick={() => void addMediaToTimeline(item)}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      title={item.name}
      style={{ display: "flex", flexDirection: "column", gap: 4, cursor: "grab" }}
    >
      {/* Thumbnail: generated cache image only. Missing thumbnails are requested
          lazily as cards enter view, so import/list commands stay cheap. */}
      <div
        style={{
          position: "relative",
          aspectRatio: "5 / 4",
          background: "var(--bg-placeholder)",
          border: `var(--bw-thin) solid ${item.missing ? "rgb(255,59,48)" : selected ? "var(--accent-primary)" : "var(--border-primary)"}`,
          borderRadius: "var(--radius-sm)",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: "var(--text-muted)",
          overflow: "hidden",
        }}
      >
        {/* `draggable={false}` on the inner media so the card's custom drag
            (MEDIA_DND_TYPE) wins instead of a native image drag. */}
        {thumb ? (
          <img
            src={thumb}
            alt={item.name}
            draggable={false}
            style={{ width: "100%", height: "100%", objectFit: "cover" }}
          />
        ) : (
          <Icon icon={TYPE_ICON[item.type]} size={22} strokeWidth={1.5} />
        )}
        {item.duration > 0 && (
          <span
            className="tabular"
            style={{
              position: "absolute",
              right: 4,
              bottom: 4,
              padding: "0 4px",
              borderRadius: "var(--radius-xs)",
              background: "rgba(0,0,0,0.6)",
              color: "var(--text-secondary)",
              fontSize: "var(--fs-micro)",
              fontWeight: "var(--fw-medium)",
            }}
          >
            {formatTimecode(durationFrames, fps)}
          </span>
        )}
        {/* Offline overlay: the source file is missing. Relink keeps the asset
            id, so the timeline clips referencing it recover (no re-import). */}
        {item.missing && (
          <div
            onClick={(e) => e.stopPropagation()}
            style={{
              position: "absolute",
              inset: 0,
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              justifyContent: "center",
              gap: 4,
              background: "rgba(255,59,48,0.32)",
              color: "#fff",
              textAlign: "center",
              padding: 4,
            }}
          >
            <Icon icon={AlertTriangle} size={18} />
            <span style={{ fontSize: "var(--fs-micro)", fontWeight: "var(--fw-medium)" }}>
              {t("media.offline")}
            </span>
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                void relinkMediaViaDialog(item.id);
              }}
              style={{
                fontSize: "var(--fs-micro)",
                fontWeight: "var(--fw-medium)",
                padding: "2px 8px",
                borderRadius: "var(--radius-xs)",
                background: "rgba(0,0,0,0.55)",
                color: "#fff",
                cursor: "pointer",
              }}
            >
              {t("media.relink")}
            </button>
          </div>
        )}
        {/* 星标收藏按钮（左上角）。stopPropagation 避免触发卡片的预览/拖拽。
            渲染在 missing 覆盖层之后并给更高 zIndex，确保离线素材仍可取消收藏。 */}
        <button
          type="button"
          aria-pressed={favorite}
          title={favorite ? t("media.unfavorite") : t("media.favorite")}
          onClick={(e) => {
            e.stopPropagation();
            toggleFavorite(item.id);
          }}
          style={{
            position: "absolute",
            left: 4,
            top: 4,
            zIndex: 2,
            display: "inline-flex",
            alignItems: "center",
            justifyContent: "center",
            width: 20,
            height: 20,
            padding: 0,
            borderRadius: "var(--radius-xs)",
            background: "rgba(0,0,0,0.6)",
            color: favorite ? "var(--accent-timecode)" : "var(--text-secondary)",
            cursor: "pointer",
          }}
        >
          <Icon icon={Star} size={12} strokeWidth={2} fill={favorite ? "currentColor" : "none"} />
        </button>
        {canExtractAudio && hovered && (
          <button
            type="button"
            title={t("media.extractAudioHint")}
            aria-label={t("media.extractAudio")}
            onClick={onExtractAudio}
            className="hover-area"
            style={{
              position: "absolute",
              right: 4,
              top: 4,
              zIndex: 3,
              width: 20,
              height: 20,
              display: "inline-flex",
              alignItems: "center",
              justifyContent: "center",
              borderRadius: "var(--radius-xs)",
              background: "rgba(0,0,0,0.6)",
              color: "var(--text-secondary)",
              cursor: "pointer",
            }}
          >
            <Icon icon={FileAudio} size={12} />
          </button>
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
      {feedback && (
        <span
          style={{
            fontSize: "var(--fs-micro)",
            color: "var(--text-tertiary)",
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {feedback}
        </span>
      )}
    </div>
  );
}

function Placeholder({ label }: { label: string }) {
  return (
    <div
      style={{
        flex: 1,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        color: "var(--text-muted)",
        fontSize: "var(--fs-sm-md)",
      }}
    >
      {label}
    </div>
  );
}
