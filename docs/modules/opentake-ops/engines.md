# 子系统：纯引擎（engines/）

> 上级：[INDEX.md](INDEX.md) · 总览：[OVERVIEW.md](OVERVIEW.md)

## 职责

`engines/` 是三个**无副作用**的纯函数引擎：给定输入返回「建议的变更」，由调用方（`ops/*`）落地。它们是剪辑算法最易直接移植、最易全单测的部分，数值逐项对齐上游。所有帧区间用半开 `[start, end)`，所有取整用 `f64::round()`（half-away-from-zero，对齐 Swift `.rounded()`）。

源文件：
- `../../../crates/opentake-ops/src/engines/mod.rs`（re-export）
- `../../../crates/opentake-ops/src/engines/overwrite.rs`
- `../../../crates/opentake-ops/src/engines/ripple.rs`
- `../../../crates/opentake-ops/src/engines/snap.rs`

## OverwriteEngine — 覆盖清区

**职责**：给定一个轨道的片段表与区间 `[region_start, region_end)`，返回为腾出该区间需要对每个重叠片段做的动作，供 `clear_region` 落地。

**关键类型 / 算法**：
- `OverwriteAction` 枚举：`Remove`（片段整体在区内 → 删）/ `TrimEnd { new_duration }`（仅左缘重叠 → 修右边）/ `TrimStart { new_start_frame, new_trim_start, new_duration }`（仅右缘重叠 → 修左边）/ `Split { left_duration, right_start_frame, right_trim_start, right_duration }`（跨越整个区间 → 拆，中段后续由调用方删）。
- `OverwriteEngine::compute_overwrite(clips, region_start, region_end) -> Vec<OverwriteAction>`：逐片段判定。`cs = start_frame`，`ce = end_frame()`：
  - `ce <= region_start || cs >= region_end` → 跳过（区外）；
  - `cs >= region_start && ce <= region_end` → `Remove`；
  - `cs < region_start && ce > region_end` → `Split`：左 `duration = region_start - cs`，右 `start = region_end`，右 `trim_start = trim_start_frame + round((region_end - cs) * speed)`，右 `duration = ce - region_end`；
  - `cs < region_start` → `TrimEnd`：`new_duration = region_start - cs`；
  - else（仅右重叠）→ `TrimStart`：`new_start = region_end`，`new_trim_start = trim_start_frame + round((region_end - cs) * speed)`，`new_duration = ce - region_end`。

**不变量与上游对齐**：
- 1:1 移植 `OverwriteEngine.swift`。源帧折算用 `round((region_end - cs) * speed)`，方向与上游一致（速度 0.25 时 `round(12.5)=13`，有单测固化）。
- 唯一刻意偏离：上游 `.split` 动作携带新铸 `rightId: UUID().uuidString`，但 `clearRegion` 忽略它（它再跑 `splitClip` 自己铸 id）。本引擎保持纯函数、不含 id，`Split` 不带 id，由调用方的 split 路径铸 id；所有数值输出一致。
- 输出动作顺序与上游一致，保证下游 id 铸造 / 撤销分组确定。
- 空区间（`region_end <= region_start`）返回空。

## RippleEngine — 波纹位移

**职责**：计算删除 / 插入后片段应如何位移（闭合空隙或整体右推）。纯数学，无副作用。

**关键类型 / 算法**：
- `ClipShift { clip_id, new_start_frame }`：对单个片段建议的新起始帧。
- `FrameRange { start, end }`（半开，`length() = end - start`）、`GapSelection { track_index, range }`。
- `RippleEngine::compute_ripple_shifts(clips, removed_ids)`：删片段后回填——剩余片段按 `start` 升序，每片段左移量 = 所有 `end <= clip.start_frame` 的（已合并）删除区间长度之和；`>0` 才产生 `ClipShift`。
- `compute_ripple_shifts_for_ranges(clips, removed_ranges)`：按外部传入的帧区间回填（用于跨轨道 sync-lock 联动）。
- `compute_ripple_push(clips, insert_frame, push_amount, exclude_ids)`：`start_frame >= insert_frame` 的片段一律 `+push_amount`。
- `merge_ranges(ranges)`：按 `start` 升序，相邻 `range.start <= last.end` 即合并取 `max(end)`（**触接也合并**）。

**不变量与上游对齐**：
- 1:1 移植 `RippleEngine.swift`，最易逐行移植（纯整数运算 + 区间合并）。
- 边界：删除区间 `end` 恰等于 `clip.start_frame` 时**计入**左移（`end <= start`），有单测固化。
- 仅在片段之前的删除区间计入左移，片段之后的区间不影响——所以「移过帧 0」的负帧拒绝实际由 `ops/ripple.rs` 的 `validate_shifts` 校验（见 [ops-algorithms.md](ops-algorithms.md)）。

## SnapEngine — 拖拽吸附

**职责**：时间线拖拽时的吸附数学——收集吸附目标、带 sticky 滞回与播放头优先的就近匹配。`SnapState` 在拖拽事件间持久化记忆当前吸附目标。

**关键类型 / 算法**：
- `SnapKind`（`Playhead` / `ClipEdge`）、`SnapTarget { frame, kind }`、`SnapResult { frame, probe_offset, x }`、`SnapState { currently_snapped_to, current_probe_offset }`。
- 常量（`consts` 模块，来自上游 `Snap`）：`THRESHOLD_PIXELS = 8.0`、`STICKY_MULTIPLIER = 1.5`、`PLAYHEAD_MULTIPLIER = 1.5`。`base_threshold` 由调用方以像素传入。
- `collect_targets(tracks, playhead_frame, exclude_clip_ids, include_playhead)`：收集所有片段头尾边缘（跳过被拖片段）+ 可选播放头为目标。
- `find_snap(position, probe_offsets, targets, state, base_threshold, pixels_per_frame) -> Option<SnapResult>`：
  - `base_frame_threshold = base_threshold / pixels_per_frame`（阈值随缩放缩放）；
  - **sticky**：已吸附且某探针位 `|probe - snapped| <= base_frame_threshold * 1.5` 且该目标仍在 → 保持；否则解除；
  - 否则遍历 `probe × target` 取最近：播放头阈值再 ×1.5，片段边缘用基础阈值；返回最近且在阈值内者，并写回 `state`。

**不变量与上游对齐**：
- 1:1 移植 `SnapEngine.swift`，数值（目标收集 / sticky 滞回 / 播放头优先 / 多探针就近）逐项保留。
- 剥离平台部分：`NSHapticFeedbackManager` 对齐触觉。返回的「全新吸附」（`Some` 且目标不同于上次 sticky）是 UI 层应触发触觉反馈的时机，本 crate 不触发。
- 文档勘误：上游注释写「2.5x」是过时的，实际 sticky 常量是 1.5（见源码模块注释）。
- 多探针：拖动片段时探针为 `[0, duration]`（头尾），命中哪个由 `probe_offset` 标识。

## 与其他子系统关系

- **被 `ops/*` 消费**：`OverwriteEngine` 由 `clear_region` 落地；`RippleEngine` 由 `ops/ripple.rs` 的 delete/insert 落地，并由 `validate_shifts` 干跑校验；`SnapEngine` 供前端拖拽手势调用（经 core / IPC）。
- **被 `command.rs` re-export**：`apply()` 的 `RippleDeleteRanges` 直接收 `Vec<FrameRange>`。
- **依赖 `opentake-domain`**：只读 `Clip` / `Track` 的帧字段，不修改。

---

> 上级：[INDEX.md](INDEX.md) · 总览：[OVERVIEW.md](OVERVIEW.md)
