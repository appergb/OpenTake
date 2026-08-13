# OpenTake Beta 5 Long-Lived External MCP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore a persistent, authenticated loopback MCP endpoint that shares the in-app Agent dispatcher and survives application restarts without persisting plaintext bearer tokens.

**Architecture:** `ExternalMcpState` owns a listener task and an atomic pairing catalog. The catalog keeps non-secret client metadata in application data while `KeyringStore` keeps one bearer token per client. `opentake-agent` exposes a dynamically authenticated Streamable HTTP endpoint; every accepted session is gated by the production `LiveProjectMcpGate` and uses the same dispatcher, capability bridges, and plugin registry as in-app chat.

**Tech Stack:** Rust, Tokio, Axum/rmcp Streamable HTTP, Tauri 2 managed state and events, system keyring, React/TypeScript, Zustand, Vitest.

## Global Constraints

- Bind only `127.0.0.1:19789`; never fall back to a random port or broader interface.
- Start only when external MCP is enabled and at least one non-revoked client exists.
- Return a plaintext 256-bit bearer token only from pair/regenerate commands and never serialize it into metadata, logs, errors, telemetry, events, or tests.
- Authenticate `/mcp` and well-known routes with constant-time token comparison plus loopback Host/Origin checks before rmcp session creation.
- Reuse `ChatState`'s dispatcher and plugin registry; do not construct a second tool universe.
- Keep a distinct undo scope per rmcp session and cancel affected requests immediately on revoke or project transition.
- Preserve all user-owned `docs/audit/2026-08-07/*` changes and assets.

---

### Task 1: Add an atomic, keychain-backed pairing catalog

**Files:**
- Create: `src-tauri/src/external_mcp.rs`
- Modify: `src-tauri/src/secret.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- `ExternalMcpClientSummary { id, name, token_digest, created_at, last_used_at, revoked_at }`
- `ExternalMcpPairingReceipt { client, endpoint, bearer_token }`
- `ExternalMcpCatalog::{load,pair,regenerate,revoke,active_credentials}`
- keychain account name `external-mcp:<client-id>` under the existing OpenTake service.

- [ ] **Step 1: Write failing catalog tests**

  Add unit tests proving pair creates a unique client id and 32-byte random token, persisted JSON omits the token, a restart reloads metadata and retrieves the token from a fake secret store, regeneration invalidates the previous token, revoke removes the secret, duplicate display names remain distinguishable, and a failed atomic rename leaves the previous catalog readable.

- [ ] **Step 2: Verify RED**

  Run `cargo test -p opentake-tauri external_mcp::tests::catalog --lib`. Expected: compilation fails because the catalog and secret-store seam do not exist.

- [ ] **Step 3: Implement the secret-store seam and catalog**

  Introduce a narrow `McpSecretStore` trait implemented by the existing keyring wrapper and by an in-memory test double. Persist only versioned metadata beneath `app_data_dir/external-mcp/clients.json` using create-new temp file, sync, rename, and parent sync. Generate credentials with the operating-system RNG, expose only a short SHA-256 digest, validate names and lengths, and serialize timestamps in a stable integer representation.

- [ ] **Step 4: Verify GREEN**

  Run `cargo test -p opentake-tauri external_mcp::tests::catalog --lib`. All catalog and restart tests must pass without touching the developer's real keychain.

- [ ] **Step 5: Commit the catalog**

  Commit `src-tauri/src/external_mcp.rs`, `src-tauri/src/secret.rs`, and the module declaration as `feat(mcp): add keychain-backed pairing catalog`.

### Task 2: Generalize Streamable HTTP authentication for long-lived credentials

**Files:**
- Modify: `crates/opentake-agent/src/mcp/server.rs`
- Modify: `crates/opentake-agent/src/mcp/mod.rs`

**Interfaces:**
- `trait BearerAuthorizer { fn authorize(&self, token: &str) -> Option<AuthenticatedMcpClient>; }`
- `AuthenticatedMcpClient { client_id: Arc<str>, credential_generation: u64 }`
- `ManagedMcpEndpoint { addr, shutdown(), wait() }`
- `bind_managed_gated_on(listener, dispatcher, registry, gate, authorizer)` for production and deterministic tests.

- [ ] **Step 1: Write failing transport/authentication tests**

  Cover missing authorization, wrong token, revoked token, token regeneration, malformed bearer syntax, remote Host, remote Origin, loopback Origin, valid initialize, and shutdown. Assert all authentication failures have the same public status/body shape and captured tracing output does not contain supplied tokens.

- [ ] **Step 2: Verify RED**

  Run `cargo test -p opentake-agent mcp::server::tests::managed_ -- --nocapture`. Expected: tests fail because the dynamic authorizer and managed endpoint are absent.

- [ ] **Step 3: Implement dynamic authorization and managed shutdown**

  Refactor the existing single-token guard into a shared boundary that parses once and delegates matching to `BearerAuthorizer`. Compare all active token byte strings in constant time, attach the authenticated client identity to request extensions, retain Beta 4 body/content-type/protocol/concurrency limits, and add an explicit cancellation token plus join handle for listener shutdown.

- [ ] **Step 4: Preserve ephemeral behavior**

  Adapt `bind_ephemeral_gated` to the new shared boundary with a one-entry authorizer, keeping its random port, one-time token receipt, Host/Origin behavior, and tests unchanged.

- [ ] **Step 5: Verify GREEN**

  Run `cargo test -p opentake-agent mcp::server::tests -- --nocapture` and `cargo test -p opentake-agent chat:: -- --nocapture`. Both suites must pass.

- [ ] **Step 6: Commit the transport**

  Commit the agent crate changes as `feat(mcp): authenticate managed loopback sessions`.

### Task 3: Promote project gating to production and share the Agent tool universe

**Files:**
- Modify: `src-tauri/src/mcp.rs`
- Modify: `src-tauri/src/chat.rs`
- Modify: `src-tauri/src/external_mcp.rs`

**Interfaces:**
- production `LiveProjectMcpGate` implementing `ChatTurnGate`
- `ChatState::external_mcp_components() -> ExternalMcpComponents`
- `ExternalMcpState::new(core, components, catalog)`

- [ ] **Step 1: Write failing shared-state and transition tests**

  Add tests proving external state receives pointer-identical dispatcher/registry values from `ChatState`, refuses mutating calls with no saved project, cancels an active old-project request before activating the new identity, rejects a stale identity, and keeps undo isolated across two rmcp sessions and in-app chat.

- [ ] **Step 2: Verify RED**

  Run `cargo test -p opentake-tauri mcp::tests::live_project_ external_mcp::tests::shared_ --lib`. Expected: the production external components and public gate construction are unavailable.

- [ ] **Step 3: Move the proven gate out of test-only compilation**

  Remove the `cfg(test)` boundary from the gate and its required imports, keep test-only helpers gated, and make transition admission/cancellation usable by the listener. Do not weaken project identity or side-effect termination checks.

- [ ] **Step 4: Expose shared ChatState components**

  Add a crate-private immutable component bundle that clones Arcs for the existing dispatcher and registry. Construct `ExternalMcpState` from that bundle during Tauri setup after the core and bridges are ready.

- [ ] **Step 5: Verify GREEN**

  Run `cargo test -p opentake-tauri mcp::tests --lib` and `cargo test -p opentake-tauri external_mcp::tests::shared_ --lib`.

- [ ] **Step 6: Commit the shared gate**

  Commit as `refactor(mcp): share live project dispatcher with external sessions`.

### Task 4: Implement the listener lifecycle and typed Tauri command surface

**Files:**
- Modify: `src-tauri/src/external_mcp.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `web/src/lib/types.ts`
- Modify: `web/src/lib/api.ts`

**Interfaces:**
- `external_mcp_status() -> ExternalMcpStatus`
- `external_mcp_set_enabled(enabled) -> ExternalMcpStatus`
- `external_mcp_pair(name) -> ExternalMcpPairingReceipt`
- `external_mcp_regenerate(client_id) -> ExternalMcpPairingReceipt`
- `external_mcp_revoke(client_id) -> ExternalMcpStatus`
- event `external_mcp_status_changed`

- [ ] **Step 1: Write failing lifecycle tests**

  Cover disabled startup, restart recovery, zero-client shutdown, port conflict, status transition ordering, pair while enabled, revoke of the final client, regenerate cancellation, application shutdown, and last-used timestamp updates without high-frequency disk writes.

- [ ] **Step 2: Verify RED**

  Run `cargo test -p opentake-tauri external_mcp::tests::lifecycle_ --lib`. Expected: command/state lifecycle types do not yet exist.

- [ ] **Step 3: Implement a serialized state machine**

  Guard lifecycle changes with one async mutex; expose `disabled`, `starting`, `listening`, `portConflict`, `authFailure`, and `paused` states; bind the fixed IPv4 socket before reporting listening; cancel old client sessions on regenerate/revoke; and stop the endpoint in Tauri exit handling. Emit sanitized summaries only.

- [ ] **Step 4: Register commands and front-end types**

  Register all five commands in `generate_handler!`, add exact camelCase TypeScript DTOs and API wrappers, and provide a typed listener that can re-sync after missed events.

- [ ] **Step 5: Verify GREEN**

  Run `cargo test -p opentake-tauri external_mcp::tests --lib`, `pnpm -C web test -- src/lib/api.test.ts`, and `pnpm -C web build`.

- [ ] **Step 6: Commit the lifecycle**

  Commit as `feat(mcp): manage persistent external endpoint lifecycle`.

### Task 5: Replace the external MCP settings placeholder with pairing management

**Files:**
- Modify: `web/src/components/settings/SettingsView.tsx`
- Create: `web/src/components/settings/ExternalMcpPane.tsx`
- Create: `web/src/components/settings/ExternalMcpPane.test.tsx`
- Modify: `web/src/i18n/dict.ts`
- Modify: `web/src/styles/components.css`

**Interfaces:**
- One enable switch, authoritative status row, fixed endpoint display, client list, and pair/regenerate/revoke/config-copy actions.
- Copy payload uses the documented Streamable HTTP endpoint and bearer header without storing the token in browser persistence.

- [ ] **Step 1: Write failing interaction tests**

  Assert disabled/listening/port-conflict/auth-failure views, enable rollback on command failure, one-time token reveal, config copy, confirmation before regenerate/revoke, token removal after dismiss, and no credential text in rendered status after navigation/reload.

- [ ] **Step 2: Verify RED**

  Run `pnpm -C web test -- src/components/settings/ExternalMcpPane.test.tsx`. Expected: the pane and API interactions are absent.

- [ ] **Step 3: Implement the pane**

  Build the settings UI with existing tokens and shared disclosure motion, accessible labels, clear destructive confirmations, clipboard failure feedback, and status re-sync on mount/event. Do not render an enabled state until the backend reports listening.

- [ ] **Step 4: Verify GREEN**

  Run `pnpm -C web test -- src/components/settings/ExternalMcpPane.test.tsx src/components/settings/SettingsView.interaction.test.tsx` and `pnpm -C web build`.

- [ ] **Step 5: Commit settings integration**

  Commit as `feat(settings): manage external MCP pairings`.

### Task 6: Exercise a real restart and security matrix

**Files:**
- Create: `src-tauri/tests/external_mcp_integration.rs`
- Create: `docs/audit/2026-08-13/beta5-external-mcp.md`

- [ ] **Step 1: Add an opt-in real-keychain integration harness**

  Use unique test service/account identifiers, bind only loopback, start a client through rmcp, restart the state against the same temporary catalog/keychain namespace, and clean up only those exact test credentials.

- [ ] **Step 2: Run the live transport matrix**

  Run `cargo test -p opentake-tauri --test external_mcp_integration -- --nocapture` plus the agent server test suite. Record authenticated restart, revoke rejection, Host/Origin rejection, project-switch cancellation, cross-session undo isolation, and port-conflict results.

- [ ] **Step 3: Audit logs and persisted bytes**

  Search captured logs and catalog files for each generated full token and require zero matches. Confirm the listener socket is closed after disable and process exit.

- [ ] **Step 4: Commit verified evidence**

  Commit the integration test and Markdown receipt as `test(mcp): verify persistent external connection boundary`.
