# opentake-process-tree — 总览

> 状态：draft · 阶段：implementation-backed · 同步日期：2026-11-16

进程树生命周期支持 crate，位于 [Cargo workspace](../../../Cargo.toml)；实现见 [lib.rs](../../../crates/opentake-process-tree/src/lib.rs)。`configure_command` 配置平台进程启动，`ProcessTree::attach` 绑定子进程，`terminate` / `terminate_and_wait` / `wait_for_exit` 提供取消与退出等待。

Windows 启动策略分两种：`background_command` 创建带 `CREATE_NO_WINDOW` 的普通命令，子进程启动后立即运行，无需 `attach`；`configure_command` 设置 `CREATE_NEW_PROCESS_GROUP | CREATE_SUSPENDED | CREATE_NO_WINDOW`，隐藏控制台的同时保留挂起启动，调用方仍须在 spawn 成功后立即调用 `ProcessTree::attach`，先加入 Job Object 再恢复执行。Unix 上 `background_command` 返回普通命令，`configure_command` 创建独立进程组。

Unix 使用进程组，Windows 使用 Job Object 相关实现。它为调用方清理媒体或工具进程提供基础能力，不持有 Timeline，也不负责 UI。源码存在不代表 Windows 原生进程树与安装器已在此候选验收；证据见[当日验证](../../audit/2026-09-06/public-beta-validation.md)。

[模块目录](INDEX.md) · [模块总目录](../INDEX.md)
