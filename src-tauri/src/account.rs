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
const SUPERSEDED_MSG: &str = "Account login was superseded by a newer account action";
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
    /// A verified credential exists in the keychain, but this process has not
    /// made a network request to restore the live account identity.
    Stored,
    Connecting,
    Online {
        info: AccountInfo,
    },
    Error {
        message: String,
    },
}

#[derive(Default)]
struct AccountRuntime {
    status: AccountStatus,
    generation: u64,
}

#[derive(Default)]
pub struct AccountState {
    runtime: std::sync::Mutex<AccountRuntime>,
}

impl AccountState {
    #[cfg(test)]
    fn get(&self) -> AccountStatus {
        self.runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .status
            .clone()
    }

    #[cfg(test)]
    fn set(&self, status: AccountStatus) {
        self.runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .status = status;
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, AccountRuntime> {
        self.runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[derive(Debug)]
struct LoginAttempt {
    backend_url: String,
    generation: u64,
    previous_status: AccountStatus,
}

fn advance_generation(runtime: &mut AccountRuntime) -> u64 {
    runtime.generation = runtime.generation.wrapping_add(1);
    runtime.generation
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
    let mut runtime = state.lock();
    let current = load_backend_url(store)?;
    if current.as_deref() == normalized.as_deref() {
        return Ok(());
    }

    // Invalidate every in-flight login before touching credentials. The runtime
    // lock also makes the delete + origin update one transition relative to
    // login completion and logout.
    advance_generation(&mut runtime);
    let previous_status = if matches!(runtime.status, AccountStatus::Connecting) {
        stored_credential_status(store)?
    } else {
        runtime.status.clone()
    };
    let previous_token = store
        .load(TOKEN_ACCOUNT)
        .map_err(|error| error.to_string())?;
    if let Err(error) = store.delete(TOKEN_ACCOUNT) {
        runtime.status = AccountStatus::Error {
            message: error.to_string(),
        };
        return Err(error.to_string());
    }

    let update_result = match normalized.as_deref() {
        Some(url) => store.save(BACKEND_URL_ACCOUNT, url),
        None => store.delete(BACKEND_URL_ACCOUNT),
    };
    if let Err(error) = update_result {
        // Keychain entries cannot be updated atomically. Best-effort rollback
        // keeps the old backend/token pair together; if rollback itself fails,
        // publish an explicit error instead of pretending the old session lives.
        let backend_rollback = match current.as_deref() {
            Some(url) => store.save(BACKEND_URL_ACCOUNT, url),
            None => store.delete(BACKEND_URL_ACCOUNT),
        };
        let token_rollback = match previous_token.as_deref() {
            Some(token) => store.save(TOKEN_ACCOUNT, token),
            None => Ok(()),
        };
        if backend_rollback.is_ok() && token_rollback.is_ok() {
            runtime.status = previous_status;
        } else {
            runtime.status = AccountStatus::Error {
                message: "Account backend update failed and credential rollback was incomplete"
                    .to_string(),
            };
        }
        return Err(error.to_string());
    }
    runtime.status = AccountStatus::Offline;
    Ok(())
}

fn stored_credential_status(store: &dyn KeyStore) -> Result<AccountStatus, String> {
    let has_backend = load_backend_url(store)?.is_some();
    let has_token = store
        .load(TOKEN_ACCOUNT)
        .map_err(|error| error.to_string())?
        .is_some();
    Ok(if has_backend && has_token {
        AccountStatus::Stored
    } else {
        AccountStatus::Offline
    })
}

fn begin_login(store: &dyn KeyStore, state: &AccountState) -> Result<LoginAttempt, String> {
    let mut runtime = state.lock();
    let backend_url = require_backend(load_backend_url(store)?)?;
    let previous_status = if matches!(runtime.status, AccountStatus::Connecting) {
        stored_credential_status(store)?
    } else {
        runtime.status.clone()
    };
    let generation = advance_generation(&mut runtime);
    runtime.status = AccountStatus::Connecting;
    Ok(LoginAttempt {
        backend_url,
        generation,
        previous_status,
    })
}

fn finish_login_success(
    store: &dyn KeyStore,
    state: &AccountState,
    attempt: LoginAttempt,
    token: &str,
    info: AccountInfo,
) -> Result<AccountInfo, String> {
    let mut runtime = state.lock();
    if runtime.generation != attempt.generation {
        return Err(SUPERSEDED_MSG.to_string());
    }
    let backend_matches = match load_backend_url(store) {
        Ok(current) => current.as_deref() == Some(attempt.backend_url.as_str()),
        Err(message) => {
            runtime.status = attempt.previous_status;
            return Err(message);
        }
    };
    if !backend_matches {
        runtime.status = stored_credential_status(store)?;
        return Err(SUPERSEDED_MSG.to_string());
    }
    if let Err(error) = store.save(TOKEN_ACCOUNT, token) {
        runtime.status = attempt.previous_status;
        return Err(error.to_string());
    }
    runtime.status = AccountStatus::Online { info: info.clone() };
    Ok(info)
}

fn finish_login_failure(state: &AccountState, attempt: LoginAttempt, message: String) -> String {
    let mut runtime = state.lock();
    if runtime.generation != attempt.generation {
        return SUPERSEDED_MSG.to_string();
    }
    runtime.status = match attempt.previous_status {
        status @ (AccountStatus::Online { .. } | AccountStatus::Stored) => status,
        _ => AccountStatus::Error {
            message: message.clone(),
        },
    };
    message
}

#[tauri::command]
pub fn account_set_backend_url(
    state: State<'_, AccountState>,
    url: Option<String>,
) -> Result<(), String> {
    set_backend_url(&keyring_store(), &state, url)
}

#[tauri::command]
pub fn account_get_backend_url(state: State<'_, AccountState>) -> Result<Option<String>, String> {
    let _runtime = state.lock();
    load_backend_url(&keyring_store())
}

#[tauri::command]
pub async fn account_login(
    state: State<'_, AccountState>,
    token: String,
) -> Result<AccountInfo, String> {
    let token = token.trim().to_string();
    if token.is_empty() {
        return Err("Token is empty".to_string());
    }

    let store = keyring_store();
    let attempt = begin_login(&store, &state)?;
    match verify_token(&attempt.backend_url, &token).await {
        Ok(info) => finish_login_success(&store, &state, attempt, &token, info),
        Err(message) => Err(finish_login_failure(&state, attempt, message)),
    }
}

fn logout(store: &dyn KeyStore, state: &AccountState) -> Result<(), String> {
    let mut runtime = state.lock();
    advance_generation(&mut runtime);
    if let Err(error) = store.delete(TOKEN_ACCOUNT) {
        runtime.status = AccountStatus::Error {
            message: error.to_string(),
        };
        return Err(error.to_string());
    }
    runtime.status = AccountStatus::Offline;
    Ok(())
}

#[tauri::command]
pub fn account_logout(state: State<'_, AccountState>) -> Result<(), String> {
    logout(&keyring_store(), &state)
}

#[tauri::command]
pub fn account_get_status(state: State<'_, AccountState>) -> Result<AccountStatus, String> {
    get_status(&keyring_store(), &state)
}

fn get_status(store: &dyn KeyStore, state: &AccountState) -> Result<AccountStatus, String> {
    let mut runtime = state.lock();
    if matches!(
        runtime.status,
        AccountStatus::Offline | AccountStatus::Stored
    ) {
        runtime.status = stored_credential_status(store)?;
    }
    Ok(runtime.status.clone())
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
    let mut response = client
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
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("Verify response read failed: {error}"))?
    {
        let next_len = body
            .len()
            .checked_add(chunk.len())
            .ok_or_else(|| "Backend verify response is too large".to_string())?;
        if next_len as u64 > MAX_VERIFY_RESPONSE_BYTES {
            return Err("Backend verify response is too large".to_string());
        }
        body.extend_from_slice(&chunk);
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
    use opentake_gen::{GenError, MemoryKeyStore};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;

    fn serve_once(response: impl Into<String>) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let backend_url = format!("http://{}", listener.local_addr().unwrap());
        let response = response.into();
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
            let _ = stream.write_all(response.as_bytes());
            String::from_utf8(request).unwrap()
        });
        (backend_url, handle)
    }

    fn chunked_ok(body: &str) -> String {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:x}\r\n{}\r\n0\r\n\r\n",
            body.len(),
            body
        )
    }

    struct BackendSaveFailStore {
        inner: MemoryKeyStore,
        fail_next_backend_save: AtomicBool,
    }

    impl BackendSaveFailStore {
        fn new() -> Self {
            Self {
                inner: MemoryKeyStore::new(),
                fail_next_backend_save: AtomicBool::new(false),
            }
        }
    }

    impl KeyStore for BackendSaveFailStore {
        fn save(&self, account: &str, value: &str) -> Result<(), GenError> {
            if account == BACKEND_URL_ACCOUNT
                && self.fail_next_backend_save.swap(false, Ordering::SeqCst)
            {
                return Err(GenError::Transport("injected backend save failure".into()));
            }
            self.inner.save(account, value)
        }

        fn load(&self, account: &str) -> Result<Option<String>, GenError> {
            self.inner.load(account)
        }

        fn delete(&self, account: &str) -> Result<(), GenError> {
            self.inner.delete(account)
        }
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
            serde_json::to_string(&AccountStatus::Stored).unwrap(),
            r#"{"type":"stored"}"#
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
    fn stale_login_cannot_resurrect_after_backend_switch_or_logout() {
        let store = MemoryKeyStore::new();
        let state = AccountState::default();
        set_backend_url(&store, &state, Some("https://one.example.com".into())).unwrap();

        let switched = begin_login(&store, &state).unwrap();
        set_backend_url(&store, &state, Some("https://two.example.com".into())).unwrap();
        let error = finish_login_success(
            &store,
            &state,
            switched,
            "token-one",
            AccountInfo {
                user_id: "one".into(),
                email: None,
                plan: None,
            },
        )
        .unwrap_err();
        assert_eq!(error, SUPERSEDED_MSG);
        assert_eq!(store.load(TOKEN_ACCOUNT).unwrap(), None);
        assert_eq!(state.get(), AccountStatus::Offline);

        let logged_out = begin_login(&store, &state).unwrap();
        logout(&store, &state).unwrap();
        assert_eq!(
            finish_login_success(
                &store,
                &state,
                logged_out,
                "token-two",
                AccountInfo {
                    user_id: "two".into(),
                    email: None,
                    plan: None,
                },
            )
            .unwrap_err(),
            SUPERSEDED_MSG
        );
        assert_eq!(store.load(TOKEN_ACCOUNT).unwrap(), None);
        assert_eq!(state.get(), AccountStatus::Offline);
    }

    #[test]
    fn newest_started_login_wins_regardless_of_completion_order() {
        let store = MemoryKeyStore::new();
        let state = AccountState::default();
        set_backend_url(&store, &state, Some("https://accounts.example.com".into())).unwrap();

        let older = begin_login(&store, &state).unwrap();
        let newer = begin_login(&store, &state).unwrap();
        let newer_info = AccountInfo {
            user_id: "newer".into(),
            email: None,
            plan: None,
        };
        finish_login_success(&store, &state, newer, "newer-token", newer_info.clone()).unwrap();
        assert_eq!(
            finish_login_success(
                &store,
                &state,
                older,
                "older-token",
                AccountInfo {
                    user_id: "older".into(),
                    email: None,
                    plan: None,
                },
            )
            .unwrap_err(),
            SUPERSEDED_MSG
        );
        assert_eq!(
            store.load(TOKEN_ACCOUNT).unwrap().as_deref(),
            Some("newer-token")
        );
        assert_eq!(state.get(), AccountStatus::Online { info: newer_info });

        logout(&store, &state).unwrap();
        let first = begin_login(&store, &state).unwrap();
        let second = begin_login(&store, &state).unwrap();
        assert_eq!(
            finish_login_success(
                &store,
                &state,
                first,
                "first-token",
                AccountInfo {
                    user_id: "first".into(),
                    email: None,
                    plan: None,
                },
            )
            .unwrap_err(),
            SUPERSEDED_MSG
        );
        finish_login_success(
            &store,
            &state,
            second,
            "second-token",
            AccountInfo {
                user_id: "second".into(),
                email: None,
                plan: None,
            },
        )
        .unwrap();
        assert_eq!(
            store.load(TOKEN_ACCOUNT).unwrap().as_deref(),
            Some("second-token")
        );
    }

    #[test]
    fn failed_relogin_preserves_the_previous_verified_session() {
        let store = MemoryKeyStore::new();
        let state = AccountState::default();
        set_backend_url(&store, &state, Some("https://accounts.example.com".into())).unwrap();
        store.save(TOKEN_ACCOUNT, "old-token").unwrap();
        let old_status = AccountStatus::Online {
            info: AccountInfo {
                user_id: "old-user".into(),
                email: None,
                plan: None,
            },
        };
        state.set(old_status.clone());

        let attempt = begin_login(&store, &state).unwrap();
        assert_eq!(
            finish_login_failure(&state, attempt, "verification failed".into()),
            "verification failed"
        );

        assert_eq!(
            store.load(TOKEN_ACCOUNT).unwrap().as_deref(),
            Some("old-token")
        );
        assert_eq!(state.get(), old_status);
    }

    #[test]
    fn backend_save_failure_rolls_back_the_backend_token_pair() {
        let store = BackendSaveFailStore::new();
        let state = AccountState::default();
        set_backend_url(&store, &state, Some("https://one.example.com".into())).unwrap();
        store.save(TOKEN_ACCOUNT, "old-token").unwrap();
        state.set(AccountStatus::Connecting);
        store.fail_next_backend_save.store(true, Ordering::SeqCst);

        assert!(set_backend_url(&store, &state, Some("https://two.example.com".into())).is_err());
        assert_eq!(
            load_backend_url(&store).unwrap().as_deref(),
            Some("https://one.example.com")
        );
        assert_eq!(
            store.load(TOKEN_ACCOUNT).unwrap().as_deref(),
            Some("old-token")
        );
        assert_eq!(state.get(), AccountStatus::Stored);
    }

    #[test]
    fn fresh_runtime_exposes_and_can_forget_a_stored_credential() {
        let store = MemoryKeyStore::new();
        store
            .save(BACKEND_URL_ACCOUNT, "https://accounts.example.com")
            .unwrap();
        store.save(TOKEN_ACCOUNT, "verified-token").unwrap();
        let state = AccountState::default();

        assert_eq!(get_status(&store, &state).unwrap(), AccountStatus::Stored);
        logout(&store, &state).unwrap();
        assert_eq!(get_status(&store, &state).unwrap(), AccountStatus::Offline);
        assert_eq!(store.load(TOKEN_ACCOUNT).unwrap(), None);
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

    #[test]
    fn verify_accepts_a_chunked_body_without_content_length() {
        let body = r#"{"userId":"chunked-user"}"#;
        let (backend_url, server) = serve_once(chunked_ok(body));

        let info = tauri::async_runtime::block_on(verify_token(&backend_url, "secret-token"))
            .expect("bounded chunked response should verify");
        server.join().unwrap();

        assert_eq!(info.user_id, "chunked-user");
    }

    #[test]
    fn verify_accepts_a_chunked_body_exactly_at_the_limit() {
        let prefix = r#"{"userId":"limit-user","padding":""#;
        let suffix = r#""}"#;
        let padding_len = MAX_VERIFY_RESPONSE_BYTES as usize - prefix.len() - suffix.len();
        let body = format!("{prefix}{}{suffix}", "x".repeat(padding_len));
        assert_eq!(body.len(), MAX_VERIFY_RESPONSE_BYTES as usize);
        let (backend_url, server) = serve_once(chunked_ok(&body));

        let info = tauri::async_runtime::block_on(verify_token(&backend_url, "secret-token"))
            .expect("a response exactly at the byte limit should verify");
        server.join().unwrap();

        assert_eq!(info.user_id, "limit-user");
    }

    #[test]
    fn verify_rejects_chunked_body_above_the_limit() {
        let body = "x".repeat(MAX_VERIFY_RESPONSE_BYTES as usize + 1);
        let (backend_url, server) = serve_once(chunked_ok(&body));

        let error =
            tauri::async_runtime::block_on(verify_token(&backend_url, "secret-token")).unwrap_err();
        server.join().unwrap();

        assert_eq!(error, "Backend verify response is too large");
    }
}
