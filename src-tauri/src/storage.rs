//! Settings "Storage" pane backend: real on-disk byte usage for derived caches
//! plus destructive-clear commands. Semantic port of upstream
//! `Settings/StoragePane.swift` (which shows cache / search-index / model bytes
//! and per-section clear buttons) — adapted to OpenTake's cache layout:
//!
//! - `thumbnails` + `waveforms` share `<cache_root>/MediaVisualCache/` (filmstrip
//!   sprites, posters, previews, `*.waveform`); the two categories are split by
//!   file extension so each clears independently.
//! - `searchIndex` = `<cache_root>/Embeddings/` (`PALMEMB1` stores).
//! - `models` = the engine's `<models_dir>` (whisper + SigLIP2 + RVM downloads).
//!   Re-downloads, NOT a lazily-rebuilt cache — gated behind an explicit
//!   `modelsConfirmed` flag the UI only sets after a confirm step.
//! - `other` = the remaining known derived-cache subdirs under the cache root
//!   (transcripts, generation staging, advanced-workflow renders).
//!
//! NEVER touched: project bundles, the global media library
//! (`<app_data_dir>/OpenTake/Library`), user media, credentials — this module
//! only ever operates on the two engine-owned roots it is handed. Cleared cache
//! roots are recreated so the engine stays functional (its save paths
//! `create_dir_all` lazily anyway). Every command is a pure function over
//! `(cache_root, models_dir)` so the whole surface is unit-testable with temp
//! dirs and fails gracefully (missing dirs = 0 bytes, no panics).

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::State;

use opentake_media::search::embed_store;
use opentake_media::transcribe::cache as transcript_cache;
use opentake_media::waveform::store as waveform_store;

use crate::media::MediaState;

/// One clearable cache category. The serialized ids are lowercase/camelCase
/// strings (`thumbnails`, `waveforms`, `searchIndex`, `models`, `other`) that
/// the web mirror shares verbatim.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StorageCategoryId {
    Thumbnails,
    Waveforms,
    SearchIndex,
    Models,
    Other,
}

/// Byte usage for one category, plus its on-disk root (display only — the UI
/// shows the concrete path so "what will be cleared" is never abstract).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageCategoryUsageDto {
    pub id: StorageCategoryId,
    pub bytes: u64,
    pub path: String,
}

/// `storage_usage` result: every category (zero bytes included — the UI needs
/// stable rows) plus the total and the cache root shown in the pane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageUsageDto {
    pub categories: Vec<StorageCategoryUsageDto>,
    pub total_bytes: u64,
    pub cache_root: String,
}

/// `storage_clear` request. `modelsConfirmed` is the destructive gate: `Models`
/// is rejected unless the user confirmed in the UI (a model is a re-download,
/// not a lazily-rebuilt cache).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageClearRequest {
    #[serde(default)]
    pub categories: Vec<StorageCategoryId>,
    #[serde(default)]
    pub models_confirmed: bool,
}

/// Known derived-cache subdirectories under the cache root that don't have
/// their own category. Deliberately enumerated — unknown dirs are never swept.
const OTHER_CACHE_SUBDIRS: [&str; 4] = [
    transcript_cache::CACHE_SUBDIR,
    "generation-staging",
    "matting",
    "object-removal",
];

/// `storage_usage`: real per-category byte counts for the engine's cache root
/// and models dir. Never mutates anything. Missing dirs read as 0 bytes.
#[tauri::command]
pub fn storage_usage(media: State<'_, MediaState>) -> StorageUsageDto {
    let engine = media.engine();
    usage_at(engine.cache_root(), engine.models_dir())
}

/// `storage_clear`: delete ONLY derived caches for the requested categories
/// (see module docs for what each category covers), recreate every cache root
/// that was removed, and return the fresh usage snapshot. `Models` requires
/// `modelsConfirmed`. Project files, the global library, user media and
/// credentials are never reachable from the two engine-owned roots.
#[tauri::command]
pub fn storage_clear(
    media: State<'_, MediaState>,
    admission: State<'_, crate::updater::InstallAdmissionGate>,
    request: StorageClearRequest,
) -> Result<StorageUsageDto, String> {
    let _activity = crate::updater::begin_mutating_activity(&admission)?;
    let engine = media.engine();
    clear_at(engine.cache_root(), engine.models_dir(), &request)?;
    Ok(usage_at(engine.cache_root(), engine.models_dir()))
}

/// Pure usage computation (the testable seam — commands resolve the real
/// engine roots, tests pass temp dirs).
pub(crate) fn usage_at(cache_root: &Path, models_dir: &Path) -> StorageUsageDto {
    let visual_dir = cache_root.join(waveform_store::CACHE_SUBDIR);
    let (thumbnails, waveforms) = visual_cache_bytes(&visual_dir);
    let search_index = dir_bytes(&cache_root.join(embed_store::CACHE_SUBDIR));
    let models = dir_bytes(models_dir);
    let other: u64 = OTHER_CACHE_SUBDIRS
        .iter()
        .map(|sub| dir_bytes(&cache_root.join(sub)))
        .sum();

    let categories = vec![
        StorageCategoryUsageDto {
            id: StorageCategoryId::Thumbnails,
            bytes: thumbnails,
            path: display(&visual_dir),
        },
        StorageCategoryUsageDto {
            id: StorageCategoryId::Waveforms,
            bytes: waveforms,
            path: display(&visual_dir),
        },
        StorageCategoryUsageDto {
            id: StorageCategoryId::SearchIndex,
            bytes: search_index,
            path: display(cache_root.join(embed_store::CACHE_SUBDIR)),
        },
        StorageCategoryUsageDto {
            id: StorageCategoryId::Models,
            bytes: models,
            path: display(models_dir),
        },
        StorageCategoryUsageDto {
            id: StorageCategoryId::Other,
            bytes: other,
            path: display(cache_root),
        },
    ];
    StorageUsageDto {
        total_bytes: categories.iter().map(|category| category.bytes).sum(),
        categories,
        cache_root: display(cache_root),
    }
}

/// Pure clear (the testable seam). Clears as much as possible, then fails with
/// a joined report if anything could not be removed — never a fake success.
pub(crate) fn clear_at(
    cache_root: &Path,
    models_dir: &Path,
    request: &StorageClearRequest,
) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    let mut failures: Vec<String> = Vec::new();
    for id in &request.categories {
        if !seen.insert(*id) {
            continue; // duplicate ids in one request are a no-op
        }
        let result = match id {
            StorageCategoryId::Thumbnails => {
                remove_visual_files(&cache_root.join(waveform_store::CACHE_SUBDIR), true)
            }
            StorageCategoryId::Waveforms => {
                remove_visual_files(&cache_root.join(waveform_store::CACHE_SUBDIR), false)
            }
            StorageCategoryId::SearchIndex => {
                clear_and_recreate(&cache_root.join(embed_store::CACHE_SUBDIR))
            }
            StorageCategoryId::Models => clear_models(models_dir, request.models_confirmed),
            StorageCategoryId::Other => OTHER_CACHE_SUBDIRS
                .iter()
                .try_for_each(|sub| clear_and_recreate(&cache_root.join(sub))),
        };
        if let Err(error) = result {
            failures.push(error);
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

/// Split `<cache_root>/MediaVisualCache` into (thumbnails, waveforms) bytes:
/// `*.waveform` files are the waveform cache, everything else (sprites,
/// posters, sidecars, partials) is the thumbnail cache.
fn visual_cache_bytes(dir: &Path) -> (u64, u64) {
    let mut thumbnails = 0u64;
    let mut waveforms = 0u64;
    let Ok(entries) = fs::read_dir(dir) else {
        return (0, 0);
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_waveform = path.extension().and_then(|ext| ext.to_str()) == Some("waveform");
        let meta = match fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(_) => continue, // raced away mid-scan: skip
        };
        if !meta.is_file() {
            continue; // symlinks and unexpected subdirs are not counted
        }
        if is_waveform {
            waveforms += meta.len();
        } else {
            thumbnails += meta.len();
        }
    }
    (thumbnails, waveforms)
}

/// Recursive byte total for `path` (missing dir = 0). Symlinks are never
/// followed; entries that fail metadata are skipped so one bad file can't
/// poison the whole usage report.
fn dir_bytes(path: &Path) -> u64 {
    let mut total = 0u64;
    walk(path, &mut total);
    total
}

fn walk(path: &Path, total: &mut u64) {
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.is_dir() && !meta.file_type().is_symlink() {
            walk(&path, total);
        } else if meta.is_file() && !meta.file_type().is_symlink() {
            *total += meta.len();
        }
    }
}

/// Delete the visual-cache files of one family (keep_waveforms=true removes
/// everything except `*.waveform`, false removes only `*.waveform`), keeping
/// the directory itself so the engine's lazy rebuild finds a valid root.
fn remove_visual_files(dir: &Path, keep_waveforms: bool) -> Result<(), String> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("read {}: {error}", dir.display())),
    };
    let mut failures: Vec<String> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let is_waveform = path.extension().and_then(|ext| ext.to_str()) == Some("waveform");
        if is_waveform == keep_waveforms {
            continue;
        }
        let meta = match fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                failures.push(format!("{}: {error}", path.display()));
                continue;
            }
        };
        if !meta.is_file() {
            continue; // never recurse into unexpected subdirectories
        }
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => failures.push(format!("{}: {error}", path.display())),
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

/// Remove `dir` (contents + root) and recreate it empty. `NotFound` is a
/// successful no-op; the recreated root keeps every consumer functional.
fn clear_and_recreate(dir: &Path) -> Result<(), String> {
    match fs::remove_dir_all(dir) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("remove {}: {error}", dir.display())),
    }
    fs::create_dir_all(dir).map_err(|error| format!("recreate {}: {error}", dir.display()))
}

/// Models are re-downloads, not lazily-rebuilt caches: clearing them requires
/// the user's explicit confirmation (`modelsConfirmed`), which the UI only
/// sends after its confirm step.
fn clear_models(models_dir: &Path, confirmed: bool) -> Result<(), String> {
    if !confirmed {
        return Err("storage_clear_models_requires_confirmation".to_string());
    }
    clear_and_recreate(models_dir)
}

fn display(path: impl Into<PathBuf>) -> String {
    path.into().to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Seed the standard cache layout under `root` and return the expected
    /// per-category bytes so every test shares one fixture.
    fn seed_caches(root: &Path) -> (u64, u64, u64, u64, u64) {
        let visual = root.join(waveform_store::CACHE_SUBDIR);
        fs::create_dir_all(&visual).unwrap();
        fs::write(visual.join("a.thumbs.jpg"), vec![0u8; 100]).unwrap();
        fs::write(visual.join("a.thumbs.json"), vec![0u8; 20]).unwrap();
        fs::write(visual.join("a.thumb.png"), vec![0u8; 30]).unwrap();
        fs::write(visual.join("a.waveform"), vec![0u8; 40]).unwrap();
        fs::write(visual.join("b.waveform"), vec![0u8; 60]).unwrap();

        let embeddings = root.join(embed_store::CACHE_SUBDIR);
        fs::create_dir_all(&embeddings).unwrap();
        fs::write(embeddings.join("x.embed"), vec![0u8; 50]).unwrap();

        let models = root.join("models");
        fs::create_dir_all(models.join("whisper")).unwrap();
        fs::write(models.join("whisper/base.bin"), vec![0u8; 500]).unwrap();

        fs::create_dir_all(root.join(transcript_cache::CACHE_SUBDIR)).unwrap();
        fs::write(
            root.join(transcript_cache::CACHE_SUBDIR).join("t.json"),
            vec![0u8; 10],
        )
        .unwrap();
        fs::create_dir_all(root.join("generation-staging")).unwrap();
        fs::write(root.join("generation-staging/g.mp4"), vec![0u8; 200]).unwrap();

        // thumbnails=150, waveforms=100, searchIndex=50, models=500, other=210
        (150, 100, 50, 500, 210)
    }

    fn usage_of(root: &Path) -> StorageUsageDto {
        usage_at(root, &root.join("models"))
    }

    fn bytes_of(dto: &StorageUsageDto, id: StorageCategoryId) -> u64 {
        dto.categories
            .iter()
            .find(|category| category.id == id)
            .expect("every category is always present")
            .bytes
    }

    #[test]
    fn usage_at_is_all_zero_when_dirs_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let dto = usage_of(tmp.path());
        assert_eq!(dto.total_bytes, 0);
        assert_eq!(dto.categories.len(), 5);
        assert_eq!(bytes_of(&dto, StorageCategoryId::Thumbnails), 0);
        assert_eq!(bytes_of(&dto, StorageCategoryId::Waveforms), 0);
        assert_eq!(bytes_of(&dto, StorageCategoryId::SearchIndex), 0);
        assert_eq!(bytes_of(&dto, StorageCategoryId::Models), 0);
        assert_eq!(bytes_of(&dto, StorageCategoryId::Other), 0);
    }

    #[test]
    fn usage_at_counts_each_category() {
        let tmp = tempfile::tempdir().unwrap();
        let expected = seed_caches(tmp.path());
        let dto = usage_of(tmp.path());
        assert_eq!(bytes_of(&dto, StorageCategoryId::Thumbnails), expected.0);
        assert_eq!(bytes_of(&dto, StorageCategoryId::Waveforms), expected.1);
        assert_eq!(bytes_of(&dto, StorageCategoryId::SearchIndex), expected.2);
        assert_eq!(bytes_of(&dto, StorageCategoryId::Models), expected.3);
        assert_eq!(bytes_of(&dto, StorageCategoryId::Other), expected.4);
        assert_eq!(
            dto.total_bytes,
            expected.0 + expected.1 + expected.2 + expected.3 + expected.4
        );
    }

    #[test]
    fn usage_at_never_follows_symlinks() {
        let tmp = tempfile::tempdir().unwrap();
        let visual = tmp.path().join(waveform_store::CACHE_SUBDIR);
        fs::create_dir_all(&visual).unwrap();
        let outside = tmp.path().join("outside.bin");
        fs::write(&outside, vec![0u8; 4096]).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, visual.join("linked.waveform")).unwrap();
        let dto = usage_of(tmp.path());
        assert_eq!(bytes_of(&dto, StorageCategoryId::Waveforms), 0);
        assert_eq!(bytes_of(&dto, StorageCategoryId::Thumbnails), 0);
    }

    #[test]
    fn clear_at_thumbnails_keeps_waveforms_and_other_categories() {
        let tmp = tempfile::tempdir().unwrap();
        seed_caches(tmp.path());
        let request = StorageClearRequest {
            categories: vec![StorageCategoryId::Thumbnails],
            models_confirmed: false,
        };
        clear_at(tmp.path(), &tmp.path().join("models"), &request).unwrap();
        let dto = usage_of(tmp.path());
        assert_eq!(bytes_of(&dto, StorageCategoryId::Thumbnails), 0);
        assert_eq!(bytes_of(&dto, StorageCategoryId::Waveforms), 100);
        assert_eq!(bytes_of(&dto, StorageCategoryId::SearchIndex), 50);
        assert_eq!(bytes_of(&dto, StorageCategoryId::Models), 500);
        assert_eq!(bytes_of(&dto, StorageCategoryId::Other), 210);
    }

    #[test]
    fn clear_at_waveforms_keeps_thumbnails() {
        let tmp = tempfile::tempdir().unwrap();
        seed_caches(tmp.path());
        let request = StorageClearRequest {
            categories: vec![StorageCategoryId::Waveforms],
            models_confirmed: false,
        };
        clear_at(tmp.path(), &tmp.path().join("models"), &request).unwrap();
        let dto = usage_of(tmp.path());
        assert_eq!(bytes_of(&dto, StorageCategoryId::Waveforms), 0);
        assert_eq!(bytes_of(&dto, StorageCategoryId::Thumbnails), 150);
    }

    #[test]
    fn clear_at_search_index_recreates_the_root_dir() {
        let tmp = tempfile::tempdir().unwrap();
        seed_caches(tmp.path());
        let request = StorageClearRequest {
            categories: vec![StorageCategoryId::SearchIndex],
            models_confirmed: false,
        };
        clear_at(tmp.path(), &tmp.path().join("models"), &request).unwrap();
        let embeddings = tmp.path().join(embed_store::CACHE_SUBDIR);
        assert!(embeddings.is_dir(), "cache root must be recreated");
        assert_eq!(fs::read_dir(&embeddings).unwrap().count(), 0);
        assert_eq!(
            bytes_of(&usage_of(tmp.path()), StorageCategoryId::SearchIndex),
            0
        );
    }

    #[test]
    fn clear_at_models_requires_explicit_confirmation() {
        let tmp = tempfile::tempdir().unwrap();
        seed_caches(tmp.path());
        let request = StorageClearRequest {
            categories: vec![StorageCategoryId::Models],
            models_confirmed: false,
        };
        let error = clear_at(tmp.path(), &tmp.path().join("models"), &request)
            .expect_err("models must require confirmation");
        assert!(error.contains("requires_confirmation"));
        assert_eq!(
            bytes_of(&usage_of(tmp.path()), StorageCategoryId::Models),
            500
        );
    }

    #[test]
    fn clear_at_models_with_confirmation_removes_and_recreates() {
        let tmp = tempfile::tempdir().unwrap();
        seed_caches(tmp.path());
        let request = StorageClearRequest {
            categories: vec![StorageCategoryId::Models],
            models_confirmed: true,
        };
        clear_at(tmp.path(), &tmp.path().join("models"), &request).unwrap();
        let models = tmp.path().join("models");
        assert!(models.is_dir(), "models root must be recreated");
        assert_eq!(fs::read_dir(&models).unwrap().count(), 0);
        assert_eq!(
            bytes_of(&usage_of(tmp.path()), StorageCategoryId::Models),
            0
        );
    }

    #[test]
    fn clear_at_other_removes_only_known_subdirs() {
        let tmp = tempfile::tempdir().unwrap();
        seed_caches(tmp.path());
        fs::create_dir_all(tmp.path().join("mystery")).unwrap();
        fs::write(tmp.path().join("mystery/keep.bin"), vec![0u8; 77]).unwrap();
        let request = StorageClearRequest {
            categories: vec![StorageCategoryId::Other],
            models_confirmed: false,
        };
        clear_at(tmp.path(), &tmp.path().join("models"), &request).unwrap();
        let dto = usage_of(tmp.path());
        assert_eq!(bytes_of(&dto, StorageCategoryId::Other), 0);
        assert!(
            tmp.path().join("mystery").is_dir(),
            "unknown dirs are never swept"
        );
        assert_eq!(
            fs::read(tmp.path().join("mystery/keep.bin")).unwrap().len(),
            77
        );
        assert_eq!(bytes_of(&dto, StorageCategoryId::Thumbnails), 150);
    }

    #[test]
    fn clear_at_preserves_sentinel_files_outside_the_caches() {
        let tmp = tempfile::tempdir().unwrap();
        seed_caches(tmp.path());
        let sentinel = tmp.path().join("library-sentinel.txt");
        fs::write(&sentinel, b"library content").unwrap();
        let request = StorageClearRequest {
            categories: vec![
                StorageCategoryId::Thumbnails,
                StorageCategoryId::Waveforms,
                StorageCategoryId::SearchIndex,
                StorageCategoryId::Models,
                StorageCategoryId::Other,
            ],
            models_confirmed: true,
        };
        clear_at(tmp.path(), &tmp.path().join("models"), &request).unwrap();
        assert_eq!(fs::read_to_string(&sentinel).unwrap(), "library content");
        assert_eq!(usage_of(tmp.path()).total_bytes, 0);
    }

    #[test]
    fn clear_at_empty_request_is_a_noop() {
        let tmp = tempfile::tempdir().unwrap();
        seed_caches(tmp.path());
        let request = StorageClearRequest {
            categories: Vec::new(),
            models_confirmed: false,
        };
        clear_at(tmp.path(), &tmp.path().join("models"), &request).unwrap();
        assert_eq!(usage_of(tmp.path()).total_bytes, 1010);
    }

    #[test]
    fn clear_at_duplicate_categories_are_handled_once() {
        let tmp = tempfile::tempdir().unwrap();
        seed_caches(tmp.path());
        let request = StorageClearRequest {
            categories: vec![
                StorageCategoryId::Thumbnails,
                StorageCategoryId::Thumbnails,
                StorageCategoryId::Thumbnails,
            ],
            models_confirmed: false,
        };
        clear_at(tmp.path(), &tmp.path().join("models"), &request).unwrap();
        assert_eq!(
            bytes_of(&usage_of(tmp.path()), StorageCategoryId::Thumbnails),
            0
        );
    }

    #[test]
    fn storage_usage_dto_round_trips_camel_case() {
        let tmp = tempfile::tempdir().unwrap();
        seed_caches(tmp.path());
        let dto = usage_of(tmp.path());
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains("\"totalBytes\":"));
        assert!(json.contains("\"cacheRoot\":"));
        assert!(json.contains("\"id\":\"searchIndex\""));
        assert!(json.contains("\"id\":\"thumbnails\""));
        assert!(json.contains("\"id\":\"waveforms\""));
        assert!(json.contains("\"id\":\"models\""));
        assert!(json.contains("\"id\":\"other\""));
        let back: StorageUsageDto = serde_json::from_str(&json).unwrap();
        assert_eq!(dto, back);
    }

    #[test]
    fn storage_clear_request_defaults_models_confirmed_false() {
        let request: StorageClearRequest =
            serde_json::from_str(r#"{"categories":["models"]}"#).unwrap();
        assert_eq!(request.categories, vec![StorageCategoryId::Models]);
        assert!(!request.models_confirmed);
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"modelsConfirmed\":false"));
    }

    #[test]
    fn unknown_category_id_is_rejected() {
        let error = serde_json::from_str::<StorageClearRequest>(
            r#"{"categories":["bogus"],"modelsConfirmed":false}"#,
        )
        .expect_err("unknown category ids must not deserialize");
        assert!(!error.to_string().is_empty());
    }
}
