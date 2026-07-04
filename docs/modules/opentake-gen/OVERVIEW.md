# opentake-gen — 模块总览

> 上级：[opentake-gen 目录](INDEX.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
>
> 模块/子系统级总览（不逐函数）。完整规格见 [SPEC.md](SPEC.md)（只读，本总览只链接、不复述）。

---

## 一句话定位 + 依赖分层

**生成式 AI 客户端**：把文/图/音/视频生成与 AI 二次编辑（放大/重跑）的请求，以**一套调用面、两种模式**发往厂商或自建后端，结果回流时间线。核心特征：**BYOK 无后端**（用户自带 key 直连 fal.ai/Replicate/OpenAI/ElevenLabs，内置静态模型目录）+ 可选托管 proxy。

依赖分层（能力层 crate，依赖只能向下）：

```
opentake-domain    ← 仅依赖它（消费/re-export GenerationInput）
   ▲
opentake-gen       ← 本模块（叶子能力，不依赖其它能力 crate）
   ▲
opentake-agent / src-tauri   ← 调用方（agent 工具 / Tauri 命令）
```

外部依赖：`reqwest`(rustls-tls) / `tokio` / `futures-util` / `async-trait` / `thiserror` / `anyhow` / `url` / `keyring` / `serde`。**BYOK 无自有后端**——这是与上游最大的架构差异。

## 职责边界

**负责**：

- 生成参数的线协议联合类型（`GenerationParams`）与从领域 `GenerationInput` 的装配。
- 统一任务抽象（`GenerationJob`）+ 提交/轮询到终态（`GenClient`）。
- BYOK 直连四厂商的 provider 适配 + 路由。
- 数据驱动的模型目录（能力矩阵 + 计价）与客户端成本预估（仅展示）。
- BYOK 密钥的系统钥匙串存取。
- HTTP 传输抽象（生产 reqwest / 测试 mock，全套离线）与错误码契约。

**不负责**（交给别处）：

- 帧↔秒换算、占位片段落轨、撤销栈 → [`opentake-domain`](../opentake-domain/INDEX.md) / [`opentake-ops`](../opentake-ops/INDEX.md)。
- 结果文件下载、媒体清单导入、缩略图/转写 → [`opentake-media`](../opentake-media/INDEX.md) / [`opentake-project`](../opentake-project/INDEX.md)。
- MCP 工具暴露与 Agent 编排、IPC 命令、钥匙串 Tauri 命令外壳 → [`opentake-agent`](../opentake-agent/INDEX.md) / [`src-tauri`](../src-tauri/INDEX.md)。
- 生成面板 UI / 拖拽参考 / 成本显示 → 前端（`ui-rebuild`）。

## 关键概念与数据流

| 概念 | 说明 | 文档 |
|---|---|---|
| **BYOK 无后端** | 用户自带 key，客户端直连厂商；模型目录是编译进二进制的静态资产；无需任何自建服务即可工作 | [keys-byok](keys-byok.md) / [catalog](catalog.md) |
| **多 Provider 抽象** | `ProviderAdapter` trait + `ProviderRegistry` 按模型 id 前缀（`fal`/`replicate`/`openai`/`elevenlabs`）路由 | [providers](providers.md) |
| **模型目录** | `CatalogEntry` + 四类能力矩阵 + 计价，UI/Agent 数据驱动的单一真相源（公理 A5） | [catalog](catalog.md) |
| **生成参数** | `GenerationParams`（按 `kind` 标签联合）逐字段复刻上游；`build_params` 从 `GenerationInput` 按固定上传顺序装配 | [params](params.md) |
| **Job 流程** | `GenerationJob`（queued→running→succeeded/failed）掩盖厂商异步差异；`GenClient.watch` 轮询到终态 | [client-transport](client-transport.md) |
| **双模一套面** | `AuthMode::Bearer`（托管 proxy + 计费）/ `Byok`（本地直连），调用面相同（公理 A7） | [client-transport](client-transport.md) |

典型 BYOK 数据流：

```
GenerationInput(持久化) + uploaded URLs
  → build_params(...)                       # 装配线协议载荷
  → GenClient::submit_byok(model, params)   # 前缀路由到 adapter，HttpTransport 发请求
  → GenClient::watch(job_id)                # 按间隔轮询，queued/running 继续
  → 终态 succeeded → first_result_url()     # 取结果 URL（下载/落轨由下游负责）
```

托管路径相同，仅 `submit`/`get` 改打 proxy REST + Bearer JWT。

## 对应上游 Swift

上游 Palmier Pro 的「Generation」子系统，verdict **`cloud-rebuild`**（详见 [`../../architecture/MODULE-PORT-MAP.md`](../../architecture/MODULE-PORT-MAP.md)「Generation」段）。映射关系：

| OpenTake（本 crate） | 上游 Swift | 移植 verdict |
|---|---|---|
| `GenClient` 双模 | `GenerationBackend`（Convex 薄层，Combine 订阅） | cloud-rebuild（不保留 Convex，自建 REST/WebSocket） |
| `GenerationParams` / `build_params` | `BackendGenerationParams` + 四 `*GenerationSubmission.buildParams` | direct-port（平台无关编辑逻辑） |
| `GenerationJob` / `JobStatus` / `GenError` | `BackendGenerationJob`/`Status` + `PalmierClientError` | direct-port |
| `Catalog` / `CatalogEntry` / `*Caps` | `ModelCatalog` / `CatalogEntry`（Convex `models:list` 订阅） | cloud-rebuild（改内置静态/proxy 下发） |
| `cost.rs` | `CostEstimator` | direct-port |
| `keys.rs` | `KeychainStore` / `AnthropicKeychain` | needs-replacement（跨平台 keyring） |
| provider/* | （上游无——一切走 Convex 云） | OpenTake 新增 |

尚未复刻的上游编排（属后续）：引用预处理（`VideoTrimExtractor`/`VideoCompressor`，需 FFmpeg）、上传缓存、Rerun 复原（`EditSubmitter`）、AI 编辑动作矩阵（`EditAction`）、生成面板（`GenerationView`，`ui-rebuild`）。

## 完成状态：已实现 vs 计划中

对照 [SPEC.md](SPEC.md) §6 实施清单、[`../../architecture/ROADMAP.md`](../../architecture/ROADMAP.md) Phase 9、[`../../architecture/PORT-1TO1-GAP.md`](../../architecture/PORT-1TO1-GAP.md) 与代码：

**已实现（库层完整、离线单测覆盖）**

- `GenerationParams` 四变体线协议 + `build_params` 五装配（含上传顺序切分契约）。
- `GenerationJob` 状态机 + 终态校验；`GenError` 错误码契约 + `map_http_error`。
- `GenClient` 双模调用面（list_models/submit/get/watch/sign_upload/upload_reference）+ `can_generate`。
- 四个 provider adapter（fal/replicate/openai/elevenlabs）submit/poll/upload + 状态归一化 + 同步厂商缓存回放。
- `Catalog` + `CatalogEntry` 自定义反序列化 + 四类 caps + 内置静态目录；`cost.rs` 全套计价。
- `KeyStore` + 跨平台 `KeyringStore` + `MemoryKeyStore`。
- `HttpTransport` + `ReqwestTransport` / `MockTransport`（全套测试零 socket）。
- **已接线**：`list_models` 工具已从存根接到内置静态目录（agent `mcp/gen_catalog.rs`，ROADMAP #111）；BYOK 钥匙串 save/load/delete Tauri 命令（聊天 LLM key）。

**计划中 / 未接线**

- **`generate_*` / `upscale_media` 仍待 async + BYOK 接线**：agent dispatch 层这四个工具目前是诚实存根（`"...: not yet implemented"`），缺 async `GenClient` 装配 + `ProviderRegistry` + BYOK key 注入。
- **托管 proxy `opentake-gen-proxy` 未实现**（Phase 9 自建后端：`/v1/models`、`/v1/generations`、`/v1/uploads/sign`、SSE stream、对象存储预签名、可选积分计费；SPEC §3）。客户端侧已就绪，等服务端。
- **生成 provider（fal/replicate/elevenlabs）key 的前端写入命令尚未开放**（`secret.rs` 白名单当前只含聊天三 provider）。
- 引用预处理 / 上传缓存 / Rerun / 成本显示 UI / `ModelPreferences` 等编排与 UI 层未实现。
- 同步厂商字节结果当前裹 `data:` URL（无对象存储时的权宜）；正式部署应改持久化 S3/R2。

## 移植铁律 + 安全

**移植铁律**（对照上游复刻时遵守）：

- 线协议字段口径逐字照抄上游：**全大写 URL 键**（`imageURLs`/`sourceVideoURL`/…）、`generateAudio` 默认 `true`、`num_images` 钳 `1..=4`。
- **省略而非空**：`None` / 空集合一律不序列化（对齐 `encodeIfPresent`/`if !x.isEmpty`）。
- **所有反序列化模型加默认 + `Option<T>`**，读旧/新工程不破坏（`GenerationJob` 全可选、`CatalogEntry` 兜底忽略未知字段、`alias = "_id"`）。
- 上传顺序硬契约：**frames → image → video → audio** 扁平化后再切分。
- 时间单位约定：面向模型/成本=秒，面向时间线=帧（帧↔秒换算属下游）。
- 计价一律 `ceil` 向上取整、`≤0` 记 0。

**安全**：

- **密钥存系统钥匙串、不硬编码**：API key 只进 OS 钥匙串（macOS Keychain / Windows Credential Manager / Linux Secret Service）与 Rust 后端；**绝不进 JS 内存 / 设置文件 / localStorage**。明文单向（前端只发 `secret_save`，回传只给掩码）。详见 [keys-byok](keys-byok.md)。
- 错误不外泄内部细节：内部 `anyhow`，边界层转 `Err(String)`；`map_http_error` 不暴露栈。
- 网络唯一出口 `HttpTransport`，便于审计与离线测试。

## 文档导航

- 子系统文档逐条入口见 [INDEX.md](INDEX.md)。
- 完整实现规格（含 proxy 端点设计、逐字段证据）见 [SPEC.md](SPEC.md)。

---

页脚：[opentake-gen 目录 INDEX.md](INDEX.md) · [模块文档树 ../INDEX.md](../INDEX.md) · [docs 总目录 ../../INDEX.md](../../INDEX.md)
