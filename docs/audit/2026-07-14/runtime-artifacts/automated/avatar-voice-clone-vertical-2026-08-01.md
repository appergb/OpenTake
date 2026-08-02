# Avatar and voice-clone vertical evidence — 2026-08-01

Scope: advanced AIGC identity workflows for digital avatar and custom voice. This artifact records the production contracts, deterministic provider fixtures, persistence/export checks, and UI component tests. It does not claim that a paid provider request was consumed.

## Production contracts

- Avatar generation uses the fixed fal `fal-ai/sync-lipsync/v3/image-to-video` endpoint with one project image and one project audio asset. Consent and paid-cost confirmation are mandatory. The fal request id, canonical request SHA-256, provider/model, source asset ids and source digests persist in generation provenance.
- fal queue polling is bounded to 30 minutes. User cancellation requests the official remote queue cancellation endpoint and prevents local import. Result URLs use the existing redirect-disabled, public-HTTPS-only, bounded generation downloader.
- Avatar output must probe as video with audio and match the narration duration within one project frame before a single durable Register + Place transaction. Failure/cancellation removes staging output.
- Voice enrollment uses ElevenLabs Instant Voice Cloning multipart upload; cloned speech uses the fixed `eleven_multilingual_v2` model. Provider resource ids are restricted to safe path-segment characters before they enter an authenticated endpoint.
- Voice reference digest, consent id, provider voice id and request hash persist without credentials. Enrollment and permanent revocation are external-identity audit mutations outside ordinary document undo/redo. Duplicate enrollment is rejected before paid submission; cancellation or local persistence failure deletes the newly created remote voice.
- Remote voice deletion is idempotent (`404` means already absent). A deleted voice cannot be revived by undo and is rejected before any later provider generation call.

Official provider references:

- <https://fal.ai/models/fal-ai/sync-lipsync/v3/image-to-video/api>
- <https://fal.ai/docs/documentation/model-apis/inference/queue>
- <https://elevenlabs.io/docs/api-reference/voices/ivc/create>

## Automated evidence

The avatar fixture generates a real H.264/AAC result and covers consent/cost failure, pre-cancellation, atomic import/placement, provenance, save/reopen, one-step generated-media undo, and six-frame export with audio.

The voice fixture covers invalid consent, enrollment, cancellation before provider use, provider failure with zero import, generated-audio provenance and audition path, generated-media undo, permanent provider/local revocation, undo resistance, save/reopen, and rejection after revocation without calling the provider.

Passing gates:

```text
CARGO_INCREMENTAL=0 cargo test -p opentake-domain -p opentake-ops -p opentake-core -p opentake-tauri
CARGO_INCREMENTAL=0 cargo clippy -p opentake-domain -p opentake-ops -p opentake-core -p opentake-tauri --all-targets -- -D warnings
CARGO_INCREMENTAL=0 cargo clippy -p opentake-tauri --all-targets --no-default-features -- -D warnings
cargo fmt --all -- --check
npm test                         # 117 files, 876 tests
npm run build
```

Rust results include 64 core unit tests, 234 domain unit tests, 202 ops unit tests, 416 Tauri unit tests, all selected integration tests, and their doc tests. Real-device-only playback/export probes remain explicitly ignored by their existing test annotations.

## Remaining Beta gate

- Enter user-owned fal and ElevenLabs keys in the packaged application and run one explicitly authorized paid request per provider.
- Verify the packaged Smart Pack tabs, consent/cost controls, cancel/retry, avatar preview, voice audition, undo and permanent revoke interactions visually.
- Retain the resulting packaged-app screenshots, media probes and project reopen evidence in the sequential Beta validation report.
