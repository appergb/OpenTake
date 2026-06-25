//! opentake-core — the assembly crate.
//!
//! This is the layer that wires `opentake-{domain,ops,project}` (and, via
//! injected handles, the still-unfinished render/media/gen crates) into **one
//! authoritative, observable editing session** exposing **a single editing entry
//! point** to its three peer clients (UI, in-app agent, MCP). It is an assembly
//! layer, not an editing layer: it owns no frame arithmetic, no overlap solving,
//! and no transaction logic of its own.
//!
//! ## What lives here vs. what is delegated
//!
//! | Concern | Owner |
//! |---|---|
//! | Editing algorithms + the snapshot/commit/version transaction + undo/redo | [`opentake_ops`] ([`opentake_ops::command::apply`], [`opentake_ops::EditorState`]) |
//! | `.opentake` bundle read/write | [`opentake_project`] ([`opentake_project::Project`]) |
//! | Value-type model (Timeline, Clip, MediaManifest, …) | [`opentake_domain`] |
//! | Preview / export / media import / generation | injected [`deps`] traits (later phases) |
//! | **Session assembly, mutation serialization, version exposure, event broadcast, Tauri DTO surface** | **this crate** |
//!
//! ## The pieces
//!
//! - [`AppCore`] — the cloneable façade over `Arc<Mutex<EditorSession>>`; the
//!   single editing entry point ([`AppCore::apply`]) plus undo/redo, project
//!   lifecycle, reads, and event broadcasting ([`core`]).
//! - [`EditorSession`] — the in-memory document: an [`opentake_ops::EditorState`]
//!   plus the bundle path and generation log it needs to round-trip ([`session`]).
//! - [`CoreEvent`] / [`EventBus`] — the one-way change-notification channel that
//!   replaces upstream's `@Observable` across the process boundary ([`events`]).
//! - [`CoreDeps`] — injected capability backends, stubbed with
//!   [`deps::UnsupportedBackends`] until their crates land ([`deps`]).
//! - [`dto`] — the Tauri command surface as plain DTOs + handler functions (no
//!   `tauri` dependency); `src-tauri` adds thin `#[tauri::command]` shims.
//! - [`CoreError`] — the unified error type for the boundary ([`error`]).
//!
//! Editing commands are re-exported from [`opentake_ops`] so callers depend on
//! just this crate to drive the editor: see [`EditCommand`] / [`EditResult`].

pub mod core;
pub mod deps;
pub mod dto;
pub mod error;
pub mod events;
pub mod session;

// --- Assembly façade ---
pub use crate::core::{AppCore, TimelineSnapshot};
pub use session::{
    importable_clip_type, EditorSession, ProbedMedia, SUPPORTED_AUDIO_EXTENSIONS,
    SUPPORTED_IMAGE_EXTENSIONS, SUPPORTED_VIDEO_EXTENSIONS,
};

// --- Events ---
pub use events::{CoreEvent, EventBus, SubscriptionId};

// --- Injected capabilities ---
pub use deps::{CoreDeps, ExportBackend, GenBackend, MediaImporter, PreviewBackend};

// --- Errors ---
pub use error::{CoreError, Result};

// --- Tauri boundary DTOs ---
pub use dto::{CmdError, EditResultDto, TimelineSnapshotDto};

// --- Re-exported editing API (so downstream needs only opentake-core) ---
pub use opentake_ops::command::{EditCommand, EditError, EditResult};
pub use opentake_ops::{EditorState, IdGen, SeqIdGen};
