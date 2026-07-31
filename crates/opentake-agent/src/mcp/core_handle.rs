//! `CoreHandle` — the testable boundary between the dispatch shell and
//! `opentake-core` (`agent-SPEC.md` §8.1).
//!
//! The dispatcher never touches [`opentake_core::AppCore`] directly; it talks to
//! this trait. Production wiring passes an [`AppCoreHandle`] (a thin delegating
//! wrapper); tests can pass a fake in-memory handle. Keeping the surface this
//! narrow (read timeline, read media, apply one command, ask for the project dir)
//! means the whole tool-dispatch pipeline is unit-testable without a UI or a
//! transport.

use std::path::PathBuf;

use opentake_core::AppCore;
use opentake_domain::{MediaManifest, MediaResolver, Timeline};
use opentake_media::{extract_pcm, PcmBuffer, PcmSpec};
use opentake_ops::command::{EditCommand, EditResult};

/// The narrow document surface the dispatch shell needs. `Send + Sync` so a
/// `Dispatcher` holding `Arc<dyn CoreHandle>` stays shareable across threads
/// (matching [`AppCore`]'s cross-client design).
pub trait CoreHandle: Send + Sync {
    /// The current timeline snapshot (the `get_timeline` source + before/after
    /// snapshots the shell takes around every tool).
    fn timeline(&self) -> Timeline;

    /// The current media manifest (the `get_media` / `list_folders` source + the
    /// id universe for short-id expansion/shortening).
    fn media(&self) -> MediaManifest;

    /// Apply one editing command, mapping the core error into `anyhow` so the
    /// shell can turn any failure into a single `ToolResult::error`.
    fn apply(&self, cmd: EditCommand) -> anyhow::Result<EditResult>;

    /// The open project's bundle directory, or `None` for an unsaved project.
    fn project_dir(&self) -> Option<PathBuf>;

    /// Resolve an asset id to the local file path that media analysis can read.
    /// The default implementation mirrors `MediaResolver.expected_path`.
    fn media_path(&self, media_ref: &str) -> Option<PathBuf> {
        let manifest = self.media();
        let project_dir = self.project_dir();
        MediaResolver::new(&manifest, project_dir.as_deref()).expected_path(media_ref)
    }

    /// Decode a media asset's first audio track into the PCM format requested by
    /// an analysis tool. Test handles can override this to inject synthetic PCM
    /// without invoking ffmpeg.
    fn extract_analysis_pcm(
        &self,
        media_ref: &str,
        spec: PcmSpec,
        range: Option<(f64, f64)>,
    ) -> anyhow::Result<PcmBuffer> {
        let path = self
            .media_path(media_ref)
            .ok_or_else(|| anyhow::anyhow!("media path not found for mediaRef: {media_ref}"))?;
        extract_pcm(&path, &spec, range).map_err(|e| anyhow::anyhow!("{e}"))
    }
}

/// Production [`CoreHandle`] over the authoritative [`AppCore`]. A clone of the
/// `AppCore` points at the same session, so this can be constructed per request
/// without copying any document state.
pub struct AppCoreHandle(pub AppCore);

impl AppCoreHandle {
    /// Wrap an [`AppCore`] handle.
    pub fn new(core: AppCore) -> Self {
        AppCoreHandle(core)
    }
}

impl CoreHandle for AppCoreHandle {
    fn timeline(&self) -> Timeline {
        self.0.get_timeline().timeline
    }

    fn media(&self) -> MediaManifest {
        self.0.media()
    }

    fn apply(&self, cmd: EditCommand) -> anyhow::Result<EditResult> {
        self.0.apply(cmd).map_err(|e| anyhow::anyhow!("{e}"))
    }

    fn project_dir(&self) -> Option<PathBuf> {
        self.0.project_dir()
    }

    fn media_path(&self, media_ref: &str) -> Option<PathBuf> {
        let snapshot = self.0.runtime_snapshot();
        MediaResolver::new(&snapshot.media, snapshot.project_dir.as_deref())
            .expected_path(media_ref)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_domain::{ClipType, MediaManifestEntry, MediaSource};
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::Barrier;

    struct RunningGuard<'a>(&'a AtomicBool);

    impl Drop for RunningGuard<'_> {
        fn drop(&mut self) {
            self.0.store(false, Ordering::Release);
        }
    }

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "opentake-agent-core-handle-{}-{sequence}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("create fixture root");
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn project_with_media(bundle: &std::path::Path, relative_path: &str) {
        std::fs::create_dir_all(bundle).expect("create project fixture");
        std::fs::write(
            bundle.join("project.json"),
            serde_json::to_vec_pretty(&Timeline::new()).expect("encode fixture timeline"),
        )
        .expect("write fixture timeline");
        let mut manifest = MediaManifest::new();
        manifest.entries.push(MediaManifestEntry {
            id: "shared-media-id".into(),
            name: "asset.mov".into(),
            kind: ClipType::Video,
            source: MediaSource::Project {
                relative_path: relative_path.into(),
            },
            duration: 1.0,
            generation_input: None,
            source_width: None,
            source_height: None,
            source_fps: None,
            has_audio: None,
            color: None,
            proxy: None,
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        });
        std::fs::write(
            bundle.join("media.json"),
            serde_json::to_vec_pretty(&manifest).expect("encode fixture manifest"),
        )
        .expect("write fixture manifest");
    }

    #[test]
    fn app_core_media_path_stress_never_mixes_project_snapshots() {
        let fixtures = TempDir::new();
        let project_a = fixtures.0.join("A.opentake");
        let project_b = fixtures.0.join("B.opentake");
        project_with_media(&project_a, "media/a.mov");
        project_with_media(&project_b, "media/b.mov");

        let core = AppCore::new();
        let handle = AppCoreHandle::new(core.clone());
        core.open_project(&project_a).expect("open project A");
        let expected_a = project_a.join("media/a.mov");
        let expected_b = project_b.join("media/b.mov");
        let running = AtomicBool::new(true);
        let start = Barrier::new(5);

        std::thread::scope(|scope| {
            let toggler_core = core.clone();
            let toggler_start = &start;
            let toggler_running = &running;
            let toggler_a = &project_a;
            let toggler_b = &project_b;
            scope.spawn(move || {
                let _running_guard = RunningGuard(toggler_running);
                toggler_start.wait();
                for iteration in 0..1_000 {
                    let bundle = if iteration % 2 == 0 {
                        toggler_b
                    } else {
                        toggler_a
                    };
                    toggler_core
                        .open_project(bundle)
                        .expect("toggle project fixture");
                }
            });

            for _ in 0..4 {
                let reader_handle = &handle;
                let reader_start = &start;
                let reader_running = &running;
                let reader_a = &expected_a;
                let reader_b = &expected_b;
                scope.spawn(move || {
                    reader_start.wait();
                    let mut observations = 0_usize;
                    while reader_running.load(Ordering::Acquire) || observations < 100_000 {
                        let resolved = reader_handle
                            .media_path("shared-media-id")
                            .expect("open project always resolves shared media");
                        assert!(
                            resolved == *reader_a || resolved == *reader_b,
                            "manifest and project directory came from different snapshots: {}",
                            resolved.display()
                        );
                        observations += 1;
                        if observations.is_multiple_of(128) {
                            std::thread::yield_now();
                        }
                    }
                });
            }
        });
    }
}
