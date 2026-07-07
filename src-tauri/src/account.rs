//! Account scaffold: configurable-backend login panel (HANDOFF §3.8).
//!
//! OpenTake ships **no official backend**. These commands let a user point the
//! app at a self-hosted backend (Settings → Account), verify a token against
//! it, and store that token in the OS keychain — the same BYOK pattern
//! [`secret.rs`](crate::secret) uses for provider API keys. Nothing here gates
//! local editing: when no backend is configured (or login fails) every
//! editing / export / Agent feature keeps working unchanged.
//!
//! `backend_url` and the `token` both live in the keychain under a dedicated
//! service (`opentake-account`) so they never touch JS memory, localStorage,
//! or the settings store. Only the live `AccountStatus` is held in managed
//! state — it is informational, not authoritative.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::State;

use opentake_gen::{KeyStore, KeyringStore};

/// Keychain service for account entries. Separate from BYOK's
/// `io.opentake.app` so account values live in their own namespace.
const ACCOUNT_SERVICE: &str = "opentake-account";
const BACKEND_URL_ACCOUNT: &str = "backend-url";
const TOKEN_ACCOUNT: &str = "auth-token";

/// Honest, verbose error surfaced by every backend-requiring command when no
/// backend URL is configured. Replicated verbatim across commands so the UI can
/// show it unchanged — the "no official backend" disclaimer must be visible,
/// not buried in a generic "login failed".
const NO_BACKEND_MSG: &str =
    "No backend configured. OpenTake has no official backend; set a custom backend URL in Settings.";

/// One verified account session (mirror of the backend `/api/auth/verify`
/// response). All fields but `userId` are optional so a minimal backend that
/// only returns `{"userId":"..."}` still works. Multi-word fields are
/// `camelCase` on the wire (the repo's #1 IPC bug class is a non-camelCase
/// DTO field that silently fails to round-trip).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    pub user_id: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub plan: Option<String>,
}

/// Live account state. `Offline` is the cold-start default; `Connecting` is
/// set the moment a verify request leaves and cleared when it resolves.
/// Tagged `camelCase` so the front end can switch on `type`.
///
/// (The HANDOFF brief wrote `Online(Error)` — read as `Online` carrying the
/// verified `AccountInfo`; an error variant is already covered by `Error`.)
#[derive(Debug, Clone, Serialize, Default)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AccountStatus {
    #[default]
    Offline,
    Connecting,
    Online {
        #[serde(default)]
        info: Option<AccountInfo>,
    },
    Error {
        message: String,
    },
}

/// Managed state holding the current status across commands. The mutex is only
/// held for the brief clone-on-read / swap-on-write; status is small.
#[derive(Default)]
pub struct AccountState {
    status: std::sync::Mutex<AccountStatus>,
}

impl AccountState {
    fn get(&self) -> AccountStatus {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }
    fn set(&self, status: AccountStatus) {
        if let Ok(mut s) = self.status.lock() {
            *s = status;
        }
    }
}

fn store() -> KeyringStore {
    KeyringStore::with_service(ACCOUNT_SERVICE)
}

/// Surface the "no backend configured" error verbatim when a backend-requiring
/// command runs with no URL. Synchronous so it can be unit-tested without a
/// tokio runtime.
fn require_backend(backend_url: Option<String>) -> Result<String, String> {
    backend_url.ok_or_else(|| NO_BACKEND_MSG.to_string())
}

/// `account_set_backend_url`: persist the backend URL to the keychain. `None`
/// (or empty/whitespace) clears it. Never touches the network — setting a URL
/// is a local preference, not a connection attempt.
#[tauri::command]
pub fn account_set_backend_url(url: Option<String>) -> Result<(), String> {
    let s = store();
    let trimmed = url.as_deref().map(str::trim).filter(|u| !u.is_empty());
    match trimmed {
        Some(u) => s.save(BACKEND_URL_ACCOUNT, u).map_err(|e| e.to_string()),
        None => s.delete(BACKEND_URL_ACCOUNT).map_err(|e| e.to_string()),
    }
}

/// `account_get_backend_url`: read the configured backend URL (or `None`).
#[tauri::command]
pub fn account_get_backend_url() -> Result<Option<String>, String> {
    store().load(BACKEND_URL_ACCOUNT).map_err(|e| e.to_string())
}

/// `account_login`: verify `token` against `{backend_url}/api/auth/verify`
/// (10s timeout). On success the token is persisted to the keychain and the
/// status moves to `Online`; on failure it moves to `Error`. Rejects with
/// [`NO_BACKEND_MSG`] when no backend URL is configured.
#[tauri::command]
pub async fn account_login(
    state: State<'_, AccountState>,
    token: String,
) -> Result<AccountInfo, String> {
    let backend_url = store()
        .load(BACKEND_URL_ACCOUNT)
        .map_err(|e| e.to_string())
        .and_then(require_backend)?;
    let token = token.trim().to_string();
    if token.is_empty() {
        return Err("Token is empty".to_string());
    }
    state.set(AccountStatus::Connecting);
    match verify_token(&backend_url, &token).await {
        Ok(info) => {
            store()
                .save(TOKEN_ACCOUNT, &token)
                .map_err(|e| e.to_string())?;
            state.set(AccountStatus::Online {
                info: Some(info.clone()),
            });
            Ok(info)
        }
        Err(msg) => {
            state.set(AccountStatus::Error {
                message: msg.clone(),
            });
            Err(msg)
        }
    }
}

/// `account_logout`: drop the stored token and return to `Offline`. Local
/// only — does not call the backend.
#[tauri::command]
pub fn account_logout(state: State<'_, AccountState>) -> Result<(), String> {
    store().delete(TOKEN_ACCOUNT).map_err(|e| e.to_string())?;
    state.set(AccountStatus::Offline);
    Ok(())
}

/// `account_get_status`: current live status (Offline / Connecting / Online /
/// Error). Cold start is `Offline`; the app does not auto-verify on launch.
#[tauri::command]
pub fn account_get_status(state: State<'_, AccountState>) -> Result<AccountStatus, String> {
    Ok(state.get())
}

/// POST the token to `{backend}/api/auth/verify` with a `Bearer` header and
/// deserialize the JSON body into [`AccountInfo`]. Any non-2xx / transport /
/// decode failure becomes a displayable `Err(String)`.
async fn verify_token(backend_url: &str, token: &str) -> Result<AccountInfo, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP client build failed: {e}"))?;
    let url = format!("{}/api/auth/verify", backend_url.trim_end_matches('/'));
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(|e| format!("Verify request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!(
            "Backend rejected token (HTTP {}).",
            resp.status().as_u16()
        ));
    }
    resp.json::<AccountInfo>()
        .await
        .map_err(|e| format!("Invalid verify response: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_offline_serializes_camelcase_tag() {
        let s = serde_json::to_string(&AccountStatus::Offline).unwrap();
        assert_eq!(s, r#"{"type":"offline"}"#);
    }

    #[test]
    fn status_connecting_serializes_camelcase_tag() {
        let s = serde_json::to_string(&AccountStatus::Connecting).unwrap();
        assert_eq!(s, r#"{"type":"connecting"}"#);
    }

    #[test]
    fn status_error_serializes_message_camelcase() {
        let s = serde_json::to_string(&AccountStatus::Error {
            message: "boom".into(),
        })
        .unwrap();
        assert_eq!(s, r#"{"type":"error","message":"boom"}"#);
    }

    #[test]
    fn status_online_serializes_info_camelcase() {
        let info = AccountInfo {
            user_id: "u1".into(),
            email: Some("a@b".into()),
            plan: None,
        };
        let s = serde_json::to_string(&AccountStatus::Online { info: Some(info) }).unwrap();
        assert!(s.contains(r#""type":"online""#), "tagged online: {s}");
        assert!(s.contains(r#""userId":"u1""#), "camelCase userId: {s}");
        assert!(s.contains(r#""email":"a@b""#), "camelCase email: {s}");
    }

    #[test]
    fn account_info_deserializes_minimal_camelcase() {
        // A minimal backend that only returns {"userId":"..."} must still
        // deserialize — email/plan default to None.
        let i: AccountInfo = serde_json::from_str(r#"{"userId":"u1"}"#).unwrap();
        assert_eq!(i.user_id, "u1");
        assert_eq!(i.email, None);
        assert_eq!(i.plan, None);
    }

    #[test]
    fn account_info_deserializes_full_camelcase() {
        let i: AccountInfo =
            serde_json::from_str(r#"{"userId":"u1","email":"a@b","plan":"pro"}"#).unwrap();
        assert_eq!(i.user_id, "u1");
        assert_eq!(i.email.as_deref(), Some("a@b"));
        assert_eq!(i.plan.as_deref(), Some("pro"));
    }

    #[test]
    fn require_backend_surfaces_no_backend_msg_when_none() {
        // The honest "no official backend" disclaimer must surface verbatim —
        // not a generic "login failed" — so the UI can show it as-is.
        assert_eq!(require_backend(None).unwrap_err(), NO_BACKEND_MSG);
    }

    #[test]
    fn require_backend_passes_through_when_set() {
        assert_eq!(
            require_backend(Some("https://my.backend".into())).unwrap(),
            "https://my.backend"
        );
    }

    #[test]
    fn account_state_default_is_offline_and_swap_round_trips() {
        // Cold start is Offline; set/get swaps are visible to the next read.
        let state = AccountState::default();
        assert!(matches!(state.get(), AccountStatus::Offline));
        state.set(AccountStatus::Error {
            message: "x".into(),
        });
        match state.get() {
            AccountStatus::Error { message } => assert_eq!(message, "x"),
            other => panic!("expected Error, got {other:?}"),
        }
    }
}
