# Task 2 Report — Preview Temporal Remap Parity

## Scope

- Task: restore Preview controls for the native temporal-remap route after Task 1 routed compositor temporal timelines to Rust.
- Baseline commit: `ad0bd70`
- Task 1 dependency acknowledged:
  - route work landed in `a833764`
  - follow-up fix landed in `1013355`
- Modified files:
  - `web/src/components/preview/Preview.tsx`
  - `web/src/components/preview/Preview.test.tsx`
  - `web/src/components/preview/previewEngine.test.ts`

## Constraints Followed

- Only changed the brief-listed Preview files plus this required task report.
- Did not modify Rust, upstream Swift, docs, or user audit files.
- Did not dispatch subagents or reviewers.
- Added/updated tests before touching production code.
- Reused the existing native playback surface, controller identity, clock, and capture flow.

## Root Cause

Task 1 already changed the route contract so temporal compositor timelines no longer return `unsupported` when Rust playback is available.

That left two Preview-side gaps:

- stale UI tests still treated `reversed` compositor timelines as unsupported
- the native timeline surface had no explicit `data-playback-surface="native"` marker, so the Preview contract could not assert the Rust surface directly

No Rust/engine production route change was needed for temporal remap playback itself.

## Red Phase

Added temporal compositor coverage with a `text` clip using:

- `reversed: true`
- `speed: 1.5`

Focused command run before implementation:

```bash
NODE_OPTIONS=--localstorage-file=/tmp/opentake-vitest-localstorage-preview-remap-ui.json pnpm exec vitest run src/components/preview/Preview.test.tsx src/components/preview/previewEngine.test.ts
```

Observed failures before implementation:

- old unsupported tests failed because the current route now correctly renders the Rust surface for temporal compositor timelines
- the new native-surface assertion failed because Preview did not emit `data-playback-surface="native"` on the timeline Rust path

This confirmed the engine route was already correct, while the Preview test contract and one surface marker were outdated/incomplete.

## Green Phase

Production change in `Preview.tsx`:

- wrapped the timeline `RustFrameBuffer` in a minimal container that emits `data-playback-surface="native"` only when `playbackRoute.kind === "rust"`

Test updates:

- added a shared temporal compositor fixture in `Preview.test.tsx`
- asserted that Rust-capable Preview shows:
  - no `unsupported-playback-surface`
  - `data-playback-surface="native"`
  - enabled play/capture controls
- asserted that the same temporal compositor timeline still shows typed unsupported UI when Rust capability is unavailable
- updated the old generic unsupported Preview cases to use a genuinely unsupported `lottie` clip instead of the now-supported temporal compositor case
- updated `previewEngine.test.ts` so the temporal compositor route now expects:
  - native playback start when capability resolves
  - transport stop without native start when capability is unavailable

## Verification

Focused Preview/engine tests after the change:

```bash
NODE_OPTIONS=--localstorage-file=/tmp/opentake-vitest-localstorage-preview-remap-ui.json pnpm exec vitest run src/components/preview/Preview.test.tsx src/components/preview/previewEngine.test.ts
```

Result:

- `2` test files passed
- `44` tests passed
- exit code `0`

Build check:

```bash
pnpm build
```

Result:

- `tsc -b && vite build` passed
- Vite emitted pre-existing bundle-size / ineffective-dynamic-import warnings only
- exit code `0`

## Commit

Commit message:

```text
feat(preview): enable temporal compositor controls
```

## Notes / Concerns

- The worktree `.git` file still points at a stale absolute path under `/Users/lvbaiqing/...`, so plain `git` fails with `fatal: not a git repository: (null)`.
- Git operations for this task must use explicit:

```bash
GIT_DIR='/Users/trip/TRUE 开发/PRIMARY-CN/OpenTake/.git/worktrees/OpenTake-generation'
GIT_COMMON_DIR='/Users/trip/TRUE 开发/PRIMARY-CN/OpenTake/.git'
GIT_WORK_TREE='/Users/trip/TRUE 开发/PRIMARY-CN/OpenTake-generation'
```

- The repository contains many unrelated user changes outside this task; they were left untouched and excluded from the staged commit.
