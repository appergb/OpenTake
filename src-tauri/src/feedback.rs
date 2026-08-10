//! Privacy-bounded feedback submission.
//!
//! The app stays offline unless an operator explicitly configures an HTTPS
//! endpoint through `OPENTAKE_FEEDBACK_ENDPOINT`. Every request is built through
//! [`FeedbackSubmission`], which attaches version metadata and serializes only
//! allow-listed feedback fields.

use std::fmt;
use std::process::Command;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::State;

const MAX_MESSAGE_CHARS: usize = 10_000;
const MAX_EMAIL_CHARS: usize = 320;
const MAX_SCREENSHOT_BASE64_CHARS: usize = 12 * 1024 * 1024;
const ENDPOINT_ENV: &str = "OPENTAKE_FEEDBACK_ENDPOINT";

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FeedbackDraft {
    pub message: String,
    pub email: Option<String>,
    pub may_contact: bool,
    pub screenshot_png_base64: Option<String>,
}

impl fmt::Debug for FeedbackDraft {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FeedbackDraft")
            .field("message", &"[REDACTED]")
            .field("email", &self.email.as_ref().map(|_| "[REDACTED]"))
            .field("may_contact", &(self.may_contact && self.email.is_some()))
            .field(
                "screenshot_png_base64",
                &self.screenshot_png_base64.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedbackSubmission {
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<String>,
    may_contact: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    screenshot_png_base64: Option<String>,
    app_version: String,
    os_version: String,
}

impl fmt::Debug for FeedbackSubmission {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FeedbackSubmission")
            .field("message", &"[REDACTED]")
            .field("email", &self.email.as_ref().map(|_| "[REDACTED]"))
            .field("may_contact", &self.may_contact)
            .field(
                "screenshot_png_base64",
                &self.screenshot_png_base64.as_ref().map(|_| "[REDACTED]"),
            )
            .field("app_version", &self.app_version)
            .field("os_version", &self.os_version)
            .finish()
    }
}

impl FeedbackSubmission {
    pub fn from_draft_with_versions(
        draft: FeedbackDraft,
        package_version: Option<&str>,
        build_version: Option<&str>,
        os_version: Option<&str>,
    ) -> Result<Self, String> {
        let original_message_chars = draft.message.chars().count();
        let message = draft.message.trim().to_owned();
        if message.is_empty() {
            return Err("feedback message must not be empty".into());
        }
        if original_message_chars > MAX_MESSAGE_CHARS {
            return Err(format!(
                "feedback message must not exceed {MAX_MESSAGE_CHARS} characters"
            ));
        }

        let email = draft
            .email
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        if email
            .as_ref()
            .is_some_and(|value| value.chars().count() > MAX_EMAIL_CHARS)
        {
            return Err(format!(
                "feedback email must not exceed {MAX_EMAIL_CHARS} characters"
            ));
        }
        if draft
            .screenshot_png_base64
            .as_ref()
            .is_some_and(|value| value.len() > MAX_SCREENSHOT_BASE64_CHARS)
        {
            return Err("feedback screenshot is too large".into());
        }

        let package_version = non_empty_or(package_version, "?");
        let build_version = non_empty_or(build_version, "?");
        let os_version = os_version
            .and_then(normalize_os_version)
            .unwrap_or_else(|| "?.?.?".into());
        let may_contact = draft.may_contact && email.is_some();

        Ok(Self {
            message,
            email,
            may_contact,
            screenshot_png_base64: draft.screenshot_png_base64,
            app_version: format!("{package_version} ({build_version})"),
            os_version,
        })
    }

    fn from_runtime(draft: FeedbackDraft) -> Result<Self, String> {
        Self::from_draft_with_versions(
            draft,
            Some(env!("CARGO_PKG_VERSION")),
            option_env!("OPENTAKE_BUILD_NUMBER"),
            detect_os_version().as_deref(),
        )
    }
}

fn non_empty_or<'a>(value: Option<&'a str>, fallback: &'a str) -> &'a str {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback)
}

fn normalize_os_version(raw: &str) -> Option<String> {
    let mut components = raw.split(['.', '-']).filter_map(|component| {
        let digits: String = component
            .chars()
            .skip_while(|character| !character.is_ascii_digit())
            .take_while(|character| character.is_ascii_digit())
            .collect();
        (!digits.is_empty()).then_some(digits)
    });
    let major = components.next()?;
    let minor = components.next().unwrap_or_else(|| "0".into());
    let patch = components.next().unwrap_or_else(|| "0".into());
    Some(format!("{major}.{minor}.{patch}"))
}

fn detect_os_version() -> Option<String> {
    #[cfg(target_os = "macos")]
    let output = Command::new("/usr/bin/sw_vers")
        .args(["-productVersion"])
        .output()
        .ok()?;

    #[cfg(all(unix, not(target_os = "macos")))]
    let output = Command::new("uname").arg("-r").output().ok()?;

    #[cfg(target_os = "windows")]
    let output = Command::new("cmd").args(["/C", "ver"]).output().ok()?;

    if !output.status.success() {
        return None;
    }
    normalize_os_version(&String::from_utf8(output.stdout).ok()?)
}

pub struct FeedbackState {
    client: Option<reqwest::Client>,
    endpoint: Option<reqwest::Url>,
    configuration_error: Option<String>,
}

impl Default for FeedbackState {
    fn default() -> Self {
        let configured = std::env::var(ENDPOINT_ENV).ok();
        let (endpoint, mut configuration_error) = match configured.as_deref() {
            None | Some("") => (None, None),
            Some(value) => match reqwest::Url::parse(value) {
                Ok(url) if url.scheme() == "https" => (Some(url), None),
                Ok(_) => (None, Some(format!("{ENDPOINT_ENV} must use HTTPS"))),
                Err(error) => (None, Some(format!("{ENDPOINT_ENV} is invalid: {error}"))),
            },
        };
        let client = match reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(15))
            .build()
        {
            Ok(client) => Some(client),
            Err(error) => {
                configuration_error.get_or_insert_with(|| {
                    format!("feedback HTTP client could not be initialized: {error}")
                });
                None
            }
        };
        Self {
            client,
            endpoint,
            configuration_error,
        }
    }
}

impl FeedbackState {
    async fn send(&self, submission: &FeedbackSubmission) -> Result<(), String> {
        if let Some(error) = &self.configuration_error {
            return Err(error.clone());
        }
        let endpoint = self.endpoint.as_ref().ok_or_else(|| {
            "feedback is unavailable until an HTTPS endpoint is configured".to_owned()
        })?;
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| "feedback HTTP client is unavailable".to_owned())?;
        let response = client
            .post(endpoint.clone())
            .json(submission)
            .send()
            .await
            .map_err(|error| format!("feedback request failed: {error}"))?;
        response
            .error_for_status()
            .map_err(|error| format!("feedback service rejected the submission: {error}"))?;
        Ok(())
    }
}

#[tauri::command]
pub async fn submit_feedback(
    state: State<'_, FeedbackState>,
    admission: State<'_, crate::updater::InstallAdmissionGate>,
    draft: FeedbackDraft,
) -> Result<(), String> {
    let _activity = crate::updater::begin_mutating_activity(&admission)?;
    let submission = FeedbackSubmission::from_runtime(draft)?;
    state.send(&submission).await
}

#[cfg(test)]
mod tests {
    use super::normalize_os_version;

    #[test]
    fn normalizes_platform_version_formats() {
        assert_eq!(normalize_os_version("15.5"), Some("15.5.0".into()));
        assert_eq!(normalize_os_version("6.8.12-arch1"), Some("6.8.12".into()));
        assert_eq!(
            normalize_os_version("Microsoft Windows [Version 10.0.26100.1]"),
            Some("10.0.26100".into())
        );
        assert_eq!(normalize_os_version("unknown"), None);
    }
}
