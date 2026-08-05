use super::*;

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct HelperRequest {
    pub(super) token: String,
    pub(super) parent_pid: u32,
    pub(super) path: String,
    pub(super) head_only: bool,
    pub(super) range: Option<String>,
    pub(super) if_range: Option<String>,
    pub(super) project: Option<HelperProjectAuthority>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct HelperProjectAuthority {
    pub(super) project_epoch: u64,
    pub(super) project_path: String,
    pub(super) root_identity: ProjectRootIdentity,
}

impl HelperProjectAuthority {
    pub(super) fn from_core(authority: &ProjectAssetAuthority) -> Option<Self> {
        Some(Self {
            project_epoch: authority.project_epoch,
            project_path: authority.project_path.to_str()?.to_owned(),
            root_identity: authority.root_identity,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct HelperResponseMetadata {
    pub(super) token: String,
    pub(super) final_path: Option<String>,
    pub(super) project_root_identity: Option<ProjectRootIdentity>,
    pub(super) status: u16,
    pub(super) headers: Vec<(String, String)>,
    pub(super) body_length: u64,
    pub(super) error_kind: Option<WireIoErrorKind>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(super) enum WireIoErrorKind {
    NotFound,
    PermissionDenied,
    InvalidInput,
    Other,
}

pub(super) struct IsolatedResponse {
    pub(super) metadata: HelperResponseMetadata,
    pub(super) body: Vec<u8>,
}

#[derive(Debug)]
pub(super) enum IsolatedHelperError {
    TimedOut,
    Degraded,
    Io,
    InvalidResponse,
}

pub(super) async fn run_isolated_helper(
    request: &HelperRequest,
    process_slots: Arc<Semaphore>,
) -> Result<IsolatedResponse, IsolatedHelperError> {
    // Reserve the quarantine capacity before spawning. If this child cannot be
    // reaped after kill, ownership of both the Child and this permit moves to
    // the bounded background reaper. Once all four slots are quarantined no
    // further process is created.
    let process_slot = process_slots
        .try_acquire_owned()
        .map_err(|_| IsolatedHelperError::Degraded)?;
    let executable = std::env::current_exe().map_err(|_| IsolatedHelperError::Io)?;
    let mut child = Command::new(executable)
        .arg(HELPER_ARG)
        .env(HELPER_TOKEN_ENV, &request.token)
        .env(HELPER_PARENT_ENV, request.parent_pid.to_string())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| IsolatedHelperError::Io)?;
    let mut stdin = child.stdin.take().ok_or(IsolatedHelperError::Io)?;
    let mut stdout = child.stdout.take().ok_or(IsolatedHelperError::Io)?;
    let encoded = serde_json::to_vec(request).map_err(|_| IsolatedHelperError::InvalidResponse)?;
    if encoded.len() > MAX_HELPER_REQUEST_BYTES {
        terminate_or_quarantine(child, process_slot).await;
        return Err(IsolatedHelperError::InvalidResponse);
    }

    let operation = async {
        stdin
            .write_all(&encoded)
            .await
            .map_err(|_| IsolatedHelperError::Io)?;
        stdin
            .shutdown()
            .await
            .map_err(|_| IsolatedHelperError::Io)?;

        let mut metadata_length = [0_u8; 4];
        stdout
            .read_exact(&mut metadata_length)
            .await
            .map_err(|_| IsolatedHelperError::InvalidResponse)?;
        let metadata_length = u32::from_be_bytes(metadata_length) as usize;
        if metadata_length == 0 || metadata_length > MAX_HELPER_METADATA_BYTES {
            return Err(IsolatedHelperError::InvalidResponse);
        }
        let mut metadata_bytes = vec![0_u8; metadata_length];
        stdout
            .read_exact(&mut metadata_bytes)
            .await
            .map_err(|_| IsolatedHelperError::InvalidResponse)?;
        let metadata: HelperResponseMetadata = serde_json::from_slice(&metadata_bytes)
            .map_err(|_| IsolatedHelperError::InvalidResponse)?;
        let body_length = usize::try_from(metadata.body_length)
            .map_err(|_| IsolatedHelperError::InvalidResponse)?;
        if body_length > MAX_HELPER_BODY_BYTES {
            return Err(IsolatedHelperError::InvalidResponse);
        }
        let mut body = vec![0_u8; body_length];
        stdout
            .read_exact(&mut body)
            .await
            .map_err(|_| IsolatedHelperError::InvalidResponse)?;
        let status = child.wait().await.map_err(|_| IsolatedHelperError::Io)?;
        if !status.success() {
            return Err(IsolatedHelperError::InvalidResponse);
        }
        Ok(IsolatedResponse { metadata, body })
    };

    match tokio::time::timeout(IO_DEADLINE, operation).await {
        Ok(Ok(response)) => Ok(response),
        Ok(Err(error)) => {
            terminate_or_quarantine(child, process_slot).await;
            Err(error)
        }
        Err(_) => {
            terminate_or_quarantine(child, process_slot).await;
            Err(IsolatedHelperError::TimedOut)
        }
    }
}

pub(super) async fn terminate_or_quarantine(
    mut child: tokio::process::Child,
    process_slot: tokio::sync::OwnedSemaphorePermit,
) {
    let _ = child.start_kill();
    // A process stuck in an uninterruptible kernel wait may not acknowledge
    // termination promptly. Never turn the helper deadline into another
    // unbounded wait; kill_on_drop remains armed if this bounded reap expires.
    if bounded_reap(child.wait(), REAP_DEADLINE).await {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let _process_slot = process_slot;
        let _ = child.wait().await;
    });
}

pub(super) async fn bounded_reap<F>(wait: F, deadline: Duration) -> bool
where
    F: std::future::Future<Output = std::io::Result<std::process::ExitStatus>>,
{
    tokio::time::timeout(deadline, wait).await.is_ok()
}

pub(super) fn isolated_response_to_http(
    core: &AppCore,
    scope: &Scope,
    expected_project: Option<&ProjectAssetAuthority>,
    token: &str,
    isolated: IsolatedResponse,
) -> Response<Vec<u8>> {
    let metadata = isolated.metadata;
    if metadata.token != token || metadata.body_length != isolated.body.len() as u64 {
        return error_response(StatusCode::BAD_GATEWAY, "local asset helper failed", None);
    }
    if let Some(kind) = metadata.error_kind {
        let status = match kind {
            WireIoErrorKind::NotFound => StatusCode::NOT_FOUND,
            WireIoErrorKind::PermissionDenied | WireIoErrorKind::InvalidInput => {
                StatusCode::FORBIDDEN
            }
            WireIoErrorKind::Other => StatusCode::UNPROCESSABLE_ENTITY,
        };
        return error_response(status, "local asset is unavailable", None);
    }
    let Some(final_path) = metadata.final_path.as_deref().map(PathBuf::from) else {
        return error_response(StatusCode::BAD_GATEWAY, "local asset helper failed", None);
    };
    if expected_project.is_none() && !scope_allows_lexical_path(scope, &final_path) {
        return error_response(
            StatusCode::FORBIDDEN,
            "the opened asset resolves outside its approved scope",
            None,
        );
    }
    match (expected_project, opentake_ancestor(&final_path)) {
        (Some(expected), Some(bundle_path))
            if metadata.project_root_identity == Some(expected.root_identity)
                && paths_equal_for_authority(&expected.project_path, &bundle_path) => {}
        (None, Some(bundle_path))
            if is_home_thumbnail_exception(scope, &final_path, &bundle_path) => {}
        (None, None) => {}
        _ => {
            return error_response(
                StatusCode::FORBIDDEN,
                "project asset authority changed during the read",
                None,
            );
        }
    }

    // No await follows this lease. Project open/save-as cannot replace the
    // retained root between the final authority comparison and publication.
    let _identity_lease = expected_project.map(|_| core.lock_project_identity_workflow());
    if expected_project.is_some_and(|expected| !core.project_asset_authority_matches(expected)) {
        return error_response(
            StatusCode::FORBIDDEN,
            "project asset authority changed during the read",
            None,
        );
    }

    let status = StatusCode::from_u16(metadata.status).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut builder = secure_response_builder(status);
    for (name, value) in metadata.headers {
        let Ok(name) = tauri::http::HeaderName::from_bytes(name.as_bytes()) else {
            return error_response(StatusCode::BAD_GATEWAY, "local asset helper failed", None);
        };
        let Ok(value) = tauri::http::HeaderValue::from_str(&value) else {
            return error_response(StatusCode::BAD_GATEWAY, "local asset helper failed", None);
        };
        builder = builder.header(name, value);
    }
    builder.body(isolated.body).unwrap_or_else(|_| {
        error_response(StatusCode::BAD_GATEWAY, "local asset helper failed", None)
    })
}

/// Run the undocumented, single-request asset reader mode before Tauri starts.
/// The random token and actual parent PID must agree across env and stdin.
#[doc(hidden)]
pub(crate) fn run_helper_if_requested() -> bool {
    if std::env::args_os().nth(1).as_deref() != Some(std::ffi::OsStr::new(HELPER_ARG)) {
        return false;
    }
    let exit_code = run_helper_stdio().map_or(1, |()| 0);
    std::process::exit(exit_code);
}

fn run_helper_stdio() -> std::io::Result<()> {
    let expected_token = std::env::var(HELPER_TOKEN_ENV)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::PermissionDenied, "missing token"))?;
    let expected_parent = std::env::var(HELPER_PARENT_ENV)
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::PermissionDenied, "missing parent")
        })?;
    if expected_parent != actual_parent_process_id()? {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "helper parent mismatch",
        ));
    }
    if !parent_is_same_executable(expected_parent)? {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "helper parent executable mismatch",
        ));
    }
    let mut encoded = Vec::new();
    std::io::stdin()
        .take((MAX_HELPER_REQUEST_BYTES + 1) as u64)
        .read_to_end(&mut encoded)?;
    if encoded.len() > MAX_HELPER_REQUEST_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "helper request is too large",
        ));
    }
    let request: HelperRequest = serde_json::from_slice(&encoded)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
    if request.token != expected_token || request.parent_pid != expected_parent {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "helper authentication failed",
        ));
    }
    let response = helper_response(&request);
    let metadata = serde_json::to_vec(&response.metadata)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    if metadata.len() > MAX_HELPER_METADATA_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "helper metadata is too large",
        ));
    }
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(&(metadata.len() as u32).to_be_bytes())?;
    stdout.write_all(&metadata)?;
    stdout.write_all(&response.body)?;
    stdout.flush()
}

#[cfg(unix)]
pub(super) fn actual_parent_process_id() -> std::io::Result<u32> {
    // SAFETY: getppid has no preconditions.
    Ok(unsafe { libc::getppid() } as u32)
}

#[cfg(target_os = "linux")]
pub(super) fn parent_is_same_executable(parent_pid: u32) -> std::io::Result<bool> {
    same_file::is_same_file(
        std::fs::read_link(format!("/proc/{parent_pid}/exe"))?,
        std::env::current_exe()?,
    )
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
pub(super) fn parent_is_same_executable(_parent_pid: u32) -> std::io::Result<bool> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "asset helper parent identity is unsupported on this platform",
    ))
}

#[cfg(target_os = "macos")]
pub(super) fn parent_is_same_executable(parent_pid: u32) -> std::io::Result<bool> {
    use std::ffi::c_void;
    use std::os::unix::ffi::OsStrExt;

    const PROC_PIDPATHINFO_MAXSIZE: usize = 4096;
    #[link(name = "proc")]
    unsafe extern "C" {
        fn proc_pidpath(pid: libc::c_int, buffer: *mut c_void, buffersize: u32) -> libc::c_int;
    }
    let mut buffer = vec![0_u8; PROC_PIDPATHINFO_MAXSIZE];
    // SAFETY: `buffer` is writable for the declared size and parent_pid names
    // the helper's live parent.
    let length = unsafe {
        proc_pidpath(
            parent_pid as libc::c_int,
            buffer.as_mut_ptr().cast(),
            buffer.len() as u32,
        )
    };
    if length <= 0 {
        return Err(std::io::Error::last_os_error());
    }
    buffer.truncate(length as usize);
    same_file::is_same_file(
        PathBuf::from(std::ffi::OsStr::from_bytes(&buffer)),
        std::env::current_exe()?,
    )
}

#[cfg(target_os = "windows")]
pub(super) fn actual_parent_process_id() -> std::io::Result<u32> {
    use std::ffi::c_void;

    #[repr(C)]
    struct ProcessBasicInformation {
        reserved1: *mut c_void,
        peb_base_address: *mut c_void,
        reserved2: [*mut c_void; 2],
        unique_process_id: usize,
        inherited_from_unique_process_id: usize,
    }
    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtQueryInformationProcess(
            process_handle: isize,
            process_information_class: u32,
            process_information: *mut c_void,
            process_information_length: u32,
            return_length: *mut u32,
        ) -> i32;
    }
    let mut information = std::mem::MaybeUninit::<ProcessBasicInformation>::uninit();
    let mut returned = 0_u32;
    // SAFETY: -1 is the documented current-process pseudo-handle and the
    // output buffer is writable for its exact declared size.
    let status = unsafe {
        NtQueryInformationProcess(
            -1_isize,
            0,
            information.as_mut_ptr().cast(),
            u32::try_from(std::mem::size_of::<ProcessBasicInformation>()).unwrap_or(u32::MAX),
            &mut returned,
        )
    };
    if status < 0 {
        return Err(std::io::Error::other(format!(
            "NtQueryInformationProcess failed with NTSTATUS {status:#x}"
        )));
    }
    // SAFETY: the successful syscall initialized the output structure.
    let information = unsafe { information.assume_init() };
    u32::try_from(information.inherited_from_unique_process_id)
        .map_err(|_| std::io::Error::other("parent process ID is out of range"))
}

#[cfg(target_os = "windows")]
pub(super) fn parent_is_same_executable(parent_pid: u32) -> std::io::Result<bool> {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStringExt;

    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn OpenProcess(desired_access: u32, inherit_handle: i32, process_id: u32) -> *mut c_void;
        fn QueryFullProcessImageNameW(
            process: *mut c_void,
            flags: u32,
            executable_name: *mut u16,
            size: *mut u32,
        ) -> i32;
        fn CloseHandle(object: *mut c_void) -> i32;
    }

    // SAFETY: the access mask is read-only and parent_pid names a live process.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, parent_pid) };
    if process.is_null() {
        return Err(std::io::Error::last_os_error());
    }
    let mut buffer = vec![0_u16; 32_768];
    let mut length = buffer.len() as u32;
    // SAFETY: `process` is live and `buffer` is writable for `length` UTF-16 units.
    let queried =
        unsafe { QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut length) };
    let query_error = (queried == 0).then(std::io::Error::last_os_error);
    // SAFETY: `process` was returned by OpenProcess and is closed exactly once.
    let _ = unsafe { CloseHandle(process) };
    if let Some(error) = query_error {
        return Err(error);
    }
    buffer.truncate(length as usize);
    same_file::is_same_file(
        PathBuf::from(std::ffi::OsString::from_wide(&buffer)),
        std::env::current_exe()?,
    )
}

pub(super) fn helper_response(request: &HelperRequest) -> IsolatedResponse {
    match read_helper_asset(request) {
        Ok((response, final_path, project_root_identity)) => {
            let (parts, body) = response.into_parts();
            let headers = parts
                .headers
                .iter()
                .filter_map(|(name, value)| {
                    value
                        .to_str()
                        .ok()
                        .map(|value| (name.as_str().to_owned(), value.to_owned()))
                })
                .collect();
            IsolatedResponse {
                metadata: HelperResponseMetadata {
                    token: request.token.clone(),
                    final_path: final_path.to_str().map(str::to_owned),
                    project_root_identity,
                    status: parts.status.as_u16(),
                    headers,
                    body_length: body.len() as u64,
                    error_kind: None,
                },
                body,
            }
        }
        Err(error) => IsolatedResponse {
            metadata: HelperResponseMetadata {
                token: request.token.clone(),
                final_path: None,
                project_root_identity: None,
                status: 0,
                headers: Vec::new(),
                body_length: 0,
                error_kind: Some(match error.kind() {
                    std::io::ErrorKind::NotFound => WireIoErrorKind::NotFound,
                    std::io::ErrorKind::PermissionDenied => WireIoErrorKind::PermissionDenied,
                    std::io::ErrorKind::InvalidInput => WireIoErrorKind::InvalidInput,
                    _ => WireIoErrorKind::Other,
                }),
            },
            body: Vec::new(),
        },
    }
}

fn read_helper_asset(
    request: &HelperRequest,
) -> std::io::Result<(Response<Vec<u8>>, PathBuf, Option<ProjectRootIdentity>)> {
    let path = PathBuf::from(&request.path);
    let range = request
        .range
        .as_deref()
        .and_then(|value| tauri::http::HeaderValue::from_str(value).ok());
    let if_range = request
        .if_range
        .as_deref()
        .and_then(|value| tauri::http::HeaderValue::from_str(value).ok());
    let (file, final_path, project_root_identity) = if let Some(project) = &request.project {
        let project_path = PathBuf::from(&project.project_path);
        let relative = relative_to_authority(&path, &project_path).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "project asset is outside the retained root",
            )
        })?;
        if relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "project asset relative path is invalid",
            ));
        }
        let root = ProjectRoot::open(&project_path).map_err(std::io::Error::other)?;
        if root.stable_identity() != project.root_identity {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "project root identity changed before asset read",
            ));
        }
        let file = root
            .open_asset_file(&relative)
            .map_err(std::io::Error::other)?;
        let final_path = validate_opened_resident_regular_file(&file)?;
        (file, final_path, Some(root.stable_identity()))
    } else {
        let (file, final_path) = open_retained_regular_file(&path)?;
        (file, final_path, None)
    };
    let response = serve_opened_file(
        file,
        &final_path,
        request.head_only,
        range.as_ref(),
        if_range.as_ref(),
    )?;
    Ok((response, final_path, project_root_identity))
}
