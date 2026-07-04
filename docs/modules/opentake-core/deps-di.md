# deps.rs — CoreDeps 依赖注入

> 上级：本模块目录 [INDEX.md](INDEX.md) · 总览 [OVERVIEW.md](OVERVIEW.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
>
> 源码：[`../../../crates/opentake-core/src/deps.rs`](../../../crates/opentake-core/src/deps.rs)

## 定位

`CoreDeps` 是 core **编排但不实现**的能力层（preview / export / media import / generation）的**注入式句柄**。[SPEC.md](SPEC.md) §5.2 要求这些是**注入的 trait 对象**而非对具体函数的硬 `use`，使装配层与尚未完成的 `opentake-render` / `opentake-media` / `opentake-gen` 解耦，并保持用 stub 即可单测。

```
CoreDeps（#[derive(Clone)]，每字段是 Arc，克隆廉价）
├── preview: Arc<dyn PreviewBackend>      // 预览/scrub 播放
├── export:  Arc<dyn ExportBackend>       // 后台导出 job
├── media:   Arc<dyn MediaImporter>       // 媒体导入（path/url/bytes）+ 缩略图/波形
└── gen:     Option<Arc<dyn GenBackend>>  // AI 生成（可选；早期 None）
```

`AppCore` 按 `Arc<CoreDeps>` 持有它（见 [core-router.md](core-router.md)）。

## 四个能力 trait

全部 `Send + Sync`（core 跨线程共享）。当前**接口刻意用不透明 JSON 串**承载参数——具体类型（`ImportSource` / `ExportOptions` 等）会随对应后端在各自阶段落地，避免现在就把未定型的 DTO 焊进 core。

| Trait | 方法 | 职责 | 实现者（阶段） |
|---|---|---|---|
| `PreviewBackend` | `request_frame(frame: i32, interactive: bool) -> Result<()>` | 请求合成某帧；`interactive` 标记 scrub（节流、草稿质量）vs 精确 seek；像素**带外**交付 | `opentake-render`（后续） |
| `ExportBackend` | `start_export(spec_json: &str) -> Result<String>` | 起后台导出，返回不透明 job id；进度带外流式推 | `opentake-render`（后续） |
| `MediaImporter` | `import(source_json: &str) -> Result<String>` | 导入媒体、物化运行时 asset、启动缩略图/波形生成，返回 asset id | `opentake-media`（后续） |
| `GenBackend` | `start_generation(request_json: &str) -> Result<String>` | 起 AI 生成（BYOK/托管），返回不透明 job id；状态带外流式推 | `opentake-gen`（最后阶段，可选） |

> 这些是后续阶段插真实实现的**接缝**。真实后端将在各自 crate 实现同一组 trait，**不触动 core**。

## 占位纪律：可达路径上无 `todo!()`

在那些 crate 落地前，core 出厂带 [`UnsupportedBackends`]——一个单元结构体，对每个能力 trait 都返回 [`CoreError::Unsupported(name)`]（一个**真实、可恢复的错误值，绝非 panic**）。

```rust
impl Default for CoreDeps {
    fn default() -> Self {
        let stub = Arc::new(UnsupportedBackends);
        CoreDeps { preview: stub.clone(), export: stub.clone(), media: stub, gen: None }
    }
}
```

这条纪律保证：

- **整 crate 始终可编译**，每条代码路径可被触发；
- 一个在 render 后端存在前就调 `seek` 的测试（或前端）拿到干净的 `Unsupported("preview")` 错误，**而不是崩溃**；
- `CoreDeps::default()` 是 core 在 render/media/gen 落地前运行的默认，也是测试隔离演练装配层用的桩。

`gen` 用 `Option` 且默认 `None`——生成能力早期缺席，不需要桩去"假装存在再报错"，直接没有。

## 与会话内同步媒体导入的区别（易混淆）

OpenTake 有**两条媒体导入路径**，不要混淆：

| | `EditorSession::import_media_file`（[session.md](session.md)） | `CoreDeps::media: MediaImporter`（本文件） |
|---|---|---|
| 状态 | **已实现** | 仍是接缝（默认 `Unsupported`） |
| 同步性 | 同步 | 异步能力（含缩略图/波形） |
| 探测 | 调用方（`src-tauri`）先探测，传 `ProbedMedia` 值进来 | 后端内部探测 |
| 依赖 | core **不**依赖 `opentake-media`（单测无需 ffprobe） | 后续接 `opentake-media` |
| 入参 | 强类型（path + name + `ProbedMedia`） | 不透明 `source_json` |

前者让"导入一个已被调用方探测过的本地文件→追加 manifest"这件纯逻辑可单测；后者是未来"core 直接驱动媒体引擎做完整导入流水线"的接缝。

## 为何注入而非硬连

- **解耦**：装配层不绑死重栈（ffmpeg / wgpu / ML / 网络），这些 crate 还在做时 core 已能编译、能跑、能测。
- **可测**：测试用 stub（或将来的 mock）演练 core 编排，无需真实后端。
- **不可达即不 panic**：未接线能力以可恢复 `Unsupported` 体现，符合"边界明确、错误诚实"。

## 错误

`CoreError::Unsupported(&'static str)` 携带后端名（`"preview"` / `"export"` / `"media"` / `"gen"`），使调用方能给出精确消息。它经 `code()` 归为 `"internal"` 类（见 [dto.md](dto.md)），因为"此构建未接此能力"是环境/装配问题，不是用户输入校验问题。

## 测试覆盖（本文件 `#[cfg(test)]`）

`default_deps_report_unsupported_not_panic`：默认 deps 的 preview/export/media 均返回 `Unsupported(对应名)` 而**非 panic**，`gen` 为 `None`。

---

> 上级：本模块目录 [INDEX.md](INDEX.md) · 总览 [OVERVIEW.md](OVERVIEW.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
