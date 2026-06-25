//! Typed errors for the motion module. Library-style (`thiserror`) so callers can
//! match on the failure kind; messages are human-readable for surfacing to the
//! agent / UI.

/// The result alias used throughout the crate.
pub type MotionResult<T> = Result<T, MotionError>;

/// Everything that can go wrong rendering or caching a motion graphic.
#[derive(Debug, thiserror::Error)]
pub enum MotionError {
    /// The `MotionSource` itself is malformed (empty code, bad color param, ...).
    #[error("invalid motion source: {0}")]
    InvalidSource(String),

    /// The `MotionRenderRequest` has out-of-range fields (fps/duration/size).
    #[error("invalid render request: {0}")]
    InvalidRequest(String),

    /// A referenced template id was not found in the registry.
    #[error("unknown template: {0}")]
    UnknownTemplate(String),

    /// A template's manifest failed validation, or bound params don't satisfy its
    /// declared schema.
    #[error("manifest error: {0}")]
    Manifest(String),

    /// The rendering backend is unavailable (e.g. the headless-Chromium feature
    /// is not compiled in, or no Chromium binary was found). Carries actionable
    /// text so the agent can explain the gap instead of failing opaquely.
    #[error("renderer unavailable: {0}")]
    RendererUnavailable(String),

    /// The render exceeded its time budget (sandbox fuse, docs §5).
    #[error("render timed out after {0:?}")]
    Timeout(std::time::Duration),

    /// A sandbox policy was violated (e.g. a disallowed network origin).
    #[error("sandbox violation: {0}")]
    Sandbox(String),

    /// The renderer ran but produced no/partial output.
    #[error("render failed: {0}")]
    RenderFailed(String),

    /// Filesystem error while reading/writing the frame cache.
    #[error("cache io error: {0}")]
    Io(#[from] std::io::Error),
}

impl MotionError {
    pub fn invalid_source(msg: impl Into<String>) -> Self {
        MotionError::InvalidSource(msg.into())
    }
    pub fn invalid_request(msg: impl Into<String>) -> Self {
        MotionError::InvalidRequest(msg.into())
    }
    pub fn unknown_template(msg: impl Into<String>) -> Self {
        MotionError::UnknownTemplate(msg.into())
    }
    pub fn manifest(msg: impl Into<String>) -> Self {
        MotionError::Manifest(msg.into())
    }
    pub fn renderer_unavailable(msg: impl Into<String>) -> Self {
        MotionError::RendererUnavailable(msg.into())
    }
    pub fn sandbox(msg: impl Into<String>) -> Self {
        MotionError::Sandbox(msg.into())
    }
    pub fn render_failed(msg: impl Into<String>) -> Self {
        MotionError::RenderFailed(msg.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_are_descriptive() {
        assert_eq!(
            MotionError::invalid_source("empty").to_string(),
            "invalid motion source: empty"
        );
        assert_eq!(
            MotionError::unknown_template("foo").to_string(),
            "unknown template: foo"
        );
        assert!(MotionError::renderer_unavailable("no chromium")
            .to_string()
            .contains("renderer unavailable"));
    }

    #[test]
    fn io_error_converts() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "nope");
        let err: MotionError = io.into();
        assert!(matches!(err, MotionError::Io(_)));
    }
}
