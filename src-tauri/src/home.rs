//! Native recent-project registry and capability-safe Home file actions.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

static REGISTRY_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ProjectEntry {
    id: String,
    path: PathBuf,
    created_at: u64,
    last_opened_at: u64,
    #[serde(default)]
    modified_at: u64,
    #[serde(default)]
    thumbnail_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeProjectEntry {
    path: String,
    name: String,
    created_at: u64,
    opened_at: u64,
    modified_at: u64,
    thumbnail_path: Option<PathBuf>,
    missing: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyRecentProject {
    path: String,
    #[serde(default)]
    opened_at: u64,
    #[serde(default)]
    created_at: Option<u64>,
    #[serde(default)]
    modified_at: Option<u64>,
    #[serde(default)]
    thumbnail_path: Option<String>,
}

struct ProjectRegistry {
    ledger_path: PathBuf,
    entries: Vec<ProjectEntry>,
}

impl ProjectRegistry {
    fn load(ledger_path: PathBuf) -> Result<Self, String> {
        let entries = match fs::read(&ledger_path) {
            Ok(bytes) => serde_json::from_slice::<Vec<ProjectEntry>>(&bytes)
                .map_err(|error| format!("decode project registry: {error}"))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(format!("read project registry: {error}")),
        };
        Ok(Self {
            ledger_path,
            entries,
        })
    }

    #[cfg(test)]
    fn entries(&self) -> &[ProjectEntry] {
        &self.entries
    }

    fn register_at(&mut self, path: PathBuf, opened_at: u64) -> Result<(), String> {
        let path = validated_project_path(&path)?;
        let mut next = self.entries.clone();
        if let Some(entry) = next.iter_mut().find(|entry| same_path(&entry.path, &path)) {
            entry.last_opened_at = opened_at;
            refresh_entry_metadata(entry);
        } else {
            let (modified_at, thumbnail_path) = project_metadata(&path, opened_at);
            next.push(ProjectEntry {
                id: uuid::Uuid::new_v4().to_string(),
                path,
                created_at: opened_at,
                last_opened_at: opened_at,
                modified_at,
                thumbnail_path,
            });
        }
        sort_entries(&mut next);
        self.replace_entries(next)
    }

    fn merge_legacy(&mut self, legacy: &[LegacyRecentProject]) -> Result<(), String> {
        let mut next = self.entries.clone();
        for item in legacy {
            let path = validated_project_path(Path::new(&item.path))?;
            if next.iter().any(|entry| same_path(&entry.path, &path)) {
                continue;
            }
            let opened_at = item.opened_at.max(1);
            let expected_thumbnail = path.join("thumbnail.jpg");
            let legacy_thumbnail = item
                .thumbnail_path
                .as_ref()
                .map(PathBuf::from)
                .filter(|candidate| candidate == &expected_thumbnail);
            let (disk_modified_at, disk_thumbnail) = project_metadata(&path, opened_at);
            next.push(ProjectEntry {
                id: uuid::Uuid::new_v4().to_string(),
                path,
                created_at: item.created_at.unwrap_or(opened_at),
                last_opened_at: opened_at,
                modified_at: if disk_modified_at == opened_at {
                    item.modified_at.unwrap_or(opened_at)
                } else {
                    disk_modified_at
                },
                thumbnail_path: disk_thumbnail.or(legacy_thumbnail),
            });
        }
        sort_entries(&mut next);
        if next != self.entries {
            self.replace_entries(next)?;
        }
        Ok(())
    }

    fn refresh_metadata(&mut self) -> Result<(), String> {
        let mut next = self.entries.clone();
        for entry in &mut next {
            refresh_entry_metadata(entry);
        }
        if next != self.entries {
            self.replace_entries(next)?;
        }
        Ok(())
    }

    fn remove(&mut self, path: &Path) -> Result<bool, String> {
        let path = validated_project_path(path)?;
        let mut next = self.entries.clone();
        next.retain(|entry| !same_path(&entry.path, &path));
        if next == self.entries {
            return Ok(false);
        }
        self.replace_entries(next)?;
        Ok(true)
    }

    fn trash_with(
        &mut self,
        path: &Path,
        move_to_trash: impl FnOnce(&Path) -> Result<(), String>,
    ) -> Result<(), String> {
        let path = validated_project_path(path)?;
        let registered = self
            .entries
            .iter()
            .find(|entry| same_path(&entry.path, &path))
            .map(|entry| entry.path.clone())
            .ok_or_else(|| "project is not registered in Home".to_string())?;

        if registered.exists() {
            move_to_trash(&registered)?;
        }
        self.remove(&registered)?;
        Ok(())
    }

    fn snapshot(&self) -> Vec<HomeProjectEntry> {
        self.entries
            .iter()
            .map(|entry| HomeProjectEntry {
                path: entry.path.to_string_lossy().into_owned(),
                name: project_name(&entry.path),
                created_at: entry.created_at,
                opened_at: entry.last_opened_at,
                modified_at: if entry.modified_at == 0 {
                    entry.last_opened_at
                } else {
                    entry.modified_at
                },
                thumbnail_path: entry.thumbnail_path.clone(),
                missing: !entry.path.exists(),
            })
            .collect()
    }

    fn registered_path(&self, path: &Path) -> Result<PathBuf, String> {
        let path = validated_project_path(path)?;
        self.entries
            .iter()
            .find(|entry| same_path(&entry.path, &path))
            .map(|entry| entry.path.clone())
            .ok_or_else(|| "project is not registered in Home".to_string())
    }

    fn replace_entries(&mut self, entries: Vec<ProjectEntry>) -> Result<(), String> {
        persist_entries(&self.ledger_path, &entries)?;
        self.entries = entries;
        Ok(())
    }
}

fn registry_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|error| format!("resolve Home registry directory: {error}"))?
        .join("project-registry.json"))
}

fn with_registry<T>(
    app: &AppHandle,
    operation: impl FnOnce(&mut ProjectRegistry) -> Result<T, String>,
) -> Result<T, String> {
    let _guard = REGISTRY_LOCK
        .lock()
        .map_err(|_| "project registry lock is poisoned".to_string())?;
    let mut registry = ProjectRegistry::load(registry_path(app)?)?;
    operation(&mut registry)
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn modified_millis(path: &Path) -> Option<u64> {
    fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_millis()
        .try_into()
        .ok()
}

fn project_metadata(path: &Path, fallback_modified_at: u64) -> (u64, Option<PathBuf>) {
    if !path.exists() {
        return (fallback_modified_at, None);
    }
    let project_file = path.join("project.json");
    let modified_at = modified_millis(&project_file)
        .or_else(|| modified_millis(path))
        .unwrap_or(fallback_modified_at);
    let thumbnail = path.join("thumbnail.jpg");
    (modified_at, thumbnail.is_file().then_some(thumbnail))
}

fn refresh_entry_metadata(entry: &mut ProjectEntry) {
    if !entry.path.exists() {
        return;
    }
    let (modified_at, thumbnail_path) = project_metadata(&entry.path, entry.last_opened_at);
    entry.modified_at = modified_at;
    entry.thumbnail_path = thumbnail_path;
}

fn validated_project_path(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err("project path must be absolute".into());
    }
    if !path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("opentake"))
    {
        return Err("Home actions only accept .opentake project bundles".into());
    }
    Ok(path.to_path_buf())
}

fn same_path(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => {
            #[cfg(target_os = "windows")]
            {
                left.to_string_lossy()
                    .eq_ignore_ascii_case(&right.to_string_lossy())
            }
            #[cfg(not(target_os = "windows"))]
            {
                left == right
            }
        }
    }
}

fn sort_entries(entries: &mut [ProjectEntry]) {
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.last_opened_at));
}

fn project_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("Untitled")
        .to_string()
}

fn persist_entries(path: &Path, entries: &[ProjectEntry]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "project registry path has no parent".to_string())?;
    fs::create_dir_all(parent).map_err(|error| format!("create project registry: {error}"))?;
    let temp = parent.join(format!(".project-registry.{}.tmp", uuid::Uuid::new_v4()));
    let bytes = serde_json::to_vec_pretty(entries)
        .map_err(|error| format!("encode project registry: {error}"))?;
    let result = (|| {
        let mut file = fs::File::create(&temp)
            .map_err(|error| format!("create project registry staging file: {error}"))?;
        file.write_all(&bytes)
            .map_err(|error| format!("write project registry: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("sync project registry: {error}"))?;

        match fs::rename(&temp, path) {
            Ok(()) => Ok(()),
            Err(_first_error) if path.exists() => {
                let backup =
                    parent.join(format!(".project-registry.{}.backup", uuid::Uuid::new_v4()));
                fs::rename(path, &backup)
                    .map_err(|error| format!("preserve project registry: {error}"))?;
                match fs::rename(&temp, path) {
                    Ok(()) => {
                        let _ = fs::remove_file(backup);
                        Ok(())
                    }
                    Err(error) => {
                        let _ = fs::rename(&backup, path);
                        Err(format!("publish project registry: {error}"))
                    }
                }
            }
            Err(error) => Err(format!("publish project registry: {error}")),
        }
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

fn reveal_in_file_manager(path: &Path) -> Result<(), String> {
    let (program, arguments): (&str, Vec<String>) = if cfg!(target_os = "macos") {
        if path.exists() {
            (
                "open",
                vec!["-R".into(), path.to_string_lossy().into_owned()],
            )
        } else {
            let parent = path.parent().unwrap_or(path);
            ("open", vec![parent.to_string_lossy().into_owned()])
        }
    } else if cfg!(target_os = "windows") {
        if path.exists() {
            (
                "explorer.exe",
                vec![format!("/select,{}", path.to_string_lossy())],
            )
        } else {
            let parent = path.parent().unwrap_or(path);
            ("explorer.exe", vec![parent.to_string_lossy().into_owned()])
        }
    } else {
        let target = path.parent().unwrap_or(path);
        ("xdg-open", vec![target.to_string_lossy().into_owned()])
    };
    let status = Command::new(program)
        .args(arguments)
        .status()
        .map_err(|error| format!("start file manager: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("file manager exited with status {status}"))
    }
}

fn move_project_to_trash(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use objc2_foundation::{NSFileManager, NSString, NSURL};

        let path_string = path.to_string_lossy();
        let path_string = NSString::from_str(&path_string);
        let url = NSURL::fileURLWithPath(&path_string);
        NSFileManager::defaultManager()
            .trashItemAtURL_resultingItemURL_error(&url, None)
            .map_err(|error| format!("system trash operation failed: {error}"))
    }

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("powershell.exe");
        command.args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Add-Type -AssemblyName Microsoft.VisualBasic; $item = Get-Item -LiteralPath $args[0]; if ($item.PSIsContainer) { [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteDirectory($item.FullName, 'OnlyErrorDialogs', 'SendToRecycleBin') } else { [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteFile($item.FullName, 'OnlyErrorDialogs', 'SendToRecycleBin') }",
            "--",
        ]);
        command.arg(path);
        command
    };

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let mut command = {
        let mut command = Command::new("gio");
        command.arg("trash").arg(path);
        command
    };

    #[cfg(not(target_os = "macos"))]
    let output = command
        .output()
        .map_err(|error| format!("start system trash operation: {error}"))?;
    #[cfg(not(target_os = "macos"))]
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "system trash operation failed with {}{}",
            output.status,
            if stderr.trim().is_empty() {
                String::new()
            } else {
                format!(": {}", stderr.trim())
            }
        ))
    }
}

#[tauri::command]
pub fn home_projects_sync(
    app: AppHandle,
    entries: Vec<LegacyRecentProject>,
) -> Result<Vec<HomeProjectEntry>, String> {
    with_registry(&app, |registry| {
        registry.merge_legacy(&entries)?;
        registry.refresh_metadata()?;
        Ok(registry.snapshot())
    })
}

#[tauri::command]
pub fn home_project_register(
    app: AppHandle,
    path: String,
    opened_at: Option<u64>,
) -> Result<(), String> {
    with_registry(&app, |registry| {
        registry.register_at(PathBuf::from(path), opened_at.unwrap_or_else(now_millis))
    })
}

#[tauri::command]
pub fn home_project_remove(app: AppHandle, path: String) -> Result<(), String> {
    with_registry(&app, |registry| {
        registry.remove(Path::new(&path)).map(|_| ())
    })
}

#[tauri::command]
pub async fn home_project_trash(app: AppHandle, path: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        with_registry(&app, |registry| {
            registry.trash_with(Path::new(&path), move_project_to_trash)
        })
    })
    .await
    .map_err(|error| format!("project trash task failed: {error}"))?
}

#[tauri::command]
pub async fn home_project_reveal(app: AppHandle, path: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let registered =
            with_registry(&app, |registry| registry.registered_path(Path::new(&path)))?;
        reveal_in_file_manager(&registered)
    })
    .await
    .map_err(|error| format!("project reveal task failed: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_entry_survives_registry_load_and_safe_trash_removes_only_after_success() {
        let directory = tempfile::tempdir().unwrap();
        let ledger = directory.path().join("project-registry.json");
        let missing = directory.path().join("Missing.opentake");
        let existing = directory.path().join("Existing.opentake");
        fs::create_dir(&existing).unwrap();

        let mut registry = ProjectRegistry::load(ledger.clone()).unwrap();
        registry.register_at(missing.clone(), 10).unwrap();
        registry.register_at(existing.clone(), 20).unwrap();

        let mut reloaded = ProjectRegistry::load(ledger).unwrap();
        assert!(reloaded.entries().iter().any(|entry| entry.path == missing));

        let denied = reloaded.trash_with(&existing, |_| Err("permission denied".into()));
        assert!(denied.is_err());
        assert!(existing.exists());
        assert!(reloaded
            .entries()
            .iter()
            .any(|entry| entry.path == existing));

        reloaded
            .trash_with(&existing, |path| {
                fs::remove_dir_all(path).map_err(|error| error.to_string())
            })
            .unwrap();
        assert!(!existing.exists());
        assert!(!reloaded
            .entries()
            .iter()
            .any(|entry| entry.path == existing));
    }

    #[test]
    fn unregistered_or_non_project_paths_cannot_reach_trash_capability() {
        let directory = tempfile::tempdir().unwrap();
        let ledger = directory.path().join("project-registry.json");
        let outside = directory.path().join("notes.txt");
        fs::write(&outside, b"keep").unwrap();
        let mut registry = ProjectRegistry::load(ledger).unwrap();
        let mut called = false;

        assert!(registry
            .trash_with(&outside, |_| {
                called = true;
                Ok(())
            })
            .is_err());
        assert!(!called);
        assert!(outside.exists());
    }

    #[test]
    fn snapshot_reads_persisted_thumbnail_and_modified_metadata() {
        let directory = tempfile::tempdir().unwrap();
        let ledger = directory.path().join("project-registry.json");
        let project = directory.path().join("Metadata.opentake");
        fs::create_dir(&project).unwrap();
        fs::write(project.join("project.json"), b"{}").unwrap();
        fs::write(project.join("thumbnail.jpg"), b"jpeg").unwrap();

        let mut registry = ProjectRegistry::load(ledger).unwrap();
        registry.register_at(project.clone(), 10).unwrap();
        let entry = registry.snapshot().pop().unwrap();

        assert!(entry.modified_at > 0);
        assert_eq!(entry.thumbnail_path, Some(project.join("thumbnail.jpg")));
    }
}
