# OpenTake 1.0.0-beta.5 Release Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish the verified Beta 5 product as immutable GitHub prerelease `v1.0.0-beta.5`, with the existing seventeen signed updater assets and auditable packaged-app evidence for every requested behavior.

**Architecture:** Functional plans land first on `release/v1.0.0-beta.5`. A test-first version-contract migration updates Cargo, Web, Tauri, WiX, workflow, documentation, and validator digests as one identity. The frozen candidate passes focused, full, security, license, real MCP, Chromium/FFmpeg, and packaged GUI gates before it can move through a PR to remote `main`; only the verified remote main SHA receives the annotated tag.

**Tech Stack:** Rust/Cargo, React/TypeScript/Vite/pnpm, Python validators, Tauri 2, GitHub Actions, gh CLI, Minisign/Tauri updater, macOS and Windows package jobs.

## Global Constraints

- Product version is exactly `1.0.0-beta.5`, tag `v1.0.0-beta.5`, WiX `1.0.0.5`.
- Do not tag, push, merge, publish, or modify secrets until all local implementation plans and preflight gates are complete.
- Never move/delete/reuse a release tag and never force-push.
- Preserve Beta 4 as rollback; fixes after publication use a higher version.
- Preserve the exact updater trust boundary and seventeen-asset release contract unless a failing primary-platform tool proves a required additive change.
- Keep macOS ad-hoc/not-notarized and Windows non-Authenticode limitations explicit.
- Never stage user-owned untracked/modified `docs/audit/2026-08-07/*` files.
- Use explicit staging paths and inspect every staged diff; never use `git add -A`.

---

### Task 1: Migrate the repository release identity to Beta 5

**Files:**
- Modify: `scripts/test_check_release_workflow.py`
- Modify: `.github/workflows/release.yml`
- Modify: `scripts/check_release_workflow.py`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `web/package.json`
- Modify: `src-tauri/tauri.conf.json`
- Create: `docs/releases/1.0.0-beta.5.md`
- Modify: `README.md`
- Modify: `docs/INDEX.md`
- Modify: `CHANGELOG.md`

**Interfaces:**
- repository-wide Cargo/Tauri/Web `1.0.0-beta.5`, WiX `1.0.0.5`, release note `docs/releases/1.0.0-beta.5.md`.
- historical/generic updater fixtures remain historical; only current-release contracts change.

- [ ] **Step 1: Write the failing metadata contract**

  Update current-release test fixtures to require Beta 5 identities, note path, tag trigger, artifact prefixes, and WiX fourth component. Add mutations for each stale Beta 4 value.

- [ ] **Step 2: Verify RED**

  Run `python3 -B -m unittest discover -s scripts -p 'test_check_release_workflow.py'`. Expected: current production metadata/workflow remains Beta 4.

- [ ] **Step 3: Apply the minimal identity migration**

  Update root workspace version, Web package, Tauri version/WiX version, release workflow/current validator literals, and approved validation/job digests derived from final YAML. Regenerate lockfiles with Cargo/pnpm rather than hand-editing resolved entries.

- [ ] **Step 4: Write Beta 5 release notes**

  Cover persistent authenticated MCP, ordered Agent/tool conversation, clear-timeline PNG, Motion Studio, settings/library/Home/title-bar improvements, licenses, platform signing limitations, updater behavior, and rollback. Update current links without rewriting Beta 4 history.

- [ ] **Step 5: Verify GREEN**

  Run release workflow tests/validator, strict YAML parser, actionlint, updater manifest/attestation tests, Windows workflow contract tests, and version searches that distinguish intentional historical references.

- [ ] **Step 6: Commit release identity**

  Commit as `chore(release): prepare v1.0.0-beta.5 metadata`.

### Task 2: Run the complete local release gate matrix

**Files:**
- Modify: `docs/audit/2026-08-13/beta5-release-candidate.md`

- [ ] **Step 1: Run focused product gates fresh**

  Re-run external MCP security/restart, Agent ordering/session/PNG, Motion document/Chromium/FFmpeg, appearance/storage, Library/Home thumbnail, and title-bar measurement suites from the four implementation plans. No cached historical receipt substitutes for a fresh command.

- [ ] **Step 2: Run full Rust gates**

  Run `cargo test --workspace --no-fail-fast`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --all -- --check`, required feature-gated integration suites, and the repository security audit command documented in the release workflow.

- [ ] **Step 3: Run full Web and dependency gates**

  Run `pnpm -C web test`, `pnpm -C web build`, lockfile/install integrity, license inventory, dependency audit commands used by CI, and ensure all CodeMirror notices match installed versions.

- [ ] **Step 4: Run release/integrity gates**

  Run all Python release tests, workflow validator, actionlint, updater tests, `git diff --check`, generated-file checks, and a case-sensitive search for secrets/tokens/private keys in tracked/staged output.

- [ ] **Step 5: Record exact results**

  Write command, exit code, test count/skip reason, timestamp, platform, artifact path/hash, and any accepted signing limitation to the candidate audit. Mark a gate not run or failed accurately; do not infer success.

### Task 3: Build and visually verify the final packaged candidate

**Files:**
- Modify: `docs/audit/2026-08-13/beta5-release-candidate.md`
- Create: `docs/audit/2026-08-13/screenshots/beta5-packaged-home.png`
- Create: `docs/audit/2026-08-13/screenshots/beta5-packaged-library.png`
- Create: `docs/audit/2026-08-13/screenshots/beta5-packaged-agent.png`
- Create: `docs/audit/2026-08-13/screenshots/beta5-packaged-motion.png`
- Create: `docs/audit/2026-08-13/screenshots/beta5-packaged-settings.png`

- [ ] **Step 1: Build the release application and packages**

  Provision checksum-pinned FFmpeg/Chromium sidecars through repository scripts, then run the platform release build with the same feature set/environment as the workflow. Record `.app`/DMG paths, sizes, and SHA-256.

- [ ] **Step 2: Run the acceptance matrix in the packaged `.app`**

  Verify external MCP across app restart/revoke, model clear animation, standard/compact switching, Library return placement, Home card content, continuous Agent/tool ordering, clear-timeline PNG, Motion real text/edit/preview/publish/reopen, and title-bar alignment.

- [ ] **Step 3: Measure rather than eyeball geometry**

  Run the title-bar image measurement script on each applicable screenshot and assert at most 1 CSS px center deviation. Record 16:9 project/Motion preview pixel bounds and disclosure animation/reduced-motion observations.

- [ ] **Step 4: Re-run affected checks after any correction**

  Any packaged-app defect returns to the relevant TDD task. Rebuild from a clean output directory and repeat the full affected acceptance slice before updating the receipt.

### Task 4: Obtain final code, security, and release review

**Files:**
- Review the complete Beta 5 diff and candidate audit; no new product file is expected unless findings require a fix.

- [ ] **Step 1: Request independent code review**

  Review Rust/TypeScript correctness, compatibility, cancellation, persistence, stale-event handling, atomicity, and test coverage. Resolve every P0–P2 finding and rerun affected tests.

- [ ] **Step 2: Request independent security review**

  Review MCP auth/loopback/Host/Origin/logging/keychain/revoke, Motion path/network/script confinement, token/image bounds, process cleanup, release secrets, updater trust, and dependency licenses. Resolve every P0–P2 finding and rerun the full security slice.

- [ ] **Step 3: Inspect the final diff and worktree**

  Check changed file inventory, no debug code, no accidental binaries/build output, no unrelated formatting, no plaintext credential, required docs/screenshots present, and all user-owned audit assets still unstaged.

- [ ] **Step 4: Freeze and commit the candidate**

  Stage explicit reviewed paths, inspect `git diff --cached --stat` and `git diff --cached`, run a staged secret scan and `git diff --cached --check`, then commit remaining candidate evidence as `chore(release): freeze v1.0.0-beta.5 candidate`.

### Task 5: Merge Beta 5 through GitHub CI

- [ ] **Step 1: Verify remote preconditions**

  Authenticate `gh`, fetch `origin/main` and tags, confirm no Beta 5 tag/release exists, confirm required signing secret names exist without reading values, and rebase/merge current remote main only through a reviewed non-destructive integration if it advanced.

- [ ] **Step 2: Push the release branch**

  Push `release/v1.0.0-beta.5` with tracking and no force. Confirm the remote branch SHA equals the local candidate SHA.

- [ ] **Step 3: Open a ready PR**

  Create a PR to `main` containing scope, risk boundaries, exact local evidence, external MCP threat model, new licenses, signing limitations, and rollback. Do not publish the tag from the branch.

- [ ] **Step 4: Wait for all required checks**

  Monitor every branch-protection check to a terminal state. Diagnose any failure from logs, patch on the release branch, rerun local affected/full gates, push normally, and wait again.

- [ ] **Step 5: Merge and validate merged main**

  Merge with the repository's required merge method. Wait for the merge commit's own `main` CI to pass and record immutable `MAIN_SHA`. If main advances again before tagging, repeat the boundary check rather than tagging an unverified SHA.

### Task 6: Tag, publish, and verify the immutable prerelease

- [ ] **Step 1: Recheck the release boundary**

  Confirm remote `main == MAIN_SHA`, merged metadata/note are Beta 5, signing secret names exist, all required main checks are green, and neither tag nor release exists.

- [ ] **Step 2: Create and push the annotated tag**

  Run `git tag -a v1.0.0-beta.5 MAIN_SHA -m 'OpenTake 1.0.0-beta.5'` and push only `refs/tags/v1.0.0-beta.5`. Never move it after this step.

- [ ] **Step 3: Monitor the tag-triggered workflow**

  Wait for validate, quality, macOS, Windows, and publish jobs to complete. A code failure requires a higher version; an external infrastructure retry may use workflow dispatch only if it leaves tag/SHA/source unchanged and the workflow contract permits it.

- [ ] **Step 4: Verify the public prerelease**

  Confirm tag target SHA, prerelease/draft/latest flags, release-note content, exactly seventeen expected asset names, GitHub digests, `SHA256SUMS`, updater platform keys/URLs, Minisign companions, attestations, content types, and non-zero bounded downloads.

- [ ] **Step 5: Verify updater discovery from Beta 4**

  Against the production endpoint, run the updater check from a clean Beta 4 installation/configuration, verify it selects Beta 5, validates signature/hash/size/attestation, and does not install an unexpected platform artifact.

- [ ] **Step 6: Publish the final receipt**

  Record release URL, workflow URL, `MAIN_SHA`, tag object SHA, asset digests/sizes, updater result, release limitations, and rollback to Beta 4 in the audit and final user report.
