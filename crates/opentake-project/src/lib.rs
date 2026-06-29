//! opentake-project — `.opentake` project bundle persistence.
//!
//! Reads and writes the `.opentake` directory bundle and stays wire-compatible
//! with upstream PalmierPro's `.palmier` package so existing `project.json` /
//! `media.json` files round-trip with no semantic drift — every field decodes
//! and re-encodes to the same value upstream would. Compatibility is at the
//! field/value level, not byte-for-byte: this crate writes pretty-printed JSON
//! whereas upstream's bare `JSONEncoder()` emits compact output, so the two
//! differ in whitespace (and key order) even when semantically identical.
//!
//! ## Bundle layout (`docs/ARCHITECTURE.md` §9)
//!
//! ```text
//! Name.opentake/
//! ├── project.json         # Timeline
//! ├── media.json           # MediaManifest (entries + folders)
//! ├── generation-log.json  # GenerationLog (AI generation audit, optional)
//! ├── thumbnail.jpg        # cover image (optional)
//! ├── media/               # project-internal media (.project relative paths)
//! └── chat-sessions/       # agent chat history, one <session>.json each
//! ```
//!
//! ## What this crate provides
//!
//! - [`Project::open`] / [`Project::save`] — bundle read/write with upstream's
//!   tolerance rules (mandatory `project.json`, strict `media.json`, lenient
//!   `generation-log.json`).
//! - [`GenerationLog`] / [`GenerationLogEntry`] — the generation audit log,
//!   including the legacy dollar-cost → credits migration.
//! - [`archive`] — the equivalent of upstream `PalmierProjectExporter`: collect
//!   resolvable media into the destination `media/` directory and rewrite the
//!   manifest to bundle-relative paths.
//! - [`layout`] — the bundle file-name contract.
//! - Timeline-interchange exporters: [`fcpxml::export_xmeml`] (XMEML 4 / FCP7
//!   XML — Premiere・DaVinci・剪映), [`edl::export_edl`] (CMX3600 EDL),
//!   [`otio::export_otio`] (OpenTimelineIO JSON), and
//!   [`fcpxml_modern::export_fcpxml`] (native Final Cut Pro X FCPXML 1.10).
//!   [`xmlnode`] is the shared XML document tree the XML emitters render through.
//!
//! The [`Timeline`](opentake_domain::Timeline),
//! [`MediaManifest`](opentake_domain::MediaManifest), and related value types
//! come from `opentake-domain`; this crate only adds IO and the
//! generation-log type that the domain layer (intentionally zero-IO) omits.

pub mod archive;
pub mod bundle;
pub mod edl;
pub mod error;
pub mod fcpxml;
pub mod fcpxml_modern;
pub mod gen_log;
pub mod layout;
pub mod otio;
pub mod xmlnode;

pub use archive::{archive, ArchiveReport, MissingMedia};
pub use bundle::Project;
pub use edl::export_edl;
pub use error::{ProjectError, Result};
pub use fcpxml::export_xmeml;
pub use fcpxml_modern::export_fcpxml;
pub use gen_log::{GenerationLog, GenerationLogEntry};
pub use otio::export_otio;

// Re-export the domain types a caller needs to construct/inspect a project, so
// downstream crates can depend on just `opentake-project` for persistence work.
pub use opentake_domain::{MediaManifest, MediaManifestEntry, MediaSource, Timeline};
