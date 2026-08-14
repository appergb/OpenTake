# Motion Studio Task 7 report

Status: COMPLETE — independently reviewed and approved before commit.

## Scope

- Added six capability-gated Agent/MCP tools for listing, reading, creating,
  patching, previewing, and publishing current-project Motion Studio documents.
- Added exact JSON schemas and bounded decoders for document identifiers,
  revision hashes, UTF-8 byte edits, source/result sizes, preview dimensions and
  frames, and add-versus-edit publishing arguments.
- Kept filesystem access behind a typed Tauri bridge. Model-visible results
  contain document and timeline identifiers, bounded source/preview content,
  diagnostics, and revision hashes, but no filesystem paths or private errors.
- Added structured, non-mutating revision conflicts with the current hash and
  explicit remediation. Patch persistence recomputes the authoritative result
  hash and commits only against the caller's exact baseline.
- Split dispatch into project-bound admission and deferred execution. Admission
  captures the current `ProjectAssetAuthority` while the lifecycle gate is
  held; execution starts only after that identity lease is released, preserving
  the publication → identity lock order while rejecting project switches.
- Reused the production Motion preview and publish pipelines, propagated the
  original MCP cancellation token, and subscribed active operations to project
  identity transitions.
- Added a sanitized `motion_document_changed` event. A clean open editor
  installs an Agent-authored authoritative revision; a dirty, saving,
  conflicting, or publishing editor preserves local work and reaches the
  existing explicit conflict flow instead of being silently overwritten.
- Bound every change event to its original project epoch and path. The Web
  decoder validates the complete payload and the store checks project identity
  before updating its list, after the authoritative read, and before reporting
  a read error; project reset also invalidates all pending external refreshes.

## TDD evidence

Initial RED verification used:

```text
cargo test -p opentake-agent mcp::motion_documents::tests -- --nocapture
```

The test target failed to compile because the Motion document bridge, requests,
tool constants, handlers, and schemas did not exist. Added regressions cover:

- exactly six advertised tools and their strict server schemas;
- source, edit, preview, publish, hash, identifier, and result bounds;
- traversal/absolute/path-like input rejection and no path-shaped result fields;
- structured stale revision conflicts with no mutation;
- deferred execution after the lifecycle identity lease is released;
- current-project authority captured at admission and cancellation after Save As;
- Chinese text patching, exact revision changes, and sanitized change events;
- clean-editor authoritative refresh and dirty-editor local-source preservation.
- delayed old-project notifications and a pending refresh crossing project reset;
- exact production render boundaries (2-pixel minimum and 3,600-frame maximum);
- consistent `documentId` serialization across list/read/create responses.

## Final fresh verification

```text
cargo test -p opentake-agent mcp::
191/191 passed

cargo test -p opentake-tauri chat::tests --lib
24/24 passed

cargo test -p opentake-tauri motion_documents::tests --lib
17/17 passed

cargo test -p opentake-tauri mcp::tests --lib
83/83 passed

cargo test -p opentake-tauri motion::tests --lib
12/12 passed

cargo test -p opentake-agent motion_document
8/8 passed

cargo clippy -p opentake-agent -p opentake-tauri --all-targets -- -D warnings
passed (only the existing block 0.1.6 future-incompatibility notice)

cargo fmt --all -- --check
passed

pnpm -C web test
149 files / 1365 tests passed

pnpm -C web build
passed (only existing dynamic-import and large-chunk warnings)

git diff --check
passed
```

## Review

The first independent review found a project-switch notification race, a
schema/production render-bound mismatch, and an `id` versus `documentId`
contract mismatch. All three were reproduced and fixed with project-bound event
CAS, aligned 2..4096 / 1..3600 limits, and a single Agent-facing
`documentId` field. Re-review then found a publish-window event could advance
the summary but leave the open document stale. The store now retains the latest
project-bound change during publishing, replays it after every publish terminal
state, and clears it on project reset; a deferred-publish regression covers the
full summary → pending → authoritative install sequence.

Final independent verdict: **Spec PASS / Quality APPROVE**, with zero CRITICAL,
HIGH, MEDIUM, or LOW findings.

## Commit

Pending: `feat(agent): edit Motion Studio documents with hash-safe tools`.
