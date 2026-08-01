fn main() {
    #[cfg(windows)]
    stage_directml();
    tauri_build::build()
}

#[cfg(windows)]
fn stage_directml() {
    use std::fs;
    use std::path::PathBuf;

    // The checksum-pinned static ONNX Runtime distribution includes a
    // DirectML provider companion DLL. ort-sys `copy-dylibs` places it in the
    // active Cargo profile, and the direct Windows build-dependency guarantees
    // that copy finishes before this script runs.
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR must be set"));
    let profile_dir = out_dir
        .ancestors()
        .nth(3)
        .expect("OUT_DIR must be inside a Cargo profile directory");
    let source = profile_dir.join("DirectML.dll");
    assert!(
        source.is_file(),
        "ort-sys did not stage the required DirectML provider at {}",
        source.display()
    );

    let workspace_target = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target");
    for destination in [
        workspace_target.join("opentake-runtime/DirectML.dll"),
        profile_dir.join("deps/DirectML.dll"),
    ] {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .unwrap_or_else(|error| panic!("failed to create {}: {error}", parent.display()));
        }
        copy_if_changed(&source, &destination);
    }
}

#[cfg(windows)]
fn copy_if_changed(source: &std::path::Path, destination: &std::path::Path) {
    let source_bytes = std::fs::read(source)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", source.display()));
    if std::fs::read(destination).ok().as_deref() == Some(source_bytes.as_slice()) {
        return;
    }
    if std::fs::symlink_metadata(destination)
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        std::fs::remove_file(destination).unwrap_or_else(|error| {
            panic!(
                "failed to replace stale provider link {}: {error}",
                destination.display()
            )
        });
    }
    std::fs::write(destination, source_bytes)
        .unwrap_or_else(|error| panic!("failed to stage {}: {error}", destination.display()));
}
