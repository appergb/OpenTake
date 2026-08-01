fn main() {
    tauri_build::build();

    // tauri-build embeds Common Controls v6 only in `[[bin]]` resources. The
    // lib-test harness still contains rfd's TaskDialogIndirect import, so every
    // Windows artifact that actually links must carry the same activation
    // dependency or Windows resolves comctl32 v5 and exits before main with
    // STATUS_ENTRYPOINT_NOT_FOUND. The global link arg reaches the lib-test
    // harness; Cargo's `rustc-link-arg-tests` does not.
    if std::env::var("TARGET").is_ok_and(|target| target.contains("windows-msvc")) {
        println!(
            "cargo:rustc-link-arg=/MANIFESTDEPENDENCY:\"type='win32' \
             name='Microsoft.Windows.Common-Controls' version='6.0.0.0' \
             processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'\""
        );
    }
}
