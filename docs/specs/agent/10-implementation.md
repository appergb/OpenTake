# 实施清单与验证

> 阶段对齐 ROADMAP：Phase 7（MCP+chat+工具）/ Phase S（Context Signal）/ Phase W（Workflow Plugin），三者同步交付。依赖 `opentake-domain`/`opentake-ops`/`opentake-core` 先行（命令层 + 领域模型 + 编辑算法）。

## 9.1 crate 骨架（`crates/opentake-agent/`）

```
src/
├── lib.rs                  # 导出 start_mcp_server / AgentService / ToolExecutor
├── mcp/
│   ├── server.rs           # rmcp StreamableHttpService 装配 + axum + 绑 127.0.0.1:19789（§1.2）
│   ├── guards.rs           # OriginGuardLayer / ContentTypeGuardLayer / ProtocolVersionGuardLayer（§1.2.2）
│   └── resources.rs        # opentake://models/{video,image}（§1.3）
├── tools/
│   ├── names.rs            # ToolName 枚举（31 + activate_workflow 等）
│   ├── descriptions.rs     # 31 条描述（§2.2，原样 const / include_str!）
│   ├── schemas.rs          # 入参 JSON Schema（§2.3）
│   ├── args.rs             # 各工具 DecodableToolArgs（allowedKeys + serde 结构体，§4）
│   ├── executor.rs         # ToolExecutor::execute 统一壳（§4.1）
│   ├── short_id.rs         # 短 ID 出站/入站（§3）
│   ├── errors.rs           # ToolError + serde_path_to_error 路径化（§4.2.3）
│   └── encode_timeline.rs  # get_timeline 压缩编码（§8.3）
├── chat/
│   ├── service.rs          # AgentService agentic loop（§5.3/5.6）
│   ├── client.rs           # trait AgentClient + AnthropicClient（BYOK）+ OpenTakeProxyClient（§5.1）
│   ├── sse.rs              # AnthropicSSE 解析（§5.2）
│   ├── request.rs          # AnthropicRequestBody + prompt cache（§5.4）
│   ├── session.rs          # ChatSession 持久化（§5.8）
│   └── keychain.rs         # keyring 存取 anthropic-api-key（§5.1）
├── signal/
│   ├── engine.rs           # ContextSignalEngine::attach（§6.1）
│   ├── classify.rs         # 视频类型检测（§6.3）
│   ├── track_roles.rs      # 轨道角色检测 + advice 常量（§6.4）
│   ├── stages.rs           # editing_stage + stage_guidance + editing_skeleton（§6.3/§6.5）
│   └── rules.rs            # 内置规则校验 + warning 常量（§6.6.1）
├── plugin/
│   ├── model.rs            # PluginManifest serde（§7.1）
│   ├── registry.rs         # PluginRegistry 加载/激活/校验（§7.2/§7.3）
│   ├── inject.rs           # instructions.md → system prompt 组装（§6.5）
│   └── rules.rs            # 插件 rules 校验（§6.6.2）
└── prompt/
    ├── base.rs             # 分层基础系统提示词（§6.5.1）
    └── assemble.rs         # base + 激活插件组装（§6.5）
```

## 9.2 任务清单（按依赖序）

**Phase 7 — MCP + chat + 工具（核心）**
1. [ ] `tools/names.rs` + `tools/descriptions.rs`：31 工具名 + 描述原样落地（产品名替换为 OpenTake，URI `opentake://`）。— 验证：描述与 §2.2 行号逐条对拍。
2. [ ] `tools/short_id.rs`：§3 出站缩短 + 入站展开 + 歧义报错。— 验证：§3.4 四个对拍用例（与 Swift 一致）。
3. [ ] `tools/errors.rs` + `tools/args.rs`：`serde_path_to_error` 路径化 + `allowedKeys` 未知字段拒绝 + 非有限数拒绝。— 验证：构造 `entries[3].startFrame` 缺失/类型错/未知字段/NaN，输出措辞与 §4.2 一致。
4. [ ] `tools/executor.rs`：§4.1 统一壳（快照→展开→run→undo 记账→signal→缩短）。
5. [ ] `tools/encode_timeline.rs`：§8.3 压缩编码（默认值剥离、captionGroups 折叠 200 行、浮点 3 位、窗口分页）。
6. [ ] `mcp/server.rs` + `mcp/guards.rs` + `mcp/resources.rs`：rmcp + axum 绑 `127.0.0.1:19789` + 三个 tower layer + 2 resources + 偏好开关幂等。— 验证：`claude mcp add` 连通；每工具走通；伪造 Origin/外网 IP 被拒。
7. [ ] `chat/`：`trait AgentClient` + `AnthropicClient`（BYOK，keyring）+ SSE 解析 + 请求体 cache_control + agentic loop + 孤儿修复 + 会话持久化。— 验证：应用内 chat 多步链式编辑；助手专属 undo 正确；DEBUG 缓存命中率日志非零。
8. [ ] `opentake-core` 对接：实现/对接 `CoreHandle`（§8.2），31 工具映射到 `EditCommand`/转发。

**Phase S — Context Signal（随 Phase 7）**
9. [ ] `opentake-domain` 增 `ContextSignal`/`TrackRole`/`VideoType`/`EditingStage` 等类型（§6.2，Phase A 已规划）。
10. [ ] `signal/classify.rs` + `signal/track_roles.rs`：§6.3/§6.4 检测规则 + advice/skeleton 常量（文本原样）。
11. [ ] `signal/engine.rs` + `signal/rules.rs`：§6.1 按工具附信号 + §6.6.1 内置规则校验。— 验证（ROADMAP Phase S `:110`）：`get_timeline` 结果含 `context_signal`；轨道角色标注正确；规则不匹配产 warning。

**Phase W — Workflow Plugin（随 Phase 7）**
12. [ ] `plugin/model.rs` + `plugin/registry.rs`：§7.1/§7.2 plugin.json 加载 + 校验 + instructions.md 读入。
13. [ ] `tools/`：`activate_workflow`（+ `list_workflows`/`deactivate_workflow`）工具（§7.4）。
14. [ ] `prompt/assemble.rs` + `plugin/inject.rs`：§6.5 base + 插件 instructions.md 组装；激活后重建 system。
15. [ ] `plugin/rules.rs`：§6.6.2 插件 do/dont 校验，与内置规则组合（顺序：内置→插件）。— 验证（ROADMAP Phase W `:119`）：`activate_workflow` 激活后 system 含 instructions.md；后续工具结果含插件阶段提示与规则 warning。

**OpenTake 增强（ARCHITECTURE §7 `:154`，可后置）**
16. [ ] 系统提示词分层化 + 模型策略从 `opentake-gen` 配置注入（§6.5.1）。
17. [ ] 高阶工具 `remove_filler_words` / `tighten_silences`（把易错帧算术在 Rust 内一次完成）。
18. [ ] 写工具统一返回结构化 JSON（§4.4 增强）。
19. [ ] `get_capabilities`（一次性返回 ASR/视觉索引/生成/编解码就绪状态）。

## 9.3 测试要求（对应 testing 规则 80% 覆盖 + 与上游对拍）

- **单元（纯逻辑，可全覆盖）**：短 ID 缩短/展开（§3）、错误路径格式化（§4.2）、未知字段/非有限数校验、get_timeline 压缩编码（默认值剥离/captionGroups 折叠/浮点 3 位）、视频类型检测规则、轨道角色检测规则、plugin.json 解析容错、提示词组装、cache_control 边界（请求体 JSON 结构）。
- **集成**：MCP `initialize`/`tools/list`/`tools/call` 端到端（rmcp 客户端打本地 server）；Origin/ContentType/版本 guard 的拒绝路径；chat agentic loop（mock `AgentClient` 吐 SSE → 工具执行 → 再请求 → end_turn）；孤儿修复后消息序列合法性。
- **安全**：外网 IP / 伪造 Origin 必须被拒（DNS-rebinding）；keychain 不落工程文件；BYOK key 不进日志。
- **对拍**：短 ID 算法、错误措辞、压缩编码字段集与上游 Swift 输出逐条比对（关键的 LLM 行为相关项）。

## 9.4 安全检查清单（提交前）

- [ ] MCP server **仅绑 `127.0.0.1`**，禁 `0.0.0.0`；三个 tower guard 生效。
- [ ] Anthropic key 存 OS keychain（`keyring`），绝不入 `project.json`/日志/遥测。
- [ ] 工具入参全部经 §4 校验（未知字段/非有限数/类型）；`import_media` 的 url 必须 HTTPS + 扩展名白名单 + ≤1GB（上游 `+Import.swift` 语义）。
- [ ] 错误信息不泄露内部路径/密钥（错误措辞对 LLM 友好但不含敏感数据）。
- [ ] 插件 `instructions.md` 注入系统提示词前不可信内容隔离（标注来源 `plugin:{id}`，避免提示注入冒充系统指令）。

---

## 附：与上游的关键差异（OpenTake 决策）

| 项 | 上游 PalmierPro | OpenTake |
|---|---|---|
| MCP 传输 | NWListener 手写 TCP + SDK StatelessHTTPServerTransport | rmcp `streamable-http-server`（axum）自带，省手写外壳 |
| Origin 校验 | SDK `OriginValidator.localhost` | tower layer 手写等价（rmcp 不内置） |
| 计费通道 | Clerk JWT → Convex `/v1/agent/stream`（闭源云） | 去掉或换 OpenTake 自建 `opentake-gen-proxy`；MVP 只留 BYOK |
| 系统提示词 | 单块字符串，模型策略写死 | 分层可组合 + 模型策略从配置注入 + 插件 instructions.md 叠加 |
| Context Signal | 无 | **新增**：每工具返回附 `context_signal`（§6） |
| Workflow Plugin | 无 | **新增**：plugin.json + `activate_workflow`（§7） |
| 写工具结果 | 多为人话字符串 | 统一结构化 JSON（§4.4 增强） |
| 进程模型 | 单进程，UI/MCP 天然一致 | 跨进程，Rust 持权威 Timeline + version 广播（§8.4） |
| 工具计数 | 31（30 case + undo） | 31 对等 + `activate_workflow` 等 Agent 层工具 + 增强工具 |
