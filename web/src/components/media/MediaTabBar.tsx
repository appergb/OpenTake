/**
 * 剪映式顶部素材面板标签条。
 * - MediaTabBar：八个主标签横排（素材/音频/文本/贴纸/特效/转场/字幕/智能包裹），
 *   选中=白+加粗+底部下划线；可用未选=次级灰+hover 提亮；禁用=极弱灰+不可点。
 *   目前仅「素材/音频」可用，其余为功能未做的置灰占位。
 * - MediaSubTabBar：素材/音频下的「导入 / 我的」二级 pill 切换。
 * 文案全部走 i18n（dict 里 media.tab.* / media.subtab.*），不硬编码中文。
 */

import { useState } from "react";
import { useT } from "../../i18n";
import type { MediaTabId, MediaSubTabId } from "../../store/uiStore";

interface MainTab {
  id: MediaTabId;
  labelKey: string;
  enabled: boolean;
}

/** 主标签定义。enabled=false 的标签置灰不可点（功能未实现的占位）。 */
const MAIN_TABS: ReadonlyArray<MainTab> = [
  { id: "material", labelKey: "media.tab.material", enabled: true },
  { id: "audio", labelKey: "media.tab.audio", enabled: true },
  { id: "text", labelKey: "media.tab.text", enabled: false },
  { id: "sticker", labelKey: "media.tab.sticker", enabled: false },
  { id: "effect", labelKey: "media.tab.effect", enabled: false },
  { id: "transition", labelKey: "media.tab.transition", enabled: false },
  { id: "subtitle", labelKey: "media.tab.subtitle", enabled: false },
  { id: "smartPack", labelKey: "media.tab.smartPack", enabled: false },
];

export function MediaTabBar({
  active,
  onSelect,
}: {
  active: MediaTabId;
  onSelect: (tab: MediaTabId) => void;
}) {
  const t = useT();
  const [hovered, setHovered] = useState<MediaTabId | null>(null);

  return (
    <div
      role="tablist"
      style={{
        flex: "0 0 auto",
        display: "flex",
        alignItems: "stretch",
        gap: "var(--space-md)",
        padding: "0 var(--space-sm)",
        background: "var(--bg-surface)",
        borderBottom: "var(--bw-thin) solid var(--border-primary)",
        overflowX: "auto",
        overflowY: "hidden",
      }}
    >
      {MAIN_TABS.map((tab) => {
        const selected = active === tab.id;
        const color = !tab.enabled
          ? "var(--text-muted)"
          : selected
            ? "var(--text-primary)"
            : hovered === tab.id
              ? "var(--text-primary)"
              : "var(--text-secondary)";
        return (
          <button
            key={tab.id}
            type="button"
            role="tab"
            aria-selected={selected}
            aria-disabled={!tab.enabled}
            disabled={!tab.enabled}
            onMouseEnter={() => tab.enabled && setHovered(tab.id)}
            onMouseLeave={() => setHovered(null)}
            onClick={() => {
              if (tab.enabled) onSelect(tab.id);
            }}
            style={{
              position: "relative",
              padding: "var(--space-sm) 2px",
              background: "transparent",
              border: "none",
              color,
              fontSize: "var(--fs-sm-md)",
              fontWeight: selected ? "var(--fw-semibold)" : "var(--fw-medium)",
              cursor: tab.enabled ? "pointer" : "not-allowed",
              whiteSpace: "nowrap",
            }}
          >
            {t(tab.labelKey)}
            {/* 选中下划线（仅可用且选中时显示）。 */}
            {selected && tab.enabled && (
              <span
                style={{
                  position: "absolute",
                  left: 0,
                  right: 0,
                  bottom: 0,
                  height: "var(--bw-thick)",
                  background: "var(--accent-primary)",
                  borderRadius: 1,
                }}
              />
            )}
          </button>
        );
      })}
    </div>
  );
}

interface SubTab {
  id: MediaSubTabId;
  labelKey: string;
}

const SUB_TABS: ReadonlyArray<SubTab> = [
  { id: "import", labelKey: "media.subtab.import" },
  { id: "mine", labelKey: "media.subtab.mine" },
];

/** 二级 pill 切换：导入 / 我的。 */
export function MediaSubTabBar({
  active,
  onSelect,
}: {
  active: MediaSubTabId;
  onSelect: (tab: MediaSubTabId) => void;
}) {
  const t = useT();
  return (
    <div
      role="tablist"
      style={{
        display: "inline-flex",
        gap: "var(--space-xs)",
        padding: 2,
        background: "var(--bg-raised)",
        border: "var(--bw-thin) solid var(--border-primary)",
        borderRadius: "var(--radius-md)",
      }}
    >
      {SUB_TABS.map((tab) => {
        const selected = active === tab.id;
        return (
          <button
            key={tab.id}
            type="button"
            role="tab"
            aria-selected={selected}
            onClick={() => onSelect(tab.id)}
            style={{
              padding: "2px var(--space-sm-md)",
              borderRadius: "var(--radius-sm)",
              border: "none",
              background: selected ? "var(--bg-prominent)" : "transparent",
              color: selected ? "var(--text-primary)" : "var(--text-secondary)",
              fontSize: "var(--fs-sm)",
              fontWeight: selected ? "var(--fw-semibold)" : "var(--fw-medium)",
              cursor: "pointer",
              whiteSpace: "nowrap",
            }}
          >
            {t(tab.labelKey)}
          </button>
        );
      })}
    </div>
  );
}
