# OpenTake — 音视频编辑 / 媒体库 / MCP 未完成清单

> 2026-06-24 整理。配合用户本轮反馈,逐项标注「已完成 / 部分完成(差什么) / 未做」,附文件路径与上游参考。
> 状态基于实际代码核查(集成分支 `codex/integration-20260624`),非臆测。

---

## 0. 本轮(2026-06-24)已修复的阻断性问题

| 反馈 | 处理 | 文件 |
|---|---|---|
| **clips 删不掉** | 删除前过滤成时间线中真实存在的 clip id(单个 stale id 会让后端整批 RemoveClips 失败),并在 Tauri 下删除后主动 `forceRefresh` | `web/src/store/editActions.ts` `liveSelectedClipIds`/`deleteSelectedClips`/`rippleDeleteSelectedClips` |
| **拖入音频自动跑到末尾** | 拖放改为按鼠标释放点落点:start=落点帧、轨道=落点轨;落点轨重叠则换其他空闲同类轨,无则新建轨(不覆盖既有片段) | `TimelineContainer.tsx` `onMediaDrop` + `editActions.ts` `addMediaToTimelineAt`/`firstOpenCompatibleTrackIndex`;`TimelineRegion.tsx` 去掉重复 append |
| **选中高亮发灰、不明显** | 选中边框改醒目蓝 `rgba(56,139,253,1)` 2px(原近白色边框在片段上发灰) | `web/src/components/timeline/clipRenderer.ts` `SELECTION_BLUE` |
| **预览拖动严重卡顿/延迟** | 合成帧由 140ms 尾随防抖改为「实时帧 + ~12fps 速率闸门」:拖动即时出帧、不再等停下,仍受限不爆量 | `web/src/components/preview/Preview.tsx` `SCRUB_MIN_INTERVAL_MS` |
| **右键菜单固定在左上(0,0)** | 菜单按光标 `clientX/clientY` 定位 + 视口翻转;并修掉渲染期 `onClose()`(改 effect) | `TimelineContainer.tsx` menu state(x/y)+ `ClipContextMenu.tsx` |
| **导入文件夹只导顶层** | 前端改 `importFolder(path, true)` 递归导入整棵目录树下的媒体 | `web/src/store/mediaActions.ts` |
| **银色 Generate 按钮点了没反应** | 加 onClick → 弹「AI 生成功能即将推出」toast(后端生成仍为 stub) | `MediaPanel.tsx` + `dict.ts` `media.generateSoon` |
| **主页 macOS 红绿灯位置偏上、OpenTake 间距挤** | 红绿灯下移 `trafficLightPosition {x:18,y:24}`;`--titlebar-safe-top` 30→44、`--titlebar-safe-left` 78→82 让顶部 UI 下移留白 | `src-tauri/tauri.conf.json`、`web/src/styles/tokens.css` |

---

## 1. 仍未完成 / 本轮明确延后的项(给 Codex 接力)

### 1.1 媒体库 / 文件夹(用户很想要)
- **嵌套文件夹浏览 UI(钻取/双击进入/面包屑/拖出)** — 未做。后端 `import_folder` 已支持 `recursive`,但 DTO 缺 `folderId/folders` 的层级展示,前端无文件夹瓦片导航。需按上游 `FolderTileView` 做(对应 issue #49)。本轮仅打开了递归导入开关。
- **不支持格式的展示** — 未做。`import_folder` 只白名单媒体;目录里的非媒体文件不会列出。需后端返回「未支持文件」占位项 + 前端灰显瓦片。
- **My ↔ Import 切换缩略图重载** — 未修(延后)。根因:`MediaPanel.tsx:90 visibleItems = items.filter(...)` 在切 subTab 时把不匹配项**卸载**,切回时 `<img>` 缩略图重新解码 → 视觉「重载」。推荐修法:渲染全部当前目录项、用 `display:none` 做 subTab 过滤(保持 img 挂载),或加一层缩略图缓存预加载。改动涉及网格结构,需单独小 PR 验证空状态/计数不回归。
- **媒体面板星标迁后端** — 部分完成。面板星标仍用 `localStorage`(`favorites.ts`);后端 `library_favorite` 命令已就绪未接。

### 1.2 片段 / 时间线编辑
- **片段右键菜单补全** — 部分完成。现有 Split/Delete/Link/Unlink;缺 **Swap Media / Save as Media / Extract Audio / Copy·Cut·Paste**(上游 `TimelineView.swift:741-748`)。Copy/Paste 键盘已可用,仅右键入口缺。
- **多选 Split** — 部分完成,当前只对单片段切。
- **Toolbar `[` / `]` / Add Track 按钮** — 未接 onClick(`Toolbar.tsx`)。
- **Inspector 三段式(scrub 实时→防抖→单条 undo)** — 未做,现每次拖动直接发命令,产生大量 undo 历史(`Inspector.tsx`)。
- **轨道重排序 / Solo** — 未做(`TrackHeaderColumn.tsx`)。
- **时间线 Marker** — 未做(上游有 `TimelineMarker`)。

### 1.3 Inspector
- **Color Grade / Chroma Key / Mask / Effect 的 UI 面板** — 未做。后端命令 + MCP 工具均已实现,缺 Inspector tab UI。
- **Text 完整 textStyle(字体/字号/颜色)** — 部分完成,依赖后端 `ClipProperties` 扩展。
- **AI Edit tab** — 未渲染(类型已定义但 tabs 数组从不 push)。

### 1.4 音频
- **真实音频输出(cpal)+ A/V 同步播放引擎(#53)** — 部分完成。`TimelinePlaybackLayer` 用 HTML5 媒体元素能放视频,但无 cpal 真实音频路径。最大未完成项。
- **转录(whisper)/ 自动字幕(add_captions)** — 未做(依赖端上 whisper 接线)。

### 1.5 主页 macOS 圆角
- **「用旧版 macOS 圆角而非 macOS 26 风格」** — 未做。窗口圆角由 OS 渲染,Tauri 不直接暴露旧版圆角半径;本轮只调了红绿灯位置与安全区。若要改圆角需自绘窗口/装饰层,代价大,建议另议。

---

## 2. MCP / Agent 逻辑现状

**服务器**:`http://127.0.0.1:19789/mcp`(rmcp Streamable-HTTP,loopback + Origin 守卫),在 `src-tauri/src/mcp.rs` `setup()` 内 `tokio::spawn` 启动,bind 失败仅记日志不影响应用。`claude mcp add --transport http opentake http://127.0.0.1:19789/mcp`。

**已完整接线(28)**:读取(get_timeline/get_media/list_folders/list_models/list_workflows)、片段(add/insert/move/remove/split/ripple_delete)、属性(set_clip_properties/set_keyframes/set_color_grade/chroma_key/set_mask/apply_effect)、文本(add_texts)、轨道(remove_tracks)、媒体库(rename/delete/create_folder/move_to_folder)、undo、workflow(activate/deactivate)。

**Stub(12,统一返回 "not yet implemented",`dispatch.rs:171-190`)**:
- **媒体读取** `inspect_media`/`get_transcript`/`search_media` — 根因 `CoreHandle` trait 未暴露 `MediaEngine`;需拓宽 `CoreHandle`(新增 `media_engine()`)。
- **导入** `import_media` — 同上,需 `AppCore` 层新增 `import_media_path` 原子命令。
- **字幕** `add_captions` — 依赖 `get_transcript`(whisper),`subtitle_export.rs` 纯逻辑已就绪缺桥接。
- **生成** `generate_video/image/audio`/`upscale_media` — 需 `opentake-gen` 异步 `GenClient` + BYOK 注入 `Dispatcher`;且 `get_timeline.canGenerate` 硬编码 `false`(`dispatch.rs:124`)阻止模型主动调用,需改为 `gen_client.is_some()` 运行时判断。
- **动效** `add_motion_graphic`/`edit_motion_graphic` — 依赖 Lottie 烘焙(#34)。
- **时间线分析** `inspect_timeline` — 依赖 `composite_frame` 稳定后接入。

**局部 gap**:`create_folder`/`move_to_folder` 不支持 batch `entries`;`canGenerate` 恒 false。
