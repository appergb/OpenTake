use serde_json::Value;

fn config() -> Value {
    serde_json::from_str(include_str!("../tauri.conf.json")).expect("tauri.conf.json must parse")
}

fn windows_config() -> Value {
    serde_json::from_str(include_str!("../tauri.windows.conf.json"))
        .expect("tauri.windows.conf.json must parse")
}

#[test]
fn packaged_webview_csp_is_explicit_and_local_only() {
    let config = config();
    let csp = config["app"]["security"]["csp"]
        .as_object()
        .expect("packaged WebView CSP must be enabled");

    assert_eq!(
        csp.get("default-src").and_then(Value::as_str),
        Some("'self'")
    );
    assert_eq!(
        csp.get("object-src").and_then(Value::as_str),
        Some("'none'")
    );
    assert_eq!(
        csp.get("frame-ancestors").and_then(Value::as_str),
        Some("'none'")
    );

    let connect = csp
        .get("connect-src")
        .and_then(Value::as_str)
        .expect("connect-src must be explicit");
    assert!(connect.contains("http://127.0.0.1:*"));
    assert!(connect.contains("ipc:"));
    assert!(!connect.contains("https:"));
    assert!(!connect.contains("http://*"));
    assert!(!connect.contains("ws:"));
    assert!(!connect.contains("ws://*"));

    for directive in ["img-src", "media-src"] {
        let value = csp
            .get(directive)
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("{directive} must be explicit"));
        assert!(value.contains("asset:"));
        assert!(value.contains("http://asset.localhost"));
        assert!(!value.contains("https:"));
        assert!(!value.contains('*'));
    }
}

#[test]
fn packaged_asset_scope_has_no_global_or_home_wildcard() {
    let config = config();
    let scope = config["app"]["security"]["assetProtocol"]["scope"]
        .as_object()
        .expect("asset scope must use explicit allow/deny form");
    let allowed = scope["allow"]
        .as_array()
        .expect("asset allow scope must be an array")
        .iter()
        .map(|value| value.as_str().expect("asset scope entry must be text"))
        .collect::<Vec<_>>();

    assert_eq!(
        allowed,
        vec![
            "$APPCACHE/**/*",
            "$APPDATA/OpenTake/Library/**/*",
            "$RESOURCE/**/*"
        ]
    );
    assert!(!allowed
        .iter()
        .any(|entry| { matches!(*entry, "**" | "**/*" | "$HOME/**" | "$HOME/**/*") }));

    let denied = scope["deny"]
        .as_array()
        .expect("sensitive-home deny scope must be present")
        .iter()
        .map(|value| value.as_str().expect("deny scope entry must be text"))
        .collect::<Vec<_>>();
    assert!(denied.contains(&"$HOME/.ssh/**/*"));
    assert!(denied.contains(&"$HOME/.gnupg/**/*"));
    assert!(denied.contains(&"$HOME/.aws/**/*"));
}

#[test]
fn main_window_capability_exposes_no_shell_or_filesystem_commands() {
    let capability: Value = serde_json::from_str(include_str!("../capabilities/default.json"))
        .expect("default capability must parse");
    let permissions = capability["permissions"]
        .as_array()
        .expect("permissions must be an array")
        .iter()
        .map(|value| value.as_str().expect("permission must be a string"))
        .collect::<Vec<_>>();

    assert!(permissions.iter().all(|permission| {
        !permission.starts_with("shell:")
            && !permission.starts_with("fs:")
            && !permission.starts_with("http:")
            && !permission.starts_with("process:")
    }));
}

#[test]
fn dialog_asset_grants_are_persisted_after_fs_initialization() {
    let manifest = include_str!("../Cargo.toml");
    assert!(manifest.contains("tauri-plugin-fs = \"2\""));
    assert!(manifest.contains(
        "tauri-plugin-persisted-scope = { version = \"2\", features = [\"protocol-asset\"] }"
    ));

    let runtime = include_str!("../src/lib.rs");
    let fs = runtime
        .find(".plugin(tauri_plugin_fs::init())")
        .expect("fs plugin must be initialized");
    let persisted = runtime
        .find(".plugin(tauri_plugin_persisted_scope::init())")
        .expect("persisted-scope plugin must be initialized");
    assert!(fs < persisted, "fs must initialize before persisted-scope");
}

#[test]
fn windows_bundle_installs_webview2_without_network_access() {
    let config = windows_config();
    let install_mode = &config["bundle"]["windows"]["webviewInstallMode"];

    assert_eq!(install_mode["type"].as_str(), Some("offlineInstaller"));
    assert_eq!(install_mode["silent"].as_bool(), Some(true));
}

#[test]
fn windows_bundle_ships_the_linked_onnxruntime_beside_the_executable() {
    let media_manifest = include_str!("../../crates/opentake-media/Cargo.toml");
    assert!(
        media_manifest
            .contains("features = [\"std\", \"ndarray\", \"download-binaries\", \"copy-dylibs\"]"),
        "ort-sys must copy the exact downloaded runtime beside Windows binaries"
    );

    let config = windows_config();
    let resources = config["bundle"]["resources"]
        .as_object()
        .expect("the Windows bundle must map runtime resources explicitly");
    assert_eq!(
        resources
            .get("../target/opentake-runtime/onnxruntime.dll")
            .and_then(Value::as_str),
        Some("onnxruntime.dll")
    );

    let build_script = include_str!("../build.rs");
    assert!(build_script.contains("profile_dir.join(\"deps/onnxruntime.dll\")"));
    assert!(build_script.contains("workspace_target.join(\"opentake-runtime/onnxruntime.dll\")"));
}

#[test]
fn every_windows_tauri_ci_job_provisions_packaged_sidecars_first() {
    // Git may materialize the workflow with CRLF on Windows runners. Normalize
    // before finding YAML job boundaries so this contract tests the workflow,
    // not the checkout's line-ending policy.
    let workflow = include_str!("../../.github/workflows/ci.yml").replace("\r\n", "\n");
    for (job, next_job) in [
        ("  windows-product:\n", "  windows-security:\n"),
        ("  windows-security:\n", "  web:\n"),
        ("  windows-library-security:\n", "  safe-filesystem:\n"),
    ] {
        let body = workflow
            .split_once(job)
            .unwrap_or_else(|| panic!("missing CI job {job}"))
            .1
            .split_once(next_job)
            .unwrap_or_else(|| panic!("missing CI boundary after {job}"))
            .0;
        let provision = body
            .find("python scripts/provision_ffmpeg_sidecars.py --target x86_64-pc-windows-msvc")
            .unwrap_or_else(|| panic!("{job} must provision packaged sidecars"));
        let tauri_compile = body
            .find("opentake-tauri")
            .or_else(|| body.find("tauri build"))
            .unwrap_or_else(|| panic!("{job} must exercise Tauri"));

        assert!(
            provision < tauri_compile,
            "{job} must provision sidecars before compiling Tauri"
        );
    }
}
