# Inspector（检查器）

> 来源:`Inspector/InspectorView.swift`(44KB) + `Components/` + `Keyframes/KeyframesLane.swift` + `TextTab.swift` + `AIEditTab.swift`。

### 6.1 顶层结构 + 标题栏（`InspectorView.swift:34-69`）

`VStack(spacing:0)`：标题栏 + 内容（依选择态四选一）。

**标题栏**（`37-47`）：`HStack(spacing: xs=4)`：图标 + 标题 + Spacer，水平 padding `lg=14`，应用 `panelHeaderBar()`（高 28，背景 `--bg-raised`，底部 1px `border-primary`，`AppTheme.swift:295-302`）。
- 标题/图标随态变（`23-31`）：选中 clip → "Inspector" + `slider.horizontal.3`；选中媒体资产 → "Source" + `info.circle`；否则 → "Timeline" + `info.circle`；框选中 → "Inspector" + `slider.horizontal.3`。

**内容四态**（`49-57`）：
1. 框选中 → marquee 摘要（"N selected" 居中，`text-tertiary`，`71-80`）。
2. 选中 clip(visual/audio) → clip 检查器（`clipInspectorContent`）。
3. 选中媒体资产 → 资产检查器（`mediaAssetInspectorContent`）。
4. 否则 → 工程元数据（`projectMetadataContent`）。

### 6.2 工程元数据态（`projectMetadataContent`，`95-161`）

`ScrollView` → 分节（spacing `xl=20`），每节标题全大写 `fontSize=xxs(9) semibold` + `tracking wide(1.5)` + `text-muted`（`metadataSection` `125-138`）。行 = label(`text-tertiary` xs) + Spacer + value(`text-secondary` xs，可选中，尾部截断)（`plainMetadataRow` `140-161`）。
- Project 节：Name / Path。
- Format 节：Resolution（`W × H`）/ Frame Rate（`N fps`）/ Aspect Ratio（约分，`gcd`）/ Duration（`formatDuration`）。

### 6.3 Clip 检查器 + Tab 栏（`clipInspectorContent` `212-241`，tab `170-283`）

**可用 tab**（`availableTabs` `170-183`，依选择）：
- 单个 text clip → `Text`。
- 有非 text 视觉 clip → `Video`。
- 有 audio clip → `Audio`。
- 单个 AI-可编辑视觉 clip → `AI Edit`（`aiEditEligible` `187-194`）。

**tab 栏**（`genericTabBar` `255-283`）：`HStack(spacing: md=10)`，每 tab = 文字（active `medium` 否则 `regular`）+ 底部下划线（active 显，`bw-medium=1.5`）。active 色 `text-primary`，非 active `text-tertiary`；**"AI Edit" tab 用 aiGradient**（active α1，否则 α0.6，`260-262`）。水平 padding `lg=14`，顶 padding `xs=4`。
- tab>1 才显示 tab 栏（`216`）。

**内容**：`ScrollView` → `VStack(spacing: lg=14)` → 依 tab：Text/Video/Audio/AI（`219-238`），内容 padding `lg=14`。

### 6.4 Video tab（`videoTabContent` `285-311`）

- Transform 节 + Playback(Speed) 节；若单 clip 且关键帧面板开 → 右侧并排 `KeyframesPanel`（`291-304`）。
- 底部 Keyframes 切换条（`keyframesToggleBar` `313-336`）：右对齐按钮 `diamond(.fill)` + "Keyframes"，开 `text-primary` 否则 `text-tertiary`，仅单 clip 可用。

**Transform 节**（`transformSection` `483-509`）：
- 可折叠标题 "TRANSFORM"（全大写小标题 + chevron + Reset 按钮）（`collapsibleHeader` `674-697`）。
- 行（每行高 `KeyframesMetrics.rowHeight=22`）：
  - Position（`InspectorPositionFields`，双字段 X/Y）。
  - Scale（`scaleScrubField`：value=宽度，×100 显示 `%`，`612-630`）。
  - Rotation（`rotationScrubField`：`°`，range -3600…3600，`632-650`）。
  - Opacity（`opacityScrubField`：×100 `%`，range 0…1，`652-670`）。
  - Crop 行 + Flip 行。
- 每个可动画行尾带**关键帧控件**（`keyframeControls` `530-563`）：← prev keyframe｜diamond 戳（on/off，在关键帧上 `accent.timecode` 否则 `text-tertiary`，戳/删，`stampButtonWidth=22`）｜→ next keyframe（navButton 宽 6）。playhead 不在 clip 内时禁用（`inRange`）。

**Flip 行**（`flipRow` `736-765`）：H/V 两个图标切换按钮（`arrow.left.and.right` / `arrow.up.and.down`）。

**Speed 节**（`speedSection` `441-463`）：标题 "PLAYBACK" + Speed 字段（range 0.25…4.0，`x`，`447-459`）。

### 6.5 Audio tab（`audioTabContent` `338-383`）

- Levels 节：Volume 行（dB，`-∞ dB` 特殊显示，range `floorDb…ceilingDb`，`386-408`）+ Fade In / Fade Out 行（秒，`411-438`）。
- 若无视觉 clip：Playback(Speed) 节。
- 单 clip + 关键帧面板开 → 右侧 `KeyframesPanel`。
- 底部 Keyframes 切换条。

### 6.6 ScrubbableNumberField（`Components/ScrubbableNumberField.swift`）—— 关键交互组件

- 显示态:暖色文字(`accent.primary`),等宽数字,右对齐;mixed(多值不同)显 `—`(`text-tertiary`)（`57-63`,`displayText` `32-36`）。
- **拖拽改值**:水平拖动,灵敏度 `dragSensitivity`(每像素改的显示单位);**Shift ×10,⌘ ×0.1**（`101-110`）;clamp 到 range;拖动中走 `onChanged`,松手走 `onCommit`（`96-117`）。
- **点击进入输入**:`onClick` 切到 TextField,回车/失焦提交,Esc 取消（`118-121`,`commitEdit` `126-140`：去后缀、逗号转点、解析、clamp）。
- 拖拽阈值 3px 才算拖（`ScrubMouseArea.mouseDragged` `194-204`）；游标 `resizeLeftRight`（`185-187`）。
- 各字段参数(width/format/suffix/sensitivity)见 §6.4/6.5 各处。Volume 用 `fieldWidth=56`,`displayTextOverride` 把 `≤floorDb` 显示成 `-∞ dB`（`InspectorView.swift:397-398`）。

> 前端:`<ScrubbableNumberField>` 用 pointer 拖拽改值 + 点击转 `<input>`;复刻 Shift/⌘ 灵敏度倍率与 3px 阈值;游标 `ew-resize`。

### 6.7 关键帧面板/泳道（`Keyframes/KeyframesLane.swift`）

- 指标（`KeyframesMetrics` `4-26`）：ruler 18 + strip 14（header 32）；row 22；stamp 22；nav 6；diamond 8。
- `ClipRulerBlock`（`29-66`）：顶部 ruler + 下方彩色 clip 条（tint α0.35）带 clip 名（xxs medium），点击/拖动 seek。
- `KeyframesLaneRow`（`69-120+`）：每属性一行，Canvas 画黄色菱形（实心 tint + black α0.4 描 0.5），可拖关键帧（带磁吸，阈值 4px，`snapThresholdPixels` `86`），每关键帧不可见命中区(±7px) 带右键菜单（插值/删除）。

### 6.8 小组件

- `InspectorSection`（小标题 + 内容容器）、`InspectorRow`（icon + label + 控件）、`ColorField`（颜色选择）、`FontPickerField`（字体选择，TextTab 用）、`TextContentField`（文本内容）、`GenerationReferencesStrip`（生成参考图条）。逐个对照源码复刻（这些是 Text/AI tab 的子件，结构同 §6 模式：label + 字段 + AppTheme 间距/字号）。
