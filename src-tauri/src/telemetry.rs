//! Explicit, privacy-bounded error telemetry initialization.
//!
//! No SDK client is created unless a non-empty, valid DSN is supplied by the
//! packaged build or the current process environment. Event preprocessing drops
//! request/user payloads and redacts paths and credential-shaped text.

use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;

use sentry::protocol::{Event, Stacktrace};
use sentry::types::Dsn;

const ENV_DSN: &str = "OPENTAKE_SENTRY_DSN";
const STANDARD_ENV_DSN: &str = "SENTRY_DSN";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TelemetrySource {
    Packaged,
    Environment,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TelemetryInitStatus {
    Disabled,
    InvalidConfiguration,
    StartFailed,
    Started(TelemetrySource),
}

pub struct TelemetryOptions {
    pub dsn: Dsn,
    pub source: TelemetrySource,
    pub send_default_pii: bool,
    pub capture_failed_requests: bool,
    pub traces_sample_rate: f32,
}

impl fmt::Debug for TelemetryOptions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TelemetryOptions")
            .field("dsn", &"[REDACTED]")
            .field("source", &self.source)
            .field("send_default_pii", &self.send_default_pii)
            .field("capture_failed_requests", &self.capture_failed_requests)
            .field("traces_sample_rate", &self.traces_sample_rate)
            .finish()
    }
}

pub fn init_telemetry_with(
    packaged_dsn: Option<&str>,
    environment_dsn: Option<&str>,
    start: impl FnOnce(&TelemetryOptions) -> Result<(), ()>,
) -> TelemetryInitStatus {
    let candidate = environment_dsn
        .and_then(non_empty)
        .map(|dsn| (dsn, TelemetrySource::Environment))
        .or_else(|| {
            packaged_dsn
                .and_then(non_empty)
                .map(|dsn| (dsn, TelemetrySource::Packaged))
        });
    let Some((raw_dsn, source)) = candidate else {
        return TelemetryInitStatus::Disabled;
    };
    let Ok(dsn) = raw_dsn.parse::<Dsn>() else {
        return TelemetryInitStatus::InvalidConfiguration;
    };
    let options = TelemetryOptions {
        dsn,
        source,
        send_default_pii: false,
        capture_failed_requests: false,
        traces_sample_rate: 0.1,
    };
    if start(&options).is_err() {
        TelemetryInitStatus::StartFailed
    } else {
        TelemetryInitStatus::Started(source)
    }
}

fn non_empty(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

pub struct TelemetryRuntime {
    _guard: Option<sentry::ClientInitGuard>,
    pub status: TelemetryInitStatus,
}

pub fn init_telemetry() -> TelemetryRuntime {
    let environment_dsn = std::env::var(ENV_DSN)
        .ok()
        .and_then(|value| non_empty(&value).map(str::to_owned))
        .or_else(|| {
            std::env::var(STANDARD_ENV_DSN)
                .ok()
                .and_then(|value| non_empty(&value).map(str::to_owned))
        });
    let packaged_dsn = option_env!("OPENTAKE_PACKAGED_SENTRY_DSN");
    let mut guard = None;
    let status = init_telemetry_with(packaged_dsn, environment_dsn.as_deref(), |options| {
        guard = Some(start_sentry(options));
        Ok(())
    });
    TelemetryRuntime {
        _guard: guard,
        status,
    }
}

fn start_sentry(options: &TelemetryOptions) -> sentry::ClientInitGuard {
    let release = format!(
        "opentake@{}+{}",
        env!("CARGO_PKG_VERSION"),
        option_env!("OPENTAKE_BUILD_NUMBER").unwrap_or("?")
    );
    let environment = if cfg!(debug_assertions) {
        "development"
    } else {
        "production"
    };
    let mut client_options =
        sentry::ClientOptions::default().traces_sample_rate(options.traces_sample_rate);
    client_options.dsn = Some(options.dsn.clone());
    client_options.release = Some(Cow::Owned(release));
    client_options.environment = Some(Cow::Borrowed(environment));
    client_options.attach_stacktrace = true;
    client_options.send_default_pii = options.send_default_pii;
    client_options.max_request_body_size = sentry::MaxRequestBodySize::None;
    client_options.enable_logs = false;
    client_options.enable_metrics = false;
    client_options.auto_session_tracking = false;
    client_options.before_send = Some(Arc::new(|event| Some(scrub_event(event))));
    client_options.before_breadcrumb = Some(Arc::new(|mut breadcrumb| {
        breadcrumb.message = breadcrumb
            .message
            .map(|value| redact_sensitive_text(&value));
        breadcrumb.data.clear();
        Some(breadcrumb)
    }));
    sentry::init(client_options)
}

fn scrub_event(mut event: Event<'static>) -> Event<'static> {
    event.user = None;
    event.request = None;
    event.server_name = None;
    event.extra.clear();
    event.message = event.message.map(|value| redact_sensitive_text(&value));
    event.culprit = event.culprit.map(|value| redact_sensitive_text(&value));
    event.transaction = event.transaction.map(|value| redact_sensitive_text(&value));
    for value in event.tags.values_mut() {
        *value = redact_sensitive_text(value);
    }
    if let Some(entry) = &mut event.logentry {
        entry.message = redact_sensitive_text(&entry.message);
        entry.params.clear();
    }
    for breadcrumb in &mut event.breadcrumbs {
        breadcrumb.message = breadcrumb
            .message
            .take()
            .map(|value| redact_sensitive_text(&value));
        breadcrumb.data.clear();
    }
    for exception in &mut event.exception {
        exception.value = exception
            .value
            .take()
            .map(|value| redact_sensitive_text(&value));
        scrub_stacktrace(exception.stacktrace.as_mut());
        scrub_stacktrace(exception.raw_stacktrace.as_mut());
    }
    scrub_stacktrace(event.stacktrace.as_mut());
    for thread in &mut event.threads {
        thread.name = thread
            .name
            .take()
            .map(|value| redact_sensitive_text(&value));
        scrub_stacktrace(thread.stacktrace.as_mut());
        scrub_stacktrace(thread.raw_stacktrace.as_mut());
    }
    event
}

fn scrub_stacktrace(stacktrace: Option<&mut Stacktrace>) {
    let Some(stacktrace) = stacktrace else {
        return;
    };
    for frame in &mut stacktrace.frames {
        frame.abs_path = None;
        frame.package = frame
            .package
            .take()
            .map(|value| redact_sensitive_text(&value));
        frame.pre_context.clear();
        frame.context_line = None;
        frame.post_context.clear();
        frame.vars.clear();
    }
    stacktrace.registers.clear();
}

pub fn redact_sensitive_text(input: &str) -> String {
    let mut redact_next = false;
    input
        .split_whitespace()
        .map(|word| {
            if redact_next {
                redact_next = false;
                return "[REDACTED]".to_owned();
            }
            let trimmed = word.trim_matches(|character: char| {
                matches!(character, '"' | '\'' | '(' | ')' | '[' | ']' | ',' | ';')
            });
            let lower = trimmed.to_ascii_lowercase();
            if lower == "bearer" {
                redact_next = true;
                return word.to_owned();
            }
            if is_sensitive_assignment(&lower) {
                return format!(
                    "{}=[REDACTED]",
                    trimmed.split(['=', ':']).next().unwrap_or("secret")
                );
            }
            if looks_like_private_path(trimmed) {
                return "[PATH]".to_owned();
            }
            word.to_owned()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_sensitive_assignment(lower: &str) -> bool {
    ["api_key", "apikey", "token", "password", "passwd", "secret"]
        .iter()
        .any(|key| lower.starts_with(&format!("{key}=")) || lower.starts_with(&format!("{key}:")))
}

fn looks_like_private_path(value: &str) -> bool {
    value.starts_with('/')
        || value.starts_with("~/")
        || value.contains("/Users/")
        || value.contains("/home/")
        || (value.len() > 3
            && value.as_bytes()[1] == b':'
            && matches!(value.as_bytes()[2], b'\\' | b'/'))
}

#[cfg(test)]
mod tests {
    use super::redact_sensitive_text;

    #[test]
    fn redacts_unix_windows_and_credential_shapes() {
        let text = redact_sensitive_text(
            r#"/home/alice/project C:\Users\alice\project password:hunter2 Bearer token-value"#,
        );
        assert_eq!(text, "[PATH] [PATH] password=[REDACTED] Bearer [REDACTED]");
    }
}
