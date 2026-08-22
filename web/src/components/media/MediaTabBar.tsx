/**
 * 剪映式顶部素材面板标签条。
 * - MediaTabBar：九个主标签横排（素材/音频/音乐/文本/贴纸/特效/转场/字幕/智能包裹），
 *   选中=白+加粗+底部下划线；可用未选=次级灰+hover 提亮；禁用=极弱灰+不可点。
 *   素材/音频/音乐/文本/字幕已接真实内容，其余为功能未做的置灰占位。
 * - MediaSubTabBar：素材/音频下的「导入 / 我的」二级 pill 切换。
 * 文案全部走 i18n（dict 里 media.tab.* / media.subtab.*），不硬编码中文。
 */

import {
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type MutableRefObject,
} from "react";
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
  { id: "music", labelKey: "media.tab.music", enabled: true },
  { id: "text", labelKey: "media.tab.text", enabled: true },
  { id: "sticker", labelKey: "media.tab.sticker", enabled: false },
  { id: "effect", labelKey: "media.tab.effect", enabled: false },
  { id: "transition", labelKey: "media.tab.transition", enabled: true },
  { id: "subtitle", labelKey: "media.tab.subtitle", enabled: true },
  { id: "smartPack", labelKey: "media.tab.smartPack", enabled: true },
];

export const MEDIA_MAIN_TAB_IDS: ReadonlyArray<MediaTabId> = MAIN_TABS.map((tab) => tab.id);

function roveTab<T extends string>(
  event: ReactKeyboardEvent<HTMLButtonElement>,
  ids: readonly T[],
  current: T,
  onSelect: (id: T) => void,
  refs: MutableRefObject<Partial<Record<T, HTMLButtonElement | null>>>,
) {
  let nextIndex: number | null = null;
  const currentIndex = Math.max(0, ids.indexOf(current));
  if (event.key === "ArrowRight" || event.key === "ArrowDown") {
    nextIndex = (currentIndex + 1) % ids.length;
  } else if (event.key === "ArrowLeft" || event.key === "ArrowUp") {
    nextIndex = (currentIndex - 1 + ids.length) % ids.length;
  } else if (event.key === "Home") {
    nextIndex = 0;
  } else if (event.key === "End") {
    nextIndex = ids.length - 1;
  }
  if (nextIndex === null) return;
  event.preventDefault();
  const next = ids[nextIndex];
  if (!next) return;
  onSelect(next);
  refs.current[next]?.focus();
}

export function MediaTabBar({
  active,
  onSelect,
}: {
  active: MediaTabId;
  onSelect: (tab: MediaTabId) => void;
}) {
  const t = useT();
  const [hovered, setHovered] = useState<MediaTabId | null>(null);
  const tabRefs = useRef<Partial<Record<MediaTabId, HTMLButtonElement | null>>>({});
  const enabledIds = MAIN_TABS.filter((tab) => tab.enabled).map((tab) => tab.id);

  return (
    <div
      role="tablist"
      aria-label={t("media.library")}
      aria-orientation="horizontal"
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
            id={`media-main-tab-${tab.id}`}
            aria-controls={`media-main-panel-${tab.id}`}
            aria-selected={selected}
            aria-disabled={!tab.enabled}
            data-media-main-tab
            data-preserve-timeline-selection={tab.id === "transition" ? "true" : undefined}
            tabIndex={selected && tab.enabled ? 0 : -1}
            disabled={!tab.enabled}
            ref={(element) => {
              tabRefs.current[tab.id] = element;
            }}
            onMouseEnter={() => tab.enabled && setHovered(tab.id)}
            onMouseLeave={() => setHovered(null)}
            onClick={() => {
              if (tab.enabled) onSelect(tab.id);
            }}
            onKeyDown={(event) => {
              if (tab.enabled) {
                roveTab(event, enabledIds, tab.id, onSelect, tabRefs);
              }
            }}
            style={{
              position: "relative",
              minHeight: 24,
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

/** 素材 tab 的二级标签：导入 / 我的。 */
export const MATERIAL_SUB_TABS: ReadonlyArray<SubTab> = [
  { id: "import", labelKey: "media.subtab.import" },
  { id: "mine", labelKey: "media.subtab.mine" },
];

/** 音频 tab 的二级标签：导入 / 我的 / 提取（从视频提取音频）/ 音效（全局音效库）。 */
export const AUDIO_SUB_TABS: ReadonlyArray<SubTab> = [
  { id: "import", labelKey: "media.subtab.import" },
  { id: "mine", labelKey: "media.subtab.mine" },
  { id: "extract", labelKey: "media.subtab.extract" },
  { id: "sound", labelKey: "media.subtab.sound" },
];

/** 二级 pill 切换。`tabs` 由调用方按主 tab 传入（素材 2 项 / 音频 4 项）。 */
export function MediaSubTabBar({
  active,
  onSelect,
  tabs = MATERIAL_SUB_TABS,
  idPrefix = "media-subtab",
}: {
  active: MediaSubTabId;
  onSelect: (tab: MediaSubTabId) => void;
  tabs?: ReadonlyArray<SubTab>;
  idPrefix?: string;
}) {
  const t = useT();
  const tabRefs = useRef<Partial<Record<MediaSubTabId, HTMLButtonElement | null>>>({});
  const tabIds = tabs.map((tab) => tab.id);
  return (
    <div
      role="tablist"
      aria-label={t("media.library")}
      aria-orientation="horizontal"
      style={{
        display: "inline-flex",
        gap: "var(--space-xs)",
        padding: 2,
        background: "var(--bg-raised)",
        border: "var(--bw-thin) solid var(--border-primary)",
        borderRadius: "var(--radius-md)",
      }}
    >
      {tabs.map((tab) => {
        const selected = active === tab.id;
        return (
          <button
            key={tab.id}
            type="button"
            role="tab"
            id={`${idPrefix}-${tab.id}`}
            aria-controls={`${idPrefix}-panel-${tab.id}`}
            aria-selected={selected}
            tabIndex={selected ? 0 : -1}
            ref={(element) => {
              tabRefs.current[tab.id] = element;
            }}
            onClick={() => onSelect(tab.id)}
            onKeyDown={(event) => roveTab(event, tabIds, tab.id, onSelect, tabRefs)}
            style={{
              minHeight: 24,
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
