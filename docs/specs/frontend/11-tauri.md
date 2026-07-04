# Tauri command / event 对接点

> 依据 ARCHITECTURE.md §2「真相源在 Rust,前端持镜像」+ §5 EditCommand 枚举。前端**所有编辑都经 invoke 发命令**,**绝不本地改镜像**;镜像由 event 更新。

### 11.1 命令（invoke）—— 编辑 / 读取 / 播放 / 工程

> 上游所有编辑手势最终调 `EditorViewModel` 的方法(如 `moveClips`/`commitTrim`/`splitClip`/`addClips`...),OpenTake 归一到 `EditCommand`(ARCHITECTURE §5)。前端把 §9 各手势的"提交"映射到对应 invoke。

| 前端动作（来自 §9 手势/菜单） | invoke 命令（建议名）| 上游对应方法 |
|---|---|---|
| 移动 clip 提交 | `edit_apply` { MoveClips } | `moveClips` (TimelineInputController:862-868) |
| 复制 clip 到位 | `edit_apply` { DuplicateClips } | `duplicateClipsToPositions` |
| trim 提交 | `edit_apply` { TrimClips } | `commitTrim` (:440-458) |
| split | `edit_apply` { SplitClip } | `splitClip` (:74) |
| 删除 / ripple 删 | `edit_apply` { RemoveClips / RippleDeleteRanges } | `deleteSelectedClips`/`rippleDelete*` |
| 拖入添加 / ripple 插 | `edit_apply` { AddClips / InsertClips } | `addClips`/`rippleInsertClips` (:966-1020) |
| 改属性(scale/rotation/opacity/volume/speed/fade/flip/crop) | `edit_apply` { SetClipProperties } | `commitScale/Rotation/Opacity/Volume/ClipSpeed/Fade` 等 (InspectorView) |
| 关键帧 戳/删/移/插值 | `edit_apply` { SetKeyframes } | `stampKeyframe`/`removeKeyframe`/`commitMoveVolumeKeyframe`/`setInterpolation` |
| 加文字 | `edit_apply` { AddTexts } | `addTextClip` (ToolbarView:39) |
| 加字幕 | `edit_apply` { AddCaptions } | CaptionTab 生成 |
| Link / Unlink | `edit_apply` { Link / Unlink } | `linkClips`/`unlinkClips` (:840-848) |
| 轨道 mute/hide/sync/高度 | `edit_apply` { SetTrackProps } | `toggleTrackMute/Hidden/SyncLock`/`setTrackHeight` (TimelineHeaderView) |
| 删轨 | `edit_apply` { RemoveTracks } | `removeTracks` |
| 文件夹 建/移/重命名/删 | `edit_apply` { CreateFolder/MoveToFolder/... } | EditorViewModel+Folders |
| Undo / Redo | `undo` / `redo`（或 `edit_apply{Undo/Redo}`）| `undo:`/`redo:` (ToolbarView:79-85) |
| Copy / Paste clip | `clip_copy` / `clip_paste`{trackIndex,frame} | `copySelectedClipsToClipboard`/`pasteClips` |
| 工程 新建/打开/保存/另存 | `project_new`/`project_open`/`project_save`/`project_save_as` | NSDocument (MainMenu) |
| 导入媒体 | `import_media`{urls} | `importMedia` |
| 导出 | `export_start`{preset} | `showExportDialog`→导出 |
| seek(播放头) | `seek`{frame, mode} | `seekToFrame` (EditorViewModel:259-267) |
| 播放/暂停 | `play`/`pause`/`toggle_playback` | `togglePlayback` (:227-233) |
| 改工程设置(fps/分辨率/宽高比) | `set_timeline_settings`{fps,width,height} | `applyTimelineSettings` (PreviewContainerView:160) |
| 截帧到媒体 | `capture_frame` | `captureCurrentFrameToMedia` (:117) |
| Swap/Save as Media | `swap_media`/`save_clip_as_media` | `beginMediaSwap`/`saveClipAsMedia` |
| 读时间线 | `get_timeline` → Timeline + version | (镜像同步用) |
| 读媒体库 | `get_media` → assets/folders | (镜像同步用) |
| relink 离线 | `relink_asset`{id,url} / `relink_folder`{url} | `relinkAsset`/`relinkOfflineAssets` (PreviewContainerView:367-390) |

> `edit_apply` 返回 `EditResult`{changed, action_name, affected_clip_ids, timeline_version, summary}（ARCHITECTURE §5）。前端据 `changed/timeline_version` 决定是否重取。

### 11.2 事件（listen）—— Rust → 前端推送

| event | payload | 前端处理 |
|---|---|---|
| `timeline_changed` | `{ version: u64 }` | 若 version > 本地 → `get_timeline` 重取镜像 → 触发重绘（对应上游 `timelineRenderRevision` 自增→刷新，`EditorViewModel.swift:27-29,304-312`）|
| `preview_frame` | 帧数据(纹理/位图引用 或 共享内存句柄) | 上屏到 `PreviewCanvas`（对应上游 `videoEngine` 帧回调）|
| `playhead_changed` | `{ frame }` | 播放时更新 `activeFrame`（上游 playheadState 由 engine 推进）|
| `media_changed` | `{}` 或增量 | 重取 `get_media`（缩略图/波形就绪、导入完成、生成完成）|
| `media_thumbnails` | `{ assetId, sprite/samples }` | 更新 timeline clip 缩略图/波形缓存（上游 `MediaVisualCache`）|
| `generation_progress` | `{ assetId, status, ... }` | 更新生成中 overlay / 失败态 |
| `export_progress` | `{ progress, ... }` | 导出进度 UI |
| `settings_mismatch` | `{ ... }` | 弹导入设置不匹配对话框（上游 `pendingSettingsMismatch`，`EditorViewModel.swift:221`）|

### 11.3 缩略图 / 波形 / 帧 的传输

上游 `MediaVisualCache`(`Timeline/MediaVisualCache.swift`)在内存持 sprite 网格 + PCM 降采样。OpenTake：Rust 媒体层算好（ffmpeg 抽帧 sprite + symphonia 波形），经 `media_thumbnails` event 或专门 command 传给前端（图片用 data-url / blob / 共享内存）。前端缓存后供 TimelineCanvas 的 `drawClip` 使用。**预览帧** `preview_frame` 优先共享内存/零拷贝（WebGL 纹理）以满足 scrub 实时性。

### 11.4 命令节流（复刻上游 debounce）

上游对快速重建做合并（`notifyTimelineChangedDebounced` 120ms，`EditorViewModel.swift:315-324`），对 scrub 用 `interactiveScrub` 模式只更 `playheadState` 不提交（`:261-265`）。前端：scrub/拖拽过程中**本地乐观更新 UI 态**（playhead、ghost），**松手才发 `edit_apply`**；连续属性拖拽（ScrubbableNumberField onChanged）可节流或仅在 onCommit 发命令。
