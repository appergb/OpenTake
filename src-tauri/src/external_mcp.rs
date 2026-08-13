use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::Write,
    net::Ipv4Addr,
    path::{Path, PathBuf},
    sync::{atomic::AtomicU64, Arc, RwLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use opentake_agent::mcp::{
    server::{bind_managed_gated_on, ManagedMcpEndpoint},
    AuthenticatedMcpClient, BearerAuthorizer,
};
use opentake_core::AppCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{Emitter, Manager};

use crate::chat::ExternalMcpComponents;
use crate::mcp::LiveProjectMcpGate;
use crate::secret::McpSecretStore;

const CATALOG_VERSION: u32 = 1;
const CATALOG_DIRECTORY: &str = "external-mcp";
const CATALOG_FILE: &str = "clients.json";
const PENDING_FILE: &str = "clients.pending.json";
const PREFERENCES_FILE: &str = "preferences.json";
const PREFERENCES_PENDING_FILE: &str = "preferences.pending.json";
const EXTERNAL_MCP_ENDPOINT: &str = "http://127.0.0.1:19789/mcp";
const EXTERNAL_MCP_STATUS_CHANGED: &str = "external_mcp_status_changed";
const EXTERNAL_MCP_PORT: u16 = 19_789;
const MAX_CLIENT_NAME_CHARS: usize = 128;
const TOKEN_BYTES: usize = 32;
const TOKEN_DIGEST_HEX_CHARS: usize = 12;
const LAST_USED_WRITE_INTERVAL_SECS: i64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ExternalMcpListenerState {
    Disabled,
    Starting,
    Listening,
    PortConflict,
    AuthFailure,
    Paused,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExternalMcpStatus {
    pub(crate) revision: u64,
    pub(crate) enabled: bool,
    pub(crate) state: ExternalMcpListenerState,
    pub(crate) endpoint: String,
    pub(crate) clients: Vec<ExternalMcpClientSummary>,
    pub(crate) error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct ExternalMcpClientSummary {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) token_digest: String,
    pub(crate) created_at: i64,
    pub(crate) last_used_at: Option<i64>,
    pub(crate) revoked_at: Option<i64>,
}

impl Default for ExternalMcpClientSummary {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            token_digest: String::new(),
            created_at: 0,
            last_used_at: None,
            revoked_at: None,
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExternalMcpPairingReceipt {
    pub(crate) client: ExternalMcpClientSummary,
    pub(crate) endpoint: String,
    pub(crate) bearer_token: String,
}

impl std::fmt::Debug for ExternalMcpPairingReceipt {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ExternalMcpPairingReceipt")
            .field("client", &self.client)
            .field("endpoint", &self.endpoint)
            .field("bearer_token", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct PersistedCatalog {
    version: u32,
    clients: Vec<ExternalMcpClientSummary>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct ExternalMcpPreferences {
    enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum PendingSecretState {
    Present { token_digest: String },
    Absent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PendingCatalogCommit {
    client_id: String,
    target: PersistedCatalog,
    secret_state: PendingSecretState,
}

#[derive(Debug)]
struct PublishError {
    error: String,
    published: bool,
}

impl Default for PersistedCatalog {
    fn default() -> Self {
        Self {
            version: CATALOG_VERSION,
            clients: Vec::new(),
        }
    }
}

pub(crate) struct ExternalMcpCatalog {
    root: PathBuf,
    clients: Vec<ExternalMcpClientSummary>,
    secrets: Arc<dyn McpSecretStore>,
    pending: bool,
    last_used_published_at: HashMap<String, i64>,
    #[cfg(test)]
    fail_next_rename: std::sync::atomic::AtomicBool,
    #[cfg(test)]
    fail_parent_sync_on_call: std::sync::atomic::AtomicUsize,
    #[cfg(test)]
    publish_count: std::sync::atomic::AtomicUsize,
}

struct ExternalMcpCatalogAuthorizer {
    credentials: RwLock<Arc<Vec<CachedCredential>>>,
    last_use: Arc<LastUseTracker>,
}

struct CachedCredential {
    token: String,
    client: AuthenticatedMcpClient,
}

#[derive(Default)]
struct LastUseTracker {
    entries: std::sync::Mutex<HashMap<String, LastUseEntry>>,
    changed: tokio::sync::Notify,
}

#[derive(Clone)]
struct LastUseEntry {
    latest: i64,
    dirty: bool,
    last_flushed: Option<Instant>,
}

struct LastUseWorker {
    shutdown: tokio::sync::oneshot::Sender<()>,
    join: tokio::task::JoinHandle<Result<(), String>>,
}

struct ExternalMcpStatusBroadcaster {
    revision: AtomicU64,
    sink: RwLock<Option<ExternalMcpStatusSink>>,
    latest: RwLock<Option<ExternalMcpStatus>>,
}

impl BearerAuthorizer for ExternalMcpCatalogAuthorizer {
    fn authorize(&self, candidate: &str) -> Option<AuthenticatedMcpClient> {
        let credentials = self.credentials.read().ok()?.clone();
        let client = credentials.iter().find_map(|credential| {
            constant_time_eq(credential.token.as_bytes(), candidate.as_bytes())
                .then(|| credential.client.clone())
        })?;
        if let Ok(now) = unix_timestamp() {
            self.last_use.record(&client.client_id, now);
        }
        Some(client)
    }
}

impl LastUseTracker {
    fn record(&self, client_id: &str, now: i64) {
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let entry = entries.entry(client_id.to_owned()).or_insert(LastUseEntry {
            latest: now,
            dirty: true,
            last_flushed: None,
        });
        entry.latest = entry.latest.max(now);
        entry.dirty = true;
        self.changed.notify_one();
    }

    fn due(&self, force: bool) -> HashMap<String, i64> {
        let now = Instant::now();
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .filter_map(|(client_id, entry)| {
                (entry.dirty
                    && (force
                        || entry.last_flushed.is_none_or(|flushed| {
                            now.duration_since(flushed)
                                >= Duration::from_secs(LAST_USED_WRITE_INTERVAL_SECS as u64)
                        })))
                .then(|| (client_id.clone(), entry.latest))
            })
            .collect()
    }

    fn mark_flushed(&self, persisted: &HashMap<String, i64>) {
        let now = Instant::now();
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for (client_id, timestamp) in persisted {
            if let Some(entry) = entries.get_mut(client_id) {
                if entry.latest <= *timestamp {
                    entry.dirty = false;
                }
                entry.last_flushed = Some(now);
            }
        }
    }
}

impl ExternalMcpStatusBroadcaster {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            revision: AtomicU64::new(0),
            sink: RwLock::new(None),
            latest: RwLock::new(None),
        })
    }

    fn publish(&self, mut status: ExternalMcpStatus) {
        status.revision = self
            .revision
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
            + 1;
        *self
            .latest
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(status.clone());
        let sink = self
            .sink
            .read()
            .ok()
            .and_then(|sink| sink.as_ref().cloned());
        if let Some(sink) = sink {
            sink(status);
        }
    }
}

pub(crate) struct ExternalMcpState {
    components: ExternalMcpComponents,
    catalog: Arc<RwLock<ExternalMcpCatalog>>,
    gate: Arc<LiveProjectMcpGate>,
    authorizer: Arc<ExternalMcpCatalogAuthorizer>,
    last_use_worker: tokio::sync::Mutex<Option<LastUseWorker>>,
    lifecycle: tokio::sync::Mutex<ExternalMcpLifecycle>,
    status: Arc<ExternalMcpStatusBroadcaster>,
    preference_parent_sync_on_call: std::sync::atomic::AtomicUsize,
}

type ExternalMcpStatusSink = Arc<dyn Fn(ExternalMcpStatus) + Send + Sync>;

struct ExternalMcpLifecycle {
    enabled: bool,
    state: ExternalMcpListenerState,
    error: Option<String>,
    auth_failure: Option<String>,
    endpoint: Option<ManagedMcpEndpoint>,
}

impl ExternalMcpState {
    pub(crate) fn new(
        core: AppCore,
        components: ExternalMcpComponents,
        catalog: ExternalMcpCatalog,
    ) -> Self {
        let credentials = catalog.load_active_credentials().unwrap_or_default();
        let catalog = Arc::new(RwLock::new(catalog));
        let last_use = Arc::new(LastUseTracker::default());
        let authorizer = Arc::new(ExternalMcpCatalogAuthorizer {
            credentials: RwLock::new(Arc::new(credentials)),
            last_use,
        });
        let gate = LiveProjectMcpGate::new(core.clone());
        Self {
            components,
            catalog,
            gate,
            authorizer,
            last_use_worker: tokio::sync::Mutex::new(None),
            lifecycle: tokio::sync::Mutex::new(ExternalMcpLifecycle {
                enabled: false,
                state: ExternalMcpListenerState::Disabled,
                error: None,
                auth_failure: None,
                endpoint: None,
            }),
            status: ExternalMcpStatusBroadcaster::new(),
            preference_parent_sync_on_call: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    pub(crate) fn load(
        core: AppCore,
        components: ExternalMcpComponents,
        app_data_dir: &Path,
        secrets: Arc<dyn McpSecretStore>,
    ) -> Self {
        let root = app_data_dir.join(CATALOG_DIRECTORY);
        let preferences = read_preferences(&root);
        let enabled = preferences
            .as_ref()
            .map(|value| value.enabled)
            .unwrap_or(false);
        let preference_error = preferences.as_ref().err().cloned();
        match ExternalMcpCatalog::load(app_data_dir, secrets.clone()) {
            Ok(catalog) if preferences.is_ok() => {
                let preferences = preferences.expect("checked external MCP preferences");
                let mut state = Self::new(core, components, catalog);
                state.lifecycle = tokio::sync::Mutex::new(ExternalMcpLifecycle {
                    enabled: preferences.enabled,
                    state: if preferences.enabled {
                        ExternalMcpListenerState::Paused
                    } else {
                        ExternalMcpListenerState::Disabled
                    },
                    error: None,
                    auth_failure: None,
                    endpoint: None,
                });
                state
            }
            catalog => {
                let error = match (catalog.err(), preference_error) {
                    (Some(catalog), _) => catalog,
                    (None, Some(preferences)) => preferences,
                    (None, None) => "external MCP authentication is unavailable".to_string(),
                };
                let catalog = ExternalMcpCatalog::unavailable(app_data_dir, secrets);
                let mut state = Self::new(core, components, catalog);
                state.lifecycle = tokio::sync::Mutex::new(ExternalMcpLifecycle {
                    enabled,
                    state: ExternalMcpListenerState::AuthFailure,
                    error: Some(sanitize_auth_failure(&error)),
                    auth_failure: Some(error),
                    endpoint: None,
                });
                state
            }
        }
    }

    pub(crate) fn auth_failure(
        core: AppCore,
        components: ExternalMcpComponents,
        error: String,
    ) -> Self {
        let root = std::env::temp_dir().join("opentake-unavailable-app-data");
        let catalog =
            ExternalMcpCatalog::unavailable(&root, Arc::new(opentake_gen::KeyringStore::new()));
        let mut state = Self::new(core, components, catalog);
        state.lifecycle = tokio::sync::Mutex::new(ExternalMcpLifecycle {
            enabled: false,
            state: ExternalMcpListenerState::AuthFailure,
            error: Some(sanitize_auth_failure(&error)),
            auth_failure: Some(error),
            endpoint: None,
        });
        state
    }

    pub(crate) async fn initialize(&self) {
        self.ensure_last_use_worker().await;
        let mut lifecycle = self.lifecycle.lock().await;
        self.reconcile_listener(&mut lifecycle).await;
    }

    pub(crate) async fn status(&self) -> ExternalMcpStatus {
        let lifecycle = self.lifecycle.lock().await;
        self.status_for(&lifecycle)
    }

    pub(crate) async fn set_enabled(&self, enabled: bool) -> Result<ExternalMcpStatus, String> {
        self.ensure_last_use_worker().await;
        let mut lifecycle = self.lifecycle.lock().await;
        if enabled {
            self.ensure_auth_ready(&lifecycle)?;
        }
        match persist_preferences(
            &self.catalog_root(),
            ExternalMcpPreferences { enabled },
            &self.preference_parent_sync_on_call,
        ) {
            Ok(()) => lifecycle.enabled = enabled,
            Err(error) if error.published => {
                lifecycle.enabled = enabled;
                self.reconcile_listener(&mut lifecycle).await;
                return Err(error.error);
            }
            Err(error) => return Err(error.error),
        }
        self.reconcile_listener(&mut lifecycle).await;
        Ok(self.status_for(&lifecycle))
    }

    pub(crate) async fn pair(&self, name: &str) -> Result<ExternalMcpPairingReceipt, String> {
        let mut lifecycle = self.lifecycle.lock().await;
        self.ensure_auth_ready(&lifecycle)?;
        let receipt = match self.with_catalog_write(|catalog| catalog.pair(name)) {
            Ok(receipt) => receipt,
            Err(error) => {
                self.handle_catalog_failure(&mut lifecycle, &error).await;
                return Err(error);
            }
        };
        if let Err(error) = self.refresh_credentials() {
            lifecycle.auth_failure = Some(error.clone());
            self.reconcile_listener(&mut lifecycle).await;
            return Err(error);
        }
        self.reconcile_listener(&mut lifecycle).await;
        Ok(receipt)
    }

    pub(crate) async fn regenerate(
        &self,
        client_id: &str,
    ) -> Result<ExternalMcpPairingReceipt, String> {
        let mut lifecycle = self.lifecycle.lock().await;
        self.ensure_auth_ready(&lifecycle)?;
        let previous = self.active_client(client_id)?;
        if let Some(endpoint) = lifecycle.endpoint.as_ref() {
            endpoint
                .cancel_client(&previous)
                .await
                .map_err(|error| error.to_string())?;
        }
        let receipt = match self.with_catalog_write(|catalog| catalog.regenerate(client_id)) {
            Ok(receipt) => receipt,
            Err(error) => {
                if self.catalog.read().is_ok_and(|catalog| !catalog.pending) {
                    if let Some(endpoint) = lifecycle.endpoint.as_ref() {
                        endpoint.restore_client(&previous);
                    }
                }
                self.handle_catalog_failure(&mut lifecycle, &error).await;
                return Err(error);
            }
        };
        self.refresh_credentials()?;
        self.reconcile_listener(&mut lifecycle).await;
        Ok(receipt)
    }

    pub(crate) async fn revoke(&self, client_id: &str) -> Result<ExternalMcpStatus, String> {
        let mut lifecycle = self.lifecycle.lock().await;
        self.ensure_auth_ready(&lifecycle)?;
        let previous = self.active_client(client_id)?;
        if let Some(endpoint) = lifecycle.endpoint.as_ref() {
            endpoint
                .cancel_client(&previous)
                .await
                .map_err(|error| error.to_string())?;
        }
        if let Err(error) = self.with_catalog_write(|catalog| catalog.revoke(client_id)) {
            if self.catalog.read().is_ok_and(|catalog| !catalog.pending) {
                if let Some(endpoint) = lifecycle.endpoint.as_ref() {
                    endpoint.restore_client(&previous);
                }
            }
            self.handle_catalog_failure(&mut lifecycle, &error).await;
            return Err(error);
        }
        self.refresh_credentials()?;
        self.reconcile_listener(&mut lifecycle).await;
        Ok(self.status_for(&lifecycle))
    }

    pub(crate) async fn shutdown(&self) -> Result<(), String> {
        let mut lifecycle = self.lifecycle.lock().await;
        self.stop_listener(&mut lifecycle).await?;
        drop(lifecycle);
        self.stop_last_use_worker().await?;
        let mut lifecycle = self.lifecycle.lock().await;
        if lifecycle.auth_failure.is_some() {
            lifecycle.state = ExternalMcpListenerState::AuthFailure;
        } else if lifecycle.enabled {
            lifecycle.state = ExternalMcpListenerState::Paused;
            lifecycle.error = None;
        } else {
            lifecycle.state = ExternalMcpListenerState::Disabled;
            lifecycle.error = None;
        }
        self.emit_status(&lifecycle);
        Ok(())
    }

    async fn reconcile_listener(&self, lifecycle: &mut ExternalMcpLifecycle) {
        if lifecycle.auth_failure.is_some() {
            let _ = self.stop_listener(lifecycle).await;
            lifecycle.state = ExternalMcpListenerState::AuthFailure;
            lifecycle.error = lifecycle.auth_failure.as_deref().map(sanitize_auth_failure);
            self.emit_status(lifecycle);
            return;
        }
        if !lifecycle.enabled {
            let stop_error = self.stop_listener(lifecycle).await.err();
            lifecycle.state = ExternalMcpListenerState::Disabled;
            lifecycle.error = stop_error;
            self.emit_status(lifecycle);
            return;
        }
        if !self.has_active_clients() {
            let stop_error = self.stop_listener(lifecycle).await.err();
            lifecycle.state = ExternalMcpListenerState::Paused;
            lifecycle.error = stop_error;
            self.emit_status(lifecycle);
            return;
        }
        if let Err(error) = self.refresh_credentials() {
            let _ = self.stop_listener(lifecycle).await;
            lifecycle.auth_failure = Some(error);
            lifecycle.state = ExternalMcpListenerState::AuthFailure;
            lifecycle.error = Some("external MCP authentication is unavailable".to_string());
            self.emit_status(lifecycle);
            return;
        }
        if lifecycle.endpoint.is_some() {
            lifecycle.state = ExternalMcpListenerState::Listening;
            lifecycle.error = None;
            self.emit_status(lifecycle);
            return;
        }
        lifecycle.state = ExternalMcpListenerState::Starting;
        lifecycle.error = None;
        self.emit_status(lifecycle);
        let listener =
            match tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, EXTERNAL_MCP_PORT)).await {
                Ok(listener) => listener,
                Err(error) => {
                    lifecycle.state = ExternalMcpListenerState::PortConflict;
                    lifecycle.error = Some(format!(
                        "external MCP port {EXTERNAL_MCP_PORT} is unavailable: {error}"
                    ));
                    self.emit_status(lifecycle);
                    return;
                }
            };
        match bind_managed_gated_on(
            listener,
            self.components.dispatcher.clone(),
            self.components.registry.clone(),
            self.gate.clone(),
            self.authorizer.clone(),
        )
        .await
        {
            Ok(endpoint) => {
                lifecycle.endpoint = Some(endpoint);
                lifecycle.state = ExternalMcpListenerState::Listening;
                lifecycle.error = None;
            }
            Err(error) => {
                lifecycle.state = ExternalMcpListenerState::PortConflict;
                lifecycle.error = Some(error.to_string());
            }
        }
        self.emit_status(lifecycle);
    }

    async fn handle_catalog_failure(&self, lifecycle: &mut ExternalMcpLifecycle, error: &str) {
        let ready = self.catalog.read().is_ok_and(|catalog| !catalog.pending);
        if !ready {
            lifecycle.auth_failure = Some(error.to_string());
        }
        self.reconcile_listener(lifecycle).await;
    }

    async fn stop_listener(&self, lifecycle: &mut ExternalMcpLifecycle) -> Result<(), String> {
        let Some(endpoint) = lifecycle.endpoint.take() else {
            return Ok(());
        };
        endpoint.shutdown();
        let result = endpoint.wait().await.map_err(|error| error.to_string());
        if let Err(error) = &result {
            lifecycle.state = ExternalMcpListenerState::Paused;
            lifecycle.error = Some(error.clone());
            self.emit_status(lifecycle);
        }
        result
    }

    fn status_for(&self, lifecycle: &ExternalMcpLifecycle) -> ExternalMcpStatus {
        let clients = self
            .catalog
            .read()
            .map(|catalog| catalog.clients().to_vec())
            .unwrap_or_default();
        ExternalMcpStatus {
            revision: self
                .status
                .revision
                .load(std::sync::atomic::Ordering::Acquire),
            enabled: lifecycle.enabled,
            state: lifecycle.state,
            endpoint: EXTERNAL_MCP_ENDPOINT.to_string(),
            clients,
            error: lifecycle.error.clone(),
        }
    }

    fn emit_status(&self, lifecycle: &ExternalMcpLifecycle) {
        self.status.publish(self.status_for(lifecycle));
    }

    fn ensure_auth_ready(&self, lifecycle: &ExternalMcpLifecycle) -> Result<(), String> {
        lifecycle.auth_failure.as_ref().map_or(Ok(()), |_| {
            Err("external MCP authentication is unavailable".to_string())
        })
    }

    fn with_catalog_write<T>(
        &self,
        operation: impl FnOnce(&mut ExternalMcpCatalog) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut catalog = self
            .catalog
            .write()
            .map_err(|_| "external MCP catalog lock is unavailable".to_string())?;
        operation(&mut catalog)
    }

    fn catalog_root(&self) -> PathBuf {
        self.catalog
            .read()
            .map(|catalog| catalog.root.clone())
            .unwrap_or_default()
    }

    fn has_active_clients(&self) -> bool {
        self.catalog.read().is_ok_and(|catalog| {
            catalog
                .clients()
                .iter()
                .any(|client| client.revoked_at.is_none())
        })
    }

    fn refresh_credentials(&self) -> Result<(), String> {
        let credentials = self
            .catalog
            .read()
            .map_err(|_| "external MCP catalog lock is unavailable".to_string())?
            .load_active_credentials()?;
        *self
            .authorizer
            .credentials
            .write()
            .map_err(|_| "external MCP credential snapshot lock is unavailable".to_string())? =
            Arc::new(credentials);
        Ok(())
    }

    fn active_client(&self, client_id: &str) -> Result<AuthenticatedMcpClient, String> {
        self.authorizer
            .credentials
            .read()
            .map_err(|_| "external MCP credential snapshot lock is unavailable".to_string())?
            .iter()
            .find(|credential| credential.client.client_id.as_ref() == client_id)
            .map(|credential| credential.client.clone())
            .ok_or_else(|| "external MCP client is unavailable".to_string())
    }

    async fn ensure_last_use_worker(&self) {
        let mut worker = self.last_use_worker.lock().await;
        if worker.is_some() {
            return;
        }
        let (shutdown, shutdown_rx) = tokio::sync::oneshot::channel();
        let join = spawn_last_use_worker(
            self.catalog.clone(),
            self.authorizer.last_use.clone(),
            self.status.clone(),
            shutdown_rx,
        );
        *worker = Some(LastUseWorker { shutdown, join });
    }

    async fn stop_last_use_worker(&self) -> Result<(), String> {
        let Some(worker) = self.last_use_worker.lock().await.take() else {
            return self.flush_last_use(true).await;
        };
        let _ = worker.shutdown.send(());
        worker
            .join
            .await
            .map_err(|error| format!("join external MCP last-use worker: {error}"))??;
        Ok(())
    }

    async fn flush_last_use(&self, force: bool) -> Result<(), String> {
        flush_last_use_blocking(
            self.catalog.clone(),
            self.authorizer.last_use.clone(),
            self.status.clone(),
            force,
        )
        .await
    }

    #[cfg(test)]
    fn set_status_sink(&self, sink: ExternalMcpStatusSink) {
        *self.status.sink.write().expect("external MCP status sink") = Some(sink);
    }

    #[cfg(test)]
    fn catalog_publish_count_for_test(&self) -> usize {
        self.catalog
            .read()
            .expect("external MCP catalog")
            .publish_count
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    #[cfg(test)]
    async fn record_last_used_at_for_test(
        &self,
        client_id: &str,
        timestamp: i64,
    ) -> Result<(), String> {
        self.authorizer.last_use.record(client_id, timestamp);
        self.flush_last_use(false).await
    }

    #[cfg(test)]
    fn fail_preference_parent_sync_after_publish_for_test(&self) {
        self.preference_parent_sync_on_call
            .store(2, std::sync::atomic::Ordering::SeqCst);
    }
}

fn spawn_last_use_worker(
    catalog: Arc<RwLock<ExternalMcpCatalog>>,
    tracker: Arc<LastUseTracker>,
    status: Arc<ExternalMcpStatusBroadcaster>,
    mut shutdown: tokio::sync::oneshot::Receiver<()>,
) -> tokio::task::JoinHandle<Result<(), String>> {
    tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(Duration::from_secs(LAST_USED_WRITE_INTERVAL_SECS as u64));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        interval.tick().await;
        loop {
            tokio::select! {
                _ = &mut shutdown => {
                    return flush_last_use_blocking(catalog, tracker, status, true).await;
                }
                () = tracker.changed.notified() => {}
                _ = interval.tick() => {}
            }
            flush_last_use_blocking(catalog.clone(), tracker.clone(), status.clone(), false)
                .await?;
        }
    })
}

async fn flush_last_use_blocking(
    catalog: Arc<RwLock<ExternalMcpCatalog>>,
    tracker: Arc<LastUseTracker>,
    status: Arc<ExternalMcpStatusBroadcaster>,
    force: bool,
) -> Result<(), String> {
    let due = tracker.due(force);
    if due.is_empty() {
        return Ok(());
    }
    let persisted = due.clone();
    let clients = tokio::task::spawn_blocking(move || {
        let mut catalog = catalog
            .write()
            .map_err(|_| "external MCP catalog lock is unavailable".to_string())?;
        catalog.persist_last_used(&due)?;
        Ok::<_, String>(catalog.clients().to_vec())
    })
    .await
    .map_err(|error| format!("join external MCP last-use publication: {error}"))??;
    tracker.mark_flushed(&persisted);
    let latest = status
        .latest
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    if let Some(mut latest) = latest {
        latest.clients = clients;
        status.publish(latest);
    }
    Ok(())
}

impl ExternalMcpCatalog {
    fn unavailable(app_data_dir: &Path, secrets: Arc<dyn McpSecretStore>) -> Self {
        Self {
            root: app_data_dir.join(CATALOG_DIRECTORY),
            clients: Vec::new(),
            secrets,
            pending: true,
            last_used_published_at: HashMap::new(),
            #[cfg(test)]
            fail_next_rename: std::sync::atomic::AtomicBool::new(false),
            #[cfg(test)]
            fail_parent_sync_on_call: std::sync::atomic::AtomicUsize::new(0),
            #[cfg(test)]
            publish_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    pub(crate) fn load(
        app_data_dir: &Path,
        secrets: Arc<dyn McpSecretStore>,
    ) -> Result<Self, String> {
        let root = app_data_dir.join(CATALOG_DIRECTORY);
        let path = root.join(CATALOG_FILE);
        let clients = read_catalog(&path)?;
        let mut catalog = Self {
            root,
            last_used_published_at: clients
                .iter()
                .filter_map(|client| {
                    client
                        .last_used_at
                        .map(|timestamp| (client.id.clone(), timestamp))
                })
                .collect(),
            clients,
            secrets,
            pending: false,
            #[cfg(test)]
            fail_next_rename: std::sync::atomic::AtomicBool::new(false),
            #[cfg(test)]
            fail_parent_sync_on_call: std::sync::atomic::AtomicUsize::new(0),
            #[cfg(test)]
            publish_count: std::sync::atomic::AtomicUsize::new(0),
        };
        catalog.recover_pending_commit()?;
        Ok(catalog)
    }

    pub(crate) fn clients(&self) -> &[ExternalMcpClientSummary] {
        &self.clients
    }

    pub(crate) fn metadata_path(&self) -> PathBuf {
        self.root.join(CATALOG_FILE)
    }

    fn pending_path(&self) -> PathBuf {
        self.root.join(PENDING_FILE)
    }

    fn recover_pending_commit(&mut self) -> Result<(), String> {
        let path = self.pending_path();
        let pending = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice::<PendingCatalogCommit>(&bytes)
                .map_err(|error| format!("read external MCP pending commit: {error}"))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(format!("read external MCP pending commit: {error}")),
        };
        validate_pending(&pending)?;
        let secret = self
            .secrets
            .load_mcp_secret(&secret_account(&pending.client_id))?;
        if pending_matches_secret(&pending.secret_state, secret.as_deref()) {
            self.publish_clients(&pending.target.clients)
                .map_err(|error| error.error)?;
            self.clients = pending.target.clients;
        } else if self.clients == pending.target.clients {
            return Err("external MCP pending commit has inconsistent secret state".to_string());
        }
        self.clear_pending()?;
        Ok(())
    }

    pub(crate) fn pair(&mut self, name: &str) -> Result<ExternalMcpPairingReceipt, String> {
        self.ensure_ready()?;
        let name = validate_name(name)?;
        let token = generate_token()?;
        let client = ExternalMcpClientSummary {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            token_digest: token_digest(&token),
            created_at: unix_timestamp()?,
            last_used_at: None,
            revoked_at: None,
        };
        let account = secret_account(&client.id);
        let mut next = self.clients.clone();
        next.push(client.clone());
        if let Err(error) = self.prepare_pending(
            &client.id,
            &next,
            PendingSecretState::Present {
                token_digest: client.token_digest.clone(),
            },
        ) {
            if self.pending {
                return Err(error);
            }
            self.clear_pending()?;
            return Err(error);
        }
        self.pending = true;
        self.secrets.save_mcp_secret(&account, &token)?;
        match self.publish_clients(&next) {
            Ok(()) => {
                self.clients = next;
                self.clear_pending()?;
            }
            Err(error) if error.published => {
                self.clients = next;
                self.pending = true;
                return Err(error.error);
            }
            Err(error) => {
                if let Err(rollback_error) = self.secrets.delete_mcp_secret(&account) {
                    return Err(format!(
                        "{}; external MCP credential cleanup failed: {rollback_error}",
                        error.error
                    ));
                }
                self.clear_pending()?;
                return Err(error.error);
            }
        }
        Ok(receipt(client, token))
    }

    pub(crate) fn regenerate(
        &mut self,
        client_id: &str,
    ) -> Result<ExternalMcpPairingReceipt, String> {
        self.ensure_ready()?;
        let index = self.client_index(client_id)?;
        if self.clients[index].revoked_at.is_some() {
            return Err("external MCP client is revoked".to_string());
        }
        let account = secret_account(client_id);
        let previous_token = self
            .secrets
            .load_mcp_secret(&account)?
            .ok_or_else(|| "external MCP client credential is unavailable".to_string())?;
        let token = generate_token()?;
        let mut next = self.clients.clone();
        next[index].token_digest = token_digest(&token);
        if let Err(error) = self.prepare_pending(
            client_id,
            &next,
            PendingSecretState::Present {
                token_digest: next[index].token_digest.clone(),
            },
        ) {
            if self.pending {
                return Err(error);
            }
            self.clear_pending()?;
            return Err(error);
        }
        self.pending = true;
        self.secrets.save_mcp_secret(&account, &token)?;
        match self.publish_clients(&next) {
            Ok(()) => {
                self.clients = next;
                self.clear_pending()?;
            }
            Err(error) if error.published => {
                self.clients = next;
                self.pending = true;
                return Err(error.error);
            }
            Err(error) => {
                if let Err(rollback_error) = self.secrets.save_mcp_secret(&account, &previous_token)
                {
                    return Err(format!(
                        "{}; external MCP credential rollback failed: {rollback_error}",
                        error.error
                    ));
                }
                self.clear_pending()?;
                return Err(error.error);
            }
        }
        Ok(receipt(self.clients[index].clone(), token))
    }

    pub(crate) fn revoke(&mut self, client_id: &str) -> Result<(), String> {
        self.ensure_ready()?;
        let index = self.client_index(client_id)?;
        if self.clients[index].revoked_at.is_some() {
            return Ok(());
        }
        let account = secret_account(client_id);
        let previous_token = self.secrets.load_mcp_secret(&account)?;
        let revoked_at = unix_timestamp()?;
        let mut next = self.clients.clone();
        next[index].revoked_at = Some(revoked_at);
        if let Err(error) = self.prepare_pending(client_id, &next, PendingSecretState::Absent) {
            if self.pending {
                return Err(error);
            }
            self.clear_pending()?;
            return Err(error);
        }
        self.pending = true;
        self.secrets.delete_mcp_secret(&account)?;
        match self.publish_clients(&next) {
            Ok(()) => {
                self.clients = next;
                self.clear_pending()?;
            }
            Err(error) if error.published => {
                self.clients = next;
                self.pending = true;
                return Err(error.error);
            }
            Err(error) => {
                if let Some(token) = previous_token {
                    if let Err(rollback_error) = self.secrets.save_mcp_secret(&account, &token) {
                        return Err(format!(
                            "{}; external MCP credential rollback failed: {rollback_error}",
                            error.error
                        ));
                    }
                }
                self.clear_pending()?;
                return Err(error.error);
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn verify_candidate(&self, candidate: &str) -> Result<Option<String>, String> {
        self.ensure_ready()?;
        let mut match_id = None;
        for client in self
            .clients
            .iter()
            .filter(|client| client.revoked_at.is_none())
        {
            let secret = self
                .secrets
                .load_mcp_secret(&secret_account(&client.id))?
                .ok_or_else(|| "external MCP client credential is unavailable".to_string())?;
            if token_digest(&secret) != client.token_digest {
                return Err("external MCP client credential is invalid".to_string());
            }
            if constant_time_eq(secret.as_bytes(), candidate.as_bytes()) {
                match_id = Some(client.id.clone());
            }
        }
        Ok(match_id)
    }

    fn load_active_credentials(&self) -> Result<Vec<CachedCredential>, String> {
        self.ensure_ready()?;
        self.clients
            .iter()
            .filter(|client| client.revoked_at.is_none())
            .map(|client| {
                let token = self
                    .secrets
                    .load_mcp_secret(&secret_account(&client.id))?
                    .ok_or_else(|| "external MCP client credential is unavailable".to_string())?;
                if token_digest(&token) != client.token_digest {
                    return Err("external MCP client credential is invalid".to_string());
                }
                Ok(CachedCredential {
                    client: AuthenticatedMcpClient {
                        client_id: client.id.clone().into(),
                        credential_generation: credential_generation(&token),
                    },
                    token,
                })
            })
            .collect()
    }

    fn persist_last_used(&mut self, updates: &HashMap<String, i64>) -> Result<(), String> {
        self.ensure_ready()?;
        let previous = self.clients.clone();
        for (client_id, timestamp) in updates {
            let index = self.client_index(client_id)?;
            self.clients[index].last_used_at = Some(
                self.clients[index]
                    .last_used_at
                    .unwrap_or(*timestamp)
                    .max(*timestamp),
            );
        }
        let next = self.clients.clone();
        if let Err(error) = self.publish_clients(&next) {
            self.clients = previous;
            return Err(error.error);
        }
        for (client_id, timestamp) in updates {
            self.last_used_published_at
                .insert(client_id.clone(), *timestamp);
        }
        Ok(())
    }

    fn client_index(&self, client_id: &str) -> Result<usize, String> {
        self.clients
            .iter()
            .position(|client| client.id == client_id)
            .ok_or_else(|| "external MCP client not found".to_string())
    }

    fn ensure_ready(&self) -> Result<(), String> {
        if self.pending {
            Err("external MCP catalog recovery is pending".to_string())
        } else {
            Ok(())
        }
    }

    fn prepare_pending(
        &mut self,
        client_id: &str,
        target_clients: &[ExternalMcpClientSummary],
        secret_state: PendingSecretState,
    ) -> Result<(), String> {
        match self.write_json_atomically(
            &self.pending_path(),
            &PendingCatalogCommit {
                client_id: client_id.to_string(),
                target: PersistedCatalog {
                    version: CATALOG_VERSION,
                    clients: target_clients.to_vec(),
                },
                secret_state,
            },
        ) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.pending = error.published;
                Err(error.error)
            }
        }
    }

    fn clear_pending(&mut self) -> Result<(), String> {
        fs::remove_file(self.pending_path())
            .or_else(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    Ok(())
                } else {
                    Err(error)
                }
            })
            .map_err(|error| format!("clear external MCP pending commit: {error}"))?;
        if let Err(error) = self.sync_parent_directory() {
            self.pending = true;
            return Err(format!("sync external MCP catalog directory: {error}"));
        }
        self.pending = false;
        Ok(())
    }

    fn publish_clients(&self, clients: &[ExternalMcpClientSummary]) -> Result<(), PublishError> {
        let result = self.write_json_atomically(
            &self.metadata_path(),
            &PersistedCatalog {
                version: CATALOG_VERSION,
                clients: clients.to_vec(),
            },
        );
        #[cfg(test)]
        if result.is_ok() {
            self.publish_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
        result
    }

    fn write_json_atomically<T: Serialize>(
        &self,
        destination: &Path,
        value: &T,
    ) -> Result<(), PublishError> {
        fs::create_dir_all(&self.root).map_err(|error| PublishError {
            error: format!("create external MCP catalog directory: {error}"),
            published: false,
        })?;
        let bytes = serde_json::to_vec_pretty(value).map_err(|error| PublishError {
            error: format!("encode external MCP catalog: {error}"),
            published: false,
        })?;
        let temp = self
            .root
            .join(format!(".clients.{}.tmp", uuid::Uuid::new_v4()));
        let result: Result<(), PublishError> = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)
                .map_err(|error| PublishError {
                    error: format!("create external MCP catalog staging file: {error}"),
                    published: false,
                })?;
            file.write_all(&bytes)
                .and_then(|()| file.sync_all())
                .map_err(|error| PublishError {
                    error: format!("write external MCP catalog staging file: {error}"),
                    published: false,
                })?;
            self.rename_atomically(&temp, destination)
                .map_err(|error| PublishError {
                    error,
                    published: false,
                })?;
            self.sync_parent_directory().map_err(|error| PublishError {
                error: format!("sync external MCP catalog directory: {error}"),
                published: true,
            })?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(temp);
        }
        result
    }

    #[cfg(test)]
    fn fail_next_atomic_rename_for_test(&self) {
        self.fail_next_rename
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    #[cfg(test)]
    fn fail_parent_sync_during_publish_for_test(&self) {
        // One sync seals the pending journal; the second seals clients.json.
        self.fail_parent_sync_on_call
            .store(2, std::sync::atomic::Ordering::SeqCst);
    }

    fn rename_atomically(&self, temp: &Path, destination: &Path) -> Result<(), String> {
        #[cfg(test)]
        if self
            .fail_next_rename
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            return Err("publish external MCP catalog: injected rename failure".to_string());
        }
        fs::rename(temp, destination)
            .map_err(|error| format!("publish external MCP catalog: {error}"))
    }

    fn sync_parent_directory(&self) -> std::io::Result<()> {
        #[cfg(test)]
        {
            let remaining = self
                .fail_parent_sync_on_call
                .load(std::sync::atomic::Ordering::SeqCst);
            if remaining != 0 {
                self.fail_parent_sync_on_call
                    .store(remaining - 1, std::sync::atomic::Ordering::SeqCst);
                if remaining == 1 {
                    return Err(std::io::Error::other("injected parent sync failure"));
                }
            }
        }
        sync_parent_directory(&self.root)
    }
}

#[cfg(unix)]
fn sync_parent_directory(parent: &Path) -> std::io::Result<()> {
    fs::File::open(parent)?.sync_all()
}

#[cfg(not(unix))]
fn sync_parent_directory(_parent: &Path) -> std::io::Result<()> {
    // Windows does not support opening a directory as a synchronizable File.
    // `rename` remains atomic; the OS owns the corresponding directory flush.
    Ok(())
}

fn read_catalog(path: &Path) -> Result<Vec<ExternalMcpClientSummary>, String> {
    match fs::read(path) {
        Ok(bytes) => {
            let persisted: PersistedCatalog = serde_json::from_slice(&bytes)
                .map_err(|error| format!("read external MCP catalog: {error}"))?;
            if persisted.version != CATALOG_VERSION {
                return Err("unsupported external MCP catalog version".to_string());
            }
            for client in &persisted.clients {
                validate_client(client)?;
            }
            Ok(persisted.clients)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(format!("read external MCP catalog: {error}")),
    }
}

fn read_preferences(root: &Path) -> Result<ExternalMcpPreferences, String> {
    let pending = root.join(PREFERENCES_PENDING_FILE);
    match fs::read(&pending) {
        Ok(bytes) => {
            let target: ExternalMcpPreferences = serde_json::from_slice(&bytes)
                .map_err(|error| format!("read external MCP pending preferences: {error}"))?;
            let current = read_preferences_file(root)?;
            if current != target {
                persist_preferences(root, target, &std::sync::atomic::AtomicUsize::new(0))
                    .map_err(|error| error.error)?;
            }
            fs::remove_file(&pending)
                .map_err(|error| format!("clear external MCP pending preferences: {error}"))?;
            sync_parent_directory(root)
                .map_err(|error| format!("sync external MCP preferences directory: {error}"))?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("read external MCP pending preferences: {error}")),
    }
    read_preferences_file(root)
}

fn read_preferences_file(root: &Path) -> Result<ExternalMcpPreferences, String> {
    match fs::read(root.join(PREFERENCES_FILE)) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|error| format!("read external MCP preferences: {error}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(ExternalMcpPreferences::default())
        }
        Err(error) => Err(format!("read external MCP preferences: {error}")),
    }
}

fn persist_preferences(
    root: &Path,
    preferences: ExternalMcpPreferences,
    fail_parent_sync_on_call: &std::sync::atomic::AtomicUsize,
) -> Result<(), PublishError> {
    fs::create_dir_all(root).map_err(|error| PublishError {
        error: format!("create external MCP preferences directory: {error}"),
        published: false,
    })?;
    let bytes = serde_json::to_vec_pretty(&preferences).map_err(|error| PublishError {
        error: format!("encode external MCP preferences: {error}"),
        published: false,
    })?;
    let pending = root.join(PREFERENCES_PENDING_FILE);
    write_preference_file(root, &pending, &bytes, fail_parent_sync_on_call, false)?;
    let staging = root.join(format!(".preferences.{}.tmp", uuid::Uuid::new_v4()));
    let result: Result<(), PublishError> = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)
            .map_err(|error| PublishError {
                error: format!("create external MCP preferences staging file: {error}"),
                published: false,
            })?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|error| PublishError {
                error: format!("write external MCP preferences staging file: {error}"),
                published: false,
            })?;
        fs::rename(&staging, root.join(PREFERENCES_FILE)).map_err(|error| PublishError {
            error: format!("publish external MCP preferences: {error}"),
            published: false,
        })?;
        sync_preference_parent(root, fail_parent_sync_on_call).map_err(|error| PublishError {
            error: format!("sync external MCP preferences directory: {error}"),
            published: true,
        })?;
        fs::remove_file(&pending).map_err(|error| PublishError {
            error: format!("clear external MCP pending preferences: {error}"),
            published: true,
        })?;
        sync_preference_parent(root, fail_parent_sync_on_call).map_err(|error| PublishError {
            error: format!("sync external MCP preferences directory: {error}"),
            published: true,
        })
    })();
    if result.is_err() {
        let _ = fs::remove_file(staging);
    }
    result
}

fn write_preference_file(
    root: &Path,
    destination: &Path,
    bytes: &[u8],
    fail_parent_sync_on_call: &std::sync::atomic::AtomicUsize,
    published: bool,
) -> Result<(), PublishError> {
    let staging = root.join(format!(".preferences.{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)
            .map_err(|error| PublishError {
                error: format!("create external MCP preferences staging file: {error}"),
                published,
            })?;
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|error| PublishError {
                error: format!("write external MCP preferences staging file: {error}"),
                published,
            })?;
        fs::rename(&staging, destination).map_err(|error| PublishError {
            error: format!("publish external MCP preferences: {error}"),
            published,
        })?;
        sync_preference_parent(root, fail_parent_sync_on_call).map_err(|error| PublishError {
            error: format!("sync external MCP preferences directory: {error}"),
            published: true,
        })
    })();
    if result.is_err() {
        let _ = fs::remove_file(staging);
    }
    result
}

fn sync_preference_parent(
    root: &Path,
    _fail_parent_sync_on_call: &std::sync::atomic::AtomicUsize,
) -> std::io::Result<()> {
    #[cfg(test)]
    if _fail_parent_sync_on_call
        .fetch_update(
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
            |remaining| remaining.checked_sub(1),
        )
        .is_ok_and(|remaining| remaining == 1)
    {
        return Err(std::io::Error::other(
            "injected preference parent sync failure",
        ));
    }
    sync_parent_directory(root)
}

fn sanitize_auth_failure(_error: &str) -> String {
    "external MCP authentication is unavailable".to_string()
}

fn validate_pending(pending: &PendingCatalogCommit) -> Result<(), String> {
    if pending.target.version != CATALOG_VERSION {
        return Err("unsupported external MCP pending catalog version".to_string());
    }
    let client = pending
        .target
        .clients
        .iter()
        .find(|client| client.id == pending.client_id)
        .ok_or_else(|| "external MCP pending commit has no target client".to_string())?;
    for candidate in &pending.target.clients {
        validate_client(candidate)?;
    }
    match &pending.secret_state {
        PendingSecretState::Present { token_digest } if token_digest == &client.token_digest => {
            Ok(())
        }
        PendingSecretState::Absent if client.revoked_at.is_some() => Ok(()),
        _ => Err("external MCP pending commit has inconsistent target state".to_string()),
    }
}

fn pending_matches_secret(state: &PendingSecretState, secret: Option<&str>) -> bool {
    match (state, secret) {
        (PendingSecretState::Absent, None) => true,
        (
            PendingSecretState::Present {
                token_digest: digest,
            },
            Some(secret),
        ) => digest == &token_digest(secret),
        _ => false,
    }
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    let width = left.len().max(right.len());
    for index in 0..width {
        let a = left.get(index).copied().unwrap_or(0);
        let b = right.get(index).copied().unwrap_or(0);
        difference |= usize::from(a ^ b);
    }
    difference == 0
}

fn receipt(client: ExternalMcpClientSummary, bearer_token: String) -> ExternalMcpPairingReceipt {
    ExternalMcpPairingReceipt {
        client,
        endpoint: EXTERNAL_MCP_ENDPOINT.to_string(),
        bearer_token,
    }
}

fn validate_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty()
        || name.chars().count() > MAX_CLIENT_NAME_CHARS
        || name.chars().any(char::is_control)
    {
        return Err(
            "external MCP client name must contain 1 to 128 non-control characters".to_string(),
        );
    }
    Ok(name.to_string())
}

fn validate_client(client: &ExternalMcpClientSummary) -> Result<(), String> {
    uuid::Uuid::parse_str(&client.id)
        .map_err(|_| "external MCP catalog has an invalid client id".to_string())?;
    validate_name(&client.name)?;
    if client.token_digest.len() != TOKEN_DIGEST_HEX_CHARS
        || !client
            .token_digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || client.created_at < 0
        || client.last_used_at.is_some_and(|timestamp| timestamp < 0)
        || client.revoked_at.is_some_and(|timestamp| timestamp < 0)
    {
        return Err("external MCP catalog has invalid client metadata".to_string());
    }
    Ok(())
}

fn generate_token() -> Result<String, String> {
    let mut bytes = [0_u8; TOKEN_BYTES];
    getrandom::fill(&mut bytes)
        .map_err(|error| format!("generate external MCP credential: {error}"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn token_digest(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    digest
        .iter()
        .take(TOKEN_DIGEST_HEX_CHARS / 2)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn credential_generation(token: &str) -> u64 {
    let digest = Sha256::digest(token.as_bytes());
    u64::from_be_bytes(
        digest[..std::mem::size_of::<u64>()]
            .try_into()
            .expect("SHA-256 contains a u64 credential generation"),
    )
}

fn secret_account(client_id: &str) -> String {
    format!("external-mcp:{client_id}")
}

fn unix_timestamp() -> Result<i64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("read external MCP clock: {error}"))
        .and_then(|duration| {
            i64::try_from(duration.as_secs())
                .map_err(|_| "external MCP clock is out of range".to_string())
        })
}

pub(crate) fn install_status_emitter<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    state: &ExternalMcpState,
) {
    let handle = app.clone();
    *state
        .status
        .sink
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Arc::new(move |status| {
        if let Err(error) = handle.emit(EXTERNAL_MCP_STATUS_CHANGED, status) {
            eprintln!("[mcp] could not emit external MCP status: {error}");
        }
    }));
}

#[tauri::command]
pub(crate) async fn external_mcp_status(
    state: tauri::State<'_, ExternalMcpState>,
) -> Result<ExternalMcpStatus, String> {
    Ok(state.status().await)
}

#[tauri::command]
pub(crate) async fn external_mcp_set_enabled(
    enabled: bool,
    state: tauri::State<'_, ExternalMcpState>,
    admission: tauri::State<'_, crate::updater::InstallAdmissionGate>,
) -> Result<ExternalMcpStatus, String> {
    let _activity = crate::updater::begin_mutating_activity(&admission)?;
    state.set_enabled(enabled).await
}

#[tauri::command]
pub(crate) async fn external_mcp_pair(
    name: String,
    state: tauri::State<'_, ExternalMcpState>,
    admission: tauri::State<'_, crate::updater::InstallAdmissionGate>,
) -> Result<ExternalMcpPairingReceipt, String> {
    let _activity = crate::updater::begin_mutating_activity(&admission)?;
    state.pair(&name).await
}

#[tauri::command]
pub(crate) async fn external_mcp_regenerate(
    client_id: String,
    state: tauri::State<'_, ExternalMcpState>,
    admission: tauri::State<'_, crate::updater::InstallAdmissionGate>,
) -> Result<ExternalMcpPairingReceipt, String> {
    let _activity = crate::updater::begin_mutating_activity(&admission)?;
    state.regenerate(&client_id).await
}

#[tauri::command]
pub(crate) async fn external_mcp_revoke(
    client_id: String,
    state: tauri::State<'_, ExternalMcpState>,
    admission: tauri::State<'_, crate::updater::InstallAdmissionGate>,
) -> Result<ExternalMcpStatus, String> {
    let _activity = crate::updater::begin_mutating_activity(&admission)?;
    state.revoke(&client_id).await
}

pub(crate) fn shutdown_on_exit<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let Some(state) = app.try_state::<ExternalMcpState>() else {
        return;
    };
    let result = tauri::async_runtime::block_on(state.shutdown());
    if let Err(error) = result {
        eprintln!("[mcp] external endpoint did not drain cleanly on application exit: {error}");
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    };

    use super::*;
    use crate::chat::ChatState;
    use crate::secret::{McpSecretStore, MemoryMcpSecretStore};
    use opentake_agent::chat::ChatTurnGate;

    static LIFECYCLE_PORT: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    #[derive(Default)]
    struct InstrumentedSecretStore {
        values: Mutex<HashMap<String, String>>,
        loads: AtomicUsize,
        fail_loads: AtomicBool,
    }

    impl InstrumentedSecretStore {
        fn reset_loads(&self) {
            self.loads.store(0, Ordering::SeqCst);
        }

        fn load_count(&self) -> usize {
            self.loads.load(Ordering::SeqCst)
        }

        fn fail_loads(&self) {
            self.fail_loads.store(true, Ordering::SeqCst);
        }
    }

    impl McpSecretStore for InstrumentedSecretStore {
        fn save_mcp_secret(&self, account: &str, value: &str) -> Result<(), String> {
            self.values
                .lock()
                .expect("instrumented secret values")
                .insert(account.to_owned(), value.to_owned());
            Ok(())
        }

        fn load_mcp_secret(&self, account: &str) -> Result<Option<String>, String> {
            self.loads.fetch_add(1, Ordering::SeqCst);
            if self.fail_loads.load(Ordering::SeqCst) {
                return Err("injected secret-store load failure".to_string());
            }
            Ok(self
                .values
                .lock()
                .expect("instrumented secret values")
                .get(account)
                .cloned())
        }

        fn delete_mcp_secret(&self, account: &str) -> Result<(), String> {
            self.values
                .lock()
                .expect("instrumented secret values")
                .remove(account);
            Ok(())
        }
    }

    fn catalog_root() -> tempfile::TempDir {
        tempfile::tempdir().expect("create temporary application data directory")
    }

    fn load_catalog(
        root: &tempfile::TempDir,
        secrets: Arc<dyn McpSecretStore>,
    ) -> ExternalMcpCatalog {
        ExternalMcpCatalog::load(root.path(), secrets)
            .expect("load catalog against the in-memory secret store")
    }

    fn shared_state(
        root: &tempfile::TempDir,
        core: opentake_core::AppCore,
    ) -> (ChatState, ExternalMcpState) {
        let chat = ChatState::new(
            core.clone(),
            root.path().join("no-workflows"),
            root.path().join("chat-cache"),
            root.path().join("chat-models"),
        );
        let catalog = load_catalog(root, Arc::new(MemoryMcpSecretStore::default()));
        let external = ExternalMcpState::new(core, chat.external_mcp_components(), catalog);
        (chat, external)
    }

    fn lifecycle_state(
        root: &tempfile::TempDir,
        secrets: Arc<dyn McpSecretStore>,
    ) -> ExternalMcpState {
        let core = opentake_core::AppCore::new();
        let chat = ChatState::new(
            core.clone(),
            root.path().join("no-workflows"),
            root.path().join("chat-cache"),
            root.path().join("chat-models"),
        );
        ExternalMcpState::load(core, chat.external_mcp_components(), root.path(), secrets)
    }

    async fn assert_fixed_port_available() {
        let listener =
            tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, EXTERNAL_MCP_PORT))
                .await
                .expect("fixed external MCP port is available");
        drop(listener);
    }

    #[tokio::test]
    async fn lifecycle_disabled_startup_never_binds() {
        let _port = LIFECYCLE_PORT.lock().await;
        let root = catalog_root();
        let state = lifecycle_state(&root, Arc::new(MemoryMcpSecretStore::default()));

        state.initialize().await;

        let status = state.status().await;
        assert!(!status.enabled);
        assert_eq!(status.state, ExternalMcpListenerState::Disabled);
        assert!(status.clients.is_empty());
        assert_fixed_port_available().await;
    }

    #[tokio::test]
    async fn lifecycle_disable_stops_an_active_listener_and_persists() {
        let _port = LIFECYCLE_PORT.lock().await;
        let root = catalog_root();
        let secrets = Arc::new(MemoryMcpSecretStore::default());
        let state = lifecycle_state(&root, secrets.clone());
        state.set_enabled(true).await.expect("enable endpoint");
        state.pair("Cursor").await.expect("pair client");

        let status = state.set_enabled(false).await.expect("disable endpoint");

        assert_eq!(status.state, ExternalMcpListenerState::Disabled);
        assert_fixed_port_available().await;
        let restarted = lifecycle_state(&root, secrets);
        restarted.initialize().await;
        assert_eq!(
            restarted.status().await.state,
            ExternalMcpListenerState::Disabled
        );
    }

    #[tokio::test]
    async fn lifecycle_catalog_recovery_failure_is_a_fail_closed_status() {
        let _port = LIFECYCLE_PORT.lock().await;
        let root = catalog_root();
        let metadata = root.path().join(CATALOG_DIRECTORY).join(CATALOG_FILE);
        std::fs::create_dir_all(metadata.parent().expect("catalog parent"))
            .expect("create corrupt catalog directory");
        std::fs::write(&metadata, b"not-json").expect("write corrupt catalog");

        let state = lifecycle_state(&root, Arc::new(MemoryMcpSecretStore::default()));
        state.initialize().await;

        let status = state.status().await;
        assert_eq!(status.state, ExternalMcpListenerState::AuthFailure);
        assert_eq!(
            status.error.as_deref(),
            Some("external MCP authentication is unavailable")
        );
        assert!(status.clients.is_empty());
        assert_fixed_port_available().await;
        assert!(state.pair("must fail closed").await.is_err());
    }

    #[tokio::test]
    async fn lifecycle_durability_failure_transitions_to_auth_failure() {
        let _port = LIFECYCLE_PORT.lock().await;
        let root = catalog_root();
        let state = lifecycle_state(&root, Arc::new(MemoryMcpSecretStore::default()));
        state.set_enabled(true).await.expect("enable endpoint");
        let first = state.pair("Cursor").await.expect("pair first client");
        state
            .catalog
            .read()
            .expect("external MCP catalog")
            .fail_parent_sync_during_publish_for_test();

        let error = state
            .regenerate(&first.client.id)
            .await
            .expect_err("injected durability failure is reported");

        assert!(error.contains("sync external MCP catalog directory"));
        let status = state.status().await;
        assert_eq!(status.state, ExternalMcpListenerState::AuthFailure);
        assert_eq!(
            status.error.as_deref(),
            Some("external MCP authentication is unavailable")
        );
        assert_fixed_port_available().await;
    }

    #[tokio::test]
    async fn lifecycle_enabled_restart_recovers_the_fixed_listener() {
        let _port = LIFECYCLE_PORT.lock().await;
        let root = catalog_root();
        let secrets = Arc::new(MemoryMcpSecretStore::default());
        let first = lifecycle_state(&root, secrets.clone());
        first.set_enabled(true).await.expect("persist enablement");
        first.pair("Claude Desktop").await.expect("pair client");
        assert_eq!(
            first.status().await.state,
            ExternalMcpListenerState::Listening
        );
        first.shutdown().await.expect("drain first listener");

        let restarted = lifecycle_state(&root, secrets);
        restarted.initialize().await;

        let status = restarted.status().await;
        assert!(status.enabled);
        assert_eq!(status.state, ExternalMcpListenerState::Listening);
        restarted
            .shutdown()
            .await
            .expect("drain restarted listener");
    }

    #[tokio::test]
    async fn lifecycle_missing_active_credential_fails_closed_before_bind() {
        let _port = LIFECYCLE_PORT.lock().await;
        let root = catalog_root();
        let secrets = Arc::new(MemoryMcpSecretStore::default());
        let first = lifecycle_state(&root, secrets.clone());
        first.set_enabled(true).await.expect("enable endpoint");
        let paired = first.pair("Cursor").await.expect("pair client");
        first.shutdown().await.expect("stop first endpoint");
        secrets
            .delete_mcp_secret(&secret_account(&paired.client.id))
            .expect("remove active credential");

        let restarted = lifecycle_state(&root, secrets);
        restarted.initialize().await;

        let status = restarted.status().await;
        assert!(status.enabled);
        assert_eq!(status.state, ExternalMcpListenerState::AuthFailure);
        assert_fixed_port_available().await;
    }

    #[tokio::test]
    async fn lifecycle_secret_store_failure_drains_a_previously_listening_endpoint() {
        let _port = LIFECYCLE_PORT.lock().await;
        let root = catalog_root();
        let secrets = Arc::new(InstrumentedSecretStore::default());
        let state = lifecycle_state(&root, secrets.clone());
        state.set_enabled(true).await.expect("enable endpoint");
        state.pair("Cursor").await.expect("pair client");
        assert_eq!(
            state.status().await.state,
            ExternalMcpListenerState::Listening
        );
        secrets.fail_loads();

        state.initialize().await;

        assert_eq!(
            state.status().await.state,
            ExternalMcpListenerState::AuthFailure
        );
        assert_fixed_port_available().await;
    }

    #[tokio::test]
    async fn lifecycle_enabled_without_clients_stays_paused_and_unbound() {
        let _port = LIFECYCLE_PORT.lock().await;
        let root = catalog_root();
        let state = lifecycle_state(&root, Arc::new(MemoryMcpSecretStore::default()));

        let status = state.set_enabled(true).await.expect("enable endpoint");

        assert_eq!(status.state, ExternalMcpListenerState::Paused);
        assert_fixed_port_available().await;
    }

    #[tokio::test]
    async fn lifecycle_port_conflict_is_reported_without_a_fallback_listener() {
        let _port = LIFECYCLE_PORT.lock().await;
        let occupied =
            tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, EXTERNAL_MCP_PORT))
                .await
                .expect("occupy fixed port");
        let root = catalog_root();
        let state = lifecycle_state(&root, Arc::new(MemoryMcpSecretStore::default()));
        state.set_enabled(true).await.expect("enable endpoint");

        state
            .pair("Claude Desktop")
            .await
            .expect("pair despite bind conflict");

        let status = state.status().await;
        assert_eq!(status.state, ExternalMcpListenerState::PortConflict);
        assert_eq!(status.endpoint, EXTERNAL_MCP_ENDPOINT);
        drop(occupied);
        state.shutdown().await.expect("shutdown conflicted state");
    }

    #[tokio::test]
    async fn lifecycle_emits_starting_before_listening_without_a_token() {
        let _port = LIFECYCLE_PORT.lock().await;
        let root = catalog_root();
        let state = lifecycle_state(&root, Arc::new(MemoryMcpSecretStore::default()));
        let events = Arc::new(Mutex::new(Vec::<ExternalMcpStatus>::new()));
        let captured = events.clone();
        state.set_status_sink(Arc::new(move |status| {
            captured.lock().expect("record status").push(status);
        }));
        state.set_enabled(true).await.expect("enable endpoint");

        let receipt = state.pair("Claude Desktop").await.expect("pair and start");

        let events = events.lock().expect("read statuses");
        let transitions = events.iter().map(|status| status.state).collect::<Vec<_>>();
        assert!(transitions.windows(2).any(|states| {
            states
                == [
                    ExternalMcpListenerState::Starting,
                    ExternalMcpListenerState::Listening,
                ]
        }));
        let serialized = serde_json::to_string(&*events).expect("serialize status events");
        assert!(!serialized.contains(&receipt.bearer_token));
        assert!(events
            .windows(2)
            .all(|statuses| { statuses[1].revision == statuses[0].revision.saturating_add(1) }));
        drop(events);
        state.shutdown().await.expect("drain listener");
    }

    #[test]
    fn lifecycle_pairing_receipt_is_the_only_dto_that_serializes_a_token() {
        let root = catalog_root();
        let core = opentake_core::AppCore::new();
        let chat = ChatState::new(
            core.clone(),
            root.path().join("no-workflows"),
            root.path().join("chat-cache"),
            root.path().join("chat-models"),
        );
        let mut catalog = load_catalog(&root, Arc::new(MemoryMcpSecretStore::default()));
        let receipt = catalog.pair("Cursor").expect("pair client");
        let state = ExternalMcpState::new(core, chat.external_mcp_components(), catalog);
        let lifecycle = state.lifecycle.blocking_lock();

        let receipt_json = serde_json::to_string(&receipt).expect("serialize receipt");
        let status_json = serde_json::to_string(&state.status_for(&lifecycle))
            .expect("serialize sanitized status");

        assert!(receipt_json.contains(&receipt.bearer_token));
        assert!(!status_json.contains(&receipt.bearer_token));
        assert!(!status_json.contains("bearerToken"));
    }

    #[tokio::test]
    async fn lifecycle_pair_while_enabled_starts_the_listener() {
        let _port = LIFECYCLE_PORT.lock().await;
        let root = catalog_root();
        let state = lifecycle_state(&root, Arc::new(MemoryMcpSecretStore::default()));
        state.set_enabled(true).await.expect("enable endpoint");

        let receipt = state.pair("Cursor").await.expect("pair client");

        assert_eq!(receipt.endpoint, EXTERNAL_MCP_ENDPOINT);
        assert_eq!(
            state.status().await.state,
            ExternalMcpListenerState::Listening
        );
        state.shutdown().await.expect("drain listener");
    }

    #[tokio::test]
    async fn lifecycle_revoking_the_final_client_stops_the_listener() {
        let _port = LIFECYCLE_PORT.lock().await;
        let root = catalog_root();
        let state = lifecycle_state(&root, Arc::new(MemoryMcpSecretStore::default()));
        state.set_enabled(true).await.expect("enable endpoint");
        let receipt = state.pair("Cursor").await.expect("pair client");

        let status = state
            .revoke(&receipt.client.id)
            .await
            .expect("revoke client");

        assert_eq!(status.state, ExternalMcpListenerState::Paused);
        assert_fixed_port_available().await;
    }

    #[tokio::test]
    async fn lifecycle_revoke_cancels_the_revoked_rmcp_session() {
        let _port = LIFECYCLE_PORT.lock().await;
        let root = catalog_root();
        let state = lifecycle_state(&root, Arc::new(MemoryMcpSecretStore::default()));
        state.set_enabled(true).await.expect("enable endpoint");
        let first = state.pair("Cursor").await.expect("pair first client");
        let survivor = state.pair("Claude").await.expect("pair survivor");
        let client = reqwest::Client::new();
        let revoked_session = initialize_managed_session(
            &client,
            EXTERNAL_MCP_ENDPOINT,
            &first.bearer_token,
            "revoked-session",
        )
        .await;
        let survivor_session = initialize_managed_session(
            &client,
            EXTERNAL_MCP_ENDPOINT,
            &survivor.bearer_token,
            "survivor-session",
        )
        .await;
        state.revoke(&first.client.id).await.expect("revoke client");

        let stale = client
            .post(EXTERNAL_MCP_ENDPOINT)
            .bearer_auth(&survivor.bearer_token)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .header("mcp-session-id", revoked_session)
            .header("mcp-protocol-version", "2025-06-18")
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/list",
                "params": {}
            }))
            .send()
            .await
            .expect("send request with revoked rmcp session");
        assert!(!stale.status().is_success());
        assert_managed_session_usable(
            &client,
            EXTERNAL_MCP_ENDPOINT,
            &survivor.bearer_token,
            &survivor_session,
            3,
        )
        .await;
        state.shutdown().await.expect("drain listener");
    }

    #[tokio::test]
    async fn lifecycle_regeneration_cancels_the_old_rmcp_session() {
        let _port = LIFECYCLE_PORT.lock().await;
        let root = catalog_root();
        let state = lifecycle_state(&root, Arc::new(MemoryMcpSecretStore::default()));
        state.set_enabled(true).await.expect("enable endpoint");
        let first = state.pair("Cursor").await.expect("pair client");
        let survivor = state.pair("Claude").await.expect("pair survivor");
        let client = reqwest::Client::new();
        let old_session = initialize_managed_session(
            &client,
            EXTERNAL_MCP_ENDPOINT,
            &first.bearer_token,
            "old-session",
        )
        .await;
        let survivor_session = initialize_managed_session(
            &client,
            EXTERNAL_MCP_ENDPOINT,
            &survivor.bearer_token,
            "survivor-session",
        )
        .await;

        let regenerated = state
            .regenerate(&first.client.id)
            .await
            .expect("regenerate credential");

        let stale = client
            .post(EXTERNAL_MCP_ENDPOINT)
            .bearer_auth(&regenerated.bearer_token)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .header("mcp-session-id", old_session)
            .header("mcp-protocol-version", "2025-06-18")
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/list",
                "params": {}
            }))
            .send()
            .await
            .expect("send request with stale rmcp session");
        assert!(!stale.status().is_success());
        let _new_session = initialize_managed_session(
            &client,
            EXTERNAL_MCP_ENDPOINT,
            &regenerated.bearer_token,
            "new-session",
        )
        .await;
        assert_managed_session_usable(
            &client,
            EXTERNAL_MCP_ENDPOINT,
            &survivor.bearer_token,
            &survivor_session,
            3,
        )
        .await;
        state.shutdown().await.expect("drain regenerated listener");
    }

    #[tokio::test]
    async fn lifecycle_application_shutdown_drains_and_releases_the_port() {
        let _port = LIFECYCLE_PORT.lock().await;
        let root = catalog_root();
        let state = lifecycle_state(&root, Arc::new(MemoryMcpSecretStore::default()));
        state.set_enabled(true).await.expect("enable endpoint");
        state.pair("Cursor").await.expect("pair client");

        state.shutdown().await.expect("application exit drain");

        assert_fixed_port_available().await;
    }

    #[tokio::test]
    async fn lifecycle_last_used_updates_are_persisted_once_per_coalescing_window() {
        let _port = LIFECYCLE_PORT.lock().await;
        let root = catalog_root();
        let secrets = Arc::new(MemoryMcpSecretStore::default());
        let state = lifecycle_state(&root, secrets.clone());
        state.set_enabled(true).await.expect("enable endpoint");
        let receipt = state.pair("Cursor").await.expect("pair client");
        let writes_before = state.catalog_publish_count_for_test();
        let client = reqwest::Client::new();

        for name in ["first", "second", "third"] {
            let _session = initialize_managed_session(
                &client,
                EXTERNAL_MCP_ENDPOINT,
                &receipt.bearer_token,
                name,
            )
            .await;
        }

        assert!(state.status().await.clients[0].last_used_at.is_some());
        assert_eq!(state.catalog_publish_count_for_test(), writes_before + 1);
        let reloaded = load_catalog(&root, secrets);
        let persisted = reloaded.clients()[0]
            .last_used_at
            .expect("last-used timestamp persisted");
        let in_memory = state.status().await.clients[0]
            .last_used_at
            .expect("last-used timestamp visible");
        assert!(in_memory >= persisted);
        state.shutdown().await.expect("drain listener");
    }

    #[tokio::test]
    async fn lifecycle_shutdown_flushes_the_latest_coalesced_last_used_value() {
        let _port = LIFECYCLE_PORT.lock().await;
        let root = catalog_root();
        let secrets = Arc::new(MemoryMcpSecretStore::default());
        let state = lifecycle_state(&root, secrets.clone());
        let paired = state.pair("Cursor").await.expect("pair client");
        let t0 = 1_800_000_000;
        state
            .record_last_used_at_for_test(&paired.client.id, t0)
            .await
            .expect("record leading last-used value");
        state
            .record_last_used_at_for_test(&paired.client.id, t0 + 30)
            .await
            .expect("record trailing last-used value");

        state.shutdown().await.expect("flush and stop lifecycle");

        let reloaded = load_catalog(&root, secrets);
        assert_eq!(reloaded.clients()[0].last_used_at, Some(t0 + 30));
    }

    #[test]
    fn lifecycle_authorizer_uses_only_the_validated_in_memory_snapshot() {
        let root = catalog_root();
        let secrets = Arc::new(InstrumentedSecretStore::default());
        let mut catalog = load_catalog(&root, secrets.clone());
        let paired = catalog.pair("Cursor").expect("pair client");
        let core = opentake_core::AppCore::new();
        let chat = ChatState::new(
            core.clone(),
            root.path().join("no-workflows"),
            root.path().join("chat-cache"),
            root.path().join("chat-models"),
        );
        let state = ExternalMcpState::new(core, chat.external_mcp_components(), catalog);
        secrets.reset_loads();
        let writes_before = state.catalog_publish_count_for_test();

        for _ in 0..3 {
            assert!(state.authorizer.authorize(&paired.bearer_token).is_some());
        }

        assert_eq!(secrets.load_count(), 0, "authorization read the keychain");
        assert_eq!(
            state.catalog_publish_count_for_test(),
            writes_before,
            "authorization synchronously published the catalog"
        );
    }

    #[tokio::test]
    async fn lifecycle_auth_failure_preserves_enabled_and_allows_persisted_disable() {
        let _port = LIFECYCLE_PORT.lock().await;
        let root = catalog_root();
        let catalog_dir = root.path().join(CATALOG_DIRECTORY);
        std::fs::create_dir_all(&catalog_dir).expect("create external MCP directory");
        persist_preferences(
            &catalog_dir,
            ExternalMcpPreferences { enabled: true },
            &std::sync::atomic::AtomicUsize::new(0),
        )
        .expect("persist enabled preference");
        std::fs::write(catalog_dir.join(CATALOG_FILE), b"not-json").expect("write corrupt catalog");
        let secrets = Arc::new(MemoryMcpSecretStore::default());

        let failed = lifecycle_state(&root, secrets.clone());
        failed.initialize().await;
        assert!(failed.status().await.enabled);
        assert_eq!(
            failed.status().await.state,
            ExternalMcpListenerState::AuthFailure
        );
        assert!(failed.set_enabled(true).await.is_err());
        assert!(failed.pair("blocked").await.is_err());

        let disabled = failed
            .set_enabled(false)
            .await
            .expect("disable remains available while authentication is failed");
        assert!(!disabled.enabled);
        assert_eq!(disabled.state, ExternalMcpListenerState::AuthFailure);

        let restarted = lifecycle_state(&root, secrets);
        assert!(!restarted.status().await.enabled);
        assert_eq!(
            restarted.status().await.state,
            ExternalMcpListenerState::AuthFailure
        );
    }

    #[tokio::test]
    async fn lifecycle_preference_post_rename_sync_failure_converges_now_and_on_restart() {
        let _port = LIFECYCLE_PORT.lock().await;
        let root = catalog_root();
        let secrets = Arc::new(MemoryMcpSecretStore::default());
        let state = lifecycle_state(&root, secrets.clone());
        state.fail_preference_parent_sync_after_publish_for_test();

        let result = state.set_enabled(true).await;
        assert!(result.is_err(), "injected durability failure is reported");
        assert!(
            state.status().await.enabled,
            "runtime follows published file"
        );

        let restarted = lifecycle_state(&root, secrets);
        assert!(
            restarted.status().await.enabled,
            "restart recovers published target"
        );
    }

    async fn initialize_managed_session(
        client: &reqwest::Client,
        url: &str,
        token: &str,
        name: &str,
    ) -> reqwest::header::HeaderValue {
        let response = client
            .post(url)
            .bearer_auth(token)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": { "name": name, "version": "0" }
                }
            }))
            .send()
            .await
            .expect("initialize managed MCP session");
        assert!(response.status().is_success());
        response
            .headers()
            .get("mcp-session-id")
            .expect("managed rmcp session id")
            .clone()
    }

    async fn assert_managed_session_usable(
        client: &reqwest::Client,
        url: &str,
        token: &str,
        session: &reqwest::header::HeaderValue,
        id: u64,
    ) {
        let response = client
            .post(url)
            .bearer_auth(token)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .header("mcp-session-id", session.clone())
            .header("mcp-protocol-version", "2025-06-18")
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "tools/list",
                "params": {}
            }))
            .send()
            .await
            .expect("send request through surviving rmcp session");
        assert!(
            response.status().is_success(),
            "surviving session was cancelled"
        );
    }

    async fn call_managed_tool(
        client: &reqwest::Client,
        url: &str,
        token: &str,
        session: &reqwest::header::HeaderValue,
        id: u64,
        name: &str,
        arguments: serde_json::Value,
    ) -> String {
        let response = client
            .post(url)
            .bearer_auth(token)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .header("mcp-session-id", session.clone())
            .header("mcp-protocol-version", "2025-06-18")
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "tools/call",
                "params": { "name": name, "arguments": arguments }
            }))
            .send()
            .await
            .expect("call managed MCP tool");
        assert!(response.status().is_success());
        response.text().await.expect("read managed MCP result")
    }

    #[test]
    fn shared_state_reuses_chat_dispatcher_and_registry_arcs() {
        let root = catalog_root();
        let core = opentake_core::AppCore::new();
        let chat = ChatState::new(
            core.clone(),
            root.path().join("no-workflows"),
            root.path().join("chat-cache"),
            root.path().join("chat-models"),
        );
        let expected = chat.external_mcp_components();
        let catalog = load_catalog(&root, Arc::new(MemoryMcpSecretStore::default()));

        let external = ExternalMcpState::new(core, chat.external_mcp_components(), catalog);

        assert!(Arc::ptr_eq(
            &external.components.dispatcher,
            &expected.dispatcher
        ));
        assert!(Arc::ptr_eq(
            &external.components.registry,
            &expected.registry
        ));
    }

    #[test]
    fn shared_live_gate_refuses_mutation_without_a_saved_project() {
        let root = catalog_root();
        let core = opentake_core::AppCore::new();
        let (_chat, external) = shared_state(&root, core.clone());

        let refused = external.gate.dispatch_cancellable_scoped(
            &external.components.dispatcher,
            "create_folder",
            serde_json::json!({ "name": "must-not-exist" }),
            "opentake:mcp:unsaved",
            &opentake_media::MediaCancelToken::new(),
        );

        assert!(refused.is_none());
        assert!(core.media().folders.is_empty());
    }

    #[tokio::test]
    async fn shared_catalog_authorizer_tracks_regenerated_credentials_without_exporting_them() {
        let root = catalog_root();
        let core = opentake_core::AppCore::new();
        let chat = ChatState::new(
            core.clone(),
            root.path().join("no-workflows"),
            root.path().join("chat-cache"),
            root.path().join("chat-models"),
        );
        let mut catalog = load_catalog(&root, Arc::new(MemoryMcpSecretStore::default()));
        let first = catalog.pair("Claude Desktop").expect("pair client");
        let external = ExternalMcpState::new(core, chat.external_mcp_components(), catalog);

        let first_client = external
            .authorizer
            .authorize(&first.bearer_token)
            .expect("authorize original credential");
        assert_eq!(first_client.client_id.as_ref(), first.client.id);
        assert!(external.authorizer.authorize("wrong credential").is_none());

        let regenerated = external
            .regenerate(&first.client.id)
            .await
            .expect("regenerate credential");
        assert!(external.authorizer.authorize(&first.bearer_token).is_none());
        let regenerated_client = external
            .authorizer
            .authorize(&regenerated.bearer_token)
            .expect("authorize regenerated credential");
        assert_eq!(regenerated_client.client_id, first_client.client_id);
        assert_ne!(
            regenerated_client.credential_generation,
            first_client.credential_generation
        );
    }

    #[tokio::test]
    async fn shared_transport_scopes_isolate_two_rmcp_sessions_and_in_app_chat_undo() {
        let root = catalog_root();
        let core = opentake_core::AppCore::new();
        core.save_project(Some(root.path().join("Shared.opentake")))
            .expect("save shared project");
        let chat = ChatState::new(
            core.clone(),
            root.path().join("no-workflows"),
            root.path().join("chat-cache"),
            root.path().join("chat-models"),
        );
        let chat_gate = chat.project_turn_gate_for_test("chat-session");
        let mut catalog = load_catalog(&root, Arc::new(MemoryMcpSecretStore::default()));
        let paired = catalog.pair("Claude Desktop").expect("pair test client");
        let external = ExternalMcpState::new(core.clone(), chat.external_mcp_components(), catalog);
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("bind managed MCP test listener");
        let endpoint = opentake_agent::mcp::server::bind_managed_gated_on(
            listener,
            external.components.dispatcher.clone(),
            external.components.registry.clone(),
            external.gate.clone(),
            external.authorizer.clone(),
        )
        .await
        .expect("start managed MCP endpoint");
        let client = reqwest::Client::new();
        let url = format!("http://{}/mcp", endpoint.addr());
        let session_a =
            initialize_managed_session(&client, &url, &paired.bearer_token, "rmcp-session-a").await;
        let session_b =
            initialize_managed_session(&client, &url, &paired.bearer_token, "rmcp-session-b").await;

        let created = call_managed_tool(
            &client,
            &url,
            &paired.bearer_token,
            &session_a,
            2,
            "create_folder",
            serde_json::json!({ "name": "MCP A" }),
        )
        .await;
        assert!(!created.contains("\"isError\":true"), "{created}");
        let foreign = call_managed_tool(
            &client,
            &url,
            &paired.bearer_token,
            &session_b,
            3,
            "undo",
            serde_json::json!({}),
        )
        .await;
        assert!(foreign.contains("\"isError\":true"), "{foreign}");
        let chat_undo = chat_gate
            .dispatch(
                &external.components.dispatcher,
                "undo",
                serde_json::json!({}),
            )
            .expect("current chat gate remains live");
        assert!(chat_undo.is_error, "chat consumed MCP A's edit");
        let owner = call_managed_tool(
            &client,
            &url,
            &paired.bearer_token,
            &session_a,
            4,
            "undo",
            serde_json::json!({}),
        )
        .await;
        assert!(!owner.contains("\"isError\":true"), "{owner}");
        assert!(core.media().folders.is_empty());

        let created = call_managed_tool(
            &client,
            &url,
            &paired.bearer_token,
            &session_b,
            5,
            "create_folder",
            serde_json::json!({ "name": "MCP B" }),
        )
        .await;
        assert!(!created.contains("\"isError\":true"), "{created}");
        let foreign = call_managed_tool(
            &client,
            &url,
            &paired.bearer_token,
            &session_a,
            6,
            "undo",
            serde_json::json!({}),
        )
        .await;
        assert!(foreign.contains("\"isError\":true"), "{foreign}");
        let chat_undo = chat_gate
            .dispatch(
                &external.components.dispatcher,
                "undo",
                serde_json::json!({}),
            )
            .expect("current chat gate remains live");
        assert!(chat_undo.is_error, "chat consumed MCP B's edit");
        let owner = call_managed_tool(
            &client,
            &url,
            &paired.bearer_token,
            &session_b,
            7,
            "undo",
            serde_json::json!({}),
        )
        .await;
        assert!(!owner.contains("\"isError\":true"), "{owner}");
        assert!(core.media().folders.is_empty());

        let created = chat_gate
            .dispatch(
                &external.components.dispatcher,
                "create_folder",
                serde_json::json!({ "name": "Chat" }),
            )
            .expect("current chat gate accepts mutation");
        assert!(!created.is_error, "{}", created.text_joined());
        for (session, id) in [(&session_a, 8), (&session_b, 9)] {
            let foreign = call_managed_tool(
                &client,
                &url,
                &paired.bearer_token,
                session,
                id,
                "undo",
                serde_json::json!({}),
            )
            .await;
            assert!(foreign.contains("\"isError\":true"), "{foreign}");
        }
        let chat_undo = chat_gate
            .dispatch(
                &external.components.dispatcher,
                "undo",
                serde_json::json!({}),
            )
            .expect("current chat gate accepts undo");
        assert!(!chat_undo.is_error, "{}", chat_undo.text_joined());
        assert!(core.media().folders.is_empty());

        endpoint.shutdown();
        endpoint.wait().await.expect("stop managed MCP endpoint");
    }

    #[test]
    fn catalog_pair_creates_unique_client_ids_and_32_byte_tokens() {
        let root = catalog_root();
        let secrets = Arc::new(MemoryMcpSecretStore::default());
        let mut catalog = load_catalog(&root, secrets);
        let first = catalog.pair("Claude Desktop").expect("pair first client");
        let second = catalog.pair("Cursor").expect("pair second client");
        assert_ne!(first.client.id, second.client.id);
        assert_ne!(first.client.token_digest, second.client.token_digest);
        for token in [&first.bearer_token, &second.bearer_token] {
            assert_eq!(token.len(), 64, "token is 32 bytes encoded as hexadecimal");
            assert!(token.bytes().all(|byte| byte.is_ascii_hexdigit()));
        }
        assert_eq!(first.endpoint, EXTERNAL_MCP_ENDPOINT);
        assert!(!format!("{first:?}").contains(&first.bearer_token));
    }

    #[test]
    fn catalog_persisted_json_omits_the_bearer_token() {
        let root = catalog_root();
        let secrets = Arc::new(MemoryMcpSecretStore::default());
        let mut catalog = load_catalog(&root, secrets);
        let receipt = catalog.pair("Claude Desktop").expect("pair client");
        let serialized = std::fs::read_to_string(catalog.metadata_path())
            .expect("read persisted client metadata");
        assert!(!serialized.contains(&receipt.bearer_token));
        assert!(!serialized.contains("bearer_token"));
        assert!(serialized.contains(&receipt.client.token_digest));
    }

    #[test]
    fn catalog_restart_reloads_metadata_and_retrieves_secret_from_fake_store() {
        let root = catalog_root();
        let secrets = Arc::new(MemoryMcpSecretStore::default());
        let receipt = {
            let mut catalog = load_catalog(&root, secrets.clone());
            catalog.pair("Claude Desktop").expect("pair client")
        };
        let catalog = load_catalog(&root, secrets);
        assert_eq!(catalog.clients(), &[receipt.client.clone()]);
        assert_eq!(
            catalog
                .verify_candidate(&receipt.bearer_token)
                .expect("verify stored secret"),
            Some(receipt.client.id)
        );
    }

    #[test]
    fn catalog_regeneration_invalidates_the_previous_token() {
        let root = catalog_root();
        let secrets = Arc::new(MemoryMcpSecretStore::default());
        let mut catalog = load_catalog(&root, secrets);
        let first = catalog.pair("Claude Desktop").expect("pair client");
        let regenerated = catalog
            .regenerate(&first.client.id)
            .expect("regenerate credential");
        assert_eq!(regenerated.client.id, first.client.id);
        assert_ne!(regenerated.client.token_digest, first.client.token_digest);
        assert_eq!(
            catalog
                .verify_candidate(&first.bearer_token)
                .expect("reject prior credential"),
            None
        );
        assert_eq!(
            catalog
                .verify_candidate(&regenerated.bearer_token)
                .expect("verify regenerated credential"),
            Some(first.client.id)
        );
    }

    #[test]
    fn catalog_revoke_removes_the_secret() {
        let root = catalog_root();
        let secrets = Arc::new(MemoryMcpSecretStore::default());
        let mut catalog = load_catalog(&root, secrets.clone());
        let receipt = catalog.pair("Claude Desktop").expect("pair client");
        catalog.revoke(&receipt.client.id).expect("revoke client");
        assert_eq!(
            catalog
                .verify_candidate(&receipt.bearer_token)
                .expect("reject revoked credential"),
            None
        );
        assert_eq!(
            secrets
                .load_mcp_secret(&secret_account(&receipt.client.id))
                .expect("read in-memory secret"),
            None
        );
        assert!(catalog.clients()[0].revoked_at.is_some());
    }

    #[test]
    fn catalog_duplicate_display_names_remain_distinguishable() {
        let root = catalog_root();
        let secrets = Arc::new(MemoryMcpSecretStore::default());
        let mut catalog = load_catalog(&root, secrets);
        let first = catalog.pair("Claude Desktop").expect("pair first client");
        let second = catalog.pair("Claude Desktop").expect("pair second client");
        assert_eq!(first.client.name, second.client.name);
        assert_ne!(first.client.id, second.client.id);
        assert_eq!(catalog.clients().len(), 2);
    }

    #[test]
    fn catalog_failed_atomic_rename_leaves_the_previous_catalog_readable() {
        let root = catalog_root();
        let secrets = Arc::new(MemoryMcpSecretStore::default());
        let mut catalog = load_catalog(&root, secrets.clone());
        let first = catalog.pair("Claude Desktop").expect("pair first client");
        catalog.fail_next_atomic_rename_for_test();
        assert!(catalog.pair("Cursor").is_err());
        let reloaded = load_catalog(&root, secrets.clone());
        assert_eq!(reloaded.clients(), &[first.client]);
    }

    #[test]
    fn catalog_recovers_after_the_post_rename_parent_sync_fails() {
        let root = catalog_root();
        let secrets = Arc::new(MemoryMcpSecretStore::default());
        let mut catalog = load_catalog(&root, secrets.clone());

        catalog.fail_parent_sync_during_publish_for_test();
        let receipt = match catalog.pair("Claude Desktop") {
            Ok(_) => panic!("report sync failure"),
            Err(error) => error,
        };

        let reloaded = load_catalog(&root, secrets.clone());
        assert_eq!(reloaded.clients().len(), 1);
        assert!(receipt.contains("sync external MCP catalog directory"));
        assert!(reloaded
            .verify_candidate("not-the-stored-token")
            .expect("catalog reconciles before authorization")
            .is_none());
        let stored = secrets
            .load_mcp_secret(&secret_account(&reloaded.clients()[0].id))
            .expect("read recovered secret")
            .expect("recovered secret exists");
        assert_eq!(token_digest(&stored), reloaded.clients()[0].token_digest);
    }
}
