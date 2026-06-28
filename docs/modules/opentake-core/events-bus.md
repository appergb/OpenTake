# events.rs — CoreEvent / EventBus 事件总线

> 上级：本模块目录 [INDEX.md](INDEX.md) · 总览 [OVERVIEW.md](OVERVIEW.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
>
> 源码：[`../../../crates/opentake-core/src/events.rs`](../../../crates/opentake-core/src/events.rs)

## 定位

`CoreEvent` + `EventBus` 是从 Rust core 到其观察者（Tauri 桥、自动保存、遥测）的**单向变更通知通道**。

上游靠 SwiftUI `@Observable`（`EditorViewModel` 是 `@Observable`）**免费**传播状态变更。OpenTake 跨**逻辑进程边界**（core 在 Rust，UI 在 WebView），变更信号必须**显式化**：每次 committing 编辑发 `CoreEvent::TimelineChanged` 携带新的单调 `version`，`src-tauri` 把它转成前端 `timeline_changed` 事件，只读镜像据此重取（见 [SPEC.md](SPEC.md) §3/§4）。

## CoreEvent（当前实现的 4 个变体）

以内部 `kind` 标签 + `snake_case` 序列化（`#[serde(tag = "kind", rename_all = "snake_case")]`），使 Tauri 桥能作为带标签的 JSON payload 转发。

| 变体 | 何时发 | payload | 前端用途 |
|---|---|---|---|
| `TimelineChanged { version }` | committing 的 edit / undo / redo（由 [`AppCore::apply`](core-router.md) 发） | `version: u64`（严格递增） | `version` 比镜像高则 `get_timeline` 重取 |
| `ProjectOpened { path, version }` | `new_project`（path 空串）/ `open_project` | `path: String`、`version: u64`（恒 0） | 打开后前端自取首快照；open **不**另发 `TimelineChanged`（[SPEC.md](SPEC.md) §5.4 步骤 6） |
| `ProjectSaved { path }` | `save_project` 成功 | `path: String`（写入的包路径） | 提示已保存 / 更新窗口标题 |
| `MediaChanged { count }` | `import_media_file` / `relink_media_file` 成功 | `count: usize`（变更后 manifest entry 数） | 经 `get_media` 重取媒体面板目录；count 供廉价过期检查 |

> **只建模了 timeline / 工程生命周期 / 媒体变更**。preview / export / generation 事件（`PreviewFrame` / `ExportProgress` / `ExportDone` / `ExportFailed` / `GenerationProgress`，见 [SPEC.md](SPEC.md) §3.1）属后续阶段，随其后端落地时再加——**当前代码没有这些变体**。

`CoreEvent` derive `Clone + Debug + PartialEq + Eq + Serialize`，故可被测试断言比较、可被订阅者克隆留存。

## EventBus：回调 Vec 同步扇出（非 tokio::broadcast）

[SPEC.md](SPEC.md) 草稿曾设想 `tokio::broadcast`，但 core 需要的唯一契约是"把一个值扇出给 N 个观察者"。一个置于 `Mutex` 之后的 `Vec<callback>` 恰好满足，且：

- **零运行时依赖**（不拉 tokio）；
- **同步、无 panic**：回调在发射线程上按注册顺序跑；**无订阅者即 no-op**；
- **易测**：测试订阅者只往共享 `Vec` 里 push。

Tauri 桥的回调只是调 `app_handle.emit(...)`。若将来需要异步多消费缓冲，可在**不触动 `CoreEvent` 契约**的前提下叠加。

```
EventBus（#[derive(Clone)]，Arc-backed，克隆共享同一订阅列表）
└── inner: Arc<Mutex<{ next_id, listeners: Vec<(SubscriptionId, Listener)> }>>
```

克隆共享同一份订阅列表——与每个 `AppCore` 克隆观察同一事件流的方式一致。

### API

| 方法 | 行为 |
|---|---|
| `subscribe(listener) -> SubscriptionId` | 注册回调（`Fn(&CoreEvent) + Send + 'static`），返回不透明句柄供日后退订。listener 须 `Send`（总线跨线程可用，命令在可被任意线程触达的 `Mutex` 下跑） |
| `unsubscribe(id)` | 移除先前注册的订阅者；未知 id 忽略 |
| `emit(&event)` | 按注册顺序投递给每个当前订阅者；无订阅者时 no-op（**永不 panic**） |

`SubscriptionId` 是 `Copy + Eq + Hash` 的不透明 `u64` 包装。

> `AppCore` 暴露便捷的 `subscribe(...)` 与 `events()`（见 [core-router.md](core-router.md)）；`src-tauri` 启动时 `subscribe` 一次、把 `CoreEvent` 转成前端 `emit`（桥接代码在 `src-tauri`，属后续阶段）。

## 发射时机：锁外（关键纪律）

`AppCore` 的所有变更路径都在**释放会话锁之后**才 `emit`（见 [core-router.md](core-router.md)）。由于 `emit` 同步运行订阅回调，锁外发射使**订阅回调可安全地重入 core**（如收到事件后回头调 `get_timeline`）而不死锁。

## 序列化形状（前端契约）

- `TimelineChanged { version: 7 }` → `{"kind":"timeline_changed","version":7}`
- `MediaChanged { count: 3 }` → `{"kind":"media_changed","count":3}`

> 单词字段（`version` / `count` / `path`）本就无大小写歧义；`kind` 标签是稳定的 snake_case 判别字段。（多词 DTO 字段的 camelCase 约定见 [dto.md](dto.md)。）

## 测试覆盖（本文件 `#[cfg(test)]`）

无订阅者 emit 是 no-op；订阅者按序收事件；退订后停止投递；`TimelineChanged` / `MediaChanged` 以 `kind` 标签序列化为预期 JSON。

---

> 上级：本模块目录 [INDEX.md](INDEX.md) · 总览 [OVERVIEW.md](OVERVIEW.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
