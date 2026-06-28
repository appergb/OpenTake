# providers — Provider 适配层（fal.ai / Replicate / OpenAI / ElevenLabs）

> 上级：[opentake-gen 目录](INDEX.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
>
> 子系统级文档（不逐函数）。源码：[`crates/opentake-gen/src/provider/`](../../../crates/opentake-gen/src/provider/mod.rs)。完整规格见 [SPEC.md](SPEC.md) §2.1 / §2.2。

---

## 定位

BYOK 模式下「直连厂商」的适配层。每个厂商一个 adapter，负责三件事：把统一的 `GenerationParams` 翻译成该厂商的请求体、提交并返回归一化 `GenerationJob`、轮询厂商状态再归一化。所有网络调用都走 [`HttpTransport`](client-transport.md)，从不直接碰 `reqwest`。

托管模式不经过本层——参数与任务由自建 proxy 在云端转译，客户端只发 `/v1/generations`（见 [client-transport.md](client-transport.md)）。

## 统一契约：`ProviderAdapter` trait

`provider/mod.rs` 定义 trait（对应上游 `GenerationBackend` 的厂商无关抽象）：

| 方法 | 职责 |
|---|---|
| `prefix() -> &'static str` | 该 adapter 的模型 id 前缀（`fal` / `replicate` / `openai` / `elevenlabs`）。 |
| `submit(route, params)` | 映射参数 → 提交 → 返回归一化 job（通常 queued/running；同步厂商直接返回终态）。 |
| `poll(job_id)` | 轮询一次厂商状态，归一化为 `GenerationJob`。 |
| `upload(path, content_type)` | 上传引用文件 → 公开 URL。 |

### 路由：`ModelRoute` + `ProviderRegistry`

- 模型 id 一律是 `<prefix>:<vendorModel>`。`ModelRoute::parse` 只按**第一个冒号**切分——vendor_model 本身可含冒号（Replicate 的 `owner/model:version`）。
- `ProviderRegistry` 是 `prefix -> Arc<dyn ProviderAdapter>` 的注册表。`route(model_id)` 解析前缀并取 adapter，未知前缀返回 `GenError::NotConfigured`；`has_prefix` 供 `can_generate` 信号判断「至少注册了一个 adapter」。

### 共享工具（`mod.rs`）

- `content_type_for(path, fallback)`：按扩展名推断上传 content-type，1:1 端口上游 `GenerationService.contentType`（jpg/png/webp/heic/gif → image，mp4/m4v/mov → video，mp3/wav/m4a → audio；未知按 fallback 回退）。
- `normalize_output_urls(value)`：把厂商 `output` 归一化为 URL 列表，容忍三种形状——裸字符串、字符串数组、`{url}` 对象/数组。
- `base64_encode` / `encode_data_url`：把同步厂商返回的音频/图片字节裹成 `data:` URL（内联实现，不引额外依赖）。

## 四个 adapter 的差异

| Adapter | 鉴权头 | 异步模型 | 提交→任务 | 轮询/取结果 |
|---|---|---|---|---|
| **fal.ai** (`fal`) | `Authorization: Key <key>` | 队列 API（异步） | `POST {queue}/{vendorModel}` 得 `request_id`；job_id 编码为 `<vendorModel>\|<request_id>` | `GET .../requests/{id}/status`；终态再 `GET .../requests/{id}` 取 output |
| **Replicate** (`replicate`) | `Authorization: Bearer <token>` | predictions（异步） | `POST /predictions {version, input}` 得 `id` | `GET /predictions/{id}` |
| **OpenAI** (`openai`) | `Authorization: Bearer <key>` | 同步 | `POST /images/generations` 或 `/audio/speech`，**直接返回终态** | `poll` 回放进程内缓存的 job |
| **ElevenLabs** (`elevenlabs`) | `xi-api-key: <key>` | 同步 | TTS `POST /text-to-speech/{voiceId}`；音乐 `POST /music`，**直接返回终态** | `poll` 回放缓存 job |

### 状态归一化

各厂商状态串映射到统一 `JobStatus`（queued/running/succeeded/failed），无法识别的串一律落 `Failed`（保守）：

- fal：`IN_QUEUE→Queued`、`IN_PROGRESS→Running`、`COMPLETED→Succeeded`、其余 `Failed`。
- Replicate：`starting→Queued`、`processing→Running`、`succeeded→Succeeded`、failed/canceled/unknown → `Failed`。
- OpenAI / ElevenLabs：同步厂商，提交即终态，无状态串。

### 同步厂商的「缓存回放」模式

OpenAI / ElevenLabs 是同步的：`submit` 内部直接拿到结果，造一个终态 `GenerationJob` 并按合成 job_id 存进进程内 `Mutex<HashMap>` 缓存；`poll` 只是回放缓存。这样 [`watch` 轮询循环](client-transport.md)对同步/异步厂商行为一致（首轮即 terminal）。

- 字节型结果（OpenAI TTS、ElevenLabs TTS/音乐）通过 `encode_data_url` 裹成 `data:` URL，便于本地直接下载——**这是未配置对象存储时的权宜做法**（见 SPEC §2.2.4 note(a)）；正式部署应改为持久化到 S3/R2。
- OpenAI 图片：响应可能带 `url` 或 `b64_json`，后者归一化为 `data:image/png;base64,...`。
- OpenAI 仅支持 image / audio，其它 kind 返回错误；ElevenLabs 仅支持 audio（vendor_model 含 `music` 走音乐端点，否则走 TTS）。

### 上传支持差异

- fal：`POST` 字节到 storage upload，取 `access_url`/`url`。
- Replicate：`POST /files`，取 `urls.get`。
- OpenAI / ElevenLabs：**无公开资产托管**，`upload` 直接返回错误，提示配置对象存储。

## 错误处理

非 2xx 响应统一交给 [`map_http_error`](client-transport.md)：先解析 `{"error":{code,message}}` 信封，再按 code/状态码归类（401→`Unauthenticated`、402→`InsufficientCredits`、其余→`Api{status,code,message}`）。adapter 自身只负责发请求与归一化，不重复定义错误码。

## 对应上游 Swift

整个 BYOK 直连层是 OpenTake 的**新增**（上游一切生成都走 Convex 闭源云，客户端不直接接触厂商）。但参数/任务的 DTO 与判别字段语义 1:1 复刻上游：

- `ProviderAdapter` ≈ 上游 `GenerationBackend`（enum，厂商无关 RPC 薄层）的跨平台重写。
- 参数映射的字段口径来自上游 `*ModelConfig` / `*GenerationParams`（详见 [params.md](params.md)）。
- content-type 推断表逐项对照 `GenerationService.swift:266-287`。

完整移植定位见 [`../../architecture/MODULE-PORT-MAP.md`](../../architecture/MODULE-PORT-MAP.md)「Generation」段（verdict：`cloud-rebuild` —— 云后端层必须自建，不保留 Convex）。

## 完成状态

- **已实现**：四个 adapter 的 submit/poll/upload 与参数映射、状态归一化、缓存回放、错误归类，均有 `MockTransport` 离线单测覆盖（全套测试不开任何 socket）。
- **计划中**：adapter 目前未被 `src-tauri` / `opentake-agent` 实际接线驱动——`generate_*` / `upscale_media` 在 dispatch 层仍是诚实存根（`"...: not yet implemented"`），缺 async GenClient 装配 + BYOK key 注入（见 [client-transport.md](client-transport.md) 与 [keys-byok.md](keys-byok.md) 的状态说明）。

## 源码

| 文件 | 内容 |
|---|---|
| [`provider/mod.rs`](../../../crates/opentake-gen/src/provider/mod.rs) | `ProviderAdapter` trait / `ModelRoute` / `ProviderRegistry` / `content_type_for` / `normalize_output_urls` / base64 + data-url 工具 |
| [`provider/fal.rs`](../../../crates/opentake-gen/src/provider/fal.rs) | `FalAdapter`（队列 API、`vendorModel\|request_id` job_id、output 提取） |
| [`provider/replicate.rs`](../../../crates/opentake-gen/src/provider/replicate.rs) | `ReplicateAdapter`（predictions、`{version,input}` 映射） |
| [`provider/openai.rs`](../../../crates/opentake-gen/src/provider/openai.rs) | `OpenAiAdapter`（同步 images/TTS、缓存回放、aspect→size 映射） |
| [`provider/elevenlabs.rs`](../../../crates/opentake-gen/src/provider/elevenlabs.rs) | `ElevenLabsAdapter`（同步 TTS/音乐、`xi-api-key`、默认 voice） |

---

页脚：[opentake-gen 目录 INDEX.md](INDEX.md) · [模块文档树 ../INDEX.md](../INDEX.md) · [docs 总目录 ../../INDEX.md](../../INDEX.md)
