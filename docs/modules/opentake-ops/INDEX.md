# opentake-ops — 模块目录

> 上级：[模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
>
> `opentake-ops` = 纯编辑引擎 + 唯一编辑入口 `EditCommand` + `apply()` 事务 + 整树快照撤销/重做栈。依赖只向下：仅依赖 `opentake-domain`，被 `opentake-core` 调用。

---

## 总览

- **[OVERVIEW.md](OVERVIEW.md)** — 定位与分层位置、职责边界、关键概念与数据流（唯一编辑入口 → apply 事务 → 整树快照撤销）、对应上游 Swift 模块、完成状态（已实现 vs 计划中）、移植铁律。

## 子系统文档

- **[engines.md](engines.md)** — `engines/` 三大纯引擎：OverwriteEngine（覆盖清区动作）、RippleEngine（波纹位移 / 区间合并）、SnapEngine（拖拽吸附，sticky 滞回 + 播放头优先 + 多探针）。
- **[command-apply.md](command-apply.md)** — `command.rs` 的 `EditCommand` 枚举（唯一编辑入口，**纯枚举无 serde**）+ `apply()` 事务（snapshot → 纯函数 → commit-if-changed → version++）+ `editor_state.rs` 整树快照撤销/重做栈。含 IPC 序列化陷阱（EditRequest DTO camelCase）。
- **[ops-algorithms.md](ops-algorithms.md)** — `ops/` 各编辑算法：clear_region（让位原语）/ place（含链接音频）/ split / trim / move / ripple（删插 + sync-lock 拒绝）/ linking（A/V 组同步）/ tracks（分区建删）/ duplicate / folders。逐个职责 + 关键不变量。
- **[intent-id.md](intent-id.md)** — `intent.rs` 编辑意图预检与归一（自动建轨 / 卡点 / 修剪到播放头 / smart-reframe）+ `id.rs` 注入式 id 生成（`IdGen` / `SeqIdGen`）。

## 相关跨切面（架构）

- [EDITING-ENGINE-PLAN.md](../../architecture/EDITING-ENGINE-PLAN.md) — 剪辑引擎现况测绘 + 1:1 差距 + 收口计划（本模块算法核已写通，缺口在前端接线 / domain 字段）。
- [MODULE-PORT-MAP.md](../../architecture/MODULE-PORT-MAP.md) — 逐模块上游 Swift → Rust 移植地图（OverwriteEngine / RippleEngine / SnapEngine / EditorViewModel / withTimelineSwap）。
- [ROADMAP.md](../../architecture/ROADMAP.md) — 分阶段路线图（Phase 1 = 本模块的引擎 + 命令事务 + 撤销栈）。
- [PORT-1TO1-GAP.md](../../architecture/PORT-1TO1-GAP.md) — 1:1 复刻差距逐项（历史参考）。
- [ARCHITECTURE.md](../../architecture/ARCHITECTURE.md) — 总体架构：单一真理状态 + 命令事务（§5）。

## 规格交叉链

- [opentake-core 规格 SPEC](../opentake-core/SPEC.md) — core 装配 `EditorState`、命令路由、Tauri 边界契约；**含编辑命令 / IPC 规格**（`EditRequest` ↔ `EditCommand` 映射、版本号广播）。

## 源码

```
crates/opentake-ops/src/
├── lib.rs            模块声明 + 公开 API re-export
├── command.rs        EditCommand 枚举 + apply 事务 + 各命令实现
├── editor_state.rs   EditorState + DocSnapshot + 撤销/重做栈
├── intent.rs         高层编辑意图预检与归一（EditPlan）
├── id.rs             IdGen trait + SeqIdGen（注入式 id 生成）
├── engines/
│   ├── mod.rs        re-export
│   ├── overwrite.rs  OverwriteEngine（覆盖清区动作）
│   ├── ripple.rs     RippleEngine（波纹位移 / 区间合并）
│   └── snap.rs       SnapEngine（拖拽吸附）
└── ops/
    ├── mod.rs        re-export
    ├── clear_region.rs  覆盖清区落地（让位原语）
    ├── place.rs         放置片段（+ 链接音频）
    ├── split.rs         分割片段（速度感知 + 关键帧边界 + 链接重组）
    ├── trim.rs          修剪片段（source↔timeline 帧折算）
    ├── move_clips.rs    移动片段（先拔再写 + clearRegion + pin-by-id）
    ├── ripple.rs        波纹删除 / 插入 + sync-lock 拒绝
    ├── linking.rs       链接组查询与 A/V 同步
    ├── tracks.rs        轨道分区 / 建删 / prune / 音轨解析
    ├── duplicate.rs     复制片段（Alt 拖拽深拷贝 + 链接重映射）
    └── folders.rs       媒体库文件夹（建 / 移 / 改名 / 级联删除）
```

源文件树根：`../../../crates/opentake-ops/src/`

---

> 上级：[模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
