//! Native recent-project registry and capability-safe Home file actions.

use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex, OnceLock,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use cap_fs_ext::{ambient_authority, DirExt};
use cap_std::fs::Dir;
use serde::{
    de::{self, SeqAccess, Visitor},
    Deserialize, Deserializer, Serialize,
};
use tauri::{AppHandle, Manager};

static REGISTRY_LOCK: Mutex<()> = Mutex::new(());
const MAX_RECENT_PROJECTS: usize = 12;
const MAX_PROJECT_PATH_BYTES: usize = 32_768;
const MAX_REGISTRY_BYTES: u64 = 512 * 1024;
const MAX_PROJECT_PREVIEW_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PROJECT_PREVIEW_TRACKS: usize = 64;
const MAX_HOME_THUMBNAIL_BYTES: u64 = 8 * 1024 * 1024;
const MAX_HOME_THUMBNAIL_DIMENSION: u32 = 16_384;
const HOME_PROBE_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ProjectBundleIdentity {
    volume: u64,
    file: u64,
}

struct HomeProbeCoordinator {
    gate: tokio::sync::Mutex<()>,
    circuit_open: AtomicBool,
}

impl HomeProbeCoordinator {
    fn new() -> Self {
        Self {
            gate: tokio::sync::Mutex::new(()),
            circuit_open: AtomicBool::new(false),
        }
    }
}

static HOME_PROBE_COORDINATOR: OnceLock<HomeProbeCoordinator> = OnceLock::new();

fn home_probe_coordinator() -> &'static HomeProbeCoordinator {
    HOME_PROBE_COORDINATOR.get_or_init(HomeProbeCoordinator::new)
}

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bundle_identity: Option<ProjectBundleIdentity>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    preview: Option<HomeProjectPreview>,
    missing: bool,
    offline: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeProjectPreview {
    canvas_width: i32,
    canvas_height: i32,
    track_kinds: Vec<opentake_domain::ClipType>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HomeProjectPreviewWire {
    width: Option<i32>,
    height: Option<i32>,
    #[serde(
        default,
        rename = "tracks",
        deserialize_with = "deserialize_optional_home_track_kinds"
    )]
    track_kinds: Option<Vec<opentake_domain::ClipType>>,
}

#[derive(Debug, Deserialize)]
struct HomeTrackPreviewWire {
    #[serde(rename = "type")]
    kind: opentake_domain::ClipType,
}

fn deserialize_optional_home_track_kinds<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<opentake_domain::ClipType>>, D::Error>
where
    D: Deserializer<'de>,
{
    struct TrackKindsVisitor;

    impl<'de> Visitor<'de> for TrackKindsVisitor {
        type Value = Vec<opentake_domain::ClipType>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                formatter,
                "at most {MAX_PROJECT_PREVIEW_TRACKS} project tracks"
            )
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut kinds = Vec::with_capacity(
                sequence
                    .size_hint()
                    .unwrap_or_default()
                    .min(MAX_PROJECT_PREVIEW_TRACKS),
            );
            while let Some(track) = sequence.next_element::<HomeTrackPreviewWire>()? {
                if kinds.len() == MAX_PROJECT_PREVIEW_TRACKS {
                    return Err(de::Error::custom(format!(
                        "project preview exceeds the {MAX_PROJECT_PREVIEW_TRACKS}-track limit"
                    )));
                }
                kinds.push(track.kind);
            }
            Ok(kinds)
        }
    }

    deserializer.deserialize_seq(TrackKindsVisitor).map(Some)
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
        let mut entries = match fs::File::open(&ledger_path) {
            Ok(file) => {
                let mut bytes = Vec::new();
                file.take(MAX_REGISTRY_BYTES + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|error| format!("read project registry: {error}"))?;
                if bytes.len() as u64 > MAX_REGISTRY_BYTES {
                    return Err(format!(
                        "project registry exceeds the {MAX_REGISTRY_BYTES}-byte limit"
                    ));
                }
                serde_json::from_slice::<Vec<ProjectEntry>>(&bytes)
                    .map_err(|error| format!("decode project registry: {error}"))?
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(format!("inspect project registry: {error}")),
        };
        entries.retain(|entry| validated_project_path(&entry.path).is_ok());
        normalize_entries(&mut entries);
        Ok(Self {
            ledger_path,
            entries,
        })
    }

    #[cfg(test)]
    fn entries(&self) -> &[ProjectEntry] {
        &self.entries
    }

    fn register_at(
        &mut self,
        path: PathBuf,
        opened_at: u64,
        bundle_identity: Option<ProjectBundleIdentity>,
    ) -> Result<(), String> {
        let path = validated_project_path(&path)?;
        let mut next = self.entries.clone();
        if let Some(entry) = next.iter_mut().find(|entry| same_path(&entry.path, &path)) {
            entry.last_opened_at = opened_at;
            if bundle_identity.is_some() {
                entry.bundle_identity = bundle_identity;
            }
        } else {
            next.push(ProjectEntry {
                id: uuid::Uuid::new_v4().to_string(),
                path,
                created_at: opened_at,
                last_opened_at: opened_at,
                modified_at: opened_at,
                thumbnail_path: None,
                bundle_identity,
            });
        }
        sort_entries(&mut next);
        self.replace_entries(next)
    }

    fn merge_legacy(&mut self, legacy: &[LegacyRecentProject]) -> Result<(), String> {
        let mut next = self.entries.clone();
        for item in legacy.iter().take(MAX_RECENT_PROJECTS) {
            let Ok(path) = validated_project_path(Path::new(&item.path)) else {
                continue;
            };
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
            next.push(ProjectEntry {
                id: uuid::Uuid::new_v4().to_string(),
                path,
                created_at: item.created_at.unwrap_or(opened_at),
                last_opened_at: opened_at,
                modified_at: item.modified_at.unwrap_or(opened_at),
                thumbnail_path: legacy_thumbnail,
                bundle_identity: None,
            });
        }
        sort_entries(&mut next);
        if next != self.entries {
            self.replace_entries(next)?;
        }
        Ok(())
    }

    fn retain_authorized(
        &mut self,
        mut authorized: impl FnMut(&Path) -> bool,
    ) -> Result<(), String> {
        let mut next = self.entries.clone();
        next.retain(|entry| authorized(&entry.path));
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

    fn entries_snapshot(&self) -> Vec<ProjectEntry> {
        self.entries.clone()
    }

    fn registered_path(&self, path: &Path) -> Result<PathBuf, String> {
        let path = validated_project_path(path)?;
        self.entries
            .iter()
            .find(|entry| same_path(&entry.path, &path))
            .map(|entry| entry.path.clone())
            .ok_or_else(|| "project is not registered in Home".to_string())
    }

    fn registered_entry(&self, path: &Path) -> Result<ProjectEntry, String> {
        let path = validated_project_path(path)?;
        self.entries
            .iter()
            .find(|entry| same_path(&entry.path, &path))
            .cloned()
            .ok_or_else(|| "project is not registered in Home".to_string())
    }

    fn remove_registered_entry(&mut self, expected: &ProjectEntry) -> Result<bool, String> {
        let path = validated_project_path(&expected.path)?;
        let Some(current) = self
            .entries
            .iter()
            .find(|entry| same_path(&entry.path, &path))
        else {
            return Ok(false);
        };
        if current.id != expected.id || current.bundle_identity != expected.bundle_identity {
            return Err("Home project registration changed during trash operation".into());
        }
        self.remove(&path)
    }

    fn replace_entries(&mut self, entries: Vec<ProjectEntry>) -> Result<(), String> {
        let mut entries = entries;
        normalize_entries(&mut entries);
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

fn modified_millis(metadata: &fs::Metadata) -> Option<u64> {
    metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_millis()
        .try_into()
        .ok()
}

fn home_entry(
    entry: &ProjectEntry,
    modified_at: u64,
    thumbnail_path: Option<PathBuf>,
    missing: bool,
    offline: bool,
) -> HomeProjectEntry {
    HomeProjectEntry {
        path: entry.path.to_string_lossy().into_owned(),
        name: project_name(&entry.path),
        created_at: entry.created_at,
        opened_at: entry.last_opened_at,
        modified_at,
        thumbnail_path,
        preview: None,
        missing,
        offline,
    }
}

fn read_project_preview(bundle: &Path) -> Option<HomeProjectPreview> {
    let root = opentake_project::ProjectRoot::open(bundle).ok()?;
    let file = root.open_asset_file(Path::new("project.json")).ok()?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() || metadata.len() > MAX_PROJECT_PREVIEW_BYTES {
        return None;
    }
    let wire: HomeProjectPreviewWire =
        serde_json::from_reader(file.take(MAX_PROJECT_PREVIEW_BYTES + 1)).ok()?;
    let (Some(canvas_width), Some(canvas_height), Some(track_kinds)) =
        (wire.width, wire.height, wire.track_kinds)
    else {
        return None;
    };
    if canvas_width <= 0 || canvas_height <= 0 {
        return None;
    }
    Some(HomeProjectPreview {
        canvas_width,
        canvas_height,
        track_kinds,
    })
}

fn stored_modified_at(entry: &ProjectEntry) -> u64 {
    if entry.modified_at == 0 {
        entry.last_opened_at
    } else {
        entry.modified_at
    }
}

fn fail_closed_entries(entries: &[ProjectEntry]) -> Vec<HomeProjectEntry> {
    entries
        .iter()
        .map(|entry| home_entry(entry, stored_modified_at(entry), None, false, true))
        .collect()
}

fn probe_project_entry(
    entry: &ProjectEntry,
    authorize_thumbnail: &mut impl FnMut(&Path) -> bool,
) -> HomeProjectEntry {
    let bundle_metadata = match fs::symlink_metadata(&entry.path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return home_entry(entry, stored_modified_at(entry), None, true, false);
        }
        Err(_) => return home_entry(entry, stored_modified_at(entry), None, false, true),
    };
    if bundle_metadata.file_type().is_symlink() || !bundle_metadata.is_dir() {
        return home_entry(entry, stored_modified_at(entry), None, true, false);
    }
    if crate::fs_availability::is_dataless(&entry.path)
        || crate::fs_availability::project_bundle_has_dataless_components(&entry.path)
    {
        return home_entry(entry, stored_modified_at(entry), None, false, true);
    }

    let project_file = entry.path.join("project.json");
    let project_metadata = match fs::symlink_metadata(&project_file) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return home_entry(entry, stored_modified_at(entry), None, true, false);
        }
        Err(_) => return home_entry(entry, stored_modified_at(entry), None, false, true),
    };
    if project_metadata.file_type().is_symlink() || !project_metadata.is_file() {
        return home_entry(entry, stored_modified_at(entry), None, true, false);
    }

    let modified_at = modified_millis(&project_metadata)
        .or_else(|| modified_millis(&bundle_metadata))
        .unwrap_or_else(|| stored_modified_at(entry));
    let thumbnail = entry.path.join("thumbnail.jpg");
    let thumbnail_path =
        (valid_home_thumbnail(&thumbnail) && authorize_thumbnail(&thumbnail)).then_some(thumbnail);
    let mut result = home_entry(entry, modified_at, thumbnail_path, false, false);
    result.preview = read_project_preview(&entry.path);
    result
}

fn valid_home_thumbnail(path: &Path) -> bool {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return false;
    };
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > MAX_HOME_THUMBNAIL_BYTES
    {
        return false;
    }
    let Ok(file) = fs::File::open(path) else {
        return false;
    };
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    if file
        .take(MAX_HOME_THUMBNAIL_BYTES + 1)
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.len() as u64 != metadata.len()
    {
        return false;
    }
    let Ok(reader) = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()
    else {
        return false;
    };
    if reader.format() != Some(image::ImageFormat::Jpeg) {
        return false;
    }
    reader.into_dimensions().is_ok_and(|(width, height)| {
        width > 0
            && height > 0
            && width <= MAX_HOME_THUMBNAIL_DIMENSION
            && height <= MAX_HOME_THUMBNAIL_DIMENSION
    })
}

fn probe_project_entries_with(
    entries: Vec<ProjectEntry>,
    mut authorize_thumbnail: impl FnMut(&Path) -> bool,
) -> Vec<HomeProjectEntry> {
    entries
        .iter()
        .map(|entry| probe_project_entry(entry, &mut authorize_thumbnail))
        .collect::<Vec<_>>()
}

fn authorize_home_thumbnail(scope: &tauri::scope::fs::Scope, thumbnail: &Path) -> bool {
    let Ok(final_path) = crate::safe_asset_protocol::validate_resident_regular_file(thumbnail)
    else {
        return false;
    };
    if !same_path(thumbnail, &final_path) {
        return false;
    }
    scope.allow_file(thumbnail).is_ok() && scope.allow_file(final_path).is_ok()
}

async fn probe_project_entries_bounded<F>(
    coordinator: &HomeProbeCoordinator,
    entries: Vec<ProjectEntry>,
    timeout: Duration,
    probe: F,
) -> Vec<HomeProjectEntry>
where
    F: FnOnce(Vec<ProjectEntry>) -> Vec<HomeProjectEntry> + Send + 'static,
{
    let fail_closed = fail_closed_entries(&entries);
    if entries.is_empty() || coordinator.circuit_open.load(Ordering::Acquire) {
        return fail_closed;
    }
    let Ok(_singleflight) = coordinator.gate.try_lock() else {
        return fail_closed;
    };
    if coordinator.circuit_open.load(Ordering::Acquire) {
        return fail_closed;
    }

    let task = tauri::async_runtime::spawn_blocking(move || probe(entries));
    match tokio::time::timeout(timeout, task).await {
        Ok(Ok(snapshot)) => snapshot,
        Ok(Err(error)) => {
            coordinator.circuit_open.store(true, Ordering::Release);
            eprintln!("[home] recent-project probe crashed; validation disabled: {error}");
            fail_closed
        }
        Err(_) => {
            coordinator.circuit_open.store(true, Ordering::Release);
            eprintln!("[home] recent-project probe timed out; validation disabled");
            fail_closed
        }
    }
}

fn validated_project_path(path: &Path) -> Result<PathBuf, String> {
    let display = path.to_string_lossy();
    if display.is_empty()
        || display.len() > MAX_PROJECT_PATH_BYTES
        || display.as_bytes().contains(&0)
    {
        return Err("project path is empty or exceeds the supported length".into());
    }
    if !path.is_absolute() {
        return Err("project path must be absolute".into());
    }
    #[cfg(target_os = "windows")]
    {
        use std::path::Prefix;
        let local_drive = path.components().next().is_some_and(|component| {
            matches!(component, std::path::Component::Prefix(prefix) if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
        });
        if !local_drive {
            return Err(
                "Home paths must use a local drive; UNC, device and NT paths are not accepted"
                    .into(),
            );
        }
    }
    if path.components().any(|component| {
        matches!(
            component,
            std::path::Component::CurDir | std::path::Component::ParentDir
        )
    }) {
        return Err("project path must not contain relative traversal components".into());
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
    path_identity_key(left) == path_identity_key(right)
}

fn sort_entries(entries: &mut [ProjectEntry]) {
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.last_opened_at));
}

fn path_identity_key(path: &Path) -> String {
    let normalized = path
        .components()
        .fold(PathBuf::new(), |mut result, component| {
            if !matches!(component, std::path::Component::CurDir) {
                result.push(component.as_os_str());
            }
            result
        });
    let key = normalized.to_string_lossy().into_owned();
    #[cfg(target_os = "windows")]
    {
        key.strip_prefix(r"\\?\").unwrap_or(&key).to_lowercase()
    }
    #[cfg(not(target_os = "windows"))]
    {
        key
    }
}

fn normalize_entries(entries: &mut Vec<ProjectEntry>) {
    sort_entries(entries);
    let mut seen = HashSet::with_capacity(entries.len().min(MAX_RECENT_PROJECTS));
    entries.retain(|entry| seen.insert(path_identity_key(&entry.path)));
    entries.truncate(MAX_RECENT_PROJECTS);
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
            Err(first_error) => {
                let backup =
                    parent.join(format!(".project-registry.{}.backup", uuid::Uuid::new_v4()));
                match fs::rename(path, &backup) {
                    Ok(()) => match fs::rename(&temp, path) {
                        Ok(()) => {
                            let _ = fs::remove_file(backup);
                            Ok(())
                        }
                        Err(error) => {
                            let _ = fs::rename(&backup, path);
                            Err(format!("publish project registry: {error}"))
                        }
                    },
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        Err(format!("publish project registry: {first_error}"))
                    }
                    Err(error) => Err(format!("preserve project registry: {error}")),
                }
            }
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

fn capability_metadata_is_symlink_or_reparse(metadata: &cap_std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(target_os = "windows")]
    {
        use cap_std::fs::MetadataExt;
        windows_file_attributes_are_reparse(metadata.file_attributes())
    }
    #[cfg(not(target_os = "windows"))]
    false
}

#[cfg(target_os = "windows")]
fn windows_file_attributes_are_reparse(attributes: u32) -> bool {
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

fn directory_identity(directory: &Dir, path: &Path) -> Result<ProjectBundleIdentity, String> {
    let file = directory
        .try_clone()
        .map_err(|error| format!("retain directory identity for {}: {error}", path.display()))?
        .into_std_file();
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let metadata = file.metadata().map_err(|error| {
            format!("inspect directory identity for {}: {error}", path.display())
        })?;
        Ok(ProjectBundleIdentity {
            volume: metadata.dev(),
            file: metadata.ino(),
        })
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Foundation::HANDLE;
        use windows_sys::Win32::Storage::FileSystem::{
            GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
        };

        // `MetadataExt::volume_serial_number/file_index` are unstable
        // (rust-lang/rust#63010); use the stable handle query instead.
        let mut information = BY_HANDLE_FILE_INFORMATION::default();
        // SAFETY: `file` owns a live handle and `information` is writable.
        if unsafe {
            GetFileInformationByHandle(
                file.as_raw_handle() as HANDLE,
                std::ptr::addr_of_mut!(information),
            )
        } == 0
        {
            return Err(format!(
                "retain directory identity for {}: {}",
                path.display(),
                std::io::Error::last_os_error()
            ));
        }
        let file_index =
            (u64::from(information.nFileIndexHigh) << 32) | u64::from(information.nFileIndexLow);
        Ok(ProjectBundleIdentity {
            volume: u64::from(information.dwVolumeSerialNumber),
            file: file_index,
        })
    }
}

struct RetainedProjectParent {
    parent_path: PathBuf,
    parent: Dir,
    parent_identity: ProjectBundleIdentity,
    original_name: OsString,
    original_path: PathBuf,
}

impl RetainedProjectParent {
    fn open(path: &Path) -> Result<Self, String> {
        let original_path = validated_project_path(path)?;
        let parent_path = original_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or_else(|| "registered project has no parent directory".to_string())?
            .to_path_buf();
        let original_name = original_path
            .file_name()
            .ok_or_else(|| "registered project has no final component".to_string())?
            .to_owned();
        let parent = Dir::open_ambient_dir(&parent_path, ambient_authority())
            .map_err(|error| format!("open registered project parent: {error}"))?;
        let parent_identity = directory_identity(&parent, &parent_path)?;
        Ok(Self {
            parent_path,
            parent,
            parent_identity,
            original_name,
            original_path,
        })
    }

    fn identity_at(&self, name: &OsStr) -> Result<Option<ProjectBundleIdentity>, String> {
        let path = self.parent_path.join(name);
        let metadata = match self.parent.symlink_metadata(name) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(format!(
                    "inspect project bundle entry {}: {error}",
                    path.display()
                ));
            }
        };
        if capability_metadata_is_symlink_or_reparse(&metadata) {
            return Err(format!(
                "project bundle entry is a symlink or reparse point: {}",
                path.display()
            ));
        }
        if !metadata.is_dir() {
            return Err(format!(
                "project bundle entry is not a directory: {}",
                path.display()
            ));
        }
        let directory = self.parent.open_dir_nofollow(name).map_err(|error| {
            format!(
                "open project bundle entry without following links {}: {error}",
                path.display()
            )
        })?;
        let retained_metadata = directory.dir_metadata().map_err(|error| {
            format!(
                "inspect retained project bundle {}: {error}",
                path.display()
            )
        })?;
        if capability_metadata_is_symlink_or_reparse(&retained_metadata)
            || !retained_metadata.is_dir()
        {
            return Err(format!(
                "retained project bundle is not a no-follow directory: {}",
                path.display()
            ));
        }
        directory_identity(&directory, &path).map(Some)
    }

    fn original_identity(&self) -> Result<Option<ProjectBundleIdentity>, String> {
        self.identity_at(&self.original_name)
    }

    fn entry_exists(&self, name: &OsStr) -> Result<bool, String> {
        match self.parent.symlink_metadata(name) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(format!("inspect project namespace entry: {error}")),
        }
    }

    fn ambient_parent_matches(&self) -> Result<bool, String> {
        let ambient = Dir::open_ambient_dir(&self.parent_path, ambient_authority())
            .map_err(|error| format!("reopen registered project parent: {error}"))?;
        Ok(directory_identity(&ambient, &self.parent_path)? == self.parent_identity)
    }

    fn unused_quarantine_name(&self) -> Result<OsString, String> {
        for _ in 0..8 {
            let candidate = OsString::from(format!(
                ".opentake-trash-{}",
                uuid::Uuid::new_v4().as_simple()
            ));
            match self.parent.symlink_metadata(&candidate) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Ok(candidate);
                }
                Ok(_) => continue,
                Err(error) => {
                    return Err(format!("inspect project quarantine namespace: {error}"));
                }
            }
        }
        Err("could not allocate a unique project quarantine name".into())
    }

    fn restore_quarantine(
        &self,
        quarantine_name: &OsStr,
        quarantined_identity: ProjectBundleIdentity,
    ) -> Result<(), String> {
        if self.original_identity()?.is_some() {
            return Err(format!(
                "the original project path is occupied; quarantined project was preserved at {}",
                self.parent_path.join(quarantine_name).display()
            ));
        }
        if self.identity_at(quarantine_name)? != Some(quarantined_identity) {
            return Err(format!(
                "the quarantined project identity changed; preserved at {}",
                self.parent_path.join(quarantine_name).display()
            ));
        }
        self.parent
            .rename(quarantine_name, &self.parent, &self.original_name)
            .map_err(|error| format!("restore project after failed trash operation: {error}"))?;
        if self.original_identity()? != Some(quarantined_identity) {
            return Err(format!(
                "restored project identity could not be verified at {}",
                self.original_path.display()
            ));
        }
        Ok(())
    }

    fn restore_quarantine_entry(&self, quarantine_name: &OsStr) -> Result<(), String> {
        if self.entry_exists(&self.original_name)? {
            return Err(format!(
                "the original project path is occupied; quarantined entry was preserved at {}",
                self.parent_path.join(quarantine_name).display()
            ));
        }
        if !self.entry_exists(quarantine_name)? {
            return Err("the quarantined project entry is no longer present".into());
        }
        self.parent
            .rename(quarantine_name, &self.parent, &self.original_name)
            .map_err(|error| format!("restore quarantined project entry: {error}"))
    }
}

fn capture_registered_bundle_identity(
    registered: &Path,
) -> Result<Option<ProjectBundleIdentity>, String> {
    RetainedProjectParent::open(registered)?.original_identity()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrashOutcome {
    Trashed,
    Missing,
}

fn move_registered_project_to_trash(
    registered: &ProjectEntry,
    move_to_trash: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<TrashOutcome, String> {
    move_registered_project_to_trash_with_hooks(registered, || {}, || {}, move_to_trash)
}

fn move_registered_project_to_trash_with_hooks(
    registered: &ProjectEntry,
    after_registry_lookup: impl FnOnce(),
    before_quarantine_rename: impl FnOnce(),
    move_to_trash: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<TrashOutcome, String> {
    after_registry_lookup();
    let target = RetainedProjectParent::open(&registered.path)?;
    let Some(current_identity) = target.original_identity()? else {
        return Ok(TrashOutcome::Missing);
    };
    let expected_identity = registered.bundle_identity.ok_or_else(|| {
        "Home has no stable identity for this project; reopen it before moving it to Trash"
            .to_string()
    })?;
    if current_identity != expected_identity {
        return Err("registered project identity changed before trash operation".into());
    }

    if target.original_identity()? != Some(expected_identity) {
        return Err("registered project identity changed before quarantine".into());
    }
    let quarantine_name = target.unused_quarantine_name()?;
    before_quarantine_rename();
    target
        .parent
        .rename(&target.original_name, &target.parent, &quarantine_name)
        .map_err(|error| format!("quarantine registered project before trash: {error}"))?;

    let quarantined_identity = match target.identity_at(&quarantine_name) {
        Ok(Some(identity)) => identity,
        Ok(None) => {
            return Err("quarantined project disappeared before identity verification".into());
        }
        Err(error) => {
            let restore = target.restore_quarantine_entry(&quarantine_name);
            return Err(match restore {
                Ok(()) => format!(
                    "quarantined project was not a no-follow directory and was restored: {error}"
                ),
                Err(restore_error) => format!(
                    "quarantined project was not a no-follow directory and was preserved: {error}; {restore_error}"
                ),
            });
        }
    };
    if quarantined_identity != expected_identity {
        let restore = target.restore_quarantine(&quarantine_name, quarantined_identity);
        return Err(match restore {
            Ok(()) => "project identity changed during quarantine; replacement was restored".into(),
            Err(restore_error) => format!(
                "project identity changed during quarantine; replacement was preserved: {restore_error}"
            ),
        });
    }
    if !target.ambient_parent_matches()? {
        let restore = target.restore_quarantine(&quarantine_name, expected_identity);
        return Err(match restore {
            Ok(()) => "project parent identity changed before trash; project was restored".into(),
            Err(restore_error) => format!(
                "project parent identity changed before trash; quarantined project was preserved: {restore_error}"
            ),
        });
    }

    let quarantine_path = target.parent_path.join(&quarantine_name);
    if let Err(error) = move_to_trash(&quarantine_path) {
        let restore = target.restore_quarantine(&quarantine_name, expected_identity);
        return Err(match restore {
            Ok(()) => format!("{error}; project was restored"),
            Err(restore_error) => {
                format!("{error}; quarantined project could not be restored: {restore_error}")
            }
        });
    }

    match target.identity_at(&quarantine_name) {
        Ok(None) => Ok(TrashOutcome::Trashed),
        Ok(Some(identity)) if identity == expected_identity => {
            let restore = target.restore_quarantine(&quarantine_name, expected_identity);
            Err(match restore {
                Ok(()) => "system trash reported success without moving the project; project was restored"
                    .into(),
                Err(restore_error) => format!(
                    "system trash reported success without moving the project; quarantined project was preserved: {restore_error}"
                ),
            })
        }
        Ok(Some(_)) => Ok(TrashOutcome::Trashed),
        Err(_) => Ok(TrashOutcome::Trashed),
    }
}

#[tauri::command]
pub async fn home_projects_sync(
    app: AppHandle,
    entries: Vec<LegacyRecentProject>,
) -> Result<Vec<HomeProjectEntry>, String> {
    let activity = crate::updater::begin_mutating_activity(
        &app.state::<crate::updater::InstallAdmissionGate>(),
    )?;
    let scope = app.asset_protocol_scope();
    let registry_scope = scope.clone();
    let registry_entries = tauri::async_runtime::spawn_blocking(move || {
        let _activity = activity;
        let authorized_legacy = entries
            .into_iter()
            .filter(|entry| {
                let path = Path::new(&entry.path);
                validated_project_path(path).is_ok()
                    && crate::safe_asset_protocol::scope_allows_lexical_path(&registry_scope, path)
            })
            .collect::<Vec<_>>();
        with_registry(&app, |registry| {
            registry.retain_authorized(|path| {
                crate::safe_asset_protocol::scope_allows_lexical_path(&registry_scope, path)
            })?;
            registry.merge_legacy(&authorized_legacy)?;
            Ok(registry.entries_snapshot())
        })
    })
    .await
    .map_err(|error| format!("Home project registry task failed: {error}"))??;

    Ok(probe_project_entries_bounded(
        home_probe_coordinator(),
        registry_entries,
        HOME_PROBE_TIMEOUT,
        move |entries| {
            probe_project_entries_with(entries, |thumbnail| {
                authorize_home_thumbnail(&scope, thumbnail)
            })
        },
    )
    .await)
}

#[tauri::command]
pub async fn home_project_register(
    app: AppHandle,
    path: String,
    opened_at: Option<u64>,
) -> Result<(), String> {
    let activity = crate::updater::begin_mutating_activity(
        &app.state::<crate::updater::InstallAdmissionGate>(),
    )?;
    tauri::async_runtime::spawn_blocking(move || {
        let _activity = activity;
        let path = validated_project_path(Path::new(&path))?;
        let scope = app.asset_protocol_scope();
        if !crate::safe_asset_protocol::scope_allows_lexical_path(&scope, &path) {
            return Err("project path has not been approved by a native file dialog".into());
        }
        let bundle_identity = capture_registered_bundle_identity(&path)?;
        with_registry(&app, |registry| {
            registry.register_at(path, opened_at.unwrap_or_else(now_millis), bundle_identity)
        })
    })
    .await
    .map_err(|error| format!("Home project registration task failed: {error}"))?
}

#[tauri::command]
pub async fn home_project_remove(app: AppHandle, path: String) -> Result<(), String> {
    let activity = crate::updater::begin_mutating_activity(
        &app.state::<crate::updater::InstallAdmissionGate>(),
    )?;
    tauri::async_runtime::spawn_blocking(move || {
        let _activity = activity;
        with_registry(&app, |registry| {
            registry.remove(Path::new(&path)).map(|_| ())
        })
    })
    .await
    .map_err(|error| format!("Home project removal task failed: {error}"))?
}

#[tauri::command]
pub async fn home_project_trash(app: AppHandle, path: String) -> Result<(), String> {
    let activity = crate::updater::begin_mutating_activity(
        &app.state::<crate::updater::InstallAdmissionGate>(),
    )?;
    tauri::async_runtime::spawn_blocking(move || {
        let _activity = activity;
        let path = validated_project_path(Path::new(&path))?;
        if !crate::safe_asset_protocol::scope_allows_lexical_path(
            &app.asset_protocol_scope(),
            &path,
        ) {
            return Err("project path has not been approved by a native file dialog".into());
        }
        let registered = with_registry(&app, |registry| registry.registered_entry(&path))?;
        move_registered_project_to_trash(&registered, move_project_to_trash)?;
        with_registry(&app, |registry| {
            registry.remove_registered_entry(&registered).map(|_| ())
        })
    })
    .await
    .map_err(|error| format!("project trash task failed: {error}"))?
}

#[tauri::command]
pub async fn home_project_reveal(app: AppHandle, path: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = validated_project_path(Path::new(&path))?;
        if !crate::safe_asset_protocol::scope_allows_lexical_path(
            &app.asset_protocol_scope(),
            &path,
        ) {
            return Err("project path has not been approved by a native file dialog".into());
        }
        let registered = with_registry(&app, |registry| registry.registered_path(&path))?;
        reveal_in_file_manager(&registered)
    })
    .await
    .map_err(|error| format!("project reveal task failed: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_test_jpeg(path: &Path, color: [u8; 3]) {
        let pixels = color.repeat(16 * 9);
        let mut bytes = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 80)
            .encode(&pixels, 16, 9, image::ExtendedColorType::Rgb8)
            .unwrap();
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn missing_entry_survives_registry_load_and_safe_trash_removes_only_after_success() {
        let directory = tempfile::tempdir().unwrap();
        let ledger = directory.path().join("project-registry.json");
        let missing = directory.path().join("Missing.opentake");
        let existing = directory.path().join("Existing.opentake");
        fs::create_dir(&existing).unwrap();

        let mut registry = ProjectRegistry::load(ledger.clone()).unwrap();
        registry.register_at(missing.clone(), 10, None).unwrap();
        let existing_identity = capture_registered_bundle_identity(&existing).unwrap();
        registry
            .register_at(existing.clone(), 20, existing_identity)
            .unwrap();

        let mut reloaded = ProjectRegistry::load(ledger).unwrap();
        assert!(reloaded.entries().iter().any(|entry| entry.path == missing));

        let registered = reloaded.registered_entry(&existing).unwrap();
        let denied =
            move_registered_project_to_trash(&registered, |_| Err("permission denied".into()));
        assert!(denied.is_err());
        assert!(existing.exists());
        assert!(reloaded
            .entries()
            .iter()
            .any(|entry| entry.path == existing));

        move_registered_project_to_trash(&registered, |path| {
            fs::remove_dir_all(path).map_err(|error| error.to_string())
        })
        .unwrap();
        reloaded.remove_registered_entry(&registered).unwrap();
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
        let registry = ProjectRegistry::load(ledger).unwrap();
        let mut called = false;

        let registered = registry.registered_entry(&outside);
        if let Ok(entry) = registered {
            move_registered_project_to_trash(&entry, |_| {
                called = true;
                Ok(())
            })
            .unwrap();
        }
        assert!(!called);
        assert!(outside.exists());
    }

    #[test]
    fn legacy_entry_without_identity_cannot_trash_an_existing_bundle() {
        let directory = tempfile::tempdir().unwrap();
        let ledger = directory.path().join("project-registry.json");
        let project = directory.path().join("Legacy.opentake");
        fs::create_dir(&project).unwrap();
        fs::write(project.join("marker"), b"keep").unwrap();
        let mut registry = ProjectRegistry::load(ledger).unwrap();
        registry.register_at(project.clone(), 10, None).unwrap();
        let registered = registry.registered_entry(&project).unwrap();
        let mut called = false;

        let result = move_registered_project_to_trash(&registered, |_| {
            called = true;
            Ok(())
        });

        assert!(result.is_err());
        assert!(!called);
        assert_eq!(fs::read(project.join("marker")).unwrap(), b"keep");
    }

    #[test]
    fn lookup_to_trash_recreation_is_rejected_and_both_bundles_survive() {
        let directory = tempfile::tempdir().unwrap();
        let ledger = directory.path().join("project-registry.json");
        let project = directory.path().join("Race.opentake");
        let original = directory.path().join("Original-held.opentake");
        fs::create_dir(&project).unwrap();
        fs::write(project.join("marker"), b"original").unwrap();
        let mut registry = ProjectRegistry::load(ledger).unwrap();
        registry
            .register_at(
                project.clone(),
                10,
                capture_registered_bundle_identity(&project).unwrap(),
            )
            .unwrap();
        let registered = registry.registered_entry(&project).unwrap();
        let mut called = false;

        let result = move_registered_project_to_trash_with_hooks(
            &registered,
            || {
                fs::rename(&project, &original).unwrap();
                fs::create_dir(&project).unwrap();
                fs::write(project.join("marker"), b"replacement").unwrap();
            },
            || {},
            |_| {
                called = true;
                Ok(())
            },
        );

        assert!(result.is_err());
        assert!(!called);
        assert_eq!(fs::read(original.join("marker")).unwrap(), b"original");
        assert_eq!(fs::read(project.join("marker")).unwrap(), b"replacement");
    }

    #[test]
    fn replacement_racing_the_quarantine_rename_is_restored_not_trashed() {
        let directory = tempfile::tempdir().unwrap();
        let ledger = directory.path().join("project-registry.json");
        let project = directory.path().join("Rename-race.opentake");
        let original = directory.path().join("Rename-race-held.opentake");
        fs::create_dir(&project).unwrap();
        fs::write(project.join("marker"), b"original").unwrap();
        let mut registry = ProjectRegistry::load(ledger).unwrap();
        registry
            .register_at(
                project.clone(),
                10,
                capture_registered_bundle_identity(&project).unwrap(),
            )
            .unwrap();
        let registered = registry.registered_entry(&project).unwrap();
        let mut called = false;

        let result = move_registered_project_to_trash_with_hooks(
            &registered,
            || {},
            || {
                fs::rename(&project, &original).unwrap();
                fs::create_dir(&project).unwrap();
                fs::write(project.join("marker"), b"replacement").unwrap();
            },
            |_| {
                called = true;
                Ok(())
            },
        );

        assert!(result.is_err());
        assert!(!called);
        assert_eq!(fs::read(original.join("marker")).unwrap(), b"original");
        assert_eq!(fs::read(project.join("marker")).unwrap(), b"replacement");
        assert!(!fs::read_dir(directory.path()).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".opentake-trash-")
        }));
    }

    #[cfg(unix)]
    #[test]
    fn symlink_racing_the_quarantine_rename_is_rejected_and_restored() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let ledger = directory.path().join("project-registry.json");
        let project = directory.path().join("Symlink-race.opentake");
        let original = directory.path().join("Symlink-race-held.opentake");
        fs::create_dir(&project).unwrap();
        fs::write(project.join("marker"), b"original").unwrap();
        let mut registry = ProjectRegistry::load(ledger).unwrap();
        registry
            .register_at(
                project.clone(),
                10,
                capture_registered_bundle_identity(&project).unwrap(),
            )
            .unwrap();
        let registered = registry.registered_entry(&project).unwrap();
        let mut called = false;

        let result = move_registered_project_to_trash_with_hooks(
            &registered,
            || {},
            || {
                fs::rename(&project, &original).unwrap();
                symlink(&original, &project).unwrap();
            },
            |_| {
                called = true;
                Ok(())
            },
        );

        assert!(result.is_err());
        assert!(!called);
        assert!(fs::symlink_metadata(&project)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::read(original.join("marker")).unwrap(), b"original");
    }

    #[test]
    fn registry_cleanup_does_not_remove_a_rebound_project_registration() {
        let directory = tempfile::tempdir().unwrap();
        let ledger = directory.path().join("project-registry.json");
        let project = directory.path().join("Rebound.opentake");
        fs::create_dir(&project).unwrap();
        let mut registry = ProjectRegistry::load(ledger).unwrap();
        registry
            .register_at(
                project.clone(),
                10,
                capture_registered_bundle_identity(&project).unwrap(),
            )
            .unwrap();
        let original_registration = registry.registered_entry(&project).unwrap();
        fs::rename(&project, directory.path().join("Rebound-held.opentake")).unwrap();
        fs::create_dir(&project).unwrap();
        registry
            .register_at(
                project.clone(),
                20,
                capture_registered_bundle_identity(&project).unwrap(),
            )
            .unwrap();

        assert!(registry
            .remove_registered_entry(&original_registration)
            .is_err());
        assert!(registry.registered_entry(&project).is_ok());
        assert!(project.exists());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_reparse_attribute_contract_is_fail_closed() {
        assert!(windows_file_attributes_are_reparse(0x400));
        assert!(!windows_file_attributes_are_reparse(0));
    }

    #[test]
    fn filesystem_probe_approves_only_the_expected_resident_thumbnail() {
        let directory = tempfile::tempdir().unwrap();
        let ledger = directory.path().join("project-registry.json");
        let project = directory.path().join("Metadata.opentake");
        fs::create_dir(&project).unwrap();
        fs::write(project.join("project.json"), b"{}").unwrap();
        write_test_jpeg(&project.join("thumbnail.jpg"), [20, 40, 80]);

        let mut registry = ProjectRegistry::load(ledger).unwrap();
        registry
            .register_at(
                project.clone(),
                10,
                capture_registered_bundle_identity(&project).unwrap(),
            )
            .unwrap();
        let entry = probe_project_entries_with(registry.entries_snapshot(), |thumbnail| {
            crate::fs_availability::is_materialized_regular_file(thumbnail)
        })
        .pop()
        .unwrap();

        assert!(entry.modified_at > 0);
        assert_eq!(entry.thumbnail_path, Some(project.join("thumbnail.jpg")));
        assert!(serde_json::to_value(&entry)
            .unwrap()
            .get("preview")
            .is_none());
        assert!(!entry.missing);
        assert!(!entry.offline);
    }

    #[test]
    fn thumbnail_invalid_prior_jpeg_is_retained_but_not_advertised() {
        let directory = tempfile::tempdir().unwrap();
        let ledger = directory.path().join("project-registry.json");
        let project = directory.path().join("InvalidCover.opentake");
        fs::create_dir(&project).unwrap();
        fs::write(project.join("project.json"), b"{}").unwrap();
        fs::write(project.join("thumbnail.jpg"), b"not-a-jpeg").unwrap();

        let mut registry = ProjectRegistry::load(ledger).unwrap();
        registry
            .register_at(
                project.clone(),
                10,
                capture_registered_bundle_identity(&project).unwrap(),
            )
            .unwrap();
        let entry = probe_project_entries_with(registry.entries_snapshot(), |_| true)
            .pop()
            .unwrap();

        assert_eq!(entry.thumbnail_path, None);
        assert_eq!(
            fs::read(project.join("thumbnail.jpg")).unwrap(),
            b"not-a-jpeg"
        );
    }

    #[cfg(unix)]
    #[test]
    fn thumbnail_atomic_replacement_failure_retains_previous_valid_jpeg() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let bundle = directory.path().join("AtomicCover.opentake");
        let prior_path = directory.path().join("prior.jpg");
        let replacement_path = directory.path().join("replacement.jpg");
        write_test_jpeg(&prior_path, [20, 40, 80]);
        write_test_jpeg(&replacement_path, [200, 10, 10]);
        let prior = fs::read(&prior_path).unwrap();
        let replacement = fs::read(&replacement_path).unwrap();
        let mut project = opentake_project::Project::new(&bundle);
        project.thumbnail = Some(prior.clone());
        project.save().unwrap();
        let original_mode = fs::metadata(&bundle).unwrap().permissions().mode();
        fs::set_permissions(&bundle, fs::Permissions::from_mode(0o555)).unwrap();

        project.thumbnail = Some(replacement);
        let result = project.save();
        fs::set_permissions(&bundle, fs::Permissions::from_mode(original_mode)).unwrap();

        assert!(result.is_err(), "read-only atomic replacement must fail");
        assert_eq!(fs::read(bundle.join("thumbnail.jpg")).unwrap(), prior);
    }

    #[test]
    fn filesystem_probe_reports_explicit_canvas_and_actual_track_kinds() {
        let directory = tempfile::tempdir().unwrap();
        let ledger = directory.path().join("project-registry.json");
        let project = directory.path().join("Portrait.opentake");
        fs::create_dir(&project).unwrap();
        fs::write(
            project.join("project.json"),
            br#"{
                "width": 1080,
                "height": 1920,
                "tracks": [
                    { "type": "video", "clips": [] },
                    { "type": "audio", "clips": [] }
                ]
            }"#,
        )
        .unwrap();

        let mut registry = ProjectRegistry::load(ledger).unwrap();
        registry.register_at(project, 10, None).unwrap();
        let entry = probe_project_entries_with(registry.entries_snapshot(), |_| false)
            .pop()
            .unwrap();
        let json = serde_json::to_value(entry).unwrap();

        assert_eq!(json["preview"]["canvasWidth"], 1080);
        assert_eq!(json["preview"]["canvasHeight"], 1920);
        assert_eq!(
            json["preview"]["trackKinds"],
            serde_json::json!(["video", "audio"])
        );
    }

    #[test]
    fn filesystem_probe_omits_preview_when_tracks_are_not_explicit() {
        let directory = tempfile::tempdir().unwrap();
        let ledger = directory.path().join("project-registry.json");
        let project = directory.path().join("Incomplete.opentake");
        fs::create_dir(&project).unwrap();
        fs::write(
            project.join("project.json"),
            br#"{ "width": 1080, "height": 1920 }"#,
        )
        .unwrap();

        let mut registry = ProjectRegistry::load(ledger).unwrap();
        registry.register_at(project, 10, None).unwrap();
        let entry = probe_project_entries_with(registry.entries_snapshot(), |_| false)
            .pop()
            .unwrap();

        assert!(serde_json::to_value(entry)
            .unwrap()
            .get("preview")
            .is_none());

        fs::write(
            directory.path().join("Incomplete.opentake/project.json"),
            br#"{ "width": 1080, "height": 1920, "tracks": [] }"#,
        )
        .unwrap();
        let entry = probe_project_entries_with(registry.entries_snapshot(), |_| false)
            .pop()
            .unwrap();
        assert_eq!(
            serde_json::to_value(entry).unwrap()["preview"]["trackKinds"],
            serde_json::json!([])
        );
    }

    #[test]
    fn home_thumbnail_scope_grant_is_exact_and_excludes_project_data() {
        let directory = tempfile::Builder::new()
            .prefix("home-scope-")
            .tempdir_in(std::env::current_dir().unwrap())
            .unwrap();
        let project = directory.path().join("Scoped.opentake");
        let thumbnail = project.join("thumbnail.jpg");
        let project_data = project.join("project.json");
        fs::create_dir(&project).unwrap();
        fs::write(&thumbnail, b"jpeg").unwrap();
        fs::write(&project_data, b"{}").unwrap();
        let app = tauri::test::mock_app();
        let scope = app.handle().asset_protocol_scope();

        assert!(authorize_home_thumbnail(&scope, &thumbnail));
        assert!(crate::safe_asset_protocol::scope_allows_lexical_path(
            &scope, &thumbnail
        ));
        assert!(!crate::safe_asset_protocol::scope_allows_lexical_path(
            &scope,
            &project_data
        ));
    }

    #[test]
    fn registry_load_keeps_only_the_newest_twelve_valid_entries() {
        let directory = tempfile::tempdir().unwrap();
        let ledger = directory.path().join("project-registry.json");
        let entries = (0..40)
            .map(|index| ProjectEntry {
                id: format!("id-{index}"),
                path: directory.path().join(format!("Project-{index}.opentake")),
                created_at: index,
                last_opened_at: index,
                modified_at: index,
                thumbnail_path: None,
                bundle_identity: None,
            })
            .collect::<Vec<_>>();
        fs::write(&ledger, serde_json::to_vec(&entries).unwrap()).unwrap();

        let registry = ProjectRegistry::load(ledger).unwrap();

        assert_eq!(registry.entries().len(), MAX_RECENT_PROJECTS);
        assert_eq!(registry.entries()[0].last_opened_at, 39);
        assert_eq!(registry.entries()[11].last_opened_at, 28);
    }

    #[test]
    fn registry_load_deduplicates_before_applying_the_recent_limit() {
        let directory = tempfile::tempdir().unwrap();
        let ledger = directory.path().join("project-registry.json");
        let duplicate = directory.path().join("Duplicate.opentake");
        let mut entries = (0..20)
            .map(|index| ProjectEntry {
                id: format!("duplicate-{index}"),
                path: duplicate.clone(),
                created_at: index,
                last_opened_at: 1_000 + index,
                modified_at: index,
                thumbnail_path: None,
                bundle_identity: None,
            })
            .collect::<Vec<_>>();
        entries.extend((0..12).map(|index| ProjectEntry {
            id: format!("unique-{index}"),
            path: directory.path().join(format!("Unique-{index}.opentake")),
            created_at: index,
            last_opened_at: index,
            modified_at: index,
            thumbnail_path: None,
            bundle_identity: None,
        }));
        fs::write(&ledger, serde_json::to_vec(&entries).unwrap()).unwrap();

        let registry = ProjectRegistry::load(ledger).unwrap();

        assert_eq!(registry.entries().len(), MAX_RECENT_PROJECTS);
        assert_eq!(
            registry
                .entries()
                .iter()
                .filter(|entry| entry.path == duplicate)
                .count(),
            1
        );
        assert_eq!(
            registry
                .entries()
                .iter()
                .map(|entry| &entry.path)
                .collect::<HashSet<_>>()
                .len(),
            MAX_RECENT_PROJECTS
        );
    }

    #[test]
    fn legacy_merge_caps_work_before_filesystem_checks() {
        let directory = tempfile::tempdir().unwrap();
        let ledger = directory.path().join("project-registry.json");
        let legacy = (0..100)
            .map(|index| LegacyRecentProject {
                path: directory
                    .path()
                    .join(format!("Legacy-{index}.opentake"))
                    .to_string_lossy()
                    .into_owned(),
                opened_at: index,
                created_at: None,
                modified_at: None,
                thumbnail_path: None,
            })
            .collect::<Vec<_>>();
        let mut registry = ProjectRegistry::load(ledger).unwrap();

        registry.merge_legacy(&legacy).unwrap();

        assert_eq!(registry.entries().len(), MAX_RECENT_PROJECTS);
    }

    #[cfg(unix)]
    #[test]
    fn path_identity_is_lexical_and_does_not_follow_symlinks() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("Target.opentake");
        let alias = directory.path().join("Alias.opentake");
        fs::create_dir(&target).unwrap();
        symlink(&target, &alias).unwrap();

        assert_ne!(path_identity_key(&target), path_identity_key(&alias));
        assert!(!same_path(&target, &alias));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn hung_probe_is_singleflight_times_out_and_opens_the_circuit() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering as AtomicOrdering},
            mpsc, Arc,
        };

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("Hung.opentake");
        let entries = vec![ProjectEntry {
            id: "hung".into(),
            path: path.clone(),
            created_at: 1,
            last_opened_at: 2,
            modified_at: 3,
            thumbnail_path: Some(path.join("thumbnail.jpg")),
            bundle_identity: None,
        }];
        let coordinator = Arc::new(HomeProbeCoordinator::new());
        let probes = Arc::new(AtomicUsize::new(0));
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let first_coordinator = Arc::clone(&coordinator);
        let first_entries = entries.clone();
        let first_probes = Arc::clone(&probes);
        let first = tokio::spawn(async move {
            probe_project_entries_bounded(
                &first_coordinator,
                first_entries,
                Duration::from_millis(50),
                move |entries| {
                    first_probes.fetch_add(1, AtomicOrdering::SeqCst);
                    let _ = started_tx.send(());
                    let _ = release_rx.recv();
                    fail_closed_entries(&entries)
                },
            )
            .await
        });
        started_rx.await.unwrap();

        let concurrent_probes = Arc::clone(&probes);
        let concurrent = probe_project_entries_bounded(
            &coordinator,
            entries.clone(),
            Duration::from_millis(50),
            move |entries| {
                concurrent_probes.fetch_add(1, AtomicOrdering::SeqCst);
                fail_closed_entries(&entries)
            },
        )
        .await;
        assert!(concurrent[0].offline);
        assert_eq!(probes.load(AtomicOrdering::SeqCst), 1);

        let timed_out = first.await.unwrap();
        assert!(timed_out[0].offline);
        assert!(REGISTRY_LOCK.try_lock().is_ok());

        let after_timeout_probes = Arc::clone(&probes);
        let after_timeout = probe_project_entries_bounded(
            &coordinator,
            entries,
            Duration::from_millis(50),
            move |entries| {
                after_timeout_probes.fetch_add(1, AtomicOrdering::SeqCst);
                fail_closed_entries(&entries)
            },
        )
        .await;
        assert!(after_timeout[0].offline);
        assert_eq!(probes.load(AtomicOrdering::SeqCst), 1);
        release_tx.send(()).unwrap();
    }

    #[test]
    fn oversized_registry_and_traversal_paths_are_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let ledger = directory.path().join("project-registry.json");
        let file = fs::File::create(&ledger).unwrap();
        file.set_len(MAX_REGISTRY_BYTES + 1).unwrap();
        assert!(ProjectRegistry::load(ledger).is_err());

        assert!(validated_project_path(Path::new("/tmp/A/../B.opentake")).is_err());
        let oversized = Path::new("/tmp").join(format!("{}.opentake", "x".repeat(32_769)));
        assert!(validated_project_path(&oversized).is_err());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_home_paths_accept_disk_forms_but_reject_unc_and_devices() {
        assert!(validated_project_path(Path::new(r"C:\Projects\Local.opentake")).is_ok());
        assert!(validated_project_path(Path::new(r"\\?\C:\Projects\Local.opentake")).is_ok());
        assert!(validated_project_path(Path::new(r"\\server\share\Remote.opentake")).is_err());
        assert!(
            validated_project_path(Path::new(r"\\?\UNC\server\share\Remote.opentake")).is_err()
        );
        assert!(validated_project_path(Path::new(r"\\.\Device\Unsafe.opentake")).is_err());
    }
}
