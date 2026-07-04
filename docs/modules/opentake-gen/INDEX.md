# opentake-gen — 模块目录

> 上级：[模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
>
> 生成式 AI 客户端（**BYOK 无后端**：自带 key 直连 fal.ai/Replicate/OpenAI/ElevenLabs + 内置静态模型目录；可选托管 proxy）。能力层叶子 crate，仅依赖 `opentake-domain`，被 `opentake-agent` / `src-tauri` 调用。
> 先读 [总览 OVERVIEW.md](OVERVIEW.md) 建立全貌，再按需进入下面的子系统文档。

---

## 总览

- **[OVERVIEW.md](OVERVIEW.md)** — 定位 / 依赖分层 / 职责边界 / 关键概念与数据流（BYOK 无后端、多 Provider 抽象、模型目录、生成参数、Job 流程、双模一套面）/ 上游对应 / 完成状态 / 移植铁律 + 安全。

## 子系统文档

- **[providers.md](providers.md)** — Provider 适配层：`ProviderAdapter` trait / `ModelRoute` / `ProviderRegistry`，fal.ai（队列 API）/ Replicate（predictions）/ OpenAI（同步 images/TTS）/ ElevenLabs（同步 TTS/音乐）各适配、状态归一化、同步厂商缓存回放、上传支持差异。
- **[catalog.md](catalog.md)** — 模型目录与能力/计价矩阵：`CatalogEntry` 按 kind 自定义反序列化、四类 `*Caps`、`Catalog` 查询、内置静态目录 `builtin_catalog.json`、`cost.rs` 客户端成本预估、`list_models` 来源（含 agent 接线）。
- **[params.md](params.md)** — 生成参数：`GenerationParams` 联合类型（全大写 URL 键、省略空值）+ `build_params` 从 `GenerationInput` 按「frames→image→video→audio」上传顺序装配。
- **[client-transport.md](client-transport.md)** — 客户端 / 传输 / Job 生命周期：`GenClient` 双模调用面、`HttpTransport`（生产 reqwest / 测试 mock）、`GenerationJob` 状态机、`watch` 终态轮询、`GenError` 错误码契约。
- **[keys-byok.md](keys-byok.md)** — BYOK 密钥管理：`ProviderKey` / `KeyStore` / 跨平台 `KeyringStore`，与 `src-tauri/src/secret.rs` 钥匙串命令配合（明文单向、掩码回传、provider 白名单）。

## 规格

- **[SPEC.md](SPEC.md)** — `opentake-gen` 完整实现就绪规格（Issue #10）：设计公理、`GenClient` 接口、`GenerationParams` 逐字段复刻、Job 状态机、BYOK provider+keyring+静态目录、托管 proxy（axum）端点设计、`CatalogEntry` 能力矩阵、与 domain/agent 接口、实施清单、上游证据索引。**只读，子系统文档只链接不复述。**

## 相关跨切面

- [`../../architecture/MODULE-PORT-MAP.md`](../../architecture/MODULE-PORT-MAP.md) — 逐模块移植地图（本 crate 对应「Generation」段，verdict `cloud-rebuild`；含上游 `GenerationService`/`GenerationBackend`/`ModelCatalog`/`CostEstimator`/各 `*Submission` 拆解）。
- [`../../architecture/ROADMAP.md`](../../architecture/ROADMAP.md) — 分阶段路线图（Phase 9 生成式 AI 后端：`GenClient` + BYOK + 托管 proxy；进阶 AIGC 编排）。
- [`../../architecture/PORT-1TO1-GAP.md`](../../architecture/PORT-1TO1-GAP.md) — 1:1 复刻差距清单（P1-12 BYOK 密钥安全存储；生成工具接线现状）。
- [`../../architecture/ADVANCED-FEATURES.md`](../../architecture/ADVANCED-FEATURES.md) — 进阶能力（E 层 AIGC 编排：音色克隆 / 数字人 / 图文成片 / 多语种字幕）。
- [`../../architecture/ARCHITECTURE.md`](../../architecture/ARCHITECTURE.md) — 总体架构（单一真理状态 + 命令事务）。

## 交叉模块

- [`../opentake-domain/INDEX.md`](../opentake-domain/INDEX.md) — 唯一上游依赖：`GenerationInput` 领域输入快照定义于此（`media.rs`），本 crate 装配并 re-export。
- [`../opentake-agent/INDEX.md`](../opentake-agent/INDEX.md) — 调用方：MCP 生成工具（`generate_*`/`upscale_media`/`list_models`）；`list_models` 已接内置目录，`generate_*` 仍待接线。
- [`../src-tauri/INDEX.md`](../src-tauri/INDEX.md) — 调用方：`secret_save`/`secret_load`/`secret_delete` 钥匙串命令复用本 crate 的 `KeyringStore`。

## 源码

`crates/opentake-gen/src/`：

| 文件 | 内容 | 子系统文档 |
|---|---|---|
| [`lib.rs`](../../../crates/opentake-gen/src/lib.rs) | crate 文档 + 模块声明 + 公共 API 扁平 re-export（含 domain `GenerationInput`） | — |
| [`client.rs`](../../../crates/opentake-gen/src/client.rs) | `GenClient` / `AuthMode` / `TokenProvider` / `can_generate` / `filter_by_kind` | [client-transport](client-transport.md) |
| [`transport.rs`](../../../crates/opentake-gen/src/transport.rs) | `HttpTransport` / `HttpRequest`/`Response`/`Body`/`Method` / `ReqwestTransport` / `MockTransport` | [client-transport](client-transport.md) |
| [`job.rs`](../../../crates/opentake-gen/src/job.rs) | `GenerationJob` / `JobStatus` / `first_result_url` | [client-transport](client-transport.md) |
| [`error.rs`](../../../crates/opentake-gen/src/error.rs) | `GenError` / `map_http_error` / `ErrorEnvelope` | [client-transport](client-transport.md) |
| [`params.rs`](../../../crates/opentake-gen/src/params.rs) | `GenerationParams` 联合 + 四 `*Params` + `clamp_num_images` | [params](params.md) |
| [`build_params.rs`](../../../crates/opentake-gen/src/build_params.rs) | `build_*` 五装配 + 顶层 `build_params` + `slice_video_uploads` | [params](params.md) |
| [`keys.rs`](../../../crates/opentake-gen/src/keys.rs) | `ProviderKey` / `KeyStore` / `KeyringStore` / `MemoryKeyStore` | [keys-byok](keys-byok.md) |
| [`provider/mod.rs`](../../../crates/opentake-gen/src/provider/mod.rs) | `ProviderAdapter` / `ModelRoute` / `ProviderRegistry` + 共享工具 | [providers](providers.md) |
| [`provider/fal.rs`](../../../crates/opentake-gen/src/provider/fal.rs) | `FalAdapter`（队列 API） | [providers](providers.md) |
| [`provider/replicate.rs`](../../../crates/opentake-gen/src/provider/replicate.rs) | `ReplicateAdapter`（predictions） | [providers](providers.md) |
| [`provider/openai.rs`](../../../crates/opentake-gen/src/provider/openai.rs) | `OpenAiAdapter`（同步 images/TTS） | [providers](providers.md) |
| [`provider/elevenlabs.rs`](../../../crates/opentake-gen/src/provider/elevenlabs.rs) | `ElevenLabsAdapter`（同步 TTS/音乐） | [providers](providers.md) |
| [`catalog/mod.rs`](../../../crates/opentake-gen/src/catalog/mod.rs) | `Catalog` 包装 + 查询 | [catalog](catalog.md) |
| [`catalog/entry.rs`](../../../crates/opentake-gen/src/catalog/entry.rs) | `CatalogEntry` 自定义 `Deserialize` + 四 `*Caps` + `AudioPricing` | [catalog](catalog.md) |
| [`catalog/builtin.rs`](../../../crates/opentake-gen/src/catalog/builtin.rs) | `builtin_catalog()`（编译期 `include_str!`） | [catalog](catalog.md) |
| [`catalog/builtin_catalog.json`](../../../crates/opentake-gen/src/catalog/builtin_catalog.json) | 内置静态目录数据 | [catalog](catalog.md) |
| [`catalog/cost.rs`](../../../crates/opentake-gen/src/catalog/cost.rs) | 客户端成本预估纯函数（展示用） | [catalog](catalog.md) |

---

页脚：[模块文档树 ../INDEX.md](../INDEX.md) · [docs 总目录 ../../INDEX.md](../../INDEX.md)
