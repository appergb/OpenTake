# 子系统：时间线模型（Timeline / Track / Clip / ClipType）

> 本模块目录：[INDEX.md](INDEX.md) · 总览：[OVERVIEW.md](OVERVIEW.md)

时间线的容器与片段值语义，以及片段上的全部派生帧换算与采样。是整个 crate 的核心，被所有上层当作权威 `Timeline` 的构件。

## 职责

- 定义三层容器：`Timeline`（fps/分辨率/轨道）→ `Track`（类型/静音/隐藏/sync-lock/片段）→ `Clip`（媒体引用 + 帧位 + trim/速度/音量/不透明度/变换/裁剪/淡变/六条关键帧轨/文本/进阶特效）。
- 提供片段的**派生计算**（不存储、按需算）：结束帧、源帧消耗、半开成员判定、各属性在某帧的采样值、源秒↔时间线帧换算。
- 定义片段类型判别 `ClipType` 及其轨道兼容规则与扩展名推断。

不做：编辑事务、引擎算法（覆盖/波纹）、UUID 生成（缺 id 解码为空串，由 project 层回填）。

## 关键类型与算法

源文件：[`timeline.rs`](../../../crates/opentake-domain/src/timeline.rs)、[`clip.rs`](../../../crates/opentake-domain/src/clip.rs)、[`clip_type.rs`](../../../crates/opentake-domain/src/clip_type.rs)

### `Timeline` / `Track` / `ClipLocation`（timeline.rs）
- `Timeline { fps, width, height, settings_configured, tracks }`，默认 `30 / 1920×1080`。`total_frames()` = 各轨 `end_frame()` 最大值（空为 0）。
- `Track { id, kind: ClipType, muted, hidden, sync_locked, clips }`，`sync_locked` 默认 `true`。`end_frame()` = 片段 `end_frame()` 最大值。
- `Track::contiguous_clip_ids(from_end, exclude_id)`：按 `start_frame` 升序走出从 `from_end` 起首尾相接的连续片段链（`start_frame == 运行链尾`才加入，遇到间隙即停），排除 `exclude_id`。供上层变速/波纹推后续紧邻片段用。
- `ClipLocation { track_index, clip_index }`：轨内定位（`Copy`，纯索引）。

### `Clip`（clip.rs）
- 帧域：`end_frame() = start_frame + duration_frames`；`contains(frame)` 为半开 `[start, end)`；`source_frames_consumed() = round(duration * speed)`；`source_duration_frames() = source_frames_consumed + trim_start + trim_end`。
- 源秒→时间线帧 `timeline_frame(source_seconds, fps)`：`source_frame = source_seconds*fps`；`offset = source_frame - trim_start`（须 ≥0）；`frame = round(start + offset / max(speed, 0.0001))`；落在 `[start, end)` 才返回 `Some`。speed 硬下限 `0.0001` 防除零。
- 采样族（绝对帧入参，内部转 clip 相对偏移 `frame - start_frame` 后采样对应关键帧轨，无轨则回退静态字段）：`opacity_at` / `raw_opacity_at` / `rotation_at` / `top_left_at` / `size_at` / `transform_at` / `crop_at` / `volume_at` / `raw_volume_at` / `live_volume_kf_db`。
- `fade_multiplier(frame)`：`rel = frame - start`，越界（`rel<0 || rel>duration`，**闭区间**）返回 0；`in = min(1, rel/fade_in)`、`out = min(1, (duration-rel)/fade_out)`，各按 `Interpolation::Smooth` 选 `smoothstep`，结果取 `min(in, out)`。`opacity_at` 对**视频**叠加它（音频不叠），`volume_at = volume * dB关键帧增益 * fade_multiplier`。
- `keyframe_frames(property)`：把某轨的 clip 相对偏移 + `start_frame` 还原为**绝对**时间线帧列表。
- 变更方法（皆夹取自身一致性）：`set_fade`/`set_fade_interpolation`/`clamp_fades_to_duration`、`set_duration`（连带 `clamp_keyframes_to_duration` + `clamp_fades_to_duration`）、`clamp_volume_kfs_to_duration`、`rescale_keyframes(scale)`（`round(frame*scale)`，非有限/非正 scale 为 no-op）。
- `VolumeScale`（dB↔线性，floor `-60`、ceiling `15`）与 `FadeEdge`（左/右）详见 [keyframe-transform.md](keyframe-transform.md)（dB 映射）与本表（fade）。

### `ClipType`（clip_type.rs）
- `Video / Audio / Image / Text / Lottie`，线上为小写名（`"video"`…）。`ALL` 保留声明顺序。
- `is_visual()`：除 `Audio` 外皆视觉类（占视频轨、贡献画布像素）。`is_compatible(other)`：相同，或双方都视觉。`from_file_extension(ext)`：小写扩展名→类型（未知返回 `None`，对应上游可失败 init）。默认 `Video`。

## 关键不变量与上游对齐点

- **半开帧区间**：`[start_frame, end_frame)` 贯穿全 crate；`fade_multiplier` 的越界判定是**闭区间** `[0, duration]`（与上游一致，注意与成员判定的半开不同）。
- **`round` 向偶**：`source_frames_consumed` / `timeline_frame` / `rescale_keyframes` 全用 `f64::round`（= Swift `.rounded()`，`.5` 远离零）。
- **speed 下限 `0.0001`**：仅在 `timeline_frame` 的除法处生效，防除零/Inf。
- **trim 是源帧偏移**：不要与时间线帧混用；源↔时间线换算一律经 `* / speed` + `round`。
- **serde 字节对齐**：`Clip` 多词键 camelCase（`mediaRef`/`startFrame`/`trimStartFrame`…）；`Track` 的 `type` 用 `#[serde(rename="type")]`；缺键回退默认、`None` 轨/可选字段 `skip_serializing_if` 不落盘（默认 clip 序列化不含 `opacityTrack`/`colorGrade` 等）。上游 `Track`/`Clip` 缺 `id` 会生成 UUID，本 crate 退为空串、由 project 层回填。

## 与其他子系统关系

- `Clip` 的关键帧轨、采样回退、`smoothstep` → [keyframe-transform.md](keyframe-transform.md)。
- `Clip` 的 `transform`/`crop` 字段类型 → [keyframe-transform.md](keyframe-transform.md)；`text_content`/`text_style` → [text-grade.md](text-grade.md)；`color_grade`/`chroma_key`/`masks`/`effects` → [split-subtitle.md](split-subtitle.md)（参考数学）与 [text-grade.md](text-grade.md)（调色）。
- `Clip` 的分割 → [split-subtitle.md](split-subtitle.md)（`split_clip`）。
- `Timeline` 被 [media-signal.md](media-signal.md) 的 `MediaResolver` 间接服务（媒体引用解析），并作为字幕/同步操作的入参（[split-subtitle.md](split-subtitle.md)）。

---

页脚：[INDEX.md](INDEX.md)
