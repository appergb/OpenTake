---
id: capabilities.readme
title: OpenTake Capability Evidence
summary: 说明 capability manifest 与人工 ledger 的状态含义和证据门槛。
kind: engineering
status: draft
content_stage: partial-implementation
scope:
  - all-modules
triggers:
  - capability ledger
  - 功能清单
  - 完成度
read_when:
  - 新增、补齐或验收用户可见能力
skip_when:
  - 只改内部实现且没有公共行为变化
priority: must
freshness_class: project
last_verified: 2026-08-21T10:06:00+08:00
owners:
  - OpenTake-generation
source_of_truth:
  - ./requirements.json
  - ./CAPABILITY-LEDGER.md
related:
  prerequisites: []
  next:
    - ../audit/2026-08-21/full-desktop-functional-matrix.md
supersedes: []
tags:
  - capabilities
  - evidence
---

# Capability Evidence

`requirements.json` 保存稳定 ID 和机器可读字段；`CAPABILITY-LEDGER.md` 保存当前人工审计结论和证据索引。状态含义：

- `implemented`：代码链路存在，但本轮尚未完成匹配范围的自动化/桌面证据。
- `verified`：代码、相关测试和匹配范围的最新运行证据都存在。
- `partial`：部分层或部分场景可用，仍有明确缺口。
- `missing`：当前没有可用实现。
- `blocked`：实现路径或验收依赖外部服务、凭据、硬件或未解决环境问题。

AX 节点存在不等于 verified；浏览器/fallback 测试也不等于安装版桌面证据。

## Related Documents

- [Capability ledger](./CAPABILITY-LEDGER.md)
- [Desktop functional matrix](../audit/2026-08-21/full-desktop-functional-matrix.md)
- [Long-term plan](../superpowers/plans/2026-08-21-opentake-full-ui-and-upstream-convergence.md)
