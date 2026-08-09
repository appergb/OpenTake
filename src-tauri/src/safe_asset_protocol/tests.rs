use super::*;

fn local_tempdir() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("safe-asset-")
        .tempdir_in(std::env::current_dir().unwrap())
        .unwrap()
}

#[test]
fn serves_only_a_bounded_single_range_from_the_retained_file() {
    let directory = local_tempdir();
    let path = directory.path().join("clip.mp4");
    std::fs::write(&path, b"0123456789").unwrap();
    let range = tauri::http::HeaderValue::from_static("bytes=2-5");

    let response = serve_open_file(&path, None, false, Some(&range)).unwrap();

    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(response.headers()[CONTENT_RANGE], "bytes 2-5/10");
    assert!(response.headers().contains_key(ETAG));
    assert_eq!(response.body(), b"2345");
}

#[test]
fn stale_if_range_never_splices_bytes_from_a_different_identity() {
    let directory = local_tempdir();
    let path = directory.path().join("clip.mp4");
    std::fs::write(&path, b"0123456789").unwrap();
    let range = tauri::http::HeaderValue::from_static("bytes=2-5");
    let stale_identity = tauri::http::HeaderValue::from_static("\"stale-file\"");
    let (file, final_path) = open_retained_regular_file(&path).unwrap();

    let response = serve_opened_file(
        file,
        &final_path,
        false,
        Some(&range),
        Some(&stale_identity),
    )
    .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.body(), b"0123456789");
}

#[cfg(unix)]
#[test]
fn nofollow_nonblocking_open_rejects_fifo_and_symlink() {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::symlink;

    let directory = local_tempdir();
    let regular = directory.path().join("regular.jpg");
    let link = directory.path().join("link.jpg");
    let fifo = directory.path().join("pipe.jpg");
    std::fs::write(&regular, b"jpeg").unwrap();
    symlink(&regular, &link).unwrap();
    let fifo_c = CString::new(fifo.as_os_str().as_bytes()).unwrap();
    // SAFETY: fifo_c is a live NUL-terminated path.
    assert_eq!(unsafe { libc::mkfifo(fifo_c.as_ptr(), 0o600) }, 0);

    assert!(open_retained_regular_file(&regular).is_ok());
    assert!(open_retained_regular_file(&link).is_err());
    assert!(open_retained_regular_file(&fifo).is_err());
}

#[cfg(unix)]
#[test]
fn final_handle_path_authorization_rejects_a_symlinked_ancestor_escape() {
    use std::os::unix::fs::symlink;
    use tauri::Manager;

    let directory = local_tempdir();
    let outside = local_tempdir();
    std::fs::write(outside.path().join("outside.jpg"), b"outside").unwrap();
    let alias = directory.path().join("alias");
    symlink(outside.path(), &alias).unwrap();
    let requested = alias.join("outside.jpg");

    let (_, final_path) = open_retained_regular_file(&requested).unwrap();
    assert_eq!(final_path, outside.path().join("outside.jpg"));

    let app = tauri::test::mock_app();
    let scope = app.handle().asset_protocol_scope();
    scope.allow_file(&requested).unwrap();
    scope.forbid_file(&final_path).unwrap();
    assert!(scope_allows_lexical_path(&scope, &requested));
    assert!(!scope_allows_lexical_path(&scope, &final_path));
    assert_eq!(
        serve_open_file(&requested, Some(&scope), false, None)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::PermissionDenied
    );
}

#[cfg(unix)]
#[test]
fn response_for_request_rejects_scope_only_alias_that_resolves_outside_scope() {
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::symlink;
    use tauri::Manager;

    let app = tauri::test::mock_app();
    app.manage(AppCore::new());
    let cache_root = app.path().app_cache_dir().unwrap();
    std::fs::create_dir_all(&cache_root).unwrap();
    let cache_directory = tempfile::Builder::new()
        .prefix("safe-asset-cache-alias-")
        .tempdir_in(&cache_root)
        .unwrap();
    let outside = local_tempdir();
    let final_path = outside.path().join("outside.jpg");
    std::fs::write(&final_path, b"outside").unwrap();
    let alias = cache_directory.path().join("alias");
    symlink(outside.path(), &alias).unwrap();
    let requested = alias.join("outside.jpg");

    let scope = app.handle().asset_protocol_scope();
    scope.allow_file(&requested).unwrap();
    scope.forbid_file(&final_path).unwrap();
    assert!(scope_allows_lexical_path(&scope, &requested));
    assert!(!scope_allows_lexical_path(&scope, &final_path));

    let encoded = percent_encoding::percent_encode(
        requested.as_os_str().as_bytes(),
        percent_encoding::NON_ALPHANUMERIC,
    );
    let request = Request::builder()
        .method(Method::GET)
        .uri(format!("http://opentake.local/{encoded}"))
        .body(Vec::new())
        .unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let response = runtime.block_on(response_for_request(
        app.handle(),
        &scope,
        request,
        Arc::new(Semaphore::new(0)),
    ));

    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "a ScopeOnly request must reject an out-of-scope retained final path before acquiring a helper slot"
    );
}

#[cfg(unix)]
#[test]
fn response_for_request_rejects_project_media_ancestor_symlink_escape() {
    use opentake_core::ProbedMedia;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::symlink;
    use tauri::Manager;

    let approved = local_tempdir();
    let outside = local_tempdir();
    let final_path = outside.path().join("outside.mp4");
    std::fs::write(&final_path, b"outside-project-media").unwrap();
    let alias = approved.path().join("selected-source");
    symlink(outside.path(), &alias).unwrap();
    let requested = alias.join("outside.mp4");

    let core = AppCore::new();
    core.save_project(Some(approved.path().join("Escape.opentake")))
        .unwrap();
    core.import_media_file(&requested, "outside", &ProbedMedia::default())
        .unwrap();
    let app = tauri::test::mock_app();
    app.manage(core);
    let scope = app.handle().asset_protocol_scope();
    scope.allow_directory(approved.path(), true).unwrap();
    assert!(scope_allows_lexical_path(&scope, &requested));
    assert!(!scope_allows_lexical_path(&scope, &final_path));

    let encoded = percent_encoding::percent_encode(
        requested.as_os_str().as_bytes(),
        percent_encoding::NON_ALPHANUMERIC,
    );
    let request = Request::builder()
        .method(Method::GET)
        .uri(format!("http://opentake.local/{encoded}"))
        .body(Vec::new())
        .unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let response = runtime.block_on(response_for_request(
        app.handle(),
        &scope,
        request,
        Arc::new(Semaphore::new(0)),
    ));

    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "project media must not use a recursive lexical grant to escape through an ancestor symlink"
    );
}

#[test]
fn rejects_multi_range_and_oversized_full_body() {
    let directory = local_tempdir();
    let path = directory.path().join("clip.bin");
    let file = File::create(&path).unwrap();
    file.set_len(MAX_FULL_BODY_BYTES + 1).unwrap();
    let multi = tauri::http::HeaderValue::from_static("bytes=0-1,4-5");

    assert_eq!(
        serve_open_file(&path, None, false, Some(&multi))
            .unwrap()
            .status(),
        StatusCode::RANGE_NOT_SATISFIABLE
    );
    assert_eq!(
        serve_open_file(&path, None, false, None).unwrap().status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert_eq!(
        serve_open_file(&path, None, true, None).unwrap().status(),
        StatusCode::OK
    );
    assert_eq!(
        serve_open_file(&path, None, true, None).unwrap().body(),
        &Vec::<u8>::new()
    );
}

#[test]
fn response_headers_are_origin_bound_and_inert() {
    let directory = local_tempdir();
    let path = directory.path().join("frame.jpg");
    std::fs::write(&path, b"jpeg").unwrap();

    let response = serve_open_file(&path, None, false, None).unwrap();

    assert_eq!(
        response.headers()[ACCESS_CONTROL_ALLOW_ORIGIN],
        asset_origin()
    );
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    assert_eq!(
        response.headers()["content-security-policy"],
        "default-src 'none'; sandbox"
    );
}

#[cfg(unix)]
#[test]
fn project_helper_rejects_an_ambient_bundle_replacement() {
    let directory = local_tempdir();
    let selected = directory.path().join("Selected.opentake");
    std::fs::create_dir_all(selected.join("media")).unwrap();
    std::fs::write(selected.join("media/clip.mp4"), b"project-a").unwrap();
    let retained = ProjectRoot::open(&selected).unwrap();
    let expected_identity = retained.stable_identity();

    std::fs::rename(&selected, directory.path().join("Retained-A.opentake")).unwrap();
    std::fs::create_dir_all(selected.join("media")).unwrap();
    std::fs::write(selected.join("media/clip.mp4"), b"project-b").unwrap();

    let request = HelperRequest {
        token: "test-token".to_owned(),
        parent_pid: std::process::id(),
        path: selected
            .join("media/clip.mp4")
            .to_string_lossy()
            .into_owned(),
        head_only: false,
        range: None,
        if_range: None,
        project: Some(HelperProjectAuthority {
            project_epoch: 7,
            project_path: selected.to_string_lossy().into_owned(),
            root_identity: expected_identity,
        }),
    };

    let response = helper_response(&request);
    assert!(matches!(
        response.metadata.error_kind,
        Some(WireIoErrorKind::PermissionDenied)
    ));
    assert!(response.body.is_empty());
}

/// cap-std retains the bundle without FILE_SHARE_DELETE: on Windows the
/// ambient replacement is rejected closed while retained (the helper's
/// replacement rejection is Unix-verified above).
#[cfg(target_os = "windows")]
#[test]
fn project_helper_blocks_an_ambient_bundle_replacement_while_retained() {
    let directory = local_tempdir();
    let selected = directory.path().join("Selected.opentake");
    std::fs::create_dir_all(selected.join("media")).unwrap();
    std::fs::write(selected.join("media/clip.mp4"), b"project-a").unwrap();
    let retained = ProjectRoot::open(&selected).unwrap();

    assert!(std::fs::rename(&selected, directory.path().join("Retained-A.opentake")).is_err());

    drop(retained);
    std::fs::rename(&selected, directory.path().join("Retained-A.opentake")).unwrap();
    assert_eq!(
        std::fs::read(directory.path().join("Retained-A.opentake/media/clip.mp4")).unwrap(),
        b"project-a"
    );
}

#[test]
fn current_project_authority_allows_nested_media_without_recursive_scope() {
    use tauri::Manager;

    let directory = local_tempdir();
    let bundle = directory.path().join("ExactRootGrant.opentake");
    let core = AppCore::new();
    core.save_project(Some(bundle.clone())).unwrap();
    std::fs::create_dir_all(bundle.join("media")).unwrap();
    let media = bundle.join("media/clip.mp4");
    std::fs::write(&media, b"project-media").unwrap();
    let app = tauri::test::mock_app();
    let scope = app.handle().asset_protocol_scope();
    scope.allow_file(&bundle).unwrap();
    assert!(!scope_allows_lexical_path(&scope, &media));

    let authority = project_request_authority(&core, &scope, &media)
        .unwrap()
        .expect("current retained project is nested-media authority");
    let request = HelperRequest {
        token: "test-token".to_owned(),
        parent_pid: std::process::id(),
        path: media.to_string_lossy().into_owned(),
        head_only: false,
        range: None,
        if_range: None,
        project: HelperProjectAuthority::from_core(&authority),
    };
    let response = helper_response(&request);
    assert_eq!(response.metadata.status, StatusCode::OK.as_u16());
    assert_eq!(response.body, b"project-media");
}

#[test]
fn home_thumbnail_exception_requires_an_exact_file_grant() {
    use tauri::Manager;

    let directory = local_tempdir();
    let bundle = directory.path().join("Recent.opentake");
    std::fs::create_dir_all(&bundle).unwrap();
    let thumbnail = bundle.join("thumbnail.jpg");
    std::fs::write(&thumbnail, b"jpeg").unwrap();
    let app = tauri::test::mock_app();
    let scope = app.handle().asset_protocol_scope();

    scope.allow_directory(&bundle, true).unwrap();
    assert!(!is_home_thumbnail_exception(&scope, &thumbnail, &bundle));
    scope.allow_file(&thumbnail).unwrap();
    assert!(is_home_thumbnail_exception(&scope, &thumbnail, &bundle));
    assert!(matches!(
        non_project_asset_authority(app.handle(), &AppCore::new(), &scope, &thumbnail),
        Some(NonProjectAssetAuthority::ScopeOnly { requested_path, .. })
            if requested_path == normalized_path(&thumbnail)
    ));
}

#[test]
fn exact_external_grants_follow_the_active_project_while_static_roots_remain_available() {
    use opentake_core::ProbedMedia;
    use tauri::Manager;

    let directory = local_tempdir();
    let source_a = directory.path().join("project-a.mp4");
    let source_b = directory.path().join("project-b.mp4");
    std::fs::write(&source_a, b"project-a").unwrap();
    std::fs::write(&source_b, b"project-b").unwrap();

    let core = AppCore::new();
    let bundle_a = directory.path().join("Project-A.opentake");
    core.save_project(Some(bundle_a)).unwrap();
    core.import_media_file(&source_a, "project-a", &ProbedMedia::default())
        .unwrap();
    core.save_project(None).unwrap();

    let replacement = AppCore::new();
    let bundle_b = directory.path().join("Project-B.opentake");
    replacement.save_project(Some(bundle_b.clone())).unwrap();
    replacement
        .import_media_file(&source_b, "project-b", &ProbedMedia::default())
        .unwrap();
    replacement.save_project(None).unwrap();

    let app = tauri::test::mock_app();
    let scope = app.handle().asset_protocol_scope();
    scope.allow_directory(directory.path(), true).unwrap();
    let epoch_a = core.project_revision().project_epoch;
    assert!(matches!(
        non_project_asset_authority(app.handle(), &core, &scope, &source_a),
        Some(NonProjectAssetAuthority::ProjectMedia {
            project_epoch,
            requested_path,
            ..
        }) if project_epoch == epoch_a && requested_path == normalized_path(&source_a)
    ));
    let unreferenced_sibling = directory.path().join("unreferenced.mp4");
    std::fs::write(&unreferenced_sibling, b"unreferenced").unwrap();
    assert!(
        non_project_asset_authority(app.handle(), &core, &scope, &unreferenced_sibling).is_none(),
        "a recursive dialog grant must not expose a sibling absent from the active manifest"
    );

    core.open_project(bundle_b).unwrap();
    assert!(
        non_project_asset_authority(app.handle(), &core, &scope, &source_a).is_none(),
        "persisted exact grants from project A must not remain active after opening B"
    );
    assert!(non_project_asset_authority(app.handle(), &core, &scope, &source_b).is_some());

    let cache = app.path().app_cache_dir().unwrap();
    std::fs::create_dir_all(&cache).unwrap();
    let derived = cache.join("poster.png");
    std::fs::write(&derived, b"png").unwrap();
    scope.allow_directory(&cache, true).unwrap();
    assert!(
        non_project_asset_authority(app.handle(), &core, &scope, &derived).is_some(),
        "application cache/resource roots must not be coupled to the project media set"
    );
}

#[cfg(unix)]
#[test]
fn external_authority_is_bound_to_the_exact_requested_path_across_ancestor_rebinding() {
    use opentake_core::ProbedMedia;
    use std::os::unix::fs::symlink;
    use tauri::Manager;

    let directory = local_tempdir();
    let source_a_dir = directory.path().join("source-a");
    let source_b_dir = directory.path().join("source-b");
    std::fs::create_dir_all(&source_a_dir).unwrap();
    std::fs::create_dir_all(&source_b_dir).unwrap();
    let source_a = source_a_dir.join("clip.mp4");
    let source_b = source_b_dir.join("clip.mp4");
    std::fs::write(&source_a, b"project-a").unwrap();
    std::fs::write(&source_b, b"project-b").unwrap();
    let alias = directory.path().join("selected-source");
    symlink(&source_a_dir, &alias).unwrap();
    let requested = alias.join("clip.mp4");

    let core = AppCore::new();
    core.save_project(Some(directory.path().join("Race.opentake")))
        .unwrap();
    core.import_media_file(&requested, "selected", &ProbedMedia::default())
        .unwrap();
    // Keep the rebound target referenced by the same current project. An epoch-only
    // token would otherwise treat the two distinct paths as interchangeable.
    core.import_media_file(&source_b, "other", &ProbedMedia::default())
        .unwrap();

    let app = tauri::test::mock_app();
    let scope = app.handle().asset_protocol_scope();
    scope.allow_directory(directory.path(), true).unwrap();
    let expected = non_project_asset_authority(app.handle(), &core, &scope, &requested)
        .expect("the selected source must be authorized before the race");
    let request = HelperRequest {
        token: "ancestor-swap-token".to_owned(),
        parent_pid: std::process::id(),
        path: requested.to_string_lossy().into_owned(),
        head_only: false,
        range: None,
        if_range: None,
        project: None,
    };

    std::fs::remove_file(&alias).unwrap();
    symlink(&source_b_dir, &alias).unwrap();
    let (_, final_path) = open_retained_regular_file(&requested).unwrap();
    assert!(paths_equal_for_authority(&final_path, &source_b));
    let rebound = non_project_asset_authority(app.handle(), &core, &scope, &final_path)
        .expect("the other path is independently referenced by the same project");

    assert_ne!(
        expected, rebound,
        "authorization must retain the exact requested path, not just project epoch"
    );
    let response = isolated_response_to_http(
        app.handle(),
        &core,
        &scope,
        None,
        Some(expected),
        &request.token,
        helper_response(&request),
    );
    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "the helper must not publish B bytes under A's pre-race authorization"
    );
}

#[cfg(unix)]
#[test]
fn response_for_request_rejects_an_exact_project_media_alias_rebound_before_authorization() {
    use opentake_core::ProbedMedia;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::symlink;
    use tauri::Manager;

    let selected = local_tempdir();
    let source_a = local_tempdir();
    let source_b = local_tempdir();
    let media_a = source_a.path().join("clip.mp4");
    let media_b = source_b.path().join("clip.mp4");
    std::fs::write(&media_a, b"project-a").unwrap();
    std::fs::write(&media_b, b"project-b").unwrap();
    let alias = selected.path().join("selected-source");
    symlink(source_a.path(), &alias).unwrap();
    let requested = alias.join("clip.mp4");

    let core = AppCore::new();
    core.save_project(Some(selected.path().join("Rebound.opentake")))
        .unwrap();
    core.import_media_file(&requested, "selected", &ProbedMedia::default())
        .unwrap();
    let app = tauri::test::mock_app();
    app.manage(core);
    let scope = app.handle().asset_protocol_scope();
    scope.allow_file(&requested).unwrap();

    std::fs::remove_file(&alias).unwrap();
    symlink(source_b.path(), &alias).unwrap();
    let (_, final_b) = open_retained_regular_file(&requested).unwrap();
    assert!(paths_equal_for_authority(&final_b, &media_b));
    assert!(scope_allows_lexical_path(&scope, &requested));
    assert!(!scope_allows_lexical_path(&scope, &final_b));

    let encoded = percent_encoding::percent_encode(
        requested.as_os_str().as_bytes(),
        percent_encoding::NON_ALPHANUMERIC,
    );
    let request = Request::builder()
        .method(Method::GET)
        .uri(format!("http://opentake.local/{encoded}"))
        .body(Vec::new())
        .unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let response = runtime.block_on(response_for_request(
        app.handle(),
        &scope,
        request,
        Arc::new(Semaphore::new(0)),
    ));

    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "an exact grant for A must not authorize B after the alias is rebound before the request"
    );
}

#[cfg(unix)]
#[test]
fn stable_external_alias_remains_authorized_for_the_same_opened_file() {
    use opentake_core::ProbedMedia;
    use std::os::unix::fs::symlink;
    use tauri::Manager;

    let selected_directory = local_tempdir();
    let source_directory = local_tempdir();
    let source = source_directory.path().join("clip.mp4");
    std::fs::write(&source, b"stable-alias").unwrap();
    let alias = selected_directory.path().join("selected-source");
    symlink(source_directory.path(), &alias).unwrap();
    let requested = alias.join("clip.mp4");

    let core = AppCore::new();
    core.save_project(Some(selected_directory.path().join("Stable.opentake")))
        .unwrap();
    core.import_media_file(&requested, "selected", &ProbedMedia::default())
        .unwrap();
    let app = tauri::test::mock_app();
    let scope = app.handle().asset_protocol_scope();
    scope.allow_file(&requested).unwrap();
    assert!(scope_allows_lexical_path(&scope, &requested));
    assert!(scope_allows_lexical_path(&scope, &source));
    let expected = non_project_asset_authority(app.handle(), &core, &scope, &requested)
        .expect("stable alias is initially authorized");
    let request = HelperRequest {
        token: "stable-alias-token".to_owned(),
        parent_pid: std::process::id(),
        path: requested.to_string_lossy().into_owned(),
        head_only: false,
        range: None,
        if_range: None,
        project: None,
    };

    let response = isolated_response_to_http(
        app.handle(),
        &core,
        &scope,
        None,
        Some(expected),
        &request.token,
        helper_response(&request),
    );

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "an unchanged alias to the initially retained file must remain valid"
    );
    assert_eq!(response.body(), b"stable-alias");
}

#[test]
fn external_authority_rejects_same_path_identity_replacement() {
    use opentake_core::ProbedMedia;
    use tauri::Manager;

    let directory = local_tempdir();
    let requested = directory.path().join("selected.mp4");
    let parked = directory.path().join("selected-original.mp4");
    std::fs::write(&requested, b"original-file").unwrap();

    let core = AppCore::new();
    core.save_project(Some(directory.path().join("Replacement.opentake")))
        .unwrap();
    core.import_media_file(&requested, "selected", &ProbedMedia::default())
        .unwrap();
    let app = tauri::test::mock_app();
    let scope = app.handle().asset_protocol_scope();
    scope.allow_file(&requested).unwrap();
    let expected = non_project_asset_authority(app.handle(), &core, &scope, &requested)
        .expect("original path is initially authorized");
    let request = HelperRequest {
        token: "same-path-replacement-token".to_owned(),
        parent_pid: std::process::id(),
        path: requested.to_string_lossy().into_owned(),
        head_only: false,
        range: None,
        if_range: None,
        project: None,
    };

    std::fs::rename(&requested, &parked).unwrap();
    std::fs::write(&requested, b"replacement-file").unwrap();
    let response = isolated_response_to_http(
        app.handle(),
        &core,
        &scope,
        None,
        Some(expected),
        &request.token,
        helper_response(&request),
    );

    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "the same pathname must not authorize a different retained file identity"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn external_authority_accepts_windows_case_equivalent_final_paths() {
    let expected = NonProjectAssetAuthority::ProjectMedia {
        project_epoch: 7,
        requested_path: PathBuf::from(r"C:\Media\Clip.mp4"),
        initial_final_path: PathBuf::from(r"C:\Media\Clip.mp4"),
        initial_etag: "\"volume-file-length-time\"".to_owned(),
    };
    let refreshed = expected.clone();

    assert!(non_project_response_matches_authority(
        &expected,
        Path::new(r"c:\media\CLIP.MP4"),
        Some("\"volume-file-length-time\""),
        Some(&refreshed),
    ));
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn helper_rejects_a_parent_that_is_not_the_same_executable() {
    // The Rust test harness is launched by Cargo, so its live parent is a
    // different executable. A self-issued token/PID pair is insufficient.
    let parent_pid = actual_parent_process_id().unwrap();
    assert!(!parent_is_same_executable(parent_pid).unwrap());
}

#[cfg(unix)]
#[test]
fn helper_request_pipe_reaches_eof_before_waiting_for_the_response() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let mut child = Command::new("/bin/sh")
            .arg("-c")
            .arg("cat >/dev/null; printf ready")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let mut stdout = child.stdout.take().unwrap();

        let exchange = write_helper_request_before_response(stdin, b"request", async {
            let mut response = [0_u8; 5];
            stdout
                .read_exact(&mut response)
                .await
                .map_err(|_| IsolatedHelperError::Io)?;
            Ok(response)
        });
        let response = tokio::time::timeout(Duration::from_millis(500), exchange)
            .await
            .expect("helper must observe request EOF before the parent waits for its response")
            .unwrap();
        assert_eq!(&response, b"ready");
        assert!(child.wait().await.unwrap().success());
    });
}

#[cfg(unix)]
#[test]
fn timed_out_isolated_workers_are_killed_reaped_and_capacity_recovers() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let permits = Arc::new(Semaphore::new(MAX_CONCURRENT_READS));
        let process_slots = Arc::new(Semaphore::new(MAX_CONCURRENT_READS));
        let mut workers = Vec::new();
        for _ in 0..MAX_CONCURRENT_READS {
            let permits = permits.clone();
            let process_slots = process_slots.clone();
            workers.push(tokio::spawn(async move {
                let _permit = permits.acquire_owned().await.unwrap();
                let process_slot = process_slots.try_acquire_owned().unwrap();
                let mut child = Command::new("/bin/sleep")
                    .arg("30")
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .kill_on_drop(true)
                    .spawn()
                    .unwrap();
                let process_id = child.id().unwrap();
                assert!(
                    tokio::time::timeout(Duration::from_millis(100), child.wait())
                        .await
                        .is_err()
                );
                terminate_or_quarantine(child, process_slot).await;
                process_id
            }));
        }
        let mut process_ids = Vec::new();
        for worker in workers {
            process_ids.push(worker.await.unwrap());
        }
        assert_eq!(permits.available_permits(), MAX_CONCURRENT_READS);
        assert_eq!(process_slots.available_permits(), MAX_CONCURRENT_READS);

        for process_id in process_ids {
            // SAFETY: signal 0 only probes whether the already-reaped PID exists.
            assert_eq!(unsafe { libc::kill(process_id as i32, 0) }, -1);
            assert_eq!(
                std::io::Error::last_os_error().raw_os_error(),
                Some(libc::ESRCH)
            );
        }

        let _permit = tokio::time::timeout(Duration::from_secs(1), permits.acquire())
            .await
            .expect("worker capacity recovers")
            .unwrap();
        let directory = local_tempdir();
        let path = directory.path().join("normal.jpg");
        std::fs::write(&path, b"normal").unwrap();
        let response = serve_open_file(&path, None, false, None).unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.body(), b"normal");
    });
}

#[test]
fn unreapable_wait_is_bounded_and_four_quarantines_fail_the_fifth_fast() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let started = std::time::Instant::now();
        assert!(
            !bounded_reap(
                std::future::pending::<std::io::Result<std::process::ExitStatus>>(),
                Duration::from_millis(25),
            )
            .await
        );
        assert!(started.elapsed() < Duration::from_secs(1));

        let slots = Arc::new(Semaphore::new(MAX_CONCURRENT_READS));
        let quarantined = (0..MAX_CONCURRENT_READS)
            .map(|_| slots.clone().try_acquire_owned().unwrap())
            .collect::<Vec<_>>();
        let spawned = std::sync::atomic::AtomicUsize::new(0);
        if slots.clone().try_acquire_owned().is_ok() {
            spawned.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        assert_eq!(spawned.load(std::sync::atomic::Ordering::Relaxed), 0);
        assert_eq!(slots.available_permits(), 0);
        drop(quarantined);
        assert_eq!(slots.available_permits(), MAX_CONCURRENT_READS);
    });
}
