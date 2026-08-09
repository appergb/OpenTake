//! The Tauri command surface — defined here as **plain Rust DTOs + handler
//! functions**, with no `tauri` dependency (`core-SPEC.md` §6).
//!
//! `src-tauri` will later wrap each `handle_*` function in a one-line
//! `#[tauri::command]` that takes `State<AppCore>`, calls the handler, and maps
//! [`CmdError`]. Keeping the request/response shapes and the
//! `AppCore`-to-response wiring here means the boundary is unit-testable without
//! pulling in the Tauri runtime, and the eventual `#[tauri::command]` shims carry
//! zero logic.
//!
//! All DTOs serialize with `camelCase` fields to match the front-end naming
//! convention (`core-SPEC.md` §6). `Timeline` itself serializes with its own
//! domain schema (= `project.json`), so the read-only mirror and the persisted
//! file share one shape (`core-SPEC.md` §4.4).

use serde::{Deserialize, Serialize};

use opentake_domain::Timeline;
use opentake_ops::command::{EditCommand, EditResult};

use crate::core::{AppCore, ProjectRevision, TimelineSnapshot};
use crate::error::{CoreError, Result};

/// Machine + human readable error for the Tauri boundary (`core-SPEC.md` §6.3).
/// `code` is `"validation"` for rejected input or `"internal"` otherwise;
/// `message` is the human-readable detail.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CmdError {
    /// Machine-readable class: `"validation"` | `"internal"`.
    pub code: String,
    /// Human-readable message (carries precise validation paths when available).
    pub message: String,
}

impl From<CoreError> for CmdError {
    fn from(err: CoreError) -> Self {
        let code = err.code();
        let message = match &err {
            CoreError::Edit(_)
            | CoreError::Media(_)
            | CoreError::StaleProject
            | CoreError::Project(opentake_project::ProjectError::CompatibilityReadOnly {
                ..
            })
            | CoreError::NoProjectOpen
            | CoreError::Unsupported(_) => err.to_string(),
            CoreError::Project(_) => {
                eprintln!("project command failed: {err}");
                "Project operation failed".to_string()
            }
        };
        CmdError {
            code: code.to_string(),
            message,
        }
    }
}

/// `get_timeline` response: the read-only mirror plus its version
/// (`core-SPEC.md` §4.1 rule 1). The front end stores `{ mirror, mirrorVersion }`.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineSnapshotDto {
    /// The timeline at [`Self::version`] (serialized with the domain schema).
    pub timeline: Timeline,
    /// The project session this timeline belongs to.
    pub project_epoch: u64,
    /// The document version this snapshot was taken at.
    pub version: u64,
    /// Current project bundle path (`null` for a new unsaved project).
    pub project_path: Option<std::path::PathBuf>,
    /// Whether project mutations are blocked to preserve unknown fields.
    #[serde(rename = "compatibilityReadOnly")]
    pub compatibility_read_only: bool,
    /// Sorted persisted-schema paths this build does not understand.
    #[serde(rename = "compatibilityBlockers")]
    pub compatibility_blockers: Vec<String>,
}

impl From<TimelineSnapshot> for TimelineSnapshotDto {
    fn from(s: TimelineSnapshot) -> Self {
        let compatibility_read_only = s.compatibility.is_read_only();
        let compatibility_blockers = s.compatibility.blockers().to_vec();
        TimelineSnapshotDto {
            timeline: s.timeline,
            project_epoch: s.project_epoch,
            version: s.version,
            project_path: s.project_path,
            compatibility_read_only,
            compatibility_blockers,
        }
    }
}

/// The outcome of an edit / undo / redo, shaped for the front end
/// (`core-SPEC.md` §2.2). A camelCase mirror of [`EditResult`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditResultDto {
    /// Whether the document actually changed.
    pub changed: bool,
    /// Undo label, e.g. `"Add Clips"`.
    pub action_name: String,
    /// Clip ids created or directly affected.
    pub affected_clip_ids: Vec<String>,
    /// Document version after the command (the prior version when unchanged).
    pub timeline_version: u64,
    /// Human-readable one-line summary.
    pub summary: String,
}

impl From<EditResult> for EditResultDto {
    fn from(r: EditResult) -> Self {
        EditResultDto {
            changed: r.changed,
            action_name: r.action_name,
            affected_clip_ids: r.affected_clip_ids,
            timeline_version: r.timeline_version,
            summary: r.summary,
        }
    }
}

// MARK: - Handlers (the body of each future `#[tauri::command]`)

/// `get_timeline`: current read-only snapshot + version. Infallible.
pub fn handle_get_timeline(core: &AppCore) -> TimelineSnapshotDto {
    core.get_timeline().into()
}

/// `edit_apply`: the single editing entry point. `command` is constructed by the
/// front end (UI gestures) and routed straight to [`AppCore::apply`].
pub fn handle_edit_apply(
    core: &AppCore,
    command: EditCommand,
) -> std::result::Result<EditResultDto, CmdError> {
    map(core.apply(command).map(EditResultDto::from))
}

/// Revision- and path-bound editing entry point for untrusted IPC clients.
/// Unlike [`handle_edit_apply`], a delayed request is rejected after any
/// project switch, Save As, or intervening timeline edit.
pub fn handle_edit_apply_at_project_revision(
    core: &AppCore,
    expected: ProjectRevision,
    expected_project_path: Option<&std::path::Path>,
    command: EditCommand,
) -> std::result::Result<EditResultDto, CmdError> {
    map(core
        .apply_at_project_revision(expected, expected_project_path, command)
        .map(EditResultDto::from))
}

/// `undo`: global undo (Cmd+Z).
pub fn handle_undo(core: &AppCore) -> std::result::Result<EditResultDto, CmdError> {
    map(core.undo().map(EditResultDto::from))
}

/// `redo`: global redo.
pub fn handle_redo(core: &AppCore) -> std::result::Result<EditResultDto, CmdError> {
    map(core.redo().map(EditResultDto::from))
}

/// `project_open`: open a `.opentake` bundle, returning the first snapshot.
pub fn handle_project_open(
    core: &AppCore,
    path: String,
) -> std::result::Result<TimelineSnapshotDto, CmdError> {
    map(core.open_project(path).map(TimelineSnapshotDto::from))
}

/// `project_save`: save the open project. `path = None` saves back to the open
/// bundle; `Some` is a save-as. Returns the written bundle path.
pub fn handle_project_save(
    core: &AppCore,
    path: Option<String>,
) -> std::result::Result<String, CmdError> {
    let target = path.map(std::path::PathBuf::from);
    map(core
        .save_project(target)
        .map(|p| p.to_string_lossy().into_owned()))
}

/// `project_new`: replace the session with a fresh, unsaved project and return
/// its first snapshot. Infallible.
pub fn handle_project_new(core: &AppCore) -> TimelineSnapshotDto {
    core.new_project().into()
}

/// Adapt a [`Result`] into the boundary's `Result<_, CmdError>`.
fn map<T>(r: Result<T>) -> std::result::Result<T, CmdError> {
    r.map_err(CmdError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_domain::{ClipType, Timeline, Track};
    use opentake_ops::command::ClipEntry;

    /// Stand up a core whose session has one empty video track. Seeds it by
    /// saving a hand-built project to a per-call-unique temp bundle and opening
    /// it back, exercising the real `open_project` path. The unique dir name
    /// (atomic counter, not `line!()`) keeps parallel tests from racing on the
    /// same directory.
    fn core_with_track() -> AppCore {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "opentake-core-dto-{}-{}.opentake",
            std::process::id(),
            n
        ));
        let _ = std::fs::remove_dir_all(&dir);

        let mut project = opentake_project::Project::new(dir.clone());
        let mut tl = Timeline::new();
        tl.tracks.push(Track::new("t1", ClipType::Video));
        project.timeline = tl;
        project.save().unwrap();

        let core = AppCore::new();
        core.open_project(dir.clone()).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        core
    }

    fn add_one_clip() -> EditCommand {
        EditCommand::AddClips {
            entries: vec![ClipEntry {
                media_ref: "a".into(),
                media_type: ClipType::Video,
                source_clip_type: ClipType::Video,
                track_index: 0,
                start_frame: 0,
                duration_frames: 30,
                trim_start_frame: None,
                trim_end_frame: None,
                has_audio: false,
                add_linked_audio: false,
                transform: None,
            }],
        }
    }

    #[test]
    fn get_timeline_handler_returns_snapshot_dto() {
        let core = core_with_track();
        let dto = handle_get_timeline(&core);
        assert_eq!(dto.version, 0);
        assert_eq!(dto.project_epoch, 1);
        assert_eq!(dto.timeline.tracks.len(), 1);
        let json = serde_json::to_value(&dto).unwrap();
        assert_eq!(json["projectEpoch"], 1);
    }

    #[test]
    fn edit_apply_handler_happy_path() {
        let core = core_with_track();
        let dto = handle_edit_apply(&core, add_one_clip()).unwrap();
        assert!(dto.changed);
        assert_eq!(dto.timeline_version, 1);
        assert_eq!(dto.action_name, "Add Clip");
    }

    #[test]
    fn edit_apply_handler_maps_validation_error() {
        let core = core_with_track();
        let before = core.get_timeline();
        let events = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let observed = events.clone();
        core.subscribe(move |_| {
            observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });

        let err = handle_edit_apply(&core, EditCommand::AddClips { entries: vec![] }).unwrap_err();

        assert_eq!(err.code, "validation");
        assert!(!err.message.is_empty());
        let after = core.get_timeline();
        assert_eq!(
            after.timeline, before.timeline,
            "failed edit mutated timeline"
        );
        assert_eq!(after.version, before.version, "failed edit mutated version");
        assert_eq!(
            after.project_epoch, before.project_epoch,
            "failed edit mutated project identity"
        );
        assert_eq!(events.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn revision_bound_edit_reports_stale_project_without_mutation() {
        let core = core_with_track();
        let snapshot = core.get_timeline();
        core.apply(add_one_clip()).unwrap();
        let before = core.get_timeline();

        let error = handle_edit_apply_at_project_revision(
            &core,
            ProjectRevision {
                project_epoch: snapshot.project_epoch,
                version: snapshot.version,
            },
            snapshot.project_path.as_deref(),
            add_one_clip(),
        )
        .expect_err("a stale IPC mirror must be rejected");

        assert_eq!(error.code, "staleProject");
        assert_eq!(
            error.message,
            "project changed while preparing a deferred edit"
        );
        let after = core.get_timeline();
        assert_eq!(after.timeline, before.timeline);
        assert_eq!(after.version, before.version);
        assert_eq!(after.project_epoch, before.project_epoch);
    }

    #[test]
    fn internal_command_error_does_not_expose_project_paths() {
        let error = CmdError::from(CoreError::Project(
            opentake_project::ProjectError::MissingTimeline {
                file: "project.json",
                bundle: "/private/customer/secret.opentake".into(),
            },
        ));

        assert_eq!(error.code, "internal");
        assert_eq!(error.message, "Project operation failed");
        assert!(!error.message.contains("/private"));
    }

    #[test]
    fn undo_redo_handlers_roundtrip() {
        let core = core_with_track();
        handle_edit_apply(&core, add_one_clip()).unwrap();
        let undo = handle_undo(&core).unwrap();
        assert!(undo.changed);
        assert_eq!(undo.timeline_version, 2);
        let redo = handle_redo(&core).unwrap();
        assert!(redo.changed);
        assert_eq!(redo.timeline_version, 3);
    }

    #[test]
    fn project_save_with_no_path_maps_internal_error() {
        let core = AppCore::new(); // unsaved, no project dir
        let err = handle_project_save(&core, None).unwrap_err();
        assert_eq!(err.code, "internal");
    }

    #[test]
    fn project_new_handler_returns_first_snapshot() {
        let core = core_with_track();
        handle_edit_apply(&core, add_one_clip()).unwrap();

        let dto = handle_project_new(&core);

        assert_eq!(dto.version, 0);
        assert_eq!(dto.project_epoch, 2);
        assert!(dto.timeline.tracks.is_empty());
    }

    #[test]
    fn dtos_serialize_camel_case() {
        let dto = EditResultDto {
            changed: true,
            action_name: "Add Clip".into(),
            affected_clip_ids: vec!["c1".into()],
            timeline_version: 1,
            summary: "s".into(),
        };
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains("\"actionName\""));
        assert!(json.contains("\"affectedClipIds\""));
        assert!(json.contains("\"timelineVersion\""));
    }
}
