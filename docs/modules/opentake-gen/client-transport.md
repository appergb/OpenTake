# client-transport — 客户端、HTTP 传输与生成 Job 生命周期

> 上级：[opentake-gen 目录](INDEX.md) · [模块文档树](../INDEX.md) · [docs 总目录](../../INDEX.md)
>
> 子系统级文档（不逐函数）。源码：[`client.rs`](../../../crates/opentake-gen/src/client.rs) + [`transport.rs`](../../../crates/opentake-gen/src/transport.rs) + [`job.rs`](../../../crates/opentake-gen/src/job.rs) + [`error.rs`](../../../crates/opentake-gen/src/error.rs)。完整规格见 [SPEC.md](SPEC.md) §1.1 / §1.3 / §1.4 / §3。

---

## 定位

本子系统是「发起生成」的执行核心：`GenClient` 是顶层入口，向上提供**一套调用面、两种鉴权/路由模式**（设计公理 A7）；`HttpTransport` 是所有网络的唯一出口；`GenerationJob` 是掩盖各厂商异步差异的统一任务抽象；`GenError` 是错误码契约。

## `GenClient` —— 一套调用面，两种模式

`AuthMode` 二选一：

- **`Bearer`（托管）**：所有调用经自建 proxy，proxy 持厂商 key 并计费。携带 `TokenProvider` 异步取 JWT（可复用任意 OIDC，公理 A6）。
- **`Byok`（本地直连）**：经 [`ProviderRegistry`](providers.md) 路由到厂商 adapter，目录用[内置静态目录](catalog.md)。

`GenClient` 廉价可 `Clone`（内部 `Arc`）。统一调用面：

| 方法 | 托管 | BYOK |
|---|---|---|
| `list_models` | `GET /v1/models` | 返回内置静态目录 |
| `submit` | `POST /v1/generations` 得 `jobId` | 路由到 adapter 提交 |
| `get` | `GET /v1/generations/:id` | adapter 轮询一次 |
| `watch` | 轮询直到终态（见下） | 同左 |
| `sign_upload` / `upload_reference` | 预签名 PUT 上传 | `upload_reference_via` 委托 adapter（厂商上传支持不一） |

BYOK 任务 id 约定为 `<prefix>::<vendorJobId>`，让 `get` 能据前缀重新选 adapter（`submit_byok` 自动加前缀；`split_byok_job_id` 校验形状）。

### `watch` —— 订阅到终态的轮询流

`watch` 返回 `impl Stream<Item = Result<GenerationJob, GenError>>`，按配置间隔（默认 2s，测试用 `Duration::ZERO` 即时跑）轮询，逐个 yield 观测到的 job 快照。复刻上游 `runJob` 订阅循环（`GenerationService.swift:338-361`）：**queued/running 继续，succeeded/failed 终止**。`with_poll_interval` 必须在 `Clone` 前调用。

### 能力信号 `can_generate`

- 托管：能取到 token 即 `true`。
- BYOK：注册了任一 adapter（fal/replicate/openai/elevenlabs）即 `true`。
  与 `get_timeline` 的 `canGenerate` 闸门衔接（SPEC §5.3）。`filter_by_kind` 另提供与 proxy `?type=` 对齐的目录过滤。

## `transport.rs` —— 网络唯一出口

`HttpTransport` trait 只有一个 `send(HttpRequest) -> HttpResponse`。本 crate **任何**网络调用都走它，从不直接用 `reqwest`：

- `ReqwestTransport`：生产实现（`reqwest`，rustls-tls）。
- `MockTransport`：测试实现，按 `"METHOD url"` 提供罐装响应。两种模式可组合——
  - **keyed map**：单条响应「粘性」重复返回；
  - **sequence per key**：按序弹出，可表达 `submit` 后多次 `poll` 的 queued→running→succeeded。
  并记录每次请求供断言。**全套测试零 socket**（完全离线）。

请求/响应是传输无关值类型：`HttpRequest`（method/url/headers/body）、`Body`（Empty/Json/Bytes）、`HttpResponse`（status/headers/body + `is_success`/`json`/大小写不敏感 `header`）。`Method` 仅 Get/Post/Put（够 adapter 与 client 用）。

## `job.rs` —— 统一 Job 抽象

`GenerationJob` 是 `queued → running → succeeded/failed` 的归一化任务（公理 A4），掩盖各厂商异步差异。1:1 端口上游 `BackendGenerationJob` / `BackendGenerationStatus`：

- `JobStatus`：`is_terminal()` 仅对 succeeded/failed 为真。
- 字段：`id`（兼容 proxy 的 `id` 与上游 Convex 文档 `_id`，serde `alias`）、`status`、`result_urls`、`error_message`、`cost_credits`（托管计费，BYOK 恒 `None`）、`completed_at`。**全部可选字段容忍缺失**（读旧载荷不破坏——移植铁律）。
- 构造器 `succeeded`/`failed`/`pending` 供同步 adapter 直接造终态。
- `first_result_url()` 复刻 `finalizeSuccess`（`GenerationService.swift:364-379`）：succeeded 但**无结果 URL 视为失败**（"No URL in response"）。

## `error.rs` —— 错误码契约

`GenError`（`thiserror`）复刻上游错误码契约（公理 A6）：`NotConfigured` / `Unauthenticated`（401）/ `InsufficientCredits`（402）/ `Transport` / `Api{status,code,message}` / `Other(anyhow)`。`Display` 是面向用户的文案（如「sign in to continue」）。

`map_http_error(status, body)` 复刻 `assertHTTPOK` + `PalmierClientError.from`：先解析 `{"error":{code,message}}` 信封，**优先按 code**（`unauthenticated`/`insufficient_credits`）再回退 HTTP 状态；无信封时把响应体当 message。内部错误一律 `anyhow`，边界层（Tauri 命令）转 `Err(String)`（代码风格要点）。

## 数据流（一次 BYOK 生成）

```
build_params(GenerationInput, uploaded)        # params 子系统装配载荷
  → GenClient::submit_byok(model, params)      # 路由到 adapter，加 "<prefix>::" 前缀
      → ProviderRegistry::route → adapter.submit  # 走 HttpTransport 发请求
  → GenClient::watch(job_id) (Stream)          # 按间隔轮询
      → adapter.poll → 归一化 GenerationJob    # queued/running 继续
  → 终态 succeeded：first_result_url()          # 拿结果 URL（下载/落轨属下游）
```

托管路径相同，区别仅 `submit`/`get` 改打 proxy REST + Bearer，`watch` 循环不变。

## 对应上游 Swift

- `GenClient` 双模 ≈ 上游 `GenerationBackend`（Convex 薄层）的跨平台重写；上游 Combine 订阅 → 这里 `futures` Stream。
- `watch` ← `runJob`（`GenerationService.swift:338-361`）；`first_result_url` ← `finalizeSuccess`（`:364-379`）。
- `GenerationJob`/`JobStatus` ← `GenerationBackend.swift:112-123`；`GenError`/`map_http_error` ← `GenerationBackend.swift:76-90` + `PalmierClient.swift:80-91`。
- 整体 verdict `cloud-rebuild`：不保留 Convex，自建 REST/WebSocket 网关（MODULE-PORT-MAP「Generation」段、SPEC §3）。

## 完成状态

- **已实现**：`GenClient` 双模调用面（list_models/submit/get/watch/sign_upload/upload_reference）、`watch` 终态轮询、`can_generate`、`HttpTransport` + 生产/Mock 实现、`GenerationJob` 状态机与终态校验、`GenError` 全套错误归类——均有离线单测（托管 + BYOK 两侧）。
- **计划中 / 未接线**：
  - **托管 proxy 本身未实现**（`opentake-gen-proxy` 是 Phase 9 自建后端目标，含 `/v1/models`、`/v1/generations`、`/v1/uploads/sign`、SSE `/stream`、对象存储预签名、可选积分计费；SPEC §3）。`GenClient` 客户端侧已就绪，等服务端。
  - **`generate_*` / `upscale_media` 尚未走通 `GenClient`**：`opentake-agent` 的 dispatch 层这四个工具仍是诚实存根（`"...: not yet implemented"`），原因明确写在代码注释——「需要 async GenClient + BYOK auth」。`list_models` 已接（见 [catalog.md](catalog.md)）。需要 async + ProviderRegistry 装配 + [BYOK key 注入](keys-byok.md)（ROADMAP Phase 8/9）。
  - `watch` 的流式 `/v1/generations/:id/stream`（SSE）目前只在客户端以轮询表达；proxy SSE 端点待建。

## 源码

| 文件 | 内容 |
|---|---|
| [`client.rs`](../../../crates/opentake-gen/src/client.rs) | `GenClient` / `AuthMode` / `TokenProvider`+`StaticToken` / `UploadTicket` / `can_generate` / `filter_by_kind` / BYOK job-id 前缀 |
| [`transport.rs`](../../../crates/opentake-gen/src/transport.rs) | `HttpTransport` / `HttpRequest`/`HttpResponse`/`Body`/`Method` / `ReqwestTransport` / `MockTransport` |
| [`job.rs`](../../../crates/opentake-gen/src/job.rs) | `GenerationJob` / `JobStatus` / 终态校验 `first_result_url` |
| [`error.rs`](../../../crates/opentake-gen/src/error.rs) | `GenError` / `map_http_error` / `ErrorEnvelope` |
| [`lib.rs`](../../../crates/opentake-gen/src/lib.rs) | crate 文档 + 模块声明 + 公共 API re-export |

---

页脚：[opentake-gen 目录 INDEX.md](INDEX.md) · [模块文档树 ../INDEX.md](../INDEX.md) · [docs 总目录 ../../INDEX.md](../../INDEX.md)
