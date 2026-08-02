# Beta 1 目标与规划对账 — 2026-08-01

## 对账范围

已检查根目录 README/CHANGELOG、`docs/architecture` 路线与差距文档、
`docs/audit/2026-07-14/implementation-plans` 的十组生成计划、
`docs/superpowers/plans` / `.superpowers/sdd` 的执行报告，以及所有 2026-07-29 至
2026-08-01 的自动化与实机证据。

2026-07-14 的 completion ledger 是审计基线，不是随每个后续实现提交自动回填的动态状态。
它仍保留大量 `incomplete` 与未勾选步骤，即使对应代码、测试和运行证据后来已经落地。
因此本次不改写用户正在生成/维护的四个受保护台账文件，而以提交历史、现行代码测试和
日期更晚的 runtime artifact 作当前裁决。

## Beta 代码状态

首个 Beta 范围内已经没有已知的“只有规划、没有生产入口”的代码项。最后一个确定缺口
`requirement-fdd45062091b48f3`（运动追踪）已在 `73a245c` 补齐：Tauri 命令、Inspector、
主预览拖框、帧范围、取消/重试、置信度/关键帧审阅、Apply、Undo、保存重开与 H.264 导出。

其余近期高级竖切均已有生产入口和自动化证据：

- RVM 抠像、智能擦除、参考色彩匹配；
- 声部分离、响度、降噪；
- 口播清理、字幕翻译、图文成片；
- Motion Canvas、原生 Chromium fallback、Lottie；
- 数字人和音色克隆的 provider、同意、成本、取消、原子导入与撤销边界；
- HDR/代理/账号、保存重开、预览/播放/导出、数据安全与跨平台打包契约。

## 候选包仍需顺序验证

以下不是代码 TODO，而是必须在同一个 `1.0.0-beta.1` 候选包上完成的发布证据：

1. 按 `docs/releases/1.0.0-beta.1.md` 的 11 个顺序阶段执行 macOS GUI 验收。
2. 对抠像、擦除、色彩匹配、运动追踪、声部分离和 Motion 结果做保存重开与最终导出检查。
3. 验证 Agent、生成、数字人、音色克隆在无凭据时明确拒绝，不发生付费请求或部分导入。
4. 构建 Apple Silicon `.app` / `.dmg`，记录 SHA-256、签名状态与 Gatekeeper 结果。
5. 把候选提交推送到远端，并运行 exact-SHA CI；Windows 安装器由该工作流生成。

## 外部条件（不允许伪造）

- 当前没有 fal、ElevenLabs、OpenAI 或 Anthropic key，不能执行真实付费 provider 请求或
  Agent 云模型对话。确定性 fixture 覆盖代码契约；GUI 只验证无凭据、同意、成本、取消和
  错误可见性。真实付费冒烟必须由用户提供 key 并明确授权费用后另行执行。
- 钥匙串只有 Apple Development 身份，没有 Developer ID Application 与公证凭据。
  本地 Beta 可用，但不是面向陌生用户的已公证分发包。
- 当前主机是 macOS，不能伪造 Windows WebView 人工交互。Windows 编译、安装器、sidecar、
  safe-fs 由 GitHub exact-SHA 原生 runner 验证；人工 UI 烟测需 Windows 交互环境。
- Unix 生产原子目录创建因缺少可移植“mkdir 并返回所创建对象 fd”原语而严格拒绝；这是
  安全架构裁决，不是可通过普通补丁消除的测试失败。

## Beta 后续路线（不在本次发布门禁）

Bezier/Spring、RGB 曲线、更多双源转场、本地神经超分、神经语义分轨、曲线变速、
多机位对齐、任意 Motion Canvas TSX/透明序列均保留在后续 Beta。当前产品不展示虚假可用
控件；这些未来目标不会阻止首个本地可用 Beta，但也不会被计为已经完成。
