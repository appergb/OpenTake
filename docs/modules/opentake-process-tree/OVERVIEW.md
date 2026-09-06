# opentake-process-tree — 总览

> 状态：draft · 阶段：implementation-backed · 同步日期：2026-09-06

进程树生命周期支持 crate，位于 [Cargo workspace](../../../Cargo.toml)；实现见 [lib.rs](../../../crates/opentake-process-tree/src/lib.rs)。`configure_command` 配置平台进程启动，`ProcessTree::attach` 绑定子进程，`terminate` / `terminate_and_wait` / `wait_for_exit` 提供取消与退出等待。

Unix 使用进程组，Windows 使用 Job Object 相关实现。它为调用方清理媒体或工具进程提供基础能力，不持有 Timeline，也不负责 UI。源码存在不代表 Windows 原生进程树与安装器已在此候选验收；证据见[当日验证](../../audit/2026-09-06/public-beta-validation.md)。

[模块目录](INDEX.md) · [模块总目录](../INDEX.md)
