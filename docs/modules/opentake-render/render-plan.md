# RenderPlan — 纯函数 Timeline → 每帧属性

> 上级：本模块目录 [INDEX.md](INDEX.md)

## 职责

把权威 `Timeline` 折算成可逐帧合成的计划，**零 IO、可全单测、与 wgpu 解耦**。这是上游 `CompositionBuilder.buildVisuals` 的移植，但输出的是「每帧/每层属性值」而非 AVFoundation ramp 指令——因为 OpenTake 是逐帧拉取架构（预览要 seek 任意帧、导出逐帧推进），而非声明式合成。

分两层，与上游「静态 `trackMappings` + 动态 `buildVisuals`」二分同构：
1. **`RenderPlan`（帧无关）**：解析一次 Timeline，固化哪些轨/哪些 clip（已去重排序）、每 clip 的源类型 + `nat_size` + `preferred_transform` + 混合顺序 + 画布尺寸 + fps + 总帧数。
2. **`FramePlan = RenderPlan::frame(timeline, f)`（瞬时）**：对单帧产出有序 `Vec<LayerDraw>`，逐 clip 调 domain 的 `*_at` 求值。

## 关键类型与算法

### 数据结构（`plan/types.rs`）
- `RenderSize { width, height }`：画布像素尺寸（已偶数化）。
- `TextureSource`：`Decoded{media_ref}` / `Image{media_ref}` / `Lottie{media_ref}` / `Text{clip_id}`——纹理来源标签（物化策略见各执行侧）。
- `ClipPlan`：单 clip 的静态描述。除几何字段（`start/end_frame`、`nat_size`、`preferred_transform`、`needs_premultiply`、`speed`、`trim_start_frame`）外，还携带 `clip_index`（省去 `frame()` 里按 id 查找）与**进阶像素效果输入** `color_grade` / `chroma_key` / `masks` / `effects`（本轮帧无关，建 plan 时从 `Clip` 原样拷，恒等 grade 会被 `filter` 掉）。
- `RenderPlan`：`clip_plans`（视频层，已去重 + 排序）+ `text_plans`（文字层，恒叠在视频之上、**不去重**）。
- `LayerDraw<'a>`：单帧单层一次 draw——`affine[6]` + `nat_size` + `crop_uv` + `opacity` + `needs_premultiply` + 借用的 `color_grade/chroma_key/masks/effects`。`nat_size` **必须**是构建 affine 时所用的源自然尺寸（非解码纹理分辨率，见不变量）。
- `FramePlan<'a>`：`clear_rgba`（恒 `[0,0,0,1]`）+ 已按混合序排好的 `draws`。

### `build_render_plan`（`plan/build.rs`）
对拍 `CompositionBuilder.build` L53-216 + `buildVisuals` 可见 clip 选择 L405-445：
1. 遍历 `timeline.tracks`（保持顺序），跳过 `track.hidden`。
2. 每轨按 `start_frame` 升序（排序的是**索引**以保留 `clip_index`）。
3. **文字 clip**：不做同轨去重、不受音频门控，只要 `duration_frames > 0` 即收进 `text_plans`（上游文字走独立 CATextLayer 路径）。
4. **音频轨**：不产生任何视频 ClipPlan（音频混合在别处）。
5. **视频轨去重**：`duration_frames > 0 && start_frame >= prev_end_frame` 才入选（上游 L152/L424 的重叠剔除），入选后 `prev_end_frame = end_frame`。
6. 每个入选 clip 经 `make_clip_plan` 构造：`texture_source_for` 选源；非文字源经 `normalize_box` 算 `nat_size` 与 re-origin 的 `preferred_transform`（上游 L166-172：`nat = |bbox(nat0, pt)|`，`pt` 末尾平移到 box 原点归零）；文字源 `nat_size` = 文字框像素、`preferred_transform` = 单位阵。
7. 最终按 `(track_index, start_frame)` 排序，**下标越大越靠上**（上游 video track 0 最顶 → 高索引先画、低索引后画）。

### `source_frame_index`（`plan/build.rs`）
对拍 `insertClip` 的 trim+speed L301-343：`rel = f - start_frame`，`trim = (Image ? max(0,trim) : trim)`，`src = trim + round(rel*speed)`。`Image`/`Text` 恒 0；`Lottie` 取模 `lottie_frame_count`（未知则下钳 0 无环绕）；`Decoded` 交解码器映射 PTS。`round` = `f64::round()`（half-away-from-zero）。

### `RenderPlan::frame`（`plan/build.rs`）
逐 `clip_plans` 再逐 `text_plans`（保证文字在视频之上）：经 `clip_for` 取 `&Clip`（按存储索引，漂移则回退 id 查找）→ `eval_layer`：
- 命中测试 `f ∈ [start,end)`，否则跳过（等价上游区间外 opacity 0）。
- `opacity = clip.opacity_at(f)`，`opacity <= 0` 跳过（行为等价优化）。
- **transform 静态/动画分流**（复刻上游 `emitTransform` L631-632）：无动画用 `clip.transform`（带 flip 标志），有动画用 `clip.transform_at(f)`（重建 top-left/size/rotation 并**有意丢弃 flip**，与 domain `transform_at` 一致）。
- `affine = compose(preferred_transform, affine_transform(transform, nat_size, render_size))`。
- `crop_uv = crop_to_uv(clip.crop_at(f))`，`source_frame = source_frame_index(plan, f)`。

## 源文件
- [`crates/opentake-render/src/plan/types.rs`](../../../crates/opentake-render/src/plan/types.rs) — 数据结构。
- [`crates/opentake-render/src/plan/build.rs`](../../../crates/opentake-render/src/plan/build.rs) — `build_render_plan` / `frame` / `source_frame_index` / `make_clip_plan` / `normalize_box`。
- [`crates/opentake-render/src/plan/affine.rs`](../../../crates/opentake-render/src/plan/affine.rs) — 几何投影（见下「几何投影」节）。
- [`crates/opentake-render/src/plan/tests.rs`](../../../crates/opentake-render/src/plan/tests.rs) — 无 GPU 纯函数单测。
- [`crates/opentake-render/src/plan/mod.rs`](../../../crates/opentake-render/src/plan/mod.rs) — re-export。

## 几何投影（`plan/affine.rs`，render 层独有）

这是 render 层在 domain 之上**唯一新增**的数学——AVFoundation 替上游做掉、domain（正确地）不承担的部分：
- `affine_transform(t, nat, rs)`：归一化画布 `Transform`（0–1）→ AVFoundation layer 期望的像素仿射，逐行照搬上游 `affineTransform` L599-614（`sx/sy` 含 flip 取负、`tx/ty` flip 偏移、rotation `translate(-c)∘rotate(θ)∘translate(c)`、`*π/180`）。
- `compose(a, b)`：CG `a.concatenating(b) = a·b`（行向量 `p' = p·a·b`，先 a 后 b），存储行优先 `[a,b,c,d,tx,ty]` 与 `CGAffineTransform` 字段 1:1。
- `crop_to_uv(c)`：源 crop inset（0–1，原点左上）→ 纹理 UV 子矩形 `(u0,v0,u1,v1)`，钳到 `[0,1]` 且保证 `u0≤u1`/`v0≤v1`（镜像上游 `max(1,…)` 一源像素下限，防退化采样）。**v 翻转不在此发生**，统一在着色器一次性翻。

## 不变量
- **采样零重写**：opacity/transform/crop 一律调 domain 的 `*_at`；render 层只投影几何，绝不重实现关键帧/fade。
- **`LayerDraw.nat_size` = 构建 affine 时的源自然尺寸**，不是解码纹理的真实分辨率。预览按降档 `max_size` 解码，用纹理尺寸当代理会与 affine 失配、把图层缩进左下角并随纹理尺寸抖动（#125）。
- **混合顺序确定**：`clip_plans` 按 `(track_index, start_frame)`、下标大者在上；`text_plans` 整体在视频之上。同轨视频已无重叠；文字不去重。
- **黑底是 clear color 不是 clip**：`FramePlan.clear_rgba` 恒 `[0,0,0,1]`。
- **`RenderPlan` 与 `Timeline` 同源配对**：`frame()` 需回看同一棵 timeline 取 `Clip`（共享 `clip_index`），二者不可变同寿命使用。

## 关系
- 输入来自 [opentake-domain](../opentake-domain/INDEX.md) 的 `Timeline/Clip/Transform/Crop/ColorGrade/ChromaKey/Mask` 与全部 `*_at` 采样。
- `nat_size` / `preferred_transform` / `needs_premultiply` / `lottie_frame_count` 经 `SourceMetrics` 查询（见 [source-size.md](source-size.md)）。
- 输出的 `FramePlan` 喂给 [gpu-compositor.md](gpu-compositor.md) 的 `render_to_rgba`；其中进阶效果字段在片元着色器消费。

---

> 上级：本模块目录 [INDEX.md](INDEX.md)
