//! Unified job abstraction (axiom A4): `queued -> running -> succeeded/failed`
//! plus `resultUrls`, masking each vendor's async differences. A 1:1 port of
//! upstream `BackendGenerationStatus` and `BackendGenerationJob`
//! (`GenerationBackend.swift:112-123`).

use serde::Deserialize;

/// Job lifecycle state. Port of `BackendGenerationStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
}

impl JobStatus {
    /// `succeeded` and `failed` are terminal; `queued`/`running` continue.
    /// Mirrors the upstream `runJob` loop (`GenerationService.swift:338-361`).
    pub fn is_terminal(self) -> bool {
        matches!(self, JobStatus::Succeeded | JobStatus::Failed)
    }
}

/// A normalized generation job. Port of `BackendGenerationJob`. Upstream uses
/// the Convex document id field `_id`; the OpenTake proxy uses `id`, so both are
/// accepted. All optional fields tolerate absence to read older payloads.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct GenerationJob {
    #[serde(rename = "id", alias = "_id")]
    pub id: String,
    pub status: JobStatus,
    #[serde(rename = "resultUrls", default)]
    pub result_urls: Option<Vec<String>>,
    #[serde(rename = "errorMessage", default)]
    pub error_message: Option<String>,
    /// Managed-mode billing; always `None` under BYOK.
    #[serde(rename = "costCredits", default)]
    pub cost_credits: Option<i64>,
    /// Epoch milliseconds (upstream `Double`).
    #[serde(rename = "completedAt", default)]
    pub completed_at: Option<f64>,
}

impl GenerationJob {
    /// Construct a terminal `succeeded` job (used by synchronous adapters).
    pub fn succeeded(id: impl Into<String>, result_urls: Vec<String>) -> Self {
        Self {
            id: id.into(),
            status: JobStatus::Succeeded,
            result_urls: Some(result_urls),
            error_message: None,
            cost_credits: None,
            completed_at: None,
        }
    }

    /// Construct a terminal `failed` job.
    pub fn failed(id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            status: JobStatus::Failed,
            result_urls: None,
            error_message: Some(message.into()),
            cost_credits: None,
            completed_at: None,
        }
    }

    /// Construct a non-terminal job in the given pending state.
    pub fn pending(id: impl Into<String>, status: JobStatus) -> Self {
        Self {
            id: id.into(),
            status,
            result_urls: None,
            error_message: None,
            cost_credits: None,
            completed_at: None,
        }
    }

    /// Terminal-success validation, replicating `finalizeSuccess`
    /// (`GenerationService.swift:364-379`): a `succeeded` job with no result
    /// URLs is treated as a failure ("No URL in response").
    pub fn first_result_url(&self) -> Result<&str, &'static str> {
        match self.status {
            JobStatus::Succeeded => match self.result_urls.as_deref() {
                Some([first, ..]) => Ok(first.as_str()),
                _ => Err("No URL in response"),
            },
            _ => Err("job is not in a succeeded state"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_terminal_only_for_succeeded_failed() {
        assert!(!JobStatus::Queued.is_terminal());
        assert!(!JobStatus::Running.is_terminal());
        assert!(JobStatus::Succeeded.is_terminal());
        assert!(JobStatus::Failed.is_terminal());
    }

    #[test]
    fn deserializes_proxy_shape_with_id() {
        let json = r#"{"id":"j1","status":"running"}"#;
        let job: GenerationJob = serde_json::from_str(json).unwrap();
        assert_eq!(job.id, "j1");
        assert_eq!(job.status, JobStatus::Running);
        assert_eq!(job.result_urls, None);
        assert_eq!(job.cost_credits, None);
    }

    #[test]
    fn deserializes_upstream_shape_with_underscore_id() {
        let json = r#"{"_id":"doc123","status":"succeeded","resultUrls":["https://x/a.png"],"costCredits":42,"completedAt":1700.0}"#;
        let job: GenerationJob = serde_json::from_str(json).unwrap();
        assert_eq!(job.id, "doc123");
        assert_eq!(job.status, JobStatus::Succeeded);
        assert_eq!(job.result_urls, Some(vec!["https://x/a.png".to_string()]));
        assert_eq!(job.cost_credits, Some(42));
        assert_eq!(job.completed_at, Some(1700.0));
    }

    #[test]
    fn deserializes_failed_with_message() {
        let json = r#"{"id":"j2","status":"failed","errorMessage":"boom"}"#;
        let job: GenerationJob = serde_json::from_str(json).unwrap();
        assert_eq!(job.status, JobStatus::Failed);
        assert_eq!(job.error_message.as_deref(), Some("boom"));
    }

    #[test]
    fn first_result_url_ok_when_present() {
        let job = GenerationJob::succeeded("j", vec!["u1".into(), "u2".into()]);
        assert_eq!(job.first_result_url().unwrap(), "u1");
    }

    #[test]
    fn first_result_url_err_when_succeeded_but_empty() {
        let job = GenerationJob {
            id: "j".into(),
            status: JobStatus::Succeeded,
            result_urls: Some(vec![]),
            error_message: None,
            cost_credits: None,
            completed_at: None,
        };
        assert_eq!(job.first_result_url(), Err("No URL in response"));
    }

    #[test]
    fn first_result_url_err_when_not_succeeded() {
        let job = GenerationJob::pending("j", JobStatus::Running);
        assert!(job.first_result_url().is_err());
    }
}
