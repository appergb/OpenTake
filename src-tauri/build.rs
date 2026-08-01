fn main() {
    tauri_build::build();

    // tauri-build embeds Common Controls v6 in the shipped binary resource,
    // but its resource helper deliberately links only `[[bin]]` targets. The
    // lib-test harness still contains rfd's TaskDialogIndirect import, so it
    // must carry the same activation-context dependency or Windows resolves
    // comctl32 v5 and exits before main with STATUS_ENTRYPOINT_NOT_FOUND.
    if std::env::var("TARGET").is_ok_and(|target| target.contains("windows-msvc")) {
        println!(
            "cargo:rustc-link-arg-tests=/MANIFESTDEPENDENCY:\"type='win32' \
             name='Microsoft.Windows.Common-Controls' version='6.0.0.0' \
             processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'\""
        );
    }
}
