//! Official Codex CLI integration.
//!
//! OpenTake never reads, copies, or stores Codex credentials. Authentication is
//! delegated to the user-installed official CLI (`codex login`), and Agent
//! turns run through `codex exec` using that CLI's existing ChatGPT session.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use opentake_agent::chat::ToolCall;
use serde::Serialize;
use serde_json::Value;
use tauri::State;
use tokio::io::{AsyncBufReadExt, BufReader};

const MCP_URL: &str = "http://127.0.0.1:19789/mcp";

#[derive(Default)]
pub struct CodexAuthState {
    login_process: Mutex<Option<Child>>,
}

impl Drop for CodexAuthState {
    fn drop(&mut self) {
        if let Ok(process) = self.login_process.get_mut() {
            if let Some(mut child) = process.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
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

#[derive(Debug)]
pub enum CodexTurnError {
    Cancelled,
    Unavailable,
    NotAuthenticated,
    Failed,
}

#[derive(Debug)]
pub struct CodexTurnOutput {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
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

fn codex_version(path: &Path) -> Option<String> {
    let output = Command::new(path)
        .arg("--version")
        .stdin(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    version.starts_with("codex-cli ").then_some(version)
}

fn discover_codex() -> Option<(PathBuf, String)> {
    candidate_paths().into_iter().find_map(|path| {
        if !path.is_file() {
            return None;
        }
        codex_version(&path).map(|version| (path, version))
    })
}

fn parse_login_status(text: &str) -> (bool, Option<String>) {
    let normalized = text.trim();
    let lower = normalized.to_ascii_lowercase();
    if !lower.contains("logged in") || lower.contains("not logged in") {
        return (false, None);
    }
    let method = normalized
        .split_once("using")
        .map(|(_, value)| value.trim().trim_end_matches('.').to_string())
        .filter(|value| !value.is_empty());
    (true, method)
}

fn login_is_running(state: &CodexAuthState) -> Result<bool, String> {
    let mut process = state.login_process.lock().map_err(|e| e.to_string())?;
    let Some(child) = process.as_mut() else {
        return Ok(false);
    };
    match child.try_wait().map_err(|e| e.to_string())? {
        None => Ok(true),
        Some(_) => {
            process.take();
            Ok(false)
        }
    }
}

fn auth_status(state: &CodexAuthState) -> Result<CodexAuthStatus, String> {
    let login_in_progress = login_is_running(state)?;
    let Some((path, version)) = discover_codex() else {
        return Ok(CodexAuthStatus::unavailable());
    };

    if login_in_progress {
        return Ok(CodexAuthStatus {
            available: true,
            authenticated: false,
            auth_method: None,
            version: Some(version),
            login_in_progress: true,
            message: "Waiting for official Codex browser login".into(),
        });
    }

    let output = Command::new(path)
        .args(["login", "status"])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("could not check Codex login: {e}"))?;
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let (authenticated, auth_method) = parse_login_status(&combined);
    Ok(CodexAuthStatus {
        available: true,
        authenticated,
        auth_method,
        version: Some(version),
        login_in_progress: false,
        message: if authenticated {
            "Codex is signed in".into()
        } else {
            "Codex is not signed in".into()
        },
    })
}

#[tauri::command]
pub fn codex_auth_status(state: State<'_, CodexAuthState>) -> Result<CodexAuthStatus, String> {
    auth_status(&state)
}

#[tauri::command]
pub fn codex_login_start(state: State<'_, CodexAuthState>) -> Result<CodexAuthStatus, String> {
    let current = auth_status(&state)?;
    if current.authenticated || current.login_in_progress {
        return Ok(current);
    }
    let Some((path, version)) = discover_codex() else {
        return Ok(CodexAuthStatus::unavailable());
    };
    let child = Command::new(path)
        .arg("login")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("could not start official Codex login: {e}"))?;
    *state.login_process.lock().map_err(|e| e.to_string())? = Some(child);
    Ok(CodexAuthStatus {
        available: true,
        authenticated: false,
        auth_method: None,
        version: Some(version),
        login_in_progress: true,
        message: "Waiting for official Codex browser login".into(),
    })
}

#[tauri::command]
pub fn codex_login_cancel(state: State<'_, CodexAuthState>) -> Result<CodexAuthStatus, String> {
    if let Some(mut child) = state
        .login_process
        .lock()
        .map_err(|e| e.to_string())?
        .take()
    {
        let _ = child.kill();
        let _ = child.wait();
    }
    auth_status(&state)
}

#[tauri::command]
pub fn codex_logout(state: State<'_, CodexAuthState>) -> Result<CodexAuthStatus, String> {
    if let Some(mut child) = state
        .login_process
        .lock()
        .map_err(|e| e.to_string())?
        .take()
    {
        let _ = child.kill();
        let _ = child.wait();
    }
    let Some((path, _)) = discover_codex() else {
        return Ok(CodexAuthStatus::unavailable());
    };
    Command::new(path)
        .arg("logout")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("could not sign out of Codex: {e}"))?;
    auth_status(&state)
}

fn parse_exec_event(line: &str, tool_calls: &mut HashMap<String, ToolCall>) -> Option<String> {
    let event: Value = serde_json::from_str(line).ok()?;
    let event_type = event.get("type")?.as_str()?;
    if event_type != "item.started" && event_type != "item.completed" {
        return None;
    }
    let item = event.get("item")?;
    match item.get("type")?.as_str()? {
        "agent_message" if event_type == "item.completed" => {
            item.get("text")?.as_str().map(str::to_owned)
        }
        "mcp_tool_call" => {
            let id = item.get("id")?.as_str()?.to_string();
            let name = item.get("tool")?.as_str()?.to_string();
            let args = item
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            let mut call = tool_calls
                .remove(&id)
                .unwrap_or_else(|| ToolCall::request(id.clone(), name, args));
            if event_type == "item.completed" {
                let error = item.get("error").filter(|value| !value.is_null());
                call.is_error = Some(error.is_some());
                call.result = Some(if let Some(error) = error {
                    serde_json::json!({ "error": error })
                } else {
                    serde_json::json!({ "status": "completed" })
                });
            }
            tool_calls.insert(id, call);
            None
        }
        _ => None,
    }
}

pub async fn run_agent_turn<F>(
    project_dir: &Path,
    prompt: &str,
    cancel: Arc<AtomicBool>,
    mut on_tool_call: F,
) -> Result<CodexTurnOutput, CodexTurnError>
where
    F: FnMut(ToolCall),
{
    let Some((path, _)) = discover_codex() else {
        return Err(CodexTurnError::Unavailable);
    };
    let output = Command::new(&path)
        .args(["login", "status"])
        .stdin(Stdio::null())
        .output()
        .map_err(|_| CodexTurnError::Unavailable)?;
    let login_text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if !parse_login_status(&login_text).0 {
        return Err(CodexTurnError::NotAuthenticated);
    }

    let mut child = tokio::process::Command::new(path)
        .args([
            "exec",
            "--json",
            "--ephemeral",
            "--ignore-user-config",
            "--ignore-rules",
            "--sandbox",
            "read-only",
            "--skip-git-repo-check",
            "--color",
            "never",
            "-C",
        ])
        .arg(project_dir)
        .args([
            "-c",
            &format!("mcp_servers.opentake.url=\"{MCP_URL}\""),
            "-c",
            "mcp_servers.opentake.required=true",
            "-c",
            "mcp_servers.opentake.default_tools_approval_mode=\"approve\"",
            "-c",
            "features.plugins=false",
            "-c",
            "features.remote_plugin=false",
            "-c",
            "features.shell_tool=false",
            "-c",
            "web_search=\"disabled\"",
        ])
        .arg(prompt)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| CodexTurnError::Failed)?;

    let stdout = child.stdout.take().ok_or(CodexTurnError::Failed)?;
    let mut lines = BufReader::new(stdout).lines();
    let mut final_text = None;
    let mut tool_calls = HashMap::new();

    loop {
        tokio::select! {
            line = lines.next_line() => match line {
                Ok(Some(line)) => {
                    let previous = tool_calls.clone();
                    if let Some(text) = parse_exec_event(&line, &mut tool_calls) {
                        final_text = Some(text);
                    }
                    for (id, call) in &tool_calls {
                        if !previous.contains_key(id)
                            || previous.get(id).and_then(|old| old.result.as_ref())
                                != call.result.as_ref()
                        {
                            on_tool_call(call.clone());
                        }
                    }
                }
                Ok(None) => break,
                Err(_) => return Err(CodexTurnError::Failed),
            },
            _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                if cancel.load(Ordering::Relaxed) {
                    let _ = child.start_kill();
                    let _ = child.wait().await;
                    return Err(CodexTurnError::Cancelled);
                }
            }
        }
    }

    if cancel.load(Ordering::Relaxed) {
        let _ = child.start_kill();
        let _ = child.wait().await;
        return Err(CodexTurnError::Cancelled);
    }
    let status = child.wait().await.map_err(|_| CodexTurnError::Failed)?;
    let text = final_text.filter(|text| !text.trim().is_empty());
    if !status.success() || text.is_none() {
        return Err(CodexTurnError::Failed);
    }
    let mut tool_calls = tool_calls.into_values().collect::<Vec<_>>();
    tool_calls.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(CodexTurnOutput {
        text: text.expect("checked above"),
        tool_calls,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_chatgpt_and_api_login_without_exposing_credentials() {
        assert_eq!(
            parse_login_status("Logged in using ChatGPT\n"),
            (true, Some("ChatGPT".into()))
        );
        assert_eq!(
            parse_login_status("Logged in using an API key."),
            (true, Some("an API key".into()))
        );
        assert_eq!(parse_login_status("Not logged in"), (false, None));
    }

    #[test]
    fn parses_codex_jsonl_agent_and_mcp_events() {
        let mut calls = HashMap::new();
        assert_eq!(
            parse_exec_event(
                r#"{"type":"item.started","item":{"id":"item_1","type":"mcp_tool_call","tool":"get_timeline","arguments":{}}}"#,
                &mut calls,
            ),
            None
        );
        assert_eq!(calls["item_1"].name, "get_timeline");
        assert_eq!(calls["item_1"].result, None);

        parse_exec_event(
            r#"{"type":"item.completed","item":{"id":"item_1","type":"mcp_tool_call","tool":"get_timeline","arguments":{},"result":{},"error":null}}"#,
            &mut calls,
        );
        assert_eq!(calls["item_1"].is_error, Some(false));
        assert_eq!(
            parse_exec_event(
                r#"{"type":"item.completed","item":{"id":"item_2","type":"agent_message","text":"1280 × 720"}}"#,
                &mut calls,
            ),
            Some("1280 × 720".into())
        );
    }

    #[test]
    fn ignores_non_json_and_codex_diagnostic_events() {
        let mut calls = HashMap::new();
        assert_eq!(parse_exec_event("diagnostic", &mut calls), None);
        assert_eq!(
            parse_exec_event(r#"{"type":"thread.started"}"#, &mut calls),
            None
        );
        assert!(calls.is_empty());
    }
}
