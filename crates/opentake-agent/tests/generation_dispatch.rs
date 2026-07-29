use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use opentake_agent::mcp::core_handle::AppCoreHandle;
use opentake_agent::mcp::dispatch::Dispatcher;
use opentake_agent::mcp::generation::{
    finalize_terminal_outputs, DownloadedGenerationArtifact, GenerationArtifactDownloader,
    GenerationBridge, GenerationFinalizationStore, GenerationRequest, GenerationSubmission,
};
use opentake_agent::plugin::registry::PluginRegistry;
use opentake_core::AppCore;
use serde_json::json;

#[derive(Default)]
struct RecordingStore {
    claimed: Mutex<BTreeSet<String>>,
    completed: Mutex<BTreeSet<String>>,
    finalized: Mutex<BTreeMap<String, PathBuf>>,
    failed: Mutex<BTreeMap<String, String>>,
    completions: Mutex<Vec<(String, usize, usize)>>,
    output_write_failures_remaining: Mutex<usize>,
}

impl GenerationFinalizationStore for RecordingStore {
    fn claim_terminal(&self, job_id: &str) -> Result<bool, String> {
        if self.completed.lock().unwrap().contains(job_id) {
            return Ok(false);
        }
        Ok(self.claimed.lock().unwrap().insert(job_id.to_string()))
    }

    fn release_terminal(&self, job_id: &str) -> Result<(), String> {
        self.claimed.lock().unwrap().remove(job_id);
        Ok(())
    }

    fn finalize_output(
        &self,
        asset_id: &str,
        artifact: DownloadedGenerationArtifact,
    ) -> Result<(), String> {
        let mut failures = self.output_write_failures_remaining.lock().unwrap();
        if *failures > 0 {
            *failures -= 1;
            return Err("transient manifest write failure".to_string());
        }
        self.finalized
            .lock()
            .unwrap()
            .insert(asset_id.to_string(), artifact.path);
        Ok(())
    }

    fn fail_output(&self, asset_id: &str, code: &str) -> Result<(), String> {
        let mut failures = self.output_write_failures_remaining.lock().unwrap();
        if *failures > 0 {
            *failures -= 1;
            return Err("transient manifest write failure".to_string());
        }
        self.failed
            .lock()
            .unwrap()
            .insert(asset_id.to_string(), code.to_string());
        Ok(())
    }

    fn complete_job(&self, job_id: &str, succeeded: usize, failed: usize) -> Result<(), String> {
        if !self.completed.lock().unwrap().insert(job_id.to_string()) {
            return Ok(());
        }
        self.completions
            .lock()
            .unwrap()
            .push((job_id.to_string(), succeeded, failed));
        Ok(())
    }
}

struct FixtureDownloader;

impl GenerationArtifactDownloader for FixtureDownloader {
    fn download(&self, asset_id: &str, url: &str) -> Result<DownloadedGenerationArtifact, String> {
        if url.contains("download-fails") {
            return Err("provider download failed with private detail".to_string());
        }
        Ok(DownloadedGenerationArtifact {
            path: PathBuf::from(format!("/fixture/{asset_id}.bin")),
            media_type: "application/octet-stream".to_string(),
            byte_size: 7,
        })
    }
}

struct RecordingGenerationBridge {
    available: AtomicBool,
    submissions: AtomicUsize,
}

impl RecordingGenerationBridge {
    fn new(available: bool) -> Self {
        Self {
            available: AtomicBool::new(available),
            submissions: AtomicUsize::new(0),
        }
    }
}

impl GenerationBridge for RecordingGenerationBridge {
    fn can_generate(&self) -> bool {
        self.available.load(Ordering::Acquire)
    }

    fn submit(
        &self,
        _request: GenerationRequest,
        _cancel: &opentake_media::MediaCancelToken,
    ) -> Result<GenerationSubmission, String> {
        self.submissions.fetch_add(1, Ordering::AcqRel);
        Ok(GenerationSubmission {
            job_id: "job-dispatch".to_string(),
            placeholder_asset_ids: vec!["asset-placeholder".to_string()],
            status: "queued".to_string(),
        })
    }
}

fn dispatcher_with_generation_bridge(bridge: Arc<dyn GenerationBridge>) -> Dispatcher {
    Dispatcher::with_bridges(
        Arc::new(AppCoreHandle::new(AppCore::new())),
        Arc::new(RwLock::new(PluginRegistry::new())),
        None,
        Some(bridge),
    )
}

#[test]
fn placeholder_persist_finalize_all_results_and_failures() {
    let store = RecordingStore::default();
    let summary = finalize_terminal_outputs(
        &store,
        &FixtureDownloader,
        "job-1",
        &[
            "asset-a".into(),
            "asset-b".into(),
            "asset-c".into(),
            "asset-d".into(),
        ],
        &[
            "https://results.test/a.png".into(),
            "https://results.test/download-fails.png".into(),
            "not-a-result-url".into(),
            "https://results.test/extra.png".into(),
            "https://results.test/ignored-extra.png".into(),
        ],
    )
    .expect("terminal outputs are recorded");

    assert!(summary.claimed);
    assert_eq!(summary.succeeded, 2);
    assert_eq!(summary.failed, 2);
    assert_eq!(summary.ignored_result_urls, 1);
    assert_eq!(
        store
            .finalized
            .lock()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["asset-a", "asset-d"]
    );
    assert_eq!(
        store.failed.lock().unwrap().clone(),
        BTreeMap::from([
            (
                "asset-b".to_string(),
                "GENERATION_DOWNLOAD_FAILED".to_string()
            ),
            (
                "asset-c".to_string(),
                "GENERATION_RESULT_URL_INVALID".to_string()
            ),
        ])
    );
    assert_eq!(
        store.completions.lock().unwrap().as_slice(),
        &[("job-1".to_string(), 2, 2)]
    );
}

#[test]
fn placeholder_persists_and_every_terminal_result_finalizes_once() {
    let store = RecordingStore::default();
    let placeholders = vec!["asset-a".to_string(), "asset-b".to_string()];
    let urls = vec!["https://results.test/a.png".to_string()];

    let first =
        finalize_terminal_outputs(&store, &FixtureDownloader, "job-once", &placeholders, &urls)
            .expect("first terminal callback succeeds");
    let duplicate =
        finalize_terminal_outputs(&store, &FixtureDownloader, "job-once", &placeholders, &urls)
            .expect("duplicate terminal callback is idempotent");

    assert!(first.claimed);
    assert_eq!((first.succeeded, first.failed), (1, 1));
    assert!(!duplicate.claimed);
    assert_eq!((duplicate.succeeded, duplicate.failed), (0, 0));
    assert_eq!(store.finalized.lock().unwrap().len(), 1);
    assert_eq!(
        store
            .failed
            .lock()
            .unwrap()
            .get("asset-b")
            .map(String::as_str),
        Some("GENERATION_RESULT_MISSING")
    );
    assert_eq!(store.completions.lock().unwrap().len(), 1);

    let retryable = RecordingStore {
        output_write_failures_remaining: Mutex::new(2),
        ..Default::default()
    };
    let first_attempt = finalize_terminal_outputs(
        &retryable,
        &FixtureDownloader,
        "job-retry",
        &["asset-retry".to_string()],
        &["https://results.test/retry.png".to_string()],
    );
    assert!(first_attempt.is_err());
    assert!(retryable.claimed.lock().unwrap().is_empty());

    let recovered = finalize_terminal_outputs(
        &retryable,
        &FixtureDownloader,
        "job-retry",
        &["asset-retry".to_string()],
        &["https://results.test/retry.png".to_string()],
    )
    .expect("restart recovery can reacquire the released terminal lease");
    assert!(recovered.claimed);
    assert_eq!((recovered.succeeded, recovered.failed), (1, 0));
    assert_eq!(retryable.completions.lock().unwrap().len(), 1);
}

#[test]
fn configured_capability_and_cost_authorization_gate_dispatch() {
    let bridge = Arc::new(RecordingGenerationBridge::new(false));
    let dispatcher = dispatcher_with_generation_bridge(bridge.clone());

    let timeline = dispatcher.dispatch("get_timeline", json!({}));
    assert!(!timeline.is_error);
    assert!(timeline.text_joined().contains("\"canGenerate\":false"));

    let unavailable = dispatcher.dispatch(
        "generate_image",
        json!({"costAuthorized": true, "prompt": "fixture"}),
    );
    assert!(unavailable.is_error);
    assert_eq!(bridge.submissions.load(Ordering::Acquire), 0);

    bridge.available.store(true, Ordering::Release);
    let timeline = dispatcher.dispatch("get_timeline", json!({}));
    assert!(timeline.text_joined().contains("\"canGenerate\":true"));

    let unauthorized = dispatcher.dispatch(
        "generate_image",
        json!({"costAuthorized": false, "prompt": "fixture"}),
    );
    assert!(unauthorized.is_error);
    assert_eq!(bridge.submissions.load(Ordering::Acquire), 0);

    let accepted = dispatcher.dispatch(
        "generate_image",
        json!({"costAuthorized": true, "prompt": "fixture"}),
    );
    assert!(!accepted.is_error, "{}", accepted.text_joined());
    assert!(accepted.text_joined().contains("job-dispatch"));
    assert!(accepted.text_joined().contains("asset-placeholder"));
    assert_eq!(bridge.submissions.load(Ordering::Acquire), 1);
}
