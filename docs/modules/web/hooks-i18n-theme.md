# hooks-i18n-theme — 横切：钩子 / 多语言 / 主题令牌 / 全局样式

> 上级：[本模块目录](INDEX.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
>
> 覆盖 `web/src/hooks/`、`web/src/i18n/`、`web/src/lib/theme.ts`、`web/src/styles/`。这些是被各面板共用的横切设施：自动保存、键盘快捷键、文案多语言、设计令牌（数值常量单一源）与全局基础样式。

---

## 一句话职责

提供「编辑器级」的副作用钩子与全局基建：让所有 UI 数值/颜色来自 theme.ts 与 CSS 变量、文案来自 i18n 字典、键盘与自动保存行为集中可控。

---

## hooks/

- `useAutosave.ts`：防抖自动保存。监听 `projectStore.timelineVersion`，当其超过 `lastSavedVersion`（即有未保存编辑）时延时 1500ms 调 `saveCurrentProject()`；仅 Tauri 且项目已打开时生效。
- `useKeyboardShortcuts.ts`：全局快捷键（按 `event.code` 物理键，兼容 ⌘/Ctrl）。
  - 修饰键：`Z`/`Shift+Z` 撤销·重做；`+`/`-` 缩放（步进 1.3）；`K`/`B` 播放头处分割；`S` 保存；`1-3` 布局预设、`0` 切媒体面板、`Alt+0` 切检查器、`Alt+A` 切 Agent 面板；`C`/`X`/`V` 复制·剪切·粘贴。
  - 无修饰键：`←`/`→` 逐 1/5 帧移播放头；`Backspace`/`Delete` 删除、`Shift+Delete` Ripple 删除；`Q`/`W` 修剪入/出点到播放头；`C`/`B` 进切割模式、`V`/`A` 回选择模式；`Shift+Z` 适应窗口；`` ` `` 最大化/还原面板；`Escape` 清选或退出最大化。
  - `Space` 播放/暂停（仅编辑器视图、且焦点不在文本输入框时）。

两个钩子均在 `App.tsx` 顶层无条件挂载（编辑器外为 no-op，以保持 hook 顺序稳定）。

## i18n/（轻量自研，无外部依赖）

- `dict.ts`：扁平 key→value 字典，支持两个 locale——**`zh-CN`（简体中文，默认）** 与 **`en`（英文）**；字符串内 `{placeholder}` 由 `vars` 插值。
- `index.ts`：Zustand 持久化当前 locale 到 localStorage（键 `locale`，默认 `zh-CN`）；`useT()` 返回记忆化翻译函数（切语言触发重渲染），`t()` 为命令式翻译器（用于非组件上下文，如行为标签），`initI18n()` 启动时写 `<html lang>`。`App.tsx` 启动调 `initI18n()`。

## lib/theme.ts — 设计令牌（数值常量单一源）

`theme.ts` 导出 `AppTheme` 系列常量，是**全部 UI 数值/颜色的唯一源**，逐字镜像上游 `AppTheme.swift`（上游强制「样式必须用 AppTheme，不得硬编码」）。分组导出：

- 颜色：`BG` / `BORDER` / `TEXT` / `ACCENT` / `TRACK_COLOR`（轨道类型色）。
- 尺度：`SPACE`（xxs..xxl）/ `RADIUS`（xs..xl）/ `FS`（字号 micro..xl）；`FONT_UI` / `FONT_MONO` 字体栈。
- 时间线几何：`LAYOUT`（标尺高 24、轨道头宽 100…）/ `TRIM` / `TRACK_SIZE` / `SNAP` / `ZOOM` / `PLAYHEAD_TRIANGLE`（8）/ `CLIP` / `FADE`，均为上游字面量。

约定：组件引用 `theme.ts` 导出或对应 CSS 变量，**不硬编码**数值/颜色（CLAUDE.md 风格要点）。

## styles/

- `tokens.css`：把 `AppTheme` 1:1 落成 `:root` CSS 自定义属性——`--bg-*` / `--border-*` / `--text-*` / `--accent-*`·`--status-*` / `--track-*` / `--radius-*` / `--space-*` / `--fs-*` / `--fw-*` / `--tracking-*` / `--font-*` / `--icon-*` / `--op-*` / `--shadow-*`，外加窗口安全区与时间线几何常量。
- `global.css`：`@import "./tokens.css"` 后设全局基础——盒模型重置、`html/body/#root` 清零与 `overflow`、正文背景/文本/字体与平滑、禁用选择与默认光标、`.hover-area`/`.is-active`/`.home-project-card:hover` 悬停态、按钮/输入重置、WebKit 滚动条自定义、`.zoom-slider`/`.tabular` 工具类。`main.tsx` 仅导入 `global.css`（tokens 经其 `@import` 引入）。

---

## 完成状态

- **已实现**：防抖自动保存、完整快捷键映射、zh-CN/en 双语运行时、AppTheme 全令牌 + tokens.css + global.css。
- **计划中**：更多 locale（字典已可扩展）；快捷键自定义。

## 相关文档

- 快捷键/自动保存最终落到的动作 → [state-stores.md](state-stores.md)
- 令牌被时间线几何消费 → [timeline-ui.md](timeline-ui.md)
- 令牌对应的上游来源 → [SPEC.md](SPEC.md)（§1 设计令牌表 AppTheme→CSS variables）

---

## 页脚

- 本模块目录：[INDEX.md](INDEX.md)
- 模块文档树：[../INDEX.md](../INDEX.md)
- docs 总目录：[../../INDEX.md](../../INDEX.md)
