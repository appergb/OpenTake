# Home upstream composite real-device evidence — 2026-07-31

## Scope

- Plan: `home-shell-implementation.md`, Task 1 `HS-upstream-home-composite`.
- Requirement: `requirement-0ccbf12f850bf335`.
- Boundary: complete Home shell aggregation after all fifteen child slices were already closed.

## Exact code evidence

The initial command

`pnpm -C web exec vitest run src/components/home/HomeView.test.tsx -t upstream_home_children_close_one_composite_acceptance`

found no matching test and reported one skipped file / three skipped tests. This was retained as the genuine missing-owning-evidence baseline rather than manufacturing a production failure.

After implementation:

- exact composite test: PASS, 1/1 (three unrelated tests skipped by the name filter);
- complete Web suite: PASS, 82 files / 774 tests;
- production Web build: PASS, with only the pre-existing large-chunk and ineffective-dynamic-import warnings;
- complete Rust workspace: PASS on the immediately preceding combined branch gate; Task 1 changed no Rust source;
- `git diff --check`: PASS;
- debug macOS `.app` and `.dmg` bundle build: PASS.

Current verification artifacts:

| Artifact | Size | SHA-256 |
|---|---:|---|
| `target/debug/bundle/macos/OpenTake.app/Contents/MacOS/opentake` | 148338232 bytes | `e7374361c03307bfa1b257f3d4977ce5ec3e8fa298913e37af28194c4df963e1` |
| `target/debug/bundle/dmg/OpenTake_1.0.0_aarch64.dmg` | 42588806 bytes | `009b679ca08fd3b4840823b8a22f039061e046627a1c504dbb459d6137123a45` |

These remain verification bundles. They are not the first Beta release because other implementation-plan groups are still open.

## Composite contract coverage

The exact owning test exercises the Home boundary rather than checking disconnected source strings:

1. With no recents and no last-seen version, Home renders sidebar, sample strip, empty welcome state, and a modal first-run welcome surface.
2. The welcome action receives initial focus; Escape dismisses it and records the exact build-time `__APP_VERSION__`.
3. A simulated prior version renders the `New in v{version}` surface and release body, then persists dismissal.
4. Existing and missing project cards render together; the missing card exposes the context menu, reveal/remove/safe-trash actions, confirmation copy, and a non-destructive cancel path.
5. Native keyboard activation of the existing project routes the exact path through `openProjectPath`.
6. The product-demo sample routes the exact `product-demo, false` arguments through `openSampleProject`.

## Packaged application result

The rebuilt macOS application opened directly to a modal `v1.0.0 新功能` surface containing the AI Edit, music, and cross-dissolve release summary. Accessibility exposed one dialog heading and one `了解` button, with the background Home controls unavailable while the modal was active.

After activating `了解`, the application exposed:

- sidebar: New Project, Open Project, Library, Settings;
- samples: Product Demo, Quick Tutorial, Template Project;
- welcome hero and tagline;
- three recent project cards with separate project-action controls.

The app was quit and relaunched. The update surface did not reappear, proving the last-seen version persisted across a real process boundary and returned to the Home shell.

## Child runtime evidence aggregated by this umbrella

- Native new/open/sample and tutorial routing: [`home-sample-project-real-device-2026-07-31.md`](home-sample-project-real-device-2026-07-31.md).
- Project create/open/close/reopen lifecycle: [`home-project-lifecycle-real-device-2026-07-31.md`](home-project-lifecycle-real-device-2026-07-31.md).
- Autosave and Home metadata: [`home-autosave-metadata-real-device-2026-07-31.md`](home-autosave-metadata-real-device-2026-07-31.md).
- Missing/reveal/remove/safe-trash and secondary Home controls: [`home-secondary-controls-real-device-2026-07-30.md`](home-secondary-controls-real-device-2026-07-30.md).
- Native project-card focus, Return, and double-click: [`recent-project-card-real-device-2026-07-30.md`](recent-project-card-real-device-2026-07-30.md).

## Outcome

Task 1 is complete. Together with Tasks 2–16, all 43 records in the `home-shell` gap group now have owning tests and retained runtime evidence. This closes the Home group only; it does not reclassify other plan groups or authorize Beta publication.
