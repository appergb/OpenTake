//! Security sandbox policy for rendering untrusted motion-graphic code.
//!
//! Motion code is authored by the agent or, worse, pulled from a community
//! template. It is rendered in an isolated offscreen engine. docs/
//! MOTION-GRAPHICS-PLUGIN.md §5 spells out the requirements; this module makes
//! them *types* so a renderer can't forget to apply one:
//!
//! - **Network is denied by default.** Only an explicit allowlist of origins is
//!   reachable, and (per web/security.md) those should be pinned with SRI and a
//!   strict CSP. The default [`SandboxPolicy`] has an empty allowlist ⇒ fully
//!   offline, which is also what keeps tests/CI deterministic.
//! - **A render time budget** fuses runaway animations / `while(true)` scripts.
//! - **No filesystem / project access.** The renderer must run the engine with
//!   no access to user files; only declared template params are injected. This is
//!   an engine-launch invariant (flags + profile), asserted here by the *absence*
//!   of any path-granting field — there is intentionally nothing to set.
//! - **A content-size ceiling** bounds inline document size before it ever
//!   reaches the engine.
//!
//! Enforcement of the network/CSP parts happens in the real CDP backend (gated
//! behind the `chromium` feature); the policy type and its pure checks
//! ([`SandboxPolicy::check_url`], [`SandboxPolicy::check_document_size`]) live
//! here and are unit-tested without any engine.

use std::time::Duration;

use crate::error::{MotionError, MotionResult};

/// Default per-render time budget. Generous enough for a few seconds of complex
/// animation across hundreds of frames, tight enough to fuse a hang.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// Default ceiling on an inline document's byte length (256 KiB). A motion
/// graphic is markup + a little script; anything larger is suspicious and should
/// ship as a template package with audited assets instead.
pub const DEFAULT_MAX_DOCUMENT_BYTES: usize = 256 * 1024;

/// An allowed network origin (scheme + host[:port]), e.g.
/// `https://cdn.jsdelivr.net`. Compared case-insensitively by exact prefix on the
/// request URL's origin. We deliberately do NOT support wildcards: each origin a
/// template needs must be named explicitly (web/security.md: no cargo-culted
/// broad `connect-src`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AllowedOrigin(String);

impl AllowedOrigin {
    /// Build an origin, normalizing to lowercase and trimming a trailing slash.
    /// Returns `None` for anything that isn't an `https://` (or `http://` for a
    /// local dev origin) URL — plaintext remote origins are refused outright.
    pub fn parse(origin: &str) -> Option<Self> {
        let lower = origin.trim().trim_end_matches('/').to_ascii_lowercase();
        let is_https = lower.starts_with("https://");
        // Allow http only for loopback dev servers; never for remote hosts.
        let is_local_http = lower.starts_with("http://localhost")
            || lower.starts_with("http://127.0.0.1")
            || lower.starts_with("http://[::1]");
        if (is_https || is_local_http) && lower.len() > "https://".len() {
            Some(AllowedOrigin(lower))
        } else {
            None
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The sandbox policy applied to a single render.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SandboxPolicy {
    /// Origins the rendered document may reach. Empty ⇒ network fully denied.
    pub allowed_origins: Vec<AllowedOrigin>,
    /// Render time budget; exceeding it aborts the render with
    /// [`MotionError::Timeout`].
    pub timeout: Duration,
    /// Max inline-document byte length accepted before rendering.
    pub max_document_bytes: usize,
}

impl Default for SandboxPolicy {
    /// The safe default: **no network**, default timeout, default size ceiling.
    /// This is what the agent's inline `Code` graphics run under unless a vetted
    /// template explicitly widens it.
    fn default() -> Self {
        SandboxPolicy {
            allowed_origins: Vec::new(),
            timeout: DEFAULT_TIMEOUT,
            max_document_bytes: DEFAULT_MAX_DOCUMENT_BYTES,
        }
    }
}

impl SandboxPolicy {
    /// An offline policy with a custom timeout (the common test/CI knob).
    pub fn offline_with_timeout(timeout: Duration) -> Self {
        SandboxPolicy {
            timeout,
            ..Default::default()
        }
    }

    /// Add an allowed origin, ignoring un-parseable / plaintext-remote inputs.
    /// Chainable.
    pub fn allow_origin(mut self, origin: &str) -> Self {
        if let Some(o) = AllowedOrigin::parse(origin) {
            if !self.allowed_origins.contains(&o) {
                self.allowed_origins.push(o);
            }
        }
        self
    }

    /// `true` when no remote origins are permitted (the deterministic default).
    pub fn is_offline(&self) -> bool {
        self.allowed_origins.is_empty()
    }

    /// Decide whether a request URL is permitted under this policy. A URL is
    /// allowed iff its (lowercased) start matches one of the allowlisted origins.
    /// With an empty allowlist every remote URL is denied. `data:` URIs are
    /// always allowed (inline, no network).
    pub fn check_url(&self, url: &str) -> MotionResult<()> {
        let lower = url.trim().to_ascii_lowercase();
        if lower.starts_with("data:") {
            return Ok(());
        }
        let allowed = self
            .allowed_origins
            .iter()
            .any(|o| lower.starts_with(o.as_str()));
        if allowed {
            Ok(())
        } else {
            Err(MotionError::sandbox(format!(
                "network access to {url:?} is not in the allowlist"
            )))
        }
    }

    /// Reject an inline document larger than the configured ceiling.
    pub fn check_document_size(&self, document: &str) -> MotionResult<()> {
        if document.len() > self.max_document_bytes {
            return Err(MotionError::sandbox(format!(
                "document is {} bytes, over the {}-byte limit",
                document.len(),
                self.max_document_bytes
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_is_offline() {
        let p = SandboxPolicy::default();
        assert!(p.is_offline());
        assert_eq!(p.timeout, DEFAULT_TIMEOUT);
        // Any remote URL is denied.
        assert!(p.check_url("https://evil.example.com/x.js").is_err());
        // data: URIs are always fine (inline, no network).
        assert!(p.check_url("data:text/css,body{}").is_ok());
    }

    #[test]
    fn allowlist_permits_only_named_origins() {
        let p = SandboxPolicy::default().allow_origin("https://cdn.jsdelivr.net/");
        assert!(!p.is_offline());
        assert!(p.check_url("https://cdn.jsdelivr.net/npm/gsap").is_ok());
        // different host -> denied
        assert!(p.check_url("https://unpkg.com/thing").is_err());
        // origin stored without trailing slash, case-insensitive match
        assert!(p.check_url("HTTPS://CDN.JSDELIVR.NET/a").is_ok());
    }

    #[test]
    fn plaintext_remote_origin_is_refused() {
        // http:// to a remote host is not a valid allowlist entry.
        assert!(AllowedOrigin::parse("http://cdn.evil.com").is_none());
        // but loopback http is allowed for local dev servers
        assert!(AllowedOrigin::parse("http://localhost:5173").is_some());
        assert!(AllowedOrigin::parse("https://example.com").is_some());
        // junk
        assert!(AllowedOrigin::parse("ftp://x").is_none());
        assert!(AllowedOrigin::parse("https://").is_none());
    }

    #[test]
    fn duplicate_origins_are_deduped() {
        let p = SandboxPolicy::default()
            .allow_origin("https://a.com")
            .allow_origin("https://a.com/");
        assert_eq!(p.allowed_origins.len(), 1);
    }

    #[test]
    fn document_size_ceiling_enforced() {
        let p = SandboxPolicy {
            max_document_bytes: 10,
            ..Default::default()
        };
        assert!(p.check_document_size("under10").is_ok());
        assert!(p.check_document_size("this is way over ten bytes").is_err());
    }

    #[test]
    fn offline_with_timeout_keeps_empty_allowlist() {
        let p = SandboxPolicy::offline_with_timeout(Duration::from_secs(5));
        assert!(p.is_offline());
        assert_eq!(p.timeout, Duration::from_secs(5));
    }
}
