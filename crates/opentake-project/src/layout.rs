//! `.opentake` bundle layout constants.
//!
//! Mirrors upstream's `enum Project` file-name contract
//! (`Utilities/Constants.swift`) so existing `project.json` / `media.json`
//! files round-trip, with one OpenTake-specific change: the chat-session
//! directory is `chat-sessions/` (per `docs/ARCHITECTURE.md` §9) instead of
//! upstream's `chat/`. See [`CHAT_SESSIONS_DIR`].

use std::path::{Path, PathBuf};

/// Canonical extension of an OpenTake project directory (without the dot).
pub const BUNDLE_EXTENSION: &str = "opentake";

/// `project.json` — the serialized [`opentake_domain::Timeline`].
pub const TIMELINE_FILE: &str = "project.json";

/// `media.json` — the serialized [`opentake_domain::MediaManifest`].
pub const MANIFEST_FILE: &str = "media.json";

/// `generation-log.json` — the serialized [`crate::GenerationLog`] (optional).
pub const GENERATION_LOG_FILE: &str = "generation-log.json";

/// `thumbnail.jpg` — JPEG cover image (optional).
pub const THUMBNAIL_FILE: &str = "thumbnail.jpg";

/// `media/` — directory holding project-internal media files. `.project`
/// relative paths in the manifest are resolved against the bundle root, and by
/// convention point inside this directory.
pub const MEDIA_DIR: &str = "media";

/// `chat-sessions/` — one `<session>.json` per agent chat session.
///
/// OpenTake-specific: upstream stores these under `chat/`
/// (`ChatSessionStore.dirName`). The `.opentake` format renames it to
/// `chat-sessions/`; readers should treat `chat/` as a legacy fallback if
/// migration of old `.palmier` bundles is ever needed (not done here).
pub const CHAT_SESSIONS_DIR: &str = "chat-sessions";

/// Absolute path to `project.json` inside `bundle`.
pub fn timeline_path(bundle: &Path) -> PathBuf {
    bundle.join(TIMELINE_FILE)
}

/// Absolute path to `media.json` inside `bundle`.
pub fn manifest_path(bundle: &Path) -> PathBuf {
    bundle.join(MANIFEST_FILE)
}

/// Absolute path to `generation-log.json` inside `bundle`.
pub fn generation_log_path(bundle: &Path) -> PathBuf {
    bundle.join(GENERATION_LOG_FILE)
}

/// Absolute path to `thumbnail.jpg` inside `bundle`.
pub fn thumbnail_path(bundle: &Path) -> PathBuf {
    bundle.join(THUMBNAIL_FILE)
}

/// Absolute path to the `media/` directory inside `bundle`.
pub fn media_dir(bundle: &Path) -> PathBuf {
    bundle.join(MEDIA_DIR)
}

/// Absolute path to the `chat-sessions/` directory inside `bundle`.
pub fn chat_sessions_dir(bundle: &Path) -> PathBuf {
    bundle.join(CHAT_SESSIONS_DIR)
}
