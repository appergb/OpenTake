我先确认一下任务范围。这是一份纯粹的综合写作任务:你已经给了 5 份详尽的研读 JSON,要求我把它们综合成一份中文 Markdown 文档。我不需要读代码库或验证——所有素材都在 JSON 里。下面直接输出文档本身。

# OpenTake 1:1 复刻差距与实现计划

## 〇、现状真话：现在能不能完成一次真实剪辑闭环？

**结论：不能。一次也跑不通。**

把"导入 → 预览 → 拖到时间线 → 剪 → 导出"逐段对照,OpenTake 当前是"内核很硬、四肢半残、神经没接线"的状态——底层算法层(Rust)复刻度高得惊人(ops 层 place/split/trim/move/clear_region/ripple 全部 1:1 端口,EditCommand 21 个变体,wgpu 合成器+shader+回读像素都已就绪,约 960+ 单测通过),但**凡是"用户能看见、能点、能拖"的边界粘合层几乎全是空的或假的**。

逐环节体检：

- **导入**：命令通路其实是通的(import_media/import_folder 已注册、emit MediaChanged、前端 refreshMedia 直刷)。但用户会觉得"点了没反应",根因有三:① 非 Tauri 环境 `openDialog()` 返回 null 后**静默 return**,毫无反馈;② ffprobe sidecar 没就绪时 probe 失败被吞成默认值,所有项 duration=0、缩略图本就是 None,网格里只有灰底图标,视觉上像没导入;③ 视频白名单只有 mov/mp4/m4v,mkv/webm/avi 被**静默丢弃**。而且 `MediaItemDto.thumbnail` 写死 `None`——**导入的视频根本没有缩略图**,尽管底层 `MediaEngine` 的缩略图能力早已实现,只是没接线。
- **预览**：**完全是空的,这是最痛的痛点。** `Preview.tsx` 自己的注释都写明"Rust composite frames via `preview_frame` event — not yet wired",当前只画一个背景 + 占位文字。**点任何素材,右侧不会出现任何画面。** `usePlaybackTicker` 只用 requestAnimationFrame 推一个假的帧号,没有解码、没有上屏、没有音频。`src-tauri` 里**根本没有任何 preview/seek/play 命令,也没有 `preview_frame` 事件**。wgpu 合成器质量很高,但目前**只被单测调用,没接到运行期窗口**。更底层:Tauri 的 `protocol-asset` 没开、CSP 为 null、capabilities 无 asset 权限,所以前端**连 `convertFileSrc` 指向本地文件都用不了**。
- **拖到时间线**：拖拽通路本身是接通的(MediaPanel onDragStart → TimelineRegion onDrop → addClips → editApply → Rust)。但**起点就断**:新建工程是空时间线(tracks: []),`AddClips` 命令没有建轨逻辑,前端 `firstCompatibleTrackIndex` 找不到轨就 return null——`addMediaToTimeline` 代码注释自承"empty timeline silently no-ops"。**拖片到空时间线 = 静默失败,什么都不会发生。**
- **剪**：一旦有了轨,核心剪辑能力其实可用(split/trim/move 真正发命令、走真算法)。但 Toolbar 的 Trim Start `[`、Trim End `]`、Add Text `T` 是**死按钮**(无 onClick);Inspector 无三段式(每次提交一条 undo,scrub 拖动会刷爆 IPC);多选 split 只支持单选。
- **导出**：5 份研读未覆盖导出子系统,但从"预览/播放引擎几乎为零"可推断成片导出链路同样未接线(导出需要的逐帧合成正是阶段 C/D 的能力)。**导出在本轮属于未验证/未接通状态,不应承诺。**

**一句话**：OpenTake 现在是一个"能记账但不能放映、能落子但开局没有棋盘"的剪辑器内核。要让它"像个剪辑软件",缺的不是算力,是**最后一公里接线 + 几处起点级的空洞(空时间线、新建不落盘、预览不上屏、窗口黑边)**。

---

## 一、差距清单（按优先级 P0 → P2）

> 格式：**差距** ｜ 上游怎么做（文件:符号）｜ 我们怎么忠实复刻（跨栈映射）｜ 涉及文件

### P0 —— 不补就没法当剪辑软件用

#### P0-1 新建项目必须让用户选择保存位置（默认 `~/Documents/OpenTake`）
- **差距**：上游新建即先选盘再落地,OpenTake 新建只调 `projectNew()` 就进编辑器,`project_dir` 恒为 None、根本不落盘——正是用户抱怨"新建无法选保存位置"。
- **上游**：`AppState.swift:123-141 createNewProject`(立即弹 `NSSavePanel`,默认目录 `~/Documents/Palmier Pro`,默认名 Untitled Project,OK 后才 `VideoProject()` + 立即 `doc.save(.saveOperation)` + `ProjectRegistry.register`);`Constants.swift:115-120 storageDirectory`;`VideoProject.swift:66-97 save/fileWrapper`。
- **复刻**：`newProjectAndEnter()` 改为先调 `tauri-plugin-dialog` 的 `save()`(defaultPath = `<docDir>/OpenTake/Untitled.opentake`,filter 扩展名 opentake)→ 用户选定后 `project_new` + `project_save(Some(path))` 立即落盘 + `recents.add(path)` + `setProjectPath`。Rust 端加常量 `storage_dir = dirs::document_dir()/"OpenTake"` 懒创建,建议出 `get_default_project_dir` 命令避免前端硬编码。
- **涉及文件**：`web/src/store/projectActions.ts:18-22`、`crates/opentake-core/src/session.rs:117-123 / 156-174`、`src-tauri/src/commands.rs:50-51`。

#### P0-2 双重导入 + 文件夹浏览（剪映式钻取）
- **差距**：① 后端 `get_media` 不返回 folders/folderId,面板无法点进文件夹浏览(用户核心诉求);② `import_folder` 只把文件拍平进根,不镜像目录树成文件夹层级;③ "导入没反应"的兜底缺失(非 Tauri 静默 return / probe 失败 / 不支持扩展静默跳过)。
- **上游**：`MediaTab.swift:242-250 openFolder / 298-352 breadcrumb / 469-478`;`EditorViewModel+MediaLibrary.swift:88-110 importFinderItems(整批一个 undo) / 113-132 importFolder(递归镜像建 MediaFolder)`;`FolderTileView.swift`;`MediaTab.swift:749-761 importMedia`;`addMediaAsset` 不支持扩展弹 `mediaPanelToast`。
- **复刻**：
  1. **DTO 补全(最先做,解锁后续)**：`MediaItemDto` 加 `folder_id: Option<String>`(camelCase folderId),`MediaListDto` 加 `folders: Vec<{id,name,parentFolderId}>`,从 `core.media().folders` 投影;前端 `MediaItem` 加 folderId、新增 `MediaFolder` 类型。
  2. **文件夹浏览 UI**：`mediaStore` 存 folders + currentFolderId + viewMode;实现 `.folder` 钻取(subfolders + assetsByFolderId)、`FolderTile`(单击选/双击 openFolder)、breadcrumb、navigateUp。
  3. **镜像导入**：重写 `import_folder`,对每级目录派 `CreateFolder`(ops::create_folder 已有),`import_media_file` 增 `folder_id` 参数,递归按 case-insensitive 名排序保证 id 确定性;整批导入做成一个原子步(对齐上游 `disableUndoRegistration` 整批)。
  4. **"没反应"兜底**：非 Tauri / dialog 缺失给 toast 或 disabled;不支持扩展回传 skipped 计数并提示(对齐 `mediaPanelToast`);核实 ffmpeg/ffprobe sidecar 就绪。
- **涉及文件**：`src-tauri/src/media.rs:60-126 / 156-255`、`web/src/components/media/MediaPanel.tsx:356-440`、`web/src/store/mediaStore.ts`、`web/src/store/mediaActions.ts:30-77`、`crates/opentake-core/src/session.rs:204-231`。

#### P0-3 点素材 → 右侧预览（单素材预览）
- **差距**：用户最直接的诉求。点素材右侧不出现任何画面;`Preview.tsx` 是静态占位,MediaPanel 点击不触发预览。
- **上游**：`EditorViewModel+PreviewTabs.swift:32-47 selectMediaAsset→openPreviewTab`;`VideoEngine.swift:89-127 activateTab`(视频/音频 = `player.replaceCurrentItem(AVPlayerItem(url:))` 直接喂原文件 URL);`PreviewContainerView.swift:287-298`(图片用 NSImage 直显)。
- **复刻（务实零 Rust 改动）**：点 MediaPanel 素材 → 设 `activePreviewTab=mediaAsset(id)`。Preview 画布按类型渲染:`video→<video src={convertFileSrc(path)}>` 受 transport 控制(currentTime=frame/fps、play/pause);`image→<img>`;`audio→<audio>` + 波形占位。`MediaItemDto.path` 已暴露,直接用。
- **涉及文件**：`web/src/components/preview/Preview.tsx:63-98`、`web/src/components/media/MediaPanel.tsx:392`。

#### P0-4 Tauri 资源协议 / 权限（P0-3 的前置）
- **差距**：Tauri webview 默认禁本地文件,`protocol-asset` 未开、CSP 为 null、capabilities 无 asset scope → `convertFileSrc` 用不了。
- **上游**：AVFoundation 原生文件访问,无对应。
- **复刻**：`tauri.conf.json` 给 `tauri` 加 `features=["protocol-asset"]`;`assetProtocol.enable=true` + scope 允许媒体目录(开发期可 `["**"]`,**生产必须收敛**);CSP 设 `media-src/img-src 'self' asset: http://asset.localhost ipc:`;capabilities 加 asset 权限。前端 `import { convertFileSrc } from '@tauri-apps/api/core'`。
- **涉及文件**：`src-tauri/tauri.conf.json:28-30`、`src-tauri/Cargo.toml:25`、`src-tauri/capabilities/default.json`。

#### P0-5 拖到时间线成片段（空时间线 + AddClips 不能建轨）
- **差距**：闭环的第 0 步就断。新建工程空时间线 + AddClips 无建轨逻辑 → 拖片到空时间线静默 no-op。
- **上游**：`ToolExecutor+Clips.swift:188-206`(trackIndex 全省略时 `insertTrack(at:0,.video/.audio)` 自动建轨);`EditorViewModel.swift:328-375 placeClip/resolveOrCreateAudioTrack`(视频带音频自动建/解析音频轨)。
- **复刻（两条路径,建议都做）**：① 新增 `EditCommand::InsertTrack{at,kind}` 薄封装 `ops::insert_track`(tracks.rs:59 已存在),或在 `add_clips` 内复刻"trackIndex 省略→insertTrack(at:0)"分支;② 新建/打开未带轨工程时 seed 默认 V1+A1 轨。前端 `firstCompatibleTrackIndex` 无轨时触发建轨而非 return null。
- **涉及文件**：`crates/opentake-ops/src/command.rs:346`、`crates/opentake-ops/src/ops/tracks.rs:59`、`web/src/store/editActions.ts:106-156`、`crates/opentake-domain/src/timeline.rs:58-68`。

#### P0-6 窗口黑边 / 消除系统（黑色）标题栏
- **差距**：`tauri.conf.json` 用系统默认装饰,会出现系统原生(可能黑色)标题栏与大黑边。
- **上游**：`HomeView.swift:212-238 HomeWindowController`(`titleVisibility=.hidden` + `titlebarAppearsTransparent` + `.fullSizeContentView` + `isOpaque=false` + 半透明 base 背景 RGB10 + `ultraThinMaterial`);`VideoProject.swift:215-233` 编辑器窗口。
- **复刻**：`tauri.conf.json` window 加 `"titleBarStyle":"Overlay"`(macOS 红绿灯悬浮,等价 fullSizeContentView+透明标题栏)+ `"hiddenTitle":true` + 深色 `backgroundColor`(与 `--bg-base` RGB10 同色,防黑/白闪);web 根容器自绘背景 + 毛玻璃。overlay 下内容左上留 ~78px 红绿灯安全区(对应上游 leading titlebar accessory)。首页/编辑器同色,杜绝黑边观感。Windows 平台用 `decorations:false` + 自绘标题栏按钮。`transparent:true` 需 `macos-private-api`,若不开则退而用 Overlay+深色背景即可。
- **涉及文件**：`src-tauri/tauri.conf.json:18-31`、`src-tauri/src/lib.rs`。

#### P0-7 关窗不退出 + 后台常驻 + Dock 重开
- **差距**：Tauri 默认"最后一个窗口关闭即退出进程",与上游"关工程窗回 Home、App 常驻"完全相反;无 macOS activation policy、无 reopen 处理、无 tray。
- **上游**：`AppDelegate.swift`(`setActivationPolicy(.regular)`;不实现 `applicationShouldTerminateAfterLastWindowClosed`→默认不退;`applicationShouldHandleReopen` 无可见窗时 `showHome()`);`AppState.swift:44-68 showHome`;`VideoProject.swift:175-182 close→showHome`。
- **复刻**：`lib.rs` 把 `.run(generate_context!())` 改为 `.build(...)?.run(|app, event| …)`,在 `RunEvent::ExitRequested { api, .. }` 里 `api.prevent_exit()`;`RunEvent::Reopen { has_visible_windows, .. }` 在无可见窗时 show+focus 主窗;`app.set_activation_policy(ActivationPolicy::Regular)`。因 OpenTake 是单 WebView,"关窗回 Home"翻译成 `on_window_event` 拦 `CloseRequested→prevent_close()+hide()` + emit 事件让前端 `setView('home')`。跨平台兜底加 `TrayIcon`(ACL 已含 tray 权限)。**必须保留 Cmd+Q→app.exit(0) 真退出入口**。
- **涉及文件**：`src-tauri/src/lib.rs:19-69`、`src-tauri/tauri.conf.json:19-27`(window 补 `"label":"main"`)。

> 说明：P0-7"关窗不退/自动保存"中的自动保存部分依赖 P0-1(新建即落盘,否则 project_dir=None 时自动保存无处可写),故自动保存归入 P1 但**必须与 P0-1 配套设计**。

---

### P1 —— 让"能用"变"好用 / 数据安全

| 编号 | 差距 | 上游 | 复刻要点 | 涉及文件 |
|---|---|---|---|---|
| P1-1 | **自动保存 / 离开前回写**(autosavesInPlace 等价) | `VideoProject.swift:29 autosavesInPlace`;`AppState.swift:59-64 showHome 前 autosave` | src-tauri 订阅已有 `CoreEvent::TimelineChanged`,启 tokio 防抖(静默 ~800ms–1.5s)调 `save_project(None)`;Close/Exit 前 flush 一次;仅当 `project_dir.is_some()` 才存 | `opentake-core/src/session.rs:156-174`、`src-tauri/src/lib.rs:30-33` |
| P1-2 | **导入缩略图**(视频首帧/图片) | `MediaAsset.swift:96-162 loadMetadata`;`AssetThumbnailView.swift:142-169`;`MediaVisualCache.swift` | 接线已实现的 `MediaEngine.image_thumbnail/video_thumbnails`(DiskCache key=path\|size\|mtime SHA256);video 首帧按真实像素比例(避免压扁竖屏);`MediaItemDto.thumbnail` 填路径,前端 convertFileSrc 显示;异步生成后再 emit MediaChanged | `src-tauri/src/media.rs:79-103`、`opentake-media/src/lib.rs:120-135`、`web/.../MediaPanel.tsx:392-407` |
| P1-3 | **缩略图渐进 finalize**(probe 异步,大文件夹不卡 UI) | `EditorViewModel+MediaLibrary.swift:64-79,419-452` | 导入先快速建 entry(duration=0)emit 让网格立刻出现,probe+缩略图后台逐个补完再 emit | `src-tauri/src/media.rs:156-210` |
| P1-4 | **拖放落点读坐标**(当前总 append 到第一条兼容轨尾) | `MediaPanelDropArea` + `addClips(trackIndex:startFrame:)` | onDrop 用 TimelineContainer 已有 `toDoc/frameAt/trackAt` 把 clientX/Y → (trackIndex,startFrame),clearRegion 覆盖语义已在 add_clips 内 | `web/.../TimelineRegion.tsx:40-47`、`editActions.ts:110-156` |
| P1-5 | **Inspector 三段式 + 文本/关键帧 UI** | `EditorViewModel+ClipMutations.swift:320-428` | live 阶段只乐观更新前端 mirror(不发命令、不入 undo),松手/防抖后发一条 `SetClipProperties`;补 Text tab 映射 text_* 与 SetKeyframes;后端已就绪 | `Inspector.tsx`、`command.rs:567-687` |
| P1-6 | **Toolbar `[` `]` `T` 死按钮接线** | `EditorViewModel+ClipMutations.swift:500-526`;ToolExecutor add_texts | `[`/`]` 封装 `trimStartToPlayhead/EndToPlayhead`(对 selection 按 activeFrame 算 source delta 调 trimClips);`T` 调 addTexts | `Toolbar.tsx:135-146`、`command.rs:163,201` |
| P1-7 | **Finder 拖拽文件/文件夹到面板导入** | `MediaPanelDropArea`;`MediaTab+Drag.swift:99-131` | 面板容器加 Tauri `onDragDropEvent` → 复用 import_media/import_folder,按 currentFolderId 落位;区分 Finder 文件拖入 vs 库内拖动 | `web/.../MediaPanel.tsx:143-233,328-346` |
| P1-8 | **预览标签页机制**(Timeline vs mediaAsset) | `PreviewTab.swift`;`EditorViewModel+PreviewTabs.swift:38-119` | 纯前端状态:点素材 push mediaAsset 标签并切单素材预览,Timeline 标签切回合成预览;实现切换/关闭/前进后退 | `Preview.tsx:147-165` |
| P1-9 | **时间线合成"暂停态"预览**(接通 wgpu) | `CompositionBuilder.swift:34-237`;`VideoEngine.swift:137-185` | 新增 `composite_frame(frame)→RGBA/PNG`:`RenderPlan.frame(f)` + `opentake-media` 实现 `FrameProvider`(decode_frame_at/image_pixels) + `Compositor.render_to_rgba`(已就绪);前端暂停/seek 时 invoke 贴到 `<canvas>`;播放态先降级 | `compositor.rs:280`、`src-tauri/src/lib.rs:53-65` |
| P1-10 | **真实播放引擎**(A/V 同步/连续解码/音频) | `VideoEngine.swift:225-325`;`CompositionBuilder.swift:375-461` | ffmpeg 连续解码 + wgpu 上屏 + cpal 音频 + 自写 A/V 同步 + 精确 seek(解到最近关键帧丢帧)+ 预览降档;工程量最大,**放最后** | `usePlaybackTicker.ts:12-47`、`opentake-media/src/decode/frame.rs` |
| P1-11 | **设置多分页**(Account/Models/Notifications/Privacy/Storage/Agent) | `SettingsView.swift:3-31` + 各 Pane | 改单页滚动为左侧 sidebar 多 tab;Models 接模型目录,Notifications 接系统通知插件,Storage 统计/清理缓存目录,Agent 加 MCP 状态行 | `SettingsView.tsx:82-87` |
| P1-12 | **BYOK 密钥安全存储**(密钥边界,不可明文) | `AgentPane.swift:110-134 AnthropicKeychain` | 用 `keyring` crate 或 `tauri-plugin-stronghold` + save/load/delete 命令;当前 AiPane 是假占位(`setSaved(true)` 不写任何地方) | `SettingsView.tsx:320-389` |
| P1-13 | **MCP Instructions 面板** | `MCPInstructionsPane.swift:1-206`;`AgentPane.swift:138-221` | 展示 server URL `http://127.0.0.1:19789/mcp` + 复制 + Cursor/Claude Desktop/Claude Code/Codex 四客户端配置;依赖 MCP server 先实现 | 新建文件 |
| P1-14 | **新建未命名工程落盘策略** | `AppState.swift:123-141` | 同 P0-1,确保始终先有盘路径再编辑,否则自动保存永远报 NoProjectOpen | `session.rs:117-123,156-160`、`commands.rs:50-51` |

---

### P2 —— 打磨 / 体验对齐 / 非核心

| 编号 | 差距 | 复刻要点 | 涉及文件 |
|---|---|---|---|
| P2-1 | My Projects 缩略图 | save 时截首帧/首图写 bundle 根 `thumbnail.jpg`;recents 补 thumbnailPath,前端 convertFileSrc 显示 | `HomeView.tsx:285-301`、`bundle.rs:129-155` |
| P2-2 | My Projects 用相对时间(非路径) | recents 加 createdDate(首次写入不随 openedAt 变);用 `Intl.RelativeTimeFormat`(跟随 i18n,"20 小时前")替换路径副标题 | `HomeView.tsx:314-325`、`recentStore.ts:13-17` |
| P2-3 | File missing 缺失态 + 右键菜单 + 删除到废纸篓 | 加 `path_exists` 命令判 isAccessible;渲染缺失态;contextMenu 加 Reveal in Finder / Delete Project(回收站) | `HomeView.tsx:265-353`、`recentStore.ts:66-70` |
| P2-4 | 主页 Sample Project 区 | 决定数据源(无后端则内置 1-2 个本地样例或 GitHub release 拉);加 SampleStrip + 下载进度;为空时不渲染(对齐上游) | `HomeView.tsx:43-85` |
| P2-5 | 库内拖拽组织(资源拖进文件夹 / 文件夹互拖) | FolderTile/breadcrumb 加 onDrop → 经 edit_apply 派 `MoveToFolder`(后端已就绪) | `MediaPanel.tsx:376-440`、`commands.rs:148-224` |
| P2-6 | 面板右键菜单 / 重命名 / 删除 / Reveal / relink + viewMode/sort/filter 死按钮 | 工具栏 LayoutGrid/ArrowUpDown/Filter 接状态;MediaCard 加右键/选中/重命名/offline 态(多为前端 + 已有后端命令) | `MediaPanel.tsx:184-208,376-440` |
| P2-7 | scrub 30Hz 节流策略移植 | 接真合成帧后必须移植 `interactiveSeekInterval/interactiveTolerance`,否则拖动让 ffmpeg/wgpu 过载 | `Preview.tsx:167-237` |
| P2-8 | 文本/字幕预览覆盖层 | 单素材/简单场景先用 HTML 绝对定位 div 叠字幕(对齐 videoRect,等价上游 CATextLayer) | `plan/types.rs:119-123` |
| P2-9 | editActions 补封装(InsertClips/RippleDeleteRanges/AddTexts/SetKeyframes/Link…) | 后端齐全,前端缺入口;按需补封装 + 绑右键/快捷键 | `editActions.ts:29-74` |
| P2-10 | 多选 split(当前仅单选) | 遍历所有选中片各发一次 splitClip | `editActions.ts:77-84` |
| P2-11 | isDocumentEdited 脏标记前端反馈 | core 用 `version != last_saved_version` 派生 dirty 传前端,标题栏显示未保存态;驱动防抖保存与退出 flush | `session.rs:251` |
| P2-12 | 两套窗口尺寸 | 单窗不强求双窗,可设 minWidth 760 避免主页空旷 | `tauri.conf.json:21-26` |

---

## 二、实现批次顺序（每批一个功能分支：写 → 审 → 修 → CI 绿 → 合并）

目标：让"能用"最快达成。**批次 1–3 跑完,用户就能完成"导入(带缩略图)→ 点素材右侧预览 → 拖到时间线出片 → 选中/分割/裁剪/移动"的最小真实闭环。**

### 批次 1 ｜ `feat/p0-foundation-window-lifecycle`（地基：让 App 不黑、不退、能落盘）
- P0-4 Tauri 资源协议/权限(后续预览的前置)
- P0-6 窗口黑边/透明标题栏(Overlay + 深色背景)
- P0-7 关窗不退出 + Dock 重开 + tray + activation policy(保留 Cmd+Q 真退出)
- P0-1 + P1-14 新建即选盘落盘 + storage_dir 常量 + get_default_project_dir
- **验收**：启动无黑边/无系统黑标题栏;关窗不退、Dock/tray 重开;新建弹保存对话框、选定后磁盘出现 `.opentake` 包。
- 风险:`.run`→`.build().run(|...|)` 改启动路径,需回归 dev/打包启动。

### 批次 2 ｜ `feat/p0-timeline-bootstrap`（打通拖入出片的起点）
- P0-5 InsertTrack 命令 + add_clips 自动建轨 + 新建/打开 seed V1+A1 + 前端无轨触发建轨
- **验收**：新建工程拖一段视频到空时间线,**立即出现片段成轨**;带音频视频自动联动音频轨。
- 注:此批让"拖到时间线成片段"真正可用,是闭环最关键的单点修复。

### 批次 3 ｜ `feat/p0-media-import-preview`（导入有缩略图 + 点素材右侧预览）
- P0-2 DTO 补 folder_id/folders + 文件夹浏览 UI + 镜像导入 + "没反应"兜底
- P1-2 + P1-3 导入缩略图(接已实现引擎) + 渐进 finalize
- P0-3 点素材 → 右侧单素材预览(`<video>`/`<img>`/`<audio>` + convertFileSrc)
- **验收**：导入文件/文件夹都有反馈;视频有缩略图;能点进文件夹钻取 + 面包屑;点素材右侧立即出现可播放/可逐帧的原片预览。
- 拆分提示:此批较大,可按 3a(导入+缩略图+兜底)、3b(文件夹浏览)、3c(单素材预览)三个子 PR 串行合并。

> **里程碑:批次 1–3 合并后,OpenTake 第一次"像个剪辑软件"——这是最高优先级目标。**

### 批次 4 ｜ `feat/p1-editing-ergonomics`（剪得顺手）
- P1-4 拖放落点读坐标
- P1-6 Toolbar `[` `]` `T` 接线
- P1-5 Inspector 三段式 + Text tab
- P1-8 预览标签页机制(Timeline vs mediaAsset)
- P2-10 多选 split、P2-9 editActions 补封装
- **验收**：拖到指定轨指定位置;`[`/`]`/`T` 可用;拖动数值不刷爆 undo/IPC;多选可一次分割。

### 批次 5 ｜ `feat/p1-autosave-persistence`（不丢工作 + 工程列表体验）
- P1-1 防抖自动保存 + 离开/退出前 flush(配合批次 1 的 close 拦截)
- P2-1/P2-2/P2-3 缩略图 / 相对时间 / File missing + 右键 + 回收站
- P2-11 脏标记前端反馈
- **验收**：编辑后静默自动落盘;关窗前 flush;My Projects 显示缩略图/相对时间/缺失态。

### 批次 6 ｜ `feat/p1-timeline-composite-preview`（成片预览接通 wgpu）
- P1-9 `composite_frame` 命令 + FrameProvider 适配器(opentake-media → render)+ 暂停态 canvas 合成预览
- P2-7 scrub 30Hz 节流移植、P2-8 文本覆盖层
- **验收**：Timeline 标签暂停/seek 时显示带变换/调色/多轨叠加的真实合成帧;拖动不过载。

### 批次 7 ｜ `feat/p1-settings-mcp-security`（设置/安全/MCP）
- P1-11 设置多分页(sidebar 重构)
- P1-12 BYOK 安全存储(keyring/stronghold + 命令)
- P1-13 MCP Instructions 面板(依赖 MCP server)
- P1-7 Finder 拖拽到面板导入、P2-5/P2-6 库内拖拽组织 + 面板右键
- **验收**:设置分页齐全;API Key 落系统钥匙串非明文;MCP 面板可复制各客户端配置。

### 批次 8 ｜ `feat/p1-realtime-playback`（真实播放引擎，工程量最大，放最后）
- P1-10 ffmpeg 连续解码 + wgpu 上屏 + cpal 音频 + A/V 同步 + 精确 seek + 预览降档
- **验收**:Timeline 合成预览可连续播放、音画同步、scrub 流畅。
- 风险高(ROADMAP 自评 🔴),**不阻塞前 7 批的可用性**。

### 批次 9 ｜ `feat/p2-samples-polish`（打磨）
- P2-4 Sample 区、P2-12 窗口尺寸、剩余 P2 项。

---

## 三、"务实复刻"取舍点（明确说明两条腿）

1. **预览先用 webview `<video>` 而非 wgpu 全合成。** 上游用 AVFoundation 让系统播,我们没有 AVFoundation。**素材预览(快、原生)**用 webview `<video>/<img>/<audio>` + `convertFileSrc` 直显原片(Tauri 自带 WebKit/WebView2 解码),零 Rust 改动即可满足"点素材右侧出现画面";**成片预览(wgpu 合成)**才走 RenderPlan + Compositor。要向用户说明这是两条腿——单素材原片预览 ≠ 带变换/调色/多轨叠加的合成预览。`<video>` 受 WebKit/WebView2 解码器限制(H.264/HEVC 通常 OK,ProRes/部分容器可能不行),不支持的格式回退到 wgpu 合成帧。

2. **合成预览先做"暂停态单帧",播放态降级,真实播放引擎最后做。** 暂停/seek 时 invoke `composite_frame` 贴 canvas 即可让用户看到合成效果;连续播放(A/V 同步/连续解码/音频)是 Phase 4 大头、风险最高,放批次 8,不阻塞可用性。

3. **窗口透明优先用 `titleBarStyle:Overlay` + 深色背景,不强求 `transparent:true`。** 真透明需 `macos-private-api` + vibrancy 才接近 `ultraThinMaterial`,成本高;Overlay + hiddenTitle + 与 `--bg-base` 同色背景已能消除"黑色标题栏 + 黑边"观感。

4. **"关窗回 Home"翻译为前端路由切换而非原生多窗。** 上游是两个 NSWindow(Home/Editor),OpenTake 是单 WebView——"回 Home"= `prevent_close()` + emit 事件让前端 `setView('home')`,不真正销毁窗。

5. **文件夹镜像导入做成一个原子步。** 上游用 `disableUndoRegistration` 包整批;OpenTake 让整个文件夹导入成单一可撤销/不可撤销步,避免 CreateFolder 在事务内、import 在事务外导致的半建状态。id 用 `core.ids.next_id()`,目录按 case-insensitive 名排序保证确定性。

6. **缩略图复用已实现的引擎 + DiskCache,只接线不重写。** `MediaEngine.image_thumbnail/video_thumbnails` 与 DiskCache(SHA256 key 与上游 `MediaVisualCache` 一致)已就绪,只需在 finalize 后台任务里接线 + 填 DTO 路径。

7. **账号/订阅/积分、Sample 后端、WelcomeOverlay/Tutorial、Models 目录** 属账号与引导体系,非窗口/保存/剪辑核心,全部后置(P2 或占位隐藏)。

8. **新建落盘与自动保存必须配套设计。** `project_dir=None` 时 `save_project(None)` 报 NoProjectOpen,所以 P0-1(新建即落盘)是 P1-1(自动保存)的硬前置,二者同期落地。

9. **全局可复用素材库(#37)是 OpenTake 新增子系统,不在本 1:1 差距清单约束内。** 上游 palmier-pro 无对应模块(grep 确认无全局收藏库)。采用 copy-on-favorite + SHA-256 内容寻址去重 + JSON manifest 原子写;后端存储层(#37-A / #104)+ Tauri 命令层(#37-B / #106)**已并入 main**,前端(#37-C / #56)待做。注意区分范畴:**#37 = 跨项目全局库;#49/#91 = 每项目媒体与文件夹浏览**,两者不同。