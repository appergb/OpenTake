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
