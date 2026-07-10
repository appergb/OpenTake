# OpenTake Full Convergence Design

## Approval And Execution Authority

The user locked the required upstream baseline and authorized the controller to
approve designs, commit code, review work, and continue without additional
human confirmation. This design therefore uses automated approval gates:

1. The controller writes and self-reviews each spec or plan.
2. A separate review agent inspects the artifact or code slice.
3. The controller fixes every actionable finding and reruns verification.
4. Only then may the next slice begin.

No implementation is considered accepted solely because its implementer says
it is complete.

## Locked Baselines

### Product Baseline

- Current Palmier Pro Swift source: `palmier-pro-upstream` at
  `origin/main@404e14f4c449bd24576e52fa24f8e50694a5da13`.
- Required OpenTake additions: every functional delta in current code, current
  or historical OpenTake branches, accepted designs, and user requirements.
  This includes, but is not limited to, the Rust/Tauri cross-platform
  architecture, Context Signal, workflow plugins, global content-addressed
  library, reverse clip, freeze frame, EDL/OTIO/SRT/VTT export, and the current
  embedded playback recovery design. A feature cannot be excluded merely
  because it has no Swift equivalent.
- CapCut-only aspirations that exist neither in the locked Swift baseline nor
  as explicit OpenTake requirements remain roadmap items. They must not be
  marketed as shipped capabilities.

### Git Baseline

- Canonical repository:
  `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake`.
- Integration base: `origin/main@ac50dc896bea821f66c88c6ed50cf9185e4e31d1`.
- Existing recovery branch:
  `recovery/superpowers-integration-20260708-v2@5ffd7ceb74f953e46385ca97c851941a96c56b02`.
- The recovery branch is 28 commits ahead of `origin/main` and already contains
  selective replays for reverse, text rendering, render pixel tests, Save as
  Media, freeze frame, media-library fixes, Agent chat, and MCP security.
- Stale topic branch heads must not be merged directly. Six have already been
  selectively replayed. `feat/account-scaffold@4986716` is the only known
  functional topic delta not yet ported and must be replayed surgically after
  its current product contract is defined.

### Installed Runtime Baseline

- Installed app: `/Applications/OpenTake.app`.
- Current main executable SHA-256:
  `c5cf2a827d718574cdbc68580d77562bdf86a99cb561052ba20143f96e1956aa`.
- It byte-matches the current local release bundle, but the source that produced
  it is still a dirty working tree. The working tree currently contains the
  playback/cache/publication-gate recovery and must be preserved before any
  integration operation.

## Problem Statement

OpenTake has three simultaneous truth gaps:

1. The implementation has advanced beyond its July 4 handoff and module status
   documents.
2. This shallow checkout reports 792 reachable commits between the local Swift
   boundary and the locked baseline. That graph count is not treated as 792
   distinct product changes, but the source delta does add product-level
   features such as multiple timelines, nested timelines, multicam, denoise,
   VAD, speaker identification, beat detection, Main10 HDR export, FCPXML 1.14,
   blend modes, mattes, layouts, text animation, audio/timecode sync, project
   setting presets, word-based editing, analytics, and the update lifecycle.
3. The most recently installed playback fixes are not committed, while old
   topic branches remain visible even though their useful deltas were already
   replayed.

A completion claim must therefore prove both code convergence and hands-on
product behavior. Passing existing tests alone is insufficient because GPU and
FFmpeg tests may skip, visual tests are mostly structural assertions, and the
desktop app has no automated GUI harness.

## Goals

1. Preserve and integrate all still-relevant local and branch work without
   losing the verified installed-app playback fixes.
2. Build a complete capability ledger from the locked Swift source, OpenTake
   additions, current code, tests, and runtime evidence.
3. Review every required feature for correctness across each participating
   domain, command, Tauri, UI, MCP/Agent, persistence, preview, and export layer;
   the frozen manifest names the required layers for every capability.
4. Implement every missing required capability through small vertical slices.
5. Test every automatable feature and every desktop feature that can be safely
   exercised on this Mac using external-drive fixtures.
6. Keep planning, module documentation, README claims, and runtime truth in
   agreement.
7. Deliver a reproducible installed product and evidence package, not merely a
   source tree that builds.

## Non-Goals

- Do not directly merge stale topic branches or dirty `opentake-pr*` worktrees.
- Do not modify `palmier-pro-upstream`; it remains a read-only reference.
- Do not edit original projects or source media on the external drive. Tests use
  copied projects or newly created QA projects.
- Do not claim CapCut/Jianying compatibility without a real target-application
  import test.
- Do not claim Windows or Linux release support from compilation alone.
- Do not add speculative abstractions unrelated to a requirement in the locked
  baseline.

## Requirements Authority

Conflicts are resolved in this order:

1. This user's objective and the locked baselines above.
2. Observable behavior in Palmier Pro `origin/main@404e14f`.
3. Current OpenTake source, tests, built artifacts, and hands-on evidence.
4. Approved Superpowers specs and implementation plans created by this effort.
5. Module and subsystem specs under `docs/modules` and `docs/specs`.
6. Historical handoffs, roadmaps, gap reports, and archived plans.

Historical documents may explain intent but cannot override current source or
runtime evidence.

## Capability Ledger

The project will maintain two tracked authorities:

- `docs/capabilities/requirements.json`: the frozen machine-readable manifest;
- `docs/capabilities/CAPABILITY-LEDGER.md`: the reviewed human-readable state
  and evidence index.

The manifest receives a stable ID for every user-visible or contract-relevant
behavior found in the locked Swift baseline and for every current or historical
OpenTake functional delta. Swift IDs use an `UP-<area>-<slug>` prefix and
OpenTake IDs use `OT-<area>-<slug>`; IDs are never reused or silently deleted.
Every capability has:

- requirement source and stable identifier;
- representative Swift path and symbol when applicable;
- OpenTake code paths across affected layers;
- automated tests and desktop scenario;
- status: `implemented`, `verified`, `partial`, `missing`, or `blocked`;
- last verification date, evidence hash, and evidence location;
- known platform or fixture limitations;
- an exclusion or blocker field with exact evidence. A requirement may be
  marked out of scope only after the user explicitly changes the locked scope;
  unavailable hardware, credentials, budget, or platform remains `blocked`.

`implemented` means code exists. `verified` additionally requires fresh evidence
covering the advertised behavior. Mock-provider evidence can reach only
`implemented` or `partial`; paid/cloud generation becomes `verified` only after
an authorized real-provider smoke produces a real artifact with redacted cost
and request evidence. Only `verified` capabilities may be stated as shipped in
README or release notes.

Large evidence files live on the external QA volume, while the tracked ledger
stores their SHA-256, relative evidence path, producing command/scenario, and
the requirement IDs they prove. The external evidence directory never becomes
the only copy of the ledger or manifest.

## Architecture Invariants

All work preserves these invariants:

1. Rust owns the authoritative project/timeline state.
2. All edits converge on one command/transaction path used by UI, MCP, and Chat.
3. Undo ownership is explicit; an Agent cannot undo a newer user edit.
4. Timeline and source time use integer frames at public editing boundaries.
5. Preview and export share the same render semantics. Unsupported content must
   fail explicitly rather than disappear.
6. Old `.opentake` projects remain readable through serde defaults and explicit
   schema migration. An unknown top-level or nested persisted field must either
   round-trip through an extension map/raw overlay or put the project into an
   explicit save-blocked compatibility mode. Saving must never silently drop
   unknown data. Tests inject unknown fields and prove either preservation or a
   non-mutating refusal whose original file hash remains unchanged.
7. Proxy media may accelerate preview but export always resolves originals.
8. Generated, indexed, and derived assets use cancellable jobs and durable,
   content-aware caches.
9. External paths, MCP inputs, downloads, and model artifacts are validated at
   their system boundaries.

## Integration And Preservation Design

### Preserve The Dirty Playback Recovery

Before any merge or branch switch:

1. Record status, diff statistics, untracked files, installed binary hash, and
   release-bundle hash.
2. Create a binary-capable patch outside the repository. Copy every untracked
   file into a content archive outside the repository, preserving relative
   paths, bytes, modes, sizes, and SHA-256 values; a filename-only manifest is
   insufficient.
3. Create a clean linked worktree from the recovery HEAD.
4. Restore the tracked patch and untracked archive into that worktree, then
   compare every restored tracked and untracked file against the recorded size
   and SHA-256 manifest before making commits. Apply the recovery in four
   reviewable slices:
   runtime/dependency removal, Rust playback/media, Web playback/UI, and tests/
   evidence.
5. Verify each slice and commit it independently.

The original dirty worktree remains untouched until the new integration branch
reproduces its behavior.

### Branch Decisions

- Keep the recovery branch's 28 commits in order.
- Do not re-merge the six stale topic branches whose behavior was replayed.
- Compare `feat/account-scaffold@4986716` file by file against the current
  settings, secret storage, and provider model. Preserve its OpenTake-specific
  functional intent unless current code provides a reviewed equivalent; lack
  of a Swift counterpart is not a rejection reason.
- Record every local and remote branch as `integrated`, `equivalent`,
  `superseded`, `deferred`, or `rejected`, with commit and test evidence.
- Do not delete old worktrees, branches, or stashes until final verification.

## Delivery Waves

### Wave 1: Publication Baseline

Scope:

- preserve and commit the current playback recovery;
- reconcile active branches;
- guarantee save-before-project-switch and safe project lifecycle behavior;
- fix installed-app export artifact verification;
- fix P0 Agent undo ownership and `move_clips` validation/linked A/V behavior;
- make unsupported render/export content fail explicitly;
- define FFmpeg bundling, CSP/asset scope, signing, model integrity, and runtime
  dependency behavior;
- implement and verify the application update/appcast lifecycle, including
  signature validation, failure recovery, and a disabled/offline path;
- create the active capability ledger and correct top-level documentation.

Acceptance:

- clean checkout reproduces the installed playback behavior;
- new/open/switch/save/reopen cannot lose edits;
- installed app plays, pauses, resumes, scrubs, and holds the tail frame without
  a separate player process;
- installed app exports H.264/AAC and the output passes ffprobe, full decode,
  and visible first/middle/last-frame checks;
- Agent undo cannot cross a user edit and invalid moves fail without mutation;
- no required media runtime or model artifact relies on undocumented host state;
- update checks cannot replace the app with an untrusted artifact and cannot
  block launch or local editing when offline.

### Wave 2: Core Editing And Export Parity

Scope:

- overwrite, ripple, split, trim, move, duplicate, copy/paste, snapping, ranges,
  track operations, linked media, undo/redo;
- reverse and freeze across preview, audio, export, save-as-media, and
  interchange;
- transform, crop, opacity, volume, fades, six keyframe tracks, text, captions,
  caption-group styling, SRT/VTT;
- project FPS/resolution settings, presets, first-media auto-match, and identical
  UI/Agent `set_project_settings` semantics;
- XMEML, modern FCPXML, OTIO, EDL, project archive, and range export.

Acceptance:

- every edit is a complete domain-to-UI/MCP vertical slice;
- linked A/V and undo invariants hold under compound edits;
- reverse uses the same trimmed source window across preview and output;
- freeze is one atomic undo step;
- animated overlays write keyframes instead of corrupting static values;
- preview/export/interchange never silently omit required content;
- old and new project schemas pass round-trip tests;
- project setting changes and auto-match are deterministic, undo-safe where
  applicable, and never silently retime existing clips.

### Wave 3: Media Assets And Intelligence

Scope:

- project media folders, global library, deduplication, favorites, missing media,
  relink, thumbnails, sprites, waveforms, and prewarming;
- automatic transcription and search indexing lifecycle;
- transcript cache correctness by language/model/options;
- word-based removal/text editing with exact word intervals, linked A/V ripple,
  and one-step undo;
- selected-clip waveform sync and source-timecode sync across UI and Agent,
  including non-mutating failure and measured frame error;
- visual and spoken search model download, integrity, indexing, and query;
- HDR probe, preview tonemap, Main10 export, and proxy media.

Acceptance:

- nested folder import remains nested and restart-safe;
- duplicate IDs do not appear after overlapping refreshes;
- relink preserves media and clip identity;
- import/open schedules required indexes without blocking editing;
- model manifests contain real size/hash evidence and reject corruption;
- remove-words plans are frame-bounded and apply atomically to linked media;
- waveform/timecode sync produces the same placement through UI and Agent and
  reports unsupported or ambiguous sources without moving clips;
- HDR metadata and visible output are verified against the HLG fixture;
- proxy preview switches are observable and export resolves originals.

### Wave 4: Agent-Native And Generation

Scope:

- one safe dispatcher for MCP, Chat, and UI automation;
- current Swift tool semantics plus Context Signal and workflow plugins;
- Agent sessions, tool cards, cancellation, errors, and project navigation;
- image/video/audio/upscale generation, BYOK providers, job recovery, automatic
  library import, references, model catalog, and cost/consent boundaries;
- Account, Agent, Models, Storage, Privacy, Help, Feedback, and provider settings;
- opt-in/opt-out analytics with a versioned event schema, identity reset, and
  testable zero-send behavior while disabled.

Acceptance:

- no advertised tool is an unreachable stub;
- unavailable capabilities return structured reasons;
- Agent transactions cannot corrupt user edit ownership;
- generation can start, cancel, fail, recover, materialize, and be placed;
- missing keys or network never block local editing;
- secrets remain in the OS keychain and are absent from logs/evidence;
- analytics disabled means no queued or transmitted events from app, Agent,
  MCP, generation, or export paths; consent changes and identity reset persist.

### Wave 5: Creative Engine

Scope:

- professional color controls, curves, LUT, chroma, masks, blend modes, mattes,
  effects, transitions, and layouts;
- Lottie/Motion rendering and explicit failure behavior;
- text animation, curve speed, audio enhancement/denoise, loudness, speech masks,
  dead-air removal, speaker identity, and beat/downbeat editing.

Acceptance:

- each capability is delivered as a renderable vertical slice rather than an
  empty schema or disabled UI;
- pause, playback, capture, and export agree on pixels/audio;
- Lottie/Motion never disappears silently;
- speed mapping remains split/trim/export safe;
- denoise, speech, and beat results are durable, cancellable, and repeatable;
- real artifacts are inspected, not only model or schema output.

### Wave 6: Multi-Timeline, Multicam, Scale, And Platforms

Scope:

- project file with multiple timelines and active timeline identity;
- timeline tabs, nested timeline media, navigation, persistence, preview, export,
  and Agent/MCP scope;
- multicam sources, synchronization, angle switching, Inspector, tools, and
  export;
- 50-track performance budgets and long-project behavior;
- macOS signed/notarized release and real Windows/Linux packages;
- target-application interoperability claims.

Acceptance:

- nested timelines save, reopen, enter, preview, edit, and export;
- multicam sync error is measured in frames and switching is atomic;
- performance evidence states hardware, codec, resolution, FPS, latency, and
  dropped-frame budgets;
- every platform package is installed and exercises import/playback/export;
- CapCut/Jianying or NLE compatibility is claimed only with real import evidence.

## Independent Agent Review Protocol

Every code slice uses separate roles:

1. An implementer works only in its assigned files and tests.
2. A reviewer that did not implement the slice checks correctness, upstream
   parity, regressions, security, and unnecessary changes.
3. The controller inspects the diff and reviewer report, fixes findings, and
   runs targeted tests plus the wave gate: Rust fmt/clippy/tests and Web
   build/tests for every public code slice, FFmpeg/GPU integration with no-skip
   evidence for media/render changes, and installed-app artifact checks for
   user-visible desktop changes.
4. A second review is required whenever a finding changes behavior or public
   contracts.

Review agents do not approve by absence of comments. They must state the files,
requirements, tests, and failure modes inspected. The controller remains
responsible for final integration and cannot delegate the completion claim.

## Test And Evidence Design

### Fixture Ladder

1. Tiny visible-frame fixture:
   `/Volumes/mac/Agent 全自动剪辑素材/work/scene_na.mp4`.
2. Tiny tail-frame fixture:
   `/Volumes/mac/macbook-new-lesson2-ac-l-cut-20260709.mp4`.
3. Small A/V fixture: `/Volumes/mac/OpenTake.mp4`.
4. PCM fixture:
   `/Volumes/mac/Agent 全自动剪辑素材/work/music_seg.wav`.
5. Short 4K HEVC B-roll:
   `/Volumes/mac/剪辑/课程1-2 第二节课/第二节课-练习素材包/视频/Broll/电脑屏幕标注.mov`
   and `色域标注.mov` in the same directory.
6. HDR HLG fixture:
   `/Volumes/mac/dji_export_20260701_123155_1782880315251_compose_0.MOV`.
7. Medium linked project: `/Volumes/mac/未命名.opentake`.
8. Final real project:
   `/Volumes/mac/剪辑/课程1-2 第二节课/未命名.opentake`.

Original external-drive projects are copied before modification. Durable QA
artifacts live under `/Volumes/mac/OpenTake-QA/<timestamp>/`, including command
logs, timeline/media JSON, screenshots, recordings, exported media, hashes,
ffprobe output, decode logs, and process sentinels. The tracked capability
ledger links these artifacts by SHA-256; the external directory does not replace
the tracked ledger.

### Verification Ladder

For every feature, use the strongest applicable levels:

1. pure-function unit tests;
2. cross-layer contract and persistence tests;
3. FFmpeg/GPU integration tests with explicit no-skip evidence;
4. Web build and targeted Vitest;
5. workspace fmt, clippy, tests, Web build/tests, and audits;
6. release bundle inspection;
7. installed-app Computer Use with real artifacts;
8. real target-application import or cross-platform installation when claimed.

A green exit code is not enough if a test that covers the current requirement
skipped; such a row remains unverified. A desktop scenario is safely testable
when it does not mutate an original user project, incur unapproved paid cost,
or require unavailable hardware/credentials. Other scenarios remain blocked
with exact evidence rather than being excluded. XML validity is not
render/export success. MCP runtime state is not persistence until the saved
project is reopened.

## Error Handling And Safety

- Preserve unrelated user changes and external state.
- Back up installed applications and project data before replacement.
- Never use destructive Git cleanup to obtain a clean tree.
- Treat privacy, Files & Folders, Accessibility, and Screen Recording prompts as
  independent test preconditions, not media-engine failures.
- Bound HTTP bodies, inline media, downloads, model sizes, decode timeouts, and
  background concurrency.
- Use explicit capability errors for unsupported codecs, effects, motion,
  providers, and platform features.
- Do not log secrets, signed URLs, private transcript content, or account tokens
  in committed evidence.
- Paid generation requires an explicit test budget/fixture and may use mocked
  provider tests until real-cost authorization is available. Mock-only evidence
  never promotes a capability beyond `implemented` or `partial`; without a real
  authorized provider artifact it remains `blocked` for release claims. Local
  editing must remain fully testable without paid services.

## Documentation Governance

- `README` lists only freshly verified user capabilities.
- `CLAUDE.md` contains navigation and engineering discipline, not a copied
  status snapshot.
- The active capability ledger is the only implementation-status authority.
- `docs/modules` and `docs/specs` define contracts and link to the ledger.
- ROADMAP and advanced/CapCut gaps describe future scope.
- The old port-gap report and completed Superpowers recovery plan are archived.
- Dated QA reports remain evidence and never substitute for the live ledger.

## Completion Criteria

The full objective is complete only when:

1. every locked-baseline requirement has a ledger row;
2. every row is `verified`, or has an external blocker explicitly accepted by
   the user rather than silently removed from scope;
3. all active branches and dirty work are preserved, integrated, superseded, or
   rejected with evidence;
4. all automated tests mapped to a locked requirement pass without skipping;
5. every safely testable desktop feature has current installed-app evidence;
6. exported artifacts are decoded and visually/audio inspected;
7. documentation and release claims match the verified ledger;
8. an independent final review agent audits the requirement-to-evidence mapping;
9. the controller reruns the release gate and verifies the delivered app hash.
