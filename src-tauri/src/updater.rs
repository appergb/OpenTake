use base64::Engine as _;
use futures_util::StreamExt as _;
use semver::Version;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest as _;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, ResourceId, Runtime, Webview};
use tauri_plugin_updater::{Update, UpdaterExt};

const GITHUB_REPOSITORY: &str = "appergb/OpenTake";
const UPDATE_RELEASES_URL: &str = "https://github.com/appergb/OpenTake/releases";
const GITHUB_RELEASES_API: &str =
    "https://api.github.com/repos/appergb/OpenTake/releases?per_page=30";
const GITHUB_RELEASES_ATOM: &str = "https://github.com/appergb/OpenTake/releases.atom";
const GITHUB_RELEASES_FEED_ID: &str =
    "tag:github.com,2008:https://github.com/appergb/OpenTake/releases";
const MAX_RELEASE_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_ATOM_RELEASE_ENTRIES: usize = 100;
const MAX_UPDATE_MANIFEST_BYTES: usize = 1024 * 1024;
const MAX_ATTESTATION_BYTES: usize = 64 * 1024;
const MAX_MINISIGN_SIGNATURE_BYTES: usize = 16 * 1024;
const MAX_UPDATE_PACKAGE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const UPDATE_USER_AGENT: &str = "OpenTake-Updater/1.0 (+https://github.com/appergb/OpenTake)";
const UPDATE_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
const RELEASE_SOURCE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);
const RELEASE_RETRY_DELAYS: [std::time::Duration; 7] = [
    std::time::Duration::from_millis(100),
    std::time::Duration::from_millis(200),
    std::time::Duration::from_millis(400),
    std::time::Duration::from_millis(800),
    std::time::Duration::from_secs(1),
    std::time::Duration::from_secs(1),
    std::time::Duration::from_secs(1),
];
const ATTESTATION_DOWNLOAD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);
const UPDATE_PACKAGE_DOWNLOAD_TIMEOUT: std::time::Duration =
    std::time::Duration::from_secs(30 * 60);

#[derive(Debug, Default)]
struct AdmissionState {
    installing: bool,
    active_operations: usize,
}

#[derive(Clone, Debug, Default)]
pub struct InstallAdmissionGate {
    state: std::sync::Arc<std::sync::Mutex<AdmissionState>>,
}

#[derive(Debug)]
pub(crate) struct ActivityLease {
    state: std::sync::Arc<std::sync::Mutex<AdmissionState>>,
}

#[derive(Debug)]
pub(crate) struct InstallAdmissionLease {
    state: std::sync::Arc<std::sync::Mutex<AdmissionState>>,
}

impl InstallAdmissionGate {
    pub(crate) fn begin_activity(&self) -> Result<ActivityLease, String> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.installing {
            return Err("app update installation is in progress".to_string());
        }
        state.active_operations = state.active_operations.saturating_add(1);
        Ok(ActivityLease {
            state: std::sync::Arc::clone(&self.state),
        })
    }

    pub(crate) fn begin_install(&self) -> Result<InstallAdmissionLease, String> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.installing || state.active_operations != 0 {
            return Err(
                "background work is still active; finish or cancel it before installing"
                    .to_string(),
            );
        }
        state.installing = true;
        Ok(InstallAdmissionLease {
            state: std::sync::Arc::clone(&self.state),
        })
    }
}

/// Enter any operation that can publish durable app, cache, project, or
/// manifest state. Update installation takes the exclusive side of this gate,
/// so a stale IPC delivered after the final save barrier fails closed.
pub(crate) fn begin_mutating_activity(
    admission: &InstallAdmissionGate,
) -> Result<ActivityLease, String> {
    admission.begin_activity()
}

impl Drop for ActivityLease {
    fn drop(&mut self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.active_operations = state.active_operations.saturating_sub(1);
    }
}

impl Drop for InstallAdmissionLease {
    fn drop(&mut self) {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .installing = false;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReleaseCandidate {
    tag: String,
    version: Version,
    manifest_url: String,
    notes: Option<String>,
    published_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReleaseSource {
    Api,
    Atom,
}

struct ReleaseSourceResponse {
    status: reqwest::StatusCode,
    body: Vec<u8>,
}

#[derive(Clone, Debug)]
struct ExplicitHttpsProxy {
    url: tauri::Url,
    no_proxy: Option<String>,
    is_https_specific: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdateAttestation {
    schema_version: u32,
    repository: String,
    tag: String,
    version: String,
    source_sha: String,
    platform: String,
    asset_name: String,
    size: u64,
    sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ManifestPlatformEntry {
    url: String,
    signature: String,
    attestation_url: String,
    attestation_signature: String,
}

#[derive(Debug)]
struct AttestationDescriptor {
    url: String,
    signature: String,
    asset_name: String,
}

struct PendingUpdate {
    update: Update,
    attestation: UpdateAttestation,
}

impl tauri::Resource for PendingUpdate {}

struct LocalManifestEndpoint {
    url: tauri::Url,
    task: Option<tauri::async_runtime::JoinHandle<Result<(), String>>>,
}

struct PackageAccumulator {
    bytes: Vec<u8>,
    expected_size: u64,
    expected_sha256: String,
    hasher: sha2::Sha256,
}

impl LocalManifestEndpoint {
    async fn start(manifest: Vec<u8>) -> Result<Self, String> {
        if manifest.is_empty() || manifest.len() > MAX_UPDATE_MANIFEST_BYTES {
            return Err("signed update manifest was empty or too large".to_string());
        }
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(|_| "could not bind the local update verifier".to_string())?;
        let address = listener
            .local_addr()
            .map_err(|_| "could not inspect the local update verifier".to_string())?;
        let token = uuid::Uuid::new_v4().simple().to_string();
        let path = format!("/manifest-{token}.json");
        let url = tauri::Url::parse(&format!("http://127.0.0.1:{}{path}", address.port()))
            .map_err(|_| "could not create the local update verifier URL".to_string())?;
        let task = tauri::async_runtime::spawn(serve_manifest_once(listener, path, manifest));
        Ok(Self {
            url,
            task: Some(task),
        })
    }

    fn url(&self) -> &tauri::Url {
        &self.url
    }

    async fn finish(mut self) -> Result<(), String> {
        let task = self
            .task
            .take()
            .ok_or_else(|| "local update verifier was already consumed".to_string())?;
        tokio::time::timeout(std::time::Duration::from_secs(1), task)
            .await
            .map_err(|_| "local update verifier did not shut down".to_string())?
            .map_err(|_| "local update verifier task failed".to_string())?
    }
}

impl Drop for LocalManifestEndpoint {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

async fn serve_manifest_once(
    listener: tokio::net::TcpListener,
    expected_path: String,
    manifest: Vec<u8>,
) -> Result<(), String> {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        let (stream, peer) = tokio::time::timeout_at(deadline, listener.accept())
            .await
            .map_err(|_| "local update verifier timed out".to_string())?
            .map_err(|_| "local update verifier could not accept a request".to_string())?;
        if !peer.ip().is_loopback() {
            continue;
        }

        if serve_manifest_connection_until(stream, &expected_path, &manifest, deadline).await? {
            return Ok(());
        }
    }
}

async fn serve_manifest_connection_until(
    mut stream: tokio::net::TcpStream,
    expected_path: &str,
    manifest: &[u8],
    deadline: tokio::time::Instant,
) -> Result<bool, String> {
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

    tokio::time::timeout_at(deadline, async {
        let mut request = Vec::with_capacity(1024);
        loop {
            let mut chunk = [0_u8; 1024];
            let read = stream
                .read(&mut chunk)
                .await
                .map_err(|_| "local update verifier could not read a request".to_string())?;
            if read == 0 || request.len().saturating_add(read) > 8 * 1024 {
                break;
            }
            request.extend_from_slice(&chunk[..read]);
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        let request_line = request
            .split(|byte| *byte == b'\n')
            .next()
            .and_then(|line| std::str::from_utf8(line).ok())
            .map(str::trim_end);
        if request_line != Some(&format!("GET {expected_path} HTTP/1.1")) {
            let _ = stream
                .write_all(
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await;
            let _ = stream.shutdown().await;
            return Ok::<bool, String>(false);
        }

        let headers = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
            manifest.len()
        );
        stream
            .write_all(headers.as_bytes())
            .await
            .map_err(|_| "local update verifier could not serve the manifest".to_string())?;
        stream
            .write_all(manifest)
            .await
            .map_err(|_| "local update verifier could not serve the manifest".to_string())?;
        stream
            .shutdown()
            .await
            .map_err(|_| "local update verifier could not finish the response".to_string())?;
        Ok(true)
    })
    .await
    .map_err(|_| "local update verifier timed out".to_string())?
}

fn release_version(tag: &str) -> Result<Version, String> {
    let value = tag
        .strip_prefix('v')
        .ok_or_else(|| "release tag must start with v".to_string())?;
    let version =
        Version::parse(value).map_err(|_| "release tag is not valid SemVer".to_string())?;
    if format!("v{version}") != tag {
        return Err("release tag is not canonical SemVer".to_string());
    }
    Ok(version)
}

fn select_release(body: &[u8], current: &Version) -> Result<Option<ReleaseCandidate>, String> {
    let releases: Vec<GitHubRelease> =
        serde_json::from_slice(body).map_err(|_| "invalid GitHub releases response".to_string())?;
    let mut candidates = Vec::new();
    for release in releases {
        if release.draft {
            continue;
        }
        let Ok(version) = release_version(&release.tag_name) else {
            continue;
        };
        // Equality here means the GitHub flag contradicts the SemVer tag:
        // prerelease=true with a stable tag, or false with a prerelease tag.
        if release.prerelease == version.pre.is_empty()
            || version <= *current
            || (current.pre.is_empty() && !version.pre.is_empty())
        {
            continue;
        }
        let expected_name = format!("updater-{}.json", release.tag_name);
        let expected_url = manifest_endpoint(&release.tag_name)?;
        let has_manifest = release
            .assets
            .iter()
            .any(|asset| asset.name == expected_name && asset.browser_download_url == expected_url);
        if !has_manifest {
            continue;
        }
        candidates.push(ReleaseCandidate {
            tag: release.tag_name,
            version,
            manifest_url: expected_url,
            notes: release.body.filter(|body| !body.trim().is_empty()),
            published_at: release.published_at,
        });
    }
    Ok(candidates
        .into_iter()
        .max_by(|left, right| left.version.cmp(&right.version)))
}

#[derive(Default)]
struct AtomReleaseEntry {
    tag: Option<String>,
    published_at: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AtomTextField {
    FeedId,
    PublishedAt,
}

fn strict_atom_attribute(
    element: &quick_xml::events::BytesStart<'_>,
    name: &[u8],
) -> Result<Option<String>, String> {
    let mut value = None;
    for attribute in element.attributes() {
        let attribute = attribute.map_err(|_| "invalid GitHub Releases Atom feed".to_string())?;
        if attribute.key.as_ref() != name {
            continue;
        }
        if value.is_some() {
            return Err("invalid GitHub Releases Atom feed".to_string());
        }
        let raw = std::str::from_utf8(attribute.value.as_ref())
            .map_err(|_| "invalid GitHub Releases Atom feed".to_string())?;
        if raw.contains('&') || !raw.is_ascii() {
            return Err("invalid GitHub Releases Atom feed".to_string());
        }
        value = Some(raw.to_string());
    }
    Ok(value)
}

fn atom_release_tag(raw: &str) -> Result<String, String> {
    let url = tauri::Url::parse(raw)
        .map_err(|_| "GitHub Releases Atom link is outside the allowlist".to_string())?;
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("GitHub Releases Atom link is outside the allowlist".to_string());
    }
    let prefix = format!("/{GITHUB_REPOSITORY}/releases/tag/");
    let tag = url
        .path()
        .strip_prefix(&prefix)
        .filter(|tag| !tag.is_empty() && !tag.contains('/') && !tag.contains('%'))
        .ok_or_else(|| "GitHub Releases Atom link is outside the allowlist".to_string())?;
    if raw != format!("https://github.com/{GITHUB_REPOSITORY}/releases/tag/{tag}") {
        return Err("GitHub Releases Atom link is outside the allowlist".to_string());
    }
    Ok(tag.to_string())
}

fn observe_atom_link(
    element: &quick_xml::events::BytesStart<'_>,
    entry: Option<&mut AtomReleaseEntry>,
    feed_self_seen: &mut bool,
) -> Result<(), String> {
    let rel = strict_atom_attribute(element, b"rel")?;
    let media_type = strict_atom_attribute(element, b"type")?;
    let href = strict_atom_attribute(element, b"href")?;
    match (
        entry,
        rel.as_deref(),
        media_type.as_deref(),
        href.as_deref(),
    ) {
        (None, Some("self"), Some("application/atom+xml"), Some(GITHUB_RELEASES_ATOM)) => {
            if *feed_self_seen {
                return Err("invalid GitHub Releases Atom feed".to_string());
            }
            *feed_self_seen = true;
        }
        (Some(entry), Some("alternate"), Some("text/html"), Some(href)) => {
            if entry.tag.is_some() {
                return Err("invalid GitHub Releases Atom feed".to_string());
            }
            entry.tag = Some(atom_release_tag(href)?);
        }
        _ => {}
    }
    Ok(())
}

fn select_atom_release(body: &[u8], current: &Version) -> Result<Option<ReleaseCandidate>, String> {
    if body.is_empty() || body.len() > MAX_RELEASE_RESPONSE_BYTES {
        return Err("GitHub Releases Atom response was empty or too large".to_string());
    }

    let mut reader = quick_xml::Reader::from_reader(body);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut depth = 0_usize;
    let mut feed_seen = false;
    let mut feed_closed = false;
    let mut feed_id = None;
    let mut feed_self_seen = false;
    let mut entry = None::<AtomReleaseEntry>;
    let mut entry_count = 0_usize;
    let mut capture = None::<AtomTextField>;
    let mut captured = String::new();
    let mut candidates = Vec::new();

    loop {
        use quick_xml::events::Event;

        match reader
            .read_event_into(&mut buffer)
            .map_err(|_| "invalid GitHub Releases Atom feed".to_string())?
        {
            Event::Start(element) => {
                let name = element.local_name();
                if depth == 0 {
                    if name.as_ref() != b"feed" || feed_seen || feed_closed {
                        return Err("invalid GitHub Releases Atom feed".to_string());
                    }
                    if strict_atom_attribute(&element, b"xmlns")?.as_deref()
                        != Some("http://www.w3.org/2005/Atom")
                    {
                        return Err("invalid GitHub Releases Atom feed".to_string());
                    }
                    feed_seen = true;
                } else if !feed_seen || feed_closed {
                    return Err("invalid GitHub Releases Atom feed".to_string());
                } else if depth == 1 && name.as_ref() == b"entry" {
                    if entry.is_some() || capture.is_some() {
                        return Err("invalid GitHub Releases Atom feed".to_string());
                    }
                    entry_count = entry_count.saturating_add(1);
                    if entry_count > MAX_ATOM_RELEASE_ENTRIES {
                        return Err("GitHub Releases Atom feed had too many entries".to_string());
                    }
                    entry = Some(AtomReleaseEntry::default());
                } else if depth == 1 && entry.is_none() && name.as_ref() == b"id" {
                    if feed_id.is_some() || capture.is_some() {
                        return Err("invalid GitHub Releases Atom feed".to_string());
                    }
                    capture = Some(AtomTextField::FeedId);
                    captured.clear();
                } else if depth == 2 && entry.is_some() && name.as_ref() == b"updated" {
                    if capture.is_some() {
                        return Err("invalid GitHub Releases Atom feed".to_string());
                    }
                    capture = Some(AtomTextField::PublishedAt);
                    captured.clear();
                } else if (depth == 1 || depth == 2) && name.as_ref() == b"link" {
                    observe_atom_link(&element, entry.as_mut(), &mut feed_self_seen)?;
                } else if capture.is_some() {
                    return Err("invalid GitHub Releases Atom feed".to_string());
                }
                depth = depth.saturating_add(1);
            }
            Event::Empty(element) => {
                let name = element.local_name();
                if depth == 0 || !feed_seen || feed_closed {
                    return Err("invalid GitHub Releases Atom feed".to_string());
                }
                if (depth == 1 || depth == 2) && name.as_ref() == b"link" {
                    observe_atom_link(&element, entry.as_mut(), &mut feed_self_seen)?;
                }
            }
            Event::Text(text) => {
                if depth == 0 {
                    let decoded = text
                        .decode()
                        .map_err(|_| "invalid GitHub Releases Atom feed".to_string())?;
                    if !decoded.trim().is_empty() {
                        return Err("invalid GitHub Releases Atom feed".to_string());
                    }
                } else if capture.is_some() {
                    let decoded = text
                        .decode()
                        .map_err(|_| "invalid GitHub Releases Atom feed".to_string())?;
                    if decoded.contains('&') || !decoded.is_ascii() {
                        return Err("invalid GitHub Releases Atom feed".to_string());
                    }
                    if captured.len().saturating_add(decoded.len()) > 256 {
                        return Err("invalid GitHub Releases Atom feed".to_string());
                    }
                    captured.push_str(&decoded);
                }
            }
            Event::End(element) => {
                if depth == 0 {
                    return Err("invalid GitHub Releases Atom feed".to_string());
                }
                depth -= 1;
                let name = element.local_name();
                if depth == 1 && name.as_ref() == b"id" && capture == Some(AtomTextField::FeedId) {
                    feed_id = Some(captured.trim().to_string());
                    capture = None;
                } else if depth == 2
                    && name.as_ref() == b"updated"
                    && capture == Some(AtomTextField::PublishedAt)
                {
                    let value = captured.trim();
                    if !value.is_empty() {
                        entry
                            .as_mut()
                            .ok_or_else(|| "invalid GitHub Releases Atom feed".to_string())?
                            .published_at = Some(value.to_string());
                    }
                    capture = None;
                } else if depth == 1 && name.as_ref() == b"entry" {
                    if capture.is_some() {
                        return Err("invalid GitHub Releases Atom feed".to_string());
                    }
                    let entry = entry
                        .take()
                        .ok_or_else(|| "invalid GitHub Releases Atom feed".to_string())?;
                    let tag = entry
                        .tag
                        .ok_or_else(|| "invalid GitHub Releases Atom feed".to_string())?;
                    let Ok(version) = release_version(&tag) else {
                        buffer.clear();
                        continue;
                    };
                    if version > *current && (!current.pre.is_empty() || version.pre.is_empty()) {
                        candidates.push(ReleaseCandidate {
                            manifest_url: manifest_endpoint(&tag)?,
                            tag,
                            version,
                            notes: None,
                            published_at: entry.published_at,
                        });
                    }
                } else if depth == 0 && name.as_ref() == b"feed" {
                    if entry.is_some() || capture.is_some() {
                        return Err("invalid GitHub Releases Atom feed".to_string());
                    }
                    feed_closed = true;
                }
            }
            Event::DocType(_) | Event::CData(_) => {
                return Err("invalid GitHub Releases Atom feed".to_string());
            }
            Event::Decl(_) => {
                if depth != 0 || feed_seen || feed_closed {
                    return Err("invalid GitHub Releases Atom feed".to_string());
                }
            }
            Event::Comment(_) | Event::PI(_) => {
                if feed_closed {
                    return Err("invalid GitHub Releases Atom feed".to_string());
                }
            }
            Event::GeneralRef(_) => {
                if depth == 0 || feed_closed || capture.is_some() {
                    return Err("invalid GitHub Releases Atom feed".to_string());
                }
            }
            Event::Eof => break,
        }
        buffer.clear();
    }

    if !feed_seen
        || !feed_closed
        || depth != 0
        || feed_id.as_deref() != Some(GITHUB_RELEASES_FEED_ID)
        || !feed_self_seen
    {
        return Err("invalid GitHub Releases Atom feed".to_string());
    }
    Ok(candidates
        .into_iter()
        .max_by(|left, right| left.version.cmp(&right.version)))
}

fn manifest_endpoint(tag: &str) -> Result<String, String> {
    release_version(tag)?;
    Ok(format!(
        "https://github.com/{GITHUB_REPOSITORY}/releases/download/{tag}/updater-{tag}.json"
    ))
}

fn download_asset_name(raw: &str, tag: &str) -> Result<String, String> {
    release_version(tag)?;
    let url = tauri::Url::parse(raw).map_err(|_| "update package URL is invalid".to_string())?;
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("update package URL is outside the OpenTake release allowlist".to_string());
    }
    let prefix = format!("/{GITHUB_REPOSITORY}/releases/download/{tag}/");
    let asset = url
        .path()
        .strip_prefix(&prefix)
        .filter(|asset| !asset.is_empty() && !asset.contains('/') && !asset.contains('%'))
        .ok_or_else(|| "update package URL is outside the selected release".to_string())?;
    if asset == "." || asset == ".." {
        return Err("update package asset name is invalid".to_string());
    }
    Ok(asset.to_string())
}

fn validate_download_url(raw: &str, tag: &str) -> Result<(), String> {
    download_asset_name(raw, tag).map(|_| ())
}

fn is_allowed_github_redirect(url: &tauri::Url) -> bool {
    url.scheme() == "https"
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && matches!(
            url.host_str(),
            Some(
                "github.com"
                    | "objects.githubusercontent.com"
                    | "release-assets.githubusercontent.com"
            )
        )
}

fn validate_checked_update(
    version: &str,
    download_url: &str,
    signature: &str,
    candidate: &ReleaseCandidate,
    current: &Version,
) -> Result<(), String> {
    let announced = Version::parse(version)
        .map_err(|_| "update manifest announced an invalid version".to_string())?;
    if announced != candidate.version || announced <= *current {
        return Err("update manifest version does not match the selected release".to_string());
    }
    if signature.trim().is_empty() {
        return Err("update manifest signature is missing".to_string());
    }
    validate_download_url(download_url, &candidate.tag)
}

fn update_platform_for_bundle(
    os: &str,
    arch: &str,
    bundle_type: Option<tauri::utils::config::BundleType>,
) -> Result<&'static str, String> {
    use tauri::utils::config::BundleType;

    match (os, arch, bundle_type) {
        ("macos", "aarch64", Some(BundleType::App | BundleType::Dmg)) => Ok("darwin-aarch64"),
        ("windows", "x86_64", Some(BundleType::Msi)) => Ok("windows-x86_64-msi"),
        ("windows", "x86_64", Some(BundleType::Nsis)) => Ok("windows-x86_64-nsis"),
        _ => Err("automatic updates are not available for this installed bundle".to_string()),
    }
}

fn validate_platform_asset_name(platform: &str, asset_name: &str) -> Result<(), String> {
    let matches_platform = match platform {
        "darwin-aarch64" => asset_name.ends_with(".app.tar.gz"),
        "windows-x86_64-msi" => asset_name.ends_with(".msi"),
        "windows-x86_64-nsis" => asset_name.ends_with(".exe"),
        _ => false,
    };
    if matches_platform {
        Ok(())
    } else {
        Err("update package type does not match the installed bundle".to_string())
    }
}

fn current_update_platform(package_url: &str, tag: &str) -> Result<&'static str, String> {
    let platform = update_platform_for_bundle(
        std::env::consts::OS,
        std::env::consts::ARCH,
        tauri::utils::platform::bundle_type(),
    )?;
    let asset_name = download_asset_name(package_url, tag)?;
    validate_platform_asset_name(platform, &asset_name)?;
    Ok(platform)
}

fn manifest_attestation_descriptor(
    raw_json: &serde_json::Value,
    platform: &str,
    package_url: &str,
    package_signature: &str,
    tag: &str,
) -> Result<AttestationDescriptor, String> {
    let platforms = raw_json
        .get("platforms")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "update manifest platforms are invalid".to_string())?;
    if platforms.contains_key("windows-x86_64") {
        return Err("generic Windows updater targets are not allowed".to_string());
    }
    let entry = platforms
        .get(platform)
        .cloned()
        .ok_or_else(|| "update manifest is missing a signed attestation".to_string())?;
    let entry: ManifestPlatformEntry = serde_json::from_value(entry)
        .map_err(|_| "update manifest attestation fields are invalid".to_string())?;
    if entry.url != package_url || entry.signature != package_signature {
        return Err(
            "update manifest platform entry does not match the selected package".to_string(),
        );
    }
    let asset_name = download_asset_name(package_url, tag)?;
    validate_platform_asset_name(platform, &asset_name)?;
    let expected_url = format!("{package_url}.attestation.json");
    if entry.attestation_url != expected_url
        || download_asset_name(&entry.attestation_url, tag)?
            != format!("{asset_name}.attestation.json")
    {
        return Err("update attestation URL does not match the selected package".to_string());
    }
    if entry.attestation_signature.trim().is_empty()
        || entry.attestation_signature.len() > MAX_MINISIGN_SIGNATURE_BYTES
    {
        return Err("update attestation signature is missing or too large".to_string());
    }
    Ok(AttestationDescriptor {
        url: entry.attestation_url,
        signature: entry.attestation_signature,
        asset_name,
    })
}

fn is_lower_hex(value: &str, len: usize) -> bool {
    value.len() == len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && value.bytes().any(|byte| byte != b'0')
}

fn validate_attestation(
    attestation: &UpdateAttestation,
    candidate: &ReleaseCandidate,
    platform: &str,
    asset_name: &str,
) -> Result<(), String> {
    let attested_version = Version::parse(&attestation.version).ok();
    if attestation.schema_version != 1
        || attestation.repository != GITHUB_REPOSITORY
        || attestation.tag != candidate.tag
        || attested_version.as_ref() != Some(&candidate.version)
        || attested_version
            .as_ref()
            .is_some_and(|version| version.to_string() != attestation.version)
    {
        return Err("update attestation does not match the selected release".to_string());
    }
    if attestation.platform != platform || attestation.asset_name != asset_name {
        return Err("update attestation does not match the selected platform asset".to_string());
    }
    if !is_lower_hex(&attestation.source_sha, 40) {
        return Err("update attestation source SHA is invalid".to_string());
    }
    if attestation.size == 0 || attestation.size > MAX_UPDATE_PACKAGE_BYTES {
        return Err("update attestation package size is invalid".to_string());
    }
    if !is_lower_hex(&attestation.sha256, 64) {
        return Err("update attestation package SHA-256 is invalid".to_string());
    }
    Ok(())
}

impl PackageAccumulator {
    fn new(attestation: &UpdateAttestation) -> Result<Self, String> {
        if attestation.size == 0 || attestation.size > MAX_UPDATE_PACKAGE_BYTES {
            return Err("update attestation package size is invalid".to_string());
        }
        if !is_lower_hex(&attestation.sha256, 64) {
            return Err("update attestation package SHA-256 is invalid".to_string());
        }
        Ok(Self {
            bytes: Vec::with_capacity(attestation.size.min(16 * 1024 * 1024) as usize),
            expected_size: attestation.size,
            expected_sha256: attestation.sha256.clone(),
            hasher: sha2::Sha256::new(),
        })
    }

    fn push(&mut self, chunk: &[u8]) -> Result<(), String> {
        let next_size = (self.bytes.len() as u64)
            .checked_add(chunk.len() as u64)
            .ok_or_else(|| "downloaded update package exceeded its signed size".to_string())?;
        if next_size > self.expected_size || next_size > MAX_UPDATE_PACKAGE_BYTES {
            return Err("downloaded update package exceeded its signed size".to_string());
        }
        self.bytes
            .try_reserve(chunk.len())
            .map_err(|_| "could not allocate memory for the update package".to_string())?;
        self.hasher.update(chunk);
        self.bytes.extend_from_slice(chunk);
        Ok(())
    }

    fn finish(self) -> Result<Vec<u8>, String> {
        if self.bytes.len() as u64 != self.expected_size {
            return Err(
                "downloaded update package size did not match its signed attestation".to_string(),
            );
        }
        let digest = format!("{:x}", self.hasher.finalize());
        if digest != self.expected_sha256 {
            return Err(
                "downloaded update package SHA-256 did not match its signed attestation"
                    .to_string(),
            );
        }
        Ok(self.bytes)
    }
}

async fn run_with_update_timeout<T>(
    timeout: std::time::Duration,
    future: impl std::future::Future<Output = Result<T, String>>,
) -> Result<T, String> {
    tokio::time::timeout(timeout, future)
        .await
        .map_err(|_| "update download timed out".to_string())?
}

fn updater_public_key() -> Result<minisign_verify::PublicKey, String> {
    let config: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json"))
        .map_err(|_| "embedded updater public key is invalid".to_string())?;
    let encoded = config["plugins"]["updater"]["pubkey"]
        .as_str()
        .ok_or_else(|| "embedded updater public key is invalid".to_string())?;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| "embedded updater public key is invalid".to_string())?;
    let armored = std::str::from_utf8(&decoded)
        .map_err(|_| "embedded updater public key is invalid".to_string())?;
    minisign_verify::PublicKey::decode(armored)
        .map_err(|_| "embedded updater public key is invalid".to_string())
}

fn verify_minisign(data: &[u8], encoded_signature: &str, subject: &str) -> Result<(), String> {
    if encoded_signature.is_empty() || encoded_signature.len() > MAX_MINISIGN_SIGNATURE_BYTES {
        return Err(format!("signed {subject} signature is invalid"));
    }
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded_signature)
        .map_err(|_| format!("signed {subject} signature is invalid"))?;
    if decoded.len() > MAX_MINISIGN_SIGNATURE_BYTES {
        return Err(format!("signed {subject} signature is invalid"));
    }
    let armored = std::str::from_utf8(&decoded)
        .map_err(|_| format!("signed {subject} signature is invalid"))?;
    let signature = minisign_verify::Signature::decode(armored)
        .map_err(|_| format!("signed {subject} signature is invalid"))?;
    updater_public_key()?
        .verify(data, &signature, false)
        .map_err(|_| format!("signed {subject} verification failed"))
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum OperationPhase {
    #[default]
    Idle,
    Checking,
    Pending {
        rid: u32,
    },
    Closing {
        rid: u32,
    },
    Installing {
        rid: u32,
    },
}

#[derive(Debug, Default)]
pub struct UpdateCoordinator {
    phase: std::sync::Mutex<OperationPhase>,
}

struct OperationLease<'a> {
    coordinator: &'a UpdateCoordinator,
    completed: bool,
}

struct CloseLease<'a> {
    coordinator: &'a UpdateCoordinator,
    rid: u32,
    completed: bool,
}

impl UpdateCoordinator {
    #[cfg(test)]
    fn begin_check(&self) -> Result<OperationLease<'_>, String> {
        let mut phase = self
            .phase
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if *phase != OperationPhase::Idle {
            return Err("another update operation is already in progress".to_string());
        }
        *phase = OperationPhase::Checking;
        Ok(OperationLease {
            coordinator: self,
            completed: false,
        })
    }

    fn begin_check_recovering_pending(
        &self,
        close_pending: impl FnOnce(ResourceId) -> Result<(), String>,
    ) -> Result<OperationLease<'_>, String> {
        let mut phase = self
            .phase
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match *phase {
            OperationPhase::Idle => {}
            OperationPhase::Pending { rid } => {
                // A check from a freshly loaded renderer has no Zustand-owned
                // RID to close. Close the exact native resource when it still
                // exists; a missing RID means the Webview resource table was
                // already replaced, so either outcome safely resets the stale
                // pending phase before the new check.
                let _ = close_pending(rid);
            }
            _ => return Err("another update operation is already in progress".to_string()),
        }
        *phase = OperationPhase::Checking;
        Ok(OperationLease {
            coordinator: self,
            completed: false,
        })
    }

    fn begin_install(&self, rid: u32) -> Result<OperationLease<'_>, String> {
        let mut phase = self
            .phase
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if *phase != (OperationPhase::Pending { rid }) {
            return Err("update resource is stale or another operation is active".to_string());
        }
        *phase = OperationPhase::Installing { rid };
        Ok(OperationLease {
            coordinator: self,
            completed: false,
        })
    }

    fn begin_close(&self, rid: u32) -> Result<CloseLease<'_>, String> {
        let mut phase = self
            .phase
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if *phase != (OperationPhase::Pending { rid }) {
            return Err("update resource is stale or busy".to_string());
        }
        *phase = OperationPhase::Closing { rid };
        Ok(CloseLease {
            coordinator: self,
            rid,
            completed: false,
        })
    }

    pub(crate) fn prevents_user_exit(&self) -> bool {
        self.phase
            .lock()
            .map(|phase| matches!(*phase, OperationPhase::Installing { .. }))
            .unwrap_or(true)
    }

    #[cfg(test)]
    fn phase(&self) -> OperationPhase {
        *self.phase.lock().unwrap()
    }
}

impl OperationLease<'_> {
    fn publish_pending(mut self, rid: u32) -> Result<(), String> {
        let mut phase = self
            .coordinator
            .phase
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if *phase != OperationPhase::Checking {
            return Err("update check lease is no longer active".to_string());
        }
        *phase = OperationPhase::Pending { rid };
        self.completed = true;
        Ok(())
    }
}

impl Drop for OperationLease<'_> {
    fn drop(&mut self) {
        if !self.completed {
            *self.coordinator.phase.lock().unwrap() = OperationPhase::Idle;
        }
    }
}

impl CloseLease<'_> {
    fn complete(mut self) -> Result<(), String> {
        let mut phase = self
            .coordinator
            .phase
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if *phase != (OperationPhase::Closing { rid: self.rid }) {
            return Err("update close lease is no longer active".to_string());
        }
        *phase = OperationPhase::Idle;
        self.completed = true;
        Ok(())
    }
}

impl Drop for CloseLease<'_> {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        let mut phase = self
            .coordinator
            .phase
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if *phase == (OperationPhase::Closing { rid: self.rid }) {
            *phase = OperationPhase::Pending { rid: self.rid };
        }
    }
}

fn install_with_save_barriers<T>(
    preflight: impl FnOnce() -> Result<T, String>,
    install: impl FnOnce(&T) -> Result<(), String>,
    post_install_save: impl FnOnce() -> Result<(), String>,
    restart: impl FnOnce(),
) -> Result<(), String> {
    let readiness = preflight()?;
    install(&readiness)?;
    post_install_save()?;
    restart();
    Ok(())
}

fn save_current_project(core: &opentake_core::AppCore) -> Result<(), String> {
    let snapshot = core.runtime_snapshot();
    if let Some(project_path) = snapshot.project_dir.as_deref() {
        core.save_project_with_thumbnail_for_project(
            snapshot.project_epoch,
            Some(project_path),
            None,
            None,
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMetadata {
    pub rid: ResourceId,
    pub current_version: String,
    pub version: String,
    pub notes: Option<String>,
    pub published_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", content = "data", rename_all = "camelCase")]
pub enum UpdateInstallEvent {
    Started { content_length: Option<u64> },
    Progress { downloaded: u64 },
    Installing,
    Restarting,
}

fn validated_explicit_proxy_url(raw: &str) -> Result<tauri::Url, String> {
    let url =
        tauri::Url::parse(raw).map_err(|_| "update proxy configuration is invalid".to_string())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || !matches!(url.path(), "" | "/")
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("update proxy configuration is invalid".to_string());
    }
    Ok(url)
}

fn first_proxy_environment_value(
    names: &[&str],
    get: &impl Fn(&str) -> Option<std::ffi::OsString>,
) -> Result<Option<String>, String> {
    for name in names {
        let Some(raw) = get(name) else {
            continue;
        };
        let raw = raw
            .into_string()
            .map_err(|_| "update proxy configuration is invalid".to_string())?;
        if !raw.trim().is_empty() {
            return Ok(Some(raw));
        }
    }
    Ok(None)
}

fn environment_https_proxy_from(
    get: impl Fn(&str) -> Option<std::ffi::OsString>,
) -> Result<Option<ExplicitHttpsProxy>, String> {
    if get("REQUEST_METHOD").is_some() {
        return Ok(None);
    }
    let https = first_proxy_environment_value(&["HTTPS_PROXY", "https_proxy"], &get)?;
    let is_https_specific = https.is_some();
    let raw = https.or(first_proxy_environment_value(
        &["ALL_PROXY", "all_proxy"],
        &get,
    )?);
    let Some(raw) = raw else {
        return Ok(None);
    };
    let no_proxy = first_proxy_environment_value(&["NO_PROXY", "no_proxy"], &get)?;
    Ok(Some(ExplicitHttpsProxy {
        url: validated_explicit_proxy_url(raw.trim())?,
        no_proxy,
        is_https_specific,
    }))
}

fn validated_proxy_environment() -> Result<Option<ExplicitHttpsProxy>, String> {
    if std::env::var_os("REQUEST_METHOD").is_some() {
        return Ok(None);
    }
    let proxy = environment_https_proxy_from(|name| std::env::var_os(name))?;
    // Validate an otherwise lower-priority ALL_PROXY too. Reqwest itself keeps
    // HTTPS_PROXY > macOS/Windows per-scheme settings > ALL_PROXY; this check
    // only prevents an unsupported or credential-bearing explicit URL from
    // being accepted silently.
    for name in ["ALL_PROXY", "all_proxy"] {
        let Some(raw) = std::env::var_os(name) else {
            continue;
        };
        let raw = raw
            .into_string()
            .map_err(|_| "update proxy configuration is invalid".to_string())?;
        if !raw.trim().is_empty() {
            validated_explicit_proxy_url(raw.trim())?;
        }
    }
    Ok(proxy)
}

fn release_http_client_with_proxy(
    proxy: Option<ExplicitHttpsProxy>,
) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder()
        // Keep a fixed, non-sensitive updater identity on both origin requests
        // and HTTPS CONNECT handshakes.
        .user_agent(UPDATE_USER_AGENT)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(UPDATE_CONNECT_TIMEOUT)
        .timeout(std::time::Duration::from_secs(15));
    if let Some(proxy) = proxy.filter(|proxy| proxy.is_https_specific) {
        let no_proxy = proxy
            .no_proxy
            .as_deref()
            .and_then(reqwest::NoProxy::from_string);
        builder = builder.proxy(
            reqwest::Proxy::https(proxy.url.as_str())
                .map_err(|_| "update proxy configuration is invalid".to_string())?
                .no_proxy(no_proxy),
        );
    }
    builder
        .build()
        .map_err(|_| "could not initialize the update client".to_string())
}

fn release_http_client() -> Result<reqwest::Client, String> {
    release_http_client_with_proxy(validated_proxy_environment()?)
}

fn updater_http_client_builder(
    mut builder: reqwest_updater::ClientBuilder,
) -> Result<reqwest_updater::ClientBuilder, String> {
    if let Some(proxy) = validated_proxy_environment()?.filter(|proxy| proxy.is_https_specific) {
        let no_proxy = proxy
            .no_proxy
            .as_deref()
            .and_then(reqwest_updater::NoProxy::from_string);
        builder = builder.proxy(
            reqwest_updater::Proxy::https(proxy.url.as_str())
                .map_err(|_| "update proxy configuration is invalid".to_string())?
                .no_proxy(no_proxy),
        );
    }
    Ok(builder)
}

fn configured_update_proxy(proxy: &tauri::Url) -> Result<reqwest_updater::Proxy, String> {
    let url = validated_explicit_proxy_url(proxy.as_str())?;
    let no_proxy = std::env::var("NO_PROXY")
        .or_else(|_| std::env::var("no_proxy"))
        .ok()
        .and_then(|value| reqwest_updater::NoProxy::from_string(&value));
    Ok(reqwest_updater::Proxy::https(url.as_str())
        .map_err(|_| "update proxy configuration is invalid".to_string())?
        .no_proxy(no_proxy))
}

async fn retry_transient<T, E, F, Fut>(
    delays: &[std::time::Duration],
    mut operation: F,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
{
    let mut remaining_delays = delays.iter();
    loop {
        match operation().await {
            Ok(value) => return Ok(value),
            Err(error) => {
                let Some(delay) = remaining_delays.next() else {
                    return Err(error);
                };
                tokio::time::sleep(*delay).await;
            }
        }
    }
}

async fn fetch_release_source(
    client: &reqwest::Client,
    source: ReleaseSource,
) -> Result<ReleaseSourceResponse, String> {
    let (url, accept, expected_media_type) = match source {
        ReleaseSource::Api => (
            GITHUB_RELEASES_API,
            "application/vnd.github+json",
            "application/json",
        ),
        ReleaseSource::Atom => (
            GITHUB_RELEASES_ATOM,
            "application/atom+xml",
            "application/atom+xml",
        ),
    };
    let mut response = tokio::time::timeout(
        RELEASE_SOURCE_TIMEOUT,
        retry_transient(&RELEASE_RETRY_DELAYS, || {
            let mut request = client.get(url).header(reqwest::header::ACCEPT, accept);
            if source == ReleaseSource::Api {
                request = request.header("X-GitHub-Api-Version", "2022-11-28");
            }
            request.send()
        }),
    )
    .await
    .map_err(|_| "could not reach GitHub Releases".to_string())?
    .map_err(|_| "could not reach GitHub Releases".to_string())?;
    let status = response.status();
    if status != reqwest::StatusCode::OK {
        return Ok(ReleaseSourceResponse {
            status,
            body: Vec::new(),
        });
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim);
    if content_type != Some(expected_media_type) {
        return Err("GitHub Releases returned an unexpected content type".to_string());
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RELEASE_RESPONSE_BYTES as u64)
    {
        return Err("GitHub Releases response was too large".to_string());
    }
    let mut body = Vec::with_capacity(
        response
            .content_length()
            .unwrap_or_default()
            .min(MAX_RELEASE_RESPONSE_BYTES as u64) as usize,
    );
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "could not read GitHub Releases response".to_string())?
    {
        if body.len().saturating_add(chunk.len()) > MAX_RELEASE_RESPONSE_BYTES {
            return Err("GitHub Releases response was too large".to_string());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(ReleaseSourceResponse { status, body })
}

async fn discover_release_with_fetch<F, Fut>(
    current: &Version,
    mut fetch: F,
) -> Result<Option<ReleaseCandidate>, String>
where
    F: FnMut(ReleaseSource) -> Fut,
    Fut: std::future::Future<Output = Result<ReleaseSourceResponse, String>>,
{
    let api = fetch(ReleaseSource::Api).await?;
    if api.status == reqwest::StatusCode::OK {
        return select_release(&api.body, current);
    }
    if !matches!(
        api.status,
        reqwest::StatusCode::FORBIDDEN | reqwest::StatusCode::TOO_MANY_REQUESTS
    ) {
        return Err(format!(
            "GitHub Releases returned status {}",
            api.status.as_u16()
        ));
    }

    let atom = fetch(ReleaseSource::Atom).await?;
    if atom.status != reqwest::StatusCode::OK {
        return Err(format!(
            "GitHub Releases feed returned status {}",
            atom.status.as_u16()
        ));
    }
    select_atom_release(&atom.body, current)
}

async fn discover_release(current: &Version) -> Result<Option<ReleaseCandidate>, String> {
    if cfg!(debug_assertions) {
        return Ok(None);
    }
    let client = release_http_client()?;
    discover_release_with_fetch(current, |source| {
        let client = client.clone();
        async move { fetch_release_source(&client, source).await }
    })
    .await
}

const MAX_UPDATER_REDIRECTS: usize = 10;

fn can_follow_updater_redirect(url: &tauri::Url, previous_redirects: usize) -> bool {
    previous_redirects < MAX_UPDATER_REDIRECTS && is_allowed_github_redirect(url)
}

fn updater_redirect_policy() -> reqwest_updater::redirect::Policy {
    reqwest_updater::redirect::Policy::custom(|attempt| {
        if attempt.previous().len() >= MAX_UPDATER_REDIRECTS {
            attempt.error("too many updater redirects")
        } else if can_follow_updater_redirect(attempt.url(), attempt.previous().len()) {
            attempt.follow()
        } else {
            attempt.stop()
        }
    })
}

fn ensure_rustls_crypto_provider() {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
}

async fn fetch_release_manifest(url: &str) -> Result<Vec<u8>, String> {
    ensure_rustls_crypto_provider();
    let builder = reqwest_updater::Client::builder()
        .user_agent(UPDATE_USER_AGENT)
        .redirect(updater_redirect_policy())
        .connect_timeout(UPDATE_CONNECT_TIMEOUT)
        .timeout(ATTESTATION_DOWNLOAD_TIMEOUT);
    let client = updater_http_client_builder(builder)?
        .build()
        .map_err(|_| "could not initialize the update manifest client".to_string())?;
    let response = client
        .get(url)
        .header(reqwest_updater::header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|_| "could not download the signed update manifest".to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "signed update manifest returned status {}",
            response.status().as_u16()
        ));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_UPDATE_MANIFEST_BYTES as u64)
    {
        return Err("signed update manifest was too large".to_string());
    }
    let mut bytes = Vec::with_capacity(
        response
            .content_length()
            .unwrap_or_default()
            .min(MAX_UPDATE_MANIFEST_BYTES as u64) as usize,
    );
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| "could not read the signed update manifest".to_string())?;
        if bytes.len().saturating_add(chunk.len()) > MAX_UPDATE_MANIFEST_BYTES {
            return Err("signed update manifest was too large".to_string());
        }
        bytes.extend_from_slice(&chunk);
    }
    if bytes.is_empty() {
        return Err("signed update manifest was empty".to_string());
    }
    Ok(bytes)
}

fn update_http_client(
    update: &Update,
    timeout: std::time::Duration,
) -> Result<reqwest_updater::Client, String> {
    ensure_rustls_crypto_provider();
    let mut builder = reqwest_updater::Client::builder()
        .user_agent(UPDATE_USER_AGENT)
        .redirect(updater_redirect_policy())
        .connect_timeout(UPDATE_CONNECT_TIMEOUT)
        .timeout(timeout);
    if update.no_proxy {
        builder = builder.no_proxy();
    } else if let Some(proxy) = &update.proxy {
        builder = builder.proxy(configured_update_proxy(proxy)?);
    } else {
        builder = updater_http_client_builder(builder)?;
    }
    builder
        .build()
        .map_err(|_| "could not initialize the update download client".to_string())
}

fn update_request_headers(update: &Update) -> reqwest_updater::header::HeaderMap {
    let mut headers = update.headers.clone();
    if !headers.contains_key(reqwest_updater::header::ACCEPT) {
        headers.insert(
            reqwest_updater::header::ACCEPT,
            reqwest_updater::header::HeaderValue::from_static("application/octet-stream"),
        );
    }
    headers
}

async fn fetch_attestation(update: &Update, url: &str) -> Result<Vec<u8>, String> {
    let client = update_http_client(update, ATTESTATION_DOWNLOAD_TIMEOUT)?;
    let response = client
        .get(url)
        .headers(update_request_headers(update))
        .send()
        .await
        .map_err(|_| "could not download the signed update attestation".to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "signed update attestation returned status {}",
            response.status().as_u16()
        ));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_ATTESTATION_BYTES as u64)
    {
        return Err("signed update attestation was too large".to_string());
    }
    let mut bytes = Vec::with_capacity(
        response
            .content_length()
            .unwrap_or_default()
            .min(MAX_ATTESTATION_BYTES as u64) as usize,
    );
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk =
            chunk.map_err(|_| "could not read the signed update attestation".to_string())?;
        if bytes.len().saturating_add(chunk.len()) > MAX_ATTESTATION_BYTES {
            return Err("signed update attestation was too large".to_string());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

async fn download_verified_package(
    update: &Update,
    attestation: &UpdateAttestation,
    on_event: &Channel<UpdateInstallEvent>,
) -> Result<Vec<u8>, String> {
    let client = update_http_client(update, UPDATE_PACKAGE_DOWNLOAD_TIMEOUT)?;
    let response = client
        .get(update.download_url.clone())
        .headers(update_request_headers(update))
        .send()
        .await
        .map_err(|_| "signed update package download failed".to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "signed update package returned status {}",
            response.status().as_u16()
        ));
    }
    if response
        .content_length()
        .is_some_and(|length| length != attestation.size)
    {
        return Err(
            "update package Content-Length did not match its signed attestation".to_string(),
        );
    }

    let _ = on_event.send(UpdateInstallEvent::Started {
        content_length: Some(attestation.size),
    });
    let mut package = PackageAccumulator::new(attestation)?;
    let mut downloaded = 0_u64;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| "could not read the signed update package".to_string())?;
        package.push(&chunk)?;
        downloaded = downloaded.saturating_add(chunk.len() as u64);
        let _ = on_event.send(UpdateInstallEvent::Progress { downloaded });
    }
    let bytes = package.finish()?;
    verify_minisign(&bytes, &update.signature, "update package")?;
    Ok(bytes)
}

#[tauri::command]
pub async fn check_for_update<R: Runtime>(
    webview: Webview<R>,
    coordinator: tauri::State<'_, UpdateCoordinator>,
) -> Result<Option<UpdateMetadata>, String> {
    let lease = coordinator.begin_check_recovering_pending(|rid| {
        webview
            .resources_table()
            .close(rid)
            .map_err(|_| "stale update resource was already unavailable".to_string())
    })?;
    let current = webview.app_handle().package_info().version.clone();
    let Some(candidate) = discover_release(&current).await? else {
        return Ok(None);
    };
    let manifest = run_with_update_timeout(
        ATTESTATION_DOWNLOAD_TIMEOUT,
        fetch_release_manifest(&candidate.manifest_url),
    )
    .await?;
    let endpoint = LocalManifestEndpoint::start(manifest).await?;
    let updater = webview
        .updater_builder()
        .endpoints(vec![endpoint.url().clone()])
        .map_err(|_| "selected update manifest was rejected".to_string())?
        .timeout(std::time::Duration::from_secs(20))
        .configure_client(|client| {
            client
                .user_agent(UPDATE_USER_AGENT)
                .no_proxy()
                .redirect(reqwest_updater::redirect::Policy::none())
        })
        .build()
        .map_err(|_| "could not initialize signed update verification".to_string())?;
    let checked = updater.check().await;
    let Some(update) = (match checked {
        Ok(update) => {
            endpoint.finish().await?;
            update
        }
        Err(error) => return Err(format!("signed update check failed: {error}")),
    }) else {
        return Ok(None);
    };
    validate_checked_update(
        &update.version,
        update.download_url.as_str(),
        &update.signature,
        &candidate,
        &current,
    )?;
    let platform = current_update_platform(update.download_url.as_str(), &candidate.tag)?;
    let descriptor = manifest_attestation_descriptor(
        &update.raw_json,
        platform,
        update.download_url.as_str(),
        &update.signature,
        &candidate.tag,
    )?;
    let attestation_bytes = run_with_update_timeout(
        ATTESTATION_DOWNLOAD_TIMEOUT,
        fetch_attestation(&update, &descriptor.url),
    )
    .await?;
    verify_minisign(
        &attestation_bytes,
        &descriptor.signature,
        "update attestation",
    )?;
    let attestation: UpdateAttestation = serde_json::from_slice(&attestation_bytes)
        .map_err(|_| "signed update attestation JSON is invalid".to_string())?;
    validate_attestation(&attestation, &candidate, platform, &descriptor.asset_name)?;
    let metadata = UpdateMetadata {
        current_version: update.current_version.clone(),
        version: update.version.clone(),
        notes: candidate.notes,
        published_at: candidate.published_at,
        rid: webview.resources_table().add(PendingUpdate {
            update,
            attestation,
        }),
    };
    if let Err(error) = lease.publish_pending(metadata.rid) {
        let _ = webview.resources_table().close(metadata.rid);
        return Err(error);
    }
    Ok(Some(metadata))
}

#[tauri::command]
pub fn close_update<R: Runtime>(
    webview: Webview<R>,
    coordinator: tauri::State<'_, UpdateCoordinator>,
    rid: ResourceId,
) -> Result<(), String> {
    let close = coordinator.begin_close(rid)?;
    webview
        .resources_table()
        .close(rid)
        .map_err(|_| "update resource is no longer available".to_string())?;
    close.complete()
}

#[tauri::command]
pub async fn install_update<R: Runtime>(
    app: AppHandle<R>,
    webview: Webview<R>,
    coordinator: tauri::State<'_, UpdateCoordinator>,
    admission: tauri::State<'_, InstallAdmissionGate>,
    rid: ResourceId,
    on_event: Channel<UpdateInstallEvent>,
) -> Result<(), String> {
    let _lease = coordinator.begin_install(rid)?;
    let pending = webview
        .resources_table()
        .take::<PendingUpdate>(rid)
        .map_err(|_| "update resource is no longer available".to_string())?;
    let bytes = run_with_update_timeout(
        UPDATE_PACKAGE_DOWNLOAD_TIMEOUT,
        download_verified_package(&pending.update, &pending.attestation, &on_event),
    )
    .await?;
    let _ = on_event.send(UpdateInstallEvent::Installing);

    let export_state = app.state::<crate::export::ExportControl>();
    let core = app.state::<opentake_core::AppCore>();
    install_with_save_barriers(
        || {
            let generation =
                app.state::<std::sync::Arc<crate::generation::TauriGenerationBridge>>();
            if generation.has_active() {
                let cancelled = generation.cancel_all_active();
                return Err(format!(
                    "cancelled {cancelled} active generation job(s); retry installation after they stop"
                ));
            }
            let motion = app.state::<crate::motion::MotionCommandState>();
            if motion.has_active() {
                let _ = motion.cancel_active();
                return Err(
                    "cancelled an active motion render; retry installation after it stops"
                        .to_string(),
                );
            }
            if app
                .state::<crate::advanced::AdvancedWorkflowCommandState>()
                .cancel_active()
            {
                return Err(
                    "cancelled an active advanced workflow; retry installation after it stops"
                        .to_string(),
                );
            }
            if app
                .state::<crate::advanced::MattingModelInstallState>()
                .cancel_active()
            {
                return Err(
                    "cancelled an active model download; retry installation after it stops"
                        .to_string(),
                );
            }
            let install_admission = admission.begin_install()?;
            let export_guard = export_state
                .try_begin("app-update-install")
                .map_err(|_| "finish or cancel the active export before installing".to_string())?;

            #[cfg(feature = "playback-engine")]
            {
                let playback = app.state::<crate::playback::PlaybackState>();
                let transition = playback.begin_project_transition().map_err(|error| {
                    format!("could not stop playback before installation: {error}")
                })?;
                playback.activate_project(
                    transition,
                    app.state::<opentake_core::AppCore>()
                        .project_revision()
                        .project_epoch,
                );
            }

            // This is the mandatory cross-platform save barrier. On Windows,
            // the official updater launches its installer and exits inside
            // Update::install, so no code after install can be relied upon.
            save_current_project(&core).map_err(|error| {
                format!("project save failed; update was not installed: {error}")
            })?;
            Ok((install_admission, export_guard))
        },
        |_readiness| {
            pending
                .update
                .install(&bytes)
                .map_err(|error| format!("update installation failed: {error}"))
        },
        || {
            // macOS/Linux return from install. Take a second fresh epoch/path
            // snapshot before restart to protect against any platform event
            // that landed at the installation boundary. Windows exits inside
            // Update::install and therefore relies on the barrier above.
            #[cfg(not(target_os = "windows"))]
            save_current_project(&core).map_err(|error| {
                format!(
                    "project save failed after update installation; OpenTake was not restarted: {error}"
                )
            })?;
            Ok(())
        },
        || {
            let _ = on_event.send(UpdateInstallEvent::Restarting);
            app.restart();
        },
    )
}

fn open_update_releases_with<E>(open: impl FnOnce(&str) -> Result<(), E>) -> Result<(), String>
where
    E: std::fmt::Display,
{
    open(UPDATE_RELEASES_URL)
        .map_err(|error| format!("could not open the OpenTake Releases page: {error}"))
}

/// Open the one fixed manual-update recovery page in the user's default browser.
/// The frontend cannot supply a URL, so this command cannot become a general
/// native URL launcher if renderer content is compromised.
#[tauri::command]
pub fn open_update_releases() -> Result<(), String> {
    open_update_releases_with(open_url_in_default_browser)
}

fn open_url_in_default_browser(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    let mut child = std::process::Command::new("open").arg(url).spawn()?;

    #[cfg(target_os = "windows")]
    let mut child = std::process::Command::new("cmd.exe")
        .args(["/C", "start", "", url])
        .spawn()?;

    #[cfg(target_os = "linux")]
    let mut child = std::process::Command::new("xdg-open").arg(url).spawn()?;

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    return Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "opening a browser is unsupported on this platform",
    ));

    // Reap the short-lived launcher without delaying the UI command. The
    // browser itself is detached by the platform launcher.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Barrier};

    const RELEASES: &str = r#"[
      {
        "tag_name": "v1.0.0-beta.2",
        "draft": false,
        "prerelease": true,
        "body": "old",
        "published_at": "2026-08-01T00:00:00Z",
        "assets": [{
          "name": "updater-v1.0.0-beta.2.json",
          "browser_download_url": "https://github.com/appergb/OpenTake/releases/download/v1.0.0-beta.2/updater-v1.0.0-beta.2.json"
        }]
      },
      {
        "tag_name": "v1.0.0-beta.4",
        "draft": false,
        "prerelease": true,
        "body": "playback fixes",
        "published_at": "2026-08-10T00:00:00Z",
        "assets": [{
          "name": "updater-v1.0.0-beta.4.json",
          "browser_download_url": "https://github.com/appergb/OpenTake/releases/download/v1.0.0-beta.4/updater-v1.0.0-beta.4.json"
        }]
      },
      {
        "tag_name": "v9.0.0-beta.1",
        "draft": true,
        "prerelease": true,
        "body": "must never ship",
        "published_at": "2026-08-11T00:00:00Z",
        "assets": [{
          "name": "updater-v9.0.0-beta.1.json",
          "browser_download_url": "https://github.com/appergb/OpenTake/releases/download/v9.0.0-beta.1/updater-v9.0.0-beta.1.json"
        }]
      }
    ]"#;

    const RELEASES_ATOM: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
      <feed xmlns="http://www.w3.org/2005/Atom">
        <id>tag:github.com,2008:https://github.com/appergb/OpenTake/releases</id>
        <link type="application/atom+xml" rel="self" href="https://github.com/appergb/OpenTake/releases.atom"/>
        <entry>
          <id>tag:github.com,2008:Repository/1275692189/v1.0.0-beta.2</id>
          <updated>2026-08-01T00:00:00Z</updated>
          <link rel="alternate" type="text/html" href="https://github.com/appergb/OpenTake/releases/tag/v1.0.0-beta.2"/>
          <title>OpenTake 1.0.0-beta.2</title>
          <content type="html">ignored release notes</content>
        </entry>
        <entry>
          <id>tag:github.com,2008:Repository/1275692189/v1.0.0-beta.4</id>
          <updated>2026-08-10T00:00:00Z</updated>
          <link rel="alternate" type="text/html" href="https://github.com/appergb/OpenTake/releases/tag/v1.0.0-beta.4"/>
          <title>OpenTake 1.0.0-beta.4</title>
          <content type="html">must not be treated as trusted markup</content>
        </entry>
      </feed>"#;

    fn attestation_for_beta4() -> UpdateAttestation {
        UpdateAttestation {
            schema_version: 1,
            repository: GITHUB_REPOSITORY.to_string(),
            tag: "v1.0.0-beta.4".to_string(),
            version: "1.0.0-beta.4".to_string(),
            source_sha: "a".repeat(40),
            platform: "darwin-aarch64".to_string(),
            asset_name: "OpenTake_aarch64.app.tar.gz".to_string(),
            size: 11,
            sha256: "3c98512128be7913714607df6a5f95a8e7056ce3960c573394bfd0f08de2a0eb".to_string(),
        }
    }

    #[test]
    fn release_discovery_selects_highest_semver_prerelease_from_tag_specific_manifest() {
        let current = Version::parse("1.0.0-beta.3").unwrap();
        let release = select_release(RELEASES.as_bytes(), &current)
            .unwrap()
            .expect("beta.4 should be available");

        assert_eq!(release.tag, "v1.0.0-beta.4");
        assert_eq!(release.version, Version::parse("1.0.0-beta.4").unwrap());
        assert_eq!(
            release.manifest_url,
            "https://github.com/appergb/OpenTake/releases/download/v1.0.0-beta.4/updater-v1.0.0-beta.4.json"
        );
        assert_eq!(release.notes.as_deref(), Some("playback fixes"));
    }

    #[test]
    fn official_atom_feed_selects_a_canonical_release_without_trusting_html_notes() {
        let current = Version::parse("1.0.0-beta.3").unwrap();
        let release = select_atom_release(RELEASES_ATOM.as_bytes(), &current)
            .unwrap()
            .expect("beta.4 should be available from the official feed");

        assert_eq!(release.tag, "v1.0.0-beta.4");
        assert_eq!(release.version, Version::parse("1.0.0-beta.4").unwrap());
        assert_eq!(
            release.manifest_url,
            "https://github.com/appergb/OpenTake/releases/download/v1.0.0-beta.4/updater-v1.0.0-beta.4.json"
        );
        assert_eq!(release.notes, None);
        assert_eq!(
            release.published_at.as_deref(),
            Some("2026-08-10T00:00:00Z")
        );
    }

    #[tokio::test]
    async fn anonymous_api_rate_limit_falls_back_to_the_official_atom_feed() {
        let sources = std::sync::Mutex::new(Vec::new());
        let release =
            discover_release_with_fetch(&Version::parse("1.0.0-beta.3").unwrap(), |source| {
                sources.lock().unwrap().push(source);
                std::future::ready(Ok(match source {
                    ReleaseSource::Api => ReleaseSourceResponse {
                        status: reqwest::StatusCode::FORBIDDEN,
                        body: Vec::new(),
                    },
                    ReleaseSource::Atom => ReleaseSourceResponse {
                        status: reqwest::StatusCode::OK,
                        body: RELEASES_ATOM.as_bytes().to_vec(),
                    },
                }))
            })
            .await
            .unwrap()
            .expect("the feed should discover beta.4 after the API is rate limited");

        assert_eq!(release.tag, "v1.0.0-beta.4");
        assert_eq!(
            *sources.lock().unwrap(),
            [ReleaseSource::Api, ReleaseSource::Atom]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn transient_release_transport_failures_retry_within_a_fixed_budget() {
        let attempts = std::sync::atomic::AtomicUsize::new(0);
        let value = retry_transient(
            &[
                std::time::Duration::from_millis(100),
                std::time::Duration::from_millis(200),
            ],
            || {
                let attempt = attempts.fetch_add(1, Ordering::SeqCst);
                std::future::ready(if attempt < 2 {
                    Err("transient")
                } else {
                    Ok("reached GitHub")
                })
            },
        )
        .await
        .unwrap();

        assert_eq!(value, "reached GitHub");
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn atom_feed_rejects_cross_repository_and_credential_bearing_release_links() {
        for malicious in [
            RELEASES_ATOM.replace(
                "https://github.com/appergb/OpenTake/releases/tag/v1.0.0-beta.4",
                "https://github.com/other/OpenTake/releases/tag/v1.0.0-beta.4",
            ),
            RELEASES_ATOM.replace(
                "https://github.com/appergb/OpenTake/releases/tag/v1.0.0-beta.4",
                "https://alice:secret@github.com/appergb/OpenTake/releases/tag/v1.0.0-beta.4",
            ),
        ] {
            assert!(select_atom_release(
                malicious.as_bytes(),
                &Version::parse("1.0.0-beta.3").unwrap(),
            )
            .is_err());
        }
    }

    #[test]
    fn atom_feed_rejects_entries_outside_its_single_document_root() {
        let rogue = r#"<wrapper>
          <entry>
            <updated>2026-08-11T00:00:00Z</updated>
            <link rel="alternate" type="text/html" href="https://github.com/appergb/OpenTake/releases/tag/v9.0.0-beta.1"/>
          </entry>
        </wrapper>"#;
        let before = RELEASES_ATOM.replacen("<feed ", &format!("{rogue}<feed "), 1);
        let after = RELEASES_ATOM.replacen("</feed>", &format!("</feed>{rogue}"), 1);

        for malicious in [before, after] {
            assert!(select_atom_release(
                malicious.as_bytes(),
                &Version::parse("1.0.0-beta.3").unwrap(),
            )
            .is_err());
        }
    }

    #[test]
    fn explicit_https_proxy_prefers_environment_and_preserves_no_proxy() {
        let environment = std::collections::HashMap::from([
            ("HTTPS_PROXY", "http://127.0.0.1:1082"),
            ("https_proxy", "http://ignored.example:8080"),
            ("ALL_PROXY", "https://ignored.example:8443"),
            ("NO_PROXY", "localhost,127.0.0.1,::1"),
        ]);
        let proxy = environment_https_proxy_from(|name| {
            environment.get(name).map(std::ffi::OsString::from)
        })
        .unwrap()
        .expect("HTTPS_PROXY should be selected");

        assert_eq!(proxy.url.as_str(), "http://127.0.0.1:1082/");
        assert_eq!(proxy.no_proxy.as_deref(), Some("localhost,127.0.0.1,::1"));
        assert!(environment_https_proxy_from(|_| None).unwrap().is_none());
    }

    #[test]
    fn explicit_proxy_rejects_unsupported_schemes_and_credentials_without_echoing_them() {
        for invalid in [
            "socks5://127.0.0.1:1082",
            "127.0.0.1:1082",
            "http://alice:secret@127.0.0.1:1082",
            "https://127.0.0.1:1082/path",
            "https://127.0.0.1:1082/?token=secret",
        ] {
            let error = environment_https_proxy_from(|name| {
                (name == "HTTPS_PROXY").then(|| std::ffi::OsString::from(invalid))
            })
            .unwrap_err();
            assert_eq!(error, "update proxy configuration is invalid");
            for secret in ["alice", "secret", "token"] {
                assert!(!error.contains(secret));
            }
        }
    }

    #[tokio::test]
    async fn release_client_identifies_itself_on_the_connect_handshake() {
        use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let proxy_url = tauri::Url::parse(&format!(
            "http://127.0.0.1:{}",
            listener.local_addr().unwrap().port()
        ))
        .unwrap();
        let captured = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            loop {
                let mut chunk = [0_u8; 1024];
                let read = stream.read(&mut chunk).await.unwrap();
                if read == 0 || request.len().saturating_add(read) > 8 * 1024 {
                    break;
                }
                request.extend_from_slice(&chunk[..read]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            stream
                .write_all(
                    b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await
                .unwrap();
            String::from_utf8(request).unwrap()
        });
        let client = release_http_client_with_proxy(Some(ExplicitHttpsProxy {
            url: proxy_url,
            no_proxy: None,
            is_https_specific: true,
        }))
        .unwrap();

        assert!(client.get(GITHUB_RELEASES_ATOM).send().await.is_err());
        let request = captured.await.unwrap();
        assert!(request.starts_with("CONNECT github.com:443 HTTP/1.1\r\n"));
        assert!(request.contains(
            "\r\nuser-agent: OpenTake-Updater/1.0 (+https://github.com/appergb/OpenTake)\r\n"
        ));
    }

    #[test]
    fn release_discovery_rejects_bad_json_same_version_downgrade_and_missing_manifest() {
        let current = Version::parse("1.0.0-beta.4").unwrap();
        assert!(select_release(b"{}", &current).is_err());
        assert_eq!(select_release(RELEASES.as_bytes(), &current).unwrap(), None);

        let missing = br#"[{
          "tag_name":"v1.0.0-beta.5","draft":false,"prerelease":true,
          "assets":[{"name":"wrong.json","browser_download_url":"https://github.com/appergb/OpenTake/releases/download/v1.0.0-beta.5/wrong.json"}]
        }]"#;
        assert_eq!(select_release(missing, &current).unwrap(), None);
    }

    #[test]
    fn current_beta_without_an_updater_manifest_is_still_up_to_date() {
        let current_release_without_manifest = br#"[{
          "tag_name":"v1.0.0-beta.3","draft":false,"prerelease":true,
          "assets":[]
        }]"#;
        assert_eq!(
            select_release(
                current_release_without_manifest,
                &Version::parse("1.0.0-beta.3").unwrap(),
            )
            .unwrap(),
            None
        );
    }

    #[test]
    fn ordinary_builds_do_not_require_signing_secrets() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(config["bundle"]["createUpdaterArtifacts"], false);
        assert_eq!(
            config["plugins"]["updater"]["dangerousInsecureTransportProtocol"], true,
            "the official updater only consumes the bounded loopback replay in release builds"
        );
        assert!(config["plugins"]["updater"].get("endpoints").is_none());
        assert!(config["plugins"]["updater"]["pubkey"]
            .as_str()
            .is_some_and(|key| !key.trim().is_empty()));
        updater_public_key().expect("the embedded minisign public key must decode");
    }

    #[test]
    fn stable_build_never_moves_to_a_prerelease_channel() {
        let releases = br#"[{
          "tag_name":"v2.0.0-beta.1","draft":false,"prerelease":true,
          "assets":[{"name":"updater-v2.0.0-beta.1.json","browser_download_url":"https://github.com/appergb/OpenTake/releases/download/v2.0.0-beta.1/updater-v2.0.0-beta.1.json"}]
        }]"#;
        assert_eq!(
            select_release(releases, &Version::parse("1.0.0").unwrap()).unwrap(),
            None
        );
    }

    #[test]
    fn update_urls_are_https_and_pinned_to_the_selected_repository_and_tag() {
        assert_eq!(
            manifest_endpoint("v1.0.0-beta.4").unwrap(),
            "https://github.com/appergb/OpenTake/releases/download/v1.0.0-beta.4/updater-v1.0.0-beta.4.json"
        );
        validate_download_url(
            "https://github.com/appergb/OpenTake/releases/download/v1.0.0-beta.4/OpenTake_aarch64.app.tar.gz",
            "v1.0.0-beta.4",
        )
        .unwrap();

        for malicious in [
            "http://github.com/appergb/OpenTake/releases/download/v1.0.0-beta.4/payload",
            "https://github.com/other/OpenTake/releases/download/v1.0.0-beta.4/payload",
            "https://github.com/appergb/OpenTake/releases/download/v1.0.0-beta.5/payload",
            "https://github.com/appergb/OpenTake/releases/download/v1.0.0-beta.4/../payload",
            "https://github.com@evil.example/appergb/OpenTake/releases/download/v1.0.0-beta.4/payload",
            "https://github.com/appergb/OpenTake/releases/download/v1.0.0-beta.4/payload?next=https://evil.example",
        ] {
            assert!(validate_download_url(malicious, "v1.0.0-beta.4").is_err(), "{malicious}");
        }
        assert!(manifest_endpoint("../../latest").is_err());
    }

    #[test]
    fn redirects_are_limited_to_https_github_release_cdn_hosts() {
        for allowed in [
            "https://github.com/appergb/OpenTake/releases/download/v1.0.0-beta.4/file",
            "https://objects.githubusercontent.com/github-production-release-asset/file",
            "https://release-assets.githubusercontent.com/github-production-release-asset/file",
        ] {
            assert!(is_allowed_github_redirect(
                &tauri::Url::parse(allowed).unwrap()
            ));
        }
        for denied in [
            "http://release-assets.githubusercontent.com/file",
            "https://api.github.com.evil.example/file",
            "https://githubusercontent.com/file",
            "https://127.0.0.1/file",
        ] {
            assert!(!is_allowed_github_redirect(
                &tauri::Url::parse(denied).unwrap()
            ));
        }

        let allowed = tauri::Url::parse(
            "https://release-assets.githubusercontent.com/github-production-release-asset/file",
        )
        .unwrap();
        assert!(can_follow_updater_redirect(
            &allowed,
            MAX_UPDATER_REDIRECTS - 1
        ));
        assert!(!can_follow_updater_redirect(
            &allowed,
            MAX_UPDATER_REDIRECTS
        ));
    }

    #[test]
    fn checked_manifest_cannot_swap_version_url_or_signature() {
        let current = Version::parse("1.0.0-beta.3").unwrap();
        let candidate = select_release(RELEASES.as_bytes(), &current)
            .unwrap()
            .unwrap();
        let package = "https://github.com/appergb/OpenTake/releases/download/v1.0.0-beta.4/OpenTake_aarch64.app.tar.gz";
        validate_checked_update(
            "1.0.0-beta.4",
            package,
            "trusted-signature",
            &candidate,
            &current,
        )
        .unwrap();

        assert!(validate_checked_update(
            "1.0.0-beta.3",
            package,
            "trusted-signature",
            &candidate,
            &current,
        )
        .is_err());
        assert!(validate_checked_update(
            "1.0.0-beta.4",
            "https://github.com/other/OpenTake/releases/download/v1.0.0-beta.4/file",
            "trusted-signature",
            &candidate,
            &current,
        )
        .is_err());
        assert!(
            validate_checked_update("1.0.0-beta.4", package, "", &candidate, &current).is_err()
        );
    }

    #[test]
    fn signed_attestation_rejects_historic_package_replay_under_a_higher_tag() {
        let current = Version::parse("1.0.0-beta.3").unwrap();
        let candidate = select_release(RELEASES.as_bytes(), &current)
            .unwrap()
            .unwrap();
        let mut replay = attestation_for_beta4();
        replay.tag = "v1.0.0-beta.3".to_string();
        replay.version = "1.0.0-beta.3".to_string();
        replay.source_sha = "b".repeat(40);

        assert_eq!(
            validate_attestation(
                &replay,
                &candidate,
                "darwin-aarch64",
                "OpenTake_aarch64.app.tar.gz",
            )
            .unwrap_err(),
            "update attestation does not match the selected release"
        );
    }

    #[test]
    fn attestation_schema_is_strict_and_binds_manifest_platform_and_asset() {
        let current = Version::parse("1.0.0-beta.3").unwrap();
        let candidate = select_release(RELEASES.as_bytes(), &current)
            .unwrap()
            .unwrap();
        validate_attestation(
            &attestation_for_beta4(),
            &candidate,
            "darwin-aarch64",
            "OpenTake_aarch64.app.tar.gz",
        )
        .unwrap();

        let mut malformed = serde_json::to_value(attestation_for_beta4()).unwrap();
        malformed["unexpected"] = serde_json::json!(true);
        assert!(serde_json::from_value::<UpdateAttestation>(malformed).is_err());

        let mut wrong_platform = attestation_for_beta4();
        wrong_platform.platform = "windows-x86_64".to_string();
        assert!(validate_attestation(
            &wrong_platform,
            &candidate,
            "darwin-aarch64",
            "OpenTake_aarch64.app.tar.gz",
        )
        .is_err());
    }

    #[test]
    fn extended_manifest_requires_exact_attestation_url_and_bounded_signature() {
        let package = "https://github.com/appergb/OpenTake/releases/download/v1.0.0-beta.4/OpenTake_aarch64.app.tar.gz";
        let raw = serde_json::json!({
            "version": "1.0.0-beta.4",
            "platforms": {
                "darwin-aarch64": {
                    "url": package,
                    "signature": "package-signature",
                    "attestationUrl": format!("{package}.attestation.json"),
                    "attestationSignature": "attestation-signature",
                }
            }
        });
        let descriptor = manifest_attestation_descriptor(
            &raw,
            "darwin-aarch64",
            package,
            "package-signature",
            "v1.0.0-beta.4",
        )
        .unwrap();
        assert_eq!(descriptor.url, format!("{package}.attestation.json"));

        let mut swapped = raw;
        swapped["platforms"]["darwin-aarch64"]["attestationUrl"] =
            serde_json::json!("https://github.com/appergb/OpenTake/releases/download/v1.0.0-beta.4/other.attestation.json");
        assert!(manifest_attestation_descriptor(
            &swapped,
            "darwin-aarch64",
            package,
            "package-signature",
            "v1.0.0-beta.4",
        )
        .is_err());
    }

    #[test]
    fn windows_manifest_selects_the_installed_bundle_type_without_generic_fallback() {
        use tauri::utils::config::BundleType;

        assert_eq!(
            update_platform_for_bundle("windows", "x86_64", Some(BundleType::Msi)).unwrap(),
            "windows-x86_64-msi"
        );
        assert_eq!(
            update_platform_for_bundle("windows", "x86_64", Some(BundleType::Nsis)).unwrap(),
            "windows-x86_64-nsis"
        );
        assert!(update_platform_for_bundle("windows", "x86_64", None).is_err());
        assert!(update_platform_for_bundle("windows", "aarch64", Some(BundleType::Msi)).is_err());
    }

    #[test]
    fn windows_manifest_rejects_generic_and_cross_installer_package_bindings() {
        let msi =
            "https://github.com/appergb/OpenTake/releases/download/v1.0.0-beta.4/OpenTake_x64.msi";
        let nsis = "https://github.com/appergb/OpenTake/releases/download/v1.0.0-beta.4/OpenTake_x64-setup.exe";
        let raw = serde_json::json!({
            "version": "1.0.0-beta.4",
            "platforms": {
                "windows-x86_64-msi": {
                    "url": msi,
                    "signature": "msi-signature",
                    "attestationUrl": format!("{msi}.attestation.json"),
                    "attestationSignature": "msi-attestation-signature",
                },
                "windows-x86_64-nsis": {
                    "url": nsis,
                    "signature": "nsis-signature",
                    "attestationUrl": format!("{nsis}.attestation.json"),
                    "attestationSignature": "nsis-attestation-signature",
                }
            }
        });

        manifest_attestation_descriptor(
            &raw,
            "windows-x86_64-msi",
            msi,
            "msi-signature",
            "v1.0.0-beta.4",
        )
        .unwrap();
        manifest_attestation_descriptor(
            &raw,
            "windows-x86_64-nsis",
            nsis,
            "nsis-signature",
            "v1.0.0-beta.4",
        )
        .unwrap();

        let mut cross_installer = raw.clone();
        cross_installer["platforms"]["windows-x86_64-nsis"] =
            cross_installer["platforms"]["windows-x86_64-msi"].clone();
        assert!(manifest_attestation_descriptor(
            &cross_installer,
            "windows-x86_64-nsis",
            msi,
            "msi-signature",
            "v1.0.0-beta.4",
        )
        .is_err());

        let mut generic = raw;
        generic["platforms"]["windows-x86_64"] =
            generic["platforms"]["windows-x86_64-nsis"].clone();
        assert!(manifest_attestation_descriptor(
            &generic,
            "windows-x86_64",
            nsis,
            "nsis-signature",
            "v1.0.0-beta.4",
        )
        .is_err());
    }

    #[test]
    fn manual_release_fallback_opens_only_the_fixed_official_page() {
        let mut opened = None;
        open_update_releases_with(|url| {
            opened = Some(url.to_string());
            Ok::<(), &'static str>(())
        })
        .unwrap();
        assert_eq!(opened.as_deref(), Some(UPDATE_RELEASES_URL));

        assert_eq!(
            open_update_releases_with(|_| Err::<(), _>("no default browser")).unwrap_err(),
            "could not open the OpenTake Releases page: no default browser"
        );
    }

    #[test]
    fn package_accumulator_enforces_attested_size_and_sha256() {
        let attestation = attestation_for_beta4();
        let mut valid = PackageAccumulator::new(&attestation).unwrap();
        valid.push(b"new ").unwrap();
        valid.push(b"package").unwrap();
        assert_eq!(valid.finish().unwrap(), b"new package");

        let mut replayed = PackageAccumulator::new(&attestation).unwrap();
        replayed.push(b"old package").unwrap();
        assert_eq!(
            replayed.finish().unwrap_err(),
            "downloaded update package SHA-256 did not match its signed attestation"
        );

        let mut oversized = PackageAccumulator::new(&attestation).unwrap();
        assert!(oversized.push(b"new package!").is_err());
    }

    #[tokio::test]
    async fn bounded_update_future_times_out_instead_of_hanging() {
        let result = run_with_update_timeout(
            std::time::Duration::from_millis(1),
            std::future::pending::<Result<(), String>>(),
        )
        .await;
        assert_eq!(result.unwrap_err(), "update download timed out");
    }

    #[tokio::test]
    async fn bounded_manifest_is_replayed_once_from_an_exact_tokenized_loopback_endpoint() {
        let manifest = br#"{"version":"1.0.0-beta.4","platforms":{}}"#.to_vec();
        let endpoint = LocalManifestEndpoint::start(manifest.clone())
            .await
            .unwrap();
        let exact_url = endpoint.url().clone();
        let mut wrong_url = exact_url.clone();
        wrong_url.set_path("/wrong-token");
        ensure_rustls_crypto_provider();
        let client = reqwest_updater::Client::builder()
            .no_proxy()
            .redirect(reqwest_updater::redirect::Policy::none())
            .build()
            .unwrap();

        assert_eq!(client.get(wrong_url).send().await.unwrap().status(), 404);
        assert_eq!(
            client
                .get(exact_url)
                .send()
                .await
                .unwrap()
                .bytes()
                .await
                .unwrap(),
            manifest
        );
        endpoint.finish().await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn local_manifest_connection_uses_one_absolute_deadline_for_drip_fed_headers() {
        use tokio::io::AsyncWriteExt as _;

        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let (client, accepted) =
            tokio::join!(tokio::net::TcpStream::connect(address), listener.accept());
        let mut client = client.unwrap();
        let (stream, _) = accepted.unwrap();
        client.write_all(b"G").await.unwrap();
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(20);
        let server = tokio::spawn(async move {
            serve_manifest_connection_until(stream, "/manifest-token.json", b"{}", deadline).await
        });

        // Stay below the old per-read two-second timeout while crossing the
        // endpoint's single twenty-second lifetime.
        for _ in 0..19 {
            tokio::time::advance(std::time::Duration::from_secs(1)).await;
            tokio::task::yield_now().await;
            if client.write_all(b"G").await.is_err() {
                break;
            }
            tokio::task::yield_now().await;
        }
        tokio::time::advance(std::time::Duration::from_secs(2)).await;
        tokio::task::yield_now().await;

        assert_eq!(
            server.await.unwrap().unwrap_err(),
            "local update verifier timed out"
        );
    }

    #[tokio::test]
    async fn local_manifest_endpoint_refuses_a_body_above_the_remote_cap() {
        assert!(
            LocalManifestEndpoint::start(vec![0; MAX_UPDATE_MANIFEST_BYTES + 1])
                .await
                .is_err()
        );
    }

    #[test]
    fn coordinator_serializes_checks_install_and_exact_resource_close() {
        let coordinator = UpdateCoordinator::default();
        let check = coordinator.begin_check().unwrap();
        assert_eq!(coordinator.phase(), OperationPhase::Checking);
        assert!(coordinator.begin_check().is_err());
        check.publish_pending(41).unwrap();
        assert_eq!(coordinator.phase(), OperationPhase::Pending { rid: 41 });

        assert!(coordinator.begin_install(99).is_err());
        assert!(coordinator.begin_close(99).is_err());
        let install = coordinator.begin_install(41).unwrap();
        assert_eq!(coordinator.phase(), OperationPhase::Installing { rid: 41 });
        assert!(coordinator.prevents_user_exit());
        assert!(coordinator.begin_check().is_err());
        drop(install);
        assert!(!coordinator.prevents_user_exit());
        assert_eq!(coordinator.phase(), OperationPhase::Idle);
    }

    #[test]
    fn install_admission_never_overlaps_a_new_generation_motion_or_advanced_activity() {
        for _ in 0..100 {
            let gate = Arc::new(InstallAdmissionGate::default());
            let start = Arc::new(Barrier::new(3));
            let attempted = Arc::new(Barrier::new(3));

            let activity_thread = {
                let gate = Arc::clone(&gate);
                let start = Arc::clone(&start);
                let attempted = Arc::clone(&attempted);
                std::thread::spawn(move || {
                    start.wait();
                    let lease = gate.begin_activity();
                    attempted.wait();
                    lease.is_ok()
                })
            };
            let install_thread = {
                let gate = Arc::clone(&gate);
                let start = Arc::clone(&start);
                let attempted = Arc::clone(&attempted);
                std::thread::spawn(move || {
                    start.wait();
                    let lease = gate.begin_install();
                    attempted.wait();
                    lease.is_ok()
                })
            };

            start.wait();
            attempted.wait();
            assert_ne!(
                activity_thread.join().unwrap(),
                install_thread.join().unwrap(),
                "exactly one admission claimant must win the race"
            );
        }
    }

    #[test]
    fn install_admission_reopens_after_each_lease_drops() {
        let gate = InstallAdmissionGate::default();
        let activity = gate.begin_activity().unwrap();
        assert!(gate.begin_install().is_err());
        drop(activity);

        let install = gate.begin_install().unwrap();
        assert!(gate.begin_activity().is_err());
        drop(install);
        assert!(gate.begin_activity().is_ok());
    }

    #[test]
    fn abandoned_check_and_successfully_closed_pending_resource_return_to_idle() {
        let coordinator = UpdateCoordinator::default();
        drop(coordinator.begin_check().unwrap());
        assert_eq!(coordinator.phase(), OperationPhase::Idle);

        coordinator
            .begin_check()
            .unwrap()
            .publish_pending(7)
            .unwrap();
        coordinator.begin_close(7).unwrap().complete().unwrap();
        assert_eq!(coordinator.phase(), OperationPhase::Idle);
    }

    #[test]
    fn webview_reload_recheck_closes_the_exact_pending_rid_and_recovers_idle() {
        let coordinator = UpdateCoordinator::default();
        coordinator
            .begin_check()
            .unwrap()
            .publish_pending(73)
            .unwrap();
        let mut closed = None;

        let recheck = coordinator
            .begin_check_recovering_pending(|rid| {
                closed = Some(rid);
                Ok(())
            })
            .unwrap();
        assert_eq!(closed, Some(73));
        assert_eq!(coordinator.phase(), OperationPhase::Checking);
        drop(recheck);
        assert_eq!(coordinator.phase(), OperationPhase::Idle);

        coordinator
            .begin_check()
            .unwrap()
            .publish_pending(91)
            .unwrap();
        let recheck = coordinator
            .begin_check_recovering_pending(|rid| {
                assert_eq!(rid, 91);
                Err("resource table was replaced by a reload".to_string())
            })
            .expect("a missing stale resource must not leave the coordinator permanently busy");
        drop(recheck);
        assert_eq!(coordinator.phase(), OperationPhase::Idle);
    }

    #[test]
    fn failed_resource_close_retains_the_exact_pending_rid_for_retry() {
        let coordinator = UpdateCoordinator::default();
        coordinator
            .begin_check()
            .unwrap()
            .publish_pending(7)
            .unwrap();

        let failed_close = coordinator.begin_close(7).unwrap();
        assert_eq!(coordinator.phase(), OperationPhase::Closing { rid: 7 });
        drop(failed_close);
        assert_eq!(coordinator.phase(), OperationPhase::Pending { rid: 7 });

        coordinator.begin_close(7).unwrap().complete().unwrap();
        assert_eq!(coordinator.phase(), OperationPhase::Idle);
    }

    #[test]
    fn failed_save_or_busy_preflight_never_calls_installer() {
        let installed = AtomicBool::new(false);
        let restarted = AtomicBool::new(false);
        let result = install_with_save_barriers(
            || Err("project save failed".to_string()),
            |&()| {
                installed.store(true, Ordering::SeqCst);
                Ok(())
            },
            || Ok(()),
            || restarted.store(true, Ordering::SeqCst),
        );

        assert_eq!(result.unwrap_err(), "project save failed");
        assert!(!installed.load(Ordering::SeqCst));
        assert!(!restarted.load(Ordering::SeqCst));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn returning_installer_saves_again_before_restart_and_save_failure_blocks_restart() {
        use std::cell::RefCell;

        let order = RefCell::new(Vec::new());
        install_with_save_barriers(
            || {
                order.borrow_mut().push("pre-save");
                Ok(())
            },
            |&()| {
                order.borrow_mut().push("install");
                Ok(())
            },
            || {
                order.borrow_mut().push("post-save");
                Ok(())
            },
            || order.borrow_mut().push("restart"),
        )
        .unwrap();
        assert_eq!(
            *order.borrow(),
            ["pre-save", "install", "post-save", "restart"]
        );

        order.borrow_mut().clear();
        let error = install_with_save_barriers(
            || {
                order.borrow_mut().push("pre-save");
                Ok(())
            },
            |&()| {
                order.borrow_mut().push("install");
                Ok(())
            },
            || {
                order.borrow_mut().push("post-save");
                Err("post-install project save failed".to_string())
            },
            || order.borrow_mut().push("restart"),
        )
        .unwrap_err();
        assert_eq!(error, "post-install project save failed");
        assert_eq!(*order.borrow(), ["pre-save", "install", "post-save"]);
    }
}
