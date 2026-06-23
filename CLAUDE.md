# OpenTake — 工作交接 / 状态文档（给压缩上下文后的自己）

> 本文件是 OpenTake 开发的**权威状态 + 操作手册**。每次上下文压缩后先读它,再读 `docs/PORT-1TO1-GAP.md`(1:1 差距与实现计划,批次蓝图)。
> 用户用中文沟通,回复用中文。用户要我**全自主**:自己开子 Agent / workflow,**绝不让用户开 agent / 不向用户提问要他操作**;**自己用真机 computer-use 测试**,做到能用再回报。

> ⚠️ computer-use 点击本机被 Dock 遮挡全局拦截(报"会落在程序坞")。改用 `preview_start` dev server(浏览器 fallback `web/src/lib/fallback.ts` 有 demo 时间线)+ `preview_eval` 注入测量验证布局,绕开真机点击。详见 `memory/opentake-editing-parity.md`。

## ✅ 2026-06-23 第三轮已合并(自动 PR 审核,均 CI 双绿 + 对抗验证 CONFIRM)

逐 PR 专家审核 + 对抗验证 + 对照文档后合并的纯新增项(详见 CHANGELOG「第三轮」):
- **#104/#106 全局素材库后端 + 命令层**(#37-A/B):`opentake-media/src/library.rs`(内容寻址去重 + JSON manifest 原子写)+ `src-tauri/src/library.rs`(7 命令)。前端 #37-C/#56 待做。
- **#107 文本工具 MVP**(#96):Toolbar `T` → `addTextClip()` + `TextTab.tsx`;textStyle 字段留后续(依赖后端 ClipProperties 扩展)。
- **#110 SRT/VTT 字幕导出纯逻辑**(#29 切片):`opentake-domain/src/subtitle_export.rs`(16 单测);导出层/agent 工具/前端对话框待接。
- **#111 `list_models` 接 opentake-gen catalog**(#9/#10 切片);`generate_*`/upscale 仍待 async+BYOK。
- **#112 整条时间线视频导出编排**(Phase 5 spine):`src-tauri/src/export.rs` + `export_video`(逐帧 composite→ffmpeg,H.264/.mp4 全分辨率,含集成测试);自包含复制 preview、未碰 `composite_frame`。H.265/ProRes/音频/进度取消留后续。
- **关闭** #76(bundle id 改名冗余);**请修改 @作者** #77/#78/#79/#105/#108(冲突/验收未达标,详见 CHANGELOG「待审 PR」)。

## ✅ 2026-06-23 第二轮已完成(分支 `feat/jianying-ui-and-timeline-fixes`,基于 PR#81)

多 Agent 模式:本人修 Bug + 小改;编排 workflow 做大功能。**全部本地验证通过、已提交、尚未推送/未真机目视确认(Dock 拦截)。**

- **B1 暂停**(`64ab37f`):`TimelinePlaybackLayer` ref-detach 路径先 `pause()` 再 `delete`——React 卸载时 ref detach 先于 effect cleanup(旧 cleanup 拿到空 Map),且 DOM 移除的媒体元素不自停。
- **B2 波形**(`c17dc04`):`crates/opentake-media/src/waveform/mod.rs` 改用 ffmpeg `extract_pcm`(原 Symphonia 解不了 .mov/非 AAC),移除 symphonia 依赖 + 前后端失败日志 + mp4 视频容器波形集成测试。
- **B3 重链接**(`183e270`):新增 `relink_media` 命令 + `EditorSession/AppCore::relink_media_file`(保持同 id、只改 source、类型须匹配,`CoreError::Media`);`MediaItemDto.missing` 按文件存在性实时派生;clip 红 wash + 卡片离线覆盖层 + "重新链接"。3+1 测试。
- **F1 触控板**(`f06c71f`)+ **剪映手势对齐**(`0892c0b`):`TimelineContainer` 用**原生 `{passive:false}` wheel 监听**(React onWheel 是 passive,preventDefault 无效→捏合会变页面缩放)。1:1 对齐剪映:Cmd/Ctrl/捏合=缩放、Option=横滚、裸滚/双指=平移;⌘±=放大缩小、⇧Z=适配窗口(`useKeyboardShortcuts`)。
- **F2**(`dedfdbb`):删 TitleBar "切换 Agent 面板"按钮(面板仍经 ViewMenu/快捷键)。
- **F3 剪映式顶栏**(`b89fc56`):8 主标签(素材/音频可用,文本/贴纸/特效/转场/字幕/智能包裹置灰);素材/音频→导入/我的;星标收藏 localStorage(`favorites.ts`);保留 missing/relink。`MediaTabBar.tsx`+`favorites.ts`。
- **F4 时间线导出**(`b89fc56`):`crates/opentake-project/src/fcpxml.rs` 导出 **XMEML 4(FCP7 XML)**——1:1 端口上游 `Export/XMLExporter.swift`(Premiere 不读 FCPXML,故选 XMEML);`export_fcpxml` 命令+api+TitleBar 导出按钮(saveDialog .xml);`Clip::keyframe_frames`(带测试)。13 fcpxml 测试。

**遗留/后续:** ① 推送 + 真机目视确认(B1暂停/B2波形需原生构建,Dock 拦截 computer-use)。② `fcpxml.rs` 1489 行 > 800 规约,建议拆分。③ `export_fcpxml` 名实不符(产物是 XMEML),可考虑改名 `export_xml`。④ Q/W(现=修剪入出点,Premiere习惯)与 ⌘K 分割 跟剪映(Q/W=删左右、⌘B/B切割)不同,待用户定夺。⑤ 星标"我的"前端仍为 localStorage(`favorites.ts`);全局库**后端已并入 main**(#37-A 存储层 #104 + #37-B 命令层 #106),**前端 #37-C/#56 未做**——收藏尚未迁到后端 `library_favorite`/`library_list`。⑥ 剪映 draft(`com.lveditor.draft`)导出未做(格式易碎,留后续)。

## 历史(第一轮,分支 `feat/realtime-timeline-playback`/PR #81,CI 绿)
真实播放、波形端口、trim/move 正确性、fade smoothstep、razor 吸附、轨道编号、⇧⌫ ripple、预览左下角根因修复、split 无选区、链接 co-trim。**PR #81 已超"播放"范围,合并时改标题/拆分。**

## 0. 项目身份
- OpenTake = [palmier-io/palmier-pro](https://github.com/palmier-io/palmier-pro)(Swift 原生 macOS 视频编辑器,GPL-3.0)的**跨平台社区分支**。**忠实 1:1 复刻其编辑逻辑与 UI**(用户反复强调:别自己发明,照上游源码复刻;除登录/账号外都对齐),再加更强 Agent 能力。
- 栈:Tauri 2 + Rust workspace + React/TS;媒体 = FFmpeg(sidecar)+ wgpu(合成)+ cpal + whisper-rs + candle/ort。
- GitHub `appergb/OpenTake`(Public,main 受保护)。许可 GPL-3.0。

## 1. 目录
- 仓库根 `OpenTake/`(在此跑 git/cargo/pnpm)。上游只读参考:`../palmier-pro-upstream/Sources/PalmierPro/`(**1:1 复刻唯一权威来源**)。
- 计划/设计:`docs/PORT-1TO1-GAP.md`(必读)、`docs/`(ARCHITECTURE/ROADMAP/specs/*)。

## 2. ✅ 已完成并入 main(CI 全绿)——MVP 编辑闭环已能用
- 14 模块(domain/ops/project/render/media/agent/gen/core/前端#12/进阶A#13/motion#14)+ PR #41–#46。
- **真机实测通过**:启动**无黑边**(透明统一标题栏)→ 新建项目**弹保存对话框选位置落盘**(`~/Documents/OpenTake`)→ **导入素材(20+)有真实缩略图** → **点素材右侧预览(视频/图/音,App 控制条驱动)** → **双击/拖拽加入时间线生成片段**(已见 id-10 片段)。**关窗不退后台常驻 + Dock 重开**。多语言(语言包,中/英,自绘下拉)。BYOK 密钥存系统钥匙串(#43)。应用图标已换成用户提供的。
- 关键实现:预览/缩略图走 **Tauri asset 协议 + `convertFileSrc`** 让 WebView 解码本地原文件(`web/src/lib/asset.ts`);拖到空时间线**按需建轨**(`EditCommand::InsertTrack`);窗口 `titleBarStyle:Overlay`+`dragDropEnabled:false`。

## 3. ⚠️ 工程纪律(踩过坑,务必遵守)
- **合并前 `gh run watch <id> --exit-status` 确认两作业(Rust+Web)都绿**;`gh pr merge <n> --merge --admin --delete-branch`。
- 写代码必跑 `cargo fmt --all`(CI 第一步 fmt --check);CI 已装 ffmpeg+GTK。macOS 无 `timeout`。
- **`RunEvent::Reopen` 仅 macOS**(已 `#[cfg(target_os="macos")]` 门控);本地 macOS clippy 过不代表 Linux 过,平台相关代码要 cfg。
- **真机测试循环**:`./web/node_modules/.bin/tauri build` → `cp -R target/release/bundle/macos/OpenTake.app /Applications/` → `open -a OpenTake` → computer-use(已授权 `com.opentake.app`,tier full)。dev 裸二进制识别不到,必须装到 /Applications。
- 同一工作树勿并行两个写同批文件的 workflow;workflow 可能撞 Cloudflare 522 让"写/审"步骤失败、审核被跳过 → 本人接手验证+自审+盯 CI。

## 4. 🟥 下一步(我认领的"第一个大开发":时间线)
**先做 #47 + #48(时间线合成预览/播放 + 片段编辑收尾)——这是用户点名的"时间线的工作还没做"。**
- **#47 时间线合成预览 + 播放**:src-tauri 新增 `composite_frame(frame)->RGBA/PNG`(RenderPlan.frame + opentake-media 实现 FrameProvider + 已就绪的 Compositor.render_to_rgba)→ 前端 Preview 在 Timeline 标签暂停/seek 时贴 `<canvas>`(替换 1920×1080 占位,现在时间线预览是黑的不播放);再做播放引擎(连续解码+cpal 音频+A/V 同步)。
- **#48 片段编辑收尾**:验证/修原生里时间线**片段点击选中**(`TimelineContainer` onPointerDown→hitTestClip→selectClips 已接,但实测 Delete 无效,疑选中没生效)→ Delete 删除、Cmd+K/剃刀分割可用;**片段右键菜单**(Copy/Swap Media/Save as Media/AI Edit);Inspector 三段式;Toolbar `[`/`]`/`T` 接线。
- **#38 自动保存**(背景已完成,剩防抖 save_project(None) + 退出前 flush)。

## 4b. ✅ 已完成(2026-06-22 续,均合并 main CI 双绿)
- **#51/#52 合成预览**(PR #59):时间线标签按播放头贴 GPU 合成帧(视频+图)。
- **#61/#62**:多素材拖入、保存/自动保存/退出 flush、预览整数帧、音频探测、播放卡顿缓解。
- **#36 MCP 工具派发层 + Skills**(PR #66/#67):单一能力派发(25 工具接线:18 EditCommand + rename/delete + workflow/Skills)+ 默认"音频先入"内置 Skill(`crates/opentake-agent/src/plugin/builtin/audio-first/`)。
- **#65 文字光栅化**(PR #68):`CosmicTextRasterizer`(cosmic-text+swash)把文字 clip 框渲染为预乘 RGBA,经既有 affine 1:1 合成置顶(对应上游 CATextLayer);字体/字号/颜色/对齐/背景/投影/边框全覆盖;真机视觉自检中英混排正常。**剩 Lottie 烘焙**。
- **#36 MCP server 网络面**(PR #69,**issue 已关闭**):rmcp Streamable-HTTP `127.0.0.1:19789/mcp` + 回环 Origin/Host 守卫 + OAuth well-known;src-tauri `mcp.rs` 在 setup spawn(会话共享的 AppCore 克隆 + 内置/用户 workflow registry)。HTTP 集成测试完成 `initialize` 握手 + 远程 Origin 403。`claude mcp add --transport http opentake http://127.0.0.1:19789/mcp` 可连。

## 5. 🟦 可认领/未完成(供同事,注意文件区避免冲突)
- **🔴 #53 [#47-C] 时间线播放引擎**(连续解码 + cpal 音频 + A/V 同步 + MJPEG 回环传输)。子项 #63(cpal)/#64(MJPEG 传输)/#65(Lottie 烘焙)。最大未完成项,需专门会话 + 真机视觉验证。
- **#48 片段编辑收尾**:Delete/切割/片段右键菜单/Inspector 三段式/Toolbar 接线。
- **剩余 MCP 工具 stub**:媒体读取(inspect_media/get_transcript/search_media)+ import_media 需**拓宽 CoreHandle 接 MediaEngine**(注意:CoreHandle 现仅持 AppCore,MediaEngine 在 MediaState,需架构扩展);`generate_*`/upscale 需异步 GenClient + BYOK;add_captions 需端上 whisper。
- **#49 项目内文件夹导入 + 嵌套文件夹浏览(剪映式)**:文件夹图标/双击进入/面包屑/拖出;DTO 加 folderId+folders;import_folder 镜像目录树。用户很想要。
- **#37 全局可复用素材库 + 收藏**(跨项目/分类/音效库/全库可见):**后端已并入 main** —— 存储层 `crates/opentake-media/src/library.rs`(#37-A/#54,PR #104,copy-on-favorite + SHA-256 内容寻址去重 + JSON manifest 原子写)+ Tauri 命令层 `src-tauri/src/library.rs`(#37-B/#55,PR #106,7 命令 list/favorite/unfavorite/categorize/rename/delete/import_to_project)。**剩前端 #37-C/#56**(库视图 UI + 收藏从 localStorage 迁到后端 `library_favorite`/`library_list`)尚无 PR。follow-up:`library.rs:322` remove() 静默吞 remove_file 错误,建议补 `tracing::warn!`;`library_delete` 与 `library_unfavorite` 现为纯别名,建议语义区分。
- **#39 提取音频星标 · #40 设置多分页+主页 1:1 · #34 motion dispatch · #27–30 进阶 B/C/D/E · #22–25 #12 follow-up · #35 bundle id 改名**。
- 冲突注意:我(#47/#48)动 opentake-render/opentake-media(decode/FrameProvider)/src-tauri(composite_frame、autosave)/web Preview+timeline;#36 动 agent+src-tauri(server 段);#37/#49 动 opentake-media(library/folders)+web media。**src-tauri/lib.rs、opentake-media 是多方交汇点,合并按 issue 顺序、各自小段、勤 rebase。**

## 6. MCP 配置(#36 落地后)
Streamable-HTTP `http://127.0.0.1:19789/mcp`(loopback+Origin 校验)。`claude mcp add --transport http opentake http://127.0.0.1:19789/mcp`;Cursor/Codex/Claude Desktop 同址。40 工具,返回附 context_signal。

## 7. 压缩后立即执行
1. 读本文件 + `docs/PORT-1TO1-GAP.md`。2. `git -C OpenTake pull`(main)。3. 盘点 `gh issue list`,挑最高价值且可完整交付的:**首选 🔴 #53 播放引擎**(大,需专门会话),或 #48 片段编辑收尾、#49/#37 库与文件夹、剩余 MCP 工具 stub。4. 每项走 分支→写→自审→`cargo fmt`+clippy+test→真机/确定性验证→`gh run watch` 双绿→`--admin` 合并。5. 新依赖先读 `~/.cargo/registry/src` 真实源码核实 API(cosmic-text/rmcp 都这么做的),别照猜测写。
