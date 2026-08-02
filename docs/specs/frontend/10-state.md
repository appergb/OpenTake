# Zustand 状态结构（只读镜像 + UI-only 态）

Rust 是工程、时间线和媒体的唯一真相源。前端将权威快照与纯 UI 交互状态分开：镜像只能整体替换，UI 态可以本地更新。

### 10.1 镜像态（来自 Rust，只读）—— `useProjectStore`

`useProjectStore` 保存 `projectEpoch`、`timelineVersion`、已深拷贝并冻结的 `timeline`、`projectPath`、兼容只读状态、保存版本与 undo/redo 可用性。工程媒体由独立 `mediaStore` 承载，同样只能由 Rust 命令返回/事件驱动刷新。

权威边界规则：

- 生产代码不得对镜像的 `tracks`、`clips`、帧位置或 version 做就地写入；所有编辑调用 Rust 命令。
- `replaceProjectSnapshot` 拒绝更小的 project epoch，也拒绝同工程更小的 version。
- 事件承诺版本 N 后，`sync` 只能发布达到 N 的快照；有界重试仍达不到时保留当前快照。
- 并发刷新用 generation 和 mutation revision 仲裁，迟到的 N/N+1 不得覆盖已接受的 N+2。
- 编辑、undo、redo 和工程切换后重取权威快照；失败命令不改镜像。

### 10.2 UI-only 态 —— `useEditorUiStore`（前端自管）

`useEditorUiStore` 只持有交互状态：

- 导航/对话框：`view`、settings/export/save-as/project-settings 状态。
- 播放头：`currentFrame`、`activeFrame`、播放/scrub 与 decoder fallback 状态。
- 选择：clip/media/folder 选择、range、gap、marquee。
- 时间线视图：`zoomScale`、最小缩放、scroll、可见宽度、工具与临时 track display height。
- 预览：canvas zoom/offset、预览质量、媒体预览、crop 交互。
- 面板：focus/maximize/fullscreen、layout preset、面板可见性与子标签。
- 项目内临时 UI：当前媒体文件夹、swap 目标、toast 等。

`resetProjectRuntimeState` 在工程 identity 变化时清除播放头、选择、滚动、track heights、预览媒体、crop/swap/对话框等项目内状态，同时保留全局 layout、面板可见性与 timeline zoom 偏好。

### 10.3 派生选择器（前端纯函数，不进 store）

`totalFrames`、clip rect、playhead x、track zones、有效 range、Inspector 选中对象与可用 tab 都由当前 Rust 镜像 + UI 态即时计算。不将这些可派生数据再次持久化或作为第二份工程真相。

### 10.4 持久化

UI store 只写入带 schema 版本的 `opentake.ui.v1.*` 全局偏好：

- `layoutPreset`
- `agentPanelVisible`
- `mediaPanelVisible`
- `inspectorPanelVisible`
- `keyframesPanelVisible`
- `zoomScale`（`0.05...40`）

旧的无前缀同名键仅作一次迁移来源；合法值迁移到 v1，损坏、越界或无法解析的值会删除并使用默认值。这些键不包含 project id/path，因为它们是跨工程用户偏好；所有项目内 UI 态在切换时清理而不持久化。

主题、窗口大小、默认导入目录和 BYOK provider 选择由 `settingsStore` 持久化；API key 只进入系统密钥链，不进入 Zustand/localStorage。最近工程在桌面端以 Rust `ProjectRegistry` 为权威，localStorage 仅用于浏览器预览和旧版迁移缓存。timeline、clips、media catalog、凭据、拖拽/播放/对话框临时状态均不得写入 UI 持久化表面。
