# 子系统：关键帧动画与几何变换（Keyframe / Transform / Crop）

> 本模块目录：[INDEX.md](INDEX.md) · 总览：[OVERVIEW.md](OVERVIEW.md)

片段的动画系统（六条关键帧轨的存储与采样）与几何变换（位置/缩放/旋转/裁剪等仿射属性 + 画布吸附）。

## 职责

- 定义关键帧值与轨道容器，提供有序插入/移动/采样与轨道分割。
- 定义可动画属性的判别 `AnimatableProperty`，以及 `f64`/`AnimPair`/`Crop` 的线性插值实现。
- 定义片段的几何变换 `Transform`（归一化画布坐标）与边缘裁剪 `Crop`，含画布边界/中心吸附与旧坐标键迁移。

不做：把关键帧写进哪个片段、stamp 时机等编辑决策（在 `opentake-ops`）；本 crate 只给值、采样与分割不变量。

## 关键类型与算法

源文件：[`keyframe.rs`](../../../crates/opentake-domain/src/keyframe.rs)、[`transform.rs`](../../../crates/opentake-domain/src/transform.rs)

### 关键帧（keyframe.rs）
- `Interpolation { Linear, Hold, Smooth }`，小写线上名。`Keyframe<V> { frame, value, interpolation_out }`，`interpolation_out` 默认 `Smooth`（缺键也回退 Smooth）。`frame` 是 **clip 相对偏移**。
- `AnimPair { a, b }`：双分量值，承载 position `(x, y)` 与 scale `(w, h)`。`KeyframeInterpolatable` 由 `f64` / `AnimPair` / `Crop` 实现（线性 `a + (b-a)*t`，`Crop` 逐分量）。
- `smoothstep(t) = t*t*(3 - 2t)`，**不 clamp**（调用方传已归一化 `t`）。
- `KeyframeTrack<V> { keyframes }`：`is_active()` = 非空；`upsert` 保持按 `frame` 升序、同帧替换；`remove(frame)`；`move_keyframe(old, new)`（源缺失则 no-op，目标被占则**放弃**，对齐上游 `move(from:to:)`）。
- `sample(frame, fallback)`（核心采样）：空→fallback；单帧→该值；`frame ≤ 首帧`→首值、`frame ≥ 末帧`→末值（**端点 clamp、无外插**）；区间内取首个 `frame > 目标` 的为右端 `b`、前一个为左端 `a`，`raw = (frame-a.frame)/(b.frame-a.frame)`，**按左端 `a.interpolation_out`** 选：`Hold→a值`、`Linear→线性`、`Smooth→smoothstep(raw)`。
- `split_keyframe_track(track, split_offset, fallback)`：在 clip 相对 `split_offset` 处切轨，两侧各插一个在切点采样出的**边界关键帧**保持曲线连续；右半 `frame` 减去 `split_offset` 重定基到 0；空/inactive 轨原样返回两侧。模型不变量取自上游 `EditorViewModel.splitKeyframeTrack`，被 [`split.rs`](../../../crates/opentake-domain/src/split.rs) 调用（见 [split-subtitle.md](split-subtitle.md)）。
- `AnimatableProperty { Opacity, Position, Scale, Rotation, Crop, Volume }`：六条轨的判别（`display_name` 等纯 UI 省略）。

### 变换与裁剪（transform.rs）
- `Point { x, y }`：归一化画布点（0–1）。
- `Transform { center_x, center_y, width, height, rotation, flip_horizontal, flip_vertical }`：归一化画布坐标，默认居中满画布（中心 `0.5,0.5`、尺寸 `1.0`、旋转 0、不翻转）；`rotation` 为度、正=顺时针。构造：`from_top_left` / `from_center`；查询：`top_left()` / `center()`。
- 吸附：`snap_to_boundary(value, threshold)`（贴 0/1）；`snap_to_canvas_edges(threshold)`（保尺寸贴画布边）；`snap_center_to_canvas_center(th_h, th_v)`（贴中心 0.5，返回 `(snapped_x, snapped_y)` 供画辅助线）。
- **旧坐标迁移**（自定义 `Deserialize`）：兼容上游旧 `x`/`y` 键，`center_x = old_x + width - 0.5`（y 同理）；现代 `centerX`/`centerY` 优先；全缺回退默认。序列化只输出现代 camelCase 键，不回吐旧键。
- `Crop { left, top, right, bottom }`：归一化边缘内缩，默认全 0（恒等）。`is_identity()`、`visible_width_fraction()` / `visible_height_fraction()`（`(1 - 两侧内缩).max(0)`，过裁夹 0）。`Crop` 实现 `KeyframeInterpolatable`，可走 `crop_track`。
- `CropAspectLock { Free, Original, R16x9, R9x16, R1x1, R4x3, R3x4, R21x9 }`：裁剪比锁，`pixel_aspect()` 返回数值比（Free/Original 为 `None`）；`label` 等纯 UI 省略。

## 关键不变量与上游对齐点

- **clip 相对存储**：轨内 `frame` 一律相对偏移；公开绝对帧由 `Clip::keyframe_frames` 还原（偏移 + `start_frame`）。
- **采样语义**：端点 clamp、无外插、插值类型取**左端**关键帧的 `interpolation_out` —— 三点逐一对齐上游 `KeyframeTrack.sample`。
- **`smoothstep` 不 clamp**：与 [`grade.rs`](../../../crates/opentake-domain/src/grade.rs) 的 `smoothstep01`（会 clamp，用于 feather）是两个不同函数，切勿互换。
- **`move_keyframe` 目标占用即放弃**：不是覆盖、不是顺延。
- **`Transform` 旧 `x/y` 迁移公式照搬**：`center = old + size - 0.5`，且现代键优先；这是老工程能打开的关键。
- **serde**：`Keyframe`/`KeyframeTrack` camelCase（`interpolationOut`）；`Transform`/`Crop` camelCase（`centerX`/`flipHorizontal`…），缺键回退默认。

## 与其他子系统关系

- 被 [timeline-model.md](timeline-model.md) 的 `Clip` 持有（六条 `Option<KeyframeTrack<…>>` + `transform` + `crop`），并由 `Clip::*_at` 采样族驱动。
- `split_keyframe_track` 被 [split-subtitle.md](split-subtitle.md) 的 `split_clip` 用来保持跨切口曲线连续。
- 淡变也复用 `smoothstep`（`Clip::fade_multiplier`，见 [timeline-model.md](timeline-model.md)）。

---

页脚：[INDEX.md](INDEX.md)
