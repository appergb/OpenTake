fn main() {
    let is_windows_msvc =
        std::env::var("TARGET").is_ok_and(|target| target.contains("windows-msvc"));
    let attributes = if is_windows_msvc {
        // Keep tauri-build's icon and version resources, but let the linker
        // below provide the single manifest shared by binary and test targets.
        tauri_build::Attributes::new()
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest())
    } else {
        tauri_build::Attributes::new()
    };
    tauri_build::try_build(attributes).expect("failed to run tauri build script");

    // The lib-test harness contains rfd's TaskDialogIndirect import too. Every
    // linked Windows image must activate Common Controls v6 or it exits before
    // main with STATUS_ENTRYPOINT_NOT_FOUND. A single linker-generated manifest
    // avoids duplicating tauri-build's RT_MANIFEST resource in the shipped EXE.
    if is_windows_msvc {
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED,ID=1");
        println!(
            "cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' \
             name='Microsoft.Windows.Common-Controls' version='6.0.0.0' \
             processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
    }
}
