/**
 * FolderBrowser (剪映-style nested folder browser, #49). A grid view that lists
 * a directory's subdirectories + importable media files, letting the user
 * navigate the file system without a native dialog. Double-click a folder to
 * descend; the breadcrumb bar + back button navigate back up. Files are
 * draggable to the timeline (MEDIA_DND_TYPE carries the file path); an "Import
 * This Folder" button imports the current directory's media tree into the
 * library (recursive, mirroring `importFolderViaDialog`).
 *
 * Outside Tauri there is no file system, so the browser shows a fallback
 * message instead of an empty grid.
 */

import { useEffect, useState } from "react";
import type { DragEvent } from "react";
import {
  Folder,
  FileVideo,
  FileAudio,
  Image as ImageIcon,
  ChevronRight,
  ArrowLeft,
  Home,
  FolderOpen,
} from "lucide-react";
import { Icon } from "../ui/Icon";
import { HoverButton } from "../ui/HoverButton";
import { useT } from "../../i18n";
import { listFolder, isTauri } from "../../lib/api";
import { importFolderByPath } from "../../store/mediaActions";
import { assetUrl } from "../../lib/asset";
import { MEDIA_DND_TYPE } from "./MediaPanel";
import { useMediaStore } from "../../store/mediaStore";
import type { FolderEntry } from "../../lib/types";

const MEDIA_TYPE_ICON: Record<string, typeof FileVideo> = {
  video: FileVideo,
  audio: FileAudio,
  image: ImageIcon,
};

/** One breadcrumb segment: a display name + the absolute path it navigates to. */
interface Crumb {
  name: string;
  path: string;
}

/**
 * Split an absolute `cwd` into breadcrumb segments (name + accumulated path).
 * Handles both `/` (Unix) and `\` (Windows) separators so the browser works
 * cross-platform. The first segment on Windows is the drive letter (e.g. `C:`).
 */
function breadcrumbs(cwd: string): Crumb[] {
  const sep = cwd.includes("\\") ? "\\" : "/";
  const parts = cwd.split(/[\\/]/).filter(Boolean);
  const segs: Crumb[] = [];
  let acc = cwd.startsWith("/") ? "/" : "";
  for (const part of parts) {
    acc = acc && !acc.endsWith(sep) ? acc + sep + part : acc + part;
    segs.push({ name: part, path: acc });
  }
  return segs;
}

export function FolderBrowser() {
  const t = useT();
  const importing = useMediaStore((s) => s.importing);
  const importError = useMediaStore((s) => s.error);
  const [cwd, setCwd] = useState<string | null>(null);
  const [entries, setEntries] = useState<FolderEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);

  // Re-fetch the directory listing whenever the current directory changes.
  useEffect(() => {
    if (!isTauri) return;
    let cancelled = false;
    setLoading(true);
    setLoadError(null);
    listFolder(cwd)
      .then((list) => {
        if (!cancelled) setEntries(list);
      })
      .catch((e: unknown) => {
        if (!cancelled) setLoadError(e instanceof Error ? e.message : String(e));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [cwd]);

  // Outside Tauri there is no file system — degrade to a fallback message.
  if (!isTauri) {
    return (
      <div
        style={{
          flex: 1,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: "var(--text-muted)",
          fontSize: "var(--fs-sm-md)",
          padding: "var(--space-lg)",
          textAlign: "center",
        }}
      >
        {t("media.folder.notAvailable")}
      </div>
    );
  }

  const crumbs = cwd ? breadcrumbs(cwd) : [];

  /** Navigate up one level: to the parent segment, or Home (null) at the root. */
  const goUp = () => {
    if (!cwd) return;
    if (crumbs.length <= 1) setCwd(null);
    else setCwd(crumbs[crumbs.length - 2].path);
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%", width: "100%" }}>
      {/* Breadcrumb + actions bar */}
      <div
        style={{
          flex: "0 0 auto",
          display: "flex",
          alignItems: "center",
          gap: "var(--space-xs)",
          padding: "var(--space-xs) var(--space-sm)",
          background: "var(--bg-surface)",
          borderBottom: "var(--bw-thin) solid var(--border-primary)",
        }}
      >
        <HoverButton title={t("media.folder.browse")} disabled={!cwd} onClick={goUp}>
          <Icon icon={ArrowLeft} size={13} />
        </HoverButton>
        {/* Breadcrumb trail: Home > segment1 > segment2 … */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 2,
            flex: 1,
            minWidth: 0,
            overflow: "hidden",
          }}
        >
          <button
            type="button"
            onClick={() => setCwd(null)}
            title={t("media.folder.browse")}
            style={{
              display: "inline-flex",
              alignItems: "center",
              color: cwd ? "var(--text-secondary)" : "var(--text-primary)",
              fontSize: "var(--fs-xs)",
              cursor: "pointer",
              background: "none",
              border: "none",
              padding: 0,
            }}
          >
            <Icon icon={Home} size={12} />
          </button>
          {crumbs.map((c) => (
            <span
              key={c.path}
              style={{ display: "inline-flex", alignItems: "center", gap: 2, minWidth: 0 }}
            >
              <span style={{ color: "var(--text-muted)", display: "inline-flex" }}>
                <Icon icon={ChevronRight} size={11} />
              </span>
              <button
                type="button"
                onClick={() => setCwd(c.path)}
                title={c.path}
                style={{
                  color: "var(--text-secondary)",
                  fontSize: "var(--fs-xs)",
                  cursor: "pointer",
                  background: "none",
                  border: "none",
                  padding: 0,
                  maxWidth: 120,
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                  whiteSpace: "nowrap",
                }}
              >
                {c.name}
              </button>
            </span>
          ))}
        </div>
        {/* Import the current folder's media tree into the library. */}
        {cwd && (
          <button
            type="button"
            onClick={() => void importFolderByPath(cwd)}
            disabled={importing}
            title={t("media.folder.import")}
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: 4,
              height: 24,
              padding: "0 8px",
              borderRadius: "var(--radius-sm)",
              background: "var(--bg-raised)",
              border: "var(--bw-thin) solid var(--border-primary)",
              color: "var(--text-secondary)",
              fontSize: "var(--fs-xs)",
              fontWeight: "var(--fw-medium)",
              cursor: importing ? "default" : "pointer",
              opacity: importing ? 0.6 : 1,
            }}
          >
            <Icon icon={FolderOpen} size={12} />
            {importing ? t("media.importing") : t("media.folder.import")}
          </button>
        )}
      </div>

      {/* Errors (directory listing or import) */}
      {(loadError || importError) && (
        <div
          style={{
            color: "var(--status-error)",
            fontSize: "var(--fs-xs)",
            padding: "var(--space-xs) var(--space-sm)",
          }}
        >
          {loadError ?? t("media.importFailed", { error: importError ?? "" })}
        </div>
      )}

      {/* Grid / loading / empty states */}
      {loading ? (
        <div
          style={{
            flex: 1,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            color: "var(--text-muted)",
            fontSize: "var(--fs-sm)",
          }}
        >
          {t("media.importing")}
        </div>
      ) : entries.length === 0 ? (
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
          {t("media.folder.empty")}
        </div>
      ) : (
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
          {entries.map((entry) => (
            <FolderEntryCard key={entry.path} entry={entry} onOpenDir={setCwd} />
          ))}
        </div>
      )}
    </div>
  );
}

/**
 * One grid cell: a folder (double-click to descend) or a media file (draggable
 * to the timeline). Folders render a Folder glyph; image/video files render
 * their decoded first frame via the asset protocol (matching `MediaCard`);
 * audio files render a type glyph.
 */
function FolderEntryCard({
  entry,
  onOpenDir,
}: {
  entry: FolderEntry;
  onOpenDir: (path: string) => void;
}) {
  const isDir = entry.isDir;
  const thumb = isDir ? null : assetUrl(entry.path);
  const glyph = isDir
    ? Folder
    : MEDIA_TYPE_ICON[entry.mediaType ?? ""] ?? FileVideo;

  const onDragStart = (e: DragEvent) => {
    e.dataTransfer.setData(MEDIA_DND_TYPE, entry.path);
    e.dataTransfer.effectAllowed = "copy";
  };

  return (
    <div
      draggable={!isDir}
      onDragStart={onDragStart}
      onDoubleClick={() => {
        if (isDir) onOpenDir(entry.path);
      }}
      title={entry.name}
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 4,
        cursor: isDir ? "pointer" : "grab",
      }}
    >
      <div
        style={{
          position: "relative",
          aspectRatio: "5 / 4",
          background: "var(--bg-placeholder)",
          border: "var(--bw-thin) solid var(--border-primary)",
          borderRadius: "var(--radius-sm)",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: "var(--text-muted)",
          overflow: "hidden",
        }}
      >
        {/* `draggable={false}` on inner media so the card's custom drag
            (MEDIA_DND_TYPE) wins instead of a native image/video drag. */}
        {thumb && entry.mediaType === "image" ? (
          <img
            src={thumb}
            alt={entry.name}
            draggable={false}
            style={{ width: "100%", height: "100%", objectFit: "cover" }}
          />
        ) : thumb && entry.mediaType === "video" ? (
          <video
            src={`${thumb}#t=0.1`}
            muted
            playsInline
            preload="metadata"
            draggable={false}
            style={{ width: "100%", height: "100%", objectFit: "cover" }}
          />
        ) : (
          <Icon icon={glyph} size={22} strokeWidth={1.5} />
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
        {entry.name}
      </span>
    </div>
  );
}
