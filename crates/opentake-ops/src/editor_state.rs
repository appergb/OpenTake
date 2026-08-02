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

    /// Commit an irreversible audit mutation without adding an undo entry.
    /// Earlier undo snapshots are retained, but restore paths keep provider
    /// voice records outside ordinary document undo/redo.
    pub(crate) fn commit_irreversible(&mut self) {
        self.redo_stack.clear();
        self.version += 1;
    }

    /// Undo the most recent committed change. Returns `true` if anything was
    /// undone. Pushes the pre-undo document onto the redo stack and bumps the
    /// version.
    pub(crate) fn undo(&mut self) -> bool {
        let current = self.snapshot();
        while let Some(mut prev) = self.undo_stack.pop() {
            preserve_voice_models(&mut prev, &current);
            if prev == current {
                continue;
            }
            self.restore(prev);
            self.redo_stack.push(current);
            self.version += 1;
            return true;
        }
        false
    }

    /// Redo the most recently undone change. Returns `true` if anything was
    /// redone. Pushes the pre-redo document onto the undo stack and bumps the
    /// version.
    pub(crate) fn redo(&mut self) -> bool {
        let current = self.snapshot();
        while let Some(mut next) = self.redo_stack.pop() {
            preserve_voice_models(&mut next, &current);
            if next == current {
                continue;
            }
            self.restore(next);
            self.undo_stack.push(current);
            self.version += 1;
            return true;
        }
        false
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

fn preserve_voice_models(target: &mut DocSnapshot, current: &DocSnapshot) {
    target.timeline.voice_models = current.timeline.voice_models.clone();
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_domain::{Clip, ClipType, Track, VoiceModelRecord};

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
    fn permanent_voice_revocation_survives_all_undo_snapshots() {
        let mut state = EditorState::default();
        let before_enroll = state.snapshot();
        state.timeline.voice_models.push(VoiceModelRecord {
            id: "voice-1".into(),
            provider: "elevenlabs".into(),
            provider_voice_id: "provider-1".into(),
            model: "model".into(),
            consent_id: "consent-1".into(),
            source_audio_asset_id: "audio-1".into(),
            source_audio_sha256: "a".repeat(64),
            request_hash: "b".repeat(64),
            voice_name: "Narrator".into(),
            revoked: false,
        });
        state.commit(before_enroll);
        state.timeline.voice_models[0].revoked = true;
        state.commit_irreversible();

        let version = state.version();
        assert!(!state.undo());
        assert_eq!(state.timeline.voice_models.len(), 1);
        assert!(state.timeline.voice_models[0].revoked);
        assert!(!state.can_undo());
        assert!(!state.redo());
        assert_eq!(state.version(), version);
    }

    #[test]
    fn active_provider_voice_survives_undo_of_an_earlier_edit() {
        let mut state = state_with_clip();
        let before_edit = state.snapshot();
        state.timeline.tracks[0].clips[0].start_frame = 12;
        state.commit(before_edit);
        state.timeline.voice_models.push(VoiceModelRecord {
            id: "voice-1".into(),
            provider: "elevenlabs".into(),
            provider_voice_id: "provider-1".into(),
            model: "model".into(),
            consent_id: "consent-1".into(),
            source_audio_asset_id: "audio-1".into(),
            source_audio_sha256: "a".repeat(64),
            request_hash: "b".repeat(64),
            voice_name: "Narrator".into(),
            revoked: false,
        });
        state.commit_irreversible();

        assert!(state.undo());
        assert_eq!(state.timeline.tracks[0].clips[0].start_frame, 0);
        assert_eq!(state.timeline.voice_models.len(), 1);
        assert!(!state.timeline.voice_models[0].revoked);
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
