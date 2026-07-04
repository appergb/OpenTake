//! Crate error type. Mirrors the upstream error enums (`ImageVideoError`,
//! `NormalizeError`, `TranscriptionError`, `DownloadError`,
//! `VisualEmbedder.ModelError`, `EmbeddingStore.StoreError`) collapsed into one
//! boundary type. `opentake-domain` is zero-IO; this crate is the first layer
//! allowed to perform IO, so it owns the IO/decode/model error surface.

use std::path::Path;

/// All fallible boundaries in this crate return `Result<T, MediaError>`.
#[derive(thiserror::Error, Debug)]
pub enum MediaError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    #[error("ffmpeg: {0}")]
    Ffmpeg(String),

    /// e.g. `NoTrack("audio", "clip.mp4")` — no audio/video track in the file.
    #[error("no {0} track in {1}")]
    NoTrack(&'static str, String),

    #[error("decode failed: {0}")]
    Decode(String),

    #[error("encode failed: {0}")]
    Encode(String),

    #[error("transcription unsupported locale: {0}")]
    UnsupportedLocale(String),

    #[error("transcription failed: {0}")]
    Transcribe(String),

    #[error("model install: {0}")]
    ModelInstall(String),

    #[error("checksum mismatch: {0}")]
    Checksum(String),

    #[error("embedding store corrupt")]
    StoreCorrupt,

    #[error("bad model output")]
    BadModelOutput,

    #[error("cancelled")]
    Cancelled,

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl MediaError {
    /// Convenience: `NoTrack` with the file's display name (lossy).
    pub(crate) fn no_track(kind: &'static str, path: &Path) -> Self {
        MediaError::NoTrack(kind, path.display().to_string())
    }
}

pub type Result<T> = std::result::Result<T, MediaError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_track_renders_kind_and_path() {
        let e = MediaError::no_track("audio", Path::new("/x/clip.mp4"));
        assert_eq!(e.to_string(), "no audio track in /x/clip.mp4");
    }

    #[test]
    fn io_error_converts_via_from() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "nope");
        let e: MediaError = io.into();
        assert!(matches!(e, MediaError::Io(_)));
        assert!(e.to_string().starts_with("io:"));
    }

    #[test]
    fn anyhow_error_converts_transparently() {
        let e: MediaError = anyhow::anyhow!("boom").into();
        assert_eq!(e.to_string(), "boom");
    }
}
