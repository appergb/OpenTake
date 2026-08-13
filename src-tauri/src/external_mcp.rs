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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExternalMcpPairingReceipt {
    pub(crate) client: ExternalMcpClientSummary,
    pub(crate) endpoint: String,
    pub(crate) bearer_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExternalMcpCredential {
    pub(crate) client_id: String,
    pub(crate) bearer_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct PersistedCatalog {
    version: u32,
    clients: Vec<ExternalMcpClientSummary>,
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
    #[cfg(test)]
    fail_next_rename: std::sync::atomic::AtomicBool,
    #[cfg(test)]
    fail_next_parent_sync: std::sync::atomic::AtomicBool,
}

impl ExternalMcpCatalog {
    pub(crate) fn load(
        app_data_dir: &Path,
        secrets: Arc<dyn McpSecretStore>,
    ) -> Result<Self, String> {
        let root = app_data_dir.join(CATALOG_DIRECTORY);
        let path = root.join(CATALOG_FILE);
        let clients = match fs::read(&path) {
            Ok(bytes) => {
                let persisted: PersistedCatalog = serde_json::from_slice(&bytes)
                    .map_err(|error| format!("read external MCP catalog: {error}"))?;
                if persisted.version != CATALOG_VERSION {
                    return Err("unsupported external MCP catalog version".to_string());
                }
                for client in &persisted.clients {
                    validate_client(client)?;
                }
                persisted.clients
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(format!("read external MCP catalog: {error}")),
        };
        Ok(Self {
            root,
            clients,
            secrets,
            #[cfg(test)]
            fail_next_rename: std::sync::atomic::AtomicBool::new(false),
            #[cfg(test)]
            fail_next_parent_sync: std::sync::atomic::AtomicBool::new(false),
        })
    }

    pub(crate) fn clients(&self) -> &[ExternalMcpClientSummary] {
        &self.clients
    }

    pub(crate) fn metadata_path(&self) -> PathBuf {
        self.root.join(CATALOG_FILE)
    }

    pub(crate) fn pair(&mut self, name: &str) -> Result<ExternalMcpPairingReceipt, String> {
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
        self.secrets.save_mcp_secret(&account, &token)?;
        self.clients.push(client.clone());
        if let Err(error) = self.persist() {
            self.clients.pop();
            if let Err(rollback_error) = self.secrets.delete_mcp_secret(&account) {
                return Err(format!(
                    "{error}; external MCP credential cleanup failed: {rollback_error}"
                ));
            }
            return Err(error);
        }
        Ok(receipt(client, token))
    }

    pub(crate) fn regenerate(
        &mut self,
        client_id: &str,
    ) -> Result<ExternalMcpPairingReceipt, String> {
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
        self.secrets.save_mcp_secret(&account, &token)?;
        let previous_client = self.clients[index].clone();
        self.clients[index].token_digest = token_digest(&token);
        if let Err(error) = self.persist() {
            self.clients[index] = previous_client;
            if let Err(rollback_error) = self.secrets.save_mcp_secret(&account, &previous_token) {
                return Err(format!(
                    "{error}; external MCP credential rollback failed: {rollback_error}"
                ));
            }
            return Err(error);
        }
        Ok(receipt(self.clients[index].clone(), token))
    }

    pub(crate) fn revoke(&mut self, client_id: &str) -> Result<(), String> {
        let index = self.client_index(client_id)?;
        if self.clients[index].revoked_at.is_some() {
            return Ok(());
        }
        let previous_client = self.clients[index].clone();
        let account = secret_account(client_id);
        let previous_token = self.secrets.load_mcp_secret(&account)?;
        self.secrets.delete_mcp_secret(&account)?;
        self.clients[index].revoked_at = Some(unix_timestamp()?);
        if let Err(error) = self.persist() {
            self.clients[index] = previous_client;
            if let Some(token) = previous_token {
                if let Err(rollback_error) = self.secrets.save_mcp_secret(&account, &token) {
                    return Err(format!(
                        "{error}; external MCP credential rollback failed: {rollback_error}"
                    ));
                }
            }
            return Err(error);
        }
        Ok(())
    }

    pub(crate) fn active_credentials(&self) -> Result<Vec<ExternalMcpCredential>, String> {
        self.clients
            .iter()
            .filter(|client| client.revoked_at.is_none())
            .map(|client| {
                let bearer_token = self
                    .secrets
                    .load_mcp_secret(&secret_account(&client.id))?
                    .ok_or_else(|| "external MCP client credential is unavailable".to_string())?;
                if bearer_token.len() != TOKEN_BYTES * 2
                    || !bearer_token.bytes().all(|byte| byte.is_ascii_hexdigit())
                    || token_digest(&bearer_token) != client.token_digest
                {
                    return Err("external MCP client credential is invalid".to_string());
                }
                Ok(ExternalMcpCredential {
                    client_id: client.id.clone(),
                    bearer_token,
                })
            })
            .collect()
    }

    fn client_index(&self, client_id: &str) -> Result<usize, String> {
        self.clients
            .iter()
            .position(|client| client.id == client_id)
            .ok_or_else(|| "external MCP client not found".to_string())
    }

    fn persist(&self) -> Result<(), String> {
        fs::create_dir_all(&self.root)
            .map_err(|error| format!("create external MCP catalog directory: {error}"))?;
        let bytes = serde_json::to_vec_pretty(&PersistedCatalog {
            version: CATALOG_VERSION,
            clients: self.clients.clone(),
        })
        .map_err(|error| format!("encode external MCP catalog: {error}"))?;
        let temp = self
            .root
            .join(format!(".clients.{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)
                .map_err(|error| format!("create external MCP catalog staging file: {error}"))?;
            file.write_all(&bytes)
                .and_then(|()| file.sync_all())
                .map_err(|error| format!("write external MCP catalog staging file: {error}"))?;
            self.rename_atomically(&temp, &self.metadata_path())?;
            // A successful rename has already made the new catalog authoritative.
            // Do not roll back metadata/keychain state if the best-effort parent
            // durability barrier fails after that point.
            let _ = self.sync_parent_directory();
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
    fn fail_next_parent_sync_for_test(&self) {
        self.fail_next_parent_sync
            .store(true, std::sync::atomic::Ordering::SeqCst);
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
        if self
            .fail_next_parent_sync
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            return Err(std::io::Error::other("injected parent sync failure"));
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
        assert_ne!(first.bearer_token, second.bearer_token);
        for token in [&first.bearer_token, &second.bearer_token] {
            assert_eq!(token.len(), 64, "token is 32 bytes encoded as hexadecimal");
            assert!(token.bytes().all(|byte| byte.is_ascii_hexdigit()));
        }
        assert_eq!(first.endpoint, EXTERNAL_MCP_ENDPOINT);
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
        let credentials = catalog
            .active_credentials()
            .expect("load active credentials");
        assert_eq!(catalog.clients(), &[receipt.client.clone()]);
        assert_eq!(credentials.len(), 1);
        assert_eq!(credentials[0].client_id, receipt.client.id);
        assert_eq!(credentials[0].bearer_token, receipt.bearer_token);
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
        let credentials = catalog
            .active_credentials()
            .expect("load active credentials");
        assert_eq!(regenerated.client.id, first.client.id);
        assert_ne!(regenerated.bearer_token, first.bearer_token);
        assert_ne!(regenerated.client.token_digest, first.client.token_digest);
        assert_eq!(credentials[0].bearer_token, regenerated.bearer_token);
        assert_ne!(credentials[0].bearer_token, first.bearer_token);
    }

    #[test]
    fn catalog_revoke_removes_the_secret() {
        let root = catalog_root();
        let secrets = Arc::new(MemoryMcpSecretStore::default());
        let mut catalog = load_catalog(&root, secrets.clone());
        let receipt = catalog.pair("Claude Desktop").expect("pair client");
        catalog.revoke(&receipt.client.id).expect("revoke client");
        assert!(catalog
            .active_credentials()
            .expect("load active credentials")
            .is_empty());
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
        let reloaded = load_catalog(&root, secrets);
        assert_eq!(reloaded.clients(), &[first.client]);
    }

    #[test]
    fn catalog_persists_after_the_post_rename_parent_sync_fails() {
        let root = catalog_root();
        let secrets = Arc::new(MemoryMcpSecretStore::default());
        let mut catalog = load_catalog(&root, secrets.clone());

        catalog.fail_next_parent_sync_for_test();
        let receipt = catalog.pair("Claude Desktop").expect("pair client");

        let reloaded = load_catalog(&root, secrets);
        assert_eq!(reloaded.clients(), &[receipt.client]);
        assert_eq!(
            reloaded
                .active_credentials()
                .expect("load active credentials")[0]
                .bearer_token,
            receipt.bearer_token
        );
    }
}
