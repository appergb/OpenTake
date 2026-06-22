//! `GenClient` — the top-level generation client. Two auth modes with an
//! identical call surface (axiom A7): `Bearer` (managed proxy + billing) and
//! `Byok` (local direct-to-vendor via provider adapters + static catalog).
//!
//! - `list_models` — managed: GET /v1/models; BYOK: built-in static catalog.
//! - `submit` — managed: POST /v1/generations; BYOK: route to an adapter.
//! - `get` — single status snapshot.
//! - `watch` — poll until terminal, replicating the upstream `runJob` loop
//!   (`GenerationService.swift:338-361`): only succeeded/failed stop the stream.
//! - `sign_upload` / `upload_reference` — managed: presigned PUT; BYOK: adapter.

use crate::catalog::{Catalog, CatalogEntry, ModelKind};
use crate::error::{map_http_error, GenError};
use crate::job::GenerationJob;
use crate::params::GenerationParams;
use crate::provider::ProviderRegistry;
use crate::transport::{HttpRequest, HttpTransport, Method, ReqwestTransport};
use async_trait::async_trait;
use futures_util::stream::Stream;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

/// Asynchronously provides a Bearer token (managed mode). UI injects this,
/// reusing any OIDC provider (axiom A6).
#[async_trait]
pub trait TokenProvider: Send + Sync {
    async fn bearer_token(&self) -> Result<String, GenError>;
}

/// A static-token provider (tests / simple deployments).
pub struct StaticToken(pub String);

#[async_trait]
impl TokenProvider for StaticToken {
    async fn bearer_token(&self) -> Result<String, GenError> {
        Ok(self.0.clone())
    }
}

/// Authentication / routing mode. Both expose the same call surface.
pub enum AuthMode {
    /// Managed: all calls go through a self-hosted proxy that holds vendor keys
    /// and bills usage.
    Bearer {
        base_url: url::Url,
        token_provider: Arc<dyn TokenProvider>,
    },
    /// BYOK: local direct-to-vendor; catalog is the built-in static one.
    Byok {
        registry: ProviderRegistry,
        catalog: Catalog,
    },
}

/// Ticket returned by `sign_upload` (managed presigned-upload flow).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct UploadTicket {
    #[serde(rename = "uploadUrl")]
    pub upload_url: String,
    #[serde(rename = "publicUrl")]
    pub public_url: String,
}

/// Submit result envelope from the proxy (`{"jobId": "..."}`).
#[derive(Debug, serde::Deserialize)]
struct SubmitResult {
    #[serde(rename = "jobId")]
    job_id: String,
}

struct GenClientInner {
    mode: AuthMode,
    http: Arc<dyn HttpTransport>,
    poll_interval: Duration,
}

/// The generation client. Cheap to clone.
#[derive(Clone)]
pub struct GenClient {
    inner: Arc<GenClientInner>,
}

impl GenClient {
    /// Managed-mode client backed by `reqwest`.
    pub fn managed(base_url: url::Url, token_provider: Arc<dyn TokenProvider>) -> Self {
        Self::with_transport(
            AuthMode::Bearer {
                base_url,
                token_provider,
            },
            Arc::new(ReqwestTransport::new()),
        )
    }

    /// BYOK-mode client. `registry` carries the provider adapters; `catalog` is
    /// the static model catalog (typically `Catalog::builtin()`).
    pub fn byok(registry: ProviderRegistry, catalog: Catalog) -> Self {
        // BYOK adapters carry their own transport; this top-level one is unused
        // for vendor calls. A reqwest transport is a safe default.
        Self::with_transport(
            AuthMode::Byok { registry, catalog },
            Arc::new(ReqwestTransport::new()),
        )
    }

    /// Construct with an explicit transport (tests inject `MockTransport`).
    pub fn with_transport(mode: AuthMode, http: Arc<dyn HttpTransport>) -> Self {
        Self {
            inner: Arc::new(GenClientInner {
                mode,
                http,
                poll_interval: Duration::from_secs(2),
            }),
        }
    }

    /// Override the `watch` poll interval (tests use zero to run instantly).
    pub fn with_poll_interval(mut self, interval: Duration) -> Self {
        Arc::get_mut(&mut self.inner)
            .expect("with_poll_interval must be called before cloning")
            .poll_interval = interval;
        self
    }

    fn proxy_parts(&self) -> Result<(&url::Url, &Arc<dyn TokenProvider>), GenError> {
        match &self.inner.mode {
            AuthMode::Bearer {
                base_url,
                token_provider,
            } => Ok((base_url, token_provider)),
            AuthMode::Byok { .. } => Err(GenError::NotConfigured),
        }
    }

    async fn bearer_header(&self) -> Result<(String, String), GenError> {
        let (_, tp) = self.proxy_parts()?;
        let token = tp.bearer_token().await?;
        Ok(("Authorization".to_string(), format!("Bearer {token}")))
    }

    fn endpoint(base: &url::Url, path: &str) -> Result<String, GenError> {
        Ok(base.join(path)?.to_string())
    }

    /// Model catalog. Managed: GET /v1/models. BYOK: built-in static catalog.
    pub async fn list_models(&self) -> Result<Vec<CatalogEntry>, GenError> {
        match &self.inner.mode {
            AuthMode::Byok { catalog, .. } => Ok(catalog.entries().to_vec()),
            AuthMode::Bearer { base_url, .. } => {
                let url = Self::endpoint(base_url, "v1/models")?;
                let (hk, hv) = self.bearer_header().await?;
                let resp = self
                    .inner
                    .http
                    .send(HttpRequest::get(url).header(hk, hv))
                    .await?;
                if !resp.is_success() {
                    return Err(map_http_error(resp.status, &resp.body));
                }
                resp.json()
            }
        }
    }

    /// Mint a presigned upload ticket (managed only). Replicates
    /// `uploads:generateUploadTicket` via object-storage presign (gen-SPEC §3.4).
    pub async fn sign_upload(&self, content_type: &str) -> Result<UploadTicket, GenError> {
        let (base, _) = self.proxy_parts()?;
        let url = Self::endpoint(base, "v1/uploads/sign")?;
        let (hk, hv) = self.bearer_header().await?;
        let resp = self
            .inner
            .http
            .send(
                HttpRequest::post(url)
                    .header(hk, hv)
                    .json(serde_json::json!({ "contentType": content_type })),
            )
            .await?;
        if !resp.is_success() {
            return Err(map_http_error(resp.status, &resp.body));
        }
        resp.json()
    }

    /// Upload a reference file -> public URL. Managed: sign then PUT bytes, use
    /// the returned `publicUrl` (gen-SPEC §3.4). BYOK: delegate to the adapter
    /// for `model_prefix` (vendors differ in upload support).
    pub async fn upload_reference(
        &self,
        path: &Path,
        content_type: &str,
    ) -> Result<String, GenError> {
        match &self.inner.mode {
            AuthMode::Byok { .. } => Err(GenError::Other(anyhow::anyhow!(
                "BYOK upload_reference requires a provider; use upload_reference_via"
            ))),
            AuthMode::Bearer { .. } => {
                let ticket = self.sign_upload(content_type).await?;
                let data = tokio::fs::read(path)
                    .await
                    .map_err(|e| GenError::Transport(format!("read upload file: {e}")))?;
                let resp = self
                    .inner
                    .http
                    .send(
                        HttpRequest::new(Method::Put, ticket.upload_url)
                            .bytes(content_type.to_string(), data),
                    )
                    .await?;
                if !resp.is_success() {
                    return Err(map_http_error(resp.status, &resp.body));
                }
                Ok(ticket.public_url)
            }
        }
    }

    /// BYOK reference upload via the adapter selected by `model_prefix`.
    pub async fn upload_reference_via(
        &self,
        model_prefix: &str,
        path: &Path,
        content_type: &str,
    ) -> Result<String, GenError> {
        match &self.inner.mode {
            AuthMode::Byok { registry, .. } => {
                let (adapter, _) = registry.route(&format!("{model_prefix}:_"))?;
                adapter.upload(path, content_type).await
            }
            AuthMode::Bearer { .. } => self.upload_reference(path, content_type).await,
        }
    }

    /// Submit a job, returning the job id. Managed: POST /v1/generations. BYOK:
    /// route to an adapter and submit.
    pub async fn submit(
        &self,
        model: &str,
        params: GenerationParams,
        project_id: Option<&str>,
    ) -> Result<String, GenError> {
        match &self.inner.mode {
            AuthMode::Byok { registry, .. } => {
                let (adapter, route) = registry.route(model)?;
                let job = adapter.submit(&route, &params).await?;
                Ok(job.id)
            }
            AuthMode::Bearer { base_url, .. } => {
                let url = Self::endpoint(base_url, "v1/generations")?;
                let (hk, hv) = self.bearer_header().await?;
                let mut body = serde_json::json!({
                    "model": model,
                    "params": params,
                });
                if let Some(pid) = project_id {
                    body["projectId"] = serde_json::json!(pid);
                }
                let resp = self
                    .inner
                    .http
                    .send(HttpRequest::post(url).header(hk, hv).json(body))
                    .await?;
                if !resp.is_success() {
                    return Err(map_http_error(resp.status, &resp.body));
                }
                let r: SubmitResult = resp.json()?;
                Ok(r.job_id)
            }
        }
    }

    /// Single status snapshot. Managed: GET /v1/generations/:id. BYOK: adapter
    /// poll. `model` is required under BYOK to select the adapter.
    pub async fn get(&self, job_id: &str) -> Result<GenerationJob, GenError> {
        match &self.inner.mode {
            AuthMode::Byok { registry, .. } => {
                // BYOK job ids are adapter-internal; the prefix is encoded by the
                // caller convention "<prefix>::<vendorJobId>" when needed. For
                // single-adapter setups we try each adapter is unnecessary, so we
                // require the prefixed form here.
                let (prefix, vendor_job) = split_byok_job_id(job_id)?;
                let (adapter, _) = registry.route(&format!("{prefix}:_"))?;
                adapter.poll(vendor_job).await
            }
            AuthMode::Bearer { base_url, .. } => {
                let url = Self::endpoint(base_url, &format!("v1/generations/{job_id}"))?;
                let (hk, hv) = self.bearer_header().await?;
                let resp = self
                    .inner
                    .http
                    .send(HttpRequest::get(url).header(hk, hv))
                    .await?;
                if !resp.is_success() {
                    return Err(map_http_error(resp.status, &resp.body));
                }
                resp.json()
            }
        }
    }

    /// BYOK convenience: submit and return a prefixed job id usable with
    /// `watch_byok` / `get` (the prefix lets `get` re-select the adapter).
    pub async fn submit_byok(
        &self,
        model: &str,
        params: GenerationParams,
    ) -> Result<String, GenError> {
        let route_prefix = crate::provider::ModelRoute::parse(model)?.prefix;
        let vendor_job = self.submit(model, params, None).await?;
        Ok(format!("{route_prefix}::{vendor_job}"))
    }

    /// Subscribe to a job until it reaches a terminal state, polling at the
    /// configured interval. Yields each observed `GenerationJob` snapshot.
    /// Replicates the upstream subscription loop: queued/running continue,
    /// succeeded/failed terminate.
    pub fn watch(
        &self,
        job_id: &str,
    ) -> impl Stream<Item = Result<GenerationJob, GenError>> + Send {
        let client = self.clone();
        let job_id = job_id.to_string();
        let interval = self.inner.poll_interval;
        futures_util::stream::unfold(
            (client, job_id, interval, false),
            |(client, job_id, interval, done)| async move {
                if done {
                    return None;
                }
                match client.get(&job_id).await {
                    Ok(job) => {
                        let terminal = job.status.is_terminal();
                        if !terminal && !interval.is_zero() {
                            tokio::time::sleep(interval).await;
                        }
                        Some((Ok(job), (client, job_id, interval, terminal)))
                    }
                    Err(e) => Some((Err(e), (client, job_id, interval, true))),
                }
            },
        )
    }
}

/// Split a BYOK prefixed job id `"<prefix>::<vendorJobId>"`.
fn split_byok_job_id(job_id: &str) -> Result<(&str, &str), GenError> {
    job_id
        .split_once("::")
        .filter(|(p, v)| !p.is_empty() && !v.is_empty())
        .ok_or_else(|| {
            GenError::Other(anyhow::anyhow!(
                "BYOK job id must be '<prefix>::<vendorJobId>', got '{job_id}'"
            ))
        })
}

/// Compute a `canGenerate` signal (gen-SPEC §5.3). Managed: a token is
/// obtainable. BYOK: at least one adapter is registered.
pub async fn can_generate(client: &GenClient) -> bool {
    match &client.inner.mode {
        AuthMode::Bearer { token_provider, .. } => token_provider.bearer_token().await.is_ok(),
        AuthMode::Byok { registry, .. } => ["fal", "replicate", "openai", "elevenlabs"]
            .iter()
            .any(|p| registry.has_prefix(p)),
    }
}

/// Filter a catalog list by kind (mirrors the proxy `?type=` filter).
pub fn filter_by_kind(entries: &[CatalogEntry], kind: ModelKind) -> Vec<CatalogEntry> {
    entries.iter().filter(|e| e.kind == kind).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::JobStatus;
    use crate::params::ImageParams;
    use crate::provider::FalAdapter;
    use crate::transport::MockTransport;
    use futures_util::StreamExt;
    use serde_json::json;

    fn byok_client(mock: &MockTransport) -> GenClient {
        let fal =
            FalAdapter::new(Arc::new(mock.clone()), "fal-secret").with_base("https://mockfal");
        let registry = ProviderRegistry::new().with(Arc::new(fal));
        GenClient::with_transport(
            AuthMode::Byok {
                registry,
                catalog: Catalog::builtin(),
            },
            Arc::new(mock.clone()),
        )
        .with_poll_interval(Duration::ZERO)
    }

    fn managed_client(mock: &MockTransport) -> GenClient {
        GenClient::with_transport(
            AuthMode::Bearer {
                base_url: url::Url::parse("https://proxy.test/").unwrap(),
                token_provider: Arc::new(StaticToken("jwt-abc".into())),
            },
            Arc::new(mock.clone()),
        )
        .with_poll_interval(Duration::ZERO)
    }

    #[tokio::test]
    async fn byok_list_models_returns_builtin_catalog() {
        let client = byok_client(&MockTransport::new());
        let models = client.list_models().await.unwrap();
        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.id == "fal:flux-pro"));
    }

    #[tokio::test]
    async fn byok_submit_then_watch_to_succeeded() {
        let mock = MockTransport::new();
        // submit
        mock.on(
            Method::Post,
            "https://mockfal/flux-pro",
            200,
            json!({"request_id": "req-7", "status": "IN_QUEUE"}),
        );
        // poll status: queued -> running -> completed
        mock.on_sequence(
            Method::Get,
            "https://mockfal/flux-pro/requests/req-7/status",
            vec![
                (200, json!({"status": "IN_QUEUE"})),
                (200, json!({"status": "IN_PROGRESS"})),
                (200, json!({"status": "COMPLETED"})),
            ],
        );
        // terminal result fetch
        mock.on(
            Method::Get,
            "https://mockfal/flux-pro/requests/req-7",
            200,
            json!({"images": [{"url": "https://out/final.png"}]}),
        );

        let client = byok_client(&mock);
        let params = GenerationParams::Image(ImageParams::new("a cat", "1:1", 1));
        let job_id = client.submit_byok("fal:flux-pro", params).await.unwrap();
        assert!(job_id.starts_with("fal::"));

        let states: Vec<_> = client.watch(&job_id).collect().await;
        let statuses: Vec<JobStatus> = states.iter().map(|r| r.as_ref().unwrap().status).collect();
        assert_eq!(
            statuses,
            vec![JobStatus::Queued, JobStatus::Running, JobStatus::Succeeded]
        );
        let last = states.last().unwrap().as_ref().unwrap();
        assert_eq!(last.result_urls, Some(vec!["https://out/final.png".into()]));
    }

    #[tokio::test]
    async fn watch_stops_immediately_on_terminal_first_poll() {
        let mock = MockTransport::new();
        mock.on(
            Method::Get,
            "https://mockfal/m/requests/r/status",
            200,
            json!({"status": "COMPLETED"}),
        );
        mock.on(
            Method::Get,
            "https://mockfal/m/requests/r",
            200,
            json!({"video": {"url": "https://out/v.mp4"}}),
        );
        let client = byok_client(&mock);
        let states: Vec<_> = client.watch("fal::m|r").collect().await;
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].as_ref().unwrap().status, JobStatus::Succeeded);
    }

    #[tokio::test]
    async fn watch_stops_on_failed() {
        let mock = MockTransport::new();
        mock.on(
            Method::Get,
            "https://mockfal/m/requests/r/status",
            200,
            json!({"status": "FAILED", "error": "bad"}),
        );
        let client = byok_client(&mock);
        let states: Vec<_> = client.watch("fal::m|r").collect().await;
        assert_eq!(states.len(), 1);
        let job = states[0].as_ref().unwrap();
        assert_eq!(job.status, JobStatus::Failed);
        assert_eq!(job.error_message.as_deref(), Some("bad"));
    }

    #[tokio::test]
    async fn managed_list_models_hits_proxy_with_bearer() {
        let mock = MockTransport::new();
        mock.on(
            Method::Get,
            "https://proxy.test/v1/models",
            200,
            json!([{
                "id": "fal:x", "kind": "image", "displayName": "X",
                "allowedEndpoints": [], "responseShape": "images",
                "uiCapabilities": {"aspectRatios": ["1:1"], "supportsImageReference": false, "maxImages": 1}
            }]),
        );
        let client = managed_client(&mock);
        let models = client.list_models().await.unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "fal:x");
        assert!(mock
            .last_call()
            .unwrap()
            .headers
            .iter()
            .any(|(k, v)| k == "Authorization" && v == "Bearer jwt-abc"));
    }

    #[tokio::test]
    async fn managed_submit_returns_job_id() {
        let mock = MockTransport::new();
        mock.on(
            Method::Post,
            "https://proxy.test/v1/generations",
            200,
            json!({"jobId": "server-job-1"}),
        );
        let client = managed_client(&mock);
        let params = GenerationParams::Image(ImageParams::new("x", "1:1", 1));
        let id = client
            .submit("fal:flux-pro", params, Some("proj-1"))
            .await
            .unwrap();
        assert_eq!(id, "server-job-1");
        match mock.last_call().unwrap().body {
            crate::transport::Body::Json(v) => {
                assert_eq!(v["model"], "fal:flux-pro");
                assert_eq!(v["projectId"], "proj-1");
                assert_eq!(v["params"]["kind"], "image");
            }
            _ => panic!("expected json"),
        }
    }

    #[tokio::test]
    async fn managed_get_then_watch() {
        let mock = MockTransport::new();
        mock.on_sequence(
            Method::Get,
            "https://proxy.test/v1/generations/job-9",
            vec![
                (200, json!({"id": "job-9", "status": "queued"})),
                (200, json!({"id": "job-9", "status": "running"})),
                (
                    200,
                    json!({"id": "job-9", "status": "succeeded", "resultUrls": ["https://out/a.png"]}),
                ),
            ],
        );
        let client = managed_client(&mock);
        let snapshot = client.get("job-9").await.unwrap();
        assert_eq!(snapshot.status, JobStatus::Queued);
        // watch continues from the next poll
        let states: Vec<_> = client.watch("job-9").collect().await;
        // first watch poll returns running (2nd seq), then succeeded (3rd)
        assert_eq!(
            states
                .iter()
                .map(|r| r.as_ref().unwrap().status)
                .collect::<Vec<_>>(),
            vec![JobStatus::Running, JobStatus::Succeeded]
        );
    }

    #[tokio::test]
    async fn managed_sign_upload_and_upload_reference() {
        let mock = MockTransport::new();
        mock.on(
            Method::Post,
            "https://proxy.test/v1/uploads/sign",
            200,
            json!({"uploadUrl": "https://put.test/key", "publicUrl": "https://cdn.test/key"}),
        );
        let ticket = managed_client(&mock)
            .sign_upload("image/png")
            .await
            .unwrap();
        assert_eq!(ticket.public_url, "https://cdn.test/key");
        match mock.last_call().unwrap().body {
            crate::transport::Body::Json(v) => assert_eq!(v["contentType"], "image/png"),
            _ => panic!("expected json"),
        }
    }

    #[tokio::test]
    async fn managed_error_envelope_maps() {
        let mock = MockTransport::new();
        mock.on(
            Method::Get,
            "https://proxy.test/v1/models",
            401,
            json!({"error": {"code": "unauthenticated", "message": "no"}}),
        );
        let client = managed_client(&mock);
        assert!(matches!(
            client.list_models().await,
            Err(GenError::Unauthenticated)
        ));
    }

    #[tokio::test]
    async fn byok_sign_upload_is_not_configured() {
        let client = byok_client(&MockTransport::new());
        assert!(matches!(
            client.sign_upload("image/png").await,
            Err(GenError::NotConfigured)
        ));
    }

    #[tokio::test]
    async fn can_generate_byok_true_with_adapter() {
        let client = byok_client(&MockTransport::new());
        assert!(can_generate(&client).await);
    }

    #[tokio::test]
    async fn can_generate_managed_true_with_token() {
        let client = managed_client(&MockTransport::new());
        assert!(can_generate(&client).await);
    }

    #[test]
    fn filter_by_kind_works() {
        let cat = Catalog::builtin();
        let imgs = filter_by_kind(cat.entries(), ModelKind::Image);
        assert!(!imgs.is_empty());
        assert!(imgs.iter().all(|e| e.kind == ModelKind::Image));
    }

    #[test]
    fn split_byok_job_id_validation() {
        assert_eq!(split_byok_job_id("fal::m|r").unwrap(), ("fal", "m|r"));
        assert!(split_byok_job_id("noseparator").is_err());
        assert!(split_byok_job_id("::x").is_err());
    }
}
