---
status: draft
content_stage: partial-implementation
updated: 2026-09-06
---

# OpenTake Public Beta 执行计划

## 目标与意图基线

同步全部项目 Markdown 及相关版本、索引、能力记录；完成既有完整功能收敛清单，验证稳定可用后发布新的公开 Beta；由独立 GPT-6 子代理交付可播放官方宣传视频。用户已授权实施、电脑操作与发布，不重复询问已确认范围。

现有公开版本为 `v1.0.0-beta.1` 至 `v1.0.0-beta.5`（2026-09-06 GitHub 实查）；下一候选默认 `1.0.0-beta.6`，不覆盖已公开 tag/资产，不将 Beta 宣称为稳定 1.0。核心架构保持 Rust 权威 Timeline → EditCommand → Tauri → React 只读镜像。

## Read Set 与事实来源

- 工作区 `AGENTS.md`、本项目 `AGENTS.md`、`docs/INDEX.md`。
- `docs/capabilities/requirements.json`、`CAPABILITY-LEDGER.md`、`docs/audit/2026-08-21/full-desktop-functional-matrix.md`。
- `docs/superpowers/plans/2026-08-21-opentake-full-ui-and-upstream-convergence.md`。
- `docs/releases/1.0.0-beta.5.md`、`.github/workflows/ci.yml`、`.github/workflows/release.yml`、版本清单。
- 按实际失败点读取模块总览、测试与对应只读 Swift 源码；不递归加载整个文档树。

## 基线与影响范围

- 活动工作树 `OpenTake-generation`，分支 `release/v1.0.0-beta.5`，初始 HEAD `33ee8e2`，比远端同名分支领先 74 个提交。
- 既有未提交改动：audit 下 5 个截图删除、2 个工程 JSON 修改，另有录屏、诊断、`marketing/`、`output/` 等未跟踪文件。保留，不纳入本次提交；测试使用新临时工程。
- 只读参考：`OpenTake/`、`palmier-pro-upstream/`；共享 git 元数据的普通操作不等于修改参考树源码。
- 文档覆盖本项目维护的 Markdown 与工作区路由；第三方依赖、只读上游不重写。历史证据保留日期及历史属性，不批量伪装为最新验收。
- 宣传子代理 Euler，模型 `gpt-6-astra`，id `01a0755a-fa74-7630-be67-cec27b352859`，独占 `marketing/official-beta-2026-09/`；不改产品源码、不推送、不发布。
- 文档子代理 Carson（`01a0755e-4412-7271-afe6-3eda663f4a6c`）负责维护Markdown及链接审计；主代理保留当前计划、Beta6说明、当前总验收和capability账本写入。
- 语义搜索子代理 Carver（`01a07561-3b74-7942-aa82-33f496255011`）补齐真实模型下载/校验/推理；Sticker子代理 Banach（`01a07566-bf36-7993-9a39-153678618e5b`）负责媒体贴纸面板纵切。各自写集与主代理分离，统一最终测试。

## 方案与里程碑

- [x] M1：核对工作树、变更、实际公开版本、能力清单；委派视频制作。
- [ ] M2：本地 baseline：Rust fmt/clippy/workspace test、Web test/build、脚本合同/许可证与依赖审计；按失败点最小修复。
- [ ] M3：从能力清单逐项完成核心闭环及安装包 GUI 验收，补齐未完成能力，更新可复现证据；未验证项保留明确状态。
- [ ] M4：统一入口和开发规范、当前发布说明、README 多语言、模块状态、历史元数据、链接/版本一致性；保留历史事实。
- [ ] M5：候选版本清单、最终 diff 审查、独立必要复核、本地最终构建；完成 PR/远端 exact-SHA CI。
- [ ] M6：合并已验证候选至 main，创建新的不可变 tag，等待完整 release pipeline；回读发布资产与校验结果。
- [ ] M7：视频 MP4、封面、可复现源工程、素材来源与视觉/解码验收；发布文档链接到实际交付。

## 测试验收

- `cargo fmt --all --check`；`cargo clippy --workspace --all-targets -- -D warnings`；`cargo test --workspace --jobs 1 -- --test-threads=1`；minimal clippy 和播放传输集成。
- `pnpm -C web exec vitest run --maxWorkers=1`；`pnpm -C web build`；按 Node 实际行为处理测试 localStorage 隔离，不使用旧缓存制造通过。
- CI/release/sidecar/许可证合同测试、Motion 锁定依赖及构建；按既有发布 workflow 执行平台 gate。
- 新打包 app：新建/导入/多轨播放与 seek/剪切/撤销重做/文本与效果/Motion/保存重开/真实导出及取消；媒体输出用 ffprobe 与完整解码校验。
- Windows 与 macOS 各自绑定实际构建源，不能把 macOS 结果当作 Windows GUI 证明。
- 视频：1920×1080 H.264 MP4 优先，首中尾抽帧、全片解码、规格/时长/来源记录。

## 风险与回滚

- 未获得云服务所需凭据/费用授权的能力只能验证无凭据拒绝，不能伪造成功；上游私有账号后台不得凭空实现。
- 现有发布需 updater signing secrets、GitHub 权限、远端 CI；任何失败不绕过门禁。
- 原有发布资产和 tag 不可变；候选回退到已公开 beta.5，工程先另存副本。
- 不替换已有本地工程和未提交证据；新产物保存独立路径。入口合并先保留有效规范到 docs，再删除重复入口。

## 执行记录

- 2026-09-06：完成基线读取。`codexhost` CLI 不存在，采用当前原生 multi_agent 工具，明确指定可用 `gpt-6-astra`。GitHub Release 历史及最近流水线已读取。
- 2026-09-06：切换 `release/v1.0.0-beta.6`，Cargo/Web/Tauri/WiX及发布身份合同已同步。Rust fmt/clippy通过；Web1429tests/build通过；脚本209、sidecar7、Motion2测试通过；两套npm审计0项。Rust workspace测试仍执行，尚未创建PR/tag/Release。详细证据见当前总验收。
- 2026-09-06：只读确认电脑工具能访问OpenTake；视频代理正在其独立演示工程采集，主代理未干扰其导出操作。已生成独立10s H.264/AAC测试夹具及PNG用于后续验收。

- 2026-09-06 收尾：本轮首个 Rust workspace 全量 2885/0/10（passed/failed/ignored）；原生 FFmpeg/Motion/导出/播放集成、音频时钟探针和真实 Keychain/MCP 两项通过。GPT-6 宣传片 60 秒 1080p 已交付并解码复核。搜索整批索引挤占交互查询的问题增加有界排队与明确 busy 状态；Windows Tract 实际模型兼容阻塞修复中。用户再次明确直接向 GitHub 提交包含全部更新的新版本，授权持续有效。
