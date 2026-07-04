# 子系统：编辑算法（ops/）

> 上级：[INDEX.md](INDEX.md) · 总览：[OVERVIEW.md](OVERVIEW.md)

## 职责

`ops/` 是 `command.rs` 组合成事务的**编辑算法积木**。每个都是上游 `EditorViewModel` 某方法的直接移植，**剥离 AppKit / UndoManager 胶水**：它们就地修改 `Timeline`（或 `MediaManifest`），由命令层在外侧 snapshot / commit。算法核已 1:1 写通，**不要重写**。

源文件（`../../../crates/opentake-ops/src/ops/`）：`mod.rs`、`clear_region.rs`、`place.rs`、`split.rs`、`trim.rs`、`move_clips.rs`、`ripple.rs`、`linking.rs`、`tracks.rs`、`duplicate.rs`、`folders.rs`。

## 各算法（一句话职责 + 关键不变量）

### clear_region.rs —— 覆盖清区（共享让位原语）
- **职责**：用 `OverwriteEngine::compute_overwrite` 算出动作，落地为删 / 修 / 拆，腾空 `[start, end)`。是 add / move / paste / duplicate 放置前的统一让位手段。
- **关键不变量**：`Split` 分支**复跑真实 split 路径**（`split_clip`），使新 id 与关键帧边界与手动分割完全一致，再删落在区内的中段——若右半越过 `end` 则在 `end` 再拆一次。`TrimEnd` 落地时反推 `trim_end_frame += round((old_duration - new_duration) * speed)`。`prune` 形参通常传 `false`（事务层末尾统一 prune 一次）。

### place.rs —— 放置片段
- **职责**：`place_clip` 放一个片段，可带链接音频；返回创建的片段 id（`[clip]` 或 `[clip, audio]`）。含 `sort_clips`（按 `start_frame` 升序）。
- **关键不变量**：链接门控 `should_link = add_linked_audio && 目标是视频轨 && source_clip_type == Video && has_audio`，与上游 `placeClip` 的 `shouldLink` 逐字一致。链接音频落到 `resolve_or_create_audio_track` 解析出的音轨，与视频共享 `link_group_id`。
- **刻意偏离**：上游 `placeClip` 从素材源尺寸推视觉 `Transform`（`fitTransform`），那需媒体元数据——本叶子 crate 不解析媒体，故视觉 `Transform` 由调用方经 `PlaceSpec.transform` 传入；未传则回落 `Transform::default()`。链接 / 音频路由 / 排序行为保留。

### split.rs —— 分割片段
- **职责**：`split_clip` 在 `at_frame` 拆片段（链接伙伴一起拆），返回新右半 id；`split_single_clip` 拆单片段。
- **关键不变量**：源消耗按速度在两半重分配（`round(offset * speed)`）使拼回等价于原片段；六条可动画轨在切点插边界关键帧保曲线连续（实际拆分在 `opentake_domain::split_clip`）。`at_frame` 不严格落在片段内则 no-op。链接组拆分后，左半保留原组、右半**重组为全新共享组**（各侧各自成对）。

### trim.rs —— 修剪片段
- **职责**：`trim_clip_internal` / `trim_clips` 把片段改到新**源帧** `trim_start` / `trim_end`；`trim_values` 算边缘拖拽 `delta`（时间线帧）对应的新源帧。
- **关键不变量**：覆盖式——片段原地缩放，**同轨不推邻片段、不向其他轨 sync-lock 推**。源帧 delta → 时间线 delta 经 `round(delta / speed)`；`trim_values` 反向用 `round(delta * speed)`。image / text 片段 trim 可为负（无源约束），video / audio 把被移动的边钳在 0。

### move_clips.rs —— 移动片段
- **职责**：`move_clips` 把片段移到新轨 / 新帧（覆盖式）；返回实际移动数。
- **关键不变量**：先把被移片段从源轨**全部摘除**，使后续 `clear_region` 不误伤它们；对每个目标范围 `clear_region`（`prune:false`）；再把每个片段 `start_frame` 设为 `to_frame` 追加；所有轨 `sort_clips`；最后 `prune_empty_tracks`。轨道**按 id 锚定**（pin-by-id），因 prune 会移位索引。目标轨类型不兼容 / 片段不存在 → 静默跳过（对齐上游 `guard … continue`）。

### ripple.rs —— 波纹删除 / 插入 + sync-lock 机制
- **职责**：落地 `RippleEngine` 的位移并维护跨轨 sync-lock 对齐。函数：`apply_shifts`、`validate_shifts`（干跑校验）、`ripple_delete`（删选中片段闭隙）、`ripple_delete_ranges_on_track`（按帧区间删，返回 `RippleRangesReport`）、`ripple_insert`（插入右推）。
- **关键不变量**：
  - **拒绝语义（原子）**：sync-lock 跟随轨若有片段移后 `start < 0` 或与相邻片段碰撞，`validate_shifts` 返回拒绝原因，整次操作**先校验、再决定**——任一拒绝则不改任何状态（`Err` / `RippleOutcome::Refused`）。上游是 `NSSound.beep` + log，这里返回错误由 UI 处理。
  - **删除**：先收集全局被删区间 `[start, end)`；逐轨——自身有删除 → `compute_ripple_shifts`；否则 sync-locked → `compute_ripple_shifts_for_ranges` 按全局区间，且先 `validate_shifts`。全部通过后才 `remove` + `apply_shifts` + prune。
  - **范围删除**：链接伙伴所在轨一并加入清区集合（A/V 跨多片段区间保持同步），sync-lock 跟随轨干跑校验后随之左移。
  - **插入**：被推轨 = 目标轨 ∪ sync-lock 轨 ∪ 链接音频落点轨；推之前对每条被推轨上跨 `at_frame` 的片段先 `split`（右半随波纹走而非被覆盖）；`compute_ripple_push` 把 `start >= at_frame` 的片段一律右推总插入时长，再把片段首尾相接铺入空隙。

### linking.rs —— 链接组查询与同步
- **职责**：共享 `link_group_id` 的片段作为一个单位用于选择 / 移动 / 修剪 / 分割 / 删除。函数：`link_index`（组 id → 成员）、`expand_to_link_group`（展开到整组）、`linked_partner_ids`（同组其余）、`timing_propagation_partners`（时序变更应同步的伙伴）、`partner_moves`（单片段移动时伙伴的同步移动）。
- **关键不变量**：`expand_to_link_group` 对无组输入原样返回。`partner_moves` 按 `delta = to_frame - lead_start` 平移伙伴并钳 `>= 0`；`delta == 0` 返回空。`timing_propagation_partners` 排除已在输入集合内的片段。

### tracks.rs —— 轨道结构 + 分区不变量
- **职责**：`zones` / `ZoneLayout`（视频/音频分区）、`insert_track`（分区钳制建轨）、`remove_tracks`、`prune_empty_tracks`、`available_audio_track_index`、`resolve_or_create_audio_track`。
- **关键不变量**：**可视轨（video/image/text/lottie）恒在音频轨之上**——`partitioned_insertion_index` 把请求索引钳进各自分区：音频轨 `max(first_audio_index)`，可视轨 `min(first_audio_index)`。`resolve_or_create_audio_track` 先找 `[start, start+duration)` 空闲的音轨，没有则在底部建一条。

### duplicate.rs —— 复制片段（Alt 拖拽）
- **职责**：`duplicate_clips` 深拷贝每个片段（全部关键帧轨 / 调色 / 抠像 / 蒙版 / 特效 / 文本 / transform / crop / fade，`Clip: Clone` 即深拷贝），铸新 id，`start_frame += offset_frames`（钳 `>= 0`），落到 `target_track_indexes[i]`；返回新 id。
- **关键不变量**：与 `move_clips` 同构（同样的目标清区 + pin-by-id + sort + prune），但源片段留原位、目标落深拷贝。链接组重映射：被复制的多片段共享组（如 A/V 对）映射到**全新共享 id** 使副本彼此仍链接；单片段组（或无组）清为 `None`。目标越界 / 类型不兼容 / 片段缺失 → 静默跳过。

### folders.rs —— 媒体库文件夹
- **职责**：操作持久化 `MediaManifest`（entries + folders）而非运行时 `MediaAsset`。函数：`create_folder` / `move_to_folder` / `rename_media` / `rename_folder` / `delete_media` / `delete_folder`。
- **关键不变量**：rename 返回是否真的改了名（同名 no-op）。`delete_media` 删 manifest 条目并**级联删除引用该素材的时间线片段**（`cascade_remove_clips` + prune），二者一步以便一起撤销。`delete_folder` 用定点迭代（`expand_descendant_folders`）递归含全部子孙文件夹及其内素材，再级联删片段。

## 与其他子系统关系

- **被 `command.rs` 调用**：所有 ops 在 `transact` 的 `work` 闭包里被组合调用（见 [command-apply.md](command-apply.md)）。
- **消费 `engines/`**：`clear_region` 用 `OverwriteEngine`；`ripple.rs` 用 `RippleEngine`（见 [engines.md](engines.md)）。
- **依赖 `IdGen`**：place / split / move / duplicate / tracks / folders 铸新 id（见 [intent-id.md](intent-id.md)）。
- **`intent.rs` 在其上预检**：高层意图归一成命令时复用 `trim_values` 等 ops 函数（见 [intent-id.md](intent-id.md)）。

---

> 上级：[INDEX.md](INDEX.md) · 总览：[OVERVIEW.md](OVERVIEW.md)
