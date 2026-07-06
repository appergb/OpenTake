/**
 * 音频 tab 的「音效」子标签（#91）。把全局音效库（#115，`category==='sound'` 的收藏）
 * 直接搬进音频 tab，一键「添加到项目」——复用 `libraryStore`（真值在 Rust 的 #55 库
 * 命令），不新建后端。数据与全屏素材库页共享同一 store，切进来 refresh 一次保证最新。
 */

import { useEffect, useMemo, useState } from "react";
import { AudioWaveform, Plus } from "lucide-react";
import { useLibraryStore, selectEntries, sourceName } from "../../store/libraryStore";
import { useT } from "../../i18n";
import { Icon } from "../ui/Icon";

export function SoundLibraryTab({ query }: { query: string }) {
  const t = useT();
  const entries = useLibraryStore((s) => s.entries);
  const loading = useLibraryStore((s) => s.loading);
  const refresh = useLibraryStore((s) => s.refresh);
  const importToProject = useLibraryStore((s) => s.importToProject);

  // 进入音效子标签时拉一次全局库（与素材库页共享 store，保证收藏是最新的）。
  useEffect(() => {
    void refresh();
  }, [refresh]);

  const sounds = useMemo(
    () => selectEntries(entries, "sound", query, "recent"),
    [entries, query],
  );

  if (sounds.length === 0) {
    return (
      <div
        style={{
          flex: 1,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          padding: "var(--space-lg)",
          color: "var(--text-tertiary)",
          fontSize: "var(--fs-sm)",
          textAlign: "center",
        }}
      >
        {loading ? t("media.importing") : t("media.sound.empty")}
      </div>
    );
  }

  return (
    <div
      style={{
        flex: 1,
        minHeight: 0,
        overflowY: "auto",
        padding: "var(--space-sm)",
        display: "flex",
        flexDirection: "column",
        gap: "var(--space-xxs)",
      }}
    >
      {sounds.map((entry) => (
        <SoundRow
          key={entry.id}
          name={sourceName(entry.source) || entry.id}
          onImport={() => importToProject(entry.id)}
        />
      ))}
    </div>
  );
}

/** 单条音效：波形图标 + 文件名 + 「添加到项目」按钮（导入后短暂反馈）。 */
function SoundRow({
  name,
  onImport,
}: {
  name: string;
  onImport: () => Promise<string | null>;
}) {
  const t = useT();
  const [busy, setBusy] = useState(false);
  const [feedback, setFeedback] = useState<string | null>(null);

  const handleImport = async () => {
    if (busy) return;
    setBusy(true);
    const added = await onImport();
    setBusy(false);
    setFeedback(added ? t("media.sound.added", { name: added }) : t("media.sound.failed"));
    setTimeout(() => setFeedback(null), 1500);
  };

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: "var(--space-sm)",
        height: 34,
        padding: "0 var(--space-sm)",
        borderRadius: "var(--radius-sm)",
        background: "var(--bg-raised)",
      }}
    >
      <Icon icon={AudioWaveform} size={14} />
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
        {name}
      </span>
      {feedback ? (
        <span style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)" }}>{feedback}</span>
      ) : (
        <button
          type="button"
          onClick={handleImport}
          disabled={busy}
          title={t("media.sound.add")}
          aria-label={t("media.sound.add")}
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: 4,
            height: 22,
            padding: "0 8px",
            borderRadius: "var(--radius-sm)",
            border: "none",
            background: "var(--bg-prominent)",
            color: "var(--text-primary)",
            fontSize: "var(--fs-xs)",
            fontWeight: "var(--fw-medium)",
            cursor: busy ? "default" : "pointer",
            opacity: busy ? 0.6 : 1,
          }}
        >
          <Icon icon={Plus} size={11} />
          {t("media.sound.add")}
        </button>
      )}
    </div>
  );
}
