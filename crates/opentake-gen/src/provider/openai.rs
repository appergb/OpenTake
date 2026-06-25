//! OpenAI adapter (`prefix = "openai"`) — synchronous images / TTS. Submit
//! returns a terminal `Succeeded` (or `Failed`) job directly; `poll` replays the
//! cached result by job id. See gen-SPEC §2.2.3.
//!
//! For images the response carries URLs (or base64, normalized to a `data:` URL).
//! For TTS the response is raw audio bytes; without object storage configured we
//! surface a `data:` URL so the result is locally downloadable (gen-SPEC §2.2.4
//! note (a)). A real deployment may instead persist to S3/R2.

use super::{ModelRoute, ProviderAdapter};
use crate::error::{map_http_error, GenError};
use crate::job::GenerationJob;
use crate::params::GenerationParams;
use crate::transport::{HttpRequest, HttpTransport};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

const API_BASE: &str = "https://api.openai.com/v1";

/// OpenAI provider adapter. Synchronous: results are cached for `poll` replay.
pub struct OpenAiAdapter {
    http: Arc<dyn HttpTransport>,
    api_key: String,
    api_base: String,
    cache: Arc<Mutex<HashMap<String, GenerationJob>>>,
}

impl OpenAiAdapter {
    pub fn new(http: Arc<dyn HttpTransport>, api_key: impl Into<String>) -> Self {
        Self {
            http,
            api_key: api_key.into(),
            api_base: API_BASE.to_string(),
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn for_test() -> Self {
        Self::new(Arc::new(crate::transport::MockTransport::new()), "test-key")
    }

    pub fn with_base(mut self, base: impl Into<String>) -> Self {
        self.api_base = base.into();
        self
    }

    fn auth_header(&self) -> (String, String) {
        (
            "Authorization".to_string(),
            format!("Bearer {}", self.api_key),
        )
    }

    /// Generate a synthetic job id and cache its terminal result.
    fn cache_job(&self, job: GenerationJob) -> GenerationJob {
        self.cache
            .lock()
            .unwrap()
            .insert(job.id.clone(), job.clone());
        job
    }

    /// Map image aspect ratio / resolution to an OpenAI `size` string.
    fn image_size(p: &crate::params::ImageParams) -> String {
        if let Some(r) = &p.resolution {
            return r.clone();
        }
        match p.aspect_ratio.as_str() {
            "16:9" | "3:2" => "1536x1024".to_string(),
            "9:16" | "2:3" => "1024x1536".to_string(),
            _ => "1024x1024".to_string(),
        }
    }

    async fn submit_image(
        &self,
        route: &ModelRoute,
        p: &crate::params::ImageParams,
    ) -> Result<GenerationJob, GenError> {
        use serde_json::json;
        let url = format!("{}/images/generations", self.api_base);
        let body = json!({
            "model": route.vendor_model,
            "prompt": p.prompt,
            "size": Self::image_size(p),
            "n": p.num_images,
        });
        let (hk, hv) = self.auth_header();
        let resp = self
            .http
            .send(HttpRequest::post(url).header(hk, hv).json(body))
            .await?;
        if !resp.is_success() {
            return Err(map_http_error(resp.status, &resp.body));
        }
        let v: serde_json::Value = resp.json()?;
        let urls = Self::extract_image_urls(&v);
        let job_id = format!("openai-img-{}", short_id(&v));
        Ok(self.cache_job(GenerationJob::succeeded(job_id, urls)))
    }

    fn extract_image_urls(v: &serde_json::Value) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(arr) = v.get("data").and_then(|d| d.as_array()) {
            for item in arr {
                if let Some(u) = item.get("url").and_then(|x| x.as_str()) {
                    out.push(u.to_string());
                } else if let Some(b64) = item.get("b64_json").and_then(|x| x.as_str()) {
                    out.push(format!("data:image/png;base64,{b64}"));
                }
            }
        }
        out
    }

    async fn submit_audio(
        &self,
        route: &ModelRoute,
        p: &crate::params::AudioParams,
    ) -> Result<GenerationJob, GenError> {
        use serde_json::json;
        let url = format!("{}/audio/speech", self.api_base);
        let body = json!({
            "model": route.vendor_model,
            "input": p.prompt,
            "voice": p.voice.clone().unwrap_or_else(|| "alloy".to_string()),
        });
        let (hk, hv) = self.auth_header();
        let resp = self
            .http
            .send(HttpRequest::post(url).header(hk, hv).json(body))
            .await?;
        if !resp.is_success() {
            return Err(map_http_error(resp.status, &resp.body));
        }
        // Raw audio bytes -> data URL (no object storage configured).
        let data_url =
            super::encode_data_url(&resp.body, resp.header("Content-Type"), "audio/mpeg");
        let job_id = format!("openai-tts-{}", resp.body.len());
        Ok(self.cache_job(GenerationJob::succeeded(job_id, vec![data_url])))
    }
}

/// Derive a short stable-ish id from a response payload.
fn short_id(v: &serde_json::Value) -> String {
    v.get("created")
        .and_then(|x| x.as_i64())
        .map(|c| c.to_string())
        .unwrap_or_else(|| "0".to_string())
}

#[async_trait]
impl ProviderAdapter for OpenAiAdapter {
    fn prefix(&self) -> &'static str {
        "openai"
    }

    async fn submit(
        &self,
        route: &ModelRoute,
        params: &GenerationParams,
    ) -> Result<GenerationJob, GenError> {
        match params {
            GenerationParams::Image(p) => self.submit_image(route, p).await,
            GenerationParams::Audio(p) => self.submit_audio(route, p).await,
            other => Err(GenError::Other(anyhow::anyhow!(
                "openai adapter does not support kind '{}'",
                other.kind_str()
            ))),
        }
    }

    async fn poll(&self, job_id: &str) -> Result<GenerationJob, GenError> {
        // Synchronous vendor: replay the cached terminal job.
        self.cache
            .lock()
            .unwrap()
            .get(job_id)
            .cloned()
            .ok_or_else(|| GenError::Transport(format!("openai: unknown job id {job_id}")))
    }

    async fn upload(&self, _path: &Path, _content_type: &str) -> Result<String, GenError> {
        Err(GenError::Other(anyhow::anyhow!(
            "openai has no public asset hosting; configure object storage for reference uploads"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::JobStatus;
    use crate::params::{AudioParams, ImageParams, UpscaleParams};
    use crate::transport::{HttpResponse, Method, MockTransport};
    use serde_json::json;

    fn adapter(mock: &MockTransport) -> OpenAiAdapter {
        OpenAiAdapter::new(Arc::new(mock.clone()), "sk-test").with_base("https://mockoai/v1")
    }

    #[tokio::test]
    async fn image_submit_is_terminal_succeeded_and_poll_replays() {
        let mock = MockTransport::new();
        mock.on(
            Method::Post,
            "https://mockoai/v1/images/generations",
            200,
            json!({"created": 123, "data": [{"url": "https://out/a.png"}]}),
        );
        let a = adapter(&mock);
        let route = ModelRoute::parse("openai:gpt-image-1").unwrap();
        let params = GenerationParams::Image(ImageParams::new("a cat", "1:1", 1));
        let job = a.submit(&route, &params).await.unwrap();
        assert_eq!(job.status, JobStatus::Succeeded);
        assert_eq!(job.result_urls, Some(vec!["https://out/a.png".into()]));
        // poll replays the cached job
        let replay = a.poll(&job.id).await.unwrap();
        assert_eq!(replay, job);
    }

    #[tokio::test]
    async fn image_size_mapping_from_aspect_ratio() {
        let mock = MockTransport::new();
        mock.on(
            Method::Post,
            "https://mockoai/v1/images/generations",
            200,
            json!({"created": 1, "data": [{"url": "u"}]}),
        );
        let a = adapter(&mock);
        let route = ModelRoute::parse("openai:gpt-image-1").unwrap();
        a.submit(
            &route,
            &GenerationParams::Image(ImageParams::new("x", "9:16", 1)),
        )
        .await
        .unwrap();
        match mock.last_call().unwrap().body {
            crate::transport::Body::Json(v) => assert_eq!(v["size"], "1024x1536"),
            _ => panic!("expected json"),
        }
    }

    #[tokio::test]
    async fn image_b64_response_becomes_data_url() {
        let mock = MockTransport::new();
        mock.on(
            Method::Post,
            "https://mockoai/v1/images/generations",
            200,
            json!({"created": 1, "data": [{"b64_json": "QUJD"}]}),
        );
        let a = adapter(&mock);
        let route = ModelRoute::parse("openai:gpt-image-1").unwrap();
        let job = a
            .submit(
                &route,
                &GenerationParams::Image(ImageParams::new("x", "1:1", 1)),
            )
            .await
            .unwrap();
        assert_eq!(
            job.result_urls,
            Some(vec!["data:image/png;base64,QUJD".into()])
        );
    }

    #[tokio::test]
    async fn tts_submit_returns_data_url_from_bytes() {
        let mock = MockTransport::new();
        // Raw bytes "ABC" -> base64 "QUJD"
        let mut resp = HttpResponse::new(200, b"ABC".to_vec());
        resp.headers
            .push(("Content-Type".into(), "audio/mpeg".into()));
        mock.on_raw(Method::Post, "https://mockoai/v1/audio/speech", resp);
        let a = adapter(&mock);
        let route = ModelRoute::parse("openai:tts-1").unwrap();
        let mut p = AudioParams::new("hello", false);
        p.voice = Some("nova".into());
        let job = a.submit(&route, &GenerationParams::Audio(p)).await.unwrap();
        assert_eq!(job.status, JobStatus::Succeeded);
        assert_eq!(
            job.result_urls,
            Some(vec!["data:audio/mpeg;base64,QUJD".into()])
        );
        match mock.last_call().unwrap().body {
            crate::transport::Body::Json(v) => {
                assert_eq!(v["input"], "hello");
                assert_eq!(v["voice"], "nova");
            }
            _ => panic!("expected json"),
        }
    }

    #[tokio::test]
    async fn unsupported_kind_errors() {
        let a = adapter(&MockTransport::new());
        let route = ModelRoute::parse("openai:x").unwrap();
        let params = GenerationParams::Upscale(UpscaleParams {
            source_url: "u".into(),
            duration_seconds: 1,
        });
        assert!(a.submit(&route, &params).await.is_err());
    }

    #[tokio::test]
    async fn poll_unknown_job_errors() {
        let a = adapter(&MockTransport::new());
        assert!(a.poll("nope").await.is_err());
    }

    #[tokio::test]
    async fn upload_unsupported() {
        let a = adapter(&MockTransport::new());
        assert!(a
            .upload(Path::new("/tmp/x.png"), "image/png")
            .await
            .is_err());
    }
}
