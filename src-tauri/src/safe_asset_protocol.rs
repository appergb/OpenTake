//! Bounded, off-main-thread delivery of user-approved local media.
//!
//! Tauri's built-in asset protocol opens the path again after its scope check.
//! A File Provider update or hostile local replacement can therefore turn a
//! previously regular file into a FIFO, symlink, device, or cloud placeholder
//! and block the WebView/AppKit thread. This protocol opens off-thread with
//! no-recall/non-blocking platform flags, authorizes the retained handle's final
//! path, and serves only bounded bodies.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use http_range::HttpRange;
use opentake_core::{AppCore, ProjectAssetAuthority};
use opentake_project::{ProjectRoot, ProjectRootIdentity};
use percent_encoding::percent_decode;
use serde::{Deserialize, Serialize};
use tauri::http::header::{
    ACCEPT_RANGES, ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS,
    ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_EXPOSE_HEADERS, CONTENT_LENGTH, CONTENT_RANGE,
    CONTENT_TYPE, ETAG, IF_RANGE, RANGE, RETRY_AFTER,
};
use tauri::http::{Method, Request, Response, StatusCode};
use tauri::scope::fs::Scope;
use tauri::{AppHandle, Manager, Runtime, UriSchemeResponder};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::sync::Semaphore;

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
#[cfg(target_os = "macos")]
use std::os::{fd::AsRawFd, macos::fs::MetadataExt};

const MAX_CONCURRENT_READS: usize = 4;
const MAX_PENDING_READS: usize = 32;
const MAX_PATH_BYTES: usize = 32_768;
const MAX_FULL_BODY_BYTES: u64 = 32 * 1024 * 1024;
const MAX_FULL_IMAGE_BODY_BYTES: u64 = 128 * 1024 * 1024;
const MAX_RANGE_BYTES: u64 = 1_000 * 1024;
const IO_DEADLINE: Duration = Duration::from_secs(5);
const REAP_DEADLINE: Duration = Duration::from_secs(1);
const MAX_HELPER_REQUEST_BYTES: usize = 64 * 1024;
const MAX_HELPER_METADATA_BYTES: usize = 64 * 1024;
const MAX_HELPER_BODY_BYTES: usize = MAX_FULL_IMAGE_BODY_BYTES as usize;
const HELPER_ARG: &str = "--opentake-internal-safe-asset-helper-v1";
const HELPER_TOKEN_ENV: &str = "OPENTAKE_INTERNAL_ASSET_TOKEN";
const HELPER_PARENT_ENV: &str = "OPENTAKE_INTERNAL_ASSET_PARENT_PID";

static GLOBAL_PROCESS_SLOTS: OnceLock<Arc<Semaphore>> = OnceLock::new();

mod helper;

pub(crate) use helper::run_helper_if_requested;
use helper::{
    isolated_response_to_http, run_isolated_helper, HelperProjectAuthority, HelperRequest,
    IsolatedHelperError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
enum NonProjectAssetAuthority {
    ScopeOnly {
        requested_path: PathBuf,
        initial_final_path: PathBuf,
        initial_etag: String,
    },
    ProjectMedia {
        project_epoch: u64,
        requested_path: PathBuf,
        initial_final_path: PathBuf,
        initial_etag: String,
    },
}

impl NonProjectAssetAuthority {
    fn requested_path(&self) -> &Path {
        match self {
            Self::ScopeOnly { requested_path, .. } | Self::ProjectMedia { requested_path, .. } => {
                requested_path
            }
        }
    }

    fn initial_final_path(&self) -> &Path {
        match self {
            Self::ScopeOnly {
                initial_final_path, ..
            }
            | Self::ProjectMedia {
                initial_final_path, ..
            } => initial_final_path,
        }
    }

    fn initial_etag(&self) -> &str {
        match self {
            Self::ScopeOnly { initial_etag, .. } | Self::ProjectMedia { initial_etag, .. } => {
                initial_etag
            }
        }
    }
}

fn non_project_response_matches_authority(
    expected: &NonProjectAssetAuthority,
    final_path: &Path,
    response_etag: Option<&str>,
    refreshed: Option<&NonProjectAssetAuthority>,
) -> bool {
    paths_equal_for_authority(expected.initial_final_path(), final_path)
        && response_etag == Some(expected.initial_etag())
        && refreshed == Some(expected)
}

#[cfg(all(test, unix))]
use helper::{
    actual_parent_process_id, parent_is_same_executable, terminate_or_quarantine,
    write_helper_request_before_response, WireIoErrorKind,
};
#[cfg(test)]
use helper::{bounded_reap, helper_response};

#[derive(Clone)]
pub(crate) struct SafeAssetProtocol {
    worker_permits: Arc<Semaphore>,
    pending_permits: Arc<Semaphore>,
    process_slots: Arc<Semaphore>,
}

impl Default for SafeAssetProtocol {
    fn default() -> Self {
        Self {
            worker_permits: Arc::new(Semaphore::new(MAX_CONCURRENT_READS)),
            pending_permits: Arc::new(Semaphore::new(MAX_PENDING_READS)),
            process_slots: GLOBAL_PROCESS_SLOTS
                .get_or_init(|| Arc::new(Semaphore::new(MAX_CONCURRENT_READS)))
                .clone(),
        }
    }
}

impl SafeAssetProtocol {
    pub(crate) fn respond(
        &self,
        app: AppHandle,
        scope: Scope,
        request: Request<Vec<u8>>,
        responder: UriSchemeResponder,
    ) {
        let Ok(pending_permit) = self.pending_permits.clone().try_acquire_owned() else {
            responder.respond(error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "local asset workers are busy",
                Some((RETRY_AFTER, "1")),
            ));
            return;
        };
        let worker_permits = self.worker_permits.clone();
        let process_slots = self.process_slots.clone();
        tauri::async_runtime::spawn(async move {
            let worker_permit =
                match tokio::time::timeout(IO_DEADLINE, worker_permits.acquire_owned()).await {
                    Ok(Ok(permit)) => permit,
                    Ok(Err(_)) => {
                        responder.respond(error_response(
                            StatusCode::SERVICE_UNAVAILABLE,
                            "local asset service is shutting down",
                            None,
                        ));
                        return;
                    }
                    Err(_) => {
                        responder.respond(error_response(
                            StatusCode::GATEWAY_TIMEOUT,
                            "local asset worker queue timed out",
                            Some((RETRY_AFTER, "1")),
                        ));
                        return;
                    }
                };
            let _pending_permit = pending_permit;
            let _worker_permit = worker_permit;
            responder.respond(response_for_request(&app, &scope, request, process_slots).await);
        });
    }
}

async fn response_for_request<R: Runtime>(
    app: &AppHandle<R>,
    scope: &Scope,
    request: Request<Vec<u8>>,
    process_slots: Arc<Semaphore>,
) -> Response<Vec<u8>> {
    if request.method() == Method::OPTIONS {
        return secure_response_builder(StatusCode::NO_CONTENT)
            .header(ACCESS_CONTROL_ALLOW_METHODS, "GET, HEAD, OPTIONS")
            .header(ACCESS_CONTROL_ALLOW_HEADERS, "Range, If-Range")
            .body(Vec::new())
            .expect("static response headers");
    }
    if request.method() != Method::GET && request.method() != Method::HEAD {
        return error_response(StatusCode::METHOD_NOT_ALLOWED, "GET and HEAD only", None);
    }
    let path = match decode_request_path(request.uri().path()) {
        Ok(path) => path,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message, None),
    };
    let core = app.state::<AppCore>();
    let project_authority = match project_request_authority(&core, scope, &path) {
        Ok(authority) => authority,
        Err(response) => return *response,
    };
    // A retained current-project root is itself the authority for nested
    // relative assets. External files and the Home thumbnail exception still
    // require an exact/runtime scope grant before any helper is spawned.
    let non_project_authority = if project_authority.is_none() {
        match non_project_asset_authority(app, &core, scope, &path) {
            Some(authority) => Some(authority),
            None => {
                return error_response(
                    StatusCode::FORBIDDEN,
                    "local asset path is not approved",
                    None,
                );
            }
        }
    } else {
        None
    };
    let Some(path_text) = path.to_str() else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "local asset path is not UTF-8",
            None,
        );
    };
    let token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let helper_request = HelperRequest {
        token: token.clone(),
        parent_pid: std::process::id(),
        path: path_text.to_owned(),
        head_only: request.method() == Method::HEAD,
        range: request
            .headers()
            .get(RANGE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned),
        if_range: request
            .headers()
            .get(IF_RANGE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned),
        project: project_authority
            .as_ref()
            .and_then(HelperProjectAuthority::from_core),
    };
    let isolated = match run_isolated_helper(&helper_request, process_slots).await {
        Ok(response) => response,
        Err(IsolatedHelperError::TimedOut) => {
            return error_response(
                StatusCode::GATEWAY_TIMEOUT,
                "local asset I/O timed out",
                Some((RETRY_AFTER, "1")),
            );
        }
        Err(IsolatedHelperError::Degraded) => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "local asset isolation is degraded",
                Some((RETRY_AFTER, "5")),
            );
        }
        Err(IsolatedHelperError::Io | IsolatedHelperError::InvalidResponse) => {
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "local asset is unavailable",
                None,
            );
        }
    };
    isolated_response_to_http(
        app,
        &core,
        scope,
        project_authority.as_ref(),
        non_project_authority,
        &token,
        isolated,
    )
}

fn project_request_authority(
    core: &AppCore,
    scope: &Scope,
    path: &Path,
) -> Result<Option<ProjectAssetAuthority>, Box<Response<Vec<u8>>>> {
    let Some(bundle_path) = opentake_ancestor(path) else {
        return Ok(None);
    };
    if let Some(authority) = core.project_asset_authority() {
        if paths_equal_for_authority(&authority.project_path, &bundle_path) {
            return Ok(Some(authority));
        }
    }
    if is_home_thumbnail_exception(scope, path, &bundle_path) {
        return Ok(None);
    }
    Err(Box::new(error_response(
        StatusCode::FORBIDDEN,
        "project assets require the current retained project authority",
        None,
    )))
}

fn opentake_ancestor(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .skip(1)
        .find(|ancestor| {
            ancestor
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    #[cfg(target_os = "windows")]
                    {
                        name.to_ascii_lowercase().ends_with(".opentake")
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        name.ends_with(".opentake")
                    }
                })
        })
        .map(normalized_path)
}

fn normalized_path(path: &Path) -> PathBuf {
    #[cfg(target_os = "windows")]
    if let Some(path) = path.to_str().and_then(|path| path.strip_prefix(r"\\?\")) {
        return PathBuf::from(path).components().collect();
    }
    path.components().collect()
}

#[cfg(target_os = "windows")]
fn paths_equal_for_authority(left: &Path, right: &Path) -> bool {
    normalized_path(left)
        .to_string_lossy()
        .eq_ignore_ascii_case(&normalized_path(right).to_string_lossy())
}

#[cfg(not(target_os = "windows"))]
fn paths_equal_for_authority(left: &Path, right: &Path) -> bool {
    normalized_path(left) == normalized_path(right)
}

fn relative_to_authority(path: &Path, root: &Path) -> Option<PathBuf> {
    let path = normalized_path(path);
    let root = normalized_path(root);
    let path_components = path.components().collect::<Vec<_>>();
    let root_components = root.components().collect::<Vec<_>>();
    if root_components.len() >= path_components.len() {
        return None;
    }
    let matches_root =
        path_components
            .iter()
            .zip(&root_components)
            .all(|(path_component, root_component)| {
                #[cfg(target_os = "windows")]
                {
                    path_component
                        .as_os_str()
                        .to_string_lossy()
                        .eq_ignore_ascii_case(&root_component.as_os_str().to_string_lossy())
                }
                #[cfg(not(target_os = "windows"))]
                {
                    path_component == root_component
                }
            });
    matches_root.then(|| {
        let mut relative = PathBuf::new();
        for component in &path_components[root_components.len()..] {
            relative.push(component.as_os_str());
        }
        relative
    })
}

fn is_home_thumbnail_exception(scope: &Scope, path: &Path, bundle_path: &Path) -> bool {
    paths_equal_for_authority(path, &bundle_path.join("thumbnail.jpg"))
        && scope_has_exact_file_grant(scope, path)
}

fn scope_has_exact_file_grant(scope: &Scope, path: &Path) -> bool {
    let escaped = glob::Pattern::escape(normalized_path(path).to_string_lossy().as_ref());
    scope.allowed_patterns().iter().any(|pattern| {
        #[cfg(target_os = "windows")]
        {
            pattern.as_str().eq_ignore_ascii_case(&escaped)
        }
        #[cfg(not(target_os = "windows"))]
        {
            pattern.as_str() == escaped
        }
    })
}

/// Runtime dialog grants are persisted by Tauri. Keep the configured
/// application-owned cache/data/resource roots available, but require every
/// other external media path to remain referenced by the current project.
/// This also closes stale recursive directory grants from folder imports, not
/// just exact file grants, without mutating persisted scope state.
fn non_project_asset_authority<R: Runtime>(
    app: &AppHandle<R>,
    core: &AppCore,
    scope: &Scope,
    path: &Path,
) -> Option<NonProjectAssetAuthority> {
    let normalized = normalized_path(path);
    if !scope_allows_lexical_path(scope, &normalized) {
        return None;
    }
    enum ScopeOnlyKind {
        HomeThumbnail,
        ApplicationOwned,
    }
    enum AuthorityKind {
        ScopeOnly(ScopeOnlyKind),
        ProjectMedia(u64),
    }
    let application_owned_roots = application_owned_asset_roots(app);
    let scope_only_kind = if opentake_ancestor(&normalized)
        .is_some_and(|bundle| is_home_thumbnail_exception(scope, &normalized, bundle.as_path()))
    {
        Some(ScopeOnlyKind::HomeThumbnail)
    } else if application_owned_roots
        .iter()
        .any(|root| path_is_at_or_below(&normalized, root))
    {
        Some(ScopeOnlyKind::ApplicationOwned)
    } else {
        None
    };
    let kind = if let Some(scope_only_kind) = scope_only_kind {
        AuthorityKind::ScopeOnly(scope_only_kind)
    } else {
        let snapshot = core.runtime_snapshot();
        if snapshot.project_dir.is_none()
            || !snapshot.media.entries.iter().any(|entry| {
                let opentake_domain::MediaSource::External { absolute_path } = &entry.source else {
                    return false;
                };
                paths_equal_for_authority(Path::new(absolute_path), &normalized)
            })
        {
            return None;
        }
        AuthorityKind::ProjectMedia(snapshot.project_epoch)
    };
    let (file, initial_final_path) = open_retained_regular_file(&normalized).ok()?;
    let metadata = file.metadata().ok()?;
    let initial_etag = retained_file_etag(&file, &metadata).ok()?;
    match kind {
        AuthorityKind::ScopeOnly(scope_only_kind) => {
            let final_path_is_authorized = scope_allows_lexical_path(scope, &initial_final_path)
                && match scope_only_kind {
                    ScopeOnlyKind::HomeThumbnail => opentake_ancestor(&initial_final_path)
                        .is_some_and(|bundle| {
                            is_home_thumbnail_exception(
                                scope,
                                &initial_final_path,
                                bundle.as_path(),
                            )
                        }),
                    ScopeOnlyKind::ApplicationOwned => application_owned_roots
                        .iter()
                        .any(|root| path_is_at_or_below(&initial_final_path, root)),
                };
            final_path_is_authorized.then_some(NonProjectAssetAuthority::ScopeOnly {
                requested_path: normalized,
                initial_final_path,
                initial_etag,
            })
        }
        AuthorityKind::ProjectMedia(project_epoch) => {
            scope_allows_lexical_path(scope, &initial_final_path).then_some(
                NonProjectAssetAuthority::ProjectMedia {
                    project_epoch,
                    requested_path: normalized,
                    initial_final_path,
                    initial_etag,
                },
            )
        }
    }
}

fn application_owned_asset_roots<R: Runtime>(app: &AppHandle<R>) -> Vec<PathBuf> {
    let resolver = app.path();
    let mut roots = Vec::with_capacity(3);
    if let Ok(path) = resolver.app_cache_dir() {
        roots.push(normalized_path(&path));
    }
    if let Ok(path) = resolver.app_data_dir() {
        roots.push(normalized_path(&path.join("OpenTake/Library")));
    }
    if let Ok(path) = resolver.resource_dir() {
        roots.push(normalized_path(&path));
    }
    roots
}

fn path_is_at_or_below(path: &Path, root: &Path) -> bool {
    paths_equal_for_authority(path, root) || relative_to_authority(path, root).is_some()
}

fn decode_request_path(uri_path: &str) -> Result<PathBuf, &'static str> {
    let encoded = uri_path
        .strip_prefix('/')
        .ok_or("local asset URL must contain an absolute path")?;
    let decoded = percent_decode(encoded.as_bytes())
        .decode_utf8()
        .map_err(|_| "local asset path is not valid UTF-8")?;
    if decoded.is_empty() || decoded.len() > MAX_PATH_BYTES || decoded.as_bytes().contains(&0) {
        return Err("local asset path length is invalid");
    }
    let path = PathBuf::from(decoded.as_ref());
    if !path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
        || !platform_path_is_local(&path)
    {
        return Err("local asset path must be on a supported local volume");
    }
    Ok(path)
}

#[cfg(test)]
fn serve_open_file(
    path: &Path,
    scope: Option<&Scope>,
    head_only: bool,
    range: Option<&tauri::http::HeaderValue>,
) -> std::io::Result<Response<Vec<u8>>> {
    let (file, final_path) = open_retained_regular_file(path)?;
    if scope.is_some_and(|scope| !scope_allows_lexical_path(scope, &final_path)) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "the opened asset resolves outside its approved scope",
        ));
    }
    serve_opened_file(file, &final_path, head_only, range, None)
}

fn serve_opened_file(
    mut file: File,
    final_path: &Path,
    head_only: bool,
    range: Option<&tauri::http::HeaderValue>,
    if_range: Option<&tauri::http::HeaderValue>,
) -> std::io::Result<Response<Vec<u8>>> {
    let metadata = file.metadata()?;
    let length = metadata.len();
    let etag = retained_file_etag(&file, &metadata)?;
    let mime = mime_guess::from_path(final_path).first_or_octet_stream();
    if !allowed_media_mime(mime.essence_str()) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "only inert media content may use the local asset protocol",
        ));
    }

    let mut builder = secure_response_builder(StatusCode::OK)
        .header(ACCEPT_RANGES, "bytes")
        .header(ETAG, &etag)
        .header(CONTENT_TYPE, mime.essence_str());

    let range = range.filter(|_| if_range.is_none_or(|value| value.as_bytes() == etag.as_bytes()));
    if let Some(range) = range {
        let range = range
            .to_str()
            .ok()
            .and_then(|value| HttpRange::parse(value, length).ok())
            .and_then(|ranges| (ranges.len() == 1).then(|| ranges[0]));
        let Some(range) = range else {
            return Ok(range_not_satisfiable(length));
        };
        if range.start >= length || range.length == 0 {
            return Ok(range_not_satisfiable(length));
        }
        let read_length = range.length.min(MAX_RANGE_BYTES);
        let end = range.start.saturating_add(read_length).saturating_sub(1);
        builder = builder
            .status(StatusCode::PARTIAL_CONTENT)
            .header(
                CONTENT_RANGE,
                format!("bytes {}-{end}/{length}", range.start),
            )
            .header(ACCESS_CONTROL_EXPOSE_HEADERS, "content-range, etag")
            .header(CONTENT_LENGTH, read_length);
        if head_only {
            return Ok(builder.body(Vec::new()).expect("static response headers"));
        }
        file.seek(SeekFrom::Start(range.start))?;
        let body = read_exact_bounded(&mut file, read_length)?;
        ensure_retained_file_identity(&file, &etag)?;
        return Ok(builder.body(body).expect("static response headers"));
    }

    builder = builder.header(CONTENT_LENGTH, length);
    if head_only {
        return Ok(builder.body(Vec::new()).expect("static response headers"));
    }

    let full_body_limit = if mime.type_().as_str() == "image" {
        MAX_FULL_IMAGE_BODY_BYTES
    } else {
        MAX_FULL_BODY_BYTES
    };
    if length > full_body_limit {
        return Ok(error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            "local asset requires a byte-range request",
            None,
        ));
    }
    let body = read_exact_bounded(&mut file, length)?;
    ensure_retained_file_identity(&file, &etag)?;
    Ok(builder.body(body).expect("static response headers"))
}

fn allowed_media_mime(mime: &str) -> bool {
    (mime.starts_with("image/") && mime != "image/svg+xml")
        || mime.starts_with("audio/")
        || mime.starts_with("video/")
        || mime == "application/octet-stream"
}

fn read_exact_bounded(file: &mut File, length: u64) -> std::io::Result<Vec<u8>> {
    let capacity = usize::try_from(length).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "asset range is too large")
    })?;
    let mut body = Vec::with_capacity(capacity);
    file.take(length).read_to_end(&mut body)?;
    if body.len() != capacity {
        return Err(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "asset changed while its retained handle was being read",
        ));
    }
    Ok(body)
}

fn ensure_retained_file_identity(file: &File, expected_etag: &str) -> std::io::Result<()> {
    let metadata = file.metadata()?;
    if retained_file_etag(file, &metadata)? != expected_etag {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "asset identity changed while its retained handle was being read",
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn retained_file_etag(_file: &File, metadata: &std::fs::Metadata) -> std::io::Result<String> {
    use std::os::unix::fs::MetadataExt as _;

    Ok(format!(
        "\"{:x}-{:x}-{:x}-{:x}-{:x}-{:x}-{:x}\"",
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
    ))
}

#[cfg(target_os = "windows")]
fn retained_file_etag(file: &File, metadata: &std::fs::Metadata) -> std::io::Result<String> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };

    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: `file` owns a live handle and `information` is writable.
    if unsafe {
        GetFileInformationByHandle(
            file.as_raw_handle() as HANDLE,
            std::ptr::addr_of_mut!(information),
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error());
    }
    let file_index =
        (u64::from(information.nFileIndexHigh) << 32) | u64::from(information.nFileIndexLow);
    Ok(format!(
        "\"{:x}-{:x}-{:x}-{:x}{:08x}\"",
        information.dwVolumeSerialNumber,
        file_index,
        metadata.len(),
        information.ftLastWriteTime.dwHighDateTime,
        information.ftLastWriteTime.dwLowDateTime,
    ))
}

pub(crate) fn open_retained_regular_file(path: &Path) -> std::io::Result<(File, PathBuf)> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options
            .share_mode(
                windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ
                    | windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE
                    | windows_sys::Win32::Storage::FileSystem::FILE_SHARE_DELETE,
            )
            .custom_flags(windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_NO_RECALL);
    }
    let file = options.open(path)?;
    let final_path = validate_opened_resident_regular_file(&file)?;
    Ok((file, final_path))
}

fn validate_opened_resident_regular_file(file: &File) -> std::io::Result<PathBuf> {
    let metadata = file.metadata()?;
    if !metadata.is_file() || retained_metadata_is_unavailable(file, &metadata) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "asset is not a resident regular file on a local volume",
        ));
    }
    let final_path = retained_final_path(file)?;
    if !platform_path_is_local(&final_path) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "asset resolves outside a supported local volume",
        ));
    }
    Ok(final_path)
}

/// Validate one exact Home thumbnail grant without expanding it to a recursive
/// bundle scope. The returned path comes from the retained file handle.
pub(crate) fn validate_resident_regular_file(path: &Path) -> std::io::Result<PathBuf> {
    open_retained_regular_file(path).map(|(_, final_path)| final_path)
}

pub(crate) fn scope_allows_lexical_path(scope: &Scope, path: &Path) -> bool {
    let normalized: PathBuf = path.components().collect();
    let options = scope_match_options();
    if scope
        .forbidden_patterns()
        .iter()
        .any(|pattern| pattern.matches_path_with(&normalized, options))
    {
        return false;
    }
    scope
        .allowed_patterns()
        .iter()
        .any(|pattern| pattern.matches_path_with(&normalized, options))
}

fn scope_match_options() -> glob::MatchOptions {
    glob::MatchOptions {
        #[cfg(target_os = "windows")]
        case_sensitive: false,
        #[cfg(not(target_os = "windows"))]
        case_sensitive: true,
        require_literal_separator: true,
        #[cfg(unix)]
        require_literal_leading_dot: true,
        #[cfg(not(unix))]
        require_literal_leading_dot: false,
    }
}

#[cfg(target_os = "macos")]
fn retained_final_path(file: &File) -> std::io::Result<PathBuf> {
    use std::ffi::CStr;
    use std::os::unix::ffi::OsStrExt;

    let mut buffer = vec![0_i8; libc::PATH_MAX as usize];
    // SAFETY: `buffer` is writable for PATH_MAX bytes and `file` owns a live fd.
    if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETPATH, buffer.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: F_GETPATH succeeded and wrote a NUL-terminated pathname.
    let bytes = unsafe { CStr::from_ptr(buffer.as_ptr()) }.to_bytes();
    Ok(PathBuf::from(std::ffi::OsStr::from_bytes(bytes)))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn retained_final_path(file: &File) -> std::io::Result<PathBuf> {
    use std::os::fd::AsRawFd;

    std::fs::read_link(format!("/proc/self/fd/{}", file.as_raw_fd()))
}

#[cfg(target_os = "windows")]
fn retained_final_path(file: &File) -> std::io::Result<PathBuf> {
    use std::ffi::OsString;
    use std::os::windows::{ffi::OsStringExt, io::AsRawHandle};
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFinalPathNameByHandleW, FILE_NAME_NORMALIZED, VOLUME_NAME_DOS,
    };

    let handle = file.as_raw_handle() as HANDLE;
    let mut buffer = vec![0_u16; 32_768];
    // SAFETY: `handle` is live and `buffer` is writable for its declared length.
    let mut length = unsafe {
        GetFinalPathNameByHandleW(
            handle,
            buffer.as_mut_ptr(),
            u32::try_from(buffer.len()).unwrap_or(u32::MAX),
            FILE_NAME_NORMALIZED | VOLUME_NAME_DOS,
        )
    };
    if length == 0 {
        return Err(std::io::Error::last_os_error());
    }
    if usize::try_from(length).unwrap_or(usize::MAX) >= buffer.len() {
        buffer.resize(usize::try_from(length).unwrap_or(MAX_PATH_BYTES) + 1, 0);
        // SAFETY: same live handle; resized buffer is writable for its declared length.
        length = unsafe {
            GetFinalPathNameByHandleW(
                handle,
                buffer.as_mut_ptr(),
                u32::try_from(buffer.len()).unwrap_or(u32::MAX),
                FILE_NAME_NORMALIZED | VOLUME_NAME_DOS,
            )
        };
        if length == 0 || usize::try_from(length).unwrap_or(usize::MAX) >= buffer.len() {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(PathBuf::from(OsString::from_wide(
        &buffer[..usize::try_from(length).unwrap_or(0)],
    )))
}

#[cfg(target_os = "macos")]
fn retained_metadata_is_unavailable(file: &File, metadata: &std::fs::Metadata) -> bool {
    const SF_DATALESS: u32 = 0x4000_0000;
    if metadata.st_flags() & SF_DATALESS != 0 {
        return true;
    }
    let mut stat = std::mem::MaybeUninit::<libc::statfs>::uninit();
    // SAFETY: `stat` points to writable storage and `file` owns a live fd.
    let result = unsafe { libc::fstatfs(file.as_raw_fd(), stat.as_mut_ptr()) };
    if result != 0 {
        return true;
    }
    // SAFETY: fstatfs returned success and initialized the structure.
    let stat = unsafe { stat.assume_init() };
    stat.f_flags & u32::try_from(libc::MNT_LOCAL).unwrap_or(u32::MAX) == 0
}

#[cfg(target_os = "windows")]
fn retained_metadata_is_unavailable(file: &File, _metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Storage::CloudFilters::{
        CfGetPlaceholderStateFromAttributeTag, CF_PLACEHOLDER_STATE_PARTIAL,
        CF_PLACEHOLDER_STATE_PARTIALLY_ON_DISK,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        FileAttributeTagInfo, GetFileInformationByHandleEx, FILE_ATTRIBUTE_OFFLINE,
        FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS, FILE_ATTRIBUTE_RECALL_ON_OPEN,
        FILE_ATTRIBUTE_TAG_INFO,
    };

    let mut info = FILE_ATTRIBUTE_TAG_INFO::default();
    // SAFETY: `file` owns a live handle and `info` is writable for its exact size.
    if unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle() as HANDLE,
            FileAttributeTagInfo,
            std::ptr::addr_of_mut!(info).cast(),
            u32::try_from(std::mem::size_of::<FILE_ATTRIBUTE_TAG_INFO>()).unwrap_or(u32::MAX),
        )
    } == 0
    {
        return true;
    }
    if info.FileAttributes
        & (FILE_ATTRIBUTE_OFFLINE
            | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS
            | FILE_ATTRIBUTE_RECALL_ON_OPEN)
        != 0
    {
        return true;
    }
    // Fully hydrated Cloud Files remain reparse points. Reject only partial
    // placeholder states; FILE_FLAG_OPEN_NO_RECALL above prevents this open
    // from silently hydrating the file.
    // SAFETY: the values come from the retained handle's attribute/tag query.
    let placeholder_state =
        unsafe { CfGetPlaceholderStateFromAttributeTag(info.FileAttributes, info.ReparseTag) };
    placeholder_state & (CF_PLACEHOLDER_STATE_PARTIAL | CF_PLACEHOLDER_STATE_PARTIALLY_ON_DISK) != 0
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn retained_metadata_is_unavailable(_file: &File, _metadata: &std::fs::Metadata) -> bool {
    false
}

#[cfg(target_os = "windows")]
fn platform_path_is_local(path: &Path) -> bool {
    use std::path::{Component, Prefix};
    use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;

    // DRIVE_* constants are not exported from Win32::Storage::FileSystem in
    // windows-sys 0.61; define locally like safe_fs/windows.rs.
    const DRIVE_REMOVABLE: u32 = 2;
    const DRIVE_FIXED: u32 = 3;
    let Some(Component::Prefix(prefix)) = path.components().next() else {
        return false;
    };
    let letter = match prefix.kind() {
        Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => letter,
        _ => return false,
    };
    let root = [u16::from(letter), u16::from(b':'), u16::from(b'\\'), 0];
    // SAFETY: `root` is a valid NUL-terminated drive-root string.
    matches!(
        unsafe { GetDriveTypeW(root.as_ptr()) },
        DRIVE_FIXED | DRIVE_REMOVABLE
    )
}

#[cfg(not(target_os = "windows"))]
fn platform_path_is_local(_path: &Path) -> bool {
    true
}

mod response;
#[cfg(test)]
use response::asset_origin;
use response::{error_response, range_not_satisfiable, secure_response_builder};

#[cfg(test)]
#[path = "safe_asset_protocol/tests.rs"]
mod tests;
