# OpenTake 分阶段实施路线图

> 原则:先把「纯逻辑层」做扎实并与上游对拍,再攻媒体引擎 blocker,最后接 UI / Agent / 生成后端。
> 每个阶段都有明确「交付物」与「验证标准」。详见 `docs/ARCHITECTURE.md`、`docs/MODULE-PORT-MAP.md`。
> 对标剪映的进阶能力(特效/转场/调色/蒙版/AI/音频工程等)穿插在下列阶段中,完整规划见 `docs/ADVANCED-FEATURES.md`。

## Phase 0 — 工程脚手架
- **做**:Cargo workspace(空 crates 骨架)+ Tauri 2 app + React/Vite 前端 + CI(fmt/clippy/test/前端 build)+ GPL-3.0 LICENSE + README(标明「基于 Palmier Pro 的社区分支」)。
- **验证**:`cargo build` 全绿;`pnpm build` 通过;Tauri 空窗口能起;CI 跑通。

## Phase 1 — 领域模型 + 编辑算法 + 命令层(纯 Rust,无 IO)🟢 最高优先
对应 `opentake-domain` + `opentake-ops`。这是「忠实复刻编辑逻辑」的核心,且最易做、可全单测。
- **做**:
  1. 移植 `Models/*` → `opentake-domain`:Timeline/Track/Clip/Keyframe/Transform/Crop/TextStyle + 全部派生函数与采样算法。
  2. 移植 `OverwriteEngine`/`RippleEngine`/`SnapEngine` → `opentake-ops`(纯函数)。
  3. 实现 `EditCommand::apply` + `UndoStack`(整树快照)= 上游 `withTimelineSwap` 事务模式。
  4. 移植 split / link / ripple-delete 的精确语义(含 ripple 拒绝语义、关键帧边界插入)。
- **验证**:针对每个算法写单测;**对拍测试**——用上游导出的 `project.json` 喂入 OpenTake,执行等价命令序列,断言结果 timeline 与上游一致(帧级)。覆盖率 ≥80%。

## Phase 2 — 持久化 + 媒体导入 + 缩略图/波形 🟢
对应 `opentake-project` + `opentake-media`(易部分)。
- **做**:`.opentake` 目录包读写(serde + `#[serde(default)]` 容错,与上游 schema 对齐);媒体导入(本地/URL/bytes,扩展名白名单);ffmpeg 抽缩略图 + sprite 网格缓存(逻辑照搬 `MediaVisualCache`);Symphonia 出波形(归一化 0..1 缓存)。
- **验证**:能打开上游导出的工程文件并还原 timeline;导入媒体后缩略图/波形/时长/分辨率正确;`.opentake` 往返序列化无损。

## Phase 3 — 🔴 wgpu 帧合成器 PoC(项目命门,尽早验证)
对应 `opentake-render`。**这一步成败决定整条 Rust core 路线。**
- **做**:
  1. 纯函数 `Timeline → RenderPlan`(每帧/每段属性值,移植 `trackOps`/`emitTransform`,smooth 段 8 分细分、归一化坐标→像素仿射、BT.709)。
  2. wgpu render graph 逐帧执行:解码源帧(ffmpeg)→ 纹理 → 采样 RenderPlan → 仿射/裁剪/opacity/多轨混合 → 输出帧。
  3. PoC 场景:**单轨视频 + 一个 transform 关键帧 + 一条字幕**,渲染指定帧。
- **验证**:**导出帧与上游 `inspect_timeline` 同帧做像素 diff**,误差在阈值内。PoC 通过即确认路线可行,再铺开。

## Phase 3.5 —(新)着色器特性框架 · OpenTake 反超窗口
合成器 PoC 通过后即铺,是对标剪映「特效/转场/调色/绿幕/蒙版」的统一承载层(上游做不到)。详见 [ADVANCED-FEATURES.md](ADVANCED-FEATURES.md) A 层。
- **做**:`Clip.effects` 着色器特效链框架 + 转场框架(相邻 clip 重叠区 pass)+ 浮点调色链(线性光:曝光/白平衡/Lift-Gamma-Gain/曲线/HSL/3D LUT)+ 绿幕色度抠图 + 蒙版(线性/圆形/钢笔,可关键帧);domain 加对应字段(`#[serde(default)]`),command 加 `SetEffects`/`SetTransition`/`SetColorGrade`/`SetChromaKey`/`SetMask`。
- **验证**:每类 pass 单独像素回归;调色链顺序正确(线性光空间);旧工程仍可读。

## Phase 4 — 🔴 播放/预览引擎
对应 `opentake-render`(播放部分)。
- **做**:ffmpeg 解码 + wgpu 上屏 + cpal 音频;自写 A/V 同步、精确 seek(解到最近关键帧+丢帧)、scrub 30Hz 节流(移植 `VideoEngine` 节流器);预览分辨率降档保帧率。
- **验证**:连续播放音画同步;scrub 流畅不卡;seek 到任意帧画面正确。

## Phase 5 — 导出
- **做**:wgpu 逐帧合成 → ffmpeg 编码;复刻预设(H.264/H.265/ProRes × 三档分辨率),逐项对齐码率/profile/色彩逼近上游;`renderSize` 取偶数。
- **验证**:导出 mp4 可播放;与上游导出对比画质/时长/音画同步一致。

## Phase 6 — React 前端
对应 ui-rebuild 模块(Timeline/MediaPanel/Inspector/Settings/Toolbar/Help/UI)。
- **做**:TimelineView(轨道/clip/playhead/拖拽,像素↔帧换算放前端)、Preview(canvas 显示 Rust 合成帧 + DOM 叠字)、Inspector、MediaPanel、设计系统(对应上游 AppTheme)。状态用 Zustand 持只读镜像 + UI-only 态。
- **验证**:能完成「导入→拖到时间线→裁剪/移动/分割→预览→导出」完整闭环;响应式无溢出;键盘可达。

## Phase 7 — MCP server + 应用内 Agent + 工具层
对应 `opentake-agent`。
- **做**:
  1. `rmcp` 起 MCP server(loopback + Origin 校验);31 工具薄包装到 `EditorCore`(描述字符串原样照搬)。
  2. 短 ID 系统 + 统一执行壳 + 精确路径错误(`serde_path_to_error`)。
  3. 应用内 chat(reqwest→Anthropic SSE,BYOK;prompt caching)。
  4. **OpenTake 增强**:分层可组合系统提示词 + 模型策略配置化;高阶工具 `remove_filler_words`/`tighten_silences`;写工具返回结构化 JSON;新增 `get_capabilities`。
- **验证**:`claude mcp add` 能连;每个工具走通;应用内 chat 能完成多步链式编辑;助手专属 undo 正确。

## Phase 8 — 文字/字幕渲染 + 转写 + 语义搜索
- **做**:cosmic-text + tiny-skia/Vello 文字渲染(阴影/描边/背景/对齐/换行,逐帧 opacity)接入合成器;whisper-rs 转写(word/segment 时间戳,`TranscriptionResult` 模型复用);candle/ort 跑 SigLIP2 + tokenizers 做视觉/口语搜索。
- **验证**:字幕静态渲染像素对齐上游;转写时间码映射正确;`search_media` 视觉/口语命中合理。
- **进阶扩展(ADVANCED-FEATURES B/C/D 层)**:
  - AI 推理特性(统一 ort worker):超分(Real-ESRGAN/SeedVR)、AI 抠像(RVM/BiRefNet)、运动追踪(CoTracker)、防抖(FFmpeg vidstab 起步)、光流补帧(RIFE/FILM,p3)、消除瑕疵(p3)。
  - 音频工程(FFmpeg):响度统一(loudnorm/EBU R128)、降噪(afftdn/arnndn→DeepFilterNet)、人声分离(Demucs via ort)。
  - 纯逻辑:导出 `.srt`/`.vtt`、字幕样式全局批量同步、曲线变速(speed 升级为关键帧轨)、多机位自动对齐(rustfft 互相关)、复合片段嵌套。

## Phase 9 — 生成式 AI 后端
对应 `opentake-gen` + `services/opentake-gen-proxy`。
- **做**:`GenClient`(复刻 `GenerationParams` 联合类型 + job 状态机);**BYOK 模式**(本地直连 fal/Replicate/OpenAI,keyring 存 key,内置静态 models catalog);**托管模式**(axum 代理 + provider adapters + 对象存储预签名 + 可选积分计费)。
- **验证**:BYOK 下能用自己的 fal key 生图/生视频并落回时间线;模型目录数据驱动 UI;托管代理可自部署。
- **进阶扩展 · AIGC 编排(ADVANCED-FEATURES E 层)**:智能剪口播(本地词级转写+静音检测→Rust 内算 ripple,高阶工具 `remove_filler_words`/`tighten_silences`)、图文成片(agent 编排既有工具+SigLIP2 选素材)、音色克隆(ElevenLabs 等)、虚拟数字人(HeyGen/fal,新增 catalog kind)、多语种字幕翻译(MT/LLM,保时码)。

## Phase 10 —(新)Web 动效模块与插件系统
对应新 crate `opentake-motion`。详见 [MOTION-GRAPHICS-PLUGIN.md](MOTION-GRAPHICS-PLUGIN.md)。
- **做**:Agent 用 HTML/CSS/JS(类 Emotion)编写动效 → 无头 Chromium + CDP 虚拟时间**确定性逐帧渲染**(带 alpha)→ content-hash 缓存 → 作为一层喂 wgpu 合成器;插件化的动效模板(manifest + 参数 schema)+ 渲染沙箱(禁网/CSP/超时);MCP 工具 `add_motion_graphic`/`edit_motion_graphic`。
- **验证**:同一动效代码每帧可复现、预览=导出;透明叠加正确;沙箱不越权;模板参数化实例化可用。
- **时机**:后期能力,依赖 wgpu 合成器(Phase 3)与媒体物化链路就绪,不阻塞核心管线。

---

## 关键里程碑顺序与理由
1. **Phase 1 先行**:纯逻辑、可对拍、零外部依赖,是所有上层的地基,且能立刻验证「忠实复刻」承诺。
2. **Phase 3(wgpu PoC)尽早**:它是唯一无现成替代的 blocker,越早验证越早排除最大风险。建议 Phase 1 完成后即插入 PoC,不必等 Phase 2 全做完。
3. **UI / Agent / 生成**在引擎可用后并行推进。

## 风险登记
| 风险 | 等级 | 缓解 |
|---|---|---|
| wgpu 合成器无法对齐 AVFoundation 输出 | 🔴 高 | Phase 3 PoC 像素 diff 早验证;先做最小场景 |
| 播放引擎 A/V 同步/seek 工程量超预期 | 🔴 高 | 复用合成器输出;预览降档;参考 gstreamer 经验 |
| 导出预设与上游画质不一致 | 🟠 中 | 逐预设调参对比;非阻塞核心功能 |
| 字幕渲染像素级对齐难 | 🟠 中 | 先静态对齐再接关键帧;可接受小阈值 |
| FFmpeg(L)GPL 与分发合规 | 🟢 低 | GPL-3.0 项目兼容;动态链接/标注来源 |
