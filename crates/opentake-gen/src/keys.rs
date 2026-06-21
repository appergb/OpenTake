//! Provider API-key storage. Replicates the upstream Keychain model
//! (`KeychainStore.swift:4-52` + `AnthropicKeychain` in `AnthropicClient.swift:7-30`):
//! `service` = bundle id, `account` = a stable per-key string, values are
//! trimmed and empty is treated as absent, and (in debug builds) an environment
//! variable can override the stored value.
//!
//! Storage is abstracted behind `KeyStore` so tests use `MemoryKeyStore` and
//! never touch the real OS keychain.

use crate::error::GenError;

/// Service identifier under which keys are stored. Replicates upstream
/// `KeychainStore.service` (bundle id), rebranded for OpenTake.
pub const SERVICE: &str = "io.opentake.app";

/// The set of provider keys OpenTake manages under BYOK.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderKey {
    Fal,
    Replicate,
    OpenAI,
    ElevenLabs,
    Anthropic,
}

impl ProviderKey {
    /// Stable keychain account string (one per key). Replicates
    /// `AnthropicKeychain.account = "anthropic-api-key"`.
    pub fn account(self) -> &'static str {
        match self {
            ProviderKey::Fal => "fal-api-key",
            ProviderKey::Replicate => "replicate-api-key",
            ProviderKey::OpenAI => "openai-api-key",
            ProviderKey::ElevenLabs => "elevenlabs-api-key",
            ProviderKey::Anthropic => "anthropic-api-key",
        }
    }

    /// Environment variable consulted in debug builds (upstream `#if DEBUG`).
    pub fn env_var(self) -> &'static str {
        match self {
            ProviderKey::Fal => "FAL_KEY",
            ProviderKey::Replicate => "REPLICATE_API_TOKEN",
            ProviderKey::OpenAI => "OPENAI_API_KEY",
            ProviderKey::ElevenLabs => "ELEVENLABS_API_KEY",
            ProviderKey::Anthropic => "ANTHROPIC_API_KEY",
        }
    }

    /// The provider-routing prefix this key corresponds to (matches
    /// `ProviderAdapter::prefix`).
    pub fn prefix(self) -> &'static str {
        match self {
            ProviderKey::Fal => "fal",
            ProviderKey::Replicate => "replicate",
            ProviderKey::OpenAI => "openai",
            ProviderKey::ElevenLabs => "elevenlabs",
            ProviderKey::Anthropic => "anthropic",
        }
    }
}

/// Normalize a stored value: trim whitespace; empty becomes `None`.
/// Replicates `KeychainStore.load` trimming (`KeychainStore.swift:37-40`).
fn normalize(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

/// Secret storage backend. Implementors persist string secrets by account.
pub trait KeyStore: Send + Sync {
    fn save(&self, account: &str, value: &str) -> Result<(), GenError>;
    /// Return the stored value, normalized; `None` when absent or empty.
    fn load(&self, account: &str) -> Result<Option<String>, GenError>;
    fn delete(&self, account: &str) -> Result<(), GenError>;
}

impl dyn KeyStore {
    /// Convenience: load a `ProviderKey`, applying the debug env-var override
    /// first (upstream `AnthropicKeychain.load` `#if DEBUG` branch).
    pub fn load_key(&self, key: ProviderKey) -> Result<Option<String>, GenError> {
        #[cfg(debug_assertions)]
        if let Ok(v) = std::env::var(key.env_var()) {
            if let Some(val) = normalize(&v) {
                return Ok(Some(val));
            }
        }
        self.load(key.account())
    }

    pub fn save_key(&self, key: ProviderKey, value: &str) -> Result<(), GenError> {
        self.save(key.account(), value)
    }

    pub fn delete_key(&self, key: ProviderKey) -> Result<(), GenError> {
        self.delete(key.account())
    }
}

/// Production key store backed by the OS keychain (`keyring` crate). Cross-
/// platform: macOS Keychain / Windows Credential Manager / Linux Secret Service.
pub struct KeyringStore {
    service: String,
}

impl KeyringStore {
    pub fn new() -> Self {
        Self {
            service: SERVICE.to_string(),
        }
    }

    pub fn with_service(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }
}

impl Default for KeyringStore {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyStore for KeyringStore {
    fn save(&self, account: &str, value: &str) -> Result<(), GenError> {
        keyring::Entry::new(&self.service, account)?.set_password(value)?;
        Ok(())
    }

    fn load(&self, account: &str) -> Result<Option<String>, GenError> {
        match keyring::Entry::new(&self.service, account)?.get_password() {
            Ok(s) => Ok(normalize(&s)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn delete(&self, account: &str) -> Result<(), GenError> {
        match keyring::Entry::new(&self.service, account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

/// In-memory key store for tests. Never touches the OS keychain.
#[derive(Default, Clone)]
pub struct MemoryKeyStore {
    map: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, String>>>,
}

impl MemoryKeyStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Seed a provider key directly (test ergonomics).
    pub fn with_key(self, key: ProviderKey, value: &str) -> Self {
        self.map
            .lock()
            .unwrap()
            .insert(key.account().to_string(), value.to_string());
        self
    }
}

impl KeyStore for MemoryKeyStore {
    fn save(&self, account: &str, value: &str) -> Result<(), GenError> {
        self.map
            .lock()
            .unwrap()
            .insert(account.to_string(), value.to_string());
        Ok(())
    }

    fn load(&self, account: &str) -> Result<Option<String>, GenError> {
        Ok(self
            .map
            .lock()
            .unwrap()
            .get(account)
            .and_then(|v| normalize(v)))
    }

    fn delete(&self, account: &str) -> Result<(), GenError> {
        self.map.lock().unwrap().remove(account);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accounts_are_stable_and_distinct() {
        assert_eq!(ProviderKey::Fal.account(), "fal-api-key");
        assert_eq!(ProviderKey::Anthropic.account(), "anthropic-api-key");
        let all = [
            ProviderKey::Fal,
            ProviderKey::Replicate,
            ProviderKey::OpenAI,
            ProviderKey::ElevenLabs,
            ProviderKey::Anthropic,
        ];
        let mut accts: Vec<&str> = all.iter().map(|k| k.account()).collect();
        accts.sort_unstable();
        accts.dedup();
        assert_eq!(accts.len(), 5);
    }

    #[test]
    fn prefix_matches_provider() {
        assert_eq!(ProviderKey::Fal.prefix(), "fal");
        assert_eq!(ProviderKey::Replicate.prefix(), "replicate");
        assert_eq!(ProviderKey::OpenAI.prefix(), "openai");
        assert_eq!(ProviderKey::ElevenLabs.prefix(), "elevenlabs");
    }

    #[test]
    fn memory_store_round_trip() {
        let store = MemoryKeyStore::new();
        let dyn_store: &dyn KeyStore = &store;
        assert_eq!(dyn_store.load_key(ProviderKey::Fal).unwrap(), None);
        dyn_store.save_key(ProviderKey::Fal, "fal-secret").unwrap();
        assert_eq!(
            dyn_store.load_key(ProviderKey::Fal).unwrap().as_deref(),
            Some("fal-secret")
        );
        dyn_store.delete_key(ProviderKey::Fal).unwrap();
        assert_eq!(dyn_store.load_key(ProviderKey::Fal).unwrap(), None);
    }

    #[test]
    fn empty_and_whitespace_values_are_none() {
        let store = MemoryKeyStore::new();
        store.save(ProviderKey::OpenAI.account(), "   ").unwrap();
        let dyn_store: &dyn KeyStore = &store;
        assert_eq!(dyn_store.load_key(ProviderKey::OpenAI).unwrap(), None);
    }

    #[test]
    fn load_trims_surrounding_whitespace() {
        let store = MemoryKeyStore::new();
        store
            .save(ProviderKey::Replicate.account(), "  tok-123\n")
            .unwrap();
        let dyn_store: &dyn KeyStore = &store;
        assert_eq!(
            dyn_store.load_key(ProviderKey::Replicate).unwrap().as_deref(),
            Some("tok-123")
        );
    }

    // In debug builds, an env var overrides the stored value. Use a dedicated
    // var unlikely to collide; serialize via a unique key name.
    #[cfg(debug_assertions)]
    #[test]
    fn debug_env_var_overrides_store() {
        // ElevenLabs env var is ELEVENLABS_API_KEY; set it for this test only.
        let store = MemoryKeyStore::new().with_key(ProviderKey::ElevenLabs, "from-store");
        let dyn_store: &dyn KeyStore = &store;
        std::env::set_var("ELEVENLABS_API_KEY", "from-env");
        let got = dyn_store.load_key(ProviderKey::ElevenLabs).unwrap();
        std::env::remove_var("ELEVENLABS_API_KEY");
        assert_eq!(got.as_deref(), Some("from-env"));
    }
}
