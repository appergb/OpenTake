# 全项目 Markdown 同步报告 — 2026-09-06

> 状态：generated · 阶段：implementation-backed · 文档审计日期：2026-09-06。
> 本报告记录文档同步与静态源码核对，不代替候选功能、GUI、安装包或远端发布验收。

工作树：`OpenTake-generation/`。起点为 `release/v1.0.0-beta.5` / `33ee8e2`，origin 为 `appergb/OpenTake`。工作期间主代理切到 `release/v1.0.0-beta.6` 并更新版本清单；Beta 1–5 已发布，Beta 6 仍是未发布候选。文档子任务未执行发布；主线已将候选提交至 [PR #249](https://github.com/appergb/OpenTake/pull/249)，tag/公开发布状态以当日验证记录为准。

当前入口：[活动计划](plans/active/2026-09-06-public-beta.md) · [Beta 6 候选](releases/1.0.0-beta.6.md) · [当日验证](audit/2026-09-06/public-beta-validation.md) · [文档总目录](INDEX.md) · [开发规范](project/conventions.md)。

## 覆盖方式与边界

`scripts/check_docs.py` 通过 Git 枚举 tracked + 非忽略 untracked Markdown（含 `.MD` 与隐藏的 `.superpowers` 历史报告），逐文件扫描可点击的 Markdown 内联链接、引用定义和 HTML href/src 的本地文件目标，检查自动入口命名。代码块/行内代码中的历史命令和路径不当作链接。完整机器清单可运行 `python3 scripts/check_docs.py --json` 获取。

覆盖包括维护文档、设计来源、日期化历史、运行时 Markdown 资源和第三方归属记录。排除 `marketing/`、`output/`、`.playwright-cli/`、构建/依赖目录；其中视频切片、媒体输出由其他所有者维护。主代理指定的计划、Beta 6 发布说明、能力账本及 `docs/audit/2026-09-06/` 只读进入清单。

“逐文件覆盖”是路径/链接与状态分类审计，不代表全部历史正文重新逐句验真。正文同步重点为项目入口、README 三语、CHANGELOG、CONTRIBUTING、架构、各模块 OVERVIEW/INDEX、已变化子系统。历史测试日志、上游映射和旧设计保持原时点含义，不批量改成 canonical/final-verified。外部 URL、Markdown 标题 fragment、代码块中的机器绝对路径不由本脚本验证。

## 主要同步结果

- 根 `AGENTS.md` 压缩为项目摘要、技术栈、意图、风险和路由；有效开发规则迁入 `docs/project/conventions.md`，补 `docs/project/index.md` 与 `intent.md`。当前活动工作树、origin 和候选版本已纠正。
- `CLAUDE.md` 明确 historical，索引不再把它当作当前交接。`docs/port-map/agent.md` 改名 `agent-port-map.md`，有效上游映射保留、所有 Markdown 引用更新；当前仓库只有根 `AGENTS.md` 自动入口。
- 三语 README 与 CHANGELOG 区分已发布 Beta 1–5、Beta 6 候选和源码新增。平台表不再把跨平台目标等同于已验证 Windows/Linux 发行；语义搜索改为模型安装及 macOS 真实 Rust 链路已验证，保留首次安装要求与 Windows/新包 UI 边界；外部 MCP 配对不再写“未来才开放”。
- 模块路由补缺失的 `opentake-process-tree`：Cargo 有 10 个库 crate + Tauri 壳，共 11 个成员。OVERVIEW/INDEX 区分源码和候选验收，早期规格保留设计状态。
- 播放架构更新 temporal compositor、Lottie 与能力路由；导出文档更新 H.264/H.265/ProRes 422/4444、alpha、进度、取消和失败清理。媒体 Text/Effect 和 Agent 面板不再写未接线；预览 tab、媒体视图有当前路由。
- 修复旧审计/实现计划中从模块目录复制后失效的 `dispatch-tools.md`、`renderer.md`、`probe-ff.md`、SPEC 和 architecture 相对链接，保留日期和原始证据正文。

## 74 个提交的源码核对

执行 `git rev-list --count aae0ae6..33ee8e2` 得到 74；`git log` 逐条核对该范围（包含文档与测试提交，不能称 74 项新功能，也不将该范围直接等同完整 Beta 5 tag diff）。

| 领域 | 提交依据 | 定向源码与结论 |
|---|---|---|
| 多预览 tab | `bd06d62`、`4c8b761` | `web/src/store/uiStore.ts`、Preview 组件；多 tab、旧状态归一存在 |
| 媒体视图/导入 | `745a190`、`5b1f45d`、`0bd7c6a` | MediaPanel 与导入动作；folder/flat/grouped、悬空 folder 归一、原生导入修复 |
| temporal compositor | `a833764`、`1013355`、`dafd5eb`、`3e124dc` | `web/src/components/preview/playbackRoute.ts` 与 native transport；合成时间线倒放/速度路由和发布流重置 |
| 透明 Motion | `e106bc3`、`ba85202`、`5e3b5b7`、`6ce80fe` | `src-tauri/src/motion.rs`、Motion UI；透明发布、编辑 alpha 保留与 Inspector 开关 |
| ProRes 4444 | `cca9327` | `src-tauri/src/export.rs` 与 media encode；透明 `.mov` 编码路径 |
| 时间线与链接行为 | `a7a372e`、`478e1b4`、`f05bac8`、`3b8ba9b`、`76e700b` | ops / editActions / timeline 手势；范围选择、选区驱动修剪/分割、链接移动、Option trim |
| 导出与播放失败 | `0bae80e`、`a7d98d6`、`eb1cc1a`、`741ff07` | 导出清理/取消/身份与缺失素材拒绝路径 |
| Lottie | `c1db732`、`2ed8a61` | Lottie 素材和原生播放接入；不能继续列为路由必然 unsupported |
| 文本/特效面板 | `17bcb8c`、`d87b74f`、`33ee8e2` | TextTab 调 addTextClip；EffectTab 向单个视觉选区追加预设，保留既有 effects，链接音轨不增加视觉计数 |

## 实际代码与旧文档冲突

| 旧说法 | 当前依据与处理 |
|---|---|
| domain 零依赖 | Cargo.toml 实际依赖 serde；改为无 I/O 叶子 |
| Swift/Rust round 中点向偶 | 两者默认就近中点远离零；纠正规范，secondsToFrame 截断语义另列 |
| web 有 lint script | `web/package.json` 只有 dev/build/preview/test；CONTRIBUTING 改用真实脚本，build 包含 TypeScript 检查 |
| 全工作区 9 crate / 11 模块 | Cargo 实际 11 成员（10 库 + shell），加 web 为 12 个文档模块 |
| Beta 2 当前入口、Beta 5 未发布候选 | 当前 Beta 6 未发布候选；旧 Beta 日期/发布文件留存为历史 |
| 透明 Motion/ProRes 4444 未做 | 本轮之前的 74 提交中已有实现；更新当前总览与导出说明，旧设计注明时点 |
| Text/Effect/Agent 面板占位 | HEAD 已有接线；源码、浏览器测试、原生 GUI 验收分别记账 |
| MCP OAuth 元数据为空等于无需认证 | 生产桌面有 Bearer 与工程 gate；空 authorization_servers 不表示无认证 |
| 语义搜索模型仍为占位/修复中 | 原空 hash/bytes=0 已修复；约 1.5 GB 固定 revision 资产校验、macOS 真实 Rust 离线安装/图文 embedding/排名均通过。使用前需安装模型；Windows ort-tract 真实图及新包 UI 待验收 |
| Sticker 仍为占位 | 核对起点是禁用占位；Banach 已完成图片/Lottie 展示、导入、预览、拖拽/落轨和工程身份隔离，4 files / 114 tests 与 build 通过；新包原生 GUI 待主线验收 |

未遇到必须由文档切片决定的新增产品意图冲突。Sticker 已由实现切片完成接线并通过定向 Web 检查，原生 GUI 仍待验收；语义搜索模型实现与 macOS 真实 Rust 链路已验证，Windows ort-tract 真实图及新包 UI 仍待。文档分别记录实现和验收状态。

Sticker 的实现与定向验证见[专项审计](audit/2026-09-06/sticker-panel.md)，该记录由实现切片维护。

语义模型状态补记：实现切片的[最终专项审计](audit/2026-09-06/semantic-search-model.md)记录固定 revision 的约 1.5 GB 资产逐项校验通过；正式 Cargo media search **84 passed / 1 ignored**、Tauri search **14 passed**，引用实际产品模块的真实模型 Rust harness **72 passed / 0 ignored**。这些是不同且有重叠的验证集合，不能求和；真实 Rust 链路已覆盖离线安装、图文 embedding 和排名。Windows ort-tract 真实图及新包 UI 仍待主线验收，不据此宣称全平台、整仓或发布包通过。

## 验证记录与遗留

文档切片执行：`python3 scripts/check_docs.py`、脚本针对链接/代码块/分类的内存检查、Markdown 范围 `git diff --check`，并审阅本切片最终 diff。无新增依赖，无 Rust/TS/package/lock 修改；未重跑与此文档切片无关的构建/GUI/付费 provider。

主代理提供的较早一轮状态（非本切片执行，未包含随后新增的 Sticker 用例）：Web 152 files / 1429 tests 通过，使用每文件 happy-dom Storage 隔离，命令 `pnpm -C web exec vitest run --maxWorkers=1`，无需 NODE_OPTIONS 或残留 localStorage 文件；workspace Clippy exit 0；脚本 209/209、sidecar 7/7 通过。Carver 跨 crate 中间态已由正式 Tauri search 14 项验证解决；主线整仓统一测试/审查仍在执行，不记整仓最终通过。依赖安全、运行验证和发布状态以当日审计后续更新为准。

用户预先删除的 5 张截图保持删除，未恢复：`09-dual-track-playback-frame-239.png`、`11-dual-track-seek-pause-frame-115.png`、`14-gui-export-completed.png`、`17-relaunch-editor-thumbnails-composite.png`、`18-relaunch-source-preview.png`（均在 `docs/audit/2026-08-07/editor-core-after-fix-assets/`）。已有工程 JSON、视频、日志、诊断素材保持原状。历史正文中的已删除证据文件名保留，不当作本轮可用证据。

后续由主代理在源码切片完成后统一复验并更新候选/当日审计，审阅文档 diff 后再决定发布。本报告不会把这些待执行门槛写成已经通过。

## 全部 Markdown 覆盖清单

<!-- INVENTORY -->

本次快照覆盖 **402** 份 Markdown、**1891** 个本地文件链接；普通断链 **0**，待生成链接 **0**。本切片已同步/新增 **71** 份 Markdown（含本报告）；历史保留类别 **188** 份，其中可能仅修复迁移链接。分类数量与变更数量为不同维度，不可直接相加。协作过程中新增文件可能改变下次审计计数。

| 分类 | 数量 |
|---|---:|
| 主代理维护 | 7 |
| 历史保留 | 188 |
| 生成的同步报告 | 1 |
| 维护文档 | 120 |
| 设计与来源参考 | 83 |
| 运行资源或归属记录 | 3 |

逐文件状态：`同步` = 本轮文档 diff 或新增（不含主线切片文件）；`保留` = 内容未更改但已完成链接/分类扫描；`主代理` = 主线及实现切片只读覆盖。历史/设计文件的状态解释由分类给出，不会把路径审计提升成完整功能验证。

| 路径 | 分类 | 本轮处理 | 本地文件链接数 |
|---|---|---|---:|
| `.superpowers/sdd/2026-08-13-beta5-agent-conversation/task-1-report.md` | 历史保留 | 保留 | 0 |
| `.superpowers/sdd/2026-08-13-beta5-agent-conversation/task-2-report.md` | 历史保留 | 保留 | 0 |
| `.superpowers/sdd/2026-08-13-beta5-agent-conversation/task-3-report.md` | 历史保留 | 保留 | 0 |
| `.superpowers/sdd/2026-08-13-beta5-agent-conversation/task-4-report.md` | 历史保留 | 保留 | 0 |
| `.superpowers/sdd/2026-08-13-beta5-external-mcp/task-6-report.md` | 历史保留 | 保留 | 0 |
| `.superpowers/sdd/2026-08-13-beta5-interface-polish/task-2-report.md` | 历史保留 | 保留 | 0 |
| `.superpowers/sdd/2026-08-13-beta5-interface-polish/task-3-report.md` | 历史保留 | 保留 | 0 |
| `.superpowers/sdd/2026-08-13-beta5-interface-polish/task-4-report.md` | 历史保留 | 保留 | 0 |
| `.superpowers/sdd/2026-08-13-beta5-interface-polish/task-5-report.md` | 历史保留 | 保留 | 0 |
| `.superpowers/sdd/2026-08-13-beta5-interface-polish/task-6-report.md` | 历史保留 | 保留 | 0 |
| `.superpowers/sdd/2026-08-13-beta5-motion-studio/task-1-report.md` | 历史保留 | 保留 | 0 |
| `.superpowers/sdd/2026-08-13-beta5-motion-studio/task-2-report.md` | 历史保留 | 保留 | 0 |
| `.superpowers/sdd/2026-08-13-beta5-motion-studio/task-3-report.md` | 历史保留 | 保留 | 0 |
| `.superpowers/sdd/2026-08-13-beta5-motion-studio/task-4-report.md` | 历史保留 | 保留 | 0 |
| `.superpowers/sdd/2026-08-13-beta5-motion-studio/task-5-report.md` | 历史保留 | 保留 | 0 |
| `.superpowers/sdd/2026-08-13-beta5-motion-studio/task-6-report.md` | 历史保留 | 保留 | 0 |
| `.superpowers/sdd/2026-08-13-beta5-motion-studio/task-7-report.md` | 历史保留 | 保留 | 0 |
| `.superpowers/sdd/2026-08-21-preview-temporal-remap-parity/task-2-report.md` | 历史保留 | 保留 | 0 |
| `.superpowers/sdd/2026-08-21-preview-temporal-remap-parity/task-3-report.md` | 历史保留 | 保留 | 0 |
| `.superpowers/sdd/task-11-red-replay.md` | 历史保留 | 保留 | 0 |
| `.superpowers/sdd/task-11-report.md` | 历史保留 | 保留 | 1 |
| `.superpowers/sdd/task-5-agent-chat-report.md` | 历史保留 | 保留 | 0 |
| `.superpowers/sdd/task-5-export-freeze-report.md` | 历史保留 | 保留 | 0 |
| `.superpowers/sdd/task-5-media-library-report.md` | 历史保留 | 保留 | 0 |
| `.superpowers/sdd/task-5-render-report.md` | 历史保留 | 保留 | 0 |
| `AGENTS.md` | 维护文档 | 同步 | 8 |
| `CHANGELOG.md` | 维护文档 | 同步 | 4 |
| `CLAUDE.md` | 历史保留 | 同步 | 2 |
| `CONTRIBUTING.md` | 维护文档 | 同步 | 5 |
| `DECISIONS.md` | 维护文档 | 保留 | 0 |
| `README.ja.md` | 维护文档 | 同步 | 24 |
| `README.md` | 维护文档 | 同步 | 25 |
| `README.zh-CN.md` | 维护文档 | 同步 | 25 |
| `THIRD_PARTY_NOTICES.md` | 运行资源或归属记录 | 保留 | 0 |
| `crates/opentake-agent/src/plugin/builtin/audio-first/instructions.md` | 运行资源或归属记录 | 保留 | 0 |
| `docs/INDEX.md` | 维护文档 | 同步 | 54 |
| `docs/architecture/ADVANCED-FEATURES.md` | 维护文档 | 同步 | 2 |
| `docs/architecture/ARCHITECTURE.md` | 维护文档 | 同步 | 4 |
| `docs/architecture/BUGS.md` | 历史保留 | 同步 | 6 |
| `docs/architecture/CAPCUT-GAP.md` | 历史保留 | 同步 | 2 |
| `docs/architecture/EDITING-ENGINE-PLAN.md` | 维护文档 | 同步 | 4 |
| `docs/architecture/FULL_PROJECT_SCAN_REPORT.md` | 历史保留 | 同步 | 13 |
| `docs/architecture/HANDOFF-2026-07.md` | 历史保留 | 同步 | 9 |
| `docs/architecture/INDEX.md` | 维护文档 | 同步 | 22 |
| `docs/architecture/MODULE-PORT-MAP.md` | 维护文档 | 同步 | 2 |
| `docs/architecture/PLAYBACK-ENGINE.md` | 维护文档 | 同步 | 4 |
| `docs/architecture/PORT-1TO1-GAP.md` | 历史保留 | 同步 | 2 |
| `docs/architecture/ROADMAP.md` | 历史保留 | 同步 | 7 |
| `docs/architecture/STEM-SEPARATION.md` | 维护文档 | 同步 | 2 |
| `docs/architecture/UPDATER.md` | 维护文档 | 同步 | 2 |
| `docs/architecture/editing-automation/EDITING-AUTOMATION-DOS.md` | 维护文档 | 保留 | 13 |
| `docs/architecture/editing-automation/EDITING-AUTOMATION/acceptance-tests.md` | 维护文档 | 保留 | 9 |
| `docs/architecture/editing-automation/EDITING-AUTOMATION/agent-editing-suggestions.md` | 维护文档 | 保留 | 8 |
| `docs/architecture/editing-automation/EDITING-AUTOMATION/auto-crop-smart-reframe.md` | 维护文档 | 保留 | 8 |
| `docs/architecture/editing-automation/EDITING-AUTOMATION/beat-sync-auto-cut.md` | 维护文档 | 保留 | 8 |
| `docs/architecture/editing-automation/EDITING-AUTOMATION/workflow-plugin-recipes.md` | 维护文档 | 保留 | 7 |
| `docs/architecture/editing-automation/README.md` | 维护文档 | 保留 | 19 |
| `docs/audit/2026-07-14/beta-1-convergence-2026-08-01.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/completion-report.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/document-reconciliation.md` | 历史保留 | 同步 | 30 |
| `docs/audit/2026-07-14/implementation-plan-index.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/implementation-plans/accessibility-polish-design.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/implementation-plans/accessibility-polish-implementation.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/implementation-plans/agent-settings-generation-design.md` | 历史保留 | 同步 | 5 |
| `docs/audit/2026-07-14/implementation-plans/agent-settings-generation-implementation.md` | 历史保留 | 同步 | 5 |
| `docs/audit/2026-07-14/implementation-plans/command-contracts-design.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/implementation-plans/command-contracts-implementation.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/implementation-plans/data-safety-design.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/implementation-plans/data-safety-implementation.md` | 历史保留 | 保留 | 6 |
| `docs/audit/2026-07-14/implementation-plans/documentation-design.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/implementation-plans/documentation-implementation.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/implementation-plans/home-shell-design.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/implementation-plans/home-shell-implementation.md` | 历史保留 | 保留 | 5 |
| `docs/audit/2026-07-14/implementation-plans/inspector-text-keyframes-design.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/implementation-plans/inspector-text-keyframes-implementation.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/implementation-plans/media-library-design.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/implementation-plans/media-library-implementation.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/implementation-plans/media-render-playback-export-design.md` | 历史保留 | 同步 | 25 |
| `docs/audit/2026-07-14/implementation-plans/media-render-playback-export-implementation.md` | 历史保留 | 同步 | 31 |
| `docs/audit/2026-07-14/implementation-plans/preview-timeline-design.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/implementation-plans/preview-timeline-implementation.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/interface-traces.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/agent-lottie-inspect-real-device-2026-08-01.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/ai-matting-vertical-2026-08-01.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/avatar-voice-clone-vertical-2026-08-01.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/beta-1-sequential-validation-2026-08-01.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/bounded-audio-streaming-real-device-2026-08-01.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/caption-translation-vertical-2026-08-01.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/cli-sidecar-boundary-real-device-2026-08-01.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/color-match-vertical-2026-08-01.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/data-safety-cache-identity-real-device-2026-07-31.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/data-safety-cross-cutting-security-partial-2026-07-31.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/data-safety-generation-seed-real-device-2026-07-31.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/data-safety-legacy-default-matrix-real-device-2026-07-31.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/data-safety-manifest-corruption-real-device-2026-07-31.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/data-safety-mcp-redaction-real-device-2026-07-31.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/data-safety-mcp-tool-import-real-device-2026-07-31.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/data-safety-mcp-transport-real-device-2026-07-31.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/data-safety-project-open-composite-real-device-2026-07-31.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/data-safety-shared-core-command-real-device-2026-07-31.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/data-safety-windows-safe-fs-native-2026-07-31.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/denoise-real-device-2026-08-01.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/ffmpeg-license-replacement-2026-08-01.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/five-panel-layout-real-device-2026-07-31.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/generation-finalization-2026-07-29.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/generic-effects-real-device-2026-07-31.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/hdr-proxy-account-real-device-2026-08-01.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/headless-chromium-real-device-2026-08-01.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/home-autosave-metadata-real-device-2026-07-31.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/home-component-mapping-real-device-2026-07-31.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/home-new-project-real-device-2026-07-31.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/home-open-project-real-device-2026-07-30.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/home-project-lifecycle-real-device-2026-07-31.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/home-sample-project-real-device-2026-07-31.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/home-secondary-controls-real-device-2026-07-30.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/home-upstream-composite-real-device-2026-07-31.md` | 历史保留 | 保留 | 5 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/hsl-secondary-real-device-2026-07-31.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/interchange-export-real-device-2026-07-30.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/lgg-real-device-2026-07-31.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/linked-audio-real-device-2026-08-01.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/loudness-real-device-2026-08-01.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/lut-real-device-2026-07-31.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/mask-rendering-2026-07-31.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/media-render-packaged-ffmpeg-real-device-2026-07-31.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/motion-tracking-agent-backend-2026-08-01.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/nested-timeline-compound-real-device-2026-07-31.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/object-removal-vertical-2026-08-01.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/optical-flow-24-to-60-real-device-2026-07-31.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/playback-route-lifecycle-real-device-2026-08-01.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/recent-project-card-real-device-2026-07-30.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/schema-safe-persistence-real-device-2026-07-31.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/script-to-video-vertical-2026-08-01.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/stabilization-real-device-2026-07-31.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/stem-separation-agent-vertical-2026-08-01.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/stems-real-device-2026-08-01.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/subtitle-export-real-device-2026-07-30.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/talking-head-cleanup-2026-07-29.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/titlebar-controls-real-device-2026-07-30.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/transition-real-device-2026-07-31.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/view-menu-contract-real-device-2026-07-31.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/runtime-artifacts/automated/view-menu-real-device-2026-07-30.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/task8-core-slice-validation.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/task8-ledger-slice-validation.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/task8-ui-slice-validation.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-07-14/upstream-downstream.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-08-02/beta-functional-verification.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-08-07/editor-core-after-fix-assets/README.md` | 历史保留 | 保留 | 1 |
| `docs/audit/2026-08-07/editor-core-after-fix-assets/installation-ledger.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-08-07/editor-core-after-fix-assets/playback-av-real-device.md` | 历史保留 | 保留 | 3 |
| `docs/audit/2026-08-07/editor-core-remediation-assets/p0-asset-scope-import.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-08-07/editor-core-remediation-assets/proxy-regression.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-08-07/editor-core-remediation-assets/resource-scheduler.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-08-07/editor-core-remediation-assets/safe-asset-authority.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-08-07/editor-core-remediation-assets/ui-semantics.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-08-07/editor-core-remediation-matrix.md` | 历史保留 | 保留 | 3 |
| `docs/audit/2026-08-07/editor-core-validation-assets/README.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-08-07/editor-core-validation-assets/competitor-official-sources.md` | 历史保留 | 保留 | 23 |
| `docs/audit/2026-08-07/editor-core-validation-assets/fresh-gui-ledger.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-08-07/editor-core-validation-assets/resource-ui-static.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-08-07/editor-core-validation-evidence-b/README.md` | 历史保留 | 保留 | 1 |
| `docs/audit/2026-08-07/editor-core-validation-evidence-b/command-ledger.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-08-07/editor-core-validation.md` | 历史保留 | 保留 | 35 |
| `docs/audit/2026-08-08/release-module-tests/competitor-gap.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-08-08/release-module-tests/compositor-render.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-08-08/release-module-tests/import-export.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-08-08/release-module-tests/motion-canvas-supply-chain.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-08-08/release-module-tests/playback-fallback-fix.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-08-08/release-module-tests/playback.md` | 历史保留 | 保留 | 6 |
| `docs/audit/2026-08-08/release-module-tests/project-media-scope-hardening.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-08-08/release-module-tests/release-workflow.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-08-08/release-module-tests/resource-loading.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-08-08/release-module-tests/ui-blockers-fix.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-08-08/release-module-tests/ui.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-08-08/release-module-tests/windows-ci-hardening-fix.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-08-08/release-module-tests/windows-ci-retest.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-08-08/release-module-tests/windows-ci.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-08-10/final-module-validation.md` | 历史保留 | 保留 | 1 |
| `docs/audit/2026-08-13/beta5-external-mcp.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-08-13/beta5-motion-studio.md` | 历史保留 | 保留 | 2 |
| `docs/audit/2026-08-13/beta5-release-candidate.md` | 历史保留 | 保留 | 0 |
| `docs/audit/2026-08-21/full-desktop-functional-matrix.md` | 历史保留 | 保留 | 3 |
| `docs/audit/2026-09-06/public-beta-validation.md` | 主代理维护 | 主代理 | 2 |
| `docs/audit/2026-09-06/semantic-search-model.md` | 主代理维护 | 主代理 | 1 |
| `docs/audit/2026-09-06/sticker-panel.md` | 主代理维护 | 主代理 | 0 |
| `docs/capabilities/CAPABILITY-LEDGER.md` | 主代理维护 | 主代理 | 6 |
| `docs/capabilities/README.md` | 维护文档 | 保留 | 3 |
| `docs/documentation-sync-2026-09-06.md` | 生成的同步报告 | 同步 | 6 |
| `docs/knowledge/2026-09-06-beta-dependencies.md` | 维护文档 | 同步 | 0 |
| `docs/knowledge/2026-09-06-semantic-search-model.md` | 主代理维护 | 主代理 | 1 |
| `docs/modules/INDEX.md` | 维护文档 | 同步 | 34 |
| `docs/modules/opentake-agent/AGENT-CONTEXT-SIGNAL.md` | 维护文档 | 保留 | 5 |
| `docs/modules/opentake-agent/INDEX.md` | 维护文档 | 同步 | 31 |
| `docs/modules/opentake-agent/OVERVIEW.md` | 维护文档 | 同步 | 15 |
| `docs/modules/opentake-agent/SPEC.md` | 设计与来源参考 | 同步 | 2 |
| `docs/modules/opentake-agent/WORKFLOW-PLUGIN-SYSTEM.md` | 维护文档 | 保留 | 4 |
| `docs/modules/opentake-agent/context-signal.md` | 维护文档 | 保留 | 12 |
| `docs/modules/opentake-agent/core-handle-convert.md` | 维护文档 | 保留 | 15 |
| `docs/modules/opentake-agent/dispatch-tools.md` | 维护文档 | 保留 | 14 |
| `docs/modules/opentake-agent/mcp-server.md` | 维护文档 | 同步 | 16 |
| `docs/modules/opentake-agent/plugin-system.md` | 维护文档 | 保留 | 14 |
| `docs/modules/opentake-agent/prompt.md` | 维护文档 | 保留 | 12 |
| `docs/modules/opentake-core/INDEX.md` | 维护文档 | 同步 | 22 |
| `docs/modules/opentake-core/OVERVIEW.md` | 维护文档 | 同步 | 27 |
| `docs/modules/opentake-core/SPEC.md` | 设计与来源参考 | 同步 | 5 |
| `docs/modules/opentake-core/core-router.md` | 维护文档 | 保留 | 17 |
| `docs/modules/opentake-core/deps-di.md` | 维护文档 | 保留 | 13 |
| `docs/modules/opentake-core/dto.md` | 维护文档 | 保留 | 19 |
| `docs/modules/opentake-core/events-bus.md` | 维护文档 | 保留 | 17 |
| `docs/modules/opentake-core/session.md` | 维护文档 | 保留 | 17 |
| `docs/modules/opentake-domain/INDEX.md` | 维护文档 | 同步 | 33 |
| `docs/modules/opentake-domain/OVERVIEW.md` | 维护文档 | 同步 | 11 |
| `docs/modules/opentake-domain/keyframe-transform.md` | 维护文档 | 保留 | 11 |
| `docs/modules/opentake-domain/media-signal.md` | 维护文档 | 保留 | 7 |
| `docs/modules/opentake-domain/split-subtitle.md` | 维护文档 | 保留 | 13 |
| `docs/modules/opentake-domain/text-grade.md` | 维护文档 | 保留 | 10 |
| `docs/modules/opentake-domain/timeline-model.md` | 维护文档 | 保留 | 15 |
| `docs/modules/opentake-gen/INDEX.md` | 维护文档 | 同步 | 57 |
| `docs/modules/opentake-gen/OVERVIEW.md` | 维护文档 | 同步 | 28 |
| `docs/modules/opentake-gen/SPEC.md` | 设计与来源参考 | 同步 | 2 |
| `docs/modules/opentake-gen/catalog.md` | 维护文档 | 保留 | 14 |
| `docs/modules/opentake-gen/client-transport.md` | 维护文档 | 保留 | 18 |
| `docs/modules/opentake-gen/keys-byok.md` | 维护文档 | 保留 | 13 |
| `docs/modules/opentake-gen/params.md` | 维护文档 | 保留 | 15 |
| `docs/modules/opentake-gen/providers.md` | 维护文档 | 保留 | 21 |
| `docs/modules/opentake-media/INDEX.md` | 维护文档 | 同步 | 28 |
| `docs/modules/opentake-media/OVERVIEW.md` | 维护文档 | 同步 | 21 |
| `docs/modules/opentake-media/SPEC.md` | 设计与来源参考 | 同步 | 2 |
| `docs/modules/opentake-media/analysis.md` | 维护文档 | 保留 | 13 |
| `docs/modules/opentake-media/decode.md` | 维护文档 | 保留 | 20 |
| `docs/modules/opentake-media/encode.md` | 维护文档 | 保留 | 14 |
| `docs/modules/opentake-media/library-index.md` | 维护文档 | 保留 | 14 |
| `docs/modules/opentake-media/probe-ff.md` | 维护文档 | 保留 | 11 |
| `docs/modules/opentake-media/semantic-search.md` | 维护文档 | 同步 | 19 |
| `docs/modules/opentake-media/thumbnail.md` | 维护文档 | 保留 | 13 |
| `docs/modules/opentake-media/transcribe.md` | 维护文档 | 保留 | 13 |
| `docs/modules/opentake-media/waveform.md` | 维护文档 | 保留 | 14 |
| `docs/modules/opentake-motion/INDEX.md` | 维护文档 | 同步 | 21 |
| `docs/modules/opentake-motion/MOTION-GRAPHICS-PLUGIN.md` | 维护文档 | 同步 | 1 |
| `docs/modules/opentake-motion/OVERVIEW.md` | 维护文档 | 同步 | 16 |
| `docs/modules/opentake-motion/cache.md` | 维护文档 | 保留 | 10 |
| `docs/modules/opentake-motion/integration.md` | 维护文档 | 保留 | 11 |
| `docs/modules/opentake-motion/manifest-source.md` | 维护文档 | 保留 | 12 |
| `docs/modules/opentake-motion/renderer.md` | 维护文档 | 保留 | 10 |
| `docs/modules/opentake-motion/sandbox.md` | 维护文档 | 保留 | 10 |
| `docs/modules/opentake-ops/INDEX.md` | 维护文档 | 同步 | 17 |
| `docs/modules/opentake-ops/OVERVIEW.md` | 维护文档 | 同步 | 15 |
| `docs/modules/opentake-ops/command-apply.md` | 维护文档 | 保留 | 8 |
| `docs/modules/opentake-ops/engines.md` | 维护文档 | 保留 | 5 |
| `docs/modules/opentake-ops/intent-id.md` | 维护文档 | 保留 | 7 |
| `docs/modules/opentake-ops/ops-algorithms.md` | 维护文档 | 保留 | 8 |
| `docs/modules/opentake-process-tree/INDEX.md` | 维护文档 | 同步 | 5 |
| `docs/modules/opentake-process-tree/OVERVIEW.md` | 维护文档 | 同步 | 5 |
| `docs/modules/opentake-project/INDEX.md` | 维护文档 | 同步 | 17 |
| `docs/modules/opentake-project/OVERVIEW.md` | 维护文档 | 同步 | 18 |
| `docs/modules/opentake-project/bundle-archive.md` | 维护文档 | 保留 | 9 |
| `docs/modules/opentake-project/fcpxml-export.md` | 维护文档 | 保留 | 4 |
| `docs/modules/opentake-project/gen-log.md` | 维护文档 | 保留 | 6 |
| `docs/modules/opentake-project/layout.md` | 维护文档 | 保留 | 10 |
| `docs/modules/opentake-render/INDEX.md` | 维护文档 | 同步 | 20 |
| `docs/modules/opentake-render/OVERVIEW.md` | 维护文档 | 同步 | 16 |
| `docs/modules/opentake-render/SPEC.md` | 设计与来源参考 | 同步 | 2 |
| `docs/modules/opentake-render/gpu-compositor.md` | 维护文档 | 保留 | 12 |
| `docs/modules/opentake-render/render-plan.md` | 维护文档 | 保留 | 10 |
| `docs/modules/opentake-render/source-size.md` | 维护文档 | 保留 | 8 |
| `docs/modules/opentake-render/text-rasterizer.md` | 维护文档 | 保留 | 7 |
| `docs/modules/src-tauri/INDEX.md` | 维护文档 | 同步 | 27 |
| `docs/modules/src-tauri/OVERVIEW.md` | 维护文档 | 同步 | 20 |
| `docs/modules/src-tauri/commands-ipc.md` | 维护文档 | 保留 | 15 |
| `docs/modules/src-tauri/export.md` | 维护文档 | 同步 | 9 |
| `docs/modules/src-tauri/library-media.md` | 维护文档 | 保留 | 16 |
| `docs/modules/src-tauri/mcp.md` | 维护文档 | 保留 | 11 |
| `docs/modules/src-tauri/render.md` | 维护文档 | 保留 | 13 |
| `docs/modules/src-tauri/secret.md` | 维护文档 | 保留 | 10 |
| `docs/modules/src-tauri/setup-lib.md` | 维护文档 | 保留 | 24 |
| `docs/modules/web/INDEX.md` | 维护文档 | 同步 | 28 |
| `docs/modules/web/OVERVIEW.md` | 维护文档 | 同步 | 9 |
| `docs/modules/web/SPEC.md` | 设计与来源参考 | 同步 | 2 |
| `docs/modules/web/hooks-i18n-theme.md` | 维护文档 | 保留 | 9 |
| `docs/modules/web/ipc-api.md` | 维护文档 | 保留 | 11 |
| `docs/modules/web/panels-ui.md` | 维护文档 | 同步 | 13 |
| `docs/modules/web/preview-ui.md` | 维护文档 | 同步 | 13 |
| `docs/modules/web/state-stores.md` | 维护文档 | 保留 | 11 |
| `docs/modules/web/timeline-ui.md` | 维护文档 | 保留 | 11 |
| `docs/plans/active/2026-09-06-public-beta.md` | 主代理维护 | 主代理 | 0 |
| `docs/port-map/account.md` | 设计与来源参考 | 保留 | 0 |
| `docs/port-map/agent-port-map.md` | 设计与来源参考 | 同步 | 1 |
| `docs/port-map/app.md` | 设计与来源参考 | 保留 | 0 |
| `docs/port-map/editor.md` | 设计与来源参考 | 保留 | 0 |
| `docs/port-map/export.md` | 设计与来源参考 | 保留 | 0 |
| `docs/port-map/generation.md` | 设计与来源参考 | 保留 | 0 |
| `docs/port-map/help.md` | 设计与来源参考 | 保留 | 0 |
| `docs/port-map/inspector.md` | 设计与来源参考 | 保留 | 0 |
| `docs/port-map/mediapanel.md` | 设计与来源参考 | 保留 | 0 |
| `docs/port-map/models.md` | 设计与来源参考 | 保留 | 0 |
| `docs/port-map/preview.md` | 设计与来源参考 | 保留 | 0 |
| `docs/port-map/project.md` | 设计与来源参考 | 保留 | 0 |
| `docs/port-map/search.md` | 设计与来源参考 | 保留 | 0 |
| `docs/port-map/settings.md` | 设计与来源参考 | 保留 | 0 |
| `docs/port-map/telemetry.md` | 设计与来源参考 | 保留 | 0 |
| `docs/port-map/timeline.md` | 设计与来源参考 | 保留 | 0 |
| `docs/port-map/toolbar.md` | 设计与来源参考 | 保留 | 0 |
| `docs/port-map/transcription.md` | 设计与来源参考 | 保留 | 0 |
| `docs/port-map/ui.md` | 设计与来源参考 | 保留 | 0 |
| `docs/port-map/utilities.md` | 设计与来源参考 | 保留 | 0 |
| `docs/project/conventions.md` | 维护文档 | 同步 | 2 |
| `docs/project/index.md` | 维护文档 | 同步 | 5 |
| `docs/project/intent.md` | 维护文档 | 同步 | 2 |
| `docs/releases/1.0.0-beta.1.md` | 历史保留 | 保留 | 0 |
| `docs/releases/1.0.0-beta.2.md` | 历史保留 | 保留 | 0 |
| `docs/releases/1.0.0-beta.3.md` | 历史保留 | 保留 | 0 |
| `docs/releases/1.0.0-beta.4.md` | 历史保留 | 保留 | 1 |
| `docs/releases/1.0.0-beta.5.md` | 历史保留 | 保留 | 2 |
| `docs/releases/1.0.0-beta.6.md` | 主代理维护 | 主代理 | 4 |
| `docs/specs/INDEX.md` | 设计与来源参考 | 保留 | 46 |
| `docs/specs/agent/0-evidence-index.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/agent/1-mcp-server.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/agent/10-implementation.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/agent/2-tools.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/agent/3-short-id.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/agent/4-execution-shell.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/agent/5-chat.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/agent/6-context-signal.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/agent/7-system-prompt.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/agent/8-core-dispatch.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/agent/9-telemetry.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/core/0-design-baseline.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/core/1-editor-state.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/core/2-command-routing.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/core/3-event-bus.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/core/4-frontend-sync.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/core/5-assembly.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/core/6-tauri-commands.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/core/7-security.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/core/8-implementation.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/frontend/0-principles.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/frontend/1-design-tokens.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/frontend/10-state.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/frontend/11-tauri.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/frontend/12-data-models.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/frontend/13-implementation.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/frontend/2-layout.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/frontend/3-components.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/frontend/4-toolbar.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/frontend/5-timeline.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/frontend/6-inspector.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/frontend/7-media-panel.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/frontend/8-preview.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/frontend/9-interactions.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/media/0-principles.md` | 设计与来源参考 | 保留 | 1 |
| `docs/specs/media/1-structure.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/media/10-acceptance.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/media/11-implementation.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/media/2-ffmpeg.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/media/3-thumbnails.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/media/4-waveform.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/media/5-search.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/media/6-transcribe.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/media/7-ort-worker.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/media/8-coordinator.md` | 设计与来源参考 | 保留 | 0 |
| `docs/specs/media/9-domain-contract.md` | 设计与来源参考 | 保留 | 0 |
| `docs/superpowers/archive/2026-07-08-branch-integration-register.md` | 历史保留 | 保留 | 0 |
| `docs/superpowers/archive/2026-07-08-editing-automation-dos-session.md` | 历史保留 | 保留 | 0 |
| `docs/superpowers/archive/2026-07-08-verification-report.md` | 历史保留 | 保留 | 1 |
| `docs/superpowers/archive/2026-07-10-playback-cache-installed-app-qa.md` | 历史保留 | 保留 | 0 |
| `docs/superpowers/archive/2026-07-10-wave1a-baseline-disposition.md` | 历史保留 | 保留 | 0 |
| `docs/superpowers/archive/2026-07-13-cloud-integration-release-report.md` | 历史保留 | 保留 | 0 |
| `docs/superpowers/plans/2026-07-08-opentake-recovery-integration.md` | 历史保留 | 保留 | 0 |
| `docs/superpowers/plans/2026-07-10-opentake-wave-1a-integration-baseline.md` | 历史保留 | 保留 | 0 |
| `docs/superpowers/plans/2026-07-11-opentake-wave-1b-schema-compatibility.md` | 历史保留 | 保留 | 0 |
| `docs/superpowers/plans/2026-07-12-opentake-wave-1b-c1a-fail-closed-export.md` | 历史保留 | 保留 | 0 |
| `docs/superpowers/plans/2026-07-12-opentake-wave-1b-c1b-safe-filesystem.md` | 历史保留 | 保留 | 0 |
| `docs/superpowers/plans/2026-07-14-opentake-completion-audit.md` | 历史保留 | 保留 | 0 |
| `docs/superpowers/plans/2026-07-18-long-media-playback-handoff.md` | 历史保留 | 保留 | 0 |
| `docs/superpowers/plans/2026-08-10-opentake-beta4-release.md` | 历史保留 | 保留 | 0 |
| `docs/superpowers/plans/2026-08-13-beta5-agent-conversation.md` | 历史保留 | 保留 | 0 |
| `docs/superpowers/plans/2026-08-13-beta5-external-mcp.md` | 历史保留 | 保留 | 0 |
| `docs/superpowers/plans/2026-08-13-beta5-interface-polish.md` | 历史保留 | 保留 | 0 |
| `docs/superpowers/plans/2026-08-13-beta5-motion-studio.md` | 历史保留 | 保留 | 0 |
| `docs/superpowers/plans/2026-08-13-beta5-release.md` | 历史保留 | 保留 | 0 |
| `docs/superpowers/plans/2026-08-21-file-import-dialog.md` | 历史保留 | 保留 | 0 |
| `docs/superpowers/plans/2026-08-21-media-view-modes.md` | 历史保留 | 保留 | 0 |
| `docs/superpowers/plans/2026-08-21-opentake-full-ui-and-upstream-convergence.md` | 历史保留 | 保留 | 0 |
| `docs/superpowers/plans/2026-08-21-preview-tabs-parity.md` | 历史保留 | 保留 | 0 |
| `docs/superpowers/plans/2026-08-21-preview-temporal-remap-parity.md` | 历史保留 | 保留 | 0 |
| `docs/superpowers/plans/c1b/2026-07-12-c1b-common-unix-normative.md` | 历史保留 | 保留 | 0 |
| `docs/superpowers/plans/c1b/2026-07-12-c1b-windows-ci-normative.md` | 历史保留 | 保留 | 0 |
| `docs/superpowers/specs/2026-07-08-opentake-recovery-integration-design.md` | 设计与来源参考 | 保留 | 0 |
| `docs/superpowers/specs/2026-07-10-opentake-full-convergence-design.md` | 设计与来源参考 | 保留 | 0 |
| `docs/superpowers/specs/2026-07-12-opentake-wave-1b-c-filesystem-security-design.md` | 设计与来源参考 | 保留 | 0 |
| `docs/superpowers/specs/2026-07-14-opentake-completion-audit-design.md` | 设计与来源参考 | 保留 | 0 |
| `docs/superpowers/specs/2026-08-13-opentake-beta5-design.md` | 设计与来源参考 | 保留 | 0 |
| `docs/upstream-analysis/01-架构与数据流.md` | 设计与来源参考 | 保留 | 0 |
| `docs/upstream-analysis/02-苹果框架可移植性.md` | 设计与来源参考 | 保留 | 0 |
| `docs/upstream-analysis/03-闭源云边界.md` | 设计与来源参考 | 保留 | 0 |
| `docs/upstream-analysis/04-MCP与Agent工具.md` | 设计与来源参考 | 保留 | 0 |
| `docs/upstream-analysis/README.md` | 设计与来源参考 | 保留 | 0 |
| `docs/需求与问题汇总.md` | 维护文档 | 保留 | 0 |
| `plugins/motion-canvas-studio/README.md` | 维护文档 | 保留 | 0 |
| `plugins/motion-canvas-studio/THIRD_PARTY_NOTICES.md` | 运行资源或归属记录 | 保留 | 0 |
| `src-tauri/README.md` | 维护文档 | 同步 | 3 |
| `src-tauri/resources/ffmpeg/SOURCE.md` | 维护文档 | 保留 | 0 |
| `update-summary/2026-06-27-ui-refresh.md` | 历史保留 | 保留 | 0 |
