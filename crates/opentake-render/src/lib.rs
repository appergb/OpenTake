//! opentake-render — rendering pipeline.
//!
//! RenderPlan (pure function: Timeline -> per-frame composition instructions)
//! + wgpu frame compositor + ffmpeg codec backends (preview + export share one plan).
//!
//! Phase 0 scaffold: implementations land in later phases (see docs/ROADMAP.md).

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles() {}
}
