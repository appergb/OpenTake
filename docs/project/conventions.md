# 开发规范

> 状态：canonical · 阶段：implementation-backed · 同步日期：2026-09-06
> 来源：根 AGENTS.md 的开发规则、Cargo.toml、web/package.json 与现有代码契约。入口迁移保留有效规则；覆盖率等要求是目标，不是已测结果。

## 工作区与源码来源

所有实现和构建在 `OpenTake-generation/` 内进行。路径含空格与中文时必须引用；不要在旧对照工作树 `OpenTake/` 实现新功能。上游 `../palmier-pro-upstream/` 只读，移植前按需读取 `Sources/PalmierPro/Models/`、`Editor/`、`Timeline/` 和 `Agent/`。不要改上游，不覆盖用户未提交文件。

## 状态与命令边界

- Rust 持权威 `Timeline`，前端持只读镜像、版本号和 UI 状态。UI、Agent、MCP 编辑最终归一到命令层，撤销/重做在 Rust。
- Overwrite/Ripple/Snap 等编辑算法保持纯函数；领域层禁止文件系统和网络 I/O。`opentake-domain` 是层次叶子，实际有 serde 依赖，不能称“零依赖”。
- 预览与导出复用 RenderPlan/合成语义；像素一致是需按素材、路由和输出格式验证的目标，不是所有组合都已验收的保证。
- IPC 多词字段遵循 camelCase；Rust DTO、前端类型和调用处一起更新。失败必须返回可见错误，禁止吞错或关测试掩盖问题。

## 移植与模型兼容

- 时间线与编辑使用整数帧；秒转帧按具体上游函数语义。`Int(seconds * fps)` 是向零截断，不可替换成 `round()`。
- Swift 默认 `.rounded()` 与 Rust `f64::round()` 的中点均远离零，不是向偶取整。显式指定的上游舍入规则须逐项保留。
- 关键帧存储用 clip 相对帧，公开编辑 API 使用时间线绝对帧；边界转换显式完成。
- `smoothstep(t) = t*t*(3-2*t)`，保持已有插值语义。
- 序列化模型按兼容契约采用 `#[serde(default)]` 与可选字段 `Option<T>`；必填字段和不支持的新格式仍须显式拒绝，不能把未知格式静默视为已支持。

## Rust 与前端风格

内部错误遵循所在 crate 的错误类型（含 anyhow 与类型化错误），到 Tauri 边界转换为 `Err(String)`。单测放 `#[cfg(test)]` 或既有集成测试目录，覆盖命令行为和关键边界；原规范 ≥80% 为覆盖目标，不能写成当前测量值。注释主要解释不明显的原因。

React 组件渲染镜像、派发动作，不另建权威领域模型。像素↔帧是前端投影；领域帧↔秒遵循 Rust 模型，HTML 媒体播放适配仍需在前端换算 seconds。设计数值/颜色使用 `AppTheme` 与 CSS tokens；图标经 lucide-react 和现有 Icon 组件，悬停使用现有 CSS 规范。公共类型保持 TypeScript 严格检查。

Agent 主动指引的设计来源保留在 [Context Signal](../modules/opentake-agent/AGENT-CONTEXT-SIGNAL.md) 与 [工作流插件](../modules/opentake-agent/WORKFLOW-PLUGIN-SYSTEM.md)，不在入口重复维护。

## 构建与验证

在活动工作树运行：

```bash
cargo build
cargo test
cargo fmt --all -- --check
cargo clippy
pnpm --dir web install --frozen-lockfile
pnpm --dir web test
pnpm --dir web build
cargo tauri dev
python3 scripts/check_docs.py
```

Rust 工具链跟随 `rust-toolchain.toml`；依赖版本以 Cargo.lock / web/pnpm-lock.yaml 为准。`web/package.json` 目前没有 lint script，不使用 `pnpm run lint`。FFmpeg sidecar 通过 `scripts/provision_ffmpeg_sidecars.py` 与锁清单准备，不提交二进制。

简单文档改动检查 diff 和链接即可；复杂实现须跑可用测试、类型检查或构建。记录实际命令、结果和环境；浏览器 fallback 测试不等于原生桌面或安装包验收，macOS 证据不替代 Windows/Linux 证据。发布与签名流程以活动计划和候选发布文档为准。

## 文档维护

一个事实一个来源；根目录只留一个 AGENTS.md 自动入口，正文在 docs 按索引渐进读取。历史日期、发布版本与原始证据保留；重要文档记录状态和阶段，设计不得标为 final-verified。新实现与文档在同任务闭环，候选状态、平台实测和已发布状态分开记录。
