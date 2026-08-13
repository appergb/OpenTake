# Task 2 implementer report

Status: DONE

## Scope

- Removed the persisted `Theme` setting, actions, startup initializer, Appearance theme control, and obsolete `Dropdown` theme reference.
- Startup now removes legacy and versioned theme keys, and retains the compatibility `data-theme="dark"` marker.
- Replaced Appearance with the two requested dark-layout radio cells: `深色 · 标准` and `深色 · 紧凑`. They are equal-width, contain no selected-state icon, and preserve label geometry.
- Made window resize transactional: state updates for immediate feedback, persistence commits after native success, failures restore the previous selection and show a toast.
- Serialized startup and user native resize requests. A stale request cannot overwrite a later selection; a post-size positioning failure restores the original native size and position.
- Preserved unrelated audit and external-MCP worktree changes.

## TDD evidence

### RED

Command:

```text
pnpm -C web test -- src/store/settingsStore.test.ts src/components/settings/SettingsView.interaction.test.tsx src/components/settings/SettingsView.visual.test.ts
```

Observed result: exit 1. The legacy `theme` value remained, `setWindowSize` returned `undefined` and persisted optimistically, failure did not restore the standard choice, and Appearance contained no radiogroup.

### GREEN

Commands:

```text
pnpm -C web exec vitest run src/store/settingsStore.test.ts src/components/settings/SettingsView.visual.test.ts
pnpm -C web test -- src/store/settingsStore.test.ts src/components/settings/SettingsView.interaction.test.tsx src/components/settings/SettingsView.visual.test.ts
pnpm -C web exec vitest run src/App.lifecycle.test.tsx
pnpm -C web build
git diff --check
```

Observed results:

- Focused store/visual tests: 2 files, 16 tests passed.
- Web test command: 144 files, 1230 tests passed. The package script forwards selectors after `--` to Vitest, which executes the full suite.
- App lifecycle: 1 file, 12 tests passed.
- Production TypeScript/Vite build: exit 0.
- `git diff --check`: exit 0.

## Self-review

- Review found and this task fixed two native-geometry race/partial-failure issues: stale native operations are serialized, and a `setPosition` failure restores the original geometry.
- Tests cover migration, native-success persistence timing, failed resize rollback/toast, size/position partial failure, stale/later choices, startup-versus-selection serialization, keyboard radiogroup behavior, and visual geometry.
- The build retains existing ineffective-dynamic-import and >500 kB chunk warnings; no new build failure occurred.

## Commit

`fix(settings): keep only stable dark window layouts`

## Concerns

None blocking.
