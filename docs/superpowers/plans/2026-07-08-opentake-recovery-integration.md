# OpenTake Recovery Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Recover OpenTake on a branch based on current `origin/main`, organize Superpowers planning docs, finish the incomplete reverse-clip work, selectively replay still-relevant branch work, and verify the app with fresh code, security, and desktop/runtime evidence.

**Architecture:** Use `recovery/superpowers-integration-20260708-v2` as the controller branch. Treat old branches and worktrees as evidence sources, not direct merge targets. Integrate one functional slice at a time, verify it, record the branch decision, then continue.

**Tech Stack:** Rust workspace crates (`opentake-domain`, `opentake-ops`, `opentake-render`, `opentake-agent`, `opentake-tauri`), Tauri 2, React/TypeScript/Vite, Zustand, pnpm, cargo, Superpowers docs under `docs/superpowers`.

## Global Constraints

- Work inside `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake`; the parent directory is not the repo.
- Current integration branch is `recovery/superpowers-integration-20260708-v2`, based on `origin/main` at `ac50dc8`.
- Keep `../palmier-pro-upstream` read-only.
- Do not directly merge stale branch heads that are about 154 commits behind current `origin/main`.
- Do not include `.claude/launch.json` or `.claude/workflows/opentake-review.js` deletions from `opentake-pr*` worktrees in recovery commits.
- Use `git add` with explicit paths only.
- Use `apply_patch` for manual source edits.
- Every branch in the integration queue must be recorded as integrated, rejected, no-op, or deferred with evidence.
- No completion claim is valid without fresh verification output from the command that proves it.

---

### Task 1: Documentation Indexes And Branch Register

**Files:**
- Create: `docs/specs/INDEX.md`
- Create: `docs/superpowers/archive/2026-07-08-branch-integration-register.md`
- Create: `docs/superpowers/archive/2026-07-08-editing-automation-dos-session.md`
- Modify: `docs/INDEX.md`

**Interfaces:**
- Consumes: approved design at `docs/superpowers/specs/2026-07-08-opentake-recovery-integration-design.md`.
- Produces: discoverable docs and an integration register later tasks append to.

- [ ] **Step 1: Create `docs/specs/INDEX.md`**

Create `docs/specs/INDEX.md` with:

```markdown
# Specs Index

This directory contains implementation specifications grouped by subsystem.

## Agent

- [Evidence index](agent/0-evidence-index.md)
- [MCP server](agent/1-mcp-server.md)
- [Tools](agent/2-tools.md)
- [Short IDs](agent/3-short-id.md)
- [Execution shell](agent/4-execution-shell.md)
- [Chat](agent/5-chat.md)
- [Context signal](agent/6-context-signal.md)
- [System prompt](agent/7-system-prompt.md)
- [Core dispatch](agent/8-core-dispatch.md)
- [Telemetry](agent/9-telemetry.md)
- [Implementation](agent/10-implementation.md)

## Core

- [Design baseline](core/0-design-baseline.md)
- [Editor state](core/1-editor-state.md)
- [Command routing](core/2-command-routing.md)
- [Event bus](core/3-event-bus.md)
- [Frontend sync](core/4-frontend-sync.md)
- [Assembly](core/5-assembly.md)
- [Tauri commands](core/6-tauri-commands.md)
- [Security](core/7-security.md)
- [Implementation](core/8-implementation.md)

## Frontend

- [Principles](frontend/0-principles.md)
- [Design tokens](frontend/1-design-tokens.md)
- [Layout](frontend/2-layout.md)
- [Components](frontend/3-components.md)
- [Toolbar](frontend/4-toolbar.md)
- [Timeline](frontend/5-timeline.md)
- [Inspector](frontend/6-inspector.md)
- [Media panel](frontend/7-media-panel.md)
- [Preview](frontend/8-preview.md)
- [Interactions](frontend/9-interactions.md)
- [State](frontend/10-state.md)
- [Tauri](frontend/11-tauri.md)
- [Data models](frontend/12-data-models.md)
- [Implementation](frontend/13-implementation.md)

## Media

- [Principles](media/0-principles.md)
- [Structure](media/1-structure.md)
- [FFmpeg](media/2-ffmpeg.md)
- [Thumbnails](media/3-thumbnails.md)
- [Waveform](media/4-waveform.md)
- [Search](media/5-search.md)
- [Transcribe](media/6-transcribe.md)
- [ORT worker](media/7-ort-worker.md)
- [Coordinator](media/8-coordinator.md)
- [Domain contract](media/9-domain-contract.md)
- [Acceptance](media/10-acceptance.md)
- [Implementation](media/11-implementation.md)
```

- [ ] **Step 2: Add docs front-door links**

In `docs/INDEX.md`, add these rows to the "仓库根级文档" table:

```markdown
| [Specs Index](specs/INDEX.md) | 已批准/历史规格目录 |
| [Superpowers Recovery](superpowers/specs/2026-07-08-opentake-recovery-integration-design.md) | 本轮恢复集成设计与计划入口 |
```

- [ ] **Step 3: Create branch register**

Create `docs/superpowers/archive/2026-07-08-branch-integration-register.md` with:

```markdown
# Branch Integration Register

## Policy

"Merge all branches" means integrate all still-relevant active branch work onto
`recovery/superpowers-integration-20260708-v2` without regressing current
`origin/main`.

Direct stale branch merges are rejected when the branch would delete current
main-line work. Selective replay is the integration method.

## Current Base

| Ref | SHA | Meaning |
|---|---|---|
| `origin/main` | `ac50dc8` | Canonical integration base |
| `recovery/superpowers-integration-20260708-v2` | `942518e` | Recovery branch with approved Superpowers design |
| `backup/before-rollback-20260708-163646` | `9eceadb` | Evidence source for reverse clip and prior planning |

## Queue

| Source | Status | Evidence | Action |
|---|---|---|---|
| `opentake-pr9` dirty worktree | In progress | reverse-clip uncommitted diff; branch head equals `origin/main` | Port useful reverse-clip changes; exclude `.claude` deletions |
| `backup/before-rollback-20260708-163646` | In progress | commits `0b72a10`, `1cfee93`, `9eceadb` | Port reverse-clip fixes and docs only |
| `fix/text-raster-alignment` | Pending | one old commit `89bf38c`, 154 behind / 1 ahead | Inspect after reverse clip |
| `test/render-pixel-diff` | Pending | one old commit `eb6e429`, 154 behind / 1 ahead | Inspect after text raster |
| `fix/91-media-library-rewrite` | Pending | one old commit `b9e4954`, 154 behind / 1 ahead | Inspect for non-regressive media-library pieces |
| `feat/save-clip-as-media` | Pending | one old commit `708fd44`, 154 behind / 1 ahead | Inspect after reverse and media surfaces |
| `feat/freeze-frame` | Pending | one old commit `da3e934`, 154 behind / 1 ahead | Inspect after source-frame mapping is stable |
| `feat/account-scaffold` | Pending | one old commit `4986716`, 154 behind / 1 ahead | Inspect after editing recovery |
| `feat/agent-chat-panel` | Pending | one old commit `dd9f224`, 154 behind / 1 ahead | Inspect last due wide surface |
| `feat/generative-ui` | No-op | branch head equals `origin/main` | No functional delta at discovery |
| `feat/inspector-ai-edit-tab` | No-op | branch head equals `origin/main` | No functional delta at discovery |
| `feat/proxy-media` | No-op | branch head equals `origin/main` | No functional delta at discovery |

## Evidence Commands

- `git status --short --branch`
- `git worktree list --porcelain`
- `git rev-list --left-right --count origin/main...<branch>`
- `git log --oneline --no-merges origin/main..<branch>`
- `git diff --name-status origin/main..<branch>`
- `git -C /Users/lvbaiqing/TRUE\ 开发/PRIMARY-CN/opentake-pr9 diff --stat`
- `git -C /Users/lvbaiqing/TRUE\ 开发/PRIMARY-CN/opentake-pr9 diff --name-status`
```

- [ ] **Step 4: Restore historical DOS archive**

Create `docs/superpowers/archive/2026-07-08-editing-automation-dos-session.md` by copying the historical text from:

```bash
git show backup/before-rollback-20260708-163646:docs/superpowers/archive/2026-07-08-editing-automation-dos-session.md
```

The first heading must be:

```markdown
# Editing Automation DOS Session Archive
```

- [ ] **Step 5: Verify and commit docs**

Run:

```bash
rg -n "Specs Index|Superpowers Recovery|Branch Integration Register|Editing Automation DOS Session Archive" docs/INDEX.md docs/specs/INDEX.md docs/superpowers/archive
git diff --check -- docs/INDEX.md docs/specs/INDEX.md docs/superpowers/archive
```

Expected: `rg` prints the new headings and links; `git diff --check` exits 0.

Commit:

```bash
git add docs/INDEX.md docs/specs/INDEX.md docs/superpowers/archive/2026-07-08-branch-integration-register.md docs/superpowers/archive/2026-07-08-editing-automation-dos-session.md
git commit -m "docs: organize superpowers recovery planning"
```

---

### Task 2: Reverse Clip Contract

**Files:**
- Modify: `crates/opentake-domain/src/clip.rs`
- Modify: `crates/opentake-ops/src/command.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `crates/opentake-agent/src/tools/args.rs`
- Modify: `crates/opentake-agent/src/mcp/dispatch.rs`
- Modify: `crates/opentake-agent/src/tools/descriptions.rs`
- Modify: `crates/opentake-agent/src/tools/encode_timeline.rs`
- Modify: `crates/opentake-agent/src/prompt/base.rs`

**Interfaces:**
- Produces: `Clip.reversed: bool`.
- Produces: `ClipProperties.reversed: Option<bool>`.
- Produces: `ClipPropertiesDto.reversed: Option<bool>`.
- Produces: `SetClipPropertiesArgs.reversed: Option<bool>`.
- Consumes: existing `EditCommand::SetClipProperties` transaction and undo system.

- [ ] **Step 1: Capture the useful old reverse diff**

Run:

```bash
git -C /Users/lvbaiqing/TRUE\ 开发/PRIMARY-CN/opentake-pr9 diff -- crates/opentake-domain/src/clip.rs crates/opentake-ops/src/command.rs src-tauri/src/commands.rs crates/opentake-render/src/plan/build.rs crates/opentake-render/src/plan/types.rs web/src/components/timeline/ClipContextMenu.tsx web/src/i18n/dict.ts web/src/lib/types.ts > /tmp/opentake-pr9-reverse.diff
git diff origin/main..backup/before-rollback-20260708-163646 -- crates/opentake-agent/src/mcp/dispatch.rs crates/opentake-agent/src/prompt/base.rs crates/opentake-agent/src/tools/args.rs crates/opentake-agent/src/tools/descriptions.rs crates/opentake-agent/src/tools/encode_timeline.rs crates/opentake-domain/src/clip.rs crates/opentake-ops/src/command.rs src-tauri/src/commands.rs > /tmp/opentake-backup-reverse-contract.diff
```

Expected: both files exist and contain no `.claude` paths.

- [ ] **Step 2: Add failing domain serde tests**

In `crates/opentake-domain/src/clip.rs`, add tests to the existing test module:

```rust
#[test]
fn clip_reversed_roundtrip() {
    let mut c = base_clip();
    c.reversed = true;
    let json = serde_json::to_string(&c).unwrap();
    assert!(json.contains("\"reversed\":true"));
    let back: Clip = serde_json::from_str(&json).unwrap();
    assert!(back.reversed);
    assert_eq!(c, back);
}

#[test]
fn clip_reversed_defaults_false_when_absent() {
    let json = r#"{"id":"x","mediaRef":"m","startFrame":0,"durationFrames":12}"#;
    let c: Clip = serde_json::from_str(json).unwrap();
    assert!(!c.reversed);
}
```

Run:

```bash
cargo test -p opentake-domain clip_reversed -- --nocapture
```

Expected before implementation: compile failure because `Clip.reversed` is missing.

- [ ] **Step 3: Add `Clip.reversed`**

In `crates/opentake-domain/src/clip.rs`, add this field after `effects`:

```rust
/// Reverse playback. When true, video clips sample their referenced source
/// window in reverse order. Non-video sources ignore this flag.
#[serde(default, skip_serializing_if = "is_false")]
pub reversed: bool,
```

Add this helper near the struct:

```rust
fn is_false(value: &bool) -> bool {
    !*value
}
```

Set `reversed: false` inside `Clip::new`.

Run:

```bash
cargo test -p opentake-domain clip_reversed -- --nocapture
```

Expected after implementation: both `clip_reversed_*` tests pass.

- [ ] **Step 4: Add ops property contract**

In `crates/opentake-ops/src/command.rs`, add this field to `ClipProperties`:

```rust
/// Reverse playback flag. Per-clip and not propagated to linked audio partners.
pub reversed: Option<bool>,
```

In `apply_property_changes`, add:

```rust
if let Some(reversed) = props.reversed {
    clip.reversed = reversed;
}
```

Add this test in a new `reversed_property_tests` module near the existing
property tests in `crates/opentake-ops/src/command.rs`:

```rust
#[cfg(test)]
mod reversed_property_tests {
    use super::*;
    use crate::id::SeqIdGen;
    use opentake_domain::{Clip, ClipType, Track};

    fn state_with_video_clip() -> EditorState {
        let mut tl = Timeline::new();
        let mut t = Track::new("v1", ClipType::Video);
        let clip = Clip::new("c1", "asset", 0, 30);
        t.clips.push(clip);
        tl.tracks.push(t);
        EditorState::from_timeline(tl)
    }

#[test]
fn set_clip_properties_reversed_sets_only_requested_clip() {
    let mut state = state_with_video_clip();
    let ids = SeqIdGen::default();
    let video_id = state.timeline.tracks[0].clips[0].id.clone();
    let before = state.timeline.clone();

    let result = apply(
        &mut state,
        EditCommand::SetClipProperties {
            clip_ids: vec![video_id.clone()],
            properties: Box::new(ClipProperties {
                reversed: Some(true),
                ..Default::default()
            }),
        },
        &ids,
    )
    .unwrap();

    assert!(result.changed);
    let clip = state
        .timeline
        .tracks
        .iter()
        .flat_map(|track| &track.clips)
        .find(|clip| clip.id == video_id)
        .unwrap();
    assert!(clip.reversed);
    assert_ne!(state.timeline, before);
}
}
```

Run:

```bash
cargo test -p opentake-ops set_clip_properties_reversed -- --nocapture
```

Expected: test passes.

- [ ] **Step 5: Add Tauri IPC contract**

In `src-tauri/src/commands.rs`, add `reversed` to `ClipPropertiesDto`:

```rust
#[serde(default)]
pub reversed: Option<bool>,
```

Map it into `opentake_ops::command::ClipProperties`.

Add this serde test near the existing `deserializes_set_clip_properties_with_text_style` test:

```rust
#[test]
fn deserializes_set_clip_properties_with_reversed() {
    let request: EditRequest =
        serde_json::from_str(r#"{"type":"setClipProperties","clipIds":["c1"],"properties":{"reversed":true}}"#)
            .expect("setClipProperties with reversed camelCase");

    match request.into_command().expect("setClipProperties command") {
        EditCommand::SetClipProperties { properties, .. } => {
            assert_eq!(properties.reversed, Some(true));
        }
        other => panic!("expected SetClipProperties, got {other:?}"),
    }
}
```

Run:

```bash
cargo test -p opentake-tauri deserializes_set_clip_properties_with_reversed --lib -- --nocapture
```

Expected: test passes.

- [ ] **Step 6: Add Agent/MCP contract**

In `crates/opentake-agent/src/tools/args.rs`, add `reversed: Option<bool>` to `SetClipPropertiesArgs`.

In `crates/opentake-agent/src/mcp/dispatch.rs`, map the arg into `ClipProperties { reversed: a.reversed, .. }` for the `set_clip_properties` path.

Update `crates/opentake-agent/src/tools/descriptions.rs`, `crates/opentake-agent/src/tools/encode_timeline.rs`, and `crates/opentake-agent/src/prompt/base.rs` so Agent-visible schemas and prompt text mention `reversed`.

Add a test to the existing Agent tests:

```rust
#[test]
fn set_clip_properties_accepts_reversed() {
    let v = serde_json::json!({
        "clipIds": ["c1"],
        "reversed": true
    });
    let a: SetClipPropertiesArgs = decode_tool_args(&v, "").unwrap();
    assert_eq!(a.reversed, Some(true));
}
```

Run:

```bash
cargo test -p opentake-agent set_clip_properties_accepts_reversed -- --nocapture
```

Expected: test passes.

- [ ] **Step 7: Verify contract slice and commit**

Run:

```bash
cargo test -p opentake-domain clip_reversed -- --nocapture
cargo test -p opentake-ops set_clip_properties_reversed -- --nocapture
cargo test -p opentake-tauri deserializes_set_clip_properties_with_reversed --lib -- --nocapture
cargo test -p opentake-agent set_clip_properties_accepts_reversed -- --nocapture
git diff --check -- crates/opentake-domain/src/clip.rs crates/opentake-ops/src/command.rs src-tauri/src/commands.rs crates/opentake-agent/src/tools/args.rs crates/opentake-agent/src/mcp/dispatch.rs crates/opentake-agent/src/tools/descriptions.rs crates/opentake-agent/src/tools/encode_timeline.rs crates/opentake-agent/src/prompt/base.rs
```

Expected: all commands exit 0.

Commit:

```bash
git add crates/opentake-domain/src/clip.rs crates/opentake-ops/src/command.rs src-tauri/src/commands.rs crates/opentake-agent/src/tools/args.rs crates/opentake-agent/src/mcp/dispatch.rs crates/opentake-agent/src/tools/descriptions.rs crates/opentake-agent/src/tools/encode_timeline.rs crates/opentake-agent/src/prompt/base.rs
git commit -m "feat(editing): add reverse clip command contract"
```

---

### Task 3: Reverse Clip Render, Preview, And Web UI

**Files:**
- Modify: `crates/opentake-render/src/plan/types.rs`
- Modify: `crates/opentake-render/src/plan/build.rs`
- Modify: `crates/opentake-render/src/plan/tests.rs`
- Modify: `web/src/lib/types.ts`
- Modify: `web/src/lib/clip.ts`
- Modify: `web/src/lib/clip.test.ts`
- Modify: `web/src/lib/mpvEdl.ts`
- Modify: `web/src/lib/mpvEdl.test.ts`
- Modify: `web/src/components/preview/timelinePlayback.ts`
- Modify: `web/src/components/preview/timelinePlayback.test.ts`
- Modify: `web/src/components/timeline/ClipContextMenu.tsx`
- Modify: `web/src/components/timeline/ClipContextMenu.test.tsx`
- Modify: `web/src/i18n/dict.ts`
- Modify: `web/src/lib/fallback.ts`

**Interfaces:**
- Consumes: `Clip.reversed` from Task 2.
- Produces: reversed source-frame sampling for render plan and web playback helpers.
- Produces: clip context-menu toggle that calls `setClipProperties([clip.id], { reversed: !clip.reversed })`.

- [ ] **Step 1: Add render plan test**

In `crates/opentake-render/src/plan/tests.rs`, add:

```rust
#[test]
fn source_frame_video_reversed_respects_trim_window() {
    let mut clip = video_clip("c0", 0, 20);
    clip.trim_start_frame = 10;
    clip.reversed = true;
    let tl = single_video_timeline(clip);
    let plan = build_render_plan(&tl, RS, &TestMetrics::default());
    let cp = &plan.clip_plans[0];

    assert_eq!(source_frame_index(&cp, 0), 29);
    assert_eq!(source_frame_index(&cp, 5), 24);
    assert_eq!(source_frame_index(&cp, 19), 10);
}
```

Run:

```bash
cargo test -p opentake-render source_frame_video_reversed_respects_trim_window -- --nocapture
```

Expected before implementation: compile failure because `ClipPlan.reversed` is missing.

- [ ] **Step 2: Implement render plan mapping**

In `crates/opentake-render/src/plan/types.rs`, add `pub reversed: bool` to `ClipPlan`.

In `crates/opentake-render/src/plan/build.rs`, map `clip.reversed` into `ClipPlan`.

Update `source_frame_index(plan, f)` so video clips use:

```rust
let offset = ((f - plan.start_frame) as f64 * plan.speed).round() as i64;
let first = plan.trim_start_frame as i64;
let last = first + plan.source_frames_consumed.max(1) as i64 - 1;
if plan.reversed {
    (last - offset).clamp(first, last)
} else {
    (first + offset).clamp(first, last)
}
```

Keep existing image and lottie behavior unchanged.

Run:

```bash
cargo test -p opentake-render source_frame -- --nocapture
```

Expected: existing source-frame tests and the reversed test pass.

- [ ] **Step 3: Add TypeScript type and fallback support**

In `web/src/lib/types.ts`, add `reversed?: boolean` to `Clip` and `ClipPropertiesReq`.

In `web/src/lib/fallback.ts`, ensure the `"setClipProperties"` case copies `properties.reversed` onto matching clips when the property is present.

Add or update a fallback test in `web/src/lib/fallback.test.ts`:

```ts
it("applies reversed through setClipProperties", async () => {
  const fallback = createFallbackApi();
  const id = fallback.getTimeline().timeline.tracks[0].clips[0].id;

  await fallback.editApply({ type: "setClipProperties", clipIds: [id], properties: { reversed: true } });

  const clip = fallback.getTimeline().timeline.tracks[0].clips[0];
  expect(clip.reversed).toBe(true);
});
```

Run:

```bash
pnpm -C web test -- src/lib/fallback.test.ts
```

Expected: fallback tests pass.

- [ ] **Step 4: Update web playback and mpv EDL helpers**

In `web/src/components/preview/timelinePlayback.ts`, update source-time mapping so reversed clips sample the same trimmed window in reverse order.

Add this test to `web/src/components/preview/timelinePlayback.test.ts`:

```ts
it("maps reversed trimmed clips from the end of the trim window", () => {
  const c = clip({ startFrame: 100, durationFrames: 20, trimStartFrame: 10, speed: 1, reversed: true });
  expect(sourceFrameForClip(c, 100)).toBe(29);
  expect(sourceFrameForClip(c, 105)).toBe(24);
  expect(sourceFrameForClip(c, 119)).toBe(10);
});
```

In `web/src/lib/mpvEdl.ts`, keep reversed clips out of mpv EDL playback if mpv cannot express reverse segments; return `null` for a primary video track that contains a reversed clip so the app uses the Rust/canvas path.

Add this test to `web/src/lib/mpvEdl.test.ts`:

```ts
it("does not create an mpv edl for reversed clips", () => {
  const t = timeline([track("video", [clip({ id: "c1", reversed: true })])]);
  expect(timelineToEdl(t, pathOf)).toBeNull();
});
```

Run:

```bash
pnpm -C web test -- src/components/preview/timelinePlayback.test.ts src/lib/mpvEdl.test.ts
```

Expected: both test files pass.

- [ ] **Step 5: Add clip context-menu toggle**

In `web/src/components/timeline/ClipContextMenu.tsx`, add labels:

```ts
reverse: string;
reverseOn: string;
reverseTooLong: string;
```

Add `onReverse` and `reverseInfo` to `clipContextMenuItems`.

For video clips, add a menu item that:

```ts
const tooLongAndNotReversed = reverseInfo.tooLong && !reverseInfo.isReversed;
items.push({
  label: tooLongAndNotReversed
    ? labels.reverseTooLong
    : reverseInfo.isReversed
      ? labels.reverseOn
      : labels.reverse,
  checked: reverseInfo.isReversed,
  action: () => {
    if (tooLongAndNotReversed) return;
    ensureSelected();
    onReverse();
  },
});
```

At the component call site, pass:

```ts
onReverse={() => {
  void edit.setClipProperties([clipId], { reversed: !clip.reversed });
}}
reverseInfo={{
  isReversed: Boolean(clip.reversed),
  tooLong: clip.durationFrames > Math.round(timeline.fps * 60),
}}
```

Add labels in `web/src/i18n/dict.ts` for English, Chinese, and Japanese.

Add tests in `web/src/components/timeline/ClipContextMenu.test.tsx` that assert:

```ts
expect(videoItems.some((item) => item.label === labels.reverse)).toBe(true);
expect(audioItems.some((item) => item.label === labels.reverse)).toBe(false);
```

Run:

```bash
pnpm -C web test -- src/components/timeline/ClipContextMenu.test.tsx
pnpm -C web exec tsc -b --pretty false
```

Expected: context-menu tests pass and TypeScript build exits 0.

- [ ] **Step 6: Verify reverse UI/render slice and commit**

Run:

```bash
cargo test -p opentake-render source_frame -- --nocapture
pnpm -C web test -- src/lib/fallback.test.ts src/components/preview/timelinePlayback.test.ts src/lib/mpvEdl.test.ts src/components/timeline/ClipContextMenu.test.tsx
pnpm -C web exec tsc -b --pretty false
git diff --check -- crates/opentake-render/src/plan/types.rs crates/opentake-render/src/plan/build.rs crates/opentake-render/src/plan/tests.rs web/src/lib/types.ts web/src/lib/clip.ts web/src/lib/clip.test.ts web/src/lib/mpvEdl.ts web/src/lib/mpvEdl.test.ts web/src/components/preview/timelinePlayback.ts web/src/components/preview/timelinePlayback.test.ts web/src/components/timeline/ClipContextMenu.tsx web/src/components/timeline/ClipContextMenu.test.tsx web/src/i18n/dict.ts web/src/lib/fallback.ts
```

Expected: all commands exit 0.

Commit:

```bash
git add crates/opentake-render/src/plan/types.rs crates/opentake-render/src/plan/build.rs crates/opentake-render/src/plan/tests.rs web/src/lib/types.ts web/src/lib/clip.ts web/src/lib/clip.test.ts web/src/lib/mpvEdl.ts web/src/lib/mpvEdl.test.ts web/src/components/preview/timelinePlayback.ts web/src/components/preview/timelinePlayback.test.ts web/src/components/timeline/ClipContextMenu.tsx web/src/components/timeline/ClipContextMenu.test.tsx web/src/i18n/dict.ts web/src/lib/fallback.ts
git commit -m "feat(editing): finish reverse clip playback"
```

---

### Task 4: Reverse Clip Follow-Up From Backup Commits

**Files:**
- Modify only files touched by reverse-clip commits `0b72a10`, `1cfee93`, and `9eceadb` that remain relevant after Tasks 2 and 3.
- Modify: `docs/superpowers/archive/2026-07-08-branch-integration-register.md`

**Interfaces:**
- Consumes: committed reverse-clip work from Tasks 2 and 3.
- Produces: a completed register entry for `opentake-pr9` and `backup/before-rollback-20260708-163646`.

- [ ] **Step 1: Compare backup reverse commits against current work**

Run:

```bash
git diff --name-status HEAD..backup/before-rollback-20260708-163646 -- crates/opentake-agent/src/mcp/dispatch.rs crates/opentake-agent/src/prompt/base.rs crates/opentake-agent/src/tools/args.rs crates/opentake-agent/src/tools/descriptions.rs crates/opentake-agent/src/tools/encode_timeline.rs crates/opentake-domain/src/clip.rs crates/opentake-domain/src/split.rs crates/opentake-ops/src/command.rs crates/opentake-ops/src/intent.rs crates/opentake-ops/src/ops/trim.rs crates/opentake-render/src/plan/build.rs crates/opentake-render/src/plan/types.rs src-tauri/src/commands.rs web/src/components/preview/timelinePlayback.ts web/src/components/timeline/ClipContextMenu.tsx web/src/i18n/dict.ts web/src/lib/clip.ts web/src/lib/fallback.ts web/src/lib/mpvEdl.ts web/src/lib/types.ts
```

Expected: remaining diffs are reviewed one file at a time.

- [ ] **Step 2: Port remaining trim/split correctness if absent**

If backup commit `1cfee93` still contains trim/split reverse behavior missing from current files, port the exact logic into:

- `crates/opentake-domain/src/split.rs`
- `crates/opentake-ops/src/ops/trim.rs`
- `web/src/lib/clip.ts`

Add tests named:

- `split_reversed_clip_preserves_source_window`
- `trim_reversed_clip_keeps_visible_window`
- `reversedTrimRightKeepsSourceWindow`

Run:

```bash
cargo test -p opentake-domain split_reversed -- --nocapture
cargo test -p opentake-ops trim_reversed -- --nocapture
pnpm -C web test -- src/lib/clip.test.ts
```

Expected: tests pass. If the behavior is already covered by current code, update the register with "already covered by Tasks 2 and 3" and do not edit these files.

- [ ] **Step 3: Update register and commit**

In `docs/superpowers/archive/2026-07-08-branch-integration-register.md`, update:

```markdown
| `opentake-pr9` dirty worktree | Integrated | reverse-clip contract/render/web tests passed | Ported useful diff; excluded `.claude` deletions |
| `backup/before-rollback-20260708-163646` | Integrated | reverse commits checked against recovery branch | Ported remaining relevant fixes; docs restored |
```

Run:

```bash
git diff --check
```

Expected: exits 0.

Commit:

```bash
git add docs/superpowers/archive/2026-07-08-branch-integration-register.md crates/opentake-domain/src/split.rs crates/opentake-ops/src/ops/trim.rs web/src/lib/clip.ts web/src/lib/clip.test.ts
git commit -m "fix(editing): complete reverse clip trim handling"
```

If no trim/split code changed, commit only the register:

```bash
git add docs/superpowers/archive/2026-07-08-branch-integration-register.md
git commit -m "docs: record reverse clip integration"
```

---

### Task 5: Remaining Branch Queue Triage And Selective Replay

**Files:**
- Modify: `docs/superpowers/archive/2026-07-08-branch-integration-register.md`
- Modify feature files only when a source branch has a still-relevant non-regressive delta.

**Interfaces:**
- Consumes: branch queue in the register.
- Produces: one verified or documented decision per branch.

- [ ] **Step 1: Inspect each pending branch**

For each branch in this exact order:

```text
fix/text-raster-alignment
test/render-pixel-diff
fix/91-media-library-rewrite
feat/save-clip-as-media
feat/freeze-frame
feat/account-scaffold
feat/agent-chat-panel
feat/generative-ui
feat/inspector-ai-edit-tab
feat/proxy-media
```

Run:

```bash
branch=<branch-name>
git rev-list --left-right --count origin/main...$branch
git log --oneline --no-merges origin/main..$branch
git diff --name-status origin/main..$branch
git diff --stat origin/main..$branch
```

Expected: command output is copied into notes or summarized in the register before any replay.

- [ ] **Step 2: Reject direct stale merges that delete current main-line work**

If `git diff --stat origin/main..$branch` shows broad deletions of current docs/specs/probes or large unrelated rewrites, do not merge the branch head.

Record in the register:

```markdown
| `<branch>` | Rejected direct merge | `<left-right-count>` and broad deletion evidence | Selective replay only |
```

- [ ] **Step 3: Replay only verified functional deltas**

For a branch with a still-relevant single-feature delta, create one commit per feature:

```bash
git show --stat <branch>
git show --name-only <branch>
```

Port only files needed by that feature. Run targeted tests named by the touched subsystem:

```bash
cargo test -p opentake-render text_raster -- --nocapture
cargo test -p opentake-render pixel -- --nocapture
cargo test -p opentake-media library -- --nocapture
cargo test -p opentake-tauri save_clip --lib -- --nocapture
cargo test -p opentake-ops freeze_frame -- --nocapture
pnpm -C web test -- src/components/settings src/components/agent src/components/media src/components/timeline
pnpm -C web exec tsc -b --pretty false
```

Use only the commands relevant to the ported files. Record every run in the register with pass/fail status and exact command.

- [ ] **Step 4: Commit each branch decision**

After each branch decision or replay, commit:

```bash
git add docs/superpowers/archive/2026-07-08-branch-integration-register.md <touched-feature-files>
git commit -m "docs: record <branch-name> integration decision"
```

For replayed code, use a feature/fix commit message:

```bash
git add docs/superpowers/archive/2026-07-08-branch-integration-register.md <touched-feature-files>
git commit -m "feat: replay <feature-name> from branch queue"
```

Expected: every branch in the queue ends with register status `Integrated`, `Rejected direct merge`, `No-op`, or `Deferred`.

---

### Task 6: Project Verification, Security, And Desktop Runtime

**Files:**
- Create: `docs/superpowers/archive/2026-07-08-verification-report.md`
- Modify: `docs/superpowers/archive/2026-07-08-branch-integration-register.md`

**Interfaces:**
- Consumes: all commits from Tasks 1 through 5.
- Produces: fresh verification evidence and runtime/security status.

- [ ] **Step 1: Create verification report**

Create `docs/superpowers/archive/2026-07-08-verification-report.md`:

```markdown
# Verification Report

## Branch

- Branch: `recovery/superpowers-integration-20260708-v2`

## Required Commands

| Command | Status | Evidence |
|---|---|---|
| `cargo fmt --all --check` | Not run | Pending final verification |
| `cargo clippy --workspace --all-targets -- -D warnings` | Not run | Pending final verification |
| `cargo test --workspace` | Not run | Pending final verification |
| `cargo clippy -p opentake-tauri --no-default-features --all-targets -- -D warnings` | Not run | Pending final verification |
| `pnpm -C web build` | Not run | Pending final verification |
| `pnpm -C web test` | Not run | Pending final verification |

## Security Checks

| Check | Status | Evidence |
|---|---|---|
| Cargo audit tooling | Not inspected | Pending final verification |
| pnpm audit | Not run | Pending final verification |
| Secret scan | Not run | Pending final verification |

## Desktop Runtime

| Flow | Status | Evidence |
|---|---|---|
| Build desktop app | Not run | Pending final verification |
| Launch app | Not run | Pending final verification |
| Import media | Not run | Pending final verification |
| Timeline edit and playback | Not run | Pending final verification |
| Agent/MCP action | Not run | Pending final verification |
```

- [ ] **Step 2: Run project-level verification**

Run:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo clippy -p opentake-tauri --no-default-features --all-targets -- -D warnings
pnpm -C web build
pnpm -C web test
```

Expected: each command exits 0 before the report marks it as passing. If a command fails, record the exact failing command and fix it before continuing.

- [ ] **Step 3: Run available security checks**

Run:

```bash
command -v cargo-audit || true
command -v cargo-deny || true
pnpm -C web audit --audit-level moderate
rg -n "AKIA[0-9A-Z]{16}|sk-[A-Za-z0-9_-]{20,}|OPENAI_API_KEY|ANTHROPIC_API_KEY|FAL_KEY|REPLICATE_API_TOKEN|password\\s*=|secret\\s*=" . --glob '!target/**' --glob '!web/node_modules/**'
```

Expected: audit tooling availability is recorded. `pnpm audit` either exits 0 or any findings are fixed or documented with severity. Secret scan returns no real secrets; false positives are recorded with file/path evidence.

- [ ] **Step 4: Run desktop/runtime checks with a dedicated agent**

Spawn one dedicated desktop/runtime agent with this task:

```text
Run desktop/runtime verification for /Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake on branch recovery/superpowers-integration-20260708-v2. Do not edit files. Build or start the app using the repo's documented commands, then verify launch, import or fallback demo media, timeline playback, pause/scrub, a clip property edit, and one Agent/MCP capability if the server starts. Return exact commands, logs, screenshots or observations, and blockers.
```

If the app cannot be launched due local libmpv, accessibility, GPU, audio, or signing constraints, record the exact blocker and run the strongest substitute:

```bash
pnpm -C web build
cargo test -p opentake-tauri --lib
cargo test -p opentake-agent
```

- [ ] **Step 5: Commit verification report**

After commands and desktop agent results are recorded, run:

```bash
git diff --check -- docs/superpowers/archive/2026-07-08-verification-report.md docs/superpowers/archive/2026-07-08-branch-integration-register.md
```

Expected: exits 0.

Commit:

```bash
git add docs/superpowers/archive/2026-07-08-verification-report.md docs/superpowers/archive/2026-07-08-branch-integration-register.md
git commit -m "docs: record recovery verification"
```

---

## Execution Mode

The user requested multi-agent execution. Use Superpowers subagent-driven development for implementation:

- Assign Task 1 to a docs worker.
- Assign Tasks 2 and 3 to separate code workers only if their write sets remain disjoint; otherwise run them sequentially.
- Assign one reviewer after each code task.
- Assign one desktop/runtime/security agent during Task 6.

The controller session must review every diff, run final verification, and keep the branch register current.
