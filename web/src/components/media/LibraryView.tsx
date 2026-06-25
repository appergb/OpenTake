/**
 * 全局素材库页(#56)。这是【跨项目】的全局收藏库,与项目内 `MediaPanel`(项目媒体)
 * 是两套独立的东西:数据走全新的 `library_*` 命令(#55)+ `libraryStore`,不复用
 * mediaStore。整页是独立全屏视图(uiStore.view === "library"),Home 与编辑器都能进。
 *
 * 布局:左侧分类树(全部 / 视频 / 音频 / 音效 / 图片 / 特效 + 自建分类),右侧工具条
 * (搜索 + 排序)+ 网格。收藏在所有子库视图都可见(「全部」聚合),通过派生函数
 * `selectEntries` 在前端过滤/排序,切分类不重拉。
 *
 * 每个条目卡片:缩略图(asset 协议解码原文件,缺省回退类型字形)、文件名、以及
 * 「导入当前项目 / 改分类 / 取消收藏」操作。导入走 `importToProject`(库→项目 manifest
 * 造新 asset,再 refreshMedia)。所有命令在非 Tauri 下安全降级。
 */

import { useEffect, useMemo, useState } from "react";
import {
  Home,
  Search,
  Star,
  Film,
  Music,
  AudioWaveform,
  Image as ImageIcon,
  Sparkles,
  Layers,
  Tag,
  Import,
  Trash2,
  type LucideIcon,
} from "lucide-react";
import { Icon } from "../ui/Icon";
import { useT } from "../../i18n";
import { useEditorUiStore } from "../../store/uiStore";
import { assetUrl } from "../../lib/asset";
import type { LibraryEntry } from "../../lib/libraryApi";
import {
  useLibraryStore,
  startLibrarySync,
  selectEntries,
  deriveCustomCategories,
  sourceName,
  type SortKey,
} from "../../store/libraryStore";

/** 内置分类 → 图标 + i18n 键。顺序即树中显示顺序。 */
const BUILTIN_CATEGORIES: ReadonlyArray<{ id: string; icon: LucideIcon; labelKey: string }> = [
  { id: "all", icon: Star, labelKey: "library.cat.all" },
  { id: "video", icon: Film, labelKey: "library.cat.video" },
  { id: "audio", icon: Music, labelKey: "library.cat.audio" },
  { id: "sound", icon: AudioWaveform, labelKey: "library.cat.sound" },
  { id: "image", icon: ImageIcon, labelKey: "library.cat.image" },
  { id: "effect", icon: Sparkles, labelKey: "library.cat.effect" },
];

const SORTS: ReadonlyArray<{ id: SortKey; labelKey: string }> = [
  { id: "recent", labelKey: "library.sort.recent" },
  { id: "oldest", labelKey: "library.sort.oldest" },
  { id: "type", labelKey: "library.sort.type" },
];

/** 素材类型 → 卡片回退字形(无缩略图时)。 */
function typeIcon(type: string): LucideIcon {
  switch (type) {
    case "video":
      return Film;
    case "audio":
      return Music;
    case "image":
      return ImageIcon;
    case "effect":
      return Sparkles;
    default:
      return Layers;
  }
}

export function LibraryView() {
  const t = useT();
  const setView = useEditorUiStore((s) => s.setView);
  const entries = useLibraryStore((s) => s.entries);
  const loading = useLibraryStore((s) => s.loading);
  const error = useLibraryStore((s) => s.error);
  const selectedCategory = useLibraryStore((s) => s.selectedCategory);
  const search = useLibraryStore((s) => s.search);
  const sort = useLibraryStore((s) => s.sort);
  const setSearch = useLibraryStore((s) => s.setSearch);
  const setSort = useLibraryStore((s) => s.setSort);

  useEffect(() => {
    void startLibrarySync();
  }, []);

  const visible = useMemo(
    () => selectEntries(entries, selectedCategory, search, sort),
    [entries, selectedCategory, search, sort],
  );
  const customCategories = useMemo(() => deriveCustomCategories(entries), [entries]);

  return (
    <div
      style={{
        display: "flex",
        height: "100%",
        width: "100%",
        background: "var(--bg-base)",
        color: "var(--text-primary)",
      }}
    >
      <CategoryTree custom={customCategories} />

      <main
        style={{
          flex: 1,
          minWidth: 0,
          display: "flex",
          flexDirection: "column",
          background:
            "radial-gradient(120% 80% at 100% 0%, rgba(245,239,228,0.05), transparent 60%), var(--bg-surface)",
        }}
      >
        {/* 顶部条:返回主页 + 标题 + 搜索 + 排序 */}
        <header
          data-tauri-drag-region
          style={{
            display: "flex",
            alignItems: "center",
            gap: "var(--space-md)",
            padding: "var(--titlebar-safe-top) var(--space-xl) var(--space-md)",
          }}
        >
          <button
            type="button"
            title={t("title.backHome")}
            aria-label={t("title.backHome")}
            onClick={() => setView("home")}
            className="hover-area"
            style={{
              width: 26,
              height: 26,
              display: "inline-flex",
              alignItems: "center",
              justifyContent: "center",
              borderRadius: "var(--radius-sm)",
              color: "var(--text-secondary)",
            }}
          >
            <Icon icon={Home} size={14} />
          </button>
          <h1
            style={{
              margin: 0,
              fontSize: "var(--fs-md-lg)",
              fontWeight: "var(--fw-semibold)",
              letterSpacing: "var(--tracking-tight)",
            }}
          >
            {t("library.title")}
          </h1>

          <div style={{ flex: 1 }} />

          <SearchBox value={search} onChange={setSearch} placeholder={t("library.search")} />
          <SortSelect value={sort} onChange={setSort} />
        </header>

        {error && (
          <div
            style={{
              margin: "0 var(--space-xl) var(--space-sm)",
              padding: "var(--space-sm) var(--space-md)",
              borderRadius: "var(--radius-sm)",
              background: "rgba(255,59,48,0.12)",
              color: "var(--status-error)",
              fontSize: "var(--fs-sm)",
            }}
          >
            {error}
          </div>
        )}

        <Grid entries={visible} loading={loading} totalEmpty={entries.length === 0} />
      </main>
    </div>
  );
}

function CategoryTree({ custom }: { custom: ReadonlyArray<string> }) {
  const t = useT();
  const selectedCategory = useLibraryStore((s) => s.selectedCategory);
  const setSelectedCategory = useLibraryStore((s) => s.setSelectedCategory);

  return (
    <aside
      style={{
        width: 200,
        flex: "0 0 auto",
        display: "flex",
        flexDirection: "column",
        gap: "var(--space-xxs)",
        padding: "var(--titlebar-safe-top) var(--space-sm) var(--space-xl)",
        background: "var(--bg-raised)",
        borderRight: "var(--bw-thin) solid var(--border-primary)",
        overflowY: "auto",
      }}
    >
      <div
        style={{
          padding: "0 var(--space-sm) var(--space-md)",
          fontSize: "var(--fs-xs)",
          fontWeight: "var(--fw-semibold)",
          letterSpacing: "var(--tracking-wide)",
          textTransform: "uppercase",
          color: "var(--text-muted)",
        }}
      >
        {t("library.libraries")}
      </div>

      {BUILTIN_CATEGORIES.map((c) => (
        <CategoryRow
          key={c.id}
          icon={c.icon}
          label={t(c.labelKey)}
          active={selectedCategory === c.id}
          onClick={() => setSelectedCategory(c.id)}
        />
      ))}

      {custom.length > 0 && (
        <div
          style={{
            padding: "var(--space-md) var(--space-sm) var(--space-xs)",
            fontSize: "var(--fs-xs)",
            fontWeight: "var(--fw-semibold)",
            letterSpacing: "var(--tracking-wide)",
            textTransform: "uppercase",
            color: "var(--text-muted)",
          }}
        >
          {t("library.myCategories")}
        </div>
      )}
      {custom.map((name) => (
        <CategoryRow
          key={name}
          icon={Tag}
          label={name}
          active={selectedCategory === name}
          onClick={() => setSelectedCategory(name)}
        />
      ))}
    </aside>
  );
}

function CategoryRow({
  icon,
  label,
  active,
  onClick,
}: {
  icon: LucideIcon;
  label: string;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="hover-area"
      style={{
        display: "flex",
        alignItems: "center",
        gap: "var(--space-sm)",
        width: "100%",
        height: 32,
        padding: "0 var(--space-sm)",
        borderRadius: "var(--radius-sm)",
        textAlign: "left",
        fontSize: "var(--fs-md)",
        fontWeight: active ? "var(--fw-semibold)" : "var(--fw-medium)",
        color: active ? "var(--text-primary)" : "var(--text-secondary)",
        background: active ? "var(--bg-selected, rgba(255,255,255,0.06))" : "transparent",
      }}
    >
      <Icon icon={icon} size={15} />
      <span
        style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}
      >
        {label}
      </span>
    </button>
  );
}

function SearchBox({
  value,
  onChange,
  placeholder,
}: {
  value: string;
  onChange: (v: string) => void;
  placeholder: string;
}) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: "var(--space-xs)",
        height: 28,
        padding: "0 var(--space-sm)",
        borderRadius: "var(--radius-sm)",
        background: "var(--bg-raised)",
        border: "var(--bw-thin) solid var(--border-primary)",
        color: "var(--text-tertiary)",
      }}
    >
      <Icon icon={Search} size={13} />
      <input
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        style={{
          width: 160,
          border: "none",
          outline: "none",
          background: "transparent",
          color: "var(--text-primary)",
          fontSize: "var(--fs-sm)",
        }}
      />
    </div>
  );
}

function SortSelect({ value, onChange }: { value: SortKey; onChange: (s: SortKey) => void }) {
  const t = useT();
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value as SortKey)}
      title={t("library.sortBy")}
      aria-label={t("library.sortBy")}
      style={{
        height: 28,
        padding: "0 var(--space-sm)",
        borderRadius: "var(--radius-sm)",
        background: "var(--bg-raised)",
        border: "var(--bw-thin) solid var(--border-primary)",
        color: "var(--text-secondary)",
        fontSize: "var(--fs-sm)",
        outline: "none",
      }}
    >
      {SORTS.map((s) => (
        <option key={s.id} value={s.id}>
          {t(s.labelKey)}
        </option>
      ))}
    </select>
  );
}

function Grid({
  entries,
  loading,
  totalEmpty,
}: {
  entries: ReadonlyArray<LibraryEntry>;
  loading: boolean;
  totalEmpty: boolean;
}) {
  const t = useT();

  if (entries.length === 0) {
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
        {loading ? t("library.loading") : totalEmpty ? t("library.empty") : t("library.noMatch")}
      </div>
    );
  }

  return (
    <div
      style={{
        flex: 1,
        overflowY: "auto",
        padding: "var(--space-md) var(--space-xl) var(--space-xl)",
      }}
    >
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fill, minmax(150px, 1fr))",
          gap: "var(--space-lg)",
          alignContent: "start",
        }}
      >
        {entries.map((e) => (
          <EntryCard key={e.id} entry={e} />
        ))}
      </div>
    </div>
  );
}

function EntryCard({ entry }: { entry: LibraryEntry }) {
  const t = useT();
  const importToProject = useLibraryStore((s) => s.importToProject);
  const unfavorite = useLibraryStore((s) => s.unfavorite);
  const categorize = useLibraryStore((s) => s.categorize);
  const [hovered, setHovered] = useState(false);
  const [busy, setBusy] = useState(false);

  const name = sourceName(entry.source) || entry.id;
  // 缩略图:库条目 thumb 优先,否则按 source 让 WebView 解码原文件(asset 协议)。
  const thumb = assetUrl(entry.thumb ?? entry.source);

  const handleImport = async () => {
    setBusy(true);
    try {
      await importToProject(entry.id);
    } finally {
      setBusy(false);
    }
  };

  const handleCategorize = () => {
    const next = window.prompt(t("library.categorizePrompt"), entry.category ?? "");
    if (next === null) return; // 取消
    const trimmed = next.trim();
    void categorize(entry.id, trimmed === "" ? null : trimmed);
  };

  return (
    <div
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      title={name}
      style={{ display: "flex", flexDirection: "column", gap: 4, position: "relative" }}
    >
      <div
        style={{
          position: "relative",
          aspectRatio: "5 / 4",
          background: "var(--bg-placeholder)",
          border: `var(--bw-thin) solid ${hovered ? "var(--border-divider)" : "var(--border-primary)"}`,
          borderRadius: "var(--radius-sm)",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: "var(--text-muted)",
          overflow: "hidden",
        }}
      >
        {thumb && entry.type === "image" ? (
          <img
            src={thumb}
            alt={name}
            style={{ width: "100%", height: "100%", objectFit: "cover" }}
          />
        ) : thumb && entry.type === "video" ? (
          <video
            src={`${thumb}#t=0.1`}
            muted
            preload="metadata"
            style={{ width: "100%", height: "100%", objectFit: "cover" }}
          />
        ) : (
          <Icon icon={typeIcon(entry.type)} size={26} strokeWidth={1.4} />
        )}

        {/* hover 操作行:导入 / 改分类 / 取消收藏 */}
        {hovered && (
          <div
            style={{
              position: "absolute",
              top: "var(--space-xs)",
              right: "var(--space-xs)",
              display: "flex",
              gap: 4,
            }}
          >
            <CardAction
              icon={Import}
              title={t("library.import")}
              onClick={() => void handleImport()}
              disabled={busy}
            />
            <CardAction icon={Tag} title={t("library.categorize")} onClick={handleCategorize} />
            <CardAction
              icon={Trash2}
              title={t("library.unfavorite")}
              danger
              onClick={() => void unfavorite(entry.id)}
            />
          </div>
        )}
      </div>

      <div
        style={{
          fontSize: "var(--fs-sm)",
          color: "var(--text-secondary)",
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}
      >
        {name}
      </div>
    </div>
  );
}

function CardAction({
  icon,
  title,
  onClick,
  danger,
  disabled,
}: {
  icon: LucideIcon;
  title: string;
  onClick: () => void;
  danger?: boolean;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      title={title}
      aria-label={title}
      disabled={disabled}
      onClick={onClick}
      className="hover-area"
      style={{
        width: "var(--icon-lg)",
        height: "var(--icon-lg)",
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        borderRadius: "var(--radius-sm)",
        background: "rgba(0,0,0,0.55)",
        color: danger ? "var(--status-error)" : "var(--text-primary)",
        opacity: disabled ? 0.5 : 1,
      }}
    >
      <Icon icon={icon} size={13} />
    </button>
  );
}
