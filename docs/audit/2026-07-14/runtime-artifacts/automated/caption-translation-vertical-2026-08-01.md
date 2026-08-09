# Caption translation vertical — 2026-08-01

`requirement-dbe026f6228381a6` is implemented as a production desktop and Agent vertical.

- Captions are addressed by persisted clip ID. Provider output is rejected on unknown or duplicate IDs, and omitted/empty items become per-caption failures.
- The production bridge uses the existing Settings → AI BYOK keychain boundary for OpenAI or Anthropic, requires explicit cost authorization, redacts provider bodies from errors, and supports cancellation before and after the network request.
- Preview returns the exact source/translated diff with the project epoch/version. Captions lets the user accept or reject each change, retry the provider, apply the accepted subset, cancel, and undo.
- `ApplyCaptionTranslations` validates the complete batch before mutation, keeps clip ID, caption group, track, start frame, and duration unchanged, and stores source text, source/target locale, provider, and model. Manual text editing clears that provenance.
- Provider-wide failure leaves the timeline unchanged. Partial success changes only successful captions; failed caption text remains original.

Focused verification:

- `CARGO_INCREMENTAL=0 cargo test -p opentake-ops caption_translation -- --nocapture` — 2 passed.
- `CARGO_INCREMENTAL=0 cargo test -p opentake-tauri caption_translation -- --nocapture` — 2 passed, including success/save-reopen/undo and partial/failure atomicity.
- `npm test -- CaptionsTab.test.tsx` — 1 passed, exercising review, individual rejection, apply, and undo.
- `npm test` — 115 files / 872 tests passed.
- `npm run build` — TypeScript and production Vite build passed (existing chunk-size/dynamic-import warnings only).
