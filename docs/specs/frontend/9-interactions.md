# 交互细节逐项清单（1:1 关键）

> 本节是验收的硬清单。每条标注上游来源。前端必须逐条等价实现。

### 9.1 选择（clip）

| 操作 | 行为 | 来源 |
|---|---|---|
| 单击 clip | 选中 linked 整组（无修饰）| `TimelineInputController.swift:102-104` |
| ⌘+单击 clip（未选）| 仅选此 clip（不扩 linked）| `100-101` |
| Shift+单击 | 切换该 clip（linked 则整链加/减）| `88-99` |
| 单击空白 | 清空选择（非 Shift）| `184-186` |
| 框选 marquee | 矩形相交选；非 Option 扩 linked 整组 | `349-383` |
| Esc | 清空选择 + 清 range + 工具回 pointer | `EditorWindowController.swift:156-159` |
| 双击 clip | 选中其源媒体资产 + Media 面板 reveal | `TimelineInputController.swift:36-49` |
| 点击 gap | 选中 gap（空隙）| `187`、`hitTestGap 730-746` |

### 9.2 拖放（移动/复制/落轨）

| 操作 | 行为 | 来源 |
|---|---|---|
| 拖 clip 横向 | 移动（带磁吸到 clip 边/playhead，多探针）| `234-279` |
| Option+拖 | 复制（duplicate）| `isDuplicate`，`180-181`、`423` |
| 拖到轨间隙 | 新建轨插入（黄线指示）| `dropTargetAt`、`425-438` |
| 跨轨 | 仅落到类型兼容轨（钳制）| `clampedTrackDelta 918-935` |
| linked 伙伴 | 跟随移动但留在各自轨（pinned）| `pinnedCompanionIds 901-915` |
| 落回原位 delta=0 | 不发命令 | `399-403` |
| 拖拽阈值 | 3px（`dragThreshold`）| `Constants.swift:53` |

### 9.3 Trim（修剪）

| 操作 | 行为 | 来源 |
|---|---|---|
| 拖左手柄（localX≤4）| trimLeft（带磁吸）| `135-145`、`281-307` |
| 拖右手柄（localX≥w-4）| trimRight | `146-156`、`309-341` |
| 最小 1 帧 | 不能 trim 到 <1 帧 | `304`、`334` |
| image/text | 可自由延长（无源素材上限）| `hasNoSourceMedia`、`305`、`335-339` |
| 默认传播 linked | trim 同步链伙伴（Option 关）| `propagateToLinked = !Option`，`144` |
| 手柄宽 | 4px（`Trim.handleWidth`）| `Constants.swift:100` |
| 游标 | trim 区 `resizeLeftRight` | `542-544` |

### 9.4 Razor / Split / 关键帧 / 淡变 / 音量

| 操作 | 行为 | 来源 |
|---|---|---|
| Razor 工具 + 点 clip | 在点击帧 split（带磁吸预览）| `70-78`、`511-532` |
| Razor 预览线 | 橙虚线 4,4 | `TimelineView.swift:245-254` |
| ⌘K | playhead 处 split | `MainMenu.swift:76` |
| ⌘+点 audio body | 添加音量关键帧 | `132-134`、`addVolumeKeyframeOnClick 646-661` |
| 拖音量关键帧 | 移动（钳制相邻 kf + dB 范围）| `343-344`、`582-618` |
| 拖淡变拐点 | 改淡变长（clamp 对边）| `346-347`、`621-643` |
| 右键音量 kf | Linear/Smooth/Hold + Delete | `TimelineView.swift:672-692` |
| 右键淡变拐点 | Linear/Smooth | `652-669` |

### 9.5 Playhead / scrub / 缩放

| 操作 | 行为 | 来源 |
|---|---|---|
| 点刻度 | scrub playhead | `55-65`、`beginPlayheadScrub 763-771` |
| 拖刻度（边缘）| scrub + 自动横滚 | `continuePlayheadScrub 783-801` |
| Shift+拖刻度 | 拉时间线 range 选区 | `59-61`、`beginTimelineRangeSelection 849-856` |
| 拖 range 边 | 调整 range | `57-58`、`834-847` |
| Option+滚轮 | 缩放（光标锚定）| `668-672` |
| ⌘+滚轮 | 横向平移 | `674-682` |
| 捏合 | 缩放 | `magnify 688-691` |
| 缩放灵敏度 | scroll 0.04 / magnify 1.5 / pan 5 | `Constants.swift:85-87` |
| 缩放范围 | minZoomScale … 40 | `Constants.swift:84` |

### 9.6 键盘快捷键全表（`EditorWindowController.handleKeyDown` `38-164` + `MainMenu.swift`）

> 文本输入聚焦时**不拦截**（`EditorWindowController.swift:39-41,176-181`）。前端：编辑 input/textarea 时跳过全局快捷键。

| 键 | keyCode | 动作 | 条件 |
|---|---|---|---|
| Space | 49 | 播放/暂停 | `55-57` |
| ← | 123 | 退一帧（Shift→退 5 帧）| `60-62` |
| → | 124 | 进一帧（Shift→进 5 帧）| `64-66` |
| ⌫ Delete | 51 | 删除选中 clip；选中文件夹/资产则删之；Shift→ripple 删（clip 或 gap）| `68-85` |
| C | 8 | Razor 工具（非 ⌘）| `87-92` |
| V | 9 | Pointer 工具（非 ⌘）| `94-99` |
| I | 34 | 标记 range 起点（无 ⌘⌥⌃）| `101-106` |
| O | 31 | 标记 range 终点 | `108-113` |
| [ | 33 | Trim Start to Playhead | `115-117` |
| ] | 30 | Trim End to Playhead | `119-121` |
| `` ` `` | 50 | 切换面板最大化（无修饰）| `123-128` |
| Return | 36 | media 面板：进文件夹（单选）；裁剪态：退出 | `130-141` |
| Esc | 53 | 取消 swap / 退裁剪 / 退最大化 / 清选择+range+回pointer | `143-159` |
| 方向键（media 聚焦）| 123/124/125/126 | 移动媒体选择（左右上下，非 Shift）| `49-53,166-174` |
| ⌘Z / ⇧⌘Z | | Undo / Redo | `MainMenu.swift:67-68` |
| ⌘C / ⌘X / ⌘V | | Copy/Cut/Paste（timeline 聚焦时作用于 clip；media 聚焦时 paste 导入）| `EditorWindowController.swift:227-248` |
| ⌘K | | Split at Playhead | `MainMenu.swift:76` |
| Q / W | | Trim Start/End（菜单，无修饰）| `MainMenu.swift:80-86` |
| ⌘N/O/S/⇧⌘S/⌘I/⌘E | | New/Open/Save/SaveAs/Import/Export | `MainMenu.swift:41-56` |
| ⌘0 / ⌘⌥0 / ⌘⌥A | | 切 Media/Inspector/Agent 面板 | `MainMenu.swift:104-114` |
| ⌘1/⌘2/⌘3 | | Layout Default/Media/Vertical | `MainMenu.swift:134-144` |
| ⌘F | | 全屏 | `MainMenu.swift:125` |
| ⌘? | | 快捷键帮助 | `MainMenu.swift:157` |
| ⌘, | | 设置 | `MainMenu.swift:29` |

> **跨平台键位**:macOS ⌘ → Win/Linux Ctrl;⌥ → Alt。Tauri 下用 `accelerator` 字符串复刻。keyCode 是 macOS 物理键码,前端用 `event.code`(如 `Space`/`KeyC`/`BracketLeft`/`Backquote`/`ArrowLeft`)更可靠,**逐键对照上表语义**。

### 9.7 Hover / 焦点 / 游标态

| 元素 | hover 效果 | 来源 |
|---|---|---|
| 图标按钮 | 背景渐显（faint 0.08；active 时 soft 0.10→muted 0.15）| `HoverHighlight.swift:31-38` |
| 面板 | 点击聚焦 → 焦点环 opacity 0.6 | `EditorView.swift:384-396` |
| MediaPanel 标签 | hover 浮出名字 capsule | `MediaPanelView.swift:111-129` |
| Preview tab | hover 文字转 primary | `PreviewContainerView.swift:566-567` |
| 时间线刻度 | `pointingHand`；Shift `crosshair` | `498-505` |
| clip trim 区 | `resizeLeftRight` | `542-544` |
| 轨道高度边 | `resizeUpDown` | `TimelineHeaderView.swift:203-209` |
| scrub 条 | `pointingHand` + 变粗 | `PreviewContainerView.swift:655,677` |
| 数字字段 | `resizeLeftRight`（ew-resize）| `ScrubbableNumberField.swift:185-187` |

### 9.8 右键菜单（汇总，见 §5.10 详表）

时间线 clip / 空白 / range / 淡变拐点 / 音量 kf / 关键帧泳道，菜单项逐字照搬（`TimelineView.swift:641-799` + `KeyframesLane.swift:contextMenu`）。Media 面板资产/文件夹右键（`MediaTab` 各处）。
