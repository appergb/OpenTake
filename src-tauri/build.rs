fn main() {
    tauri_build::build();

    // tauri-build embeds Common Controls v6 in the shipped binary resource,
    // but its resource helper deliberately links only `[[bin]]` targets. The
    // lib-test harness still contains rfd's TaskDialogIndirect import, so it
    // must carry the same activation-context dependency or Windows resolves
    // comctl32 v5 and exits before main with STATUS_ENTRYPOINT_NOT_FOUND.
    if std::env::var("TARGET").is_ok_and(|target| target.contains("windows-msvc")) {
        embed_resource::compile_for_tests("windows-test-manifest.rc", embed_resource::NONE)
            .manifest_required()
            .expect("failed to embed the Windows test activation manifest");
    }
}
