fn main() {
    tauri_build::build();

    // tauri-build embeds Common Controls v6 only in `[[bin]]` resources. The
    // lib-test harness still contains rfd's TaskDialogIndirect import, so every
    // Windows artifact that actually links must carry the same activation
    // dependency or Windows resolves comctl32 v5 and exits before main with
    // STATUS_ENTRYPOINT_NOT_FOUND. Keep these flags test-only because the
    // shipped binary already receives resource #1 from tauri-build.
    if std::env::var("TARGET").is_ok_and(|target| target.contains("windows-msvc")) {
        println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED,ID=1");
        println!(
            "cargo:rustc-link-arg-tests=/MANIFESTDEPENDENCY:type='win32' \
             name='Microsoft.Windows.Common-Controls' version='6.0.0.0' \
             processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
    }
}
