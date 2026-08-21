# Preview Temporal Remap Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让包含 compositor 内容的变速或反向视频时间线进入已有 Rust 原生预览路径，消除当前把合法时间线直接判为 unsupported 的上游行为差异。

**Architecture:** 保留现有 WebKit 路由用于普通单视频轨，保留 Rust playback 作为多轨/compositor 的唯一原生路径；只调整 capability route，使 `speed`/`reversed` 不再在 `needsRust` 时生成 unsupported reason。Rust playback 已经通过 `RenderPlan` 和 `frameForSourceTime` 处理 source-frame remap，本切片先用测试证明这条链路，再补必要的播放/音频边界，而不是重写 Timeline 或 EditCommand。

**Tech Stack:** React/TypeScript, Zustand, Vitest/happy-dom, Rust/Tauri playback, opentake-render RenderPlan, FFmpeg/cpal。

**Spec:** `docs/superpowers/plans/2026-08-21-opentake-full-ui-and-upstream-convergence.md` Task 5；上游 `palmier-pro-upstream/Sources/PalmierPro/Preview/PreviewView.swift` 与 `EditorViewModel+ClipMutations.swift`。

## Global Constraints

- 只修改 `OpenTake-generation/`；`palmier-pro-upstream/` 永远只读。
- 不修改 `Timeline`、`EditCommand`、Preview tabs 或媒体组织状态；本切片只改变预览路由和必要的 playback 适配。
- 普通单视频 WebKit 预览继续保留；只有 compositor、变速/反向或多视频轨需要 Rust 时才切原生路径。
- Rust 不可用或被禁用时，必须保留明确的 `rust-unavailable` / `rust-disabled` unsupported surface，不能静默显示不完整画面。
- 预览帧和导出帧必须共享 source-frame 映射；音频使用同一 clip 时间窗，不能通过放宽断言掩盖漂移。
- 先写失败测试，再改实现；每个任务结束运行最相关测试并检查 diff。

---

### Task 1: Remove the false unsupported route for temporal remap

**Files:**
- Modify: `web/src/components/preview/playbackRoute.ts`
- Modify: `web/src/components/preview/playbackRoute.test.ts`

**Interfaces:**
- `resolveTimelinePlaybackRoute(timeline, runtime)` 对 `needsRust && (speed !== 1 || reversed)` 在 Rust 可用/启用时返回 `{ kind: "rust", reasons: [] }`。
- 普通单视频 `speed`/`reversed` 继续返回 WebKit；Rust 不可用/禁用时返回现有 typed unsupported reason。

- [x] **Step 1: Write failing route tests**

  在 `playbackRoute.test.ts` 增加三组断言：`text + reversed`、`colorGrade/mask + speed: 1.5` 在 `{ rustAvailable: true, rustEnabled: true }` 下返回 `rust`；同一时间线在 Rust 不可用时返回 `rust-unavailable`；普通单视频变速/反向仍返回 `webkit`。

- [x] **Step 2: Run the focused route tests and observe the old failure**

  Run:

  `NODE_OPTIONS=--localstorage-file=/tmp/opentake-vitest-localstorage-preview-remap-route.json pnpm exec vitest run src/components/preview/playbackRoute.test.ts`

  Expected before implementation: the compositor temporal cases remain `unsupported` with `composited-reverse` or `composited-speed`.

- [x] **Step 3: Implement the route matrix**

  Stop adding `composited-reverse` and `composited-speed` reasons when Rust is available. Allow the existing Rust branch for compositor timelines and allow the multi-video Rust branch even when temporal remapping is present. Keep lottie, unknown effects, mask overflow, Rust unavailable, and Rust disabled as explicit reasons.

- [x] **Step 4: Run route tests and inspect the matrix**

  Run the focused command again and confirm all route cases pass, including fallback behavior when the native endpoint is absent.

- [x] **Step 5: Commit the route slice**

  Commit only `playbackRoute.ts` and `playbackRoute.test.ts` with `feat(preview): route temporal compositor playback to rust`.

### Task 2: Restore Preview controls for the native temporal route

**Files:**
- Modify: `web/src/components/preview/Preview.tsx`
- Modify: `web/src/components/preview/Preview.test.tsx`
- Modify: `web/src/components/preview/previewEngine.ts`
- Test: `web/src/components/preview/previewEngine.test.ts`

**Interfaces:**
- A `rust` route renders the existing native playback surface, keeps play/pause/seek/capture enabled, and starts `nativePlaybackController` with the current `projectEpoch`, `timelineVersion`, and start frame.
- An unsupported route keeps the current visible error surface and disabled controls.

- [x] **Step 1: Add failing Preview surface tests**

  Add a compositor timeline with `speed: 1.5` and a reversed text/compositor clip to the existing Preview fixtures. Assert there is no `unsupported-playback-surface`, `data-playback-surface="native"` is present when the mocked endpoint is available, and play/capture are not disabled.

- [x] **Step 2: Run Preview and engine tests before implementation**

  Run:

  `NODE_OPTIONS=--localstorage-file=/tmp/opentake-vitest-localstorage-preview-remap-ui.json pnpm exec vitest run src/components/preview/Preview.test.tsx src/components/preview/previewEngine.test.ts`

  Expected before implementation: the new cases fail because the route is still unsupported.

- [x] **Step 3: Wire only the route-dependent surface**

  Reuse the existing `playbackRoute.kind === "rust"` branches. Do not add a second clock or a WebKit fallback for authored temporal properties. If a stale comment or conditional still says temporal remapping must stay on WebKit, update that contract to match Task 1.

- [x] **Step 4: Run Preview tests and verify failure fallback**

  Run the focused command again. Confirm native temporal playback starts with Rust available, while the same timeline still shows a typed unsupported surface when Rust is unavailable or disabled.

- [x] **Step 5: Commit the Preview slice**

  Commit the UI and engine tests with `feat(preview): enable temporal compositor controls`.

### Task 3: Prove RenderPlan/playback temporal parity at the native boundary

**Files:**
- Modify: `src-tauri/tests/playback_transport_integration.rs`
- Modify: `src-tauri/src/playback/engine.rs` only if the integration test exposes a real remap bug
- Modify: `src-tauri/src/playback/audio.rs` only if the same clip window is not used by the playback clock
- Test: `crates/opentake-render/src/plan/tests.rs` when a missing pure-plan case is found

**Interfaces:**
- Native playback of a compositor timeline with `speed: 1.5` or `reversed: true` emits valid frames and monotonic timeline frame events.
- `RenderPlan` maps timeline frame to source frame consistently with existing export tests; no new independent remap formula is introduced.

- [x] **Step 1: Write a deterministic native playback regression test**

  Extend the existing `playback_transport_integration.rs` fixture helpers with one visual clip that has a compositor property and temporal remap. Start the playback engine through its existing `PreviewServer`/publication seam, collect at least the first, middle, and terminal frame publications, and assert JPEG bodies decode plus emitted timeline frames never move backwards.

- [x] **Step 2: Run the native test before any Rust change**

  Run:

  `cargo test -p opentake-tauri --features playback-engine --test playback_transport_integration`

  Record whether the existing RenderLoop already supports the case; if it does, keep Rust production code unchanged.

- [x] **Step 3: Add the smallest Rust fix only if the test proves a defect**

  Reuse `try_build_render_plan`, `frameForSourceTime`, and the existing `PlaybackResolverState`; do not add a second source-frame conversion. Preserve seek cancellation, pause/resume, and audio clock ownership.

- [x] **Step 4: Run native parity and existing playback tests**

  Run the focused integration test, `cargo test -p opentake-render plan`, and the existing playback transport/unit tests. Run `cargo fmt --all -- --check`.

- [x] **Step 5: Commit the native proof**

  Commit the test or minimal Rust fix with `test(preview): cover temporal compositor playback`.

### Task 4: Installed-app parity verification and capability write-back

**Files:**
- Modify: `docs/capabilities/CAPABILITY-LEDGER.md`
- Modify: `docs/capabilities/requirements.json`
- Modify: `docs/audit/2026-08-21/full-desktop-functional-matrix.md`
- Modify: `docs/superpowers/plans/2026-08-21-opentake-full-ui-and-upstream-convergence.md`

- [x] **Step 1: Build and install the app**

  Run `web/node_modules/.bin/tauri build --bundles app`, copy the generated `.app` to `/Applications/OpenTake.app`, and record the executable hash without including sidecar binaries.

- [x] **Step 2: Exercise the real temporal case with Computer Use**

  Open a QA project containing a video clip with a compositor property and `speed=1.5`; verify the unsupported banner is absent, native playback advances, pause/resume works, scrub reaches the middle and tail, and capture remains enabled. Repeat with `reversed=true` if the fixture exposes it.

- [ ] **Step 3: Compare preview and export evidence**

  Export the same QA timeline, inspect the output with ffprobe, and compare start/middle/end frames against the preview captures. Record any audio timing discrepancy as a separate partial capability rather than marking Preview verified.

- [x] **Step 4: Run final gates for this slice**

  Run the focused Web tests, serial Web suite with an explicit localStorage file, relevant Rust tests, `pnpm build`, `cargo fmt --all -- --check`, `git diff --check`, and JSON validation.

- [x] **Step 5: Write evidence and update status**

  Mark only the proven portion in the ledger; keep `UP-PREVIEW-PLAYBACK` partial until temporal playback, audio timing, and preview/export frame evidence all pass. Update the desktop matrix with exact actions and results.

**Current result:** temporal route/native/source-frame, installed-app screen checks, and H.264 SavePanel/export/ffprobe checks pass; Step 3 remains open for a fresh preview-vs-export start/middle/end comparison with a带音频 fixture and independent audio-sync evidence.

## Completion Conditions

- Compositor + speed/reverse no longer becomes unsupported solely because of temporal remapping when Rust playback is available.
- Rust unavailable/disabled and genuinely unsupported effects still fail visibly and safely.
- Web route tests, Preview interaction tests, native playback tests, and installed-app checks all pass for the same QA fixture.
- Preview/export source-frame behavior is aligned; any remaining audio drift is explicitly tracked and not hidden.
