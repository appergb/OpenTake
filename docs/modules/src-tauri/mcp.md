# mcp — 回环 MCP server 拉起

> 上级：[本模块目录](INDEX.md) · [总览](OVERVIEW.md) · [模块文档树](../INDEX.md)
>
> 源码：[`../../../src-tauri/src/mcp.rs`](../../../src-tauri/src/mcp.rs)

## 定位

在 Tauri async runtime 上拉起回环 MCP server（#36）。这是本模块对「AI Agent 网络面」的唯一接线点——真正的 server / 工具派发 / 工作流逻辑都在 `opentake-agent`，本文件只负责**用一个共享会话的 `AppCore` 克隆把它 spawn 起来**。

server 经 Streamable-HTTP 暴露在 `http://127.0.0.1:19789/mcp`，让外部 agent（`claude mcp add --transport http opentake http://127.0.0.1:19789/mcp`、Cursor、Codex…）驱动**与 UI 同一个 `AppCore`**。绑 `127.0.0.1` + 端口 19789 沿用上游约定（见 [`../../architecture/MODULE-PORT-MAP.md`](../../architecture/MODULE-PORT-MAP.md) 设置 / Help 模块）。

## 调用时机

`lib.rs::run()` 的 `setup` 闭包里、`core` 被移入 managed state **之前**调用 `mcp::spawn(core.clone(), workflows_dir)`：

```rust
mcp::spawn(core.clone(), workflows_dir);   // core.clone() 共享同一 live 会话
…
app.manage(core);
```

`workflows_dir` = `<app_data_dir>/workflows`。

## spawn 流程

```rust
pub fn spawn(core: AppCore, workflows_dir: PathBuf) {
    let handle: Arc<dyn CoreHandle> = Arc::new(AppCoreHandle::new(core));
    let registry = Arc::new(RwLock::new(build_registry(&workflows_dir)));
    tauri::async_runtime::spawn(async move {
        let addr = server::DEFAULT_ADDR.parse()…;     // 解析失败仅记日志后返回
        if let Err(e) = server::serve(addr, handle, registry).await {
            eprintln!("[mcp] server stopped: {e}");
        }
    });
}
```

- **会话共享**：`AppCoreHandle::new(core)` 包住共享会话克隆，对外暴露为 `dyn CoreHandle`。Agent 的编辑与 UI 走同一权威 Timeline 与撤销栈。
- **容错**：bind 失败（端口占用）等只记日志、**不致命**——app 照常运行，只是没有 agent 网络面。`DEFAULT_ADDR` 解析失败同样只记日志后返回。

## 工作流插件 registry（build_registry）

```rust
fn build_registry(workflows_dir: &Path) -> PluginRegistry {
    let mut registry = PluginRegistry::with_builtins();   // 内置工作流（如默认音频优先 Skill）
    if workflows_dir.is_dir() {
        let (user, errors) = PluginRegistry::scan(workflows_dir);
        for e in &errors { eprintln!("[mcp] workflow plugin load error: {e}"); }
        for plugin in user.installed() { registry.register(plugin.clone()); }
    }
    registry
}
```

- 先装内置工作流，再扫 `<app_data_dir>/workflows` 下用户自著插件。
- **用户插件覆盖同 id 内置**：`register` 按 id 替换，且在内置之后运行。
- 加载错误逐条记日志，不阻断启动。

> 工具集、Context Signal、工作流插件格式、内置 Agent 提示等详见跨模块文档 [opentake-agent](../opentake-agent/INDEX.md)。本文件只是宿主壳里的拉起点。

---

> 相关：[setup-lib.md](setup-lib.md)（`setup` 中的调用时机）· 跨模块 [opentake-agent](../opentake-agent/INDEX.md)（MCP server / 工具 / 工作流）· [opentake-core](../opentake-core/INDEX.md)（`AppCore` 共享会话）
>
> 导航：[本模块目录](INDEX.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
