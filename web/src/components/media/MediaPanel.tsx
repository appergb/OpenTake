/**
 * MediaPanel (SPEC §7 + 剪映式顶栏改造)。顶部横排主标签（素材/音频/文本/贴纸/
 * 特效/转场/字幕/智能包裹，仅素材/音频可用，其余置灰占位）取代了原左侧竖排
 * Media/Captions/Music 标签条。素材/音频下再分「导入 / 我的」二级标签：导入=全部
 * （音频标签仅 type==='audio'），我的=星标收藏（localStorage 持久化，见 favorites.ts）。
 * 内容区仍是 actions/search/context 工具栏 + 资产网格；网格项 HTML5-draggable 到
 * 时间线（见 `MediaGrid` / `TimelineRegion`）。
 */

import { useEffect, useRef, useState } from "react";
import {
  Plus,
  Sparkles,
  Filter,
  ArrowUpDown,
  LayoutGrid,
  FolderOpen,
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
import { assetUrl } from "../../lib/asset";
import { useProjectStore } from "../../store/projectStore";
import { addMediaToTimeline } from "../../store/editActions";
import type { MediaItem } from "../../lib/types";
import { MediaTabBar, MediaSubTabBar } from "./MediaTabBar";
import { useFavoritesStore, useIsFavorite } from "./favorites";

/** MIME-ish type used on dataTransfer when dragging a media item to the timeline. */
export const MEDIA_DND_TYPE = "application/x-opentake-media";

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
  const importing = useMediaStore((s) => s.importing);
  const error = useMediaStore((s) => s.error);
  const subTab = useEditorUiStore((s) => s.mediaSubTab);
  const setSubTab = useEditorUiStore((s) => s.setMediaSubTab);
  const favoriteIds = useFavoritesStore((s) => s.ids);

  // 过滤管线（全部不可变 filter，不改 store）：
  // 1) 按主标签——「音频」仅纯音频素材（严格 type==='audio'，不含有声视频，匹配剪映）；
  //    若日后需含「有音轨的视频」，改为 `item.type === "audio" || item.hasAudio`。
  //    「素材」显示全部类型。
  // 2) 按二级标签——「我的」仅星标收藏（命中当前已加载 items）；「导入」显示全部。
  const filtered = items.filter((item) => {
    if (kind === "audio" && item.type !== "audio") return false;
    if (subTab === "mine" && !favoriteIds.has(item.id)) return false;
    return true;
  });

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
          <span>{importing ? t("media.importing") : t("media.itemCount", { count: filtered.length })}</span>
        </div>
        {error && (
          <div style={{ color: "var(--status-error)", fontSize: "var(--fs-xs)" }}>
            {t("media.importFailed", { error })}
          </div>
        )}
      </div>

      {filtered.length === 0 ? <EmptyState subTab={subTab} /> : <MediaGrid items={filtered} />}
    </>
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

function EmptyState({ subTab }: { subTab: MediaSubTabId }) {
  const t = useT();
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
      {subTab === "mine" ? t("media.empty.mine") : t("media.empty")}
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

function MediaGrid({ items }: { items: MediaItem[] }) {
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
      {items.map((item) => (
        <MediaCard key={item.id} item={item} />
      ))}
    </div>
  );
}

function MediaCard({ item }: { item: MediaItem }) {
  const t = useT();
  const fps = useProjectStore((s) => s.timeline.fps);
  const setPreviewMedia = useEditorUiStore((s) => s.setPreviewMedia);
  const previewMediaId = useEditorUiStore((s) => s.previewMediaId);
  const durationFrames = Math.round(item.duration * fps);
  const selected = previewMediaId === item.id;
  const favorite = useIsFavorite(item.id);
  const toggleFavorite = useFavoritesStore((s) => s.toggle);
  // Offline assets shouldn't try to load a (now-missing) thumbnail.
  const thumb = item.missing ? null : assetUrl(item.path);

  const onDragStart = (e: React.DragEvent) => {
    e.dataTransfer.setData(MEDIA_DND_TYPE, item.id);
    e.dataTransfer.effectAllowed = "copy";
  };

  return (
    <div
      draggable
      onDragStart={onDragStart}
      onClick={() => setPreviewMedia(item.id)}
      onDoubleClick={() => void addMediaToTimeline(item)}
      title={item.name}
      style={{ display: "flex", flexDirection: "column", gap: 4, cursor: "grab" }}
    >
      {/* Thumbnail: the original file decoded by the WebView (asset protocol); a
          type glyph stands in when no resolvable path / outside Tauri. */}
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
            (MEDIA_DND_TYPE) wins instead of a native image/video drag. The
            `#t=0.1` fragment makes the WebView paint the first frame as a video
            poster (a metadata-only <video> otherwise stays blank). */}
        {thumb && item.type === "image" ? (
          <img
            src={thumb}
            alt={item.name}
            draggable={false}
            style={{ width: "100%", height: "100%", objectFit: "cover" }}
          />
        ) : thumb && item.type === "video" ? (
          <video
            src={`${thumb}#t=0.1`}
            muted
            playsInline
            preload="metadata"
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
