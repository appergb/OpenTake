//! Provider adapters. Each vendor has one adapter that (1) translates the
//! unified `GenerationParams` into a vendor request, (2) submits and returns a
//! normalized job, and (3) polls vendor status, normalizing into `GenerationJob`.
//! Adapter selection is by model-id prefix: a full id is `<prefix>:<vendorModel>`.

pub mod elevenlabs;
pub mod fal;
pub mod openai;
pub mod replicate;

use crate::error::GenError;
use crate::job::GenerationJob;
use crate::params::GenerationParams;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

pub use elevenlabs::ElevenLabsAdapter;
pub use fal::FalAdapter;
pub use openai::OpenAiAdapter;
pub use replicate::ReplicateAdapter;

/// A parsed model id: `<prefix>:<vendor_model>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRoute {
    pub prefix: String,
    pub vendor_model: String,
}

impl ModelRoute {
    /// Parse `"fal:flux-pro"` into its prefix and vendor model. The vendor model
    /// may itself contain colons (e.g. a Replicate `owner/model:version`); only
    /// the first colon is the separator.
    pub fn parse(model_id: &str) -> Result<Self, GenError> {
        match model_id.split_once(':') {
            Some((prefix, vendor)) if !prefix.is_empty() && !vendor.is_empty() => Ok(Self {
                prefix: prefix.to_string(),
                vendor_model: vendor.to_string(),
            }),
            _ => Err(GenError::Other(anyhow::anyhow!(
                "invalid model id '{model_id}': expected '<prefix>:<vendorModel>'"
            ))),
        }
    }
}

/// The provider adapter contract.
#[async_trait]
pub trait ProviderAdapter: Send + Sync {
    /// Short name used as the model-id prefix.
    fn prefix(&self) -> &'static str;

    /// Submit: map unified params to the vendor request, returning a normalized
    /// job (typically queued/running, or terminal for synchronous vendors).
    async fn submit(
        &self,
        route: &ModelRoute,
        params: &GenerationParams,
    ) -> Result<GenerationJob, GenError>;

    /// Poll the vendor once and normalize.
    async fn poll(&self, job_id: &str) -> Result<GenerationJob, GenError>;

    /// Upload a reference file -> public URL.
    async fn upload(&self, path: &Path, content_type: &str) -> Result<String, GenError>;
}

/// Registry mapping prefixes to adapters.
#[derive(Clone, Default)]
pub struct ProviderRegistry {
    adapters: HashMap<String, Arc<dyn ProviderAdapter>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an adapter under its `prefix()`.
    pub fn with(mut self, adapter: Arc<dyn ProviderAdapter>) -> Self {
        self.adapters.insert(adapter.prefix().to_string(), adapter);
        self
    }

    /// Resolve `model_id` to its adapter + parsed route. Errors on unknown prefix.
    pub fn route(
        &self,
        model_id: &str,
    ) -> Result<(Arc<dyn ProviderAdapter>, ModelRoute), GenError> {
        let route = ModelRoute::parse(model_id)?;
        match self.adapters.get(&route.prefix) {
            Some(a) => Ok((a.clone(), route)),
            None => Err(GenError::NotConfigured),
        }
    }

    /// Whether an adapter is registered for the given prefix.
    pub fn has_prefix(&self, prefix: &str) -> bool {
        self.adapters.contains_key(prefix)
    }
}

/// Infer an upload content-type from a path extension. A 1:1 port of upstream
/// `GenerationService.contentType` (`GenerationService.swift:266-287`).
/// `fallback` is one of "image" | "video" | "audio".
pub fn content_type_for(path: &Path, fallback: &str) -> String {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "heic" => "image/heic",
        "gif" => "image/gif",
        "mp4" | "m4v" => "video/mp4",
        "mov" => "video/quicktime",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "m4a" => "audio/mp4",
        _ => match fallback {
            "image" => "image/jpeg",
            "video" => "video/mp4",
            "audio" => "audio/mpeg",
            _ => "application/octet-stream",
        },
    }
    .to_string()
}

/// Standard base64 encoder (used to surface synchronous audio/image bytes as
/// `data:` URLs). Inlined to avoid an extra dependency.
pub(crate) fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((n >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(n & 63) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Encode raw media bytes into a `data:` URL with a best-effort MIME type.
pub(crate) fn encode_data_url(
    bytes: &[u8],
    content_type: Option<&str>,
    default_mime: &str,
) -> String {
    let mime = content_type.unwrap_or(default_mime);
    format!("data:{};base64,{}", mime, base64_encode(bytes))
}

/// Normalize a vendor `output` value into a list of result URLs. Accepts a
/// single string, an array of strings, or an array/object of `{url}` entries.
pub(crate) fn normalize_output_urls(value: &serde_json::Value) -> Vec<String> {
    fn push_from(v: &serde_json::Value, out: &mut Vec<String>) {
        match v {
            serde_json::Value::String(s) => out.push(s.clone()),
            serde_json::Value::Object(map) => {
                if let Some(serde_json::Value::String(u)) = map.get("url") {
                    out.push(u.clone());
                }
            }
            serde_json::Value::Array(arr) => {
                for item in arr {
                    push_from(item, out);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    push_from(value, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn parses_simple_route() {
        let r = ModelRoute::parse("fal:flux-pro").unwrap();
        assert_eq!(r.prefix, "fal");
        assert_eq!(r.vendor_model, "flux-pro");
    }

    #[test]
    fn parses_route_with_colons_in_vendor_model() {
        let r = ModelRoute::parse("replicate:owner/model:abc123").unwrap();
        assert_eq!(r.prefix, "replicate");
        assert_eq!(r.vendor_model, "owner/model:abc123");
    }

    #[test]
    fn rejects_missing_prefix_or_model() {
        assert!(ModelRoute::parse("noprefix").is_err());
        assert!(ModelRoute::parse(":model").is_err());
        assert!(ModelRoute::parse("prefix:").is_err());
    }

    #[test]
    fn registry_routes_to_registered_adapter() {
        let reg = ProviderRegistry::new().with(Arc::new(FalAdapter::for_test()));
        let (adapter, route) = reg.route("fal:flux-pro").unwrap();
        assert_eq!(adapter.prefix(), "fal");
        assert_eq!(route.vendor_model, "flux-pro");
    }

    #[test]
    fn registry_unknown_prefix_is_not_configured() {
        let reg = ProviderRegistry::new().with(Arc::new(FalAdapter::for_test()));
        assert!(matches!(
            reg.route("unknown:model"),
            Err(GenError::NotConfigured)
        ));
        assert!(reg.has_prefix("fal"));
        assert!(!reg.has_prefix("openai"));
    }

    #[test]
    fn content_type_inference_matches_upstream_table() {
        assert_eq!(
            content_type_for(&PathBuf::from("a.JPG"), "image"),
            "image/jpeg"
        );
        assert_eq!(
            content_type_for(&PathBuf::from("a.png"), "image"),
            "image/png"
        );
        assert_eq!(
            content_type_for(&PathBuf::from("a.webp"), "image"),
            "image/webp"
        );
        assert_eq!(
            content_type_for(&PathBuf::from("a.heic"), "image"),
            "image/heic"
        );
        assert_eq!(
            content_type_for(&PathBuf::from("a.gif"), "image"),
            "image/gif"
        );
        assert_eq!(
            content_type_for(&PathBuf::from("a.mp4"), "video"),
            "video/mp4"
        );
        assert_eq!(
            content_type_for(&PathBuf::from("a.m4v"), "video"),
            "video/mp4"
        );
        assert_eq!(
            content_type_for(&PathBuf::from("a.mov"), "video"),
            "video/quicktime"
        );
        assert_eq!(
            content_type_for(&PathBuf::from("a.mp3"), "audio"),
            "audio/mpeg"
        );
        assert_eq!(
            content_type_for(&PathBuf::from("a.wav"), "audio"),
            "audio/wav"
        );
        assert_eq!(
            content_type_for(&PathBuf::from("a.m4a"), "audio"),
            "audio/mp4"
        );
    }

    #[test]
    fn content_type_fallbacks() {
        assert_eq!(
            content_type_for(&PathBuf::from("a.xyz"), "image"),
            "image/jpeg"
        );
        assert_eq!(
            content_type_for(&PathBuf::from("noext"), "video"),
            "video/mp4"
        );
        assert_eq!(
            content_type_for(&PathBuf::from("a.xyz"), "audio"),
            "audio/mpeg"
        );
        assert_eq!(
            content_type_for(&PathBuf::from("a.xyz"), "other"),
            "application/octet-stream"
        );
    }

    #[test]
    fn base64_encoding_correct() {
        assert_eq!(base64_encode(b"ABC"), "QUJD");
        assert_eq!(base64_encode(b"A"), "QQ==");
        assert_eq!(base64_encode(b"AB"), "QUI=");
        assert_eq!(base64_encode(b""), "");
    }

    #[test]
    fn data_url_uses_content_type_or_default() {
        assert_eq!(
            encode_data_url(b"ABC", Some("audio/wav"), "audio/mpeg"),
            "data:audio/wav;base64,QUJD"
        );
        assert_eq!(
            encode_data_url(b"ABC", None, "audio/mpeg"),
            "data:audio/mpeg;base64,QUJD"
        );
    }

    #[test]
    fn normalize_output_handles_shapes() {
        use serde_json::json;
        assert_eq!(normalize_output_urls(&json!("u")), vec!["u"]);
        assert_eq!(normalize_output_urls(&json!(["a", "b"])), vec!["a", "b"]);
        assert_eq!(
            normalize_output_urls(&json!([{"url":"a"},{"url":"b"}])),
            vec!["a", "b"]
        );
        assert_eq!(
            normalize_output_urls(&json!({"url":"single"})),
            vec!["single"]
        );
        assert!(normalize_output_urls(&json!(null)).is_empty());
    }
}
