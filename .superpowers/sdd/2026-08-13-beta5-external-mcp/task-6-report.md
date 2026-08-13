# Task 6 report — real restart and security matrix

Date: 2026-08-14

## Status

Complete. The opt-in integration test exercised the real loopback Streamable HTTP listener, formal `rmcp` client, temporary persisted catalog, and uniquely namespaced macOS Keychain credentials. No user-owned `docs/audit/2026-08-07/**` file was modified by this task.

## Implementation

- Added `src-tauri/tests/external_mcp_integration.rs` with a safe default skip and explicit `OPENTAKE_RUN_REAL_KEYCHAIN_MCP=1` opt-in.
- Added a narrow, doc-hidden integration harness around the production `ExternalMcpState`. Network admission still uses the production bearer authorizer, Host/Origin checks, dispatcher, request scopes, live-project gate, and listener lifecycle.
- Used the formal `rmcp 2.2.0` Streamable HTTP client for initialization, discovery, and tool calls. Raw `reqwest` is limited to adversarial Host/Origin probes.
- Pinned `sse-stream = 0.2.4` as a dev dependency because rmcp 2.2's reqwest transport calls the compatibility alias introduced in that patch release.
- Captured tracing events, child-process output, matrix logs, and all temporary catalog files; compared their raw bytes against every complete generated bearer and required zero matches.
- Used one UUID-suffixed Keychain service per run, tracked only the exact accounts returned by this run's pairing receipts, deleted only those accounts, and verified each deletion by readback.

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
OPENTAKE_RUN_REAL_KEYCHAIN_MCP=1 cargo test -p opentake-tauri --test external_mcp_integration -- --nocapture
2 passed; all live matrix rows passed; credential scan zero matches

cargo test -p opentake-tauri --test external_mcp_integration -- --nocapture
2 passed; safe non-opt-in path

cargo test -p opentake-tauri external_mcp::tests --lib
40 passed

cargo test -p opentake-agent mcp::server::tests -- --nocapture
30 passed
```

Final strict Clippy, fmt, diff, and repeated live-keychain results are recorded after their last execution below.

### Final static checks

```text
cargo clippy -p opentake-tauri --test external_mcp_integration -- -D warnings
PASS

cargo clippy -p opentake-tauri --test external_mcp_integration --no-default-features -- -D warnings
PASS

cargo fmt --all -- --check
PASS
```

`cargo clippy --workspace --all-targets -- -D warnings` was run and did not reach a clean workspace result: it failed in concurrently landed Task 7 code at `src-tauri/src/commands.rs:2552-2553` (`field_reassign_with_default`). Task 6 neither owns nor modified that file. The parent task was notified so the workspace gate can be rerun after its owner fixes the unrelated lint.

## API and security review

`pub mod external_mcp`, the public listener-state enum, and the two doc-hidden harness DTOs are the only public expansion. Existing production status/client/pairing DTOs remain crate-private. The harness is not registered as a Tauri command, cannot obtain stored bearer values, and cannot bypass transport gates. It accepts a caller-selected Keychain service only so the integration test never shares the production service namespace.

The test never formats a bearer into assertion failures or reports. Authentication errors are deliberately collapsed to `Result<_, ()>`. Cleanup is account-exact rather than service-wide. The fixed listener stays loopback-only.

## Limitations

- Real Keychain execution is opt-in because CI login sessions and macOS permission prompts are environment-dependent; it was actually run locally.
- The cancellation bridge is deterministic test plumbing around the real transport/dispatcher/project gate and does not invoke a real media decoder.
- The child-process case proves OS-level socket release on process exit, but does not boot the complete Tauri GUI event loop; the explicit application-shutdown lifecycle has separate unit coverage.
- An independent reviewer could not be started because the shared parent task had reached its agent-thread limit. Local security/API/diff review was completed; the parent integration should add the independent review gate.

## Commit

Requested commit message: `test(mcp): verify persistent external connection boundary`
