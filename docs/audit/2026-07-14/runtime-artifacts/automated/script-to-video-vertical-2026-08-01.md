# Script-to-video vertical — 2026-08-01

`requirement-30bcd764cc0c454d` is implemented through the capability-gated Agent contract and the enabled Smart Pack / Script-to-Video panel.

- Planning validates exact visual and narration media IDs, source duration, narration duration within one project frame, script bounds, frame durations, and supported cross-dissolve transitions.
- The canonical plan is SHA-256 identified and persists planner/version provenance, script, media IDs, narration IDs, transition choices, start frame, and durations in `project.json` before assembly clips exist. `apply=true` refuses an unreviewed or changed plan.
- Applying uses one `ApplyScriptAssemblyPlan` document transaction to add fresh visual/narration tracks, mute source visual audio where narration exists, preserve existing tracks, and bind outgoing transitions to exact adjacent clip IDs. One undo removes the complete assembly while retaining its reviewed plan.
- Cancellation/failure before commit leaves no partial tracks. The panel exposes editing, add/remove segment, plan review, retry, progress phases, cancel, apply, and undo, and ignores stale async completions.

Focused verification:

- `CARGO_INCREMENTAL=0 cargo test -p opentake-ops script_assembly -- --nocapture` — 2 passed.
- `CARGO_INCREMENTAL=0 cargo test -p opentake-tauri script_to_video_three_segment -- --nocapture` — passed using three real PNG sources plus real WAV narration; proved persisted preview, cancel atomicity, 3+3 aligned clips, two bound transitions, one undo, save/reopen, and an 18-frame H.264/AAC export whose audio stream probes successfully.
- `npm test -- ScriptToVideoTab.test.tsx` — 2 passed (review/edit/retry/apply/undo and cancel/stale-result isolation).

Full slice gates after integration:

- `CARGO_INCREMENTAL=0 cargo test -p opentake-domain -p opentake-ops -p opentake-project -p opentake-agent -p opentake-tauri` — passed, including the real script assembly/export integration; only the repository's seven explicitly ignored real-device probes remained ignored.
- `npm test` — 116 test files and 874 tests passed.
- `cargo fmt --all -- --check` — passed.
- `CARGO_INCREMENTAL=0 cargo clippy -p opentake-domain -p opentake-ops -p opentake-project -p opentake-agent -p opentake-tauri --all-targets -- -D warnings` — passed. Cargo reported only the pre-existing future-incompatibility notice for transitive `block v0.1.6`.
- `npm run build` — passed. Vite retained the existing dynamic-import and large-chunk advisories.
- `git diff --check` — passed.
