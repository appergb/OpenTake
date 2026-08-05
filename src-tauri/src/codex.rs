//! Official Codex CLI integration.
//!
//! OpenTake never reads, copies, or stores Codex credentials. Authentication is
//! delegated to the user-installed official CLI (`codex login`), and Agent
//! turns run through `codex exec` using that CLI's existing ChatGPT session.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
#[cfg(all(test, unix))]
use std::process::Command;
use std::process::{ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime};

use base64::Engine as _;
use opentake_agent::chat::{ChatTurnGate, ToolCall};
use opentake_agent::mcp::dispatch::Dispatcher;
use opentake_agent::plugin::registry::PluginRegistry;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tauri::State;
use tokio::io::{
    AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader,
};

const MINIMUM_CODEX_VERSION: (u64, u64, u64) = (0, 146, 0);
const CODEX_TURN_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const CODEX_AUTH_TIMEOUT: Duration = Duration::from_secs(15);
const CODEX_LOGOUT_TIMEOUT: Duration = Duration::from_secs(20);
const CODEX_LOGIN_SESSION_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const CANCEL_POLL_INTERVAL: Duration = Duration::from_millis(50);
const MAX_JSONL_LINE_BYTES: usize = 1024 * 1024;
const MAX_STDOUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_STDERR_CAPTURE_BYTES: usize = 64 * 1024;
const MAX_PROBE_CAPTURE_BYTES: usize = 16 * 1024;
const MAX_FINAL_TEXT_BYTES: usize = 256 * 1024;
const MAX_TOOL_CALLS: usize = 512;
const CODEX_MCP_BEARER_ENV: &str = "OPENTAKE_CODEX_MCP_BEARER_TOKEN";
const CODEX_CLEANUP_RESERVE: Duration = Duration::from_secs(2);

#[derive(Clone)]
struct LoginController {
    id: u64,
    cancel: Arc<AtomicBool>,
    completion: Arc<LoginCompletion>,
}

#[derive(Default)]
struct LoginCompletion {
    done: AtomicBool,
    notify: tokio::sync::Notify,
}

impl LoginCompletion {
    fn finish(&self) {
        self.done.store(true, Ordering::Release);
        self.notify.notify_waiters();
    }

    async fn wait_until(&self, deadline: tokio::time::Instant) -> Result<(), String> {
        loop {
            let notified = self.notify.notified();
            if self.done.load(Ordering::Acquire) {
                return Ok(());
            }
            if tokio::time::timeout_at(deadline, notified).await.is_err() {
                return Err("Codex login cleanup timed out".to_string());
            }
        }
    }
}

#[derive(Default)]
pub struct CodexAuthState {
    login_process: Mutex<Option<LoginController>>,
    next_login_id: AtomicU64,
}

impl Drop for CodexAuthState {
    fn drop(&mut self) {
        if let Ok(process) = self.login_process.get_mut() {
            if let Some(controller) = process.take() {
                controller.cancel.store(true, Ordering::Release);
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ExecutableIdentity {
    canonical_path: PathBuf,
    byte_len: u64,
    modified: Option<SystemTime>,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
}

impl ExecutableIdentity {
    fn capture(path: &Path) -> Option<Self> {
        let canonical_path = std::fs::canonicalize(path).ok()?;
        let metadata = std::fs::metadata(path).ok()?;
        if !metadata.is_file() {
            return None;
        }
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;
        Some(Self {
            canonical_path,
            byte_len: metadata.len(),
            modified: metadata.modified().ok(),
            #[cfg(unix)]
            device: metadata.dev(),
            #[cfg(unix)]
            inode: metadata.ino(),
        })
    }

    fn is_current(&self, path: &Path) -> bool {
        Self::capture(path).as_ref() == Some(self)
    }
}

#[derive(Clone, Debug)]
struct VerifiedCodex {
    path: PathBuf,
    version: String,
    identity: ExecutableIdentity,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodexAuthStatus {
    pub available: bool,
    pub authenticated: bool,
    pub auth_method: Option<String>,
    pub version: Option<String>,
    pub login_in_progress: bool,
    pub message: String,
}

impl CodexAuthStatus {
    fn unavailable() -> Self {
        Self {
            available: false,
            authenticated: false,
            auth_method: None,
            version: None,
            login_in_progress: false,
            message: "Official Codex CLI was not found".into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodexTurnError {
    Cancelled,
    Unavailable,
    IncompatibleCli,
    NotAuthenticated,
    McpStart,
    StrictConfigRejected,
    Timeout,
    Protocol,
    ProviderFailed,
}

#[derive(Debug)]
pub struct CodexTurnOutput {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
}

pub(crate) struct CodexTurnContext {
    pub dispatcher: Arc<Dispatcher>,
    pub registry: Arc<RwLock<PluginRegistry>>,
    pub gate: Arc<dyn ChatTurnGate>,
    pub cancel: Arc<AtomicBool>,
}

/// Return candidate locations without trusting a packaged app's truncated PATH
/// alone. `OPENTAKE_CODEX` is an explicit administrator/developer override.
fn candidate_paths() -> Vec<PathBuf> {
    let executable = if cfg!(windows) { "codex.exe" } else { "codex" };
    let mut candidates = Vec::new();

    if let Some(explicit) = std::env::var_os("OPENTAKE_CODEX") {
        candidates.push(PathBuf::from(explicit));
    }
    if let Some(path) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&path).map(|dir| dir.join(executable)));
    }

    for dir in [
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/opt/local/bin",
        "/usr/bin",
    ] {
        candidates.push(PathBuf::from(dir).join(executable));
    }

    if let Some(home) = home_dir() {
        candidates.push(home.join(".local/bin").join(executable));
        candidates.push(home.join(".volta/bin").join(executable));
        candidates.push(home.join(".cargo/bin").join(executable));

        let nvm_nodes = home.join(".nvm/versions/node");
        if let Ok(entries) = std::fs::read_dir(nvm_nodes) {
            let mut versions = entries
                .flatten()
                .map(|entry| entry.path())
                .collect::<Vec<_>>();
            versions.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
            candidates.extend(
                versions
                    .into_iter()
                    .map(|version| version.join("bin").join(executable)),
            );
        }
    }

    #[cfg(windows)]
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        candidates.push(
            PathBuf::from(local_app_data)
                .join("Programs/OpenAI/Codex/bin")
                .join(executable),
        );
    }

    let mut seen = std::collections::HashSet::new();
    candidates.retain(|path| seen.insert(path.clone()));
    candidates
}

fn home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

fn supported_codex_version(version: &str) -> bool {
    let Some(raw) = version.strip_prefix("codex-cli ") else {
        return false;
    };
    let core = raw.split(['-', '+']).next().unwrap_or(raw);
    let mut parts = core.split('.');
    let parsed = (
        parts.next().and_then(|value| value.parse::<u64>().ok()),
        parts.next().and_then(|value| value.parse::<u64>().ok()),
        parts.next().and_then(|value| value.parse::<u64>().ok()),
    );
    if parts.next().is_some() {
        return false;
    }
    match parsed {
        (Some(major), Some(minor), Some(patch)) => (major, minor, patch) >= MINIMUM_CODEX_VERSION,
        _ => false,
    }
}

fn parsed_codex_version(stdout: &[u8]) -> Option<String> {
    let version = String::from_utf8_lossy(stdout).trim().to_string();
    version.starts_with("codex-cli ").then_some(version)
}

fn parse_login_status(text: &str) -> (bool, Option<String>) {
    let normalized = text.trim();
    let lower = normalized.to_ascii_lowercase();
    if !lower.contains("logged in") || lower.contains("not logged in") {
        return (false, None);
    }
    let reported_method = normalized
        .split_once("using")
        .map(|(_, value)| value.trim().trim_end_matches('.'));
    let method = match reported_method.map(str::to_ascii_lowercase).as_deref() {
        Some("chatgpt") => Some("ChatGPT".to_string()),
        Some("an api key" | "api key") => Some("API key".to_string()),
        _ => None,
    };
    (true, method)
}

fn redacted_url(raw: &str) -> String {
    let Ok(mut url) = reqwest::Url::parse(raw) else {
        return "[redacted-url]".to_string();
    };
    if url.set_username("").is_err() || url.set_password(None).is_err() {
        return "[redacted-url]".to_string();
    }
    url.set_query(None);
    url.set_fragment(None);
    url.to_string()
}

fn redacted_inline_bytes(encoded: &str) -> Value {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .unwrap_or_else(|_| encoded.as_bytes().to_vec());
    let digest = Sha256::digest(&bytes);
    serde_json::json!({
        "byteLength": bytes.len(),
        "sha256": format!("{digest:x}"),
        "redacted": true,
    })
}

fn redacted_tool_args(tool_name: &str, args: Value) -> Value {
    let leaf_name = tool_name.rsplit(['.', '/']).next().unwrap_or(tool_name);
    if leaf_name != "import_media" && !leaf_name.ends_with("__import_media") {
        return args;
    }
    let Value::Object(mut args) = args else {
        return serde_json::json!({ "redacted": true });
    };
    let Some(source) = args.get_mut("source") else {
        return Value::Object(args);
    };
    let Value::Object(source) = source else {
        *source = serde_json::json!({ "redacted": true });
        return Value::Object(args);
    };
    if let Some(url) = source.get_mut("url") {
        *url = match url.as_str() {
            Some(raw) => Value::String(redacted_url(raw)),
            None => Value::String("[redacted-url]".to_string()),
        };
    }
    if let Some(bytes) = source.get_mut("bytes") {
        *bytes = match bytes.as_str() {
            Some(encoded) => redacted_inline_bytes(encoded),
            None => serde_json::json!({ "redacted": true }),
        };
    }
    Value::Object(args)
}

fn login_is_running(state: &CodexAuthState) -> Result<bool, String> {
    let mut process = state.login_process.lock().map_err(|e| e.to_string())?;
    let Some(controller) = process.as_ref() else {
        return Ok(false);
    };
    if controller.completion.done.load(Ordering::Acquire) {
        process.take();
        Ok(false)
    } else {
        Ok(true)
    }
}

fn remove_login_controller(state: &CodexAuthState, id: u64) -> Result<(), String> {
    let mut process = state.login_process.lock().map_err(|e| e.to_string())?;
    if process
        .as_ref()
        .is_some_and(|controller| controller.id == id)
    {
        process.take();
    }
    Ok(())
}

async fn cancel_login_until(
    state: &CodexAuthState,
    deadline: tokio::time::Instant,
) -> Result<(), String> {
    let controller = state
        .login_process
        .lock()
        .map_err(|e| e.to_string())?
        .take();
    let Some(controller) = controller else {
        return Ok(());
    };
    controller.cancel.store(true, Ordering::Release);
    controller.completion.wait_until(deadline).await
}

fn auth_probe_error(context: &str, error: CodexTurnError) -> String {
    match error {
        CodexTurnError::Timeout => format!("{context} timed out"),
        CodexTurnError::Cancelled => format!("{context} was cancelled"),
        _ => format!("{context} failed"),
    }
}

async fn auth_status_until(
    state: &CodexAuthState,
    deadline: tokio::time::Instant,
) -> Result<(CodexAuthStatus, Option<VerifiedCodex>), String> {
    let login_in_progress = login_is_running(state)?;
    let cancel = AtomicBool::new(false);
    let Some(codex) = discover_codex_until(&cancel, deadline)
        .await
        .map_err(|error| auth_probe_error("Codex version check", error))?
    else {
        return Ok((CodexAuthStatus::unavailable(), None));
    };

    if login_in_progress {
        return Ok((
            CodexAuthStatus {
                available: true,
                authenticated: false,
                auth_method: None,
                version: Some(codex.version.clone()),
                login_in_progress: true,
                message: "Waiting for official Codex browser login".into(),
            },
            Some(codex),
        ));
    }

    let output = run_verified_probe(&codex, &["login", "status"], &cancel, deadline)
        .await
        .map_err(|error| auth_probe_error("Codex login status check", error))?;
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let (authenticated, auth_method) = parse_login_status(&combined);
    Ok((
        CodexAuthStatus {
            available: true,
            authenticated,
            auth_method,
            version: Some(codex.version.clone()),
            login_in_progress: false,
            message: if authenticated {
                "Codex is signed in".into()
            } else {
                "Codex is not signed in".into()
            },
        },
        Some(codex),
    ))
}

struct LoginCompletionGuard(Arc<LoginCompletion>);

impl Drop for LoginCompletionGuard {
    fn drop(&mut self) {
        self.0.finish();
    }
}

async fn run_login_process(
    codex: VerifiedCodex,
    cancel: Arc<AtomicBool>,
    ready: tokio::sync::oneshot::Sender<Result<(), String>>,
) {
    if !codex.identity.is_current(&codex.path) {
        let _ = ready.send(Err("Codex executable changed before login".to_string()));
        return;
    }
    let mut command = tokio::process::Command::new(&codex.path);
    command
        .arg("login")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    opentake_media::process_tree::configure_command(command.as_std_mut());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(_) => {
            let _ = ready.send(Err("could not start official Codex login".to_string()));
            return;
        }
    };
    let child_id = match child.id() {
        Some(child_id) => child_id,
        None => {
            let _ = child.start_kill();
            let _ = ready.send(Err("could not contain official Codex login".to_string()));
            return;
        }
    };
    let mut tree = match opentake_media::process_tree::ProcessTree::attach(child_id) {
        Ok(tree) => tree,
        Err(_) => {
            let _ = child.start_kill();
            let cleanup_deadline = tokio::time::Instant::now() + CODEX_CLEANUP_RESERVE;
            let _ = tokio::time::timeout_at(cleanup_deadline, child.wait()).await;
            let _ = ready.send(Err("could not contain official Codex login".to_string()));
            return;
        }
    };
    if ready.send(Ok(())).is_err() {
        let cleanup_deadline = tokio::time::Instant::now() + CODEX_CLEANUP_RESERVE;
        let _ = terminate_and_reap_until(&mut child, &mut tree, cleanup_deadline).await;
        return;
    }

    let session_deadline = tokio::time::Instant::now() + CODEX_LOGIN_SESSION_TIMEOUT;
    let cancellation = wait_for_cancel(cancel.as_ref());
    tokio::pin!(cancellation);
    let selected = tokio::select! {
        status = child.wait() => status.map_err(|_| CodexTurnError::ProviderFailed),
        _ = &mut cancellation => Err(CodexTurnError::Cancelled),
        _ = tokio::time::sleep_until(session_deadline) => Err(CodexTurnError::Timeout),
    };
    match selected {
        Ok(_) => {
            if tree.terminate().is_ok() {
                tree.disarm();
            }
        }
        Err(_) => {
            let cleanup_deadline = tokio::time::Instant::now() + CODEX_CLEANUP_RESERVE;
            let _ = terminate_and_reap_until(&mut child, &mut tree, cleanup_deadline).await;
        }
    }
}

async fn start_login_until(
    state: &CodexAuthState,
    codex: VerifiedCodex,
    deadline: tokio::time::Instant,
) -> Result<(), String> {
    let id = state.next_login_id.fetch_add(1, Ordering::AcqRel) + 1;
    let cancel = Arc::new(AtomicBool::new(false));
    let completion = Arc::new(LoginCompletion::default());
    let controller = LoginController {
        id,
        cancel: cancel.clone(),
        completion: completion.clone(),
    };
    {
        let mut process = state.login_process.lock().map_err(|e| e.to_string())?;
        if process
            .as_ref()
            .is_some_and(|active| !active.completion.done.load(Ordering::Acquire))
        {
            return Err("Codex login is already in progress".to_string());
        }
        *process = Some(controller.clone());
    }

    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let thread_completion = completion.clone();
    let spawn = std::thread::Builder::new()
        .name("codex-login".to_string())
        .spawn(move || {
            let _guard = LoginCompletionGuard(thread_completion);
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(_) => {
                    let _ = ready_tx.send(Err("could not start Codex login runtime".to_string()));
                    return;
                }
            };
            runtime.block_on(run_login_process(codex, cancel, ready_tx));
        });
    if spawn.is_err() {
        completion.finish();
        remove_login_controller(state, id)?;
        return Err("could not start Codex login worker".to_string());
    }

    match tokio::time::timeout_at(deadline, ready_rx).await {
        Ok(Ok(Ok(()))) => Ok(()),
        Ok(Ok(Err(error))) => {
            let _ = completion.wait_until(deadline).await;
            remove_login_controller(state, id)?;
            Err(error)
        }
        Ok(Err(_)) => {
            let _ = completion.wait_until(deadline).await;
            remove_login_controller(state, id)?;
            Err("Codex login worker stopped before startup".to_string())
        }
        Err(_) => {
            controller.cancel.store(true, Ordering::Release);
            Err("Codex login startup timed out".to_string())
        }
    }
}

#[tauri::command]
pub async fn codex_auth_status(
    state: State<'_, CodexAuthState>,
) -> Result<CodexAuthStatus, String> {
    let deadline = tokio::time::Instant::now() + CODEX_AUTH_TIMEOUT;
    auth_status_until(&state, deadline)
        .await
        .map(|(status, _)| status)
}

#[tauri::command]
pub async fn codex_login_start(
    state: State<'_, CodexAuthState>,
) -> Result<CodexAuthStatus, String> {
    let deadline = tokio::time::Instant::now() + CODEX_AUTH_TIMEOUT;
    let (current, codex) = auth_status_until(&state, deadline).await?;
    if current.authenticated || current.login_in_progress {
        return Ok(current);
    }
    let Some(codex) = codex else {
        return Ok(CodexAuthStatus::unavailable());
    };
    start_login_until(&state, codex.clone(), deadline).await?;
    Ok(CodexAuthStatus {
        available: true,
        authenticated: false,
        auth_method: None,
        version: Some(codex.version),
        login_in_progress: true,
        message: "Waiting for official Codex browser login".into(),
    })
}

#[tauri::command]
pub async fn codex_login_cancel(
    state: State<'_, CodexAuthState>,
) -> Result<CodexAuthStatus, String> {
    let deadline = tokio::time::Instant::now() + CODEX_AUTH_TIMEOUT;
    cancel_login_until(&state, deadline).await?;
    auth_status_until(&state, deadline)
        .await
        .map(|(status, _)| status)
}

#[tauri::command]
pub async fn codex_logout(state: State<'_, CodexAuthState>) -> Result<CodexAuthStatus, String> {
    let deadline = tokio::time::Instant::now() + CODEX_LOGOUT_TIMEOUT;
    cancel_login_until(&state, deadline).await?;
    let cancel = AtomicBool::new(false);
    let Some(codex) = discover_codex_until(&cancel, deadline)
        .await
        .map_err(|error| auth_probe_error("Codex version check", error))?
    else {
        return Ok(CodexAuthStatus::unavailable());
    };
    let output = run_verified_probe(&codex, &["logout"], &cancel, deadline)
        .await
        .map_err(|error| auth_probe_error("Codex logout", error))?;
    if !output.status.success() {
        return Err("Codex logout failed".to_string());
    }
    auth_status_until(&state, deadline)
        .await
        .map(|(status, _)| status)
}

#[derive(Debug, PartialEq, Eq)]
enum ExecEvent {
    Ignored,
    AgentMessage(String),
    ToolChanged(String),
    TurnFailed,
}

fn parse_exec_event(
    line: &str,
    tool_calls: &mut HashMap<String, ToolCall>,
) -> Result<ExecEvent, CodexTurnError> {
    let event: Value = serde_json::from_str(line).map_err(|_| CodexTurnError::Protocol)?;
    let event_type = event
        .get("type")
        .and_then(Value::as_str)
        .ok_or(CodexTurnError::Protocol)?;
    if event_type == "turn.failed" {
        return Ok(ExecEvent::TurnFailed);
    }
    if event_type != "item.started" && event_type != "item.completed" {
        return Ok(ExecEvent::Ignored);
    }
    let item = event.get("item").ok_or(CodexTurnError::Protocol)?;
    match item.get("type").and_then(Value::as_str) {
        Some("agent_message") if event_type == "item.completed" => {
            let text = item
                .get("text")
                .and_then(Value::as_str)
                .ok_or(CodexTurnError::Protocol)?;
            if text.len() > MAX_FINAL_TEXT_BYTES {
                return Err(CodexTurnError::Protocol);
            }
            Ok(ExecEvent::AgentMessage(text.to_owned()))
        }
        Some("mcp_tool_call") => {
            let id = item
                .get("id")
                .and_then(Value::as_str)
                .ok_or(CodexTurnError::Protocol)?
                .to_string();
            if !tool_calls.contains_key(&id) && tool_calls.len() >= MAX_TOOL_CALLS {
                return Err(CodexTurnError::Protocol);
            }
            let existed = tool_calls.contains_key(&id);
            let previous_result = tool_calls
                .get(&id)
                .and_then(|call| call.result.as_ref())
                .cloned();
            let name = item
                .get("tool")
                .and_then(Value::as_str)
                .ok_or(CodexTurnError::Protocol)?
                .to_string();
            let args = item
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            let args = redacted_tool_args(&name, args);
            let mut call = tool_calls
                .remove(&id)
                .unwrap_or_else(|| ToolCall::request(id.clone(), name, args));
            if event_type == "item.completed" {
                let failed = item.get("error").is_some_and(|value| !value.is_null());
                call.is_error = Some(failed);
                call.result = Some(serde_json::json!({
                    "status": if failed { "failed" } else { "completed" }
                }));
            }
            let changed = !existed || previous_result.as_ref() != call.result.as_ref();
            tool_calls.insert(id.clone(), call);
            Ok(if changed {
                ExecEvent::ToolChanged(id)
            } else {
                ExecEvent::Ignored
            })
        }
        _ => Ok(ExecEvent::Ignored),
    }
}

fn push_config(args: &mut Vec<OsString>, value: impl Into<OsString>) {
    args.push(OsString::from("-c"));
    args.push(value.into());
}

fn loopback_no_proxy(existing: Option<OsString>) -> OsString {
    let required = "127.0.0.1,localhost,::1,[::1]";
    match existing.filter(|value| !value.is_empty()) {
        Some(existing) => {
            let mut combined = existing;
            combined.push(",");
            combined.push(required);
            combined
        }
        None => OsString::from(required),
    }
}

fn build_exec_args(endpoint_url: &str, isolated_cwd: &Path) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("exec"),
        OsString::from("--strict-config"),
        OsString::from("--json"),
        OsString::from("--ephemeral"),
        OsString::from("--ignore-user-config"),
        OsString::from("--ignore-rules"),
        OsString::from("--sandbox"),
        OsString::from("read-only"),
        OsString::from("--skip-git-repo-check"),
        OsString::from("--color"),
        OsString::from("never"),
        OsString::from("-C"),
        isolated_cwd.as_os_str().to_owned(),
    ];
    push_config(
        &mut args,
        format!("mcp_servers.opentake.url=\"{endpoint_url}\""),
    );
    push_config(
        &mut args,
        format!("mcp_servers.opentake.bearer_token_env_var=\"{CODEX_MCP_BEARER_ENV}\""),
    );
    for config in [
        "mcp_servers.opentake.required=true",
        "mcp_servers.opentake.default_tools_approval_mode=\"approve\"",
        "approval_policy=\"never\"",
        "agents.enabled=false",
        "skills.include_instructions=false",
        "apps._default.enabled=false",
        "features.apps=false",
        "features.auth_elicitation=false",
        "features.browser_use=false",
        "features.in_app_browser=false",
        "features.code_mode_host=false",
        "features.computer_use=false",
        "features.goals=false",
        "features.hooks=false",
        "features.image_generation=false",
        "features.memories=false",
        "features.multi_agent=false",
        "features.personality=false",
        "features.plugins=false",
        "features.remote_plugin=false",
        "features.request_permissions_tool=false",
        "features.shell_tool=false",
        "features.skill_search=false",
        "features.tool_call_mcp_elicitation=false",
        "features.tool_suggest=false",
        "features.unified_exec=false",
        "features.workspace_dependencies=false",
        "web_search=\"disabled\"",
    ] {
        push_config(&mut args, config);
    }
    args.push(OsString::from("-"));
    args
}

async fn read_bounded_line<R: AsyncBufRead + Unpin>(
    reader: &mut R,
    buffer: &mut Vec<u8>,
) -> Result<Option<String>, CodexTurnError> {
    buffer.clear();
    loop {
        let available = reader
            .fill_buf()
            .await
            .map_err(|_| CodexTurnError::Protocol)?;
        if available.is_empty() {
            if buffer.is_empty() {
                return Ok(None);
            }
            break;
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let take = newline.unwrap_or(available.len());
        if buffer.len().saturating_add(take) > MAX_JSONL_LINE_BYTES {
            return Err(CodexTurnError::Protocol);
        }
        buffer.extend_from_slice(&available[..take]);
        reader.consume(take + usize::from(newline.is_some()));
        if newline.is_some() {
            break;
        }
    }
    if buffer.last() == Some(&b'\r') {
        buffer.pop();
    }
    String::from_utf8(buffer.clone())
        .map(Some)
        .map_err(|_| CodexTurnError::Protocol)
}

async fn drain_bounded_capture<R: AsyncRead + Unpin>(mut reader: R, limit: usize) -> Vec<u8> {
    let mut captured = Vec::with_capacity(limit);
    let mut chunk = [0_u8; 8192];
    loop {
        match reader.read(&mut chunk).await {
            Ok(0) | Err(_) => return captured,
            Ok(read) => {
                let remaining = limit.saturating_sub(captured.len());
                captured.extend_from_slice(&chunk[..read.min(remaining)]);
            }
        }
    }
}

async fn drain_stderr<R: AsyncRead + Unpin>(reader: R) -> Vec<u8> {
    drain_bounded_capture(reader, MAX_STDERR_CAPTURE_BYTES).await
}

fn work_deadline(deadline: tokio::time::Instant) -> tokio::time::Instant {
    deadline
        .checked_sub(CODEX_CLEANUP_RESERVE)
        .unwrap_or(deadline)
}

async fn join_capture_until(
    task: Option<tokio::task::JoinHandle<Vec<u8>>>,
    deadline: tokio::time::Instant,
) -> Result<Vec<u8>, CodexTurnError> {
    let Some(mut task) = task else {
        return Ok(Vec::new());
    };
    match tokio::time::timeout_at(deadline, &mut task).await {
        Ok(Ok(bytes)) => Ok(bytes),
        Ok(Err(_)) => Ok(Vec::new()),
        Err(_) => {
            task.abort();
            Err(CodexTurnError::Timeout)
        }
    }
}

async fn terminate_and_reap_until(
    child: &mut tokio::process::Child,
    tree: &mut opentake_media::process_tree::ProcessTree,
    deadline: tokio::time::Instant,
) -> Result<ExitStatus, CodexTurnError> {
    let _ = tree.terminate();
    let _ = child.start_kill();
    let result = tokio::time::timeout_at(deadline, child.wait()).await;
    tree.disarm();
    match result {
        Ok(Ok(status)) => Ok(status),
        Ok(Err(_)) => Err(CodexTurnError::ProviderFailed),
        Err(_) => Err(CodexTurnError::Timeout),
    }
}

struct ProbeOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

async fn wait_for_cancel(cancel: &AtomicBool) {
    let mut poll = tokio::time::interval(CANCEL_POLL_INTERVAL);
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        poll.tick().await;
        if cancel.load(Ordering::Acquire) {
            return;
        }
    }
}

async fn run_probe(
    path: &Path,
    args: &[&str],
    cancel: &AtomicBool,
    deadline: tokio::time::Instant,
) -> Result<ProbeOutput, CodexTurnError> {
    let mut command = tokio::process::Command::new(path);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    opentake_media::process_tree::configure_command(command.as_std_mut());
    let mut child = command
        .spawn()
        .map_err(|_| CodexTurnError::ProviderFailed)?;
    let child_id = child.id().ok_or(CodexTurnError::ProviderFailed)?;
    let mut tree = match opentake_media::process_tree::ProcessTree::attach(child_id) {
        Ok(tree) => tree,
        Err(_) => {
            let _ = child.start_kill();
            let _ = tokio::time::timeout_at(deadline, child.wait()).await;
            return Err(CodexTurnError::ProviderFailed);
        }
    };
    let stdout_task = child
        .stdout
        .take()
        .map(|stdout| tokio::spawn(drain_bounded_capture(stdout, MAX_PROBE_CAPTURE_BYTES)));
    let stderr_task = child
        .stderr
        .take()
        .map(|stderr| tokio::spawn(drain_bounded_capture(stderr, MAX_PROBE_CAPTURE_BYTES)));
    let cancellation = wait_for_cancel(cancel);
    tokio::pin!(cancellation);
    let deadline_wait = tokio::time::sleep_until(work_deadline(deadline));
    tokio::pin!(deadline_wait);
    let selected = tokio::select! {
        status = child.wait() => status.map_err(|_| CodexTurnError::ProviderFailed),
        _ = &mut cancellation => Err(CodexTurnError::Cancelled),
        _ = &mut deadline_wait => Err(CodexTurnError::Timeout),
    };
    let status = match selected {
        Ok(status) => {
            // Kill descendants that inherited either capture pipe before the
            // drain tasks are joined.
            let _ = tree.terminate();
            tree.disarm();
            status
        }
        Err(error) => {
            let _ = terminate_and_reap_until(&mut child, &mut tree, deadline).await;
            let _ = join_capture_until(stdout_task, deadline).await;
            let _ = join_capture_until(stderr_task, deadline).await;
            return Err(error);
        }
    };
    let stdout = join_capture_until(stdout_task, deadline).await?;
    let stderr = join_capture_until(stderr_task, deadline).await?;
    Ok(ProbeOutput {
        status,
        stdout,
        stderr,
    })
}

async fn discover_codex_until(
    cancel: &AtomicBool,
    deadline: tokio::time::Instant,
) -> Result<Option<VerifiedCodex>, CodexTurnError> {
    for path in candidate_paths() {
        let Some(identity) = ExecutableIdentity::capture(&path) else {
            continue;
        };
        let probe = match run_probe(&path, &["--version"], cancel, deadline).await {
            Ok(probe) => probe,
            Err(CodexTurnError::Cancelled) => return Err(CodexTurnError::Cancelled),
            Err(CodexTurnError::Timeout) => return Err(CodexTurnError::Timeout),
            Err(_) => continue,
        };
        if !probe.status.success() || !identity.is_current(&path) {
            continue;
        }
        let Some(version) = parsed_codex_version(&probe.stdout) else {
            continue;
        };
        if supported_codex_version(&version) {
            return Ok(Some(VerifiedCodex {
                path,
                version,
                identity,
            }));
        }
    }
    Ok(None)
}

async fn run_verified_probe(
    codex: &VerifiedCodex,
    args: &[&str],
    cancel: &AtomicBool,
    deadline: tokio::time::Instant,
) -> Result<ProbeOutput, CodexTurnError> {
    if !codex.identity.is_current(&codex.path) {
        return Err(CodexTurnError::Unavailable);
    }
    let output = run_probe(&codex.path, args, cancel, deadline).await?;
    if !codex.identity.is_current(&codex.path) {
        return Err(CodexTurnError::Unavailable);
    }
    Ok(output)
}

async fn discover_codex_for_turn(
    cancel: &AtomicBool,
    deadline: tokio::time::Instant,
) -> Result<Option<(PathBuf, String)>, CodexTurnError> {
    Ok(discover_codex_until(cancel, deadline)
        .await?
        .map(|codex| (codex.path, codex.version)))
}

async fn write_prompt_with_lifecycle<W: AsyncWrite + Unpin>(
    mut stdin: W,
    prompt: &str,
    endpoint: &opentake_agent::mcp::server::EphemeralMcpEndpoint,
    cancel: &AtomicBool,
    deadline: tokio::time::Instant,
) -> Result<(), CodexTurnError> {
    let write = async {
        stdin.write_all(prompt.as_bytes()).await?;
        stdin.shutdown().await
    };
    tokio::pin!(write);
    let mut poll = tokio::time::interval(CANCEL_POLL_INTERVAL);
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let deadline = tokio::time::sleep_until(deadline);
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            result = &mut write => {
                return result.map_err(|_| CodexTurnError::ProviderFailed);
            }
            _ = poll.tick() => {
                if cancel.load(Ordering::Acquire) {
                    return Err(CodexTurnError::Cancelled);
                }
            }
            _ = endpoint.stopped() => return Err(CodexTurnError::McpStart),
            _ = &mut deadline => return Err(CodexTurnError::Timeout),
        }
    }
}

fn strict_config_rejected(stderr: &[u8]) -> bool {
    let text = String::from_utf8_lossy(stderr).to_ascii_lowercase();
    text.contains("strict config")
        || text.contains("unknown configuration")
        || text.contains("unknown config")
        || text.contains("unknown field")
}

async fn consume_exec_stream<R, F>(
    stdout: R,
    endpoint: &opentake_agent::mcp::server::EphemeralMcpEndpoint,
    context: &CodexTurnContext,
    deadline: tokio::time::Instant,
    on_tool_call: &mut F,
) -> Result<CodexTurnOutput, CodexTurnError>
where
    R: AsyncRead + Unpin,
    F: FnMut(ToolCall),
{
    let mut reader = BufReader::new(stdout);
    let mut line_buffer = Vec::new();
    let mut stdout_bytes = 0_usize;
    let mut final_text = None;
    let mut tool_calls = HashMap::new();
    let mut poll = tokio::time::interval(CANCEL_POLL_INTERVAL);
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let deadline = tokio::time::sleep_until(deadline);
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            line = read_bounded_line(&mut reader, &mut line_buffer) => {
                let Some(line) = line? else {
                    break;
                };
                stdout_bytes = stdout_bytes.saturating_add(line.len()).saturating_add(1);
                if stdout_bytes > MAX_STDOUT_BYTES {
                    return Err(CodexTurnError::Protocol);
                }
                match parse_exec_event(&line, &mut tool_calls)? {
                    ExecEvent::AgentMessage(text) => final_text = Some(text),
                    ExecEvent::TurnFailed => return Err(CodexTurnError::ProviderFailed),
                    ExecEvent::ToolChanged(id) => {
                        let call = tool_calls.get(&id).ok_or(CodexTurnError::Protocol)?;
                        on_tool_call(call.clone());
                    }
                    ExecEvent::Ignored => {}
                }
            }
            _ = poll.tick() => {
                if context.cancel.load(Ordering::Acquire) {
                    return Err(CodexTurnError::Cancelled);
                }
            }
            _ = endpoint.stopped() => return Err(CodexTurnError::McpStart),
            _ = &mut deadline => return Err(CodexTurnError::Timeout),
        }
    }

    let text = final_text.filter(|text| !text.trim().is_empty());
    let Some(text) = text else {
        return Err(CodexTurnError::Protocol);
    };
    let mut tool_calls = tool_calls.into_values().collect::<Vec<_>>();
    tool_calls.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(CodexTurnOutput { text, tool_calls })
}

pub async fn run_agent_turn<F>(
    context: CodexTurnContext,
    prompt: &str,
    on_tool_call: F,
) -> Result<CodexTurnOutput, CodexTurnError>
where
    F: FnMut(ToolCall),
{
    let deadline = tokio::time::Instant::now() + CODEX_TURN_TIMEOUT;
    let Some((path, version)) = discover_codex_for_turn(context.cancel.as_ref(), deadline).await?
    else {
        return Err(CodexTurnError::Unavailable);
    };
    if !supported_codex_version(&version) {
        return Err(CodexTurnError::IncompatibleCli);
    }
    let output = run_probe(
        &path,
        &["login", "status"],
        context.cancel.as_ref(),
        deadline,
    )
    .await
    .map_err(|error| match error {
        CodexTurnError::Cancelled | CodexTurnError::Timeout => error,
        _ => CodexTurnError::Unavailable,
    })?;
    let login_text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if !output.status.success() || !parse_login_status(&login_text).0 {
        return Err(CodexTurnError::NotAuthenticated);
    }

    run_agent_turn_with_executable_until(&path, context, prompt, on_tool_call, deadline).await
}

#[cfg(all(test, unix))]
async fn run_agent_turn_with_executable<F>(
    path: &Path,
    context: CodexTurnContext,
    prompt: &str,
    on_tool_call: F,
) -> Result<CodexTurnOutput, CodexTurnError>
where
    F: FnMut(ToolCall),
{
    let deadline = tokio::time::Instant::now() + CODEX_TURN_TIMEOUT;
    run_agent_turn_with_executable_until(path, context, prompt, on_tool_call, deadline).await
}

async fn run_agent_turn_with_executable_until<F>(
    path: &Path,
    context: CodexTurnContext,
    prompt: &str,
    mut on_tool_call: F,
    deadline: tokio::time::Instant,
) -> Result<CodexTurnOutput, CodexTurnError>
where
    F: FnMut(ToolCall),
{
    let isolated_cwd = tempfile::tempdir().map_err(|_| CodexTurnError::ProviderFailed)?;
    let endpoint = crate::mcp::spawn(
        context.dispatcher.clone(),
        context.registry.clone(),
        context.gate.clone(),
    )
    .await
    .map_err(|_| CodexTurnError::McpStart)?;
    let args = build_exec_args(endpoint.url(), isolated_cwd.path());
    let no_proxy =
        loopback_no_proxy(std::env::var_os("NO_PROXY").or_else(|| std::env::var_os("no_proxy")));
    let mut command = tokio::process::Command::new(path);
    command
        .args(args)
        .current_dir(isolated_cwd.path())
        .env_remove("PWD")
        .env_remove("OLDPWD")
        .env_remove("INIT_CWD")
        .env_remove("npm_config_local_prefix")
        .env_remove("CARGO_MANIFEST_DIR")
        .env_remove("CARGO_WORKSPACE_DIR")
        .env(CODEX_MCP_BEARER_ENV, endpoint.bearer_token())
        .env("NO_PROXY", &no_proxy)
        .env("no_proxy", &no_proxy)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    opentake_media::process_tree::configure_command(command.as_std_mut());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(_) => {
            context.gate.request_dispatch_cancel();
            let _ = tokio::time::timeout_at(deadline, endpoint.close()).await;
            drop(isolated_cwd);
            return Err(CodexTurnError::ProviderFailed);
        }
    };
    let child_id = match child.id() {
        Some(child_id) => child_id,
        None => {
            context.gate.request_dispatch_cancel();
            let _ = child.start_kill();
            let _ = tokio::time::timeout_at(deadline, child.wait()).await;
            let _ = tokio::time::timeout_at(deadline, endpoint.close()).await;
            drop(isolated_cwd);
            return Err(CodexTurnError::ProviderFailed);
        }
    };
    let mut tree = match opentake_media::process_tree::ProcessTree::attach(child_id) {
        Ok(tree) => tree,
        Err(_) => {
            context.gate.request_dispatch_cancel();
            let _ = child.start_kill();
            let _ = tokio::time::timeout_at(deadline, child.wait()).await;
            let _ = tokio::time::timeout_at(deadline, endpoint.close()).await;
            drop(isolated_cwd);
            return Err(CodexTurnError::ProviderFailed);
        }
    };

    let stderr_task = child
        .stderr
        .take()
        .map(|stderr| tokio::spawn(drain_stderr(stderr)));
    let stdout = child.stdout.take();
    let operation_deadline = work_deadline(deadline);
    let prompt_result = match child.stdin.take() {
        Some(stdin) => {
            write_prompt_with_lifecycle(
                stdin,
                prompt,
                &endpoint,
                context.cancel.as_ref(),
                operation_deadline,
            )
            .await
        }
        None => Err(CodexTurnError::Protocol),
    };
    let mut outcome = prompt_result.err().map(Err);
    let mut status: Option<std::io::Result<ExitStatus>> = None;
    if outcome.is_none() {
        if let Some(stdout) = stdout {
            let consume = consume_exec_stream(
                stdout,
                &endpoint,
                &context,
                operation_deadline,
                &mut on_tool_call,
            );
            tokio::pin!(consume);
            let wait = child.wait();
            tokio::pin!(wait);
            let mut poll = tokio::time::interval(CANCEL_POLL_INTERVAL);
            poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            let wait_deadline = tokio::time::sleep_until(operation_deadline);
            tokio::pin!(wait_deadline);
            loop {
                tokio::select! {
                    result = &mut consume, if outcome.is_none() => outcome = Some(result),
                    waited = &mut wait, if status.is_none() => {
                        status = Some(waited);
                        // The immediate CLI may have left descendants holding the
                        // JSONL/stderr pipes. Terminate them so EOF is observable.
                        let _ = tree.terminate();
                        tree.disarm();
                    }
                    _ = poll.tick(), if outcome.is_none() => {
                        if context.cancel.load(Ordering::Acquire) {
                            outcome = Some(Err(CodexTurnError::Cancelled));
                        }
                    }
                    _ = endpoint.stopped(), if outcome.is_none() => {
                        outcome = Some(Err(CodexTurnError::McpStart));
                    }
                    _ = &mut wait_deadline, if outcome.is_none() || status.is_none() => {
                        outcome = Some(Err(CodexTurnError::Timeout));
                    }
                }
                if outcome.is_some() && status.is_some() {
                    break;
                }
                if outcome.as_ref().is_some_and(Result::is_err) {
                    break;
                }
            }
        } else {
            outcome = Some(Err(CodexTurnError::Protocol));
        }
    }
    let mut outcome = outcome.unwrap_or(Err(CodexTurnError::Protocol));
    let wait_failed = match status.as_ref() {
        Some(Ok(status)) => !status.success(),
        Some(Err(_)) => true,
        None => false,
    };
    if wait_failed && outcome.is_ok() {
        outcome = Err(CodexTurnError::ProviderFailed);
    }
    let outcome_was_cancelled = matches!(&outcome, Err(CodexTurnError::Cancelled));
    let externally_cancelled = context.cancel.load(Ordering::Acquire);
    let requested_cleanup_cancel = outcome.is_err() || externally_cancelled;
    if requested_cleanup_cancel {
        if outcome_was_cancelled || externally_cancelled {
            context.gate.request_cancel();
        } else {
            context.gate.request_dispatch_cancel();
        }
        let _ = tree.terminate();
        let _ = child.start_kill();
    }
    let status = match status {
        Some(Ok(status)) => Ok(status),
        Some(Err(_)) | None => terminate_and_reap_until(&mut child, &mut tree, deadline).await,
    };
    let endpoint_close = tokio::time::timeout_at(deadline, endpoint.close()).await;
    let endpoint_result = match endpoint_close {
        Ok(result) => result,
        Err(_) => {
            if outcome.is_ok() {
                outcome = Err(CodexTurnError::Timeout);
            }
            Err(opentake_agent::mcp::server::EphemeralMcpError::Join)
        }
    };
    let stderr = match join_capture_until(stderr_task, deadline).await {
        Ok(stderr) => stderr,
        Err(error) => {
            if outcome.is_ok() {
                outcome = Err(error);
            }
            Vec::new()
        }
    };
    drop(isolated_cwd);

    if outcome_was_cancelled
        || externally_cancelled
        || (!requested_cleanup_cancel && context.cancel.load(Ordering::Acquire))
    {
        return Err(CodexTurnError::Cancelled);
    }
    let status = match status {
        Ok(status) => status,
        Err(error) => return Err(error),
    };
    if !status.success()
        && !matches!(
            outcome,
            Err(CodexTurnError::Cancelled | CodexTurnError::Timeout | CodexTurnError::McpStart)
        )
    {
        outcome = Err(if strict_config_rejected(&stderr) {
            CodexTurnError::StrictConfigRejected
        } else {
            CodexTurnError::ProviderFailed
        });
    }
    if endpoint_result.is_err() && outcome.is_ok() {
        context.gate.request_dispatch_cancel();
        outcome = Err(CodexTurnError::McpStart);
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_agent::chat::{ChatMessage, ChatSession};
    use opentake_agent::mcp::core_handle::{AppCoreHandle, CoreHandle};
    use opentake_agent::tools::result::ToolResult;
    use opentake_core::AppCore;

    struct TestTurnGate;

    impl ChatTurnGate for TestTurnGate {
        fn timeline(&self, dispatcher: &Dispatcher) -> Option<opentake_domain::Timeline> {
            Some(dispatcher.timeline())
        }

        fn dispatch(&self, dispatcher: &Dispatcher, name: &str, args: Value) -> Option<ToolResult> {
            Some(dispatcher.dispatch(name, args))
        }
    }

    fn turn_context(cancel: Arc<AtomicBool>) -> CodexTurnContext {
        let registry = Arc::new(RwLock::new(PluginRegistry::with_builtins()));
        let handle: Arc<dyn CoreHandle> = Arc::new(AppCoreHandle::new(AppCore::new()));
        let dispatcher = Arc::new(Dispatcher::new(handle, registry.clone()));
        CodexTurnContext {
            dispatcher,
            registry,
            gate: Arc::new(TestTurnGate),
            cancel,
        }
    }

    #[cfg(unix)]
    fn fake_codex_script(root: &Path, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let path = root.join("fake-codex");
        std::fs::write(&path, format!("#!/bin/sh\nset -eu\n{body}\n")).unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&path, permissions).unwrap();
        path
    }

    #[cfg(unix)]
    fn process_exists(pid: &str) -> bool {
        Command::new("kill")
            .args(["-0", pid])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    #[cfg(unix)]
    fn captured_endpoint_and_cwd(capture: &str) -> (std::net::SocketAddr, PathBuf) {
        let cwd = capture
            .lines()
            .find_map(|line| line.strip_prefix("cwd="))
            .map(PathBuf::from)
            .expect("captured isolated cwd");
        let url = capture
            .lines()
            .find(|line| line.contains("mcp_servers.opentake.url="))
            .and_then(|line| line.split('"').nth(1))
            .expect("captured dynamic endpoint");
        let addr = url
            .strip_prefix("http://")
            .and_then(|value| value.strip_suffix("/mcp"))
            .expect("loopback MCP URL")
            .parse()
            .expect("socket address");
        (addr, cwd)
    }

    #[test]
    fn parses_chatgpt_and_api_login_without_exposing_credentials() {
        assert_eq!(
            parse_login_status("Logged in using ChatGPT\n"),
            (true, Some("ChatGPT".into()))
        );
        assert_eq!(
            parse_login_status("Logged in using an API key."),
            (true, Some("API key".into()))
        );
        assert_eq!(
            parse_login_status("Logged in using token=must-not-surface"),
            (true, None)
        );
        assert_eq!(parse_login_status("Not logged in"), (false, None));
    }

    #[test]
    fn requires_the_verified_codex_cli_baseline() {
        assert!(!supported_codex_version("codex-cli 0.145.9"));
        assert!(supported_codex_version("codex-cli 0.146.0"));
        assert!(supported_codex_version("codex-cli 1.0.0"));
        assert!(!supported_codex_version("codex-cli unknown"));
        assert!(!supported_codex_version("other 0.146.0"));
    }

    #[test]
    fn exec_args_are_strict_dynamic_and_use_stdin_in_an_isolated_cwd() {
        let isolated = Path::new("/private/tmp/opentake-codex-turn");
        let endpoint = "http://127.0.0.1:43127/mcp";
        let args = build_exec_args(endpoint, isolated);
        let rendered = args
            .iter()
            .map(|arg| arg.to_string_lossy())
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(args.first(), Some(&OsString::from("exec")));
        assert_eq!(args.last(), Some(&OsString::from("-")));
        assert!(rendered.contains("--strict-config"));
        assert!(rendered.contains("--ignore-user-config"));
        assert!(rendered.contains("--ignore-rules"));
        assert!(rendered.contains("--sandbox\nread-only"));
        assert!(rendered.contains(endpoint));
        assert!(rendered.contains(&format!(
            "mcp_servers.opentake.bearer_token_env_var=\"{CODEX_MCP_BEARER_ENV}\""
        )));
        assert!(!rendered.contains("not-a-real-secret"));
        assert!(rendered.contains(isolated.to_string_lossy().as_ref()));
        assert!(rendered.contains("approval_policy=\"never\""));
        assert!(rendered.contains("features.shell_tool=false"));
        assert!(rendered.contains("features.unified_exec=false"));
        assert!(rendered.contains("features.multi_agent=false"));
        assert!(rendered.contains("apps._default.enabled=false"));
        assert!(rendered.contains("web_search=\"disabled\""));
        assert!(!rendered.contains("127.0.0.1:19789"));
        assert!(!rendered.contains("tools.view_image"));
        assert!(!rendered.contains("secret-project-path"));
        let exec = args.iter().position(|arg| arg == "exec").unwrap();
        let ignore = args
            .iter()
            .position(|arg| arg == "--ignore-user-config")
            .unwrap();
        assert!(ignore > exec, "exec-only flag must follow the subcommand");
    }

    #[test]
    fn parses_codex_jsonl_agent_and_mcp_events() {
        let mut calls = HashMap::new();
        assert_eq!(
            parse_exec_event(
                r#"{"type":"item.started","item":{"id":"item_1","type":"mcp_tool_call","tool":"get_timeline","arguments":{}}}"#,
                &mut calls,
            ),
            Ok(ExecEvent::ToolChanged("item_1".into()))
        );
        assert_eq!(calls["item_1"].name, "get_timeline");
        assert_eq!(calls["item_1"].result, None);

        assert_eq!(
            parse_exec_event(
                r#"{"type":"item.completed","item":{"id":"item_1","type":"mcp_tool_call","tool":"get_timeline","arguments":{},"result":{},"error":null}}"#,
                &mut calls,
            ),
            Ok(ExecEvent::ToolChanged("item_1".into()))
        );
        assert_eq!(calls["item_1"].is_error, Some(false));
        assert_eq!(
            parse_exec_event(
                r#"{"type":"item.completed","item":{"id":"item_1","type":"mcp_tool_call","tool":"get_timeline","arguments":{},"result":{},"error":null}}"#,
                &mut calls,
            ),
            Ok(ExecEvent::Ignored),
            "duplicate events must not emit duplicate tool-call updates"
        );
        assert_eq!(
            parse_exec_event(
                r#"{"type":"item.completed","item":{"id":"item_2","type":"agent_message","text":"1280 × 720"}}"#,
                &mut calls,
            ),
            Ok(ExecEvent::AgentMessage("1280 × 720".into()))
        );
    }

    #[test]
    fn codex_import_args_are_redacted_before_events_blocks_and_session_json() {
        const URL_TOKEN: &str = "CODEX_SENTINEL_URL_TOKEN";
        const INLINE_BYTES: &str = "Q09ERVhfU0VOVElORUxfQkFTRTY0";
        let mut calls = HashMap::new();
        let event = serde_json::json!({
            "type": "item.started",
            "item": {
                "id": "secret-import",
                "type": "mcp_tool_call",
                "tool": "mcp__opentake__import_media",
                "arguments": {
                    "source": {
                        "url": format!(
                            "https://user:password@example.invalid/media.mp4?token={URL_TOKEN}#{URL_TOKEN}"
                        ),
                        "bytes": INLINE_BYTES,
                        "mimeType": "video/mp4"
                    }
                }
            }
        });
        assert_eq!(
            parse_exec_event(&event.to_string(), &mut calls),
            Ok(ExecEvent::ToolChanged("secret-import".into()))
        );

        let call = calls.remove("secret-import").unwrap();
        assert_eq!(
            call.args["source"]["url"],
            "https://example.invalid/media.mp4"
        );
        assert_eq!(call.args["source"]["bytes"]["byteLength"], 21);
        assert_eq!(call.args["source"]["bytes"]["redacted"], true);
        assert_eq!(
            call.args["source"]["bytes"]["sha256"]
                .as_str()
                .unwrap()
                .len(),
            64
        );

        let emitted = serde_json::to_string(&call).unwrap();
        assert!(!emitted.contains(URL_TOKEN));
        assert!(!emitted.contains(INLINE_BYTES));
        assert!(!emitted.contains("user:password"));

        let message = ChatMessage::assistant("imported", vec![call]);
        let blocks = serde_json::to_string(&message.blocks).unwrap();
        assert!(!blocks.contains(URL_TOKEN));
        assert!(!blocks.contains(INLINE_BYTES));

        let mut session = ChatSession::new("provider-switch");
        session.provider = Some("codex".into());
        session.messages.push(message);
        session.provider = Some("openai".into());
        let persisted = serde_json::to_string(&session).unwrap();
        assert!(!persisted.contains(URL_TOKEN));
        assert!(!persisted.contains(INLINE_BYTES));
        assert!(!persisted.contains("user:password"));
    }

    #[test]
    fn rejects_non_json_and_recognizes_safe_terminal_events() {
        let mut calls = HashMap::new();
        assert_eq!(
            parse_exec_event("diagnostic", &mut calls),
            Err(CodexTurnError::Protocol)
        );
        assert_eq!(
            parse_exec_event(r#"{"type":"thread.started"}"#, &mut calls),
            Ok(ExecEvent::Ignored)
        );
        assert_eq!(
            parse_exec_event(
                r#"{"type":"turn.failed","error":{"message":"private"}}"#,
                &mut calls,
            ),
            Ok(ExecEvent::TurnFailed)
        );
        assert!(calls.is_empty());
    }

    #[tokio::test]
    async fn bounded_jsonl_reader_rejects_oversized_lines_before_allocating_more() {
        let data = vec![b'x'; MAX_JSONL_LINE_BYTES + 1];
        let mut reader = BufReader::new(data.as_slice());
        let mut buffer = Vec::new();
        assert_eq!(
            read_bounded_line(&mut reader, &mut buffer).await,
            Err(CodexTurnError::Protocol)
        );
        assert!(buffer.len() <= MAX_JSONL_LINE_BYTES);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cancelled_cli_probe_kills_and_reaps_the_probe_process() {
        let root = tempfile::tempdir().unwrap();
        let script = fake_codex_script(
            root.path(),
            r#"
hold="$(dirname "$0")/probe-hold"
mkfifo "$hold"
sleep 60 &
descendant="$!"
printf 'pid=%s\ndescendant=%s\n' "$$" "$descendant" > "$(dirname "$0")/probe-capture"
: > "$(dirname "$0")/probe-ready"
exec 3<> "$hold"
IFS= read -r ignored <&3
"#,
        );
        let cancel = Arc::new(AtomicBool::new(false));
        let task_cancel = cancel.clone();
        let task_script = script.clone();
        let task = tokio::spawn(async move {
            run_probe(
                &task_script,
                &["--version"],
                task_cancel.as_ref(),
                tokio::time::Instant::now() + Duration::from_secs(60),
            )
            .await
        });
        let ready = root.path().join("probe-ready");
        tokio::time::timeout(Duration::from_secs(5), async {
            while !ready.exists() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("probe child reached its blocking point");
        let capture = std::fs::read_to_string(root.path().join("probe-capture")).unwrap();
        let pid = capture
            .lines()
            .find_map(|line| line.strip_prefix("pid="))
            .expect("captured probe pid")
            .to_string();
        let descendant = capture
            .lines()
            .find_map(|line| line.strip_prefix("descendant="))
            .expect("captured probe descendant")
            .to_string();

        cancel.store(true, Ordering::Release);
        let result = tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("probe cancellation completed")
            .expect("probe task joined");
        assert!(matches!(result, Err(CodexTurnError::Cancelled)));
        assert!(!process_exists(&pid), "probe child must be reaped");
        assert!(
            !process_exists(&descendant),
            "probe descendant inheriting capture pipes must be killed"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn fake_cli_success_closes_endpoint_waits_child_and_removes_tempdir() {
        let root = tempfile::tempdir().unwrap();
        let script = fake_codex_script(
            root.path(),
            r#"
capture="$(dirname "$0")/capture"
{
  sleep 60 &
  descendant="$!"
  printf 'cwd=%s\n' "$PWD"
  printf 'token_length=%s\n' "${#OPENTAKE_CODEX_MCP_BEARER_TOKEN}"
  printf 'descendant=%s\n' "$descendant"
  printf 'arg=%s\n' "$@"
  IFS= read -r prompt || true
  printf 'prompt=%s\n' "$prompt"
} > "$capture"
printf '%s\n' '{"type":"thread.started"}'
printf '%s\n' '{"type":"item.completed","item":{"id":"message_1","type":"agent_message","text":"finished"}}'
printf '%s\n' 'child-waited' > "$(dirname "$0")/finished"
"#,
        );
        let result = run_agent_turn_with_executable(
            &script,
            turn_context(Arc::new(AtomicBool::new(false))),
            "prompt over stdin",
            |_| {},
        )
        .await
        .expect("fake Codex turn succeeds");
        assert_eq!(result.text, "finished");
        assert!(root.path().join("finished").exists());

        let capture = std::fs::read_to_string(root.path().join("capture")).unwrap();
        let (addr, cwd) = captured_endpoint_and_cwd(&capture);
        let descendant = capture
            .lines()
            .find_map(|line| line.strip_prefix("descendant="))
            .expect("captured inherited-pipe descendant");
        assert!(capture.contains("prompt=prompt over stdin"));
        assert!(capture.contains("token_length=64"));
        assert!(
            !process_exists(descendant),
            "successful turn must kill descendants that retain JSONL pipes"
        );
        assert!(!cwd.exists(), "isolated cwd removed only after cleanup");
        assert!(tokio::net::TcpStream::connect(addr).await.is_err());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn fake_cli_cancel_interrupts_blocked_stdin_and_cleans_up_everything() {
        let root = tempfile::tempdir().unwrap();
        let script = fake_codex_script(
            root.path(),
            r#"
capture="$(dirname "$0")/capture"
hold="$(dirname "$0")/hold"
mkfifo "$hold"
{
  printf 'cwd=%s\n' "$PWD"
  printf 'pid=%s\n' "$$"
  printf 'arg=%s\n' "$@"
} > "$capture"
: > "$(dirname "$0")/ready"
printf '%s\n' '{"type":"thread.started"}'
exec 3<> "$hold"
IFS= read -r ignored <&3
"#,
        );
        let cancel = Arc::new(AtomicBool::new(false));
        let task_cancel = cancel.clone();
        let task_script = script.clone();
        let prompt = "x".repeat(2 * 1024 * 1024);
        let task = tokio::spawn(async move {
            run_agent_turn_with_executable(&task_script, turn_context(task_cancel), &prompt, |_| {})
                .await
        });
        let capture_path = root.path().join("capture");
        let ready_path = root.path().join("ready");
        tokio::time::timeout(Duration::from_secs(5), async {
            while !ready_path.exists() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("fake child reached its blocking point");
        let capture = std::fs::read_to_string(&capture_path).unwrap();
        let pid = capture
            .lines()
            .find_map(|line| line.strip_prefix("pid="))
            .expect("captured child pid")
            .to_string();
        let (addr, cwd) = captured_endpoint_and_cwd(&capture);
        cancel.store(true, Ordering::Release);
        let result = tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("cancel cleanup completed")
            .expect("runner task joined");
        assert_eq!(result.unwrap_err(), CodexTurnError::Cancelled);
        assert!(!cwd.exists());
        assert!(tokio::net::TcpStream::connect(addr).await.is_err());
        assert!(
            !Command::new("kill")
                .args(["-0", &pid])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .unwrap()
                .success(),
            "child must be reaped before the runner returns"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn fake_cli_deadline_kills_descendant_and_finishes_within_absolute_deadline() {
        let root = tempfile::tempdir().unwrap();
        let script = fake_codex_script(
            root.path(),
            r#"
capture="$(dirname "$0")/deadline-capture"
sleep 60 &
descendant="$!"
printf 'descendant=%s\n' "$descendant" > "$capture"
printf '%s\n' '{"type":"thread.started"}'
sleep 60
"#,
        );
        // The runner reserves two seconds for bounded cleanup. Leave three
        // seconds for CLI probes/spawn so this process-tree assertion remains
        // meaningful under a parallel, CPU-contended test suite.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let started = tokio::time::Instant::now();
        let result = run_agent_turn_with_executable_until(
            &script,
            turn_context(Arc::new(AtomicBool::new(false))),
            "deadline",
            |_| {},
            deadline,
        )
        .await;

        assert_eq!(result.unwrap_err(), CodexTurnError::Timeout);
        assert!(
            tokio::time::Instant::now().duration_since(started) <= Duration::from_secs(6),
            "cleanup exceeded its absolute deadline by an unbounded amount"
        );
        let capture = std::fs::read_to_string(root.path().join("deadline-capture")).unwrap();
        let descendant = capture
            .trim()
            .strip_prefix("descendant=")
            .expect("captured deadline descendant");
        assert!(
            !process_exists(descendant),
            "deadline cleanup must kill the descendant process tree"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn fake_cli_strict_failure_is_structured_and_fully_cleaned_up() {
        let root = tempfile::tempdir().unwrap();
        let script = fake_codex_script(
            root.path(),
            r#"
capture="$(dirname "$0")/capture"
{
  printf 'cwd=%s\n' "$PWD"
  printf 'arg=%s\n' "$@"
  IFS= read -r prompt || true
} > "$capture"
printf '%s\n' 'unknown configuration key containing private/path/token' >&2
exit 2
"#,
        );
        let result = run_agent_turn_with_executable(
            &script,
            turn_context(Arc::new(AtomicBool::new(false))),
            "do not expose failures",
            |_| {},
        )
        .await;
        assert_eq!(
            result.as_ref().unwrap_err(),
            &CodexTurnError::StrictConfigRejected
        );

        let capture = std::fs::read_to_string(root.path().join("capture")).unwrap();
        let (addr, cwd) = captured_endpoint_and_cwd(&capture);
        assert!(!cwd.exists());
        assert!(tokio::net::TcpStream::connect(addr).await.is_err());
        assert!(!format!("{result:?}").contains("private/path/token"));
    }
}
