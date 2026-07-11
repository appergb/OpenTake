# Verification Report

## Branch

- Branch: `recovery/superpowers-integration-20260708-v2`
- Verification date: `2026-07-09`
- Security/code verification commit: `2193ac7`
- Baseline: `origin/main` at `ac50dc8`; stale branch heads were selectively replayed, not merged wholesale.

## Required Commands

| Command | Status | Evidence |
|---|---|---|
| `cargo fmt --all --check` | Passed | Exited 0 after Task 6 security fixes. |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed | Exited 0. Cargo printed a future-incompatibility notice for transitive `block v0.1.6`, but clippy diagnostics were clean. |
| `cargo test --workspace` | Passed | Exited 0 after the MCP body-limit regression test was added. Workspace suites included `opentake-agent` and `opentake-tauri`; probe tests remained ignored by design. |
| `cargo clippy -p opentake-tauri --no-default-features --all-targets -- -D warnings` | Passed | Exited 0 with the same transitive `block v0.1.6` future-incompatibility notice. |
| `pnpm -C web build` | Passed | Exited 0 on Vite `8.1.3`. Warnings remained for known dynamic-import/chunk-size diagnostics. |
| `pnpm -C web test` | Passed | Exited 0 on Vitest `4.1.10`: `48` files and `523` tests passed. |

## Security Checks

| Check | Status | Evidence |
|---|---|---|
| Cargo audit tooling | Passed | Global `cargo-audit` was not installed; local `target/cargo-tools/bin/cargo-audit` was installed and run. It exited 0 after dependency fixes. |
| RustSec vulnerabilities | Fixed | Fixed `crossbeam-epoch`, `quick-xml` through `plist`, `quinn-proto`, and `rmcp`. The remaining audit output is the repository's allowed warning set, including GTK3 stack maintenance warnings and advisories for `anyhow`, `glib`, and `memmap2`. |
| pnpm audit | Passed | `pnpm -C web audit --audit-level moderate` returned `No known vulnerabilities found`. |
| Secret scan | Passed | The configured `rg` scan found only false positives: the scan command in planning docs, branch-register task filenames like `task-5-*`, documented placeholder env var names, historical upstream-analysis text, and env var name references in `crates/opentake-gen/src/keys.rs`. |
| MCP inline bytes limit | Passed | Added `IMPORT_BYTES_BASE64_MAX`, `IMPORT_BYTES_DECODED_MAX`, and `MCP_REQUEST_BODY_MAX`; dispatcher rejects oversized base64 before bridge work, and Tauri rejects oversized decoded bytes before project writes. |
| MCP HTTP body limit | Passed | `/mcp` is wrapped with `RequestBodyLimitLayer`. `crates/opentake-agent/tests/mcp_http.rs` sends `MCP_REQUEST_BODY_MAX + 1` bytes through `build_router` and asserts `413 Payload Too Large`. |

## Desktop Runtime

| Flow | Status | Evidence |
|---|---|---|
| Build desktop app | Passed | Dedicated runtime Agent launched the dev app with `./web/node_modules/.bin/tauri dev --no-watch -v`; Rust finished building and Vite `8.1.3` became ready. |
| Launch app | Passed | Dev app process opened an `OpenTake` window, and the MCP listener was observed on `127.0.0.1:19789`. |
| Import media | Passed | Earlier desktop runtime Agent imported fallback ffmpeg media through MCP and added it to the timeline; post-security smoke verified the same app/MCP stack still launches after dependency updates. |
| Timeline edit and playback | Passed | Earlier runtime Agent performed MCP timeline edits, including opacity and reversed clip state, and `inspect_timeline` returned JPEG frame metadata. Workspace tests after Task 6 cover reverse mapping, freeze frame, save-as-media, export, fallback playback, and edit command behavior. |
| Agent/MCP action | Passed | Post-security smoke initialized MCP, sent `notifications/initialized`, listed tools, and called `get_timeline`. Earlier runtime smoke also executed editing tools and `activate_workflow`. |
| Known local limitation | Recorded | Direct UI click automation for some menu flows was limited by macOS Accessibility in the first runtime pass, so verification used MCP/runtime calls plus focused web/Rust tests. The later smoke confirmed minimal UI presence was not blocked. |

## Agent Reviews

| Agent | Scope | Result |
|---|---|---|
| Godel | Security review | Found a real medium issue: unbounded `import_media.source.bytes` could exhaust memory/disk. The issue was fixed in Task 6. |
| Franklin | Security re-review | Approved. Verified the real `/mcp` body-limit regression test and ran `cargo fmt --all --check`, `cargo test -p opentake-agent --test mcp_http -- --nocapture`, targeted `git diff --check`, and a focused secret/risky-pattern scan. |
| Herschel | Desktop/runtime smoke | Passed. Verified dev launch, window presence, MCP initialization, tool listing, `get_timeline`, and over-limit HTTP body rejection. |
| Darwin | Desktop/runtime edit flow | Passed before the final security dependency update. Verified import, timeline edit, reverse state, `inspect_timeline`, and workflow activation; final workspace tests and post-update smoke covered the changed code path afterward. |

## Completion Notes

- Branch integration is recorded in `docs/superpowers/archive/2026-07-08-branch-integration-register.md`.
- The unfinished Open Code reverse-clip task is complete across domain, ops, render, Tauri, Agent/MCP, and web playback surfaces.
- Stale branches were not direct-merged because they would have deleted current main-line files; still-relevant deltas were selectively replayed and tested.
- Security-critical dependency vulnerabilities were fixed, and the MCP import surface now has explicit base64, decoded byte, and HTTP request-body limits.

## 2026-07-10/11 Wave 1A Reviewed Update

This addendum does not replace the historical branch, commit, or `48` files /
`523` tests figures above. It records the later full-convergence review at
integration HEAD `1f2bf4e49877c145b7c7990e2a7ad85b32685aed`.

### Reviewed slices

| Slice | Final SHA | Review result |
|---|---|---|
| Runtime dependency/libmpv removal | `bffbcf64d991ac39d8dcef84d95298e968bed6f7` | APPROVE |
| Project runtime identity | `2eff907cfc9cbb816a1a961d546c93f4ed363f7e` | APPROVE |
| Playback session/publication identity | `e2daeb279a337e22c412e31ab3acd95acbcb4456` | APPROVE |
| Bounded playback workers/control | `ba5b1ceac463f01c6830fa2e5932734d99d66eeb` | APPROVE |
| Exact cancellable bootstrap | `24ab2590ce964fd04f3dc960be23c26408270ef4` | APPROVE |
| Project-scoped media prewarm | `f99da16c27b440a10715cc4b2e50b9ab713fafa9` | APPROVE |
| Live exact transport evidence | `3fe09766819b0d07b17d94a7c870ac744d41129c` | APPROVE |
| Playback capability route | `8b47e64a8e6c679f6bb3605ac1c92fb1a98415b5` | APPROVE |
| Retained Rust frame UI | `dc83284319bdd7ec816a0175f1e97b90a9bc5e1a` | APPROVE after repaired exact-bundle QA |
| Project/source visual-cache UI | `1f2bf4e49877c145b7c7990e2a7ad85b32685aed` | APPROVE; Critical 0 / Important 0 / Minor 0 |

### Exact pre-document gates

The persisted safety-snapshot `logs/04a-tests-evidence-integration` evidence
reports:

- focused Web: 5 files, 62/62 tests passed;
- full Web: 54 files, 570/570 tests passed;
- playback integration: 7/7 passed, 0 ignored;
- live playback transport: 6/6 passed, 0 ignored;
- Rust workspace: passed, with seven deliberate ignored tests still reported.

The seven ignored tests are one `opentake-media` ffmpeg+ffprobe environment
probe and six real-device probes (three export and three playback). They are not
counted as passes. All named playback/media assertions in the focused playback
and transport commands executed and passed.

### Evidence boundaries

The 2026-07-10 installed application, the 2026-07-11 Task 6.2 detached bundle,
and the final Task 6.3 detached bundle have different executable and app-tree
hashes. Results are attributed only to the artifact actually exercised; see
[`2026-07-10-playback-cache-installed-app-qa.md`](2026-07-10-playback-cache-installed-app-qa.md).
Unsupported capabilities are fail-closed, not verified renderer support.
Installed-app export UI artifact verification is still not yet complete.
