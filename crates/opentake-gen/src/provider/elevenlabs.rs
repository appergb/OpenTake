//! ElevenLabs adapter (`prefix = "elevenlabs"`) — TTS / music / sfx. Synchronous:
//! the response is audio bytes, surfaced as a `data:` URL; `poll` replays the
//! cached result. Auth uses the `xi-api-key` header. See gen-SPEC §2.2.4.

use super::{ModelRoute, ProviderAdapter};
use crate::error::{map_http_error, GenError};
use crate::job::GenerationJob;
use crate::params::{AudioParams, GenerationParams};
use crate::transport::{HttpRequest, HttpTransport};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

const API_BASE: &str = "https://api.elevenlabs.io/v1";
const DEFAULT_VOICE_ID: &str = "21m00Tcm4TlvDq8ikWAM"; // "Rachel"

/// ElevenLabs provider adapter.
pub struct ElevenLabsAdapter {
    http: Arc<dyn HttpTransport>,
    api_key: String,
    api_base: String,
    cache: Arc<Mutex<HashMap<String, GenerationJob>>>,
}

impl ElevenLabsAdapter {
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
        ("xi-api-key".to_string(), self.api_key.clone())
    }

    fn cache_job(&self, job: GenerationJob) -> GenerationJob {
        self.cache
            .lock()
            .unwrap()
            .insert(job.id.clone(), job.clone());
        job
    }

    /// Resolve a voice id from params (falls back to a default voice).
    fn voice_id(p: &AudioParams) -> String {
        p.voice
            .clone()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| DEFAULT_VOICE_ID.to_string())
    }

    /// Whether this is a music request (vs TTS), inferred from the vendor model.
    fn is_music(vendor_model: &str) -> bool {
        vendor_model.contains("music")
    }

    async fn submit_tts(
        &self,
        route: &ModelRoute,
        p: &AudioParams,
    ) -> Result<GenerationJob, GenError> {
        use serde_json::json;
        let voice = Self::voice_id(p);
        let url = format!("{}/text-to-speech/{}", self.api_base, voice);
        let body = json!({ "text": p.prompt, "model_id": route.vendor_model });
        let (hk, hv) = self.auth_header();
        let resp = self
            .http
            .send(HttpRequest::post(url).header(hk, hv).json(body))
            .await?;
        if !resp.is_success() {
            return Err(map_http_error(resp.status, &resp.body));
        }
        let data_url =
            super::encode_data_url(&resp.body, resp.header("Content-Type"), "audio/mpeg");
        let job_id = format!("eleven-tts-{}", resp.body.len());
        Ok(self.cache_job(GenerationJob::succeeded(job_id, vec![data_url])))
    }

    async fn submit_music(
        &self,
        route: &ModelRoute,
        p: &AudioParams,
    ) -> Result<GenerationJob, GenError> {
        use serde_json::json;
        let url = format!("{}/music", self.api_base);
        let mut body = json!({ "prompt": p.prompt, "model_id": route.vendor_model });
        if let Some(d) = p.duration_seconds {
            body["music_length_ms"] = json!(d as u64 * 1000);
        }
        if p.instrumental {
            body["instrumental"] = json!(true);
        }
        let (hk, hv) = self.auth_header();
        let resp = self
            .http
            .send(HttpRequest::post(url).header(hk, hv).json(body))
            .await?;
        if !resp.is_success() {
            return Err(map_http_error(resp.status, &resp.body));
        }
        let data_url =
            super::encode_data_url(&resp.body, resp.header("Content-Type"), "audio/mpeg");
        let job_id = format!("eleven-music-{}", resp.body.len());
        Ok(self.cache_job(GenerationJob::succeeded(job_id, vec![data_url])))
    }
}

#[async_trait]
impl ProviderAdapter for ElevenLabsAdapter {
    fn prefix(&self) -> &'static str {
        "elevenlabs"
    }

    async fn submit(
        &self,
        route: &ModelRoute,
        params: &GenerationParams,
    ) -> Result<GenerationJob, GenError> {
        match params {
            GenerationParams::Audio(p) => {
                if Self::is_music(&route.vendor_model) {
                    self.submit_music(route, p).await
                } else {
                    self.submit_tts(route, p).await
                }
            }
            other => Err(GenError::Other(anyhow::anyhow!(
                "elevenlabs adapter only supports audio, got '{}'",
                other.kind_str()
            ))),
        }
    }

    async fn poll(&self, job_id: &str) -> Result<GenerationJob, GenError> {
        self.cache
            .lock()
            .unwrap()
            .get(job_id)
            .cloned()
            .ok_or_else(|| GenError::Transport(format!("elevenlabs: unknown job id {job_id}")))
    }

    async fn upload(&self, _path: &Path, _content_type: &str) -> Result<String, GenError> {
        Err(GenError::Other(anyhow::anyhow!(
            "elevenlabs has no public asset hosting; configure object storage for reference uploads"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::JobStatus;
    use crate::transport::{Body, HttpResponse, Method, MockTransport};
    use serde_json::json;

    fn adapter(mock: &MockTransport) -> ElevenLabsAdapter {
        ElevenLabsAdapter::new(Arc::new(mock.clone()), "xi-secret").with_base("https://mockel/v1")
    }

    #[tokio::test]
    async fn tts_uses_voice_path_and_xi_header() {
        let mock = MockTransport::new();
        let mut resp = HttpResponse::new(200, b"ABC".to_vec());
        resp.headers
            .push(("Content-Type".into(), "audio/mpeg".into()));
        mock.on_raw(
            Method::Post,
            "https://mockel/v1/text-to-speech/rachel-id",
            resp,
        );
        let a = adapter(&mock);
        let route = ModelRoute::parse("elevenlabs:eleven-multilingual-v2").unwrap();
        let mut p = AudioParams::new("hello world", false);
        p.voice = Some("rachel-id".into());
        let job = a.submit(&route, &GenerationParams::Audio(p)).await.unwrap();
        assert_eq!(job.status, JobStatus::Succeeded);
        assert_eq!(
            job.result_urls,
            Some(vec!["data:audio/mpeg;base64,QUJD".into()])
        );
        let last = mock.last_call().unwrap();
        assert!(last
            .headers
            .iter()
            .any(|(k, v)| k == "xi-api-key" && v == "xi-secret"));
        match last.body {
            Body::Json(v) => {
                assert_eq!(v["text"], "hello world");
                assert_eq!(v["model_id"], "eleven-multilingual-v2");
            }
            _ => panic!("expected json"),
        }
    }

    #[tokio::test]
    async fn tts_falls_back_to_default_voice() {
        let mock = MockTransport::new();
        let resp = HttpResponse::new(200, b"X".to_vec());
        mock.on_raw(
            Method::Post,
            format!("https://mockel/v1/text-to-speech/{DEFAULT_VOICE_ID}"),
            resp,
        );
        let a = adapter(&mock);
        let route = ModelRoute::parse("elevenlabs:tts").unwrap();
        let p = AudioParams::new("hi", false);
        let job = a.submit(&route, &GenerationParams::Audio(p)).await.unwrap();
        assert_eq!(job.status, JobStatus::Succeeded);
    }

    #[tokio::test]
    async fn music_route_hits_music_endpoint_with_length() {
        let mock = MockTransport::new();
        let resp = HttpResponse::new(200, b"MUS".to_vec());
        mock.on_raw(Method::Post, "https://mockel/v1/music", resp);
        let a = adapter(&mock);
        let route = ModelRoute::parse("elevenlabs:eleven-music").unwrap();
        let mut p = AudioParams::new("epic score", true);
        p.duration_seconds = Some(30);
        a.submit(&route, &GenerationParams::Audio(p)).await.unwrap();
        match mock.last_call().unwrap().body {
            Body::Json(v) => {
                assert_eq!(v["prompt"], "epic score");
                assert_eq!(v["music_length_ms"], 30000);
                assert_eq!(v["instrumental"], true);
            }
            _ => panic!("expected json"),
        }
    }

    #[tokio::test]
    async fn poll_replays_cached_job() {
        let mock = MockTransport::new();
        let resp = HttpResponse::new(200, b"ABC".to_vec());
        mock.on_raw(Method::Post, "https://mockel/v1/text-to-speech/v", resp);
        let a = adapter(&mock);
        let route = ModelRoute::parse("elevenlabs:tts").unwrap();
        let mut p = AudioParams::new("hi", false);
        p.voice = Some("v".into());
        let job = a.submit(&route, &GenerationParams::Audio(p)).await.unwrap();
        assert_eq!(a.poll(&job.id).await.unwrap(), job);
    }

    #[tokio::test]
    async fn error_maps_via_envelope() {
        let mock = MockTransport::new();
        mock.on(
            Method::Post,
            "https://mockel/v1/text-to-speech/v",
            422,
            json!({"error": {"code": "bad", "message": "nope"}}),
        );
        let a = adapter(&mock);
        let route = ModelRoute::parse("elevenlabs:tts").unwrap();
        let mut p = AudioParams::new("hi", false);
        p.voice = Some("v".into());
        match a.submit(&route, &GenerationParams::Audio(p)).await {
            Err(GenError::Api {
                status, message, ..
            }) => {
                assert_eq!(status, 422);
                assert_eq!(message, "nope");
            }
            other => panic!("expected Api error, got {other:?}"),
        }
    }
}
