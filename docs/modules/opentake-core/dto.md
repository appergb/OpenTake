# dto.rs + error.rs — Tauri 边界 DTO 与错误

> 上级：本模块目录 [INDEX.md](INDEX.md) · 总览 [OVERVIEW.md](OVERVIEW.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
>
> 源码：[`../../../crates/opentake-core/src/dto.rs`](../../../crates/opentake-core/src/dto.rs) · [`../../../crates/opentake-core/src/error.rs`](../../../crates/opentake-core/src/error.rs)

## 定位

这两个文件定义 core 与 `src-tauri` 之间的**边界契约**：

- `dto.rs` —— Tauri 命令表面，以**纯 Rust DTO + handler 函数**形式存在，**不依赖 `tauri`**。`src-tauri` 日后用一行 `#[tauri::command]`（取 `State<AppCore>` → 调 handler → 映射 `CmdError`）包住每个 `handle_*`。把请求/响应形状与 `AppCore`→响应的接线放这里，使边界**无需 Tauri 运行时即可单测**，最终的 `#[tauri::command]` 薄壳**零逻辑**。
- `error.rs` —— `CoreError`，把下层各错误折叠成一个边界可统一映射的类型。

---

## dto.rs

### DTO（全 camelCase）

所有 DTO 以 `camelCase` 字段序列化以对齐前端命名约定（[SPEC.md](SPEC.md) §6）。`Timeline` 本体用其自身 domain schema 序列化（= `project.json`），故只读镜像与持久化文件**同一形状**（[SPEC.md](SPEC.md) §4.4）。

| DTO | 字段（camelCase） | 来源 | 用途 |
|---|---|---|---|
| `TimelineSnapshotDto` | `timeline`、`version` | `From<TimelineSnapshot>`（[core-router.md](core-router.md)） | `get_timeline` 响应；前端存为 `{ mirror, mirrorVersion }`，用 `version` 幂等重取 |
| `EditResultDto` | `changed`、`actionName`、`affectedClipIds`、`timelineVersion`、`summary` | `From<EditResult>`（来自 `opentake-ops`） | edit / undo / redo 的结果，面向前端 |
| `CmdError` | `code`、`message` | `From<CoreError>` | 机读 + 人读的边界错误 |

> **camelCase 是硬约束**：历史上 IPC 多词字段 camelCase 没对齐导致反序列化失败、"删除/分割/Inspector 全静默失效"（见项目 `CLAUDE.md` 的 IPC 序列化陷阱、[opentake-ops INDEX](../opentake-ops/INDEX.md)）。本文件用 `#[serde(rename_all = "camelCase")]` 在 DTO 层落实；改 IPC 字段时 Rust DTO、前端类型、调用处三边必须同步。

### Handler 函数（未来每个 `#[tauri::command]` 的体）

当前实现的 handler（每个就是"调 `AppCore` 方法 → `From` 成 DTO → `map` 错误"）：

| Handler | 调用的 AppCore 方法 | 返回 |
|---|---|---|
| `handle_get_timeline(core)` | `get_timeline` | `TimelineSnapshotDto`（**无误**） |
| `handle_edit_apply(core, command)` | `apply(command)` | `Result<EditResultDto, CmdError>` |
| `handle_undo(core)` | `undo` | `Result<EditResultDto, CmdError>` |
| `handle_redo(core)` | `redo` | `Result<EditResultDto, CmdError>` |
| `handle_project_open(core, path)` | `open_project(path)` | `Result<TimelineSnapshotDto, CmdError>` |
| `handle_project_save(core, path)` | `save_project(path?)` | `Result<String, CmdError>`（写入路径） |
| `handle_project_new(core)` | `new_project` | `()`（**无误**） |

`edit_apply` 的 `command: EditCommand` 由前端（UI 手势）构造，直送 `AppCore::apply`——UI 客户端的入口；`AppCore::apply` 则是三客户端的共同汇聚点（二者不重复，前者是后者一个调用方，见 [core-router.md](core-router.md)）。私有 `map<T>` 把 `crate::Result<T>` 适配成 `Result<T, CmdError>`。

> **计划中：能力相关 Tauri 命令尚未在此出现**。[SPEC.md](SPEC.md) §6 草拟的 `seek` / `import_media`（异步后端版）/ `export_start` 依赖 [`CoreDeps`](deps-di.md) 的真实后端（render / media），当前**只有 trait 接缝 + `Unsupported` 占位**，对应 handler 待这些后端落地再加。（注意：会话内**同步** `import_media_file` 已实现，见 [session.md](session.md)，但它尚未在 `dto.rs` 暴露为命令 handler。）

---

## error.rs

### CoreError

装配层编排三个下层、各有自己的错误类型；`CoreError` 把它们折叠成一个 Tauri 命令表面（与内置 agent）可统一映射的类型，并补只在装配级才有的几种条件。

```rust
pub enum CoreError {
    Edit(#[from] EditError),        // 编辑层拒绝（坏索引/缺 clip/ripple 拒绝…）→ validation；文档不变、version 不动
    Project(#[from] ProjectError),  // .opentake 包读写失败 → internal
    NoProjectOpen,                  // 需要打开的工程但没有（无路径且无记忆目录的 save 等）→ internal
    Unsupported(&'static str),      // 某能力后端此构建未接线，携带后端名 → internal（见 deps-di.md）
    Media(String),                  // 媒体库操作被输入校验拒绝（重链 id 未知/类型不匹配）→ validation；目录不变
}
```

`#[from]` 使 `EditError`（来自 `opentake-ops`）/ `ProjectError`（来自 `opentake-project`）可经 `?` 自动上抛。`Result<T>` = `Result<T, CoreError>`，是装配层可错操作的统一别名。

### code() 分类（validation vs internal）

```rust
pub fn code(&self) -> &'static str {
    match self {
        Edit(_) | Media(_) => "validation",
        Project(_) | NoProjectOpen | Unsupported(_) => "internal",
    }
}
```

这条划分镜像 [SPEC.md](SPEC.md) §6.3 的 `code: "validation"` vs `"internal"`：

- **`validation`**（`Edit` / `Media`）：调用方输入被拒，**文档/目录原样未动、version 不动**。前端镜像保持一致（事务早返回、无事件）。`CmdError.message` 带精确路径（如 ops 层的 `entries[3].startFrame: …`）。
- **`internal`**（`Project` / `NoProjectOpen` / `Unsupported`）：IO / 解码 / 装配问题。`message` 友好化，详细上下文记日志（对齐"UI 友好 + 服务端详细"）。

`CmdError` 经 `From<CoreError>` 构造：`code = err.code()`，`message = err.to_string()`（`thiserror` 的 `#[error(...)]` 文案）。

## 错误流向小结

```
opentake-ops::EditError  ─┐
opentake-project::ProjectError ─┤  #[from]
                          ├─→ CoreError ──code()──→ "validation" | "internal"
装配级 NoProjectOpen / Unsupported / Media ─┘        └─ to_string() ─→ CmdError{code,message} ─→ 前端
```

## 测试覆盖（两文件 `#[cfg(test)]`）

**dto.rs**：`get_timeline` handler 返回快照 DTO（version 0、1 轨）；`edit_apply` happy path（changed、version 1、`actionName == "Add Clip"`）；`edit_apply` 映射校验错误（`code == "validation"`、message 非空）；undo/redo handler 往返（version 2、3）；无路径 save 映射 `internal`；DTO 序列化为 camelCase（`actionName` / `affectedClipIds` / `timelineVersion`）。测试用 `core_with_track()` 经真实 `open_project` 路径用每调用唯一的临时包播种。

**error.rs**：`code()` 分类逻辑由 dto.rs 的错误映射测试间接覆盖（`validation` / `internal` 两类均被断言）。

---

> 上级：本模块目录 [INDEX.md](INDEX.md) · 总览 [OVERVIEW.md](OVERVIEW.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
