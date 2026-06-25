# Timeline（时间线）—— 核心

> 来源:`Timeline/` 全目录。这是上游用 **AppKit 直接绘制(CGContext)** + `TimelineInputController` 手势的部分。OpenTake 前端**强烈建议用 `<canvas>` 2D 绘制 clip/轨道/刻度**(性能 + 像素级一致),playhead/snap 指示用绝对定位 DOM/SVG 叠加层,生成中 overlay 用 DOM 叠加。
> **核心原因**:clip 渲染含缩略图条/波形/音量橡皮筋/淡变楔形/关键帧菱形,DOM 逐元素会卡;Canvas 2D 能逐字照搬 `ClipRenderer.draw` 的绘制顺序与坐标。

### 5.1 容器结构（`TimelineContainerView.swift:6-57`）

```
TimelineContainer (relative)
├─ TrackHeaderColumn   ← 左固定列, 宽 trackHeaderWidth=100, 高随内容
├─ 竖直分隔线          ← x = 100-1, 宽1, 色 border-primary  (TimelineContainerView.swift:30-35)
└─ ScrollArea          ← x=100 起, 双向滚动(横+纵), 自动隐藏滚动条, mini 滚动条
     └─ TimelineCanvas ← document 视图, 宽=内容宽, 高=内容高
```

- 滚动条:横纵都有,`autohidesScrollers=true`,`controlSize=.mini`,`drawsBackground=false`（`TimelineContainerView.swift:17-20`）。
- **header 列与画布纵向滚动联动**:画布纵向滚动时,header 列 `setBoundsOrigin(y)` 跟随（`TimelineContainerView.swift:115-122`），即 header 不随横向滚,但随纵向滚。前端:header 列纵向 `translateY(-scrollTop)`,横向固定。
- **内容尺寸**（`TimelineView.swift:116-129`）：宽 = `zoomScale * totalFrames + visibleWidth*0.5`（右侧留半屏空白）;高 = `max(visibleHeight, 最后一轨底 + dropZoneHeight)`。

### 5.2 几何（必须逐字照搬，`TimelineGeometry.swift`）

所有公式（`headerWidth` 在画布坐标里通常传 0，因 header 是独立列）：

- `pixelsPerFrame = zoomScale`。
- **轨道 Y 起点**:第一轨顶 = `rulerHeight(24) + dropZoneHeight(60)`,之后累加各轨 `displayHeight`（`TimelineGeometry.swift:39-46`）。即**刻度下方有 60px 的顶部 drop zone**,再开始第一条轨道。
- `trackY(at:i)` = 累积 Y（`53-55`）；`trackHeight(at:i)` = `displayHeight`（默认 50，`Track.displayHeight` `Timeline.swift:34`；范围 `[TrackSize.minHeight=32, maxHeight=200]` `Constants.swift:76-77`）。
- **clip rect**（`TimelineGeometry.swift:62-69`）：
  ```
  x = headerWidth + clip.startFrame * pixelsPerFrame
  y = trackY(i) + 2
  width = clip.durationFrames * pixelsPerFrame
  height = trackHeight(i) - 4          // 上下各留 2px
  ```
- `frameAt(x)` = `max(0, Int((x - headerWidth)/pixelsPerFrame))`（`71-73`，**截断取整**）。
- `trackAt(y)`：找第一个 `y < cumulativeY[i]+height[i]` 的 i（`75-80`）。
- `xForFrame(frame)` = `headerWidth + frame*pixelsPerFrame`（`138-140`）。
- 常量:`Layout.rulerHeight=24, dropZoneHeight=60, trackHeaderWidth=100, insertThreshold=10, dragThreshold=3`（`Constants.swift:46-53`）;`Trim.handleWidth=4, clipCornerRadius=3`（`Constants.swift:99-102`）。

### 5.3 刻度 Ruler（`TimelineRuler.swift`）

- 高 `rulerHeight=24`，背景 `--bg-surface`，底部 1px 分隔线 `border-primary`（`TimelineRuler.swift:13-21`）。
- **主刻度间隔**:目标 ~80px,从 `[1,2,5,10,15,30,60,120,300,600,1200,1800,3600]*fps` 选第一个 `>= 80/pixelsPerFrame` 的（`TimelineRuler.swift:87-94`）。
- **次刻度细分**:在 `[10,5,4,2]` 中选第一个使每格 `>=12px` 的（`97-106`）。
- 次刻度:`text-muted α0.4`,线宽 0.5;中点(偶数细分的一半处)高 6px,其余 4px（`44-60`）。
- 主刻度:`text-muted`,线宽 1,高 8px;标签 = `formatTimecode(frame, fps)`,等宽数字,`fontSize=xs(10)`,色 `text-tertiary`,绘制在 `(x+3, top+2)`（`63-83`）。
- 刻度随横向滚动:绘制矩形 `x=scrollOffsetX`,宽=可视宽（`TimelineView.swift:256-262`）—— 即**刻度固定在可视区顶部,内容横滚时刻度数字滚动**。前端:刻度画在可视区顶层(sticky),依 scrollLeft 重绘。

### 5.4 Clip 渲染（`ClipRenderer.draw`，`ClipRenderer.swift:51-158`）—— 逐字复刻

绘制顺序（在 clip rect 内，`cornerRadius = 3`）：

1. **底色填充**（`ClipRenderer.swift:74-81`）：`fill = sourceClipType.themeColor`，selected α0.45，否则 α0.30。圆角矩形。
2. **可视内容**（`95-106`，contentX = `minX+stripWidth(3)+1`，contentWidth = `width-3-1-handleW(4)`，contentY = `minY+labelBarHeight(16)`）：
   - video：缩略图条（`drawThumbnailStrip`，`494-536`，按 trim 映射可见时段，水平平铺缩略图，按 clip 圆角裁剪）。
   - image：平铺图片（`drawTiledImage`，`540-560`）。
   - audio：波形（`drawWaveform`，`195-263`，dB 归一峰值检测，音量移 dB 轴）。
3. **音量橡皮筋 / 不透明度淡变**（`108-112`）：audio → `drawVolumeRubberBand`(白线音量曲线 + 淡入淡出楔形 + 选中时菱形关键帧/方块拐点)（`267-380`）；非 audio → `drawOpacityFades`(淡变楔形,选中时拐点方块)（`382-436`）。
4. **左色条**（`114-119`）：`x=minX, 宽 stripWidth=3, 高=全高`，圆角，色 `sourceClipType.themeColor`（实心，比底色更饱和）。
5. **边框**（`121-132`）：selected → `white α0.9, 线宽 1.5`；否则 `border-primary, 线宽 0.5`。
6. **缺失媒体红洗**（`134-143`）：若 missing 且非 generating → 填 `status.error α0.25` + 描边 `status.error α0.80 线宽 1.5`。
7. **标签栏**（`drawLabelBar`，`594-621`）：clip 宽 `>20` 才画。文本 = `"<首个非空行的名字>  <时长时间码>"`，`fontSize=xs(10) medium`，`text-primary`，左 inset 6，垂直居中于 16px 标签栏；若 `linkGroupId != nil` 给名字部分加**下划线**（`607-609`）。文本按标签栏裁剪。
8. **out-of-sync 偏移徽标**（`147-149`，`drawOffsetBadge` `627-655`）：linkOffset≠0 时右上角红色圆角徽标 `"+N"/"-N"`，色 `rgb(255,71,71)`，`fontSize=xs(10) semibold`。
9. **关键帧菱形标记**（`drawKeyframeMarkers`，`163-191`）：clip 底部（`y = maxY-5`）一排黄色小菱形（半径3），`systemYellow α0.95` 填 + `black α0.5` 描 0.5；位置 = 各 opacity/position/scale/crop 关键帧的绝对帧（**volume 关键帧不在这,画在橡皮筋上**）。
10. **trim 手柄**（`drawTrimHandles`，`659-666`）：左右各一条 `width=handleWidth(4)` 的 `text-muted` 竖条，贴 clip 左右边。

**音量橡皮筋细节**（audio，`drawVolumeRubberBand` `267-380`）：
- body = 标签栏下方区域（`clipBodyRect`：`y=minY+16, height=clip高-16-1`，`ClipRenderer.swift:18-25`）。
- dB↔Y：`volumeRubberBandTopDb=6, bottomDb=-60`，高 dB→小 Y（`y(forDb:)` `28-34`）。
- 音量线:有关键帧 → 折线(linear/hold/smooth 分段,smooth 用 12 步)；无 → 平直线在静态音量 dB（`281-318`）。线色 selected `white α0.95` 否则 α0.75，线宽 1.5。
- 淡变楔形:`drawFadeWedge`（拐点在 body 顶部 `fadeKneeTopInset=4` 处的"fade lane"，silenceY=body 底）。
- 选中时画：音量关键帧菱形（`volumeKeyframeSize=7`）+ 左右淡变拐点方块（`7×7`）（`351-379`）。

> **前端实现要点**:`ClipRenderer` 整体移植为一个 `drawClip(ctx, clip, rect, {isSelected, opacity, isMissing, isGenerating, displayName, fps, cache, linkOffset})` 函数,**绘制顺序与每个子绘制函数逐一对应**。缩略图/波形来自 Rust 媒体缓存(见 §11,经 event/asset 协议拿到 sprite/采样)。

### 5.5 轨道头列（`TimelineHeaderView.swift`，AppKit draw）

左固定列宽 100，背景 `--bg-surface`（`TimelineHeaderView.swift:7`）。每条轨道（`55-98`）：
- 顶部空 `rulerHeight=24`（与刻度对齐，header 列顶部 24px 是空 + 1px 分隔线在 `rulerHeight-0.5` 处，`37-39`）；header 内容从 `rulerHeight` 下开始裁剪绘制（`42-43`）。
- 左侧 3px 色条 = `track.type.themeColor`（`59-61`）。
- 轨道标签:`timelineTrackDisplayLabel(i)`(如 "V1"/"A1"),`fontSize=sm(11) medium`,色 `text-secondary`,绘制在 `(stripWidth+6, 垂直居中)`（`63-67`，labelAttrs `8-11`）。
- 右侧图标(14px,SF config 11pt,`48-50`):
  - sync-lock 切换:`active=syncLocked` → `link` 否则 `personalhotspot.slash`（`74-77`）。位置 = 最右减一格。
  - audio 轨:静音切换 `active=!muted` → `speaker.wave.2.fill` 否则 `speaker.slash.fill`（`78-82`）。
  - 非 audio:隐藏切换 `active=!hidden` → `eye` 否则 `eye.slash`（`83-88`）。
  - 图标 tint:active = `text-secondary`;非 active = `text-secondary α0.3`（`116`）。命中区 = 图标外扩 4px（`118`）。
- 每轨上下 1px 白色边框(第一轨顶 + 每轨底,`border-primary`,`91-97`)。
- **视频区/音频区之间粗分隔**:若同时有视频轨和音频轨,在首个音频轨顶画 2px `border-divider` 线（`101-106`）。

**轨道头交互**（`TimelineHeaderView.swift:150-210`）：
- 点击 mute/hide/sync 图标 → 切对应状态（命令：`toggleTrackMute/Hidden/SyncLock`）。
- **轨道高度拖拽**:命中轨道底边 ±`TrackSize.resizeHandleZone=6px`（`Constants.swift:78`）→ 拖拽改 `displayHeight`,clamp `[32,200]`（`180-190`）;拖拽中只改本地 displayHeight,松手才 commit `setTrackHeight`（`192-201`）。游标:命中 resize 区显示 `resizeUpDown`,否则 `arrow`（`203-210`）。
- **displayHeight 不持久化**(开工程重置默认 50,`Timeline.swift:33-34`)→ 前端作为 UI-only 态,不进 Rust 镜像。

### 5.6 Playhead（`PlayheadOverlay.swift`）

- 颜色 `systemRed`，线宽 1（`PlayheadOverlay.swift:38-40`）。
- 形状（`Playhead.appendPath` `7-23`）：竖线从 `top=rulerHeight(24)` 到 `bottom=可视高`；顶端一个**朝下三角**（`triangleSize=8`，宽=8、高=8，尖朝下）。
- X = `playheadState.timelineFrame * pixelsPerFrame - viewport.minX`（`51`）—— **相对可视区**，即 playhead 是固定在可视层的叠加，按 scrollLeft 偏移。
- zIndex 100，跟随 `playheadState.timelineFrame` 与 `zoomScale` 变化（`observe()` `68-78`）。
- **播放时自动滚动**:播放中若 playhead 接近可视边缘(margin 60px)→ 滚动使其到 `可视宽*0.25` 处（`TimelineContainerView.swift:75-90`）。

> 前端:用绝对定位的 SVG(竖线+三角)或两个 div,`left = frame*zoom - scrollLeft`,zIndex 高于 canvas,`pointer-events:none`。

### 5.7 Snap 指示线（`SnapIndicatorOverlay.swift` + `SnapEngine.swift`）

- 黄色虚线（`systemYellow`，线宽 1，`lineDashPattern [4,4]`，zIndex 90，`SnapIndicatorOverlay.swift:14-19`），从 `rulerHeight` 到内容底。
- 两个 X 源:本地拖拽 `localX` 与外部拖入 `externalX`,localX 优先（`40`）。
- 显隐:磁吸命中时设 X 显示,否则隐藏。

**磁吸算法 SnapEngine**（`SnapEngine.swift`，**逐字照搬**）：
- 收集目标 = 所有 clip 的 startFrame/endFrame（排除被拖的 clip）+ 可选 playhead（`collectTargets` `31-48`）。
- 阈值:`baseThreshold = Snap.thresholdPixels=8`(`Constants.swift:70`);换算成帧 = `8/pixelsPerFrame`;clipEdge 用基础阈值,playhead 用 `*Snap.playheadMultiplier=1.5`（`81-84`，`Constants.swift:72`）。
- **黏滞**:已吸附则保持到移出 `*Snap.stickyMultiplier=1.5` 阈值（`64-74`，`Constants.swift:71`）。
- 命中时触发 `alignment` 触觉反馈（`93`）→ 前端无触觉,**忽略或可选轻微视觉脉冲**。
- 多探针:move 时用所有选中 clip 的 start/end 偏移做探针,取最近（`findSnap` probeOffsets，`TimelineInputController.swift:244-264`）。

### 5.8 时间线全部手势（`TimelineInputController.swift` + `TimelineView` 输入转发）

> 前端在 TimelineCanvas 上挂 `pointerdown/move/up`,**逐分支照搬 mouseDown 的判定树**。

**mouseDown 判定树**（`TimelineInputController.swift:32-194`）：
1. **双击 clip**（在刻度下方）→ 选中该 clip 的源媒体资产 + 在 Media 面板 reveal（`36-49`）。
2. 切换 preview tab 回 timeline（若当前非 timeline）（`51-53`）。
3. **点在刻度区**（`scrollOffsetY ≤ y < +rulerHeight`，`55-65`）：
   - 命中 range 边缘 → 拖 range 边（`beginTimelineRangeEdgeDrag`）。
   - 按住 Shift → 开始拉时间线 range 选区（`beginTimelineRangeSelection`）。
   - 否则 → 拖动 playhead scrub（`beginPlayheadScrub`）。
4. **Razor 工具**且命中 clip → 在点击帧 split（用 razorPreviewFrame 或当前帧）（`70-78`）。
5. **命中 clip**（pointer 工具，`80-182`）：
   - **选择逻辑**（linkedOn = !Option，`84-104`）：
     - Shift：已选则减(linked 则减整链)，否则加(linked 加整链)。
     - Option 且未选：仅选此 clip。
     - 无修饰且未选：选 linked 整组（或仅此 clip，若 Option）。
   - **子区域命中（按优先级）**：
     - 淡变拐点命中 → `fadeKnee` 拖拽（`109-118`）。
     - audio 音量关键帧命中 → `audioVolumeKf` 拖拽（`119-131`）。
     - ⌘ + audio + body 命中 → 添加音量关键帧（`132-134`）。
     - 非 Option 且 localX ≤ handleWidth(4) → `trimLeft`（`135-145`）。
     - 非 Option 且 localX ≥ width-handleWidth → `trimRight`（`146-156`）。
     - 否则 → `moveClip`（收集所有选中 clip 作 companions，记 grabOffset）（`157-182`）。
6. **点空白**（`183-190`）：非 Shift 清空选择；命中 gap 则选 gap；开始 marquee 框选。

**mouseDragged 分支**（`198-391`）：
- scrubPlayhead → seek（含边缘自动横滚 `autoScrollHorizontallyForTimelineDrag`，`continuePlayheadScrub`）。
- timelineRange → 拉 range（带磁吸到 clip 边/playhead）。
- moveClip → 算 deltaFrames（多探针磁吸）+ 算落轨（`dropTargetAt(y)`：existingTrack 或 newTrackAt 插入新轨）；跨轨受类型兼容钳制（`clampedTrackDelta`）。
- trimLeft/Right → 算 deltaFrames（磁吸 + clamp：不能 <1 帧；非 image/text 不能超出源素材）。
- audioVolumeKf → 移动音量关键帧（钳制在相邻关键帧之间 + dB 范围）。
- fadeKnee → 算淡变长度（clamp `[0, duration-对边淡变]`）。
- marquee → 矩形相交选择（非 Option 则扩到 linked 整组）；只重绘变化区域。

**mouseUp**（`395-490`）：依据 dragState commit 对应命令（move→`moveClips`/`duplicateClipsToPositions`，新轨 move→建轨+移动；trim→`commitTrim`；volumeKf→`commitMoveVolumeKeyframe`；fade→`commitFade`；range→保留有效 range；marquee 结束）。**无修改的拖拽不发命令**（如 move 落回原位 delta=0 直接 break，`399-403`）。

**mouseMoved（游标）**（`494-557`）：
- 刻度区：range 边缘→`resizeLeftRight`；Shift→`crosshair`；否则→`pointingHand`。
- Razor 工具在轨道区：算 razorPreviewFrame（带磁吸）+ `crosshair` + 画橙色虚线预览（见 5.9）。
- 命中 trim 区→`resizeLeftRight`；淡变拐点→`resizeLeftRight`；audio 音量关键帧→`openHand`；否则→`arrow`。

**scrollWheel / 缩放 / 平移**（`667-711`）：
- **Option + 滚轮 → 缩放**（以光标为锚点），factor = `exp(deltaY * Zoom.scrollSensitivity=0.04)`（`Constants.swift:85`）。
- **⌘ + 滚轮 → 横向平移**（delta = `raw * Zoom.panSpeed=5`，`Constants.swift:87`）。
- 普通滚轮 → 转发给外层滚动视图（双向滚动）。
- **触控板捏合 → 缩放**（factor = `1 + magnification * Zoom.magnifySensitivity=1.5`，`Constants.swift:86`）。
- **缩放锚定**：缩放时保持光标下的帧位置不变（`applyZoom` `693-711`）；`newScale = clamp(minZoomScale, Zoom.max=40, zoomScale*factor)`。
- **playhead 锚定缩放**:非光标缩放(如滑块)时,保持 playhead 视口位置（`applyPlayheadAnchoredScroll` `180-196`）。

> 前端缩放/平移:监听 `wheel`(带 `ctrlKey`/`metaKey`/`altKey`)与 `gesturechange`(Safari 捏合)或 pointer 双指;**逐字复刻锚定与灵敏度常量**。

### 5.9 时间线叠加绘制（`TimelineView.drawContent`，`TimelineView.swift:201-265`）

绘制顺序（在画布上）：
1. 轨道背景（`drawTrackBackgrounds` `591-613`，每轨 `--bg-surface` + 上下 1px 边 + 视频/音频区粗分隔）。
2. range 选区轨道填充（`385-397`，`text-primary α0.06`）。
3. clips（`drawClips`，含 move/trim 的 ghost 预览，`271-381`）。
4. gap 选区（`436-451`，白虚线框 `white α0.12` 填 + `white α0.9` 描 3,3 虚线）。
5. 生成中 overlay 同步（DOM 叠加，`syncGeneratingClipOverlays` `456-473`）。
6. 外部拖入 ghost clips（`drawExternalDragGhosts` `486-538`，opacity 0.5）。
7. marquee 框（`219-228`，`white α0.6` 描 + `white α0.1` 填，3,3 虚线）。
8. 新轨插入线（`230-243`，`systemYellow` 线宽 2 横线）。
9. Razor 预览线（`245-254`，`systemOrange α0.8` 线宽 1，4,4 虚线，竖线）。
10. ruler（`256-262`）。
11. range 选区刻度填充 + 边（`263-264`：刻度区 `text-primary α0.10` 填；边 `accent.timecode α0.80` 线宽 1.5 竖线）。

**move 时 ghost 渲染**（`drawClips` `307-344`）：原位 clip 画淡（opacity 0.3，复制态 1.0），ghost 画 0.7 + 选中边。

**Ripple-insert 指示**（⌘ 拖入时，`drawRippleInsertIndicator` `567-587`）：白色竖线 + 顶部朝右箭头。

### 5.10 时间线右键菜单（`TimelineView.menu(for:)`，`TimelineView.swift:641-799`）

> 前端用自定义右键菜单（contextmenu 事件 + 浮层）。**菜单项与分组逐字照搬。**

命中位置决定菜单：
- **淡变拐点上**（`652-669`）：Linear / Smooth（当前项打勾），设淡变插值。
- **audio 音量关键帧上**（`672-692`）：Linear / Smooth / Hold（打勾）+ 分隔 + Delete Keyframe。
- **命中 clip**（`694-764`，未选则先选 linked 整组）：分组（组间分隔线）：
  - **Timeline 组**：Copy；Paste(若可粘贴，带 trackIndex+frame)；Link(若可链接)；Unlink(若可解链)。
  - **AI 组**：Add to Chat；AI Edit(子菜单，若有)。
  - **Media 组**：Swap Media(非 text 且单链组)；Save as Media(video/audio)。
  - 若点击落在 range 内：追加 range 项。
- **空白区**（`766-781`）：Paste(若可)；range 项(若点在 range 内)。
- **range 项**（`783-799`）：Add Range to Chat / Save Range as Media / Clear Range。

### 5.11 时间线拖放（从媒体面板/Finder 拖入，`TimelineView` NSDraggingDestination，`904-1020`）

- 接受类型 `.string`(内部 asset payload) 和 `.fileURL`(Finder)（`TimelineView.swift:26`）。
- draggingEntered/Updated（`906-929`）：解析 asset payload → 算落点(`dropTargetAt(y)`) + 磁吸帧(`applyExternalSnap`，探针 0 与总时长) + 是否 ripple-insert(⌘)；画 ghost。
- performDrop（`966-1020`）：⌘=ripple-insert，否则普通 add；视觉资产与纯音频分别落轨；走 `addClipsWithSettingsCheck`（导入设置不匹配时弹对话框）。

> **macOS 拖放陷阱**（`AGENTS.md`）：上游警告 SwiftUI `.onDrop` 父视图会遮蔽子 drop。前端无此问题，但**跨面板拖拽**（Media→Timeline）建议用 HTML5 DnD 或 pointer 自管拖拽，payload 用资产 id 列表 + 可选源时段。
