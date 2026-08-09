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
const TOKEN_BACKEND_ACCOUNT: &str = "auth-token-backend";
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
    previous_credential: Option<BoundCredential>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BoundCredential {
    token: String,
    backend_url: String,
}

/// Stored, backend-bound credential for managed generation. The tuple remains
/// inside the Rust process and is never returned by a Tauri command.
pub(crate) fn generation_credential() -> Result<Option<(String, String)>, String> {
    let store = keyring_store();
    let backend = load_backend_url(&store)?;
    Ok(load_bound_credential(&store, backend.as_deref())?
        .map(|credential| (credential.backend_url, credential.token)))
}

pub(crate) fn configured_backend_url() -> Result<Option<String>, String> {
    load_backend_url(&keyring_store())
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

fn load_bound_credential(
    store: &dyn KeyStore,
    backend_url: Option<&str>,
) -> Result<Option<BoundCredential>, String> {
    let token = store
        .load(TOKEN_ACCOUNT)
        .map_err(|error| error.to_string())?;
    let token_backend = store
        .load(TOKEN_BACKEND_ACCOUNT)
        .map_err(|error| error.to_string())?;
    let Some((token, token_backend, backend_url)) = token
        .zip(token_backend)
        .zip(backend_url.map(str::to_string))
        .map(|((token, token_backend), backend_url)| (token, token_backend, backend_url))
    else {
        return Ok(None);
    };
    let Ok(token_backend) = normalize_backend_url(&token_backend) else {
        return Ok(None);
    };
    Ok((token_backend == backend_url).then_some(BoundCredential { token, backend_url }))
}

fn clear_bound_credential(store: &dyn KeyStore) -> Result<(), String> {
    let token_error = store.delete(TOKEN_ACCOUNT).err();
    let backend_error = store.delete(TOKEN_BACKEND_ACCOUNT).err();
    match (token_error, backend_error) {
        (None, None) => Ok(()),
        (Some(error), None) | (None, Some(error)) => Err(error.to_string()),
        (Some(token_error), Some(backend_error)) => Err(format!(
            "Failed to clear account token ({token_error}) and origin binding ({backend_error})"
        )),
    }
}

fn save_bound_credential(store: &dyn KeyStore, credential: &BoundCredential) -> Result<(), String> {
    if let Err(error) = store.save(TOKEN_ACCOUNT, &credential.token) {
        let cleanup = clear_bound_credential(store);
        return Err(match cleanup {
            Ok(()) => error.to_string(),
            Err(cleanup_error) => format!(
                "Failed to save account token ({error}); credential cleanup also failed ({cleanup_error})"
            ),
        });
    }
    if let Err(error) = store.save(TOKEN_BACKEND_ACCOUNT, &credential.backend_url) {
        let cleanup = clear_bound_credential(store);
        return Err(match cleanup {
            Ok(()) => error.to_string(),
            Err(cleanup_error) => format!(
                "Failed to save account origin binding ({error}); credential cleanup also failed ({cleanup_error})"
            ),
        });
    }
    Ok(())
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
    let previous_credential = load_bound_credential(store, current.as_deref())?;
    if let Err(error) = clear_bound_credential(store) {
        runtime.status = AccountStatus::Error {
            message: error.clone(),
        };
        return Err(error);
    }

    let update_result = match normalized.as_deref() {
        Some(url) => store.save(BACKEND_URL_ACCOUNT, url),
        None => store.delete(BACKEND_URL_ACCOUNT),
    };
    if let Err(error) = update_result {
        // Keychain entries cannot be updated atomically. Restore credentials
        // only after the old backend is known to be back in place. If either
        // half fails, clear both halves so a restart cannot rebind the token.
        let backend_rollback = match current.as_deref() {
            Some(url) => store.save(BACKEND_URL_ACCOUNT, url),
            None => store.delete(BACKEND_URL_ACCOUNT),
        };
        let credential_rollback = if backend_rollback.is_ok() {
            previous_credential.as_ref().map_or(Ok(()), |credential| {
                save_bound_credential(store, credential)
            })
        } else {
            clear_bound_credential(store)
        };
        if backend_rollback.is_ok() && credential_rollback.is_ok() {
            runtime.status = if previous_credential.is_some()
                || !matches!(
                    previous_status,
                    AccountStatus::Online { .. } | AccountStatus::Stored
                ) {
                previous_status
            } else {
                AccountStatus::Offline
            };
            return Err(error.to_string());
        }
        let _ = clear_bound_credential(store);
        let message = "Account backend update failed and credential rollback was incomplete";
        runtime.status = AccountStatus::Error {
            message: message.to_string(),
        };
        return Err(format!("{error}; {message}"));
    }
    runtime.status = AccountStatus::Offline;
    Ok(())
}

fn stored_credential_status(store: &dyn KeyStore) -> Result<AccountStatus, String> {
    let backend = load_backend_url(store)?;
    if load_bound_credential(store, backend.as_deref())?.is_some() {
        return Ok(AccountStatus::Stored);
    }

    // A plaintext token without a matching normalized origin is ambiguous and
    // must never be paired with whichever backend happens to be configured at
    // restart. Clear both halves instead of silently adopting it.
    let has_token = store
        .load(TOKEN_ACCOUNT)
        .map_err(|error| error.to_string())?
        .is_some();
    let has_binding = store
        .load(TOKEN_BACKEND_ACCOUNT)
        .map_err(|error| error.to_string())?
        .is_some();
    if has_token || has_binding {
        clear_bound_credential(store)?;
    }
    Ok(AccountStatus::Offline)
}

fn begin_login(store: &dyn KeyStore, state: &AccountState) -> Result<LoginAttempt, String> {
    let mut runtime = state.lock();
    let backend_url = require_backend(load_backend_url(store)?)?;
    let previous_status = if matches!(runtime.status, AccountStatus::Connecting) {
        stored_credential_status(store)?
    } else {
        runtime.status.clone()
    };
    let previous_credential = load_bound_credential(store, Some(&backend_url))?;
    let generation = advance_generation(&mut runtime);
    runtime.status = AccountStatus::Connecting;
    Ok(LoginAttempt {
        backend_url,
        generation,
        previous_status,
        previous_credential,
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
    let credential = BoundCredential {
        token: token.to_string(),
        backend_url: attempt.backend_url.clone(),
    };
    if let Err(error) = save_bound_credential(store, &credential) {
        let restore_result = attempt
            .previous_credential
            .as_ref()
            .map_or(Ok(()), |previous| {
                clear_bound_credential(store)?;
                save_bound_credential(store, previous)
            });
        runtime.status = if restore_result.is_ok()
            && (attempt.previous_credential.is_some()
                || !matches!(
                    attempt.previous_status,
                    AccountStatus::Online { .. } | AccountStatus::Stored
                )) {
            attempt.previous_status
        } else {
            AccountStatus::Error {
                message: error.clone(),
            }
        };
        return Err(error);
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
    if let Err(error) = clear_bound_credential(store) {
        runtime.status = AccountStatus::Error {
            message: error.clone(),
        };
        return Err(error);
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
    use std::collections::VecDeque;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::Mutex;
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

    #[derive(Clone, Copy)]
    enum FailureTiming {
        BeforeApply,
        AfterApply,
    }

    struct SaveFailure {
        account: &'static str,
        timing: FailureTiming,
    }

    struct ScriptedSaveFailStore {
        inner: MemoryKeyStore,
        failures: Mutex<VecDeque<SaveFailure>>,
    }

    impl ScriptedSaveFailStore {
        fn new() -> Self {
            Self {
                inner: MemoryKeyStore::new(),
                failures: Mutex::new(VecDeque::new()),
            }
        }

        fn fail_save(&self, account: &'static str, timing: FailureTiming) {
            self.failures
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push_back(SaveFailure { account, timing });
        }

        fn assert_failures_consumed(&self) {
            let failures = self
                .failures
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            assert!(
                failures.is_empty(),
                "unconsumed scripted save failures: {}",
                failures.len()
            );
        }
    }

    impl KeyStore for ScriptedSaveFailStore {
        fn save(&self, account: &str, value: &str) -> Result<(), GenError> {
            let failure = {
                let mut failures = self
                    .failures
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                (failures
                    .front()
                    .is_some_and(|failure| failure.account == account))
                .then(|| failures.pop_front().expect("front exists"))
            };
            if let Some(failure) = failure {
                if matches!(failure.timing, FailureTiming::AfterApply) {
                    self.inner.save(account, value)?;
                }
                return Err(GenError::Transport(format!(
                    "injected save failure for {account}"
                )));
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
        save_bound_credential(
            &store,
            &BoundCredential {
                token: "old-token".into(),
                backend_url: "https://one.example.com".into(),
            },
        )
        .unwrap();
        state.set(AccountStatus::Error {
            message: "old state".into(),
        });

        set_backend_url(&store, &state, Some("https://two.example.com/".into())).unwrap();

        assert_eq!(store.load(TOKEN_ACCOUNT).unwrap(), None);
        assert_eq!(store.load(TOKEN_BACKEND_ACCOUNT).unwrap(), None);
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
        save_bound_credential(
            &store,
            &BoundCredential {
                token: "verified-token".into(),
                backend_url: "https://accounts.example.com".into(),
            },
        )
        .unwrap();
        state.set(AccountStatus::Connecting);

        set_backend_url(&store, &state, Some("https://accounts.example.com".into())).unwrap();

        assert_eq!(
            store.load(TOKEN_ACCOUNT).unwrap().as_deref(),
            Some("verified-token")
        );
        assert_eq!(
            store.load(TOKEN_BACKEND_ACCOUNT).unwrap().as_deref(),
            Some("https://accounts.example.com")
        );
        assert_eq!(state.get(), AccountStatus::Connecting);
    }

    #[test]
    fn logout_is_local_and_idempotent() {
        let store = MemoryKeyStore::new();
        let state = AccountState::default();
        store.save(TOKEN_ACCOUNT, "verified-token").unwrap();
        store
            .save(TOKEN_BACKEND_ACCOUNT, "https://accounts.example.com")
            .unwrap();
        state.set(AccountStatus::Connecting);

        logout(&store, &state).unwrap();
        logout(&store, &state).unwrap();

        assert_eq!(store.load(TOKEN_ACCOUNT).unwrap(), None);
        assert_eq!(store.load(TOKEN_BACKEND_ACCOUNT).unwrap(), None);
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
        assert_eq!(
            store.load(TOKEN_BACKEND_ACCOUNT).unwrap().as_deref(),
            Some("https://accounts.example.com")
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
        save_bound_credential(
            &store,
            &BoundCredential {
                token: "old-token".into(),
                backend_url: "https://accounts.example.com".into(),
            },
        )
        .unwrap();
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
        assert_eq!(
            store.load(TOKEN_BACKEND_ACCOUNT).unwrap().as_deref(),
            Some("https://accounts.example.com")
        );
        assert_eq!(state.get(), old_status);
        assert_eq!(
            get_status(&store, &AccountState::default()).unwrap(),
            AccountStatus::Stored
        );
    }

    #[test]
    fn backend_save_failure_rolls_back_the_backend_token_pair() {
        let store = ScriptedSaveFailStore::new();
        let state = AccountState::default();
        set_backend_url(&store, &state, Some("https://one.example.com".into())).unwrap();
        save_bound_credential(
            &store,
            &BoundCredential {
                token: "old-token".into(),
                backend_url: "https://one.example.com".into(),
            },
        )
        .unwrap();
        state.set(AccountStatus::Connecting);
        store.fail_save(BACKEND_URL_ACCOUNT, FailureTiming::BeforeApply);

        assert!(set_backend_url(&store, &state, Some("https://two.example.com".into())).is_err());
        assert_eq!(
            load_backend_url(&store).unwrap().as_deref(),
            Some("https://one.example.com")
        );
        assert_eq!(
            store.load(TOKEN_ACCOUNT).unwrap().as_deref(),
            Some("old-token")
        );
        assert_eq!(
            store.load(TOKEN_BACKEND_ACCOUNT).unwrap().as_deref(),
            Some("https://one.example.com")
        );
        assert_eq!(state.get(), AccountStatus::Stored);
        store.assert_failures_consumed();
    }

    #[test]
    fn incomplete_backend_rollback_clears_credentials_for_a_fresh_runtime() {
        let store = ScriptedSaveFailStore::new();
        let state = AccountState::default();
        set_backend_url(&store, &state, Some("https://one.example.com".into())).unwrap();
        save_bound_credential(
            &store,
            &BoundCredential {
                token: "old-token".into(),
                backend_url: "https://one.example.com".into(),
            },
        )
        .unwrap();
        state.set(AccountStatus::Connecting);
        store.fail_save(BACKEND_URL_ACCOUNT, FailureTiming::AfterApply);
        store.fail_save(BACKEND_URL_ACCOUNT, FailureTiming::BeforeApply);

        let error =
            set_backend_url(&store, &state, Some("https://two.example.com".into())).unwrap_err();

        assert!(error.contains("credential rollback was incomplete"));
        assert_eq!(
            store.inner.load(BACKEND_URL_ACCOUNT).unwrap().as_deref(),
            Some("https://two.example.com")
        );
        assert_eq!(store.load(TOKEN_ACCOUNT).unwrap(), None);
        assert_eq!(store.load(TOKEN_BACKEND_ACCOUNT).unwrap(), None);
        assert!(matches!(state.get(), AccountStatus::Error { .. }));
        let fresh_store = store.inner.clone();
        let fresh_state = AccountState::default();
        assert_eq!(
            get_status(&fresh_store, &fresh_state).unwrap(),
            AccountStatus::Offline
        );
        store.assert_failures_consumed();
    }

    #[test]
    fn partial_credential_restore_never_survives_as_stored() {
        for (account, timing) in [
            (TOKEN_ACCOUNT, FailureTiming::AfterApply),
            (TOKEN_BACKEND_ACCOUNT, FailureTiming::BeforeApply),
        ] {
            let store = ScriptedSaveFailStore::new();
            let state = AccountState::default();
            set_backend_url(&store, &state, Some("https://one.example.com".into())).unwrap();
            save_bound_credential(
                &store,
                &BoundCredential {
                    token: "old-token".into(),
                    backend_url: "https://one.example.com".into(),
                },
            )
            .unwrap();
            state.set(AccountStatus::Stored);
            store.fail_save(BACKEND_URL_ACCOUNT, FailureTiming::BeforeApply);
            store.fail_save(account, timing);

            let error = set_backend_url(&store, &state, Some("https://two.example.com".into()))
                .unwrap_err();

            assert!(error.contains("credential rollback was incomplete"));
            assert_eq!(store.inner.load(TOKEN_ACCOUNT).unwrap(), None);
            assert_eq!(store.inner.load(TOKEN_BACKEND_ACCOUNT).unwrap(), None);
            assert_eq!(
                get_status(&store.inner.clone(), &AccountState::default()).unwrap(),
                AccountStatus::Offline
            );
            store.assert_failures_consumed();
        }
    }

    #[test]
    fn fresh_runtime_exposes_and_can_forget_a_stored_credential() {
        let store = MemoryKeyStore::new();
        store
            .save(BACKEND_URL_ACCOUNT, "https://accounts.example.com")
            .unwrap();
        save_bound_credential(
            &store,
            &BoundCredential {
                token: "verified-token".into(),
                backend_url: "https://accounts.example.com".into(),
            },
        )
        .unwrap();
        let state = AccountState::default();

        assert_eq!(get_status(&store, &state).unwrap(), AccountStatus::Stored);
        logout(&store, &state).unwrap();
        assert_eq!(get_status(&store, &state).unwrap(), AccountStatus::Offline);
        assert_eq!(store.load(TOKEN_ACCOUNT).unwrap(), None);
        assert_eq!(store.load(TOKEN_BACKEND_ACCOUNT).unwrap(), None);
    }

    #[test]
    fn fresh_runtime_rejects_unbound_or_mismatched_tokens() {
        for binding in [None, Some("https://other.example.com")] {
            let store = MemoryKeyStore::new();
            store
                .save(BACKEND_URL_ACCOUNT, "https://accounts.example.com")
                .unwrap();
            store.save(TOKEN_ACCOUNT, "ambiguous-token").unwrap();
            if let Some(binding) = binding {
                store.save(TOKEN_BACKEND_ACCOUNT, binding).unwrap();
            }

            assert_eq!(
                get_status(&store, &AccountState::default()).unwrap(),
                AccountStatus::Offline
            );
            assert_eq!(store.load(TOKEN_ACCOUNT).unwrap(), None);
            assert_eq!(store.load(TOKEN_BACKEND_ACCOUNT).unwrap(), None);
        }
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
