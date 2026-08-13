# Task 4 implementer report

Status: DONE

## Scope

- Moved the sole Library Home action from the content header to the top of the left category rail.
- Passed the navigation callback into `CategoryTree`; category state remains owned by `libraryStore` and therefore survives a return to Home and Library re-entry.
- Added rail-specific styling that applies the shared `--titlebar-safe-top` token and uses the title-bar control-size token with a 26px fallback.
- Kept the right header limited to its title, search, and sort controls. There is no alternate/mobile Home rendering.

## TDD evidence

### RED

Command:

```text
pnpm -C web test -- src/components/media/LibraryView.test.tsx
```

Observed result: exit 1. The new navigation test failed exactly as expected: the rail's first button was `All`, while the lone `Back to Home` action was in the content header.

The package-script invocation also discovered pre-existing concurrent MCP work: `ExternalMcpPane.test.tsx` could not resolve its companion component. That separate failure was not modified by this task.

### GREEN

Commands:

```text
pnpm -C web exec vitest run src/components/media/LibraryView.test.tsx --reporter=verbose
pnpm -C web build
git diff --check
```

Observed results:

- Focused Library suite: 1 file, 7 tests passed.
- Production TypeScript/Vite build: exit 0.
- Diff check: exit 0, no whitespace errors.
- The build retained the repository's existing ineffective-dynamic-import and >500 kB chunk warnings.

The full `pnpm -C web test` suite was also run after concurrent MCP files appeared. It finished with 144 files / 1247 tests passing and 1 MCP-only file / 5 tests failing in `ExternalMcpPane.test.tsx`; those assertions concern pairing API error and receipt behavior, not Library navigation, and are outside this task's owned files.

## Self-review

- `CategoryTree` renders exactly one accessible Home button before all category buttons, below the title-bar safe area.
- The test exercises actual navigation and category controls, asserts the Home control stays absent from the header while each built-in category is selected, and confirms the active Video category after re-entry.
- The change is confined to the three assigned Library files plus this required task report; concurrent settings/MCP and audit files were preserved.

## Commit

`fix(library): place Home navigation in the global rail`
