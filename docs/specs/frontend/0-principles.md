# 总体复刻原则与验收方式

### 0.1 三条不可动摇的原则

1. **像素与帧的换算公式逐字照搬**。时间线 X 轴：`x = headerWidth + frame * pixelsPerFrame`（`TimelineGeometry.swift:138-140`）。`pixelsPerFrame == editor.zoomScale`，初值 `Defaults.pixelsPerFrame = 4.0`（`Constants.swift:61`）。
2. **所有阈值、命中区、磁吸距离、动画时长照搬常量**。不允许"差不多"。
3. **真相源在 Rust**。前端仅持 `Timeline` 只读镜像 + UI-only 态（selection/zoom/hover/tab 等）。每次编辑命令后由 Rust 广播 `timeline_changed{version}`，前端据此重取（对应上游 `timelineRenderRevision`，`EditorViewModel.swift:76,27-29`）。

### 0.2 上游唯一可观测容器 → 前端 store 映射

上游 `EditorViewModel`（`@Observable @MainActor`，`EditorViewModel.swift:21-23`）同时持有①持久化态（`timeline`/`mediaManifest`/`generationLog`）②面板焦点态③大量瞬态 UI 态。OpenTake 跨进程拆分：①持久化态 = Rust 真相，前端持镜像；②③ = 前端 Zustand UI 态。**§10 给出完整字段拆分。**

### 0.3 验收方式（每个面板都要做）

- **截图对拍**：同一工程在上游 macOS app 与 OpenTake 中并排，逐面板对比布局/间距/字号/颜色。断点至少 1600×1000（`AppTheme.Window.projectDefault`，`AppTheme.swift:234`）。
- **行为对拍**：§9 每条交互逐项手动验证（拖拽落点、磁吸、右键菜单项、快捷键）。
- **几何对拍**：给定 `fps/zoom/clip` 集合，断言 clip rect / playhead x / 刻度位置与上游公式输出一致（可单测 geometry 纯函数）。
