# 子系统：编辑意图与 id 生成（intent.rs + id.rs）

> 上级：[INDEX.md](INDEX.md) · 总览：[OVERVIEW.md](OVERVIEW.md)

## 职责

两个小而独立的支撑层：
- `intent.rs`：把**高层编辑意图**做预检并归一成已有的 `EditCommand`（不直接改状态、不绕过 `apply`）。
- `id.rs`：为新建实体注入式生成 id，让本叶子 crate 不必依赖 `uuid`。

源文件：
- `../../../crates/opentake-ops/src/intent.rs`
- `../../../crates/opentake-ops/src/id.rs`

## intent.rs —— 编辑意图预检与归一

**定位**：刻意做薄——只做**预检校验**与**便捷意图展开**（例如「加片段，缺兼容轨就建一条」），然后产出 `EditCommand` 列表。它**从不修改 `EditorState`、从不绕过 `apply`**；真正执行仍走命令事务（见 [command-apply.md](command-apply.md)）。

**关键类型**：
- `EditPlan { label, commands: Vec<EditCommand>, warnings: Vec<String> }`：预检输出（命令 + 警告）。
- `IntentClipEntry`：片段放置意图；`track_index = None` 表示「预检时挑选或新建共享兼容轨」。

**主要函数（均返回 `Result<EditPlan, EditError>`）**：
- `plan_auto_track_add(timeline, entries)`：要么所有 entry 都给 `track_index`（→ `AddClips`），要么全省略（→ `AddClipsAutoTrack`，自动建共享轨）；混用报错。逐条校验后归一。
- `plan_trim_to_playhead(timeline, clip_ids, frame, edge)`：CapCut 式「修剪到播放头」。`frame` 不在片段内则跳过；按边缘算 `delta` 并 `clamp_trim_delta`（钳到 `±(duration-1)`），复用 ops 的 `trim_values` 算源帧，产出 `TrimClips`；无片段相交则返回带 warning 的空计划。
- `plan_ripple_delete_range(track_index, start, end)`：单区间波纹删除（`end <= start` 报错）→ `RippleDeleteRanges`。
- `plan_beat_sync_placement(timeline, entries, beat_frames)`：把片段放到节拍帧（`beat_frames` 不足报错），再复用 `plan_auto_track_add`。
- `plan_smart_reframe(clip_ids, crop, transform)`：把 smart-reframe 的 crop/transform 经 `SetClipProperties` 应用。

**关键不变量与上游对齐**：
- 预检校验（`validate_intent_entry`）：`duration_frames >= 1`、`start_frame >= 0`、trim 非负、`track_index` 在范围内且 `source_clip_type` 与目标轨兼容；不满足返回 `EditError::Invalid`。
- 「全给或全不给 `track_index`」的二选一约束，对齐放置语义。
- 复用 ops 函数（`trim_values`）保证与手动编辑同一套帧折算与钳制规则。

## id.rs —— 注入式 id 生成

**定位**：上游内联铸 `UUID().uuidString`（分割右半 / 放置片段 / 链接伙伴 / 链接组 / 新轨 / 文件夹）。本 crate 保持**零业务依赖叶子**——不引 `uuid`——故 id 创建经 `IdGen` trait 注入。生产侧由已依赖 `uuid` 的上层（project / core）提供 UUID 后端生成器；测试用确定性 `SeqIdGen` 使 split / link / place 的 id 可断言。

**关键类型 / 函数**：
- `trait IdGen { fn next_id(&self) -> String; }`：按需铸唯一 id。
- `SeqIdGen`：确定性单调生成器，`"{prefix}{n}"`，`n` 从 1 起；内部 `Cell` 可变以穿过 `&self` 编辑调用；默认前缀 `"id-"`；`count()` 返回已铸数量。

**关键不变量**：
- 确定性递增（从 1），便于单测固化 id（如 place 链接音频铸序为 组 → 视频 → 音频）。
- 所有需要新 id 的 ops（place / split / move / duplicate / tracks / folders）都接收 `&dyn IdGen` 参数，铸造点集中、可控。

## 与其他子系统关系

- `intent.rs` 在 [ops-algorithms.md](ops-algorithms.md) 与 [command-apply.md](command-apply.md) 之上：产出 `EditCommand` 交给 `apply` 执行，复用 ops 的 `trim_values`。
- `id.rs` 被几乎所有改结构的 ops 与命令实现消费（`&dyn IdGen` 一路下传到 `place_clip` / `split_clip` / `insert_track` / `create_folder` 等）。

---

> 上级：[INDEX.md](INDEX.md) · 总览：[OVERVIEW.md](OVERVIEW.md)
