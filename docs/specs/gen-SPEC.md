# gen-SPEC — opentake-gen 实现就绪规格（Issue #10）

> 状态：实现就绪（implementation-ready）。本规格基于上游 Swift 真实源码逐字段复刻，每个契约点给出 `文件:行号` 证据。
> 范围：`crates/opentake-gen`（BYOK 生成客户端 + 静态 catalog + provider adapters）与可选的 `opentake-gen-proxy`（axum 托管模式后端）。
> 约束遵循 OpenTake `AGENTS.md`：内部错误用 `anyhow::Error`，边界层转 `Err(String)`；所有 serde 模型加 `#[serde(default)]` + `Option<T>` 以读旧数据不破坏；注释最小化。

---

## 0. 设计公理（来自上游拆解，逐条带证据）

这 7 条是上游最干净的边界，OpenTake 全部沿用。偏离它们会破坏与 domain/agent 的兼容。

| # | 公理 | 上游证据 |
|---|---|---|
| A1 | **`model` 是不透明字符串**，客户端永不硬编码厂商。提交时只传 `model: String` + `params`。 | `GenerationBackend.submit(model:params:projectId:)` — `GenerationBackend.swift:56-74`；agent 侧 `args.string("model")` — `ToolExecutor+Generate.swift:14,154,204` |
| A2 | **后端只吃 URL，不吃字节**。所有引用素材（首/尾帧、参考图/视频/音频、源视频）先上传换成公开 URL，再放进 `params`。 | 三步上传 `uploadReference` — `GenerationBackend.swift:20-54`；`params` 字段全是 `...URL`/`...URLs` — `VideoModelConfig.swift:67-78` |
| A3 | **`params` 是带 `kind` 判别字段的联合类型**，四态 image/video/audio/upscale。 | `BackendGenerationParams` enum — `GenerationBackend.swift:95-110`；各 `encode` 写 `kind` — `ImageModelConfig.swift:17`、`VideoModelConfig.swift:111`、`AudioModelConfig.swift:18`、`UpscaleModelConfig.swift:11` |
| A4 | **统一 job 抽象**：`queued → running → succeeded/failed` + `resultUrls`，屏蔽各厂商异步差异。 | `BackendGenerationStatus` — `GenerationBackend.swift:112-114`；`BackendGenerationJob` — `GenerationBackend.swift:116-123` |
| A5 | **catalog 由数据驱动**：每个模型的能力矩阵 + 定价是运行时下发（上游）或静态内置（OpenTake BYOK），UI/agent 纯被动适配。 | `CatalogEntry` + `*Caps` — `ModelCatalog.swift:112-241`；agent `list_models` 透传 caps — `ToolExecutor+Generate.swift:373-396` |
| A6 | **认证契约只有一条**：一个 Bearer JWT，后端用它鉴权 + 关联用量/扣费。错误码 `401/unauthenticated`、`402/insufficient_credits`。 | `PalmierClient` 取 Clerk JWT 注入 `Authorization: Bearer` — `PalmierClient.swift:37-46`；错误码映射 — `PalmierClient.swift:80-91` |
| A7 | **BYOK 优先级**：有用户 key → 直连厂商；无 key 且已登录 → 走托管代理 + 扣费；都没有 → 不可用。 | `AgentService.selectClient()` — `AgentService.swift:52-59`；`canStream` — `AgentService.swift:41-45` |

**双模总览**：

```
托管模式 (managed)                          BYOK 模式 (byok)
─────────────────                          ────────────────
GenClient ──Bearer JWT──▶ opentake-gen-proxy   GenClient ──厂商 key (keyring)──▶ fal/Replicate/OpenAI/ElevenLabs
                          │ provider adapters                                    │ 同一套 provider adapters（本地执行）
                          │ 持厂商 key + 计费                                     │ 静态内置 catalog
                          │ 预签名上传 (S3/R2)                                    │ 直接用厂商上传/直传 URL
                          ▼                                                       ▼
                      统一 GenerationJob ◀──────────── 两条路径归一到同一 job 抽象 ──────────▶ 统一 GenerationJob
```

唯一差异在 `GenClient` 构造时选的 `AuthMode`；`submit/watch/list_models` 的调用方签名两模一致。

---

## 1. GenClient Rust 接口（crate `opentake-gen`）

### 1.0 crate 依赖（写入 `crates/opentake-gen/Cargo.toml` 的 `[dependencies]`，当前为空 — 见 `crates/opentake-gen/Cargo.toml:9`）

```toml
serde = { workspace = true }
serde_json = { workspace = true }
reqwest = { version = "0.12", default-features = false, features = ["json", "stream", "rustls-tls", "multipart"] }
tokio = { version = "1", features = ["rt", "macros", "time", "fs"] }
futures-util = "0.3"          # Stream 组合
async-trait = "0.1"          # provider adapter trait
thiserror = "2"              # GenError
anyhow = "1"                 # 内部错误
url = "2"
keyring = "3"                # OS keychain（macOS Keychain / Windows Credential Manager / Linux Secret Service）
eventsource-stream = "0.2"   # SSE 解析（watch + agent_stream）
bytes = "1"
opentake-domain = { workspace = true }  # GenerationInput 等共享类型（见 §5）
```

> 说明：`opentake-domain` 是零依赖叶子 crate（`crates/opentake-domain/src/lib.rs:1-8`），不允许网络/FS。因此 **`GenerationInput` 放 domain，`GenerationParams`/`GenClient`/adapters 放 opentake-gen**，方向是 gen → domain。详见 §5.1。

### 1.1 顶层客户端

```rust
// crates/opentake-gen/src/client.rs
use std::sync::Arc;
use std::path::Path;
use url::Url;
use futures_util::Stream;

#[derive(Clone)]
pub struct GenClient {
    inner: Arc<GenClientInner>,
}

struct GenClientInner {
    mode: Mode,
    http: reqwest::Client,
}

pub enum Mode {
    /// 托管：所有调用走自建 proxy；proxy 持厂商 key + 计费。
    Managed { base_url: Url, auth: Arc<dyn TokenProvider> },
    /// BYOK：本地直连厂商；catalog 走静态内置；可完全无 proxy 运行。
    Byok { registry: ProviderRegistry, catalog: Catalog },
}

/// 异步取 Bearer token；UI 注入（复用任意 OIDC，见 A6）。
#[async_trait::async_trait]
pub trait TokenProvider: Send + Sync {
    async fn bearer_token(&self) -> Result<String, GenError>;
}

impl GenClient {
    pub fn managed(base_url: Url, auth: Arc<dyn TokenProvider>) -> Self { /* ... */ }
    pub fn byok(registry: ProviderRegistry, catalog: Catalog) -> Self { /* ... */ }

    /// 模型目录。托管 = GET /v1/models；BYOK = 返回内置静态 catalog（同结构）。
    /// 对应上游 models:list（ModelCatalog.swift:54）+ agent list_models（ToolExecutor+Generate.swift:373）。
    pub async fn list_models(&self) -> Result<Vec<CatalogEntry>, GenError>;

    /// 取一个上传票据（预签名 URL）。对应上游 uploads:generateUploadTicket（GenerationBackend.swift:30）。
    /// BYOK 模式：部分厂商（fal）有自己的上传端点，由 adapter 决定是否需要外部对象存储。
    pub async fn sign_upload(&self, content_type: &str) -> Result<UploadTicket, GenError>;

    /// 上传一个引用素材 → 公开 URL（三步式，见 §3.4 / GenerationBackend.swift:20-54）。
    /// content_type 推断表见 §3.5（复刻 GenerationService.swift:266-287）。
    pub async fn upload_reference(&self, path: &Path, content_type: &str) -> Result<String, GenError>;

    /// 提交任务，返回 job_id。对应 generations:submit（GenerationBackend.swift:56-74）。
    pub async fn submit(
        &self,
        model: &str,
        params: GenerationParams,
        project_id: Option<&str>,
    ) -> Result<String, GenError>;

    /// 取单次 job 当前快照（轮询一次）。对应 generations:byId 的单次读取。
    pub async fn get(&self, job_id: &str) -> Result<GenerationJob, GenError>;

    /// 订阅 job 直到终态。托管：优先 SSE（GET /v1/generations/:id/stream），降级轮询。
    /// BYOK：adapter 内部对厂商 status 轮询并归一化。语义复刻上游 Combine 订阅
    /// （GenerationService.runJob 的 for-await 循环，GenerationService.swift:328-361）：
    /// 只在 succeeded/failed 终止，queued/running 继续等待。
    pub fn watch(&self, job_id: &str) -> impl Stream<Item = Result<GenerationJob, GenError>> + Send;

    /// LLM 文本代理流（仅托管模式有意义；BYOK 文本由 opentake-agent 直连）。见 §3.6。
    pub fn agent_stream(
        &self,
        req: AgentRequest,
    ) -> impl Stream<Item = Result<AgentEvent, GenError>> + Send;
}
```

### 1.2 GenerationParams 联合类型（逐字段复刻，**这是规格核心**）

下表把上游 4 个 `Encodable` 结构的每个字段精确映射到 Rust。**JSON 字段名必须与上游 wire 名逐字一致**（上游 `CodingKeys` 用 camelCase，故 Rust 端用 `#[serde(rename_all = "camelCase")]` 或逐字段 `rename`），否则 proxy 转发 / 与上游兼容会断。

```rust
// crates/opentake-gen/src/params.rs
use serde::Serialize;

/// 复刻 BackendGenerationParams（GenerationBackend.swift:95-110）。
/// 用 internally-tagged "kind" 判别字段，与上游 encode 写出的 {"kind": "...", ...} 等价。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum GenerationParams {
    Image(ImageParams),     // kind="image"
    Video(VideoParams),     // kind="video"
    Audio(AudioParams),     // kind="audio"
    Upscale(UpscaleParams), // kind="upscale"
}
```

> 关键：上游用 `singleValueContainer` 直接 encode 内层结构，内层各自写 `kind`（`GenerationBackend.swift:101-109` + 各 `encode`）。Rust 的 `#[serde(tag = "kind")]` 产出**等价 wire 结构**（`kind` 提到顶层与各字段平铺），且更省样板。`rename_all = "lowercase"` 让变体名 → `"image"/"video"/"audio"/"upscale"`，与上游字面量一致（`ImageModelConfig.swift:17` 等）。

#### 1.2.1 ImageParams — 证据 `ImageModelConfig.swift:3-25`

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageParams {
    pub prompt: String,                         // :8  必填
    pub aspect_ratio: String,                   // :8 aspectRatio 必填
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,             // :8 encodeIfPresent (:20)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<String>,                // :9 encodeIfPresent (:21)
    /// 引用图 URL（已上传）。上游：仅当非空才写（:22）。
    #[serde(rename = "imageURLs", skip_serializing_if = "Vec::is_empty")]
    pub image_urls: Vec<String>,                // :9
    pub num_images: u8,                         // :9 numImages 必填，clamp 1..=4（见下）
}
```
> wire 名注意：上游键名是 `imageURLs`（全大写 URL），不是 `imageUrls`。必须 `#[serde(rename = "imageURLs")]`。`numImages` 走 camelCase 自动派生即可。
> `num_images` 钳制：上游在 `GenerationService.generate` clamp `max(1, min(4, numImages))`（`GenerationService.swift:41`）+ `ImageModelConfig.maxImages` 同样 `max(1,min(4,...))`（`ImageModelConfig.swift:47`）。GenClient 构造 `ImageParams` 时也 clamp。

#### 1.2.2 VideoParams — 证据 `VideoModelConfig.swift:67-124`

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoParams {
    pub prompt: String,                         // :68
    pub duration: u32,                          // :69（秒；整数；上游 Int）
    pub aspect_ratio: String,                   // :70 aspectRatio
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,             // :71 encodeIfPresent (:115)
    #[serde(rename = "sourceVideoURL", skip_serializing_if = "Option::is_none")]
    pub source_video_url: Option<String>,       // :72 encodeIfPresent (:116)
    #[serde(rename = "startFrameURL", skip_serializing_if = "Option::is_none")]
    pub start_frame_url: Option<String>,        // :73 encodeIfPresent (:117)
    #[serde(rename = "endFrameURL", skip_serializing_if = "Option::is_none")]
    pub end_frame_url: Option<String>,          // :74 encodeIfPresent (:118)
    #[serde(rename = "referenceImageURLs", skip_serializing_if = "Vec::is_empty")]
    pub reference_image_urls: Vec<String>,      // :75 仅非空写 (:119)
    #[serde(rename = "referenceVideoURLs", skip_serializing_if = "Vec::is_empty")]
    pub reference_video_urls: Vec<String>,      // :76 仅非空写 (:120)
    #[serde(rename = "referenceAudioURLs", skip_serializing_if = "Vec::is_empty")]
    pub reference_audio_urls: Vec<String>,      // :77 仅非空写 (:121)
    pub generate_audio: bool,                   // :78 generateAudio 必填（默认 true，见 init :87）
}
```
> wire 名注意：`sourceVideoURL / startFrameURL / endFrameURL / referenceImageURLs / referenceVideoURLs / referenceAudioURLs`（URL/URLs 全大写）。
> 上游 init 默认 `generateAudio = true`（`VideoModelConfig.swift:87`）；Rust 端建议 `impl Default for VideoParams` 或 builder 默认 `generate_audio: true`。

**URL 装配顺序（上游切片逻辑，必须复刻）** — 证据 `VideoGenerationSubmission.swift:287-304`：
上传产出的 `uploaded: Vec<String>` 按 **frames → imageRefs → videoRefs → audioRefs** 顺序切片：
- `frames = uploaded[0..frameCount]`，其中 `start_frame_url = frames[0]`、`end_frame_url = frames[1]`（若 `frameCount>1`）— `:277-278`
- `rest = uploaded[frameCount..]`，依次 `imageRefs[..imageRefCount]`、`videoRefs[..videoRefCount]`、`audioRefs[..audioRefCount]` — `:294-303`
- **视频编辑模型（`requiresSourceVideo`）特殊**：`source_video_url = uploaded[0]`，`reference_image_urls = uploaded[1..]`，frames 全 nil — `VideoGenerationSubmission.swift:66-77`

#### 1.2.3 AudioParams — 证据 `AudioModelConfig.swift:3-27`（同时覆盖 TTS / 音乐 / 音效）

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioParams {
    pub prompt: String,                         // :4
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,                  // :5 encodeIfPresent (:21) — 仅 TTS
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lyrics: Option<String>,                 // :6 encodeIfPresent (:22) — MiniMax 音乐
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style_instructions: Option<String>,     // :7 styleInstructions encodeIfPresent (:23)
    pub instrumental: bool,                      // :8 必填 (:24)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<u32>,          // :9 durationSeconds encodeIfPresent (:25)
    #[serde(rename = "videoURL", skip_serializing_if = "Option::is_none")]
    pub video_url: Option<String>,              // :10 encodeIfPresent (:26) — video-to-music/sfx
}
```
> wire 名：`styleInstructions`/`durationSeconds` 走 camelCase；`videoURL` 大写需 rename。
> `video_url` 装配：上游在 `AudioGenerationSubmission.buildParams` 里，若 `videoURL == nil` 用 `uploaded.first` 兜底（`AudioGenerationSubmission.swift:31-34`）。GenClient 的 video-to-audio 路径同样：先 `upload_reference` 视频，把返回 URL 填 `video_url`。

#### 1.2.4 UpscaleParams — 证据 `UpscaleModelConfig.swift:3-15`

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpscaleParams {
    pub source_url: String,        // :4 sourceURL 必填 (:12)
    pub duration_seconds: u32,     // :5 durationSeconds 必填 (:13)（图像放大时为占位时长）
}
```
> wire 名：`sourceURL` 大写需 `#[serde(rename = "sourceURL")]`；`durationSeconds` camelCase。

### 1.3 Job 状态机与结果（逐字段复刻）

```rust
// crates/opentake-gen/src/job.rs
use serde::Deserialize;

/// 复刻 BackendGenerationStatus（GenerationBackend.swift:112-114）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus { Queued, Running, Succeeded, Failed }

impl JobStatus {
    pub fn is_terminal(self) -> bool { matches!(self, JobStatus::Succeeded | JobStatus::Failed) }
}

/// 复刻 BackendGenerationJob（GenerationBackend.swift:116-123）。
/// 注意上游字段名 `_id`（Convex 文档 id）；OpenTake proxy 用 `id`，故两名都接受。
#[derive(Debug, Clone, Deserialize)]
pub struct GenerationJob {
    #[serde(rename = "id", alias = "_id")]
    pub id: String,                          // :117 _id
    pub status: JobStatus,                    // :118
    #[serde(rename = "resultUrls", default)]
    pub result_urls: Option<Vec<String>>,    // :119 resultUrls
    #[serde(rename = "errorMessage", default)]
    pub error_message: Option<String>,       // :120 errorMessage
    #[serde(rename = "costCredits", default)]
    pub cost_credits: Option<i64>,           // :121 costCredits（托管计费；BYOK 恒 None）
    #[serde(rename = "completedAt", default)]
    pub completed_at: Option<f64>,           // :122 completedAt（epoch 毫秒，上游 Double）
}
```

**状态机语义（消费侧，复刻 `GenerationService.runJob` `:338-361`）**：

```
          submit() → job_id
                │
          watch(job_id)  ┌─────────────┐
                ▼        ▼             │ (queued|running: continue)
        ┌──────────┐  ┌─────────┐      │
        │ queued   │─▶│ running │──────┘
        └──────────┘  └────┬────┘
                           ├──────────────▶ succeeded → 下载 result_urls[i]（见 §5.3 与 domain 衔接）
                           └──────────────▶ failed    → error_message ?? "Generation failed"
```

终态处理规则（复刻 `GenerationService.finalizeSuccess` `:364-407`）：
- `succeeded` 但 `result_urls` 为空/None → 视为失败：`"No URL in response"`（`:372-379`）。
- `result_urls.len() < placeholders.len()` → 多出的占位标记失败，不报全局错（`:380-382`、`:385-389`）。
- N 图生成：`result_urls[i]` 一一对应第 i 个占位资产。

### 1.4 错误类型（复刻上游错误码契约 A6）

```rust
// crates/opentake-gen/src/error.rs
#[derive(Debug, thiserror::Error)]
pub enum GenError {
    #[error("backend not configured")]
    NotConfigured,                              // GenerationBackend.swift:126,132
    #[error("sign in to continue")]
    Unauthenticated,                            // 401 / code "unauthenticated" — PalmierClient.swift:84,87
    #[error("{0}")]
    InsufficientCredits(String),                // 402 / code "insufficient_credits" — PalmierClient.swift:85,88
    #[error("transport error: {0}")]
    Transport(String),                          // GenerationBackend.swift:127
    #[error("api error {status} [{code}]: {message}")]
    Api { status: u16, code: String, message: String }, // GenerationBackend.swift:128,82-88
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
```
> HTTP 错误解析复刻 `assertHTTPOK`（`GenerationBackend.swift:76-90`）+ `PalmierClientError.from`（`PalmierClient.swift:80-91`）：先尝试解析 `{"error":{"code","message"}}` 信封；按 code 优先，再按 status 兜底（401→Unauthenticated，402→InsufficientCredits）。
> 边界层（Tauri command）按 `AGENTS.md:66` 转 `Err(String)`，用 `e.to_string()`。

---

## 2. BYOK 模式：provider adapters + keyring + 静态 catalog

### 2.1 Provider adapter trait

每个厂商一个 adapter，职责（复刻分析报告 §4.1 第 4 条 + §4.2 fal 示例 `03-闭源云边界.md:206-214`）：
1. 把统一 `GenerationParams` 翻译成厂商请求；
2. 提交后返回一个 `job_id`（厂商的 request_id / prediction_id）；
3. 轮询厂商 status，归一化成 `GenerationJob`（statuses → 我们的 4 态，输出 → `result_urls`）。

```rust
// crates/opentake-gen/src/provider/mod.rs
#[async_trait::async_trait]
pub trait ProviderAdapter: Send + Sync {
    /// adapter 短名，用作 model id 前缀（"fal" / "replicate" / "openai" / "elevenlabs"）。
    fn prefix(&self) -> &'static str;

    /// 提交：把统一 params 映射成厂商请求，返回归一化 job（通常状态 queued/running）。
    async fn submit(&self, model: &ModelRoute, params: &GenerationParams) -> Result<GenerationJob, GenError>;

    /// 轮询一次厂商 job 状态，归一化。
    async fn poll(&self, job_id: &str) -> Result<GenerationJob, GenError>;

    /// 上传一个引用文件 → 公开 URL（厂商各异：fal 有 storage、其他需外部 S3/R2）。
    async fn upload(&self, path: &std::path::Path, content_type: &str) -> Result<String, GenError>;
}

/// model id 解析：完整 id = "<prefix>:<vendorModel>"，例 "fal:flux-pro"。
pub struct ModelRoute { pub prefix: String, pub vendor_model: String }

pub struct ProviderRegistry { /* HashMap<String /*prefix*/, Arc<dyn ProviderAdapter>> */ }
impl ProviderRegistry {
    pub fn route(&self, model_id: &str) -> Result<(&dyn ProviderAdapter, ModelRoute), GenError>;
}
```
> 路由前缀法：上游 A1 用不透明 id；OpenTake BYOK 用 `prefix:vendorModel` 让“换 fal 为 Replicate”只改前缀、客户端零改动（分析报告 §4.4 “provider 中立” `03-闭源云边界.md:279`）。

### 2.2 四个 adapter 的参数映射（实现就绪映射表）

下表给出**统一 params → 厂商请求字段**的精确映射。adapter 只需实现这些键转换 + 状态归一化。

#### 2.2.1 fal.ai（`prefix = "fal"`）— queue API

| 我方字段 | fal 请求字段 | 备注 |
|---|---|---|
| 提交端点 | `POST https://queue.fal.run/<vendorModel>` | 同步/排队混合；返回 `request_id` |
| `image.prompt` / `video.prompt` / `audio.prompt` | `prompt` | |
| `image.imageURLs[0]` | `image_url` | 单图厂商 |
| `image.imageURLs[]` | `image_urls` | 多图厂商 |
| `image.aspectRatio` | `aspect_ratio` / `image_size` | 按 vendorModel 能力（catalog 决定） |
| `video.duration` | `duration` | 秒 |
| `video.startFrameURL` | `image_url`（i2v） | |
| `video.endFrameURL` | `end_image_url` | |
| `video.referenceImageURLs[]` | `reference_image_urls` / elements | Seedance/Kling |
| `video.generateAudio` | `enable_audio` / `with_audio` | |
| `upscale.sourceURL` | `video_url` / `image_url` | |
| 轮询 | `GET https://queue.fal.run/<vendorModel>/requests/<id>/status` → 终态再 `GET .../<id>` 取 `output` | |
| 状态归一 | `IN_QUEUE`→Queued；`IN_PROGRESS`→Running；`COMPLETED`→Succeeded；`FAILED`→Failed | |
| `result_urls` | `output.images[].url` / `output.video.url` / `output.audio.url` | 按 `responseShape` |
| 上传 | fal storage `POST https://rest.alpha.fal.ai/storage/upload` → 返回 url | adapter 内置 |
| 鉴权 | header `Authorization: Key <FAL_KEY>` | |

#### 2.2.2 Replicate（`prefix = "replicate"`）— predictions API

| 我方字段 | Replicate 请求 | 备注 |
|---|---|---|
| 提交端点 | `POST https://api.replicate.com/v1/predictions`，body `{ "version": <vendorModel>, "input": {...} }` | 返回 `id` |
| 各 params 字段 | 平铺进 `input.{prompt,image,...}` | 按模型 schema |
| 轮询 | `GET https://api.replicate.com/v1/predictions/<id>` | |
| 状态归一 | `starting`→Queued；`processing`→Running；`succeeded`→Succeeded；`failed`/`canceled`→Failed | |
| `result_urls` | `output`（string 或 string[]）归一为数组 | |
| 上传 | `POST https://api.replicate.com/v1/files` → `urls.get` | adapter 内置 |
| 鉴权 | header `Authorization: Bearer <REPLICATE_API_TOKEN>` | |

#### 2.2.3 OpenAI（`prefix = "openai"`）— 同步 images / TTS

| 我方字段 | OpenAI 请求 | 备注 |
|---|---|---|
| image 提交 | `POST https://api.openai.com/v1/images/generations`，`{model, prompt, size, n}` | **同步返回**，adapter 直接产出 `succeeded` job |
| `image.numImages` | `n` | |
| `image.aspectRatio`/`resolution` | 映射 → `size`（如 `1024x1024`） | |
| `image.imageURLs` | 走 `/v1/images/edits`（multipart） | 编辑模型 |
| audio(TTS) 提交 | `POST https://api.openai.com/v1/audio/speech`，`{model, input: prompt, voice}` | 同步返回二进制 → adapter 落临时对象存储或 data URL → `result_urls` |
| `audio.voice` | `voice` | |
| 状态归一 | 同步 → 直接 Succeeded；HTTP 错误 → Failed | 无轮询 |
| 鉴权 | header `Authorization: Bearer <OPENAI_API_KEY>` | |

> 同步厂商在 BYOK 下：adapter 在 `submit` 内即拿到结果，返回的 `GenerationJob` 直接是 `Succeeded` + `result_urls`，`poll` 直接回放缓存结果（用内存 `HashMap<job_id, GenerationJob>`）。`watch` 流首帧即终态，语义不变。

#### 2.2.4 ElevenLabs（`prefix = "elevenlabs"`）— TTS / music / sfx

| 我方字段 | ElevenLabs 请求 | 备注 |
|---|---|---|
| TTS 提交 | `POST https://api.elevenlabs.io/v1/text-to-speech/<voiceId>`，`{text: prompt, model_id}` | 同步返回 audio |
| `audio.voice` | path `<voiceId>`（voice 名→id 由静态 catalog 映射） | |
| music 提交 | `POST https://api.elevenlabs.io/v1/music`（或 sound-generation for sfx） | |
| `audio.duration_seconds` | `duration_seconds` / `music_length_ms` | |
| `audio.instrumental` | 模型相关开关 | |
| 状态归一 | 同步 → Succeeded（落对象存储得 url）；错误 → Failed | |
| 鉴权 | header `xi-api-key: <ELEVENLABS_API_KEY>` | |

> **BYOK 上传的对象存储问题**：fal/Replicate 自带上传端点（adapter `upload` 直接用）。OpenAI/ElevenLabs 没有公开素材托管，且它们多为同步、产物是字节流。两种处理：(a) 同步厂商的产物 adapter 写入用户配置的 S3/R2（可选）或临时 `file://`/`data:` URL 供本地下载；(b) 若 BYOK 用户没配对象存储，则 video-to-* 这类需要先上传引用素材的能力在该 provider 下不可用（catalog 标记 `supportsImageReference=false` 等）。这是 BYOK 模式的真实约束，须在 catalog 与 UI 文案体现。

### 2.3 Keyring 存 key（复刻上游 Keychain 模式）

上游 `AnthropicKeychain`（`AnthropicClient.swift:7-30`）+ `KeychainStore`（`KeychainStore.swift:4-52`）的关键决策，全部复刻：

```rust
// crates/opentake-gen/src/keys.rs
use keyring::Entry;

const SERVICE: &str = "io.opentake.app";   // 复刻 KeychainStore.service（KeychainStore.swift:5）

pub enum ProviderKey { Fal, Replicate, OpenAI, ElevenLabs, Anthropic }

impl ProviderKey {
    fn account(&self) -> &'static str {
        match self {                            // 复刻 AnthropicKeychain.account = "anthropic-api-key"（:8）
            ProviderKey::Fal => "fal-api-key",
            ProviderKey::Replicate => "replicate-api-key",
            ProviderKey::OpenAI => "openai-api-key",
            ProviderKey::ElevenLabs => "elevenlabs-api-key",
            ProviderKey::Anthropic => "anthropic-api-key",
        }
    }
}

pub fn save(k: ProviderKey, value: &str) -> Result<(), GenError> {
    Entry::new(SERVICE, k.account())?.set_password(value)?; Ok(())
}

pub fn load(k: ProviderKey) -> Result<Option<String>, GenError> {
    // #[cfg(debug_assertions)] 下允许 env 覆盖（复刻 AnthropicKeychain.load 的 #if DEBUG，:16-22）
    #[cfg(debug_assertions)]
    if let Ok(v) = std::env::var(k.env_var()) {
        let t = v.trim();
        if !t.is_empty() { return Ok(Some(t.to_string())); }
    }
    match Entry::new(SERVICE, k.account())?.get_password() {
        Ok(s) => { let t = s.trim().to_string(); Ok((!t.is_empty()).then_some(t)) }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn delete(k: ProviderKey) -> Result<(), GenError> {
    match Entry::new(SERVICE, k.account())?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}
```

复刻的关键点：
- **service = bundle id**（`KeychainStore.swift:5`），account = 每 key 一个稳定串（`AnthropicClient.swift:8`）。
- **load 去空白 + 空串视为 None**（`KeychainStore.swift:37-40`）。
- **DEBUG 允许环境变量覆盖**（`AnthropicClient.swift:16-22`，对应 `keys.rs` 的 `#[cfg(debug_assertions)]`）。`env_var()` 映射如 `FAL_KEY`/`REPLICATE_API_TOKEN`/`OPENAI_API_KEY`/`ELEVENLABS_API_KEY`/`ANTHROPIC_API_KEY`。
- **绝不写工程文件/明文配置**（`AGENTS.md` 安全 + 上游全程 Keychain）。
- keyring crate 跨平台：macOS Keychain / Windows Credential Manager / Linux Secret Service（分析报告 §4.4 `03-闭源云边界.md:277`）。

### 2.4 内置静态 catalog（BYOK 无 proxy 也能数据驱动 UI）

BYOK 模式下 `list_models()` 返回**编译进二进制的静态 JSON**（分析报告 §4.4 “统一 catalog” `03-闭源云边界.md:278`）。结构与托管 `/v1/models` **完全一致**（即 §4 的 `CatalogEntry`），让 UI/agent 两模零差异。

```rust
// crates/opentake-gen/src/catalog/builtin.rs
/// 编译期内嵌；BYOK list_models 直接 parse 返回。
const BUILTIN_CATALOG_JSON: &str = include_str!("builtin_catalog.json");

pub fn builtin_catalog() -> Catalog {
    serde_json::from_str::<Vec<CatalogEntry>>(BUILTIN_CATALOG_JSON)
        .expect("builtin catalog must parse")
        .into()
}
```

`builtin_catalog.json` 中每条目的 `id` 用 `prefix:vendorModel`（如 `"fal:flux-pro"`），`creditsPer*` 在 BYOK 下可省略（None；BYOK 不计费），但 `uiCapabilities`（能力矩阵）必须填全，否则 UI 无法校验。维护策略：随版本手工更新这份 JSON（厂商上新模型 = 改 JSON，不改代码，沿用 A5）。

---

## 3. 托管模式 opentake-gen-proxy（axum）端点设计

> 形态：开源、可自托管的**无状态 HTTP 服务 + 轻量任务队列**（分析报告 §4.1 `03-闭源云边界.md:174-197`）。单二进制，Rust + axum，与 core 同栈。
> **建议落位**：新 crate `crates/opentake-gen-proxy`（bin），复用 `opentake-gen` 的 `ProviderAdapter`/`GenerationParams`/`Catalog` 作为 lib。**注意：本规格只规定接口与行为；按任务约束，不在此创建/修改 crate 代码。**

### 3.0 鉴权与中间件（贯穿所有端点）
- 所有 `/v1/*`（除健康检查）要求 `Authorization: Bearer <JWT>`（A6）。中间件验签（OIDC JWKS）→ 解析 `user_id`。
- 错误信封统一 `{"error":{"code","message"}}`（复刻上游解析契约 `PalmierClient.swift:93-100`、`GenerationBackend.swift:155-161`）。
- `401`→`code:"unauthenticated"`；`402`→`code:"insufficient_credits"`（A6 / `PalmierClient.swift:84-85`）。
- provider 中立：客户端永远不见 fal/Replicate（A1 + 分析报告“provider 中立”）。

### 3.1 `GET /v1/models` — 模型目录下发
- 行为：返回 `Vec<CatalogEntry>`（§4 结构），对应上游 `models:list`（`ModelCatalog.swift:54`）。
- 可按 `?type=video|image|audio|upscale` 过滤（复刻 agent `list_models` 的 type 过滤 `ToolExecutor+Generate.swift:374-387`）。
- `creditsPer*` 字段在托管模式必填（计费用），与 BYOK（可空）相反。

### 3.2 `POST /v1/generations` — 提交任务
- 请求体（复刻 `generations:submit` args `GenerationBackend.swift:64-68`）：
  ```jsonc
  { "model": "fal:flux-pro", "params": { "kind": "image", "prompt": "...", ... }, "projectId": "optional" }
  ```
  `params` 即 §1.2 的联合类型 wire 形态。
- 服务端流程：
  1. 鉴权 + 预扣费检查（积分不足 → 402）。
  2. `registry.route(model)` 选 adapter。
  3. `adapter.submit(route, params)` → 厂商 job。
  4. 落库 `{ job_id(我方), user_id, provider, vendor_job_id, status, project_id, created_at }`（队列持久化：SQLite/Postgres，分析报告 `03-闭源云边界.md:196`）。
  5. 返回 `{ "jobId": "<我方 id>" }`（复刻 `SubmitGenerationResult.jobId` `GenerationBackend.swift:151-153,73`）。

### 3.3 `GET /v1/generations/:id` 与 `GET /v1/generations/:id/stream`
- `:id` → 单次快照 `GenerationJob`（§1.3 结构）。服务端按需 `adapter.poll(vendor_job_id)` 刷新并归一化。
- `:id/stream` → SSE，事件 `data: <GenerationJob json>`，直到终态后服务端关闭流（复刻上游 Convex 订阅推送语义 `GenerationService.swift:328-361`；上游用 WebSocket，proxy 用 SSE 等价）。
- **终态时**：服务端结算真实 `costCredits` 写回（复刻“真实扣费由后端在 job 完成时算出 costCredits 回传” `03-闭源云边界.md:141`），并在 job 中返回。
- 客户端预估成本仍走本地 CostEstimator（§4.3），只做展示。

### 3.4 `POST /v1/uploads/sign` — 预签名上传 URL
- 复刻上游三步上传的第 1 步（`uploads:generateUploadTicket` `GenerationBackend.swift:30`），但换成对象存储预签名（S3/R2，分析报告 `03-闭源云边界.md:154,195`）。
- 请求：`{ "contentType": "image/jpeg" }`。响应：
  ```jsonc
  { "uploadUrl": "https://<r2-presigned-PUT-or-POST>", "publicUrl": "https://cdn.../<key>" }
  ```
- 客户端流程（GenClient.upload_reference，复刻 `GenerationBackend.uploadReference` `:20-54`）：
  1. `POST /v1/uploads/sign` 取 `uploadUrl` + `publicUrl`。
  2. 直传字节到 `uploadUrl`（`PUT`/`POST`，带 `Content-Type`）。
  3. 用 `publicUrl` 作为后续 `params` 里的素材 URL。
  > 与上游差异：上游第 3 步还有 `uploads:commitUpload`（`:48-53`）把 storageId 换 URL；预签名方案可省该步（直传后 publicUrl 已知），简化为两步。`UploadTicket` 结构：`{ upload_url: String, public_url: String }`。

### 3.5 content-type 推断表（复刻 `GenerationService.contentType` `:266-287`）
GenClient 在 `upload_reference` 自动推断（按扩展名），下表逐项复刻：

| 扩展名 | content-type | 证据 |
|---|---|---|
| jpg/jpeg | image/jpeg | `:268` |
| png | image/png | `:269` |
| webp | image/webp | `:270` |
| heic | image/heic | `:271` |
| gif | image/gif | `:272` |
| mp4/m4v | video/mp4 | `:273` |
| mov | video/quicktime | `:274` |
| mp3 | audio/mpeg | `:275` |
| wav | audio/wav | `:276` |
| m4a | audio/mp4 | `:277` |
| 兜底 image/video/audio | image/jpeg / video/mp4 / audio/mpeg | `:279-285` |

### 3.6 `POST /v1/agent/stream` — LLM 代理（SSE）
- 复刻上游唯一裸 HTTP 路径（`PalmierClient.swift:32-63`）。请求体复刻 Anthropic Messages 形态（`AnthropicRequestBody.build` `AgentClientTypes.swift:156-193`）：
  ```jsonc
  { "model": "claude-sonnet-4-6", "max_tokens": 8192, "stream": true,
    "system": [{ "type":"text","text":"...","cache_control":{"type":"ephemeral"} }],
    "tools": [...], "messages": [...] }
  ```
- 服务端：验 JWT → 预扣费 → 转发 Anthropic（持平台 key）→ 把 Anthropic SSE 透传回客户端 → 终态结算扣费。
- 错误码同 A6（401/402）。
- **BYOK 旁路**：用户配了 Anthropic key 时，`opentake-agent` 直连 `api.anthropic.com`（复刻 `AnthropicClient` `:32-87` + `selectClient` 优先级 `AgentService.swift:52-59`），完全不经 proxy。详见 §5.2。

### 3.7 端点汇总

| 方法 + 路径 | 用途 | 上游对应 | 鉴权 |
|---|---|---|---|
| `GET /healthz` | 健康检查 | — | 否 |
| `GET /v1/models` | 模型目录 + 定价 | `models:list` | 是 |
| `POST /v1/generations` | 提交任务 → jobId | `generations:submit` | 是 |
| `GET /v1/generations/:id` | 单次状态 | `generations:byId`（读） | 是 |
| `GET /v1/generations/:id/stream` | SSE 状态推送 | `generations:byId`（订阅） | 是 |
| `POST /v1/uploads/sign` | 预签名上传 URL | `uploads:generateUploadTicket`(+commit) | 是 |
| `POST /v1/agent/stream` | LLM 文本代理 | `POST /v1/agent/stream` | 是 |

---

## 4. CatalogEntry + 能力矩阵（托管/BYOK 共用结构）

逐字段复刻 `ModelCatalog.swift:112-241`。**两模同结构**，是 UI/agent 数据驱动的唯一来源（A5）。

```rust
// crates/opentake-gen/src/catalog/entry.rs
#[derive(Debug, Clone, Deserialize)]
pub struct CatalogEntry {
    pub id: String,                                   // :113
    pub kind: ModelKind,                              // :114 video|image|audio|upscale
    #[serde(rename = "displayName")]
    pub display_name: String,                         // :115
    #[serde(rename = "allowedEndpoints", default)]
    pub allowed_endpoints: Vec<String>,               // :116（后端内部端点名；BYOK 可空）
    #[serde(rename = "responseShape")]
    pub response_shape: ResponseShape,                // :117 video|images|audio|upscaledImage
    #[serde(rename = "uiCapabilities")]
    pub ui_capabilities: UiCapabilities,              // :118（按 kind 反序列化，见下）
    #[serde(rename = "creditsPerSecond", default)]
    pub credits_per_second: Option<std::collections::HashMap<String, f64>>,   // :119
    #[serde(rename = "audioDiscountRate", default)]
    pub audio_discount_rate: Option<std::collections::HashMap<String, f64>>,  // :120
    #[serde(rename = "creditsPerImage", default)]
    pub credits_per_image: Option<std::collections::HashMap<String, f64>>,    // :121
    #[serde(default)]
    pub qualities: Option<Vec<String>>,               // :122
    #[serde(rename = "audioPricing", default)]
    pub audio_pricing: Option<AudioPricing>,          // :123
    #[serde(rename = "creditsPerSecondUpscale", default)]
    pub credits_per_second_upscale: Option<f64>,      // :124
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelKind { Video, Image, Audio, Upscale }   // :126

#[derive(Debug, Clone, Copy, Deserialize)]
pub enum ResponseShape {                               // :127-129
    #[serde(rename = "video")] Video,
    #[serde(rename = "images")] Images,
    #[serde(rename = "audio")] Audio,
    #[serde(rename = "upscaledImage")] UpscaledImage,
}

/// 复刻 AudioPricing（ModelCatalog.swift:138-161），internally-tagged "mode"。
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(tag = "mode")]
pub enum AudioPricing {
    #[serde(rename = "perThousandChars")] PerThousandChars { rate: f64 },  // :148-149
    #[serde(rename = "perSecond")] PerSecond { rate: f64 },                // :150-151
    #[serde(rename = "flat")] Flat { price: f64 },                         // :152-153
}

/// uiCapabilities 按 kind 分派反序列化（复刻 CatalogEntry.init from decoder :182-191）。
/// 实现：自定义 Deserialize，先读 kind，再按 kind decode 对应 Caps。
#[derive(Debug, Clone)]
pub enum UiCapabilities {
    Video(VideoCaps), Image(ImageCaps), Audio(AudioCaps), Upscale(UpscaleCaps),
}
```

**VideoCaps** — 复刻 `ModelCatalog.swift:195-211`：
```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoCaps {
    pub durations: Vec<u32>,                         // :196
    pub resolutions: Option<Vec<String>>,           // :197
    pub aspect_ratios: Vec<String>,                 // :198
    pub supports_first_frame: bool,                 // :199
    pub supports_last_frame: bool,                  // :200
    pub max_reference_images: u32,                  // :201
    pub max_reference_videos: u32,                  // :202
    pub max_reference_audios: u32,                  // :203
    pub max_total_references: Option<u32>,          // :204
    pub max_combined_video_ref_seconds: Option<f64>,// :205
    pub max_combined_audio_ref_seconds: Option<f64>,// :206
    pub frames_and_references_exclusive: bool,      // :207
    pub reference_tag_noun: String,                 // :208
    pub requires_source_video: bool,                // :209
    pub requires_reference_image: bool,             // :210
}
```

**ImageCaps** — 复刻 `:213-219`：
```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageCaps {
    pub resolutions: Option<Vec<String>>,     // :214
    pub aspect_ratios: Vec<String>,           // :215
    pub qualities: Option<Vec<String>>,       // :216
    pub supports_image_reference: bool,       // :217
    pub max_images: u32,                      // :218
}
```

**AudioCaps** — 复刻 `:221-234`（`category` 字符串 "tts"|"music"|"sfx"，`inputs` "text"|"video"）：
```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioCaps {
    pub category: String,                     // :222 "tts"|"music"|"sfx"
    pub voices: Option<Vec<String>>,          // :223
    pub default_voice: Option<String>,        // :224
    pub supports_lyrics: bool,                // :225
    pub supports_instrumental: bool,          // :226
    pub supports_style_instructions: bool,    // :227
    pub durations: Option<Vec<u32>>,          // :228
    pub min_prompt_length: u32,               // :229
    pub inputs: Option<Vec<String>>,          // :230 "text"|"video"
    pub prompt_label: Option<String>,         // :231
    pub min_seconds: Option<u32>,             // :232
    pub max_seconds: Option<u32>,             // :233
}
```

**UpscaleCaps** — 复刻 `:236-240`：
```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpscaleCaps {
    pub speed: String,                        // :237 "Fast"|"Medium"|"Slow"
    pub p75_duration_seconds: u32,            // :238
    pub supported_types: Vec<String>,         // :239 "video"|"image"
}
```

### 4.3 客户端成本预估（复刻 `CostEstimator.swift:3-108`，仅展示用）

放 `opentake-gen`（纯函数，可单测）。真实扣费由 proxy 终态回传（§3.3）。逐函数复刻：

| 函数 | 公式 | 证据 |
|---|---|---|
| `video_cost` | `ceil(rate * duration)`，`rate = creditsPerSecond[resolution] ?? [""]`；若 `!generateAudio` 乘 `audioDiscount`（`audioDiscountRate[resolution] ?? [""]`） | `:5-17` + `audioDiscount :44-48` |
| `image_cost` | 先查 2D `"<res>|<quality>"`，再查 quality-only，再 `creditsPerImage[res] ?? [""]`；乘 `numImages` | `:19-37` |
| `audio_cost` | perThousandChars: `ceil(rate*chars/1000)`；perSecond: `ceil(rate*secs)`；flat: `ceil(price)` | `:39-57` |
| `upscale_cost` | `ceil(creditsPerSecondUpscale * max(1,duration))` | `:59-62` |
| `ceil_credits` | `credits<=0 → 0`，否则 `ceil` | `:104-107` |
| `resolved_rate` | `dict[key] ?? dict[""]` | `:99-102` |

> Rust 注意：`ceil` 用 `f64::ceil() as i64`；`resolved_rate` 的空串键 `""` 是“默认档”，必须保留。

---

## 5. 与 domain（GenerationInput）/ agent 的接口

### 5.1 GenerationInput 落在 opentake-domain（逐字段复刻 `MediaManifest.swift:36-63`）

`GenerationInput` 是**持久化到工程文件**的领域类型（上游存于 `MediaManifestEntry.generationInput` `MediaManifest.swift:26`），故放零依赖 `opentake-domain`，按 `AGENTS.md:62` 全字段 `#[serde(default)]` + `Option<T>`：

```rust
// crates/opentake-domain/src/generation.rs（新增；domain 当前无此文件）
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationInput {
    pub prompt: String,                              // :37
    pub model: String,                               // :38
    pub duration: i64,                               // :39（Int；秒；image 时为 0）
    pub aspect_ratio: String,                        // :40
    #[serde(default)] pub resolution: Option<String>,     // :41
    #[serde(default)] pub quality: Option<String>,        // :42
    #[serde(rename = "imageURLs", default)] pub image_urls: Option<Vec<String>>, // :43
    #[serde(default)] pub num_images: Option<i64>,        // :45 image-only
    #[serde(default)] pub voice: Option<String>,          // :47 audio-only
    #[serde(default)] pub lyrics: Option<String>,         // :48
    #[serde(default)] pub style_instructions: Option<String>, // :49
    #[serde(default)] pub instrumental: Option<bool>,     // :50
    #[serde(default)] pub generate_audio: Option<bool>,   // :52 video-only
    #[serde(rename = "referenceImageURLs", default)] pub reference_image_urls: Option<Vec<String>>, // :53
    #[serde(rename = "referenceVideoURLs", default)] pub reference_video_urls: Option<Vec<String>>, // :54
    #[serde(rename = "referenceAudioURLs", default)] pub reference_audio_urls: Option<Vec<String>>, // :55
    #[serde(rename = "imageURLAssetIds", default)] pub image_url_asset_ids: Option<Vec<String>>,    // :58
    #[serde(rename = "referenceImageAssetIds", default)] pub reference_image_asset_ids: Option<Vec<String>>, // :59
    #[serde(rename = "referenceVideoAssetIds", default)] pub reference_video_asset_ids: Option<Vec<String>>, // :60
    #[serde(rename = "referenceAudioAssetIds", default)] pub reference_audio_asset_ids: Option<Vec<String>>, // :61
    #[serde(default)] pub created_at: Option<f64>,        // :62（epoch；上游 Date）
}
```

**`GenerationInput` → `GenerationParams` 映射（在 opentake-gen，依赖 domain）**：上游通过 `*GenerationSubmission.buildParams` 闭包把 `GenerationInput` + 上传后的 URL 拼成 `params`。OpenTake 把这层做成纯函数 `build_params(input: &GenerationInput, uploaded: &[String], model_kind: ModelKind) -> GenerationParams`，复刻：
- image：`ImageGenerationSubmission.swift:54-63`（`imageURLs = uploaded`）。
- video text-to-video：URL 切片顺序见 §1.2.2（`VideoGenerationSubmission.swift:264-284`）。
- video edit（requiresSourceVideo）：`source_video_url = uploaded[0]`，其余为 image refs（`VideoGenerationSubmission.swift:66-77`）。
- audio：`video_url` 兜底 `uploaded.first`（`AudioGenerationSubmission.swift:31-34`）。

> 职责切分：domain 只放**数据类型**（`GenerationInput`，可序列化、零 IO）；`build_params` 含装配逻辑，放 gen（gen → domain 单向依赖）。

### 5.2 与 opentake-agent 的接口（A7 双模选择 + 生成工具）

`opentake-agent`（`crates/opentake-agent/src/lib.rs:1-6` 已声明工具层 + MCP + chat）通过 `GenClient` 调用生成；文本流则按上游 `selectClient` 优先级（`AgentService.swift:52-59`）：

**(a) Agent 生成工具 → GenClient**：上游 agent 工具（`generate_video/image/audio`、`upscale_media`、`list_models`，枚举 `ToolDefinitions.swift:19-24`）的执行体（`ToolExecutor+Generate.swift`）构造 `GenerationInput` 再 submit。OpenTake 的工具执行器：解析工具 args → `GenerationInput` → `build_params` → `GenClient.submit` → `GenClient.watch` → 下载。工具 arg 名（即 proxy/GenClient 的逻辑输入）逐项复刻：

| 工具 | 关键 args | 证据 |
|---|---|---|
| `generate_video` | `prompt, model, duration, aspectRatio, resolution, startFrameMediaRef, endFrameMediaRef, sourceVideoMediaRef, sourceClipId, referenceImageMediaRefs[], referenceVideoMediaRefs[], referenceAudioMediaRefs[], name, folderId` | `ToolDefinitions.swift:367-373`；执行 `ToolExecutor+Generate.swift:78-147` |
| `generate_image` | `prompt, model, aspectRatio, resolution, quality, referenceMediaRefs[], name, folderId` | 执行 `ToolExecutor+Generate.swift:150-195`（`:160-163`） |
| `generate_audio` | `prompt, model, voice, lyrics, styleInstructions, instrumental, duration, videoSourceMediaRef, videoSourceStartFrame, videoSourceEndFrame, name, folderId` | `ToolDefinitions.swift:397-407`；执行 `:197-311` |
| `upscale_media` | `mediaRef, model, sourceClipId` | 执行 `:313-348` |
| `list_models` | `type?`(video/image/audio/upscale) | `:373-396` |

> `*MediaRef`/`*MediaRefs` 是本地资产 id；执行器先 `upload_reference` 换成 URL 再进 params（A2）。`sourceClipId` + trim → 只上传裁剪片段（`ToolExecutor+Generate.swift:350-371`，对应 OpenTake 的 trim-extract，由 opentake-media/FFmpeg 实现，不在本 crate）。

**(b) Agent 文本流 → 双模选择**（复刻 `AgentService.selectClient` `:52-59`）：
```
有 Anthropic key（keyring/env）          → opentake-agent 直连 api.anthropic.com（复刻 AnthropicClient :32-87）
无 key 且托管已配置 + 已登录             → GenClient.agent_stream → proxy /v1/agent/stream（复刻 PalmierClient :32-63）
都没有                                   → 不可用（复刻 selectClient 返回 nil :58）
```
> `agent_stream` 的 `AgentRequest`/`AgentEvent` 复刻 `AnthropicMessage`/`AnthropicToolSchema`/`AnthropicStreamEvent`（`AgentClientTypes.swift:29-45`）+ SSE 解析（`AgentClientTypes.swift:88-152`）。这部分若由 opentake-agent 主导，则 `GenClient.agent_stream` 仅作托管模式的薄封装。

### 5.3 `canGenerate` 能力信号（与 agent context signal 衔接）
上游每次 `get_timeline` 返回带 `canGenerate = isSignedIn && hasCredits`（`ToolExecutor+Timeline.swift:45`）。OpenTake 等价：
- 托管模式：`canGenerate = TokenProvider 可取 token && remaining_credits > 0`（积分来自账户服务，超出本 crate）。
- BYOK 模式：`canGenerate = 至少配置了一个 provider key`（generate 工具用到的 provider 有 key）。
- `canGenerate=false` 时所有 generate/upscale 工具应 fail-fast，文案复刻上游（`ToolExecutor+Generate.swift:6-11`："Generation requires signing in" / "Out of credits"；BYOK 版改为 "未配置 <provider> API key"）。

### 5.4 下载与终态（与 opentake-media / project 衔接）
`watch` 收到 `succeeded` → 逐个下载 `result_urls[i]` 落到工程 media 目录（复刻 `GenerationService.downloadAndFinalize` `:192-218`：按远端扩展名修正本地扩展名 `:197-201`，原子 move `:202-203`）。**下载/落盘属 opentake-media/opentake-project 职责**，本 crate 只提供 `watch` 流 + URL；具体落盘在 core 编排。

---

## 6. 实施清单（按依赖顺序，每步带验收）

> 遵循 `AGENTS.md`：`opentake-domain` 零网络/FS；gen 内部 `anyhow`，边界转 `Err(String)`；每命令一个 test module，覆盖率 ≥80%。
> 本清单是后续 PR 的施工序；当前任务只产出本规格，不写 crate 代码。

**阶段 G0 — 类型骨架（纯数据，无网络）**
1. `opentake-domain`：新增 `generation.rs`，定义 `GenerationInput`（§5.1）。 → 验收：`serde_json` round-trip 测试 + 读“缺字段旧 JSON”不 panic（`#[serde(default)]` 全覆盖）。
2. `opentake-gen`：`params.rs` 定义 `GenerationParams` 4 变体（§1.2）。 → 验收：对 4 个变体各写一个 `serde_json::to_value` 测试，**断言 wire 键名与上游逐字一致**（`imageURLs`/`sourceVideoURL`/`kind` 等），断言空 Vec / None 字段不出现（复刻 `encodeIfPresent` / 仅非空写）。
3. `opentake-gen`：`job.rs` 定义 `JobStatus`/`GenerationJob`（§1.3）+ `error.rs`（§1.4）。 → 验收：反序列化上游样例 JSON（含 `_id` 别名、缺 `resultUrls`）通过；`is_terminal` 测试。

**阶段 G1 — catalog**
4. `opentake-gen`：`catalog/entry.rs` 定义 `CatalogEntry` + 4 `*Caps` + `AudioPricing`（§4），实现 `UiCapabilities` 的 kind 分派 `Deserialize`（复刻 `CatalogEntry.init` `:169-192`）。 → 验收：用 4 类各一条样例 JSON 反序列化通过。
5. `opentake-gen`：`cost.rs` 复刻 `CostEstimator`（§4.3）。 → 验收：对 video(含 audio 折扣)/image(2D 矩阵)/audio(三种 pricing)/upscale 各写黄金值测试，对齐上游公式与 `ceil` 行为。
6. `opentake-gen`：`catalog/builtin.rs` + `builtin_catalog.json`（§2.4），首版填 fal/Replicate/OpenAI/ElevenLabs 各 1-2 个模型。 → 验收：`builtin_catalog()` 解析成功（`include_str!` 编译期内嵌）。

**阶段 G2 — keyring + provider adapters（BYOK 可跑）**
7. `opentake-gen`：`keys.rs` 复刻 keychain（§2.3），含 DEBUG env 覆盖。 → 验收：save/load/delete 往返（CI 用 `keyring` mock 或跳过真实 keychain，仅测 env 覆盖分支）。
8. `opentake-gen`：`provider/mod.rs` 定义 `ProviderAdapter` trait + `ProviderRegistry` + `ModelRoute` 解析（§2.1）。 → 验收：`route("fal:flux-pro")` 返回正确 prefix/vendorModel；未知前缀 → `GenError`。
9. `opentake-gen`：`provider/fal.rs`、`replicate.rs`、`openai.rs`、`elevenlabs.rs`（§2.2 映射表）。 → 验收：每 adapter 用 mock HTTP（`wiremock`）测 submit→poll→归一化：状态映射正确、`result_urls` 提取正确、错误 → Failed。同步厂商（OpenAI/ElevenLabs）测 submit 即终态 + poll 回放。
10. `opentake-gen`：`client.rs` 的 `GenClient::byok` + `list_models`(静态)/`submit`/`get`/`watch`(轮询)/`upload_reference`(委托 adapter)（§1.1）。 → 验收：BYOK 全链路 mock 测试：submit → watch 至 succeeded → 拿到 `result_urls`。

**阶段 G3 — 托管模式 + GenClient managed**
11. `opentake-gen`：`build_params`（§5.1）纯函数。 → 验收：video URL 切片顺序测试（frames→imageRefs→videoRefs→audioRefs，含 edit 模型 source_video_url）严格对齐 `VideoGenerationSubmission` 切片。
12. `opentake-gen`：`GenClient::managed`（§1.1）+ HTTP 客户端：`/v1/models`、`/v1/generations`、`/v1/generations/:id(+/stream)`、`/v1/uploads/sign`、上传两步（§3.4）、错误信封解析（§3.0/§1.4）。 → 验收：mock proxy 测全部端点；错误码 401→Unauthenticated、402→InsufficientCredits、`{"error":{code,message}}` 解析。
13. （独立交付，按约束本规格不实现代码）`crates/opentake-gen-proxy`（axum bin）：实现 §3 全部端点，复用 gen 的 adapters/params/catalog；JWT 中间件 + 预扣费 + 终态结算 + 队列持久化（SQLite 起步）。 → 验收：集成测试覆盖提交→轮询→SSE→上传签名；provider 中立（响应不泄露厂商）；鉴权/计费分支。

**阶段 G4 — agent / core 衔接**
14. `opentake-agent`：generate/upscale/list_models 工具执行器调用 `GenClient`（§5.2a），arg 名复刻表格。 → 验收：工具 args → `GenerationInput` → `build_params` 的转换测试；fail-fast 文案（§5.3）。
15. `opentake-agent`：文本流双模选择（§5.2b，复刻 `selectClient`）。 → 验收：有 key 走直连、无 key+登录走 proxy、都无→不可用 三分支测试。
16. core 编排：`watch` succeeded → 下载落盘（§5.4，复刻 `downloadAndFinalize`，落 opentake-media/project）。 → 验收：扩展名修正 + 原子 move 测试。

---

## 7. 风险与决策点（需实现前确认）

| # | 项 | 上游做法 | OpenTake 建议 | 需确认 |
|---|---|---|---|---|
| R1 | 上传第 3 步 commit | `uploads:commitUpload` 把 storageId 换 URL（`GenerationBackend.swift:48-53`） | 预签名方案省去该步（直传后 publicUrl 已知），两步式 | 是否需保留 commit 以兼容某些对象存储的 ACL 流程 |
| R2 | watch 传输 | Convex WebSocket 推送（`GenerationService.swift:328-336`） | proxy 用 SSE；BYOK 用轮询 | SSE 是否够（vs WebSocket）；轮询间隔（建议 2s，带退避） |
| R3 | BYOK 同步厂商素材托管 | 不适用（上游全托管） | OpenAI/ElevenLabs 无素材托管 → video-to-* 在无对象存储的 BYOK 下禁用（§2.2.4） | 是否要求 BYOK 用户配 S3/R2 才解锁这些能力 |
| R4 | wire 字段名大小写 | `imageURLs`/`sourceVideoURL` 等 URL 全大写 | 逐字段 `#[serde(rename)]` 复刻 | proxy 是否要同时接受 camelCase 变体以放宽 |
| R5 | `cost_credits` BYOK 语义 | 后端结算回传（`03-闭源云边界.md:141`） | BYOK 恒 `None`（不计费） | UI 在 BYOK 下是否仍显示预估积分（建议显示“按厂商计费”提示） |
| R6 | agent_stream 归属 | 上游在客户端 selectClient | 建议 opentake-agent 主导，`GenClient.agent_stream` 仅薄封装托管路径 | 是否把 agent_stream 从 GenClient 移出，避免与 opentake-agent 职责重叠 |

---

## 附录 A — 上游源文件证据索引（绝对路径）

- `…/palmier-pro-upstream/Sources/PalmierPro/Generation/GenerationBackend.swift` — RPC 层 / `BackendGenerationParams` / `BackendGenerationJob` / 三步上传 / 错误信封（核心）
- `…/Generation/GenerationService.swift` — 编排：上传→submit→watch→下载；终态处理（`:328-407`）；content-type 表（`:266-287`）
- `…/Generation/Catalog/ModelCatalog.swift` — `CatalogEntry` + 4 `*Caps` + `AudioPricing`（catalog 蓝图）
- `…/Generation/Catalog/ImageModelConfig.swift` — `ImageGenerationParams`（`:3-25`）
- `…/Generation/Catalog/VideoModelConfig.swift` — `VideoGenerationParams`（`:67-124`）
- `…/Generation/Catalog/AudioModelConfig.swift` — `AudioGenerationParams`（`:3-27`）
- `…/Generation/Catalog/UpscaleModelConfig.swift` — `UpscaleGenerationParams`（`:3-15`）
- `…/Generation/Catalog/CostEstimator.swift` — 成本预估公式（`:3-108`）
- `…/Generation/Submission/{Image,Video,Audio,Music}GenerationSubmission.swift` — `GenerationInput`→params 装配 + URL 切片顺序
- `…/Account/AccountService.swift` — 账户/积分/计费/认证编排（积分模型 `:117-125`）
- `…/Account/BackendConfig.swift` — 后端配置注入（无硬编码密钥）
- `…/Agent/Clients/PalmierClient.swift` — 托管代理 `/v1/agent/stream`（JWT、错误码 A6）
- `…/Agent/Clients/AnthropicClient.swift` — BYOK 直连 + `AnthropicKeychain`（keychain 模式）
- `…/Agent/Clients/AgentClientTypes.swift` — Anthropic 请求体 / SSE 解析 / 流事件
- `…/Agent/AgentService.swift` — `selectClient()` 双模优先级（`:52-59`）、`canStream`（`:41-45`）
- `…/Agent/Tools/ToolExecutor+Generate.swift` — 生成工具执行体（args→GenerationInput→submit）
- `…/Agent/Tools/ToolDefinitions.swift` — 工具 arg schema（`:19-24,367-407`）
- `…/Agent/Tools/ToolExecutor+Timeline.swift` — `canGenerate` 信号（`:45`）
- `…/Utilities/KeychainStore.swift` — 跨调用 keychain 原语（service=bundle id）
- `…/Models/MediaManifest.swift` — `GenerationInput` 持久化定义（`:36-63`）

## 附录 B — OpenTake 现状证据

- `…/OpenTake/Cargo.toml` — workspace 含 `opentake-gen`/`opentake-domain`/`opentake-agent`（`members`）；`serde`/`serde_json` 在 `[workspace.dependencies]`
- `…/OpenTake/crates/opentake-gen/{Cargo.toml,src/lib.rs}` — 当前为 Phase 0 空脚手架（`[dependencies]` 为空 / lib 仅 `crate_compiles` 测试）
- `…/OpenTake/crates/opentake-domain/{Cargo.toml,src/lib.rs}` — 零依赖叶子 crate（仅依赖 serde；声明“Zero IO”）
- `…/OpenTake/crates/opentake-agent/src/lib.rs` — 工具层 + MCP(rmcp) + chat 客户端（peer clients）
- `…/OpenTake/AGENTS.md` — Rust 风格铁律（`anyhow`→边界 `Err(String)`；domain 零网络/FS；serde `default`+`Option`；≥80% 覆盖）
