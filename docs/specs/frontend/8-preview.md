# Preview（预览）

> 来源:`Preview/PreviewContainerView.swift`(34KB) + `PreviewView.swift` + `TransformOverlayView.swift` + `CropOverlayView.swift` + `PreviewTab.swift`。

### 8.1 结构（`PreviewContainerView.swift:12-60`）

`VStack(spacing:0)`：
1. **tab 栏**（水平 padding `sm=6` + `panelHeaderBar()` 高28）。
2. **画布区**（`GeometryReader`，居中，按宽高比 fit + canvasZoom 缩放）。
3. **scrub 条** + **transport 条**（非 image）或 **image 设置条**（image）。
- 背景 `--bg-surface`。

### 8.2 画布区（`18-51`）

- 宽高比 = `timeline.width/height`（生成中用生成宽高比，`19,313-318`）。
- fit 到容器（`fitSize` `300-306`）× `canvasZoom`（`21-22`）。
- 层叠（ZStack）：`PreviewView`(Rust 合成帧上屏) + (image 预览) + (失败/生成中/离线 overlay) + Transform/Crop overlay（`23-42`）。
- canvasZoom<1 时画边框 `white α0.25`（`44-47`）。
- 位置：居中 + `canvasOffset` 平移（`48-49`）。

> **`PreviewView` = Rust 合成帧显示层**。OpenTake 用 `<canvas>`/WebGL 显示 Rust 经 `preview_frame` event 推来的帧（见 §11 + ARCHITECTURE §2）。DOM 叠字另算（上游 `TextLayerController`，前端可 DOM 叠加或并入 Rust 合成）。

### 8.3 tab 栏（`tabBar` `529-559`）

`HStack(spacing: xs=4)`：← → 导航按钮 + 水平滚动 tab 列表 + 溢出菜单。
- tab（`tabItem` `561-594`）：名字（active semibold 否则 medium，active/hover `text-primary` 否则 `text-secondary`）+ 可关闭 tab 的 ✕；底部下划线（active 显，`bw-medium`，色 = tab.underlineColor：timeline=`accent.primary`，媒体 tab=类型色，`PreviewTab.swift:50-55`）。
- 始终有 "Timeline" tab（不可关，`PreviewTab.swift:30`）；媒体资产可开成 tab。
- 溢出菜单（`609-628`）：Close All Tabs。

### 8.4 transport 条（`transportBar` `64-101`，高 36）

`HStack(spacing: sm=6)`：时间码文本 + Spacer + 传输按钮组 + Spacer + 截帧按钮(video/timeline) + 工程设置组。
- **传输按钮**（`78-90`，`HStack spacing md=10`）：跳首 `backward.end.fill` / 逐帧退 `backward.frame.fill` / 播放暂停 `play.fill`↔`pause.fill` / 逐帧进 `forward.frame.fill` / 跳尾 `forward.end.fill`。timeline tab 用 `togglePlayback`，源 tab 用 `toggleSourcePlayback`。
- 时间码（`PreviewTimecodeText`）：当前/总时长，等宽数字。
- **截帧按钮**（`116-127`）：`camera`，help "Capture Frame to Media"，动作 `captureCurrentFrameToMedia`。
- **工程设置组**（`projectSettingsGroup` `131-154`，`ViewThatFits`：宽则展开 4 个 badge，窄则收成单 `slider.horizontal.3` menu）：
  - Aspect（badge `W:H`，菜单 `AspectPreset` 各项打勾，`156-171`）。
  - Frame Rate（badge `N`，菜单 24/25/30/50/60，`173-188`）。
  - Quality（badge HD/FHD/2K/4K，`QualityPreset`，`190-206`，badge 算法 `245-251`）。
  - Zoom（badge `Fit`/`N%`，`ZoomPreset`，`208-232`）。
  - badge 样式（`badgeLabel` `270-276`）：`fontSize=xxs(9) bold rounded`，`text-secondary`，水平 padding `sm`，高 `IconSize.mdLg=24`。

### 8.5 scrub 条（`scrubBar` `651-...`）

- 进度条（Capsule 背景 `white α0.10`）+ 进度填充 + 拖拽 thumb。
- hover/拖拽时变粗:thumb 6→10,bar 3→4（`655-657`）。游标 `pointingHand`（`677`）。
- 拖拽 = scrub seek（暂停-拖-恢复语义，同时间线 scrub）。

### 8.6 Transform / Crop 叠加（`TransformOverlayView.swift` / `CropOverlayView.swift`）

- **TransformOverlay**:选中视觉 clip 时画变换框 + 8 个缩放手柄 + 旋转 + 拖动移动;边到画布中心/边缘吸附(`Transform.snapToCanvasEdges`/`snapCenterToCanvasCenter`，`Timeline.swift:455-497`)。
- **CropOverlay**:裁剪态(`cropEditingActive`)画裁剪框 + 手柄,带宽高比锁(`CropAspectLock`，`Timeline.swift:513-540`：Custom/Original/16:9/9:16/1:1/4:3/3:4/21:9)。
- 二选一显示（`PreviewContainerView.swift:37-41`）：cropEditingActive → Crop，否则 Transform。
- 这两层是**坐标变换 + 手柄拖拽**，前端用绝对定位 SVG/div 手柄 + pointer 拖拽，**复刻吸附阈值与手柄命中区**（对照两文件源码）。

### 8.7 离线/失败/生成中 overlay（`326-525`）

- 离线媒体（`offlinePreview` `430-480`）：`exclamationmark.triangle.fill`(display 36, status.error) + 标题 + 说明 + 路径 + Relink…/Relink Folder… 按钮（capsule prominent/secondary）。
- 失败（`failedPreview` `482-525`）：生成失败 + 错误文本（可选中滚动）+ Retry Download（若有）。
- 生成中（`generatingPreview` `392-405`）：参考图模糊背景 + `black α0.55` + `GeneratingOverlay(label, .preview)`。
