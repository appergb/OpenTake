# 子系统：分割、字幕与抠像/蒙版（split / subtitle / caption_sync / 像素数学）

> 本模块目录：[INDEX.md](INDEX.md) · 总览：[OVERVIEW.md](OVERVIEW.md)

落在 domain 的若干**纯模型不变量级别操作**：片段分割、字幕导出、caption-group 样式批量同步；外加 `grade.rs` 中抠像/蒙版/通用特效的**参考像素数学**（render 层 WGSL 镜像之）。它们虽是「编辑动作/像素效果」，但只读写值或纯数学，无副作用，故归 domain 而非 ops/render。

## 职责

- `split.rs`：在时间线帧处把一个 `Clip` 切成左右两半，trim 折算守恒、关键帧跨切口连续、淡变正确归属。
- `subtitle_export.rs`：把 caption 片段序列化为 SubRip（`.srt`）/ WebVTT（`.vtt`）字符串（零 IO）。
- `caption_sync.rs`：把一个 `TextStyle` 不可变地批量应用到某 caption 组的所有片段，并提供组枚举/成员查询。
- `grade.rs`（抠像/蒙版/特效部分）：`ChromaKey` / `Mask` / `Effect` 的值类型与可单测的参考像素数学。

不做：覆盖/波纹让位（ops）、字幕的转写生成与导出文件落盘（media / project）、着色器执行（render）。

## 关键类型与算法

源文件：[`split.rs`](../../../crates/opentake-domain/src/split.rs)、[`subtitle_export.rs`](../../../crates/opentake-domain/src/subtitle_export.rs)、[`caption_sync.rs`](../../../crates/opentake-domain/src/caption_sync.rs)、[`grade.rs`](../../../crates/opentake-domain/src/grade.rs)（抠像/蒙版/特效）

### 片段分割（split.rs）
- `split_clip(clip, at_frame, right_id) -> Option<(Clip, Clip)>`，取自上游 `EditorViewModel.splitSingleClip`。
- 半开守卫：仅 `start_frame < at_frame < end_frame` 才切，端点不切（返回 `None`）。
- trim 折算守恒：`split_offset = at_frame - start`；`left_source = round(split_offset * speed)`、`right_source = round((duration - split_offset) * speed)`。左半 `duration = split_offset`、`trim_end += right_source`、清 `fade_out`；右半新 `id`、`start = at_frame`、`duration = 原 - split_offset`、`trim_start += left_source`、清 `fade_in`；两半都 `clamp_fades_to_duration`。butt-join 后两半引用的源跨度与原片段相同（有单测验证）。
- 关键帧连续：六条轨各经 `split_keyframe_track`（见 [keyframe-transform.md](keyframe-transform.md)）在切点插边界关键帧、右半重定基到 0，fallback 与上游一致（position `(0,0)`、scale `(1,1)`、rotation 0、opacity/volume/crop 取静态值）。
- id 由调用方传入（上游 stamp 新 UUID；domain 无 `uuid` 依赖，左半保留原 id）。

### 字幕导出（subtitle_export.rs，ROADMAP Phase 8 «#110 已落地»）
- caption 片段判定：同时具备 `caption_group_id` 且 `text_content` 非空白。
- `collect_caption_cues(timeline)`：跨所有轨收集，按 `start_frame` 升序（同帧按 `id` 稳定 tie-break），从 1 编号，空/纯空白文本跳过；返回 `SubtitleCue { index, start_frame, end_frame, text }`（end 为 `clip.end_frame()`，半开）。
- `export_srt` / `export_vtt`：帧→毫秒用 `(frame*1000)/fps`，**`fps` 下限 1** 防除零；SRT 时间戳 `HH:MM:SS,mmm`（逗号）、VTT `HH:MM:SS.mmm`（点）；VTT 始终以 `WEBVTT\n\n` 开头。零 IO，仅返回字符串。

### caption-group 样式同步（caption_sync.rs，不可变）
- `caption_group_ids(timeline)`：去重、按 track→clip 首见顺序返回全部 `caption_group_id`。
- `clips_in_group(timeline, group_id)`：返回该组全部片段的借用引用（storage 顺序）。
- `sync_caption_group_style(timeline, group_id, style)`：返回**新 `Timeline`**（深克隆），把目标组每个片段的 `text_style` 换成 `style` 克隆，其余一概不动；未知组/空时间线/旧工程（无 caption 字段）为值相等 no-op，跨组不串样式。成员判定纯看 `caption_group_id`（与 `text_content` 是否为空无关——这里是restyle 不是导出）。

### 抠像/蒙版/特效参考数学（grade.rs）
- 共享助手：`luma709(r,g,b)`（BT.709 相对亮度）、`smoothstep01(edge0,edge1,x)`（**会 clamp** 的边缘 feather ramp，区别于 `keyframe::smoothstep`）、`chroma_cb_cr(r,g,b)`（按亮度归一的色度向量，使抠像 luma 无关）。
- `ChromaKey { key_color, similarity, smoothness, spill }`：默认键纯绿。`alpha(r,g,b)`：在 `(cb,cr)` 平面算到 key 的色度距离，经 `similarity`/`smoothness` 映射为 matte alpha（1=保留、0=抠掉），luma 无关（同色相明暗都被抠）；`suppress_spill(r,g,b)`：把主键通道压向其余两通道均值去溢色。
- `Mask { shape, feather, invert }`：`MaskShape::{ Linear{point,normal}, Circle{center,radius}, Poly{points} }`（标签 `kind`，`Point2` 归一化点）。默认是覆盖全画布的大圆（避免误隐藏）。`signed_distance(x,y)`（内负外正 SDF；多边形为偶奇判定 + 最近边距近似）→ `coverage(x,y)`（`[0,1]`，按 `feather` 在边界 smoothstep，`invert` 翻转）。
- `Effect { name, params: BTreeMap<String,f64>, enabled }`：通用命名特效链的**稳定序列化契约**（每个 = 一个 wgpu pass）。`new`/`with_param`/`param`。具体特效的 WGSL 在 render 层增量实现，未知 `name` 被忽略。

## 关键不变量与上游对齐点

- **分割守恒**：`round(offset*speed)` 折进 trim 使源跨度守恒；端点不切（半开）；关键帧切点插边界保连续；左 id 留、右 id 由调用方给——逐条对齐上游 `splitSingleClip`/`splitKeyframeTrack`。
- **`round` 向偶**：分割 trim 折算用 `f64::round`（`.5` 远离零）。
- **字幕 `fps` 下限 1**：`fps==0` 不得 panic；SRT/VTT 分隔符（逗号/点）与 `WEBVTT` 头照标准。
- **不可变操作**：`sync_caption_group_style` 不原地改，返回深克隆新 `Timeline`；输入永不被改（与 crate 不可变约定一致）。
- **两个 smoothstep 区分**：feather 用 `smoothstep01`（clamp），关键帧/淡变用 `keyframe::smoothstep`（不 clamp）。
- **恒等默认**：`ChromaKey::default()` 算合理 matte 但仅在片段上设置时才「激活」（render 层把字段 `None` 视为关闭）；`Mask::default()` 覆盖全画布；`Effect` 默认 `enabled`。
- **serde**：`Mask` 用 `#[serde(tag="kind")]` 标签 enum；`Effect`/`ChromaKey`/`Mask` camelCase（`keyColor`/`feather`…），都 `#[serde(default)]` + `Option`/`Vec`，老工程缺这些键照样解码（默认 clip 序列化不含这些字段）。

## 与其他子系统关系

- 全部操作 input 是 [timeline-model.md](timeline-model.md) 的 `Timeline`/`Clip`；`split_clip` 复用 [keyframe-transform.md](keyframe-transform.md) 的 `split_keyframe_track`。
- caption 同步的目标值 `TextStyle`、抠像/蒙版同文件的调色 `ColorGrade` 见 [text-grade.md](text-grade.md)；`ChromaKey`/`Mask`/`Effect` 是 `Clip` 的进阶特效字段，与 `ColorGrade` 并列。
- 字幕导出/抠像蒙版/特效对应 [`../../architecture/ADVANCED-FEATURES.md`](../../architecture/ADVANCED-FEATURES.md) 的 D 层 / A 层；接导出层与 command/着色器/UI 属上层后续（见 [OVERVIEW.md](OVERVIEW.md) 完成状态）。

---

页脚：[INDEX.md](INDEX.md)
