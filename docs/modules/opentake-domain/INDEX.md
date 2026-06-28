# opentake-domain — 模块目录

> 上级：[模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
>
> 值语义叶子层（依赖只有 `serde`，禁 `std::fs` / 网络）。被 `opentake-ops` 及其上所有层依赖。
> 先读 [总览 OVERVIEW.md](OVERVIEW.md) 建立全貌，再按需进入下面的子系统文档。

---

## 总览

- **[OVERVIEW.md](OVERVIEW.md)** — 定位 / 依赖分层 / 职责边界 / 关键概念与数据流 / 上游对应 / 完成状态 / 移植铁律。

## 子系统文档

- **[timeline-model.md](timeline-model.md)** — 容器与片段值语义：`Timeline` / `Track` / `Clip` / `ClipType`、半开帧区间、trim 为源帧偏移、片段采样与帧换算入口。
- **[keyframe-transform.md](keyframe-transform.md)** — 关键帧动画与几何变换：`KeyframeTrack`/`Keyframe`/`Interpolation`、clip 相对存储与插值/`smoothstep`、`Transform`/`Crop` 仿射属性与吸附。
- **[text-grade.md](text-grade.md)** — 文字与调色值：`TextStyle`/`Rgba`/hex 解析/`TextLayout`（近似度量）、`ColorGrade` 线性光调色链（曝光/白平衡/LGG/对比/饱和）。
- **[media-signal.md](media-signal.md)** — 媒体资产与上下文信号：`MediaManifest`/`MediaAsset`/`MediaSource`/`MediaResolver`/`GenerationInput`、Agent `ContextSignal` 系列领域类型。
- **[split-subtitle.md](split-subtitle.md)** — 分割与字幕：`split_clip` 纯分割逻辑、SRT/VTT 导出（`export_srt`/`export_vtt`）、caption-group 样式批量同步、抠像/蒙版参考像素数学。

## 相关跨切面

- [`../../architecture/MODULE-PORT-MAP.md`](../../architecture/MODULE-PORT-MAP.md) — 逐模块移植地图（本 crate 对应「Models」段）。
- [`../../architecture/ROADMAP.md`](../../architecture/ROADMAP.md) — 分阶段路线图（Phase 1 领域/引擎、Phase 8 字幕、Phase S Context Signal）。
- [`../../architecture/PORT-1TO1-GAP.md`](../../architecture/PORT-1TO1-GAP.md) — 1:1 复刻差距清单。
- [`../../architecture/ADVANCED-FEATURES.md`](../../architecture/ADVANCED-FEATURES.md) — 进阶能力设计（A 层调色/抠像/蒙版、D 层字幕；对应 `grade.rs` / `subtitle_export.rs`）。
- [`../../architecture/ARCHITECTURE.md`](../../architecture/ARCHITECTURE.md) — 总体架构（单一真理状态 + 命令事务）。

## 交叉模块

- [`../opentake-core/SPEC.md`](../opentake-core/SPEC.md) — 本模块的 `Timeline` 值模型在 core 的 `EditorState` 中作为唯一权威状态（含版本号 / 事务 / 撤销栈规格）。
- [`../opentake-ops/INDEX.md`](../opentake-ops/INDEX.md) — 直接上层：覆盖/波纹/吸附引擎 + `EditCommand`，作用于本 crate 的值。

## 源码

`crates/opentake-domain/src/`（无子目录，叶子 crate）：

| 文件 | 内容 |
|---|---|
| [`lib.rs`](../../../crates/opentake-domain/src/lib.rs) | crate 文档 + 模块声明 + 公共 API 扁平 re-export |
| [`timeline.rs`](../../../crates/opentake-domain/src/timeline.rs) | `Timeline` / `Track` / `ClipLocation` |
| [`clip.rs`](../../../crates/opentake-domain/src/clip.rs) | `Clip` 值类型 + 全部派生采样 + `VolumeScale` / `FadeEdge` |
| [`clip_type.rs`](../../../crates/opentake-domain/src/clip_type.rs) | `ClipType`（video/audio/image/text/lottie）+ 兼容/扩展名映射 |
| [`keyframe.rs`](../../../crates/opentake-domain/src/keyframe.rs) | `Keyframe` / `KeyframeTrack` / `Interpolation` / `AnimPair` / `smoothstep` / `split_keyframe_track` / `AnimatableProperty` |
| [`transform.rs`](../../../crates/opentake-domain/src/transform.rs) | `Transform` / `Crop` / `Point` / `CropAspectLock`（含旧 `x/y` 迁移） |
| [`text.rs`](../../../crates/opentake-domain/src/text.rs) | `TextStyle` / `Rgba` / `Fill` / `Shadow` / `TextAlignment` / `TextLayout` |
| [`grade.rs`](../../../crates/opentake-domain/src/grade.rs) | `ColorGrade` / `ChromaKey` / `Mask` / `MaskShape` / `Effect` / `Rgb` / `Point2` + 参考像素数学 |
| [`media.rs`](../../../crates/opentake-domain/src/media.rs) | `MediaManifest` / `MediaManifestEntry` / `MediaSource` / `MediaFolder` / `MediaResolver` / `MediaAsset` / `GenerationInput` / `GenerationStatus` |
| [`signal.rs`](../../../crates/opentake-domain/src/signal.rs) | `ContextSignal` / `VideoType` / `TrackRole` / `EditingStage` / `StageGuidance` / `EditingSkeleton` / `TrackHint` / `TrackRoleAssignment` |
| [`split.rs`](../../../crates/opentake-domain/src/split.rs) | `split_clip`（片段分割纯逻辑） |
| [`subtitle_export.rs`](../../../crates/opentake-domain/src/subtitle_export.rs) | `SubtitleCue` / `collect_caption_cues` / `export_srt` / `export_vtt` |
| [`caption_sync.rs`](../../../crates/opentake-domain/src/caption_sync.rs) | `caption_group_ids` / `clips_in_group` / `sync_caption_group_style` |

---

页脚：[模块文档树 ../INDEX.md](../INDEX.md) · [docs 总目录 ../../INDEX.md](../../INDEX.md)
