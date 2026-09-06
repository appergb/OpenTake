---
status: canonical
content_stage: implementation-backed
retrieved: 2026-09-06
freshness_class: rapid
valid_until: 2026-09-07
confidence: high
---

# Beta 候选依赖修复来源

发布前需实时审计，本记录不代替发布安全门禁。

| 依赖 | 项目原版本 | 目标版本 | 来源与结论 |
|---|---|---|---|
| Web/PostCSS → nanoid | 3.3.16 | 3.3.18 | [维护者发布](https://github.com/ai/nanoid/releases/tag/3.3.18)及[GitHub公告](https://github.com/advisories/GHSA-2v37-7h3g-55p8)；审计要求3.x分支至少3.3.18，避免零长度自定义生成器的无限循环。锁文件仅更新此传递依赖。 |
| Motion Canvas → speech-rule-engine → @xmldom/xmldom | 0.9.10 | 0.9.12 | [维护者安全公告](https://github.com/xmldom/xmldom/security/advisories/GHSA-6gmq-8vp8-gcm6)说明无效 EntityReference 名称可注入序列化XML。speech-rule-engine 4.1.4 精确依赖旧版，使用 npm override 保持其4.x及Motion Canvas3.17.2，锁定修复版本。 |

本轮实际 `pnpm -C web audit --audit-level moderate` 与 Motion Canvas `npm audit --audit-level=moderate` 均为0项；Motion测试、许可证和runner构建通过。此结论限定本次解析出的依赖图；应用自身未直接使用公告中的危险API，升级不代表已验证每条第三方执行路径。
