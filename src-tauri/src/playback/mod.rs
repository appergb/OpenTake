//! Continuous Rust streaming playback engine (#53), gated behind the
//! `playback-engine` feature.
//!
//! PR1 (this slice) lands the **headless core**: continuous per-clip decode
//! ([`resolver`]) feeding the same-pixel-path compositor on a dedicated render
//! thread ([`engine`]), behind clock / frame-sink / playhead-emitter traits, with
//! the timeline→render projections in [`project`]. Nothing here is wired to a
//! Tauri command or the front end yet.
//!
//! PR2 adds the cpal master clock + MJPEG transport and registers the
//! `playback_*` commands; PR3 switches the front end's PLAY path over. Several
//! public items are therefore intentionally unused until PR2/PR3 wires them —
//! hence the module-scoped `dead_code` allow.
#![allow(dead_code)]

pub mod audio;
pub mod commands;
pub mod engine;
pub mod project;
pub mod resolver;
pub mod transport;

pub use engine::{
    FrameSink, InstantClock, PlaybackClock, PlaybackCmd, PlaybackEngine, PlayheadEmitter,
    RenderLoop,
};
pub use project::{project_media, project_text, ManifestMetrics, MediaInfo, TextInfo};
pub use resolver::{PlaybackResolverState, StreamingResolver};

pub use commands::PlaybackState;
pub use transport::{MjpegSink, PreviewServer, TauriPlayheadEmitter};
