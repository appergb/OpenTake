# Task 6 report — real restart and security matrix

Date: 2026-08-14

## Status

Complete. The opt-in integration test exercised the real loopback Streamable HTTP listener, formal `rmcp` client, temporary persisted catalog, and uniquely namespaced macOS Keychain credentials. No user-owned `docs/audit/2026-08-07/**` file was modified by this task.

## Implementation

- Added `src-tauri/tests/external_mcp_integration.rs` as a feature-required target; once selected, it retains a safe runtime skip and explicit `OPENTAKE_RUN_REAL_KEYCHAIN_MCP=1` opt-in.
- Added a narrow integration harness around the production `ExternalMcpState`, gated behind the disabled-by-default `external-mcp-integration` feature and a `required-features` integration target. Normal library and release builds contain no public harness surface. Network admission still uses the production bearer authorizer, Host/Origin checks, dispatcher, request scopes, live-project gate, and listener lifecycle.
- Used the formal `rmcp 2.2.0` Streamable HTTP client for initialization, discovery, and tool calls. Raw `reqwest` is limited to adversarial Host/Origin probes.
- Pinned `sse-stream = 0.2.4` as a dev dependency because rmcp 2.2's reqwest transport calls the compatibility alias introduced in that patch release.
- Captured tracing events, child-process output, matrix logs, and all temporary catalog files; compared their raw bytes against every complete generated bearer and required zero matches.
- Used one UUID-suffixed Keychain service per run, tracked only the exact accounts returned by this run's pairing receipts, deleted only those accounts, and verified each deletion by readback.
- Replaced the cancellation probe's lossy `notify_waiters` edge with a stored `notify_one` permit and added a 2,048-iteration concurrent signal/wait regression.
- Strengthened the live undo row to inspect folder state after creation, rejected foreign undo, and successful owner undo.

## Matrix result

- Authenticated restart using the same temporary catalog and Keychain namespace: PASS.
- Targeted revoke while a surviving credential and listener remain usable: PASS.
- Remote Host and Origin rejection with a valid bearer: PASS (HTTP 403).
- Project-switch cancellation of an in-flight rmcp `import_media`, with no media committed to the new project: PASS.
- Cross-session undo isolation with owner undo preserved: PASS.
- Fixed-port conflict without fallback binding, followed by recovery: PASS.
- Socket closure after disable: PASS.
- Socket closure after an actual child process exits while its listener is alive: PASS.
- Full generated bearer scan across captured logs/process output/catalog bytes: PASS, zero matches.
- Exact Keychain account cleanup and readback: PASS.

## TDD evidence

### RED

The integration test was written before its public seam and dependencies. Its first `--no-run` build failed because `rmcp` was unresolved, `external_mcp` was private, and `ExternalMcpIntegrationHarness` did not exist.

The next build exposed a version-specific dependency failure: rmcp 2.2 called `SseStream::from_bytes_stream`, while the lock selected sse-stream 0.2.3 with only the older `from_byte_stream` spelling. Exact 0.2.4 resolution fixed compilation. The first live test then reproduced a rustls no-provider panic; the test process now installs the existing ring provider before constructing the rmcp reqwest transport.

### GREEN

```text
OPENTAKE_RUN_REAL_KEYCHAIN_MCP=1 cargo test -p opentake-tauri --features external-mcp-integration --test external_mcp_integration -- --nocapture
2 passed; all live matrix rows passed; credential scan zero matches

cargo test -p opentake-tauri --features external-mcp-integration --test external_mcp_integration -- --nocapture
2 passed; safe non-opt-in path

cargo test -p opentake-tauri --features external-mcp-integration external_mcp::tests::integration_cancel_probe_never_loses_a_concurrent_entry_signal --lib
1 passed; 2,048 concurrent signal/wait iterations

cargo test -p opentake-tauri --test external_mcp_integration --no-run
EXPECTED REFUSAL; target requires `external-mcp-integration`

cargo test -p opentake-tauri --no-default-features --test external_mcp_integration --no-run
EXPECTED REFUSAL; target requires `external-mcp-integration`

cargo test -p opentake-tauri external_mcp::tests --lib
40 passed

cargo test -p opentake-agent mcp::server::tests -- --nocapture
30 passed
```

Final strict Clippy, fmt, diff, and repeated live-keychain results are recorded after their last execution below.

### Final static checks

```text
cargo clippy -p opentake-tauri --features external-mcp-integration --test external_mcp_integration -- -D warnings
BLOCKED outside Task 6: concurrent `render.rs` `dead_code`

cargo clippy -p opentake-tauri --no-default-features --features external-mcp-integration --test external_mcp_integration -- -D warnings
BLOCKED outside Task 6: concurrent `render.rs` `dead_code` / `too_many_arguments`

cargo clippy -p opentake-tauri --features external-mcp-integration --test external_mcp_integration -- -D warnings -A dead-code
PASS

cargo clippy -p opentake-tauri --no-default-features --features external-mcp-integration --test external_mcp_integration -- -D warnings -A dead-code -A clippy::too-many-arguments
PASS

rustfmt --check --edition 2021 src-tauri/src/external_mcp.rs src-tauri/tests/external_mcp_integration.rs
PASS

cargo fmt --all -- --check
BLOCKED outside Task 6: concurrent composite files are not yet rustfmt-clean
```

Default and `--no-default-features` library builds were also inspected with
`nm`; neither contained `ExternalMcpIntegrationHarness` nor
`IntegrationCancelProbe` symbols.

For the review fix, scoped formatting checks passed for all three owned Rust
files. Strict targeted Clippy reached only concurrent composite-renderer
diagnostics outside this task: `src-tauri/src/render.rs:270,275` (`dead_code`)
and the no-default build's `src-tauri/src/render.rs:1028`
(`too_many_arguments`). Re-running the same default and no-default Task 6
targets while allowing only those exact unrelated lint classes passed. Full
workspace formatting was likewise deferred because the concurrently edited
composite files were not yet rustfmt-clean; no Task 6-owned formatting diff was
reported. The parent integration will rerun the unqualified workspace gates
after the composite owner converges.

`cargo clippy --workspace --all-targets -- -D warnings` was run and did not reach a clean workspace result: it failed in concurrently landed Task 7 code at `src-tauri/src/commands.rs:2552-2553` (`field_reassign_with_default`). Task 6 neither owns nor modified that file. The parent task was notified so the workspace gate can be rerun after its owner fixes the unrelated lint.

## API and security review

The `external_mcp` module, public listener-state enum, and two harness DTOs are visible only with the disabled-by-default `external-mcp-integration` feature. The integration target declares that feature through `required-features`; ordinary default, no-default-feature, and release builds keep the module private and omit all harness code. Existing production status/client/pairing DTOs remain crate-private. The harness is not registered as a Tauri command, cannot obtain stored bearer values, and cannot bypass transport gates. It accepts a caller-selected Keychain service only so the integration test never shares the production service namespace.

The test never formats a bearer into assertion failures or reports. Authentication errors are deliberately collapsed to `Result<_, ()>`. Cleanup is account-exact rather than service-wide. The fixed listener stays loopback-only.

## Limitations

- Real Keychain execution is opt-in because CI login sessions and macOS permission prompts are environment-dependent; it was actually run locally.
- The cancellation bridge is deterministic test plumbing around the real transport/dispatcher/project gate and does not invoke a real media decoder.
- The child-process case proves OS-level socket release on process exit, but does not boot the complete Tauri GUI event loop; the explicit application-shutdown lifecycle has separate unit coverage.
- Independent security review found and drove fixes for the public harness surface, cancellation-probe lost wakeup, and missing live undo state assertions.

## Commit

Review-fix commit message: `test(mcp): isolate persistent connection harness`
