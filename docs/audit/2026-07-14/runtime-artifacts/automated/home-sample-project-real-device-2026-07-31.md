# Home sample project real-device verification (2026-07-31)

## Scope

- Contract: `HS-new-open-sample`
- Owning requirement: `requirement-31a4c6a115076c19`
- Packaged artifact: `target/debug/bundle/macos/OpenTake.app`
- DMG artifact: `target/debug/bundle/dmg/OpenTake_1.0.0_aarch64.dmg`

## RED evidence

- `cargo test -p opentake-tauri failed_materialization_rolls_back_entire_sample_directory` initially failed to compile because `SampleProjectService`, `ResolvedSample`, and `SampleDownload` did not exist.
- The existing Web suite remained green, while the newly declared Home interaction test failed because the tutorial sample button and its action route did not exist.

## GREEN evidence

- `cargo fmt --all -- --check`: PASS.
- `cargo test -p opentake-tauri failed_materialization_rolls_back_entire_sample_directory`: PASS (`1 passed`).
- All sample service tests: PASS (`3 passed`), including valid publish/progress and the offline tutorial contents.
- `cargo clippy -p opentake-tauri --all-targets -- -D warnings`: PASS.
- `cargo test --workspace --no-fail-fast`: PASS across the workspace; hardware-only GPU/audio probes remain explicitly ignored by their owning runners.
- `pnpm -C web test -- --run`: PASS (`76` files, `752` tests).
- `pnpm -C web build`: PASS. Vite retained its pre-existing mixed dynamic-import and bundle-size advisories.
- `git diff --check`: PASS.

## Artifact identity

- App executable SHA-256: `299d1c3efd845ce7c61ff84825a22ca3acb40c85b2936584723ae4bd97999179`.
- DMG SHA-256: `0f622f0b91a1e3f20929c35a1082aa6183797d8669931033fb725956206d11ab`.
- Tauri produced both bundles successfully from the tested source tree.

## Packaged macOS walkthrough

The exact `.app` path above was targeted through the macOS accessibility surface. An older running process was first quit, then the just-built artifact was launched so the verification could not reuse stale code.

1. Home rendered an accessible `示例项目` region with `产品演示`, `快速教程`, and `模板项目` buttons.
2. Before opening a sample, Home showed exactly three existing recent projects: `TalkingHeadQA`, `Task7NativeSaveAs-20260731`, and `Task9Empty-20260731`.
3. Activating `快速教程` entered the editor and displayed `教程项目已打开。按照时间线中的示例开始编辑。`.
4. The editor reported a `00:12:00` timeline at `30 fps`, with one `T1` track and three sequential clips: `sample-text-0`, `sample-text-1`, and `sample-text-2`.
5. Direct inspection of the materialized cache bundle confirmed one track, three 120-frame text clips at frames `0`, `120`, and `240`, for `360` frames total.
6. Returning Home still showed the same three recent projects. The cache bundle was not registered as user work.
7. Activating `快速教程` a second time succeeded with the same three clips and tutorial toast, exercising replacement of the existing stable cache entry.

## Failure and recovery coverage

- The exact Rust ownership test injects a failure after the first file write and verifies that the stable sample directory does not exist and that the entire staging directory is removed.
- The action-layer Web test rejects materialization and verifies that `projectOpen` is not called, the editor remains on Home, existing recents are unchanged, and a localized error toast is displayed.
- The packaged walkthrough did not intentionally corrupt or intercept external network traffic; deterministic service and action tests own that destructive failure path.

## Environment maintenance

The first focused rerun after packaging exhausted the local disk while Rust attempted to write a test archive. Only the repository's regenerable `target/debug/incremental` cache was removed (about 10 GiB); the verified `.app`, DMG, source files, and user project data were preserved. The focused checks then passed.
