//! Replicate adapter (`prefix = "replicate"`) — predictions API. Submit POSTs
//! `{version, input}` and returns an `id`; polling reads `/predictions/{id}`.
//! Status mapping: starting -> Queued, processing -> Running, succeeded ->
//! Succeeded, failed/canceled -> Failed. See gen-SPEC §2.2.2.

use super::{normalize_output_urls, ModelRoute, ProviderAdapter};
use crate::error::{map_http_error, GenError};
use crate::job::{GenerationJob, JobStatus};
use crate::params::GenerationParams;
use crate::transport::{HttpRequest, HttpTransport};
use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;

const API_BASE: &str = "https://api.replicate.com/v1";

/// Replicate provider adapter.
pub struct ReplicateAdapter {
    http: Arc<dyn HttpTransport>,
    api_token: String,
    api_base: String,
}

impl ReplicateAdapter {
    pub fn new(http: Arc<dyn HttpTransport>, api_token: impl Into<String>) -> Self {
        Self {
            http,
            api_token: api_token.into(),
            api_base: API_BASE.to_string(),
        }
    }

    pub fn for_test() -> Self {
        Self::new(Arc::new(crate::transport::MockTransport::new()), "test-token")
    }

    pub fn with_base(mut self, base: impl Into<String>) -> Self {
        self.api_base = base.into();
        self
    }

    fn auth_header(&self) -> (String, String) {
        (
            "Authorization".to_string(),
            format!("Bearer {}", self.api_token),
        )
    }

    /// Map unified params into the Replicate `input` object (flattened fields).
    fn map_input(params: &GenerationParams) -> serde_json::Value {
        use serde_json::json;
        match params {
            GenerationParams::Image(p) => {
                let mut input = json!({ "prompt": p.prompt, "aspect_ratio": p.aspect_ratio });
                if let Some(r) = &p.resolution {
                    input["resolution"] = json!(r);
                }
                if let Some(q) = &p.quality {
                    input["quality"] = json!(q);
                }
                if let Some(first) = p.image_urls.first() {
                    input["image"] = json!(first);
                }
                input["num_outputs"] = json!(p.num_images);
                input
            }
            GenerationParams::Video(p) => {
                let mut input = json!({
                    "prompt": p.prompt,
                    "duration": p.duration,
                    "aspect_ratio": p.aspect_ratio,
                });
                if let Some(r) = &p.resolution {
                    input["resolution"] = json!(r);
                }
                if let Some(u) = &p.start_frame_url {
                    input["image"] = json!(u);
                }
                if let Some(u) = &p.end_frame_url {
                    input["end_image"] = json!(u);
                }
                input
            }
            GenerationParams::Audio(p) => {
                let mut input = json!({ "prompt": p.prompt });
                if let Some(d) = p.duration_seconds {
                    input["duration"] = json!(d);
                }
                input
            }
            GenerationParams::Upscale(p) => json!({ "video": p.source_url, "image": p.source_url }),
        }
    }

    fn map_status(s: &str) -> JobStatus {
        match s {
            "starting" => JobStatus::Queued,
            "processing" => JobStatus::Running,
            "succeeded" => JobStatus::Succeeded,
            _ => JobStatus::Failed, // failed / canceled / unknown
        }
    }

    fn normalize(&self, v: &serde_json::Value) -> GenerationJob {
        let id = v.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let status = v
            .get("status")
            .and_then(|x| x.as_str())
            .map(Self::map_status)
            .unwrap_or(JobStatus::Failed);
        match status {
            JobStatus::Succeeded => {
                let urls = v
                    .get("output")
                    .map(normalize_output_urls)
                    .unwrap_or_default();
                GenerationJob::succeeded(id, urls)
            }
            JobStatus::Failed => {
                let msg = v
                    .get("error")
                    .and_then(|x| x.as_str())
                    .unwrap_or("Generation failed")
                    .to_string();
                GenerationJob::failed(id, msg)
            }
            other => GenerationJob::pending(id, other),
        }
    }
}

#[async_trait]
impl ProviderAdapter for ReplicateAdapter {
    fn prefix(&self) -> &'static str {
        "replicate"
    }

    async fn submit(
        &self,
        route: &ModelRoute,
        params: &GenerationParams,
    ) -> Result<GenerationJob, GenError> {
        use serde_json::json;
        let url = format!("{}/predictions", self.api_base);
        let body = json!({ "version": route.vendor_model, "input": Self::map_input(params) });
        let (hk, hv) = self.auth_header();
        let resp = self
            .http
            .send(HttpRequest::post(url).header(hk, hv).json(body))
            .await?;
        if !resp.is_success() {
            return Err(map_http_error(resp.status, &resp.body));
        }
        let v: serde_json::Value = resp.json()?;
        if v.get("id").and_then(|x| x.as_str()).is_none() {
            return Err(GenError::Transport("replicate: missing prediction id".into()));
        }
        Ok(self.normalize(&v))
    }

    async fn poll(&self, job_id: &str) -> Result<GenerationJob, GenError> {
        let url = format!("{}/predictions/{}", self.api_base, job_id);
        let (hk, hv) = self.auth_header();
        let resp = self.http.send(HttpRequest::get(url).header(hk, hv)).await?;
        if !resp.is_success() {
            return Err(map_http_error(resp.status, &resp.body));
        }
        let v: serde_json::Value = resp.json()?;
        Ok(self.normalize(&v))
    }

    async fn upload(&self, path: &Path, content_type: &str) -> Result<String, GenError> {
        // Replicate files API: POST /files -> urls.get
        let data = tokio::fs::read(path)
            .await
            .map_err(|e| GenError::Transport(format!("read upload file: {e}")))?;
        let url = format!("{}/files", self.api_base);
        let (hk, hv) = self.auth_header();
        let resp = self
            .http
            .send(
                HttpRequest::post(url)
                    .header(hk, hv)
                    .bytes(content_type.to_string(), data),
            )
            .await?;
        if !resp.is_success() {
            return Err(map_http_error(resp.status, &resp.body));
        }
        let v: serde_json::Value = resp.json()?;
        v.get("urls")
            .and_then(|u| u.get("get"))
            .and_then(|x| x.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| GenError::Transport("replicate: upload missing urls.get".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::ImageParams;
    use crate::transport::{Body, Method, MockTransport};
    use serde_json::json;

    fn adapter(mock: &MockTransport) -> ReplicateAdapter {
        ReplicateAdapter::new(Arc::new(mock.clone()), "r8-token").with_base("https://mockrep/v1")
    }

    #[tokio::test]
    async fn submit_sends_version_and_input() {
        let mock = MockTransport::new();
        mock.on(
            Method::Post,
            "https://mockrep/v1/predictions",
            201,
            json!({"id": "pred-1", "status": "starting"}),
        );
        let a = adapter(&mock);
        let route = ModelRoute::parse("replicate:owner/model:v123").unwrap();
        let params = GenerationParams::Image(ImageParams {
            prompt: "x".into(),
            aspect_ratio: "1:1".into(),
            resolution: Some("1024x1024".into()),
            quality: Some("high".into()),
            image_urls: vec!["https://x/ref.png".into()],
            num_images: 2,
        });
        let job = a.submit(&route, &params).await.unwrap();
        assert_eq!(job.id, "pred-1");
        assert_eq!(job.status, JobStatus::Queued);
        match mock.last_call().unwrap().body {
            Body::Json(v) => {
                assert_eq!(v["version"], "owner/model:v123");
                assert_eq!(v["input"]["prompt"], "x");
                assert_eq!(v["input"]["image"], "https://x/ref.png");
                assert_eq!(v["input"]["num_outputs"], 2);
                assert_eq!(v["input"]["quality"], "high");
            }
            _ => panic!("expected json"),
        }
        // Bearer auth.
        assert!(mock
            .last_call()
            .unwrap()
            .headers
            .iter()
            .any(|(k, v)| k == "Authorization" && v == "Bearer r8-token"));
    }

    #[tokio::test]
    async fn poll_normalizes_status_transitions() {
        let mock = MockTransport::new();
        mock.on_sequence(
            Method::Get,
            "https://mockrep/v1/predictions/pred-1",
            vec![
                (200, json!({"id": "pred-1", "status": "starting"})),
                (200, json!({"id": "pred-1", "status": "processing"})),
                (
                    200,
                    json!({"id": "pred-1", "status": "succeeded", "output": "https://out/x.png"}),
                ),
            ],
        );
        let a = adapter(&mock);
        assert_eq!(a.poll("pred-1").await.unwrap().status, JobStatus::Queued);
        assert_eq!(a.poll("pred-1").await.unwrap().status, JobStatus::Running);
        let done = a.poll("pred-1").await.unwrap();
        assert_eq!(done.status, JobStatus::Succeeded);
        // single string output normalized to a one-element array
        assert_eq!(done.result_urls, Some(vec!["https://out/x.png".into()]));
    }

    #[tokio::test]
    async fn poll_array_output_normalized() {
        let mock = MockTransport::new();
        mock.on(
            Method::Get,
            "https://mockrep/v1/predictions/p",
            200,
            json!({"id": "p", "status": "succeeded", "output": ["a", "b"]}),
        );
        let a = adapter(&mock);
        let done = a.poll("p").await.unwrap();
        assert_eq!(done.result_urls, Some(vec!["a".into(), "b".into()]));
    }

    #[tokio::test]
    async fn poll_canceled_maps_to_failed() {
        let mock = MockTransport::new();
        mock.on(
            Method::Get,
            "https://mockrep/v1/predictions/p",
            200,
            json!({"id": "p", "status": "canceled"}),
        );
        let a = adapter(&mock);
        assert_eq!(a.poll("p").await.unwrap().status, JobStatus::Failed);
    }

    #[tokio::test]
    async fn poll_failed_with_error_message() {
        let mock = MockTransport::new();
        mock.on(
            Method::Get,
            "https://mockrep/v1/predictions/p",
            200,
            json!({"id": "p", "status": "failed", "error": "OOM"}),
        );
        let a = adapter(&mock);
        let done = a.poll("p").await.unwrap();
        assert_eq!(done.error_message.as_deref(), Some("OOM"));
    }

    #[tokio::test]
    async fn submit_402_maps_to_insufficient_credits() {
        let mock = MockTransport::new();
        mock.on(
            Method::Post,
            "https://mockrep/v1/predictions",
            402,
            json!({"error": {"code": "insufficient_credits", "message": "pay up"}}),
        );
        let a = adapter(&mock);
        let route = ModelRoute::parse("replicate:m:v").unwrap();
        let params = GenerationParams::Image(ImageParams::new("x", "1:1", 1));
        match a.submit(&route, &params).await {
            Err(GenError::InsufficientCredits(m)) => assert_eq!(m, "pay up"),
            other => panic!("expected InsufficientCredits, got {other:?}"),
        }
    }
}
