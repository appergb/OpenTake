/**
 * MediaPanel (SPEC §7 + 剪映式顶栏改造)。顶部横排主标签（素材/音频/文本/贴纸/
 * 特效/转场/字幕/智能包裹；素材/音频/音乐/文本/字幕已接真实内容）取代了原左侧竖排
 * Media/Captions/Music 标签条。素材/音频下再分「导入 / 我的」二级标签：导入=当前
 * 项目素材（音频标签仅 type==='audio'），我的=跨项目全局收藏库。
 * 内容区仍是 actions/search/context 工具栏 + 资产网格；网格项 HTML5-draggable 到
 * 时间线（见 `MediaGrid` / `TimelineRegion`）。
 */

import { useEffect, useId, useMemo, useRef, useState } from "react";
import {
  Plus,
  Sparkles,
  Filter,
  ArrowUpDown,
  LayoutGrid,
  List,
  Check,
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
import {
  applyMediaErrorForProject,
  applyMediaListForProject,
  captureMediaProjectIdentity,
  isCurrentMediaProject,
  useMediaStore,
  type MediaProjectIdentity,
} from "../../store/mediaStore";
import { sourceName, useLibraryStore } from "../../store/libraryStore";
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
import {
  projectMediaView,
  type MediaOrganizationMode,
  type MediaViewGroup,
} from "../../lib/mediaViewModes";
import {
  derivedResourceKinds,
  derivedResourceScheduler,
} from "../../lib/derivedResourceScheduler";
import { folderTrail } from "../../lib/folderTree";
import { useProjectStore } from "../../store/projectStore";
import {
  addMediaToTimeline,
  addTextClip,
  reportMediaPlacementFailure,
  setEffects,
} from "../../store/editActions";
import {
  deleteFolderFromContextMenu,
  deleteMediaFromContextMenu,
  deleteSelectedFolders,
  deleteSelectedMediaAssets,
} from "../../store/mediaDeleteActions";
import {
  cancelGeneration,
  retryGeneration,
  extractAudio,
  generateThumbnail,
  getWaveform,
  preloadMedia,
  toggleFavorite,
} from "../../lib/api";
import { saveDialog } from "../../lib/dialog";
import type { MediaFolder, MediaItem } from "../../lib/types";
import {
  AUDIO_SUB_TABS,
  MATERIAL_SUB_TABS,
  MEDIA_MAIN_TAB_IDS,
  MediaSubTabBar,
  MediaTabBar,
} from "./MediaTabBar";
import { SoundLibraryTab } from "./SoundLibraryTab";
import { MusicTab } from "./MusicTab";
import { TransitionTab } from "./TransitionTab";
import { CaptionsTab } from "./CaptionsTab";
import { SmartPackTab } from "./SmartPackTab";
import { MediaSearchResults } from "./MediaSearch";
import { applyFavoriteMigrationOutcome, migrateLocalFavorites } from "./favorites";
import { LibraryEntryGrid } from "./LibraryView";
import { EFFECT_REGISTRY, newAdvertisedEffect, type AdvertisedEffectName } from "../../lib/effects";

/** MIME-ish type used on dataTransfer when dragging a media item to the timeline. */
export const MEDIA_DND_TYPE = "application/x-opentake-media";
/** Bound for the in-memory thumbnail-path cache. A long library scrolled top to
 *  bottom would otherwise grow this Map without limit; cap it (LRU) so memory
 *  stays bounded — evicted keys just re-request a (disk-cached) path later. */
const MEDIA_THUMBNAIL_CACHE_MAX = 256;
const MEDIA_DRAG_PREVIEW_WIDTH = 80;
const MEDIA_DRAG_PREVIEW_HEIGHT = 60;

/** Bounded LRU over the resolved thumbnail paths, so a long library scrolled top
 *  to bottom can't grow memory without limit (see {@link BoundedCache}). */
const mediaThumbnailCache = new BoundedCache<string | null>(MEDIA_THUMBNAIL_CACHE_MAX);

// ── 视图层展示状态（纯前端，不改媒体镜像）──────────────────────────────
// 排序键与类型筛选只作用于「已加载媒体列表」的渲染顺序/子集；数据本身仍是
// Rust 权威镜像（store 不动、不发后端命令）。
export type MediaViewLayout = "grid" | "list";
export type MediaSortKey = "default" | "name" | "duration" | "fileSize";
export type MediaTypeFilter = MediaItem["type"] | "all";

const SORT_OPTIONS: ReadonlyArray<{ id: MediaSortKey; labelKey: string }> = [
  { id: "default", labelKey: "media.sort.default" },
  { id: "name", labelKey: "media.sort.name" },
  { id: "duration", labelKey: "media.sort.duration" },
  { id: "fileSize", labelKey: "media.sort.fileSize" },
];

const TYPE_FILTER_OPTIONS: ReadonlyArray<{ id: MediaTypeFilter; labelKey: string }> = [
  { id: "all", labelKey: "media.filter.all" },
  { id: "video", labelKey: "media.filter.video" },
  { id: "audio", labelKey: "media.filter.audio" },
  { id: "image", labelKey: "media.filter.image" },
  { id: "text", labelKey: "media.filter.text" },
  { id: "lottie", labelKey: "media.filter.lottie" },
];

const ORGANIZATION_OPTIONS: ReadonlyArray<{
  id: MediaOrganizationMode;
  labelKey: string;
}> = [
  { id: "folder", labelKey: "media.organization.folder" },
  { id: "flat", labelKey: "media.organization.flat" },
  { id: "grouped", labelKey: "media.organization.grouped" },
];

/** 局部排序（ES2019 稳定排序，同值保持原顺序）。`default` 返回原数组引用。 */
export function sortMediaItems(items: MediaItem[], key: MediaSortKey): MediaItem[] {
  if (key === "default" || items.length < 2) return items;
  const sorted = [...items];
  switch (key) {
    case "name":
      sorted.sort((a, b) =>
        a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: "base" }),
      );
      break;
    case "duration":
      sorted.sort((a, b) => b.duration - a.duration);
      break;
    case "fileSize":
      // 缺失/离线素材没有 fileSize，按 -1 兜底排在最后。
      sorted.sort((a, b) => (b.fileSize ?? -1) - (a.fileSize ?? -1));
      break;
  }
  return sorted;
}

/** 局部类型筛选；`all` 返回原数组引用。 */
export function filterMediaByType(items: MediaItem[], type: MediaTypeFilter): MediaItem[] {
  if (type === "all") return items;
  return items.filter((item) => item.type === type);
}

/** Keep command-routing selection synchronized across every media surface.
 * Delete/Enter commands intentionally consume selectedMediaAssetIds. */
export function selectMediaAsset(mediaId: string): void {
  const ui = useEditorUiStore.getState();
  ui.focusPanel("media");
  ui.selectMediaAssets(new Set([mediaId]));
  useEditorUiStore.setState({ selectedFolderIds: new Set() });
}

/** Select an asset and make it the preview source when it is ready to decode. */
export function selectMediaForPreview(mediaId: string): void {
  selectMediaAsset(mediaId);
  useEditorUiStore.getState().setPreviewMedia(mediaId);
}

/** Select a folder without entering it so pointer and keyboard users receive a
 * visible selection step before Enter/double-click drills into the folder. */
export function selectMediaFolder(folderId: string): void {
  const ui = useEditorUiStore.getState();
  ui.focusPanel("media");
  ui.selectMediaAssets(new Set());
  ui.closeAllPreviewTabs();
  useEditorUiStore.setState({ selectedFolderIds: new Set([folderId]) });
}

/** Commit folder navigation and clear the selection that would otherwise stay
 * hidden after the folder grid changes. */
export function openMediaFolder(folderId: string, onOpen: (id: string) => void): void {
  const ui = useEditorUiStore.getState();
  ui.closeAllPreviewTabs();
  useEditorUiStore.setState({
    selectedMediaAssetIds: new Set(),
    selectedFolderIds: new Set(),
  });
  onOpen(folderId);
}

/** Replace the browser's default whole-card drag ghost (which includes the
 * filename) with a compact visual-only preview, matching the native app. */
export function setMediaThumbnailDragImage(
  dataTransfer: Pick<DataTransfer, "setDragImage">,
  thumbnail: HTMLElement,
): void {
  const preview = thumbnail.cloneNode(true) as HTMLElement;
  preview.querySelectorAll("span, button").forEach((node) => node.remove());
  preview.querySelectorAll("img").forEach((image) => image.setAttribute("alt", ""));
  preview.setAttribute("aria-hidden", "true");
  Object.assign(preview.style, {
    position: "fixed",
    left: "-1000px",
    top: "-1000px",
    width: `${MEDIA_DRAG_PREVIEW_WIDTH}px`,
    height: `${MEDIA_DRAG_PREVIEW_HEIGHT}px`,
    aspectRatio: "auto",
    pointerEvents: "none",
  });
  document.body.append(preview);
  dataTransfer.setDragImage(
    preview,
    MEDIA_DRAG_PREVIEW_WIDTH / 2,
    MEDIA_DRAG_PREVIEW_HEIGHT / 2,
  );
  const remove = () => preview.remove();
  if (typeof requestAnimationFrame === "function") requestAnimationFrame(remove);
  else setTimeout(remove, 0);
}

function mediaThumbnailKey(item: MediaItem): string {
  return `${item.id}|${item.path ?? ""}|${item.thumbnail ?? ""}|${item.missing ? "missing" : "online"}`;
}

function requestMediaCardThumbnail(item: MediaItem, projectEpoch: number) {
  const key = mediaThumbnailKey(item);
  return derivedResourceScheduler.request<string | null>({
    projectEpoch,
    kind: derivedResourceKinds.thumbnail,
    key: `thumbnail:card:${key}`,
    priority: "visible",
    run: async () => {
      if (mediaThumbnailCache.has(key)) return mediaThumbnailCache.get(key) ?? null;
      const result = await generateThumbnail(item.id, { includeSprite: false });
      const path = result?.thumbnailPath ?? null;
      mediaThumbnailCache.set(key, path);
      return path;
    },
  });
}

/** Media/Audio share the asset surface; Text has its own lightweight action surface. */
type MediaTabKind = "material" | "audio";

export function MediaPanel() {
  const mediaTab = useEditorUiStore((s) => s.mediaTab);
  const setMediaTab = useEditorUiStore((s) => s.setMediaTab);
  const t = useT();

  // One-time (#91): drain any legacy `opentake.favorites` localStorage stars into
  // the current project's manifest once its media has loaded. `migrateLocalFavorites`
  // self-guards (empty store / no matching items → no-op), so this settles after
  // the first successful migration and safely re-checks on project switch.
  const items = useMediaStore((s) => s.items);
  const projectEpoch = useProjectStore((s) => s.projectEpoch);
  const projectPath = useProjectStore((s) => s.projectPath);
  useEffect(() => {
    if (!projectPath) return;
    const project = { projectEpoch, projectPath };
    void migrateLocalFavorites(items, project)
      .then((outcome) => {
        if (!applyFavoriteMigrationOutcome(project, outcome)) return;
        if (outcome.synced) void useLibraryStore.getState().refresh();
      })
      .catch((error: unknown) => {
        applyMediaErrorForProject(project, String(error));
      });
  }, [items, projectEpoch, projectPath]);

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%", width: "100%" }}>
      <MediaTabBar active={mediaTab} onSelect={setMediaTab} />
      {/* minHeight:0 lets the inner grid actually scroll instead of overflowing
          and pushing the whole panel (which hid the tab bar + killed scroll). */}
      {MEDIA_MAIN_TAB_IDS.map((tab) => {
        const active = tab === mediaTab;
        return (
          <div
            key={tab}
            id={`media-main-panel-${tab}`}
            role="tabpanel"
            aria-labelledby={`media-main-tab-${tab}`}
            hidden={!active}
            style={
              active
                ? {
                    flex: 1,
                    minWidth: 0,
                    minHeight: 0,
                    display: "flex",
                    flexDirection: "column",
                  }
                : { display: "none" }
            }
          >
            {active ? (
              tab === "material" || tab === "audio" ? (
                <MediaTab kind={tab as MediaTabKind} />
              ) : tab === "music" ? (
                <MusicTab />
              ) : tab === "transition" ? (
                <TransitionTab />
              ) : tab === "subtitle" ? (
                <CaptionsTab />
              ) : tab === "smartPack" ? (
                <SmartPackTab />
              ) : tab === "effect" ? (
                <EffectTab />
              ) : tab === "text" ? (
                <TextTab />
              ) : (
                <Placeholder label={t(`media.tab.${tab}`)} />
              )
            ) : null}
          </div>
        );
      })}
    </div>
  );
}

function TextTab() {
  const t = useT();
  const pushToast = useEditorUiStore((state) => state.pushToast);
  const [pending, setPending] = useState(false);

  const onAddText = async () => {
    if (pending) return;
    setPending(true);
    try {
      await addTextClip();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      pushToast(`${t("toolbar.addText")}: ${message}`);
    } finally {
      setPending(false);
    }
  };

  return (
    <div
      style={{
        flex: 1,
        minHeight: 0,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        gap: "var(--space-md)",
        color: "var(--text-secondary)",
      }}
    >
      <Icon icon={TypeIcon} size={28} strokeWidth={1.5} />
      <button
        type="button"
        aria-label={t("toolbar.addText")}
        aria-busy={pending || undefined}
        disabled={pending}
        onClick={() => void onAddText()}
        style={{
          minHeight: 30,
          padding: "0 var(--space-lg)",
          borderRadius: "var(--radius-sm)",
          border: "var(--bw-thin) solid var(--border-primary)",
          background: "var(--bg-raised)",
          color: "var(--text-primary)",
          cursor: pending ? "wait" : "pointer",
          opacity: pending ? 0.6 : 1,
        }}
      >
        {t("toolbar.addText")}
      </button>
    </div>
  );
}

function EffectTab() {
  const t = useT();
  const timeline = useProjectStore((state) => state.timeline);
  const selectedClipIds = useEditorUiStore((state) => state.selectedClipIds);
  const pushToast = useEditorUiStore((state) => state.pushToast);
  const [pending, setPending] = useState<AdvertisedEffectName | null>(null);
  const selectedVisualClips = timeline.tracks
    .flatMap((track) => track.clips)
    .filter((clip) => selectedClipIds.has(clip.id) && clip.mediaType !== "audio");
  const selectedClip = selectedVisualClips.length === 1 ? selectedVisualClips[0] : undefined;
  const editable = selectedClip !== undefined;

  const onAddEffect = async (name: AdvertisedEffectName) => {
    if (!selectedClip || !editable || pending) return;
    setPending(name);
    try {
      await setEffects([selectedClip.id], [
        ...(selectedClip.effects ?? []),
        newAdvertisedEffect(name),
      ]);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      pushToast(`${t("media.tab.effect")}: ${message}`);
    } finally {
      setPending(null);
    }
  };

  return (
    <div
      style={{
        flex: 1,
        minHeight: 0,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        gap: "var(--space-md)",
        color: "var(--text-secondary)",
      }}
    >
      <span>{t("media.tab.effect")}</span>
      <div style={{ display: "flex", flexWrap: "wrap", justifyContent: "center", gap: "var(--space-xs)" }}>
        {EFFECT_REGISTRY.map((effect) => (
          <button
            key={effect.name}
            type="button"
            data-testid="effect-preset"
            data-effect-name={effect.name}
            aria-label={t(effect.labelKey)}
            disabled={!editable || pending !== null}
            onClick={() => void onAddEffect(effect.name)}
            style={{
              minHeight: 28,
              padding: "0 var(--space-sm)",
              borderRadius: "var(--radius-sm)",
              border: "var(--bw-thin) solid var(--border-primary)",
              background: "var(--bg-raised)",
              color: "var(--text-primary)",
              cursor: editable && pending === null ? "pointer" : "not-allowed",
              opacity: editable && pending === null ? 1 : 0.5,
            }}
          >
            {t(effect.labelKey)}
          </button>
        ))}
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
  const libraryEntries = useLibraryStore((s) => s.entries);
  const libraryLoading = useLibraryStore((s) => s.loading);
  const libraryError = useLibraryStore((s) => s.error);
  const refreshLibrary = useLibraryStore((s) => s.refresh);
  const subTab = useEditorUiStore((s) => s.mediaSubTab);
  const setSubTab = useEditorUiStore((s) => s.setMediaSubTab);
  const currentFolderId = useEditorUiStore((s) => s.mediaPanelCurrentFolderId);
  const setCurrentFolderId = useEditorUiStore((s) => s.setMediaPanelCurrentFolderId);
  const [search, setSearch] = useState("");
  // 视图层展示状态：排序/筛选/视图模式只影响渲染，不改媒体镜像（store 不动）。
  const [organizationMode, setOrganizationMode] = useState<MediaOrganizationMode>("folder");
  const [viewMode, setViewMode] = useState<MediaViewLayout>("grid");
  const [sortKey, setSortKey] = useState<MediaSortKey>("default");
  const [typeFilter, setTypeFilter] = useState<MediaTypeFilter>("all");
  const [organizationOpen, setOrganizationOpen] = useState(false);
  const [sortOpen, setSortOpen] = useState(false);
  const [filterOpen, setFilterOpen] = useState(false);
  const isAudio = kind === "audio";
  const subTabs = isAudio ? AUDIO_SUB_TABS : MATERIAL_SUB_TABS;

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

  useEffect(() => {
    if (subTab === "mine") void refreshLibrary();
  }, [subTab, refreshLibrary]);

  // The extract/sound subtabs exist only on the audio tab; if we land on the
  // material tab still pointing at one, fall back to import.
  useEffect(() => {
    if (!isAudio && (subTab === "extract" || subTab === "sound")) {
      setSubTab("import");
    }
  }, [isAudio, subTab, setSubTab]);

  // Effective cursor: favorites view is always flat (root).
  const folderId = browsing ? currentFolderId : null;
  const query = search.trim().toLowerCase();
  const importItems = useMemo(
    () => items.filter((item) => (kind === "audio" ? item.type === "audio" : true)),
    [items, kind],
  );
  const importProjection = useMemo(() => {
    const projection = projectMediaView({
      mode: organizationMode,
      items: importItems,
      folders,
      currentFolderId: folderId,
      query,
      typeFilter,
      favoriteOnly: false,
    });
    return {
      folders: projection.folders,
      items: sortMediaItems(projection.items, sortKey),
      groups: projection.groups.map((group) => ({
        ...group,
        items: sortMediaItems(group.items, sortKey),
      })),
    };
  }, [organizationMode, importItems, folders, folderId, query, typeFilter, sortKey]);
  const searchNameMatches = useMemo(
    () =>
      sortMediaItems(
        projectMediaView({
          mode: "flat",
          items: importItems,
          folders,
          currentFolderId: folderId,
          query,
          typeFilter,
          favoriteOnly: false,
        }).items,
        sortKey,
      ),
    [importItems, folders, folderId, query, typeFilter, sortKey],
  );
  const filteredLibraryEntries = useMemo(
    () =>
      libraryEntries.filter((entry) => {
        if (kind === "audio" && entry.type !== "audio") return false;
        if (query === "") return true;
        return sourceName(entry.source ?? entry.storedPath).toLowerCase().includes(query);
      }),
    [libraryEntries, kind, query],
  );

  // "提取" subtab (audio only): project videos carrying an extractable audio
  // track. The shared MediaCard keeps Extract keyboard-reachable without
  // leaving the audio tab. Search filters by name like the other views; the
  // local sort applies too (the view is inherently video-only, so the type
  // filter does not).
  const extractableVideos = useMemo(
    () =>
      sortMediaItems(
        items.filter(
          (item) =>
            item.type === "video" &&
            item.hasAudio &&
            !item.missing &&
            (query === "" || item.name.toLowerCase().includes(query)),
        ),
        sortKey,
      ),
    [items, query, sortKey],
  );

  const trail = useMemo(() => folderTrail(folders, folderId), [folders, folderId]);
  const totalCount =
    organizationMode === "grouped"
      ? importProjection.groups.reduce((count, group) => count + group.items.length, 0)
      : importProjection.folders.length + importProjection.items.length;
  const isEmpty =
    organizationMode === "grouped" ? importProjection.groups.length === 0 : totalCount === 0;
  const audioExtractView = isAudio && subTab === "extract";
  const audioSoundView = isAudio && subTab === "sound";
  const displayCount = audioExtractView
    ? extractableVideos.length
    : subTab === "mine"
      ? filteredLibraryEntries.length
      : totalCount;

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
          {/* AI 生成尚未接线（generate_* 仍是 stub）。封边：明确「即将推出」并禁用，
              不给测试者一个点了没反应的死按钮。 */}
          <button
            type="button"
            disabled
            aria-disabled
            title={t("media.generateSoon")}
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
              opacity: 0.55,
              cursor: "not-allowed",
            }}
          >
            <Icon icon={Sparkles} size={12} />
            {t("media.generate")}
          </button>
          <div style={{ flex: 1 }} />
          {/* 二级标签：素材=导入/我的；音频额外有 提取 / 音效。 */}
          <MediaSubTabBar
            active={subTab}
            onSelect={setSubTab}
            tabs={isAudio ? AUDIO_SUB_TABS : MATERIAL_SUB_TABS}
            idPrefix={`media-${kind}-subtab`}
          />
        </div>
        {/* searchControlsRow */}
        <div style={{ height: 28, display: "flex", alignItems: "center", gap: "var(--space-xs)" }}>
          <input
            type="search"
            aria-label={t("media.search")}
            placeholder={t("media.search")}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            style={{
              flex: 1,
              height: 24,
              background: "var(--bg-raised)",
              border: "var(--bw-thin) solid var(--border-primary)",
              borderRadius: "var(--radius-sm)",
              color: "var(--text-primary)",
              fontSize: "var(--fs-sm)",
              padding: "0 8px",
            }}
          />
          {/* 视图层展示控件（导入视图）：网格/列表切换 + 排序 + 类型筛选。
              纯本地渲染行为，不改媒体镜像。我的/音效/提取是任务型列表
              （数据源不同或语义固定），不显示这些控件，避免死按钮。 */}
          {subTab === "import" && (
            <>
              <ToolbarMenu
                title={t("media.organizationMode")}
                icon={FolderIcon}
                open={organizationOpen}
                onToggle={setOrganizationOpen}
              >
                {(closeAndRestore) =>
                  ORGANIZATION_OPTIONS.map((option, index) => (
                    <ToolbarMenuOption
                      key={option.id}
                      label={t(option.labelKey)}
                      selected={organizationMode === option.id}
                      tabIndex={index === 0 ? 0 : -1}
                      onSelect={() => {
                        setOrganizationMode(option.id);
                        closeAndRestore();
                      }}
                    />
                  ))
                }
              </ToolbarMenu>
              <HoverButton
                title={t("media.viewMode")}
                active={viewMode === "list"}
                disabled={query !== ""}
                onClick={() => setViewMode(viewMode === "grid" ? "list" : "grid")}
              >
                <Icon icon={viewMode === "grid" ? LayoutGrid : List} size={13} />
              </HoverButton>
              <ToolbarMenu
                title={t("media.sort")}
                icon={ArrowUpDown}
                open={sortOpen}
                onToggle={setSortOpen}
              >
                {(closeAndRestore) =>
                  SORT_OPTIONS.map((option, index) => (
                    <ToolbarMenuOption
                      key={option.id}
                      label={t(option.labelKey)}
                      selected={sortKey === option.id}
                      tabIndex={index === 0 ? 0 : -1}
                      onSelect={() => {
                        setSortKey(option.id);
                        closeAndRestore();
                      }}
                    />
                  ))
                }
              </ToolbarMenu>
              <ToolbarMenu
                title={t("media.filter")}
                icon={Filter}
                open={filterOpen}
                onToggle={setFilterOpen}
              >
                {(closeAndRestore) =>
                  TYPE_FILTER_OPTIONS.map((option, index) => (
                    <ToolbarMenuOption
                      key={option.id}
                      label={t(option.labelKey)}
                      selected={typeFilter === option.id}
                      tabIndex={index === 0 ? 0 : -1}
                      onSelect={() => {
                        setTypeFilter(option.id);
                        closeAndRestore();
                      }}
                    />
                  ))
                }
              </ToolbarMenu>
            </>
          )}
        </div>
        {/* Breadcrumb / 返回上级 — only while browsing the library tree and not
            searching. Root is always clickable; the current folder is plain text. */}
        {browsing && organizationMode === "folder" && query === "" && (
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
          <span>
            {importing
              ? t("media.importing")
              : audioSoundView
                ? ""
                : t("media.itemCount", { count: displayCount })}
          </span>
        </div>
        {(error || (subTab === "mine" && libraryError)) && (
          <div style={{ color: "var(--status-error)", fontSize: "var(--fs-xs)" }}>
            {t("media.importFailed", { error: error ?? libraryError ?? "" })}
          </div>
        )}
      </div>

      {subTabs.map((tab) => {
        const active = tab.id === subTab;
        return (
          <div
            key={tab.id}
            id={`media-${kind}-subtab-panel-${tab.id}`}
            role="tabpanel"
            aria-labelledby={`media-${kind}-subtab-${tab.id}`}
            hidden={!active}
            style={
              active
                ? { flex: 1, minHeight: 0, display: "flex", flexDirection: "column" }
                : { display: "none" }
            }
          >
            {active ? (
              subTab === "mine" ? (
                <LibraryEntryGrid
                  entries={filteredLibraryEntries}
                  loading={libraryLoading}
                  totalEmpty={libraryEntries.length === 0}
                />
              ) : audioSoundView ? (
                // 音效库（#115 全局库的 sound 分类）搬进音频 tab，一键导入项目。
                <SoundLibraryTab query={query} />
              ) : audioExtractView ? (
                // 从视频提取音频：列出含音轨的项目视频，卡片提取按钮即走
                // extract_audio，无需离开音频 tab。
                extractableVideos.length === 0 ? (
                  <div
                    style={{
                      flex: 1,
                      display: "flex",
                      flexDirection: "column",
                      alignItems: "center",
                      justifyContent: "center",
                      gap: "var(--space-xs)",
                      padding: "var(--space-lg)",
                      color: "var(--text-tertiary)",
                      fontSize: "var(--fs-sm)",
                      textAlign: "center",
                    }}
                  >
                    <span>{t("media.extract.empty")}</span>
                    <span style={{ fontSize: "var(--fs-xs)" }}>{t("media.extract.hint")}</span>
                  </div>
                ) : (
                  <MediaGrid
                    folders={[]}
                    items={extractableVideos}
                    onOpenFolder={setCurrentFolderId}
                    layout={viewMode}
                  />
                )
              ) : query !== "" ? (
                // Smart search: three result groups (Moments / Spoken / Files) + the
                // index-status affordance. `searchNameMatches` is the name-matched Files group
                // (already scoped to the current main/subtab). Moments/Spoken come from
                // the backend query; they degrade to empty with no model, leaving Files.
                <MediaSearchResults
                  query={query}
                  nameMatches={searchNameMatches}
                  hasIndexableAssets={items.some((i) => i.type === "video" || i.type === "image")}
                />
              ) : isEmpty ? (
                <EmptyState subTab={subTab} insideFolder={browsing && folderId !== null} />
              ) : organizationMode === "grouped" ? (
                <MediaGroupedView groups={importProjection.groups} layout={viewMode} />
              ) : (
                <MediaGrid
                  folders={importProjection.folders}
                  items={importProjection.items}
                  onOpenFolder={setCurrentFolderId}
                  layout={viewMode}
                  organization={organizationMode}
                />
              )
            ) : null}
          </div>
        );
      })}
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
        data-folder-breadcrumb-target={target ?? "root"}
        onClick={() => onNavigate(target)}
        className="hover-area"
        style={{
          background: "transparent",
          border: "none",
          minWidth: 24,
          minHeight: 24,
          display: "inline-flex",
          alignItems: "center",
          justifyContent: "center",
          padding: "0 4px",
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
        minHeight: 24,
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
            width: 24,
            height: 24,
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

function popupItems(root: HTMLElement | null): HTMLButtonElement[] {
  if (!root) return [];
  return [...root.querySelectorAll<HTMLButtonElement>('[role^="menuitem"]')];
}

function setPopupTabStop(items: HTMLButtonElement[], target: HTMLButtonElement | undefined) {
  if (!target) return;
  for (const item of items) item.tabIndex = item === target ? 0 : -1;
}

function focusPopupItem(items: HTMLButtonElement[], target: HTMLButtonElement | undefined) {
  setPopupTabStop(items, target);
  target?.focus();
}

function handlePopupKeyDown(
  event: React.KeyboardEvent<HTMLElement>,
  root: HTMLElement | null,
  closeAndRestore: () => void,
  closeWithoutRestore: () => void,
) {
  if (event.key === "Tab") {
    closeWithoutRestore();
    return;
  }
  if (event.key === "Escape") {
    event.preventDefault();
    closeAndRestore();
    return;
  }
  const items = popupItems(root);
  if (items.length === 0) return;
  const current = items.indexOf(document.activeElement as HTMLButtonElement);
  let next: number | null = null;
  if (event.key === "ArrowDown") next = (Math.max(0, current) + 1) % items.length;
  else if (event.key === "ArrowUp") next = current <= 0 ? items.length - 1 : current - 1;
  else if (event.key === "Home") next = 0;
  else if (event.key === "End") next = items.length - 1;
  if (next === null) return;
  event.preventDefault();
  focusPopupItem(items, items[next]);
}

const MEDIA_POPUP_OPEN_EVENT = "opentake:media-popup-open";

function announceMediaPopupOpen(menuId: string) {
  window.dispatchEvent(new CustomEvent<string>(MEDIA_POPUP_OPEN_EVENT, { detail: menuId }));
}

function activatePopupItemFromKeyboard(
  event: React.KeyboardEvent<HTMLButtonElement>,
  onSelect: () => void,
) {
  if (event.key !== "Enter" && event.key !== " ") return;
  event.preventDefault();
  onSelect();
}

/** Import button with a small folder/files menu (CapCut-style folder import). */
function ImportMenu() {
  const t = useT();
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const initialFocusRef = useRef<"first" | "last">("first");
  const menuId = useId();

  const openMenu = (edge: "first" | "last" = "first") => {
    initialFocusRef.current = edge;
    announceMediaPopupOpen(menuId);
    setOpen(true);
  };
  const closeWithoutRestore = () => setOpen(false);
  const closeAndRestore = () => {
    closeWithoutRestore();
    triggerRef.current?.focus();
  };

  useEffect(() => {
    if (!open) return;
    const items = popupItems(menuRef.current);
    const target = initialFocusRef.current === "last" ? items[items.length - 1] : items[0];
    focusPopupItem(items, target);
  }, [open]);

  useEffect(() => {
    const onPopupOpen = (event: Event) => {
      if ((event as CustomEvent<string>).detail !== menuId) closeWithoutRestore();
    };
    window.addEventListener(MEDIA_POPUP_OPEN_EVENT, onPopupOpen);
    return () => window.removeEventListener(MEDIA_POPUP_OPEN_EVENT, onPopupOpen);
  }, [menuId]);

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
      <HoverButton
        title={t("media.importHint")}
        active={open}
        buttonRef={triggerRef}
        ariaHasPopup="menu"
        ariaExpanded={open}
        ariaControls={menuId}
        onClick={() => (open ? closeAndRestore() : openMenu())}
        onKeyDown={(event) => {
          if (event.key === "ArrowDown" || event.key === "ArrowUp") {
            event.preventDefault();
            openMenu(event.key === "ArrowUp" ? "last" : "first");
          } else if (event.key === "Escape" && open) {
            closeAndRestore();
          }
        }}
      >
        <Icon icon={Plus} size={13} />
      </HoverButton>
      {open && (
        <div
          ref={menuRef}
          id={menuId}
          role="menu"
          aria-label={t("media.importHint")}
          onFocus={(event) => {
            if (event.target instanceof HTMLButtonElement) {
              setPopupTabStop(popupItems(menuRef.current), event.target);
            }
          }}
          onBlur={(event) => {
            if (!event.currentTarget.contains(event.relatedTarget as Node | null)) {
              closeWithoutRestore();
            }
          }}
          onKeyDown={(event) =>
            handlePopupKeyDown(event, menuRef.current, closeAndRestore, closeWithoutRestore)
          }
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
            tabIndex={0}
            onClick={() => {
              closeAndRestore();
              void importFolderViaDialog();
            }}
          />
          <ImportMenuItem
            icon={Plus}
            label={t("media.importFiles")}
            tabIndex={-1}
            onClick={() => {
              closeAndRestore();
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
  tabIndex,
  onClick,
}: {
  icon: typeof Plus;
  label: string;
  tabIndex: number;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      role="menuitem"
      tabIndex={tabIndex}
      onClick={onClick}
      onKeyDown={(event) => activatePopupItemFromKeyboard(event, onClick)}
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
        color: "var(--text-tertiary)",
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
  layout = "grid",
  organization = "folder",
}: {
  folders: MediaFolder[];
  items: MediaItem[];
  onOpenFolder: (id: string) => void;
  layout?: MediaViewLayout;
  organization?: MediaOrganizationMode;
}) {
  const selectedMediaAssetIds = useEditorUiStore((s) => s.selectedMediaAssetIds);
  const selectedFolderIds = useEditorUiStore((s) => s.selectedFolderIds);
  const activeFolderId = folders.find((folder) => selectedFolderIds.has(folder.id))?.id;
  const activeMediaId = activeFolderId
    ? undefined
    : items.find((item) => selectedMediaAssetIds.has(item.id))?.id;
  const defaultFolderId = folders[0]?.id;
  const defaultMediaId = defaultFolderId ? undefined : items[0]?.id;

  // 列表视图：同一 roving/selection 契约（data-media-tile + role gridcell），
  // 只是行布局。文件夹仍在素材前。
  if (layout === "list") {
    return (
      <div
        role="grid"
        aria-label="Media"
        data-media-roving-container="true"
        data-media-layout="list"
        data-media-organization={organization}
        style={{
          flex: 1,
          overflowY: "auto",
          display: "flex",
          flexDirection: "column",
          gap: 2,
          padding: "var(--space-sm)",
          alignContent: "start",
        }}
      >
        {folders.map((folder) => (
          <div key={folder.id} role="row" style={{ minWidth: 0 }}>
            <MediaFolderRow
              folder={folder}
              onOpen={onOpenFolder}
              rovingTabIndex={
                folder.id === (activeFolderId ?? (activeMediaId ? undefined : defaultFolderId))
                  ? 0
                  : -1
              }
            />
          </div>
        ))}
        {items.map((item) => (
          <div key={item.id} role="row" style={{ minWidth: 0 }}>
            <MediaListRow
              item={item}
              rovingTabIndex={item.id === (activeMediaId ?? defaultMediaId) ? 0 : -1}
            />
          </div>
        ))}
      </div>
    );
  }

  return (
    <div
      role="grid"
      aria-label="Media"
      data-media-roving-container="true"
      data-media-layout="grid"
      data-media-organization={organization}
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
        <div key={folder.id} role="row" style={{ minWidth: 0 }}>
          <FolderTile
            folder={folder}
            onOpen={onOpenFolder}
            rovingTabIndex={
              folder.id === (activeFolderId ?? (activeMediaId ? undefined : defaultFolderId))
                ? 0
                : -1
            }
          />
        </div>
      ))}
      {items.map((item) => (
        <div key={item.id} role="row" style={{ minWidth: 0 }}>
          <MediaCard
            item={item}
            rovingTabIndex={item.id === (activeMediaId ?? defaultMediaId) ? 0 : -1}
          />
        </div>
      ))}
    </div>
  );
}

function MediaGroupedView({
  groups,
  layout,
}: {
  groups: MediaViewGroup[];
  layout: MediaViewLayout;
}) {
  const t = useT();
  const selectedMediaAssetIds = useEditorUiStore((s) => s.selectedMediaAssetIds);
  const allItems = groups.flatMap((group) => group.items);
  const activeMediaId = allItems.find((item) => selectedMediaAssetIds.has(item.id))?.id;
  const defaultMediaId = allItems[0]?.id;

  return (
    <div
      role="grid"
      aria-label="Media"
      data-media-roving-container="true"
      data-media-layout={layout}
      data-media-organization="grouped"
      style={{
        flex: 1,
        overflowY: "auto",
        display: "flex",
        flexDirection: "column",
        gap: "var(--space-sm)",
        padding: "var(--space-sm)",
      }}
    >
      {groups.map((group) => (
        <section
          key={group.folderId ?? "__root__"}
          style={{ display: "flex", flexDirection: "column", gap: "var(--space-xs)" }}
        >
          <div
            data-media-group-heading="true"
            style={{
              color: "var(--text-secondary)",
              fontSize: "var(--fs-xs)",
              fontWeight: "var(--fw-medium)",
            }}
          >
            {group.folderId === null ? t("media.folderRoot") : group.label}
          </div>
          {layout === "list" ? (
            <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
              {group.items.map((item) => (
                <div key={item.id} role="row" style={{ minWidth: 0 }}>
                  <MediaListRow
                    item={item}
                    rovingTabIndex={item.id === (activeMediaId ?? defaultMediaId) ? 0 : -1}
                  />
                </div>
              ))}
            </div>
          ) : (
            <div
              style={{
                display: "grid",
                gridTemplateColumns: "repeat(auto-fill, minmax(96px, 1fr))",
                gap: "var(--space-sm)",
                alignContent: "start",
              }}
            >
              {group.items.map((item) => (
                <div key={item.id} role="row" style={{ minWidth: 0 }}>
                  <MediaCard
                    item={item}
                    rovingTabIndex={item.id === (activeMediaId ?? defaultMediaId) ? 0 : -1}
                  />
                </div>
              ))}
            </div>
          )}
        </section>
      ))}
    </div>
  );
}

/** 视图层下拉菜单（排序/筛选）：仿 ImportMenu 的弹出菜单 + 外点关闭。 */
function ToolbarMenu({
  title,
  icon,
  open,
  onToggle,
  children,
}: {
  title: string;
  icon: typeof LayoutGrid;
  open: boolean;
  onToggle: (open: boolean) => void;
  children: (closeAndRestore: () => void) => React.ReactNode;
}) {
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const initialFocusRef = useRef<"first" | "last">("first");
  const menuId = useId();

  const openMenu = (edge: "first" | "last" = "first") => {
    initialFocusRef.current = edge;
    announceMediaPopupOpen(menuId);
    onToggle(true);
  };
  const closeWithoutRestore = () => onToggle(false);
  const closeAndRestore = () => {
    closeWithoutRestore();
    triggerRef.current?.focus();
  };

  useEffect(() => {
    if (!open) return;
    const items = popupItems(menuRef.current);
    const target = initialFocusRef.current === "last" ? items[items.length - 1] : items[0];
    focusPopupItem(items, target);
  }, [open]);

  useEffect(() => {
    const onPopupOpen = (event: Event) => {
      if ((event as CustomEvent<string>).detail !== menuId) closeWithoutRestore();
    };
    window.addEventListener(MEDIA_POPUP_OPEN_EVENT, onPopupOpen);
    return () => window.removeEventListener(MEDIA_POPUP_OPEN_EVENT, onPopupOpen);
  }, [menuId, onToggle]);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) onToggle(false);
    };
    window.addEventListener("mousedown", onDown);
    return () => window.removeEventListener("mousedown", onDown);
  }, [open, onToggle]);

  return (
    <div ref={rootRef} style={{ position: "relative", display: "inline-flex" }}>
      <HoverButton
        title={title}
        active={open}
        buttonRef={triggerRef}
        ariaHasPopup="menu"
        ariaExpanded={open}
        ariaControls={menuId}
        onClick={() => (open ? closeAndRestore() : openMenu())}
        onKeyDown={(event) => {
          if (event.key === "ArrowDown" || event.key === "ArrowUp") {
            event.preventDefault();
            openMenu(event.key === "ArrowUp" ? "last" : "first");
          } else if (event.key === "Escape" && open) {
            closeAndRestore();
          }
        }}
      >
        <Icon icon={icon} size={13} />
      </HoverButton>
      {open && (
        <div
          ref={menuRef}
          id={menuId}
          role="menu"
          aria-label={title}
          onFocus={(event) => {
            if (event.target instanceof HTMLButtonElement) {
              setPopupTabStop(popupItems(menuRef.current), event.target);
            }
          }}
          onBlur={(event) => {
            if (!event.currentTarget.contains(event.relatedTarget as Node | null)) {
              closeWithoutRestore();
            }
          }}
          onKeyDown={(event) =>
            handlePopupKeyDown(event, menuRef.current, closeAndRestore, closeWithoutRestore)
          }
          style={{
            position: "absolute",
            top: "calc(100% + 6px)",
            right: 0,
            minWidth: 140,
            padding: "var(--space-xs)",
            background: "var(--bg-raised)",
            border: "var(--bw-thin) solid var(--border-primary)",
            borderRadius: "var(--radius-md)",
            boxShadow: "var(--shadow-lg)",
            zIndex: 200,
          }}
        >
          {children(closeAndRestore)}
        </div>
      )}
    </div>
  );
}

function ToolbarMenuOption({
  label,
  selected,
  tabIndex,
  onSelect,
}: {
  label: string;
  selected: boolean;
  tabIndex: number;
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      role="menuitemradio"
      aria-checked={selected}
      tabIndex={tabIndex}
      onClick={onSelect}
      onKeyDown={(event) => activatePopupItemFromKeyboard(event, onSelect)}
      className="hover-area"
      style={{
        display: "flex",
        alignItems: "center",
        gap: "var(--space-sm)",
        width: "100%",
        minHeight: 28,
        padding: "0 var(--space-sm)",
        borderRadius: "var(--radius-sm)",
        color: selected ? "var(--text-primary)" : "var(--text-secondary)",
        fontSize: "var(--fs-sm)",
        fontWeight: selected ? "var(--fw-medium)" : "var(--fw-regular)",
        textAlign: "left",
      }}
    >
      <span style={{ width: 14, display: "inline-flex", justifyContent: "center" }}>
        {selected && <Icon icon={Check} size={13} />}
      </span>
      <span style={{ flex: 1 }}>{label}</span>
    </button>
  );
}

/** 列表视图里的文件夹行：单击选中、双击/Enter 进入、Delete 删除，
 *  右键菜单打开/删除（与 FolderTile 同一交互契约）。 */
export function MediaFolderRow({
  folder,
  onOpen,
  rovingTabIndex = 0,
}: {
  folder: MediaFolder;
  onOpen: (id: string) => void;
  rovingTabIndex?: number;
}) {
  const t = useT();
  const rowRef = useRef<HTMLDivElement | null>(null);
  const [focused, setFocused] = useState(false);
  const [menuPoint, setMenuPoint] = useState<TileMenuState | null>(null);
  const selected = useEditorUiStore((s) => s.selectedFolderIds.has(folder.id));

  const removeContextFolder = () => {
    void deleteFolderFromContextMenu(folder.id).catch((error) => {
      useEditorUiStore.getState().pushToast(String(error));
    });
  };
  const openFolder = () => openMediaFolder(folder.id, onOpen);

  return (
    <div
      ref={rowRef}
      onClick={(event) => {
        event.currentTarget.focus();
        selectMediaFolder(folder.id);
      }}
      onDoubleClick={openFolder}
      onFocus={(event) => {
        setFocused(true);
        if (event.target === event.currentTarget) selectMediaFolder(folder.id);
      }}
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) setFocused(false);
      }}
      onContextMenu={(event) => {
        event.preventDefault();
        event.stopPropagation();
        setMenuPoint({ x: event.clientX, y: event.clientY, restoreFocus: false });
      }}
      onKeyDown={(e) => {
        if (e.target !== e.currentTarget) return;
        if (handleMediaTileArrowNavigation(e)) return;
        if (e.key === "Enter") {
          e.preventDefault();
          e.stopPropagation();
          selectMediaFolder(folder.id);
          openFolder();
        } else if (e.key === " ") {
          e.preventDefault();
          e.stopPropagation();
          selectMediaFolder(folder.id);
        } else if (e.key === "Delete" || e.key === "Backspace") {
          e.preventDefault();
          e.stopPropagation();
          if (!e.repeat) {
            void deleteSelectedFolders().catch((error) => {
              useEditorUiStore.getState().pushToast(String(error));
            });
          }
        } else if (e.key === "ContextMenu" || (e.shiftKey && e.key === "F10")) {
          e.preventDefault();
          e.stopPropagation();
          setMenuPoint({ ...keyboardMenuPoint(e.currentTarget), restoreFocus: true });
        }
      }}
      title={folder.name}
      role="gridcell"
      tabIndex={rovingTabIndex}
      aria-selected={selected}
      aria-haspopup="menu"
      data-media-tile="true"
      data-media-folder-id={folder.id}
      style={{
        display: "flex",
        alignItems: "center",
        gap: "var(--space-sm)",
        height: 36,
        padding: "0 var(--space-sm)",
        borderRadius: "var(--radius-sm)",
        cursor: "pointer",
        outline: focused ? "2px solid var(--accent-primary)" : "none",
        outlineOffset: 2,
      }}
    >
      <Icon icon={FolderIcon} size={15} />
      <span
        style={{
          flex: 1,
          minWidth: 0,
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
          fontSize: "var(--fs-sm)",
          color: "var(--text-secondary)",
        }}
      >
        {folder.name}
      </span>
      {menuPoint && (
        <TileContextMenu
          point={menuPoint}
          onClose={() => setMenuPoint(null)}
          returnFocus={rowRef}
          restoreFocus={menuPoint.restoreFocus}
          actions={[
            { label: t("common.open"), onSelect: openFolder },
            { label: t("contextMenu.delete"), onSelect: removeContextFolder, destructive: true },
          ]}
        />
      )}
    </div>
  );
}

/** 列表视图里的素材行：与网格卡片同一交互契约（选择/预览、双击上时间线、
 *  拖拽、键盘 roving、右键/Delete 删除、收藏）。行内不做懒缩略图与生成
 *  覆盖层——网格视图保留这些富交互，列表是密度更高的概览。 */
export function MediaListRow({
  item,
  rovingTabIndex = 0,
}: {
  item: MediaItem;
  rovingTabIndex?: number;
}) {
  const t = useT();
  const rowRef = useRef<HTMLDivElement | null>(null);
  const thumbnailRef = useRef<HTMLDivElement | null>(null);
  const fps = useProjectStore((s) => s.timeline.fps);
  const selected = useEditorUiStore((s) => s.selectedMediaAssetIds.has(item.id));
  const durationFrames = Math.round(item.duration * fps);
  const favorite = item.favorite ?? false;
  const generationActive =
    item.generationStatus === "generating" || item.generationStatus === "downloading";
  const generationFailed =
    item.generationStatus === "failed" || item.generationStatus === "cancelled";
  const [focused, setFocused] = useState(false);
  const [menuPoint, setMenuPoint] = useState<TileMenuState | null>(null);
  const [feedback, setFeedback] = useState<string | null>(null);

  const activate = () => {
    selectMediaAsset(item.id);
    if (generationActive) return;
    useEditorUiStore.getState().setPreviewMedia(item.id);
    void preloadMedia(item.id);
  };

  const removeContextMedia = () => {
    void deleteMediaFromContextMenu(item.id).catch((error) => {
      useEditorUiStore.getState().pushToast(String(error));
    });
  };

  const onDragStart = (e: React.DragEvent) => {
    e.dataTransfer.setData(MEDIA_DND_TYPE, item.id);
    e.dataTransfer.effectAllowed = "copy";
    if (thumbnailRef.current) {
      setMediaThumbnailDragImage(e.dataTransfer, thumbnailRef.current);
    }
    setDraggingMedia(item);
    void preloadMedia(item.id);
  };

  const onDragEnd = () => setDraggingMedia(null);

  return (
    <div
      ref={rowRef}
      draggable={!generationActive}
      onDragStart={onDragStart}
      onDragEnd={onDragEnd}
      onClick={(event) => {
        event.currentTarget.focus();
        activate();
      }}
      onDoubleClick={() => {
        if (!generationActive && !generationFailed) {
          void addMediaToTimeline(item).catch(reportMediaPlacementFailure);
        }
      }}
      onFocus={(event) => {
        setFocused(true);
        if (event.target === event.currentTarget) activate();
      }}
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) setFocused(false);
      }}
      onContextMenu={(event) => {
        event.preventDefault();
        event.stopPropagation();
        setMenuPoint({ x: event.clientX, y: event.clientY, restoreFocus: false });
      }}
      onKeyDown={(event) => {
        if (event.target !== event.currentTarget) return;
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
          setMenuPoint({ ...keyboardMenuPoint(event.currentTarget), restoreFocus: true });
        }
      }}
      title={item.name}
      role="gridcell"
      tabIndex={rovingTabIndex}
      aria-selected={selected}
      aria-haspopup="menu"
      data-media-tile="true"
      data-media-asset-id={item.id}
      style={{
        display: "flex",
        alignItems: "center",
        gap: "var(--space-sm)",
        minHeight: 36,
        padding: "0 var(--space-sm)",
        borderRadius: "var(--radius-sm)",
        cursor: generationActive ? "progress" : generationFailed ? "default" : "grab",
        outline: focused ? "2px solid var(--accent-primary)" : "none",
        outlineOffset: 2,
      }}
    >
      <div
        ref={thumbnailRef}
        style={{
          flex: "0 0 auto",
          width: 40,
          height: 28,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          borderRadius: "var(--radius-xs)",
          background: "var(--bg-placeholder)",
          color: "var(--text-muted)",
          overflow: "hidden",
        }}
      >
        {item.thumbnail ? (
          <img
            src={assetUrl(item.thumbnail) ?? undefined}
            alt=""
            draggable={false}
            style={{ width: "100%", height: "100%", objectFit: "cover" }}
          />
        ) : (
          <Icon icon={TYPE_ICON[item.type]} size={14} strokeWidth={1.5} />
        )}
      </div>
      <span
        style={{
          flex: 1,
          minWidth: 0,
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
          fontSize: "var(--fs-sm)",
          color: "var(--text-primary)",
        }}
      >
        {item.name}
      </span>
      {item.duration > 0 && (
        <span
          className="tabular"
          style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)", flex: "0 0 auto" }}
        >
          {formatTimecode(durationFrames, fps)}
        </span>
      )}
      {/* MediaFavoriteButton 自带绝对定位，套一个相对定位盒让它落在行内。 */}
      <div style={{ position: "relative", width: 28, height: 28, flex: "0 0 auto" }}>
        <MediaFavoriteButton
          assetId={item.id}
          favorite={favorite}
          title={favorite ? t("media.unfavorite") : t("media.favorite")}
          onSuccess={async (media, project) => {
            if (!applyMediaListForProject(project, media)) return;
            await useLibraryStore.getState().refresh();
          }}
          onError={(message, project) => {
            if (!applyMediaErrorForProject(project, message)) return;
            setFeedback(message);
          }}
          onStart={() => setFeedback(null)}
        />
      </div>
      {feedback && (
        <span style={{ fontSize: "var(--fs-micro)", color: "var(--text-tertiary)" }}>
          {feedback}
        </span>
      )}
      {menuPoint && (
        <TileContextMenu
          point={menuPoint}
          onClose={() => setMenuPoint(null)}
          returnFocus={rowRef}
          restoreFocus={menuPoint.restoreFocus}
          actions={[
            { label: t("contextMenu.delete"), onSelect: removeContextMedia, destructive: true },
          ]}
        />
      )}
    </div>
  );
}

export interface TileMenuPoint {
  x: number;
  y: number;
}

interface TileMenuState extends TileMenuPoint {
  restoreFocus: boolean;
}

export interface TileMenuAction {
  label: string;
  onSelect: () => void;
  destructive?: boolean;
}

/** Compact app-native context menu. App.tsx suppresses the WebView's native
 * menu, so media tiles need their own pointer- and keyboard-reachable actions. */
export function TileContextMenu({
  point,
  actions,
  onClose,
  returnFocus,
  restoreFocus = true,
}: {
  point: TileMenuPoint;
  actions: TileMenuAction[];
  onClose: () => void;
  returnFocus: React.RefObject<HTMLElement | null>;
  restoreFocus?: boolean;
}) {
  const menuRef = useRef<HTMLDivElement | null>(null);
  const onCloseRef = useRef(onClose);
  const firstItemRef = useRef<HTMLButtonElement | null>(null);
  const restoreFocusRef = useRef(restoreFocus);
  onCloseRef.current = onClose;

  useEffect(() => {
    firstItemRef.current?.focus();
    const closeWithoutRestoringFocus = () => {
      restoreFocusRef.current = false;
      onCloseRef.current();
    };
    window.addEventListener("pointerdown", closeWithoutRestoringFocus);
    window.addEventListener("blur", closeWithoutRestoringFocus);
    return () => {
      window.removeEventListener("pointerdown", closeWithoutRestoringFocus);
      window.removeEventListener("blur", closeWithoutRestoringFocus);
      if (restoreFocusRef.current) returnFocus.current?.focus();
    };
  }, [returnFocus]);

  return (
    <div
      ref={menuRef}
      role="menu"
      onPointerDown={(event) => event.stopPropagation()}
      onClick={(event) => event.stopPropagation()}
      onKeyDown={(event) => {
        // Keep editor-global transport/delete shortcuts from firing while a
        // menu item owns keyboard focus.
        event.stopPropagation();
        if (["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
          event.preventDefault();
          const items = [
            ...(menuRef.current?.querySelectorAll<HTMLButtonElement>(
              '[role="menuitem"]:not(:disabled)',
            ) ?? []),
          ];
          if (items.length === 0) return;
          const current = items.indexOf(document.activeElement as HTMLButtonElement);
          const next =
            event.key === "Home"
              ? 0
              : event.key === "End"
                ? items.length - 1
                : event.key === "ArrowDown"
                  ? (Math.max(current, -1) + 1) % items.length
                  : (current <= 0 ? items.length : current) - 1;
          items[next]?.focus();
        } else if (event.key === "Escape") {
          event.preventDefault();
          restoreFocusRef.current = true;
          onClose();
        } else if (event.key === "Tab") {
          // Let the browser advance focus, but remove the transient menu first.
          restoreFocusRef.current = false;
          onClose();
        }
      }}
      onBlur={(event) => {
        const next = event.relatedTarget;
        if (next instanceof Node && event.currentTarget.contains(next)) return;
        restoreFocusRef.current = false;
        onClose();
      }}
      onContextMenu={(event) => event.preventDefault()}
      style={{
        position: "fixed",
        left: point.x,
        top: point.y,
        zIndex: 1000,
        minWidth: 132,
        padding: "var(--space-xs)",
        borderRadius: "var(--radius-md)",
        border: "var(--bw-thin) solid var(--border-primary)",
        background: "var(--bg-raised)",
        boxShadow: "var(--shadow-lg)",
      }}
    >
      {actions.map((action, index) => (
        <button
          key={action.label}
          ref={index === 0 ? firstItemRef : undefined}
          type="button"
          role="menuitem"
          className="hover-area"
          onClick={() => {
            onClose();
            action.onSelect();
          }}
          style={{
            display: "flex",
            width: "100%",
            height: 28,
            alignItems: "center",
            padding: "0 var(--space-sm)",
            borderRadius: "var(--radius-sm)",
            color: action.destructive ? "var(--status-error)" : "var(--text-secondary)",
            fontSize: "var(--fs-sm)",
            textAlign: "left",
          }}
        >
          {action.label}
        </button>
      ))}
    </div>
  );
}

export function keyboardMenuPoint(target: HTMLElement): TileMenuPoint {
  const rect = target.getBoundingClientRect();
  return { x: rect.left + 8, y: rect.top + 24 };
}

/** Move through the rendered tile order and update every part of the roving
 * selection contract before focus moves. */
export function handleMediaTileArrowNavigation(
  event: React.KeyboardEvent<HTMLElement>,
): boolean {
  const delta =
    event.key === "ArrowLeft" || event.key === "ArrowUp"
      ? -1
      : event.key === "ArrowRight" || event.key === "ArrowDown"
        ? 1
        : 0;
  if (delta === 0) return false;
  const container = event.currentTarget.closest<HTMLElement>(
    '[data-media-roving-container="true"]',
  );
  const tiles = [
    ...(container?.querySelectorAll<HTMLElement>('[data-media-tile="true"]') ?? []),
  ];
  const current = tiles.indexOf(event.currentTarget);
  if (current < 0 || tiles.length === 0) return false;
  event.preventDefault();
  event.stopPropagation();
  const nextIndex = Math.max(0, Math.min(tiles.length - 1, current + delta));
  const next = tiles[nextIndex]!;
  const folderId = next.dataset.mediaFolderId;
  const mediaId = next.dataset.mediaAssetId;
  if (folderId) selectMediaFolder(folderId);
  else if (mediaId) selectMediaForPreview(mediaId);
  next.focus();
  next.scrollIntoView?.({ block: "nearest", inline: "nearest" });
  return true;
}

/** A folder shown in the grid (剪映式). Single click selects/enters on
 *  double-click — keeping it consistent with media cards (click=preview,
 *  double-click=add). Not draggable (folders aren't dropped on the timeline). */
export function FolderTile({
  folder,
  onOpen,
  rovingTabIndex = 0,
}: {
  folder: MediaFolder;
  onOpen: (id: string) => void;
  rovingTabIndex?: number;
}) {
  const tileRef = useRef<HTMLDivElement | null>(null);
  const [hovered, setHovered] = useState(false);
  const [focused, setFocused] = useState(false);
  const [menuPoint, setMenuPoint] = useState<TileMenuState | null>(null);
  const selected = useEditorUiStore((s) => s.selectedFolderIds.has(folder.id));
  const t = useT();

  const removeSelectedFolders = () => {
    void deleteSelectedFolders().catch((error) => {
      useEditorUiStore.getState().pushToast(String(error));
    });
  };
  const removeContextFolder = () => {
    void deleteFolderFromContextMenu(folder.id).catch((error) => {
      useEditorUiStore.getState().pushToast(String(error));
    });
  };
  const openFolder = () => openMediaFolder(folder.id, onOpen);

  return (
    <div
      ref={tileRef}
      onClick={(event) => {
        event.currentTarget.focus();
        selectMediaFolder(folder.id);
      }}
      onDoubleClick={openFolder}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      onFocus={(event) => {
        setFocused(true);
        if (event.target === event.currentTarget) selectMediaFolder(folder.id);
      }}
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) setFocused(false);
      }}
      onContextMenu={(event) => {
        event.preventDefault();
        event.stopPropagation();
        setMenuPoint({
          x: event.clientX,
          y: event.clientY,
          restoreFocus: false,
        });
      }}
      title={folder.name}
      role="gridcell"
      tabIndex={rovingTabIndex}
      aria-selected={selected}
      aria-haspopup="menu"
      data-media-tile="true"
      data-media-folder-id={folder.id}
      onKeyDown={(e) => {
        if (e.target !== e.currentTarget) return;
        if (handleMediaTileArrowNavigation(e)) return;
        if (e.key === "Enter") {
          e.preventDefault();
          e.stopPropagation();
          selectMediaFolder(folder.id);
          openFolder();
        } else if (e.key === " ") {
          e.preventDefault();
          e.stopPropagation();
          selectMediaFolder(folder.id);
        } else if (e.key === "Delete" || e.key === "Backspace") {
          e.preventDefault();
          e.stopPropagation();
          if (!e.repeat) removeSelectedFolders();
        } else if (e.key === "ContextMenu" || (e.shiftKey && e.key === "F10")) {
          e.preventDefault();
          e.stopPropagation();
          setMenuPoint({
            ...keyboardMenuPoint(e.currentTarget),
            restoreFocus: true,
          });
        }
      }}
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 4,
        cursor: "pointer",
        borderRadius: "var(--radius-sm)",
        outline: focused ? "2px solid var(--accent-primary)" : "none",
        outlineOffset: 2,
      }}
    >
      <div
        style={{
          aspectRatio: "5 / 4",
          background: hovered ? "var(--bg-raised)" : "var(--bg-placeholder)",
          border: `${selected ? "var(--bw-thick)" : "var(--bw-thin)"} solid ${selected || hovered ? "var(--accent-primary)" : "var(--border-primary)"}`,
          borderRadius: "var(--radius-sm)",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: selected || hovered ? "var(--accent-primary)" : "var(--text-secondary)",
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
      {menuPoint && (
        <TileContextMenu
          point={menuPoint}
          onClose={() => setMenuPoint(null)}
          returnFocus={tileRef}
          restoreFocus={menuPoint.restoreFocus}
          actions={[
            { label: t("common.open"), onSelect: openFolder },
            {
              label: t("contextMenu.delete"),
              onSelect: removeContextFolder,
              destructive: true,
            },
          ]}
        />
      )}
    </div>
  );
}

export function MediaCard({
  item,
  rovingTabIndex = 0,
}: {
  item: MediaItem;
  rovingTabIndex?: number;
}) {
  const t = useT();
  const cardRef = useRef<HTMLDivElement | null>(null);
  const thumbnailRef = useRef<HTMLDivElement | null>(null);
  const fps = useProjectStore((s) => s.timeline.fps);
  const projectEpoch = useProjectStore((s) => s.projectEpoch);
  const selected = useEditorUiStore((s) => s.selectedMediaAssetIds.has(item.id));
  const durationFrames = Math.round(item.duration * fps);
  const favorite = item.favorite ?? false;
  const generationActive =
    item.generationStatus === "generating" || item.generationStatus === "downloading";
  const generationFailed =
    item.generationStatus === "failed" || item.generationStatus === "cancelled";
  const thumbnailKey = mediaThumbnailKey(item);
  const [lazyThumbnail, setLazyThumbnail] = useState<string | null>(
    item.thumbnail ?? mediaThumbnailCache.get(thumbnailKey) ?? null,
  );
  // Offline assets shouldn't try to load a (now-missing) thumbnail.
  const thumb = item.missing ? null : assetUrl(lazyThumbnail);
  const [hovered, setHovered] = useState(false);
  const [focused, setFocused] = useState(false);
  const [menuPoint, setMenuPoint] = useState<TileMenuState | null>(null);
  const [feedback, setFeedback] = useState<string | null>(null);
  const [audioWaveformVisible, setAudioWaveformVisible] = useState(false);

  const activate = () => {
    selectMediaAsset(item.id);
    if (generationActive) return;
    useEditorUiStore.getState().setPreviewMedia(item.id);
    // Warm poster/sprite/waveform caches so preview + a later timeline drop
    // are instant instead of decoding on the interaction path.
    void preloadMedia(item.id);
  };

  const removeSelectedMedia = () => {
    void deleteSelectedMediaAssets().catch((error) => {
      useEditorUiStore.getState().pushToast(String(error));
    });
  };

  const removeContextMedia = () => {
    void deleteMediaFromContextMenu(item.id).catch((error) => {
      useEditorUiStore.getState().pushToast(String(error));
    });
  };

  useEffect(() => {
    setLazyThumbnail(item.thumbnail ?? mediaThumbnailCache.get(thumbnailKey) ?? null);
  }, [item.thumbnail, thumbnailKey]);

  useEffect(() => {
    if (item.type !== "audio" || item.missing) {
      setAudioWaveformVisible(false);
      return;
    }
    const el = cardRef.current;
    if (!el || typeof IntersectionObserver === "undefined") {
      setAudioWaveformVisible(true);
      return;
    }
    const observer = new IntersectionObserver(
      ([entry]) => setAudioWaveformVisible(Boolean(entry?.isIntersecting)),
      { root: null, rootMargin: "160px" },
    );
    observer.observe(el);
    return () => observer.disconnect();
  }, [item.id, item.type, item.missing]);

  useEffect(() => {
    if (item.missing || item.thumbnail || (item.type !== "video" && item.type !== "image")) {
      return;
    }
    let cancelled = false;
    let handle: ReturnType<typeof requestMediaCardThumbnail> | null = null;
    const request = () => {
      if (cancelled || useProjectStore.getState().projectEpoch !== projectEpoch) return;
      derivedResourceScheduler.activateProject(projectEpoch);
      handle = requestMediaCardThumbnail(item, projectEpoch);
      void handle.promise
        .then((path) => {
          if (!cancelled && path) setLazyThumbnail(path);
        })
        .catch(() => undefined);
    };
    const el = cardRef.current;
    if (!el || typeof IntersectionObserver === "undefined") {
      request();
      return () => {
        cancelled = true;
        handle?.cancel();
      };
    }
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (cancelled || !entry?.isIntersecting) return;
        observer.disconnect();
        request();
      },
      { root: null, rootMargin: "160px" },
    );
    observer.observe(el);
    return () => {
      cancelled = true;
      handle?.cancel();
      observer.disconnect();
    };
  }, [item, projectEpoch, thumbnailKey]);

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
    if (thumbnailRef.current) {
      setMediaThumbnailDragImage(e.dataTransfer, thumbnailRef.current);
    }
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
      draggable={!generationActive}
      onDragStart={onDragStart}
      onDragEnd={onDragEnd}
      onClick={(event) => {
        event.currentTarget.focus();
        activate();
      }}
      onDoubleClick={() => {
        if (!generationActive && !generationFailed) {
          void addMediaToTimeline(item).catch(reportMediaPlacementFailure);
        }
      }}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      onFocus={(event) => {
        setFocused(true);
        if (event.target === event.currentTarget) activate();
      }}
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) setFocused(false);
      }}
      onContextMenu={(event) => {
        event.preventDefault();
        event.stopPropagation();
        setMenuPoint({
          x: event.clientX,
          y: event.clientY,
          restoreFocus: false,
        });
      }}
      onKeyDown={(event) => {
        if (event.target !== event.currentTarget) return;
        if (handleMediaTileArrowNavigation(event)) return;
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          event.stopPropagation();
          activate();
        } else if (event.key === "Delete" || event.key === "Backspace") {
          event.preventDefault();
          event.stopPropagation();
          if (!event.repeat) removeSelectedMedia();
        } else if (event.key === "ContextMenu" || (event.shiftKey && event.key === "F10")) {
          event.preventDefault();
          event.stopPropagation();
          setMenuPoint({
            ...keyboardMenuPoint(event.currentTarget),
            restoreFocus: true,
          });
        }
      }}
      title={item.name}
      role="gridcell"
      tabIndex={rovingTabIndex}
      aria-selected={selected}
      aria-haspopup="menu"
      data-media-tile="true"
      data-media-asset-id={item.id}
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 4,
        cursor: generationActive ? "progress" : generationFailed ? "default" : "grab",
        borderRadius: "var(--radius-sm)",
        outline: focused ? "2px solid var(--accent-primary)" : "none",
        outlineOffset: 2,
      }}
    >
      {/* Thumbnail: generated cache image only. Missing thumbnails are requested
          lazily as cards enter view, so import/list commands stay cheap. */}
      <div
        ref={thumbnailRef}
        style={{
          position: "relative",
          aspectRatio: "5 / 4",
          background: "var(--bg-placeholder)",
          border: `${item.missing || selected ? "var(--bw-thick)" : "var(--bw-thin)"} solid ${item.missing ? "rgb(255,59,48)" : selected ? "var(--accent-primary)" : "var(--border-primary)"}`,
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
        ) : item.type === "audio" ? (
          <AudioWaveform
            mediaRef={item.id}
            projectEpoch={projectEpoch}
            sourceKey={thumbnailKey}
            enabled={audioWaveformVisible}
            missing={item.missing}
            fallback={<Icon icon={TYPE_ICON[item.type]} size={22} strokeWidth={1.5} />}
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
        {generationActive && (
          <div
            onClick={(event) => event.stopPropagation()}
            style={{
              position: "absolute",
              inset: 0,
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              justifyContent: "center",
              gap: 6,
              background: "rgba(15,18,24,0.76)",
              color: "#fff",
              textAlign: "center",
              padding: 8,
            }}
          >
            <Icon icon={Sparkles} size={18} />
            <span style={{ fontSize: "var(--fs-micro)", fontWeight: "var(--fw-medium)" }}>
              {item.generationStatus === "downloading" ? "正在下载结果" : "正在生成"}
              {typeof item.generationProgress === "number"
                ? ` ${Math.round(item.generationProgress * 100)}%`
                : ""}
            </span>
            {item.generationInput?.jobId && (
              <button
                type="button"
                onClick={(event) => {
                  event.stopPropagation();
                  void cancelGeneration(item.generationInput!.jobId!);
                }}
                style={{
                  minWidth: 24,
                  minHeight: 24,
                  fontSize: "var(--fs-micro)",
                  padding: "0 8px",
                  borderRadius: "var(--radius-xs)",
                  background: "rgba(255,255,255,0.14)",
                  color: "#fff",
                }}
              >
                取消
              </button>
            )}
          </div>
        )}
        {generationFailed && (
          <div
            onClick={(event) => event.stopPropagation()}
            style={{
              position: "absolute",
              inset: 0,
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              justifyContent: "center",
              gap: 4,
              background: "rgba(180,35,35,0.55)",
              color: "#fff",
              textAlign: "center",
              padding: 6,
            }}
          >
            <Icon icon={AlertTriangle} size={18} />
            <span style={{ fontSize: "var(--fs-micro)", fontWeight: "var(--fw-medium)" }}>
              {item.generationStatus === "cancelled"
                ? "生成已取消"
                : item.generationErrorCode ?? "GENERATION_FAILED"}
            </span>
            {item.generationInput?.jobId && (
              <button
                type="button"
                onClick={(event) => {
                  event.stopPropagation();
                  const approved = window.confirm(
                    "重试会再次调用生成服务并可能产生费用。是否继续？",
                  );
                  if (approved) {
                    void retryGeneration(item.generationInput!.jobId!, true);
                  }
                }}
                style={{
                  minWidth: 24,
                  minHeight: 24,
                  fontSize: "var(--fs-micro)",
                  padding: "0 8px",
                  borderRadius: "var(--radius-xs)",
                  background: "rgba(255,255,255,0.16)",
                  color: "#fff",
                }}
              >
                重试
              </button>
            )}
          </div>
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
                minWidth: 24,
                minHeight: 24,
                fontSize: "var(--fs-micro)",
                fontWeight: "var(--fw-medium)",
                padding: "0 8px",
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
        <MediaFavoriteButton
          assetId={item.id}
          favorite={favorite}
          title={favorite ? t("media.unfavorite") : t("media.favorite")}
          onSuccess={async (media, project) => {
            if (!applyMediaListForProject(project, media)) return;
            await useLibraryStore.getState().refresh();
          }}
          onError={(message, project) => {
            if (!applyMediaErrorForProject(project, message)) return;
            setFeedback(message);
          }}
          onStart={() => setFeedback(null)}
        />
        {canExtractAudio && (
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
              width: 24,
              height: 24,
              display: "inline-flex",
              alignItems: "center",
              justifyContent: "center",
              borderRadius: "var(--radius-xs)",
              background: "rgba(0,0,0,0.6)",
              color: "var(--text-secondary)",
              cursor: "pointer",
              opacity: hovered || focused ? 1 : 0,
              pointerEvents: hovered || focused ? "auto" : "none",
              transition: "opacity var(--anim-hover, 150ms) ease-out",
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
      {menuPoint && (
        <TileContextMenu
          point={menuPoint}
          onClose={() => setMenuPoint(null)}
          returnFocus={cardRef}
          restoreFocus={menuPoint.restoreFocus}
          actions={[
            {
              label: t("contextMenu.delete"),
              onSelect: removeContextMedia,
              destructive: true,
            },
          ]}
        />
      )}
    </div>
  );
}

interface MediaFavoriteButtonProps {
  assetId: string;
  favorite: boolean;
  title: string;
  onStart?: () => void;
  onSuccess: (
    media: Awaited<ReturnType<typeof toggleFavorite>>,
    project: MediaProjectIdentity,
  ) => void | Promise<void>;
  onError: (message: string, project: MediaProjectIdentity) => void;
  performToggle?: typeof toggleFavorite;
}

/** The card's durable-favorite interaction. Its pending state lives here so a
 * rejection cannot optimistically alter the `favorite` prop rendered from the
 * Rust mirror. Exported for a real DOM regression of the async contract. */
export function MediaFavoriteButton({
  assetId,
  favorite,
  title,
  onStart,
  onSuccess,
  onError,
  performToggle = toggleFavorite,
}: MediaFavoriteButtonProps) {
  const [pending, setPending] = useState(false);
  return (
    <button
      type="button"
      aria-label={title}
      aria-pressed={favorite}
      aria-busy={pending}
      disabled={pending}
      title={title}
      onClick={(event) => {
        event.stopPropagation();
        const project = captureMediaProjectIdentity();
        setPending(true);
        onStart?.();
        void performToggle(assetId, !favorite, project)
          .then((media) => {
            if (!isCurrentMediaProject(project)) return;
            return onSuccess(media, project);
          })
          .catch((error: unknown) => {
            if (!isCurrentMediaProject(project)) return;
            onError(String(error), project);
          })
          .finally(() => setPending(false));
      }}
      style={{
        position: "absolute",
        left: 4,
        top: 4,
        zIndex: 2,
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        width: 24,
        height: 24,
        padding: 0,
        borderRadius: "var(--radius-xs)",
        background: "rgba(0,0,0,0.6)",
        color: favorite ? "var(--accent-timecode)" : "var(--text-secondary)",
        cursor: pending ? "wait" : "pointer",
        opacity: pending ? 0.55 : 1,
      }}
    >
      <Icon icon={Star} size={12} strokeWidth={2} fill={favorite ? "currentColor" : "none"} />
    </button>
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

/** 音频卡片的波形缩略图（#91-B3）。复用 `get_waveform` 命令拿归一化桶
 *  (0=响, 1=静)，采样到固定条数渲染竖条。decode 失败 / 无音频轨 / 空桶时回退
 *  到调用方提供的类型图标，避免卡片缩略图区域变空白。 */
export function AudioWaveform({
  mediaRef,
  projectEpoch,
  sourceKey,
  enabled = true,
  missing,
  fallback,
  bucketsOverride,
}: {
  mediaRef: string;
  projectEpoch: number;
  sourceKey?: string;
  enabled?: boolean;
  missing?: boolean;
  fallback: React.ReactNode;
  bucketsOverride?: number[] | null;
}) {
  const [buckets, setBuckets] = useState<number[] | null>(bucketsOverride ?? null);
  useEffect(() => {
    if (bucketsOverride !== undefined) return;
    if (!enabled || missing) return;
    let cancelled = false;
    setBuckets(null);
    derivedResourceScheduler.activateProject(projectEpoch);
    const handle = derivedResourceScheduler.request<number[] | null>({
      projectEpoch,
      kind: derivedResourceKinds.waveform,
      key: `waveform:${sourceKey ?? mediaRef}`,
      priority: "background",
      run: () => getWaveform(mediaRef),
    });
    void handle.promise
      .then((next) => {
        if (!cancelled) setBuckets(next);
      })
      .catch(() => {
        if (!cancelled) setBuckets(null);
      });
    return () => {
      cancelled = true;
      handle.cancel();
    };
  }, [mediaRef, projectEpoch, sourceKey, enabled, missing, bucketsOverride]);
  if (!buckets || buckets.length === 0) return <>{fallback}</>;
  const sampled = sampleWaveform(buckets, 48);
  return (
    <div
      data-testid="audio-waveform"
      style={{
        display: "flex",
        alignItems: "center",
        gap: 1,
        width: "70%",
        height: "50%",
      }}
    >
      {sampled.map((v, i) => {
        const h = Math.max(8, (1 - v) * 100);
        return (
          <div
            key={i}
            style={{
              flex: 1,
              height: `${h}%`,
              minHeight: 2,
              background: "var(--accent-primary)",
              opacity: 0.65,
              borderRadius: 1,
            }}
          />
        );
      })}
    </div>
  );
}

/** 把任意长度的波形桶采样到 `target` 条：取每段代表点的值（无插值）。 */
function sampleWaveform(buckets: number[], target: number): number[] {
  if (buckets.length <= target) return buckets;
  const step = buckets.length / target;
  const out: number[] = new Array(target);
  for (let i = 0; i < target; i++) {
    out[i] = buckets[Math.floor(i * step)];
  }
  return out;
}
