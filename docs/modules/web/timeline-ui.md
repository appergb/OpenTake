# timeline-ui — 时间线 UI 与几何（components/timeline + lib 几何库）

> 上级：[本模块目录](INDEX.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
>
> 覆盖 `web/src/components/timeline/` 与 `web/src/lib/` 的几何子集（`geometry.ts`/`snap.ts`/`ruler.ts`/`zones.ts`/`clip.ts`）。时间线是编辑器最重的交互面：**Canvas 绘制 + 指针/触控板手势 → 像素↔帧换算 → `EditRequest`**。

---

## 一句话职责

把后端 `timeline` 镜像在 `<canvas>` 上画出来，并把刮擦/缩放/平移/拖动/修剪/切割等手势按上游常量精确换算成帧，再经 [editActions](state-stores.md) 发命令。

---

## 像素↔帧：换算放前端（移植铁律）

像素↔帧的全部算式在前端 `lib/geometry.ts`，对应上游 `TimelineGeometry.swift`：

- `xForFrame(frame, pixelsPerFrame) = frame * pixelsPerFrame`；线上叠加 `headerWidth - scrollLeft`（轨道头宽 100、`pixelsPerFrame == uiStore.zoomScale`）。
- `frameAt(x, pixelsPerFrame) = Math.trunc(x / pixelsPerFrame)` 并钳到 ≥0（**截断，不四舍五入**，对齐 `secondsToFrame` 的 `Int()`）。
- `trackY()` 递归累加各轨显示高度，首轨从 `rulerHeight + dropZoneHeight` 起；`clipRect()` 组合轨道 Y 与帧→像素，默认上下各内缩 2px。

> 帧↔秒换算在 Rust，像素↔帧换算在前端——两侧不可越界。

## 渲染：Canvas 2D（非 DOM 堆叠）

时间线主体是一张 `<canvas>`，由三支纯绘制模块输出，clip 不是 DOM 节点：

- `timelineCanvas.ts`：文档空间绘制（`translate(-scrollLeft*dpr, -scrollTop*dpr)`），画轨道背景、视频/音频分隔、所有 clip（含拖动/修剪幽灵）、新轨插入区指示。
- `clipRenderer.ts`：单个 clip 的分层绘制——填充 → 淡入淡出楔 → 左色带 → 边框 → 缺失底纹 → 标签栏 → 关键帧菱形 → 修剪把手；支持波形缓存、半透明幽灵、链接偏移徽章、音量 KF 幽灵。
- `rulerCanvas.ts`：标尺**粘在视口顶部**（不随竖向滚动），靠 `scrollLeft` 平移刻度，画主/次刻度 + 时码标签。

DOM 叠加层只剩少数非绘制元素：`Playhead`、`SnapIndicator`、`TrackHeaderColumn`、右键菜单、交换选择器。

## 几何/命中/吸附库（lib/）

- `ruler.ts`：`chooseTicks()` 选首个 ≥80px 的主间隔，再选使副单元 ≥12px 的细分。
- `snap.ts`：`collectTargets()` 收集非排除 clip 的起止帧（可含播放头）为吸附点；`findSnap()` 用基础阈值 `SNAP.thresholdPixels / pixelsPerFrame`（帧），粘性 1.5×，播放头阈值更大且优先；`findSnapDelta()` 多探针（如选中 clip 起+止）取最小校正。
- `zones.ts`：`firstAudioIndex()` 划分视觉/音频区；`trackDisplayLabel()` 生成 V1/A1/I1/L1（视频从下往上编号，音频从上往下）。
- `hitTest.ts`：`hitTestClip()` 逐轨逐 clip 命中并返回子区域（trimLeft/trimRight/body）；`clipsInRect()` 选框集合；`audioVolumeKfHit()`（8px 半径）、`fadeKneeHit()`（14×14px）命中关键帧/淡入淡出拐点。
- `clip.ts`：clip 辅助（如 `fitTransformForMedia()` 适配变换、`trimToPlayheadEdits()` 生成 trim 编辑），被 editActions 复用。

## 手势与触控板（TimelineContainer.tsx / TimelineRegion.tsx）

`TimelineContainer` 的指针决策树（`onPointerDown`）：① 标尺命中 → 刮擦播放头；② 剃须刀模式 → 带吸附切割；③ 音量 KF 点拖拽；④ Cmd+音频 clip → 在该帧戳音量 KF；⑤ clip 命中 → 选择（Shift 扩展 / Alt 反链接）+ 进入 trim/淡入淡出/move；⑥ 空白 → 清选 + 选框。

`onWheel`：`Ctrl/Cmd+滚轮` 光标锚定缩放；`Option+滚轮` 水平滚动；裸滚轮/两指滑动自由平移（含触控板水平分量）。

`onPointerMove`（move 拖动）：算帧 delta，多探针吸附 + 粘性带维持，落点是既有轨或新轨插入区（上/下/区间），companion clip 同步移动；Option/Alt 改为复制。手势收尾统一转 `moveClips`/`trimClips`/`splitClip`/`duplicateClips` 等命令。`TimelineRegion` 负责接收媒体面板拖来的 drop（`MEDIA_DND_TYPE`）→ `addMediaToTimelineAt`。

## 叠加组件

- `TrackHeaderColumn.tsx`：固定 100px 左列，色条 + V1/A1 标签 + 静音/隐藏/同步锁开关（→ `setTrackProps`），轨高拖拽（UI-only）。
- `Playhead.tsx`：`x = frame * pixelsPerFrame - scrollLeft + headerWidth`，红线 + 下三角，`pointer-events: none`。
- `SnapIndicator.tsx`：黄色虚线，文档空间从标尺到底，`z-index 90`。
- `ClipContextMenu.tsx`：右键菜单（分割/删除/链接/交换/淡入淡出插值…），位置防溢出反转。
- `SwapMediaPicker.tsx`：模态选源，严格类型匹配候选，保留 trim/速度/关键帧/变换（→ `swapMedia`）。

---

## 完成状态

- **已实现**：Canvas 绘制（含波形/缩略图占位、关键帧、淡入淡出、修剪把手）、像素↔帧换算、吸附/多探针、标尺刻度、命中测试、刮擦/缩放/平移/移动/修剪/切割手势、媒体拖放落轨、轨道头开关、右键菜单、媒体交换。
- **计划中/对齐项**：与 SPEC §5「Timeline 核心」逐项 1:1 验收（拖拽落点/磁吸距离/动画时长照搬常量）；缩略图/波形数据接线随 `opentake-media` 推进。

## 相关文档

- 手势产出的命令与串行化 → [state-stores.md](state-stores.md)
- 数值常量（`LAYOUT`/`SNAP`/`ZOOM`/`CLIP`/`FADE`）来源 → [hooks-i18n-theme.md](hooks-i18n-theme.md)
- 完整几何/交互规格 → [SPEC.md](SPEC.md)（§5 Timeline）
- 上游算式真理来源 → [../../architecture/MODULE-PORT-MAP.md](../../architecture/MODULE-PORT-MAP.md)

---

## 页脚

- 本模块目录：[INDEX.md](INDEX.md)
- 模块文档树：[../INDEX.md](../INDEX.md)
- docs 总目录：[../../INDEX.md](../../INDEX.md)
