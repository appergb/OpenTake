//! Remote sample-project materialization.
//!
//! A sample is assembled below the application cache, validated as a complete
//! `.opentake` bundle, and only then renamed into its stable cache location.
//! Any fetch, decode, write, or validation failure removes the entire staging
//! directory and leaves a previously cached sample untouched.

use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use opentake_domain::{Clip, ClipType, MediaManifest, TextStyle, Timeline, Track};
use opentake_project::{layout, Project};
use reqwest::blocking::{Client, Response};
use reqwest::redirect::Policy;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};

const MAX_RESOLVE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_DOWNLOAD_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SampleDownload {
    id: String,
    relative_path: String,
    url: String,
}

#[derive(Debug, Clone, Deserialize)]
struct SampleChatDownload {
    name: String,
    url: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResolvedSample {
    title: String,
    project: Value,
    manifest: Value,
    #[serde(default)]
    generation_log: Option<Value>,
    #[serde(default)]
    poster_url: Option<String>,
    #[serde(default)]
    downloads: Vec<SampleDownload>,
    #[serde(default)]
    chat: Vec<SampleChatDownload>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SampleProgress {
    slug: String,
    completed: usize,
    total: usize,
}

struct StagingDirectory {
    path: PathBuf,
    armed: bool,
}

impl Drop for StagingDirectory {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

struct SampleProjectService {
    cache_root: PathBuf,
    client: Client,
}

impl SampleProjectService {
    fn new(cache_root: PathBuf) -> Result<Self, String> {
        Ok(Self {
            cache_root,
            client: Client::builder()
                .redirect(Policy::limited(5))
                .timeout(REQUEST_TIMEOUT)
                .build()
                .map_err(|error| format!("configure sample HTTP client: {error}"))?,
        })
    }

    fn materialize(
        &self,
        backend_url: &str,
        slug: &str,
        on_progress: impl FnMut(f64),
    ) -> Result<PathBuf, String> {
        validate_slug(slug)?;
        let mut endpoint = validated_network_url(backend_url)?;
        endpoint.set_path("/v1/samples/resolve");
        endpoint.set_query(None);
        endpoint.query_pairs_mut().append_pair("slug", slug);
        let response = self
            .client
            .get(endpoint)
            .send()
            .map_err(|error| format!("resolve sample {slug}: {error}"))?;
        let bytes = read_bounded_response(response, MAX_RESOLVE_BYTES, "sample metadata")?;
        let resolved: ResolvedSample = serde_json::from_slice(&bytes)
            .map_err(|error| format!("decode sample {slug}: {error}"))?;
        self.materialize_resolved(
            slug,
            resolved,
            |download, target| self.download_file(download, target),
            on_progress,
        )
    }

    fn materialize_builtin(
        &self,
        slug: &str,
        on_progress: impl FnMut(f64),
    ) -> Result<PathBuf, String> {
        self.materialize_resolved(
            slug,
            builtin_sample(slug)?,
            |_, _| Err("built-in sample unexpectedly requested a download".into()),
            on_progress,
        )
    }

    fn materialize_resolved(
        &self,
        slug: &str,
        resolved: ResolvedSample,
        mut download_file: impl FnMut(&SampleDownload, &Path) -> Result<(), String>,
        mut on_progress: impl FnMut(f64),
    ) -> Result<PathBuf, String> {
        validate_slug(slug)?;
        fs::create_dir_all(&self.cache_root)
            .map_err(|error| format!("create sample cache: {error}"))?;
        let stage_root = self
            .cache_root
            .join(format!(".{slug}.{}.tmp", uuid::Uuid::new_v4()));
        fs::create_dir(&stage_root).map_err(|error| format!("create sample stage: {error}"))?;
        let mut stage = StagingDirectory {
            path: stage_root,
            armed: true,
        };
        let bundle = stage
            .path
            .join(format!("{}.opentake", safe_name(&resolved.title)));
        fs::create_dir_all(bundle.join(layout::MEDIA_DIR))
            .map_err(|error| format!("create sample bundle: {error}"))?;
        write_json(&bundle.join(layout::TIMELINE_FILE), &resolved.project)?;
        write_json(&bundle.join(layout::MANIFEST_FILE), &resolved.manifest)?;
        if let Some(log) = &resolved.generation_log {
            write_json(&bundle.join(layout::GENERATION_LOG_FILE), log)?;
        }

        let mut downloads = resolved.downloads;
        downloads.extend(resolved.chat.into_iter().map(|chat| SampleDownload {
            id: chat.name.clone(),
            relative_path: format!("chat-sessions/{}", chat.name),
            url: chat.url,
        }));
        if let Some(url) = resolved.poster_url {
            downloads.push(SampleDownload {
                id: "poster".into(),
                relative_path: layout::THUMBNAIL_FILE.into(),
                url,
            });
        }
        validate_downloads(&downloads)?;
        let total = downloads.len().max(1);
        on_progress(0.0);
        if downloads.is_empty() {
            on_progress(1.0);
        }
        for (index, download) in downloads.iter().enumerate() {
            let target = safe_target(&bundle, &download.relative_path)?;
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("create sample download directory: {error}"))?;
            }
            download_file(download, &target)?;
            on_progress((index + 1) as f64 / total as f64);
        }

        Project::open(&bundle).map_err(|error| format!("validate sample bundle: {error}"))?;
        let stable_root = self.cache_root.join(slug);
        let backup = self.cache_root.join(format!(".{slug}.backup"));
        let _ = fs::remove_dir_all(&backup);
        if stable_root.exists() {
            fs::rename(&stable_root, &backup)
                .map_err(|error| format!("preserve cached sample: {error}"))?;
        }
        if let Err(error) = fs::rename(&stage.path, &stable_root) {
            if backup.exists() {
                let _ = fs::rename(&backup, &stable_root);
            }
            return Err(format!("publish sample: {error}"));
        }
        stage.armed = false;
        let _ = fs::remove_dir_all(&backup);
        Ok(stable_root.join(bundle.file_name().unwrap_or_default()))
    }

    fn download_file(&self, download: &SampleDownload, target: &Path) -> Result<(), String> {
        let url = validated_network_url(&download.url)?;
        let mut response = self
            .client
            .get(url)
            .send()
            .map_err(|error| format!("download sample file {}: {error}", download.id))?;
        validate_response(&response, MAX_DOWNLOAD_BYTES, &download.id)?;
        let temp = target.with_extension(format!("download-{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| {
            let mut file = fs::File::create(&temp)
                .map_err(|error| format!("create sample file {}: {error}", download.id))?;
            let mut copied = 0_u64;
            let mut buffer = [0_u8; 128 * 1024];
            loop {
                let read = response
                    .read(&mut buffer)
                    .map_err(|error| format!("read sample file {}: {error}", download.id))?;
                if read == 0 {
                    break;
                }
                copied = copied.saturating_add(read as u64);
                if copied > MAX_DOWNLOAD_BYTES {
                    return Err(format!(
                        "{}: response exceeds {} bytes",
                        download.id, MAX_DOWNLOAD_BYTES
                    ));
                }
                file.write_all(&buffer[..read])
                    .map_err(|error| format!("write sample file {}: {error}", download.id))?;
            }
            file.sync_all()
                .map_err(|error| format!("sync sample file {}: {error}", download.id))?;
            fs::rename(&temp, target)
                .map_err(|error| format!("publish sample file {}: {error}", download.id))
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result
    }
}

fn builtin_sample(slug: &str) -> Result<ResolvedSample, String> {
    let (title, cards): (&str, &[&str]) = match slug {
        "product-demo" => (
            "OpenTake Product Demo",
            &[
                "Welcome to OpenTake",
                "Import media, trim clips, and add captions",
                "Export when your story is ready",
            ],
        ),
        "quick-tutorial" => (
            "OpenTake Quick Tutorial",
            &[
                "1. Import media from the Media panel",
                "2. Drag clips onto the timeline and trim their edges",
                "3. Press Space to preview, then Export",
            ],
        ),
        "template-project" => ("OpenTake Template", &[]),
        _ => return Err(format!("unknown built-in sample: {slug}")),
    };
    let mut timeline = Timeline::new();
    timeline.settings_configured = true;
    if !cards.is_empty() {
        let mut track = Track::new("sample-text", ClipType::Text);
        for (index, content) in cards.iter().enumerate() {
            let mut clip = Clip::new(format!("sample-text-{index}"), "", index as i32 * 120, 120);
            clip.media_type = ClipType::Text;
            clip.source_clip_type = ClipType::Text;
            clip.text_content = Some((*content).into());
            clip.text_style = Some(TextStyle::default());
            track.clips.push(clip);
        }
        timeline.tracks.push(track);
    }
    Ok(ResolvedSample {
        title: title.into(),
        project: serde_json::to_value(timeline)
            .map_err(|error| format!("encode built-in sample timeline: {error}"))?,
        manifest: serde_json::to_value(MediaManifest::new())
            .map_err(|error| format!("encode built-in sample manifest: {error}"))?,
        generation_log: None,
        poster_url: None,
        downloads: vec![],
        chat: vec![],
    })
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("encode {}: {error}", path.display()))?;
    fs::write(path, bytes).map_err(|error| format!("write {}: {error}", path.display()))
}

fn validate_slug(slug: &str) -> Result<(), String> {
    if slug.is_empty()
        || slug.len() > 80
        || !slug
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err("sample slug must contain only ASCII letters, digits, '-' or '_'".into());
    }
    Ok(())
}

fn safe_name(value: &str) -> String {
    let clean = value
        .chars()
        .map(|character| match character {
            '/' | '\\' | ':' => ' ',
            other => other,
        })
        .collect::<String>();
    let trimmed = clean.trim();
    if trimmed.is_empty() {
        "Sample".into()
    } else {
        trimmed.chars().take(120).collect()
    }
}

fn validate_downloads(downloads: &[SampleDownload]) -> Result<(), String> {
    let mut paths = HashSet::new();
    for download in downloads {
        validated_network_url(&download.url)?;
        let normalized = Path::new(&download.relative_path);
        if normalized.as_os_str().is_empty()
            || normalized
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(format!("unsafe sample path: {}", download.relative_path));
        }
        if !paths.insert(download.relative_path.clone()) {
            return Err(format!("duplicate sample path: {}", download.relative_path));
        }
    }
    Ok(())
}

fn safe_target(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let path = Path::new(relative);
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("unsafe sample path: {relative}"));
    }
    Ok(root.join(path))
}

fn validated_network_url(raw: &str) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(raw).map_err(|error| format!("invalid sample URL: {error}"))?;
    let loopback = url.host_str().is_some_and(|host| {
        host.eq_ignore_ascii_case("localhost")
            || host
                .trim_start_matches('[')
                .trim_end_matches(']')
                .parse::<std::net::IpAddr>()
                .is_ok_and(|address| address.is_loopback())
    });
    if url.scheme() != "https" && !(url.scheme() == "http" && loopback) {
        return Err("sample URL must use HTTPS or loopback HTTP".into());
    }
    if !url.username().is_empty() || url.password().is_some() || url.host_str().is_none() {
        return Err("sample URL contains unsupported credentials or host".into());
    }
    Ok(url)
}

fn read_bounded_response(mut response: Response, max: u64, label: &str) -> Result<Vec<u8>, String> {
    validate_response(&response, max, label)?;
    let mut bytes = Vec::new();
    response
        .by_ref()
        .take(max + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read {label}: {error}"))?;
    if bytes.len() as u64 > max {
        return Err(format!("{label}: response exceeds {max} bytes"));
    }
    Ok(bytes)
}

fn validate_response(response: &Response, max: u64, label: &str) -> Result<(), String> {
    if !response.status().is_success() {
        return Err(format!(
            "{label}: server returned HTTP {}",
            response.status()
        ));
    }
    if response.content_length().is_some_and(|length| length > max) {
        return Err(format!("{label}: response exceeds {max} bytes"));
    }
    Ok(())
}

#[tauri::command]
pub async fn sample_project_materialize(app: AppHandle, slug: String) -> Result<String, String> {
    let activity = crate::updater::begin_mutating_activity(
        &app.state::<crate::updater::InstallAdmissionGate>(),
    )?;
    let backend = crate::account::configured_backend_url().ok().flatten();
    let cache = app
        .path()
        .app_cache_dir()
        .map_err(|error| format!("resolve sample cache: {error}"))?
        .join("samples");
    let progress_app = app.clone();
    let progress_slug = slug.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _activity = activity;
        let service = SampleProjectService::new(cache)?;
        let progress = |fraction: f64| {
            let total = 10_000;
            let _ = progress_app.emit(
                "sample-materialization-progress",
                SampleProgress {
                    slug: progress_slug.clone(),
                    completed: (fraction * total as f64).round() as usize,
                    total,
                },
            );
        };
        match backend {
            Some(backend) => service.materialize(&backend, &slug, progress),
            None => service.materialize_builtin(&slug, progress),
        }
        .map(|path| path.to_string_lossy().into_owned())
    })
    .await
    .map_err(|error| format!("sample materialization task failed: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_domain::{MediaManifest, Timeline};

    #[test]
    fn failed_materialization_rolls_back_entire_sample_directory() {
        let cache = tempfile::tempdir().unwrap();
        let service = SampleProjectService::new(cache.path().to_path_buf()).unwrap();
        let sample = ResolvedSample {
            title: "Rollback demo".into(),
            project: serde_json::to_value(Timeline::new()).unwrap(),
            manifest: serde_json::to_value(MediaManifest::new()).unwrap(),
            generation_log: None,
            poster_url: None,
            downloads: vec![
                SampleDownload {
                    id: "first".into(),
                    relative_path: "media/first.bin".into(),
                    url: "https://samples.example/first".into(),
                },
                SampleDownload {
                    id: "broken".into(),
                    relative_path: "media/broken.bin".into(),
                    url: "https://samples.example/broken".into(),
                },
            ],
            chat: vec![],
        };

        let error = service
            .materialize_resolved(
                "rollback-demo",
                sample,
                |download, target| {
                    if download.id == "broken" {
                        return Err("fixture download failed".into());
                    }
                    fs::write(target, b"complete bytes").map_err(|error| error.to_string())
                },
                |_| {},
            )
            .unwrap_err();

        assert!(error.contains("fixture download failed"), "{error}");
        assert!(!cache.path().join("rollback-demo").exists());
        assert_eq!(fs::read_dir(cache.path()).unwrap().count(), 0);
    }

    #[test]
    fn successful_materialization_publishes_a_valid_bundle_and_completes_progress() {
        let cache = tempfile::tempdir().unwrap();
        let service = SampleProjectService::new(cache.path().to_path_buf()).unwrap();
        let sample = ResolvedSample {
            title: "Starter / sample".into(),
            project: serde_json::to_value(Timeline::new()).unwrap(),
            manifest: serde_json::to_value(MediaManifest::new()).unwrap(),
            generation_log: None,
            poster_url: None,
            downloads: vec![],
            chat: vec![],
        };
        let mut progress = Vec::new();

        let bundle = service
            .materialize_resolved(
                "starter",
                sample,
                |_, _| panic!("empty sample must not download"),
                |value| progress.push(value),
            )
            .unwrap();

        assert_eq!(progress, vec![0.0, 1.0]);
        assert_eq!(
            bundle.file_name().and_then(|name| name.to_str()),
            Some("Starter   sample.opentake")
        );
        Project::open(bundle).unwrap();
        assert_eq!(fs::read_dir(cache.path()).unwrap().count(), 1);
    }

    #[test]
    fn built_in_tutorial_is_offline_and_contains_editing_steps() {
        let cache = tempfile::tempdir().unwrap();
        let service = SampleProjectService::new(cache.path().to_path_buf()).unwrap();

        let bundle = service
            .materialize_builtin("quick-tutorial", |_| {})
            .unwrap();
        let project = Project::open(bundle).unwrap();

        let text = project.timeline.tracks[0]
            .clips
            .iter()
            .filter_map(|clip| clip.text_content.as_deref())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(text.contains("Import media"));
        assert!(text.contains("Press Space"));
        assert_eq!(project.timeline.total_frames(), 360);
    }
}
