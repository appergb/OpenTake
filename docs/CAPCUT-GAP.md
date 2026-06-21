# OpenTake vs 剪映 特性差距报告(模块 1–5)

> 由 5 个 max-思考子 Agent 对照 OpenTake 设计稿 + 上游源码逐特性核对。状态:has=已覆盖 / partial=部分 / missing=缺失。已排除剪映模块 6(自然语言交互/语音助手,OpenTake 已有 Agent)与模块 7(云生态/企业协作)。

## 模块1:基础剪辑与时间线管理

**整体差距**:总体判定:6 项里只有「基础线性变速」是 OpenTake 设计稿已完整覆盖(has);「50 条多轨道复杂工程」属于"数据模型已覆盖、但大工程性能尚未验证"的 partial;其余 4 项(复合片段嵌套、多机位自动对齐、高阶曲线变速、光流补帧)在上游 Palmier Pro 源码与 OpenTake 三份设计文档里都完全没有踪迹,均为 missing,且恰好对应上游 FAQ 自述「尚无特效/转场/调色/蒙版/图形」之外的另一类"高阶时间线能力"空白。\n\n关键证据链:① 上游 Clip 模型(Models/Timeline.swift:82-85)速度是单个标量 `var speed: Double = 1.0`,而非关键帧轨;可关键帧的属性枚举 `AnimatableProperty`(Models/Keyframe.swift:77-78)只有 opacity/position/scale/rotation/crop/volume,根本没有 speed → 曲线变速在现模型里不可表达。② 插值类型 `Interpolation` 只有 linear/hold/smooth(Models/Keyframe.swift:3-5),没有任意贝塞尔曲线。③ 对全仓做穷举式 grep,multicam/nested/compound/optical-flow/interpolation/speedCurve/retime/frameblending 这些关键词除了 SF Symbol 图标名"film"和被排除的 compoundPredicate 外零命中;导出器甚至把 `frameblending=FALSE` 写死(Export/XMLExporter.swift:336)。④ 唯一与"嵌套"沾边的是 saveClipAsMedia/saveTimelineRangeAsMedia(Editor/ViewModel/EditorViewModel+SaveAsMedia.swift),那是用 AVMutableComposition 把片段/区间烘焙成一个新扁平媒体的破坏性 flatten,不是活的嵌套序列;OpenTake 设计稿(MODULE-PORT-MAP.md:233)也只把它当 FFmpeg flatten 处理。⑤ 轨道是无上限的 `tracks: [Track]` 数组,没有任何 maxTracks 闸门;多轨编辑逻辑(OverwriteEngine/RippleEngine/SnapEngine)已被 Phase 1 完整规划进 opentake-domain/opentake-ops,所以 50 轨的难点不在"功能缺失"而在 wgpu 合成器/预览引擎(架构文档点名的两大 blocker)能否扛住几十轨逐帧合成的性能。\n\n补齐落点优先级:p0 = 先保证 50 轨工程在 wgpu 合成器/预览引擎上的可用性能(否则其余高阶能力无处施展);p1 = 曲线变速(需把 speed 升级为关键帧轨 + 重做 setpts/RenderPlan 时间映射,工程量中等但牵动核心模型);p2 = 复合片段嵌套(架构性较大,建议先做"非破坏 flatten 缓存"过渡)、多机位自动对齐(可用 FFmpeg 音频互相关纯本地实现,工程独立);p3 = 光流补帧(唯一需要 ML 模型/外部 API、且对画质一致性要求最高的 blocker 级特性,放最后)。

### 高达 50 条多轨道复杂工程 — `partial` · 难度 high · 优先级 p0
- **判定依据**:上游轨道为无上限 `tracks: [Track]` 数组,全仓无 maxTracks/track limit 闸门;多轨编辑算法 OverwriteEngine/RippleEngine/SnapEngine 已是纯函数,OpenTake 已在 Phase 1 规划进 opentake-domain/opentake-ops(ROADMAP.md:10-17、MODULE-PORT-MAP.md:233、304)。但架构文档(ARCHITECTURE.md:22-27)明确把 wgpu 帧合成器与播放/预览引擎列为两大 🔴 blocker,且 PoC 场景只到『单轨视频+1个 transform 关键帧+1条字幕』(ROADMAP.md:29)——几十轨逐帧『解码→采样关键帧→仿射/裁剪/混合→多轨合成』的实时性能完全未验证。结论:数据模型与编辑逻辑层 has,但大工程的渲染/预览性能 partial。
- **落点(crate/层)**:opentake-render(wgpu 合成器 + 预览后端,性能关键)+ opentake-domain/opentake-ops(模型与算法,已覆盖)
- **实现方案**:模型与编辑无需新增,直接沿用既定移植。性能侧需在 wgpu 合成器做工程化:(1) 每帧只对『当前帧实际可见、未被上层完全遮挡且 opacity>0』的轨道解码合成,跳过空轨/全透明轨;(2) ffmpeg 解码器池 + 帧缓存,避免每轨重复 seek;(3) 预览分辨率降档 + scrub 丢帧到最新请求(架构已提该策略 ARCHITECTURE.md:130、MODULE-PORT-MAP.md:308);(4) RenderPlan 预计算每帧合成指令,wgpu 端用单 render pass 批量混合多层纹理。全部跨平台 Rust/wgpu,无需外部依赖。
- **前置依赖**:强依赖 wgpu 帧合成器(blocker 1)与播放/预览引擎(blocker 2)先落地;Phase 3/4 完成后才能压测 50 轨

### 新建复合片段(工程嵌套 / nested compound clip) — `missing` · 难度 high · 优先级 p2
- **判定依据**:上游领域模型只有扁平的 Timeline→Track→Clip,Clip 永远指向单个 media_ref(asset id),没有『Clip 引用一个子 Timeline』的类型(Models/Timeline.swift)。全仓 grep nested/compound 零命中(仅 SF Symbol 与被排除的 compoundPredicate)。唯一相邻能力是 saveClipAsMedia/saveTimelineRangeAsMedia(EditorViewModel+SaveAsMedia.swift:8/60),用 AVMutableComposition 把片段或时间线区间烘焙成一个新的扁平 mp4/m4a 媒体——这是破坏性 flatten,不可再编辑内部,且 OpenTake 设计稿(MODULE-PORT-MAP.md:233)也只把它当 FFmpeg flatten 移植。真正的『活的嵌套序列/精简图层』完全缺失。
- **落点(crate/层)**:opentake-domain(新增 nested 片段类型与子 Timeline 引用)+ opentake-ops(进入/退出嵌套、子树编辑命令)+ opentake-render(嵌套 RenderPlan 递归展开/物化)+ opentake-project(子序列序列化)
- **实现方案**:分两步落地,优先级有别。【过渡方案,先做】把上游 saveTimelineRangeAsMedia 的 flatten 用 FFmpeg/wgpu 重写为『一键打组烧成内部媒体』(content-hash 缓存,源区间改了才重烧),立即满足『精简图层』的视觉诉求,纯本地、工程小。【完整方案,后做】在 domain 新增 `MediaSource::Nested(child_timeline_id)` 或独立 `CompoundClip{child: Timeline}`,Clip 可引用子 Timeline;RenderPlan 生成时对嵌套节点递归展开成扁平合成指令(或先渲染子序列为离屏纹理再当单层合成,二选一按性能定);ops 增加 enter/exit 嵌套、子树内复用既有 OverwriteEngine/RippleEngine。全程跨平台 Rust/wgpu,无外部模型。
- **前置依赖**:完整方案依赖 wgpu 合成器支持离屏渲染/递归 RenderPlan;过渡 flatten 方案依赖导出/FFmpeg 链路可用即可

### 多机位自动对齐剪辑(multicam auto-sync) — `missing` · 难度 medium · 优先级 p2
- **判定依据**:上游与 OpenTake 三份设计文档全无 multicam/angle/sync-cam/多机位 任何痕迹(穷举 grep 仅命中无关的 sync-locked 轨道锁与 audio/video 链接 link_group)。上游的 A/V Link(同 link_group_id 作为一个单位移动,ARCHITECTURE.md:113)只是单机位音画捆绑,与多机位多角度对齐切换是两回事。该能力 0 覆盖。
- **落点(crate/层)**:opentake-media(音频指纹/互相关对齐算法)+ opentake-ops(对齐后生成多角度片段、批量位移到同一时基)+ opentake-domain(可选:角度组/multicam clip 概念)
- **实现方案**:对齐本身用纯本地 Rust/FFmpeg,不必调外部 API:(1) 对每条机位音轨用 Symphonia/FFmpeg 解 PCM → 降采样 → 计算包络或 MFCC;(2) 用 FFT 互相关(rustfft)求两两最佳时移 offset(峰值位置),得到各机位相对主轨的帧偏移;(3) 把偏移落到 ops:对每条机位 clip 整体平移到对齐时基,放到各自轨道。MVP 先做『自动对齐到同一时间线』(对应剪映核心价值),多角度实时切换面板可作为后续 UI 增强(切换时只改哪条轨可见/取片段区间)。无需 GPU、无需 ML 模型,工程相对独立。
- **前置依赖**:依赖 opentake-media 的 PCM 解码与波形链路(Phase 2 已规划);对齐算法本身不依赖 wgpu

### 基础线性变速 — `has` · 难度 low · 优先级 p1
- **判定依据**:上游完整实现且 OpenTake 已规划直译。Clip 有标量 `var speed: Double = 1.0`(Models/Timeline.swift:85);setClipSpeed(EditorViewModel+ClipMutations.swift:254-276):sourceFrames=basis.duration*basis.speed,newDuration=max(1,round(sourceFrames/newSpeed)),写 speed+duration 后 clampKeyframesToDuration/clampFadesToDuration,并对接触链 contiguousClipIds 整体平移 rippleDelta。渲染端 scaleTimeRange 实现变速(CompositionBuilder.swift:339)。OpenTake 已把 setClipSpeed/各 ripple 列为 direct-port 到 opentake-ops,变速重算 sourceFrames=max(1,round(dur*speed)) 明确照搬(MODULE-PORT-MAP.md:176、211、233)。速度下限 0.0001 也已记录(:109)。
- **落点(crate/层)**:opentake-ops(setClipSpeed 纯函数 + ripple)+ opentake-render(setpts/atempo 变速)——均已在既定移植计划内
- **实现方案**:已覆盖,无需补功能,仅需落地时注意两处取整/音频一致性:(1) round 方向与 max(1,_) 边界逐处对齐 Swift(MODULE-PORT-MAP.md:233 坑①);(2) 音频变速:上游 saveAsMedia 用 scaleTimeRange(会变调),但 OpenTake FFmpeg 移植计划写的是 atempo(保持音高,MODULE-PORT-MAP.md:233『setpts/atempo』)——这是与上游的有意分歧,需确认产品期望是『变调』还是『保调』并统一,导出与预览要一致。
- **前置依赖**:无新增;随 Phase 1(ops)+ Phase 5(导出)自然交付

### 高阶曲线变速(非线性速度控制 / speed curve) — `missing` · 难度 high · 优先级 p1
- **判定依据**:在现模型里根本不可表达:速度是单标量 `Double speed`(Models/Timeline.swift:85)而非 KeyframeTrack;可关键帧属性枚举 `AnimatableProperty` 只有 opacity/position/scale/rotation/crop/volume(Models/Keyframe.swift:77-78),没有 speed;插值类型只有 linear/hold/smooth(Models/Keyframe.swift:3-5),没有任意贝塞尔。导出器 timeRemapFilter 固定 variablespeed=0、frameblending=FALSE(XMLExporter.swift:330-337),即明确只支持恒定变速。OpenTake 文档亦无曲线变速任何提及。
- **落点(crate/层)**:opentake-domain(新增 speed 关键帧轨 / 时间重映射曲线)+ opentake-ops(变速曲线编辑 + 源帧↔时间线帧的非线性折算)+ opentake-render(RenderPlan 把曲线积分成逐帧 setpts/时间映射)
- **实现方案**:纯本地 Rust,不需外部模型。(1) domain:把速度从标量升级为可选 `speedCurve: KeyframeTrack<f64>`(或独立 time-remap 曲线,关键帧值=源时间或瞬时速度),复用现有 sample/smoothstep 框架,新增 bezier 段以支持平滑加减速;(2) 核心是时间映射:对速度曲线做累积积分得到『时间线帧→源帧』的单调映射函数,ops 据此重算 durationFrames 与 trim,split/ripple 钳制逻辑要按曲线重做;(3) render:导出走 FFmpeg setpts='非线性表达式' 或预采样成密集 sendcmd/逐帧 PTS(类比上游 smooth 段 8 等分细分思路 MODULE-PORT-MAP.md:347),预览在 wgpu 端按映射取源帧。难点在曲线积分与帧对齐的数值一致性。
- **前置依赖**:依赖基础线性变速链路先稳;依赖关键帧采样框架(已有);若要配合补帧出丝滑慢动作则进一步依赖光流补帧

### 光流法智能补帧(optical-flow frame interpolation) — `missing` · 难度 blocker · 优先级 p3
- **判定依据**:上游零实现:穷举 grep optical/interpolat(非关键帧插值)/RIFE/FILM/motion-interpolat 全部零命中;导出器把 frameblending 写死为 FALSE(XMLExporter.swift:336),连最基础的帧混合都不开。OpenTake 媒体引擎清单(ARCHITECTURE.md:123-137)涵盖解码/合成/字幕/转写/语义搜索,但完全没有补帧/插帧条目。该能力 0 覆盖,且是本模块技术门槛最高的一项。
- **落点(crate/层)**:opentake-media(补帧推理:本地 ort/candle 跑光流或学习式插帧模型;或 opentake-gen 走外部 API)+ opentake-render(变速/慢动作时按需调用补帧填充中间帧)
- **实现方案**:需要算法/模型,分两条路权衡:【本地优先】用 ort(onnxruntime,项目已用于 SigLIP2)或 candle 加载学习式插帧模型(RIFE/FILM 系)做两帧间插值,GPU 加速(wgpu/CUDA 视平台),完全离线、无云成本,契合 OpenTake 自托管定位;退一步可用传统光流(Farneback,OpenCV/纯 Rust 实现)做 warp,质量弱但零模型依赖。【外部 API 兜底】接 fal.ai/Replicate 的插帧端点(经既有 opentake-gen BYOK/代理),省去本地集成与算力,但要联网且按量计费。建议:慢动作变速场景默认本地补帧,质量不够再降级为帧混合/最近帧。这是 blocker 级:模型体积、跨平台 GPU 推理、与变速曲线的帧对齐都需打通。
- **前置依赖**:依赖 ort/candle 推理链路(语义搜索已铺,可复用)与 GPU 推理后端;强依赖高阶曲线变速一起用才有意义;模型权重分发与许可需先确认

## 模块2:视觉特效与空间重构

**整体差距**:整体判定:模块2 是 OpenTake 当前设计稿与剪映差距最大的一块,因为它几乎全部落在「像素级合成 + 神经网络推理」两个上游 Palmier Pro 主动放弃的领域。上游 FAQ 自述「尚无:特效/转场/调色/蒙版/图形」,源码核对完全坐实:(1) Clip 值类型只有仿射 Transform(centerX/Y、width/height、rotation、flip)+ 矩形 Crop(left/top/right/bottom 四边内缩),没有任何 effect/filter/mask/chroma/colorGrade/blend/lut 字段(Models/Timeline.swift:77-107、364-371);(2) 合成全部委托 AVVideoComposition 声明式 layer instruction,Preview 路径里 grep 不到任何 CIFilter/blendMode/compositingFilter(Preview/CompositionBuilder.swift);(3) 关键帧引擎虽完整(6 条轨 opacity/position/scale/rotation/crop/volume),但插值枚举只有 linear/hold/smooth 三种,smooth=smoothstep t²(3-2t),没有贝塞尔/弹簧/物理缓动(Models/Keyframe.swift:3-5、40、244-248);(4) 唯一的「AI 二次编辑」(upscale/edit/rerun/createVideo/generateMusic/SFX)全是云端 Convex 调用,本地零实现(EditAction.swift、ModelCatalog.swift、MODULE-PORT-MAP.md:231)。

十项里只有 1 项(关键帧引擎本体)算 has,1 项(超清修复)算 partial(上游有但纯云端、且仅放大不修复),其余 8 项全 missing。

但有两个关键利好:① OpenTake 的命门 wgpu 自写合成器(ARCHITECTURE §1、ROADMAP Phase 3)一旦落地,转场/特效/滤镜/调色/绿幕/蒙版这些「逐帧片元着色器」类能力就有了统一承载层,FFmpeg+wgpu 路线天然适合做这些——这恰恰是上游被 AVFoundation 锁死、做不了的地方,是 OpenTake 反超的最大机会窗口。② 上游已有两处「本地神经网络」先例——Apple Speech 端上转写(→whisper-rs)与 SigLIP2 视觉嵌入做语义搜索(Search/Models/VisualEmbedder.swift,经 CoreML/候 candle/ort),证明 OpenTake 设计稿的 candle/ort 推理栈已就位,抠像/追踪/防抖/超分/消除这些 AI 特性可复用同一条 ort/candle ONNX 推理通道,不必从零搭机器学习基础设施。

落地次序建议:先补 p0 的高级 easing(纯 Rust、零依赖、紧贴现有关键帧引擎)与通用 effects/transitions 框架(依赖 wgpu 合成器,是后续一切像素特效的地基);再补 p1 的绿幕抠图(wgpu 着色器,纯算法无需模型)、蒙版(几何+着色器)、超分(ort/candle 接 Real-ESRGAN/SeedVR 本地或保留云端 BYOK);AI 抠像/追踪/防抖/消除作为 p2/p3,统一走 ort 本地模型 + 可选外部 API 双轨。所有新增能力都应落到 opentake-domain(新增 Clip 字段:effects 链 / mask / chromaKey)+ opentake-render(wgpu 着色器与 RenderPlan 扩展)+ opentake-media(ort 推理 worker),并在 opentake-agent 暴露对应 MCP 工具,严守「单一能力层、多前端」原则。

### 关键帧动画引擎 + 物理级缓入缓出(高级 easing 曲线) — `partial` · 难度 low · 优先级 p0
- **判定依据**:关键帧引擎本体已被上游完整实现且 OpenTake 设计稿明确 1:1 复刻:6 条关键帧轨(opacity/position/scale/rotation/crop/volume,Models/Keyframe.swift:77、Models/Timeline.swift:102-107),KeyframeTrack.upsert/remove/move/sample、绝对帧↔相对偏移换算、分割时插边界帧保连续、clamp/rescale 全在(Keyframe.swift:18-37、231-249;MODULE-PORT-MAP.md:40-42、分割算法)。ARCHITECTURE §4/§6 与 ROADMAP Phase 1/Phase 3 把这些列为最高优先纯函数移植。但『物理级缓入缓出/高级 easing』缺失:Interpolation 枚举只有 linear/hold/smooth 三种(Keyframe.swift:3-5),smooth 即 smoothstep t²(3-2t)(Keyframe.swift:40),导出按 smoothSegments=8 段细分(MODULE-PORT-MAP.md:391)。没有三次贝塞尔(cubic-bezier)、没有弹簧/回弹(spring/overshoot)、没有 ease 预设库(easeInOutQuint/back/elastic/bounce 等剪映标配)。
- **落点(crate/层)**:opentake-domain(扩展 Interpolation 枚举 + 新增 easing 求值函数)+ opentake-render(RenderPlan 采样端按新曲线取值,8 段细分逻辑泛化)
- **实现方案**:纯 Rust、零外部依赖、风险极低。① 把 Interpolation 从 enum 升级为可携参数:新增 CubicBezier{x1,y1,x2,y2}(用牛顿迭代或二分解 t,对齐 CSS cubic-bezier 语义)、Spring{stiffness,damping,mass} 或更简单的 Penner easing 预设集(easeIn/Out/InOut × Quad/Cubic/Quart/Quint/Sine/Expo/Circ/Back/Elastic/Bounce)。② sample() 的分支从 3 路扩到 N 路,smoothstep 作为默认保持向后兼容。③ 序列化保持 #[serde(default)] 容错——旧工程的 linear/hold/smooth 原样读;新曲线作为新 variant 追加。④ wgpu 合成器侧只需把『每段按曲线求 t』替换原 smoothstep,8 段细分对贝塞尔可提到 16~32 段保平滑。剪映的『物理级』本质是弹簧/惯性预设,用 Penner elastic/back + 可调阻尼即可覆盖 95% 体感,不必引真实物理积分器。
- **前置依赖**:依赖现有关键帧引擎(Phase 1 已规划),不依赖 wgpu 合成器即可先在 domain 层完成并单测;最终视觉验证需 Phase 3 合成器

### 蒙版工具(线性 / 圆形 / 钢笔 mask) — `missing` · 难度 medium · 优先级 p1
- **判定依据**:上游完全没有蒙版能力。Clip 结构体无 mask 字段(Models/Timeline.swift:77-107);Crop 只是矩形四边内缩(left/top/right/bottom 归一化 0-1,Models/Timeline.swift、Keyframe.swift:65-74),不是任意形状遮罩,无羽化(feather)、无反转(invert)、无形状类型。源码里『mask』只出现在 SwiftUI 视图裁剪(GeneratingOverlay/TourOverlay)与 Search 文本 tokenizer 的 attention mask,与视频蒙版无关。AnimatableProperty 是封闭集 {opacity,position,scale,rotation,crop,volume}(Keyframe.swift:78),无 mask 项。
- **落点(crate/层)**:opentake-domain(Clip 新增 mask: Option<Mask>,Mask 枚举 Linear/Radial/Rectangle/Ellipse/Pen{points},含 feather/invert/可关键帧化)+ opentake-render(wgpu 片元着色器算 mask alpha)+ web(前端蒙版手柄/钢笔锚点绘制)
- **实现方案**:wgpu 片元着色器 + 纯几何,无需 AI。① 线性/圆形/矩形/椭圆蒙版:着色器内按当前像素 UV 到蒙版几何的有符号距离场(SDF)算 alpha,feather 用 smoothstep 在边界过渡带插值,invert 取 1-alpha,蒙版参数(中心/角度/半径/羽化)全部可挂关键帧(复用上面 easing 体系)。② 钢笔 mask:前端用贝塞尔锚点画闭合路径,Rust 端三角化(lyon crate)或在着色器里用 even-odd 环绕数判定内外 + 距离场做羽化。③ 蒙版作用对象 = 当前 clip 纹理的 alpha 通道,合成时按 z 序混合即可天然实现『局部显隐』。④ 与 Transform 解耦:蒙版坐标系绑定 clip 还是绑定画布需提供开关(剪映两种都有)。这是 wgpu 合成器最自然的扩展,上游因 AVFoundation layer 模型做不了,是 OpenTake 结构性优势点。
- **前置依赖**:强依赖 wgpu 帧合成器(ROADMAP Phase 3)先落地;前端钢笔交互依赖 Phase 6 React Preview;关键帧化依赖 easing 体系

### AI 运动追踪(自适应形变 motion tracking) — `missing` · 难度 high · 优先级 p2
- **判定依据**:上游零实现且零基础设施:全目录 grep 不到 import Vision / VNTrackObjectRequest / VNDetectTrajectories / opticalFlow / 任何 tracking 相关(『tracking』命中全是 AppTheme 字间距 letter-spacing 与 NSTrackingArea 鼠标区,见 UI/AppTheme.swift:200、Timeline/TimelineView.swift:894)。关键帧只能手 K,无『跟踪点驱动关键帧』通路。
- **落点(crate/层)**:opentake-media(新增追踪 worker:ort/candle 跑光流或点追踪模型,输出逐帧 2D 轨迹)→ opentake-domain(把轨迹烘焙成 position/scale/rotation 关键帧)+ opentake-render(贴合 clip/蒙版/文字到轨迹)
- **实现方案**:本地推理优先,双轨可选。① 平面/点追踪(剪映主力场景:贴文字/贴 logo 跟随):本地用 CoTracker / TAPIR(点追踪)或经典 Lucas-Kanade 光流(OpenCV-rust 或纯 candle 实现 KLT),输出目标点逐帧 (x,y[,scale,rotation]),再烘焙成现有 position/scale/rotation 关键帧轨——下游复用既有合成,改动面小。② 复用先例:上游已用 SigLIP2 经 ort/candle 做视觉嵌入(Search/Models/VisualEmbedder.swift、VisualModelLoader.swift),OpenTake 推理栈(candle/ort)已就位,追踪模型走同一条 ONNX/CoreML 通道,不必新搭 ML 基础设施。③ 『自适应形变』(网格变形跟随)较重,可二期:用 RAFT 稠密光流 + 薄板样条/网格 warp,在 wgpu 着色器里做形变采样。④ 外部 API 兜底:对追踪质量要求极高时可接 runwayml/第三方,但作为可选 BYOK,默认本地。
- **前置依赖**:依赖 candle/ort 推理栈(设计稿已含,Phase 8 语义搜索同栈);关键帧烘焙依赖 Phase 1 关键帧引擎;形变贴合依赖 wgpu 合成器

### 绿幕色度抠图(chroma key) — `missing` · 难度 low · 优先级 p1
- **判定依据**:上游零实现:Clip 无 chromaKey 字段(Models/Timeline.swift:77-107),Preview 合成路径 grep 不到 chroma/greenscreen/colorMatrix/任何抠像着色逻辑(Preview/ 目录无命中);AVVideoComposition 声明式合成不含像素级键控。AnimatableProperty 封闭集无相关项。
- **落点(crate/层)**:opentake-domain(Clip 新增 chroma_key: Option<ChromaKey{key_color,similarity,smoothness,spill}>)+ opentake-render(wgpu 片元着色器算 alpha)
- **实现方案**:纯 wgpu 着色器、本地算法、无需模型、难度低——这是 FFmpeg+wgpu 路线最划算的一项。① 着色器内把像素 RGB 转 YCbCr 或在 RGB 空间算与 key_color 的色度距离,distance<similarity 置透明,similarity..similarity+smoothness 用 smoothstep 做软边(羽化发丝边缘)。② 溢色抑制(spill suppression):对保留像素降绿通道饱和度。③ 参数(键色/容差/边缘/溢色)可挂关键帧应对光照变化。④ 也可先用 FFmpeg chromakey/colorkey 滤镜做导出兜底(MODULE-PORT-MAP.md:391 已列 colorchannelmixer 控 alpha 思路),但实时预览必须走 wgpu。⑤ 取色用前端吸管点画布像素回传 key_color。
- **前置依赖**:依赖 wgpu 帧合成器(Phase 3);取色交互依赖 Phase 6 前端 Preview;无 AI 依赖

### AI 智能抠像(无需纯色背景 / AI matting) — `missing` · 难度 high · 优先级 p2
- **判定依据**:上游零实现:无任何 matting/segmentation/CoreML 分割模型;唯一 AI 视觉能力是 SigLIP2 嵌入做检索(Search/Models/),非分割。云端 AI『edit』是整片文生重绘(≤10s,EditAction.swift:50-70),不是抠像。
- **落点(crate/层)**:opentake-media(matting worker:ort/candle 跑分割/抠像模型,输出逐帧 alpha matte)+ opentake-render(把 matte 作为 clip alpha 合成)+ opentake-domain(Clip 标记 ai_matte 资源引用,matte 作为派生 alpha 序列缓存)
- **实现方案**:本地推理优先,与 chroma key 共用合成下游(都是给 clip 一张 alpha)。① 视频抠像本地模型:RobustVideoMatting(RVM,轻量、时序稳定、有 ONNX 权重)是首选,经 ort 跑,逐帧出 alpha;人像场景可用 MODNet/BiRefNet,通用前景可用 SAM2(分割+追踪一体,但较重)。② 复用上游已验证的 ONNX 推理通道(ort/candle + ModelDownloader 先例,Search/Models/ModelDownloader.swift),模型按需下载缓存,与 SigLIP2/whisper 同套基础设施。③ alpha matte 用 content-hash 缓存(ARCHITECTURE §6 已有物化缓存策略),避免重复推理。④ 外部 API 兜底:质量优先可接 fal/Replicate 的 matting 端点,作为 BYOK 可选(沿用 opentake-gen 双模),默认本地零成本。⑤ matte 出来后下游与 chroma key 完全一致(clip alpha → 多轨混合),边缘可再过 wgpu 羽化/前景色净化。
- **前置依赖**:依赖 candle/ort 推理栈 + ModelDownloader(设计稿已含同类先例);依赖 wgpu 合成器消费 alpha;受益于追踪(SAM2 可一体化)

### 视频防抖(stabilization) — `missing` · 难度 medium · 优先级 p2
- **判定依据**:上游零实现:全目录 grep 不到 stabiliz/stabilis/VNDetectTrajectories/opticalFlow/任何防抖逻辑;无 Vision 框架引用。无运动估计基础设施。
- **落点(crate/层)**:opentake-media(防抖 worker:两遍分析——估计逐帧全局运动→平滑相机轨迹→反向补偿)+ opentake-domain(把补偿写成 position/scale/rotation 关键帧,或标记 stabilize 参数由 render 应用)
- **实现方案**:成熟开源算法可移植,本地、无需重型 AI、难度中。① 最省力路线:直接复用 FFmpeg vidstab(vidstabdetect + vidstabtransform 两遍),OpenTake 已绑 ffmpeg-next(ARCHITECTURE §10),导出路径几乎零成本接入;实时预览可先用降采样代理。② 自研路线(更可控):光流/特征点(KLT)估全局仿射运动 → 用低通/高斯平滑相机路径 → 反向仿射补偿 + 自动裁掉边缘黑边(轻微放大)。补偿量可烘焙进现有 transform 关键帧轨,复用既有合成,无需新合成原语。③ 平滑强度/裁切比例作为可调参数。④ 可选 AI 增强(深度学习防抖如 DUT)作为远期 p3,默认经典算法已够用。
- **前置依赖**:FFmpeg vidstab 路线仅依赖 ffmpeg-next(已有);自研路线依赖光流(与追踪共用);补偿应用依赖关键帧引擎/wgpu

### 画质超清修复(super-resolution / enhance) — `partial` · 难度 medium · 优先级 p1
- **判定依据**:上游有『Upscale』入口但纯云端、且仅放大不修复:AIEditTab『AI Enhance / Upscale / Enhance resolution with AI』(Inspector/AIEditTab.swift:31-36)、UpscaleModelConfig 从 Convex ModelCatalog 拉取(Generation/Catalog/UpscaleModelConfig.swift)、EditSubmitter.submitUpscale 把 sourceURL+durationSeconds 发后端(UpscaleModelConfig.swift:3-15)、需登录订阅否则禁用(ToolExecutor+Generate.swift:320)、video 仅 <2160 可放大(MODULE-PORT-MAP.md:528)。本地零推理。OpenTake 设计稿 Generation 章把它归为 cloud-rebuild,未规划本地超分实现(MODULE-PORT-MAP.md:231、ARCHITECTURE §8 只讲生成代理双模,未列本地 SR)。
- **落点(crate/层)**:opentake-gen(BYOK 云超分,复刻 UpscaleGenerationParams + job 状态机,设计稿已含)+ 新增 opentake-media 本地超分 worker(ort/candle 跑 SR 模型)作为 OpenTake 增强项
- **实现方案**:双轨。① 云端 BYOK(低成本起步、对齐上游):opentake-gen 已规划复刻 GenerationParams 联合类型与 job 抽象(ARCHITECTURE §8、ROADMAP Phase 9),upscale 作为其中一类,用户自带 fal/Replicate key 直连厂商超分模型(Topaz/SeedVR/Real-ESRGAN 端点),零运营成本,这是把上游云能力『去 Convex 化』的自然落点。② 本地推理(OpenTake 反超点,p2):经 ort 跑 Real-ESRGAN / SwinIR(图像)或 SeedVR2(视频,时序一致),复用 SigLIP2/RVM 同套 ONNX 通道与 ModelDownloader;『修复』(去噪/去压缩伪影/人脸增强 GFPGAN/CodeFormer)与『放大』分开提供,补上上游只放大不修复的短板。③ 处理结果作为新媒体资产回填时间线(沿用上游 upscale 回填语义)。
- **前置依赖**:云端轨依赖 opentake-gen 双模框架(Phase 9);本地轨依赖 candle/ort 推理栈 + ModelDownloader

### AI 智能消除瑕疵(object/blemish removal) — `missing` · 难度 high · 优先级 p3
- **判定依据**:上游零本地实现:grep inpaint/removeObject/blemish/magicEraser 全无命中(『cleanup』仅命中 GenerationService 临时文件清理,GenerationService.swift:67-149)。云端『edit』是整片重绘非局部消除(EditAction.swift:50;≤10s 限制 MODULE-PORT-MAP.md:528),且无 mask 通道可指定消除区域。
- **落点(crate/层)**:opentake-media(inpainting worker:mask 区域 + ort 跑视频 inpaint 模型)+ 复用蒙版工具圈选区域 + opentake-gen(可选外部 inpaint API)
- **实现方案**:依赖蒙版先行,本地+云双轨。① 用户/AI 用蒙版工具(本模块第2项)圈出待消除区域(或 AI 自动分割出对象),生成逐帧 mask。② 本地推理:图像用 LaMa(ONNX,擦除效果好);视频用 ProPainter / E2FGVI(时序一致视频补全,有开源权重),经 ort 跑;轻度瑕疵/人脸可用更小模型。复用 RVM/SAM2/超分同套 ONNX 通道。③ 外部 API 兜底:fal/Replicate 的 video-inpaint / object-removal 端点作为 BYOK 可选(opentake-gen 双模),质量优先时用。④ 与 AI 抠像、追踪强协同:SAM2 可同时给分割 mask + 跨帧追踪,是消除/抠像/追踪三项的共享上游。结果回填为新片段或叠加层。这是模块2 里依赖链最长、工程量最大的一项。
- **前置依赖**:强依赖蒙版工具(p1)产出区域 + AI 抠像/追踪(SAM2 共享)+ candle/ort 推理栈;视频 inpaint 模型较重

### 通用视觉特效 / 滤镜(effects/filters)+ 调色 — `missing` · 难度 medium · 优先级 p0
- **判定依据**:上游 FAQ 明示『尚无特效/调色/图形』,源码坐实:Clip 无 effect/filter/colorGrade/lut 字段(Models/Timeline.swift:77-107);Preview 合成 grep 不到 CIFilter/blendMode/colorControls/任何调色或滤镜(Preview/CompositionBuilder.swift 等);AVVideoComposition 只做 transform/crop/opacity ramp。AnimatableProperty 封闭集无 effect。导出 XMLExporter 也只导 transform/crop/opacity/fade/音量,不导任何滤镜。
- **落点(crate/层)**:opentake-domain(Clip 新增 effects: Vec<Effect> 效果链 + 调色字段/LUT 引用,Effect 参数可关键帧)+ opentake-render(wgpu 片元着色器效果库 + RenderPlan 串联效果链)+ opentake-agent(MCP 加 apply_effect 工具)
- **实现方案**:wgpu 着色器效果框架 = 一切像素特效的地基,本地、无需 AI。① 在 RenderPlan 里给每个 clip 增加『效果链』:解码纹理 → 依次过 N 个着色器 pass → 合成。② 基础调色(剪映高频):brightness/contrast/saturation/exposure/temperature/tint/HSL/曲线,纯片元运算;专业级支持 3D LUT(.cube 加载为 3D 纹理,着色器三线性采样)对齐达芬奇/剪映滤镜包。③ 滤镜/特效:模糊(高斯/方向/径向)、锐化、晕影、颗粒、故障、复古、发光(bloom,需多 pass)等,逐个着色器实现,所有参数挂关键帧(复用 easing)。④ 效果链顺序、开关、强度可调。⑤ FFmpeg 滤镜(eq/curves/lut3d/gblur)作为导出兜底,但实时预览统一走 wgpu 保预览=导出一致(ARCHITECTURE §6 共享 RenderPlan 原则)。⑥ 这是上游被 AVFoundation 锁死、OpenTake 自写合成器后『整类能力解锁』的核心机会(MODULE-PORT-MAP.md:391 自渲染方案)。建议先搭『效果链 + 1~2 个调色 pass』的最小框架,再增量加效果。
- **前置依赖**:强依赖 wgpu 帧合成器(Phase 3,项目命门);效果参数关键帧化依赖 easing 体系;前端调参 UI 依赖 Phase 6

### 转场(transitions) — `missing` · 难度 medium · 优先级 p0
- **判定依据**:上游无真正转场引擎:Clip 无 transition 字段、轨道无 clip-to-clip 转场模型(Models/Timeline.swift);源码『transition』仅两处——AppTheme.Anim.transition(UI 动画时长,与视频无关)与 XMLExporter 把单边 fade 映射成 Final Cut 的 Cross Dissolve/Cross Fade 导出占位(Export/XMLExporter.swift:296-341,且注释明示『no clip-to-clip model』『single-sided』)。所谓『matte 转场』只是 Agent 用 crop 关键帧做信箱遮罩的提示词技巧(AgentPanelView.swift:20),非转场原语。现有只有 fade in/out(fadeInFrames/fadeOutFrames + 插值,Timeline.swift:87-90),是单片淡变不是双片转场。
- **落点(crate/层)**:opentake-domain(新增 Transition 模型:轨道上两相邻 clip 之间的转场区,含类型/时长/参数;或 clip 持 transition_in/out)+ opentake-ops(转场区与覆盖/波纹/分割的交互算法)+ opentake-render(wgpu 双源混合着色器)+ opentake-agent(MCP add_transition 工具)
- **实现方案**:wgpu 双纹理混合着色器 + 时间线模型扩展,本地无 AI。① 数据模型:在两相邻 clip 重叠/接缝处定义转场区(duration、type、参数),需扩展 ops 层处理转场区与 ripple/overwrite/split/trim 的交互(这是逻辑难点——上游编辑算法纯按 clip 区间,引入转场区要保证不破坏对齐)。② 渲染:转场区内同时解码前后两 clip 的帧,着色器按进度 t(挂 easing)混合——溶解(crossfade,两纹理 alpha 插值)、擦除/滑动(wipe/slide,按 UV 阈值/位移切换)、缩放/旋转、模糊溶解、亮度/亮片过渡等,GL Transitions 开源库有大量现成着色器可直接移植到 wgsl。③ fade in/out 作为退化的单边转场保留兼容。④ 导出可继续映射到 XMEML Cross Dissolve(上游已有)+ wgpu 烧录非标准转场。⑤ 建议先做 crossfade(改动最小)验证转场区与编辑算法的交互,再铺开 wipe/3D 等。
- **前置依赖**:依赖 wgpu 帧合成器(Phase 3,双源混合)+ 需扩展 opentake-ops 编辑算法(Phase 1 之上)处理转场区交互;参数化依赖 easing 与 effects 框架

## 模块3:专业级色彩科学

**整体差距**:整体结论:本模块 6 项特性在 OpenTake 当前设计稿中【全部 missing】,且属于"上游本就没有、OpenTake 忠实复刻所以也没有"的同源缺口。源码级铁证:(1) 在 palmier-pro-upstream/Sources/PalmierPro 全量 grep `colorgrade|LUT|colorWheel|HSL|lift-gamma-gain|whiteBalance|colorMatch|grading|exposure|saturation|contrast|brightness` 命中 0 文件;(2) Clip 模型(Models/Timeline.swift:75-114)只有 trim/speed/volume/fade/opacity/transform/crop + 6 条关键帧轨(opacity/position/scale/rotation/crop/volume),无任何颜色字段;(3) 合成器 Preview/CompositionBuilder.swift 仅处理 transform/opacity/crop/audioMix 的 AVFoundation 声明式 layer instruction,无 CIFilter/colorMatrix;(4) OpenTake 自有报告 docs/_analysis/02-苹果框架可移植性.md:11/90 明确"零直接使用 CoreImage、无滤镜链可复刻";(5) 设计稿 ARCHITECTURE/ROADMAP/MODULE-PORT-MAP 全文无任何调色/LUT/色轮规划,wgpu 管线措辞为"解码→采样关键帧→仿射/裁剪/混合→多轨合成",无 color stage。

关键利好:OpenTake 抛弃了上游 AVFoundation 黑盒、改为【从零自建 wgpu 帧合成器】,这恰恰是实现专业调色的理想地基——色彩科学本质就是在合成片元着色器里对像素做数学变换,wgpu 路线比上游可移植性反而强得多。落地总策略:① 在 opentake-domain 新增 ColorGrade 值类型(挂到 Clip,序列化用 #[serde(default)] 保持向后兼容,不破坏读旧工程);② 在 opentake-render 的 wgpu 合成片元着色器里加一段【线性光空间调色链】(关键:务必在 BT.709→线性 解码后、混合/输出前做,顺序锁定 输入解码→曝光/白平衡→Lift/Gamma/Gain→曲线→HSL→LUT→线性→BT.709 编码);③ command 层加 SetColorGrade 命令并复刻 withTimelineSwap 事务+UndoStack;④ Agent 工具层加 set_color_grade / apply_lut / match_color 三个写工具(OpenTake 增强点)。优先级:浮点调色引擎(P0,是其余 5 项的共同底座)> 色轮/曲线/HSL(P1,纯着色器数学,中等)> 3D LUT 导入(P1,需 .cube 解析 + 3D 纹理三线性采样)> 一键色彩匹配(P2,需直方图/统计算法,可本地 Reinhard 起步)。建议作为 Phase 3(wgpu 合成器 PoC)之后、Phase 5(导出)之前插入的"色彩科学增强阶段",因为它强依赖合成器 blocker 先打通。

### 高阶浮点调色引擎 — `missing` · 难度 high · 优先级 p0
- **判定依据**:上游零调色:grep colorgrade/grading/exposure 命中 0;Clip(Models/Timeline.swift:75-93)无任何颜色字段;合成器 CompositionBuilder.swift 仅 transform/opacity/crop,_analysis/02:11 明确'零使用 CoreImage、无滤镜链'。OpenTake 设计稿(ARCHITECTURE/ROADMAP)同样无任何调色管线规划——属同源缺口。但 OpenTake 自建 wgpu 合成器(ARCHITECTURE.md:22)+ 已锁定 BT.709/sRGB 色彩管理(_analysis raw 1034 '全程 BT.709 + sRGB,zscale/colorspace 显式标注'),为浮点调色提供了理想着色器地基。
- **落点(crate/层)**:opentake-domain(新增 ColorGrade 值类型挂到 Clip)+ opentake-render(wgpu 合成片元着色器内的线性光调色链)+ opentake-ops/command(SetColorGrade 命令+UndoStack)
- **实现方案**:纯本地 wgpu 方案,零外部模型:在合成片元着色器里把每帧像素 BT.709→线性光(去 gamma)后,以 f32 RGB 跑统一调色链,最后线性→BT.709 编码回去。这是其余 5 项特性的公共底座——色轮/曲线/HSL/LUT 都是这条链上的算子。domain 加 ColorGrade{exposure,contrast,saturation,temp,tint,lift/gamma/gain,curves,hsl,lut_ref}(全 #[serde(default)] 保证读旧工程不破),render 把它编进 RenderPlan 逐 clip 传 uniform/storage buffer。预览与导出共享同一 RenderPlan 保证像素一致(对齐 ARCHITECTURE.md:120)。导出走 ffmpeg 编码前在 wgpu 已调好,无需 ffmpeg 调色滤镜。
- **前置依赖**:强依赖 Phase 3 wgpu 帧合成器 blocker 先打通(ARCHITECTURE.md:24 列为🔴命门);需先确立'线性光工作空间'与色彩管理约定(BT.709 解码/编码节点)。

### 色轮(暗部 lift / 中灰 gamma / 亮部 gain 矩阵) — `missing` · 难度 medium · 优先级 p1
- **判定依据**:grep lift-gamma-gain/colorWheel 上游命中 0;Clip 模型与 6 条关键帧轨(Models/Timeline.swift:102-107)均无颜色通道;设计稿无色轮规划。属浮点调色引擎之上的标准算子,上游/OpenTake 均未覆盖。
- **落点(crate/层)**:opentake-render(着色器内 lift/gamma/gain 数学)+ opentake-domain(ColorGrade 内 lift/gamma/gain 各一组 RGB 偏移)+ web 前端(三色轮 UI 控件)
- **实现方案**:纯本地着色器数学,DaVinci 经典公式:out = gain * (in + lift*(1-in)) ^ (1/gamma),三参数各为 RGB 三通道向量。在线性光空间(浮点引擎已建)对每像素做一次乘加+幂运算即可,GPU 成本极低。前端三个色轮控件输出 RGB 偏移,经 Tauri command 写 ColorGrade,走 SetColorGrade 命令入 UndoStack。可叠加关键帧化(复用上游关键帧采样 hold/linear/smoothstep 算法,但需为颜色新增关键帧轨,工作量在 domain 侧)。
- **前置依赖**:依赖'高阶浮点调色引擎'(P0)提供线性光工作空间与着色器调色链骨架。

### RGB 多维曲线 — `missing` · 难度 medium · 优先级 p1
- **判定依据**:grep curve/曲线 在上游调色语境命中 0(上游仅有关键帧插值 KeyframeTrack.sample,与色彩曲线无关);Clip 无颜色字段;设计稿未规划色彩曲线。
- **落点(crate/层)**:opentake-render(着色器内曲线 LUT 采样)+ opentake-domain(ColorGrade 内 4 条曲线控制点:Master/R/G/B)+ web 前端(贝塞尔曲线编辑器)
- **实现方案**:纯本地方案:前端用控制点定义曲线,Rust 端把每条曲线在 CPU 预烘焙成 1D LUT(256/1024 项),作为 1D 纹理上传,着色器对 R/G/B 各通道做 1D 纹理采样(textureSample)即可,运行时零计算开销。曲线插值可复用 Catmull-Rom 或单调三次样条(可直接引 crate splines)。Master 曲线作用于亮度或三通道统一,RGB 各自独立。注意曲线应在合适色彩空间(常用 gamma 编码空间而非线性,需与色轮顺序明确定义)。
- **前置依赖**:依赖'高阶浮点调色引擎'(P0);需明确调色链内曲线相对色轮/LUT 的次序。

### HSL 分区调色 — `missing` · 难度 medium · 优先级 p1
- **判定依据**:grep HSL/hue/saturation 上游命中 0(全部 color 标识符均为 UI 主题色 primaryColor/TrackColor/textColor,见 grep 统计);Clip 无颜色字段;设计稿无规划。
- **落点(crate/层)**:opentake-render(着色器内 RGB↔HSL 转换 + 分区加权)+ opentake-domain(ColorGrade 内 8 个色相区的 H/S/L 偏移)+ web 前端(分区滑杆/取色器)
- **实现方案**:纯本地着色器方案:片元着色器内把像素 RGB 转 HSL,按目标色相区(如红/橙/黄/绿/青/蓝/紫/品红 8 区)用平滑权重(基于色相距离的高斯/三角窗,避免硬边带)对落在该区的像素施加 H/S/L 偏移,再转回 RGB。这是 Lumetri/达芬奇 HSL Qualifier 的简化版。进阶可做'限定器'(luma/sat/hue 三维 key + 羽化),但首版做 8 固定区即可覆盖剪映同类能力。全部为本地像素数学,无需外部模型。
- **前置依赖**:依赖'高阶浮点调色引擎'(P0);需 RGB↔HSL 着色器工具函数。

### 3D LUT 导入 — `missing` · 难度 medium · 优先级 p1
- **判定依据**:grep LUT/.cube/colorMatrix 上游命中 0;_analysis/02:11 明确上游无任何 GPU shader/CoreImage 滤镜;设计稿(含技术选型清单 ARCHITECTURE.md:172-190)无 LUT 解析或 3D 纹理规划。
- **落点(crate/层)**:opentake-media 或 opentake-project(.cube/.3dl 文件解析 + 校验)+ opentake-render(3D 纹理三线性采样)+ opentake-domain(ColorGrade 内 lut_ref 指向工程内 LUT 资产 + 强度 0..1)
- **实现方案**:纯本地、跨平台、无外部模型:Rust 端写 .cube 解析器(行业标准 ASCII 格式:LUT_3D_SIZE N + N³ 行 RGB,可参考 crate 'lut' 或几十行自写,顺带支持 .3dl/.cube 1D)。把 LUT 上传为 wgpu 3D 纹理(rgba16f),着色器对线性/log 输入做三线性插值采样,再按强度与原图 mix。LUT 应放进 .opentake 工程包(媒体清单管理,复刻 content-hash 缓存机制 ARCHITECTURE.md:121),clip 只存 lut_ref(=资产 id),与上游'clip 永不存路径'语义一致(_analysis/01:127)。需明确 LUT 期望的输入色彩空间(sRGB/Rec709/Log)并在采样前做对应转换,否则色偏。
- **前置依赖**:依赖'高阶浮点调色引擎'(P0)的色彩空间约定;依赖工程包媒体管理(opentake-project,Phase 2 已规划)。

### 一键色彩匹配(自动统一色调与曝光) — `missing` · 难度 high · 优先级 p2
- **判定依据**:grep colorMatch/色彩匹配 上游命中 0;上游无任何图像统计/直方图分析代码(仅有 ffmpeg 抽帧缩略图与 Symphonia 波形,与色彩统计无关);设计稿未规划。属本模块技术门槛最高项。
- **落点(crate/层)**:opentake-render 或 opentake-media(参考帧/目标帧像素统计)+ opentake-agent(可选 match_color 高阶工具)+ opentake-domain(把匹配结果落成一组 ColorGrade 参数,可继续手调)
- **实现方案**:优先本地算法,分两档:① 起步版(本地、确定性、零成本)——对参考片段与目标片段各抽若干代表帧,在线性/Lab 空间统计每通道均值与标准差,用 Reinhard 色彩迁移(out=(in-μ_src)/σ_src*σ_ref+μ_ref)解出曝光/白平衡/对比的近似参数,写成 ColorGrade(关键:落成可见可调参数而非黑盒,符合 OpenTake 增强点'写工具返回结构化 JSON')。直方图匹配可作为进阶(逐通道 CDF 映射→烘焙成曲线 LUT,复用'RGB 曲线'通道)。② 增强版(可选,外部模型)——若要语义级'氛围匹配',可走 BYOK 调外部模型,但应作为可选项,不作默认(避免联网/成本,违背自托管卖点 ARCHITECTURE.md:152)。建议首版只做本地统计匹配,够用且可解释。Agent 侧暴露 match_color(source_clip, target_clips) 工具,把帧统计+解参在 Rust 内一次完成(对齐 OpenTake'把易错算术在 Rust 内一次完成'的 remove_filler_words 思路 ARCHITECTURE.md:145)。
- **前置依赖**:依赖以上全部调色基建(浮点引擎/曲线作为参数落点);依赖能从合成器/解码器取到代表帧像素并做统计(opentake-render/media)。

## 模块4:音频工程与智能字幕

**整体差距**:总体判断:本模块在 OpenTake/上游中呈"字幕强、音频空"的两极分布。字幕侧上游有完整端上转写子系统(Transcription 模块)+确定性断句计时(CaptionBuilder)+一键生成(CaptionTab/generateCaptions)+ MCP 工具(add_captions/get_transcript),OpenTake 设计稿已在 ARCHITECTURE §6 与 Phase 8 明确把这套纯逻辑直译进 opentake-domain/opentake-media,并用 whisper-rs 替换 Apple Speech——故"高精度 ASR 一键转字幕"判 has。音频工程侧(响度统一/降噪/人声分离)经 word-boundary grep 确认上游零实现:全工程无 loudness/LUFS/EBU-R128/loudnorm/denoise/noise gate/vocal isolation/source separation/EQ/compressor 任一关键字(唯一的 VideoCompressor 是上传前视频体积压缩,与音频无关),音频能力只有 Clip 静态 volume(线性)+ volumeTrack 关键帧(以 dB 存,VolumeScale=20·log10,floor -60/ceil +15)+ fade in/out + track mute。OpenTake 设计稿同样只移植 volume 包络与 VolumeScale,未引入测量/降噪/分离——故三项音频修复全判 missing。字幕翻译上游仅以 LLM Agent 草稿提示形式存在(captionTask("translate the captions to X")→handoff 到聊天面板),非独立 MT 引擎,OpenTake 继承同一 agent 机制但无一等公民翻译能力,判 partial。字幕样式全局批量同步:上游用 captionGroupId 把同组字幕共享样式、get_timeline 把众数样式 hoist 进 shared,但无"改一处样式→批量回写整组"的命令(实际靠 agent 逐条 set_clip_properties),数据模型在、批量算子缺,判 partial。导出 .srt:上游 Export 仅有视频渲染(文字经 CoreAnimation 烤进画面)+ FCPXML/XMEML(XMLExporter 明确声明文字不导出)+ .palmier 打包,全工程零 .srt/.vtt/SubRip,OpenTake 文档亦从未提及字幕文件导出,判 missing。落地优先级:ASR 已覆盖(p0 仅指接 whisper);响度统一与 SRT 导出是高性价比纯 FFmpeg/纯逻辑补齐(p1);降噪 p1(FFmpeg 内置滤镜先行,深度学习降噪 p2);批量样式同步是低成本编辑命令(p1);人声分离需引入额外深度模型,工程量最大(p2);翻译可先靠 agent 兜底再补离线/外部 API(p2)。所有音频特性的"施加层"已存在(per-clip dB 增益 + FFmpeg 音频滤镜链),真正缺口在"分析/处理层"。

### 音频响度统一(loudness normalization / LUFS 归一) — `missing` · 难度 medium · 优先级 p1
- **判定依据**:上游源码 word-boundary grep 对 loudness/LUFS/EBU/R128/loudnorm 全部 0 命中;音频模型仅 Models/Timeline.swift 的 volume(线性静态)+ volumeTrack(dB 关键帧)+ fadeIn/Out + track muted。VolumeScale(Inspector/InspectorView.swift:1072,20·log10,floor -60dB/ceil +15dB)证明只有手动增益、无任何自动测量/归一。OpenTake docs(MODULE-PORT-MAP Preview/Timeline 移植策略)只把 VolumeScale 与 volume 包络 direct-port,未提响度测量;ARCHITECTURE/ROADMAP 无任何响度相关条目。
- **落点(crate/层)**:分析落 opentake-media(新增 audio analyze 模块,产出 integrated/true-peak/LRA);应用落 opentake-ops(新增 EditCommand::NormalizeLoudness,把目标 LUFS 反算成 clip 静态 volume 的 dB 增量,复用既有 VolumeScale/volume);单段也可走 opentake-render 导出期 FFmpeg loudnorm 二次扫描。
- **实现方案**:纯 FFmpeg 跨平台,无需外部 API:用 ffmpeg ebur128 滤镜(或 loudnorm print_format=json 第一遍分析)测得每个音频源的 I/TP/LRA,按目标(常用 -14 LUFS / 短视频 -16 LUFS)算增益,写回 clip.volume(dB→linear 经 VolumeScale)。导出要严格达标时再走 loudnorm 双轨(linear=true,measured_* 回填)。本地算法,零模型、零联网。可做整轨/逐clip/全时间线三档。
- **前置依赖**:依赖 opentake-media 的 FFmpeg 音频解码链(Phase 2 已规划);写回增益依赖 opentake-domain 的 volume/VolumeScale(Phase 1 已就绪)。无 blocker。

### 智能降噪(noise reduction) — `missing` · 难度 medium · 优先级 p1
- **判定依据**:上游对 denoise/noiseReduction/noiseGate/noiseFloor/spectralGate/audioFilter/AVAudioUnit 全部 0 命中,无任何音频效果链。Export 仅 audioMix 做音量混音(CompositionBuilder),无降噪节点。OpenTake docs 无降噪条目。
- **落点(crate/层)**:opentake-media 新增 audio-fx 处理层(离线渲染音频片段→处理后 content-hash 缓存,与缩略图/波形缓存同构);opentake-domain clip 新增可空 audio_fx 描述字段,由 opentake-render 在合成/导出时插入 FFmpeg 音频滤镜链。
- **实现方案**:分两档跨平台。轻量档(p1 先行):FFmpeg 内置 afftdn(FFT 频域)/anlmdn(非局部均值)/highpass+lowpass+agate(噪声门),纯本地零依赖,覆盖稳态底噪/嗡声/嘶声。深度档(p2):引入 RNNoise(成熟 C 库+Rust 绑定,可静态链接,跨平台)或用 candle/ort 跑 DeepFilterNet ONNX 做非稳态人声降噪(贴近剪映'智能降噪')。降噪做成可选音频效果而非破坏式,结果落 content-hash 缓存。避免调外部云 API。
- **前置依赖**:需先有 opentake-media 音频解码/重编码 PCM 往返;深度档需 ort/candle 运行时(已在栈内用于 SigLIP2)+ 模型分发渠道(可学上游 SearchIndexConfig 静态 CDN 下载);需在 domain 增 clip 效果字段并打通 render 音频路径。

### 人声分离 / 提取人声(vocal isolation / source separation) — `missing` · 难度 high · 优先级 p2
- **判定依据**:上游对 vocalIsolation/sourceSeparation/stemSeparation/demucs/spleeter 全部 0 命中(grep 命中的 stem 是 ToolExecutor+Import.swift 文件名变量,与音频无关)。无任何 stem 分离能力。OpenTake docs 无相关条目。
- **落点(crate/层)**:opentake-media 新增 stem-separation 模块(输入音频源→输出 vocals/accompaniment 多轨 stem,落工程 media/ 作为派生素材);分离出的 stem 作为新 MediaAsset 回填,经 opentake-ops AddClips 放新轨,复用既有素材/轨道机制,无需改 domain 核心。
- **实现方案**:必须引入深度模型,无纯 FFmpeg 等价。优先本地跨平台:用 ort(onnxruntime 已在栈)跑 Demucs(htdemucs)或 MDX-Net 的 ONNX 导出,GPU 可选 CPU 兜底;或集成成熟 C/C++ 推理库经 FFI。把分离做成异步任务(对齐上游 generate 的 job 语义:placeholder→ready),结果作为派生 stem 入库。次选:托管模式走 opentake-gen-proxy 外接分离 API。本地优先以保隐私与零成本。这是本模块工程量最大、最依赖模型与算力一项。
- **前置依赖**:依赖 ort/candle 推理运行时 + 模型权重分发 + opentake-media 音频 IO + 任务/进度框架(可复用 opentake-gen job 状态机);GPU 加速可选。建议放在音频基建(解码/缓存/效果链)就绪之后。

### 高精度 ASR 一键转字幕 — `has` · 难度 medium · 优先级 p0
- **判定依据**:上游有完整链路:Transcription 模块(Transcription.swift/TranscriptCache.swift,词级+段级时间戳、语言匹配、缓存)、CaptionBuilder(确定性断句+按字符计时+最小时长防重叠)、EditorViewModel.generateCaptions(可见源区间转写→短语归属 clip→插新文本轨,一步撤销)、CaptionTab 一键 Generate Captions UI、MCP 工具 add_captions/get_transcript(ToolDefinitions.swift)。OpenTake 在 ARCHITECTURE §6(转写=whisper-rs,word/segment 时间戳)、Phase 8、MODULE-PORT-MAP Transcription 移植策略明确:数据模型与 CaptionBuilder/缓存/搜索算法 direct-port 进 Rust,ASR 引擎换 whisper-rs,上层'区间转写→切行→归属'逻辑不变。设计稿已完整覆盖。
- **落点(crate/层)**:opentake-media(whisper-rs 转写 + TranscriptCache)+ opentake-domain(TranscriptionResult/Segment/Word + CaptionBuilder + Clip.timelineFrame 映射)+ opentake-agent(add_captions/get_transcript 工具)+ 前端 Captions 面板。
- **实现方案**:按已定方案:whisper-rs(whisper.cpp 绑定,跨平台离线、支持词级时间戳与语言自检)替换 Apple Speech;FFmpeg 解码音频为 16k/mono/s16le 喂入;保留上游 CaptionBuilder 全套纯逻辑(断句优先级 .!?→,;:→词中点、按字符等比计时、minDisplayDuration≈0.7s 防重叠)与缓存键(path|mtime|size 的 sha256)。注意 whisper 词级时间戳需开 token timestamps,精度对齐到秒即可驱动下游帧映射。
- **前置依赖**:whisper-rs/whisper.cpp + 模型下载分发;opentake-media 音频抽取(Phase 2);CaptionBuilder/帧映射(Phase 1)。设计上无 blocker,仅工程量。

### 多语种翻译(字幕翻译) — `partial` · 难度 medium · 优先级 p2
- **判定依据**:上游字幕翻译非独立 MT 引擎,而是 LLM Agent 草稿:CaptionTab.swift:42 translateLanguages 列表(西/法/德/意/葡/日/韩/中/印地/阿拉伯)+ :237 captionTask("translate the captions to X, keeping each caption's timing unchanged")→handoff() 把提示词塞进聊天面板由模型执行(同机制还有 remove filler/fix names/add emoji)。无离线翻译、无翻译工具、无双语字幕轨。OpenTake Phase 7 移植同一 agent/chat 工具机制,agent-提示路径被继承,但设计稿无一等公民翻译特性。
- **落点(crate/层)**:短期:opentake-agent 复刻同款预置任务提示(保持 timing)走应用内 chat;中期:opentake-agent 新增结构化高阶工具 translate_captions(读 captionGroup→批量译→保 startFrame/duration→回写),或 opentake-gen 接外部 MT;译文可作同 group 另一语言字幕轨。
- **实现方案**:分层:① 兜底层=照搬上游 agent 提示(零额外依赖,已在 Phase 7 范围)。② 增强层=做确定性工具:整组字幕文本批量送翻译后逐条回填、严格保留时间码与 captionGroupId,避免 LLM 漏条/串行错位;后端二选一——离线(candle 跑 NLLB/M2M100 小模型,跨平台隐私优先,质量中等)或外部 API(DeepL/LLM,质量高需联网+key,走 BYOK)。默认 BYOK 外部 API 保质量、可选离线模型保隐私。③ 支持生成双语字幕(原文+译文分轨或上下叠)。
- **前置依赖**:依赖已生成的字幕(本模块 ASR 项);增强工具依赖 opentake-agent 写工具框架(Phase 7)与可选 opentake-gen/外部 key 管理(keyring);离线档需 candle + 翻译模型。

### 字幕样式全局批量同步 — `partial` · 难度 low · 优先级 p1
- **判定依据**:上游已有分组数据模型:Clip.captionGroupId 把一组字幕归为一体,CaptionBuilder.specs 生成时共享同一 TextStyle,get_timeline/ToolDefinitions.swift 把同组众数样式 hoist 进 shared、偏离者单列。但不存在'编辑组样式→一键回写整组'命令:CaptionTab 样式仅在生成时生效,生成后改样式靠 agent 逐条 set_clip_properties,Inspector 也是逐 clip 编辑。OpenTake docs(MODULE-PORT-MAP Agent/MediaPanel)移植 captionGroupId 与负载压缩,但未定义批量改样式算子。
- **落点(crate/层)**:opentake-domain 已有 captionGroupId(分组依据现成);opentake-ops 新增 EditCommand::SetCaptionGroupStyle{group_id, style_patch}(对组内全部 clip textStyle 做 partial-merge 回写,整体一条 undo);opentake-agent 暴露同名工具;前端 Inspector/Captions 面板加'应用到整组'入口。
- **实现方案**:纯编辑层、零媒体依赖、低成本:按 captionGroupId 选出组内全部文本 clip,对 textStyle 做不可变 partial merge(字体/字号/颜色/背景/描边/对齐/大小写),复用既有 mutateClips 快照撤销与 timelineFrame 不变性;样式 patch 语义对齐 set_clip_properties 文本字段。可选支持'跨组/全工程所有字幕'范围与'仅改差异项'。高频刚需且实现廉价,建议早做。
- **前置依赖**:依赖 opentake-domain captionGroupId 与 TextStyle(Phase 1 就绪)、opentake-ops 命令/撤销栈(Phase 1)、前端 Inspector(Phase 6)。无前置 blocker。

### 导出 .srt 字幕文件 — `missing` · 难度 low · 优先级 p1
- **判定依据**:上游 Export 模块(ExportService/XMLExporter/PalmierProjectExporter)对 .srt/.vtt/SubRip/WebVTT 全部 0 命中;导出只有三类——视频渲染(文字经 AVVideoCompositionCoreAnimationTool 烤进画面)、FCPXML/XMEML(MODULE-PORT-MAP 明确'文字不导出'/XMLExporter 注释声明文字叠加不进 XMEML)、.palmier 工程包。OpenTake docs(ARCHITECTURE/ROADMAP/MODULE-PORT-MAP Export)从未提及字幕文件导出。完全空白。
- **落点(crate/层)**:opentake-project 或 opentake-render 导出层新增 subtitle exporter(纯函数 Timeline→SRT/VTT 文本);opentake-agent 可加 export_captions 工具;前端导出对话框加'导出字幕(.srt/.vtt)'选项。
- **实现方案**:纯逻辑、跨平台、极低成本:遍历时间线文本/字幕 clip(优先 captionGroupId 组),按 startFrame/durationFrames 用 timeline.fps 反算时间码(SRT 用 HH:MM:SS,mmm 逗号毫秒;VTT 用点毫秒),按起始时间排序、合并同帧、输出标准 SubRip。帧→时间用既有 frameToSeconds(f/fps)、秒→时码做整毫秒;可选导出'烧录字幕'(已有,走视频渲染)对'软字幕文件'(本项)两条路。无外部依赖,纯字符串生成。建议同时支持 .vtt 便于 Web/平台分发。
- **前置依赖**:依赖 opentake-domain clip 时间字段与帧/秒换算(Phase 1 就绪);前端入口依赖导出 UI(Phase 6)。无 blocker。

## 模块5:AI 生成式创作(AIGC)

**整体差距**:总体判定:剪映模块5的四项 AIGC 能力,在 OpenTake 当前设计稿里【没有任何一项是"上游已有、可机械复刻"的现成功能】——因为上游 Palmier Pro 是瘦客户端,生成能力全部封装在闭源 Convex 后端,且其能力面只有 image/video/audio(TTS/music/sfx)/upscale 四类生成 + 端侧转写/语义搜索,catalog 的模型 Kind 枚举只有 {video,image,audio,upscale}(ModelCatalog.swift:126),根本不存在"成片/数字人/克隆"类目。

关键事实分两层:
1) 上游的"智能化"全是 LLM Agent 编排原子工具涌现出来的,不是专门功能。所谓"图文成片""生成 B-roll""配音"在上游只是 AgentPanelView.swift 里的几条 starterPrompts(film/waveform 图标),由 agent 调 generate_image→generate_video→generate_audio→add_clips/add_texts 拼出来;素材匹配靠端侧 SigLIP2 语义搜索 + import_media 接外部 stock;真正的"转场"上游 FAQ 自承认"尚无",代码里只有单边 fade(淡入淡出发 dissolve 到黑/静音,XMLExporter.swift:296)。
2) "智能剪口播"上游也无专用算法:CaptionTab 的"Remove filler words"(CaptionTab.swift:226)只是让 LLM 改字幕文本、保持时码不变,根本不剪音频;真正剪音视频是通用 agent 读词级 get_transcript + ripple_delete_ranges 组合完成的(易出帧算术错,系统提示词反复警告"段视图有损")。

对 OpenTake 的含义:四项里两项(图文成片、剪口播)OpenTake 设计稿已有"地基"(agent 工具层=opentake-agent、生成后端=opentake-gen、whisper-rs 转写、SigLIP2 搜索),且 ARCHITECTURE §7/ROADMAP Phase 7/report 04 已明确把 remove_filler_words/tighten_silences 列为"超越上游"的内置高阶工具——故判 partial。另两项(数字人、音色克隆)上游零代码、OpenTake 设计稿也完全未提,且本质都依赖外部生成模型,判 missing。所有四项的生成执行最终都要落到自建后端/BYOK(opentake-gen + opentake-gen-proxy)直连 fal/Replicate/ElevenLabs/HeyGen 等厂商,本地 Rust/FFmpeg/wgpu 只能承担编排、转写、静音检测、素材物化与落轨,不可能本地跑生成大模型。

### 图文成片(文案指令 → 自动匹配素材 + 配音 + 转场 → 成片) — `partial` · 难度 high · 优先级 p1
- **判定依据**:上游无一键成片功能,但构件齐全且 OpenTake 已规划对应层:① 编排能力=AgentPanelView.swift:13-35 的 starterPrompts("Generate an AI video"/"Generate B-roll: Inspect the current edit, identify sections...generate suitable B-roll, place it"/"Create a voiceover: Draft concise narration...add to audio track"),全靠 LLM agent 调原子工具涌现,AgentInstructions.swift:75-78 规定"Default flow: images first, then video"。② 配音=generate_audio 的 TTS 分支(AudioGenerationParams,云端;OpenTake report03 已逆向出 {kind:audio,prompt,voice,styleInstructions,...} 契约)。③ 素材匹配=端侧 SigLIP2 语义搜索(Search/,OpenTake 用 candle/ort 复刻)+ import_media 接外部 stock(ToolDefinitions.swift:431)。④ 转场=上游 FAQ 自承认"尚无",仅 fade 单边 dissolve(XMLExporter.swift:296)。OpenTake 设计稿:opentake-agent 工具层 + opentake-gen 生成 + 转写/搜索都已规划,但无"成片"高阶编排工具,也无真正的 transition 渲染。
- **落点(crate/层)**:opentake-agent(高阶编排工具 + 系统提示词)为主;依赖 opentake-gen(TTS/视频生成)、opentake-media(whisper-rs 转写 + SigLIP2 搜索)、opentake-render(转场合成)、opentake-ops(批量落轨)。
- **实现方案**:分两段。【编排即可达 80%】沿用上游"单一能力层、agent 编排"思路:在 opentake-agent 内置一个高阶工具/提示词模板 script_to_video(把"解析文案分镜→search_media 匹配本地素材/import_media 拉 stock→缺口走 generate_image/video→generate_audio TTS 配音→add_clips/add_texts 按节拍落轨"在 Rust 内编排成一次多步事务,易错的帧算术在 Rust 完成,只把创意决策留给 LLM)。生成执行走 BYOK/自建代理直连 fal/Replicate(opentake-gen 的 GenerationParams 联合类型已在 report03 设计)。【真转场需补 wgpu】上游本就只有 fade;若要做剪映式 dissolve/wipe/zoom 转场,必须在 opentake-render 的 wgpu 合成器里实现 clip-to-clip 双源混合(两段重叠帧按曲线 alpha/位移/缩放插值)——FFmpeg filter_complex 的 xfade 可作导出兜底但与预览难像素一致,推荐 wgpu 自渲染。配音建议优先"先转写脚本→TTS→落轨",时长按 audioTTSDurationSeconds=10s 默认(Constants,MODULE-PORT-MAP:1032)。
- **前置依赖**:强依赖 opentake-gen 生成后端(BYOK/代理)先可用;转场依赖 wgpu 帧合成器(项目最大 blocker)落地;素材匹配依赖 SigLIP2 语义搜索与 whisper-rs 转写就绪;落轨依赖 ops 命令层。

### 智能剪口播(剔除无意义停顿与语气词 / filler & silence removal) — `partial` · 难度 medium · 优先级 p1
- **判定依据**:上游无专用本地算法,且 OpenTake 已明确规划为"超越上游"的增强点。上游两条路径:① CaptionTab.swift:226-227 "Remove filler words (um, uh, er, like, you know)" 实为 LLM captionTask,只改字幕文本、"keeping each caption's timing unchanged",并不剪音视频;② 真正剪辑靠通用 agent:读词级 get_transcript(ToolDefinitions.swift:79)+ ripple_delete_ranges(ToolDefinitions.swift:282,"the fast path for filler-word/dead-air removal")组合,AgentInstructions.swift:65-69 反复警告"段视图有损、要把词级 transcript 当散文读"(说明这是踩坑痛点)。OpenTake 侧:ARCHITECTURE §7 与 ROADMAP Phase7 与 _analysis/04:227 均明确"内置 remove_filler_words / tighten_silences 高阶工具(参数化阈值),把读词→定位→ripple 在 Rust 内一次完成";转写已规划 whisper-rs(词/段时间戳)。故"语气词删除"设计层已覆盖且更强,但"无意义停顿检测"还缺一个真正的本地静音检测算法。
- **落点(crate/层)**:opentake-agent(remove_filler_words/tighten_silences 工具,复用 ripple_delete_ranges 内核)+ opentake-media(whisper-rs 词级转写 + 新增基于 PCM 的 VAD/静音检测)+ opentake-ops(RippleEngine 已是纯函数,可直接承接区间删除)。
- **实现方案**:全本地、跨平台,不需外部模型。【语气词】用 whisper-rs 拿词级时间戳,语气词识别两选一:轻量=内置多语停用词/语气词词典(um/uh/er/like/嗯/呃/那个…)直接命中;增强=可选调 LLM 给"哪些词是 filler"判定(只判定不算帧)。命中后把 [start,end] 词级区间喂给已移植的 RippleEngine::ripple_delete_ranges(MODULE-PORT-MAP:100 行该工具规格),linked A/V 同删、sync-locked 同移、放不下整体拒绝——帧算术全在 Rust,杜绝 LLM 帧错。【停顿/dead-air】用 Symphonia 解 PCM(波形管线已规划)做能量阈值 VAD:滑窗 RMS < 阈值且时长 > 最小静音时长(参数化,如 0.3s)即判为停顿,生成删除区间;同样走 ripple_delete。可选保留"留白"参数(每段静音保留 N ms 呼吸感)。导出为单步可撤销事务。
- **前置依赖**:依赖 whisper-rs 转写(Phase8)就绪;停顿检测依赖 Symphonia PCM 波形管线(Phase2);区间删除依赖 RippleEngine(Phase1,已是纯函数);最好在 agent 工具层(Phase7)统一暴露。

### 虚拟数字人出镜(digital avatar / 数字人口播) — `missing` · 难度 high · 优先级 p3
- **判定依据**:上游完全没有。全仓库搜 avatar/digital human/lip-sync/talking head,只命中 Account/IdentityViews.swift 的 UserAvatar(账户头像 UI),零生成相关代码。模型目录的 Kind 枚举只有 {video,image,audio,upscale}(ModelCatalog.swift:126),responseShape 只有 {video,images,audio,upscaledImage},根本没有 avatar/lip-sync 类目;AudioCaps.category 只有 tts/music/sfx。OpenTake 的 ARCHITECTURE/ROADMAP/MODULE-PORT-MAP 三份设计稿也均未提及数字人。
- **落点(crate/层)**:opentake-gen(新增 provider adapter + 一个新生成 kind=avatar/talking-head,落入 BackendGenerationParams 联合类型与 models catalog)+ services/opentake-gen-proxy(对接外部数字人 API);落轨复用 opentake-ops add_clips。
- **实现方案**:本地无法实现(数字人=驱动型生成大模型,需 GPU 集群),必须调外部模型 API。沿用 OpenTake 既定"统一 job 抽象 + provider adapter"架构(report03 §4),新增一类生成:输入=一张人像图(或预置形象)+ 一段脚本文本或一条音频(可串联前面的 TTS/音色克隆产物),provider 调 HeyGen / D-ID / fal 上的 talking-head/lip-sync 模型(如 sadtalker/hallo/latentsync 类),后端只吃 URL(人像/音频先预签名上传),返回 resultUrls→下载落库→add_clips 落到视频轨。catalog 用 uiCapabilities 描述"需人像+音频/文本、时长上限、分辨率"。BYOK 模式下 Rust core 直连厂商。难度主要在外部依赖与成本,不在本地工程。
- **前置依赖**:依赖 opentake-gen 生成后端骨架(job 状态机 + 上传预签名 + catalog 下发)先落地;通常需先有 TTS 或音色克隆产出音频作为驱动输入;无外部数字人 provider 则无法交付。

### 音色克隆(voice cloning / 自定义音色) — `missing` · 难度 high · 优先级 p2
- **判定依据**:上游完全没有。搜 voice clone/voiceprint/custom voice/speaker embedding 零命中。TTS 的 voice 只能从后端 catalog 下发的【预设字符串枚举】里选:AudioCaps.voices 是 [String]?(ModelCatalog.swift:223),GenerationView.swift:852-856 的 voicePicker 仅 ForEach 预设 voices 列按钮,无任何"上传参考音频/录制样本/创建我的音色"入口;AudioGenerationParams.voice 也只是 String?(预设名),无 referenceAudio 字段用于克隆(AudioModelConfig.swift:3-9)。agent 侧同理,只暴露 voicesSample/voiceCount(ToolExecutor+Generate.swift:445-446)。OpenTake 三份设计稿亦未提音色克隆。
- **落点(crate/层)**:opentake-gen(扩展 AudioGenerationParams 增加 referenceAudioURL/voiceId 字段 + provider adapter)+ services/opentake-gen-proxy(对接 ElevenLabs voice-clone / fal / MiniMax 等)+ 音色库管理(可存于 .opentake 工程或账户级)+ keyring 存厂商 key。
- **实现方案**:本地无法稳定实现高质量克隆(需说话人编码+声码器大模型),走外部 API 最务实。两步:① 创建音色:用户提供参考音频样本→预签名上传→调 ElevenLabs Instant Voice Cloning / MiniMax voice clone 等→拿回一个 voiceId,存入音色库。② 使用:把该 voiceId 当作 TTS 的 voice 传入现有 generate_audio 流水线即可复用全部落轨逻辑。需扩展 OpenTake 既有的 AudioGenerationParams 联合类型(report03 已设计该 enum)加 referenceAudioURL/clonedVoiceId 字段,catalog 用 caps 标注"该模型支持克隆"。合规上需加"声音授权确认"。若坚持本地路线,可评估 OpenVoice/XTTS(candle/ort 跑)做轻量零样本音色迁移,但质量与多语稳定性远逊云端,定位为可选实验功能。
- **前置依赖**:依赖 opentake-gen 生成后端 + 上传预签名 + keyring 密钥存储先就绪;依赖支持克隆的外部 provider;与 TTS(图文成片配音)、数字人驱动音频可串联复用。

