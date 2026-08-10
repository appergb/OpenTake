# OpenTake 1.0.0-beta.4 Release Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish the tested OpenTake changes as the immutable GitHub prerelease `v1.0.0-beta.4`, including signed updater artifacts for macOS ARM64 and Windows x64.

**Architecture:** A release branch carries the already-reviewed product changes plus a TDD version-contract migration from Beta 3 to Beta 4. The branch is merged through GitHub CI into `main`; only the resulting remote `main` SHA may receive the annotated Beta 4 tag. The tag-triggered workflow builds, signs, attests, uploads, re-downloads, and verifies all seventeen assets before publishing.

**Tech Stack:** Rust workspace, Tauri 2, React/TypeScript/Vite, Python release validators, GitHub Actions, Minisign/Tauri updater, GitHub CLI.

## Global Constraints

- Release version is exactly `1.0.0-beta.4`; tag is exactly `v1.0.0-beta.4`; Windows WiX version is exactly `1.0.0.4`.
- Never move, delete, or reuse an existing release tag; the Beta 4 tag must point to the then-current remote `main` HEAD.
- Do not commit files below the three untracked `docs/audit/2026-08-07/*` asset trees, including videos, screenshots, logs, captions, or `.opentake-lock`.
- Commit the final 2026-08-10 Markdown audit, updater sources/tests/docs, all reviewed tracked product changes, and this plan.
- The workflow must fail closed unless both `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` exist as GitHub Actions secrets.
- Preserve the exact updater trust boundary: fixed `appergb/OpenTake`, HTTPS allowlist, signed attestation, package Minisign, exact SHA/size, bounded downloads, installer-specific Windows platform keys, and seventeen exact assets.
- Beta 4 remains a prerelease with macOS ad-hoc signing and no Windows Authenticode; do not claim notarization or platform publisher signing.
- No force-push, no `git add -A`, no release publication before all required CI checks and the merged-main CI are green.

---

### Task 1: Migrate the release contract to Beta 4

**Files:**
- Modify: `scripts/test_check_release_workflow.py`
- Modify: `.github/workflows/release.yml`
- Modify: `scripts/check_release_workflow.py`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `web/package.json`
- Modify: `src-tauri/tauri.conf.json`
- Create: `docs/releases/1.0.0-beta.4.md`
- Modify: `docs/releases/1.0.0-beta.3.md`
- Modify: `README.md`
- Modify: `docs/INDEX.md`
- Modify: `docs/audit/2026-08-10/final-module-validation.md`

**Interfaces:**
- Produces one repository-wide version identity: Cargo/Tauri/Web `1.0.0-beta.4`, WiX `1.0.0.4`, release note `docs/releases/1.0.0-beta.4.md`.
- Preserves generic updater unit fixtures that intentionally exercise historical versions; only current-release repository fixtures and contracts move to Beta 4.

- [ ] **Step 1: Write the failing release metadata test**

  Change the repository metadata fixture in `scripts/test_check_release_workflow.py` to expect Cargo/Tauri/Web `1.0.0-beta.4`, WiX `1.0.0.4`, and `docs/releases/1.0.0-beta.4.md`. The mutation caught is a tagged candidate whose product metadata or installer identity remains Beta 3.

- [ ] **Step 2: Verify RED**

  Run `python3 -B -m unittest discover -s scripts -p 'test_check_release_workflow.py'`. Expected: failure from the still-Beta-3 production validator/workflow metadata, not a syntax/import error.

- [ ] **Step 3: Apply the minimal Beta 4 identity**

  Update the three package versions, WiX version, workflow literals, validator current-release literals, release-note path, and approved validate-step/job digests computed from the final YAML. Regenerate `Cargo.lock` with Cargo rather than hand-editing package entries.

- [ ] **Step 4: Move unreleased notes to Beta 4 without rewriting Beta 3 history**

  Restore `docs/releases/1.0.0-beta.3.md` to its tagged historical content and create `docs/releases/1.0.0-beta.4.md` covering playback timing, native source preview, Space transport, transition persistence, export consistency, signed updater, proxy/Atom fallback, Motion/Chromium fixes, and the `tract` security upgrade. Update README/current docs/audit headings and links to Beta 4.

- [ ] **Step 5: Verify GREEN**

  Run the release validator/test suite, updater attestation tests, updater manifest tests, Windows workflow contract suite, strict YAML parser, and actionlint. All must exit 0.

### Task 2: Freeze, review, and commit the Beta 4 candidate

**Files:**
- Stage all 70 reviewed tracked product/test/release/doc changes.
- Stage only the reviewed new updater files, `docs/architecture/UPDATER.md`, `docs/audit/2026-08-10/final-module-validation.md`, the Beta 4 note, and this plan.
- Exclude every untracked `docs/audit/2026-08-07/*` runtime/evidence asset.

- [ ] **Step 1: Run fresh full gates**

  Run `cargo test --workspace --no-fail-fast`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `pnpm -C web test`, `pnpm -C web build`, release contract tests, `git diff --check`, and the Windows-target security audit.

- [ ] **Step 2: Obtain independent review**

  Dispatch a code reviewer and a security reviewer over the final unstaged diff. Resolve all P0-P2 findings and re-run affected gates.

- [ ] **Step 3: Configure updater signing secrets without exposing values**

  Verify the local private/public key pair and embedded public key, then stream the private key file and Keychain password directly into `gh secret set` for `appergb/OpenTake`. Confirm only the two secret names appear in `gh secret list`.

- [ ] **Step 4: Stage explicitly and inspect**

  Use explicit paths and `git add -u` for reviewed tracked files; never use `git add -A`. Inspect `git diff --cached --stat`, secret-scan the staged diff, and verify excluded audit assets remain unstaged.

- [ ] **Step 5: Commit**

  Commit the candidate as `chore(release): prepare v1.0.0-beta.4` on `release/v1.0.0-beta.4`.

### Task 3: Merge the release candidate through GitHub CI

- [ ] **Step 1: Push the release branch**

  Fetch `origin/main` and tags, ensure the release branch is based on the current remote main, then push with tracking. Never force-push.

- [ ] **Step 2: Open a ready-for-review PR**

  Create a PR from `release/v1.0.0-beta.4` to `main` with the Beta 4 scope, verification evidence, release limitations, and rollback notes.

- [ ] **Step 3: Wait for required checks**

  Watch all nine branch-protection checks to terminal completion. Any failure stops the merge and enters diagnosis; do not rerun blindly.

- [ ] **Step 4: Merge and verify merged main**

  Merge with an explicit merge commit (admin bypass only if the repository's single self-CODEOWNER makes ordinary approval impossible). Wait for the merge commit's own `main` CI to pass, then record `MAIN_SHA`.

### Task 4: Tag, publish, and verify Beta 4

- [ ] **Step 1: Recheck the immutable boundary**

  Confirm remote `main == MAIN_SHA`, the candidate metadata and note are Beta 4, both signing secret names exist, and neither the Beta 4 tag nor release exists.

- [ ] **Step 2: Create and push the annotated tag**

  Run `git tag -a v1.0.0-beta.4 MAIN_SHA -m 'OpenTake 1.0.0-beta.4'` and push only `refs/tags/v1.0.0-beta.4`.

- [ ] **Step 3: Monitor the tag-triggered Release workflow**

  Wait for validate, quality, macOS, Windows, and publish jobs to terminal completion. Never move the tag after failure; use `workflow_dispatch` only for an unchanged existing tag after fixing an external prerequisite while `main` remains at `MAIN_SHA`.

- [ ] **Step 4: Verify the public prerelease**

  Confirm tag/target SHA, prerelease/draft/latest flags, exactly seventeen asset names, GitHub digests, `SHA256SUMS`, updater manifest platform keys and URLs, companion signatures/attestations, and downloadable package sizes.

- [ ] **Step 5: Report rollback and residual platform limitations**

  Preserve Beta 3 as the rollback release. Report that Beta 4 is ad-hoc/not notarized on macOS and not Authenticode-signed on Windows; retain exact-SHA receipts and workflow URL.
