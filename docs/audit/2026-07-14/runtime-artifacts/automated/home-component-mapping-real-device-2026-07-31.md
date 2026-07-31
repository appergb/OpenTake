# Home component mapping real-device evidence — 2026-07-31

## Scope

- Plan: `home-shell-implementation.md`, Task 8 `HS-component-mapping-composite`.
- Requirement: `requirement-047e16af5d02f827`.
- Product boundary: packaged macOS Tauri application, not a browser-only mock.
- Verification project: `/private/tmp/opentake-beta-qa-20260729/TalkingHeadQA.opentake`.

## Code and package gates

- `pnpm --dir web test`: PASS, 82 files / 773 tests.
- `pnpm -C web exec vitest run src/components/shell/ShellComponentMapping.test.tsx -t every_documented_shell_component_has_exact_owner`: PASS, 1/1 exact owner-index test.
- `pnpm --dir web build`: PASS. Only the pre-existing Vite large-chunk and ineffective-dynamic-import warnings remain.
- `cargo test --workspace --no-fail-fast --quiet`: PASS across the workspace; the platform/real-device probes marked ignored by their owning suites remained ignored.
- `cargo fmt --all -- --check`: PASS.
- `git diff --check`: PASS.
- `web/node_modules/.bin/tauri build --debug --bundles app,dmg`: PASS.

Verified package artifacts:

| Artifact | Size | SHA-256 |
|---|---:|---|
| `target/debug/bundle/macos/OpenTake.app/Contents/MacOS/opentake` | 148338232 bytes | `329a937ee161abf913252ebed2e1dcde4e24dcce84fb06111091dd949156c68f` |
| `target/debug/bundle/dmg/OpenTake_1.0.0_aarch64.dmg` | 42588353 bytes | `a63074ee6a879767feb381d8b1add548ab989efa4e7efd74cef6cad2a722e7b1` |

These are Task 8 verification artifacts, not the project-wide Beta release. Beta remains blocked until every implementation-plan record and the release gates are closed.

## Packaged application walkthrough

### Logical linked A/V selection and Inspector ownership

1. Opened `TalkingHeadQA` from Home in the rebuilt application.
2. Selected the V1 picture clip whose linked audio companion is selected by the timeline.
3. Inspector exposed `视频`, `音频`, and `AI 编辑` rather than treating the linked pair as an unrelated multi-selection.
4. A genuine multi-selection continues to use the multi-selection Inspector state in the owning tests.

This walkthrough exposed and closed the original linked-selection defect by routing Inspector, crop, and transition selection through `findLogicalSingleClip`.

### AI Edit

1. Generated the deterministic balanced-polish proposal.
2. Rejected the first proposal and observed that no undo entry was created.
3. Generated again, accepted, and applied the proposal. The UI reported `建议已通过可撤销编辑命令应用。`; global undo became enabled.
4. Activated `撤销本次应用`; the proposal returned to idle, undo disabled, and redo enabled.
5. Owning tests also cover generation failure and cancellation without command submission.

### Music

1. Opened `音乐` and verified independent project-music and saved-library empty states.
2. Activated `AI 生成音乐`; Agent opened and received the exact draft requiring style, mood, duration, instrumental choice, model, and cost confirmation before paid generation.
3. Imported `/tmp/opentake-task8-music.wav` through the native macOS file panel. Input SHA-256: `c087187ef80798631ceac4eee8d43c8d4d441451576abdf45a8d7b7bd2269129`.
4. The imported item appeared in project music, `放到时间线` succeeded, timeline duration advanced to `00:30:20`, Audio Inspector opened, and undo became available.
5. After application restart, the project music entry and placed audio remained present.

### Cross-dissolve transition

1. Opened `转场`, selected linked V1 clip `id-4`, and resolved the exact adjacent cut `id-4 → c7e1e6d4-6be2-4844-a827-c4fa9e20d402`.
2. Applied a 15-frame / 0.50-second cross dissolve. The selection and transition panel stayed visible and the UI reported success.
3. Returned Home, reopened the project, and observed the same transition selected with `aria-pressed=true`, a 15-frame duration, and a visible Remove action.
4. Project persistence contained `transitionOut.kind = crossDissolve`, `durationFrames = 15`, and the exact `toClipId` on `id-4`.
5. Removed the transition, observed `转场已移除。`, then used global undo. The transition returned, redo became available, and the stale removal message was no longer rendered.

The walkthrough found and closed two packaged-app-only interaction defects: linked A/V selection hid the single-clip controls, and the generic media panel mouse-down handler cleared the cut selection while applying a transition. A final pass also bound transient feedback to the current project state so global undo cannot leave contradictory text.

## Real export result

With the saved cross dissolve active, the packaged application exported `/tmp/opentake-task8-transition-export.mp4` through the native save panel.

`ffprobe` result:

- video: H.264, 1280×720, `yuv420p`, 30/1 fps;
- audio: AAC;
- duration: 30.666667 seconds;
- size: 226647 bytes;
- SHA-256: `133a275ac193bf2e9e5b18f49e701a84cbdde6e278fec7d5d05b186e680f126b`.

The render-plan owning tests additionally verify the outgoing/incoming weighted layer sequence inside the dissolve window and that preview/export consume the same plan.

## Outcome

Task 8 is complete and verified at its owning code, persistence, packaged-interaction, and export boundaries. This evidence closes only the Task 8 component-mapping slice; it does not reclassify the broader Inspector plan or authorize the first Beta release.
