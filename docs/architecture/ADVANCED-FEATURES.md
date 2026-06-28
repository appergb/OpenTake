# OpenTake 进阶能力设计(对标剪映模块 1–5 的差距深化)

> 来源:`docs/CAPCUT-GAP.md`(5 个子 Agent 对照剪映模块 1–5 与上游源码的逐特性差距分析)。
> 范围:**不含**剪映模块 6(自然语言交互/语音助手 —— OpenTake 已有 Agent)与模块 7(云生态/企业协作)。
> 现状:33 项中 已有 2 / 部分 7 / 缺失 24。上游 Palmier Pro 自述「尚无:特效/转场/调色/蒙版/图形」,源码核对完全坐实。

## 0. 战略前提:两个既有系统承载全部缺口

OpenTake 补齐这些进阶能力,**几乎不需要新建基础设施**,全部是两个架构里已规划系统的扩展:

1. **wgpu 帧合成器(ARCHITECTURE §1 / ROADMAP Phase 3)** —— 原本是最大风险(命门),实为最大机会。
   特效 / 转场 / 滤镜 / 调色 / 绿幕抠图 / 蒙版 本质都是「在合成片元着色器里对像素做数学变换」。上游被 AVFoundation 声明式合成锁死、**做不了**这些;OpenTake 自写 wgpu 合成器**天生适合**,这是反超 Palmier Pro、逼近剪映的核心窗口。
2. **ort / candle 本地推理栈(已为 SigLIP2 语义搜索铺设,ARCHITECTURE §6)** —— AI 抠像 / 运动追踪 / 防抖 / 超分 / 消除 / 补帧 复用同一条 ONNX 推理通道(`opentake-media` 推理 worker),不必从零搭 ML 基础设施。

外加三条轻量补充层:**纯逻辑(domain/ops)**、**FFmpeg 音频滤镜**、**外部生成 API(opentake-gen / BYOK)**。

> 严守原则:所有新增能力都落到 `opentake-domain`(新增 Clip 字段)+ 对应执行层,并在 `opentake-agent` 暴露 MCP 工具,坚持「单一能力层、多前端」。所有 Clip 新字段一律 `#[serde(default)] + Option<T>`,保证读旧工程不破坏。

---

## A 层 · wgpu 着色器特性(依赖 Phase 3 合成器;OpenTake 反超窗口)

| 能力 | 现状 | 难度 | 优先级 | 方案要点 |
|---|---|---|---|---|
| 通用特效/滤镜框架 | missing | medium | **p0** | `Clip.effects: Vec<Effect>` 链;每个 Effect = 一个 wgpu 片元 pass(参数可关键帧化)。是后续一切像素特效的地基 |
| 转场 transitions | missing | medium | **p0** | 新概念:作用于**两个相邻 clip 重叠区**的过渡 pass(dissolve/wipe/slide/zoom…);domain 新增 `Transition{kind,duration,params}` 挂在 clip 边界 |
| 高阶浮点调色引擎 | missing | high | **p0** | 合成片元着色器内的**线性光调色链**;顺序锁定:BT.709→线性 → 曝光/白平衡 → Lift/Gamma/Gain → 曲线 → HSL → LUT → 线性→BT.709。是下面 5 项调色的共同底座 |
| 色轮 Lift/Gamma/Gain | missing | medium | p1 | 调色链一段,纯着色器数学 |
| RGB 多维曲线 | missing | medium | p1 | 曲线采样成 1D LUT 纹理喂着色器 |
| HSL 分区调色 | missing | medium | p1 | 按色相分区增益,着色器实现 |
| 3D LUT 导入(.cube) | missing | medium | p1 | 解析 `.cube` → 3D 纹理 → 三线性采样 |
| 一键色彩匹配 | missing | high | p2 | 本地直方图/统计(Reinhard 色彩迁移起步),自动统一色调曝光 |
| 绿幕色度抠图 chroma key | missing | low | p1 | 纯着色器算法(色度距离 + spill suppression + 边缘羽化),**无需模型** |
| 蒙版 masks(线性/圆形/钢笔) | missing | medium | p1 | domain 新增 `Mask{shape,points,feather,invert}`;着色器按 SDF/多边形生成 alpha;可关键帧化做动态蒙版 |
| 物理级 easing(贝塞尔/弹簧) | partial | low | **p0** | **纯 domain,不需 wgpu**:`Interpolation` 扩展为 `Linear/Hold/Smooth/Bezier(c1,c2)/Spring(params)`,复用现有 sample 框架。当前只有 linear/hold/smooth |

新增 domain 字段(`Clip` 上,均 `#[serde(default)]`):`effects: Vec<Effect>`、`masks: Vec<Mask>`、`chroma_key: Option<ChromaKey>`、`color_grade: Option<ColorGrade>`;`Interpolation` 枚举扩展。
新增命令:`SetEffects` / `SetTransition` / `SetMask` / `SetChromaKey` / `SetColorGrade`(各自走 `withTimelineSwap` 事务 + UndoStack)。
新增 MCP 工具:`apply_effect` / `set_transition` / `set_mask` / `chroma_key` / `set_color_grade` / `apply_lut` / `match_color`。

---

## B 层 · ort/candle 本地 AI 推理(复用 SigLIP2 推理通道,`opentake-media` worker)

| 能力 | 现状 | 难度 | 优先级 | 方案要点 |
|---|---|---|---|---|
| 画质超清修复 super-res | partial(上游仅云端放大) | high | p1 | ort/candle 跑 Real-ESRGAN / SeedVR 本地;或保留 BYOK 云端双轨 |
| AI 智能抠像 matting | missing | high | p2 | ort 跑 RVM / BiRefNet(逐帧 alpha)→ 喂合成器当 mask;无需纯色背景 |
| 视频防抖 stabilization | missing | medium | p2 | 先 **FFmpeg vidstab**(零模型,快速可用);进阶用光流/特征 |
| AI 运动追踪 motion tracking | missing | high | p2 | ort 跑 CoTracker/点追踪 → 输出轨迹关键帧,驱动 transform/mask 自适应形变 |
| 光流法智能补帧 | missing | blocker | p3 | ort/candle 跑 RIFE/FILM 做帧间插值;退化方案 Farneback 光流 warp 或帧混合。与曲线变速配合做丝滑慢动作 |
| AI 智能消除瑕疵 | missing | high | p3 | 视频 inpainting(ProPainter 等)本地重,初期建议走外部 API |

统一形态:`opentake-media` 提供 ONNX 推理 worker(GPU 加速,wgpu/CUDA/CoreML EP 按平台);AI 特性产出(alpha/轨迹/插帧/超分帧)交给 wgpu 合成器或写回媒体缓存(content-hash)。每项配 MCP 工具(`ai_matte` / `track_motion` / `stabilize` / `upscale`(已有)/ `interpolate_frames` / `remove_object`)。

---

## C 层 · FFmpeg 音频工程(`opentake-media` 音频)

> 上游音频只有 per-clip dB 增益 + 关键帧 + fade(「施加层」已在),缺的是「分析/处理层」。

| 能力 | 现状 | 难度 | 优先级 | 方案要点 |
|---|---|---|---|---|
| 音频响度统一(LUFS/EBU R128) | missing | medium | p1 | FFmpeg `loudnorm` 两遍法;高性价比纯滤镜补齐 |
| 智能降噪 | missing | medium | p1 | FFmpeg `afftdn`/`arnndn` 先行;深度降噪(RNNoise/DeepFilterNet via ort)p2 |
| 人声分离 / 提取人声 | missing | high | p2 | Demucs/Spleeter via ort 或外部 API;工程量最大 |

---

## D 层 · 纯逻辑(`opentake-domain` / `opentake-ops`,跨平台 Rust,工程独立)

| 能力 | 现状 | 难度 | 优先级 | 方案要点 |
|---|---|---|---|---|
| 50 条多轨道性能 | partial(模型已覆盖,性能未验证) | high | **p0** | wgpu 工程化:仅合成「可见、未被完全遮挡、opacity>0」轨;ffmpeg 解码器池 + 帧缓存;预览降档 + scrub 丢帧;RenderPlan 预计算 + 单 pass 批量混合 |
| 高阶曲线变速 speed curve | missing | high | p1 | speed 标量 → `speed_curve: Option<KeyframeTrack<f64>>`;对速度曲线**累积积分**得「时间线帧→源帧」单调映射;render 走 setpts 非线性表达式/逐帧 PTS;split/ripple 钳制按曲线重做 |
| 复合片段嵌套 nested clip | missing | high | p2 | **过渡方案先做**:`saveTimelineRange` 用 FFmpeg/wgpu 重写为「打组烧成内部媒体 + content-hash 缓存」满足「精简图层」;**完整方案后做**:domain 新增 `MediaSource::Nested(child_timeline_id)`,RenderPlan 递归展开或子序列离屏渲染成单层 |
| 多机位自动对齐 multicam | missing | medium | p2 | 纯本地:各机位音轨 → PCM → rustfft **互相关**求最佳时移 → ops 整体平移到同一时基;多角度切换面板作后续 UI |
| 字幕样式全局批量同步 | partial(共享样式在,批量算子缺) | low | p1 | 新增「改一处 → 批量回写整 captionGroup」编辑命令 |
| 导出 .srt 字幕文件 | missing | low | p1 | 从 caption 模型按时码序列化 SubRip;顺带支持 .vtt |

---

## E 层 · 外部生成(`opentake-gen` / BYOK / `opentake-gen-proxy`)

> 这些本质依赖外部生成大模型,本地只承担编排/转写/静音检测/素材物化/落轨。

| 能力 | 现状 | 难度 | 优先级 | 方案要点 |
|---|---|---|---|---|
| 智能剪口播(剔除停顿/语气词) | partial(已规划) | medium | p1 | **本地为主**:词级 `get_transcript` + 静音检测 → Rust 内一次算好 ripple 区间(高阶工具 `remove_filler_words`/`tighten_silences`,避免把帧算术外包给 LLM) |
| 图文成片 script-to-video | partial(地基在) | high | p1 | agent 编排既有工具:脚本 → `generate_image`→`generate_video`→`generate_audio`(配音)→`add_clips`/`add_texts`/`set_transition`;素材匹配用 SigLIP2 搜索 + import_media 接 stock |
| 音色克隆 voice cloning | missing | high | p2 | 外部 API(ElevenLabs 等)经 opentake-gen,扩展 audio 生成参数支持自定义音色 |
| 虚拟数字人 digital avatar | missing | high | p3 | 外部 API(HeyGen/fal 等)经 opentake-gen,新增 catalog kind |
| 多语种翻译(字幕) | partial(靠 agent) | medium | p2 | 一等公民:离线 MT 或外部 API + LLM 兜底,翻译后保持时码 |

---

## 优先级汇总(建议纳入路线图的顺序)

**P0(随合成器一并打通,是其余能力的地基)**
- 物理级 easing(纯 domain,可在 Phase 1 就做)
- 通用 effects/filters 框架 + 转场框架(Phase 3 合成器之上)
- 浮点调色引擎(Phase 3 之后、Phase 5 导出之前的「色彩科学阶段」)
- 50 轨性能工程化(Phase 3/4 内)

**P1(高性价比 / 中等)**:绿幕抠图、蒙版、色轮/曲线/HSL/3D LUT、超分、响度统一、降噪、SRT 导出、字幕批量样式、曲线变速、剪口播、图文成片

**P2**:AI 抠像、运动追踪、防抖、色彩匹配、人声分离、复合片段嵌套、多机位对齐、音色克隆、多语种翻译、AI 消除瑕疵

**P3**:光流补帧、虚拟数字人

## 对路线图的影响(新增/插入阶段)

- **Phase 1** 顺带:`Interpolation` 扩展(bezier/spring)。
- **Phase 3.5(新)· 着色器特性框架**:effects 链 + 转场框架 + 调色链 + 绿幕 + 蒙版(全部 wgpu pass,合成器 PoC 通过后即铺)。
- **Phase 8 扩展 · AI 推理特性**:抠像/追踪/防抖/超分/补帧/消除,统一 ort worker。
- **Phase 8 扩展 · 音频工程**:loudnorm/降噪/人声分离 + SRT 导出。
- **Phase 9 扩展 · AIGC 编排**:图文成片、剪口播高阶工具、音色克隆、数字人、字幕翻译。
