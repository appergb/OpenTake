# core-handle-convert — CoreHandle 边界 + 结果转换 + 模型目录

> 上级：[模块目录](INDEX.md) · [总览](OVERVIEW.md) · [docs 总目录](../../INDEX.md)
>
> 源码：[`../../../crates/opentake-agent/src/mcp/core_handle.rs`](../../../crates/opentake-agent/src/mcp/core_handle.rs)、[`../../../crates/opentake-agent/src/mcp/convert.rs`](../../../crates/opentake-agent/src/mcp/convert.rs)、[`../../../crates/opentake-agent/src/mcp/gen_catalog.rs`](../../../crates/opentake-agent/src/mcp/gen_catalog.rs)

---

这三个文件是派发壳与外部世界（`opentake-core` / rmcp / `opentake-gen`）之间的**适配边界**。

## CoreHandle：可测的文档边界（`core_handle.rs`）

派发壳从不直接碰 `opentake_core::AppCore`，而是经 `CoreHandle` trait（`Send + Sync`）。把这层接口收得**极窄**，使整条工具派发管线无需 UI 或传输即可单测（生产传真实 `AppCore`，测试传内存假实现）。

### trait 接口

| 方法 | 用途 |
|---|---|
| `timeline() -> Timeline` | `get_timeline` 源 + 每次工具前后的快照 |
| `media() -> MediaManifest` | `get_media` / `list_folders` 源 + 短 id 全集 |
| `apply(cmd) -> anyhow::Result<EditResult>` | 应用一个 `EditCommand`，把 core 错误转 `anyhow`（壳再转单个 `ToolResult::error`） |
| `project_dir() -> Option<PathBuf>` | 工程 bundle 目录（未保存为 `None`） |
| `media_path(media_ref)`（默认实现） | 资产 id → 本地文件路径，镜像 `MediaResolver.expected_path` |
| `extract_analysis_pcm(media_ref, spec, range)`（默认实现） | 解码资产首条音轨为分析 PCM；测试可覆盖以注入合成 PCM 而不跑 ffmpeg |

`media_path` / `extract_analysis_pcm` 给默认实现（前者经 `MediaResolver`，后者经 `opentake-media::extract_pcm`），让分析驱动工具（[dispatch-tools.md](dispatch-tools.md) 的 `detect_beats` / `tighten_silences`）开箱即用，又能在测试里替换。

### AppCoreHandle：生产实现

`AppCoreHandle(pub AppCore)` 把上述方法委托给 `AppCore`。`AppCore` 的 clone 指向同一会话，故可按请求构造而不复制任何文档状态——契合 `AppCore` 的跨客户端设计。`src-tauri/src/mcp.rs` 即用 `Arc::new(AppCoreHandle::new(core))` 作为 `Arc<dyn CoreHandle>` 喂给 `server::serve`。

## convert.rs：ToolResult → rmcp CallToolResult

把传输中立的 [`ToolResult`](dispatch-tools.md) 映射到 rmcp `CallToolResult`：`Block::Text` → `Content::text`，`Block::Image`（如未来 `inspect_timeline` 的帧）→ `Content::image`；`is_error` 驱动 `CallToolResult::error` vs `success`。被 [`McpServer`](mcp-server.md) 在每次 `call_tool` 后调用。

## gen_catalog.rs：list_models 投影

`list_models` 工具的数据源，也是 `opentake-agent` 与 [`opentake-gen`](../opentake-gen/INDEX.md) 的第一座桥。BYOK 模式下模型目录是编译进 `opentake-gen` 的**静态资产**（`opentake_gen::builtin_catalog()`）。

- `parse_kind(raw)` —— 把可选 `?type=` 解析为 `ModelKind`（`video`/`image`/`audio`/`upscale`），未知值给精确路径错误（与工具层自纠错契约一致）。
- `list_models_payload(kind)` —— 读静态目录，可选按 `kind` 过滤（`filter_by_kind`），投影成 `{ models, loaded }` JSON。

因为目录内嵌二进制，`loaded` 恒 `true`（无异步同步步骤会失败），纯本地、无网络、无 BYOK key，故派发壳同步跑、测试可离线覆盖。`entry_to_json` 手写投影（而非 `serde_json::to_value`），因为 `CatalogEntry` 只派生自定义 `Deserialize`（1:1 上游线型，不打算再序列化）；字段名镜像内嵌 `builtin_catalog.json`（camelCase），保证 UI/Agent 在两种模式下看到同一形状。

## 上游对照

- `CoreHandle` ↔ 上游 `ToolExecutor` 对 `EditorViewModel` 的依赖收口为窄接口（`agent-SPEC.md` §8.1）。
- `convert.rs` ↔ 上游 `ToolResult` → MCP `CallTool.Result`。
- `gen_catalog.rs` ↔ 上游 `ToolExecutor+Generate.swift` 的 `list_models`（上游经 Convex 订阅 `models:list` 动态目录；OpenTake 改为 BYOK 静态目录，见 [`../../upstream-analysis/03-闭源云边界.md`](../../upstream-analysis/03-闭源云边界.md)）。资源 URI `palmier://models/*` → `opentake://`。

## 完成状态

- 已实现：`CoreHandle` + `AppCoreHandle`、`convert`、`gen_catalog`（`list_models` 全链路，含 `?type=` 过滤）。均有测试覆盖。
- 计划中：`CoreHandle` 尚未暴露**媒体读后端**（采样帧 / 转写 / 语义搜索）与**异步 GenClient**，这正是 [dispatch-tools.md](dispatch-tools.md) 中 `inspect_media` / `generate_*` 等 stub 的前置条件。

---

> 上级：[模块目录](INDEX.md) · [总览](OVERVIEW.md) · [docs 总目录](../../INDEX.md)
