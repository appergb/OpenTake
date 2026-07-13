# OpenTake Recovery Integration Design

## Approval

The user approved this design direction on 2026-07-08 after the
Superpowers brainstorming review:

- Use `origin/main` as the canonical integration base.
- Interpret "merge all branches" as selective replay of still-relevant branch
  work, not blind merging of stale branch heads.
- Finish the incomplete Open Code reverse-clip work first.
- Organize planning documents under `docs/superpowers`.
- Use multiple agents for implementation, review, and desktop/runtime checks.

## Context

OpenTake is the canonical repository at
`/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake`. The parent directory is not a
git repository.

At discovery time, the canonical checkout was on `main` at `c092d5b`, behind
`origin/main` at `ac50dc8` by two commits. The new recovery branch
`recovery/superpowers-integration-20260708-v2` is based on `origin/main`.

The workspace also contains linked worktrees named `opentake-pr*`. They are not
separate source-of-truth repositories. Several branches are one old functional
commit on top of an ancestor around `f511190` and about 154 commits behind
current `origin/main`. Directly merging those branch heads would delete current
docs, specs, playback probes, and other main-line work.

The original `recovery/superpowers-integration-20260708` branch and the backup
branch `backup/before-rollback-20260708-163646` are evidence sources. They are
not the base for new recovery work.

## Goal

Recover OpenTake by integrating still-relevant active branch work onto current
`origin/main`, completing the unfinished reverse-clip fix started by Open Code,
organizing Superpowers planning documents, and verifying the app through code,
security, and desktop acceptance checks before any completion claim.

## Non-Goals

- Do not directly merge stale branch heads into the recovery branch.
- Do not modify `../palmier-pro-upstream`; it remains read-only reference
  material.
- Do not claim that OpenTake has every capability of a commercial video editor
  unless current evidence proves it. Current implemented capabilities must work;
  missing product scope must remain visible in handoff docs.
- Do not hide deferred branch work. Each branch must be integrated, rejected, or
  deferred with a reason and evidence.
- Do not bundle unrelated `.claude` deletions from stale worktrees into recovery
  commits unless a later task explicitly decides to preserve or restore them.

## Branch Integration Design

Use `recovery/superpowers-integration-20260708-v2` as the integration branch.
The branch starts at `origin/main` (`ac50dc8`).

The integration mechanism is selective replay:

1. Inventory active branches and linked worktrees.
2. Exclude historical backups, already-merged no-op branches, mirror remotes,
   and upstream reference repositories.
3. For each active branch, inspect its delta against current `origin/main`.
4. Cherry-pick, manually replay, or port only the functional delta that still
   applies to current code.
5. Run targeted verification after each slice.
6. Record the branch decision and evidence under `docs/superpowers`.

Initial branch queue:

| Source | Decision |
|---|---|
| `opentake-pr9` dirty worktree and `backup/before-rollback-20260708-163646` | Integrate first as the incomplete reverse-clip work. |
| `fix/text-raster-alignment` | Inspect and replay if still relevant to current render behavior. |
| `test/render-pixel-diff` | Inspect and replay if it strengthens current render verification. |
| `fix/91-media-library-rewrite` | Inspect carefully; replay only non-regressive media-library fixes. |
| `feat/save-clip-as-media` | Inspect after media/reverse surfaces stabilize. |
| `feat/freeze-frame` | Inspect after save-as-media and source-frame mapping are stable. |
| `feat/account-scaffold` | Inspect after core editing recovery work. |
| `feat/agent-chat-panel` | Inspect after lower-risk editing slices, because it touches wider Agent/Tauri/web surfaces. |
| `feat/generative-ui`, `feat/inspector-ai-edit-tab`, `feat/proxy-media` | No-op at discovery because the branch heads equal `origin/main`; keep visible in register. |

## Open Code Reverse-Clip Design

Reverse clip is the first recovery target because it has the clearest unfinished
evidence:

- `backup/before-rollback-20260708-163646` contains committed reverse-clip
  implementation and fixes.
- `opentake-pr9` has uncommitted reverse-clip changes on a branch whose head
  equals `origin/main`.

The final design should be one clean implementation:

- Add `Clip.reversed: bool` with serde default false.
- Set the field through the existing `EditCommand::SetClipProperties` path so
  undo/redo, UI, Tauri IPC, MCP, and Agent tooling use the existing edit
  authority.
- Reverse only video source sampling. Image, text, and motion clips ignore
  reverse sampling.
- Preserve trim semantics: a reversed clip samples the same trimmed source
  window in reverse order.
- Keep export, preview, mpv EDL, fallback timeline, and Agent encoding aligned.
- Expose the user-facing toggle through the clip context menu only when the
  implementation is verified.

Targeted reverse-clip verification must include Rust domain/ops/render tests,
Tauri serde tests, TypeScript build, and web context-menu tests.

## Documentation Design

Create or restore a Superpowers documentation home:

- `docs/superpowers/specs/` for approved designs.
- `docs/superpowers/plans/` for implementation plans.
- `docs/superpowers/archive/` for branch registers and historical recovery
  notes.

Current-state notes:

- `docs/superpowers` is absent in the current worktree before this design.
- `docs/specs` exists but lacks `docs/specs/INDEX.md`.
- Root scratch files `findings.md`, `progress.md`, and `task_plan.md` are absent
  in the current worktree, so the prior archive should be restored from backup
  only as historical evidence, not as a current root cleanup task.
- Keep `docs/architecture/HANDOFF-2026-07.md` as the live project-status
  authority.
- Treat `docs/architecture/ROADMAP.md`, `docs/architecture/EDITING-ENGINE-PLAN.md`,
  and `docs/需求与问题汇总.md` as historical/context documents unless a later
  audit refreshes them.

## Agent Coordination Design

Use subagents only for bounded, independent work:

- One or two implementer agents for disjoint write scopes.
- One reviewer agent after each code slice.
- One desktop/runtime/security agent when app launch, playback, or security
  checks can run independently.

The controller session remains responsible for integration, conflict resolution,
final verification, and ensuring unrelated branch deltas are not mixed together.

## Verification Design

No completion claim is valid without fresh verification evidence.

Per-slice checks:

- Run targeted tests that prove the feature or branch slice.
- Run `git diff --check` on touched files.
- Record branch decision, tests, and known gaps in the Superpowers register.

Project-level checks before declaring the recovery branch ready:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo clippy -p opentake-tauri --no-default-features --all-targets -- -D warnings
pnpm -C web build
pnpm -C web test
```

Desktop and security checks:

- Inspect available audit tooling before adding new tools.
- Run `cargo audit`, `cargo deny`, `pnpm audit`, or equivalent only if available
  or intentionally added in an implementation task.
- Build and launch the Tauri app when local libmpv and platform constraints
  allow it.
- Use a dedicated desktop/computer-use or E2E agent to verify import, timeline
  edit, playback, pause/scrub, export or save-as, and Agent/MCP control flows.
- If a desktop check cannot run, document the exact blocker and the strongest
  substitute evidence.

## Acceptance Criteria

- The recovery branch is based on current `origin/main`.
- Superpowers design, implementation plan, branch register, and archives are
  stored under `docs/superpowers`.
- `docs/specs/INDEX.md` exists and is linked from `docs/INDEX.md`.
- The incomplete reverse-clip work is either fully integrated with verification
  evidence or explicitly blocked with exact failing evidence.
- Every active branch in the queue is integrated, rejected, or deferred with
  evidence.
- Project-level build, lint, test, security, and desktop/runtime checks are run
  fresh before any final completion claim.
- Remaining product gaps are documented instead of hidden behind a broad
  "complete video editor" claim.

## Open Risks

- Stale one-commit topic branches may need manual porting rather than clean
  cherry-picks.
- Full desktop acceptance depends on local GPU/audio behavior, media fixtures,
  libmpv setup, and app accessibility permissions.
- The requested product goal is larger than one recovery pass. The recovery can
  make current implemented features correct and verified, but any uncovered
  editor capability must remain visible as remaining product scope.
