use serde_json::Value;

fn config() -> Value {
    serde_json::from_str(include_str!("../tauri.conf.json")).expect("tauri.conf.json must parse")
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
