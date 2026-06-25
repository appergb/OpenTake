# 剪辑引擎实现现状与规划(EDITING-ENGINE-PLAN)

> 目标:把剪辑(片段增删改移 + 链接音频 + 吸附 + 右键/快捷键)按上游 `palmier-pro` **1:1 写通**。
> 本文是「现状测绘 + 1:1 差距 + 收口计划」,与 [PORT-1TO1-GAP.md](PORT-1TO1-GAP.md)、[ROADMAP.md](ROADMAP.md) 配套。
> 渲染/播放管线另见 issue [#142](https://github.com/appergb/OpenTake/issues/142) 与 `memory/opentake-render-pipeline-rewrite`。

## 1. 剪辑数据流(已测绘,逐层 1:1)

```
拖拽/双击素材 → editActions.ts(addMediaToTimeline*)
              → Tauri command edit_apply(EditRequest)  [serde camelCase 总根因已修 #143]
              → into_command() → EditCommand
              → opentake-ops/src/ops/*  ← 真正的剪辑算法层(与上游 1:1)
              → EditResult(TimelineSnapshot) → 前端 sync 镜像
```

**Rust ops 层(`crates/opentake-ops/src/ops/`)质量高、已 1:1 移植上游:**

| 操作 | 文件 | 上游对应 | 状态 |
|---|---|---|---|
| 放置 place_clip | `place.rs` | `EditorViewModel.placeClip` | ✅ 1:1(含 linked audio、sortClips、trim 注入) |
| 分割 split_clip | `split.rs` | `ClipMutations.splitClip` | ✅ 1:1(速度感知 source 重分配 / keyframe 边界拆 / link 重组) |
| 修剪 trim | `trim.rs` | `ClipMutations` trim | ✅ 1:1(source delta→timeline delta 经 round(delta/speed)) |
| 移动 move_clips | `move_clips.rs` | `ClipMutations.moveClips` | ✅ 1:1(先拔再写 / clearRegion / pin-by-id / pruneEmptyTracks) |
| overwrite / ripple | `engines/` | `OverwriteEngine` / `RippleEngine` | ✅ 1:1 |

**结论:剪辑的「算法核」基本写通了。** 真正的缺口在**前端接线层**与**几处上游特性未端口**(见 §3)。

## 2. 链接音频(linked audio)—— 这是上游既定设计,不是 bug

- 加入「带音频的视频」时,会在视频轨下方生成一条**独立的链接 audio clip**(共享 `linkGroupId`),trim/move 联动。
- 门控条件(前端 `editActions.ts` `addLinkedAudio = item.type==="video" && item.hasAudio` → 后端 `place.rs` `should_link` 4 条件含 `spec.has_audio`)与上游 `EditorViewModel.swift:341` `shouldLink = addLinkedAudio && targetIsVideo && asset.type==.video && asset.hasAudio` **逐字一致**。
- **无音频视频** → `has_audio=false` → 不建音轨(`probe.rs` 的 `channels==0` 守卫;`channels` 缺失保守保留,因 ffprobe 对真实音频必报 channels,且纯音频文件不能误杀)。

### ⚠️ 待用户定夺(1:1 偏离决策)
用户反馈「带音频的视频应把音频显示在视频片段内,而非独立 A1/A2 轨」。这与上游设计(独立链接音轨)**相悖**。两条路:
1. **保持 1:1**(推荐,默认):视频+音频→独立链接音轨。若用户看到的视频「确实无音频却生成了音轨」,优先排查是否为**旧构建导入的陈旧 `hasAudio` 缓存**(重新导入即可刷新),而非改逻辑。
2. **偏离 1:1**:音频内嵌视频片段、不建独立轨。需改 place/渲染/导出多处,且破坏与上游的可对拍性。**不建议,除非用户明确要求。**

## 3. 与上游的 1:1 差距(已建 issue,按"相关则修"推进)

| 差距 | issue | 类别 | 计划 |
|---|---|---|---|
| fade knee 拖拽编辑(`DragState.fadeKnee`) | [#145](https://github.com/appergb/OpenTake/issues/145) | 剪辑引擎 | 认领:TimelineContainer 加 fadeKnee 拖拽态 |
| 隐藏轨片段仍可拖拽(hitTest 未过滤 `track.hidden`) | [#146](https://github.com/appergb/OpenTake/issues/146) | 剪辑引擎 | 认领:hitTest 过滤 hidden |
| Clip 缺 `isSoloed`/`linkedClip` 字段 | [#147](https://github.com/appergb/OpenTake/issues/147) | 核心模型 | 需前后端 DTO 扩展 |
| 布局常量偏离 | [#148](https://github.com/appergb/OpenTake/issues/148) | 低优先 | 仅记录 |
| 轨间插入阈值(insertThreshold) | [#98](https://github.com/appergb/OpenTake/issues/98) | 剪辑引擎 | 落点路由补轨间探测 |
| Snap DPI 容差 / includePlayhead | [#86](https://github.com/appergb/OpenTake/issues/86) | 剪辑引擎 | 容差按 DPI 缩放 |
| 链接 offset 角标位置 | [#87](https://github.com/appergb/OpenTake/issues/87) | 渲染 | 角标移到剪辑右上角 |

## 4. 收口计划(把剪辑引擎"写通")

**阶段 A — 接线层补齐(认领修复,本批)**
1. [#146] hitTest 过滤 `track.hidden` —— 小、安全,先做。
2. [#86] Snap 容差按 `devicePixelRatio` 缩放;核对所有 snap 调用都传 `includePlayhead`。
3. [#87] 链接 offset 角标移到剪辑右上角(对照 `ClipRenderer.swift:656`)。

**阶段 B — 上游特性端口**
4. [#145] fade knee 拖拽(`DragState.fadeKnee` + TimelineInputController 对应手势)。
5. [#98] 轨间插入阈值(`TimelineGeometry.dropTargetAt` 的 `Layout.insertThreshold`)。

**阶段 C — 模型扩展(需后端)**
6. [#147] Clip 加 `isSoloed`(solo 功能)+ 前后端 DTO 同步。

**阶段 D — 验收**
7. 每项:对照上游源码 → 写/改 → `cargo fmt`+clippy+test → 真机/dev 复现验证 → 提交。
8. 剪辑与渲染解耦推进:渲染按 [#142](https://github.com/appergb/OpenTake/issues/142) 重写,剪辑按本文 A→B→C。

## 5. 纪律
- **算法核(ops/*)已 1:1,别重写**;改在接线层与渲染层。
- 改任何剪辑行为前先读上游对应源码(`../palmier-pro-upstream/Sources/PalmierPro/`),不自己发明。
- 偏离 1:1 的取舍(如音频内嵌)必须先经用户确认。
