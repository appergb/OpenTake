# 窗口外壳与五面板布局

> 来源:`Editor/EditorView.swift`(NSSplitViewController 实现)、`Constants.swift`(LayoutPreset + Layout 尺寸)、`Editor/EditorWindowController.swift`、`Editor/TitleBarView.swift`、`App/MainMenu.swift`。

### 2.1 五个面板 + LayoutPreset

五个叶面板（`EditorViewModel.FocusedPanel`，`EditorViewModel.swift:35-44`）：**agent / media / preview / inspector / timeline**。
（注：`agent` 面板属另一个 Issue，本规格只在布局位置上为其预留列，内部 UI 不展开。）

三种布局预设（`LayoutPreset`，`Constants.swift:3-23`），可在 View 菜单/⌘1/⌘2/⌘3 切换，持久化到 `localStorage["layoutPreset"]`（上游 `UserDefaults`，`EditorViewModel.swift:99-107`）：

| preset | label | 结构 |
|---|---|---|
| `.default` | Default | 见 2.2 |
| `.media` | Media | 见 2.3 |
| `.vertical` | Vertical | 见 2.4 |

最外层永远是「**agent 列（左）｜ preset 子树（右）**」的水平 split（`EditorView.swift:182-202`）。agent 列默认折叠（`agentPanelVisible` 默认 `false`，`EditorViewModel.swift:135-137`），宽度区间 `[agentPanelMin=240, agentPanelMax=640]`（`Constants.swift:35-36`）；preset 子树 `minimumThickness=400`（`EditorView.swift:201`）。

### 2.2 Default 布局（`EditorView.swift:207-230`）

```
┌──────────────────────────── preset 子树 ────────────────────────────┐
│  上半(水平 split):  [Media] | [Preview] | [Inspector]               │  ← 高度 = 70%
│  ───────────────────────────────────────────────────────────────── │  (target.height*0.7)
│  下半:  [Toolbar(38) + Timeline]                                    │  ← 30%
└─────────────────────────────────────────────────────────────────────┘
```

- 垂直 split 分割位:`setPosition(round(targetH*0.7), dividerAt:0)`（`EditorView.swift:226`）。
- 上半水平三栏:Media 宽 `mediaPanelDefault=500`（左,`Constants.swift:27`,`EditorView.swift:227`）;Inspector 宽 `inspectorDefault=260`（右,`Constants.swift:31`,`EditorView.swift:228`）;Preview 居中吃剩余。
- 上半 `minimumThickness = previewMinHeight = 320`（`EditorView.swift:217`,`Constants.swift:57`）。

### 2.3 Media 布局（`EditorView.swift:235-261`）

```
┌─────────┬──────────────────────────────────────┐
│         │  上(水平): [Preview] | [Inspector]      │ ← 高 55%
│ [Media] │  ──────────────────────────────────── │
│  (30%)  │  下: [Toolbar + Timeline]              │ ← 45%
└─────────┴──────────────────────────────────────┘
```

- 最外水平:Media 宽 = `round(targetW*0.3)`（`EditorView.swift:256-257`）。
- 右侧垂直 split:上半 = `round(rightH*0.55)`（`258`）。
- 上半水平:Inspector 宽 `inspectorDefault=260`,Preview 吃剩余（`259`）。

### 2.4 Vertical 布局（`EditorView.swift:266-288`）

```
┌────────────────────────────────┬───────────┐
│ 上(水平): [Media] | [Inspector]  │           │
│ ────────────────────────────── │ [Preview] │
│ 下: [Toolbar + Timeline]        │           │
└────────────────────────────────┴───────────┘
```

- 最外水平:左子树宽 = `round(targetW*0.5)`,右 = Preview（`EditorView.swift:284`）。
- 左侧垂直:上半 = `round(leftH*0.55)`（`285`）。
- 上半水平:Media 宽 `mediaPanelDefault=500`,Inspector 吃剩余（`286`）。

> **实现建议**：用 CSS Grid + 可拖拽分隔条（如 `react-resizable-panels` 或 `allotment`），并在 React 内**复刻初始百分比/像素分割位**。三种 preset = 三套不同 grid 模板。分隔条命中区加宽（上游 `effectiveRect` 把分隔条 hit 区上下/左右各扩 `panelGap/2 = 2.5px`，`EditorView.swift:24-36`）。

### 2.5 面板外观「shell」（每个叶面板的统一包装，`EditorView.swift:331-350`）

每个叶面板被包成：
1. 内容 `background = --bg-surface`（rgb 22）；
2. `clipShape(RoundedRectangle(cornerRadius: radius.sm=6, style:.continuous))` → CSS `border-radius: 6px`；
3. `padding(panelGap/2 = 2.5px)`；
4. 外层 `background = --bg-base`（rgb 10）—— 即面板之间露出的"沟槽"是 base 色，每个面板是 surface 色的圆角卡片浮在上面；
5. 焦点环叠加（见 2.6）。

> 视觉净效果:深色底(base)上漂浮一组 surface 圆角卡片,卡片间 5px 沟槽。**必须照搬**:这是上游辨识度最高的外观特征。

### 2.6 面板焦点环 PanelFocusRing（`EditorView.swift:384-396`）

- 当前聚焦面板:`RoundedRectangle(radius.sm=6).strokeBorder(accent.primary, lineWidth: 1.5).opacity(0.6)`,非聚焦 `opacity 0`,过渡 `easeOut 0.2s`。
- `allowsHitTesting(false)`(不拦截鼠标)。
- 聚焦由点击决定:`EditorWindowController.handlePanelClick`（`EditorWindowController.swift:183-194`）—— 点击落在哪个面板,该面板的 `accessibilityIdentifier`(= `"<panel>Panel"`,`EditorViewModel.swift:38`)即设为 `focusedPanel`。**副作用**:点进 media 面板清空 `selectedClipIds`;点进 timeline 面板清空 `selectedMediaAssetIds`（`EditorWindowController.swift:188-189`）。

### 2.7 面板可见性 / 折叠 / 最大化

- **可见性切换**:media（⌘0）/ inspector（⌘⌥0）/ agent（⌘⌥A）（`MainMenu.swift:104-114`）。状态持久化:`mediaPanelVisible`(默认 true)、`inspectorPanelVisible`(默认 true)、`agentPanelVisible`(默认 false)（`EditorViewModel.swift:135-151`）。preview / timeline 不可折叠(`canCollapse=false` 仅 media/inspector/agent;preview/timeline 永远在)。
- **最大化**:聚焦面板 + 反引号键 `` ` ``（无修饰键）→ `maximizedPanel`（`MainMenu.swift:118-120`,`EditorWindowController.swift:123-128`）。最大化时折叠所有兄弟,Esc 退出（`EditorWindowController.swift:152-155`）。前端等价:把目标面板 grid 区域扩满,其余 `display:none`。
- 折叠用动画(`item.animator().isCollapsed`,`EditorView.swift:164-166`)→ CSS 宽/高过渡。

### 2.8 窗口 chrome / 标题栏（`Editor/TitleBarView.swift`，窗口尺寸 `AppTheme.swift:231-237`）

Tauri 自定义标题栏区域：
- **Leading（左）**：Agent 面板切换按钮 —— 图标 `bubble.left(.fill)`，`fontSize=md(13)`，`foregroundStyle = aiGradient`，可见时 `opacity 1` 否则 `0.55`，frame `IconSize.lg(26)`，`hoverHighlight`，help "Toggle Agent Panel"（`TitleBarView.swift:6-19`）。
- **Trailing（右）**：`Spacer` → UpdateBadge(可选,OpenTake 用自己的更新机制,可暂略) → **Export 按钮**（图标 `square.and.arrow.up` 上移 1px + 文字 "Export"，`fontSize=sm(11) medium`，`text-secondary`，水平 padding `sm(6)`，高 `IconSize.lg(26)`，`hoverHighlight`，help "Export (⌘E)"，点击 `showExportDialog=true`）→ UserAvatar(账户,属另一 Issue)（`TitleBarView.swift:22-48`）。
- 窗口默认尺寸 `projectDefault = 1600×1000`，最小 `projectMin = 960×600`，标题栏 trailing 预留宽 280（`AppTheme.swift:234-236`）。

### 2.9 主菜单 / 应用菜单（`App/MainMenu.swift`）

Tauri 原生菜单（macOS）/ 应用内菜单（Win/Linux）需复刻以下项与快捷键（**§9.6 是完整快捷键表**）。菜单结构（`MainMenu.swift:8-163`）：

- **App**：About / Check for Updates… / Settings…(⌘,) / Quit(⌘Q)
- **File**：New(⌘N) / Open…(⌘O) / Save(⌘S) / Save As…(⇧⌘S) / Import Media…(⌘I) / Export…(⌘E)
- **Edit**：Undo(⌘Z) / Redo(⇧⌘Z) / Cut(⌘X) / Copy(⌘C) / Paste(⌘V) / Select All(⌘A) / Split at Playhead(⌘K) / Trim Start to Playhead(Q) / Trim End to Playhead(W) / Delete(⌫)
- **View**：Media Panel(⌘0) / Inspector(⌘⌥0) / Agent Panel(⌘⌥A) / Maximize Focused Panel(`` ` ``) / Layout ▸ {Default ⌘1, Media ⌘2, Vertical ⌘3} / Enter Full Screen(⌘F)
- **Help**：Tutorial / Keyboard Shortcuts(⌘?) / MCP Instructions / Send Feedback…

菜单项勾选态:layout 三项、三个面板可见性、最大化态都带 checkmark/state(`EditorWindowController.validateMenuItem`,`EditorWindowController.swift:270-303`)。
