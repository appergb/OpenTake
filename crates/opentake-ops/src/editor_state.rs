//! `EditorState` — the mutable document edited through [`crate::command`].
//!
//! Holds the `Timeline` plus the `MediaManifest` (folder commands mutate the
//! manifest, not the timeline), the undo/redo stacks, and a monotonic version
//! counter. The undo model is upstream `withTimelineSwap` generalized: a command
//! snapshots the whole document, mutates it, and only commits (pushes the
//! snapshot onto the undo stack + bumps the version) when the document actually
//! changed (`PartialEq` short-circuit).
//!
//! Snapshots are whole-tree clones (`Timeline` + `MediaManifest` both derive
//! `Clone`/`PartialEq`), matching the "undo stack in Rust, integral-tree
//! snapshot" decision from `ARCHITECTURE.md §5`.

use opentake_domain::{ClipLocation, MediaManifest, Timeline};

/// Immutable snapshot of everything an [`crate::command::EditCommand`] can touch.
#[derive(Clone, PartialEq, Debug)]
pub struct DocSnapshot {
    pub timeline: Timeline,
    pub manifest: MediaManifest,
}

/// The editable document + undo/redo history + version.
#[derive(Clone, Debug)]
pub struct EditorState {
    pub timeline: Timeline,
    pub manifest: MediaManifest,
    undo_stack: Vec<DocSnapshot>,
    redo_stack: Vec<DocSnapshot>,
    version: u64,
}

impl Default for EditorState {
    fn default() -> Self {
        EditorState::new(Timeline::new(), MediaManifest::new())
    }
}

impl EditorState {
    /// New state wrapping `timeline` + `manifest` with empty history at version 0.
    pub fn new(timeline: Timeline, manifest: MediaManifest) -> Self {
        EditorState {
            timeline,
            manifest,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            version: 0,
        }
    }

    /// New state from a timeline only (empty manifest).
    pub fn from_timeline(timeline: Timeline) -> Self {
        EditorState::new(timeline, MediaManifest::new())
    }

    /// The current version. Bumps by 1 on every committed (changing) command and
    /// on every undo/redo.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Whether an undo is available.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Whether a redo is available.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Depth of the undo stack (test/inspection helper).
    pub fn undo_depth(&self) -> usize {
        self.undo_stack.len()
    }

    /// A snapshot of the current document.
    pub(crate) fn snapshot(&self) -> DocSnapshot {
        DocSnapshot {
            timeline: self.timeline.clone(),
            manifest: self.manifest.clone(),
        }
    }

    /// Restore a snapshot into the live document (does not touch history).
    pub(crate) fn restore(&mut self, snap: DocSnapshot) {
        self.timeline = snap.timeline;
        self.manifest = snap.manifest;
    }

    /// Commit a structural change: push `before` onto the undo stack, clear the
    /// redo stack (a new edit invalidates redo), bump the version. Called only
    /// when `before != after`.
    pub(crate) fn commit(&mut self, before: DocSnapshot) {
        self.undo_stack.push(before);
        self.redo_stack.clear();
        self.version += 1;
    }

    /// Undo the most recent committed change. Returns `true` if anything was
    /// undone. Pushes the pre-undo document onto the redo stack and bumps the
    /// version.
    pub(crate) fn undo(&mut self) -> bool {
        let Some(prev) = self.undo_stack.pop() else {
            return false;
        };
        let current = self.snapshot();
        self.restore(prev);
        self.redo_stack.push(current);
        self.version += 1;
        true
    }

    /// Redo the most recently undone change. Returns `true` if anything was
    /// redone. Pushes the pre-redo document onto the undo stack and bumps the
    /// version.
    pub(crate) fn redo(&mut self) -> bool {
        let Some(next) = self.redo_stack.pop() else {
            return false;
        };
        let current = self.snapshot();
        self.restore(next);
        self.undo_stack.push(current);
        self.version += 1;
        true
    }

    // MARK: - Lookups (1:1 port of EditorViewModel.findClip)

    /// Locate a clip by id. 1:1 port of `findClip`.
    pub fn find_clip(&self, id: &str) -> Option<ClipLocation> {
        for (ti, track) in self.timeline.tracks.iter().enumerate() {
            if let Some(ci) = track.clips.iter().position(|c| c.id == id) {
                return Some(ClipLocation::new(ti, ci));
            }
        }
        None
    }

    /// Index of the track holding `track_id`.
    pub fn track_index(&self, track_id: &str) -> Option<usize> {
        self.timeline.tracks.iter().position(|t| t.id == track_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_domain::{Clip, ClipType, Track};

    fn state_with_clip() -> EditorState {
        let mut tl = Timeline::new();
        let mut t = Track::new("t1", ClipType::Video);
        t.clips.push(Clip::new("c1", "asset", 0, 30));
        tl.tracks.push(t);
        EditorState::from_timeline(tl)
    }

    #[test]
    fn new_state_has_zero_version_and_empty_history() {
        let s = EditorState::default();
        assert_eq!(s.version(), 0);
        assert!(!s.can_undo());
        assert!(!s.can_redo());
    }

    #[test]
    fn find_clip_locates_by_id() {
        let s = state_with_clip();
        assert_eq!(s.find_clip("c1"), Some(ClipLocation::new(0, 0)));
        assert_eq!(s.find_clip("nope"), None);
    }

    #[test]
    fn commit_undo_redo_cycle_restores_and_versions() {
        let mut s = state_with_clip();
        let before = s.snapshot();
        // mutate then commit
        s.timeline.tracks[0].clips[0].start_frame = 99;
        s.commit(before);
        assert_eq!(s.version(), 1);
        assert!(s.can_undo());
        assert!(!s.can_redo());

        // undo restores
        assert!(s.undo());
        assert_eq!(s.timeline.tracks[0].clips[0].start_frame, 0);
        assert_eq!(s.version(), 2);
        assert!(s.can_redo());

        // redo reapplies
        assert!(s.redo());
        assert_eq!(s.timeline.tracks[0].clips[0].start_frame, 99);
        assert_eq!(s.version(), 3);
    }

    #[test]
    fn new_edit_clears_redo_stack() {
        let mut s = state_with_clip();
        let b1 = s.snapshot();
        s.timeline.tracks[0].clips[0].start_frame = 10;
        s.commit(b1);
        assert!(s.undo());
        assert!(s.can_redo());
        // a fresh commit invalidates redo
        let b2 = s.snapshot();
        s.timeline.tracks[0].clips[0].start_frame = 20;
        s.commit(b2);
        assert!(!s.can_redo());
    }

    #[test]
    fn undo_on_empty_history_is_noop() {
        let mut s = state_with_clip();
        assert!(!s.undo());
        assert_eq!(s.version(), 0);
    }
}
