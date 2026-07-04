# params — 生成参数：联合类型与从 `GenerationInput` 装配

> 上级：[opentake-gen 目录](INDEX.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
>
> 子系统级文档（不逐函数）。源码：[`params.rs`](../../../crates/opentake-gen/src/params.rs) + [`build_params.rs`](../../../crates/opentake-gen/src/build_params.rs)。完整规格见 [SPEC.md](SPEC.md) §1.2 / §5。

---

## 定位

两层：

1. **`params.rs` —— 发往后端的线上载荷**。`GenerationParams` 是按 `kind` 标签的联合类型，逐字段 1:1 复刻上游 `BackendGenerationParams`，JSON 形状与上游逐字符对齐。这是「线协议层」。
2. **`build_params.rs` —— 从领域输入装配载荷**。把持久化在工程文件里的 `GenerationInput`（住在 [`opentake-domain`](../opentake-domain/INDEX.md)）+ 已上传 URL 列表，按上游各 `*GenerationSubmission.buildParams` 闭包的**精确切分顺序**组装成 `GenerationParams`。方向单一：gen → domain 只读消费。

## `GenerationParams` 联合类型（线协议核心）

`#[serde(tag = "kind", rename_all = "lowercase")]`，四个变体：`Image` / `Video` / `Audio` / `Upscale`。`kind` 是内部判别字段，序列化出一个带顶层 `kind` 的扁平对象（等价上游 `singleValueContainer` 编码）。

线上字段口径的关键约定（务必保持，IPC/兼容性高频坑）：

- **全大写 URL 键**：`imageURLs` / `sourceVideoURL` / `startFrameURL` / `endFrameURL` / `referenceImageURLs` / `referenceVideoURLs` / `referenceAudioURLs` / `videoURL` / `sourceURL`——逐字照抄上游，不可改成驼峰 `Url`。
- **省略而非空**：`Option::None` 与空集合字段一律 `skip_serializing_if` 不写出（对齐上游 `encodeIfPresent` / `if !x.isEmpty`），读端不会看到 `null`/`[]`。

各变体要点：

| 变体 | 上游来源 | 关键字段 / 默认 |
|---|---|---|
| `ImageParams` | `ImageModelConfig.swift:3-25` | `prompt` / `aspect_ratio` / 可选 `resolution`/`quality` / `imageURLs` / `num_images`（构造时 `clamp(1,4)`） |
| `VideoParams` | `VideoModelConfig.swift:67-124` | 帧/参考各 URL（全大写键）+ `generate_audio`（`Default` 为 `true`，对齐上游 init） |
| `AudioParams` | `AudioModelConfig.swift:3-27` | `prompt` / 可选 `voice`/`lyrics`/`style_instructions`/`duration_seconds`/`videoURL` / `instrumental`（覆盖 TTS/音乐/音效） |
| `UpscaleParams` | `UpscaleModelConfig.swift:3-15` | `sourceURL` / `duration_seconds` |

`num_images` 通过 `clamp_num_images` 钳到 `1..=4`（上游 `max(1,min(4,n))`）。

> 注：本 crate 的 `GenerationParams` 是纯 `Serialize`（线上发出），与 [`opentake-ops`](../opentake-ops/INDEX.md) 的 `EditCommand`、`src-tauri` IPC 的 `EditRequest` 是**不同**的类型；它们各自的序列化口径互不影响。

## `build_params.rs` —— 装配与切分契约

输入：`&GenerationInput` + `uploaded: &[String]`（按固定顺序上传后回来的 URL）。各 `build_*` 函数：

| 函数 | 规则（复刻上游 `buildParams`） |
|---|---|
| `build_video_params` | 非编辑模型：上传扁平顺序 = **frames → image refs → video refs → audio refs**；切出 `frames[0]→startFrameURL`、`frames[1]→endFrameURL`，其余按计数切给三类参考 |
| `build_video_edit_params` | 编辑模型（`requires_source_video`）：`uploaded.first()→sourceVideoURL`，`dropFirst()→referenceImageURLs`，frames 全 nil |
| `build_image_params` | `imageURLs = uploaded`；`num_images` 钳 1..4 |
| `build_audio_params` | `videoURL` 优先取 `reference_video_urls.first()`，回退 `uploaded.first()`；`duration>0` 才写 `duration_seconds` |
| `build_upscale_params` | `sourceURL` 取 `uploaded.first()`，回退 `image_urls.first()` |
| `build_params` | 顶层分发：按 `ModelKind`；video 再按 `requires_source_video` 走编辑/简单两条路径 |

**「frames 在前、再 image、再 video、再 audio」的扁平上传顺序与切分是硬契约**（上游 `VideoGenerationSubmission.swift:289-305`）——顺序错位会把参考 URL 切到错误的槽位。`slice_video_uploads` 用 `take`/`skip` 严格实现这一切分。

简单视频路径下，各计数从 `GenerationInput` 的对应列表长度推导（frames 取 `image_urls` 槽位长度、三类参考取各自 `reference_*_urls` 长度）；编辑模型检测（`requires_source_video`）由调用方按所选模型的 caps 决定（见 [catalog.md](catalog.md) `VideoCaps`）。

## 与 `GenerationInput`（domain）的关系

`GenerationInput` 是生成的完整可序列化输入快照（prompt/model/duration/aspectRatio/各类 URL 与 assetId/音频专属字段/createdAt），随占位资产持久化进工程文件，是「重跑 Rerun」的唯一数据来源。它**定义在 `opentake-domain`**（`media.rs`，禁网络），装配逻辑放在本 crate（gen → domain 单向）。本 crate 通过 `lib.rs` re-export `GenerationInput` 供下游使用。

字段时间单位：`GenerationInput.duration` 面向模型/成本是「秒」；写回时间线才转帧（帧↔秒换算属下游职责，见移植铁律）。

## 对应上游 Swift

- `GenerationParams` ← `GenerationBackend.swift:95-110` + 四个 `*GenerationParams` Encodable 结构。
- `build_*` ← `VideoGenerationSubmission.swift` / `ImageGenerationSubmission.swift` / `AudioGenerationSubmission.swift` 的 `buildParams` 闭包。
- 编排层（上传顺序与切分契约、四个 `*Submission`）在 MODULE-PORT-MAP「Generation」段被标为 `direct-port`——「最有价值且平台无关的编辑逻辑」。

## 完成状态

- **已实现**：四变体的线协议序列化（含全大写 URL 键、省略空值）、`num_images` 钳制、五个 `build_*` 装配 + 顶层分发，逐项有单测核对线上形状与切分顺序。
- **计划中**：装配的上游全貌还有未复刻部分——引用预处理（视频裁剪段导出 `VideoTrimExtractor`、参考视频降采样 `VideoCompressor`）、上传缓存（TTL/内容哈希去重）、Rerun 参数复原（`EditSubmitter`）属编排层后续工作（ROADMAP Phase 9 / 进阶 AIGC 编排）。

## 源码

| 文件 | 内容 |
|---|---|
| [`params.rs`](../../../crates/opentake-gen/src/params.rs) | `GenerationParams` 联合 + `ImageParams`/`VideoParams`/`AudioParams`/`UpscaleParams` + `clamp_num_images` |
| [`build_params.rs`](../../../crates/opentake-gen/src/build_params.rs) | `build_video_params` / `build_video_edit_params` / `build_image_params` / `build_audio_params` / `build_upscale_params` / `build_params` + `slice_video_uploads` |
| [`../opentake-domain/src/media.rs`](../../../crates/opentake-domain/src/media.rs) | `GenerationInput`（领域侧输入快照，本 crate re-export） |

---

页脚：[opentake-gen 目录 INDEX.md](INDEX.md) · [模块文档树 ../INDEX.md](../INDEX.md) · [docs 总目录 ../../INDEX.md](../../INDEX.md)
