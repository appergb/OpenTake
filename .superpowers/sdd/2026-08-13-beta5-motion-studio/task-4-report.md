# Motion Studio Task 4 report

Status: COMPLETE — independently reviewed and approved before commit.

## Scope

- Added `motion` as a persisted primary application view. The store migrates
  the legacy key, restores supported primary views, and discards invalid or
  modal-only values to a safe Home fallback.
- Added a four-control primary title-bar navigation group in the required
  order: Home, Chat, Motion Studio, Panel Management. Every control retains the
  existing 26-by-26-pixel title-bar geometry and has a localized accessible
  label; the active primary destination is exposed semantically.
- Chat navigation reopens the existing editor and Agent panel without toggling
  an already-open panel off. The App keeps every visited primary view mounted
  but exposes and lays out exactly one, so Chat/editor and Motion Studio local
  state survive round trips.
- Added the first independent Motion Studio shell with semantic landmarks for
  files, HTML/CSS editor, 16:9 preview, inspector, and keyframe timeline. The
  shell uses the existing dark design tokens and contains visible starter
  content rather than an empty placeholder.
- Added complete Simplified Chinese and English navigation and workspace copy.

## TDD evidence

Initial RED:

```text
MotionStudio module was absent.
"motion" was not assignable to AppView or persisted by uiStore.
The title bar exposed no Motion Studio entry or required four-control order.
App could not mount an independent Motion Studio view.
5 target failures / missing suite were observed.
```

During the first full regression run, two compatibility tests exposed stale
assumptions: one still expected every restart to return Home after selecting
the editor, and the new shell referenced an undefined `--border-secondary`
token. The persistence assertion now reflects the intentional primary-view
contract and the shell uses the existing `--border-subtle` token.

Final fresh GREEN:

```text
pnpm -C web exec vitest run \
  src/store/uiStore.persistence.test.ts src/styles/tokenUsage.test.ts \
  src/store/uiStore.test.ts src/components/shell/TitleBar.interaction.test.tsx \
  src/components/motion/MotionStudio.test.tsx src/App.lifecycle.test.tsx
6 files / 43 tests passed

pnpm -C web test
145 files / 1323 tests passed

pnpm -C web build
passed (only the existing dynamic-import and large-chunk warnings)

git diff --check
passed
```

## Review

The first independent review found two release-blocking semantic issues. The
preview was a named `<figure>` but not an ARIA landmark, and the idempotent Chat
navigation exposed `aria-pressed` toggle semantics. Two focused regressions
were added and witnessed failing; the preview is now a named `region`, while
Chat uses the same `aria-current="page"` navigation state as the other primary
destinations.

Final independent re-review verdict: **Spec PASS / Quality APPROVE**, with zero
remaining findings. It also checked the real 760px Tauri minimum against the
708px minimum Motion Studio grid, primary-view lifecycle/state retention,
focus-visible and reduced-motion behavior, persistence migration/fallback, and
the full Chinese/English key set. Its fresh focused suite passed 42/42 and its
scoped diff check passed.

## Commit

Pending: `feat(motion): add Motion Studio as a primary view`.
