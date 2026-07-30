# Generation dispatch/finalization runtime evidence — 2026-07-29

Scope: `AG-generation-dispatch-finalization + generation-upscale-finalization` (`implementation-slice-f60bdd3e656a7cb0`). All provider responses are deterministic local `MockTransport` fixtures; no paid provider request or user credit was consumed.

## Production-path artifact assertions

- `generate_image`: production Dispatcher → shared Tauri GenerationBridge → configured fal adapter; two durable placeholders are returned before background work, paired in output-index order, probed as 2×2 PNG, persisted `ready`, and resolve to real project media files.
- `generate_video`: configured fal adapter returns a valid 16×16 MP4 fixture; the original placeholder identity becomes a durable video asset with probed dimensions.
- `generate_audio`: configured OpenAI adapter returns a valid WAV fixture; the placeholder becomes a durable audio asset with `hasAudio=true` and positive probed duration.
- `upscale_media`: configured Replicate adapter uploads a 2×2 source fixture and returns 4×4 PNG; the output is exactly 2x and the source file bytes remain byte-for-byte identical.
- Cancellation leaves no imported result; ready outputs in a partially finalized N-output job remain ready while non-terminal siblings become cancelled.
- Restart with provider id resumes and finalizes; restart before provider id persists `GENERATION_RESTART_RETRY_REQUIRED` without any resubmission. Retry requires fresh cost authorization and creates a new job.
- Authentication/rate-limit failures persist fixed safe codes. Local/private result targets are rejected. Provider body, credentials, and signed result URLs are absent from project persistence.

## Verification commands

```text
cargo test -p opentake-agent --test generation_dispatch placeholder_persist_finalize_all_results_and_failures -- --exact
cargo test -p opentake-agent --test generation_dispatch placeholder_persists_and_every_terminal_result_finalizes_once -- --exact
cargo test -p opentake-gen upscale_uses_first_upload_as_source
cargo test -p opentake-gen byok_submit_then_watch_to_succeeded
cargo test -p opentake-tauri generation::tests
cargo test -p opentake-core --test generation_persistence
cargo test -p opentake-media trim_video_range_materializes_only_visible_window_and_honors_cancel
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast -q
pnpm build
pnpm test
git diff --check
```

Final result: all commands passed. Workspace tests reported zero failures; repository-declared ignored tests remained ignored. Web result: 70 test files, 703 tests passed. Vite emitted only the pre-existing chunk-size/dynamic-import optimization warnings.

## Windows CI correction

The first exact-tree Windows full-product run (`30459985870`, job `90603221524`) failed all three `generation_persistence` tests with Win32 error 32 while replacing `Generation.opentake`. The durable generation transaction had correctly staged a complete sibling bundle, but the live session still retained an open `ProjectRoot` directory handle when the journaled publisher tried to rename the old target. Windows forbids that rename even though macOS permits it.

The corrected path copies the complete bundle through the retained root, explicitly consumes and closes that root, and only then invokes the existing journal/backup/restore publication commit. Save-As continues to retain its distinct source root. `complete_publish_replaces_the_owned_source_root` pins this same-target Windows requirement, and the three generation persistence tests plus full format/Clippy/workspace gates pass locally after the correction. The replacement keeps whole-bundle atomicity; it does not fall back to two independently visible JSON writes.

The next exact-tree Windows run (`30462600088`, job `90612253763`) proved that two Tauri-layer handle lifetimes also had to be corrected. Six generation tests failed: four finalization flows held an uncommitted project-media leaf without `FILE_SHARE_DELETE` while complete-bundle publication renamed the project, and two restart tests kept the pre-restart `AppCore` alive while independently opening the same bundle. The import handle now shares deletion on Windows while retaining delete access, identity validation, and handle-relative rollback. The restart fixtures explicitly drop the old core before constructing the new process-equivalent core.

A later full-product Windows run (`30538580323`, job `90857715826`) narrowed the remaining error-32 failures to five successful-provider finalization paths. Sharing deletion on the leaf was insufficient because `ProjectMediaCapability` also retained root and `media/` directory handles across the live bundle rename. Finalization now streams the downloaded artifact directly into the unpublished sibling stage through `ProjectRoot`; the generated media leaf, `media.json`, and `generation-log.json` therefore share the existing journaled whole-bundle commit. The stream is bounded to the downloader-recorded byte size, and an undersized, oversized, or failed stream aborts without changing the live bundle. `generated_media_stream_failure_preserves_the_live_bundle_byte_exact` pins that rollback guarantee.

After this correction, local `generation::tests` pass 9/9, the complete workspace test run reports zero failures (only repository-declared hardware probes remain ignored), workspace Clippy with warnings denied passes, and formatting/diff checks pass. The next exact-tree Windows CI rerun is the authoritative remaining platform confirmation.
