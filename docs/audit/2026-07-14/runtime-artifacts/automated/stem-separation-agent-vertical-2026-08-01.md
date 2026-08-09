# Stem Separation Agent / Track-Import Vertical — 2026-08-01

## Delivered surface

- The capability-gated `separate_stems` Agent tool now invokes the same local,
  integrity-checked `opentake-center-v1` owner as the Inspector. Unsupported
  hosted providers/models fail closed and no upload adapter is implied.
- Separation produces two ordinary audio assets with stable IDs and persisted
  source asset/SHA-256, execution, model SHA-256, stem kind, and output index.
- Existing Inspector progress, cancellation, retry, local-privacy copy, hosted
  consent refusal, and result state are retained. Each completed asset now has
  an inline native audio audition.
- A reviewed pair can be placed at the current playhead on two independent
  aligned audio tracks. Both placements form one `Import Stems To Tracks` undo
  entry; Undo removes both tracks while keeping reusable stem media in the
  catalog.
- The Agent `importToTracks` option uses the same separate-track transaction and
  returns both placed clip IDs and the action name.
- The local release profile remains intentionally scoped to centred dialogue
  plus complementary stereo side content. It is not represented as arbitrary
  semantic Demucs/MDX separation.

## Automated evidence

- `CARGO_INCREMENTAL=0 cargo test -p opentake-media --test stems -- --nocapture`
  - deterministic 48 kHz stereo mixture passed;
  - vocals improved SDR by at least 12 dB;
  - accompaniment achieved at least 60 dB against the documented
    mono-compatible reference;
  - the sum of published stems reconstructed that documented mixture at at
    least 60 dB SDR;
  - model integrity, progress endpoints, cancellation cleanup, and hosted
    fail-closed behavior passed.
- `CARGO_INCREMENTAL=0 cargo test -p opentake-tauri stem_bridge_imports_provenance_aligned_tracks_undo_reopen_and_cancel --lib -- --nocapture`
  - a real WAV source produced two persisted provenanced assets;
  - both three-frame outputs were placed at frame 45 on separate audio tracks;
  - one Undo removed both tracks while retaining the two media assets;
  - Redo plus save/reopen restored both tracks and all three media entries;
  - a pre-cancelled repeat added no media or tracks.
- `CARGO_INCREMENTAL=0 cargo test -p opentake-ops aligned_stem_track_tests --lib`
  - overlapping aligned entries use separate fresh tracks and one undo entry.
- `CARGO_INCREMENTAL=0 cargo clippy -p opentake-media -p opentake-ops -p opentake-tauri --all-targets -- -D warnings`
  - passed.
- `npm test -- --run`
  - all 114 web test files passed: 871 tests;
  - focused UI/IPC coverage includes progress, cancellation, audition elements,
    aligned-track import at the playhead, and Undo.
- Existing packaged-runtime evidence in
  `runtime-artifacts/automated/stems-real-device-2026-08-01.md` verifies local
  privacy/failure/success paths, source/model hashes, direct asset preview,
  cancellation cleanup, save/reopen, and independent packaged exports.

## Remaining acceptance evidence

The newly added explicit audition and aligned-track controls still require a
final packaged macOS GUI pass in the assembled Beta candidate. The existing
packaged evidence already covers the underlying separation assets and export
path; this final pass closes the updated interaction surface.
