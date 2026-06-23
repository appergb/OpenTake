/**
 * MediaPanel (SPEC §7). Left vertical tab rail (Media/Captions/Music) + content.
 * The Media tab shows the actions/search/context toolbar and the asset grid.
 * Asset data comes from the `get_media` command via `mediaStore`; importing is
 * driven by the native dialog (folder or files, CapCut-style). Grid items are
 * HTML5-draggable onto the timeline (see `MediaGrid` / `TimelineRegion`).
 * Captions/Music tabs are scaffolded.
 */

import { useEffect, useRef, useState } from "react";
import {
  Folder,
  Captions,
  Music,
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
} from "lucide-react";
import { Icon } from "../ui/Icon";
import { HoverButton } from "../ui/HoverButton";
import { useEditorUiStore, type MediaTabId } from "../../store/uiStore";
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

/** MIME-ish type used on dataTransfer when dragging a media item to the timeline. */
export const MEDIA_DND_TYPE = "application/x-opentake-media";

const TABS: Array<{ id: MediaTabId; icon: typeof Folder; labelKey: string }> = [
  { id: "media", icon: Folder, labelKey: "media.tab.media" },
  { id: "captions", icon: Captions, labelKey: "media.tab.captions" },
  { id: "music", icon: Music, labelKey: "media.tab.music" },
];

export function MediaPanel() {
  const mediaTab = useEditorUiStore((s) => s.mediaTab);
  const setMediaTab = useEditorUiStore((s) => s.setMediaTab);
  const t = useT();

  return (
    <div style={{ display: "flex", height: "100%", width: "100%" }}>
      <TabRail active={mediaTab} onSelect={setMediaTab} />
      <div style={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column" }}>
        {mediaTab === "media" && <MediaTab />}
        {mediaTab === "captions" && <Placeholder label={t("media.tab.captions")} />}
        {mediaTab === "music" && <Placeholder label={t("media.tab.music")} />}
      </div>
    </div>
  );
}

function TabRail({ active, onSelect }: { active: MediaTabId; onSelect: (t: MediaTabId) => void }) {
  const [hovered, setHovered] = useState<MediaTabId | null>(null);
  const t = useT();
  return (
    <div
      style={{
        width: "var(--tab-rail-width)",
        flex: "0 0 auto",
        background: "var(--bg-raised)",
        borderRight: "var(--bw-thin) solid var(--border-primary)",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        gap: "var(--space-xs)",
        padding: "var(--space-sm)",
      }}
    >
      {TABS.map((tab) => {
        const selected = active === tab.id;
        const label = t(tab.labelKey);
        return (
          <div
            key={tab.id}
            style={{ position: "relative" }}
            onMouseEnter={() => setHovered(tab.id)}
            onMouseLeave={() => setHovered(null)}
          >
            {/* Selection capsule on the left edge. */}
            {selected && (
              <div
                style={{
                  position: "absolute",
                  left: -6,
                  top: "50%",
                  transform: "translateY(-50%)",
                  width: "var(--bw-thick)",
                  height: "var(--icon-sm)",
                  background: "var(--border-primary)",
                  borderRadius: 1,
                }}
              />
            )}
            <HoverButton
              title={label}
              active={selected}
              size={26}
              onClick={() => onSelect(tab.id)}
            >
              <Icon icon={tab.icon} size={13} strokeWidth={selected ? 2.4 : 2} />
            </HoverButton>
            {/* Hover label capsule. */}
            {hovered === tab.id && !selected && (
              <div
                style={{
                  position: "absolute",
                  left: 32,
                  top: "50%",
                  transform: "translateY(-50%)",
                  background: "var(--bg-prominent)",
                  border: "var(--bw-thin) solid var(--border-primary)",
                  borderRadius: "var(--radius-sm)",
                  boxShadow: "var(--shadow-sm)",
                  padding: "2px 8px",
                  fontSize: "var(--fs-xs)",
                  fontWeight: "var(--fw-medium)",
                  color: "var(--text-secondary)",
                  whiteSpace: "nowrap",
                  zIndex: 10,
                  pointerEvents: "none",
                }}
              >
                {label}
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}

function MediaTab() {
  const t = useT();
  const items = useMediaStore((s) => s.items);
  const importing = useMediaStore((s) => s.importing);
  const error = useMediaStore((s) => s.error);

  return (
    <>
      {/* Toolbar */}
      <div
        style={{
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
          <span>{importing ? t("media.importing") : t("media.itemCount", { count: items.length })}</span>
        </div>
        {error && (
          <div style={{ color: "var(--status-error)", fontSize: "var(--fs-xs)" }}>
            {t("media.importFailed", { error })}
          </div>
        )}
      </div>

      {items.length === 0 ? <EmptyState /> : <MediaGrid items={items} />}
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

function EmptyState() {
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
      {t("media.empty")}
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
