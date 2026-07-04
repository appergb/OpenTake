//! Media-library folder ops. Ports `createFolder` / `moveAssetsToFolder` from
//! `EditorViewModel+Folders.swift`, operating on the persisted [`MediaManifest`]
//! (entries + folders) rather than the runtime `MediaAsset` list — this leaf
//! crate holds the manifest, not the media-layer asset objects.

use std::collections::HashSet;

use opentake_domain::{MediaFolder, MediaManifest, Timeline};

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

/// Rename a media asset by id, returning `true` if a matching entry's name
/// actually changed. 1:1 port of `renameMediaAsset(id:name:)`.
pub fn rename_media(
    manifest: &mut MediaManifest,
    media_ref: &str,
    name: impl Into<String>,
) -> bool {
    let name = name.into();
    match manifest.entries.iter_mut().find(|e| e.id == media_ref) {
        Some(e) if e.name != name => {
            e.name = name;
            true
        }
        _ => false,
    }
}

/// Rename a folder by id, returning `true` if a matching folder's name actually
/// changed. 1:1 port of `renameFolder(id:name:)`.
pub fn rename_folder(
    manifest: &mut MediaManifest,
    folder_id: &str,
    name: impl Into<String>,
) -> bool {
    let name = name.into();
    match manifest.folders.iter_mut().find(|f| f.id == folder_id) {
        Some(f) if f.name != name => {
            f.name = name;
            true
        }
        _ => false,
    }
}

/// Delete media assets by id and cascade-remove any timeline clips referencing
/// them. Returns `(assets_removed, clips_removed)`. 1:1 port of
/// `deleteMediaAssets(ids:)` — the asset removal and the clip cleanup are one
/// step so they undo together.
pub fn delete_media(
    timeline: &mut Timeline,
    manifest: &mut MediaManifest,
    asset_ids: &HashSet<String>,
) -> (usize, usize) {
    if asset_ids.is_empty() {
        return (0, 0);
    }
    let before = manifest.entries.len();
    manifest.entries.retain(|e| !asset_ids.contains(&e.id));
    let assets_removed = before - manifest.entries.len();
    let clips_removed = cascade_remove_clips(timeline, asset_ids);
    (assets_removed, clips_removed)
}

/// Delete folders recursively (with all descendant folders and the assets inside
/// them) and cascade-remove clips referencing any deleted asset. Returns
/// `(folders_removed, assets_removed, clips_removed)`. 1:1 port of
/// `deleteFolders(ids:)`.
pub fn delete_folder(
    timeline: &mut Timeline,
    manifest: &mut MediaManifest,
    folder_ids: &HashSet<String>,
) -> (usize, usize, usize) {
    if folder_ids.is_empty() {
        return (0, 0, 0);
    }
    let doomed_folders = expand_descendant_folders(manifest, folder_ids);
    // Assets that live (directly) in any doomed folder are deleted too.
    let doomed_assets: HashSet<String> = manifest
        .entries
        .iter()
        .filter(|e| {
            e.folder_id
                .as_deref()
                .is_some_and(|fid| doomed_folders.contains(fid))
        })
        .map(|e| e.id.clone())
        .collect();
    let before = manifest.folders.len();
    manifest.folders.retain(|f| !doomed_folders.contains(&f.id));
    let folders_removed = before - manifest.folders.len();
    let (assets_removed, clips_removed) = delete_media(timeline, manifest, &doomed_assets);
    (folders_removed, assets_removed, clips_removed)
}

/// Remove every clip whose `media_ref` is in `asset_ids`, then prune any tracks
/// left empty (mirroring `remove_clips`). Returns the count of clips removed.
fn cascade_remove_clips(timeline: &mut Timeline, asset_ids: &HashSet<String>) -> usize {
    let doomed: Vec<String> = timeline
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter())
        .filter(|c| asset_ids.contains(&c.media_ref))
        .map(|c| c.id.clone())
        .collect();
    let count = doomed.len();
    for id in &doomed {
        crate::ops::clear_region::remove_clip(timeline, id);
    }
    if count > 0 {
        crate::ops::prune_empty_tracks(timeline);
    }
    count
}

/// Expand a set of root folder ids to include all transitive descendant folders
/// (so deleting a folder also deletes its subfolders). Fixed-point over the
/// parent links.
fn expand_descendant_folders(manifest: &MediaManifest, roots: &HashSet<String>) -> HashSet<String> {
    let mut all: HashSet<String> = roots.clone();
    loop {
        let mut added = false;
        for f in &manifest.folders {
            if let Some(parent) = &f.parent_folder_id {
                if all.contains(parent) && all.insert(f.id.clone()) {
                    added = true;
                }
            }
        }
        if !added {
            break;
        }
    }
    all
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

    fn timeline_with_clip(clip_id: &str, media_ref: &str) -> Timeline {
        use opentake_domain::{Clip, Track};
        let mut tl = Timeline::new();
        let mut t = Track::new("t1", ClipType::Video);
        t.clips.push(Clip::new(clip_id, media_ref, 0, 30));
        tl.tracks.push(t);
        tl
    }

    #[test]
    fn rename_media_sets_name_and_reports_change() {
        let mut m = MediaManifest::new();
        m.entries.push(entry("a"));
        assert!(rename_media(&mut m, "a", "Hero Shot"));
        assert_eq!(m.entries[0].name, "Hero Shot");
        // Renaming to the same name reports no change.
        assert!(!rename_media(&mut m, "a", "Hero Shot"));
        // Unknown id reports no change.
        assert!(!rename_media(&mut m, "ghost", "X"));
    }

    #[test]
    fn rename_folder_sets_name_and_reports_change() {
        let mut m = MediaManifest::new();
        let g = SeqIdGen::new("f-");
        let id = create_folder(&mut m, "Old", None, &g);
        assert!(rename_folder(&mut m, &id, "New"));
        assert_eq!(m.folders[0].name, "New");
        assert!(!rename_folder(&mut m, "ghost", "X"));
    }

    #[test]
    fn delete_media_removes_entry_and_cascades_clips() {
        let mut m = MediaManifest::new();
        m.entries.push(entry("a"));
        m.entries.push(entry("b"));
        let mut tl = timeline_with_clip("clip-1", "a");
        // A second clip referencing the surviving asset 'b' stays.
        tl.tracks[0]
            .clips
            .push(opentake_domain::Clip::new("clip-2", "b", 40, 30));

        let (assets, clips) =
            delete_media(&mut tl, &mut m, &["a".to_string()].into_iter().collect());
        assert_eq!(assets, 1);
        assert_eq!(clips, 1);
        assert_eq!(m.entries.len(), 1);
        assert_eq!(m.entries[0].id, "b");
        // Only the clip referencing 'a' was removed.
        assert_eq!(tl.tracks[0].clips.len(), 1);
        assert_eq!(tl.tracks[0].clips[0].id, "clip-2");
    }

    #[test]
    fn delete_folder_recurses_and_cascades() {
        let mut m = MediaManifest::new();
        let g = SeqIdGen::new("f-");
        let parent = create_folder(&mut m, "Parent", None, &g);
        let child = create_folder(&mut m, "Child", Some(parent.clone()), &g);
        // Asset 'a' lives in the child folder; asset 'b' is at root.
        let mut a = entry("a");
        a.folder_id = Some(child.clone());
        m.entries.push(a);
        m.entries.push(entry("b"));
        let tl = &mut timeline_with_clip("clip-1", "a");

        let (folders, assets, clips) = delete_folder(tl, &mut m, &[parent].into_iter().collect());
        assert_eq!(folders, 2); // parent + child
        assert_eq!(assets, 1); // 'a' was inside the child
        assert_eq!(clips, 1); // its clip cascaded
        assert_eq!(m.folders.len(), 0);
        assert_eq!(m.entries.len(), 1);
        assert_eq!(m.entries[0].id, "b");
        assert!(tl.tracks.is_empty()); // empty track pruned
    }
}
