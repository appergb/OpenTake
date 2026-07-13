//! Continuous Rust streaming playback engine (#53), gated behind the
//! `playback-engine` feature (a DEFAULT feature — this is the shipped preview
//! path; `--no-default-features` drops it for a minimal build).
//!
//! Structure: continuous per-clip decode ([`resolver`]) feeding the
//! same-pixel-path compositor on a dedicated render thread ([`engine`]), behind
//! clock / frame-sink / playhead-emitter traits, with the timeline→render
//! projections in [`project`]. The cpal master clock ([`audio`]) + MJPEG
//! transport ([`transport`]) realise those traits, and [`commands`] registers the
//! `playback_*` Tauri commands the front end drives during PLAY.
//!
//! A few public items stay exercised only by the gated GPU+ffmpeg integration
//! tests or one build-feature matrix, so the module keeps a `dead_code` allow.
#![allow(dead_code)]

pub mod audio;
pub mod commands;
pub mod engine;
pub mod project;
pub mod resolver;
pub mod session;
pub mod transport;

pub use engine::{
    FrameSink, InstantClock, PlaybackClock, PlaybackCmd, PlaybackEngine, PlayheadEmitter,
    RenderLoop,
};
pub use project::{project_media, project_text, ManifestMetrics, MediaInfo, TextInfo};
pub use resolver::{PlaybackResolverState, StreamingResolver};

pub use commands::PlaybackState;
pub use transport::{MjpegSink, PreviewServer, TauriPlayheadEmitter};
