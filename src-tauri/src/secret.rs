//! Secure BYOK API-key storage commands.
//!
//! Thin `#[tauri::command]` wrappers over `opentake-gen`'s cross-platform
//! [`KeyringStore`] (macOS Keychain / Windows Credential Manager / Linux Secret
//! Service). The plaintext key crosses the boundary in **one** direction only —
//! the WebView sends it on `secret_save`. It is never returned to the front end:
//! `secret_load` yields a *masked* representation, replicating the upstream
//! `AgentPane.mask` (`AgentPane.swift:131-134`). The key therefore lives solely
//! in the OS keychain and the Rust backend — never in JS memory, the settings
//! store, or `localStorage`.

use serde::Serialize;

use opentake_gen::{KeyStore, KeyringStore};

/// Masked status of a provider's stored key. `has_key` drives the UI; `masked`
/// is the bullet-masked form (empty when there is no key).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretStatus {
    has_key: bool,
    masked: String,
}

/// Allowed BYOK chat providers → stable keychain account strings, following the
/// `<prefix>-api-key` convention from [`opentake_gen::keys`]. Validating the
/// provider here means an unknown value can never address an arbitrary keychain
/// entry — the only writable accounts are the three the UI offers.
fn account_for(provider: &str) -> Result<&'static str, String> {
    match provider {
        "anthropic" => Ok("anthropic-api-key"),
        "openai" => Ok("openai-api-key"),
        "google" => Ok("google-api-key"),
        other => Err(format!("unknown provider: {other}")),
    }
}

/// Bullet mask replicating upstream `AgentPane.mask` (`AgentPane.swift:131-134`):
/// keys of length ≤ 4 show 32 bullets; otherwise 36 bullets plus the last 4
/// characters, so the user can recognise the key without it being recoverable.
fn mask(key: &str) -> String {
    const BULLET: &str = "\u{2022}";
    let chars: Vec<char> = key.chars().collect();
    if chars.len() <= 4 {
        BULLET.repeat(32)
    } else {
        let suffix: String = chars[chars.len() - 4..].iter().collect();
        format!("{}{suffix}", BULLET.repeat(36))
    }
}

/// Load the current key for `account` and return its masked status.
fn status_for(account: &str) -> Result<SecretStatus, String> {
    match KeyringStore::new()
        .load(account)
        .map_err(|e| e.to_string())?
    {
        Some(key) => Ok(SecretStatus {
            has_key: true,
            masked: mask(&key),
        }),
        None => Ok(SecretStatus {
            has_key: false,
            masked: String::new(),
        }),
    }
}

/// `secret_save`: persist a provider's API key to the OS keychain. The key is
/// trimmed; an empty key is rejected rather than stored. Returns the new masked
/// status so the front end never has to round-trip the plaintext back.
#[tauri::command]
pub fn secret_save(provider: String, key: String) -> Result<SecretStatus, String> {
    let account = account_for(&provider)?;
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err("API key is empty".to_string());
    }
    KeyringStore::new()
        .save(account, trimmed)
        .map_err(|e| e.to_string())?;
    status_for(account)
}

/// `secret_load`: masked status for a provider (never the plaintext key).
#[tauri::command]
pub fn secret_load(provider: String) -> Result<SecretStatus, String> {
    status_for(account_for(&provider)?)
}

/// `secret_delete`: remove a provider's key from the keychain. Deleting an
/// absent key is a no-op (treated as success). Returns the now-empty status.
#[tauri::command]
pub fn secret_delete(provider: String) -> Result<SecretStatus, String> {
    let account = account_for(&provider)?;
    KeyringStore::new()
        .delete(account)
        .map_err(|e| e.to_string())?;
    status_for(account)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_mapping_is_stable_for_known_providers() {
        assert_eq!(account_for("anthropic").unwrap(), "anthropic-api-key");
        assert_eq!(account_for("openai").unwrap(), "openai-api-key");
        assert_eq!(account_for("google").unwrap(), "google-api-key");
    }

    #[test]
    fn unknown_provider_is_rejected() {
        let err = account_for("evil").unwrap_err();
        assert!(err.contains("unknown provider"));
    }

    #[test]
    fn short_keys_are_fully_masked() {
        // length ≤ 4 → 32 bullets, nothing of the key revealed.
        let masked = mask("ab");
        assert_eq!(masked.chars().count(), 32);
        assert!(masked.chars().all(|c| c == '\u{2022}'));
        assert_eq!(mask("abcd").chars().count(), 32);
    }

    #[test]
    fn long_keys_reveal_only_last_four() {
        let masked = mask("sk-ant-secret-1234");
        // 36 bullets + 4 revealed chars.
        assert_eq!(masked.chars().count(), 40);
        assert!(masked.ends_with("1234"));
        assert!(!masked.contains("secret"));
    }

    #[test]
    fn mask_counts_unicode_by_char_not_byte() {
        // A 5-codepoint key (mix of multibyte) is "long": 36 bullets + last 4.
        let masked = mask("é🔑abc");
        assert_eq!(masked.chars().count(), 40);
        assert!(masked.ends_with("🔑abc"));
    }
}
