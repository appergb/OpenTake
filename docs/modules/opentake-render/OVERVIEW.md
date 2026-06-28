# opentake-render 总览

> 上级：[模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md) · 本模块目录：[INDEX.md](INDEX.md) · 完整规格：[SPEC.md](SPEC.md)

## 一句话定位

`opentake-render` 是 OpenTake 的**像素合成层**：把权威 `Timeline` 经纯函数 `RenderPlan` 折算成「每帧每层的几何/裁剪/不透明度/调色」属性，再用 wgpu 合成器 `render_to_rgba` 逐帧画出一张画布 RGBA 帧——**预览与导出共用同一条 RenderPlan + 同一个合成器，从而保证像素一致**。它是上游被 AVFoundation 黑盒锁死、OpenTake 必须从零自建的「项目命门」（ARCHITECTURE §1 / ROADMAP Phase 3）。

### 依赖分层位置

```
opentake-domain        值语义叶子层（Timeline/Clip/Transform/Crop/Keyframe/ColorGrade/TextStyle）
   ▲
opentake-render  ★本模块  RenderPlan（纯函数）+ wgpu 合成器 + 文本栅格化
   ▲
opentake-core / src-tauri   会话装配 + Tauri 命令（预览 / 导出后端调用本模块）
```

依赖**只向下**：本 crate 只依赖 `opentake-domain`（取 `Clip::*_at` 采样、`ColorGrade` 像素数学等），**不依赖 `opentake-media`**——解码/文件系统通过本 crate *定义、由 media 侧实现* 的 trait（`SourceMetrics` / `FrameProvider` / `TextureResolver`）反转进来（见 [source-size.md](source-size.md)）。`wgpu` 从 `lib.rs` 重导出，调用方借此命名设备/纹理类型而不必直接依赖 `wgpu`、避免版本错配。

## 职责边界

**做：**
- 纯函数 `Timeline → RenderPlan`（静态结构）+ `RenderPlan::frame(f)`（单帧 `FramePlan`），把上游声明式 ramp 改写成**逐帧直接求值**。
- render 层独有的**几何投影**：归一化画布坐标（0–1）→ 像素仿射（`affine_transform`）、CG `concatenating`（`compose`）、crop → 纹理 UV（`crop_to_uv`）、preferredTransform 朝向修正。
- wgpu 帧合成器：单 pipeline + 每层一个变换纹理 quad，预乘 alpha-over 顺序混合，离屏渲染后回读为 RGBA8（`Compositor::render_to_rgba`）。
- 进阶像素链 in-shader：色彩调色（线性光 LGG/曝光/白平衡/对比/饱和）、绿幕色度抠图、线性/圆形蒙版——着色器数学 1:1 镜像 `opentake_domain::grade` 的已单测参考。
- 文字 clip 栅格化为预乘 RGBA 纹理（`CosmicTextRasterizer`，cosmic-text 排版 + swash 栅格），对应上游 `CATextLayer`。
- 导出渲染尺寸偶数化与短边缩放纯函数（`even` / `export_render_size`）。

**不做：**
- **不碰文件系统 / 不解码**：`media_ref` → 路径、视频解码、图片/Lottie 像素都由 media 侧经 trait 注入。
- **不做关键帧/淡变/dB 采样**：一律调 domain 的 `Clip::opacity_at / transform_at / crop_at`，render 层**绝不重实现插值**（SPEC §0 铁律）。
- **不做音频**：音频混合（`volume_at` 包络 + track muted/去重）归 media/播放后端；RenderPlan 视频侧只列可视 clip。
- **不持播放状态**：A/V 同步、seek、scrub 节流属播放引擎（计划中），本 crate 只提供「给一帧 FramePlan、画一张图」的无状态能力。
- **不做秒↔帧换算**：一切以整数帧入参。

## 关键概念与数据流

### 核心：两层 RenderPlan（静态结构 + 逐帧求值）

上游 `CompositionBuilder.buildVisuals` 一次性发射整段 ramp 指令给 AVFoundation；OpenTake 是**逐帧拉取**架构（预览要 seek 任意帧、导出逐帧推进），因此 plan 分两层，与上游「静态 trackMappings + 动态 buildVisuals」二分同构：

```
build_render_plan(&timeline, render_size, &sources)   // 解析一次，帧无关
  → RenderPlan { fps, render_size, total_frames,
                 clip_plans: Vec<ClipPlan>,   // 视频层，已去重 + 按混合序排好
                 text_plans: Vec<ClipPlan> }  // 文字层，恒叠在视频之上、不去重

RenderPlan::frame(&timeline, f) -> FramePlan   // 对单帧求值（瞬时）
  对每个 ClipPlan：命中 [start,end) → 调 domain 取 opacity_at/transform_at/crop_at
                   → 几何投影成 affine + crop_uv + opacity → 一个 LayerDraw
  → FramePlan { clear_rgba: [0,0,0,1], draws: Vec<LayerDraw> }  // 已按混合序

Compositor::render_to_rgba(device, queue, size, &frame_plan, resolver) -> DecodedFrame
  clear 不透明黑 → 逐 draw（后者在上）解析纹理 + 上传 uniform + 画变换 quad
                 → 片元链：抠像→调色→蒙版→预乘→全局 opacity → alpha-over
  → 离屏 RT 回读为 RGBA8
```

数据流要点：
- **黑底不是 clip**：上游烧黑视频铺底，OpenTake 直接把合成器 clear color 设成不透明黑 `(0,0,0,1)`，整类「烧中间视频」hack 消失（SPEC §3.5）。
- **混合顺序**：`clip_plans` 按 `(track_index, start_frame)` 排好，下标越大越靠上；文字层（`text_plans`）整体叠在所有视频之上，对应上游 CoreAnimationTool 把文字烤在视频合成之上。
- **几何投影方向是像素 diff 命脉**：CG 行向量左乘 `p' = p·M`、坐标系原点左下/y 上、纹理 v 翻转**只发生一次**（在着色器 UV）——WGSL 顶点着色器用与上游 `CGAffineTransform` 完全相同的约定，6 元组 `[a,b,c,d,tx,ty]` 原样上传（SPEC §1.3 / §3.3）。
- **`nat_size` 随 affine 携带**（不是用解码纹理的真实分辨率）：预览按降档 `max_size` 解码，若用纹理尺寸当代理会与 affine 失配、把图层缩进角落并抖动（#125 修复，见 [render-plan.md](render-plan.md) / [gpu-compositor.md](gpu-compositor.md)）。

## 对应上游 Swift 模块

对照 [MODULE-PORT-MAP.md](../../architecture/MODULE-PORT-MAP.md)（上游路径 `palmier-pro-upstream/Sources/PalmierPro/`，Preview/ 与 Export/ 目录，verdict 多为 `needs-replacement`：上游全部委托 AVFoundation 黑盒，无 Metal/手写 shader）：

| 本模块 | 上游 Swift | 移植性质 |
|---|---|---|
| `plan/build.rs`（`build_render_plan` / `frame`） | `Preview/CompositionBuilder.swift` 的 `build` + `buildVisuals`（`trackOps` / `emitTransform` / `emitCrop` / `emitOpacity`） | 算法 1:1，但输出**每帧属性值**而非 AVFoundation ramp 指令 |
| `plan/affine.rs`（`affine_transform`） | `CompositionBuilder.affineTransform`（L599-614）| 逐行照搬（含 flip 符号、rotation 三段平移） |
| `gpu/compositor.rs`（`render_to_rgba`）| AVFoundation `AVVideoComposition` + layer instructions（黑盒）| **从零自建**——上游做不到，OpenTake 反超窗口 |
| `gpu/text_engine.rs`（`CosmicTextRasterizer`）| `Preview/TextLayerController.swift` 的 `CATextLayer` 树 / CoreAnimationTool 烧字 | 文字栈替换：cosmic-text + swash 替 CoreText |
| `gpu/{color,shader}` 的调色/抠像/蒙版 | 上游**无对应**（自述「尚无特效/调色/蒙版」）| OpenTake 新增（ADVANCED-FEATURES A 层） |
| `source.rs`（`SourceMetrics` / `FrameProvider`）| `AVAssetTrack.naturalSize` / `preferredTransform` / `AlphaVideoNormalizer` | 抽象成 trait，由 media 用 ffmpeg display matrix 实现 |
| `size.rs`（`even` / `export_render_size`）| `Export/ExportService.swift` `ExportResolution.renderSize` + `TimelineRenderer.even` | 纯函数照搬 |

上游被本模块**整类删除**的 hack：图片烧 30min 静止 .mov（`ImageVideoGenerator`）、黑底 .mov、Lottie 烧 ProRes4444、直通 alpha 预乘成 ProRes4444（`AlphaVideoNormalizer`）——合成器原生吃纹理后全部不需要。

## 完成状态：已实现 vs 计划中

对照 [ROADMAP.md](../../architecture/ROADMAP.md)（Phase 3 / 3.5 / 5 / 8）、[PORT-1TO1-GAP.md](../../architecture/PORT-1TO1-GAP.md)（P1-9 / P1-10）、[ADVANCED-FEATURES.md](../../architecture/ADVANCED-FEATURES.md) 与代码现状：

**已实现（代码中存在且带单测）：**
- 纯函数 `build_render_plan` + `RenderPlan::frame` + `source_frame_index`（变速/trim 源帧换算），含混合序、同轨去重、隐藏轨剔除、文字层分离、黑底 clear（`plan/`，单测见 `plan/tests.rs` + `affine.rs` 内联测试）。
- render 层几何 helpers `affine_transform` / `compose` / `crop_to_uv` / `normalize_box`，对拍上游 `affineTransform`（已知点 + 手算矩阵单测）。
- wgpu 合成器 `Compositor::render_to_rgba`：单 pipeline、预乘 alpha-over blend、离屏 RT 256 对齐回读；设备获取 `RenderDevice::try_new` 无 GPU 时优雅跳过（CI/headless）。
- 进阶像素链 in-shader（ADVANCED-FEATURES A 层落地部分）：线性光调色（曝光/白平衡/LGG/对比/饱和）、绿幕色度抠图（matte + spill 抑制）、线性/圆形蒙版（SDF + 羽化 + 反相，上限 `MASK_CAP=4`），WGSL 1:1 镜像 `opentake_domain::grade`。
- 文字栅格化两套：`NullTextRasterizer`（占位，永不 panic）+ `CosmicTextRasterizer`（cosmic-text 排版 + swash 栅格，含字号画布相对缩放、对齐、背景盒、投影 box-blur、描边，输出预乘 RGBA）。
- 纹理上传 + content-hash LRU 缓存（`TextureCache`）；导出尺寸 `even` / `export_render_size`（720p/1080p/4K 短边，不夹 1.0）。
- 媒体源契约 trait `SourceMetrics` / `FrameProvider` / `TextureResolver` + `DecodedFrame`（render 侧定义，带默认实现 + 单测）。

> 注：**整条时间线逐帧导出已在 `src-tauri` 落地**（#112 `export.rs` + `export_video`，H.264/.mp4，逐帧 `render_to_rgba` → 编码；#117 线性音频混音 AAC mux），证明「导出后端共享 RenderPlan + 合成器」已跑通——但该 spine 在 src-tauri/media，不在本 crate（见 [模块文档树](../INDEX.md) 的 src-tauri 条目）。

**计划中（仅 SPEC / ROADMAP / GAP 规划，本 crate 代码尚未落地或仅占位）：**
- **运行期预览接线**（PORT-1TO1-GAP P1-9）：`composite_frame(frame) → RGBA/PNG` 命令 + media 侧 `FrameProvider` 适配器 + 前端暂停态贴 canvas。合成器本身就绪，缺的是 Tauri 命令与前端粘合。
- **真实播放引擎**（ROADMAP Phase 4 / GAP P1-10）：ffmpeg 连续解码 + wgpu 上屏 + cpal 音频 + A/V 同步 + 精确 seek + scrub 30Hz 节流（移植 `VideoEngine`）。本 crate 不含任何播放状态。
- **线性光混合**：当前 PoC 在 **sRGB 非线性域**直接混合以最贴近 AVFoundation（`RT_FORMAT = Rgba8Unorm`，`color.rs` 的 sRGB↔linear 已备但合成 over 未切线性）；线性光（RGBA16F）为质量增强项，仅在像素 diff 通过后切换（SPEC §3.7）。
- **多边形（钢笔）蒙版 in-shader**：变长点列不适配固定 uniform，编码为全画布 no-op；穿 storage buffer 是 render 侧 TODO（domain 已存储且单测）。`effects: Vec<Effect>` 链亦仅透传、尚无对应 pass。
- **转场 transitions**（相邻 clip 重叠区 pass）：ADVANCED-FEATURES A 层 p0，尚无实现。
- **图片 / Lottie 物化**：`TextureSource::Image / Lottie` + `source_frame_index` 取模语义已在 plan 侧；实际像素由 media 侧 `image_pixels` / `lottie_frame` 提供（接线进行中）。

## 移植铁律（Swift → Rust，本模块强约束）

- **几何投影方向不得擅改**：CG 行向量左乘 `p' = p·M`、原点左下 y 上、半像素中心、纹理 v 翻转只一次——任一处错会整帧偏移。靠已知点单测 + 方向标记测试图锁死，绝不靠肉眼（SPEC §1.3/§3.3/§3.4 / 风险登记）。
- **一切整数帧**；`round` = half-away-from-zero（`f64::round()`），与 domain 一致；变速源帧 `trim + round(rel*speed)`，图片 trim 下限 `max(0,…)`（SPEC §2.5）。
- **`affine_transform` 逐行照搬上游**：flip 取负 + tx/ty 偏移、rotation `translate(-c)∘rotate(θ)∘translate(c)`、角度 `*π/180`。
- **`smoothstep(t)=t·t·(3-2t)`** 不换公式（采样在 domain，着色器内 `smoothstep01` 与之一致用于羽化/抠像）。
- **采样零重写**：opacity/transform/crop/调色数学一律走 domain 的 `*_at` 与 `opentake_domain::grade`，render 只加几何投影 + GPU 合成（SPEC §0 铁律）。
- **唯一真相源** = 上游 `CompositionBuilder.swift` + 已移植的 `opentake-domain`；每实现一个几何/调度函数立即与对应上游行号 + domain 方法对拍。

---

> 本模块目录：[INDEX.md](INDEX.md) · 上级：[模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
