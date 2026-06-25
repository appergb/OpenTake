# 遥测与日志

> **注**：原文档中并无独立的“遥测与日志”章节。相关日志与遥测内容分散于 §4.1（执行壳日志）、§5.5（用量日志）以及 §9.4（安全检查清单）中。此文件为保持目录结构一致性而保留。

相关内容索引：

*   **4.1 执行壳日志**
    包含工具调用开始和结束的 `telemetry`，如工具名称、执行时长、timeline 是否改变等。
*   **5.5 用量日志（`AgentUsageLog.record:73-84`）**
    DEBUG 下打印缓存命中率：`billed = input + cache_creation + cache_read`；`read% = cache_read/billed`。Rust 用 `tracing::debug!`。
