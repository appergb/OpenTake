---
status: canonical
content_stage: implementation-backed
retrieved: 2026-09-06
freshness_class: normal
valid_until: 2026-09-16
confidence: high
---

# Tauri 搜索任务调度

项目使用 Tauri 2（Cargo.lock 解析到 2.11.3）。[官方命令文档](https://v2.tauri.app/develop/calling-rust/#async-commands)说明：默认同步命令在主线程执行；async命令在异步任务执行，适合避免重任务卡住界面。

本次真实SigLIP2模型接通后，模型字节校验、初始化和推理是实际重任务。原`search_query`同步完成这些工作，`search_index_start`虽然提交后台worker，但同步等待结果，都会占住调用线程。

本轮将两个命令改为async，并通过`spawn_blocking`等待CPU任务。视觉查询复用现有有界`OrtWorker`，优先级为Interactive，索引为Background；两者共享已校验模型的registry，避免并发加载多份大型模型。没有新增调度依赖或另一套模型缓存。`visualError`是向后兼容的可选结果字段，文件名与字幕关键词结果仍保留。

此文记录官方执行规则和代码改动；真实安装包的响应性以当前候选验收为准。

排队复核补充：现有 worker 不抢占已经开始的整批索引。视觉查询采用 250 ms 的排队等待上限；仍未启动时持有与 worker 状态转换相同的锁取消该查询，返回 SEARCH_VISUAL_BUSY，已开始/完成的任务保留正常结果。队列满也返回相同忙状态。独立源码复核与 channel 阻塞索引的回归用例通过，避免用优先级名称误称存在抢占。

2026-09-07原生GUI进一步确认同一规则影响export_video：进程sample显示主AppKit线程同步运行完整GPU/FFmpeg导出，导致进度和取消无法处理。导出改用async+spawn_blocking并让后台任务持有ExportGuard；转写也沿既有有界推理worker后台执行。同步命令不会自动在worker执行，相关旧注释已纠正。
