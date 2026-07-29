//! The `.opentake` directory bundle: in-memory [`Project`] plus
//! [`Project::open`] / [`Project::save`].
//!
//! Port of `VideoProject`'s persistence (`Project/VideoProject.swift`), minus
//! the AppKit `NSDocument` / `FileWrapper` machinery. A bundle is a plain
//! directory; we read and write its files by path.
//!
//! Read semantics match upstream `read(from:)`:
//! - `project.json` is mandatory; absence is [`ProjectError::MissingTimeline`]
//!   (upstream throws `fileReadCorruptFile`).
//! - `media.json`, if present, is parsed strictly; a parse failure is an error
//!   (upstream throws `fileReadCorruptFile`).
//! - `generation-log.json`, if present, is parsed leniently; a parse failure
//!   yields an in-memory `None` recovery (upstream `try?`) plus a compatibility
//!   blocker, so the damaged bytes remain readable but cannot be overwritten.
//!
//! Write semantics follow the architecture note "assemble an in-memory
//! snapshot, then write atomically": each JSON component is written to a
//! sibling temp file and renamed into place, so a crash never leaves a
//! half-written `project.json`. `save` owns only the JSON components (and the
//! thumbnail when held); it never creates or deletes `media/` or
//! `chat-sessions/`, which the media and agent layers manage out-of-band.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use opentake_domain::{MediaManifest, Timeline};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::compatibility;
use crate::error::{ProjectError, Result};
use crate::gen_log::{GenerationLog, GenerationLogEntry};
use crate::layout;
use crate::ProjectRoot;

/// Persisted schema details this build cannot safely write back.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectCompatibility {
    blockers: Vec<String>,
}

impl ProjectCompatibility {
    /// Whether saving would discard data this build does not understand.
    pub fn is_read_only(&self) -> bool {
        !self.blockers.is_empty()
    }

    /// Sorted, file-qualified reasons the project is compatibility read-only.
    pub fn blockers(&self) -> &[String] {
        &self.blockers
    }

    fn extend(&mut self, blockers: impl IntoIterator<Item = String>) {
        self.blockers.extend(blockers);
        self.blockers.sort();
        self.blockers.dedup();
    }

    /// Refuse a write that would discard unknown persisted data.
    pub fn ensure_writable(&self) -> Result<()> {
        if self.is_read_only() {
            return Err(ProjectError::CompatibilityReadOnly {
                blockers: self.blockers.clone(),
            });
        }
        Ok(())
    }
}

/// An opened `.opentake` project: the bundle path plus its decoded components.
///
/// Media files referenced by `manifest` live under the bundle's `media/`
/// directory (`.project` sources) or at absolute paths (`.external`); they are
/// not loaded into this struct. Chat sessions and the thumbnail are likewise
/// left on disk, except for an optional in-memory `thumbnail` that `save` will
/// persist when set.
#[derive(Clone, Debug)]
pub struct Project {
    /// Absolute path to the bundle directory (`…/Name.opentake`).
    pub bundle_path: PathBuf,
    /// The timeline (`project.json`).
    pub timeline: Timeline,
    /// The media manifest (`media.json`). Defaults to empty when the file was
    /// absent.
    pub manifest: MediaManifest,
    /// The generation log (`generation-log.json`). `None` when the file was
    /// absent or failed to parse; the latter also makes compatibility read-only.
    pub generation_log: Option<GenerationLog>,
    /// JPEG thumbnail bytes to write on the next `save`. `None` leaves any
    /// existing `thumbnail.jpg` on disk untouched.
    pub thumbnail: Option<Vec<u8>>,
    compatibility: ProjectCompatibility,
}

impl Project {
    /// Create a fresh, empty project rooted at `bundle_path` (not yet written).
    pub fn new(bundle_path: impl Into<PathBuf>) -> Self {
        Self::new_with_compatibility(bundle_path, ProjectCompatibility::default())
    }

    /// Create a project while preserving compatibility state from an opened bundle.
    pub fn new_with_compatibility(
        bundle_path: impl Into<PathBuf>,
        compatibility: ProjectCompatibility,
    ) -> Self {
        Project {
            bundle_path: bundle_path.into(),
            timeline: Timeline::new(),
            manifest: MediaManifest::new(),
            generation_log: None,
            thumbnail: None,
            compatibility,
        }
    }

    /// Compatibility state detected while opening the persisted components.
    pub fn compatibility(&self) -> &ProjectCompatibility {
        &self.compatibility
    }

    /// Reconstruct the legacy generation audit rows carried only by manifest
    /// entries saved before `generation-log.json` existed.
    ///
    /// Canonically identical [`opentake_domain::GenerationInput`] snapshots
    /// represent one generation even when it produced multiple assets. The full
    /// SHA-256 provenance digest supplies a fixed-size, deterministic synthetic
    /// row id. Canonical keys also impose a total row order, so manifest ordering
    /// cannot perturb saved bytes.
    /// Legacy manifests contain no trustworthy billed-cost field, so seeded rows
    /// keep `cost_credits = None` instead of applying a mutable pricing catalog
    /// retroactively.
    pub fn seed_generation_log_from_assets(&self) -> Result<GenerationLog> {
        let mut seeds = BTreeMap::<Vec<u8>, (String, Option<f64>)>::new();

        for entry in &self.manifest.entries {
            let Some(provenance) = &entry.generation_input else {
                continue;
            };
            let canonical_key = serde_json::to_vec(provenance)
                .map_err(|error| ProjectError::json(layout::MANIFEST_FILE, error))?;
            seeds
                .entry(canonical_key)
                .or_insert_with(|| (provenance.model.clone(), provenance.created_at));
        }

        Ok(GenerationLog {
            version: 1,
            entries: seeds
                .into_iter()
                .map(|(canonical_key, (model, created_at))| {
                    GenerationLogEntry::new(
                        format!("legacy-generation:{}", sha256_hex(&canonical_key)),
                        model,
                        None,
                        created_at,
                    )
                })
                .collect(),
        })
    }

    /// Open the `.opentake` bundle at `path`.
    ///
    /// Returns [`ProjectError::NotABundle`] if `path` is not a directory,
    /// [`ProjectError::MissingTimeline`] if `project.json` is absent, and
    /// [`ProjectError::Json`] if `project.json` or `media.json` fails to parse.
    /// A malformed `generation-log.json` opens as a compatibility read-only
    /// recovery with no decoded log.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let root = ProjectRoot::open(path)?;
        Self::open_from_root(&root)
    }

    /// Decode every project component from one retained root capability.
    pub fn open_from_root(root: &ProjectRoot) -> Result<Self> {
        Self::open_from_root_with_hook(root, |_| {})
    }

    fn open_from_root_with_hook(
        root: &ProjectRoot,
        mut after_component: impl FnMut(&str),
    ) -> Result<Self> {
        let bundle = root.path();
        let timeline_bytes = root.read_optional(layout::TIMELINE_FILE)?.ok_or_else(|| {
            ProjectError::MissingTimeline {
                file: layout::TIMELINE_FILE,
                bundle: bundle.to_path_buf(),
            }
        })?;
        let (mut timeline, timeline_blockers, timeline_document) =
            decode_component::<Timeline>(&timeline_bytes, layout::TIMELINE_FILE)?;
        compatibility::repair_timeline_ids(&mut timeline, &timeline_document);
        after_component(layout::TIMELINE_FILE);
        let mut compatibility = ProjectCompatibility::default();
        compatibility.extend(timeline_blockers);

        // media.json: strict when present, empty default when absent.
        let manifest = if let Some(bytes) = root.read_optional(layout::MANIFEST_FILE)? {
            let (manifest, blockers, _) =
                decode_component::<MediaManifest>(&bytes, layout::MANIFEST_FILE)?;
            compatibility.extend(blockers);
            manifest
        } else {
            MediaManifest::new()
        };
        after_component(layout::MANIFEST_FILE);

        // generation-log.json: lenient read recovery — a parse error degrades
        // to None but records a blocker so no save can overwrite the bytes.
        let generation_log = match root.read_optional(layout::GENERATION_LOG_FILE) {
            Ok(Some(bytes)) => {
                match decode_component::<GenerationLog>(&bytes, layout::GENERATION_LOG_FILE) {
                    Ok((log, blockers, _)) => {
                        compatibility.extend(blockers);
                        Some(log)
                    }
                    Err(_) => {
                        compatibility.extend([format!(
                            "{}:invalid-or-unreadable",
                            layout::GENERATION_LOG_FILE
                        )]);
                        None
                    }
                }
            }
            Ok(None) => None,
            Err(_) => {
                compatibility.extend([format!(
                    "{}:invalid-or-unreadable",
                    layout::GENERATION_LOG_FILE
                )]);
                None
            }
        };
        after_component(layout::GENERATION_LOG_FILE);

        Ok(Project {
            bundle_path: bundle.to_path_buf(),
            timeline,
            manifest,
            generation_log,
            thumbnail: None,
            compatibility,
        })
    }

    /// Write this project's JSON components into [`Self::bundle_path`].
    ///
    /// Creates the bundle directory if needed. Always (re)writes `project.json`
    /// and `media.json`; writes `generation-log.json` when a log is held and
    /// `thumbnail.jpg` when [`Self::thumbnail`] is set. Each file is written
    /// atomically (temp file + rename). Existing `media/` and `chat-sessions/`
    /// directories are left untouched.
    pub fn save(&self) -> Result<()> {
        let encoded = EncodedProject::prepare(self)?;
        if let Some(root) = ProjectRoot::open_optional(&self.bundle_path)? {
            encoded.write_to(&root)
        } else {
            let publisher = ProjectRoot::begin_replace(&self.bundle_path)?;
            encoded.write_to(publisher.stage())?;
            publisher.publish().map(|_| ())
        }
    }

    /// Persist only `media.json` through one atomic replacement.
    ///
    /// Media-library workflows mutate no timeline, generation log, thumbnail,
    /// or bundled media bytes. Restricting their durable commit to this one
    /// component prevents a later unrelated component failure from turning an
    /// error result into a partially saved manifest.
    pub fn save_manifest(&self) -> Result<()> {
        self.compatibility.ensure_writable()?;
        let manifest = encode_component(layout::MANIFEST_FILE, &self.manifest)?;
        let root = ProjectRoot::create(&self.bundle_path)?;
        root.write_atomic(layout::MANIFEST_FILE, &manifest)
    }

    /// Persist only `media.json` through a retained bundle root.
    pub fn save_manifest_to_root(&self, root: &ProjectRoot) -> Result<()> {
        self.compatibility.ensure_writable()?;
        let manifest = encode_component(layout::MANIFEST_FILE, &self.manifest)?;
        root.write_atomic(layout::MANIFEST_FILE, &manifest)
    }

    /// Like [`Self::save`] but targets an explicit `bundle` directory (used by
    /// the archiver to stage a self-contained copy). Does not mutate `self`.
    pub fn save_to(&self, bundle: impl AsRef<Path>) -> Result<()> {
        let encoded = EncodedProject::prepare(self)?;
        let publisher = ProjectRoot::begin_replace(bundle.as_ref())?;
        encoded.write_to(publisher.stage())?;
        publisher.publish().map(|_| ())
    }

    /// Persist this snapshot exclusively through `root` authority.
    pub fn save_to_root(&self, root: &ProjectRoot) -> Result<()> {
        EncodedProject::prepare(self)?.write_to(root)
    }

    /// Publish a complete fresh sibling bundle and return the exact root that
    /// became visible. Sessions adopt this retained authority only after the
    /// directory publication commit succeeds.
    pub fn publish_complete_to(
        &self,
        bundle: impl AsRef<Path>,
        media_source: Option<&ProjectRoot>,
    ) -> Result<ProjectRoot> {
        let encoded = EncodedProject::prepare(self)?;
        let publisher = ProjectRoot::begin_replace(bundle.as_ref())?;
        encoded.write_to(publisher.stage())?;
        if let Some(source) = media_source {
            source.copy_media_to(publisher.stage())?;
            source.copy_chat_sessions_to(publisher.stage())?;
            if self.thumbnail.is_none() {
                source.copy_thumbnail_to(publisher.stage())?;
            }
        }
        publisher.publish()
    }

    /// Replace the same bundle represented by an owned retained root.
    ///
    /// Windows refuses to rename a directory while a process still owns an
    /// open directory handle to it. Complete same-target transactions therefore
    /// stage and copy through the retained authority first, explicitly close
    /// that authority, and only then enter the existing journaled publication
    /// commit. Save-As keeps using [`Self::publish_complete_to`] because its
    /// source and destination are distinct.
    pub fn publish_complete_replacing_root(
        &self,
        bundle: impl AsRef<Path>,
        media_source: ProjectRoot,
    ) -> Result<ProjectRoot> {
        let encoded = EncodedProject::prepare(self)?;
        let publisher = ProjectRoot::begin_replace(bundle.as_ref())?;
        encoded.write_to(publisher.stage())?;
        media_source.copy_media_to(publisher.stage())?;
        media_source.copy_chat_sessions_to(publisher.stage())?;
        if self.thumbnail.is_none() {
            media_source.copy_thumbnail_to(publisher.stage())?;
        }
        drop(media_source);
        publisher.publish()
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

struct EncodedProject {
    timeline: Vec<u8>,
    manifest: Vec<u8>,
    generation_log: Option<Vec<u8>>,
    thumbnail: Option<Vec<u8>>,
}

impl EncodedProject {
    /// Produce the exact byte snapshot before any destination path is created.
    fn prepare(project: &Project) -> Result<Self> {
        project.compatibility.ensure_writable()?;
        Ok(Self {
            timeline: encode_component(layout::TIMELINE_FILE, &project.timeline)?,
            manifest: encode_component(layout::MANIFEST_FILE, &project.manifest)?,
            generation_log: project
                .generation_log
                .as_ref()
                .map(|log| encode_component(layout::GENERATION_LOG_FILE, log))
                .transpose()?,
            thumbnail: project.thumbnail.clone(),
        })
    }

    fn write_to(&self, root: &ProjectRoot) -> Result<()> {
        root.write_atomic(layout::TIMELINE_FILE, &self.timeline)?;
        root.write_atomic(layout::MANIFEST_FILE, &self.manifest)?;
        if let Some(log) = &self.generation_log {
            root.write_atomic(layout::GENERATION_LOG_FILE, log)?;
        }
        if let Some(thumbnail) = &self.thumbnail {
            root.write_atomic(layout::THUMBNAIL_FILE, thumbnail)?;
        }
        Ok(())
    }
}

fn encode_component<T: Serialize>(file_name: &str, value: &T) -> Result<Vec<u8>> {
    serde_json::to_vec_pretty(value).map_err(|error| ProjectError::json(file_name, error))
}

fn decode_component<T: DeserializeOwned>(
    bytes: &[u8],
    file: &str,
) -> Result<(T, Vec<String>, Value)> {
    let document: Value =
        serde_json::from_slice(bytes).map_err(|error| ProjectError::json(file, error))?;

    // The normal path performs one formal decode and no Track.clips probes or
    // document clone. Only a failed timeline decode enters the narrow upstream
    // Track.clips fallback.
    let initial = deserialize_with_ignored(bytes, file, &document);
    let (value, mut ignored, failed_tracks) = match initial {
        Ok((value, ignored)) => (value, ignored, Vec::new()),
        Err(initial_error) if file == layout::TIMELINE_FILE => {
            let Some(fallback) = compatibility::prepare_timeline_fallback(&document) else {
                return Err(ProjectError::json(file, initial_error));
            };
            let normalized = serde_json::to_vec(&fallback.normalized)
                .map_err(|error| ProjectError::json(file, error))?;
            let (value, ignored) = deserialize_with_ignored(&normalized, file, &document)
                .map_err(|error| ProjectError::json(file, error))?;
            (value, ignored, fallback.failed_tracks)
        }
        Err(error) => return Err(ProjectError::json(file, error)),
    };

    if file == layout::TIMELINE_FILE {
        compatibility::scan_timeline(&document, file, &failed_tracks, &mut ignored);
    }
    ignored.sort();
    ignored.dedup();
    Ok((value, ignored, document))
}

fn deserialize_with_ignored<T: DeserializeOwned>(
    bytes: &[u8],
    file: &str,
    document: &Value,
) -> std::result::Result<(T, Vec<String>), serde_json::Error> {
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    let mut ignored = Vec::new();
    let value = serde_ignored::deserialize(&mut decoder, |path| {
        ignored.push(format!(
            "{file}:{}",
            compatibility::canonical_ignored_path(&path, document)
        ));
    })?;
    decoder.end()?;
    Ok((value, ignored))
}

/// Copy a source bundle's `media/` directory into `dest_bundle`, recursively,
/// preserving the relative layout — the port of upstream `mediaDirWrapper`
/// (`Project/VideoProject.swift:112-117`), which folds the whole `media/`
/// directory into the saved package on every save/save-as. Save-as builds the
/// new bundle at a fresh path; without this, project-internal media
/// ([`MediaSource::Project`](opentake_domain::MediaSource) relative paths — AI
/// output, pasted, captured stills) is left behind and every reference silently
/// dangles.
///
/// Contract:
/// - **Missing source `media/`** → no-op `Ok(())` (upstream returns `nil` from
///   `mediaDirWrapper` when the dir doesn't exist; nothing to carry).
/// - **Same-path save** (source and dest bundle are the same directory) → no-op,
///   so autosave never copies `media/` onto itself.
/// - **Existing destination `media/`** → fail without modifying it. Complete
///   Save As replacement is owned by [`Project::publish_complete_to`], which
///   publishes a fresh whole-bundle sibling through the backup/recovery state
///   machine rather than deleting a live media tree.
pub fn copy_media_dir(source_bundle: &Path, dest_bundle: &Path) -> Result<()> {
    if source_bundle == dest_bundle {
        return Ok(());
    }
    let source = ProjectRoot::open(source_bundle)?;
    if !source.has_media_tree()? {
        return Ok(());
    }
    let destination = ProjectRoot::create(dest_bundle)?;
    source.copy_media_to(&destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// A per-call-unique scratch dir under the system temp dir, removed on drop.
    struct TmpDir(PathBuf);
    impl TmpDir {
        fn new(tag: &str) -> Self {
            use std::sync::atomic::{AtomicU64, Ordering};
            static N: AtomicU64 = AtomicU64::new(0);
            let n = N.fetch_add(1, Ordering::Relaxed);
            let p = std::env::temp_dir()
                .join(format!("opentake-bundle-{tag}-{}-{n}", std::process::id()));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            TmpDir(p)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TmpDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn tree_receipt(root: &Path) -> Vec<(PathBuf, Option<Vec<u8>>)> {
        fn visit(base: &Path, path: &Path, receipt: &mut Vec<(PathBuf, Option<Vec<u8>>)>) {
            let mut entries = fs::read_dir(path)
                .unwrap()
                .map(std::result::Result::unwrap)
                .collect::<Vec<_>>();
            entries.sort_by_key(std::fs::DirEntry::file_name);
            for entry in entries {
                let path = entry.path();
                let relative = path.strip_prefix(base).unwrap().to_path_buf();
                if entry.file_type().unwrap().is_dir() {
                    receipt.push((relative, None));
                    visit(base, &path, receipt);
                } else {
                    receipt.push((relative, Some(fs::read(path).unwrap())));
                }
            }
        }

        let mut receipt = Vec::new();
        visit(root, root, &mut receipt);
        receipt
    }

    #[cfg(unix)]
    #[test]
    fn retained_root_prevents_component_mixing_during_ambient_aba() {
        let tmp = TmpDir::new("open-aba");
        let projects = tmp.path().join("projects");
        let retained = tmp.path().join("projects-retained");
        let replacement = tmp.path().join("projects-replacement");
        let bundle = projects.join("A.opentake");
        let replacement_bundle = replacement.join("A.opentake");
        let mut original = Project::new(&bundle);
        original.timeline.fps = 24;
        original.manifest.favorites.push("from-original".into());
        original.save().unwrap();
        let mut other = Project::new(&replacement_bundle);
        other.timeline.fps = 60;
        other.manifest.favorites.push("from-replacement".into());
        other.save().unwrap();
        let root = ProjectRoot::open(&bundle).unwrap();

        let opened = Project::open_from_root_with_hook(&root, |component| {
            if component == layout::TIMELINE_FILE {
                fs::rename(&projects, &retained).unwrap();
                fs::rename(&replacement, &projects).unwrap();
            } else if component == layout::MANIFEST_FILE {
                fs::rename(&projects, &replacement).unwrap();
                fs::rename(&retained, &projects).unwrap();
            }
        })
        .unwrap();

        assert_eq!(opened.timeline.fps, 24);
        assert_eq!(opened.manifest.favorites, ["from-original"]);
    }

    #[cfg(unix)]
    #[test]
    fn final_bundle_symlink_is_rejected() {
        use std::os::unix::fs::symlink;

        let tmp = TmpDir::new("root-symlink");
        let real = tmp.path().join("Real.opentake");
        Project::new(&real).save().unwrap();
        let link = tmp.path().join("Link.opentake");
        symlink(&real, &link).unwrap();

        assert!(matches!(
            Project::open(link),
            Err(ProjectError::NotABundle(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn retained_root_save_never_writes_an_ambient_replacement() {
        let tmp = TmpDir::new("save-aba");
        let projects = tmp.path().join("projects");
        let retained = tmp.path().join("projects-retained");
        let bundle = projects.join("A.opentake");
        let mut original = Project::new(&bundle);
        original.timeline.fps = 24;
        original.save().unwrap();
        let root = ProjectRoot::open(&bundle).unwrap();
        fs::rename(&projects, &retained).unwrap();
        let mut replacement = Project::new(&bundle);
        replacement.timeline.fps = 60;
        replacement.save().unwrap();

        original.timeline.fps = 48;
        original.save_to_root(&root).unwrap();

        assert_eq!(Project::open(&bundle).unwrap().timeline.fps, 60);
        assert_eq!(
            Project::open(retained.join("A.opentake"))
                .unwrap()
                .timeline
                .fps,
            48
        );
    }

    #[test]
    fn complete_publish_replaces_an_existing_bundle_with_fresh_media() {
        let tmp = TmpDir::new("complete-replace");
        let source = tmp.path().join("Source.opentake");
        let target = tmp.path().join("Target.opentake");
        let mut project = Project::new(&source);
        project.timeline.fps = 48;
        project.save().unwrap();
        fs::create_dir_all(source.join("media/nested")).unwrap();
        fs::write(source.join("media/nested/clip.bin"), b"fresh media").unwrap();
        let source_root = ProjectRoot::open(&source).unwrap();
        fs::create_dir_all(target.join("media")).unwrap();
        fs::write(target.join("project.json"), b"old timeline").unwrap();
        fs::write(target.join("media/stale.bin"), b"stale media").unwrap();
        fs::write(target.join("stale.txt"), b"stale component").unwrap();

        let published = project
            .publish_complete_to(&target, Some(&source_root))
            .expect("complete Save As publication");

        assert_eq!(
            Project::open_from_root(&published).unwrap().timeline.fps,
            48
        );
        assert_eq!(
            fs::read(target.join("media/nested/clip.bin")).unwrap(),
            b"fresh media"
        );
        assert!(!target.join("media/stale.bin").exists());
        assert!(!target.join("stale.txt").exists());
    }

    #[test]
    fn complete_publish_carries_project_chat_sessions_across_save_as() {
        let tmp = TmpDir::new("complete-chat-sessions");
        let source = tmp.path().join("Source.opentake");
        let target = tmp.path().join("Target.opentake");
        let project = Project::new(&source);
        project.save().unwrap();
        fs::create_dir_all(source.join("chat-sessions")).unwrap();
        fs::write(
            source.join("chat-sessions/chat-1.json"),
            br#"{"id":"chat-1","messages":[]}"#,
        )
        .unwrap();
        let source_root = ProjectRoot::open(&source).unwrap();

        project
            .publish_complete_to(&target, Some(&source_root))
            .expect("Save As must carry project-local conversations");

        assert_eq!(
            fs::read(target.join("chat-sessions/chat-1.json")).unwrap(),
            br#"{"id":"chat-1","messages":[]}"#
        );
    }

    #[test]
    fn complete_publish_replaces_the_owned_source_root() {
        let tmp = TmpDir::new("complete-same-target");
        let target = tmp.path().join("Project.opentake");
        let mut project = Project::new(&target);
        project.timeline.fps = 24;
        project.save().unwrap();
        fs::create_dir_all(target.join("media")).unwrap();
        fs::write(target.join("media/clip.bin"), b"media").unwrap();
        fs::write(target.join("thumbnail.jpg"), b"cover").unwrap();
        let source_root = ProjectRoot::open(&target).unwrap();

        project.timeline.fps = 48;
        let published = project
            .publish_complete_replacing_root(&target, source_root)
            .expect("same-target publication must release the old root before rename");

        assert_eq!(
            Project::open_from_root(&published).unwrap().timeline.fps,
            48
        );
        assert_eq!(fs::read(target.join("media/clip.bin")).unwrap(), b"media");
        assert_eq!(fs::read(target.join("thumbnail.jpg")).unwrap(), b"cover");
    }

    #[cfg(unix)]
    #[test]
    fn media_copy_failure_leaves_an_existing_target_tree_byte_exact() {
        use std::os::unix::fs::symlink;

        let tmp = TmpDir::new("complete-copy-failure");
        let source = tmp.path().join("Source.opentake");
        let target = tmp.path().join("Target.opentake");
        let project = Project::new(&source);
        project.save().unwrap();
        fs::create_dir_all(source.join("media")).unwrap();
        symlink(
            tmp.path().join("outside"),
            source.join("media/refused-link"),
        )
        .unwrap();
        let source_root = ProjectRoot::open(&source).unwrap();
        fs::create_dir_all(target.join("media/nested")).unwrap();
        fs::write(target.join("project.json"), b"old timeline").unwrap();
        fs::write(target.join("media/nested/clip.bin"), b"old media").unwrap();
        let before = tree_receipt(&target);

        project
            .publish_complete_to(&target, Some(&source_root))
            .expect_err("a symlink in retained project media must fail closed");

        assert_eq!(tree_receipt(&target), before);
        assert!(!fs::read_dir(tmp.path()).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".Target.opentake.opentake-stage")
        }));
        assert!(!tmp
            .path()
            .join(".Target.opentake.opentake-journal")
            .exists());
        assert!(!tmp.path().join(".Target.opentake.opentake-backup").exists());
    }

    #[test]
    fn copy_media_dir_mirrors_nested_layout() {
        let tmp = TmpDir::new("nested");
        let src = tmp.path().join("Src.opentake");
        let dst = tmp.path().join("Dst.opentake");
        let src_media = layout::media_dir(&src);
        fs::create_dir_all(src_media.join("sub")).unwrap();
        fs::write(src_media.join("a.png"), b"AAA").unwrap();
        fs::write(src_media.join("sub").join("b.mov"), b"BBBB").unwrap();

        copy_media_dir(&src, &dst).unwrap();

        assert_eq!(fs::read(dst.join("media").join("a.png")).unwrap(), b"AAA");
        assert_eq!(
            fs::read(dst.join("media").join("sub").join("b.mov")).unwrap(),
            b"BBBB"
        );
    }

    #[test]
    fn copy_media_dir_missing_source_is_noop() {
        let tmp = TmpDir::new("missing");
        let src = tmp.path().join("Src.opentake"); // no media/ under it
        let dst = tmp.path().join("Dst.opentake");
        fs::create_dir_all(&src).unwrap();

        copy_media_dir(&src, &dst).unwrap();
        assert!(!dst.join("media").exists());
    }

    #[test]
    fn copy_media_dir_same_path_is_noop() {
        let tmp = TmpDir::new("same");
        let bundle = tmp.path().join("Same.opentake");
        let media = layout::media_dir(&bundle);
        fs::create_dir_all(&media).unwrap();
        fs::write(media.join("keep.png"), b"KEEP").unwrap();

        // Source == dest: must not touch (delete/replace) the existing media/.
        copy_media_dir(&bundle, &bundle).unwrap();
        assert_eq!(fs::read(media.join("keep.png")).unwrap(), b"KEEP");
    }

    #[test]
    fn copy_media_dir_refuses_existing_dest_media_without_changes() {
        let tmp = TmpDir::new("replace");
        let src = tmp.path().join("Src.opentake");
        let dst = tmp.path().join("Dst.opentake");
        fs::create_dir_all(layout::media_dir(&src)).unwrap();
        fs::write(layout::media_dir(&src).join("new.png"), b"NEW").unwrap();
        // Pre-existing stale file in the destination media/ that is NOT in the
        // source; a full swap must not leave it behind.
        fs::create_dir_all(layout::media_dir(&dst)).unwrap();
        fs::write(layout::media_dir(&dst).join("stale.png"), b"OLD").unwrap();

        let before = tree_receipt(&dst);
        copy_media_dir(&src, &dst).expect_err("legacy media-only copy must not delete live media");

        assert_eq!(tree_receipt(&dst), before);
        assert!(!dst.join("media").join("new.png").exists());
    }
}
