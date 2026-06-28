# opentake-domain — 总览

> 模块目录：[INDEX.md](INDEX.md) · 上级：[模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)

---

## 一句话定位

OpenTake 的**值语义领域层**：把 Palmier Pro 上游 `Models/` 的 `Timeline / Track / Clip / Keyframe / Transform / Crop / TextStyle / Media*` 等纯数据类型 1:1 复刻成 Rust `struct/enum`，连同它们的**派生计算与采样算法**（`end_frame` / `source_frames_consumed` / 各属性 `*_at(frame)` / `fade_multiplier` / 关键帧 `sample` / dB↔线性 / 分割 / 调色·抠像·蒙版的参考像素数学），全部是零 IO、纯逻辑、可单测的值。

## 依赖分层位置

```
opentake-domain        ← 你在这里（值语义叶子层）
   ▲
opentake-ops           纯编辑引擎 + EditCommand + 撤销栈
   ▲
project / render / media / motion / agent / gen
   ▲
opentake-core / src-tauri / web
```

- **依赖谁**：仅依赖 `serde`（+ `std::collections` / `std::path`）。**叶子 crate，禁止 `std::fs` 与网络**。
- **被谁依赖**：被 `opentake-ops` 及其上所有层依赖。上层把本 crate 的类型当作权威 `Timeline` 的构件，全部编辑都作用在这些值上。
- 权威 `Timeline` 由 `opentake-core` 的 `EditorState` 持有（见 [`../opentake-core/SPEC.md`](../opentake-core/SPEC.md)），其结构 `use opentake_domain::Timeline`；本 crate 只定义值与不变量，不持有会话状态、不做事务。

## 职责边界

**做：**
- 定义整套领域值类型 + serde 序列化模型（线上格式与上游 `JSONEncoder` 字节对齐，老工程可往返）。
- 提供所有**平台无关的派生函数与采样算法**：帧换算、关键帧插值、淡变包络、音量 dB 映射、变换/裁剪几何、调色/抠像/蒙版的参考像素数学。
- 提供**纯模型不变量级别**的操作：clip 分割（`split_clip`）、关键帧轨分割（`split_keyframe_track`）、字幕导出（SRT/VTT）、caption-group 样式批量同步。这些虽是「编辑动作」，但只读写 `Clip` 字段、属于模型不变量，故落在 domain 而非 ops。

**不做：**
- 不做编辑事务 / 撤销重做 / 命令路由（→ `opentake-ops` + `opentake-core`）。
- 不做覆盖/波纹/吸附等**引擎级**编辑算法（→ `opentake-ops`）。
- 不做文件读写、媒体探测、缩略图/波形/转写（→ `opentake-project` / `opentake-media`）。
- 不做像素栅格化（wgpu 合成、文本栅格化）；本 crate 只给出参考像素数学，GPU 侧 WGSL 镜像之（→ `opentake-render`）。
- 不生成 UUID：缺 `id` 解码为空串，由 `opentake-project` 回填（domain 无 `uuid` 依赖）；分割右半 id 由调用方传入。

## 关键概念与数据流

- **整数帧 + 半开区间**：所有时间以 `i32` 帧表达；clip 占据 `[start_frame, start_frame + duration_frames)`。`Timeline.total_frames` = 各轨 `end_frame` 最大值。
- **trim 为源帧偏移**：`trim_start_frame` / `trim_end_frame` 是源媒体偏移（不是时间线偏移）。`source_frames_consumed = round(duration * speed)`，`source_duration_frames = source_frames_consumed + trim_start + trim_end`。
- **关键帧 clip 相对存储**：六条 `KeyframeTrack`（opacity / position / scale / rotation / crop / volume）的 `frame` 存的是 **clip 相对偏移**；公开 API（如 `keyframe_frames`）暴露绝对时间线帧（偏移 + `start_frame`）。
- **采样链**：`Clip::*_at(frame)` 把绝对帧转 clip 相对偏移 → `KeyframeTrack::sample`（端点 clamp、无外插、插值类型取**左端**关键帧的 `interpolation_out`）→ 叠加淡变 / dB→线性。
- **serde 容错**：模型字段普遍 `#[serde(default)]` + `Option<T>`，缺键回退默认（`Transform` 旧 `x/y`→中心迁移、`MediaManifest.version` 缺省回退 1 由自定义 `Deserialize` 处理），保证读旧工程不破坏。
- **典型流向**：上层（ops/core）持有 `Timeline` 值 → 调用 domain 的派生函数采样（渲染取每帧属性、ops 算让位）→ 修改后整树 `Clone` 快照入撤销栈。domain 自身不感知这些上层动作。

## 对应上游 Swift 模块

核对自 [`../../architecture/MODULE-PORT-MAP.md`](../../architecture/MODULE-PORT-MAP.md)「Models」段（verdict：高保真直译，风险集中在媒体探测与文字度量两处）：

| 本 crate 文件 | 上游 Swift（`Sources/PalmierPro/Models/`） |
|---|---|
| `timeline.rs` / `clip.rs` / `clip_type.rs` / `transform.rs` | `Timeline.swift`、`ClipType.swift` |
| `keyframe.rs` | `Keyframe.swift`（+ `split_keyframe_track` 取自上游 `EditorViewModel.splitKeyframeTrack`） |
| `split.rs` | 取自上游 `EditorViewModel.splitSingleClip`（模型不变量，下沉到 domain） |
| `text.rs` | `TextStyle.swift`（数据 + hex 解析）+ `TextLayout.swift`（**近似**，见下） |
| `media.rs` | `MediaManifest.swift`、`MediaFolder.swift`、`MediaResolver.swift`、`MediaAsset.swift`（数据/派生部分） |
| `grade.rs` / `subtitle_export.rs` / `caption_sync.rs` / `signal.rs` | **非上游移植**：进阶能力（`ADVANCED-FEATURES.md` A/D 层）与 Agent Context Signal（`AGENT-CONTEXT-SIGNAL.md`）的新增领域类型 |

被有意省略的上游成分：AppKit/CoreText/AVFoundation 等平台相关辅助（`NSColor`/`swiftUIColor`、`resolvedFont`、`loadMetadata`、SF Symbol 名、CATextLayer 对齐）——属纯 UI / 媒体探测，在前端或 render/media 层重建。

## 完成状态：已实现 vs 计划中

对照 [`../../architecture/ROADMAP.md`](../../architecture/ROADMAP.md)、[`../../architecture/PORT-1TO1-GAP.md`](../../architecture/PORT-1TO1-GAP.md) 与实际代码（每个源文件均带 `#[cfg(test)]`，覆盖序列化往返与边界）：

**已实现（代码存在且有单测）：**
- Timeline/Track/Clip/ClipType 全字段 + 派生（`end_frame` / `source_frames_consumed` / `contains` / `timeline_frame` / `contiguous_clip_ids`）。
- 六条关键帧轨：`KeyframeTrack`（`upsert`/`remove`/`move_keyframe`/`sample`）、`smoothstep`、`split_keyframe_track`；Transform/Crop 几何与吸附、旧 `x/y` 迁移；`VolumeScale` dB↔线性。
- `Clip` 采样族：`opacity_at` / `raw_opacity_at` / `rotation_at` / `top_left_at` / `size_at` / `transform_at` / `crop_at` / `volume_at` / `fade_multiplier` / `keyframe_frames`，以及 `clamp_*` / `rescale_keyframes` / `set_*` 变更。
- `split_clip`（trim 折算 + 关键帧边界连续 + 淡变归属）。
- 进阶像素数学参考实现：`ColorGrade`（线性光链）、`ChromaKey`（luma 无关抠像 + spill 抑制）、`Mask`（线性/圆形/多边形 SDF + feather）、`Effect`（通用命名特效链占位契约）。
- 字幕导出 `export_srt` / `export_vtt` / `collect_caption_cues`（ROADMAP Phase 8「SRT/VTT 导出纯逻辑已落地 #110」）。
- caption-group 样式批量同步 `sync_caption_group_style` / `caption_group_ids` / `clips_in_group`。
- Media 值模型 `MediaManifest`（version 回退）/ `MediaManifestEntry` / `MediaSource` / `MediaFolder` / `MediaResolver`（仅算期望路径）/ `MediaAsset`（派生 + manifest 互转）/ `GenerationInput` / `GenerationStatus`。
- Agent Context Signal 类型 `ContextSignal` / `VideoType` / `TrackRole` / `EditingStage` / `StageGuidance` / `EditingSkeleton` / `TrackHint`（ROADMAP Phase S 第 1 步「在 domain 定义类型」）。

**计划中 / 部分（本 crate 仅占其一环，余下在上层）：**
- `TextLayout::natural_size` 是**近似实现**：用固定字符步进估算宽度，复刻了 canvas-scale 基准（`canvas_height/1080`）、阴影 padding（`12*2`）与 `+4` 余量的**公式形状**，但**宽度不与上游 CoreText 像素一致**，必须由 render 层文本引擎（cosmic-text）重算以求像素对齐（见 `MODULE-PORT-MAP.md` 文字度量 needs-replacement）。
- Context Signal 的**检测/填充**逻辑不在本 crate（Phase B–D，落在 agent 层）；domain 只定义形状 + serde。
- `Effect` 是稳定的序列化契约，**具体特效**（blur/glow/sharpen…）的 WGSL pass 在 render 层增量实现。
- 调色/抠像/蒙版的 domain 值与参考数学已就位，但**着色器接入、command（`SetColorGrade`/`SetChromaKey`/`SetMask`/`SetEffects`）与 UI** 属 ROADMAP Phase 3/A 层后续。

## 适用的移植铁律

本 crate 是移植铁律最密集的地方，改动务必遵守：

1. **一切整数帧**：时间用 `i32` 帧，半开区间 `[start, start+dur)`。
2. **`secondsToFrame` 截断**：源秒→帧用 `Int(s*fps)` 截断，不是四舍五入（本 crate 的 `timeline_frame` 走源帧路径，注意区分）。
3. **`f64::round` 向偶**：所有 `round()` 与 Swift `.rounded()` 一致 = 就近、`.5` 远离零（`source_frames_consumed`、`rescale_keyframes`、`split_clip` 的 trim 折算均如此）。
4. **关键帧 clip 相对存储**：轨内存相对偏移，公开 API 用绝对帧；分割时在切点插边界关键帧保持曲线连续。
5. **`smoothstep(t) = t*t*(3-2t)`**：关键帧/淡变用此式且**不 clamp**（`keyframe::smoothstep`）；feather 用的是另一个会 clamp 的 `grade::smoothstep01`，两者不要混用。
6. **serde `default` + `Option`**：所有模型加 `#[serde(default)]` + `Option<T>`/`Vec`，缺键回退默认；复杂迁移（Transform 旧 `x/y`、Manifest version）用自定义 `Deserialize`。线上多词键须与上游 `JSONEncoder` 字节对齐（camelCase，但缩写casing 如 `imageURLs`/`sourceFPS` 用显式 `rename`）。

---

页脚：[本模块目录 INDEX.md](INDEX.md) · [模块文档树 ../INDEX.md](../INDEX.md) · [docs 总目录 ../../INDEX.md](../../INDEX.md)
