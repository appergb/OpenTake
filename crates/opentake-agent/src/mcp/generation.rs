//! Provider-neutral generation lifecycle coordination.
//!
//! The Agent dispatcher creates durable placeholders synchronously, while the
//! desktop bridge submits and watches the paid provider job off-thread. This
//! module owns the deterministic terminal pairing contract so every returned
//! result is applied to at most one placeholder and every placeholder reaches
//! one persisted terminal state.

use std::path::PathBuf;

use serde::Serialize;

use crate::tools::args::{
    GenerateAudioArgs, GenerateImageArgs, GenerateVideoArgs, UpscaleMediaArgs,
};

/// Typed paid-generation request passed from the tool dispatcher to the
/// desktop runtime. The bridge owns model resolution, reference validation,
/// durable placeholder creation, provider submission, and background watch.
#[derive(Debug, Clone, PartialEq)]
pub enum GenerationRequest {
    Video(GenerateVideoArgs),
    Image(GenerateImageArgs),
    Audio(GenerateAudioArgs),
    Upscale(UpscaleMediaArgs),
}

impl GenerationRequest {
    pub fn cost_authorized(&self) -> bool {
        match self {
            Self::Video(args) => args.cost_authorized == Some(true),
            Self::Image(args) => args.cost_authorized == Some(true),
            Self::Audio(args) => args.cost_authorized == Some(true),
            Self::Upscale(args) => args.cost_authorized == Some(true),
        }
    }
}

/// Immediate response from an accepted asynchronous generation submission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationSubmission {
    pub job_id: String,
    pub placeholder_asset_ids: Vec<String>,
    pub status: String,
}

/// Host boundary for production generation. Implementations must return only
/// after the placeholder/job record is durably committed, and must continue
/// provider work off the synchronous MCP dispatch thread.
pub trait GenerationBridge: Send + Sync {
    /// True only when managed authorization or at least one compatible BYOK
    /// credential is currently usable.
    fn can_generate(&self) -> bool;

    fn submit(
        &self,
        request: GenerationRequest,
        cancel: &opentake_media::MediaCancelToken,
    ) -> Result<GenerationSubmission, String>;
}

/// A provider result downloaded into a private staging location. The store
/// validates and commits it into the project before reporting success.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadedGenerationArtifact {
    pub path: PathBuf,
    pub media_type: String,
    pub byte_size: u64,
}

/// Downloads one provider result without exposing credentials or signed URL
/// details to the persistence layer's public error contract.
pub trait GenerationArtifactDownloader {
    fn download(&self, asset_id: &str, url: &str) -> Result<DownloadedGenerationArtifact, String>;
}

/// Durable state boundary implemented by the desktop project runtime.
pub trait GenerationFinalizationStore {
    /// Atomically claim a terminal-finalization lease. `false` means the job is
    /// already complete or another callback currently owns the lease.
    fn claim_terminal(&self, job_id: &str) -> Result<bool, String>;

    /// Release a failed terminal-finalization lease so restart recovery or a
    /// duplicate provider callback can retry. Output operations below must be
    /// idempotent because a prior attempt may already have committed a prefix.
    fn release_terminal(&self, job_id: &str) -> Result<(), String>;

    /// Commit one staged artifact to the matching placeholder identity.
    fn finalize_output(
        &self,
        asset_id: &str,
        artifact: DownloadedGenerationArtifact,
    ) -> Result<(), String>;

    /// Persist one fixed, non-sensitive terminal failure code.
    fn fail_output(&self, asset_id: &str, code: &str) -> Result<(), String>;

    /// Persist the aggregate job terminal state after every placeholder has a
    /// terminal record.
    fn complete_job(&self, job_id: &str, succeeded: usize, failed: usize) -> Result<(), String>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenerationFinalizationSummary {
    pub claimed: bool,
    pub succeeded: usize,
    pub failed: usize,
    pub ignored_result_urls: usize,
}

/// Pair provider URLs to placeholders in order and terminalize every
/// placeholder exactly once. Missing, malformed, download-failed, and
/// commit-failed results become fixed failure codes; extra URLs are ignored.
pub fn finalize_terminal_outputs(
    store: &dyn GenerationFinalizationStore,
    downloader: &dyn GenerationArtifactDownloader,
    job_id: &str,
    placeholder_ids: &[String],
    result_urls: &[String],
) -> Result<GenerationFinalizationSummary, String> {
    if !store.claim_terminal(job_id)? {
        return Ok(GenerationFinalizationSummary {
            claimed: false,
            succeeded: 0,
            failed: 0,
            ignored_result_urls: 0,
        });
    }

    let attempt = (|| {
        let mut succeeded = 0;
        let mut failed = 0;
        for (index, asset_id) in placeholder_ids.iter().enumerate() {
            let Some(url) = result_urls.get(index) else {
                store.fail_output(asset_id, "GENERATION_RESULT_MISSING")?;
                failed += 1;
                continue;
            };
            if !is_accepted_result_url(url) {
                store.fail_output(asset_id, "GENERATION_RESULT_URL_INVALID")?;
                failed += 1;
                continue;
            }
            let artifact = match downloader.download(asset_id, url) {
                Ok(artifact) => artifact,
                Err(error) if error == "GENERATION_CANCELLED" => return Err(error),
                Err(_) => {
                    store.fail_output(asset_id, "GENERATION_DOWNLOAD_FAILED")?;
                    failed += 1;
                    continue;
                }
            };
            if store.finalize_output(asset_id, artifact).is_err() {
                store.fail_output(asset_id, "GENERATION_FINALIZE_FAILED")?;
                failed += 1;
                continue;
            }
            succeeded += 1;
        }
        store.complete_job(job_id, succeeded, failed)?;
        Ok::<_, String>((succeeded, failed))
    })();

    let (succeeded, failed) = match attempt {
        Ok(summary) => summary,
        Err(error) => {
            if let Err(release_error) = store.release_terminal(job_id) {
                return Err(format!(
                    "generation finalization failed and lease release failed: {release_error}"
                ));
            }
            return Err(error);
        }
    };

    Ok(GenerationFinalizationSummary {
        claimed: true,
        succeeded,
        failed,
        ignored_result_urls: result_urls.len().saturating_sub(placeholder_ids.len()),
    })
}

fn is_accepted_result_url(raw: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(raw) else {
        return false;
    };
    match url.scheme() {
        "https" => {
            url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
                && url.port().is_none_or(|port| port == 443)
        }
        "data" => {
            let value = url.as_str();
            value.starts_with("data:image/")
                || value.starts_with("data:audio/")
                || value.starts_with("data:video/")
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::is_accepted_result_url;

    #[test]
    fn provider_result_urls_are_https_or_bounded_media_data_urls() {
        assert!(is_accepted_result_url("https://cdn.test/result.png"));
        assert!(is_accepted_result_url("data:image/png;base64,AAAA"));
        assert!(is_accepted_result_url("data:audio/mpeg;base64,AAAA"));
        assert!(is_accepted_result_url("data:video/mp4;base64,AAAA"));
        assert!(!is_accepted_result_url("http://cdn.test/result.png"));
        assert!(!is_accepted_result_url(
            "https://user:secret@cdn.test/result.png"
        ));
        assert!(!is_accepted_result_url("file:///tmp/result.png"));
        assert!(!is_accepted_result_url("not-a-url"));
    }
}
