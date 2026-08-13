#![allow(dead_code)] // Task 1 establishes the catalog before listener/UI tasks consume it.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::secret::McpSecretStore;

const CATALOG_VERSION: u32 = 1;
const CATALOG_DIRECTORY: &str = "external-mcp";
const CATALOG_FILE: &str = "clients.json";
const PENDING_FILE: &str = "clients.pending.json";
const EXTERNAL_MCP_ENDPOINT: &str = "http://127.0.0.1:19789/mcp";
const MAX_CLIENT_NAME_CHARS: usize = 128;
const TOKEN_BYTES: usize = 32;
const TOKEN_DIGEST_HEX_CHARS: usize = 12;

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
    #[cfg(test)]
    fail_next_rename: std::sync::atomic::AtomicBool,
    #[cfg(test)]
    fail_parent_sync_on_call: std::sync::atomic::AtomicUsize,
}

impl ExternalMcpCatalog {
    pub(crate) fn load(
        app_data_dir: &Path,
        secrets: Arc<dyn McpSecretStore>,
    ) -> Result<Self, String> {
        let root = app_data_dir.join(CATALOG_DIRECTORY);
        let path = root.join(CATALOG_FILE);
        let clients = read_catalog(&path)?;
        let mut catalog = Self {
            root,
            clients,
            secrets,
            pending: false,
            #[cfg(test)]
            fail_next_rename: std::sync::atomic::AtomicBool::new(false),
            #[cfg(test)]
            fail_parent_sync_on_call: std::sync::atomic::AtomicUsize::new(0),
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
        self.write_json_atomically(
            &self.metadata_path(),
            &PersistedCatalog {
                version: CATALOG_VERSION,
                clients: clients.to_vec(),
            },
        )
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::secret::{McpSecretStore, MemoryMcpSecretStore};

    fn catalog_root() -> tempfile::TempDir {
        tempfile::tempdir().expect("create temporary application data directory")
    }

    fn load_catalog(
        root: &tempfile::TempDir,
        secrets: Arc<MemoryMcpSecretStore>,
    ) -> ExternalMcpCatalog {
        ExternalMcpCatalog::load(root.path(), secrets)
            .expect("load catalog against the in-memory secret store")
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
