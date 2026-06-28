# OpenTake — 已知 Bug 与问题

> 本文记录扫描确认的所有真实 Bug 和有问题部分。不包括规划中已承认的"未实现功能"（那些在 [PORT-1TO1-GAP.md](PORT-1TO1-GAP.md) 和 [ROADMAP.md](ROADMAP.md) 中）。
> 最后更新：2026-06-26

---

## 🐛 实际 Bug（会导致功能异常）

### B1. `swapMedia` IPC DTO 缺失（关键）

| 属性 | 值 |
|---|---|
| **位置** | `src-tauri/src/commands.rs:EditRequest` 枚举缺失 `SwapMedia` 变体 |
| **描述** | 前端 `types.ts:199-207` 定义了 `{ type: "swapMedia", clipId, mediaRef, ... }`，Rust 引擎 `command.rs:331` 也有 `EditCommand::SwapMedia` 和完整实现，但 IPC 桥梁 `commands.rs` 的 `EditRequest` 枚举没有对应变体。前端发出的 `swapMedia` 命令会被 Tauri 反序列化拒绝（unknown variant），功能完全不可用 |
| **修复方案** | 在 `commands.rs` 的 `EditRequest` 枚举中添加 `SwapMedia` 变体，并在 `into_command()` 中添加映射 |

---

### B2. 前端帧数学截断不一致（高）

| 属性 | 值 |
|---|---|
| **位置** | `web/src/store/editActions.ts:389,410,560` — 三处 `Math.round(seconds * timeline.fps)` |
| **描述** | Rust 端 `seconds_to_frame()` 使用截断 `(seconds * fps) as i32`（对应上游 `Int(s*fps)`），前端使用 `Math.round`（四舍五入）。当 `seconds * fps` 小数部分 ≥0.5 时，双方结果差 1 帧，导致同一媒体的计算时长不一致 |
| **影响** | 媒体导入时的帧数计算偏差可能导致 clip 时长 off-by-1 |
| **修复方案** | 将前端的 `Math.round(seconds * fps)` 改为 `Math.floor(seconds * fps)` |

---

### B3. `rippleDeleteRanges` Agent 工具忽略 `clipId`/`units`（高）

| 属性 | 值 |
|---|---|
| **位置** | `crates/opentake-agent/src/mcp/dispatch.rs:346-368` |
| **描述** | `RippleDeleteRangesArgs` 声明了 `clip_id: Option<String>` 和 `units: Option<String>` 字段，但 dispatch 层从未读取它们，只用了 `track_index`（默认 0）。描述中声称的`"clamp ranges to clip visible span"` 逻辑从未实现 |
| **影响** | Agent 调用此工具时只能指定具体 track_index，不能通过 clipId 定位；`units: "seconds"` 模式无效 |
| **修复方案** | dispatch 层实现 clipId 模式的 range clamp 和 units 转换 |

---

### B4. `canGenerate` 硬编码为 `false`（中）

| 属性 | 值 |
|---|---|
| **位置** | `crates/opentake-agent/src/tools/...` — 具体待定位 |
| **描述** | Agent 工具的 `canGenerate` 返回值硬编码为 `false`，即使 BYOK key 已配置 |
| **影响** | Agent 无法进行任何 AI 生成操作 |

---

## ⚠️ 功能缺陷（功能可用但不完整）

### D1. 预览未接入 GPU 合成（高）

| 属性 | 值 |
|---|---|
| **位置** | `web/src/components/preview/Preview.tsx` |
| **描述** | GPU 合成的 infrastructure 已就绪（`composite_frame` Tauri 命令、`useTimelineFrame` hook 均存在），但 `Preview.tsx` 仍然使用 DOM `<video>`/`<img>` 路径渲染，未接入 `useTimelineFrame`。后果：看不到关键帧动画、transform/crop/text/effects，preview ≠ export |
| **当前状态** | 已有完整的 wgpu 合成管线，只需在 Preview.tsx 中接入 `useTimelineFrame` hook |

### D2. Agent/MCP 工具 12/40 为 stub（高）

| 属性 | 值 |
|---|---|
| **位置** | `crates/opentake-agent/src/mcp/dispatch.rs:177-191` |
| **描述** | 40 个工具中 12 个（30%）返回 `"not yet implemented"` stub 错误：InspectMedia、GetTranscript、InspectTimeline、SearchMedia、GenerateVideo/Image/Audio、UpscaleMedia、ImportMedia、AddCaptions、AddMotionGraphic、EditMotionGraphic。另有 `create_folder` 和 `move_to_folder` 的 batch 形式 stub |

### D3. Media 缩略图始终返回 `None`（中）

| 属性 | 值 |
|---|---|
| **位置** | `src-tauri/src/media.rs:114` |
| **描述** | `MediaItemDto::from_entry()` 中硬编码 `thumbnail: None`。底层 `thumbnail` 模块（`video_thumbnails()`/`image_thumbnail()` + sprite 缓存）已完整实现并通过测试，但 `media.rs` 从未调用它们 |

### D4. Export 仅 H.264（中）

| 属性 | 值 |
|---|---|
| **位置** | `src-tauri/src/export.rs:57-67` |
| **描述** | 编码器 preset（H265/ProRes）已在 `opentake-media/src/encode/preset.rs` 中实现，但 `export.rs` 的 `resolve_preset()` 分支返回 `Err("not wired yet")`。导出无进度回调、无取消机制 |

### D5. Inspector TextTab/AIEdit 为 scaffold（中）

| 属性 | 值 |
|---|---|
| **位置** | `web/src/components/inspector/TextTab.tsx` |
| **描述** | TextTab 仅有 textContent textarea，`textStyle`（fontSize/color/align）无后端支持。AIEdit tab 在代码路径中从未被执行 `tabs.push("aiEdit")`，UI 上不可见 |

### D6. Captions/Subtitles UI 完全缺失（中）

| 属性 | 值 |
|---|---|
| **位置** | 前端组件树 |
| **描述** | MediaTabBar 有"字幕"标签但硬编码 `enabled: false`，无任何 caption 相关 UI 组件 |

### D7. 上游 1:1 gap 列表（低~中）

详见 [EDITING-ENGINE-PLAN.md](EDITING-ENGINE-PLAN.md) 和 [PORT-1TO1-GAP.md](PORT-1TO1-GAP.md)：

- Speed 改变时 contiguous 后续链 ripple push 未在 ops 层调用
- Snap DPI 容差未缩放
- 轨间 insertThreshold 未在前端实现
- Batch folders 的 Agent 工具未实现完整

---

## 📊 Bug 优先级矩阵

| ID | 问题 | 严重度 | 影响范围 | 修复成本 |
|---|---|---|---|---|
| **B1** | swapMedia IPC 缺失 | 🔴 关键 | 单功能完全不可用 | 低（~10 行代码） |
| **B2** | 前端帧数学截断不一致 | 🟠 高 | 所有媒体导入时长偏差 | 低（3 处 Math.round→floor） |
| **B3** | rippleDeleteRanges 忽略 clipId | 🟠 高 | Agent 工具精度下降 | 中 |
| **B4** | canGenerate 硬编码 false | 🟡 中 | AI 生成功能不可用 | 低 |
| **D1** | 预览未接入 GPU 合成 | 🟠 高 | 所有编辑效果不可见 | 中 |
| **D2** | Agent 工具 30% stub | 🟠 高 | AI 协作核心缺失 | 高 |
| **D3** | 缩略图始终 None | 🟡 中 | 媒体库 UX 差 | 中 |
| **D4** | Export 仅 H.264 | 🟡 中 | 输出格式受限 | 低 |
| **D5** | TextTab/AIEdit scaffold | 🟡 中 | 功能不完整 | 中 |
| **D6** | Captions UI 缺失 | 🟡 中 | 字幕功能不可用 | 高 |
