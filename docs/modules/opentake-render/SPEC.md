# opentake-render 实现就绪规格(Issue #7:wgpu 帧合成器 + RenderPlan)

> 状态:实现就绪(implementation-ready)。本文是 `crates/opentake-render/` 的逐项施工图。
> 范围:① 纯函数 `Timeline → RenderPlan`(Rust 数据结构 + 算法,逐条对应上游公式);② wgpu render graph;③ 预览/导出共享 RenderPlan;④ 图片/文字/Lottie 物化为纹理;⑤ 与 `opentake-domain` / `opentake-media` 的接口契约;⑥ PoC 验收(与上游 `inspect_timeline` 像素 diff)+ 分步实施清单。
> 定位:这是**全项目命门**(ARCHITECTURE.md §1、ROADMAP Phase 3)。上游所有像素级合成都委托给 AVFoundation 黑盒(`AVVideoComposition` + layer instructions + ramps),无 Metal/CoreImage/手写 shader;OpenTake 必须自建 wgpu 合成器把这块从零补回。

## 0. 证据基准(只读上游源码与 docs,均为绝对路径)

本规格的每条算法都锚定到下列文件的具体行号。实现时以上游为唯一真相,逐函数对拍。

| 角色 | 文件 | 关键行 |
|---|---|---|
| 合成核心(待移植算法) | `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/palmier-pro-upstream/Sources/PalmierPro/Preview/CompositionBuilder.swift`(843 行) | 见下逐项 |
| 播放/重建驱动 | `…/Preview/VideoEngine.swift`(326 行) | `rebuild` L137、`refreshVisuals` L187、seek 节流 L225-272 |
| 导出 | `…/Export/ExportService.swift`(274 行) | `makeExportSession` L216、preset 映射 L254-273、文字烧录 L237-248、`renderSize` 偶数化 L39-46 |
| 任意帧区间渲染(共享 plan 先例) | `…/Preview/TimelineRenderer.swift`(86 行) | `render` L13、`renderSize` L76、`even` L85 |
| 图片→静止视频 hack | `…/Preview/ImageVideoGenerator.swift`(227 行) | `stillVideo` L16、`createPixelBuffer` L101(premultipliedFirst+sRGB L118/126)、`blackVideo` L74、`imageNativeSize` L90、`clampedForEncoder` L57 |
| alpha 直通→预乘 hack | `…/Preview/AlphaVideoNormalizer.swift`(163 行) | `premultipliedVideo` L9、`trackContainsAlpha` L34、`premultiply`(vImage)L138 |
| 文字渲染(预览+导出) | `…/Preview/TextLayerController.swift`(224 行) | `applyStyle` L152、`isGeometryFlipped` L13、`applyOpacityAnimation` discrete L191、`buildForExport` L75、`visibleTextClips` L122、`referenceCanvasHeight=1080` L150 |
| 文字样式/测量 | `…/Models/TextStyle.swift`、`…/Models/TextLayout.swift` | `attributes` L138、`naturalSize` L9、shadowPadding=12 L6 |
| 领域模型(已 1:1 移植到 Rust) | `…/Models/Timeline.swift`、`…/Models/Keyframe.swift`、`…/Models/ClipType.swift` | `Transform` L364、`Crop` L501、`affineTransform` 输入语义见下 |
| 架构/路线 | `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake/docs/ARCHITECTURE.md` §1/§6 | `…/docs/_analysis/02-苹果框架可移植性.md`、`…/docs/ROADMAP.md` Phase 3 L25-34 |

**已就位的 Rust 依赖(本 crate 不得重写,只能调用):** `crates/opentake-domain/` 已逐行移植并单测覆盖:
- `Clip::transform_at / size_at / top_left_at / rotation_at`(`clip.rs` L214-246)
- `Clip::crop_at`(L254)、`Clip::opacity_at / raw_opacity_at`(L188-204)、`Clip::volume_at / raw_volume_at`(L277-296)、`Clip::fade_multiplier`(L300)
- `Clip::has_transform_animation`(L248)、`Clip::end_frame / contains / source_frames_consumed`(L164-181)
- `KeyframeTrack::sample`(`keyframe.rs` L164,左端点 interpolation_out 决定 hold/linear/smooth,端点钳制)、`smoothstep`(L26)
- `VolumeScale::linear_from_db / db_from_linear`(`clip.rs` L23-42)
- `Transform{center_x,center_y,width,height,rotation,flip_horizontal,flip_vertical}`、`Transform::top_left/center`(`transform.rs`)、`Crop::visible_width_fraction/visible_height_fraction`(L266-272)
- `Timeline{fps,width,height,tracks}`、`Track{id,kind,muted,hidden,sync_locked,clips}`、`Track::end_frame`、`Timeline::total_frames`(`timeline.rs`)

> ⚠️ **施工铁律**:RenderPlan 的属性采样**一律调用 domain 的 `*_at` 方法**,严禁在 render 层重新实现关键帧插值/fade/dB 换算。render 层只做上游 `CompositionBuilder.buildVisuals` 之上的**几何投影(归一化→像素仿射 + 裁剪矩形 + premultiply + 多轨混合)与帧采样调度**,这些恰是 AVFoundation 替我们做掉、domain 层没有的部分。

---

## 1. 上游合成模型逐项拆解(待复刻语义,带行号)

上游 `CompositionBuilder` 做两件事:(A)`build`(L34)构造 `AVMutableComposition` 轨道 + 插入/变速;(B)`buildVisuals`(L375)发射 layer instructions(opacity/transform/crop ramp)。OpenTake 自建合成器后,(A) 的"轨道拼接"由我们自己的逐帧解码调度承担,(B) 的"声明式 ramp"被**逐帧直接求值**取代——我们不需要发 ramp 给黑盒,我们就是黑盒。

### 1.1 轨道与帧映射(对应 build L34-237)
- 逐 track 逐 clip,按 `startFrame` 升序(L56)。**text clip 永不进合成轨道**,走独立文字路径(L57 `filter { $0.mediaType != .text }`,见 §4.2)。
- clip 在时间线占 `[startFrame, endFrame)`(半开),`endFrame = startFrame + durationFrames`(domain `end_frame`)。
- gap(clip 之间空隙)上游用 `insertEmptyTimeRange`(L316);OpenTake 中"该轨该帧无 clip 覆盖"= 此轨此帧不贡献像素。
- 同轨重叠防御:`clip.startFrame >= previousEndFrame`(L152、L424)。重叠的后续 clip 被丢弃。**RenderPlan 必须复刻这个 skip 规则**(见 §2.4)。
- **最底层不透明黑底**:上游 `insertBlackBackground`(L346)铺一段黑视频覆盖 `[0, desiredDuration)`,`desiredDuration = max(totalFrames, 最后视频结束)`(L206-207)。OpenTake 中这是合成器的 **clear color = 不透明黑 (0,0,0,1)**,无需纹理(见 §3.5)。
- **变速**:`speed != 1.0` 时 source 区间 = `round(durationFrames * speed)` 帧,再 `scaleTimeRange` 缩到 `durationFrames`(L319-340)。OpenTake 中映射为"源时间游标"推进(见 §2.5、§5.3)。

### 1.2 opacity(对应 emitOpacity L731 / emitOpacitySet L565 / emitEnvelopeRamps L507)
- 每条可视 layer instruction 起始 `setOpacity(0, at: .zero)`(L407)——即未进入 clip 区间前完全透明。
- 无 fade:走 `trackOps`(opacityTrack,fallback=clip.opacity,L741),逐段发 setStatic/ramp。
- 有 fade:`emitEnvelopeRamps`(L761)把 opacity 关键帧 + fade 头尾折叠成分段线性包络,采样点 `clip.opacityAt(frame)`(domain `opacity_at`,已含 fade×kf×static)。
- clip 末尾强制 `setOpacity(0, at: end)`(L431):clip 一结束立即透明。
- **OpenTake 等价**:对帧 `f`,clip 在 `[start,end)` 内可见,其 alpha 乘子 = `clip.opacity_at(f)`。`f<start || f>=end` ⇒ 该 clip 不贡献(等价 opacity 0)。**无需发 ramp,逐帧直接调 `opacity_at`。**

### 1.3 transform(对应 emitTransform L617 / affineTransform L599)
上游把归一化画布坐标(0–1)的 `Transform` 映射成 AVFoundation 期望的 `CGAffineTransform`,公式(L599-614)**必须逐行照搬**(domain 未含此几何投影,这是 render 层职责):

```
// affineTransform(for t: Transform, natSize, renderSize):  natSize=clip 源像素显示尺寸,renderSize=画布像素
sx = (renderSize.w / natSize.w) * t.width  * (t.flipHorizontal ? -1 : 1)
sy = (renderSize.h / natSize.h) * t.height * (t.flipVertical   ? -1 : 1)
tx = (t.flipHorizontal ? t.topLeft.x + t.width  : t.topLeft.x) * renderSize.w
ty = (t.flipVertical   ? t.topLeft.y + t.height : t.topLeft.y) * renderSize.h
placed = scale(sx,sy) 然后 translate(tx,ty)              // CG concatenating = 先 self 后参数
if t.rotation == 0 { return placed }
cx = t.centerX * renderSize.w ;  cy = t.centerY * renderSize.h
return placed ∘ translate(-cx,-cy) ∘ rotate(rotation*π/180) ∘ translate(cx,cy)
```

并且最终矩阵是 `preferredTransform.concatenating(affineTransform(...))`(L628):`preferredTransform` 是源轨道朝向修正(下文 §4.1 说明 OpenTake 用 ffmpeg display matrix 取得)。

- 无 transform 动画(`!hasTransformAnimation`,domain `has_transform_animation`):整段一个静态矩阵 `affine(clip.transform)`(L632)。
- 有动画:上游对 position/scale/rotation 关键帧帧号取并集(L637-647),每段用 8 分 `smoothSegments` 细分发 ramp(L666-689),采样 `clip.transformAt(frame)`(domain `transform_at`)。
- **OpenTake 等价**:逐帧直接 `affine(preferredTransform, clip.transform_at(f), natSize, renderSize)`。**8 分细分是 AVFoundation ramp 的离散化近似;我们逐帧求值,精度严格更高**,但为对拍上游(§6)需保留"按帧采样"语义——上游 ramp 在整数帧上的值 = 我们逐帧值,两者在帧中心一致(ramp 端点取自同一 `transformAt`)。细分仅在上游内部用于子帧插值,不影响整数帧输出。

> **关键投影约定(CG → wgpu)**:CG 仿射 `concatenating(B)` 语义是 `A.concatenating(B) = A·B`(行向量左乘:`p' = p · A · B`,先 A 后 B)。CG 坐标系原点左下、y 向上(AVFoundation 视频空间)。**OpenTake 顶点着色器用同一约定**:把上游 6 元组 `(a,b,c,d,tx,ty)` 原样塞进 `mat3x2`(或贴到 NDC 的 `mat4`),坐标系按"原点左下 / y 上"建立,保证与上游逐像素一致(见 §3.3)。这是像素 diff 通过的命脉,不得擅自改半像素中心或 y 翻转。

### 1.4 crop(对应 emitCrop L700 / trackOps L780)
- crop 是源坐标(0–1 inset)矩形:`x=left*natW, y=top*natH, w=max(1, visibleWidthFraction*natW), h=max(1, visibleHeightFraction*natH)`(L711-716),再 `.applying(preferredTransform.inverted())`(L709,把源 crop 矩形映回源像素方向)。
- crop 关键帧用 `trackOps`(cropTrack,fallback=clip.crop,L718),逐段 setStatic/ramp;采样 `clip.crop_at(f)`(domain)。
- **OpenTake 等价**:对帧 `f` 取 `crop = clip.crop_at(f)`,转成源纹理 UV 子矩形(见 §3.4)。`visible*Fraction` 已在 domain 钳到 ≥0(`transform.rs` L266)。注意上游 `max(1, …)` 是像素下限,OpenTake 在 UV 空间对应"至少 1 源像素宽/高",防退化采样。

### 1.5 多轨混合(对应 instruction.layerInstructions 顺序 L405-449)
- `layerInstructions` 顺序 = `trackMappings.filter(\.isVideo)` 顺序 = **轨道枚举顺序 + 黑底最后追加**(L194/L206)。AVFoundation 按 layer instruction 数组顺序混合,**后者在上**。
- 因此 OpenTake 混合顺序:**黑底(最底)→ track[0] → track[1] → … → track[n-1](最顶)**。注意上游黑底 mapping 是最后 append 到 trackMappings 的(L209-215),但它的 opacity 在自身区间是 1、其余轨道盖在它上面——等价于"黑底铺底,视频轨从下标 0 到 n-1 依次叠加"。OpenTake 直接按此顺序 alpha-over 合成(见 §3.6)。
- `track.hidden` ⇒ 整轨不渲染(L419);`track.muted` ⇒ 音频不出声(audio 路径 L391,本 crate 视频侧只需关心 hidden)。

### 1.6 色彩空间(对应 vcConfig L456-458 + ImageVideoGenerator L169-173)
- 视频合成输出锁 **BT.709**:primaries / transfer / YCbCr matrix 全 `ITU_R_709_2`(L456-458)。
- 但**图片/黑底**烧视频时,transfer 用 **IEC sRGB**(L171),primaries/matrix 仍 709。即:图片像素在 sRGB 空间绘制(`createPixelBuffer` 用 `CGColorSpace.sRGB`,premultipliedFirst,L118/126),编码进 mov 时打 709 primaries + sRGB 传递函数标签。
- **OpenTake 策略**(见 §3.7):内部合成在**线性光**做混合(物理正确的 alpha-over),输入纹理按其传递函数解码到线性,输出再编码回目标传递函数。为对齐上游 709/sRGB,默认管线见 §3.7 表。PoC 阶段可先在 sRGB 非线性空间直接混合(与上游 AVFoundation 行为更接近,误差更小),线性光混合作为质量增强项延后(见 §6 容差说明)。

---

## 2. 产出①:纯函数 `Timeline → RenderPlan`(Rust 数据结构 + 算法)

**模块**:`crates/opentake-render/src/plan/`(`mod.rs`、`build.rs`、`types.rs`、`tests.rs`)。
**纯函数,零 IO,可全单测,可与 wgpu 解耦**(ROADMAP L28、ARCHITECTURE §6)。这是把上游 `buildVisuals` 移植成 Rust 的部分,但**输出的是"每帧/每段属性值",不是 AVFoundation instruction**。

### 2.1 设计取舍:两层 plan(static 结构 + per-frame 求值)

上游 `buildVisuals` 一次性把整段时间线的 ramp 都发出去(声明式)。OpenTake 是**逐帧拉取**架构(预览要 seek 到任意帧、导出逐帧推进),因此 RenderPlan 分两层:

1. **`RenderPlan`(静态、与帧无关)**:解析一次 `Timeline`,固化"哪些轨道、每轨哪些 clip(已去重/排序)、每个 clip 的源类型与 natSize 来源、preferredTransform、混合顺序、画布尺寸、fps、总帧数、黑底区间"。**纯结构信息,不含逐帧数值。** 缓存友好:Timeline 不变则 plan 不变(对应上游 `trackMappings` 缓存 + `refreshVisuals` 快路径 VideoEngine L187)。
2. **`FramePlan = RenderPlan::frame(f)`(瞬时、对单帧求值)**:对给定帧 `f`,产出**有序的 `Vec<LayerDraw>`**——每个 = 一次合成器 draw(纹理源 + 仿射矩阵 + crop UV + premultiplied alpha 乘子)。逐帧调用 domain 的 `*_at`。

> 这与上游"static trackMappings + 动态 buildVisuals"的二分完全同构(VideoEngine 缓存 trackMappings 重算 visuals,L169/L195)。

### 2.2 数据结构(`plan/types.rs`)

```rust
use opentake_domain::{Timeline, Clip, ClipType, Transform, Crop};

/// 画布像素尺寸(已偶数化,见 §5.2)。
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct RenderSize { pub width: u32, pub height: u32 }

/// 源纹理在合成器中的来源。物化策略见 §4。
#[derive(Clone, PartialEq, Debug)]
pub enum TextureSource {
    /// 视频/音频(音频无视频纹理,渲染侧忽略):按 media_ref + 源帧索引解码。
    Decoded { media_ref: String },
    /// 图片:一张静态纹理(content-hash 缓存)。上游烧 30min 静止视频的 hack 在此消失。
    Image   { media_ref: String },
    /// Lottie:按"Lottie 内部帧"光栅化的纹理序列(content-hash 缓存)。
    Lottie  { media_ref: String },
    /// 文字:对该 clip 在画布尺寸下排版光栅化的纹理(content-hash 缓存,key 含 style+content+canvas)。
    Text    { clip_id: String },
}

/// 单 clip 的静态渲染描述(帧无关)。
#[derive(Clone, PartialEq, Debug)]
pub struct ClipPlan {
    pub clip_id: String,
    pub track_index: usize,
    pub source: TextureSource,
    pub start_frame: i32,
    pub end_frame: i32,            // 半开
    /// 源像素显示尺寸 = 上游 clipNaturalSizes[clip.id](CompositionBuilder L166-172):
    /// natSize = |CGRect(origin:.zero, size: natSize0).applying(preferredTransform)|.size
    pub nat_size: (f64, f64),
    /// 源轨道朝向修正(上游 preferredTransform,已平移到 box.minX/minY 归零,L172)。
    /// 行优先 [a, b, c, d, tx, ty]。无朝向 = 单位阵 [1,0,0,1,0,0]。
    pub preferred_transform: [f64; 6],
    /// 是否需要 premultiply(直通 alpha 源,见 §4.1)。图片/文字已是预乘。
    pub needs_premultiply: bool,
    /// 速度(用于源帧索引换算,见 §2.5)。
    pub speed: f64,
    pub trim_start_frame: i32,
    pub media_type: ClipType,
}

/// 整条时间线的静态计划。
#[derive(Clone, PartialEq, Debug)]
pub struct RenderPlan {
    pub fps: i32,
    pub render_size: RenderSize,
    pub total_frames: i32,
    /// 混合顺序:index 越大越靠上(见 §1.5)。clip_plans 内部已按 (track_index 升序, start_frame 升序) 排好。
    /// 黑底不在此列表——它是合成器 clear color(§3.5)。
    pub clip_plans: Vec<ClipPlan>,
}

/// 对单帧求值后的一次 draw(瞬时)。
#[derive(Clone, PartialEq, Debug)]
pub struct LayerDraw<'a> {
    pub source: &'a TextureSource,
    /// clip 在该帧引用的"源帧索引"(Decoded/Lottie 用;Image/Text 恒 0)。见 §2.5。
    pub source_frame: i64,
    /// 归一化画布(0–1)→ 像素的最终仿射,= preferred ∘ affineTransform(transform_at(f))(§1.3)。
    /// 行优先 [a,b,c,d,tx,ty],坐标系:原点左下、y 向上、单位像素(§1.3 投影约定)。
    pub affine: [f64; 6],
    /// 源纹理 UV 子矩形(crop_at(f) 折算),(u0,v0,u1,v1) ∈ [0,1]。见 §3.4。
    pub crop_uv: (f64, f64, f64, f64),
    /// premultiplied alpha 全局乘子 = clip.opacity_at(f) ∈ [0,1]。
    pub opacity: f64,
    pub needs_premultiply: bool,
    pub clip_id: &'a str,
}

/// 单帧的有序 draw 列表 + clear color。
#[derive(Clone, PartialEq, Debug)]
pub struct FramePlan<'a> {
    pub clear_rgba: [f64; 4],     // 恒 [0,0,0,1] 不透明黑(§3.5)
    pub draws: Vec<LayerDraw<'a>>,// 已按混合顺序排好,直接顺序 alpha-over
}
```

### 2.3 主算法 `build_render_plan`(`plan/build.rs`)

签名(纯函数,natSize 解析需要外部喂源尺寸,见契约 §5.1):

```rust
pub fn build_render_plan(
    timeline: &Timeline,
    render_size: RenderSize,
    // 源固有尺寸/朝向查询(由 opentake-media 提供;text/image/lottie 见 §4)。
    sources: &dyn SourceMetrics,
) -> RenderPlan;
```

逐步(对拍 CompositionBuilder.build L53-216 + buildVisuals L405-445):

1. `total_frames = timeline.total_frames()`(domain)。`render_size` 已偶数化(调用方按 §5.2 处理)。
2. 遍历 `timeline.tracks.iter().enumerate()`,跳过 `track.hidden`(上游 buildVisuals L419 在发指令时判 hidden;OpenTake 直接在 plan 阶段剔除)。
3. 每轨内 `clips` 按 `start_frame` 升序(`sort_by_key`),**复刻去重**:维护 `prev_end_frame = i32::MIN`,clip 入选需 `duration_frames > 0 && start_frame >= prev_end_frame`(上游 L152 视频 / L424 visuals),入选后 `prev_end_frame = end_frame`。**text clip 跳过**(`media_type == Text`,上游 L57/L422),走 §4.2 文字路径单独收集。
4. 每个入选 clip 构造 `ClipPlan`:
   - `source` 按 `media_type` 选 `TextureSource`(video/audio→Decoded,image→Image,lottie→Lottie)。audio clip 无视频纹理:**audio track 整轨不产生 ClipPlan**(本 crate 视频侧;音频混合在播放/导出后端另行处理,见 §3.8)。
   - `nat_size` / `preferred_transform`:见 §4.1(视频从 ffmpeg display matrix + naturalSize;图片从解码尺寸;lottie 从 manifest;text 从 §4.2 排版尺寸)。复刻上游 L166-172 的 box 归一化:`nat_size = |bbox(natSize0, preferredTransform)|`,`preferred_transform` 末尾平移 `(-box.minX, -box.minY)`。
   - `needs_premultiply`:视频且源为直通 alpha → true(§4.1);图片/文字/lottie 已预乘 → false。
   - `speed / trim_start_frame / media_type` 原样拷。
5. 收集所有 `ClipPlan` 到 `clip_plans`,**最终排序键 `(track_index, start_frame)`**(保证混合顺序 = 轨道顺序;同轨已无重叠)。
6. 返回 `RenderPlan { fps, render_size, total_frames, clip_plans }`。

### 2.4 单帧求值 `RenderPlan::frame`(`plan/build.rs`)

```rust
impl RenderPlan {
    pub fn frame<'a>(&'a self, timeline: &'a Timeline, f: i32) -> FramePlan<'a>;
}
```

> 注:`frame()` 需要回看 `timeline` 取 `Clip`(domain 采样方法在 `Clip` 上)。`RenderPlan` 与 `Timeline` 同源不可变,二者配对使用(等价上游 buildVisuals 持 trackMappings + timeline 双引用)。可选:把每个 clip 的 `&Clip` 指针/索引存进 `ClipPlan` 以省查找。

逐 `ClipPlan`(已按混合顺序):
1. 命中测试:`f < start_frame || f >= end_frame` ⇒ 跳过(等价上游 clip 区间外 opacity=0,L407/L431)。
2. 取该 clip 的 `&Clip`(按 `clip_id` 或预存索引)。
3. `opacity = clip.opacity_at(f)`(domain;已折 fade×kf×static)。**`opacity == 0` 可跳过该 draw**(优化,行为等价)。
4. `transform = clip.transform_at(f)`(domain);`affine = compose(preferred_transform, affine_transform(transform, nat_size, render_size))`(§1.3 公式,纯几何,在 render 层实现 `affine_transform` + `compose`,见 §2.6)。
5. `crop = clip.crop_at(f)`(domain);`crop_uv = crop_to_uv(crop)`(§3.4)。
6. `source_frame = source_frame_index(clip_plan, f)`(§2.5)。
7. push `LayerDraw { source, source_frame, affine, crop_uv, opacity, needs_premultiply, clip_id }`。
返回 `FramePlan { clear_rgba: [0,0,0,1], draws }`。

### 2.5 源帧索引换算 `source_frame_index`(对拍 insertClip L301-343 的 trim+speed+scaleTimeRange)

给定时间线帧 `f`(已知 `f ∈ [start,end)`)求该 clip 引用的源帧:

```
rel = f - start_frame                       // clip 内偏移(时间线帧)
trim = (media_type == Image) ? max(0, trim_start_frame) : trim_start_frame   // 上游 L310
src  = trim + round(rel * speed)            // 变速:source 推进 = rel*speed(上游 scaleTimeRange 反向,L319-340)
```
- `Image` / `Text`:恒 `source_frame = 0`(单帧纹理)。
- `Lottie`:`source_frame = (trim + round(rel*speed)) % lottie_total_frames`(或钳到末帧,与 §4.3 物化语义一致)。
- `Decoded`(video/audio):`source_frame = src`,交给解码后端按"源帧号→PTS"定位(§5.3、§3.8)。
- `round` = half-away-from-zero(与 domain 一致,`clip.rs` L7 约定)。

### 2.6 render 层纯几何 helpers(`plan/affine.rs`,需单测对拍上游)

```rust
/// 行优先 3x2 仿射,语义同 CG:p' = p · M。compose(a,b) = a ∘ b(CG concatenating)。
pub fn affine_transform(t: &Transform, nat: (f64,f64), rs: RenderSize) -> [f64;6];   // §1.3 公式
pub fn compose(a: [f64;6], b: [f64;6]) -> [f64;6];                                    // CG concatenating
pub fn crop_to_uv(c: Crop) -> (f64,f64,f64,f64);                                      // §3.4
```

`affine_transform` 严格逐行照搬 §1.3(含 flip 符号、rotation 三段平移-旋转-平移、`*π/180`)。**这是与上游 `affineTransform`(CompositionBuilder L599)对拍的核心单测点。**

### 2.7 RenderPlan 纯函数单测(`plan/tests.rs`,无 GPU)

必测(逐条对应上游公式,断言 `LayerDraw` 字段数值):
- 单 clip 居中满画布、无变换 ⇒ affine = `[rs.w/nat.w, 0, 0, rs.h/nat.h, 0, 0]`(由 `transform.width=height=1, topLeft=(0,0)`)。
- flipHorizontal ⇒ sx 取负、tx = `(topLeft.x+width)*rs.w`(§1.3)。
- rotation=90° at center(0.5,0.5) ⇒ 与手算矩阵 1e-9 一致。
- transform 关键帧线性插值:帧中点的 affine = 用 `transform_at(mid)` 算出的矩阵(验证逐帧求值 = 上游 ramp 端点)。
- crop_at 关键帧 ⇒ crop_uv 子矩形随帧变化。
- opacity fade-in:`opacity_at(start)=0`,`opacity_at(start+fade/2)` 线性/smoothstep 命中。
- 同轨重叠后 clip 被去重(plan.clip_plans 不含它)。
- 多轨混合顺序:track0 在 track1 之下(draws 顺序)。
- hidden track 不出现在 clip_plans;text clip 不出现在 clip_plans(归 §4.2)。
- 黑底:`FramePlan.clear_rgba == [0,0,0,1]` 恒成立。

---

## 3. 产出②:wgpu render graph 设计

**模块**:`crates/opentake-render/src/gpu/`(`device.rs`、`compositor.rs`、`texture.rs`、`shader.wgsl`、`color.rs`)。
依据 ARCHITECTURE §1/§6、`02-苹果框架可移植性.md` §4(blocker #1)。

### 3.1 顶层:逐帧合成 = "为每个 LayerDraw 画一个带变换的纹理 quad,顺序 alpha-over 到画布 RT"

输入:`FramePlan`(§2.4)+ 已上传/缓存的纹理(§4)。输出:一张画布尺寸 RGBA 纹理(预览上屏 / 导出回读编码)。

```
渲染单帧(framebuffer = 画布 RT,RGBA16F 线性 或 RGBA8 sRGB,见 §3.7):
  1. clear RT = clear_rgba(不透明黑)                          // §3.5
  2. for draw in framePlan.draws (按序,后者在上):
       a. tex = 取纹理(draw.source, draw.source_frame)        // §4 物化/缓存
       b. 设 uniform: affine(mat3x2)、crop_uv、opacity、flags(premultiply/transfer)
       c. draw 一个覆盖 [0,1]² 的 quad(顶点经 affine 映到画布像素再到 NDC)
       d. blend = alpha-over(premultiplied):src.rgb + dst.rgb*(1-src.a)   // §3.6
```

单一 render pipeline + 每 draw 换 bind group(纹理 + uniform)。无几何缓冲膨胀:quad 是 4 顶点常量,变换全在 uniform。

### 3.2 资源

- **Render target**:画布尺寸纹理(`TextureUsages::RENDER_ATTACHMENT | TEXTURE_BINDING | COPY_SRC`)。预览路径可直接是 surface texture 或离屏后 blit;导出路径离屏 + `COPY_SRC` 回读(§3.8)。
- **Uniform buffer**(每 draw):`{ affine: mat3x2<f32>(用 vec4+vec2 对齐), crop_uv: vec4<f32>, opacity: f32, flags: u32 }`。用 dynamic offset 或每帧小 uniform 池,避免每 draw 重建。
- **Sampler**:`linear / clamp-to-edge`(crop 子矩形在边缘 clamp,防越界采样;对应上游 crop `max(1,…)` 像素下限语义 §1.4)。
- **纹理缓存**:见 §4.4。

### 3.3 顶点着色器(`shader.wgsl`)——投影约定是像素 diff 命脉

```wgsl
// quad 顶点 in [0,1]^2(左下原点)。affine 把归一化画布坐标 → 画布像素(原点左下、y 上)。
// 然后像素 → NDC。注意:上游 CG/AVFoundation 用"原点左下、y 上";wgpu NDC y 也向上,
// 但纹理坐标/viewport 需对齐。务必让"画布像素 (0,0)=左下角"与上游一致(§1.3 约定)。
struct U {
  ar0: vec4<f32>,   // a, b, c, d
  ar1: vec2<f32>,   // tx, ty
  crop_uv: vec4<f32>,
  opacity: f32,
  flags: u32,
};
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var t_color: texture_2d<f32>;
@group(0) @binding(2) var s_color: sampler;
@group(0) @binding(3) var<uniform> canvas: vec2<f32>; // 画布像素尺寸

@vertex fn vs(@builtin(vertex_index) vi: u32) -> VsOut {
  let quad = array<vec2<f32>,4>(vec2(0.,0.), vec2(1.,0.), vec2(0.,1.), vec2(1.,1.));
  let p = quad[vi];                              // [0,1] 归一化画布坐标
  // 行向量左乘:p' = p · M(CG 语义,§1.3)
  let px = vec2(p.x*u.ar0.x + p.y*u.ar0.z + u.ar1.x,
                p.x*u.ar0.y + p.y*u.ar0.w + u.ar1.y);  // 画布像素(原点左下)
  let ndc = vec2(px.x/canvas.x*2.0 - 1.0, px.y/canvas.y*2.0 - 1.0);
  // UV:quad 角 → crop 子矩形。纹理 v 方向按源方向(§3.4 决定是否翻转)。
  let uv = mix(u.crop_uv.xy, u.crop_uv.zw, p);
  return VsOut(vec4(ndc, 0.0, 1.0), uv);
}
```

> **注意**:`p · M`(行向量)对应上游 CG `point.applying(transform)`。`M` 的 6 元组顺序 `[a,b,c,d,tx,ty]` 与 CGAffineTransform 字段一一对应。**单测必须用已知点验证此乘法 = 上游 `CGPoint.applying`。**

### 3.4 crop → UV

源 crop 是 inset(0–1):可见区 `[left, 1-right] × [top, 1-bottom]`(源坐标,原点左上,CG 源像素方向)。
- `u0 = left, u1 = 1 - right`(domain `visible_width_fraction = 1-left-right`,已 ≥0)。
- 纹理 v 方向:取决于纹理上传时的行序与"原点左下"约定。源 crop `top` 在源图上方;若纹理按"行 0=顶部"上传(常见 ffmpeg/图片解码),则 `v0 = top, v1 = 1 - bottom`,采样时着色器/viewport 统一翻转一次以匹配 §1.3 的"y 上"。**翻转只能发生一次**:要么在 UV、要么在顶点 y、要么在上传——三选一并固定,PoC 用像素 diff 锁定方向(§6)。
- `preferredTransform.inverted()`(上游 L709)的影响已折进 `preferred_transform`(它作用在顶点几何,不作用在 UV);crop 矩形在源像素方向定义,故 UV 用未旋转的 inset 即可。旋转源(preferredTransform≠单位)由顶点 affine 承担。

### 3.5 黑底(clear color,取代上游 blackVideo)

上游用生成的黑视频铺底(ImageVideoGenerator.blackVideo L74 + insertBlackBackground L346)。OpenTake **直接 clear RT = (0,0,0,1) 不透明黑**,语义完全等价且省一条纹理路径(ARCHITECTURE §6"图片烧成静止视频/黑底这类 hack 整类消失")。clear 在 §3.7 选定的工作色彩空间里就是 sRGB/线性 的纯黑(0 在两空间相同),无歧义。

### 3.6 混合:premultiplied alpha,顺序 alpha-over

- 所有纹理在采样后转成 **premultiplied**(若 `needs_premultiply`,在片元里 `rgb *= a`;图片/文字已预乘则跳过)。再乘全局 `opacity`:`rgb *= opacity; a *= opacity`(预乘下 opacity 同时缩放 rgb 和 a)。
- wgpu blend state(预乘 over):
  ```
  color: src_factor=ONE, dst_factor=ONE_MINUS_SRC_ALPHA, op=ADD
  alpha: src_factor=ONE, dst_factor=ONE_MINUS_SRC_ALPHA, op=ADD
  ```
- 与上游一致性:AVFoundation layer 合成用 premultiplied(故上游才需 `AlphaVideoNormalizer` 把直通 alpha 预乘,§4.1)。OpenTake 把"预乘"放进片元(直通源)或上传期(图片),结果等价。

### 3.7 色彩空间管线(对拍 §1.6)

| 阶段 | 上游 | OpenTake PoC(对齐优先) | OpenTake 质量增强(后置) |
|---|---|---|---|
| 输入解码 | AVFoundation 标记 709/sRGB | 视频:ffmpeg 输出 RGBA(按源 transfer);图片:sRGB | 同 PoC |
| 工作空间 | AVFoundation 内部(709 非线性近似) | **sRGB 非线性 RGBA8**(直接混合,最贴近上游) | **线性光 RGBA16F**(物理正确 over) |
| 混合 | 黑盒 | 在 sRGB 非线性域 alpha-over | 解码到线性 → over → 编码回目标 |
| 输出 | 709 primaries + sRGB transfer | RGBA8 sRGB | 编码到 709/sRGB |

PoC 用 sRGB 非线性直接混合以最小化与 AVFoundation 的差异(§6 容差);线性光混合作为视觉质量增强延后,且只在确认像素 diff 通过后切换(切换会引入与上游的可控偏差)。`color.rs` 提供 sRGB↔linear 转换以备增强。

### 3.8 帧回读 / 音频(导出/播放接口边界)

- **导出**:合成 RT → `COPY_SRC` 到 buffer → 映射读回 → 交 `opentake-media` 编码(§5.4)。逐帧 `[0, total_frames)`。
- **预览**:合成 RT → 上屏(surface)或交前端纹理。
- **音频**:本 crate **不做音频混合**。音频包络(上游 `emitVolumeEnvelope` L475 + `volume_at`)由播放后端(cpal)/导出后端用 domain `Clip::volume_at` 逐样本/逐段求值。RenderPlan 视频侧只列视频可视 clip。音频侧的"轨道 muted、clip 去重、volume_at 包络"在 §5.3 接口里交给 media/播放层(它同样调 domain,无需 render 介入)。

### 3.9 GPU 依赖(写入 `crates/opentake-render/Cargo.toml`,本规格不改文件,仅声明)

`wgpu`、`bytemuck`(uniform POD)、`pollster`(同步等设备,非 async 上下文)、`glam`(可选,矩阵)、`opentake-domain`(path)、`opentake-media`(path,纹理源/编解码)。`thiserror`(错误)。

---

## 4. 产出④:图片/文字/Lottie 物化为纹理的策略

依据 ARCHITECTURE §6「媒体物化策略照搬」+「content-hash 缓存」+「自建合成器后上游烧中间视频的 hack 整类消失」。**核心反超点**:上游因 AVPlayer 不能直接放图/Lottie/文字,被迫烧成视频(ImageVideoGenerator/LottieVideoGenerator/CoreAnimationTool);OpenTake 合成器原生吃纹理,全部 hack 删除。

### 4.1 视频源(Decoded)+ preferredTransform + premultiply

- **natSize / preferredTransform**:上游从 `AVAssetTrack.naturalSize` + `preferredTransform`(CompositionBuilder L166-172)取。OpenTake 从 ffmpeg:`naturalSize` = 解码帧尺寸;`preferredTransform` = 容器 **display matrix**(`AV_PKT_DATA_DISPLAYMATRIX` / `AVStream` side data,旋转 90/180/270 + flip)。转成 6 元组,按上游 L170-172 做 box 归一化(`nat_size = |bbox|`,平移归零)。无 side data ⇒ 单位阵、`nat_size = 解码尺寸`。
- **premultiply**:上游 `AlphaVideoNormalizer`(L9)检测**编解码器 alpha 标志**(`kCMFormatDescriptionExtension_ContainsAlphaChannel` L37),仅对**直通 alpha 且 preferredTransform 为单位**(L16)的源预乘。OpenTake:解码出带 alpha 的像素格式(如 yuva/ rgba)⇒ `needs_premultiply = true`,在片元 `rgb*=a`。**无需烧中间 ProRes4444**;旋转源不再是障碍(我们的几何在顶点处理,与 premultiply 解耦,可放宽上游"仅单位阵才预乘"的限制——但 PoC 阶段保持一致以便对拍)。
- 解码接口见 §5.3。

### 4.2 文字源(Text)——上游 CATextLayer → cosmic-text + tiny-skia/Vello 光栅纹理

上游文字**不进合成轨道**,预览用 `CATextLayer` 树逐帧改 opacity(TextLayerController),导出用 `CAKeyframeAnimation`(discrete)+ `AVVideoCompositionCoreAnimationTool` 烧入。OpenTake **把每个文字 clip 排版光栅化成一张带 alpha 的纹理**,作为一个 `LayerDraw` 参与合成(逐帧 opacity = `clip.opacity_at(f)`,与视频同路径)。

物化规格(对拍 TextLayerController.applyStyle L152 + TextLayout L9,数据已在 `opentake-domain::text::{TextStyle, Rgba, TextLayout}`):
- **排版引擎**:cosmic-text(换行/对齐),光栅 tiny-skia 或 Vello;字体注册 fontdb(ARCHITECTURE §6、§10)。
- **尺寸/缩放**:`scale = containerHeight / 1080`(referenceCanvasHeight,L155/L150);`fontSize = textStyle.font_size * font_scale * scale`(L165)。
- **文字框**:`frame = (topLeft.x*W, topLeft.y*H, transform.width*W, transform.height*H)`(L157-163)——即文字 clip 的 `nat_size` 用此框像素尺寸 + shadow padding(TextLayout shadowPadding=12 双边,L6/L28),`preferred_transform = 单位`。注意:文字纹理已是"画布像素尺寸的一块",其 `affine_transform` 用 `nat_size = 文字框像素`、`transform.width/height` 仍是归一化 ⇒ 与视频统一公式(§1.3)。
- **样式**:对齐(left/center/right,L170)、阴影(color 含 alpha 作 opacity,offset/ blur × scale,L176-187)、背景盒(enabled 时填色 L172)、描边(border,thin×scale L173-174)、前景色 `Rgba`(sRGB,text.rs)。换行 `byWordWrapping`(TextStyle L133)。
- **可见性**:`visibleTextClips`(L122)= 非 hidden 轨的 `media_type==Text && end>start`。**注意上游文字 clip 收集不做"同轨去重"**(与视频不同),OpenTake 文字路径照此:每个文字 clip 各自一张纹理、各自一个 `LayerDraw`。
- **混合顺序**:上游文字层 `zPosition = clip 在 visibleTextClips 中的 index`(L58),且文字整体由 CoreAnimationTool 叠在视频合成**之上**(ExportService L237-248:`postProcessingAsVideoLayer`,文字 parent 包视频 layer)。**OpenTake 等价:所有文字 LayerDraw 排在所有视频 LayerDraw 之后(最顶层)**,文字之间按 visibleTextClips 顺序叠。→ §2.3 第 5 步排序需把文字 clip_plans 统一置于视频之后(可用 `track_index` 之上再加"文字层"维度,或单独 `text_plans: Vec<ClipPlan>` 在 frame() 里拼到 draws 末尾)。**推荐**:`RenderPlan` 增 `text_plans: Vec<ClipPlan>`,`frame()` 先视频后文字。
- **缓存 key**:`hash(clip_id? 否 → content + style + container_size)`。文字内容/样式/画布尺寸不变则纹理复用。content-hash 缓存(ARCHITECTURE §6)。
- **逐帧 opacity**:纹理静态,opacity 走 `LayerDraw.opacity = clip.opacity_at(f)`(等价上游 discrete CAKeyframeAnimation 逐帧值,TextLayerController L209)。

### 4.3 Lottie 源(Lottie)——上游烧 ProRes4444 → rlottie/velato 逐内部帧光栅纹理

上游 `LottieVideoGenerator` 把 Lottie 烧成视频再当普通 video clip。OpenTake **按需把 Lottie 的"内部帧"光栅化成纹理**(rlottie FFI 优先,velato 备选;ARCHITECTURE §6/§10)。
- `nat_size` / 帧数 / framerate:从 manifest(`source_width/height/fps`,MediaAsset.loadMetadata 对应字段)或 rlottie inspect。
- `source_frame`(§2.5):`(trim + round(rel*speed)) % lottie_total_frames`(或钳末帧,与物化一致;Lottie 通常循环,取模更贴近"长视频"语义)。
- 已带 alpha 且预乘 ⇒ `needs_premultiply = false`(rlottie 输出预乘 BGRA/RGBA)。
- 缓存:每"内部帧"一张纹理,content-hash(文件 hash + 帧号 + 尺寸)缓存(避免重复光栅)。

### 4.4 纹理缓存与生命周期(`gpu/texture.rs`)

- **图片/文字/Lottie 帧**:content-hash → GPU 纹理。LRU 容量上限(防显存膨胀)。预览只缓存近窗口(对拍 TextLayerController preroll=30,L24);导出可顺序物化、用后即弃。
- **视频帧**:不长期缓存原始帧;解码后上传当前所需帧(预览 seek 到最近关键帧+丢帧,§5.3;导出顺序解码)。
- content-hash 缓存复用 ARCHITECTURE §6 策略;磁盘层(可选)由 `opentake-media` 提供(对应上游 DiskCache,ImageVideoGenerator L7)。

---

## 5. 产出⑤:与 opentake-domain / opentake-media 的接口契约

### 5.1 依赖 opentake-domain(只读,已就位)

render 层**只调用**,不重实现:`Timeline / Track / Clip / Transform / Crop / ClipType / TextStyle / Rgba / TextLayout`,以及 `Clip` 的全部 `*_at` 采样方法、`has_transform_animation`、`end_frame / contains / source_frames_consumed`、`KeyframeTrack::sample`、`smoothstep`、`VolumeScale`。证据:`crates/opentake-domain/src/{clip,keyframe,transform,timeline,text}.rs` 已单测覆盖(见 §0)。

> render 层**新增且仅新增**上游有、domain 没有的几何投影:`affine_transform`(CompositionBuilder L599)、`compose`(CG concatenating)、`crop_to_uv`、preferredTransform 的 box 归一化(L166-172)。这些是"AVFoundation 替上游做掉、domain 不该承担 IO/几何投影"的部分。

### 5.2 渲染尺寸偶数化(对拍 ExportResolution.renderSize L39-46 / TimelineRenderer.even L85)

render 层提供(纯函数,供导出后端/调用方用):
```rust
pub fn even(v: f64) -> u32 { ((v.round() as i64 / 2) * 2).max(2) as u32 }   // 上游 even L85
pub fn export_render_size(canvas: (i32,i32), short_side: Option<i32>) -> RenderSize; // L39-46/L76-83
```
- 导出按短边目标缩放:`scale = min(1.0, short_side / min(w,h))`(TimelineRenderer L81;ExportResolution 用 `short/canvasShort` 不夹 1,L43——两者差异:导出全片用 ExportResolution 语义,任意区间渲染用 TimelineRenderer 语义。**默认采用 ExportResolution L39-46(720/1080/2160 短边),不夹 1.0**,与正式导出一致)。
- 宽高各自 `even()`,最小 2。预览用画布原尺寸或降档(§5.3)。

### 5.3 依赖 opentake-media(解码/纹理源 trait,media 实现)

render 定义 trait,media 实现(契约,媒体侧逐帧喂像素):

```rust
/// 源固有尺寸/朝向(build_render_plan 用,纯查询)。
pub trait SourceMetrics {
    /// 视频:解码帧尺寸;图片:像素尺寸;lottie:画布尺寸。(对拍 imageNativeSize L90 / naturalSize)
    fn natural_size(&self, media_ref: &str) -> Option<(u32, u32)>;
    /// 视频容器 display matrix → 6 元组(无则单位)。对拍 preferredTransform L169。
    fn preferred_transform(&self, media_ref: &str) -> [f64; 6];
    /// 源是否带 alpha 且需预乘(对拍 trackContainsAlpha L34)。
    fn needs_premultiply(&self, media_ref: &str) -> bool;
    /// lottie 内部总帧数(取模用,§4.3)。
    fn lottie_frame_count(&self, media_ref: &str) -> Option<i64>;
}

/// 逐帧像素供给(合成器渲染时按需拉)。返回已解码 RGBA(或可上传格式)。
pub trait FrameProvider {
    /// 取 media_ref 的源帧 source_frame(§2.5)的像素。预览:解到最近关键帧+丢帧到目标;
    /// 导出:顺序解码。返回 (width,height,rgba8) 或纹理句柄。
    fn decoded_frame(&self, media_ref: &str, source_frame: i64) -> Option<DecodedFrame>;
    /// 图片像素(单帧,sRGB premultipliedFirst 等价,对拍 createPixelBuffer L101)。
    fn image_pixels(&self, media_ref: &str) -> Option<DecodedFrame>;
    /// lottie 内部帧光栅(预乘 RGBA)。
    fn lottie_frame(&self, media_ref: &str, frame: i64) -> Option<DecodedFrame>;
}

pub struct DecodedFrame { pub width: u32, pub height: u32, pub rgba: Vec<u8>, pub premultiplied: bool }
```

- **文字纹理**不经 media:render 层用 cosmic-text/tiny-skia 自渲(§4.2),因为它依赖 domain 的 TextStyle 且是合成层职责(上游也在 Preview/ 内做)。可放 `gpu/text_raster.rs`,依赖 fontdb/cosmic-text/tiny-skia(写入 Cargo.toml,本规格不改)。
- **媒体解析(media_ref → 路径)**:由 `opentake-project::MediaResolver`(对拍上游 MediaResolver,`expectedURL`/`resolveURL` L13-27)解析,经 media 或调用方注入。render 不碰文件系统。
- **音频**:render 不参与;播放/导出后端用 `Clip::volume_at`(domain)+ track.muted/去重(§3.8)。

### 5.4 导出/预览后端如何共享 RenderPlan(产出③)

**同一个 `build_render_plan` + `RenderPlan::frame` + wgpu compositor,两个后端只是"帧来源/帧去向"不同**(对拍上游 ExportService L216 与 VideoEngine L137 共享 CompositionBuilder;TimelineRenderer L29 也复用同一 build,证明"一套 plan 多后端"是上游既定模式)。

```
共享: RenderPlan = build_render_plan(&timeline, render_size, &metrics)
                   FramePlan = plan.frame(&timeline, f)
                   compositor.render(framePlan, &textures) -> 画布 RT

预览后端(低延迟,ROADMAP Phase 4):
  - render_size = 画布原尺寸或降档(保帧率,02 报告/ARCHITECTURE §6)
  - FrameProvider::decoded_frame 用"解到最近关键帧 + 丢帧到目标"(seek tolerance 概念)
  - compositor 输出 → surface 上屏;A/V 同步、scrub 30Hz 节流(移植 VideoEngine L225-272)在播放层
  - refreshVisuals 快路径:Timeline 仅改可视属性(opacity/transform/crop/volume)时,plan.clip_plans 结构不变,
    只需重算 frame()(对拍 VideoEngine.refreshVisuals L187:不重建轨道只重发 visuals)

导出后端(全质量,ROADMAP Phase 6):
  - render_size = export_render_size(canvas, short_side)(§5.2,偶数化)
  - 逐帧 f ∈ [start, start+count):FrameProvider 顺序解码 → compositor → RT → COPY_SRC 回读 → media 编码
  - 预设 H.264/H.265/ProRes × 720p/1080p/4K(对拍 ExportService L254-273),码率/profile/色彩逐项逼近(02 报告:预设黑盒需测试)
  - 任意帧区间渲染(对拍 TimelineRenderer L13)= 同管线,frame 范围参数化
```

**像素一致性保证**:预览与导出走同一 `frame()` 求值 + 同一 compositor + 同一 affine/crop/混合,差异仅在分辨率与解码精度(预览降档/丢帧),全质量路径(导出)与上游对拍。这正是 ARCHITECTURE §6「两者共享同一个 RenderPlan,保证预览与导出像素一致」的落地。

---

## 6. 产出⑥:PoC 验收(与上游 inspect_timeline 像素 diff)+ 分步实施清单

### 6.1 PoC 场景(对拍 ROADMAP Phase 3 L30-31)

**最小场景**:单轨视频 + 一个 transform 关键帧 + 一条字幕,渲染指定帧。验证整条 Rust core 路线后再铺开。

### 6.2 验收方法:像素 diff

- **黄金参考**:用上游 PalmierPro(或其 `inspect_timeline` / `TimelineRenderer`)对**同一 Timeline JSON**、**同一帧号**、**同一 renderSize** 渲出 PNG(上游 MCP 工具 `inspect_timeline` / `get_timeline` 提供 timeline,导出帧用上游 TimelineRenderer L13 路径)。
- **被测**:OpenTake `build_render_plan → frame(f) → compositor → RT 回读 PNG`。
- **指标**:逐像素 RGBA 差,统计 **max Δ / 平均 Δ / PSNR / SSIM**。
- **容差**(分级,务实):
  - 几何(仿射/裁剪位置):**亚像素级**,边缘 ≤1px 偏移(采样滤波/半像素中心差异);max Δ 主要落在抗锯齿边缘。
  - 颜色:PoC 用 sRGB 非线性混合(§3.7),与 AVFoundation 平均 Δ 应很小(同 sRGB/709 标签);设阈 **平均 Δ ≤ 2/255、PSNR ≥ 40dB** 作为通过线(文字抗锯齿/编码舍入会贡献少量 Δ)。
  - 文字:字形光栅引擎不同(CoreText vs cosmic-text),边缘 Δ 不可避免;**对文字区域放宽**(SSIM ≥ 0.98 结构一致即可),几何位置/字号/对齐必须准。
- diff 工具:`opentake-render/tests/` 集成测试 + 离线脚本(image crate 读 PNG 比对)。**不依赖 GPU 的 plan 单测(§2.7)是第一道关**,GPU diff 是第二道关。

### 6.3 分步实施清单(严格顺序,每步可独立验证)

1. **RenderPlan 纯函数(无 GPU)** → 验证:§2.7 全部单测过;`affine_transform` 对拍上游 `affineTransform`(手算/已知点)。**这步不碰 wgpu,先锁算法正确性。**
2. **wgpu 设备 + 单 quad 纹理 pipeline** → 验证:渲一张静态图片纹理铺满画布,回读 PNG = 输入图(色彩/方向正确)。锁定 §3.3 投影方向 + §3.4 UV 翻转(用一张带方向标记的测试图)。
3. **affine + crop 接入**(单视频 clip,静态 transform/crop) → 验证:与上游单 clip 帧像素 diff(几何容差)。这步打通 §1.3/§1.4 → §3.3/§3.4。
4. **opacity + premultiplied 混合 + 黑底**(单 clip fade、双 clip 叠加) → 验证:fade 帧 alpha 正确;两轨叠加顺序 = 上游(§3.5/§3.6/§1.5)。
5. **transform 关键帧逐帧求值** → 验证:动画中间帧 affine = `transform_at(f)`;与上游动画帧 diff。
6. **文字纹理物化 + 叠加**(cosmic-text + tiny-skia) → 验证:单字幕静态帧结构 diff(文字容差);位置/字号/对齐/阴影对拍 §4.2。
7. **PoC 合场景**(单轨视频 + transform 关键帧 + 一条字幕,§6.1) → 验证:§6.2 像素 diff 整体通过。**PoC 通过 = 路线确认。**
8. **图片源 / Lottie 源**(§4.1 图片、§4.3 Lottie) → 验证:图片帧 = 上游静止视频对应帧;Lottie 帧光栅对齐。
9. **导出后端接线**(逐帧回读 → media 编码,§5.4) → 验证:导出短片与上游导出逐帧 diff(全质量路径)。
10. **预览快路径 `refreshVisuals`**(plan 复用 + 仅重算 frame,§5.4) → 验证:改 opacity/transform 不重建 plan,结果与重建一致。

> 与 ROADMAP 对齐:步 1-7 = Phase 3(命门 PoC);步 9 = Phase 6(导出);步 10 + 播放器(A/V 同步/seek/scrub 节流,VideoEngine L225-272)= Phase 4。文字引擎深化(描边/背景/换行精修)= Phase 8。

### 6.4 模块落点汇总(`crates/opentake-render/src/`,本规格不创建文件,仅规划)

```
lib.rs                 # 重导出 RenderPlan / build_render_plan / Compositor / RenderSize 等
plan/
  mod.rs  types.rs     # §2.2 数据结构
  build.rs             # §2.3 build_render_plan + §2.4 frame()
  affine.rs            # §2.6 affine_transform / compose / crop_to_uv(对拍上游 L599)
  tests.rs             # §2.7 纯函数单测(无 GPU)
gpu/
  device.rs            # wgpu 设备/队列
  compositor.rs        # §3.1 render(FramePlan, &textures) -> RT
  texture.rs           # §4.4 缓存(content-hash + LRU)
  text_raster.rs       # §4.2 cosmic-text + tiny-skia 文字光栅
  color.rs             # §3.7 sRGB<->linear
  shader.wgsl          # §3.3 顶点+片元
size.rs                # §5.2 even / export_render_size
source.rs              # §5.3 SourceMetrics / FrameProvider / DecodedFrame trait
tests/                 # §6.2 GPU 像素 diff 集成测试
```

---

## 7. 关键风险与对拍纪律(收尾)

- 🔴 **几何投影方向**(§1.3/§3.3/§3.4):y 翻转、半像素中心、CG 行向量左乘语义,任一处错都会整帧偏移。**步 1-3 用已知点单测 + 方向标记测试图锁死,绝不靠肉眼。**
- 🔴 **色彩空间**(§1.6/§3.7):PoC 先用 sRGB 非线性混合贴上游;线性光增强仅在 diff 通过后切换,且记录切换前后差异。
- 🟠 **文字字形差异**(§4.2/§6.2):CoreText vs cosmic-text 必有边缘 Δ,验收对文字区放宽到结构一致(SSIM),几何/字号/对齐必须严格对拍。
- 🟠 **变速源帧映射**(§2.5):`round(rel*speed)` + trim(图片 `max(0,...)`)逐条对拍 insertClip L301-343。
- ✅ **采样逻辑零重写**:所有关键帧/fade/dB 一律调 domain `*_at`(§0 铁律),render 只加几何投影 + GPU 合成。这把"忠实复刻编辑算法"的承诺锁在已单测的 domain 层,render 层只需对拍几何与像素。

**唯一真相源 = 上游 `CompositionBuilder.swift` + 已移植的 `opentake-domain`。每实现一个几何/调度函数,立即与对应上游行号 + domain 方法对拍,再进下一步。**
