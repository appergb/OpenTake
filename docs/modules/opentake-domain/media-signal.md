# 子系统：媒体资产与上下文信号（Media* / ContextSignal）

> 本模块目录：[INDEX.md](INDEX.md) · 总览：[OVERVIEW.md](OVERVIEW.md)

媒体库的值模型（清单/资产/来源/文件夹/解析器）与生成参数快照，以及面向 AI Agent 的上下文信号领域类型。两组都是纯数据 + serde；前者 1:1 移植上游，后者是新增。

## 职责

- 定义可序列化媒体清单 `MediaManifest` 及条目/来源/文件夹，提供**仅算期望路径**的 `MediaResolver`（零 IO）。
- 定义运行期 `MediaAsset`（数据 + 派生）与 manifest 互转，及 AI 生成输入快照 `GenerationInput` / 生成状态 `GenerationStatus`。
- 定义 Agent Context Signal 的全套形状（视频类型/轨道角色/编辑阶段/阶段指引/编辑骨架/轨道提示）。

不做：文件系统存在检查 / 路径真解析 / 媒体探测（`resolveURL`/`isMissing`/`loadMetadata` 在 project / media 层）；Context Signal 的**检测与填充**（在 agent 层，Phase B–D）。

## 关键类型与算法

源文件：[`media.rs`](../../../crates/opentake-domain/src/media.rs)、[`signal.rs`](../../../crates/opentake-domain/src/signal.rs)

### 媒体（media.rs）
- `MediaSource`：`External { absolute_path }` 或 `Project { relative_path }`，外部标签 enum，线上形如 `{"external":{"absolutePath":...}}` / `{"project":{"relativePath":...}}`（对齐 Swift 关联值 Codable）。
- `GenerationInput`：生成资产的完整输入快照（`prompt`/`model`/`duration`/`aspect_ratio` 必填，其余 `Option`：分辨率/质量/各类 URL 与 assetId 列表/音频专属/视频专属/`created_at`）。是 Rerun 的唯一数据来源。缩写 casing 用显式 `rename`：`imageURLs`/`referenceImageURLs`/`referenceVideoURLs`/`referenceAudioURLs`/`imageURLAssetIds`。
- `MediaManifestEntry`：清单条目（`id`/`name`/`type`/`source`/`duration`/`generation_input?`/源宽高/`sourceFPS`/`has_audio`/`folder_id`/`cachedRemoteURL` + 过期）。
- `MediaFolder { id, name, parent_folder_id? }`：库文件夹（可层级）。
- `MediaManifest { version, entries, folders }`：默认 `version = 2`，但**缺 `version` 解码回退 1**（自定义 `Deserialize`，对齐上游）。
- `MediaResolver`（**零 IO**）：`entry(id)`、`display_name(id)`（未知返回 `"Offline"`）、`expected_path(id)`（External 返绝对路径；Project 把相对路径接到 `project_base`；未知或 Project 无 base → `None`）。仅算**期望**路径，不查文件是否存在。
- `GenerationStatus { None, Generating, Downloading, Rendering, Failed(String) }`，默认 `None`。
- `MediaAsset`：运行期媒体对象（数据 + 派生），省略 AppKit/AVFoundation 成员（缩略图 `NSImage`、`loadMetadata`，在 media 层用 FFmpeg 重建）。
  - 构造：`new`（视频默认 `has_audio = true`，直到元数据另说）、`from_entry(entry, resolved_url)`。
  - 派生：`is_generated()`（有 `generation_input`）、`is_generating()`（状态在生成/下载/渲染中）、`fresh_remote_url(now)`（缓存直链未过期才返回，`now` 注入保持无时钟）、`to_manifest_entry(project_base, now)`（在 base 内→`Project` 否则 `External`；过期缓存直链连同过期时间一并丢弃）。
  - 日期约定：`f64`，单位为 Apple 参考日期（2001-01-01）的秒，与上游 `JSONEncoder` 字节兼容；墙钟换算在 project/render 层。

### 上下文信号（signal.rs，非上游移植，依 `AGENT-CONTEXT-SIGNAL.md` §1.2）
- `VideoType { TalkingHead, Vlog, Montage, Interview, ShortForm, LongForm }`，线上 snake_case（`"talking_head"`）。
- `TrackRole { MainCamera, BRoll, Voice, Bgm, Sfx, Text, Caption }`，拼写照文档：`"MainCamera"` / `"B_Roll"` / `"BGM"` / `"SFX"`（用显式 `rename`）。
- `EditingStage { Importing, Classifying, RoughCut, BRollOverlay, AudioPolish, ColorGrade, ExportReady }`，线上 PascalCase（`"RoughCut"`）。
- `StageGuidance { description, next_actions, warnings }`、`EditingSkeleton { approach, flow, rules }`、`TrackHint { track_index, role, advice }`、`TrackRoleAssignment { track_index, role }`。
- `ContextSignal { video_type, confidence, track_roles, editing_stage, stage_guidance, editing_skeleton, track_hints }`：附加在 MCP 工具结果上的完整信号，1:1 文档形状；集合/指引字段缺省为空（最小解码只需 `video_type`/`confidence`/`editing_stage`）。

## 关键不变量与上游对齐点

- **零 IO 红线**：`MediaResolver` 只算期望路径，绝不 `std::fs`；存在性检查上层做。
- **`MediaManifest.version` 缺省回退 1**（非 struct 默认的 2），靠自定义 `Deserialize`——老 manifest 兼容的关键。
- **日期=Apple 参考秒（`f64`）**：保持与上游字节兼容，不要换成 Unix 时间戳。
- **`fresh_remote_url` 过期是排他 `>`**：`expires_at > now` 才算新鲜；`to_manifest_entry` 落盘时丢弃过期直链及其过期时间。
- **缩写 casing 显式 `rename`**：容器 `camelCase` 会把 `URL`/`FPS` 等缩写小写化，故 `imageURLs`/`sourceFPS`/`cachedRemoteURL` 等逐个 `rename`，与上游 `JSONEncoder` 对齐。
- **Context Signal 线上大小写按文档**：`video_type` snake_case、`editing_stage` PascalCase、角色保留文档拼写——这是 agent 层填充与前端消费的契约。

## 与其他子系统关系

- `MediaResolver` 服务 [timeline-model.md](timeline-model.md) 中 `Clip.media_ref` 的资产解析（上层用），`ClipType` 是清单/资产的 `type` 字段。
- `GenerationInput`/`GenerationStatus` 由 `opentake-gen` 与 agent 层填充；本 crate 只定形。
- `ContextSignal` 系列由 `opentake-agent` 在 MCP 工具结果中填充（检测逻辑不在本 crate），设计见 [`../opentake-agent/AGENT-CONTEXT-SIGNAL.md`](../opentake-agent/AGENT-CONTEXT-SIGNAL.md)。

---

页脚：[INDEX.md](INDEX.md)
