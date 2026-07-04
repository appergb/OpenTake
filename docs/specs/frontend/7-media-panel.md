# MediaPanel（媒体面板）

> 来源:`MediaPanel/MediaPanelView.swift` + `MediaTab/` + `CaptionsTab/` + `MusicTab.swift`。

### 7.1 外层 + 标签轨（`MediaPanelView.swift`）

`HStack(spacing:0)`：左**标签轨**(垂直，宽 `tabRailWidth=38`) + 右**内容区**（`MediaPanelView.swift:20-35`）。右边缘 1px `border-primary`（`36-38`）。

**标签轨**（`panelTabRail` `55-78`）：`VStack(spacing: xs=4)` + 三个标签：Media(`folder`)/Captions(`captions.bubble`)/Music(`music.note`)（`PanelTab` `9-18`）。背景 `--bg-raised`，右边 1px `border-primary`，水平/上/下 padding `sm=6`。
- 标签按钮（`panelTabButton` `80-109`）：图标 `fontSize=md(13)`（selected semibold 否则 medium），frame `IconSize.lg(26)`，selected `text-primary` 否则 `text-tertiary`，`hoverHighlight(radius.sm, isActive:selected)`；**selected 左侧竖 capsule**（`border-primary`，宽 `bw-thick=2`，高 `IconSize.sm=18`）（`92-98`）。
- **hover 标签提示**（`hoverLabel` `111-129`）：hover 时右侧浮出 capsule 标签（`bg-prominent` + `border-primary` 描 + `shadow.sm`），`fontSize=xs(10) medium`。
- tab 切换动画 `easeInOut 0.2s`（`84`）。

### 7.2 Media tab（`MediaTab/MediaTab.swift`，30KB）

**结构**（`84-156`）：`VStack`：toolbar + (可选 swap banner) + grid 区(ZStack：drop area + toast) + (可选 GenerationView 底部展开)。

**Toolbar**（`254-264`）：`VStack(spacing: xs=4)`：actionsRow + searchControlsRow + contextBar。水平 padding `sm=6`，上 `sm`，下 `xs`，背景 `--bg-surface`。
- **actionsRow**（`266-284`，高 `panelHeaderHeight=28`）：Import 按钮(`plus`) + Generate 按钮(`sparkles`，filled，aiGradient) + 溢出菜单 + Spacer + 搜索索引状态。
- **searchControlsRow**（`286-294`，高 28）：搜索框 + 显示控制（视图模式/缩略图大小/排序/筛选 menu 图标）。
- **displayControls**（`354-404`）：
  - 视图模式 menu（`rectangle.grid.2x2`）：Folders/Flat/Grouped（`ViewMode` `37-55`）+ 缩略图大小 Small80/Medium110/Large150/XL200（`ThumbnailPreset` `61-80`）。
  - 排序 menu（`arrow.up.arrow.down`）：SortMode 各项。
  - 筛选 menu（`line.3.horizontal.decrease`，有筛选时 `accent.primary`）：按类型(video/audio/image) + AI Generated + Clear Filters。
- **contextBar**（`312-322`，高 `contextRowHeight=22`）：面包屑/视图名 + Spacer + 项数。面包屑（`breadcrumbBar` `337-352`）：Library ▸ folder ▸ …，chip 可点导航 + 可拖入。

**网格**（三种视图，`MediaTab+Grids.swift`）：folder/flat/grouped。cell = `AssetTile`（缩略图）或 `FolderTile`。
- 缩略图大小由 `thumbnailSize`(80–200) 控制。
- **搜索结果**（`MediaTab+Search.swift`）：非空查询时显示视觉/语音命中分区。

**AssetTile**（`MediaTab/AssetThumbnailView.swift`）：缩略图卡片，含类型角标、AI 标记、时长、选中态、生成中 overlay。**交互**：拖到时间线（payload=asset id + 可选时段，`MediaTab+Drag.swift`）、双击（预览 tab）、右键菜单、点击选择（Shift/⌘ 多选，方向键移动选择见 §9.6）。

**FolderTile**（`FolderTileView.swift`）：双击进入、拖入资产、重命名（inline）、右键。

**Drop area**（`MediaPanelDropArea.swift`，**AppKit 原生**，因 SwiftUI 父 onDrop 遮蔽问题）：Finder 文件拖入整面板。前端用整区 drop handler。

### 7.3 Captions tab（`CaptionsTab/CaptionTab.swift`）

`ScrollView` 表单（`CaptionTab.swift:53-84`）：Source 节（音频源选择）+ Style 节（字体/大小/颜色/描边样式，实时预览 "Captions will look like this"）+ Placement 节（位置，中心吸附 0.02）+ 底部 Generate 条。生成中显示 `GeneratingOverlay("Transcribing…")`。参数：textCase/censorProfanity/locale/translate 语言列表（`42-45`）。字号默认 48（`AppTheme.Caption.defaultFontSize`），范围 12–300。

### 7.4 Music tab（`MusicTab.swift`）

音乐选取/生成（结构同 tab 模式）。逐项对照源码复刻。
