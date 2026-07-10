# OpenTake Wave 1A Integration Baseline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Preserve the installed-app playback recovery exactly as an immutable
safety commit, reconstruct it hunk-by-hunk in a clean integration worktree,
split it into the design's four reviewable categories, and prove every baseline
hunk is either delivered in its owning slice or explicitly superseded by an
independently reviewed fix.

**Architecture:** Keep the canonical dirty worktree immutable while creating a
content-addressed safety archive and a linked worktree from the current
`recovery/superpowers-integration-20260708-v2` HEAD, which contains design
commit `05da823` and this reviewed plan. Record tracked and untracked content
byte-for-byte in an alternate-index safety commit without modifying the
canonical index or worktree. Keep the integration worktree clean and use the
safety commit only as a per-hunk reference while committing four coherent
categories: runtime/dependency removal, Rust playback/media, Web playback/UI,
and tests/evidence. Every public code slice gets the complete wave gate and a
separate reviewer before the next slice.

**Tech Stack:** Git worktrees, Rust 2021 workspace, Tauri 2, React 18,
TypeScript 5.6, Vitest 4, pnpm 10, FFmpeg 8.1.2 on the current Mac.

## Global Constraints

- Canonical repository:
  `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake`.
- Source branch before restoration:
  `recovery/superpowers-integration-20260708-v2`, with `05da823` as a required
  ancestor. Record the exact reviewed plan commit at execution time.
- New integration branch:
  `integration/opentake-full-convergence-20260710`.
- New worktree:
  `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-full-convergence`.
- Clean review worktree retained for per-commit verification:
  `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-wave1a-review`.
- Safety archive root:
  `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence`.
- Do not switch, reset, clean, stash, or otherwise mutate the canonical dirty
  worktree.
- Do not delete any branch, worktree, stash, installed-app backup, or external
  media/project.
- Preserve every tracked and untracked byte in the immutable alternate-index
  safety commit before making integration commits.
- Never apply the full restored patch or copy a whole restored blob into the
  integration worktree. Overlapping paths contain multiple future slices;
  implement each owned hunk with `apply_patch` against the clean branch while
  consulting the safety commit and design.
- Stage only the explicit paths listed in each task; never use `git add -A`.
- Before every slice review, show both `git diff --name-status <parent> HEAD`
  and `git diff <parent> HEAD --`; the reviewer must reject any hunk owned by a
  later slice even when the path itself is allowed in both slices.
- A separate agent that did not implement a slice must review it before the next
  slice begins.
- GPU/FFmpeg tests must report executed assertions; a skip is not a pass.

## Required Public Code-Slice Gate

In Task 1 Step 4, create
`$SAFETY/run-code-slice-gate.zsh` with `apply_patch` using this exact content:

```zsh
#!/bin/zsh
set -euo pipefail
REVIEW="${1:?review worktree path required}"
SLICE="${2:?unique slice id required}"
SAFETY='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence'
CARGO_AUDIT_BIN='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake/target/cargo-tools/bin/cargo-audit'
LOG="$SAFETY/logs/$SLICE"
test ! -e "$LOG"
mkdir -p "$LOG"
cd "$REVIEW"
test -x "$CARGO_AUDIT_BIN"
pnpm -C web install --frozen-lockfile 2>&1 | tee "$LOG/pnpm-install.log"
cargo fmt --all --check 2>&1 | tee "$LOG/cargo-fmt.log"
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tee "$LOG/cargo-clippy.log"
cargo test --workspace -- --nocapture 2>&1 | tee "$LOG/cargo-test.log"
cargo clippy -p opentake-tauri --no-default-features --all-targets -- -D warnings \
  2>&1 | tee "$LOG/cargo-clippy-no-default.log"
pnpm -C web build 2>&1 | tee "$LOG/web-build.log"
pnpm -C web test 2>&1 | tee "$LOG/web-test.log"
pnpm -C web audit --audit-level high 2>&1 | tee "$LOG/pnpm-audit.log"
"$CARGO_AUDIT_BIN" audit 2>&1 | tee "$LOG/cargo-audit.log"
test -z "$(git status --porcelain)"
```

Each task calls the script with the review worktree path and a unique category,
commit-SHA, and attempt suffix. The script refuses to overwrite prior logs.
Every pipeline exits zero because `set -o pipefail` is active; test logs identify
all ignored/skipped tests. Any skip mapped to the slice's behavior blocks
approval.

Also create `$SAFETY/pin-and-gate-slice.zsh` with `apply_patch` using this exact
content; it pins the clean review worktree to the integration SHA before calling
the full gate:

```zsh
#!/bin/zsh
set -euo pipefail
INTEGRATION="${1:?integration worktree path required}"
REVIEW="${2:?review worktree path required}"
CATEGORY="${3:?category required}"
ATTEMPT="${4:?attempt required}"
ARTIFACT="${5:?artifact mode required}"
SAFETY='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence'
case "$ARTIFACT" in
  required|skip) ;;
  *) printf 'invalid artifact mode: %s\n' "$ARTIFACT" >&2; exit 2 ;;
esac
test -z "$(git -C "$INTEGRATION" status --porcelain)"
SHA=$(git -C "$INTEGRATION" rev-parse HEAD)
test -z "$(git -C "$REVIEW" status --porcelain)"
git -C "$REVIEW" switch --detach "$SHA"
test "$(git -C "$REVIEW" rev-parse HEAD)" = "$SHA"
SHORT=$(git -C "$REVIEW" rev-parse --short=12 HEAD)
SLICE="$CATEGORY-$SHORT-$ATTEMPT"
BASE_DIR="$SAFETY/slice-bases"
BASE_FILE="$BASE_DIR/$CATEGORY.txt"
mkdir -p "$BASE_DIR"
if test "$ATTEMPT" = initial; then
  test ! -e "$BASE_FILE"
  SLICE_BASE=$(git -C "$REVIEW" rev-parse "$SHA^")
  printf '%s\n' "$SLICE_BASE" > "$BASE_FILE"
else
  test -s "$BASE_FILE"
  SLICE_BASE=$(sed -n '1p' "$BASE_FILE")
fi
git -C "$REVIEW" merge-base --is-ancestor "$SLICE_BASE" "$SHA"
zsh "$SAFETY/run-code-slice-gate.zsh" "$REVIEW" "$SLICE"
LOG="$SAFETY/logs/$SLICE"
PARENT=$(git -C "$REVIEW" rev-parse "$SHA^")
git -C "$REVIEW" diff --name-status "$PARENT" "$SHA" | \
  tee "$LOG/commit-name-status.log"
git -C "$REVIEW" diff --binary "$PARENT" "$SHA" -- \
  > "$LOG/commit.diff"
printf '%s\n' "$SLICE_BASE" | tee "$LOG/slice-base.txt"
git -C "$REVIEW" diff --name-status "$SLICE_BASE" "$SHA" | \
  tee "$LOG/slice-name-status.log"
git -C "$REVIEW" diff --binary "$SLICE_BASE" "$SHA" -- \
  > "$LOG/slice.diff"
if test "$ARTIFACT" = required; then
  zsh "$SAFETY/run-desktop-artifact-gate.zsh" "$REVIEW" "$SLICE"
fi
printf '%s\n%s\n%s\n' "$SHA" "$LOG" "$SLICE_BASE"
```

For every user-visible desktop slice, also create and call
`$SAFETY/run-desktop-artifact-gate.zsh` with this exact content:

```zsh
#!/bin/zsh
set -euo pipefail
REVIEW="${1:?review worktree path required}"
SLICE="${2:?slice id required}"
SAFETY='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence'
LOG="$SAFETY/logs/$SLICE"
test -d "$LOG"
test ! -e "$LOG/tauri-build.log"
cd "$REVIEW"
./web/node_modules/.bin/tauri build --bundles app --no-sign \
  2>&1 | tee "$LOG/tauri-build.log"
APP='target/release/bundle/macos/OpenTake.app'
BIN="$APP/Contents/MacOS/opentake"
test -x "$BIN"
if otool -L "$BIN" | rg -i 'libmpv'; then exit 1; fi
shasum -a 256 "$BIN" | tee "$LOG/binary-sha256.log"
BUNDLE_SHA=$(zsh "$SAFETY/bundle-tree-digest.zsh" "$APP" "$LOG/app-bundle.manifest")
printf '%s\n' "$BUNDLE_SHA" | tee "$LOG/app-bundle-sha256.log"
codesign -dv --verbose=4 "$APP" 2>&1 | tee "$LOG/codesign.log"
test -z "$(git status --porcelain)"
```

Create `$SAFETY/verify-review-report.zsh` with `apply_patch` using this exact
parameterized content:

```zsh
#!/bin/zsh
set -euo pipefail
REVIEW="${1:?review worktree path required}"
LOG="${2:?exact attempt log path required}"
EXPECTED_SHA="${3:?exact reviewed SHA required}"
EXPECTED_VERDICT="${4:?expected APPROVE or REJECT required}"
REPORT="$LOG/reviewer-report.md"
case "$EXPECTED_VERDICT" in
  APPROVE|REJECT) ;;
  *) printf 'invalid expected verdict: %s\n' "$EXPECTED_VERDICT" >&2; exit 2 ;;
esac
test -d "$LOG"
test -s "$REPORT"
test "$(git -C "$REVIEW" rev-parse HEAD)" = "$EXPECTED_SHA"
test "$(rg -c '^reviewer agent: .+' "$REPORT" || true)" -eq 1
test "$(rg -c '^exact SHA: [0-9a-f]{40}$' "$REPORT" || true)" -eq 1
test "$(sed -n 's/^exact SHA: //p' "$REPORT")" = "$EXPECTED_SHA"
for heading in 'inspected files' requirements tests 'failure modes'; do
  test "$(rg -c "^$heading:" "$REPORT" || true)" -eq 1
done
test "$(rg -c '^verdict: (APPROVE|REJECT)$' "$REPORT" || true)" -eq 1
test "$(sed -n 's/^verdict: //p' "$REPORT")" = "$EXPECTED_VERDICT"
```

Create `$SAFETY/bundle-tree-digest.zsh` with `apply_patch` using this exact
content. This is the sole bundle-identity implementation used by baseline,
artifact, smoke, and final gates:

```zsh
#!/bin/zsh
set -euo pipefail
APP="${1:?app bundle path required}"
OUT="${2:?output manifest path required}"
test -d "$APP"
test ! -e "$OUT"
(
  cd "$APP"
  {
    printf 'D\t%s\t%s\t-\t.\n' "$(stat -f '%Sp' .)" "$(stat -f '%z' .)"
    find . -mindepth 1 -print | LC_ALL=C sort | while IFS= read -r rel; do
      mode=$(stat -f '%Sp' "$rel")
      size=$(stat -f '%z' "$rel")
      if test -L "$rel"; then
        printf 'L\t%s\t%s\t%s\t%s\n' "$mode" "$size" "$(readlink "$rel")" "$rel"
      elif test -f "$rel"; then
        digest=$(shasum -a 256 "$rel" | awk '{print $1}')
        printf 'F\t%s\t%s\t%s\t%s\n' "$mode" "$size" "$digest" "$rel"
      elif test -d "$rel"; then
        printf 'D\t%s\t%s\t-\t%s\n' "$mode" "$size" "$rel"
      else
        printf 'unsupported bundle entry: %s\n' "$rel" >&2
        exit 1
      fi
    done
  }
) > "$OUT"
shasum -a 256 "$OUT" | awk '{print $1}'
```

## Required Independent Review Artifact

Every reviewer response is part of the immutable gate evidence, not transient
chat. After each independent review, use the exact SHA and log path printed by
`pin-and-gate-slice.zsh`, assert that `reviewer-report.md` is absent, and use
`apply_patch` to create it once. Use the exact unique keys required by
`verify-review-report.zsh`; also record category, parent SHA, inspected file
details, requirement details, test/log details, failure-mode analysis, and every
P0/P1/P2 finding. Run the verifier with four explicit arguments: review
worktree path, exact attempt log path, exact 40-character SHA, and expected
`APPROVE` or `REJECT`. The next slice always passes `APPROVE` for the latest
attempt before moving on.

A rejected attempt keeps its report forever. Required fixes use a new commit and
new attempt directory; only that new directory may receive the re-review report.
The next slice is blocked until its latest report contains an explicit
`APPROVE`. Task 4, every Task 5/6 sub-slice, Task 7, and the final Task 8 review
all follow this persistence rule. Task 7's disposition ledger references these
exact report paths.

---

### Task 1: Capture A Content-Addressed Safety Archive

**Files:**
- Create outside repo:
  `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence/tracked.patch`
- Create outside repo:
  `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence/untracked.tar`
- Create outside repo:
  `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence/manifest.txt`
- Create outside repo:
  `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence/run-code-slice-gate.zsh`
- Create outside repo:
  `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence/pin-and-gate-slice.zsh`
- Create outside repo:
  `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence/run-desktop-artifact-gate.zsh`
- Create outside repo:
  `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence/verify-review-report.zsh`
- Create outside repo:
  `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence/bundle-tree-digest.zsh`
- Create outside repo:
  `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence/installed-app-bundle.manifest`
- Create outside repo:
  `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence/release-app-bundle.manifest`
- Inspect: canonical worktree status and installed/release binaries

**Interfaces:**
- Consumes: canonical dirty worktree whose HEAD contains `05da823` plus this
  reviewed plan.
- Produces: immutable tracked patch, untracked archive, and per-file/hash
  evidence used by Task 3.

- [ ] **Step 1: Verify the source identity and capture read-only evidence**

Run from the canonical repo:

```bash
set -euo pipefail
git rev-parse HEAD
git merge-base --is-ancestor 05da823 HEAD
git branch --show-current
test "$(git branch --show-current)" = \
  'recovery/superpowers-integration-20260708-v2'
git status --porcelain=v1
git cat-file -e HEAD:docs/superpowers/plans/2026-07-10-opentake-wave-1a-integration-baseline.md
test -z "$(git diff --cached --name-only)"
git diff HEAD --stat
test "$(git diff HEAD --name-only | wc -l | tr -d ' ')" -eq 52
cmp \
  <(git ls-files --others --exclude-standard | LC_ALL=C sort) \
  <(printf '%s\n' \
      'docs/superpowers/archive/2026-07-10-playback-cache-installed-app-qa.md' \
      'web/src/components/preview/nativePlaybackSession.ts' \
      'web/src/components/preview/nativePlaybackSession.test.ts' \
      'web/src/store/mediaActions.test.ts' | LC_ALL=C sort)
EXPECTED_BIN='c5cf2a827d718574cdbc68580d77562bdf86a99cb561052ba20143f96e1956aa'
INSTALLED_BIN=$(shasum -a 256 /Applications/OpenTake.app/Contents/MacOS/opentake | awk '{print $1}')
RELEASE_BIN=$(shasum -a 256 target/release/bundle/macos/OpenTake.app/Contents/MacOS/opentake | awk '{print $1}')
test "$INSTALLED_BIN" = "$EXPECTED_BIN"
test "$RELEASE_BIN" = "$EXPECTED_BIN"
printf '%s  %s\n' "$INSTALLED_BIN" /Applications/OpenTake.app/Contents/MacOS/opentake
printf '%s  %s\n' "$RELEASE_BIN" target/release/bundle/macos/OpenTake.app/Contents/MacOS/opentake
```

Expected:

- `05da823` is an ancestor of HEAD and the plan file exists at HEAD.
- Branch is `recovery/superpowers-integration-20260708-v2`.
- The two executable hashes are
  `c5cf2a827d718574cdbc68580d77562bdf86a99cb561052ba20143f96e1956aa`.
- The dirty paths include the four required untracked files:
  `docs/superpowers/archive/2026-07-10-playback-cache-installed-app-qa.md`,
  `web/src/components/preview/nativePlaybackSession.ts`,
  `web/src/components/preview/nativePlaybackSession.test.ts`, and
  `web/src/store/mediaActions.test.ts`.

- [ ] **Step 2: Create the archive directory and tracked binary patch**

Run:

```bash
set -euo pipefail
SAFETY='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence'
test ! -e "$SAFETY"
mkdir -p "$SAFETY"
git diff HEAD --binary --no-ext-diff --no-textconv \
  --output="$SAFETY/tracked.patch"
```

Expected: the command refuses to overwrite a pre-existing safety root;
`tracked.patch` is non-empty and `git apply --stat tracked.patch` lists every
tracked dirty path without an error. If the root already exists, stop and amend
this plan with a new timestamped path; do not delete or overwrite it.

- [ ] **Step 3: Archive every untracked file with its relative path**

Run from the canonical repo:

```bash
set -euo pipefail
COPYFILE_DISABLE=1 tar -cf '/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence/untracked.tar' \
  'docs/superpowers/archive/2026-07-10-playback-cache-installed-app-qa.md' \
  'web/src/components/preview/nativePlaybackSession.ts' \
  'web/src/components/preview/nativePlaybackSession.test.ts' \
  'web/src/store/mediaActions.test.ts'
tar -tf '/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence/untracked.tar'
```

Expected: the archive lists exactly those four paths.

- [ ] **Step 4: Record per-file sizes and SHA-256 values**

First create all five gate/review/bundle scripts with `apply_patch` using the exact global
content above. Then run and preserve this output for `manifest.txt` using the
controller's patch writer rather than shell redirection:

```bash
set -euo pipefail
SAFETY='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence'
INSTALLED_BUNDLE_SHA=$(zsh "$SAFETY/bundle-tree-digest.zsh" \
  /Applications/OpenTake.app "$SAFETY/installed-app-bundle.manifest")
RELEASE_BUNDLE_SHA=$(zsh "$SAFETY/bundle-tree-digest.zsh" \
  target/release/bundle/macos/OpenTake.app "$SAFETY/release-app-bundle.manifest")
printf '# installed_bundle_sha256 %s\n' "$INSTALLED_BUNDLE_SHA"
printf '# release_bundle_sha256 %s\n' "$RELEASE_BUNDLE_SHA"
git diff HEAD --name-only -z | while IFS= read -r -d '' file; do
  if test -e "$file"; then
    stat -f '# tracked_stat %z %Sp %N' "$file"
    shasum -a 256 "$file"
  else
    printf '# tracked_deleted %s\n' "$file"
  fi
done
for file in \
  'docs/superpowers/archive/2026-07-10-playback-cache-installed-app-qa.md' \
  'web/src/components/preview/nativePlaybackSession.ts' \
  'web/src/components/preview/nativePlaybackSession.test.ts' \
  'web/src/store/mediaActions.test.ts'; do
  stat -f '# untracked_stat %z %Sp %N' "$file"
  shasum -a 256 "$file"
done
shasum -a 256 '/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence/tracked.patch'
shasum -a 256 '/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence/untracked.tar'
shasum -a 256 '/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence/run-code-slice-gate.zsh'
shasum -a 256 '/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence/pin-and-gate-slice.zsh'
shasum -a 256 '/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence/run-desktop-artifact-gate.zsh'
shasum -a 256 '/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence/verify-review-report.zsh'
shasum -a 256 '/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence/bundle-tree-digest.zsh'
shasum -a 256 '/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence/installed-app-bundle.manifest'
shasum -a 256 '/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence/release-app-bundle.manifest'
```

Expected: all 52 tracked paths are recorded as either `# tracked_stat ...`
plus a standard SHA-256 line or `# tracked_deleted ...`; every untracked source
file is recorded as `# untracked_stat ...` plus a standard SHA-256 line; both
archive files, all five scripts, and both complete bundle manifests have
SHA-256 lines. Write those exact lines to `manifest.txt` with `apply_patch`.
The Step 4 output already begins with the unique
`# installed_bundle_sha256 ...` and `# release_bundle_sha256 ...` lines. Prepend
only four additional comment lines from Step 1: `# source_head ...`,
`# source_branch ...`, `# installed_binary_sha256 ...`, and
`# release_binary_sha256 ...`. The resulting six identity keys occur exactly
once. The manifest remains directly usable by
`shasum -c` while binding the archives to one source commit and complete
installed/release bundle identities.

- [ ] **Step 5: Verify the safety artifacts without changing the source tree**

Run:

```bash
set -euo pipefail
SAFETY='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence'
TMP=$(mktemp -d)
git apply --stat "$SAFETY/tracked.patch"
tar -tf "$SAFETY/untracked.tar"
INSTALLED_NOW=$(zsh "$SAFETY/bundle-tree-digest.zsh" \
  /Applications/OpenTake.app "$TMP/installed-app-bundle.manifest")
RELEASE_NOW=$(zsh "$SAFETY/bundle-tree-digest.zsh" \
  target/release/bundle/macos/OpenTake.app "$TMP/release-app-bundle.manifest")
test "$INSTALLED_NOW" = "$(sed -n 's/^# installed_bundle_sha256 //p' "$SAFETY/manifest.txt")"
test "$RELEASE_NOW" = "$(sed -n 's/^# release_bundle_sha256 //p' "$SAFETY/manifest.txt")"
cmp "$SAFETY/installed-app-bundle.manifest" "$TMP/installed-app-bundle.manifest"
cmp "$SAFETY/release-app-bundle.manifest" "$TMP/release-app-bundle.manifest"
git status --short --branch
```

Expected: patch statistics are readable, archive listing is complete, and canonical
status is byte-for-byte unchanged from Step 1.

### Task 2: Create The Isolated Integration Worktree

**Files:**
- Create worktree directory:
  `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-full-convergence`
- Create branch: `integration/opentake-full-convergence-20260710`

**Interfaces:**
- Consumes: committed design at `05da823`, the committed reviewed plan, and Task
  1 safety artifacts.
- Produces: clean linked worktree ready for exact restoration.

- [ ] **Step 1: Detect Git topology before worktree creation**

Run from the canonical repo:

```bash
set -euo pipefail
GIT_DIR=$(cd "$(git rev-parse --git-dir)" && pwd -P)
GIT_COMMON=$(cd "$(git rev-parse --git-common-dir)" && pwd -P)
BRANCH=$(git branch --show-current)
MANIFEST='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence/manifest.txt'
SOURCE_HEAD=$(sed -n 's/^# source_head //p' "$MANIFEST")
printf '%s\n%s\n%s\n' "$GIT_DIR" "$GIT_COMMON" "$BRANCH"
test "$SOURCE_HEAD" = "$(git rev-parse HEAD)"
test "$SOURCE_HEAD" = "$(git rev-parse recovery/superpowers-integration-20260708-v2)"
test -z "$(git rev-parse --show-superproject-working-tree)"
git worktree list --porcelain
```

Expected: canonical checkout is not detached, and the target worktree path and
branch do not already exist. If either exists, inspect it and reuse only when it
points at the exact reviewed-plan commit recorded in Task 1 with a clean status;
never delete it automatically.

- [ ] **Step 2: Create the branch and linked worktree**

Run:

```bash
set -euo pipefail
MANIFEST='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence/manifest.txt'
SOURCE_HEAD=$(sed -n 's/^# source_head //p' "$MANIFEST")
git worktree add \
  -b integration/opentake-full-convergence-20260710 \
  '/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-full-convergence' \
  "$SOURCE_HEAD"
```

Expected: Git reports a new worktree on the named integration branch.

- [ ] **Step 3: Verify isolation**

Run from the new worktree:

```bash
set -euo pipefail
git rev-parse HEAD
git branch --show-current
git status --porcelain
pnpm -C web install --frozen-lockfile
test -z "$(git status --porcelain)"
```

Expected: HEAD matches the canonical source branch's recorded plan commit,
`05da823` is an ancestor, the branch is correct, and status is empty. Re-run
`git status --short --branch` in the canonical worktree and confirm its dirty
state is unchanged. The integration worktree now has ignored local dependencies
needed for its RED/GREEN commands without adding a repository path.

- [ ] **Step 4: Create the clean detached review worktree**

Run from the integration worktree:

```bash
set -euo pipefail
REVIEW='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-wave1a-review'
MANIFEST='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence/manifest.txt'
SOURCE_HEAD=$(sed -n 's/^# source_head //p' "$MANIFEST")
test ! -e "$REVIEW"
git worktree add --detach "$REVIEW" "$SOURCE_HEAD"
test "$(git -C "$REVIEW" rev-parse HEAD)" = "$SOURCE_HEAD"
test -z "$(git -C "$REVIEW" status --porcelain)"
```

Expected: a second clean worktree exists at the exact immutable source SHA. If
the path already exists, stop and inspect; never overwrite or remove it.

- [ ] **Step 5: Establish the clean pre-restoration baseline**

Run from the detached review worktree before applying any patch:

```bash
set -euo pipefail
SAFETY='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence'
LOG="$SAFETY/logs/clean-source-baseline"
REVIEW='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-wave1a-review'
CARGO_AUDIT_BIN='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake/target/cargo-tools/bin/cargo-audit'
test ! -e "$LOG"
mkdir -p "$LOG"
cd "$REVIEW"
test -x "$CARGO_AUDIT_BIN"
pnpm -C web install --frozen-lockfile 2>&1 | tee "$LOG/pnpm-install.log"
cargo fmt --all --check 2>&1 | tee "$LOG/cargo-fmt.log"
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tee "$LOG/cargo-clippy.log"
cargo test --workspace -- --nocapture 2>&1 | tee "$LOG/cargo-test.log"
cargo clippy -p opentake-tauri --no-default-features --all-targets -- -D warnings \
  2>&1 | tee "$LOG/cargo-clippy-no-default.log"
pnpm -C web build 2>&1 | tee "$LOG/web-build.log"
pnpm -C web test 2>&1 | tee "$LOG/web-test.log"
pnpm -C web audit --audit-level high 2>&1 | tee "$LOG/pnpm-audit.log"
"$CARGO_AUDIT_BIN" audit 2>&1 | tee "$LOG/cargo-audit.log"
test -z "$(git status --porcelain)"
```

Expected: the clean source commit passes before restoration and stays clean.
If any command fails, stop before Task 3, diagnose the source baseline, and
amend this plan with the reviewed prerequisite; never attribute an existing
source failure to the restored patch or continue silently.

### Task 3: Hash-Verify And Freeze Every Dirty Byte Without Polluting Integration

**Files:**
- Create an alternate Git index at `$SAFETY/restored.index`.
- Create immutable ref `safety/opentake-wave1a-restored-20260710`.
- Do not create or modify a file in the integration or review worktree.

**Interfaces:**
- Consumes: Task 1 archive, the canonical dirty worktree as a read-only byte
  source, and the two clean Task 2 worktrees.
- Produces: an immutable safety commit whose parent is `SOURCE_HEAD` and whose
  tree exactly represents all 52 tracked plus four untracked baseline paths.
  Integration and review remain clean at `SOURCE_HEAD`.

- [ ] **Step 1: Revalidate the canonical bytes and normal index**

Run from the canonical worktree. These commands are read-only:

```bash
set -euo pipefail
CANON='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake'
INTEGRATION='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-full-convergence'
SAFETY='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence'
MANIFEST="$SAFETY/manifest.txt"
cd "$CANON"
SOURCE_HEAD=$(sed -n 's/^# source_head //p' "$MANIFEST")
for key in \
  source_head source_branch installed_binary_sha256 release_binary_sha256 \
  installed_bundle_sha256 release_bundle_sha256; do
  test "$(rg -c "^# $key " "$MANIFEST" || true)" -eq 1
done
test "$(git rev-parse HEAD)" = "$SOURCE_HEAD"
test -z "$(git diff --cached --name-only)"
test "$(git diff HEAD --name-only | wc -l | tr -d ' ')" -eq 52
cmp \
  <(git ls-files --others --exclude-standard | LC_ALL=C sort) \
  <(printf '%s\n' \
      'docs/superpowers/archive/2026-07-10-playback-cache-installed-app-qa.md' \
      'web/src/components/preview/nativePlaybackSession.ts' \
      'web/src/components/preview/nativePlaybackSession.test.ts' \
      'web/src/store/mediaActions.test.ts' | LC_ALL=C sort)
shasum -a 256 -c "$MANIFEST"
git -C "$INTEGRATION" apply --check "$SAFETY/tracked.patch"
test "$(tar -tf "$SAFETY/untracked.tar" | wc -l | tr -d ' ')" -eq 4
```

Expected: source identity, the empty normal index, exact 52+4 path set, archive
hashes, and patch applicability all still match Task 1. Stop on any drift.

- [ ] **Step 2: Build the safety tree through an alternate index only**

The following changes only `$SAFETY/restored.index` and the shared Git object
database. It must not alter the canonical normal index or any worktree file:

```bash
set -euo pipefail
CANON='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake'
SAFETY='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence'
MANIFEST="$SAFETY/manifest.txt"
INDEX="$SAFETY/restored.index"
SAFETY_REF='refs/heads/safety/opentake-wave1a-restored-20260710'
ZERO='0000000000000000000000000000000000000000'
cd "$CANON"
SOURCE_HEAD=$(sed -n 's/^# source_head //p' "$MANIFEST")
test ! -e "$INDEX"
test -z "$(git show-ref --verify --hash "$SAFETY_REF" || true)"
GIT_INDEX_FILE="$INDEX" git read-tree "$SOURCE_HEAD"
{
  git diff "$SOURCE_HEAD" --name-only -z
  printf '%s\0' \
    'docs/superpowers/archive/2026-07-10-playback-cache-installed-app-qa.md' \
    'web/src/components/preview/nativePlaybackSession.ts' \
    'web/src/components/preview/nativePlaybackSession.test.ts' \
    'web/src/store/mediaActions.test.ts'
} | xargs -0 env GIT_INDEX_FILE="$INDEX" git add --
test "$(GIT_INDEX_FILE="$INDEX" git diff --cached --name-only "$SOURCE_HEAD" | wc -l | tr -d ' ')" -eq 56
RESTORED_TREE=$(GIT_INDEX_FILE="$INDEX" git write-tree)
GIT_INDEX_FILE="$INDEX" git diff --exit-code "$RESTORED_TREE" --
test "$(git diff-tree --no-commit-id --name-only -r "$SOURCE_HEAD" "$RESTORED_TREE" | wc -l | tr -d ' ')" -eq 56
RESTORED_COMMIT=$(printf '%s\n' 'safety: preserve Wave 1A restored snapshot' | \
  git commit-tree "$RESTORED_TREE" -p "$SOURCE_HEAD")
git update-ref "$SAFETY_REF" "$RESTORED_COMMIT" "$ZERO"
git show --no-patch --format='%H %T %P %s' "$RESTORED_COMMIT"
test -z "$(git diff --cached --name-only)"
```

Expected: the alternate index matches the canonical worktree, exactly 56 paths
differ from the source parent, and the canonical normal index remains empty.
Append `# restored_tree ...` and `# restored_commit ...` to `manifest.txt` with
`apply_patch`, using the exact printed values. Never rewrite this safety ref.

- [ ] **Step 3: Prove the safety commit reproduces both archives**

Run from the canonical worktree:

```bash
set -euo pipefail
SAFETY='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence'
MANIFEST="$SAFETY/manifest.txt"
SOURCE_HEAD=$(sed -n 's/^# source_head //p' "$MANIFEST")
RESTORED_COMMIT=$(sed -n 's/^# restored_commit //p' "$MANIFEST")
git diff --binary --no-ext-diff --no-textconv \
  "$SOURCE_HEAD" "$RESTORED_COMMIT" -- \
  . \
  ':(exclude)docs/superpowers/archive/2026-07-10-playback-cache-installed-app-qa.md' \
  ':(exclude)web/src/components/preview/nativePlaybackSession.ts' \
  ':(exclude)web/src/components/preview/nativePlaybackSession.test.ts' \
  ':(exclude)web/src/store/mediaActions.test.ts' \
  > /tmp/opentake-wave1a-safety-tracked.patch
cmp "$SAFETY/tracked.patch" /tmp/opentake-wave1a-safety-tracked.patch
for file in \
  'docs/superpowers/archive/2026-07-10-playback-cache-installed-app-qa.md' \
  'web/src/components/preview/nativePlaybackSession.ts' \
  'web/src/components/preview/nativePlaybackSession.test.ts' \
  'web/src/store/mediaActions.test.ts'; do
  cmp "$file" <(tar -xOf "$SAFETY/untracked.tar" "$file")
  cmp "$file" <(git show "$RESTORED_COMMIT:$file")
done
test "$(git rev-parse "$RESTORED_COMMIT^")" = "$SOURCE_HEAD"
test "$(git rev-parse safety/opentake-wave1a-restored-20260710)" = "$RESTORED_COMMIT"
```

Expected: the safety commit recreates the tracked binary patch byte-for-byte,
every tar member and all four safety blobs match the canonical files
byte-for-byte, and the commit has exactly `SOURCE_HEAD` as parent.

- [ ] **Step 4: Prove implementation worktrees are still clean**

```bash
set -euo pipefail
INTEGRATION='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-full-convergence'
REVIEW='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-wave1a-review'
MANIFEST='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence/manifest.txt'
SOURCE_HEAD=$(sed -n 's/^# source_head //p' "$MANIFEST")
for tree in "$INTEGRATION" "$REVIEW"; do
  test "$(git -C "$tree" rev-parse HEAD)" = "$SOURCE_HEAD"
  test -z "$(git -C "$tree" status --porcelain)"
done
```

Expected: neither implementation worktree ever received the 56-path mixed
snapshot. From Task 4 onward, inspect baseline hunks with
`git diff "$SOURCE_HEAD" "$RESTORED_COMMIT" -- <owned-paths>` and implement only
the current slice with `apply_patch`; never apply that whole diff or replace a
whole overlapping path from `RESTORED_COMMIT`.

### Task 4: Commit Runtime And Dependency Removal

**Files:**
- Modify: `.github/workflows/ci.yml`
- Modify: `.gitignore`
- Modify: `Cargo.lock`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/capabilities/default.json`
- Modify: `src-tauri/src/lib.rs`
- Delete: `src-tauri/src/mpv_bootstrap.rs`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `web/package.json`
- Modify: `web/pnpm-lock.yaml`
- Modify: `web/src/App.tsx`
- Modify: `web/src/components/preview/Preview.tsx`
- Modify: `web/src/components/preview/Preview.test.tsx`
- Modify: `web/src/components/preview/previewEngine.ts`
- Modify: `web/src/components/preview/previewEngine.test.ts`
- Delete: `web/src/lib/mpvEdl.ts`
- Delete: `web/src/lib/mpvEdl.test.ts`
- Modify: `web/src/styles/global.css`

**Interfaces:**
- Consumes: the immutable restored snapshot from Task 3.
- Produces: a buildable runtime with the external libmpv plugin, wrapper dylib,
  and obsolete EDL bridge removed while retaining the embedded Rust compositor.

- [ ] **Step 1: Remove libmpv as one buildable vertical slice**

Inspect the Task 4 hunks in the safety commit, then use `apply_patch` in the
clean integration worktree. Remove the obsolete `.gitignore` exception, Rust
plugin/bootstrap/capability/configuration, JS dependency/lock entry, EDL bridge,
and transparent native-window hole. `Preview.tsx` must become an opaque WebKit
surface with no native margin call and no frame listener yet. Remove the mpv
branch from `previewEngine.ts`; until Sub-slice 5.2 lands its complete
identity-scoped native vertical path, the existing single WebKit clock remains
the only playback path. Make the body/root background opaque in `global.css`
and remove its stale mpv comment; do not take any later UI/accessibility style
hunk from the safety snapshot. Update the two owning Web tests to prove ordinary
play/pause still works and neither component imports or calls the removed
plugin. Do not introduce the native frame API in this slice. Then run:

```bash
set -euo pipefail
if rg -n \
  'tauri-plugin-libmpv|tauri_plugin_libmpv|mpv_bootstrap|mpvEdl|MpvPlayer' \
  .github Cargo.lock src-tauri web; then
  exit 1
fi
pnpm -C web install --frozen-lockfile
pnpm -C web test \
  src/components/preview/Preview.test.tsx \
  src/components/preview/previewEngine.test.ts
pnpm -C web exec tsc -b --pretty false
git diff --check
```

Expected: the scan finds no live source/config/lock reference, the clean commit
candidate compiles without the removed npm module, WebKit playback tests pass,
and the diff is whitespace-clean.

- [ ] **Step 2: Stage only runtime/dependency paths**

```bash
set -euo pipefail
PARENT=$(git rev-parse HEAD)
git add -- \
  .github/workflows/ci.yml \
  .gitignore \
  Cargo.lock \
  src-tauri/Cargo.toml \
  src-tauri/capabilities/default.json \
  src-tauri/src/lib.rs \
  src-tauri/src/mpv_bootstrap.rs \
  src-tauri/tauri.conf.json \
  web/package.json \
  web/pnpm-lock.yaml \
  web/src/App.tsx \
  web/src/components/preview/Preview.tsx \
  web/src/components/preview/Preview.test.tsx \
  web/src/components/preview/previewEngine.ts \
  web/src/components/preview/previewEngine.test.ts \
  web/src/lib/mpvEdl.ts \
  web/src/lib/mpvEdl.test.ts \
  web/src/styles/global.css
git diff --cached --check
git diff --cached --name-status
git diff --name-status "$PARENT" --cached
git diff "$PARENT" --cached --
```

Expected: exactly the eighteen Task 4 paths are staged. Also record the parent
SHA and show `git diff --name-status "$PARENT" --cached` plus
`git diff "$PARENT" --cached --`; reject native-session, retained-frame,
prewarm, or later route hunks even though `Preview.tsx` and `previewEngine.ts`
will be modified again in later slices.

- [ ] **Step 3: Commit the exact slice**

```bash
set -euo pipefail
git commit -m "refactor(playback): remove the libmpv plugin runtime"
```

- [ ] **Step 4: Run the complete public-slice and desktop-artifact gates**

```bash
set -euo pipefail
SAFETY='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence'
INTEGRATION='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-full-convergence'
REVIEW='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-wave1a-review'
zsh "$SAFETY/pin-and-gate-slice.zsh" \
  "$INTEGRATION" "$REVIEW" '01-runtime-dependency' 'initial' 'required'
```

Expected: full Rust/Web gates and audits pass from the exact clean commit; a
fresh app bundle is produced without a libmpv dependency; logs are immutable.

- [ ] **Step 5: Independent reviewer and mandatory re-review gate**

Dispatch a reviewer that did not implement the slice. It inspects the exact
commit, design category, dependency/lock changes, no-orphan scan, full gate,
bundle resources, security, and unnecessary changes. For every Required or
Critical finding, fix in the integration worktree, commit with
`fix(review): address runtime dependency findings`, switch the review worktree
to the new exact SHA, rerun Step 4 with a unique `review-fix-N` suffix, and send
the new diff/logs back to an independent reviewer. Do not enter Task 5 until a
reviewer explicitly states `APPROVE` with inspected files, requirements, tests,
and failure modes.

### Task 5: Resolve Rust Playback And Media Review Findings

**Interfaces:**
- Consumes: approved Task 4 runtime and the immutable restored snapshot.
- Produces: six independently reviewed Rust/vertical commits that close every
  Critical/Required backend finding before Web rendering work continues.

**Mandatory protocol for each sub-slice:**

1. Inspect only the current slice's hunks with
   `git diff "$SOURCE_HEAD" "$RESTORED_COMMIT" -- <owned-paths>`, then add the
   named RED tests (a minimal compile-only API skeleton is allowed, but
   the RED run must fail on the intended assertion, not missing symbols).
2. Implement only that sub-slice and run its targeted GREEN commands.
3. Stage only its exact paths and commit with the exact message. Never copy a
   whole overlapping path from the safety commit. Inspect the generated
   `commit-name-status.log`/`commit.diff` for the latest commit and
   `slice-name-status.log`/`slice.diff` for the original slice base through the
   current review-fix SHA; reject any hunk owned by a later slice even when the
   path is repeated later in this plan.
4. Run `pin-and-gate-slice.zsh` with the exact category and `initial`. Pass
   `required` for a user-visible slice so the helper invokes
   `run-desktop-artifact-gate.zsh` exactly once; pass `skip` otherwise. Never
   call the desktop helper a second time for the same attempt/log directory.
5. Dispatch an independent reviewer over the exact commit and immutable logs.
   Required fixes use a dedicated `fix(review): ...` commit, `review-fix-1`
   (incrementing for later rounds), full targeted + public + artifact reruns, and
   independent re-review. The next sub-slice is blocked until explicit
   `APPROVE` names inspected files, requirements, tests, and failure modes.

The exact pin/full/artifact command shape is:

```bash
set -euo pipefail
INTEGRATION='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-full-convergence'
REVIEW='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-wave1a-review'
SAFETY='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence'
zsh "$SAFETY/pin-and-gate-slice.zsh" \
  "$INTEGRATION" "$REVIEW" '02a-core-project-identity' 'initial' 'required'
```

Replace only the category with the exact category named by that sub-slice. For
a test-only or pure-contract sub-slice pass `skip`; never omit the pin/full gate
or reviewer. Review reruns replace `initial` with `review-fix-1`, incrementing
the suffix for each later round; the script refuses log reuse.

#### Sub-slice 5.1: Authoritative Project Epoch And Atomic Runtime Snapshot

**Files:**
- Modify: `crates/opentake-core/src/core.rs`
- Modify: `crates/opentake-core/src/dto.rs`
- Modify: `crates/opentake-core/src/events.rs`
- Modify: `crates/opentake-core/src/lib.rs`
- Modify: `src-tauri/src/commands.rs`

Create `CoreSessionSlot { project_epoch, editor }` under the existing session
mutex; do not use a separately-read epoch atomic. Add `ProjectRevision`,
`ProjectRuntimeSnapshot`, `project_revision()`, and `runtime_snapshot()` so
timeline, media, project directory, epoch, and version come from one lock.
Split project opening into non-mutating `prepare_project_open` and atomic
`commit_project_open`; increment epoch only after successful new/open. Extend
snapshot DTOs and core events with `projectEpoch`; return the first snapshot
from `project_new` as `project_open` already does.

RED/GREEN tests in `core.rs`:

- `opening_two_projects_produces_distinct_epochs_at_version_zero`
- `new_project_advances_epoch_even_when_versions_collide`
- `runtime_snapshot_never_mixes_timeline_media_and_project_dir`

Run:

```bash
set -euo pipefail
cargo test -p opentake-core opening_two_projects_produces_distinct_epochs_at_version_zero -- --nocapture
cargo test -p opentake-core new_project_advances_epoch_even_when_versions_collide -- --nocapture
cargo test -p opentake-core runtime_snapshot_never_mixes_timeline_media_and_project_dir -- --nocapture
cargo fmt --all --check
```

Stage the five paths above and commit:

```bash
set -euo pipefail
git add -- \
  crates/opentake-core/src/core.rs \
  crates/opentake-core/src/dto.rs \
  crates/opentake-core/src/events.rs \
  crates/opentake-core/src/lib.rs \
  src-tauri/src/commands.rs
git diff --cached --check
git commit -m "fix(core): identify project sessions across version resets"
```

Set `CATEGORY=02a-core-project-identity`, `ATTEMPT=initial`, run the mandatory
protocol including the desktop artifact gate, and obtain explicit approval.

#### Sub-slice 5.2: Stale-Start Rejection And Session-Scoped Publication

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/playback/audio.rs`
- Modify: `src-tauri/src/playback/commands.rs`
- Modify: `src-tauri/src/playback/engine.rs`
- Modify: `src-tauri/src/playback/mod.rs`
- Modify: `src-tauri/src/playback/resolver.rs`
- Create: `src-tauri/src/playback/session.rs`
- Modify: `src-tauri/src/playback/transport.rs`
- Modify: `src-tauri/tests/playback_integration.rs`
- Modify: `src-tauri/tests/playback_transport_integration.rs`
- Create: `web/src/components/preview/nativePlaybackSession.ts`
- Create: `web/src/components/preview/nativePlaybackSession.test.ts`
- Modify: `web/src/components/preview/Preview.tsx`
- Modify: `web/src/components/preview/Preview.test.tsx`
- Modify: `web/src/components/preview/previewEngine.ts`
- Modify: `web/src/components/preview/previewEngine.test.ts`
- Modify: `web/src/components/preview/timelinePlayback.ts`
- Modify: `web/src/components/preview/timelinePlayback.test.ts`
- Modify: `web/src/lib/api.ts`
- Create: `web/src/lib/api.test.ts`
- Modify: `web/src/lib/types.ts`
- Modify: `web/src/store/projectActions.ts`
- Modify: `web/src/store/projectActions.test.ts`
- Modify: `web/src/store/projectStore.ts`
- Modify: `web/src/store/sync.ts`
- Create: `web/src/store/sync.test.ts`

Define one public identity
`{ projectEpoch, timelineVersion, sessionId }`; the Web client mints `sessionId`
before registering the listener and before starting. Add `frame`, `sequence`,
and `terminal` to frame events and tag the latest JPEG with the same identity.
Validate `sessionId` at the Tauri/HTTP
boundary as 1–128 ASCII alphanumeric/hyphen characters. Start uses one atomic runtime snapshot,
rejects mismatched revision before disturbing the current session, and rechecks
authoritative revision during `install_if_current`. Project new/open use
`begin_project_transition` before commit and activate the new epoch after commit;
failed open changes neither epoch nor playback. Pause/seek/stop require matching
identity; only project boundaries call `stop_all`. `/frame` returns 204 for the
wrong session/sequence. Before forwarding any `TimelineChanged` or
`ProjectOpened` event originating from UI, MCP, or another client,
`src-tauri/lib.rs` synchronously invalidates the matching playback revision so
external project/edit paths cannot bypass the guard.

Make this a complete pixel-producing Web vertical slice in the same commit.
`previewEngine.ts` is the only `onPlaybackFrame` subscriber: it validates all
three identity fields, frame, sequence, and terminal before publishing a typed
native frame state from `nativePlaybackSession.ts`. `Preview.tsx` consumes that
validated state without registering a listener. A pure helper in
`timelinePlayback.ts` creates exactly this cache-busted URL:
`/frame?projectEpoch=<epoch>&timelineVersion=<version>&sessionId=<id>&frame=<frame>&sequence=<sequence>`.
No identity field and no sequence may be defaulted or inferred from current
store state. Keep the existing WebKit surface visible until the first matching
native `<img>` load succeeds; a one-slot native surface is sufficient in this
slice, because Sub-slice 6.2 upgrades it to the retained two-slot terminal
buffer. Pause must retain the last successful one-slot image. Keep the Rust
addition wire-compatible only within this single atomic commit; final Web code
must consume the full identity immediately and no `?f=&seq=` compatibility URL
may remain.
Correct the stale `spawn_ready` and preview-endpoint comments while touching
their owning functions; comments must describe buffered first-frame publication
and `/frame`, not a pre-published sink or WebSocket canvas.

Return a serializable `PlaybackCommandError` with code
`superseded | cancelled | busy | engine`; Web treats the first three as expected
state transitions and never activates compatibility fallback. Only `engine` is
eligible for fallback; Sub-slice 6.1 adds the required WebKit pixel-parity gate.

Required Rust tests (in `session.rs`, `commands.rs`, and `transport.rs`):

- `same_version_different_epoch_never_resumes_paused_playback`
- `project_transition_rejects_start_between_invalidation_and_commit`
- `project_open_failure_does_not_advance_epoch_or_stop_playback`
- `project_swap_happens_only_after_old_publication_is_closed`
- `stale_requested_revision_is_rejected_without_displacing_current_session`
- `timeline_change_during_start_rejects_pending_install`
- `project_change_during_start_rejects_pending_install_even_when_version_is_zero`
- `stale_session_control_cannot_pause_seek_or_stop_replacement`
- `playhead_event_carries_session_revision_sequence_and_terminal`
- `frame_route_never_serves_another_session_latest`

Required Web tests:

- `stops a retained session when project epoch changes but both versions are zero`
- `rejects a frame from a stale backend session even when revisions collide`
- `does not let stale cleanup pause a replacement session`
- `retains one session id across pause and resume of the exact revision`
- `mints a new session id after project or timeline revision changes`
- `session scopes pause seek and stop commands`
- `stops native playback before opening a project whose version collides`
- `stops native playback before creating a fresh project`
- `invalidates project scoped playback on externally initiated project_opened`
- `decodes the full playback frame identity instead of accepting frame only`
- `registers exactly one playback frame listener before starting the session`
- `builds a frame URL with project epoch timeline version session id frame and sequence`
- `rejects an event or image load missing any identity field`
- `keeps WebKit visible until the first matching native frame has loaded`
- `retains the matching loaded native frame when playback pauses`

Focused GREEN:

```bash
set -euo pipefail
cargo test -p opentake-tauri --features playback-engine playback::session --lib -- --nocapture
cargo test -p opentake-tauri --features playback-engine playback::commands --lib -- --nocapture
cargo test -p opentake-tauri --features playback-engine playback::transport --lib -- --nocapture
pnpm -C web test \
  src/components/preview/nativePlaybackSession.test.ts \
  src/components/preview/Preview.test.tsx \
  src/components/preview/previewEngine.test.ts \
  src/components/preview/timelinePlayback.test.ts \
  src/lib/api.test.ts \
  src/store/projectActions.test.ts \
  src/store/sync.test.ts
pnpm -C web exec tsc -b --pretty false
```

Stage and commit:

```bash
set -euo pipefail
git add -- \
  src-tauri/src/commands.rs \
  src-tauri/src/lib.rs \
  src-tauri/src/playback/audio.rs \
  src-tauri/src/playback/commands.rs \
  src-tauri/src/playback/engine.rs \
  src-tauri/src/playback/mod.rs \
  src-tauri/src/playback/resolver.rs \
  src-tauri/src/playback/session.rs \
  src-tauri/src/playback/transport.rs \
  src-tauri/tests/playback_integration.rs \
  src-tauri/tests/playback_transport_integration.rs \
  web/src/components/preview/nativePlaybackSession.ts \
  web/src/components/preview/nativePlaybackSession.test.ts \
  web/src/components/preview/Preview.tsx \
  web/src/components/preview/Preview.test.tsx \
  web/src/components/preview/previewEngine.ts \
  web/src/components/preview/previewEngine.test.ts \
  web/src/components/preview/timelinePlayback.ts \
  web/src/components/preview/timelinePlayback.test.ts \
  web/src/lib/api.ts \
  web/src/lib/api.test.ts \
  web/src/lib/types.ts \
  web/src/store/projectActions.ts \
  web/src/store/projectActions.test.ts \
  web/src/store/projectStore.ts \
  web/src/store/sync.ts \
  web/src/store/sync.test.ts
git diff --cached --check
git commit -m "fix(playback): reject stale starts and scope frame publications"
```

Set
`CATEGORY=02b-playback-session-identity`, run the mandatory protocol and desktop
artifact gate, then obtain explicit approval.

#### Sub-slice 5.3: Cancellable Audio Preparation And Bounded Reaping

**Files:**
- Create: `crates/opentake-media/src/cancel.rs`
- Modify: `crates/opentake-media/src/lib.rs`
- Modify: `crates/opentake-media/src/decode/audio_stream.rs`
- Modify: `crates/opentake-media/src/decode/mod.rs`
- Modify: `crates/opentake-media/src/decode/pcm.rs`
- Modify: `crates/opentake-media/src/decode/frame.rs`
- Modify: `crates/opentake-media/src/waveform/mod.rs`
- Modify: `src-tauri/src/playback/audio.rs`
- Modify: `src-tauri/src/playback/commands.rs`
- Modify: `src-tauri/src/playback/engine.rs`
- Modify: `src-tauri/src/playback/session.rs`

Add cloneable `MediaCancelToken` and cancellable media variants. PCM cancellation
must drain stdout/stderr concurrently, poll the child, `kill` + `wait`, join
readers, and return `MediaError::Cancelled`. Propagate `Result` through audio
decode/mix; never treat decode failure as a silent timeline. Replace per-start
`spawn_blocking` with one capacity-1 `AudioPrepareWorker`, and per-session
reaper threads with one capacity-2 `BoundedReaper`. Shutdown order is:
publication gate closed, session JPEG cleared, audio muted, stop requested,
combined handles queued; no Tauri command performs an unbounded join.

Bound memory as well as concurrency: compute PCM output size with checked
arithmetic before decode, cap one session's pre-mix at 256 MiB, cap each stdout
reader at its expected bytes plus one frame, use `try_reserve_exact`, and return
structured `audio_buffer_too_large` or allocation errors before OOM. Never
truncate audio or silently switch clocks; longer projects receive an explicit
route error until chunked streaming lands in the later audio wave.

Required tests:

- `cancelling_running_pcm_decode_kills_child_and_reaps_readers`
- `pre_cancelled_pcm_decode_does_not_spawn_ffmpeg`
- `audio_prepare_cancel_stops_before_decoding_next_clip`
- `large_mix_observes_cancellation_between_chunks`
- `audio_prepare_rejects_projected_mix_over_256_mib_without_allocation`
- `audio_decode_failure_is_not_silently_treated_as_silent_timeline`
- `rapid_superseding_starts_never_exceed_one_audio_prepare_job`
- `bounded_reaper_rejects_new_start_when_teardown_backlog_is_full`
- `cancelled_prepare_releases_capacity_only_after_worker_exits`
- `shutdown_closes_publication_and_mutes_audio_before_any_reap`
- `late_inflight_render_cannot_publish_after_project_boundary_returns`
- `clearing_old_session_latest_cannot_clear_replacement_session_frame`

Run the matching `opentake-media` decode tests and
`cargo test -p opentake-tauri --features playback-engine playback --lib --
--nocapture`; no cancellation test may skip. Stage and commit:

```bash
set -euo pipefail
cargo test -p opentake-media cancelling_running_pcm_decode_kills_child_and_reaps_readers -- --nocapture
cargo test -p opentake-media pre_cancelled_pcm_decode_does_not_spawn_ffmpeg -- --nocapture
cargo test -p opentake-tauri --features playback-engine playback --lib -- --nocapture
cargo fmt --all --check
git add -- \
  crates/opentake-media/src/cancel.rs \
  crates/opentake-media/src/lib.rs \
  crates/opentake-media/src/decode/audio_stream.rs \
  crates/opentake-media/src/decode/mod.rs \
  crates/opentake-media/src/decode/pcm.rs \
  crates/opentake-media/src/decode/frame.rs \
  crates/opentake-media/src/waveform/mod.rs \
  src-tauri/src/playback/audio.rs \
  src-tauri/src/playback/commands.rs \
  src-tauri/src/playback/engine.rs \
  src-tauri/src/playback/session.rs
git diff --cached --check
git commit -m "fix(playback): cancel and bound native playback workers"
```

Set
`CATEGORY=02c-playback-bounded-workers`, run the mandatory protocol/artifact
gate, and obtain approval.

#### Sub-slice 5.4: Exact Cold Bootstrap Frame

**Files:**
- Modify: `src-tauri/src/playback/engine.rs`
- Modify: `src-tauri/src/playback/resolver.rs`
- Modify: `src-tauri/tests/playback_integration.rs`

Extract `bootstrap_frame_request` with zero tolerance at the exact source frame;
return decode errors instead of publishing a black layer. Add:

- `bootstrap_request_has_zero_tolerance_at_exact_source_frame`
- `cold_bootstrap_uses_exact_trimmed_source_frame`
- `cold_bootstrap_decode_failure_is_reported_instead_of_publishing_black`

The integration test generates a CFR fixture whose adjacent frames have distinct
pixels, trims from a non-zero frame, and asserts the first composite matches the
target and not its predecessor. Run the focused resolver/engine unit tests and
the no-skip `playback_integration` target. Stage and commit:

```bash
set -euo pipefail
cargo test -p opentake-tauri --features playback-engine bootstrap_request_has_zero_tolerance_at_exact_source_frame --lib -- --nocapture
cargo test -p opentake-tauri --features playback-engine --test playback_integration cold_bootstrap -- --nocapture
cargo fmt --all --check
git add -- \
  src-tauri/src/playback/engine.rs \
  src-tauri/src/playback/resolver.rs \
  src-tauri/tests/playback_integration.rs
git diff --cached --check
git commit -m "fix(playback): bootstrap the exact requested source frame"
```

Set
`CATEGORY=02d-playback-exact-bootstrap`, run the mandatory protocol/artifact
gate, and obtain approval.

#### Sub-slice 5.5: Bounded Project-Scoped Media Prewarm

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/media.rs`
- Create: `src-tauri/src/media/prewarm.rs`
- Modify: `crates/opentake-media/src/decode/frame.rs`
- Modify: `crates/opentake-media/src/waveform/mod.rs`

Move the global queue/reservation/schedule/warm code out of the 2k-line
`media.rs`. `PrewarmScheduler` uses `sync_channel(24)`, exactly three workers,
`try_send`, and state `{ active_epoch, transitioning, cancel, in_flight }`.
Every job carries epoch/token/kind/cache key; project transition cancels the old
token and forbids admission, activation rotates the token, stale work cannot
rename cache output, and queue pressure returns structured
`Queued | Duplicate | Cached | Busy | StaleProject` rather than silent `Ok(())`.
The final open order is prepare project, begin playback transition, begin
prewarm transition, commit the core swap, then activate the new epoch in both
states; failed prepare changes none of them.

Tests in `media/prewarm.rs`:

- `queue_capacity_is_bounded_and_excess_job_returns_busy`
- `same_epoch_kind_and_cache_key_is_coalesced`
- `project_epoch_rotation_cancels_queued_jobs`
- `project_epoch_rotation_cancels_running_decoder`
- `stale_epoch_job_never_commits_cache_file`
- `worker_concurrency_never_exceeds_three`
- `new_epoch_can_schedule_same_cache_key_after_old_reservation_drops`

Run `cargo test -p opentake-tauri --features playback-engine media::prewarm
--lib -- --nocapture` and `media::tests`; stage and commit:

```bash
set -euo pipefail
cargo test -p opentake-tauri --features playback-engine media::prewarm --lib -- --nocapture
cargo test -p opentake-tauri --features playback-engine media::tests --lib -- --nocapture
cargo fmt --all --check
git add -- \
  src-tauri/src/commands.rs \
  src-tauri/src/lib.rs \
  src-tauri/src/media.rs \
  src-tauri/src/media/prewarm.rs \
  crates/opentake-media/src/decode/frame.rs \
  crates/opentake-media/src/waveform/mod.rs
git diff --cached --check
git commit -m "fix(media): scope bounded prewarm jobs to the active project"
```

Set
`CATEGORY=02e-media-project-prewarm`, run the mandatory protocol/artifact gate,
and obtain approval.

#### Sub-slice 5.6: Fail-Closed Live Transport Integration

**Files:**
- Modify: `.github/workflows/ci.yml`
- Modify: `src-tauri/src/playback/transport.rs`
- Modify: `src-tauri/tests/playback_transport_integration.rs`

Make `start_server()` return `Arc<PreviewServer>` and `expect` loopback bind.
Read complete bodies by `Content-Length`, decode JPEG dimensions, and parse two
complete multipart frames. Add/rename exact tests:

- `frame_route_transitions_from_204_to_valid_200_jpeg`
- `frame_route_returns_complete_decodable_jpeg_body`
- `frame_route_rejects_cross_origin`
- `frame_route_returns_204_for_wrong_session_identity`
- `stream_route_delivers_two_distinct_complete_jpeg_parts`
- `stream_route_rejects_cross_origin`

Add the explicit CI command:

```bash
set -euo pipefail
cargo test -p opentake-tauri \
  --features playback-engine \
  --test playback_transport_integration \
  -- --test-threads=1
```

Run it locally with no skip/return, then stage and commit:

```bash
set -euo pipefail
git add -- \
  .github/workflows/ci.yml \
  src-tauri/src/playback/transport.rs \
  src-tauri/tests/playback_transport_integration.rs
git diff --cached --check
git commit -m "test(playback): require live transport integration evidence"
```

Set
`CATEGORY=02f-playback-live-transport`, run the mandatory full gate (test-only,
so no desktop artifact gate), and obtain explicit approval before Task 6.

### Task 6: Resolve Web Playback And UI Review Findings

**Interfaces:**
- Consumes: six approved Task 5 commits and the full session identity contract.
- Produces: three independently reviewed Web commits for explicit routing,
  retained-frame rendering, and project/media/UI coordination.

Every sub-slice follows Task 5's mandatory RED→GREEN→exact staging→pin/full
gate→desktop artifact (when user-visible)→independent review→re-review protocol.

#### Sub-slice 6.1: Pure Playback Route Contract

**Files:**
- Create: `web/src/components/preview/playbackRoute.ts`
- Create: `web/src/components/preview/playbackRoute.test.ts`
- Modify: `web/src/components/preview/rustEngine.ts`
- Modify: `web/src/components/preview/rustEngine.test.ts`

Create one pure `resolveTimelinePlaybackRoute` returning `rust`, `webkit`, or
`unsupported` with structured reasons. The capability matrix is exact:

- plain forward video/image/audio, reverse-only, and speed-only → WebKit;
- text, color grade, chroma key, and supported masks → Rust;
- composited content plus reverse/speed, Lottie, enabled generic effects,
  polygon masks, and mask count above four → Unsupported;
- Rust unavailable/disabled while WebKit lacks full parity → Unsupported, never
  silent WebKit fallback.

Test names:

- `routes plain forward video to WebKit`
- `routes reverse only and speed only timelines to WebKit`
- `routes text color chroma and supported masks to Rust`
- `returns Unsupported for text plus reverse instead of dropping text`
- `returns Unsupported for composited content plus speed`
- `returns Unsupported for Lottie enabled effects polygon masks and mask overflow`
- `does not select an incomplete renderer because of a runtime preference`
- `returns Unsupported when Rust is unavailable and WebKit lacks parity`

Run the two focused Vitest files and TypeScript, then stage and commit:

```bash
set -euo pipefail
pnpm -C web test \
  src/components/preview/playbackRoute.test.ts \
  src/components/preview/rustEngine.test.ts
pnpm -C web exec tsc -b --pretty false
git add -- \
  web/src/components/preview/playbackRoute.ts \
  web/src/components/preview/playbackRoute.test.ts \
  web/src/components/preview/rustEngine.ts \
  web/src/components/preview/rustEngine.test.ts
git diff --cached --check
git commit -m "fix(preview): model unsupported playback routes explicitly"
```

Set
`CATEGORY=03a-preview-route-contract`; run the pin/full gate (no desktop artifact
yet because this pure contract is not wired), and obtain explicit approval.

#### Sub-slice 6.2: Unsupported UI And True Retained-Frame Double Buffer

**Files:**
- Modify: `web/src/hooks/useKeyboardShortcuts.ts`
- Modify: `web/src/hooks/useKeyboardShortcuts.test.ts`
- Modify: `web/src/components/preview/Preview.tsx`
- Modify: `web/src/components/preview/Preview.test.tsx`
- Create: `web/src/components/preview/RustFrameBuffer.tsx`
- Modify: `web/src/components/preview/TimelinePlaybackLayer.tsx`
- Modify: `web/src/components/preview/previewEngine.ts`
- Modify: `web/src/components/preview/previewEngine.test.ts`
- Create: `web/src/components/preview/rustFrameBuffer.ts`
- Create: `web/src/components/preview/rustFrameBuffer.test.ts`
- Modify: `web/src/components/preview/timelinePlayback.ts`
- Modify: `web/src/components/preview/timelinePlayback.test.ts`
- Modify: `web/src/i18n/dict.ts`

Wire the 6.1 route through Play, Capture, Space, and the final preview-engine
guard. `UnsupportedPlaybackSurface` blocks incomplete playback with localized
reasons. Delete the superseded boolean route helpers from `timelinePlayback.ts`
so `playbackRoute.ts` remains the only capability authority. `Preview.tsx` must
consume the validated native state and must not register a second
`onPlaybackFrame` listener. Keep two stable `<img>` slots mounted: new identity/sequence loads only
the inactive slot; load promotes that same node; stale/out-of-order load is
ignored; error retains the last good node; terminal retries at most twice and
playback stops after terminal promotion. If both terminal retries fail, converge
explicitly: retain the last successful slot, stop the matching native transport,
set playing false, and show one localized error toast/surface; never wait
forever and never clear to black. Pause retains the active slot;
project epoch, timeline version, or session ID change clears both. After native
driving ends, hold the terminal slot until the matching composite still loads.

Required tests:

- `renders a user visible unsupported surface instead of incomplete DOM media`
- `disables play and capture for unsupported playback`
- `does not start timeline playback from Space when route is Unsupported`
- `loads the next frame into the inactive slot while keeping the active slot visible`
- `promotes the already loaded DOM slot without issuing a second URL`
- `ignores an out of order load replaced by a newer request`
- `keeps the last good slot visible when the pending frame errors`
- `does not end playback until the terminal frame is promoted`
- `retries a terminal load without clearing the active frame`
- `stops transport and retains the last good frame after terminal retries are exhausted`
- `clears both slots when project epoch timeline version or session id changes`
- `keeps two stable Rust frame image slots mounted`
- `holds the painted terminal frame after engineDriving becomes false`

Focused GREEN:

```bash
set -euo pipefail
pnpm -C web test \
  src/hooks/useKeyboardShortcuts.test.ts \
  src/components/preview/Preview.test.tsx \
  src/components/preview/previewEngine.test.ts \
  src/components/preview/rustFrameBuffer.test.ts \
  src/components/preview/timelinePlayback.test.ts \
  src/components/preview/playbackRoute.test.ts
pnpm -C web exec tsc -b --pretty false
cargo test -p opentake-tauri --features playback-engine --test playback_integration -- --nocapture
cargo test -p opentake-tauri --features playback-engine --test playback_transport_integration -- --test-threads=1
```

Stage and commit:

```bash
set -euo pipefail
git add -- \
  web/src/hooks/useKeyboardShortcuts.ts \
  web/src/hooks/useKeyboardShortcuts.test.ts \
  web/src/components/preview/Preview.tsx \
  web/src/components/preview/Preview.test.tsx \
  web/src/components/preview/RustFrameBuffer.tsx \
  web/src/components/preview/TimelinePlaybackLayer.tsx \
  web/src/components/preview/previewEngine.ts \
  web/src/components/preview/previewEngine.test.ts \
  web/src/components/preview/rustFrameBuffer.ts \
  web/src/components/preview/rustFrameBuffer.test.ts \
  web/src/components/preview/timelinePlayback.ts \
  web/src/components/preview/timelinePlayback.test.ts \
  web/src/i18n/dict.ts
git diff --cached --check
git commit -m "fix(preview): retain terminal frames and block incomplete playback"
```

Set
`CATEGORY=03b-preview-retained-frames`, and run the full + desktop artifact
gates. If the complete `.app` tree digest differs from the manifest's installed
bundle digest, create a
new dated QA root and launch this exact review bundle; with three isolated
scenario copies of `/Volumes/mac/未命名.opentake` plus external media inputs whose
bytes and modes are guarded before/after, verify visible
play/pause/resume/scrub, Unsupported
blocking, terminal-frame retention, and no
standalone mpv or lingering playback ffmpeg. Store hashes, screenshots, and
before/during/after process logs under the QA root. Create or reuse evidence
only through this hash gate:

```bash
set -euo pipefail
SAFETY='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence'
MANIFEST="$SAFETY/manifest.txt"
REVIEW='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-wave1a-review'
APP="$REVIEW/target/release/bundle/macos/OpenTake.app"
BIN="$APP/Contents/MacOS/opentake"
FRESH_BINARY_SHA=$(shasum -a 256 "$BIN" | awk '{print $1}')
TMP_BUNDLE=$(mktemp -d)
BUNDLE_DIGEST=$(zsh "$SAFETY/bundle-tree-digest.zsh" \
  "$APP" "$TMP_BUNDLE/app-bundle.manifest")
BASELINE_BUNDLE=$(sed -n 's/^# installed_bundle_sha256 //p' "$MANIFEST")
if test "$BUNDLE_DIGEST" != "$BASELINE_BUNDLE"; then
  SOURCE_PROJECT='/Volumes/mac/未命名.opentake'
  EXTRA_MEDIA='/Volumes/mac/OpenTake.mp4'
  RECEIPT_DIR="$SAFETY/smoke-roots"
  RECEIPT="$RECEIPT_DIR/$BUNDLE_DIGEST.txt"
  mkdir -p "$RECEIPT_DIR"
  if test -e "$RECEIPT"; then
    test "$(rg -c '^bundle_digest=[0-9a-f]{64}$' "$RECEIPT" || true)" -eq 1
    test "$(rg -c '^qa_root=/.+$' "$RECEIPT" || true)" -eq 1
    test "$(rg -c '^status=preflight-complete$' "$RECEIPT" || true)" -eq 1
    test "$(sed -n 's/^bundle_digest=//p' "$RECEIPT")" = "$BUNDLE_DIGEST"
    QA_ROOT=$(sed -n 's/^qa_root=//p' "$RECEIPT")
    test -d "$QA_ROOT"
    test -s "$QA_ROOT/preflight.complete"
    cmp "$TMP_BUNDLE/app-bundle.manifest" "$QA_ROOT/app-bundle.manifest"
  else
    STAMP=$(date '+%Y%m%d-%H%M%S')
    QA_ROOT="/Volumes/mac/OpenTake-QA/$STAMP/wave1a-fresh-bundle-smoke"
    test ! -e "$QA_ROOT"
    test -d "$SOURCE_PROJECT"
    test -f "$EXTRA_MEDIA"
    command -v jq >/dev/null
    mkdir -p "$QA_ROOT"
    ditto "$TMP_BUNDLE/app-bundle.manifest" "$QA_ROOT/app-bundle.manifest"
    printf '%s\n' "$BUNDLE_DIGEST" | tee "$QA_ROOT/app-bundle-sha256.txt"
    printf '%s\n' "$FRESH_BINARY_SHA" | tee "$QA_ROOT/binary-sha256.txt"
    {
      find "$SOURCE_PROJECT" -type f \
        -exec stat -f '# project_stat %z %Sp %N' {} + | LC_ALL=C sort
      find "$SOURCE_PROJECT" -type f \
        -exec shasum -a 256 {} + | LC_ALL=C sort
    } | tee "$QA_ROOT/source-project-before.txt"
    MEDIA_LIST="$QA_ROOT/external-media-inputs.txt"
    {
      jq -r '.entries[]?.source.external.absolutePath // empty' \
        "$SOURCE_PROJECT/media.json"
      printf '%s\n' "$EXTRA_MEDIA"
    } | LC_ALL=C sort -u | tee "$MEDIA_LIST"
    while IFS= read -r media; do
      test -f "$media"
      stat -f '# media_stat %z %Sp %N' "$media"
      shasum -a 256 "$media"
    done < "$MEDIA_LIST" | tee "$QA_ROOT/external-media-before.txt"
    for scenario in \
      "$QA_ROOT/plain-webkit.opentake" \
      "$QA_ROOT/rust-text-overlay.opentake" \
      "$QA_ROOT/unsupported-text-speed.opentake"; do
      ditto "$SOURCE_PROJECT" "$scenario"
      diff -qr "$SOURCE_PROJECT" "$scenario"
    done | tee "$QA_ROOT/initial-copy-diff.log"
    pgrep -lf 'OpenTake|opentake|mpv|ffmpeg' | \
      tee "$QA_ROOT/processes-before.log" || true
    printf 'bundle_digest=%s\nstatus=preflight-complete\n' "$BUNDLE_DIGEST" | \
      tee "$QA_ROOT/preflight.complete"
    RECEIPT_TMP="$RECEIPT.tmp.$$"
    (umask 022; printf 'bundle_digest=%s\nqa_root=%s\nstatus=preflight-complete\n' \
      "$BUNDLE_DIGEST" "$QA_ROOT" > "$RECEIPT_TMP")
    mv "$RECEIPT_TMP" "$RECEIPT"
  fi
  PLAIN_PROJECT="$QA_ROOT/plain-webkit.opentake"
  RUST_PROJECT="$QA_ROOT/rust-text-overlay.opentake"
  UNSUPPORTED_PROJECT="$QA_ROOT/unsupported-text-speed.opentake"
  test -d "$PLAIN_PROJECT"
  test -d "$RUST_PROJECT"
  test -d "$UNSUPPORTED_PROJECT"
  printf 'qa_root=%s\n' "$QA_ROOT"
  printf 'plain_project=%s\n' "$PLAIN_PROJECT"
  printf 'rust_project=%s\n' "$RUST_PROJECT"
  printf 'unsupported_project=%s\n' "$UNSUPPORTED_PROJECT"
  if test -s "$QA_ROOT/qa.complete"; then
    test "$(sed -n 's/^bundle_digest=//p' "$QA_ROOT/qa.complete")" = \
      "$BUNDLE_DIGEST"
    test -s "$QA_ROOT/results.md"
    test "$(find "$QA_ROOT/screenshots" -type f -name '*.png' | wc -l | tr -d ' ')" -ge 3
    printf 'reusing completed QA for bundle %s\n' "$BUNDLE_DIGEST"
  else
    printf 'starting or resuming QA for bundle %s\n' "$BUNDLE_DIGEST"
    open -n "$APP"
  fi
else
  printf 'byte-identical full app bundle: %s\n' "$BUNDLE_DIGEST"
fi
```

Open only the three printed copies; importing or autosaving must never touch the
source bundle. In `PLAIN_PROJECT`, verify the plain video/audio WebKit route,
play/pause/resume/scrub, and visible terminal frame using the project's actual
externally referenced A-roll. In `RUST_PROJECT`, use the UI to import
`/Volumes/mac/OpenTake.mp4`, add a short clip plus an overlapping text clip,
save only that copy, and verify the Rust route, composited text, retained-frame
transitions, and terminal promotion. In `UNSUPPORTED_PROJECT`, use the UI to
import and add the same `OpenTake.mp4`, add the same text overlay, and set that
overlapping imported clip to reverse or speed 1.25; verify the
localized Unsupported surface and that Play, Capture, and Space are blocked.
Capture at least one PNG per scenario under `$QA_ROOT/screenshots/` and write
the exact route/result observations to `$QA_ROOT/results.md` with `apply_patch`.
Quit the review app after UI actions, then
use the printed `QA_ROOT` to capture during/after process logs. Recompute the
source-project mode/size/hash list into `source-project-after.txt`; recompute every
`MEDIA_LIST` stat/hash line into `external-media-after.txt`; `cmp` both against
their corresponding before files. Write the exact commands/results with
`apply_patch`. Any source project or external media mode/size/hash change fails
the smoke. Obtain explicit approval.

The after-check is self-contained and recovers the exact QA root from the
immutable full-bundle-digest receipt:

```bash
set -euo pipefail
SAFETY='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence'
REVIEW='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-wave1a-review'
APP="$REVIEW/target/release/bundle/macos/OpenTake.app"
TMP_BUNDLE=$(mktemp -d)
BUNDLE_DIGEST=$(zsh "$SAFETY/bundle-tree-digest.zsh" \
  "$APP" "$TMP_BUNDLE/app-bundle.manifest")
RECEIPT="$SAFETY/smoke-roots/$BUNDLE_DIGEST.txt"
test -s "$RECEIPT"
test "$(sed -n 's/^bundle_digest=//p' "$RECEIPT")" = "$BUNDLE_DIGEST"
QA_ROOT=$(sed -n 's/^qa_root=//p' "$RECEIPT")
SOURCE_PROJECT='/Volumes/mac/未命名.opentake'
MEDIA_LIST="$QA_ROOT/external-media-inputs.txt"
test -d "$QA_ROOT"
cmp "$TMP_BUNDLE/app-bundle.manifest" "$QA_ROOT/app-bundle.manifest"
test -s "$QA_ROOT/results.md"
test "$(find "$QA_ROOT/screenshots" -type f -name '*.png' | wc -l | tr -d ' ')" -ge 3
{
  find "$SOURCE_PROJECT" -type f \
    -exec stat -f '# project_stat %z %Sp %N' {} + | LC_ALL=C sort
  find "$SOURCE_PROJECT" -type f \
    -exec shasum -a 256 {} + | LC_ALL=C sort
} | tee "$QA_ROOT/source-project-after.txt"
cmp "$QA_ROOT/source-project-before.txt" "$QA_ROOT/source-project-after.txt"
while IFS= read -r media; do
  test -f "$media"
  stat -f '# media_stat %z %Sp %N' "$media"
  shasum -a 256 "$media"
done < "$MEDIA_LIST" | tee "$QA_ROOT/external-media-after.txt"
cmp "$QA_ROOT/external-media-before.txt" "$QA_ROOT/external-media-after.txt"
pgrep -lf 'OpenTake|opentake|mpv|ffmpeg' | tee "$QA_ROOT/processes-after.log" || true
if pgrep -lf '(^|/)(mpv|ffmpeg)( |$)' | tee "$QA_ROOT/lingering-media-processes.log"; then
  exit 1
fi
QA_MARKER="$QA_ROOT/qa.complete"
if test -e "$QA_MARKER"; then
  test "$(sed -n 's/^bundle_digest=//p' "$QA_MARKER")" = "$BUNDLE_DIGEST"
else
  QA_MARKER_TMP="$QA_MARKER.tmp.$$"
  printf 'bundle_digest=%s\nstatus=complete\n' "$BUNDLE_DIGEST" > "$QA_MARKER_TMP"
  mv "$QA_MARKER_TMP" "$QA_MARKER"
fi
```

#### Sub-slice 6.3: Project, Media, Timeline, And Accessibility Coordination

**Files:**
- Modify: `web/src/components/home/HomeView.tsx`
- Modify: `web/src/components/home/HomeView.visual.test.ts`
- Modify: `web/src/components/media/MediaPanel.tsx`
- Modify: `web/src/components/media/MediaSearch.tsx`
- Modify: `web/src/components/shell/TitleBar.visual.test.ts`
- Modify: `web/src/components/shell/ViewMenu.tsx`
- Modify: `web/src/components/timeline/TimelineContainer.tsx`
- Modify: `web/src/components/timeline/TimelineContainer.test.ts`
- Modify: `web/src/components/ui/PanelShell.tsx`
- Modify: `web/src/lib/api.ts`
- Modify: `web/src/store/editActions.ts`
- Modify: `web/src/store/editActions.test.ts`
- Modify: `web/src/store/mediaActions.ts`
- Create: `web/src/store/mediaActions.test.ts`
- Modify: `web/src/store/uiStore.ts`
- Modify: `web/src/styles/global.css`

Preserve the restored project-bound runtime reset, preload result handling,
timeline gesture/accessibility behavior, opaque body after libmpv removal, and
menu/UI state changes. Do not add a second project or timeline authority.

Run the focused Home/TitleBar/TimelineContainer/editActions/mediaActions tests
plus TypeScript. Stage and commit:

```bash
set -euo pipefail
pnpm -C web test \
  src/components/home/HomeView.visual.test.ts \
  src/components/shell/TitleBar.visual.test.ts \
  src/components/timeline/TimelineContainer.test.ts \
  src/store/editActions.test.ts \
  src/store/mediaActions.test.ts
pnpm -C web exec tsc -b --pretty false
git add -- \
  web/src/components/home/HomeView.tsx \
  web/src/components/home/HomeView.visual.test.ts \
  web/src/components/media/MediaPanel.tsx \
  web/src/components/media/MediaSearch.tsx \
  web/src/components/shell/TitleBar.visual.test.ts \
  web/src/components/shell/ViewMenu.tsx \
  web/src/components/timeline/TimelineContainer.tsx \
  web/src/components/timeline/TimelineContainer.test.ts \
  web/src/components/ui/PanelShell.tsx \
  web/src/lib/api.ts \
  web/src/store/editActions.ts \
  web/src/store/editActions.test.ts \
  web/src/store/mediaActions.ts \
  web/src/store/mediaActions.test.ts \
  web/src/store/uiStore.ts \
  web/src/styles/global.css
git diff --cached --check
git commit -m "fix(app): coordinate project state and media prewarm"
```

Set
`CATEGORY=03c-web-project-media-ui`, run full + desktop artifact gates, and
obtain explicit approval after any required fix/re-review cycle.

### Task 7: Commit Historical Evidence, Then Baseline Disposition

**Files:**
- Modify: `README.md`
- Modify: `docs/architecture/HANDOFF-2026-07.md`
- Modify: `docs/architecture/PLAYBACK-ENGINE.md`
- Modify: `docs/superpowers/archive/2026-07-08-verification-report.md`
- Create: `docs/superpowers/archive/2026-07-10-playback-cache-installed-app-qa.md`
- Create in a second meta-only commit:
  `docs/superpowers/archive/2026-07-10-wave1a-baseline-disposition.md`

**Interfaces:**
- Consumes: approved runtime, Rust, and Web slices plus their immutable logs.
- Produces: first, reviewed dated architecture/QA evidence; second, a separately
  reviewed hunk-level disposition ledger for the immutable 56-path baseline.
  The ledger classifies its parent delivery tree and explicitly exempts only its
  own metadata hunk, avoiding self-reference.

- [ ] **Step 1: Run all restored and reviewer-added tests before documentation**

Run from the clean integration worktree:

```bash
set -euo pipefail
SAFETY='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence'
LOG="$SAFETY/logs/04a-tests-evidence-integration"
test ! -e "$LOG"
mkdir -p "$LOG"
pnpm -C web test \
  src/components/home/HomeView.visual.test.ts \
  src/components/shell/TitleBar.visual.test.ts \
  src/components/timeline/TimelineContainer.test.ts \
  src/store/editActions.test.ts \
  src/store/mediaActions.test.ts 2>&1 | tee "$LOG/focused-web.log"
pnpm -C web test 2>&1 | tee "$LOG/web-test.log"
cargo test -p opentake-tauri --features playback-engine --test playback_integration -- --nocapture \
  2>&1 | tee "$LOG/playback-integration.log"
cargo test -p opentake-tauri --features playback-engine --test playback_transport_integration -- --test-threads=1 \
  2>&1 | tee "$LOG/playback-transport.log"
cargo test --workspace -- --nocapture 2>&1 | tee "$LOG/cargo-test.log"
test -z "$(git status --porcelain)"
```

Expected: every named and workspace assertion executes and passes; no
playback/media test skips or returns early.

- [ ] **Step 2: Implement and verify only dated evidence documents**

Use `apply_patch`, the safety snapshot, gate logs, reviewer reports, and fresh
smoke evidence to update the five non-ledger files. Do not create the disposition
ledger yet. Then run:

```bash
set -euo pipefail
test ! -e docs/superpowers/archive/2026-07-10-wave1a-baseline-disposition.md
rg -n 'libmpv|WebKit|PublicationGate|tail|prewarm|verified|not yet complete' \
  README.md \
  docs/architecture/HANDOFF-2026-07.md \
  docs/architecture/PLAYBACK-ENGINE.md \
  docs/superpowers/archive/2026-07-08-verification-report.md \
  docs/superpowers/archive/2026-07-10-playback-cache-installed-app-qa.md
git diff --check
```

Expected: historical evidence is dated, unsupported capabilities are not
called verified, and fresh-bundle results are not conflated with the older
installed binary unless their hashes match.

- [ ] **Step 3: Commit the evidence documents without the ledger**

```bash
set -euo pipefail
git add -- \
  README.md \
  docs/architecture/HANDOFF-2026-07.md \
  docs/architecture/PLAYBACK-ENGINE.md \
  docs/superpowers/archive/2026-07-08-verification-report.md \
  docs/superpowers/archive/2026-07-10-playback-cache-installed-app-qa.md
git diff --cached --check
git diff --cached --name-status
git commit -m "docs: record the reviewed playback recovery evidence"
```

- [ ] **Step 4: Gate and independently review the evidence commit**

```bash
set -euo pipefail
SAFETY='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence'
INTEGRATION='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-full-convergence'
REVIEW='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-wave1a-review'
zsh "$SAFETY/pin-and-gate-slice.zsh" \
  "$INTEGRATION" "$REVIEW" '04a-reviewed-evidence' 'initial' 'skip'
```

The reviewer compares every claim with source commits, immutable logs, binary
hashes, external-drive smoke, and the dated QA record. Persist and verify its
report. Required corrections use `fix(review): correct playback evidence`, a
new attempt, and independent re-review. Do not build the ledger until the latest
evidence report says `verdict: APPROVE`.

- [ ] **Step 5: Build the ledger against the approved delivery parent**

Start clean. The approved evidence SHA becomes `DELIVERY_HEAD`; the ledger path
is the sole explicit meta-artifact exemption:

```bash
set -euo pipefail
SAFETY='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence'
MANIFEST="$SAFETY/manifest.txt"
LEDGER='docs/superpowers/archive/2026-07-10-wave1a-baseline-disposition.md'
SOURCE_HEAD=$(sed -n 's/^# source_head //p' "$MANIFEST")
RESTORED_COMMIT=$(sed -n 's/^# restored_commit //p' "$MANIFEST")
DELIVERY_HEAD=$(git rev-parse HEAD)
test -z "$(git status --porcelain)"
test ! -e "$LEDGER"
git diff --binary "$SOURCE_HEAD" "$RESTORED_COMMIT" -- \
  > /tmp/opentake-baseline-source-to-safety.patch
git diff --binary "$SOURCE_HEAD" "$DELIVERY_HEAD" -- \
  . ":(exclude)$LEDGER" \
  > /tmp/opentake-baseline-source-to-delivery.patch
git diff --binary "$RESTORED_COMMIT" "$DELIVERY_HEAD" -- \
  . ":(exclude)$LEDGER" \
  > /tmp/opentake-baseline-safety-to-delivery.patch
git diff --name-status "$SOURCE_HEAD" "$RESTORED_COMMIT"
git diff --name-status "$SOURCE_HEAD" "$DELIVERY_HEAD" -- \
  . ":(exclude)$LEDGER"
```

Create the ledger with `apply_patch`. Record `DELIVERY_HEAD` exactly and state
that the ledger's own hunk is audit metadata outside the classified delivery
set. Give every baseline hunk (not merely every path) a stable ID, owning
slice/commit, and exactly one disposition: `delivered-exact`,
`superseded-by-reviewed-fix`, or `historical-evidence-only`. For a superseded
hunk, name the replacement test and exact `reviewer-report.md`. No generic
"covered by rewrite" row is allowed. Record every non-ledger integration-only
hunk as a reviewer-approved addition. A missing, duplicated, premature, or
unclassified hunk blocks the ledger commit.

- [ ] **Step 6: Commit only the meta ledger**

```bash
set -euo pipefail
LEDGER='docs/superpowers/archive/2026-07-10-wave1a-baseline-disposition.md'
test "$(git diff --name-only | wc -l | tr -d ' ')" -eq 1
test "$(git diff --name-only)" = "$LEDGER"
git add -- "$LEDGER"
git diff --cached --check
git diff --cached --name-status
git commit -m "docs: map Wave 1A baseline hunk disposition"
test "$(git diff --name-only HEAD^ HEAD)" = "$LEDGER"
```

- [ ] **Step 7: Gate and independently review the ledger commit**

```bash
set -euo pipefail
SAFETY='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence'
INTEGRATION='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-full-convergence'
REVIEW='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-wave1a-review'
zsh "$SAFETY/pin-and-gate-slice.zsh" \
  "$INTEGRATION" "$REVIEW" '04b-baseline-disposition' 'initial' 'skip'
```

The reviewer verifies that the ledger's recorded `DELIVERY_HEAD` equals the
ledger commit's parent and that every classified hunk points to the exact owning
commit, test, gate, and persisted approval report. Ledger-only corrections use
a new review-fix commit/attempt and record the immediate pre-fix HEAD as the new
`delivery head` while continuing to exclude the ledger path. A code or evidence
defect returns to the owning slice, then requires a new evidence approval and regenerated ledger. Task 8
cannot start until the latest ledger report says `verdict: APPROVE`.

### Task 8: Prove The Restored Branch Is Complete And Clean

**Files:**
- Inspect all restored files and commits.
- Update no source file unless verification finds a defect.

**Interfaces:**
- Consumes: reviewed commits from Tasks 4-7 and the immutable Task 3 safety ref.
- Produces: a clean integration baseline where every immutable recovery hunk is
  delivered or explicitly superseded, and every integration-only hunk has an
  independent approval trail.

- [ ] **Step 1: Prove no restored dirty path remains**

Run:

```bash
set -euo pipefail
git status --short --branch
git diff --check
```

Expected: clean status on
`integration/opentake-full-convergence-20260710`.

- [ ] **Step 2: Prove canonical bytes match the immutable snapshot and capture all deltas**

Run from the integration worktree:

```bash
set -euo pipefail
CANON='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake'
SAFETY='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence'
MANIFEST="$SAFETY/manifest.txt"
ATTEMPT="${FINAL_ATTEMPT:-initial}"
SAFETY_REF='refs/heads/safety/opentake-wave1a-restored-20260710'
SOURCE_HEAD=$(sed -n 's/^# source_head //p' "$MANIFEST")
RESTORED_COMMIT=$(sed -n 's/^# restored_commit //p' "$MANIFEST")
RESTORED_TREE=$(sed -n 's/^# restored_tree //p' "$MANIFEST")
test "$(git rev-parse "$SAFETY_REF")" = "$RESTORED_COMMIT"
test "$(git show -s --format='%T' "$RESTORED_COMMIT")" = "$RESTORED_TREE"
cd "$CANON"
shasum -a 256 -c "$MANIFEST"
cmp \
  <(git ls-files --others --exclude-standard | LC_ALL=C sort) \
  <(printf '%s\n' \
      'docs/superpowers/archive/2026-07-10-playback-cache-installed-app-qa.md' \
      'web/src/components/preview/nativePlaybackSession.ts' \
      'web/src/components/preview/nativePlaybackSession.test.ts' \
      'web/src/store/mediaActions.test.ts' | LC_ALL=C sort)
cmp \
  <(sed -n '/^# tracked_/p' "$MANIFEST") \
  <(git diff HEAD --name-only -z | while IFS= read -r -d '' file; do
      if test -e "$file"; then
        stat -f '# tracked_stat %z %Sp %N' "$file"
      else
        printf '# tracked_deleted %s\n' "$file"
      fi
    done)
cmp \
  <(sed -n '/^# untracked_stat /p' "$MANIFEST") \
  <(for file in \
      'docs/superpowers/archive/2026-07-10-playback-cache-installed-app-qa.md' \
      'web/src/components/preview/nativePlaybackSession.ts' \
      'web/src/components/preview/nativePlaybackSession.test.ts' \
      'web/src/store/mediaActions.test.ts'; do
      stat -f '# untracked_stat %z %Sp %N' "$file"
    done)
test "$(shasum -a 256 /Applications/OpenTake.app/Contents/MacOS/opentake | awk '{print $1}')" = \
  "$(sed -n 's/^# installed_binary_sha256 //p' "$MANIFEST")"
test "$(shasum -a 256 "$CANON/target/release/bundle/macos/OpenTake.app/Contents/MacOS/opentake" | awk '{print $1}')" = \
  "$(sed -n 's/^# release_binary_sha256 //p' "$MANIFEST")"
TMP_BUNDLE=$(mktemp -d)
INSTALLED_BUNDLE_NOW=$(zsh "$SAFETY/bundle-tree-digest.zsh" \
  /Applications/OpenTake.app "$TMP_BUNDLE/installed-app-bundle.manifest")
RELEASE_BUNDLE_NOW=$(zsh "$SAFETY/bundle-tree-digest.zsh" \
  "$CANON/target/release/bundle/macos/OpenTake.app" \
  "$TMP_BUNDLE/release-app-bundle.manifest")
test "$INSTALLED_BUNDLE_NOW" = \
  "$(sed -n 's/^# installed_bundle_sha256 //p' "$MANIFEST")"
test "$RELEASE_BUNDLE_NOW" = \
  "$(sed -n 's/^# release_bundle_sha256 //p' "$MANIFEST")"
cmp "$SAFETY/installed-app-bundle.manifest" \
  "$TMP_BUNDLE/installed-app-bundle.manifest"
cmp "$SAFETY/release-app-bundle.manifest" \
  "$TMP_BUNDLE/release-app-bundle.manifest"
git diff HEAD --name-only -z | while IFS= read -r -d '' file; do
  if test -e "$CANON/$file"; then
    git cat-file -e "$RESTORED_COMMIT:$file"
    cmp "$CANON/$file" <(git cat-file blob "$RESTORED_COMMIT:$file")
  else
    if git cat-file -e "$RESTORED_COMMIT:$file" 2>/dev/null; then
      printf 'expected deleted path in snapshot: %s\n' "$file" >&2
      exit 1
    fi
  fi
done
for file in \
  'docs/superpowers/archive/2026-07-10-playback-cache-installed-app-qa.md' \
  'web/src/components/preview/nativePlaybackSession.ts' \
  'web/src/components/preview/nativePlaybackSession.test.ts' \
  'web/src/store/mediaActions.test.ts'; do
  cmp "$CANON/$file" <(git cat-file blob "$RESTORED_COMMIT:$file")
done
INTEGRATION='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-full-convergence'
LEDGER="$INTEGRATION/docs/superpowers/archive/2026-07-10-wave1a-baseline-disposition.md"
test -s "$LEDGER"
FINAL_SHA=$(git -C "$INTEGRATION" rev-parse HEAD)
SHORT=$(git -C "$INTEGRATION" rev-parse --short=12 HEAD)
DELIVERY_HEAD=$(git -C "$INTEGRATION" rev-parse "$FINAL_SHA^")
test "$(sed -n 's/^delivery head: //p' "$LEDGER")" = "$DELIVERY_HEAD"
LOG="$SAFETY/logs/05-final-baseline-$SHORT-$ATTEMPT"
test ! -e "$LOG"
mkdir -p "$LOG"
git diff --binary "$SOURCE_HEAD" "$RESTORED_COMMIT" -- \
  > "$LOG/source-to-safety.patch"
git -C "$INTEGRATION" diff --binary "$SOURCE_HEAD" HEAD -- \
  > "$LOG/source-to-final.patch"
git -C "$INTEGRATION" diff --binary "$RESTORED_COMMIT" HEAD -- \
  > "$LOG/safety-to-final.patch"
git -C "$INTEGRATION" diff --binary "$SOURCE_HEAD" "$DELIVERY_HEAD" -- \
  . ':(exclude)docs/superpowers/archive/2026-07-10-wave1a-baseline-disposition.md' \
  > "$LOG/source-to-classified-delivery.patch"
git -C "$INTEGRATION" diff --binary "$RESTORED_COMMIT" "$DELIVERY_HEAD" -- \
  . ':(exclude)docs/superpowers/archive/2026-07-10-wave1a-baseline-disposition.md' \
  > "$LOG/safety-to-classified-delivery.patch"
git -C "$INTEGRATION" diff --name-status "$SOURCE_HEAD" "$RESTORED_COMMIT" | \
  tee "$LOG/source-to-safety-name-status.log"
git -C "$INTEGRATION" diff --name-status "$SOURCE_HEAD" HEAD | \
  tee "$LOG/source-to-final-name-status.log"
git -C "$INTEGRATION" diff --name-status "$RESTORED_COMMIT" HEAD | \
  tee "$LOG/safety-to-final-name-status.log"
printf '%s\n' "$DELIVERY_HEAD" | tee "$LOG/classified-delivery-head.txt"
```

Expected: canonical bytes, modes, sizes, deletions, archives, source commit, and
safety ref all match the recorded restored snapshot. The three complete binary
diffs, the two ledger-excluded delivery diffs, and path lists are immutable
final-review inputs under the exact SHA/attempt log directory. They need not be
empty or byte-identical: the disposition ledger must classify every non-ledger
baseline/delivery hunk with its owning reviewed commit and evidence, and its
recorded delivery head must equal the ledger commit's parent.

- [ ] **Step 3: Run the branch-wide automated gate**

Pin the detached review worktree to the final exact integration SHA, then reuse
the immutable logging gate and rerun no-skip playback integration:

```bash
set -euo pipefail
INTEGRATION='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-full-convergence'
REVIEW='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-wave1a-review'
SAFETY='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence'
ATTEMPT="${FINAL_ATTEMPT:-initial}"
FINAL_SHA=$(git -C "$INTEGRATION" rev-parse HEAD)
test -z "$(git -C "$INTEGRATION" status --porcelain)"
test -z "$(git -C "$REVIEW" status --porcelain)"
git -C "$REVIEW" switch --detach "$FINAL_SHA"
SHORT=$(git -C "$REVIEW" rev-parse --short=12 HEAD)
SLICE="05-final-$SHORT-$ATTEMPT"
zsh "$SAFETY/run-code-slice-gate.zsh" "$REVIEW" "$SLICE"
LOG="$SAFETY/logs/$SLICE"
cd "$REVIEW"
cargo test -p opentake-tauri --features playback-engine --test playback_integration -- --nocapture \
  2>&1 | tee "$LOG/playback-integration.log"
cargo test -p opentake-tauri --features playback-engine --test playback_transport_integration -- --test-threads=1 --nocapture \
  2>&1 | tee "$LOG/playback-transport.log"
test -z "$(git status --porcelain)"
test -z "$(git -C "$INTEGRATION" status --porcelain)"
```

Expected: every command exits zero with immutable logs. Every playback/media
assertion executes; a skip or early return keeps the gate incomplete. Both
worktrees remain clean.

- [ ] **Step 4: Verify the release bundle can be reproduced**

Run from the final detached review worktree, using the Step 3 log directory:

```bash
set -euo pipefail
SAFETY='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260710-full-convergence'
MANIFEST="$SAFETY/manifest.txt"
ATTEMPT="${FINAL_ATTEMPT:-initial}"
SHORT=$(git rev-parse --short=12 HEAD)
SLICE="05-final-$SHORT-$ATTEMPT"
LOG="$SAFETY/logs/$SLICE"
test ! -e "$LOG/final-tauri-build.log"
./web/node_modules/.bin/tauri build --bundles app --no-sign \
  2>&1 | tee "$LOG/final-tauri-build.log"
APP='target/release/bundle/macos/OpenTake.app'
BIN="$APP/Contents/MacOS/opentake"
FRESH_BINARY_SHA=$(shasum -a 256 "$BIN" | awk '{print $1}')
BASELINE_BINARY_SHA=$(sed -n 's/^# installed_binary_sha256 //p' "$MANIFEST")
FRESH_BUNDLE_SHA=$(zsh "$SAFETY/bundle-tree-digest.zsh" \
  "$APP" "$LOG/final-app-bundle.manifest")
BASELINE_BUNDLE_SHA=$(sed -n 's/^# installed_bundle_sha256 //p' "$MANIFEST")
printf 'fresh_binary=%s\nbaseline_binary=%s\nfresh_bundle=%s\nbaseline_bundle=%s\n' \
  "$FRESH_BINARY_SHA" "$BASELINE_BINARY_SHA" \
  "$FRESH_BUNDLE_SHA" "$BASELINE_BUNDLE_SHA" | \
  tee "$LOG/final-artifact-hashes.log"
codesign -dv --verbose=4 "$APP" 2>&1 | tee "$LOG/final-codesign.log"
if otool -L "$BIN" | rg -i 'libmpv'; then exit 1; fi
test -z "$(git status --porcelain)"
```

Expected: build and structural checks exit zero. If `FRESH_BUNDLE_SHA` differs
from `BASELINE_BUNDLE_SHA`, launch this exact fresh bundle without replacing the
installed app and repeat Task 6's receipt-aware external-drive
play/pause/resume/scrub/tail-frame and process-leak smoke. Store new evidence under
`/Volumes/mac/OpenTake-QA/$STAMP/wave1a-final-bundle-smoke/`, where `STAMP` is
created with `date '+%Y%m%d-%H%M%S'` and the directory is required not to exist.
Reuse earlier evidence only when its recorded full bundle tree digest exactly
equals `FRESH_BUNDLE_SHA` and its `qa.complete` marker plus before/after inputs
revalidate.

- [ ] **Step 5: Final independent baseline review**

Dispatch a fresh reviewer over the recorded reviewed-plan commit through HEAD,
all immutable logs, Task 1 manifest, safety ref/tree, fresh-bundle evidence, and
both clean statuses. It must prove:

- no dirty source byte was lost from the immutable restored snapshot;
- every source-to-safety hunk is classified exactly once as delivered,
  superseded by a named reviewed fix, or historical-only;
- every final-only hunk and every safety-to-final difference points to its
  owning commit, test, gate log, and reviewer approval;
- commits are coherent and do not include unrelated user changes;
- restored tests actually cover the publication/cache behavior;
- documentation distinguishes historical evidence from current verified state.

For any finding, do not leave the ledger stale. Fix code/evidence in a dedicated
commit and rerun affected targeted tests. Then regenerate the ledger with
`apply_patch` against that fix SHA while excluding the ledger path, record that
SHA as `delivery head: ...`, and commit only the refreshed ledger. A ledger-only
finding uses the prior HEAD as the newly recorded delivery head and changes only
the ledger. Set `FINAL_ATTEMPT=review-fix-N`, rerun Steps 1–4 so both delta and
gate log directories include the new SHA/attempt, and repeat fresh-bundle smoke
when the hash changes. Persist a fresh independent review report for the new
exact ledger SHA. Repeat until it explicitly states `verdict: APPROVE`; no final
fix may occur after the last delta capture, ledger refresh, gate, and approval.

- [ ] **Step 6: Record the handoff to Wave 1B**

Update the task plan status and retain the safety archive. Do not delete or
clean the canonical worktree. Wave 1B starts from the clean reviewed integration
branch and addresses project save safety, unknown-field preservation, Agent
undo ownership, `move_clips` validation, strict CSP/asset scope, and bundled
FFmpeg/app-distribution safety with new failing tests. Project-scoped playback/
session defects found during Wave 1A must already be closed.
