//! Optional account login against a user-configured backend.
//!
//! OpenTake has no official account service. Local editing, export, BYOK, chat,
//! and MCP features remain available when this module is unconfigured or
//! offline. The backend URL and verified token are stored in a dedicated OS
//! keychain namespace; the token is never returned to the WebView or logged.

use std::net::IpAddr;
use std::time::Duration;

use opentake_gen::{KeyStore, KeyringStore};
use reqwest::redirect::Policy;
use serde::{Deserialize, Serialize};
use tauri::State;

const ACCOUNT_SERVICE: &str = "opentake-account";
const BACKEND_URL_ACCOUNT: &str = "backend-url";
const TOKEN_ACCOUNT: &str = "auth-token";
const VERIFY_PATH: &str = "/api/auth/verify";
const MAX_VERIFY_RESPONSE_BYTES: u64 = 64 * 1024;
const NO_BACKEND_MSG: &str =
    "No backend configured. OpenTake has no official backend; set a custom backend URL in Settings.";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    pub user_id: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub plan: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AccountStatus {
    #[default]
    Offline,
    Connecting,
    Online {
        info: AccountInfo,
    },
    Error {
        message: String,
    },
}

#[derive(Default)]
pub struct AccountState {
    status: std::sync::Mutex<AccountStatus>,
}

impl AccountState {
    fn get(&self) -> AccountStatus {
        self.status
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn set(&self, status: AccountStatus) {
        *self
            .status
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = status;
    }
}

fn keyring_store() -> KeyringStore {
    KeyringStore::with_service(ACCOUNT_SERVICE)
}

fn is_loopback_host(url: &reqwest::Url) -> bool {
    match url.host_str() {
        Some("localhost") => true,
        Some(host) => host
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback()),
        None => false,
    }
}

/// Accept a root HTTPS URL, or root HTTP only for an explicit loopback host.
/// Keeping the base URL path-free makes the verify endpoint deterministic.
fn normalize_backend_url(raw: &str) -> Result<String, String> {
    let url = reqwest::Url::parse(raw.trim()).map_err(|_| "Backend URL is invalid".to_string())?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err("Backend URL must not contain credentials".to_string());
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err("Backend URL must not contain a query or fragment".to_string());
    }
    if url.path() != "/" {
        return Err("Backend URL must be an origin without a path".to_string());
    }
    match url.scheme() {
        "https" => {}
        "http" if is_loopback_host(&url) => {}
        "http" => {
            return Err("Remote account backends must use HTTPS".to_string());
        }
        _ => {
            return Err("Backend URL must use HTTPS (or HTTP on loopback)".to_string());
        }
    }
    Ok(url.as_str().trim_end_matches('/').to_string())
}

fn load_backend_url(store: &dyn KeyStore) -> Result<Option<String>, String> {
    store
        .load(BACKEND_URL_ACCOUNT)
        .map_err(|error| error.to_string())?
        .map(|raw| normalize_backend_url(&raw))
        .transpose()
}

fn require_backend(backend_url: Option<String>) -> Result<String, String> {
    backend_url.ok_or_else(|| NO_BACKEND_MSG.to_string())
}

fn set_backend_url(
    store: &dyn KeyStore,
    state: &AccountState,
    raw_url: Option<String>,
) -> Result<(), String> {
    let normalized = raw_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(normalize_backend_url)
        .transpose()?;
    let current = store
        .load(BACKEND_URL_ACCOUNT)
        .map_err(|error| error.to_string())?;
    if current.as_deref() == normalized.as_deref() {
        return Ok(());
    }

    // A token is scoped to one backend. Delete it before changing the origin so
    // credentials verified by one service can never become another's session.
    store
        .delete(TOKEN_ACCOUNT)
        .map_err(|error| error.to_string())?;
    match normalized {
        Some(url) => store
            .save(BACKEND_URL_ACCOUNT, &url)
            .map_err(|error| error.to_string())?,
        None => store
            .delete(BACKEND_URL_ACCOUNT)
            .map_err(|error| error.to_string())?,
    }
    state.set(AccountStatus::Offline);
    Ok(())
}

#[tauri::command]
pub fn account_set_backend_url(
    state: State<'_, AccountState>,
    url: Option<String>,
) -> Result<(), String> {
    set_backend_url(&keyring_store(), &state, url)
}

#[tauri::command]
pub fn account_get_backend_url() -> Result<Option<String>, String> {
    load_backend_url(&keyring_store())
}

#[tauri::command]
pub async fn account_login(
    state: State<'_, AccountState>,
    token: String,
) -> Result<AccountInfo, String> {
    let backend_url = require_backend(load_backend_url(&keyring_store())?)?;
    let token = token.trim();
    if token.is_empty() {
        return Err("Token is empty".to_string());
    }

    state.set(AccountStatus::Connecting);
    let result = async {
        let info = verify_token(&backend_url, token).await?;
        keyring_store()
            .save(TOKEN_ACCOUNT, token)
            .map_err(|error| error.to_string())?;
        Ok::<AccountInfo, String>(info)
    }
    .await;

    match result {
        Ok(info) => {
            state.set(AccountStatus::Online { info: info.clone() });
            Ok(info)
        }
        Err(message) => {
            state.set(AccountStatus::Error {
                message: message.clone(),
            });
            Err(message)
        }
    }
}

fn logout(store: &dyn KeyStore, state: &AccountState) -> Result<(), String> {
    store
        .delete(TOKEN_ACCOUNT)
        .map_err(|error| error.to_string())?;
    state.set(AccountStatus::Offline);
    Ok(())
}

#[tauri::command]
pub fn account_logout(state: State<'_, AccountState>) -> Result<(), String> {
    logout(&keyring_store(), &state)
}

#[tauri::command]
pub fn account_get_status(state: State<'_, AccountState>) -> AccountStatus {
    state.get()
}

fn verify_endpoint(backend_url: &str) -> Result<reqwest::Url, String> {
    let normalized = normalize_backend_url(backend_url)?;
    reqwest::Url::parse(&format!("{normalized}{VERIFY_PATH}"))
        .map_err(|_| "Backend verify URL is invalid".to_string())
}

async fn verify_token(backend_url: &str, token: &str) -> Result<AccountInfo, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(Policy::none())
        .build()
        .map_err(|error| format!("HTTP client build failed: {error}"))?;
    let response = client
        .post(verify_endpoint(backend_url)?)
        .bearer_auth(token)
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(|error| format!("Verify request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Backend rejected token (HTTP {}).",
            response.status().as_u16()
        ));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_VERIFY_RESPONSE_BYTES)
    {
        return Err("Backend verify response is too large".to_string());
    }
    let body = response
        .bytes()
        .await
        .map_err(|error| format!("Verify response read failed: {error}"))?;
    if body.len() as u64 > MAX_VERIFY_RESPONSE_BYTES {
        return Err("Backend verify response is too large".to_string());
    }
    let mut info: AccountInfo = serde_json::from_slice(&body)
        .map_err(|error| format!("Invalid verify response: {error}"))?;
    info.user_id = info.user_id.trim().to_string();
    if info.user_id.is_empty() {
        return Err("Invalid verify response: userId is empty".to_string());
    }
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_gen::MemoryKeyStore;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    fn serve_once(response: &'static str) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let backend_url = format!("http://{}", listener.local_addr().unwrap());
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0_u8; 4096];
            loop {
                let count = stream.read(&mut buffer).unwrap();
                if count == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..count]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            stream.write_all(response.as_bytes()).unwrap();
            String::from_utf8(request).unwrap()
        });
        (backend_url, handle)
    }

    #[test]
    fn status_dtos_use_the_expected_camel_case_contract() {
        assert_eq!(
            serde_json::to_string(&AccountStatus::Offline).unwrap(),
            r#"{"type":"offline"}"#
        );
        assert_eq!(
            serde_json::to_string(&AccountStatus::Connecting).unwrap(),
            r#"{"type":"connecting"}"#
        );
        assert_eq!(
            serde_json::to_string(&AccountStatus::Error {
                message: "boom".into(),
            })
            .unwrap(),
            r#"{"type":"error","message":"boom"}"#
        );
        assert_eq!(
            serde_json::to_string(&AccountStatus::Online {
                info: AccountInfo {
                    user_id: "u1".into(),
                    email: None,
                    plan: Some("pro".into()),
                },
            })
            .unwrap(),
            r#"{"type":"online","info":{"userId":"u1","email":null,"plan":"pro"}}"#
        );
    }

    #[test]
    fn account_info_accepts_the_minimum_backend_response() {
        let info: AccountInfo = serde_json::from_str(r#"{"userId":"u1"}"#).unwrap();
        assert_eq!(
            info,
            AccountInfo {
                user_id: "u1".into(),
                email: None,
                plan: None,
            }
        );
    }

    #[test]
    fn backend_url_accepts_https_and_loopback_http_origins() {
        assert_eq!(
            normalize_backend_url(" https://accounts.example.com/ ").unwrap(),
            "https://accounts.example.com"
        );
        assert_eq!(
            normalize_backend_url("http://127.0.0.1:8787").unwrap(),
            "http://127.0.0.1:8787"
        );
        assert_eq!(
            normalize_backend_url("http://[::1]:8787").unwrap(),
            "http://[::1]:8787"
        );
        assert_eq!(
            normalize_backend_url("http://localhost:8787").unwrap(),
            "http://localhost:8787"
        );
    }

    #[test]
    fn backend_url_rejects_unsafe_or_ambiguous_origins() {
        for raw in [
            "http://accounts.example.com",
            "ftp://accounts.example.com",
            "https://user:secret@accounts.example.com",
            "https://accounts.example.com/base",
            "https://accounts.example.com?tenant=one",
            "https://accounts.example.com#fragment",
            "not a url",
        ] {
            assert!(normalize_backend_url(raw).is_err(), "accepted {raw}");
        }
    }

    #[test]
    fn backend_change_clears_the_old_token_and_live_status() {
        let store = MemoryKeyStore::new();
        let state = AccountState::default();
        set_backend_url(&store, &state, Some("https://one.example.com".into())).unwrap();
        store.save(TOKEN_ACCOUNT, "old-token").unwrap();
        state.set(AccountStatus::Error {
            message: "old state".into(),
        });

        set_backend_url(&store, &state, Some("https://two.example.com/".into())).unwrap();

        assert_eq!(store.load(TOKEN_ACCOUNT).unwrap(), None);
        assert_eq!(
            store.load(BACKEND_URL_ACCOUNT).unwrap().as_deref(),
            Some("https://two.example.com")
        );
        assert_eq!(state.get(), AccountStatus::Offline);
    }

    #[test]
    fn setting_the_same_backend_preserves_the_verified_session() {
        let store = MemoryKeyStore::new();
        let state = AccountState::default();
        set_backend_url(&store, &state, Some("https://accounts.example.com".into())).unwrap();
        store.save(TOKEN_ACCOUNT, "verified-token").unwrap();
        state.set(AccountStatus::Connecting);

        set_backend_url(&store, &state, Some("https://accounts.example.com".into())).unwrap();

        assert_eq!(
            store.load(TOKEN_ACCOUNT).unwrap().as_deref(),
            Some("verified-token")
        );
        assert_eq!(state.get(), AccountStatus::Connecting);
    }

    #[test]
    fn logout_is_local_and_idempotent() {
        let store = MemoryKeyStore::new();
        let state = AccountState::default();
        store.save(TOKEN_ACCOUNT, "verified-token").unwrap();
        state.set(AccountStatus::Connecting);

        logout(&store, &state).unwrap();
        logout(&store, &state).unwrap();

        assert_eq!(store.load(TOKEN_ACCOUNT).unwrap(), None);
        assert_eq!(state.get(), AccountStatus::Offline);
    }

    #[test]
    fn no_backend_is_explicit_and_verify_path_is_fixed() {
        assert_eq!(require_backend(None).unwrap_err(), NO_BACKEND_MSG);
        assert_eq!(
            verify_endpoint("https://accounts.example.com")
                .unwrap()
                .as_str(),
            "https://accounts.example.com/api/auth/verify"
        );
    }

    #[test]
    fn verify_request_uses_the_fixed_path_and_bearer_contract() {
        let (backend_url, server) = serve_once(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 34\r\nConnection: close\r\n\r\n{\"userId\":\" user-1 \",\"plan\":\"pro\"}",
        );

        let info = tauri::async_runtime::block_on(verify_token(&backend_url, "secret-token"))
            .expect("loopback backend should verify");
        let request = server.join().unwrap();

        assert_eq!(info.user_id, "user-1");
        assert_eq!(info.plan.as_deref(), Some("pro"));
        assert!(
            request.starts_with("POST /api/auth/verify HTTP/1.1\r\n"),
            "unexpected request target: {request}"
        );
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer secret-token\r\n"),
            "missing bearer header"
        );
    }

    #[test]
    fn verify_request_does_not_follow_redirects_with_the_token() {
        let (backend_url, server) = serve_once(
            "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/stolen\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        );

        let error =
            tauri::async_runtime::block_on(verify_token(&backend_url, "secret-token")).unwrap_err();
        server.join().unwrap();

        assert_eq!(error, "Backend rejected token (HTTP 302).");
    }
}
