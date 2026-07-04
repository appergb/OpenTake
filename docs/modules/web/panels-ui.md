# panels-ui — 面板与外壳（inspector / media / toolbar / home / settings / agent / shell / ui）

> 上级：[本模块目录](INDEX.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
>
> 覆盖时间线/预览之外的全部可视层：检查器、媒体面板、工具栏、主页、设置、Agent、窗口外壳与通用 UI 原始件。共同约定：**组件只渲染快照 + 派发动作，不持领域逻辑**；图标统一走 `lucide-react`；数值/颜色走 theme.ts / CSS 变量。

---

## 一句话职责

把后端镜像与 UI 态投影成各功能面板，并把用户操作归一到 [editActions](state-stores.md) / 各 actions，再由 Rust 真理回流刷新。

---

## shell（窗口外壳与五面板布局）

- `EditorSplit.tsx`：根布局控制器（SPEC §2.2-2.4）。按 `uiStore.layoutPreset` 在三种主布局间切换——**Default**（上层 Media|Preview|Inspector 三列 over 时间线）、**Media**（左 Media | 右 Preview|Inspector over 时间线）、**Vertical**（左 Media|Inspector over 时间线 | 右 Preview），并叠加 Agent 面板可见性；支持任一面板「最大化」全屏。尺寸基于 `ResizeObserver` 动态计算，分割点经 `SplitPane` 拖拽。
- `SplitPane.tsx`：可拖拽二叉分割条（horizontal/vertical + initial/min/secondMin），拖动回调 `onResize`。
- `TitleBar.tsx`：标题栏（含 macOS 红绿灯安全区、`ViewMenu` 入口）。
- `ViewMenu.tsx`：视图菜单（布局预设、面板显隐等）。
- 注：`PanelShell`（surface 圆角卡 + 焦点环 + 点击切焦）实现在 `ui/PanelShell.tsx`，包裹每个面板叶子。

## inspector（检查器）

- `Inspector.tsx`：属性检查器主体（SPEC §6）。四态：多选摘要 / 单选 clip 检查（Video·Audio·Text 标签）/ 无选时显工程元数据 / 媒体资产检查（占位）。**现场采样**：每次 render 从 `activeFrame` 取 clip 的动画值，故数值字段总显示播放头处当前值；已有关键帧轨的属性显示为只读「(animated)」并把编辑转到关键帧面板。所有编辑经 `editActions.setClipProperties()`。
- `KeyframesPanel.tsx` + `KeyframesLaneRow.tsx`：关键帧面板（SPEC §6.4）。单选 clip 下每个可动画属性一行（视频 position/scale/rotation/opacity/crop；音频 volume），顶部刻度尺 + 面板级红色播放头叠加，行内可拖拽菱形标记（→ stamp/move/remove/insertation 关键帧动作）。
- `ScrubbableNumberField.tsx`：可拖拽数值控件（SPEC §6.6）。水平拖拽改值（Shift×10 / Cmd×0.1），单击切文本输入（Enter/失焦提交、ESC 取消），动画属性时只读；`onCommit` 触发命令。
- `TextTab.tsx`：编辑 `Clip.textContent`（草稿本地态，失焦提交；字号/颜色/对齐等样式延后）。
- `SwapMediaSection.tsx`：检查器内的「替换素材」入口（→ `swapMedia`）。

## media（媒体面板与全局库）

- `MediaPanel.tsx`：媒体库容器。顶部主标签（Material/Audio/Text/Sticker/Effect/Transition/Captions/Smart Wrap，仅 Material·Audio 可用、余者置灰占位，仿剪映）；二级标签 Import/Mine（Mine=星标收藏，localStorage）。过滤管线不可变（audio 标签仅显纯音频，Mine 仅显收藏）。卡片 HTML5-draggable（`MEDIA_DND_TYPE`），单击预览、双击 `addMediaToTimeline`、星标切换、视频可「萃取音频」（`extractAudio` + 保存对话框）；离线素材红覆盖 + Relink。订阅 `uiStore.mediaTab/mediaSubTab`，消费 `mediaStore`。
- `LibraryView.tsx`：全局库整页视图（`view === "library"`，跨项目永久库），消费 `libraryStore`，支持分类/搜索/排序与「导入到项目」。
- `MediaTabBar.tsx`：主/次标签按钮组。
- `favorites.ts`：星标收藏 store（localStorage）。

## toolbar（工具条）

- `Toolbar.tsx`：顶部工具栏（SPEC §4，高 38px）。左：撤销/重做、指针/剃须刀、分割、修剪入/出点、文本(T)；右：对数缩放滑块。订阅 `uiStore.toolMode/zoomScale` 与 `projectStore.canUndo/canRedo`；操作全经 `editActions.*`（undo/redo/setToolMode/setZoomScale/splitAtPlayhead/trimStartToPlayhead/trimEndToPlayhead/addTextClip）。

## home / settings / agent

- `home/HomeView.tsx`：启动器。最近项目列表 / 空态 + 新建/打开/设置入口。消费 `recentStore`，调 `newProjectAndEnter`/`openProjectViaDialog`/`openProjectPath`。
- `settings/SettingsView.tsx`：模态设置。标签式：General(语言) / Appearance(主题) / Import(默认目录) / AI(BYOK 密钥) / About(版本·许可)。**BYOK 明文密钥存 OS keychain**（`secret_*`），不进 localStorage。消费 `settingsStore` 与 i18n。
- `agent/AgentPanel.tsx`：内置 Agent 聊天面板（当前占位，SPEC §2.1，后续接通）。

## ui（通用原始件）

- `Icon.tsx`：**lucide-react 薄包装**（确认）。接 `icon: LucideIcon` + `size`/`strokeWidth`/`fill`，继承 `currentColor`（SPEC §3.3）。全项目图标统一 `<Icon icon={Foo} size={13} />`。
- `HoverButton.tsx`：带悬停/活跃态的图标按钮（24×24 命中框，`hover-area` + `is-active`）。
- `Dropdown.tsx`：下拉菜单原始件。
- `PanelShell.tsx`：面板叶子包装（surface 圆角卡 radius 6、2.5px gap inset、焦点环，鼠标按下 `focusPanel`）。

---

## 完成状态

- **已实现**：三种布局 + 最大化、检查器（Video/Audio/Text + 现场采样 + 关键帧面板 + 可拖拽数值）、媒体面板（导入/拖放/双击/星标/萃取音频/Relink）、全局库页、工具栏、主页启动器、设置（含 BYOK keychain）、lucide 图标体系。
- **计划中/占位**：Agent 面板真实对话；媒体面板的 Text/Sticker/Effect/Transition/Captions/Smart Wrap 标签为置灰占位；检查器文本样式（字号/颜色/对齐）与媒体资产检查待补。

## 相关文档

- 各面板派发的动作 → [state-stores.md](state-stores.md)
- 密钥/库/对话框/asset 封装 → [ipc-api.md](ipc-api.md)
- 预览面板单独成篇 → [preview-ui.md](preview-ui.md)
- 布局/检查器/媒体面板规格 → [SPEC.md](SPEC.md)（§2 外壳布局、§6 Inspector、§7 MediaPanel）

---

## 页脚

- 本模块目录：[INDEX.md](INDEX.md)
- 模块文档树：[../INDEX.md](../INDEX.md)
- docs 总目录：[../../INDEX.md](../../INDEX.md)
