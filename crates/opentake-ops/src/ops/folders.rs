//! Media-library folder ops. Ports `createFolder` / `moveAssetsToFolder` from
//! `EditorViewModel+Folders.swift`, operating on the persisted [`MediaManifest`]
//! (entries + folders) rather than the runtime `MediaAsset` list — this leaf
//! crate holds the manifest, not the media-layer asset objects.

use std::collections::HashSet;

use opentake_domain::{MediaFolder, MediaManifest};

use crate::id::IdGen;

/// Create a folder named `name` (optionally nested under `parent_folder_id`),
/// appending it to the manifest. Returns the new folder id. 1:1 port of
/// `createFolder(name:in:)`.
pub fn create_folder(
    manifest: &mut MediaManifest,
    name: impl Into<String>,
    parent_folder_id: Option<String>,
    ids: &dyn IdGen,
) -> String {
    let id = ids.next_id();
    let mut folder = MediaFolder::new(id.clone(), name);
    folder.parent_folder_id = parent_folder_id;
    manifest.folders.push(folder);
    id
}

/// Set `folder_id` on each manifest entry whose id is in `asset_ids` (skipping
/// entries already in that folder). Returns the count of entries actually
/// changed. 1:1 port of `moveAssetsToFolder(assetIds:folderId:)` semantics,
/// against the manifest.
pub fn move_to_folder(
    manifest: &mut MediaManifest,
    asset_ids: &HashSet<String>,
    folder_id: Option<String>,
) -> usize {
    if asset_ids.is_empty() {
        return 0;
    }
    let mut changed = 0;
    for entry in &mut manifest.entries {
        if asset_ids.contains(&entry.id) && entry.folder_id != folder_id {
            entry.folder_id = folder_id.clone();
            changed += 1;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::SeqIdGen;
    use opentake_domain::{ClipType, MediaManifestEntry, MediaSource};

    fn entry(id: &str) -> MediaManifestEntry {
        MediaManifestEntry {
            id: id.into(),
            name: id.into(),
            kind: ClipType::Video,
            source: MediaSource::External {
                absolute_path: format!("/{id}.mp4"),
            },
            duration: 1.0,
            generation_input: None,
            source_width: None,
            source_height: None,
            source_fps: None,
            has_audio: None,
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        }
    }

    #[test]
    fn create_folder_appends_and_returns_id() {
        let mut m = MediaManifest::new();
        let g = SeqIdGen::new("f-");
        let id = create_folder(&mut m, "B-Roll", None, &g);
        assert_eq!(id, "f-1");
        assert_eq!(m.folders.len(), 1);
        assert_eq!(m.folders[0].name, "B-Roll");
        assert!(m.folders[0].parent_folder_id.is_none());
    }

    #[test]
    fn create_folder_nests_under_parent() {
        let mut m = MediaManifest::new();
        let g = SeqIdGen::default();
        let id = create_folder(&mut m, "Child", Some("root".into()), &g);
        let f = m.folders.iter().find(|f| f.id == id).unwrap();
        assert_eq!(f.parent_folder_id.as_deref(), Some("root"));
    }

    #[test]
    fn move_to_folder_sets_folder_id() {
        let mut m = MediaManifest::new();
        m.entries.push(entry("a"));
        m.entries.push(entry("b"));
        let n = move_to_folder(
            &mut m,
            &["a".to_string()].into_iter().collect(),
            Some("f1".into()),
        );
        assert_eq!(n, 1);
        assert_eq!(m.entries[0].folder_id.as_deref(), Some("f1"));
        assert!(m.entries[1].folder_id.is_none());
    }

    #[test]
    fn move_to_folder_skips_already_in_folder() {
        let mut m = MediaManifest::new();
        let mut e = entry("a");
        e.folder_id = Some("f1".into());
        m.entries.push(e);
        let n = move_to_folder(
            &mut m,
            &["a".to_string()].into_iter().collect(),
            Some("f1".into()),
        );
        assert_eq!(n, 0); // no change
    }

    #[test]
    fn move_to_root_clears_folder_id() {
        let mut m = MediaManifest::new();
        let mut e = entry("a");
        e.folder_id = Some("f1".into());
        m.entries.push(e);
        let n = move_to_folder(&mut m, &["a".to_string()].into_iter().collect(), None);
        assert_eq!(n, 1);
        assert!(m.entries[0].folder_id.is_none());
    }
}
