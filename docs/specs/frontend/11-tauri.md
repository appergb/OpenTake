# Tauri command / event 对接点

前端的时间线是 Rust 真相源的只读投影。所有提交性编辑通过 `api.editApply(command)` 调用 `edit_apply`；成功后根据事件中的 project epoch/version 重取 `get_timeline`。拖拽 ghost、hover、playhead 等短暂 UI 状态可本地更新，不得写回时间线镜像。

### 11.1 命令（invoke）—— 编辑 / 读取 / 播放 / 工程

| 前端行为 | 生产命令 |
|---|---|
| 41 种时间线编辑（移动、trim、split、文字、字幕、属性、关键帧、轨道、文件夹等） | `edit_apply { command }` |
| 撤销/重做/可用性 | `undo`, `redo`, `can_undo`, `can_redo` |
| 工程新建/打开/保存 | `project_new`, `project_open`, `project_save { path }` |
| 导入/重链/媒体目录 | `import_folder`, `import_media`, `relink_media`, `get_media` |
| 播放/精确 seek/预览端点 | `playback_start`, `playback_pause`, `playback_stop`, `playback_seek`, `get_preview_endpoint` |
| 视频导出/取消 | `export_video`, `cancel_export` |
| 时间线交换/字幕 | `export_xmeml`, `export_fcpxml_modern`, `export_edl`, `export_otio`, `export_subtitles` |
| 生成/转录/字幕/语义搜索 | 使用核心规范第 6.1 节的同名已注册 handler |

全部前端字面量 invoke 名由 `api.commandContract.test.ts#frontend_command_names_match_invoke_handler` 与 Rust 注册表比对。安全的自包含 bundle 导出尚未完成 Rust 持有的目标选择/披露流程，因此 UI 不提供该选项，`exportBundle` 能力门会明确拒绝，不会伪造成功或调用未注册命令。

### 11.2 事件（listen）

| event | 核心载荷/语义 | 前端处理 |
|---|---|---|
| `timeline_changed` | `{ kind, projectEpoch, version }` | 刷新不低于事件版本的权威快照 |
| `project_opened` | `{ kind, path, projectEpoch, version }` | 原子切换工程 identity 后刷新 |
| `media_changed` | `{ kind, projectEpoch, count }` | 重取工程媒体 |
| `export://progress` | `{ operationId, done, total }` | 只更新同 operation 的导出进度 |
| `transcribe://progress`, `search-model://progress`, `search-index://progress` | 各任务进度 DTO | 更新对应任务状态 |
| `playback://frame` | 带 playback identity 的帧位置 | 仅接收当前 session |
| `chat://delta`, `chat://tool-call`, `chat://done` | 带 session/turn identity 的流式消息 | 隔离迟到或已取消 turn |

`src-tauri/src/lib.rs#forward_core_event` 负责将每个 `CoreEvent` 转换为同名 Tauri 事件；单个 WebView emit 失败不得中断后续 `EventBus` 观察者。

### 11.3 缩略图、波形与预览帧

Rust 媒体层生成波形和 timeline sprite，前端通过 `get_waveform`、`request_timeline_sprite`、`generate_thumbnail`、`preview_poster` 和 `preload_media` 按需取得并缓存。复合预览通过 `composite_frame`；持续播放通过 playback engine 的环回传输，不使用已废弃的 `preview_frame` 大块 event 契约。

### 11.4 命令节流与提交

Scrub 和拖拽过程只改 playhead/ghost 等 UI 态；松手时提交一次 `edit_apply`。连续属性控件在 commit 边界合并。命令成功后仍以 Rust 快照为准，不将本地乐观状态当成已提交时间线。
