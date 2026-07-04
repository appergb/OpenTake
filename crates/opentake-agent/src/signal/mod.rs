//! Context-signal layer (`agent-SPEC.md` §6): video-type classification, track-
//! role detection + advice, editing-stage/skeleton/guidance, the built-in rule
//! checks, and the engine that attaches a `context_signal` block to a tool
//! result after the tool runs (before short-id shortening). Types are defined in
//! `opentake-domain`; this layer only generates + attaches them.

pub mod classify;
pub mod engine;
pub mod rules;
pub mod stages;
pub mod track_roles;
