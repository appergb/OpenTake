# mcp — Agent 媒体桥与逐轮认证 MCP 宿主

> 上级：[本模块目录](INDEX.md) · [总览](OVERVIEW.md) · [模块文档树](../INDEX.md)
>
> 源码：[`../../../src-tauri/src/mcp.rs`](../../../src-tauri/src/mcp.rs)

## 定位

本模块实现 Tauri 宿主侧的 `MediaBridge`，让 Agent 的检查、导入、检索和媒体分析复用
UI 所在的同一个 `AppCore`、缓存、模型与 retained file capability。固定端口外部 MCP
因为缺少认证配对 UX，在 Beta 2 不由 `lib.rs` 启动。

官方 Codex / ChatGPT 的网络面由 `src-tauri/src/codex.rs` 在每轮调用
`bind_ephemeral_gated` 创建：操作系统分配随机 loopback 端口，随机 256-bit Bearer
通过子进程私有配置传入，并绑定当前工程 identity / revision / sequence scope。轮次完成、
取消、deadline 或切工程时关闭 listener 并清理 Codex 进程树。

## 调用时机

`lib.rs::run()` 在 setup 中创建共享 `ChatState`、工作流注册表和各能力 bridge，但不启动
旧的 `server::DEFAULT_ADDR`。用户选择官方 Codex provider 并发送消息时，`codex.rs` 才为
该轮创建 `Dispatcher`、`AppCoreHandle`、媒体/生成/Motion/高级能力 bridge 与临时 MCP。

## 安全与失败语义

- **逐轮认证**：每轮新端口、新 Bearer；缺失或错误 Authorization 直接拒绝。
- **工程绑定**：迟到调用必须通过工程 identity、revision 与嵌套序列 scope 守卫。
- **生命周期闭合**：成功、失败、取消和 deadline 都会关闭 listener 并清理子进程树。
- **外部入口关闭**：`127.0.0.1:19789` 仍作为 agent crate 的测试/兼容常量保留，
  但不是 Beta 2 产品启动入口，不能作为公开连接说明。

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

> 工具集、Context Signal、工作流插件格式、内置 Agent 提示等详见跨模块文档 [opentake-agent](../opentake-agent/INDEX.md)。本文件只描述宿主 bridge 与安全生命周期。

---

> 相关：[setup-lib.md](setup-lib.md)（`setup` 中的调用时机）· 跨模块 [opentake-agent](../opentake-agent/INDEX.md)（MCP server / 工具 / 工作流）· [opentake-core](../opentake-core/INDEX.md)（`AppCore` 共享会话）
>
> 导航：[本模块目录](INDEX.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
