# 有意省略 / 不归本 crate(避免范围蔓延)

| 上游 | 处置 | 依据 |
|---|---|---|
| `ImageVideoGenerator`(图片烧静止视频) | **整类删除**;图片在 render 物化为纹理 | `docs/_analysis/02` 表 L74;ARCHITECTURE §6 L130 |
| `AlphaVideoNormalizer`(直 alpha 预乘转码) | **整类删除**;wgpu 着色器内处理 premultiplied alpha | `docs/_analysis/02` 表 L75 |
| `LottieVideoGenerator`(Lottie 烧 ProRes) | 暂不归本 crate;render 物化层或 `opentake-motion`(Phase 10) | ROADMAP Phase 10;§8.3 |
| `CompositionBuilder` / `VideoEngine` / wgpu 合成 / RenderPlan / 关键帧 ramp / renderSize 偶数化 / BT.709 instruction | **属 `opentake-render`** | ARCHITECTURE §6;ROADMAP Phase 3/4/5 |
| `MediaVisualCache` 的 @MainActor 内存表 + `needsDisplay` 触发重绘 | 内存镜像/事件推送属 `opentake-core`/前端;本 crate 只做纯生成 + 磁盘缓存 | §3.5 |
| `VisualModelLoader` 的 `@Observable` UI 状态机 | UI 镜像属上层;本 crate 提供 `installed/install/load` + warm-up | §5.6;`VisualModelLoader.swift` |
| Settings/Storage UI、账户/计费/Clerk/Convex | UI-rebuild / cloud-rebuild,非本 crate | `MODULE-PORT-MAP` L940 |

---

## 附:关键证据索引(绝对路径 + 行号)

- 解码/读写:`palmier-pro-upstream/Sources/PalmierPro/Preview/AlphaVideoNormalizer.swift:34-148`(alpha 检测/预乘转码);`Preview/ImageVideoGenerator.swift:16-206`(静止视频/偶数尺寸 `encoderDimension:68-72`/BT.709 `:168-174`);`Transcription/Transcription.swift:203-280`(`extractAudioTrack` 16k mono PCM)。
- 缩略图/波形:`Timeline/MediaVisualCache.swift`(`videoThumbnailTimes:192-202`、缩略图 `:113-118`、`makeImageThumbnail:152-163`、sprite `loadThumbnails:238-262`/`saveThumbnails:264-293`、`waveformSampleCount:186-190`、`.waveform` LE `:218-227`、缓存键 `:209-216`、闸门 `:16/27`)。
- 转写:`Transcription/Transcription.swift:5-39`(模型/`offsetting:26-38`)、`:72-90`(locale)、`:284-322`(`decodeResults` segment/word);`Transcription/TranscriptCache.swift:12-88`(缓存/filter/key);`Transcription/TranscriptSearch.swift:12-36`(terms/matches)。
- 语义搜索:`Search/SearchIndexConfig.swift:4-45`(阈值/manifest);`Search/Models/VisualEmbedder.swift:7-87`(Spec/encode/预处理 squash 黑底 `:81-85`/输出断言 `:53-61`);`Search/Models/TextTokenizer.swift:16-23`(截断 64 右填 0);`Search/Models/ModelDownloader.swift:46-172`(安装/校验/解压);`Search/Models/VisualModelLoader.swift:86-110`(load + warm-up);`Search/Indexing/FrameSampler.swift:40-117`(采样/LumaGrid `:94-117`);`Search/Indexing/VisualIndexer.swift:15-86`(累积/幂等);`Search/Indexing/EmbeddingStore.swift:30-115`(PALMEMB1);`Search/Query/VisualSearch.swift:16-56`(sgemv 排名);`Search/SearchIndexCoordinator.swift:37-257`(调度/导出暂停/查询)。
- 领域契约:`OpenTake/crates/opentake-domain/src/media.rs:226-440`(MediaResolver/MediaAsset)、`clip_type.rs:781-832`(ClipType)、`timeline.rs:931-1031`、`clip.rs`。
- 横切:`OpenTake/docs/_analysis/02-苹果框架可移植性.md`(能力→栈映射表 L66-83、攻坚清单 L98-136);`OpenTake/docs/MODULE-PORT-MAP.md`(L833-883 搜索、L923-940 存储、L1211 转写);`OpenTake/docs/ARCHITECTURE.md` §1/§6/§7;`OpenTake/docs/ROADMAP.md` Phase 2/8。
