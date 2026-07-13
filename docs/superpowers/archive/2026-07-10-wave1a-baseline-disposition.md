# Wave 1A baseline hunk disposition ledger

## Reproducible identity

- Source head: `c2f807aafd6e46088365eac2de45fe8803a7e1d0`
- Restored safety commit: `eb6556a5e4d266cfef39a37eb222b4071cd7133e`
- Delivery head: `be23c51d082f0d0467727e98e922f9c777670b42`
- This ledger's own hunk is audit metadata outside the classified set; every count and patch identity below excludes this file.
- Parser: `git diff --unified=0 --no-color <A>..<B>`; each textual `@@` block is one hunk. The fingerprint is the first 16 hex characters of SHA-256 over `path + LF + normalized-body`, where normalized-body omits the `@@` header/ranges but preserves every `+`, `-`, context, no-newline, and binary-marker line in order. Exact matching requires the same path and normalized body and consumes duplicate occurrences FIFO, one-to-one.
- Delivery ownership: added/mixed ranges use `git blame DELIVERY -L <new-range>` and map the latest contributing commit to the first cumulative reviewed slice containing it; deletion-only ranges use the last source-to-delivery commit touching the path. Notes retain the deciding commit and any additional blamed contributors.

### Ledger-excluded parent patch receipts

These immutable receipts were produced for the delivery parent and deliberately exclude the ledger hunk:

- source -> safety: `9a6e82175fd18d49617a3af494b03e8aaf0df4a71b1192263133f99725f7af09`
- source -> delivery: `52fae51f9bb3eae2ca972a915230f1414320bf8d9fe7ebea20210e16ec943355`
- safety -> delivery: `01559ff9161ee8904f5a2a2ef018b3119ee970bae72995c3885ee94008be71e9`

### Verified counts

- source -> safety: 56 paths, 403 hunks.
- source -> delivery: 84 paths, 698 hunks.
- Baseline dispositions: delivered-exact 99; superseded-by-reviewed-fix 284; historical-evidence-only 20.
- Exact baseline occurrences consumed one-to-one: 99.
- Integration-only delivery hunks: 599 (= 698 - 99 consumed exact occurrences).

## Evidence catalog

All table evidence codes expand to the concrete regression tests, gate, and exact report below. A `superseded-by-reviewed-fix` row therefore names its replacement tests and report through its mandatory code, rather than using a generic rewrite claim.

| Code | Reviewed slice / exact commit | Principal regression evidence | Gate and exact reviewer report (under safety root) | Approval |
|---|---|---|---|---|
| E04 | Runtime/libmpv removal `bffbcf64d991ac39d8dcef84d95298e968bed6f7` | PanelShell PLAY/PAUSE opaque-surface regression; production `useTimelinePlaybackEngine` play/pause DOM-media regression; full Web 48/518 | gate `logs/01-runtime-dependency-bffbcf64d991-review-fix-1/`; report `logs/01-runtime-dependency-bffbcf64d991-review-fix-1/reviewer-report.md` | APPROVE 0/0/0 |
| E51 | Project runtime identity `2eff907cfc9cbb816a1a961d546c93f4ed363f7e` | `opening_two_projects_produces_distinct_epochs_at_version_zero`; `runtime_snapshot_is_atomic_across_project_swaps`; event-after-unlock lifecycle regressions | gate `logs/02a-core-project-identity-2eff907cfc9c-initial/`; report `logs/02a-core-project-identity-2eff907cfc9c-initial/reviewer-report.md` | APPROVE; blocking 0/0, advisory Minor 1 |
| E52 | Playback session/publication identity `e2daeb279a337e22c412e31ab3acd95acbcb4456` | overlapping project-boundary `busy` interleave; StrictMode listener rejection/retry; exact encoded publication coordinator regressions | gate `logs/02b-playback-session-identity-e2daeb279a33-review-fix-2/`; report `logs/02b-playback-session-identity-e2daeb279a33-review-fix-2/reviewer-report.md` | APPROVE 0/0/0 |
| E53 | Bounded cancellable playback workers `ba5b1ceac463f01c6830fa2e5932734d99d66eeb` | blocked PCM/frame cancellation; admitted audio-worker panic/capacity; bounded memory; reaper and Result clock all-target regressions | gate `logs/02c-playback-bounded-workers-ba5b1ceac463-review-fix-5/`; report `logs/02c-playback-bounded-workers-ba5b1ceac463-review-fix-5/reviewer-report.md` | APPROVE 0/0/0 |
| E54 | Exact cold bootstrap lifecycle `24ab2590ce964fd04f3dc960be23c26408270ef4` | `bootstrap_request_has_zero_tolerance_at_exact_source_frame`; `cold_bootstrap_uses_exact_trimmed_source_frame`; `cold_bootstrap_decode_failure_is_reported_instead_of_publishing_black`; readiness cancellation/ownership regressions | gate `logs/02d-playback-exact-bootstrap-24ab2590ce96-review-fix-3/`; report `logs/02d-playback-exact-bootstrap-24ab2590ce96-review-fix-3/reviewer-report.md` | APPROVE 0/0/0 |
| E55 | Project-scoped bounded media prewarm `f99da16c27b440a10715cc4b2e50b9ab713fafa9` | seven `media::prewarm` capacity/coalescing/epoch/cancellation/commit/concurrency regressions; completed same-id/different-source swap; queued stale import; project lifecycle ordering | gate `logs/02e-media-project-prewarm-f99da16c27b4-review-fix-2/`; report `logs/02e-media-project-prewarm-f99da16c27b4-review-fix-2/reviewer-report.md` | APPROVE 0/0/0 |
| E56 | Fail-closed live exact transport `3fe09766819b0d07b17d94a7c870ac744d41129c` | `frame_route_transitions_from_204_to_valid_200_jpeg`; complete JPEG; wrong identity; cross-origin; two distinct multipart frames; stream cross-origin | gate `logs/02f-playback-live-transport-3fe09766819b-initial/`; report `logs/02f-playback-live-transport-3fe09766819b-initial/reviewer-report.md` | APPROVE 0/0/0 |
| E61 | Capability route contract `8b47e64a8e6c679f6bb3605ac1c92fb1a98415b5` | eight exact playback-route cases plus non-unit/non-finite speed matrix; hidden-track and cross-clip compositor-temporal regressions | gate `logs/03a-preview-route-contract-8b47e64a8e6c-review-fix-1/`; report `logs/03a-preview-route-contract-8b47e64a8e6c-review-fix-1/reviewer-report.md` | APPROVE 0/0/0 |
| E62 | Retained Rust frame UI `dc83284319bdd7ec816a0175f1e97b90a9bc5e1a` | six focused Web files covering retained-frame/currentSrc/double-rAF/stale-session/retry behavior; 88/88 tests; exact-bundle Plain/Rust/Unsupported QA | full gate `logs/03b-preview-retained-frames-dc83284319bd-review-fix-2/`; final approval gate `logs/03b-preview-retained-frames-dc83284319bd-qa-repair-review/`; report `logs/03b-preview-retained-frames-dc83284319bd-qa-repair-review/reviewer-report.md` | APPROVE 0/0/0 |
| E63 | Project/source visual-cache UI `1f2bf4e49877c145b7c7990e2a7ad85b32685aed` | same-ID/different-source admission/cache/in-flight regressions; stale async write/finally guards; same-project cache stability; focused Web 8 files/71 and exact-bundle visual QA | gate `logs/03c-web-project-media-ui-1f2bf4e49877-review-fix-5/`; report `logs/03c-web-project-media-ui-1f2bf4e49877-review-fix-5/reviewer-report.md` | APPROVE 0/0/0 |
| E7A | Reviewed evidence documentation `be23c51d082f0d0467727e98e922f9c777670b42` | README libmpv/setup removal; dated historical/current and manual/automated boundaries; artifact hash separation; cumulative Web 54/570 plus 04a focused 62, playback 7/7, transport 6/6 | gate `logs/04a-reviewed-evidence-be23c51d082f-review-fix-1/`; report `logs/04a-reviewed-evidence-be23c51d082f-review-fix-1/reviewer-report.md` | APPROVE 0/0/0 |

## Baseline hunk dispositions

Every B row is one source-to-safety occurrence. Only the three listed disposition values are valid.

| ID | Path | Zero-context header/range | Fingerprint | Disposition | Evidence | Owner / note |
|---|---|---|---|---|---|---|
| B0001 | `.github/workflows/ci.yml` | `@@ -58,10 +57,0 @@ jobs:` | `67b38713b6f2cda2` | delivered-exact | E04 | 2836a5386328 |
| B0002 | `.gitignore` | `@@ -15 +15,2 @@ src-tauri/gen/` | `bfb83bc07a20a881` | superseded-by-reviewed-fix | E04 | reviewed semantic replacement |
| B0003 | `Cargo.lock` | `@@ -3441 +3440,0 @@ dependencies = [` | `ac9898c29d1e33f1` | delivered-exact | E04 | 2836a5386328 |
| B0004 | `Cargo.lock` | `@@ -5218,21 +5216,0 @@ dependencies = [` | `f1e5c287320160f4` | delivered-exact | E04 | 2836a5386328 |
| B0005 | `README.md` | `@@ -268 +267,0 @@ Key files for comparison:` | `a39e8e08aac69c87` | delivered-exact | E7A | 43524e9d7181 |
| B0006 | `README.md` | `@@ -285,3 +283,0 @@ cd ..` | `af53d55d7a259074` | delivered-exact | E7A | 43524e9d7181 |
| B0007 | `docs/architecture/HANDOFF-2026-07.md` | `@@ -35,2 +35,2 @@` | `81ddfb24e553bdf0` | historical-evidence-only | E7A | Task 7A historical preservation |
| B0008 | `docs/architecture/HANDOFF-2026-07.md` | `@@ -91 +91 @@` | `f7ab9920c2880199` | historical-evidence-only | E7A | Task 7A historical preservation |
| B0009 | `docs/architecture/PLAYBACK-ENGINE.md` | `@@ -3 +3 @@` | `83625b9e8d900d9b` | historical-evidence-only | E7A | Task 7A historical preservation |
| B0010 | `docs/architecture/PLAYBACK-ENGINE.md` | `@@ -5 +5 @@` | `be71f5d2614fb708` | historical-evidence-only | E7A | Task 7A historical preservation |
| B0011 | `docs/architecture/PLAYBACK-ENGINE.md` | `@@ -9 +9 @@` | `cbb2c284337c9ac6` | historical-evidence-only | E7A | Task 7A historical preservation |
| B0012 | `docs/architecture/PLAYBACK-ENGINE.md` | `@@ -14 +14,4 @@` | `2208ee978da43f34` | historical-evidence-only | E7A | Task 7A historical preservation |
| B0013 | `docs/architecture/PLAYBACK-ENGINE.md` | `@@ -21 +24 @@ PLAY 态:` | `127940fe6b02294b` | historical-evidence-only | E7A | Task 7A historical preservation |
| B0014 | `docs/architecture/PLAYBACK-ENGINE.md` | `@@ -23 +26 @@ PLAY 态:` | `e0328924e03ad68c` | historical-evidence-only | E7A | Task 7A historical preservation |
| B0015 | `docs/architecture/PLAYBACK-ENGINE.md` | `@@ -25 +28,2 @@ PLAY 态:` | `4b6df8ab95a7d3c1` | historical-evidence-only | E7A | Task 7A historical preservation |
| B0016 | `docs/architecture/PLAYBACK-ENGINE.md` | `@@ -41 +45 @@ SCRUB / PAUSE 态: 引擎停, 回原 <video> + composite_frame 路径(零改动)` | `6dd06a6e143efca5` | historical-evidence-only | E7A | Task 7A historical preservation |
| B0017 | `docs/architecture/PLAYBACK-ENGINE.md` | `@@ -44 +48 @@ SCRUB / PAUSE 态: 引擎停, 回原 <video> + composite_frame 路径(零改动)` | `5b587d5dbd09fe6e` | historical-evidence-only | E7A | Task 7A historical preservation |
| B0018 | `docs/architecture/PLAYBACK-ENGINE.md` | `@@ -49,2 +53,2 @@ SCRUB / PAUSE 态: 引擎停, 回原 <video> + composite_frame 路径(零改动)` | `bbd463c6b781c693` | historical-evidence-only | E7A | Task 7A historical preservation |
| B0019 | `docs/architecture/PLAYBACK-ENGINE.md` | `@@ -52,2 +56,3 @@ SCRUB / PAUSE 态: 引擎停, 回原 <video> + composite_frame 路径(零改动)` | `68c71335efacdb6e` | historical-evidence-only | E7A | Task 7A historical preservation |
| B0020 | `docs/architecture/PLAYBACK-ENGINE.md` | `@@ -55 +60,2 @@ SCRUB / PAUSE 态: 引擎停, 回原 <video> + composite_frame 路径(零改动)` | `23e65f024705296c` | historical-evidence-only | E7A | Task 7A historical preservation |
| B0021 | `docs/architecture/PLAYBACK-ENGINE.md` | `@@ -57,3 +63,3 @@ SCRUB / PAUSE 态: 引擎停, 回原 <video> + composite_frame 路径(零改动)` | `b622872740159015` | historical-evidence-only | E7A | Task 7A historical preservation |
| B0022 | `docs/architecture/PLAYBACK-ENGINE.md` | `@@ -61 +67 @@ SCRUB / PAUSE 态: 引擎停, 回原 <video> + composite_frame 路径(零改动)` | `d8f714012a34ecc8` | historical-evidence-only | E7A | Task 7A historical preservation |
| B0023 | `docs/architecture/PLAYBACK-ENGINE.md` | `@@ -65,0 +72,2 @@ SCRUB / PAUSE 态: 引擎停, 回原 <video> + composite_frame 路径(零改动)` | `4baf4fe99ba7b5f7` | historical-evidence-only | E7A | Task 7A historical preservation |
| B0024 | `docs/architecture/PLAYBACK-ENGINE.md` | `@@ -68 +76,2 @@ SCRUB / PAUSE 态: 引擎停, 回原 <video> + composite_frame 路径(零改动)` | `48404e100c57210b` | historical-evidence-only | E7A | Task 7A historical preservation |
| B0025 | `docs/superpowers/archive/2026-07-08-verification-report.md` | `@@ -57,0 +58,42 @@` | `40d3c3b36696403f` | historical-evidence-only | E7A | Task 7A historical preservation |
| B0026 | `docs/superpowers/archive/2026-07-10-playback-cache-installed-app-qa.md` | `@@ -0,0 +1,311 @@` | `3ae58d5f06e9dea2` | historical-evidence-only | E7A | Task 7A historical preservation |
| B0027 | `src-tauri/Cargo.toml` | `@@ -65 +64,0 @@ cpal = { version = "0.15", optional = true }` | `67b9b289b682e7dc` | delivered-exact | E04 | 2836a5386328 |
| B0028 | `src-tauri/capabilities/default.json` | `@@ -15,2 +15 @@` | `ec7e03088f85aff7` | delivered-exact | E04 | 2836a5386328 |
| B0029 | `src-tauri/src/lib.rs` | `@@ -20 +19,0 @@ mod media;` | `f24f13f2c7c86197` | delivered-exact | E04 | 2836a5386328 |
| B0030 | `src-tauri/src/lib.rs` | `@@ -57 +55,0 @@ pub fn run() {` | `9b45654f34285243` | delivered-exact | E04 | 2836a5386328 |
| B0031 | `src-tauri/src/lib.rs` | `@@ -81,4 +78,0 @@ pub fn run() {` | `2d8d324c7df9344a` | delivered-exact | E04 | 2836a5386328 |
| B0032 | `src-tauri/src/media.rs` | `@@ -25 +25,6 @@` | `b078671fc5141e9c` | superseded-by-reviewed-fix | E55 | reviewed semantic replacement |
| B0033 | `src-tauri/src/media.rs` | `@@ -42 +47 @@ use opentake_media::{` | `a91510936ffc8613` | superseded-by-reviewed-fix | E55 | reviewed semantic replacement |
| B0034 | `src-tauri/src/media.rs` | `@@ -323,0 +329,122 @@ const PREVIEW_POSTER_MAX_SIZE: (u32, u32) = (1920, 1080);` | `9715c5f6bdf719f4` | superseded-by-reviewed-fix | E55 | reviewed semantic replacement |
| B0035 | `src-tauri/src/media.rs` | `@@ -518,0 +646 @@ fn generate_thumbnail_for_entry(` | `f056ada91769550d` | superseded-by-reviewed-fix | E55 | reviewed semantic replacement |
| B0036 | `src-tauri/src/media.rs` | `@@ -526,0 +655,48 @@ fn generate_thumbnail_for_entry(` | `9849af96b07a20ef` | superseded-by-reviewed-fix | E55 | reviewed semantic replacement |
| B0037 | `src-tauri/src/media.rs` | `@@ -559,0 +736,3 @@ fn generate_thumbnail_for_entry(` | `466e65c61c1cbc52` | superseded-by-reviewed-fix | E55 | reviewed semantic replacement |
| B0038 | `src-tauri/src/media.rs` | `@@ -645,8 +824,4 @@ pub(crate) const IMPORT_ACCEPTED_MIMES: &str =` | `29ab38c2fa474e1a` | superseded-by-reviewed-fix | E55 | reviewed semantic replacement |
| B0039 | `src-tauri/src/media.rs` | `@@ -665 +840 @@ pub(crate) fn import_one(` | `bb8a19a5f913b2b1` | superseded-by-reviewed-fix | E55 | reviewed semantic replacement |
| B0040 | `src-tauri/src/media.rs` | `@@ -669 +844,59 @@ pub(crate) fn import_one(` | `62cecdf610fdf1f7` | superseded-by-reviewed-fix | E55 | reviewed semantic replacement |
| B0041 | `src-tauri/src/media.rs` | `@@ -671,3 +904 @@ pub(crate) fn import_one(` | `2435a60250607c9f` | superseded-by-reviewed-fix | E55 | reviewed semantic replacement |
| B0042 | `src-tauri/src/media.rs` | `@@ -685 +916,26 @@ fn warm_import_poster(engine: &MediaEngine, entry: &MediaManifestEntry, path: &P` | `fb8cae022411d332` | superseded-by-reviewed-fix | E55 | reviewed semantic replacement |
| B0043 | `src-tauri/src/media.rs` | `@@ -1126,0 +1383 @@ pub fn generate_thumbnail(` | `ffd3a0237dd898e4` | superseded-by-reviewed-fix | E55 | reviewed semantic replacement |
| B0044 | `src-tauri/src/media.rs` | `@@ -1141,0 +1399 @@ pub fn generate_thumbnail(` | `cbf0ec4c3a91abdf` | superseded-by-reviewed-fix | E55 | reviewed semantic replacement |
| B0045 | `src-tauri/src/media.rs` | `@@ -1202,0 +1461 @@ pub fn get_waveform(` | `ffd3a0237dd898e4` | superseded-by-reviewed-fix | E55 | reviewed semantic replacement |
| B0046 | `src-tauri/src/media.rs` | `@@ -1216,0 +1476,4 @@ pub fn get_waveform(` | `680484c835ee8687` | superseded-by-reviewed-fix | E55 | reviewed semantic replacement |
| B0047 | `src-tauri/src/media.rs` | `@@ -1228,12 +1491,4 @@ pub fn get_waveform(` | `1f5d364c17847494` | superseded-by-reviewed-fix | E55 | reviewed semantic replacement |
| B0048 | `src-tauri/src/media.rs` | `@@ -1244,0 +1500 @@ pub fn preload_media(` | `11ae269fbbd81a8f` | superseded-by-reviewed-fix | E55 | reviewed semantic replacement |
| B0049 | `src-tauri/src/media.rs` | `@@ -1250,3 +1505,0 @@ pub fn preload_media(` | `9a346bdc6713bd85` | superseded-by-reviewed-fix | E55 | reviewed semantic replacement |
| B0050 | `src-tauri/src/media.rs` | `@@ -1259,3 +1512,5 @@ pub fn preload_media(` | `6c130d5b06cde1ae` | superseded-by-reviewed-fix | E55 | reviewed semantic replacement |
| B0051 | `src-tauri/src/media.rs` | `@@ -1265,0 +1521,21 @@ pub fn preload_media(` | `611eb57524d3bde4` | superseded-by-reviewed-fix | E55 | reviewed semantic replacement |
| B0052 | `src-tauri/src/media.rs` | `@@ -1436,0 +1713,15 @@ mod tests {` | `612123727257ca97` | superseded-by-reviewed-fix | E55 | reviewed semantic replacement |
| B0053 | `src-tauri/src/mpv_bootstrap.rs` | `@@ -1,97 +0,0 @@` | `c0b68ce4da2773d3` | delivered-exact | E04 | 2836a5386328 |
| B0054 | `src-tauri/src/playback/audio.rs` | `@@ -24,2 +24,2 @@ use std::collections::HashMap;` | `bd26b494e71a393f` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0055 | `src-tauri/src/playback/audio.rs` | `@@ -27,0 +28 @@ use std::thread::{self, JoinHandle};` | `5f55094044b75d4c` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0056 | `src-tauri/src/playback/audio.rs` | `@@ -43,0 +45,31 @@ const MIX_CHANNELS: usize = 2;` | `2bc26dfc60b962ec` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0057 | `src-tauri/src/playback/audio.rs` | `@@ -81 +113,2 @@ pub struct AudioPlayback {` | `ea7fad673862d6a5` | delivered-exact | E52 | 6551e6744dbd |
| B0058 | `src-tauri/src/playback/audio.rs` | `@@ -84,0 +118,6 @@ pub struct AudioPlayback {` | `8d6c3431e003c849` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0059 | `src-tauri/src/playback/audio.rs` | `@@ -90,2 +129,6 @@ impl AudioPlayback {` | `c750de9ccf1be5be` | delivered-exact | E52 | 6551e6744dbd |
| B0060 | `src-tauri/src/playback/audio.rs` | `@@ -92,0 +136,3 @@ impl AudioPlayback {` | `f82709b97e4eb548` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0061 | `src-tauri/src/playback/audio.rs` | `@@ -95 +141,10 @@ impl AudioPlayback {` | `63ed11c34792f56b` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0062 | `src-tauri/src/playback/audio.rs` | `@@ -97 +152 @@ impl AudioPlayback {` | `f83fe1e73188d3ba` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0063 | `src-tauri/src/playback/audio.rs` | `@@ -99 +154,2 @@ impl AudioPlayback {` | `0c6c718e79042548` | delivered-exact | E52 | 6551e6744dbd |
| B0064 | `src-tauri/src/playback/audio.rs` | `@@ -106 +162,42 @@ impl AudioPlayback {` | `45827a54a2454c6d` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0065 | `src-tauri/src/playback/audio.rs` | `@@ -113 +210 @@ impl Drop for AudioPlayback {` | `9b5ccd442d652673` | delivered-exact | E52 | 6551e6744dbd |
| B0066 | `src-tauri/src/playback/audio.rs` | `@@ -125 +222,3 @@ fn audio_thread(` | `09db9bfd71cf50d3` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0067 | `src-tauri/src/playback/audio.rs` | `@@ -128 +227 @@ fn audio_thread(` | `5acf9d585881c979` | delivered-exact | E52 | 6551e6744dbd |
| B0068 | `src-tauri/src/playback/audio.rs` | `@@ -129,0 +229,4 @@ fn audio_thread(` | `31098d5f1c760f2b` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0069 | `src-tauri/src/playback/audio.rs` | `@@ -131,2 +234,21 @@ fn audio_thread(` | `68d543f20847fcf9` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0070 | `src-tauri/src/playback/audio.rs` | `@@ -143 +265,5 @@ fn audio_thread(` | `28c4a0d364e6b3fe` | delivered-exact | E52 | 6551e6744dbd |
| B0071 | `src-tauri/src/playback/audio.rs` | `@@ -153,2 +279,11 @@ fn build_and_play(buffer: &Arc<Vec<f32>>, pos: &Arc<AtomicU64>) -> Result<cpal::` | `c4c08b1b97b83e25` | delivered-exact | E52 | 6551e6744dbd |
| B0072 | `src-tauri/src/playback/audio.rs` | `@@ -164,0 +300 @@ fn build_stream(` | `5f49e719803195a5` | delivered-exact | E52 | 6551e6744dbd |
| B0073 | `src-tauri/src/playback/audio.rs` | `@@ -170,10 +306,10 @@ fn build_stream(` | `4765f285e315a592` | delivered-exact | E52 | 6551e6744dbd |
| B0074 | `src-tauri/src/playback/audio.rs` | `@@ -208,0 +345 @@ fn out_stream<T>(` | `5f49e719803195a5` | delivered-exact | E52 | 6551e6744dbd |
| B0075 | `src-tauri/src/playback/audio.rs` | `@@ -219,0 +357,6 @@ where` | `358ed7a1fbbcd74d` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0076 | `src-tauri/src/playback/audio.rs` | `@@ -223 +365,0 @@ where` | `7d05c3883cf08c67` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0077 | `src-tauri/src/playback/audio.rs` | `@@ -239,0 +382,7 @@ where` | `f50d1c1833478ee3` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0078 | `src-tauri/src/playback/audio.rs` | `@@ -380 +529 @@ fn mix_timeline_stereo(` | `1e5f030fc3c540a9` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0079 | `src-tauri/src/playback/audio.rs` | `@@ -384,0 +534 @@ pub fn build_clock(` | `b9a212962d5b9cfc` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0080 | `src-tauri/src/playback/audio.rs` | `@@ -393,0 +544 @@ pub fn build_clock(` | `4aa4498c1ccd8450` | delivered-exact | E52 | 6551e6744dbd |
| B0081 | `src-tauri/src/playback/audio.rs` | `@@ -401 +552 @@ pub fn build_clock(` | `83f8341ba95b4f07` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0082 | `src-tauri/src/playback/audio.rs` | `@@ -409,0 +561,21 @@ pub fn build_clock(` | `0fc58b7b8a8019e7` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0083 | `src-tauri/src/playback/audio.rs` | `@@ -413,0 +586,30 @@ mod tests {` | `a5a6d341c49e39f0` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0084 | `src-tauri/src/playback/commands.rs` | `@@ -14,0 +15 @@ use std::sync::{Arc, Mutex};` | `392e88a746a5e8e7` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0085 | `src-tauri/src/playback/commands.rs` | `@@ -21 +22 @@ use opentake_render::{even, RenderSize};` | `8a44c770956e43c6` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0086 | `src-tauri/src/playback/commands.rs` | `@@ -28,0 +30 @@ const PLAYBACK_PREVIEW_CAP: u32 = 1280;` | `c72cf4ff64b5c35e` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0087 | `src-tauri/src/playback/commands.rs` | `@@ -32 +34 @@ const PLAYBACK_PREVIEW_CAP: u32 = 1280;` | `82c31b3feb0eb3a3` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0088 | `src-tauri/src/playback/commands.rs` | `@@ -35 +37,89 @@ struct RunningPlayback {` | `eb58482f3e7367ec` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0089 | `src-tauri/src/playback/commands.rs` | `@@ -42 +132 @@ pub struct PlaybackState {` | `e84b852357128100` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0090 | `src-tauri/src/playback/commands.rs` | `@@ -50,12 +140,42 @@ impl PlaybackState {` | `b8d1ef4ddc6a4940` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0091 | `src-tauri/src/playback/commands.rs` | `@@ -64 +184 @@ impl PlaybackState {` | `0c6467764e4bf032` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0092 | `src-tauri/src/playback/commands.rs` | `@@ -65,0 +186,79 @@ impl PlaybackState {` | `b3d75317fa51fa7c` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0093 | `src-tauri/src/playback/commands.rs` | `@@ -71,2 +270,3 @@ impl PlaybackState {` | `ab2a4ce8b484bcd2` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0094 | `src-tauri/src/playback/commands.rs` | `@@ -75 +275 @@ impl PlaybackState {` | `0c6467764e4bf032` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0095 | `src-tauri/src/playback/commands.rs` | `@@ -81,2 +281,2 @@ impl PlaybackState {` | `8a61923e073d2e69` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0096 | `src-tauri/src/playback/commands.rs` | `@@ -112 +312,16 @@ fn playback_render_size(canvas_w: i32, canvas_h: i32, cap: u32) -> RenderSize {` | `1d53277d5b7a6073` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0097 | `src-tauri/src/playback/commands.rs` | `@@ -115 +330 @@ pub async fn playback_start(app: AppHandle, from_frame: i32) -> Result<(), Strin` | `fdc8e5b1365a40b2` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0098 | `src-tauri/src/playback/commands.rs` | `@@ -117 +332,2 @@ pub async fn playback_start(app: AppHandle, from_frame: i32) -> Result<(), Strin` | `4d248b38a248e771` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0099 | `src-tauri/src/playback/commands.rs` | `@@ -128,0 +345 @@ pub async fn playback_start(app: AppHandle, from_frame: i32) -> Result<(), Strin` | `f9c5df0a234d24ae` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0100 | `src-tauri/src/playback/commands.rs` | `@@ -138 +354,0 @@ pub async fn playback_start(app: AppHandle, from_frame: i32) -> Result<(), Strin` | `d1359fbed7a49b70` | delivered-exact | E52 | 6551e6744dbd |
| B0101 | `src-tauri/src/playback/commands.rs` | `@@ -146,3 +362,9 @@ pub async fn playback_start(app: AppHandle, from_frame: i32) -> Result<(), Strin` | `bfc85ab079f7e505` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0102 | `src-tauri/src/playback/commands.rs` | `@@ -151,9 +373,21 @@ pub async fn playback_start(app: AppHandle, from_frame: i32) -> Result<(), Strin` | `3d60bde2204665eb` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0103 | `src-tauri/src/playback/commands.rs` | `@@ -161 +395,3 @@ pub async fn playback_start(app: AppHandle, from_frame: i32) -> Result<(), Strin` | `22ed525d9c254d7d` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0104 | `src-tauri/src/playback/commands.rs` | `@@ -165,2 +401 @@ pub async fn playback_start(app: AppHandle, from_frame: i32) -> Result<(), Strin` | `786660bc4780a9a3` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0105 | `src-tauri/src/playback/commands.rs` | `@@ -168,3 +403,2 @@ pub async fn playback_start(app: AppHandle, from_frame: i32) -> Result<(), Strin` | `f5d65438616ecf9c` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0106 | `src-tauri/src/playback/commands.rs` | `@@ -187,3 +421,4 @@ pub fn playback_seek(playback: State<'_, PlaybackState>, frame: i32) -> Result<(` | `1a834eebc4985e7e` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0107 | `src-tauri/src/playback/commands.rs` | `@@ -198,0 +434,38 @@ mod tests {` | `82d7915d3a5d7278` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0108 | `src-tauri/src/playback/engine.rs` | `@@ -18 +18,2 @@ use std::collections::HashMap;` | `a33807472a041fb6` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0109 | `src-tauri/src/playback/engine.rs` | `@@ -52,0 +54,36 @@ pub trait PlayheadEmitter: Send + Sync {` | `6351bf9cf38ecb99` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0110 | `src-tauri/src/playback/engine.rs` | `@@ -54,0 +92,4 @@ pub enum PlaybackCmd {` | `3a51b5f735305bba` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0111 | `src-tauri/src/playback/engine.rs` | `@@ -60,0 +102,29 @@ pub enum PlaybackCmd {` | `0db5707e3744ce17` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0112 | `src-tauri/src/playback/engine.rs` | `@@ -77,0 +148,13 @@ fn loop_step(clock_frame: i32, total: i32) -> (i32, bool) {` | `f621a4c1681107a4` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0113 | `src-tauri/src/playback/engine.rs` | `@@ -119,0 +203 @@ pub struct RenderLoop {` | `7c19683f724a8d67` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0114 | `src-tauri/src/playback/engine.rs` | `@@ -150,0 +235 @@ impl RenderLoop {` | `c80b9f37974bf0f7` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0115 | `src-tauri/src/playback/engine.rs` | `@@ -167,0 +253 @@ impl RenderLoop {` | `df90c6398508b20b` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0116 | `src-tauri/src/playback/engine.rs` | `@@ -182,0 +269,12 @@ impl RenderLoop {` | `a91a7f6272233023` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0117 | `src-tauri/src/playback/engine.rs` | `@@ -190,0 +289 @@ pub struct PlaybackEngine {` | `22ab057f26febf99` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0118 | `src-tauri/src/playback/engine.rs` | `@@ -206,0 +306,72 @@ impl PlaybackEngine {` | `93eee0cdfe5223f1` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0119 | `src-tauri/src/playback/engine.rs` | `@@ -208,0 +380,2 @@ impl PlaybackEngine {` | `ddd671b8e719a435` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0120 | `src-tauri/src/playback/engine.rs` | `@@ -220,0 +394 @@ impl PlaybackEngine {` | `9e8c4e8cb6dd5dbe` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0121 | `src-tauri/src/playback/engine.rs` | `@@ -221,0 +396,2 @@ impl PlaybackEngine {` | `ee2f6c2ad848fa3e` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0122 | `src-tauri/src/playback/engine.rs` | `@@ -227,0 +404 @@ impl PlaybackEngine {` | `714f274cfc6457a0` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0123 | `src-tauri/src/playback/engine.rs` | `@@ -235,0 +413,44 @@ impl PlaybackEngine {` | `8757d33c25df34a4` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0124 | `src-tauri/src/playback/engine.rs` | `@@ -237,0 +459 @@ impl PlaybackEngine {` | `ee7cd071ce9dbca6` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0125 | `src-tauri/src/playback/engine.rs` | `@@ -247,0 +470 @@ impl Drop for PlaybackEngine {` | `ee7cd071ce9dbca6` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0126 | `src-tauri/src/playback/engine.rs` | `@@ -266,0 +490 @@ fn run_render_thread(` | `22ab057f26febf99` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0127 | `src-tauri/src/playback/engine.rs` | `@@ -267,0 +492,2 @@ fn run_render_thread(` | `8dbfe22eadbc7217` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0128 | `src-tauri/src/playback/engine.rs` | `@@ -271,0 +498,3 @@ fn run_render_thread(` | `0865edbc23f12e18` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0129 | `src-tauri/src/playback/engine.rs` | `@@ -278,0 +508,3 @@ fn run_render_thread(` | `6ae025ec8c0db5f1` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0130 | `src-tauri/src/playback/engine.rs` | `@@ -280,0 +513,3 @@ fn run_render_thread(` | `bae02e281de8e19f` | delivered-exact | E52 | 6551e6744dbd |
| B0131 | `src-tauri/src/playback/engine.rs` | `@@ -281,0 +517,2 @@ fn run_render_thread(` | `2d60deb2660a32db` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0132 | `src-tauri/src/playback/engine.rs` | `@@ -283,0 +521,35 @@ fn run_render_thread(` | `ac5a7f890d1bf8c8` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0133 | `src-tauri/src/playback/engine.rs` | `@@ -288,0 +561,15 @@ fn run_render_thread(` | `d9b78b39be3a3ed7` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0134 | `src-tauri/src/playback/engine.rs` | `@@ -298,0 +586,4 @@ fn run_render_thread(` | `3ee4217d581bc008` | delivered-exact | E52 | 6551e6744dbd |
| B0135 | `src-tauri/src/playback/engine.rs` | `@@ -300 +591,2 @@ fn run_render_thread(` | `ed609082aed0d4bc` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0136 | `src-tauri/src/playback/engine.rs` | `@@ -302,2 +594,15 @@ fn run_render_thread(` | `b2755f976819eba4` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0137 | `src-tauri/src/playback/engine.rs` | `@@ -305,2 +610,9 @@ fn run_render_thread(` | `fcd79d113b7e53f1` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0138 | `src-tauri/src/playback/engine.rs` | `@@ -308,2 +620,5 @@ fn run_render_thread(` | `4def782c7b679394` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0139 | `src-tauri/src/playback/engine.rs` | `@@ -329,0 +645,43 @@ mod tests {` | `ab5063b34746c325` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0140 | `src-tauri/src/playback/engine.rs` | `@@ -353,0 +712,7 @@ mod tests {` | `58c98c97c4333f63` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0141 | `src-tauri/src/playback/engine.rs` | `@@ -371,0 +737,24 @@ mod tests {` | `0f597d507069628f` | superseded-by-reviewed-fix | E53 | reviewed semantic replacement |
| B0142 | `src-tauri/src/playback/resolver.rs` | `@@ -28,0 +29 @@ use std::collections::{HashMap, HashSet};` | `e646ce4e21a5d9b1` | superseded-by-reviewed-fix | E54 | reviewed semantic replacement |
| B0143 | `src-tauri/src/playback/resolver.rs` | `@@ -256,0 +258,5 @@ impl<'d, 's> StreamingResolver<'d, 's> {` | `4527ba1448bb660d` | superseded-by-reviewed-fix | E54 | reviewed semantic replacement |
| B0144 | `src-tauri/src/playback/resolver.rs` | `@@ -258,3 +264,2 @@ impl<'d, 's> StreamingResolver<'d, 's> {` | `662baef601991357` | superseded-by-reviewed-fix | E54 | reviewed semantic replacement |
| B0145 | `src-tauri/src/playback/resolver.rs` | `@@ -271,0 +277,12 @@ impl<'d, 's> StreamingResolver<'d, 's> {` | `23af3645c45351ed` | superseded-by-reviewed-fix | E54 | reviewed semantic replacement |
| B0146 | `src-tauri/src/playback/resolver.rs` | `@@ -336,0 +354,26 @@ impl<'d, 's> StreamingResolver<'d, 's> {` | `6ec4baf9bb348b3d` | superseded-by-reviewed-fix | E54 | reviewed semantic replacement |
| B0147 | `src-tauri/src/playback/transport.rs` | `@@ -136,7 +136 @@ fn origin_is_allowed(headers: &HeaderMap) -> bool {` | `5584c21aa0ad3b7b` | delivered-exact | E56 | 3fe09766819b |
| B0148 | `src-tauri/src/playback/transport.rs` | `@@ -147,0 +142,25 @@ fn origin_is_allowed(headers: &HeaderMap) -> bool {` | `5b58bd874f45ef74` | superseded-by-reviewed-fix | E56 | reviewed semantic replacement |
| B0149 | `src-tauri/src/playback/transport.rs` | `@@ -356,0 +376 @@ mod tests {` | `236bf7971b262909` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0150 | `src-tauri/src/playback/transport.rs` | `@@ -358,0 +379 @@ mod tests {` | `22943a08a5172be4` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0151 | `src-tauri/src/playback/transport.rs` | `@@ -371,6 +392,14 @@ mod tests {` | `0ae8b03e2ab5dfac` | superseded-by-reviewed-fix | E56 | reviewed semantic replacement |
| B0152 | `src-tauri/tauri.conf.json` | `@@ -59,4 +59 @@` | `10ef7c2cda6c5ec9` | delivered-exact | E04 | 2836a5386328 |
| B0153 | `src-tauri/tests/playback_integration.rs` | `@@ -17 +17 @@ use std::sync::atomic::{AtomicI32, AtomicUsize, Ordering};` | `000de44a8c0235e4` | superseded-by-reviewed-fix | E54 | reviewed semantic replacement |
| B0154 | `src-tauri/tests/playback_integration.rs` | `@@ -108,2 +108,7 @@ fn try_render_loop(` | `0288988c551f355a` | superseded-by-reviewed-fix | E54 | reviewed semantic replacement |
| B0155 | `src-tauri/tests/playback_integration.rs` | `@@ -115 +120 @@ fn render_until_content(rl: &mut RenderLoop, target: i32, w: u32, h: u32) -> Opt` | `5b7a91146b3e4aeb` | delivered-exact | E52 | 6551e6744dbd |
| B0156 | `src-tauri/tests/playback_integration.rs` | `@@ -156,3 +161,11 @@ fn render_loop_streams_frames_advances_and_seeks() {` | `81bb5d6653c367ca` | superseded-by-reviewed-fix | E54 | reviewed semantic replacement |
| B0157 | `src-tauri/tests/playback_integration.rs` | `@@ -164 +177 @@ fn render_loop_streams_frames_advances_and_seeks() {` | `4acc3b8cd9bf9819` | delivered-exact | E52 | 6551e6744dbd |
| B0158 | `src-tauri/tests/playback_integration.rs` | `@@ -224 +237 @@ fn render_loop_composites_two_tracks_concurrently() {` | `c8c32345956afe34` | delivered-exact | E52 | 6551e6744dbd |
| B0159 | `src-tauri/tests/playback_integration.rs` | `@@ -267,0 +281 @@ fn playback_engine_thread_streams_frames_to_sink_and_emitter() {` | `337c7251915401ff` | delivered-exact | E52 | 6551e6744dbd |
| B0160 | `src-tauri/tests/playback_integration.rs` | `@@ -270 +284,4 @@ fn playback_engine_thread_streams_frames_to_sink_and_emitter() {` | `5da19863241d1bbf` | delivered-exact | E52 | 6551e6744dbd |
| B0161 | `src-tauri/tests/playback_integration.rs` | `@@ -272,2 +289,6 @@ fn playback_engine_thread_streams_frames_to_sink_and_emitter() {` | `229e7119983aa2b4` | superseded-by-reviewed-fix | E54 | reviewed semantic replacement |
| B0162 | `src-tauri/tests/playback_integration.rs` | `@@ -284 +305,4 @@ fn playback_engine_thread_streams_frames_to_sink_and_emitter() {` | `7fe904cd67cb722d` | delivered-exact | E52 | 6551e6744dbd |
| B0163 | `src-tauri/tests/playback_integration.rs` | `@@ -287,17 +311,11 @@ fn playback_engine_thread_streams_frames_to_sink_and_emitter() {` | `186c82355d9617ab` | superseded-by-reviewed-fix | E54 | reviewed semantic replacement |
| B0164 | `src-tauri/tests/playback_integration.rs` | `@@ -306,0 +325,10 @@ fn playback_engine_thread_streams_frames_to_sink_and_emitter() {` | `d159939b7864f161` | superseded-by-reviewed-fix | E54 | reviewed semantic replacement |
| B0165 | `src-tauri/tests/playback_transport_integration.rs` | `@@ -15 +15,2 @@ use std::time::Duration;` | `b325aa953bc325fc` | superseded-by-reviewed-fix | E56 | reviewed semantic replacement |
| B0166 | `src-tauri/tests/playback_transport_integration.rs` | `@@ -17 +18 @@ use opentake_tauri_lib::playback::PreviewServer;` | `1d3d41ad5a3a35be` | superseded-by-reviewed-fix | E56 | reviewed semantic replacement |
| B0167 | `src-tauri/tests/playback_transport_integration.rs` | `@@ -61 +62 @@ fn start_server() -> Option<std::sync::Arc<PreviewServer>> {` | `e971c8baa1c37a24` | superseded-by-reviewed-fix | E56 | reviewed semantic replacement |
| B0168 | `src-tauri/tests/playback_transport_integration.rs` | `@@ -64 +65 @@ fn get(port: u16, extra_headers: &str) -> String {` | `fb5a8c47dab7e492` | superseded-by-reviewed-fix | E56 | reviewed semantic replacement |
| B0169 | `src-tauri/tests/playback_transport_integration.rs` | `@@ -69,0 +71,4 @@ fn get(port: u16, extra_headers: &str) -> String {` | `0c6ceb9fbc3da508` | superseded-by-reviewed-fix | E56 | reviewed semantic replacement |
| B0170 | `src-tauri/tests/playback_transport_integration.rs` | `@@ -100,0 +106,21 @@ fn stream_route_rejects_cross_origin() {` | `2a033e57c59c6d73` | superseded-by-reviewed-fix | E56 | reviewed semantic replacement |
| B0171 | `web/package.json` | `@@ -18 +17,0 @@` | `113f6df61cb8f187` | delivered-exact | E04 | 2836a5386328 |
| B0172 | `web/pnpm-lock.yaml` | `@@ -26,3 +25,0 @@ importers:` | `7933c3c3d36398f0` | delivered-exact | E04 | 2836a5386328 |
| B0173 | `web/pnpm-lock.yaml` | `@@ -57,5 +53,0 @@ packages:` | `d33e1f2dabf79b58` | delivered-exact | E04 | 2836a5386328 |
| B0174 | `web/pnpm-lock.yaml` | `@@ -187,3 +178,0 @@ packages:` | `0c0e773aab14dc7f` | delivered-exact | E04 | 2836a5386328 |
| B0175 | `web/pnpm-lock.yaml` | `@@ -264 +252,0 @@ packages:` | `2f1c00ef167a8b98` | superseded-by-reviewed-fix | E04 | reviewed semantic replacement |
| B0176 | `web/pnpm-lock.yaml` | `@@ -455 +442,0 @@ packages:` | `2f1c00ef167a8b98` | superseded-by-reviewed-fix | E04 | reviewed semantic replacement |
| B0177 | `web/pnpm-lock.yaml` | `@@ -468 +454,0 @@ packages:` | `2f1c00ef167a8b98` | superseded-by-reviewed-fix | E04 | reviewed semantic replacement |
| B0178 | `web/pnpm-lock.yaml` | `@@ -497,4 +482,0 @@ packages:` | `a8928b849eb15339` | delivered-exact | E04 | 2836a5386328 |
| B0179 | `web/pnpm-lock.yaml` | `@@ -504 +485,0 @@ packages:` | `2f1c00ef167a8b98` | superseded-by-reviewed-fix | E04 | reviewed semantic replacement |
| B0180 | `web/pnpm-lock.yaml` | `@@ -522,5 +502,0 @@ packages:` | `618e23e433c2b0af` | delivered-exact | E04 | 2836a5386328 |
| B0181 | `web/pnpm-lock.yaml` | `@@ -548 +523,0 @@ packages:` | `2f1c00ef167a8b98` | superseded-by-reviewed-fix | E04 | reviewed semantic replacement |
| B0182 | `web/pnpm-lock.yaml` | `@@ -553 +527,0 @@ packages:` | `2f1c00ef167a8b98` | superseded-by-reviewed-fix | E04 | reviewed semantic replacement |
| B0183 | `web/pnpm-lock.yaml` | `@@ -596 +569,0 @@ packages:` | `2f1c00ef167a8b98` | superseded-by-reviewed-fix | E04 | reviewed semantic replacement |
| B0184 | `web/pnpm-lock.yaml` | `@@ -637 +609,0 @@ packages:` | `2f1c00ef167a8b98` | superseded-by-reviewed-fix | E04 | reviewed semantic replacement |
| B0185 | `web/pnpm-lock.yaml` | `@@ -659,4 +630,0 @@ snapshots:` | `474ada170becd9f2` | delivered-exact | E04 | 2836a5386328 |
| B0186 | `web/pnpm-lock.yaml` | `@@ -745,2 +712,0 @@ snapshots:` | `7fceac35fffc87ab` | delivered-exact | E04 | 2836a5386328 |
| B0187 | `web/pnpm-lock.yaml` | `@@ -983,2 +948,0 @@ snapshots:` | `c7011128f7efc6b6` | delivered-exact | E04 | 2836a5386328 |
| B0188 | `web/pnpm-lock.yaml` | `@@ -1018,5 +981,0 @@ snapshots:` | `1910a899360efe5a` | delivered-exact | E04 | 2836a5386328 |
| B0189 | `web/src/App.tsx` | `@@ -101,4 +101 @@ export default function App() {` | `4d9469181cf2ff23` | delivered-exact | E04 | 2836a5386328 |
| B0190 | `web/src/components/home/HomeView.tsx` | `@@ -315,0 +316 @@ function ProjectLauncher({ recents }: { recents: RecentProject[] }) {` | `acfe7136811f4fd5` | delivered-exact | E63 | bf4e3a7dc670 |
| B0191 | `web/src/components/home/HomeView.tsx` | `@@ -343,0 +345,2 @@ function ProjectLauncher({ recents }: { recents: RecentProject[] }) {` | `d20c35a0d6f8c5b4` | delivered-exact | E63 | bf4e3a7dc670 |
| B0192 | `web/src/components/home/HomeView.tsx` | `@@ -450,0 +454 @@ function ProjectGridCard({` | `acfe7136811f4fd5` | delivered-exact | E63 | bf4e3a7dc670 |
| B0193 | `web/src/components/home/HomeView.tsx` | `@@ -461 +465,10 @@ function ProjectGridCard({` | `2dec9fd0959ef532` | delivered-exact | E63 | bf4e3a7dc670 |
| B0194 | `web/src/components/home/HomeView.tsx` | `@@ -477 +490 @@ function ProjectGridCard({` | `7b526b9f771c6dd4` | delivered-exact | E63 | bf4e3a7dc670 |
| B0195 | `web/src/components/home/HomeView.visual.test.ts` | `@@ -70,0 +71,7 @@ describe("HomeView Vercel embedded visual direction", () => {` | `3e3c2a87878421ed` | superseded-by-reviewed-fix | E63 | reviewed semantic replacement |
| B0196 | `web/src/components/media/MediaPanel.tsx` | `@@ -790 +790 @@ function MediaCard({ item }: { item: MediaItem }) {` | `2685e211200e9e1a` | superseded-by-reviewed-fix | E63 | reviewed semantic replacement |
| B0197 | `web/src/components/media/MediaSearch.tsx` | `@@ -37,0 +38 @@ import {` | `07854935424bba57` | delivered-exact | E63 | bf4e3a7dc670 |
| B0198 | `web/src/components/media/MediaSearch.tsx` | `@@ -438,0 +440 @@ function MomentCard({ hit }: { hit: MomentHit }) {` | `53c99373f12470c0` | delivered-exact | E63 | bf4e3a7dc670 |
| B0199 | `web/src/components/media/MediaSearch.tsx` | `@@ -456 +458,4 @@ function MomentCard({ hit }: { hit: MomentHit }) {` | `cfc497d71c927645` | delivered-exact | E63 | bf4e3a7dc670 |
| B0200 | `web/src/components/media/MediaSearch.tsx` | `@@ -493,0 +499 @@ function SpokenRow({ hit }: { hit: SpokenHit }) {` | `53c99373f12470c0` | delivered-exact | E63 | bf4e3a7dc670 |
| B0201 | `web/src/components/media/MediaSearch.tsx` | `@@ -506 +512,4 @@ function SpokenRow({ hit }: { hit: SpokenHit }) {` | `cfc497d71c927645` | delivered-exact | E63 | bf4e3a7dc670 |
| B0202 | `web/src/components/media/MediaSearch.tsx` | `@@ -557,0 +567 @@ function FileCard({ item }: { item: MediaItem }) {` | `53c99373f12470c0` | delivered-exact | E63 | bf4e3a7dc670 |
| B0203 | `web/src/components/media/MediaSearch.tsx` | `@@ -567 +577,4 @@ function FileCard({ item }: { item: MediaItem }) {` | `cfc497d71c927645` | delivered-exact | E63 | bf4e3a7dc670 |
| B0204 | `web/src/components/preview/Preview.test.tsx` | `@@ -62 +62 @@ vi.mock("../../lib/asset", () => ({` | `1a31d396b5cbee2c` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0205 | `web/src/components/preview/Preview.test.tsx` | `@@ -128 +128,12 @@ describe("Preview timeline rendering", () => {` | `a83ed245460edb8c` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0206 | `web/src/components/preview/Preview.test.tsx` | `@@ -164,0 +176,24 @@ describe("Preview timeline rendering", () => {` | `65b0ed645758bec2` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0207 | `web/src/components/preview/Preview.tsx` | `@@ -38,0 +39,2 @@ import {` | `e33f82140de4c7d0` | delivered-exact | E62 | 716a09c78543; contrib 6551e6744dbd |
| B0208 | `web/src/components/preview/Preview.tsx` | `@@ -39,0 +42 @@ import {` | `829107a12e6aab81` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0209 | `web/src/components/preview/Preview.tsx` | `@@ -43,2 +46,10 @@ import { rustEngineEnabled } from "./rustEngine";` | `86ed766d8c5b6072` | superseded-by-reviewed-fix | E04 | reviewed semantic replacement |
| B0210 | `web/src/components/preview/Preview.tsx` | `@@ -55,0 +67 @@ import {` | `95ed071d5c388dca` | delivered-exact | E62 | 716a09c78543 |
| B0211 | `web/src/components/preview/Preview.tsx` | `@@ -62,0 +75,21 @@ import type { MediaItem } from "../../lib/types";` | `388591be784fc23c` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0212 | `web/src/components/preview/Preview.tsx` | `@@ -65,0 +99 @@ export function Preview() {` | `52373f9d57431b82` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0213 | `web/src/components/preview/Preview.tsx` | `@@ -70,0 +105 @@ export function Preview() {` | `cabce5f41bdaaf81` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0214 | `web/src/components/preview/Preview.tsx` | `@@ -81,0 +117 @@ export function Preview() {` | `c42a04771a984ce3` | delivered-exact | E62 | 716a09c78543 |
| B0215 | `web/src/components/preview/Preview.tsx` | `@@ -135,0 +172,12 @@ export function Preview() {` | `ecacaabcdb813baf` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0216 | `web/src/components/preview/Preview.tsx` | `@@ -184,0 +233 @@ export function Preview() {` | `cde1fce3b419f04c` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0217 | `web/src/components/preview/Preview.tsx` | `@@ -228 +277 @@ export function Preview() {` | `d415135cc14e06f1` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0218 | `web/src/components/preview/Preview.tsx` | `@@ -266,5 +315,4 @@ export function Preview() {` | `de1e3ca81b90305b` | superseded-by-reviewed-fix | E04 | reviewed semantic replacement |
| B0219 | `web/src/components/preview/Preview.tsx` | `@@ -271,0 +320,6 @@ export function Preview() {` | `bc23073c2e1fe431` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0220 | `web/src/components/preview/Preview.tsx` | `@@ -273 +327 @@ export function Preview() {` | `dd998b12a1f6ef19` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0221 | `web/src/components/preview/Preview.tsx` | `@@ -280,2 +333,0 @@ export function Preview() {` | `daf5824853f42d34` | superseded-by-reviewed-fix | E04 | reviewed semantic replacement |
| B0222 | `web/src/components/preview/Preview.tsx` | `@@ -283,15 +335,12 @@ export function Preview() {` | `5d935e4ac53f640a` | superseded-by-reviewed-fix | E04 | reviewed semantic replacement |
| B0223 | `web/src/components/preview/Preview.tsx` | `@@ -299,4 +348,70 @@ export function Preview() {` | `7cd38f3e5cc372a1` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0224 | `web/src/components/preview/Preview.tsx` | `@@ -304,2 +419,2 @@ export function Preview() {` | `e749b1532c3f75cd` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0225 | `web/src/components/preview/Preview.tsx` | `@@ -307 +422,13 @@ export function Preview() {` | `ea812d40b9a6846a` | superseded-by-reviewed-fix | E04 | reviewed semantic replacement |
| B0226 | `web/src/components/preview/Preview.tsx` | `@@ -325,0 +453,31 @@ export function Preview() {` | `fd469c9bfcbe0709` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0227 | `web/src/components/preview/Preview.tsx` | `@@ -374 +532 @@ export function Preview() {` | `9f686e36c98d424a` | delivered-exact | E04 | 2836a5386328 |
| B0228 | `web/src/components/preview/Preview.tsx` | `@@ -397 +554,0 @@ export function Preview() {` | `3d3a3b23b5ce7879` | delivered-exact | E04 | 2836a5386328 |
| B0229 | `web/src/components/preview/Preview.tsx` | `@@ -406 +563,4 @@ export function Preview() {` | `92c1f999477b2773` | superseded-by-reviewed-fix | E04 | reviewed semantic replacement |
| B0230 | `web/src/components/preview/Preview.tsx` | `@@ -410,0 +571,43 @@ export function Preview() {` | `ac6a2ebd528d66e4` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0231 | `web/src/components/preview/TimelinePlaybackLayer.tsx` | `@@ -22 +21,0 @@ import {` | `887ec48b3633339b` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0232 | `web/src/components/preview/TimelinePlaybackLayer.tsx` | `@@ -23,0 +23 @@ import {` | `88f1404842f571c7` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0233 | `web/src/components/preview/TimelinePlaybackLayer.tsx` | `@@ -36 +36 @@ export function TimelinePlayback({ timeline, fps }: { timeline: Timeline; fps: n` | `1eb56431892cb23b` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0234 | `web/src/components/preview/TimelinePlaybackLayer.tsx` | `@@ -70 +70 @@ export function TimelinePlayback({ timeline, fps }: { timeline: Timeline; fps: n` | `a770c1e3d82ea7de` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0235 | `web/src/components/preview/nativePlaybackSession.test.ts` | `@@ -0,0 +1,101 @@` | `96b44587c13398f2` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0236 | `web/src/components/preview/nativePlaybackSession.ts` | `@@ -0,0 +1,46 @@` | `47325b6035e0b5b8` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0237 | `web/src/components/preview/previewEngine.test.ts` | `@@ -3 +3,6 @@ import * as previewEngine from "./previewEngine";` | `dc0a2505a1e899b4` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0238 | `web/src/components/preview/previewEngine.test.ts` | `@@ -104,0 +110,11 @@ describe("pausedSeekToleranceSec", () => {` | `72702296f9115732` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0239 | `web/src/components/preview/previewEngine.test.ts` | `@@ -133,0 +150,90 @@ describe("pausedPlayheadFrameFromFrozenVideo", () => {` | `f03914d7429a6afb` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0240 | `web/src/components/preview/previewEngine.ts` | `@@ -28,0 +29 @@ import {` | `2954b1ed69c55d98` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0241 | `web/src/components/preview/previewEngine.ts` | `@@ -31,0 +33,3 @@ import {` | `6ff6e57c63bfec34` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0242 | `web/src/components/preview/previewEngine.ts` | `@@ -41,3 +45 @@ import {` | `7fa89fdf16a90aed` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0243 | `web/src/components/preview/previewEngine.ts` | `@@ -45,62 +47,16 @@ import {` | `03c03caaad1e11a3` | superseded-by-reviewed-fix | E04 | reviewed semantic replacement |
| B0244 | `web/src/components/preview/previewEngine.ts` | `@@ -108,3 +64,5 @@ function ensureMpv(): Promise<void> {` | `df4cd2c1b6845bde` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0245 | `web/src/components/preview/previewEngine.ts` | `@@ -140,7 +97,0 @@ const SEEK_EPSILON_FRAMES = 2;` | `e410ee5f41516b2b` | delivered-exact | E04 | 2836a5386328 |
| B0246 | `web/src/components/preview/previewEngine.ts` | `@@ -148,0 +100 @@ let interactiveSeekTimer: ReturnType<typeof setTimeout> \| null = null;` | `44435c0d33472d84` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0247 | `web/src/components/preview/previewEngine.ts` | `@@ -153 +105 @@ function activeAt(tl: Timeline, frame: number): ActiveMedia[] {` | `f004734cdf704f66` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0248 | `web/src/components/preview/previewEngine.ts` | `@@ -162 +114 @@ export function activeVideoForPausedSnap(tl: Timeline, frame: number): ActiveMed` | `20e512d74f9ad11a` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0249 | `web/src/components/preview/previewEngine.ts` | `@@ -198,0 +151,49 @@ export function pausedPlayheadFrameFromFrozenVideo(` | `6fda6b623d93b195` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0250 | `web/src/components/preview/previewEngine.ts` | `@@ -230 +231,2 @@ function syncPausedTo(tl: Timeline, frame: number, fps: number): void {` | `c84f7796d3c0192e` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0251 | `web/src/components/preview/previewEngine.ts` | `@@ -234 +236 @@ function syncPausedTo(tl: Timeline, frame: number, fps: number): void {` | `51476eddd765a08c` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0252 | `web/src/components/preview/previewEngine.ts` | `@@ -241 +243,2 @@ function performInteractiveSeek(tl: Timeline, frame: number, fps: number): void` | `c84f7796d3c0192e` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0253 | `web/src/components/preview/previewEngine.ts` | `@@ -246 +249 @@ function performInteractiveSeek(tl: Timeline, frame: number, fps: number): void` | `51476eddd765a08c` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0254 | `web/src/components/preview/previewEngine.ts` | `@@ -304,3 +307,2 @@ export function useTimelinePlaybackEngine(): void {` | `2ae3f6455cc7b457` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0255 | `web/src/components/preview/previewEngine.ts` | `@@ -326 +328 @@ export function useTimelinePlaybackEngine(): void {` | `029b69a186d05d46` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0256 | `web/src/components/preview/previewEngine.ts` | `@@ -363 +365,7 @@ export function useTimelinePlaybackEngine(): void {` | `ab3b8ac576bab98b` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0257 | `web/src/components/preview/previewEngine.ts` | `@@ -365,2 +373,2 @@ export function useTimelinePlaybackEngine(): void {` | `eb6bac365f544209` | superseded-by-reviewed-fix | E04 | reviewed semantic replacement |
| B0258 | `web/src/components/preview/previewEngine.ts` | `@@ -371,2 +379 @@ export function useTimelinePlaybackEngine(): void {` | `66c45e8cb4ffd2d1` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0259 | `web/src/components/preview/previewEngine.ts` | `@@ -374,2 +381,2 @@ export function useTimelinePlaybackEngine(): void {` | `e3aa9b385a5c257e` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0260 | `web/src/components/preview/previewEngine.ts` | `@@ -378,3 +385,3 @@ export function useTimelinePlaybackEngine(): void {` | `6b90de153c134367` | superseded-by-reviewed-fix | E04 | reviewed semantic replacement |
| B0261 | `web/src/components/preview/previewEngine.ts` | `@@ -386,2 +392,0 @@ export function useTimelinePlaybackEngine(): void {` | `f212ba510d5f7fc4` | delivered-exact | E04 | 2836a5386328 |
| B0262 | `web/src/components/preview/previewEngine.ts` | `@@ -389,12 +394,18 @@ export function useTimelinePlaybackEngine(): void {` | `95c28ace6710c171` | superseded-by-reviewed-fix | E04 | reviewed semantic replacement |
| B0263 | `web/src/components/preview/previewEngine.ts` | `@@ -402,33 +413,12 @@ export function useTimelinePlaybackEngine(): void {` | `6ba06b3208a9349f` | superseded-by-reviewed-fix | E04 | reviewed semantic replacement |
| B0264 | `web/src/components/preview/previewEngine.ts` | `@@ -436,2 +426,10 @@ export function useTimelinePlaybackEngine(): void {` | `d1898686d17e1d13` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0265 | `web/src/components/preview/previewEngine.ts` | `@@ -441,4 +439,14 @@ export function useTimelinePlaybackEngine(): void {` | `c4a376d37ab22148` | superseded-by-reviewed-fix | E04 | reviewed semantic replacement |
| B0266 | `web/src/components/preview/previewEngine.ts` | `@@ -446 +454 @@ export function useTimelinePlaybackEngine(): void {` | `3ed55ed771e384f5` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0267 | `web/src/components/preview/previewEngine.ts` | `@@ -451 +459 @@ export function useTimelinePlaybackEngine(): void {` | `1f00548eca703783` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0268 | `web/src/components/preview/previewEngine.ts` | `@@ -489,0 +498,2 @@ export function useTimelinePlaybackEngine(): void {` | `fd79ff845799de14` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0269 | `web/src/components/preview/previewEngine.ts` | `@@ -491,0 +502,8 @@ export function useTimelinePlaybackEngine(): void {` | `9661cc880ab0800f` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0270 | `web/src/components/preview/previewEngine.ts` | `@@ -550,2 +568,8 @@ export function useTimelinePlaybackEngine(): void {` | `89ab16c13769d51e` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0271 | `web/src/components/preview/previewEngine.ts` | `@@ -587 +611 @@ export function useTimelinePlaybackEngine(): void {` | `9e190c082dadec5d` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0272 | `web/src/components/preview/previewEngine.ts` | `@@ -597 +621,7 @@ export function useTimelinePlaybackEngine(): void {` | `ae3454d8798b2f28` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0273 | `web/src/components/preview/previewEngine.ts` | `@@ -608,7 +638 @@ export function useTimelinePlaybackEngine(): void {` | `aeeebd17725b1d21` | superseded-by-reviewed-fix | E04 | reviewed semantic replacement |
| B0274 | `web/src/components/preview/rustEngine.test.ts` | `@@ -2,4 +2,2 @@` | `25e46c1824825ee4` | superseded-by-reviewed-fix | E61 | reviewed semantic replacement |
| B0275 | `web/src/components/preview/rustEngine.test.ts` | `@@ -27 +25 @@ function makeLocalStorage(seed?: Record<string, string>): Storage {` | `003fdd43356c7f2d` | superseded-by-reviewed-fix | E61 | reviewed semantic replacement |
| B0276 | `web/src/components/preview/rustEngine.test.ts` | `@@ -33,2 +31,2 @@ afterEach(() => {` | `cdadf142605617d9` | superseded-by-reviewed-fix | E61 | reviewed semantic replacement |
| B0277 | `web/src/components/preview/rustEngine.test.ts` | `@@ -36 +34,2 @@ describe("rustEngineEnabled (default-on)", () => {` | `487c8384b4e6d0d1` | superseded-by-reviewed-fix | E61 | reviewed semantic replacement |
| B0278 | `web/src/components/preview/rustEngine.test.ts` | `@@ -39 +38 @@ describe("rustEngineEnabled (default-on)", () => {` | `e7068741e39475b9` | superseded-by-reviewed-fix | E61 | reviewed semantic replacement |
| B0279 | `web/src/components/preview/rustEngine.test.ts` | `@@ -41,0 +41 @@ describe("rustEngineEnabled (default-on)", () => {` | `e939796c2d977049` | delivered-exact | E61 | a2f747f04f8e |
| B0280 | `web/src/components/preview/rustEngine.test.ts` | `@@ -44 +44 @@ describe("rustEngineEnabled (default-on)", () => {` | `c715ccc53df88cfa` | superseded-by-reviewed-fix | E61 | reviewed semantic replacement |
| B0281 | `web/src/components/preview/rustEngine.test.ts` | `@@ -49 +49 @@ describe("rustEngineEnabled (default-on)", () => {` | `8e7d404a2fbf677e` | superseded-by-reviewed-fix | E61 | reviewed semantic replacement |
| B0282 | `web/src/components/preview/rustEngine.test.ts` | `@@ -51 +51,2 @@ describe("rustEngineEnabled (default-on)", () => {` | `487c8384b4e6d0d1` | superseded-by-reviewed-fix | E61 | reviewed semantic replacement |
| B0283 | `web/src/components/preview/rustEngine.test.ts` | `@@ -53 +54 @@ describe("rustEngineEnabled (default-on)", () => {` | `ba222b25399b233b` | superseded-by-reviewed-fix | E61 | reviewed semantic replacement |
| B0284 | `web/src/components/preview/rustEngine.test.ts` | `@@ -55 +56 @@ describe("rustEngineEnabled (default-on)", () => {` | `ba222b25399b233b` | superseded-by-reviewed-fix | E61 | reviewed semantic replacement |
| B0285 | `web/src/components/preview/rustEngine.test.ts` | `@@ -58 +59 @@ describe("rustEngineEnabled (default-on)", () => {` | `ead7a8f30e886a3a` | superseded-by-reviewed-fix | E61 | reviewed semantic replacement |
| B0286 | `web/src/components/preview/rustEngine.test.ts` | `@@ -60 +61 @@ describe("rustEngineEnabled (default-on)", () => {` | `ba222b25399b233b` | superseded-by-reviewed-fix | E61 | reviewed semantic replacement |
| B0287 | `web/src/components/preview/rustEngine.test.ts` | `@@ -63 +64 @@ describe("rustEngineEnabled (default-on)", () => {` | `427b8f85486a5a7c` | superseded-by-reviewed-fix | E61 | reviewed semantic replacement |
| B0288 | `web/src/components/preview/rustEngine.test.ts` | `@@ -69 +70 @@ describe("rustEngineEnabled (default-on)", () => {` | `ba222b25399b233b` | superseded-by-reviewed-fix | E61 | reviewed semantic replacement |
| B0289 | `web/src/components/preview/rustEngine.ts` | `@@ -4,7 +4,4 @@` | `2183b8ac326c01d7` | superseded-by-reviewed-fix | E61 | reviewed semantic replacement |
| B0290 | `web/src/components/preview/rustEngine.ts` | `@@ -12,3 +9,4 @@` | `493d93767c3cd31c` | superseded-by-reviewed-fix | E61 | reviewed semantic replacement |
| B0291 | `web/src/components/preview/rustEngine.ts` | `@@ -18,3 +16,3 @@` | `2353efdf66c76a76` | superseded-by-reviewed-fix | E61 | reviewed semantic replacement |
| B0292 | `web/src/components/preview/rustEngine.ts` | `@@ -22,4 +20,4 @@` | `57935f578c019c0b` | superseded-by-reviewed-fix | E61 | reviewed semantic replacement |
| B0293 | `web/src/components/preview/rustEngine.ts` | `@@ -27 +25 @@` | `f6b666e32bd28b1a` | superseded-by-reviewed-fix | E61 | reviewed semantic replacement |
| B0294 | `web/src/components/preview/rustEngine.ts` | `@@ -29 +27 @@ const FLAG_KEY = "opentake.rustEngine";` | `067835498b25ead6` | superseded-by-reviewed-fix | E61 | reviewed semantic replacement |
| B0295 | `web/src/components/preview/rustEngine.ts` | `@@ -31,4 +29,5 @@ export function rustEngineEnabled(): boolean {` | `c45aeb1a5b645b20` | superseded-by-reviewed-fix | E61 | reviewed semantic replacement |
| B0296 | `web/src/components/preview/rustEngine.ts` | `@@ -36,3 +35 @@ export function rustEngineEnabled(): boolean {` | `5bceb165351d33d1` | superseded-by-reviewed-fix | E61 | reviewed semantic replacement |
| B0297 | `web/src/components/preview/timelinePlayback.test.ts` | `@@ -14,0 +15 @@ import {` | `a0593a9dfb21c477` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0298 | `web/src/components/preview/timelinePlayback.test.ts` | `@@ -15,0 +17,3 @@ import {` | `4bdd400b78538b7d` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0299 | `web/src/components/preview/timelinePlayback.test.ts` | `@@ -18,0 +23,3 @@ import {` | `681325c21db272bb` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0300 | `web/src/components/preview/timelinePlayback.test.ts` | `@@ -85,0 +93,174 @@ describe("shouldUseRustEngine", () => {` | `c17828cb4aac51f9` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0301 | `web/src/components/preview/timelinePlayback.test.ts` | `@@ -87,4 +268,2 @@ describe("shouldFallBackToLegacy", () => {` | `6bea4f85b8ff0967` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0302 | `web/src/components/preview/timelinePlayback.test.ts` | `@@ -93,4 +272,2 @@ describe("shouldFallBackToLegacy", () => {` | `e27279b0c8a03c7e` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0303 | `web/src/components/preview/timelinePlayback.test.ts` | `@@ -99 +276,33 @@ describe("shouldFallBackToLegacy", () => {` | `874d8670cbc645a7` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0304 | `web/src/components/preview/timelinePlayback.test.ts` | `@@ -101,2 +310,5 @@ describe("shouldFallBackToLegacy", () => {` | `e8fc3adc1e3030bf` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0305 | `web/src/components/preview/timelinePlayback.test.ts` | `@@ -105 +317,3 @@ describe("shouldFallBackToLegacy", () => {` | `5f0091723b23d763` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0306 | `web/src/components/preview/timelinePlayback.test.ts` | `@@ -107 +321 @@ describe("shouldFallBackToLegacy", () => {` | `1f15df5df50c1f32` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0307 | `web/src/components/preview/timelinePlayback.test.ts` | `@@ -108,0 +323,62 @@ describe("shouldFallBackToLegacy", () => {` | `36740a00f40dd5c4` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0308 | `web/src/components/preview/timelinePlayback.test.ts` | `@@ -175,0 +452,21 @@ describe("playbackFrameFromActiveFrame", () => {` | `73c903c3e3202af4` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0309 | `web/src/components/preview/timelinePlayback.ts` | `@@ -13,0 +14 @@ import { volumeAt } from "../../lib/clip";` | `3aed1e315e11c3c6` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0310 | `web/src/components/preview/timelinePlayback.ts` | `@@ -16 +17 @@ import type { Clip, Timeline, Track } from "../../lib/types";` | `2b42f21a33a9cdee` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0311 | `web/src/components/preview/timelinePlayback.ts` | `@@ -26 +27,4 @@ function clipLastSourceFrame(clip: Clip): number {` | `c9e77eab85563853` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0312 | `web/src/components/preview/timelinePlayback.ts` | `@@ -31 +35 @@ function sourceFrameForTimelineFrame(clip: Clip, frame: number): number {` | `19e8843953368076` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0313 | `web/src/components/preview/timelinePlayback.ts` | `@@ -49,0 +54,77 @@ export function playbackFrameFromActiveFrame(activeFrame: number): number {` | `06e5ab044952351c` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0314 | `web/src/components/preview/timelinePlayback.ts` | `@@ -123 +204 @@ export function frameForSourceTime(clip: Clip, timeSec: number, fps: number): nu` | `0233fe1a38431d3c` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0315 | `web/src/components/preview/timelinePlayback.ts` | `@@ -222,5 +303,3 @@ export function isExternalSeekWhilePlaying(args: {` | `d561bb66eba0000d` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0316 | `web/src/components/preview/timelinePlayback.ts` | `@@ -244,8 +323,3 @@ export function shouldUseRustEngine(args: {` | `f3ac1513f7fc599b` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0317 | `web/src/components/preview/timelinePlayback.ts` | `@@ -254,2 +328 @@ export function shouldFallBackToLegacy(args: {` | `4f90d69fd6612126` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0318 | `web/src/components/preview/timelinePlayback.ts` | `@@ -257 +330,56 @@ export function shouldFallBackToLegacy(args: {` | `a735e7e1945c9637` | superseded-by-reviewed-fix | E62 | reviewed semantic replacement |
| B0319 | `web/src/components/shell/TitleBar.visual.test.ts` | `@@ -21,0 +22,6 @@ describe("TitleBar alignment", () => {` | `4dbabf973708d45f` | superseded-by-reviewed-fix | E63 | reviewed semantic replacement |
| B0320 | `web/src/components/shell/ViewMenu.tsx` | `@@ -10 +10 @@ import { useEffect, useRef, useState } from "react";` | `8d5c31d49f3da6bf` | delivered-exact | E63 | bf4e3a7dc670 |
| B0321 | `web/src/components/shell/ViewMenu.tsx` | `@@ -31,0 +32,2 @@ export function ViewMenu() {` | `63a3fd1b84f64b71` | delivered-exact | E63 | bf4e3a7dc670 |
| B0322 | `web/src/components/shell/ViewMenu.tsx` | `@@ -105,0 +108,7 @@ export function ViewMenu() {` | `29aa1e89d8f2854b` | delivered-exact | E63 | bf4e3a7dc670 |
| B0323 | `web/src/components/timeline/TimelineContainer.test.ts` | `@@ -3,0 +4 @@ import { findSnapDelta } from "../../lib/snap";` | `ad0d12eff93104ba` | delivered-exact | E63 | bf4e3a7dc670 |
| B0324 | `web/src/components/timeline/TimelineContainer.test.ts` | `@@ -206,0 +208,38 @@ describe("volumeKeyframeMenuItems", () => {` | `38fba66199010b06` | superseded-by-reviewed-fix | E63 | reviewed semantic replacement |
| B0325 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -13,0 +14 @@ import {` | `50b54b660f0fc3a0` | delivered-exact | E63 | bf4e3a7dc670 |
| B0326 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -20 +21 @@ import { gapAtFrame } from "../../lib/timelineGap";` | `9b02648510179cc7` | delivered-exact | E63 | bf4e3a7dc670 |
| B0327 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -50 +51 @@ import { forceRefresh } from "../../store/sync";` | `2b489b9dcfaed192` | superseded-by-reviewed-fix | E55 | reviewed semantic replacement |
| B0328 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -170,0 +172,44 @@ export function collectMoveSnapTargets(` | `dda2a04240ac1552` | superseded-by-reviewed-fix | E63 | reviewed semantic replacement |
| B0329 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -379,0 +425,4 @@ export function TimelineContainer() {` | `1aaddd7e8de76186` | superseded-by-reviewed-fix | E63 | reviewed semantic replacement |
| B0330 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -383 +432,4 @@ export function TimelineContainer() {` | `a71acff92653e0a5` | delivered-exact | E63 | bf4e3a7dc670 |
| B0331 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -387,0 +440,21 @@ export function TimelineContainer() {` | `434b1d7aa851948f` | superseded-by-reviewed-fix | E55 | reviewed semantic replacement |
| B0332 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -396,0 +470,4 @@ export function TimelineContainer() {` | `2bc0327c54aa8e3b` | delivered-exact | E63 | bf4e3a7dc670 |
| B0333 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -570,3 +647,20 @@ export function TimelineContainer() {` | `825c263d83476c74` | superseded-by-reviewed-fix | E55 | reviewed semantic replacement |
| B0334 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -576,0 +671 @@ export function TimelineContainer() {` | `8bbaaddaacba9704` | superseded-by-reviewed-fix | E63 | reviewed semantic replacement |
| B0335 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -580,0 +676,2 @@ export function TimelineContainer() {` | `9dfdda5a0b85019e` | superseded-by-reviewed-fix | E63 | reviewed semantic replacement |
| B0336 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -585 +682 @@ export function TimelineContainer() {` | `7bc9cf3e7f82e69a` | superseded-by-reviewed-fix | E55 | reviewed semantic replacement |
| B0337 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -593,0 +691,2 @@ export function TimelineContainer() {` | `a82915ad8ef9562d` | superseded-by-reviewed-fix | E55 | reviewed semantic replacement |
| B0338 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -601 +700 @@ export function TimelineContainer() {` | `7848f7cdaaf717e9` | superseded-by-reviewed-fix | E63 | reviewed semantic replacement |
| B0339 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -666,2 +765,7 @@ export function TimelineContainer() {` | `f9b6126c15bcddc3` | superseded-by-reviewed-fix | E63 | reviewed semantic replacement |
| B0340 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -683 +787,4 @@ export function TimelineContainer() {` | `24ea11fb5549341b` | superseded-by-reviewed-fix | E63 | reviewed semantic replacement |
| B0341 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -686,0 +794,3 @@ export function TimelineContainer() {` | `1dfa0457409b395a` | superseded-by-reviewed-fix | E63 | reviewed semantic replacement |
| B0342 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -698 +808 @@ export function TimelineContainer() {` | `2a0418590decf9fb` | superseded-by-reviewed-fix | E63 | reviewed semantic replacement |
| B0343 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -1586,0 +1697,40 @@ export function TimelineContainer() {` | `db5b54e14e07bd8c` | superseded-by-reviewed-fix | E63 | reviewed semantic replacement |
| B0344 | `web/src/components/ui/PanelShell.tsx` | `@@ -9,3 +8,0 @@ import { useEditorUiStore, type Panel } from "../../store/uiStore";` | `23a385e2aeb87e49` | delivered-exact | E04 | bffbcf64d991 |
| B0345 | `web/src/components/ui/PanelShell.tsx` | `@@ -22,17 +18,0 @@ export function PanelShell({ panel, children }: PanelShellProps) {` | `6fa0cdf7ac85e232` | delivered-exact | E04 | bffbcf64d991 |
| B0346 | `web/src/components/ui/PanelShell.tsx` | `@@ -47 +27 @@ export function PanelShell({ panel, children }: PanelShellProps) {` | `7155f8247cec86bd` | delivered-exact | E04 | bffbcf64d991 |
| B0347 | `web/src/components/ui/PanelShell.tsx` | `@@ -59 +39 @@ export function PanelShell({ panel, children }: PanelShellProps) {` | `2d687b03d7ac51bb` | delivered-exact | E04 | bffbcf64d991 |
| B0348 | `web/src/i18n/dict.ts` | `@@ -106,0 +107 @@ const zh: Dict = {` | `9fec0fbd5892e701` | delivered-exact | E63 | 9c3d304327bc |
| B0349 | `web/src/i18n/dict.ts` | `@@ -431,0 +433 @@ const zh: Dict = {` | `1b2f5e5fa93a21fe` | superseded-by-reviewed-fix | E63 | reviewed semantic replacement |
| B0350 | `web/src/i18n/dict.ts` | `@@ -698,0 +701 @@ const en: Dict = {` | `fc6314142f1488ea` | delivered-exact | E63 | 9c3d304327bc |
| B0351 | `web/src/i18n/dict.ts` | `@@ -1015,0 +1019 @@ const en: Dict = {` | `96e9aa26c6e4d3bc` | superseded-by-reviewed-fix | E63 | reviewed semantic replacement |
| B0352 | `web/src/lib/api.ts` | `@@ -566 +566 @@ export async function generateThumbnail(` | `6a30e06a2d21a8dd` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0353 | `web/src/lib/api.ts` | `@@ -573,0 +574 @@ export async function generateThumbnail(` | `70f637352f3e890f` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0354 | `web/src/lib/api.ts` | `@@ -610,8 +611,8 @@ export async function previewPoster(` | `c6ecc03e9c51a9a9` | superseded-by-reviewed-fix | E55 | reviewed semantic replacement |
| B0355 | `web/src/lib/api.ts` | `@@ -621 +622,4 @@ export async function preloadMedia(mediaRef: string): Promise<void> {` | `c022fe473328f54d` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0356 | `web/src/lib/api.ts` | `@@ -696 +700,4 @@ export async function captureFrameToMedia(` | `de13c6b2aacae0b9` | superseded-by-reviewed-fix | E55 | reviewed semantic replacement |
| B0357 | `web/src/lib/api.ts` | `@@ -700 +707,4 @@ export async function getWaveform(mediaRef: string): Promise<number[] \| null> {` | `af0d39677af5c379` | superseded-by-reviewed-fix | E55 | reviewed semantic replacement |
| B0358 | `web/src/lib/api.ts` | `@@ -867,2 +877,2 @@ export async function onChatDone(` | `f8f16e60d9f5d392` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0359 | `web/src/lib/api.ts` | `@@ -871,2 +881,2 @@ export async function onChatDone(` | `e804733ff1c2d08f` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0360 | `web/src/lib/api.ts` | `@@ -874,3 +884,3 @@ export async function onChatDone(` | `42e8e530c1e54091` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0361 | `web/src/lib/api.ts` | `@@ -879 +889,4 @@ export async function playbackStart(fromFrame: number): Promise<void> {` | `c7b299f0a96150e0` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0362 | `web/src/lib/api.ts` | `@@ -882,3 +895,2 @@ export async function playbackStart(fromFrame: number): Promise<void> {` | `fb6b156bc11fba8d` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0363 | `web/src/lib/api.ts` | `@@ -886 +898 @@ export async function playbackPause(): Promise<void> {` | `0e249adce936f686` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0364 | `web/src/lib/api.ts` | `@@ -901 +913 @@ export async function playbackSeek(frame: number): Promise<void> {` | `d68b6769a86c2a7e` | superseded-by-reviewed-fix | E52 | reviewed semantic replacement |
| B0365 | `web/src/lib/mpvEdl.test.ts` | `@@ -1,112 +0,0 @@` | `b8041e16176d0b1a` | delivered-exact | E04 | 2836a5386328 |
| B0366 | `web/src/lib/mpvEdl.ts` | `@@ -1,86 +0,0 @@` | `75883b2ddb76f0a6` | delivered-exact | E04 | 2836a5386328 |
| B0367 | `web/src/store/editActions.test.ts` | `@@ -300,0 +301,12 @@ describe("addMediaToTimeline", () => {` | `7c9a8bde1f939e35` | superseded-by-reviewed-fix | E63 | reviewed semantic replacement |
| B0368 | `web/src/store/editActions.ts` | `@@ -795 +795 @@ async function addMediaToTimelineInner(item: MediaItem): Promise<void> {` | `0744de503e00f454` | delivered-exact | E63 | bf4e3a7dc670 |
| B0369 | `web/src/store/editActions.ts` | `@@ -797,3 +797,3 @@ async function addMediaToTimelineInner(item: MediaItem): Promise<void> {` | `27102fe4f8752b51` | delivered-exact | E63 | bf4e3a7dc670 |
| B0370 | `web/src/store/editActions.ts` | `@@ -800,0 +801,6 @@ async function addMediaToTimelineInner(item: MediaItem): Promise<void> {` | `fbd8cbadc8c1d746` | delivered-exact | E63 | bf4e3a7dc670 |
| B0371 | `web/src/store/editActions.ts` | `@@ -838,0 +845 @@ async function addMediaToTimelineAtInner(` | `a95f6cfea352d90e` | delivered-exact | E63 | bf4e3a7dc670 |
| B0372 | `web/src/store/editActions.ts` | `@@ -840 +847,4 @@ async function addMediaToTimelineAtInner(` | `d820b03116898044` | delivered-exact | E63 | bf4e3a7dc670 |
| B0373 | `web/src/store/editActions.ts` | `@@ -842 +851,0 @@ async function addMediaToTimelineAtInner(` | `c504d9fe1ec6b9ac` | delivered-exact | E63 | bf4e3a7dc670 |
| B0374 | `web/src/store/editActions.ts` | `@@ -927,0 +937 @@ async function addMomentToTimelineAtInner(` | `a95f6cfea352d90e` | delivered-exact | E63 | bf4e3a7dc670 |
| B0375 | `web/src/store/editActions.ts` | `@@ -929 +939,4 @@ async function addMomentToTimelineAtInner(` | `d820b03116898044` | delivered-exact | E63 | bf4e3a7dc670 |
| B0376 | `web/src/store/editActions.ts` | `@@ -931 +943,0 @@ async function addMomentToTimelineAtInner(` | `c504d9fe1ec6b9ac` | delivered-exact | E63 | bf4e3a7dc670 |
| B0377 | `web/src/store/mediaActions.test.ts` | `@@ -0,0 +1,62 @@` | `e0c5946575caba71` | delivered-exact | E63 | bf4e3a7dc670 |
| B0378 | `web/src/store/mediaActions.ts` | `@@ -47,0 +48,8 @@ function reportSkipped(list: MediaList): void {` | `1a685eccb09ec160` | delivered-exact | E63 | bf4e3a7dc670 |
| B0379 | `web/src/store/mediaActions.ts` | `@@ -54,0 +63 @@ export async function importFolderViaDialog(): Promise<void> {` | `f7fff3f05b0a87b9` | delivered-exact | E63 | bf4e3a7dc670 |
| B0380 | `web/src/store/mediaActions.ts` | `@@ -62,0 +72 @@ export async function importFolderViaDialog(): Promise<void> {` | `d1aab42c269e5913` | delivered-exact | E63 | bf4e3a7dc670 |
| B0381 | `web/src/store/mediaActions.ts` | `@@ -106,0 +117 @@ export async function importFilesViaDialog(): Promise<void> {` | `f7fff3f05b0a87b9` | delivered-exact | E63 | bf4e3a7dc670 |
| B0382 | `web/src/store/mediaActions.ts` | `@@ -118,0 +130 @@ export async function importFilesViaDialog(): Promise<void> {` | `d1aab42c269e5913` | delivered-exact | E63 | bf4e3a7dc670 |
| B0383 | `web/src/store/projectActions.test.ts` | `@@ -25 +25,19 @@ const srv = vi.hoisted(() => {` | `02fad31ce07b2664` | superseded-by-reviewed-fix | E63 | reviewed semantic replacement |
| B0384 | `web/src/store/projectActions.test.ts` | `@@ -29 +47,6 @@ vi.mock("../lib/api", () => ({` | `c6e0db9bb4a1cb51` | superseded-by-reviewed-fix | E63 | reviewed semantic replacement |
| B0385 | `web/src/store/projectActions.test.ts` | `@@ -30,0 +54,2 @@ vi.mock("../lib/api", () => ({` | `e7afe33961d856b0` | superseded-by-reviewed-fix | E63 | reviewed semantic replacement |
| B0386 | `web/src/store/projectActions.test.ts` | `@@ -33 +58,6 @@ vi.mock("../lib/api", () => ({` | `235be20e7c8128f3` | superseded-by-reviewed-fix | E63 | reviewed semantic replacement |
| B0387 | `web/src/store/projectActions.test.ts` | `@@ -36,0 +67 @@ import { useProjectStore } from "./projectStore";` | `8169b74441747b64` | superseded-by-reviewed-fix | E63 | reviewed semantic replacement |
| B0388 | `web/src/store/projectActions.test.ts` | `@@ -38,5 +69,26 @@ import { useProjectStore } from "./projectStore";` | `37efef44677fe8b4` | superseded-by-reviewed-fix | E63 | reviewed semantic replacement |
| B0389 | `web/src/store/projectActions.test.ts` | `@@ -43,0 +96,35 @@ describe("openProjectPath", () => {` | `a43c55740b6a9f71` | superseded-by-reviewed-fix | E63 | reviewed semantic replacement |
| B0390 | `web/src/store/projectActions.test.ts` | `@@ -44,0 +132 @@ describe("openProjectPath", () => {` | `5b60c07f9b354363` | superseded-by-reviewed-fix | E63 | reviewed semantic replacement |
| B0391 | `web/src/store/projectActions.test.ts` | `@@ -51,0 +140,130 @@ describe("openProjectPath", () => {` | `d511f9faeac1dab9` | superseded-by-reviewed-fix | E63 | reviewed semantic replacement |
| B0392 | `web/src/store/projectActions.ts` | `@@ -14,0 +15 @@ import { refreshMedia } from "./mediaStore";` | `ca4b728ee242e1cd` | superseded-by-reviewed-fix | E63 | reviewed semantic replacement |
| B0393 | `web/src/store/projectActions.ts` | `@@ -24,0 +26,8 @@ function withExt(path: string): string {` | `ba95c6e97dc09385` | superseded-by-reviewed-fix | E63 | reviewed semantic replacement |
| B0394 | `web/src/store/projectActions.ts` | `@@ -35,0 +45,2 @@ export async function newProjectAndEnter(): Promise<void> {` | `45c2f15364bafeeb` | superseded-by-reviewed-fix | E63 | reviewed semantic replacement |
| B0395 | `web/src/store/projectActions.ts` | `@@ -39,0 +51,5 @@ export async function newProjectAndEnter(): Promise<void> {` | `7fcc8c9101594ed9` | superseded-by-reviewed-fix | E63 | reviewed semantic replacement |
| B0396 | `web/src/store/projectActions.ts` | `@@ -59,0 +76,2 @@ export async function newProjectAndEnter(): Promise<void> {` | `16c75eb04862c101` | superseded-by-reviewed-fix | E63 | reviewed semantic replacement |
| B0397 | `web/src/store/projectActions.ts` | `@@ -63 +81,2 @@ export async function newProjectAndEnter(): Promise<void> {` | `722efce36cb040b1` | superseded-by-reviewed-fix | E63 | reviewed semantic replacement |
| B0398 | `web/src/store/projectActions.ts` | `@@ -87,0 +107,2 @@ export async function openProjectPath(path: string): Promise<void> {` | `45c2f15364bafeeb` | superseded-by-reviewed-fix | E63 | reviewed semantic replacement |
| B0399 | `web/src/store/projectActions.ts` | `@@ -93,0 +115,2 @@ export async function openProjectPath(path: string): Promise<void> {` | `9044e622143da6bf` | superseded-by-reviewed-fix | E63 | reviewed semantic replacement |
| B0400 | `web/src/store/uiStore.ts` | `@@ -213,0 +214,3 @@ interface UiState {` | `bff7cdfa9ed99ab7` | delivered-exact | E63 | bf4e3a7dc670 |
| B0401 | `web/src/store/uiStore.ts` | `@@ -411,0 +415,27 @@ export const useEditorUiStore = create<UiState>((set, get) => ({` | `12f1a7e38daa8e21` | delivered-exact | E63 | bf4e3a7dc670 |
| B0402 | `web/src/styles/global.css` | `@@ -19,4 +19 @@ body {` | `fd4721d3bc04ea64` | delivered-exact | E04 | 2836a5386328 |
| B0403 | `web/src/styles/global.css` | `@@ -65,0 +63,17 @@ button {` | `724100356235f6fe` | delivered-exact | E63 | bf4e3a7dc670 |

## Reviewer-approved integration-only delivery hunks

Each I row is a source-to-delivery occurrence left after one-to-one consumption of exact baseline occurrences. The evidence code supplies concrete tests and the final independent report.

| ID | Path | Zero-context header/range | Fingerprint | Owning reviewed slice | Deciding commit / contributors |
|---|---|---|---|---|---|
| I0001 | `.github/workflows/ci.yml` | `@@ -81,0 +72,8 @@ jobs:` | `9d811c6b12c6f2b7` | E56 | 3fe09766819b |
| I0002 | `.gitignore` | `@@ -14,2 +13,0 @@ src-tauri/gen/` | `cd2beffb1d3820df` | E04 | 2836a5386328 |
| I0003 | `README.md` | `@@ -124,0 +125,10 @@ Built-in Agent chat panel shares tool definitions and system prompt with MCP.` | `d478c5af72de6407` | E7A | 43524e9d7181 |
| I0004 | `crates/opentake-core/src/core.rs` | `@@ -87,0 +88,2 @@ pub struct TimelineSnapshot {` | `00ea9723b4cc1182` | E51 | 2eff907cfc9c |
| I0005 | `crates/opentake-core/src/core.rs` | `@@ -91,0 +94,54 @@ pub struct TimelineSnapshot {` | `0dc6ce518c3514aa` | E55 | 51243429cf5f; contrib 2eff907cfc9c |
| I0006 | `crates/opentake-core/src/core.rs` | `@@ -95 +151 @@ pub struct AppCore {` | `dda2f2aa3639df04` | E51 | 2eff907cfc9c |
| I0007 | `crates/opentake-core/src/core.rs` | `@@ -119 +175,4 @@ impl AppCore {` | `a04a9da2ddfc2bbc` | E51 | 2eff907cfc9c |
| I0008 | `crates/opentake-core/src/core.rs` | `@@ -152,0 +212,5 @@ impl AppCore {` | `64c2386733ebccfa` | E51 | 2eff907cfc9c |
| I0009 | `crates/opentake-core/src/core.rs` | `@@ -154,3 +218,15 @@ impl AppCore {` | `90b7c2e75a04d61c` | E51 | 2eff907cfc9c |
| I0010 | `crates/opentake-core/src/core.rs` | `@@ -162 +238 @@ impl AppCore {` | `73ecd366968b365c` | E51 | 2eff907cfc9c |
| I0011 | `crates/opentake-core/src/core.rs` | `@@ -167 +243 @@ impl AppCore {` | `110c5bd769cdc9a6` | E51 | 2eff907cfc9c |
| I0012 | `crates/opentake-core/src/core.rs` | `@@ -172 +248 @@ impl AppCore {` | `91042439accff592` | E51 | 2eff907cfc9c |
| I0013 | `crates/opentake-core/src/core.rs` | `@@ -184 +260 @@ impl AppCore {` | `b6e919e03da8fafa` | E51 | 2eff907cfc9c |
| I0014 | `crates/opentake-core/src/core.rs` | `@@ -186 +262,2 @@ impl AppCore {` | `39a41d3f510f90bb` | E51 | 2eff907cfc9c |
| I0015 | `crates/opentake-core/src/core.rs` | `@@ -189,0 +267 @@ impl AppCore {` | `8c81ba13b0c88bf9` | E51 | 2eff907cfc9c |
| I0016 | `crates/opentake-core/src/core.rs` | `@@ -211,4 +289,5 @@ impl AppCore {` | `f30e108b15232bb2` | E51 | 2eff907cfc9c |
| I0017 | `crates/opentake-core/src/core.rs` | `@@ -216,2 +295,2 @@ impl AppCore {` | `ab05316e3f179741` | E51 | 2eff907cfc9c |
| I0018 | `crates/opentake-core/src/core.rs` | `@@ -220 +299,2 @@ impl AppCore {` | `60ab7ca044bb1c93` | E51 | 2eff907cfc9c |
| I0019 | `crates/opentake-core/src/core.rs` | `@@ -221,0 +302 @@ impl AppCore {` | `596f8dedb921986b` | E51 | 2eff907cfc9c |
| I0020 | `crates/opentake-core/src/core.rs` | `@@ -229,2 +310,10 @@ impl AppCore {` | `3fcf3da0facda393` | E55 | 51243429cf5f; contrib 2eff907cfc9c |
| I0021 | `crates/opentake-core/src/core.rs` | `@@ -233,5 +322 @@ impl AppCore {` | `d2f71ba7b682324d` | E51 | 2eff907cfc9c |
| I0022 | `crates/opentake-core/src/core.rs` | `@@ -240 +325,2 @@ impl AppCore {` | `d459a87a85609037` | E51 | 2eff907cfc9c |
| I0023 | `crates/opentake-core/src/core.rs` | `@@ -243 +329 @@ impl AppCore {` | `8acfce69d9e88205` | E51 | 2eff907cfc9c |
| I0024 | `crates/opentake-core/src/core.rs` | `@@ -264 +350 @@ impl AppCore {` | `7f88132c9d06e825` | E51 | 2eff907cfc9c |
| I0025 | `crates/opentake-core/src/core.rs` | `@@ -266 +352,4 @@ impl AppCore {` | `4740f4def306415d` | E51 | 2eff907cfc9c |
| I0026 | `crates/opentake-core/src/core.rs` | `@@ -269,0 +359 @@ impl AppCore {` | `e972df54d0f2adeb` | E51 | 2eff907cfc9c |
| I0027 | `crates/opentake-core/src/core.rs` | `@@ -279 +369 @@ impl AppCore {` | `124149fedf1cb5c8` | E51 | 2eff907cfc9c |
| I0028 | `crates/opentake-core/src/core.rs` | `@@ -288 +378 @@ impl AppCore {` | `1b92de525704a9ed` | E51 | 2eff907cfc9c |
| I0029 | `crates/opentake-core/src/core.rs` | `@@ -295 +385 @@ impl AppCore {` | `373d05fce9272e44` | E51 | 2eff907cfc9c |
| I0030 | `crates/opentake-core/src/core.rs` | `@@ -314 +404 @@ impl AppCore {` | `b81ed0ea02ad2015` | E51 | 2eff907cfc9c |
| I0031 | `crates/opentake-core/src/core.rs` | `@@ -316,3 +406,3 @@ impl AppCore {` | `2adf244a085974f3` | E51 | 2eff907cfc9c |
| I0032 | `crates/opentake-core/src/core.rs` | `@@ -320 +410,4 @@ impl AppCore {` | `286d8f7ba33608a9` | E51 | 2eff907cfc9c |
| I0033 | `crates/opentake-core/src/core.rs` | `@@ -330 +423 @@ impl AppCore {` | `d32cc6591a35ef26` | E51 | 2eff907cfc9c |
| I0034 | `crates/opentake-core/src/core.rs` | `@@ -332,3 +425,3 @@ impl AppCore {` | `8ba74318ab06d2fd` | E51 | 2eff907cfc9c |
| I0035 | `crates/opentake-core/src/core.rs` | `@@ -337 +430,4 @@ impl AppCore {` | `f461495129c0e275` | E51 | 2eff907cfc9c |
| I0036 | `crates/opentake-core/src/core.rs` | `@@ -354 +450 @@ impl AppCore {` | `b81ed0ea02ad2015` | E51 | 2eff907cfc9c |
| I0037 | `crates/opentake-core/src/core.rs` | `@@ -356,3 +452,3 @@ impl AppCore {` | `9705ec60aaf7ac6e` | E51 | 2eff907cfc9c |
| I0038 | `crates/opentake-core/src/core.rs` | `@@ -360 +456,4 @@ impl AppCore {` | `286d8f7ba33608a9` | E51 | 2eff907cfc9c |
| I0039 | `crates/opentake-core/src/core.rs` | `@@ -370 +469 @@ impl AppCore {` | `4508663aa68cab20` | E51 | 2eff907cfc9c |
| I0040 | `crates/opentake-core/src/core.rs` | `@@ -391 +490 @@ mod tests {` | `1938a74739e91d8e` | E51 | 2eff907cfc9c |
| I0041 | `crates/opentake-core/src/core.rs` | `@@ -395,0 +495,51 @@ mod tests {` | `c27461a1edf2fa13` | E51 | 2eff907cfc9c |
| I0042 | `crates/opentake-core/src/core.rs` | `@@ -456 +606,7 @@ mod tests {` | `9257cc80f1160a6f` | E51 | 2eff907cfc9c |
| I0043 | `crates/opentake-core/src/core.rs` | `@@ -491 +647 @@ mod tests {` | `63aae3a2588d2cf9` | E51 | 2eff907cfc9c |
| I0044 | `crates/opentake-core/src/core.rs` | `@@ -511,0 +668,71 @@ mod tests {` | `c172d217a20845ac` | E51 | 2eff907cfc9c |
| I0045 | `crates/opentake-core/src/core.rs` | `@@ -522 +749 @@ mod tests {` | `11e6ad96091388b8` | E51 | 2eff907cfc9c |
| I0046 | `crates/opentake-core/src/core.rs` | `@@ -523,0 +751 @@ mod tests {` | `a28a26ff1fb60275` | E51 | 2eff907cfc9c |
| I0047 | `crates/opentake-core/src/core.rs` | `@@ -528,0 +757 @@ mod tests {` | `77bde3bd1162b974` | E51 | 2eff907cfc9c |
| I0048 | `crates/opentake-core/src/core.rs` | `@@ -556,0 +786 @@ mod tests {` | `c0eb418cf24476e8` | E51 | 2eff907cfc9c |
| I0049 | `crates/opentake-core/src/core.rs` | `@@ -563 +793,4 @@ mod tests {` | `2411cf57a5ec1220` | E51 | 2eff907cfc9c |
| I0050 | `crates/opentake-core/src/core.rs` | `@@ -592 +825,4 @@ mod tests {` | `d429f399c23d9e73` | E51 | 2eff907cfc9c |
| I0051 | `crates/opentake-core/src/dto.rs` | `@@ -51,0 +52,2 @@ pub struct TimelineSnapshotDto {` | `3b9686fa116ef3fa` | E51 | 2eff907cfc9c |
| I0052 | `crates/opentake-core/src/dto.rs` | `@@ -59,0 +62 @@ impl From<TimelineSnapshot> for TimelineSnapshotDto {` | `1be03117d4ae9808` | E51 | 2eff907cfc9c |
| I0053 | `crates/opentake-core/src/dto.rs` | `@@ -140,3 +143,4 @@ pub fn handle_project_save(` | `8382bc72aafa71ec` | E51 | 2eff907cfc9c |
| I0054 | `crates/opentake-core/src/dto.rs` | `@@ -206,0 +211 @@ mod tests {` | `45b79b35563f18ec` | E51 | 2eff907cfc9c |
| I0055 | `crates/opentake-core/src/dto.rs` | `@@ -207,0 +213,2 @@ mod tests {` | `696a6e27efdbb315` | E51 | 2eff907cfc9c |
| I0056 | `crates/opentake-core/src/dto.rs` | `@@ -245,0 +253,12 @@ mod tests {` | `cadd653581187cdb` | E51 | 2eff907cfc9c |
| I0057 | `crates/opentake-core/src/events.rs` | `@@ -40,0 +41,3 @@ pub enum CoreEvent {` | `99aa80a6df92140d` | E51 | 2eff907cfc9c |
| I0058 | `crates/opentake-core/src/events.rs` | `@@ -51,0 +55,3 @@ pub enum CoreEvent {` | `59a7691805679e4c` | E51 | 2eff907cfc9c |
| I0059 | `crates/opentake-core/src/events.rs` | `@@ -59,0 +66,3 @@ pub enum CoreEvent {` | `f71624af4c732812` | E51 | 2eff907cfc9c |
| I0060 | `crates/opentake-core/src/events.rs` | `@@ -65,0 +75,3 @@ pub enum CoreEvent {` | `e438521e966f74c3` | E51 | 2eff907cfc9c |
| I0061 | `crates/opentake-core/src/events.rs` | `@@ -145 +157,4 @@ mod tests {` | `b580127a0e7f384b` | E51 | 2eff907cfc9c |
| I0062 | `crates/opentake-core/src/events.rs` | `@@ -155,2 +170,8 @@ mod tests {` | `8bd0c4772378c0b4` | E51 | 2eff907cfc9c |
| I0063 | `crates/opentake-core/src/events.rs` | `@@ -162,2 +183,8 @@ mod tests {` | `800c2dae17797c26` | E51 | 2eff907cfc9c |
| I0064 | `crates/opentake-core/src/events.rs` | `@@ -175 +202,4 @@ mod tests {` | `2280f6d48b8b0d6e` | E51 | 2eff907cfc9c |
| I0065 | `crates/opentake-core/src/events.rs` | `@@ -177 +207,4 @@ mod tests {` | `2280f6d48b8b0d6e` | E51 | 2eff907cfc9c |
| I0066 | `crates/opentake-core/src/events.rs` | `@@ -184,2 +217,9 @@ mod tests {` | `28492993b9b3eb18` | E51 | 2eff907cfc9c |
| I0067 | `crates/opentake-core/src/events.rs` | `@@ -190,2 +230,9 @@ mod tests {` | `b2111808feb7827f` | E51 | 2eff907cfc9c |
| I0068 | `crates/opentake-core/src/lib.rs` | `@@ -22,2 +22,2 @@` | `70224b9973ce5f08` | E51 | 2eff907cfc9c |
| I0069 | `crates/opentake-core/src/lib.rs` | `@@ -46 +46 @@ pub mod session;` | `4a6f8d0a6de9954f` | E51 | 2eff907cfc9c |
| I0070 | `crates/opentake-media/src/cancel.rs` | `@@ -0,0 +1,68 @@` | `7dec5782fed6af3f` | E53 | b5e5596a43ac; contrib f5aa9646da26 |
| I0071 | `crates/opentake-media/src/decode/audio_stream.rs` | `@@ -12 +11,0 @@` | `1b9d127a33c23357` | E53 | f5aa9646da26 |
| I0072 | `crates/opentake-media/src/decode/audio_stream.rs` | `@@ -15 +14,2 @@ use std::path::Path;` | `2e108750efffac8f` | E53 | f5aa9646da26 |
| I0073 | `crates/opentake-media/src/decode/audio_stream.rs` | `@@ -17,2 +16,0 @@ use crate::error::{MediaError, Result};` | `b475c9867ff36abf` | E53 | f5aa9646da26 |
| I0074 | `crates/opentake-media/src/decode/audio_stream.rs` | `@@ -22,0 +21 @@ use crate::probe;` | `b310c64ba7635a5a` | E53 | f5aa9646da26 |
| I0075 | `crates/opentake-media/src/decode/audio_stream.rs` | `@@ -52 +51,9 @@ fn interleaved_args(path: &Path, spec: &PcmSpec, range: Option<(f64, f64)>) -> V` | `91b50a76ca2c7cf8` | E53 | b5e5596a43ac |
| I0076 | `crates/opentake-media/src/decode/audio_stream.rs` | `@@ -54,8 +61,10 @@ fn raw_to_interleaved_f32(bytes: &[u8], spec: &PcmSpec) -> Vec<f32> {` | `59ad8bb2c888b5d2` | E53 | b5e5596a43ac |
| I0077 | `crates/opentake-media/src/decode/audio_stream.rs` | `@@ -62,0 +72 @@ fn raw_to_interleaved_f32(bytes: &[u8], spec: &PcmSpec) -> Vec<f32> {` | `4bc02dfc60eb0ef1` | E53 | b5e5596a43ac |
| I0078 | `crates/opentake-media/src/decode/audio_stream.rs` | `@@ -73,23 +83,2 @@ pub fn decode_pcm_interleaved(` | `3b61c27c72b735ab` | E53 | f5aa9646da26 |
| I0079 | `crates/opentake-media/src/decode/audio_stream.rs` | `@@ -97 +86,8 @@ pub fn decode_pcm_interleaved(` | `fb94a9d1d03863bd` | E53 | b5e5596a43ac; contrib f5aa9646da26 |
| I0080 | `crates/opentake-media/src/decode/audio_stream.rs` | `@@ -143 +139 @@ mod tests {` | `26983b17e05b26a8` | E53 | b5e5596a43ac |
| I0081 | `crates/opentake-media/src/decode/audio_stream.rs` | `@@ -153 +149 @@ mod tests {` | `3647efdef0480fd3` | E53 | b5e5596a43ac |
| I0082 | `crates/opentake-media/src/decode/audio_stream.rs` | `@@ -163 +159 @@ mod tests {` | `573c4910c4f38132` | E53 | b5e5596a43ac |
| I0083 | `crates/opentake-media/src/decode/frame.rs` | `@@ -13,0 +14,2 @@ use std::path::Path;` | `cc5ccc649b261653` | E53 | b5e5596a43ac |
| I0084 | `crates/opentake-media/src/decode/frame.rs` | `@@ -15,0 +18 @@ use ffmpeg_sidecar::event::FfmpegEvent;` | `f6b00514211c1c19` | E55 | 51243429cf5f |
| I0085 | `crates/opentake-media/src/decode/frame.rs` | `@@ -16,0 +20 @@ use ffmpeg_sidecar::event::FfmpegEvent;` | `6bfff3c4109e0cfb` | E53 | f5aa9646da26 |
| I0086 | `crates/opentake-media/src/decode/frame.rs` | `@@ -20,0 +25,2 @@ use crate::frame::RgbaFrame;` | `9587a60e7c71b383` | E53 | b5e5596a43ac |
| I0087 | `crates/opentake-media/src/decode/frame.rs` | `@@ -119,0 +126,11 @@ pub fn decode_frame_at(path: &Path, req: &FrameRequest) -> Result<(f64, RgbaFram` | `c0c9cac3ae902e2c` | E53 | f5aa9646da26 |
| I0088 | `crates/opentake-media/src/decode/frame.rs` | `@@ -123,0 +141 @@ pub fn decode_frame_at(path: &Path, req: &FrameRequest) -> Result<(f64, RgbaFram` | `413e1aee0f3224c3` | E53 | b5e5596a43ac |
| I0089 | `crates/opentake-media/src/decode/frame.rs` | `@@ -125,8 +143,57 @@ pub fn decode_frame_at(path: &Path, req: &FrameRequest) -> Result<(f64, RgbaFram` | `b7e9a6799e0c2e73` | E53 | b5e5596a43ac; contrib f5aa9646da26 |
| I0090 | `crates/opentake-media/src/decode/frame.rs` | `@@ -134,2 +201,58 @@ pub fn decode_frame_at(path: &Path, req: &FrameRequest) -> Result<(f64, RgbaFram` | `638aeaf74f0cef01` | E55 | 51243429cf5f; contrib f5aa9646da26,b5e5596a43ac |
| I0091 | `crates/opentake-media/src/decode/frame.rs` | `@@ -137,0 +261,12 @@ pub fn decode_frame_at(path: &Path, req: &FrameRequest) -> Result<(f64, RgbaFram` | `153bf89895a46350` | E53 | f5aa9646da26 |
| I0092 | `crates/opentake-media/src/decode/frame.rs` | `@@ -139,2 +274 @@ pub fn decode_frame_at(path: &Path, req: &FrameRequest) -> Result<(f64, RgbaFram` | `5791698ea8934c07` | E53 | f5aa9646da26 |
| I0093 | `crates/opentake-media/src/decode/frame.rs` | `@@ -177,0 +312,4 @@ mod tests {` | `2efe0a7d6684159b` | E53 | b5e5596a43ac |
| I0094 | `crates/opentake-media/src/decode/frame.rs` | `@@ -179,0 +318,42 @@ mod tests {` | `c8ca5c7bc8df81d3` | E53 | b5e5596a43ac |
| I0095 | `crates/opentake-media/src/decode/mod.rs` | `@@ -9,3 +9,6 @@ pub mod stream;` | `56e0248c9f272fb8` | E53 | f5aa9646da26 |
| I0096 | `crates/opentake-media/src/decode/pcm.rs` | `@@ -13,0 +14,3 @@ use std::path::Path;` | `8772db7ae57c4c65` | E53 | f5aa9646da26 |
| I0097 | `crates/opentake-media/src/decode/pcm.rs` | `@@ -14,0 +18 @@ use std::path::Path;` | `9333bd58e8468326` | E53 | f5aa9646da26 |
| I0098 | `crates/opentake-media/src/decode/pcm.rs` | `@@ -34 +38 @@ impl PcmFormat {` | `50089273a74925c9` | E53 | f5aa9646da26 |
| I0099 | `crates/opentake-media/src/decode/pcm.rs` | `@@ -41,0 +46,197 @@ impl PcmFormat {` | `95036402c9cd5fef` | E53 | b5e5596a43ac; contrib f5aa9646da26 |
| I0100 | `crates/opentake-media/src/decode/pcm.rs` | `@@ -92 +293 @@ fn pcm_args(path: &Path, spec: &PcmSpec, range: Option<(f64, f64)>) -> Vec<Strin` | `f27892423f2677e3` | E53 | b5e5596a43ac |
| I0101 | `crates/opentake-media/src/decode/pcm.rs` | `@@ -97 +298 @@ fn raw_to_mono_f32(bytes: &[u8], spec: &PcmSpec) -> Vec<f32> {` | `148ed02bc3c7969b` | E53 | b5e5596a43ac |
| I0102 | `crates/opentake-media/src/decode/pcm.rs` | `@@ -100 +301,3 @@ fn raw_to_mono_f32(bytes: &[u8], spec: &PcmSpec) -> Vec<f32> {` | `8745448672edaf42` | E53 | b5e5596a43ac |
| I0103 | `crates/opentake-media/src/decode/pcm.rs` | `@@ -119 +322 @@ fn raw_to_mono_f32(bytes: &[u8], spec: &PcmSpec) -> Vec<f32> {` | `5d2c6660906f06fb` | E53 | b5e5596a43ac |
| I0104 | `crates/opentake-media/src/decode/pcm.rs` | `@@ -126,3 +329,29 @@ pub fn extract_pcm(path: &Path, spec: &PcmSpec, range: Option<(f64, f64)>) -> Re` | `b8073483e8c2a6a9` | E53 | b5e5596a43ac; contrib f5aa9646da26 |
| I0105 | `crates/opentake-media/src/decode/pcm.rs` | `@@ -131 +360,18 @@ pub fn extract_pcm(path: &Path, spec: &PcmSpec, range: Option<(f64, f64)>) -> Re` | `3aa4d5b22d74654a` | E53 | f5aa9646da26 |
| I0106 | `crates/opentake-media/src/decode/pcm.rs` | `@@ -134 +380 @@ pub fn extract_pcm(path: &Path, spec: &PcmSpec, range: Option<(f64, f64)>) -> Re` | `9350db18dc391a62` | E53 | f5aa9646da26 |
| I0107 | `crates/opentake-media/src/decode/pcm.rs` | `@@ -136,0 +383,49 @@ pub fn extract_pcm(path: &Path, spec: &PcmSpec, range: Option<(f64, f64)>) -> Re` | `c0786597b0d43287` | E53 | b5e5596a43ac; contrib f5aa9646da26 |
| I0108 | `crates/opentake-media/src/decode/pcm.rs` | `@@ -138,7 +433,12 @@ pub fn extract_pcm(path: &Path, spec: &PcmSpec, range: Option<(f64, f64)>) -> Re` | `69bfa636534d9697` | E53 | f5aa9646da26 |
| I0109 | `crates/opentake-media/src/decode/pcm.rs` | `@@ -146,3 +446,16 @@ pub fn extract_pcm(path: &Path, spec: &PcmSpec, range: Option<(f64, f64)>) -> Re` | `10f1cfc792c72e50` | E53 | f5aa9646da26 |
| I0110 | `crates/opentake-media/src/decode/pcm.rs` | `@@ -150,5 +463,44 @@ pub fn extract_pcm(path: &Path, spec: &PcmSpec, range: Option<(f64, f64)>) -> Re` | `782113c0156a2d87` | E53 | f5aa9646da26 |
| I0111 | `crates/opentake-media/src/decode/pcm.rs` | `@@ -157,6 +509,23 @@ pub fn extract_pcm(path: &Path, spec: &PcmSpec, range: Option<(f64, f64)>) -> Re` | `b5ee34b0e85dddc8` | E53 | b5e5596a43ac |
| I0112 | `crates/opentake-media/src/decode/pcm.rs` | `@@ -164,3 +533,42 @@ pub fn extract_pcm(path: &Path, spec: &PcmSpec, range: Option<(f64, f64)>) -> Re` | `4f2493db169d338e` | E53 | b5e5596a43ac |
| I0113 | `crates/opentake-media/src/decode/pcm.rs` | `@@ -224 +632 @@ mod tests {` | `fffa69c4b39f4e83` | E53 | b5e5596a43ac |
| I0114 | `crates/opentake-media/src/decode/pcm.rs` | `@@ -243 +651 @@ mod tests {` | `fffa69c4b39f4e83` | E53 | b5e5596a43ac |
| I0115 | `crates/opentake-media/src/decode/pcm.rs` | `@@ -257 +665 @@ mod tests {` | `6ccc9dab2abec16a` | E53 | b5e5596a43ac |
| I0116 | `crates/opentake-media/src/lib.rs` | `@@ -30,0 +31 @@ pub mod cache_key;` | `2467715d13760ee2` | E53 | f5aa9646da26 |
| I0117 | `crates/opentake-media/src/lib.rs` | `@@ -48,0 +50 @@ use std::path::{Path, PathBuf};` | `7033a34df243b678` | E53 | f5aa9646da26 |
| I0118 | `crates/opentake-media/src/lib.rs` | `@@ -55,3 +57,4 @@ pub use decode::{` | `14e338959f43fc51` | E53 | f5aa9646da26 |
| I0119 | `crates/opentake-media/src/lib.rs` | `@@ -70 +73,4 @@ pub use timecode::{parse_smpte_timecode, read_start_timecode_frame};` | `fc52663fe9282f70` | E53 | f5aa9646da26 |
| I0120 | `crates/opentake-media/src/waveform/mod.rs` | `@@ -30 +30 @@ use crate::error::Result;` | `8c83f811006e9161` | E53 | f5aa9646da26 |
| I0121 | `crates/opentake-media/src/waveform/mod.rs` | `@@ -40,0 +41,8 @@ pub fn waveform(path: &Path, duration_secs: f64) -> Result<Vec<f32>> {` | `3195cedbb039fdcd` | E53 | f5aa9646da26 |
| I0122 | `crates/opentake-media/src/waveform/mod.rs` | `@@ -46 +54 @@ pub fn waveform(path: &Path, duration_secs: f64) -> Result<Vec<f32>> {` | `f1ed6095e38c6d35` | E53 | f5aa9646da26 |
| I0123 | `crates/opentake-media/src/waveform/mod.rs` | `@@ -50,0 +59,23 @@ pub fn waveform(path: &Path, duration_secs: f64) -> Result<Vec<f32>> {` | `b5219d8608f6a8b0` | E55 | 51243429cf5f |
| I0124 | `crates/opentake-media/src/waveform/mod.rs` | `@@ -53,0 +85,9 @@ pub fn waveform_cached(cache_root: &Path, path: &Path, duration_secs: f64) -> Re` | `21e2cfbf84e8cb73` | E53 | f5aa9646da26 |
| I0125 | `crates/opentake-media/src/waveform/mod.rs` | `@@ -58 +98 @@ pub fn waveform_cached(cache_root: &Path, path: &Path, duration_secs: f64) -> Re` | `a39d5918b62e3e5c` | E53 | f5aa9646da26 |
| I0126 | `crates/opentake-media/src/waveform/mod.rs` | `@@ -62 +102,14 @@ pub fn waveform_cached(cache_root: &Path, path: &Path, duration_secs: f64) -> Re` | `3740f1c215b21243` | E55 | 51243429cf5f; contrib f5aa9646da26 |
| I0127 | `docs/architecture/HANDOFF-2026-07.md` | `@@ -2,0 +3,32 @@` | `15be1bf18b9f9a7a` | E7A | 43524e9d7181 |
| I0128 | `docs/architecture/PLAYBACK-ENGINE.md` | `@@ -1 +1 @@` | `ee1f9f94f098cd70` | E7A | 43524e9d7181 |
| I0129 | `docs/architecture/PLAYBACK-ENGINE.md` | `@@ -3,3 +3,2 @@` | `702b2e42c4ebc1a3` | E7A | 43524e9d7181 |
| I0130 | `docs/architecture/PLAYBACK-ENGINE.md` | `@@ -7 +6 @@` | `cbd89a303fbba4a5` | E7A | 43524e9d7181 |
| I0131 | `docs/architecture/PLAYBACK-ENGINE.md` | `@@ -9 +8,2 @@` | `2e6e4d15eedddeb9` | E7A | 43524e9d7181 |
| I0132 | `docs/architecture/PLAYBACK-ENGINE.md` | `@@ -11 +11,7 @@` | `deae712e729f1935` | E7A | 43524e9d7181 |
| I0133 | `docs/architecture/PLAYBACK-ENGINE.md` | `@@ -13,14 +19,18 @@` | `322cf848f9f26465` | E7A | 43524e9d7181 |
| I0134 | `docs/architecture/PLAYBACK-ENGINE.md` | `@@ -28,41 +38,4 @@ SCRUB / PAUSE 态: 引擎停, 回原 <video> + composite_frame 路径(零改动)` | `ab845569eed0f28d` | E7A | 43524e9d7181 |
| I0135 | `docs/architecture/PLAYBACK-ENGINE.md` | `@@ -69,0 +43,86 @@ SCRUB / PAUSE 态: 引擎停, 回原 <video> + composite_frame 路径(零改动)` | `2511d492af7077b8` | E7A | 43524e9d7181 |
| I0136 | `docs/superpowers/archive/2026-07-08-verification-report.md` | `@@ -57,0 +58,46 @@` | `181347bf31d2be55` | E7A | 43524e9d7181 |
| I0137 | `docs/superpowers/archive/2026-07-10-playback-cache-installed-app-qa.md` | `@@ -0,0 +1,172 @@` | `5ecd5db3541ea725` | E7A | be23c51d082f; contrib 43524e9d7181 |
| I0138 | `src-tauri/src/commands.rs` | `@@ -4,3 +4,4 @@` | `2083d9f74b9b11bd` | E52 | e2daeb279a33 |
| I0139 | `src-tauri/src/commands.rs` | `@@ -16,2 +17,2 @@ use opentake_core::dto::{` | `9401b2f908000605` | E55 | 51243429cf5f |
| I0140 | `src-tauri/src/commands.rs` | `@@ -50 +51,3 @@ pub fn redo(core: State<'_, AppCore>) -> Result<EditResultDto, String> {` | `87e90265b9b729b4` | E52 | 6551e6744dbd; contrib 2eff907cfc9c |
| I0141 | `src-tauri/src/commands.rs` | `@@ -52,2 +55,45 @@ pub fn redo(core: State<'_, AppCore>) -> Result<EditResultDto, String> {` | `1c84ba83f4efdd7a` | E55 | 51243429cf5f; contrib 6551e6744dbd,e2daeb279a33 |
| I0142 | `src-tauri/src/commands.rs` | `@@ -56,0 +103 @@ pub fn project_new(core: State<'_, AppCore>) {` | `653efea8ab802b52` | E52 | 6551e6744dbd |
| I0143 | `src-tauri/src/commands.rs` | `@@ -58,2 +105,56 @@ pub fn project_new(core: State<'_, AppCore>) {` | `058a17dd02ef6641` | E55 | 51243429cf5f; contrib 6551e6744dbd,7a0a05cb1dd2,e2daeb279a33 |
| I0144 | `src-tauri/src/commands.rs` | `@@ -76,5 +177,6 @@ pub fn project_save(core: State<'_, AppCore>, path: Option<String>) -> Result<St` | `6109cab946dc0be1` | E51 | 2eff907cfc9c |
| I0145 | `src-tauri/src/commands.rs` | `@@ -112,3 +214 @@ pub fn export_xmeml(core: State<'_, AppCore>, path: String) -> Result<(), String` | `efd3d9fd0a9cf5f4` | E51 | 2eff907cfc9c |
| I0146 | `src-tauri/src/commands.rs` | `@@ -118 +218,5 @@ pub fn export_xmeml(core: State<'_, AppCore>, path: String) -> Result<(), String` | `01e2e814798323f5` | E51 | 2eff907cfc9c |
| I0147 | `src-tauri/src/commands.rs` | `@@ -120,3 +224,3 @@ pub fn export_xmeml(core: State<'_, AppCore>, path: String) -> Result<(), String` | `6e6ae9c72908c46f` | E51 | 2eff907cfc9c |
| I0148 | `src-tauri/src/commands.rs` | `@@ -177,3 +281,2 @@ pub fn export_edl(core: State<'_, AppCore>, path: String) -> Result<(), String>` | `0ce567e9b94e4169` | E51 | 2eff907cfc9c |
| I0149 | `src-tauri/src/commands.rs` | `@@ -190,4 +293,6 @@ pub fn export_otio(core: State<'_, AppCore>, path: String) -> Result<(), String>` | `bcfb1a9a44f2081e` | E51 | 2eff907cfc9c |
| I0150 | `src-tauri/src/commands.rs` | `@@ -204,4 +309,6 @@ pub fn export_fcpxml_modern(core: State<'_, AppCore>, path: String) -> Result<()` | `894cf5fd7575b24b` | E51 | 2eff907cfc9c |
| I0151 | `src-tauri/src/commands.rs` | `@@ -1084,0 +1192,58 @@ impl KeyframeValueDto {` | `6b2aacfbc650d71d` | E55 | 51243429cf5f |
| I0152 | `src-tauri/src/lib.rs` | `@@ -40,0 +40 @@ use tauri::RunEvent;` | `08754046ae14cf6d` | E55 | 51243429cf5f |
| I0153 | `src-tauri/src/lib.rs` | `@@ -86,0 +82 @@ pub fn run() {` | `f2fd30e8a06b4b14` | E55 | 51243429cf5f |
| I0154 | `src-tauri/src/lib.rs` | `@@ -149,0 +146 @@ pub fn run() {` | `8c1c48547afb9ad5` | E55 | 51243429cf5f |
| I0155 | `src-tauri/src/lib.rs` | `@@ -302,0 +300,26 @@ fn forward_event(app: &tauri::AppHandle, event: &CoreEvent) {` | `4341e20c1950367c` | E55 | 51243429cf5f; contrib 6551e6744dbd |
| I0156 | `src-tauri/src/media.rs` | `@@ -45,0 +46,2 @@ use opentake_media::{` | `37ae05f1699e15c1` | E55 | 51243429cf5f |
| I0157 | `src-tauri/src/media.rs` | `@@ -216,0 +219,12 @@ pub struct MediaListDto {` | `1215cb079d8452a2` | E55 | 0fbb79f3b6b2 |
| I0158 | `src-tauri/src/media.rs` | `@@ -225 +239 @@ impl MediaListDto {` | `b8475b54255359ba` | E55 | 0fbb79f3b6b2 |
| I0159 | `src-tauri/src/media.rs` | `@@ -228,3 +242 @@ impl MediaListDto {` | `cbb5749eb8e4f438` | E55 | 0fbb79f3b6b2 |
| I0160 | `src-tauri/src/media.rs` | `@@ -233,0 +246 @@ impl MediaListDto {` | `c843ede4a6af4c9a` | E55 | 0fbb79f3b6b2 |
| I0161 | `src-tauri/src/media.rs` | `@@ -259,0 +273 @@ impl MediaListDto {` | `457ba3497f312ad6` | E55 | 0fbb79f3b6b2 |
| I0162 | `src-tauri/src/media.rs` | `@@ -349,0 +364,5 @@ fn write_png(path: &Path, frame: &RgbaFrame) -> Result<(), String> {` | `2d7ce775edd25340` | E55 | 51243429cf5f |
| I0163 | `src-tauri/src/media.rs` | `@@ -359 +378 @@ fn write_png(path: &Path, frame: &RgbaFrame) -> Result<(), String> {` | `c214e2086df1214b` | E55 | 51243429cf5f |
| I0164 | `src-tauri/src/media.rs` | `@@ -645,8 +663,0 @@ pub(crate) const IMPORT_ACCEPTED_MIMES: &str =` | `42c58ec8b0b8bcfd` | E55 | 0fbb79f3b6b2 |
| I0165 | `src-tauri/src/media.rs` | `@@ -665 +675,0 @@ pub(crate) fn import_one(` | `aa4692db808c6c45` | E55 | 0fbb79f3b6b2 |
| I0166 | `src-tauri/src/media.rs` | `@@ -669,13 +679,30 @@ pub(crate) fn import_one(` | `f5d393ee91b3d137` | E55 | f99da16c27b4; contrib 0fbb79f3b6b2 |
| I0167 | `src-tauri/src/media.rs` | `@@ -683,3 +709,0 @@ fn warm_import_poster(engine: &MediaEngine, entry: &MediaManifestEntry, path: &P` | `de9cb43b6e8690f5` | E55 | 0fbb79f3b6b2 |
| I0168 | `src-tauri/src/media.rs` | `@@ -700,0 +725 @@ pub fn import_folder(` | `8e603dbb1deef8ee` | E55 | 0fbb79f3b6b2 |
| I0169 | `src-tauri/src/media.rs` | `@@ -710,0 +736 @@ pub fn import_folder(` | `c80a164c88b82f34` | E55 | 0fbb79f3b6b2 |
| I0170 | `src-tauri/src/media.rs` | `@@ -712 +738,9 @@ pub fn import_folder(` | `30079fb793d4b5b6` | E55 | 0fbb79f3b6b2 |
| I0171 | `src-tauri/src/media.rs` | `@@ -716 +750,5 @@ pub fn import_folder(` | `30067b685ea1ca7e` | E55 | 0fbb79f3b6b2 |
| I0172 | `src-tauri/src/media.rs` | `@@ -720 +758 @@ pub fn import_folder(` | `fc6994e4d633bf7b` | E55 | 0fbb79f3b6b2 |
| I0173 | `src-tauri/src/media.rs` | `@@ -723,0 +762 @@ pub fn import_folder(` | `bf8a8b4c42e70678` | E55 | 0fbb79f3b6b2 |
| I0174 | `src-tauri/src/media.rs` | `@@ -737,0 +777,41 @@ pub(crate) fn mirror_dir(` | `535562288d8f8b78` | E55 | 0fbb79f3b6b2 |
| I0175 | `src-tauri/src/media.rs` | `@@ -748,0 +829,3 @@ pub(crate) fn mirror_dir(` | `fd84d66c4c3cb767` | E55 | 0fbb79f3b6b2 |
| I0176 | `src-tauri/src/media.rs` | `@@ -762 +845,9 @@ pub(crate) fn mirror_dir(` | `6b36bb6e7350bd23` | E55 | 0fbb79f3b6b2 |
| I0177 | `src-tauri/src/media.rs` | `@@ -841,0 +933 @@ pub fn import_media(` | `8e603dbb1deef8ee` | E55 | 0fbb79f3b6b2 |
| I0178 | `src-tauri/src/media.rs` | `@@ -845,0 +938 @@ pub fn import_media(` | `c80a164c88b82f34` | E55 | 0fbb79f3b6b2 |
| I0179 | `src-tauri/src/media.rs` | `@@ -859 +952,5 @@ pub fn import_media(` | `e95f4148f0bcfa0b` | E55 | 0fbb79f3b6b2 |
| I0180 | `src-tauri/src/media.rs` | `@@ -861 +958 @@ pub fn import_media(` | `fc6994e4d633bf7b` | E55 | 0fbb79f3b6b2 |
| I0181 | `src-tauri/src/media.rs` | `@@ -864,0 +962 @@ pub fn import_media(` | `bf8a8b4c42e70678` | E55 | 0fbb79f3b6b2 |
| I0182 | `src-tauri/src/media.rs` | `@@ -950,0 +1049 @@ pub fn save_clip_as_media(` | `8e603dbb1deef8ee` | E55 | 0fbb79f3b6b2 |
| I0183 | `src-tauri/src/media.rs` | `@@ -981,3 +1080,5 @@ pub fn save_clip_as_media(` | `f433cc5397d2fdf7` | E55 | 0fbb79f3b6b2 |
| I0184 | `src-tauri/src/media.rs` | `@@ -985,0 +1087,2 @@ pub fn save_clip_as_media(` | `1987e5ba4584bdee` | E55 | 0fbb79f3b6b2 |
| I0185 | `src-tauri/src/media.rs` | `@@ -1228,6 +1331,4 @@ pub fn get_waveform(` | `126794bc65cf430b` | E55 | 51243429cf5f |
| I0186 | `src-tauri/src/media.rs` | `@@ -1235,5 +1336,3 @@ pub fn get_waveform(` | `ce7dce68ada250de` | E55 | 51243429cf5f |
| I0187 | `src-tauri/src/media.rs` | `@@ -1243,0 +1343 @@ pub fn preload_media(` | `8e603dbb1deef8ee` | E55 | 51243429cf5f |
| I0188 | `src-tauri/src/media.rs` | `@@ -1245,4 +1345,4 @@ pub fn preload_media(` | `fa10412695a09d03` | E55 | 51243429cf5f |
| I0189 | `src-tauri/src/media.rs` | `@@ -1250,5 +1350,2 @@ pub fn preload_media(` | `3b0f582fafda98d9` | E55 | 51243429cf5f |
| I0190 | `src-tauri/src/media.rs` | `@@ -1257 +1354 @@ pub fn preload_media(` | `c166c325490af99a` | E55 | 51243429cf5f |
| I0191 | `src-tauri/src/media.rs` | `@@ -1259,3 +1356,54 @@ pub fn preload_media(` | `57971ad01f51dfa2` | E55 | 51243429cf5f |
| I0192 | `src-tauri/src/media.rs` | `@@ -1263 +1410,0 @@ pub fn preload_media(` | `33160d516f16ae4f` | E55 | 51243429cf5f |
| I0193 | `src-tauri/src/media.rs` | `@@ -1436,0 +1584,101 @@ mod tests {` | `e78b79ac1766ab86` | E55 | f99da16c27b4; contrib 0fbb79f3b6b2 |
| I0194 | `src-tauri/src/media.rs` | `@@ -1471,0 +1720 @@ mod tests {` | `6a601f4e11eb7ed0` | E55 | 0fbb79f3b6b2 |
| I0195 | `src-tauri/src/media.rs` | `@@ -1473 +1722,10 @@ mod tests {` | `9b26df9ec1e49a5a` | E55 | 0fbb79f3b6b2 |
| I0196 | `src-tauri/src/media.rs` | `@@ -1502,0 +1761 @@ mod tests {` | `6a601f4e11eb7ed0` | E55 | 0fbb79f3b6b2 |
| I0197 | `src-tauri/src/media.rs` | `@@ -1504 +1763,10 @@ mod tests {` | `9b26df9ec1e49a5a` | E55 | 0fbb79f3b6b2 |
| I0198 | `src-tauri/src/media.rs` | `@@ -1696,0 +1965 @@ mod tests {` | `0ab7af21fc3f19ba` | E55 | 0fbb79f3b6b2 |
| I0199 | `src-tauri/src/media.rs` | `@@ -1704,0 +1974 @@ mod tests {` | `0ab7af21fc3f19ba` | E55 | 0fbb79f3b6b2 |
| I0200 | `src-tauri/src/media.rs` | `@@ -1716 +1986 @@ mod tests {` | `292e661bb74bc00b` | E55 | 0fbb79f3b6b2 |
| I0201 | `src-tauri/src/media.rs` | `@@ -1719,0 +1990 @@ mod tests {` | `54f89162ecf72b50` | E55 | 0fbb79f3b6b2 |
| I0202 | `src-tauri/src/media/prewarm.rs` | `@@ -0,0 +1,692 @@` | `3145a302558caa27` | E55 | 0fbb79f3b6b2; contrib 51243429cf5f |
| I0203 | `src-tauri/src/playback/audio.rs` | `@@ -4 +4 @@` | `77083686ecad2afc` | E53 | ba5b1ceac463 |
| I0204 | `src-tauri/src/playback/audio.rs` | `@@ -24,2 +24,3 @@ use std::collections::HashMap;` | `c8f6257c2841b328` | E53 | f4a098331536; contrib 6551e6744dbd,f5aa9646da26 |
| I0205 | `src-tauri/src/playback/audio.rs` | `@@ -33 +34,3 @@ use opentake_domain::{Clip, ClipType, Timeline};` | `fd5feaa263e35c51` | E53 | f5aa9646da26 |
| I0206 | `src-tauri/src/playback/audio.rs` | `@@ -43,0 +47,185 @@ const MIX_CHANNELS: usize = 2;` | `de28d01ad7971e4a` | E53 | f4a098331536; contrib f5aa9646da26,b5e5596a43ac,5a9c75b08db3 |
| I0207 | `src-tauri/src/playback/audio.rs` | `@@ -84,0 +274,6 @@ pub struct AudioPlayback {` | `b2f9abea334a55c6` | E52 | 6551e6744dbd |
| I0208 | `src-tauri/src/playback/audio.rs` | `@@ -92,0 +292 @@ impl AudioPlayback {` | `be627feef78d141e` | E52 | 6551e6744dbd |
| I0209 | `src-tauri/src/playback/audio.rs` | `@@ -95 +295 @@ impl AudioPlayback {` | `5783256197a66eac` | E52 | 6551e6744dbd |
| I0210 | `src-tauri/src/playback/audio.rs` | `@@ -108,0 +310,95 @@ impl AudioPlayback {` | `d63cd78da1018969` | E53 | b5e5596a43ac; contrib 6551e6744dbd,f5aa9646da26 |
| I0211 | `src-tauri/src/playback/audio.rs` | `@@ -125 +421,2 @@ fn audio_thread(` | `f4cffb441293392e` | E52 | 6551e6744dbd |
| I0212 | `src-tauri/src/playback/audio.rs` | `@@ -131,2 +428,13 @@ fn audio_thread(` | `775ef25420e98281` | E52 | 6551e6744dbd |
| I0213 | `src-tauri/src/playback/audio.rs` | `@@ -222,0 +546,6 @@ where` | `14e40766126251b3` | E52 | 6551e6744dbd |
| I0214 | `src-tauri/src/playback/audio.rs` | `@@ -281 +610,2 @@ fn project_clip_audio_stereo(` | `07cefbeaceeb7fa7` | E53 | f5aa9646da26 |
| I0215 | `src-tauri/src/playback/audio.rs` | `@@ -283 +613 @@ fn project_clip_audio_stereo(` | `07afb7af494a46bf` | E53 | f5aa9646da26 |
| I0216 | `src-tauri/src/playback/audio.rs` | `@@ -285,2 +615,6 @@ fn project_clip_audio_stereo(` | `7472d83e9d22aa19` | E53 | f5aa9646da26 |
| I0217 | `src-tauri/src/playback/audio.rs` | `@@ -293 +627,2 @@ fn project_clip_audio_stereo(` | `1cc3da955954b26c` | E53 | f5aa9646da26 |
| I0218 | `src-tauri/src/playback/audio.rs` | `@@ -296 +631 @@ fn project_clip_audio_stereo(` | `07afb7af494a46bf` | E53 | f5aa9646da26 |
| I0219 | `src-tauri/src/playback/audio.rs` | `@@ -302 +637,4 @@ fn project_clip_audio_stereo(` | `dcc14f68ccc649c1` | E53 | f5aa9646da26 |
| I0220 | `src-tauri/src/playback/audio.rs` | `@@ -313 +651 @@ fn project_clip_audio_stereo(` | `1536d11a39cbbc5b` | E53 | f5aa9646da26 |
| I0221 | `src-tauri/src/playback/audio.rs` | `@@ -317 +655 @@ fn project_clip_audio_stereo(` | `3acaa44849f40904` | E53 | f5aa9646da26 |
| I0222 | `src-tauri/src/playback/audio.rs` | `@@ -322 +660 @@ fn project_clip_audio_stereo(` | `c8e82964dbb67269` | E53 | f5aa9646da26 |
| I0223 | `src-tauri/src/playback/audio.rs` | `@@ -325 +663,7 @@ fn mix_stereo(clips: &[StereoClip]) -> Vec<f32> {` | `2fb63e3d5f35b5a0` | E53 | f5aa9646da26 |
| I0224 | `src-tauri/src/playback/audio.rs` | `@@ -328 +672,8 @@ fn mix_stereo(clips: &[StereoClip]) -> Vec<f32> {` | `5c16385b51feb58e` | E53 | f5aa9646da26 |
| I0225 | `src-tauri/src/playback/audio.rs` | `@@ -331,5 +682,11 @@ fn mix_stereo(clips: &[StereoClip]) -> Vec<f32> {` | `09420882d747a06a` | E53 | f5aa9646da26 |
| I0226 | `src-tauri/src/playback/audio.rs` | `@@ -338,2 +695,7 @@ fn mix_stereo(clips: &[StereoClip]) -> Vec<f32> {` | `444fe5ea25d22e9a` | E53 | f5aa9646da26 |
| I0227 | `src-tauri/src/playback/audio.rs` | `@@ -341 +703 @@ fn mix_stereo(clips: &[StereoClip]) -> Vec<f32> {` | `034226973420c15a` | E53 | f5aa9646da26 |
| I0228 | `src-tauri/src/playback/audio.rs` | `@@ -350 +712,2 @@ fn mix_timeline_stereo(` | `f7df33269131dc85` | E53 | f5aa9646da26 |
| I0229 | `src-tauri/src/playback/audio.rs` | `@@ -352 +715,7 @@ fn mix_timeline_stereo(` | `9e4c41a250a947b4` | E53 | f4a098331536; contrib f5aa9646da26 |
| I0230 | `src-tauri/src/playback/audio.rs` | `@@ -359,0 +729,3 @@ fn mix_timeline_stereo(` | `4d3ff5e099ee4dd0` | E53 | f5aa9646da26 |
| I0231 | `src-tauri/src/playback/audio.rs` | `@@ -363 +735 @@ fn mix_timeline_stereo(` | `d7f56df1f0ace2de` | E53 | f5aa9646da26 |
| I0232 | `src-tauri/src/playback/audio.rs` | `@@ -369 +741 @@ fn mix_timeline_stereo(` | `b99a056c8098ab34` | E53 | f5aa9646da26 |
| I0233 | `src-tauri/src/playback/audio.rs` | `@@ -371 +743,39 @@ fn mix_timeline_stereo(` | `47ea099633053c1f` | E53 | f4a098331536; contrib 6551e6744dbd,f5aa9646da26,5a9c75b08db3 |
| I0234 | `src-tauri/src/playback/audio.rs` | `@@ -374,7 +784 @@ fn mix_timeline_stereo(` | `15600824a1d20ff8` | E52 | 6551e6744dbd |
| I0235 | `src-tauri/src/playback/audio.rs` | `@@ -385 +789,3 @@ pub fn build_clock(` | `78e7353943f21f66` | E53 | f5aa9646da26; contrib 6551e6744dbd |
| I0236 | `src-tauri/src/playback/audio.rs` | `@@ -387 +793 @@ pub fn build_clock(` | `e15cb0a7fbd25ee0` | E53 | f5aa9646da26 |
| I0237 | `src-tauri/src/playback/audio.rs` | `@@ -389 +795 @@ pub fn build_clock(` | `7ca23cf26afc3dbd` | E53 | f5aa9646da26 |
| I0238 | `src-tauri/src/playback/audio.rs` | `@@ -401,2 +808,2 @@ pub fn build_clock(` | `450164fb9a741119` | E53 | f5aa9646da26; contrib 6551e6744dbd |
| I0239 | `src-tauri/src/playback/audio.rs` | `@@ -405 +812 @@ pub fn build_clock(` | `000d93d122c72c3f` | E53 | f5aa9646da26 |
| I0240 | `src-tauri/src/playback/audio.rs` | `@@ -413 +820,335 @@ mod tests {` | `23fa0993001d704b` | E53 | f4a098331536; contrib f5aa9646da26,b5e5596a43ac |
| I0241 | `src-tauri/src/playback/audio.rs` | `@@ -477 +1218,5 @@ mod tests {` | `541f2fbd6a77586f` | E53 | f5aa9646da26 |
| I0242 | `src-tauri/src/playback/audio.rs` | `@@ -484 +1229,5 @@ mod tests {` | `f3815041680c4bcd` | E53 | f5aa9646da26 |
| I0243 | `src-tauri/src/playback/audio.rs` | `@@ -501 +1250 @@ mod tests {` | `c2e59adb4a476579` | E53 | f5aa9646da26 |
| I0244 | `src-tauri/src/playback/audio.rs` | `@@ -518 +1267 @@ mod tests {` | `bd8a570859f8f05e` | E53 | f5aa9646da26 |
| I0245 | `src-tauri/src/playback/commands.rs` | `@@ -21,2 +21,6 @@ use opentake_render::{even, RenderSize};` | `6f9b5c286d4c5858` | E53 | 5a9c75b08db3; contrib f5aa9646da26 |
| I0246 | `src-tauri/src/playback/commands.rs` | `@@ -24 +28,5 @@ use super::project::{project_media, project_text};` | `6160bde00d441933` | E52 | 6551e6744dbd |
| I0247 | `src-tauri/src/playback/commands.rs` | `@@ -33,0 +42 @@ struct RunningPlayback {` | `7ccacaff7c61c5da` | E52 | 6551e6744dbd |
| I0248 | `src-tauri/src/playback/commands.rs` | `@@ -35 +44,40 @@ struct RunningPlayback {` | `a8d96f2511a07fd2` | E53 | 5a9c75b08db3; contrib 6551e6744dbd,f5aa9646da26 |
| I0249 | `src-tauri/src/playback/commands.rs` | `@@ -38,2 +85,0 @@ struct RunningPlayback {` | `a72e3dca4198d6e7` | E52 | 6551e6744dbd |
| I0250 | `src-tauri/src/playback/commands.rs` | `@@ -40,0 +87,11 @@ struct RunningPlayback {` | `244bf722ac93953a` | E53 | b5e5596a43ac; contrib 6551e6744dbd |
| I0251 | `src-tauri/src/playback/commands.rs` | `@@ -42 +99,13 @@ pub struct PlaybackState {` | `27c27b8358e4a784` | E53 | f5aa9646da26; contrib 6551e6744dbd |
| I0252 | `src-tauri/src/playback/commands.rs` | `@@ -50,8 +119,144 @@ impl PlaybackState {` | `147179add5774846` | E55 | 51243429cf5f; contrib 6551e6744dbd,f5aa9646da26,b5e5596a43ac,5a9c75b08db3,24ab2590ce96 |
| I0253 | `src-tauri/src/playback/commands.rs` | `@@ -59,3 +264,136 @@ impl PlaybackState {` | `2c7bea0236b135ce` | E54 | 24ab2590ce96; contrib 6551e6744dbd,7a0a05cb1dd2,e2daeb279a33,f5aa9646da26,b5e5596a43ac,de58ba008358 |
| I0254 | `src-tauri/src/playback/commands.rs` | `@@ -63,2 +401,2 @@ impl PlaybackState {` | `0047ac91d2db6eb5` | E53 | f5aa9646da26; contrib 6551e6744dbd |
| I0255 | `src-tauri/src/playback/commands.rs` | `@@ -68,5 +406,9 @@ impl PlaybackState {` | `8fa1e46701d70470` | E53 | b5e5596a43ac; contrib 6551e6744dbd,7a0a05cb1dd2 |
| I0256 | `src-tauri/src/playback/commands.rs` | `@@ -74,2 +416,3 @@ impl PlaybackState {` | `f5ed66ac3b382569` | E53 | f5aa9646da26; contrib 6551e6744dbd |
| I0257 | `src-tauri/src/playback/commands.rs` | `@@ -76,0 +420 @@ impl PlaybackState {` | `a560e37b0ac1076b` | E52 | 6551e6744dbd |
| I0258 | `src-tauri/src/playback/commands.rs` | `@@ -79,5 +423,27 @@ impl PlaybackState {` | `dad8b415fcd2c129` | E53 | b5e5596a43ac; contrib 6551e6744dbd,f5aa9646da26 |
| I0259 | `src-tauri/src/playback/commands.rs` | `@@ -84,0 +451,10 @@ impl PlaybackState {` | `48f9b1c7cb375f81` | E52 | 6551e6744dbd |
| I0260 | `src-tauri/src/playback/commands.rs` | `@@ -104,0 +481,42 @@ fn playback_render_size(canvas_w: i32, canvas_h: i32, cap: u32) -> RenderSize {` | `b9004adbbf720d6e` | E53 | 5a9c75b08db3; contrib 7a0a05cb1dd2,f5aa9646da26,b5e5596a43ac |
| I0261 | `src-tauri/src/playback/commands.rs` | `@@ -112 +530,23 @@ fn playback_render_size(canvas_w: i32, canvas_h: i32, cap: u32) -> RenderSize {` | `b9925036ad0f785e` | E53 | 5a9c75b08db3; contrib 6551e6744dbd,b5e5596a43ac |
| I0262 | `src-tauri/src/playback/commands.rs` | `@@ -115,5 +555,4 @@ pub async fn playback_start(app: AppHandle, from_frame: i32) -> Result<(), Strin` | `b9b34108cef67601` | E53 | f5aa9646da26; contrib 6551e6744dbd |
| I0263 | `src-tauri/src/playback/commands.rs` | `@@ -125,2 +564,9 @@ pub async fn playback_start(app: AppHandle, from_frame: i32) -> Result<(), Strin` | `7ef5f0604c7b411d` | E53 | f5aa9646da26; contrib 6551e6744dbd |
| I0264 | `src-tauri/src/playback/commands.rs` | `@@ -135,0 +582,2 @@ pub async fn playback_start(app: AppHandle, from_frame: i32) -> Result<(), Strin` | `705a2894886a19bf` | E53 | f5aa9646da26; contrib 6551e6744dbd |
| I0265 | `src-tauri/src/playback/commands.rs` | `@@ -143 +590 @@ pub async fn playback_start(app: AppHandle, from_frame: i32) -> Result<(), Strin` | `1462423d183bc230` | E53 | f5aa9646da26 |
| I0266 | `src-tauri/src/playback/commands.rs` | `@@ -146,3 +593,65 @@ pub async fn playback_start(app: AppHandle, from_frame: i32) -> Result<(), Strin` | `4e3207e24e2abe5c` | E54 | de58ba008358; contrib 7a0a05cb1dd2,f5aa9646da26,b5e5596a43ac,5a9c75b08db3 |
| I0267 | `src-tauri/src/playback/commands.rs` | `@@ -149,0 +659,16 @@ pub async fn playback_start(app: AppHandle, from_frame: i32) -> Result<(), Strin` | `c14a01f2078ded00` | E54 | 24ab2590ce96; contrib 6551e6744dbd,f5aa9646da26,de58ba008358 |
| I0268 | `src-tauri/src/playback/commands.rs` | `@@ -151,16 +676 @@ pub async fn playback_start(app: AppHandle, from_frame: i32) -> Result<(), Strin` | `0a9a56500ac95d40` | E52 | 6551e6744dbd |
| I0269 | `src-tauri/src/playback/commands.rs` | `@@ -168,3 +678,6 @@ pub async fn playback_start(app: AppHandle, from_frame: i32) -> Result<(), Strin` | `734d0ca314c10532` | E52 | 6551e6744dbd |
| I0270 | `src-tauri/src/playback/commands.rs` | `@@ -175,3 +688,5 @@ pub fn playback_pause(playback: State<'_, PlaybackState>) -> Result<(), String>` | `3e46d64ec071fac3` | E52 | 6551e6744dbd |
| I0271 | `src-tauri/src/playback/commands.rs` | `@@ -182,3 +697,6 @@ pub fn playback_stop(playback: State<'_, PlaybackState>) -> Result<(), String> {` | `5d713fd550d463c5` | E52 | 6551e6744dbd |
| I0272 | `src-tauri/src/playback/commands.rs` | `@@ -187,3 +705 @@ pub fn playback_seek(playback: State<'_, PlaybackState>, frame: i32) -> Result<(` | `de1e1d0ad8939be0` | E52 | 6551e6744dbd |
| I0273 | `src-tauri/src/playback/commands.rs` | `@@ -197,0 +714,964 @@ mod tests {` | `899de302bab5d645` | E54 | 24ab2590ce96; contrib 7a0a05cb1dd2,e2daeb279a33,f5aa9646da26,b5e5596a43ac,5a9c75b08db3,f4a098331536,de58ba008358 |
| I0274 | `src-tauri/src/playback/engine.rs` | `@@ -17,0 +18 @@ use std::collections::HashMap;` | `b1e06d4c609141f0` | E53 | f5aa9646da26 |
| I0275 | `src-tauri/src/playback/engine.rs` | `@@ -23,0 +25 @@ use opentake_domain::Timeline;` | `7a196b1a8fe7bb5d` | E54 | c7b74a621cbd |
| I0276 | `src-tauri/src/playback/engine.rs` | `@@ -30,0 +33,95 @@ use super::resolver::{PlaybackResolverState, StreamingResolver};` | `88a94cfcdc5c6dd1` | E53 | b5e5596a43ac; contrib f5aa9646da26 |
| I0277 | `src-tauri/src/playback/engine.rs` | `@@ -54,0 +152,4 @@ pub enum PlaybackCmd {` | `c764273c2016b376` | E52 | 6551e6744dbd |
| I0278 | `src-tauri/src/playback/engine.rs` | `@@ -131,0 +233,18 @@ impl RenderLoop {` | `00e8dac4e7717870` | E54 | c7b74a621cbd |
| I0279 | `src-tauri/src/playback/engine.rs` | `@@ -141,0 +261 @@ impl RenderLoop {` | `a8b892aca327e808` | E54 | c7b74a621cbd |
| I0280 | `src-tauri/src/playback/engine.rs` | `@@ -167 +287 @@ impl RenderLoop {` | `f584910f4aeb8b0c` | E54 | ddf08f0b917b |
| I0281 | `src-tauri/src/playback/engine.rs` | `@@ -190,0 +311 @@ pub struct PlaybackEngine {` | `2e1376ba75423b07` | E54 | c7b74a621cbd |
| I0282 | `src-tauri/src/playback/engine.rs` | `@@ -206,0 +328,104 @@ impl PlaybackEngine {` | `aeaec4162677a303` | E54 | de58ba008358; contrib 6551e6744dbd,7a0a05cb1dd2 |
| I0283 | `src-tauri/src/playback/engine.rs` | `@@ -208,0 +434 @@ impl PlaybackEngine {` | `4fa1ea156dac372e` | E54 | c7b74a621cbd |
| I0284 | `src-tauri/src/playback/engine.rs` | `@@ -221,0 +448,3 @@ impl PlaybackEngine {` | `14d7a3cc63dcac37` | E54 | c7b74a621cbd; contrib 6551e6744dbd |
| I0285 | `src-tauri/src/playback/engine.rs` | `@@ -227,0 +457 @@ impl PlaybackEngine {` | `a8b892aca327e808` | E54 | c7b74a621cbd |
| I0286 | `src-tauri/src/playback/engine.rs` | `@@ -235,0 +466,18 @@ impl PlaybackEngine {` | `8000efc2473f77ce` | E52 | 6551e6744dbd |
| I0287 | `src-tauri/src/playback/engine.rs` | `@@ -237,0 +486 @@ impl PlaybackEngine {` | `454a369b5d3d281b` | E54 | c7b74a621cbd |
| I0288 | `src-tauri/src/playback/engine.rs` | `@@ -242,0 +492,34 @@ impl PlaybackEngine {` | `e564a5012a5ab939` | E54 | c7b74a621cbd; contrib f5aa9646da26 |
| I0289 | `src-tauri/src/playback/engine.rs` | `@@ -247,0 +531 @@ impl Drop for PlaybackEngine {` | `454a369b5d3d281b` | E54 | c7b74a621cbd |
| I0290 | `src-tauri/src/playback/engine.rs` | `@@ -267,0 +552,3 @@ fn run_render_thread(` | `544b4e733055e8be` | E54 | c7b74a621cbd; contrib 6551e6744dbd |
| I0291 | `src-tauri/src/playback/engine.rs` | `@@ -269,7 +556,11 @@ fn run_render_thread(` | `9721606c012adead` | E54 | c7b74a621cbd; contrib 6551e6744dbd |
| I0292 | `src-tauri/src/playback/engine.rs` | `@@ -278,0 +570,3 @@ fn run_render_thread(` | `21f6429a3854b250` | E52 | 6551e6744dbd |
| I0293 | `src-tauri/src/playback/engine.rs` | `@@ -281,0 +579,2 @@ fn run_render_thread(` | `a01675ffce7fa706` | E52 | 6551e6744dbd |
| I0294 | `src-tauri/src/playback/engine.rs` | `@@ -283,0 +583,25 @@ fn run_render_thread(` | `4ff044da8d4b310e` | E52 | 6551e6744dbd |
| I0295 | `src-tauri/src/playback/engine.rs` | `@@ -288,0 +613,11 @@ fn run_render_thread(` | `bb378d0c20ba84fa` | E52 | 6551e6744dbd |
| I0296 | `src-tauri/src/playback/engine.rs` | `@@ -302,2 +641,17 @@ fn run_render_thread(` | `b387c54d57af19c1` | E52 | 6551e6744dbd |
| I0297 | `src-tauri/src/playback/engine.rs` | `@@ -305 +658,0 @@ fn run_render_thread(` | `c5e84c021be9fdd5` | E52 | 6551e6744dbd |
| I0298 | `src-tauri/src/playback/engine.rs` | `@@ -309,2 +662,3 @@ fn run_render_thread(` | `79f2fb4e1edca90a` | E52 | 6551e6744dbd |
| I0299 | `src-tauri/src/playback/engine.rs` | `@@ -329,0 +684,32 @@ mod tests {` | `a3e74f8103f9afb5` | E53 | f5aa9646da26 |
| I0300 | `src-tauri/src/playback/mod.rs` | `@@ -20,0 +21 @@ pub mod resolver;` | `c919ec0f6e063f75` | E52 | 6551e6744dbd |
| I0301 | `src-tauri/src/playback/resolver.rs` | `@@ -23,4 +23,4 @@` | `254acbe16b68cbae` | E54 | c7b74a621cbd |
| I0302 | `src-tauri/src/playback/resolver.rs` | `@@ -29,0 +30 @@ use std::rc::Rc;` | `4ccb134a872c5b3a` | E54 | c7b74a621cbd |
| I0303 | `src-tauri/src/playback/resolver.rs` | `@@ -34 +35 @@ use opentake_media::decode::{` | `94d3ae3fd56a1211` | E54 | c7b74a621cbd |
| I0304 | `src-tauri/src/playback/resolver.rs` | `@@ -59 +60,4 @@ struct ClipStream {` | `507cb4b78517fa31` | E54 | c7b74a621cbd; contrib ddf08f0b917b |
| I0305 | `src-tauri/src/playback/resolver.rs` | `@@ -63 +67 @@ impl ClipStream {` | `ed5776e1516fa335` | E54 | c7b74a621cbd |
| I0306 | `src-tauri/src/playback/resolver.rs` | `@@ -67 +71,2 @@ impl ClipStream {` | `6042271b7d01e4c5` | E54 | c7b74a621cbd; contrib ddf08f0b917b |
| I0307 | `src-tauri/src/playback/resolver.rs` | `@@ -71,5 +76,11 @@ impl ClipStream {` | `04f633d262e7f90a` | E54 | c7b74a621cbd; contrib 7a0a05cb1dd2,ddf08f0b917b |
| I0308 | `src-tauri/src/playback/resolver.rs` | `@@ -78,3 +88,0 @@ impl ClipStream {` | `f550d9b40d59ac15` | E54 | c7b74a621cbd |
| I0309 | `src-tauri/src/playback/resolver.rs` | `@@ -83 +91 @@ impl ClipStream {` | `3587907597c51dd8` | E54 | c7b74a621cbd |
| I0310 | `src-tauri/src/playback/resolver.rs` | `@@ -85 +93 @@ impl ClipStream {` | `00056e1d9db6e72f` | E54 | c7b74a621cbd |
| I0311 | `src-tauri/src/playback/resolver.rs` | `@@ -87,0 +96 @@ impl ClipStream {` | `a15aa75523785a45` | E54 | c7b74a621cbd |
| I0312 | `src-tauri/src/playback/resolver.rs` | `@@ -90 +99 @@ impl ClipStream {` | `dcd9100d35cf13c6` | E54 | ddf08f0b917b |
| I0313 | `src-tauri/src/playback/resolver.rs` | `@@ -91,0 +101,45 @@ impl ClipStream {` | `c3be7a1f23359ced` | E54 | c7b74a621cbd; contrib 7a0a05cb1dd2,ddf08f0b917b |
| I0314 | `src-tauri/src/playback/resolver.rs` | `@@ -103 +157 @@ impl ClipStream {` | `90db47606c49f76b` | E54 | c7b74a621cbd |
| I0315 | `src-tauri/src/playback/resolver.rs` | `@@ -105 +159 @@ fn drain_to_target(` | `124f3f74077e527e` | E54 | c7b74a621cbd |
| I0316 | `src-tauri/src/playback/resolver.rs` | `@@ -107 +161 @@ fn drain_to_target(` | `7c8c59213c119fa8` | E54 | c7b74a621cbd |
| I0317 | `src-tauri/src/playback/resolver.rs` | `@@ -111 +165 @@ fn drain_to_target(` | `e5ed73ed9cfca527` | E54 | c7b74a621cbd |
| I0318 | `src-tauri/src/playback/resolver.rs` | `@@ -115 +169 @@ fn drain_to_target(` | `ce3f8075606aee91` | E54 | c7b74a621cbd |
| I0319 | `src-tauri/src/playback/resolver.rs` | `@@ -119 +173 @@ fn drain_to_target(` | `a9af9d7203b641d3` | E54 | c7b74a621cbd |
| I0320 | `src-tauri/src/playback/resolver.rs` | `@@ -124 +178 @@ fn drain_to_target(` | `f64cc271b2938426` | E54 | c7b74a621cbd |
| I0321 | `src-tauri/src/playback/resolver.rs` | `@@ -128 +182 @@ fn drain_to_target(` | `c305e3ea632b3fa9` | E54 | c7b74a621cbd |
| I0322 | `src-tauri/src/playback/resolver.rs` | `@@ -130 +184,10 @@ fn drain_to_target(` | `18e9748cdb7cc9aa` | E54 | c7b74a621cbd |
| I0323 | `src-tauri/src/playback/resolver.rs` | `@@ -148,0 +212 @@ pub struct PlaybackResolverState {` | `c3af74b33afcd460` | E54 | c7b74a621cbd |
| I0324 | `src-tauri/src/playback/resolver.rs` | `@@ -156,0 +221 @@ impl PlaybackResolverState {` | `e3a2959d1fcba50a` | E54 | c7b74a621cbd |
| I0325 | `src-tauri/src/playback/resolver.rs` | `@@ -165,0 +231 @@ impl PlaybackResolverState {` | `cab8454d484aca60` | E54 | c7b74a621cbd |
| I0326 | `src-tauri/src/playback/resolver.rs` | `@@ -213 +279 @@ pub struct StreamingResolver<'d, 's> {` | `bf3a87d624f57127` | E54 | c7b74a621cbd |
| I0327 | `src-tauri/src/playback/resolver.rs` | `@@ -234,2 +300,2 @@ impl<'d, 's> StreamingResolver<'d, 's> {` | `a9a7f20f49b0b325` | E54 | ddf08f0b917b |
| I0328 | `src-tauri/src/playback/resolver.rs` | `@@ -238 +304 @@ impl<'d, 's> StreamingResolver<'d, 's> {` | `25374646e8690a18` | E54 | ddf08f0b917b |
| I0329 | `src-tauri/src/playback/resolver.rs` | `@@ -255 +321 @@ impl<'d, 's> StreamingResolver<'d, 's> {` | `df5bcc264eb82243` | E54 | c7b74a621cbd |
| I0330 | `src-tauri/src/playback/resolver.rs` | `@@ -257,13 +323,39 @@ impl<'d, 's> StreamingResolver<'d, 's> {` | `fede0ddf9d544d69` | E54 | c7b74a621cbd; contrib 6551e6744dbd,ddf08f0b917b |
| I0331 | `src-tauri/src/playback/resolver.rs` | `@@ -271,4 +363,6 @@ impl<'d, 's> StreamingResolver<'d, 's> {` | `824beaf9efcef734` | E54 | c7b74a621cbd; contrib ddf08f0b917b |
| I0332 | `src-tauri/src/playback/resolver.rs` | `@@ -279,2 +373,13 @@ impl<'d, 's> StreamingResolver<'d, 's> {` | `d373a8c7be865d2c` | E54 | c7b74a621cbd |
| I0333 | `src-tauri/src/playback/resolver.rs` | `@@ -281,0 +387 @@ impl<'d, 's> StreamingResolver<'d, 's> {` | `d2e686f036129ded` | E54 | ddf08f0b917b |
| I0334 | `src-tauri/src/playback/resolver.rs` | `@@ -298 +404,2 @@ impl<'d, 's> StreamingResolver<'d, 's> {` | `44d75589c05b66c9` | E54 | c7b74a621cbd |
| I0335 | `src-tauri/src/playback/resolver.rs` | `@@ -340,4 +447,4 @@ impl TextureResolver for StreamingResolver<'_, '_> {` | `dba24e832a4ee4c6` | E54 | c7b74a621cbd |
| I0336 | `src-tauri/src/playback/resolver.rs` | `@@ -347 +454 @@ impl TextureResolver for StreamingResolver<'_, '_> {` | `5e42db4ae353b767` | E54 | c7b74a621cbd |
| I0337 | `src-tauri/src/playback/resolver.rs` | `@@ -361,0 +469,10 @@ mod tests {` | `d4b8d29877faee83` | E54 | ddf08f0b917b |
| I0338 | `src-tauri/src/playback/resolver.rs` | `@@ -371 +488,3 @@ mod tests {` | `f66a00d6b4535cbd` | E54 | c7b74a621cbd |
| I0339 | `src-tauri/src/playback/resolver.rs` | `@@ -373 +492 @@ mod tests {` | `2a5e594fce3e1ca4` | E54 | c7b74a621cbd |
| I0340 | `src-tauri/src/playback/resolver.rs` | `@@ -379 +498 @@ mod tests {` | `6946de49be11d879` | E54 | c7b74a621cbd |
| I0341 | `src-tauri/src/playback/resolver.rs` | `@@ -388 +507 @@ mod tests {` | `f87df8e138ac2d32` | E54 | c7b74a621cbd |
| I0342 | `src-tauri/src/playback/resolver.rs` | `@@ -397 +516 @@ mod tests {` | `3066116a1b063d66` | E54 | c7b74a621cbd |
| I0343 | `src-tauri/src/playback/resolver.rs` | `@@ -406 +525 @@ mod tests {` | `bce9d0607ad6d43e` | E54 | c7b74a621cbd |
| I0344 | `src-tauri/src/playback/resolver.rs` | `@@ -414 +533 @@ mod tests {` | `258e26fa406491c9` | E54 | c7b74a621cbd |
| I0345 | `src-tauri/src/playback/resolver.rs` | `@@ -422 +541 @@ mod tests {` | `6946de49be11d879` | E54 | c7b74a621cbd |
| I0346 | `src-tauri/src/playback/resolver.rs` | `@@ -430 +549 @@ mod tests {` | `258e26fa406491c9` | E54 | c7b74a621cbd |
| I0347 | `src-tauri/src/playback/resolver.rs` | `@@ -433,0 +553,66 @@ mod tests {` | `46b00a01b535ff5d` | E54 | c7b74a621cbd; contrib 7a0a05cb1dd2,ddf08f0b917b |
| I0348 | `src-tauri/src/playback/session.rs` | `@@ -0,0 +1,533 @@` | `c54d6e5a7d11ff2b` | E55 | 51243429cf5f; contrib 6551e6744dbd,7a0a05cb1dd2,e2daeb279a33,b5e5596a43ac,5a9c75b08db3 |
| I0349 | `src-tauri/src/playback/transport.rs` | `@@ -20 +20,2 @@ use std::convert::Infallible;` | `a933e20d4493c34c` | E52 | 6551e6744dbd |
| I0350 | `src-tauri/src/playback/transport.rs` | `@@ -22,0 +24 @@ use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};` | `b7237e5a0e91ee3c` | E52 | 6551e6744dbd |
| I0351 | `src-tauri/src/playback/transport.rs` | `@@ -32,0 +35 @@ use super::engine::{FrameSink, PlayheadEmitter};` | `fa96ec3baf9fa4f0` | E52 | 6551e6744dbd |
| I0352 | `src-tauri/src/playback/transport.rs` | `@@ -53 +56 @@ pub struct PreviewServer {` | `2092df79d0d2079e` | E52 | 6551e6744dbd |
| I0353 | `src-tauri/src/playback/transport.rs` | `@@ -61 +64 @@ struct ServerState {` | `2092df79d0d2079e` | E52 | 6551e6744dbd |
| I0354 | `src-tauri/src/playback/transport.rs` | `@@ -63,0 +67,156 @@ struct ServerState {` | `37264f8ce5a6100b` | E52 | 7a0a05cb1dd2; contrib 6551e6744dbd |
| I0355 | `src-tauri/src/playback/transport.rs` | `@@ -69 +228 @@ impl PreviewServer {` | `d4e8bb9fde476969` | E52 | 6551e6744dbd |
| I0356 | `src-tauri/src/playback/transport.rs` | `@@ -122 +281 @@ impl PreviewServer {` | `a6c754867185a124` | E52 | 6551e6744dbd |
| I0357 | `src-tauri/src/playback/transport.rs` | `@@ -125 +284,7 @@ impl PreviewServer {` | `10410a7949772819` | E52 | e2daeb279a33 |
| I0358 | `src-tauri/src/playback/transport.rs` | `@@ -127,0 +293,4 @@ impl PreviewServer {` | `86c26b057d208e4c` | E52 | 6551e6744dbd |
| I0359 | `src-tauri/src/playback/transport.rs` | `@@ -147,0 +311,22 @@ fn origin_is_allowed(headers: &HeaderMap) -> bool {` | `da83c7e9dbba3c8f` | E56 | 3fe09766819b |
| I0360 | `src-tauri/src/playback/transport.rs` | `@@ -218,5 +403,9 @@ async fn ws_handler(` | `78da5473d7f5e5a1` | E52 | 6551e6744dbd |
| I0361 | `src-tauri/src/playback/transport.rs` | `@@ -226,6 +415,4 @@ async fn frame_handler(State(state): State<ServerState>, headers: HeaderMap) ->` | `fbb1d3193af9316f` | E52 | 6551e6744dbd |
| I0362 | `src-tauri/src/playback/transport.rs` | `@@ -271 +458,68 @@ pub struct MjpegSink {` | `a1cd90c0a84945b3` | E52 | e2daeb279a33; contrib 6551e6744dbd |
| I0363 | `src-tauri/src/playback/transport.rs` | `@@ -275,0 +530,3 @@ impl FrameSink for MjpegSink {` | `15e3d2d2c726cf24` | E52 | e2daeb279a33; contrib 6551e6744dbd |
| I0364 | `src-tauri/src/playback/transport.rs` | `@@ -284,5 +541 @@ impl FrameSink for MjpegSink {` | `bd32a919e5d61f0d` | E52 | e2daeb279a33 |
| I0365 | `src-tauri/src/playback/transport.rs` | `@@ -320 +573 @@ fn encode_jpeg(frame: &DecodedFrame) -> Option<Vec<u8>> {` | `92e553b63aa50836` | E52 | 6551e6744dbd |
| I0366 | `src-tauri/src/playback/transport.rs` | `@@ -322 +575,4 @@ fn encode_jpeg(frame: &DecodedFrame) -> Option<Vec<u8>> {` | `86cdca568787ae36` | E52 | e2daeb279a33; contrib 6551e6744dbd |
| I0367 | `src-tauri/src/playback/transport.rs` | `@@ -323,0 +580,15 @@ struct PlayheadDto {` | `70309246df412ba0` | E52 | e2daeb279a33; contrib 6551e6744dbd |
| I0368 | `src-tauri/src/playback/transport.rs` | `@@ -329,0 +601,2 @@ pub struct TauriPlayheadEmitter {` | `934e4a98e2f2d334` | E52 | e2daeb279a33; contrib 6551e6744dbd |
| I0369 | `src-tauri/src/playback/transport.rs` | `@@ -333,2 +606,6 @@ impl TauriPlayheadEmitter {` | `8b0866c8d5702830` | E52 | e2daeb279a33; contrib 6551e6744dbd |
| I0370 | `src-tauri/src/playback/transport.rs` | `@@ -340 +617,3 @@ impl PlayheadEmitter for TauriPlayheadEmitter {` | `90ffd3df2fcfe256` | E52 | e2daeb279a33 |
| I0371 | `src-tauri/src/playback/transport.rs` | `@@ -398,0 +678,44 @@ mod tests {` | `89a7eea16ffa868a` | E52 | e2daeb279a33; contrib 6551e6744dbd |
| I0372 | `src-tauri/tests/playback_integration.rs` | `@@ -17,3 +17,3 @@ use std::sync::atomic::{AtomicI32, AtomicUsize, Ordering};` | `5ee95b1c464ea0fc` | E54 | c7b74a621cbd |
| I0373 | `src-tauri/tests/playback_integration.rs` | `@@ -23,0 +24 @@ use opentake_domain::{` | `63519525315b673c` | E54 | de58ba008358 |
| I0374 | `src-tauri/tests/playback_integration.rs` | `@@ -24,0 +26 @@ use opentake_render::{DecodedFrame, RenderSize};` | `df5939f8f1559798` | E54 | c7b74a621cbd |
| I0375 | `src-tauri/tests/playback_integration.rs` | `@@ -69,0 +72,35 @@ fn make_video(path: &Path, w: u32, h: u32, fps: u32, frames: u32, hue: i32) -> b` | `a43264a5539a1c3d` | E54 | ddf08f0b917b |
| I0376 | `src-tauri/tests/playback_integration.rs` | `@@ -108,2 +145,6 @@ fn try_render_loop(` | `62dbcf9282655e11` | E52 | 6551e6744dbd |
| I0377 | `src-tauri/tests/playback_integration.rs` | `@@ -122,0 +164,299 @@ fn render_until_content(rl: &mut RenderLoop, target: i32, w: u32, h: u32) -> Opt` | `86c04a63364a8787` | E54 | de58ba008358; contrib ddf08f0b917b,c7b74a621cbd |
| I0378 | `src-tauri/tests/playback_integration.rs` | `@@ -156,3 +496,8 @@ fn render_loop_streams_frames_advances_and_seeks() {` | `2674829c0a17b1d6` | E52 | 6551e6744dbd |
| I0379 | `src-tauri/tests/playback_integration.rs` | `@@ -272,2 +621,9 @@ fn playback_engine_thread_streams_frames_to_sink_and_emitter() {` | `d920f462b1b83899` | E52 | 6551e6744dbd |
| I0380 | `src-tauri/tests/playback_integration.rs` | `@@ -287,17 +646,5 @@ fn playback_engine_thread_streams_frames_to_sink_and_emitter() {` | `c5a44f2b4c38efea` | E52 | 6551e6744dbd |
| I0381 | `src-tauri/tests/playback_integration.rs` | `@@ -306,0 +654,8 @@ fn playback_engine_thread_streams_frames_to_sink_and_emitter() {` | `f86469f0c0aa5af3` | E52 | 6551e6744dbd |
| I0382 | `src-tauri/tests/playback_probe.rs` | `@@ -32 +32 @@ use opentake_tauri_lib::playback::{` | `d6be157f36df8b51` | E53 | 8a9bca069258 |
| I0383 | `src-tauri/tests/playback_probe.rs` | `@@ -74 +74 @@ fn run_engine(` | `5e51f8b30a6c2fde` | E53 | 8a9bca069258 |
| I0384 | `src-tauri/tests/playback_transport_integration.rs` | `@@ -1,8 +1 @@` | `84989e46d146d430` | E56 | 3fe09766819b |
| I0385 | `src-tauri/tests/playback_transport_integration.rs` | `@@ -10,0 +4 @@` | `af8f91eff429a4bb` | E56 | 3fe09766819b |
| I0386 | `src-tauri/tests/playback_transport_integration.rs` | `@@ -12,0 +7 @@ use std::net::TcpStream;` | `96f18380045606f8` | E56 | 3fe09766819b |
| I0387 | `src-tauri/tests/playback_transport_integration.rs` | `@@ -15 +10,11 @@ use std::time::Duration;` | `c37d69853c110d50` | E56 | 3fe09766819b; contrib 7a0a05cb1dd2,e2daeb279a33 |
| I0388 | `src-tauri/tests/playback_transport_integration.rs` | `@@ -17 +21,0 @@ use opentake_tauri_lib::playback::PreviewServer;` | `b89a28b87bccae5e` | E56 | 3fe09766819b |
| I0389 | `src-tauri/tests/playback_transport_integration.rs` | `@@ -23 +27 @@ fn port_of(endpoint: &str) -> u16 {` | `1f176eb15cd477dd` | E56 | 3fe09766819b |
| I0390 | `src-tauri/tests/playback_transport_integration.rs` | `@@ -27,4 +31,13 @@ fn port_of(endpoint: &str) -> u16 {` | `7bdd1f7286fccc2e` | E56 | 3fe09766819b |
| I0391 | `src-tauri/tests/playback_transport_integration.rs` | `@@ -34,9 +47,92 @@ fn read_head(stream: &mut TcpStream) -> String {` | `4de672bb28e68bc9` | E56 | 3fe09766819b |
| I0392 | `src-tauri/tests/playback_transport_integration.rs` | `@@ -45 +141,4 @@ fn read_head(stream: &mut TcpStream) -> String {` | `6c1435beaf0e8eb9` | E56 | 3fe09766819b |
| I0393 | `src-tauri/tests/playback_transport_integration.rs` | `@@ -48 +146,0 @@ fn read_head(stream: &mut TcpStream) -> String {` | `81717e6bfadf69db` | E56 | 3fe09766819b |
| I0394 | `src-tauri/tests/playback_transport_integration.rs` | `@@ -51,6 +149,32 @@ fn read_head(stream: &mut TcpStream) -> String {` | `5e4f4435737bfbc9` | E56 | 3fe09766819b |
| I0395 | `src-tauri/tests/playback_transport_integration.rs` | `@@ -61,4 +185,19 @@ fn start_server() -> Option<std::sync::Arc<PreviewServer>> {` | `cc9767037b4d96ca` | E56 | 3fe09766819b; contrib 6551e6744dbd |
| I0396 | `src-tauri/tests/playback_transport_integration.rs` | `@@ -66,2 +205,2 @@ fn get(port: u16, extra_headers: &str) -> String {` | `56a7a0b60d2d7776` | E56 | 3fe09766819b |
| I0397 | `src-tauri/tests/playback_transport_integration.rs` | `@@ -71,9 +210,13 @@ fn get(port: u16, extra_headers: &str) -> String {` | `91dc342a723e6b53` | E56 | 3fe09766819b; contrib 6551e6744dbd |
| I0398 | `src-tauri/tests/playback_transport_integration.rs` | `@@ -81,3 +224,12 @@ fn stream_route_serves_multipart_mjpeg() {` | `3b5795c162c9d2a2` | E56 | 3fe09766819b |
| I0399 | `src-tauri/tests/playback_transport_integration.rs` | `@@ -84,0 +237 @@ fn stream_route_serves_multipart_mjpeg() {` | `101d3f69dc4da95b` | E56 | 3fe09766819b |
| I0400 | `src-tauri/tests/playback_transport_integration.rs` | `@@ -88,3 +241,34 @@ fn stream_route_serves_multipart_mjpeg() {` | `361c84937a946c8a` | E56 | 3fe09766819b; contrib 7a0a05cb1dd2 |
| I0401 | `src-tauri/tests/playback_transport_integration.rs` | `@@ -92 +276,12 @@ fn stream_route_rejects_cross_origin() {` | `2e1397a9b3c91b77` | E56 | 3fe09766819b |
| I0402 | `src-tauri/tests/playback_transport_integration.rs` | `@@ -94,5 +289,2 @@ fn stream_route_rejects_cross_origin() {` | `93d43553109a114d` | E56 | 3fe09766819b |
| I0403 | `src-tauri/tests/playback_transport_integration.rs` | `@@ -99,0 +292 @@ fn stream_route_rejects_cross_origin() {` | `101d3f69dc4da95b` | E56 | 3fe09766819b |
| I0404 | `web/src/components/home/HomeView.visual.test.ts` | `@@ -4,0 +5,5 @@ const homeSource = readFileSync(new URL("./HomeView.tsx", import.meta.url), "utf` | `7ebedf0259d12cf2` | E63 | bf4e3a7dc670 |
| I0405 | `web/src/components/home/HomeView.visual.test.ts` | `@@ -70,0 +76,8 @@ describe("HomeView Vercel embedded visual direction", () => {` | `347dd19983f288f9` | E63 | bf4e3a7dc670 |
| I0406 | `web/src/components/preview/Preview.test.tsx` | `@@ -4 +4 @@ import { beforeEach, describe, expect, it, vi } from "vitest";` | `ef6f4b39db29e743` | E52 | 6551e6744dbd |
| I0407 | `web/src/components/preview/Preview.test.tsx` | `@@ -6,0 +7,2 @@ const store = vi.hoisted(() => ({` | `fd8fc632c6ee053c` | E62 | 716a09c78543 |
| I0408 | `web/src/components/preview/Preview.test.tsx` | `@@ -25,0 +28 @@ const store = vi.hoisted(() => ({` | `79360922388710cf` | E62 | 716a09c78543 |
| I0409 | `web/src/components/preview/Preview.test.tsx` | `@@ -36,0 +40 @@ const store = vi.hoisted(() => ({` | `51482eebe716ef26` | E52 | 6551e6744dbd |
| I0410 | `web/src/components/preview/Preview.test.tsx` | `@@ -61,0 +66,5 @@ vi.mock("../../lib/asset", () => ({` | `dce222c396b8b718` | E62 | 716a09c78543; contrib 6551e6744dbd |
| I0411 | `web/src/components/preview/Preview.test.tsx` | `@@ -125,0 +135 @@ describe("Preview timeline rendering", () => {` | `e1a32f69493acb72` | E52 | 6551e6744dbd |
| I0412 | `web/src/components/preview/Preview.test.tsx` | `@@ -140 +150 @@ describe("Preview timeline rendering", () => {` | `2f8e006b35d01612` | E62 | 716a09c78543 |
| I0413 | `web/src/components/preview/Preview.test.tsx` | `@@ -143,0 +154,72 @@ describe("Preview timeline rendering", () => {` | `83c9c41efa123673` | E62 | 716a09c78543; contrib 2836a5386328,bffbcf64d991,6551e6744dbd |
| I0414 | `web/src/components/preview/Preview.tsx` | `@@ -6 +6 @@` | `0932da529b5f07ff` | E62 | 716a09c78543 |
| I0415 | `web/src/components/preview/Preview.tsx` | `@@ -42,3 +43,0 @@ import {` | `6cb6a3fc23572bda` | E04 | 2836a5386328 |
| I0416 | `web/src/components/preview/Preview.tsx` | `@@ -61,0 +62,4 @@ import type { MediaItem } from "../../lib/types";` | `c90f54cca6cf246a` | E62 | 716a09c78543; contrib 6551e6744dbd |
| I0417 | `web/src/components/preview/Preview.tsx` | `@@ -65,0 +70,2 @@ export function Preview() {` | `731ec5c318cbdc62` | E62 | 716a09c78543 |
| I0418 | `web/src/components/preview/Preview.tsx` | `@@ -69,2 +74,0 @@ export function Preview() {` | `8fbdff9978397fcc` | E04 | 2836a5386328 |
| I0419 | `web/src/components/preview/Preview.tsx` | `@@ -130,0 +136,2 @@ export function Preview() {` | `08743730b47dd925` | E52 | 6551e6744dbd |
| I0420 | `web/src/components/preview/Preview.tsx` | `@@ -140,0 +148,9 @@ export function Preview() {` | `85d19bd6c1bd498b` | E52 | 6551e6744dbd |
| I0421 | `web/src/components/preview/Preview.tsx` | `@@ -185,0 +202,13 @@ export function Preview() {` | `a22d6be3b16b1e1b` | E62 | 716a09c78543; contrib 6551e6744dbd |
| I0422 | `web/src/components/preview/Preview.tsx` | `@@ -207,0 +237 @@ export function Preview() {` | `9247a8ff6c1335c2` | E62 | 716a09c78543 |
| I0423 | `web/src/components/preview/Preview.tsx` | `@@ -218 +248,2 @@ export function Preview() {` | `cc53fa93fb375af9` | E62 | 716a09c78543 |
| I0424 | `web/src/components/preview/Preview.tsx` | `@@ -266,42 +296,0 @@ export function Preview() {` | `a0a6fdf0b01a2595` | E04 | 2836a5386328 |
| I0425 | `web/src/components/preview/Preview.tsx` | `@@ -400 +388,3 @@ export function Preview() {` | `233664502f76e414` | E62 | 716a09c78543 |
| I0426 | `web/src/components/preview/Preview.tsx` | `@@ -402,7 +392 @@ export function Preview() {` | `b6914558654599bb` | E62 | 716a09c78543 |
| I0427 | `web/src/components/preview/Preview.tsx` | `@@ -410 +394,10 @@ export function Preview() {` | `3388aaff3460b597` | E62 | 716a09c78543; contrib 6551e6744dbd |
| I0428 | `web/src/components/preview/Preview.tsx` | `@@ -505 +498,5 @@ export function Preview() {` | `28d09c29dd816f67` | E62 | 716a09c78543 |
| I0429 | `web/src/components/preview/Preview.tsx` | `@@ -528,0 +526,31 @@ export function Preview() {` | `bf85a079bb51bdad` | E62 | 716a09c78543 |
| I0430 | `web/src/components/preview/RustFrameBuffer.tsx` | `@@ -0,0 +1,247 @@` | `f5dbdb6e13cd7670` | E62 | dc83284319bd; contrib 716a09c78543,00e76f015335 |
| I0431 | `web/src/components/preview/TimelinePlaybackLayer.tsx` | `@@ -75 +75 @@ export function TimelinePlayback({ timeline, fps }: { timeline: Timeline; fps: n` | `51576539d0a4b9c5` | E62 | 716a09c78543 |
| I0432 | `web/src/components/preview/nativePlaybackSession.test.ts` | `@@ -0,0 +1,129 @@` | `998d0e273d21585f` | E52 | 6551e6744dbd |
| I0433 | `web/src/components/preview/nativePlaybackSession.ts` | `@@ -0,0 +1,222 @@` | `3867bf047e07b03c` | E52 | 6551e6744dbd |
| I0434 | `web/src/components/preview/playbackRoute.test.ts` | `@@ -0,0 +1,236 @@` | `fcef16332062df8e` | E61 | 8b47e64a8e6c; contrib a2f747f04f8e |
| I0435 | `web/src/components/preview/playbackRoute.ts` | `@@ -0,0 +1,102 @@` | `91d02ad8656e2fe3` | E61 | 8b47e64a8e6c; contrib a2f747f04f8e |
| I0436 | `web/src/components/preview/previewEngine.test.ts` | `@@ -1 +1,71 @@` | `8e1dd96dcf0f7e6c` | E52 | e2daeb279a33; contrib bffbcf64d991,7a0a05cb1dd2 |
| I0437 | `web/src/components/preview/previewEngine.test.ts` | `@@ -5,0 +76,75 @@ import type { Clip, ClipType, Timeline, Track } from "../../lib/types";` | `3b00278edea0b330` | E52 | e2daeb279a33; contrib 2836a5386328,bffbcf64d991,7a0a05cb1dd2 |
| I0438 | `web/src/components/preview/previewEngine.test.ts` | `@@ -52,0 +198,10 @@ function timeline(tracks: Track[]): Timeline {` | `acacfeffc0cf8fce` | E62 | 716a09c78543 |
| I0439 | `web/src/components/preview/previewEngine.test.ts` | `@@ -53,0 +209,145 @@ describe("shouldSyncPausedMediaToFrame", () => {` | `3596a74e649b4851` | E62 | 716a09c78543; contrib 6551e6744dbd,7a0a05cb1dd2,e2daeb279a33 |
| I0440 | `web/src/components/preview/previewEngine.test.ts` | `@@ -175,0 +476,87 @@ describe("shouldSeekPlayingFollower", () => {` | `830ef09c54b12c81` | E52 | 7a0a05cb1dd2; contrib 2836a5386328,bffbcf64d991 |
| I0441 | `web/src/components/preview/previewEngine.ts` | `@@ -29,2 +28,0 @@ import {` | `a7e863a5679315a3` | E04 | 2836a5386328 |
| I0442 | `web/src/components/preview/previewEngine.ts` | `@@ -42,2 +40 @@ import type { Timeline } from "../../lib/types";` | `acbed445f559044f` | E52 | 6551e6744dbd |
| I0443 | `web/src/components/preview/previewEngine.ts` | `@@ -45,59 +42,61 @@ import {` | `f19ff54e4cbf6b57` | E62 | 716a09c78543; contrib 6551e6744dbd,7a0a05cb1dd2,e2daeb279a33 |
| I0444 | `web/src/components/preview/previewEngine.ts` | `@@ -105 +104,52 @@ function ensureMpv(): Promise<void> {` | `872a8d7ee21549f7` | E52 | e2daeb279a33; contrib 7a0a05cb1dd2 |
| I0445 | `web/src/components/preview/previewEngine.ts` | `@@ -108,3 +158,6 @@ function ensureMpv(): Promise<void> {` | `1fc2c8264ecb2a3d` | E52 | 6551e6744dbd |
| I0446 | `web/src/components/preview/previewEngine.ts` | `@@ -209,0 +256,19 @@ export function shouldSeekPlayingFollower(args: {` | `08000e3bb7601325` | E04 | bffbcf64d991; contrib 2836a5386328 |
| I0447 | `web/src/components/preview/previewEngine.ts` | `@@ -211 +276 @@ function pauseAll(): void {` | `eafe205f5e5af84c` | E04 | 2836a5386328 |
| I0448 | `web/src/components/preview/previewEngine.ts` | `@@ -297,0 +363 @@ export function useTimelinePlaybackEngine(): void {` | `2b186b249257ec31` | E52 | 6551e6744dbd |
| I0449 | `web/src/components/preview/previewEngine.ts` | `@@ -299,3 +364,0 @@ export function useTimelinePlaybackEngine(): void {` | `128cb0fb394982ec` | E04 | 2836a5386328 |
| I0450 | `web/src/components/preview/previewEngine.ts` | `@@ -303,6 +366,4 @@ export function useTimelinePlaybackEngine(): void {` | `6b08fe45c789f058` | E52 | 7a0a05cb1dd2; contrib 6551e6744dbd |
| I0451 | `web/src/components/preview/previewEngine.ts` | `@@ -311,0 +373,11 @@ export function useTimelinePlaybackEngine(): void {` | `ea127fbbf302d8a5` | E52 | 7a0a05cb1dd2; contrib 6551e6744dbd |
| I0452 | `web/src/components/preview/previewEngine.ts` | `@@ -319,14 +391,9 @@ export function useTimelinePlaybackEngine(): void {` | `9e5f9bdeea52bb38` | E52 | 6551e6744dbd |
| I0453 | `web/src/components/preview/previewEngine.ts` | `@@ -354 +421 @@ export function useTimelinePlaybackEngine(): void {` | `6ae7cfe050063054` | E52 | 6551e6744dbd |
| I0454 | `web/src/components/preview/previewEngine.ts` | `@@ -357,10 +424,12 @@ export function useTimelinePlaybackEngine(): void {` | `b84848d8413a3949` | E62 | 716a09c78543 |
| I0455 | `web/src/components/preview/previewEngine.ts` | `@@ -368,2 +436,0 @@ export function useTimelinePlaybackEngine(): void {` | `94b7e273b9a23840` | E04 | 2836a5386328 |
| I0456 | `web/src/components/preview/previewEngine.ts` | `@@ -371,14 +438,8 @@ export function useTimelinePlaybackEngine(): void {` | `cc63d7f2b5e5f129` | E52 | 6551e6744dbd |
| I0457 | `web/src/components/preview/previewEngine.ts` | `@@ -389,12 +448,20 @@ export function useTimelinePlaybackEngine(): void {` | `19ac186f17385d8f` | E52 | e2daeb279a33; contrib 6551e6744dbd,7a0a05cb1dd2 |
| I0458 | `web/src/components/preview/previewEngine.ts` | `@@ -402,33 +469,10 @@ export function useTimelinePlaybackEngine(): void {` | `c78e0adc7080e220` | E52 | 6551e6744dbd |
| I0459 | `web/src/components/preview/previewEngine.ts` | `@@ -436,2 +480 @@ export function useTimelinePlaybackEngine(): void {` | `eb31105179717b83` | E52 | 6551e6744dbd |
| I0460 | `web/src/components/preview/previewEngine.ts` | `@@ -441,15 +484,15 @@ export function useTimelinePlaybackEngine(): void {` | `113ff48ba750b115` | E52 | 6551e6744dbd |
| I0461 | `web/src/components/preview/previewEngine.ts` | `@@ -458,0 +502,6 @@ export function useTimelinePlaybackEngine(): void {` | `48740d680e258b0a` | E62 | 716a09c78543 |
| I0462 | `web/src/components/preview/previewEngine.ts` | `@@ -493,2 +542 @@ export function useTimelinePlaybackEngine(): void {` | `b4eeaf0d295f43f4` | E04 | 2836a5386328 |
| I0463 | `web/src/components/preview/previewEngine.ts` | `@@ -585,10 +633,2 @@ export function useTimelinePlaybackEngine(): void {` | `efed8faf29e50fb0` | E52 | 6551e6744dbd |
| I0464 | `web/src/components/preview/previewEngine.ts` | `@@ -596,3 +636,5 @@ export function useTimelinePlaybackEngine(): void {` | `e5ac61088de423dc` | E62 | 716a09c78543 |
| I0465 | `web/src/components/preview/previewEngine.ts` | `@@ -599,0 +642,2 @@ export function useTimelinePlaybackEngine(): void {` | `18f8dbf2a100d59f` | E52 | 6551e6744dbd |
| I0466 | `web/src/components/preview/previewEngine.ts` | `@@ -600,0 +645 @@ export function useTimelinePlaybackEngine(): void {` | `0fd25f1ed3b66a45` | E52 | 6551e6744dbd |
| I0467 | `web/src/components/preview/previewEngine.ts` | `@@ -606,9 +651,2 @@ export function useTimelinePlaybackEngine(): void {` | `6c9702f0b365816c` | E52 | 6551e6744dbd |
| I0468 | `web/src/components/preview/previewEngine.ts` | `@@ -616 +654 @@ export function useTimelinePlaybackEngine(): void {` | `71298f60132e19e3` | E52 | 6551e6744dbd |
| I0469 | `web/src/components/preview/rustEngine.test.ts` | `@@ -33,0 +34,7 @@ describe("rustEngineEnabled (default-on)", () => {` | `0272440c4401fac5` | E61 | a2f747f04f8e |
| I0470 | `web/src/components/preview/rustEngine.test.ts` | `@@ -46,0 +55 @@ describe("rustEngineEnabled (default-on)", () => {` | `85fc8f3721900ca3` | E61 | a2f747f04f8e |
| I0471 | `web/src/components/preview/rustEngine.test.ts` | `@@ -51,0 +61 @@ describe("rustEngineEnabled (default-on)", () => {` | `b17e699d38e56542` | E61 | a2f747f04f8e |
| I0472 | `web/src/components/preview/rustEngine.test.ts` | `@@ -60,0 +71 @@ describe("rustEngineEnabled (default-on)", () => {` | `b17e699d38e56542` | E61 | a2f747f04f8e |
| I0473 | `web/src/components/preview/rustEngine.test.ts` | `@@ -69,0 +81 @@ describe("rustEngineEnabled (default-on)", () => {` | `b17e699d38e56542` | E61 | a2f747f04f8e |
| I0474 | `web/src/components/preview/rustEngine.ts` | `@@ -4,2 +4,3 @@` | `2062624e7ae83289` | E61 | a2f747f04f8e |
| I0475 | `web/src/components/preview/rustEngine.ts` | `@@ -12,3 +13,2 @@` | `cd7a323a35489c0c` | E61 | a2f747f04f8e |
| I0476 | `web/src/components/preview/rustEngine.ts` | `@@ -22,4 +22,3 @@` | `e1ac35a971a9ca63` | E61 | a2f747f04f8e |
| I0477 | `web/src/components/preview/rustEngine.ts` | `@@ -29 +28 @@ const FLAG_KEY = "opentake.rustEngine";` | `21ba34370352c2f5` | E61 | a2f747f04f8e |
| I0478 | `web/src/components/preview/rustEngine.ts` | `@@ -31,4 +30,5 @@ export function rustEngineEnabled(): boolean {` | `3871d2c6a3d261a1` | E61 | a2f747f04f8e |
| I0479 | `web/src/components/preview/rustEngine.ts` | `@@ -36,3 +36 @@ export function rustEngineEnabled(): boolean {` | `5ce9715a27d80516` | E61 | a2f747f04f8e |
| I0480 | `web/src/components/preview/rustFrameBuffer.test.ts` | `@@ -0,0 +1,272 @@` | `802c55ec0cf74c57` | E62 | dc83284319bd; contrib 716a09c78543,00e76f015335 |
| I0481 | `web/src/components/preview/rustFrameBuffer.ts` | `@@ -0,0 +1,206 @@` | `3458b919dc9d54de` | E62 | 716a09c78543 |
| I0482 | `web/src/components/preview/timelinePlayback.test.ts` | `@@ -16,2 +15,0 @@ import {` | `af2488c574f77a8b` | E62 | 716a09c78543 |
| I0483 | `web/src/components/preview/timelinePlayback.test.ts` | `@@ -53,59 +50,0 @@ describe("isExternalSeekWhilePlaying", () => {` | `5f820c59513a2f7c` | E62 | 716a09c78543 |
| I0484 | `web/src/components/preview/timelinePlayback.ts` | `@@ -213,46 +212,0 @@ export function isExternalSeekWhilePlaying(args: {` | `1d7d622441e4afb9` | E62 | 716a09c78543 |
| I0485 | `web/src/components/shell/TitleBar.visual.test.ts` | `@@ -2,0 +3 @@ import { describe, expect, it } from "vitest";` | `b398cb0f730fa75e` | E63 | 9c3d304327bc |
| I0486 | `web/src/components/shell/TitleBar.visual.test.ts` | `@@ -21,0 +23,11 @@ describe("TitleBar alignment", () => {` | `f230b4207b336cf5` | E63 | 9c3d304327bc; contrib bf4e3a7dc670 |
| I0487 | `web/src/components/timeline/TimelineContainer.test.ts` | `@@ -1,0 +2 @@ import { describe, expect, it, vi } from "vitest";` | `4f1305a1907a9a23` | E63 | bf4e3a7dc670 |
| I0488 | `web/src/components/timeline/TimelineContainer.test.ts` | `@@ -5,0 +8,2 @@ import type { Clip, ClipType, Timeline, Track } from "../../lib/types";` | `72e156724b2d1fe6` | E63 | bf4e3a7dc670 |
| I0489 | `web/src/components/timeline/TimelineContainer.test.ts` | `@@ -206,0 +211,146 @@ describe("volumeKeyframeMenuItems", () => {` | `7ec902c83822584f` | E63 | 1f2bf4e49877; contrib bf4e3a7dc670,cf95430778ba,08443832106a,7c6dd6156d14 |
| I0490 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -50 +51,7 @@ import { forceRefresh } from "../../store/sync";` | `5a5e1691f16c1fec` | E63 | bf4e3a7dc670 |
| I0491 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -170,0 +178,216 @@ export function collectMoveSnapTargets(` | `45327f9dab95245d` | E63 | 1f2bf4e49877; contrib bf4e3a7dc670,cf95430778ba,08443832106a,7c6dd6156d14 |
| I0492 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -309,0 +533 @@ export function TimelineContainer() {` | `2dea4c4f539190fb` | E63 | 7c6dd6156d14 |
| I0493 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -346,0 +571,10 @@ export function TimelineContainer() {` | `1718b8a49b24b8d2` | E63 | 1f2bf4e49877 |
| I0494 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -372,2 +606,3 @@ export function TimelineContainer() {` | `b837e9c2dc0ec30b` | E63 | 1f2bf4e49877 |
| I0495 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -379,0 +615,5 @@ export function TimelineContainer() {` | `2fe256f5d22acfe4` | E63 | 7c6dd6156d14; contrib bf4e3a7dc670 |
| I0496 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -386 +629,45 @@ export function TimelineContainer() {` | `164044def92d61f0` | E63 | 1f2bf4e49877; contrib bf4e3a7dc670,7c6dd6156d14 |
| I0497 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -541,2 +832,2 @@ export function TimelineContainer() {` | `d57b2a5534131af7` | E63 | 1f2bf4e49877 |
| I0498 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -564,0 +856,2 @@ export function TimelineContainer() {` | `3289f12afb650b18` | E63 | 1f2bf4e49877 |
| I0499 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -570,3 +863,3 @@ export function TimelineContainer() {` | `7d32a16fa0f541fb` | E63 | bf4e3a7dc670 |
| I0500 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -577 +870 @@ export function TimelineContainer() {` | `513c323d921549af` | E63 | bf4e3a7dc670 |
| I0501 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -580,0 +874,50 @@ export function TimelineContainer() {` | `03c0718f01be0b50` | E63 | 1f2bf4e49877; contrib bf4e3a7dc670,7c6dd6156d14 |
| I0502 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -583,2 +926,10 @@ export function TimelineContainer() {` | `9c39934d71b723fc` | E63 | 1f2bf4e49877 |
| I0503 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -587,5 +938,7 @@ export function TimelineContainer() {` | `24d17ecc0176f264` | E63 | 1f2bf4e49877 |
| I0504 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -592,0 +946 @@ export function TimelineContainer() {` | `e5d478b1c90293cb` | E63 | 1f2bf4e49877 |
| I0505 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -598 +952 @@ export function TimelineContainer() {` | `74d70299139e75bb` | E63 | 1f2bf4e49877 |
| I0506 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -601 +955 @@ export function TimelineContainer() {` | `167a18211f4f60b9` | E63 | 1f2bf4e49877 |
| I0507 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -604,2 +958,11 @@ export function TimelineContainer() {` | `3ac49fba23e08816` | E63 | 1f2bf4e49877 |
| I0508 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -607,2 +970,2 @@ export function TimelineContainer() {` | `4aad88bea2c94a04` | E63 | 1f2bf4e49877 |
| I0509 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -610,2 +973,2 @@ export function TimelineContainer() {` | `3303b1db6ea00fef` | E63 | 1f2bf4e49877 |
| I0510 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -614,2 +977,3 @@ export function TimelineContainer() {` | `88352c3189d163bd` | E63 | 1f2bf4e49877 |
| I0511 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -631 +995 @@ export function TimelineContainer() {` | `2fc31c61585c26da` | E63 | 1f2bf4e49877 |
| I0512 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -647,2 +1011 @@ export function TimelineContainer() {` | `de921811b51ba088` | E63 | 1f2bf4e49877 |
| I0513 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -658 +1021 @@ export function TimelineContainer() {` | `829369516f194fa6` | E63 | 1f2bf4e49877 |
| I0514 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -663,3 +1026,3 @@ export function TimelineContainer() {` | `7a292c86db775dfb` | E63 | 1f2bf4e49877 |
| I0515 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -667 +1030 @@ export function TimelineContainer() {` | `8e800c41bcffc02f` | E63 | 1f2bf4e49877 |
| I0516 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -672 +1035 @@ export function TimelineContainer() {` | `ca90820b0cd4653d` | E63 | 1f2bf4e49877 |
| I0517 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -677,3 +1040,6 @@ export function TimelineContainer() {` | `7070aa856600dac4` | E63 | 1f2bf4e49877 |
| I0518 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -681,2 +1047,5 @@ export function TimelineContainer() {` | `08b9322549c12374` | E63 | 1f2bf4e49877 |
| I0519 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -685,2 +1054,2 @@ export function TimelineContainer() {` | `0d71927329102b90` | E63 | 1f2bf4e49877 |
| I0520 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -692 +1061 @@ export function TimelineContainer() {` | `7c2f327ebd7ca5fe` | E63 | 1f2bf4e49877 |
| I0521 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -695 +1064 @@ export function TimelineContainer() {` | `2801c633dead9bfc` | E63 | 1f2bf4e49877 |
| I0522 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -698 +1067 @@ export function TimelineContainer() {` | `2b9fe51455059328` | E63 | 1f2bf4e49877 |
| I0523 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -819 +1188,9 @@ export function TimelineContainer() {` | `8cc1ba06a651383a` | E63 | 08443832106a |
| I0524 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -873,19 +1250 @@ export function TimelineContainer() {` | `58ee914534db1924` | E63 | cf95430778ba |
| I0525 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -954 +1313,15 @@ export function TimelineContainer() {` | `e897b38f57551523` | E63 | 08443832106a |
| I0526 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -1336 +1709,9 @@ export function TimelineContainer() {` | `8cc1ba06a651383a` | E63 | 08443832106a |
| I0527 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -1344,2 +1725,2 @@ export function TimelineContainer() {` | `16b8709ce5533eec` | E63 | cf95430778ba |
| I0528 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -1347 +1728 @@ export function TimelineContainer() {` | `478d0ce4a33eb8e5` | E63 | cf95430778ba |
| I0529 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -1351 +1732,10 @@ export function TimelineContainer() {` | `c223e1319379f281` | E63 | 08443832106a |
| I0530 | `web/src/components/timeline/TimelineContainer.tsx` | `@@ -1586,0 +1977,55 @@ export function TimelineContainer() {` | `aa171a09bbf2f875` | E63 | 08443832106a; contrib bf4e3a7dc670,cf95430778ba |
| I0531 | `web/src/components/ui/PanelShell.test.tsx` | `@@ -0,0 +1,65 @@` | `260e9bacba72e4e7` | E04 | bffbcf64d991 |
| I0532 | `web/src/hooks/useKeyboardShortcuts.test.ts` | `@@ -40,0 +41 @@ describe("keyboard transport Space shortcut", () => {` | `aa5a70ef5fb9de53` | E62 | 716a09c78543 |
| I0533 | `web/src/hooks/useKeyboardShortcuts.test.ts` | `@@ -61,0 +63,22 @@ describe("keyboard transport Space shortcut", () => {` | `8bf4281c8893a437` | E62 | 716a09c78543 |
| I0534 | `web/src/hooks/useKeyboardShortcuts.ts` | `@@ -16,0 +17,3 @@ import type { AppView } from "../store/uiStore";` | `177ad3a2740eb375` | E62 | 716a09c78543 |
| I0535 | `web/src/hooks/useKeyboardShortcuts.ts` | `@@ -41,0 +45 @@ interface TransportSpaceUi {` | `bfd8f1eddb94bd06` | E62 | 716a09c78543 |
| I0536 | `web/src/hooks/useKeyboardShortcuts.ts` | `@@ -56 +60 @@ export function handleTransportSpaceKeyDown(` | `8007d8db4f2c23c9` | E62 | 716a09c78543 |
| I0537 | `web/src/hooks/useKeyboardShortcuts.ts` | `@@ -66 +70,9 @@ export function useKeyboardShortcuts() {` | `fcb8f66420d00b7e` | E62 | 716a09c78543 |
| I0538 | `web/src/i18n/dict.ts` | `@@ -422,0 +424,10 @@ const zh: Dict = {` | `4c57b231dec5e406` | E62 | 716a09c78543 |
| I0539 | `web/src/i18n/dict.ts` | `@@ -1007,0 +1020,10 @@ const en: Dict = {` | `16cf7b86fde67d2e` | E62 | 716a09c78543 |
| I0540 | `web/src/lib/api.test.ts` | `@@ -0,0 +1,67 @@` | `fa103c375fcccada` | E63 | bf4e3a7dc670; contrib 6551e6744dbd |
| I0541 | `web/src/lib/api.ts` | `@@ -20,0 +21,4 @@ import type {` | `a64b5c484a8bd173` | E52 | 6551e6744dbd |
| I0542 | `web/src/lib/api.ts` | `@@ -25 +28,0 @@ import type {` | `5ccd25cdf40040f8` | E52 | 6551e6744dbd |
| I0543 | `web/src/lib/api.ts` | `@@ -53 +56 @@ async function ensureTauri(): Promise<void> {` | `87521f2b99e6b609` | E52 | 6551e6744dbd |
| I0544 | `web/src/lib/api.ts` | `@@ -55,2 +58,2 @@ export async function getTimeline(): Promise<TimelineSnapshot> {` | `0d6734026990e951` | E52 | 6551e6744dbd |
| I0545 | `web/src/lib/api.ts` | `@@ -99 +102 @@ export async function canRedo(): Promise<boolean> {` | `585eb94535c0aa02` | E52 | 6551e6744dbd |
| I0546 | `web/src/lib/api.ts` | `@@ -102,2 +105 @@ export async function projectNew(): Promise<void> {` | `a3fe29d5e1e06f8e` | E52 | 6551e6744dbd |
| I0547 | `web/src/lib/api.ts` | `@@ -105,0 +108 @@ export async function projectNew(): Promise<void> {` | `04b6ff959bee7d9f` | E52 | 6551e6744dbd |
| I0548 | `web/src/lib/api.ts` | `@@ -108 +111 @@ export async function projectNew(): Promise<void> {` | `47acc362ea130671` | E52 | 6551e6744dbd |
| I0549 | `web/src/lib/api.ts` | `@@ -110,2 +113,2 @@ export async function projectOpen(path: string): Promise<TimelineSnapshot> {` | `362ec127ab2f04be` | E52 | 6551e6744dbd |
| I0550 | `web/src/lib/api.ts` | `@@ -610,8 +613,21 @@ export async function previewPoster(` | `241d0b10338ce8ac` | E63 | bf4e3a7dc670 |
| I0551 | `web/src/lib/api.ts` | `@@ -619 +635 @@ export async function preloadMedia(mediaRef: string): Promise<void> {` | `6f21885aef08ef18` | E63 | bf4e3a7dc670 |
| I0552 | `web/src/lib/api.ts` | `@@ -621 +637 @@ export async function preloadMedia(mediaRef: string): Promise<void> {` | `0d40be0084fa23f1` | E63 | bf4e3a7dc670 |
| I0553 | `web/src/lib/api.ts` | `@@ -623,0 +640 @@ export async function preloadMedia(mediaRef: string): Promise<void> {` | `8ecc2586d9fa7608` | E63 | bf4e3a7dc670 |
| I0554 | `web/src/lib/api.ts` | `@@ -786 +803 @@ export async function onTimelineChanged(` | `4e0f7be2d0ffba8e` | E52 | 6551e6744dbd |
| I0555 | `web/src/lib/api.ts` | `@@ -791,2 +808,8 @@ export async function onTimelineChanged(` | `b53bdd3c33249886` | E52 | 6551e6744dbd |
| I0556 | `web/src/lib/api.ts` | `@@ -797 +820 @@ export async function onProjectOpened(` | `7df9a8759ec8e2d7` | E52 | 6551e6744dbd |
| I0557 | `web/src/lib/api.ts` | `@@ -802,2 +825,11 @@ export async function onProjectOpened(` | `c0ad6bbf739c31c9` | E52 | 6551e6744dbd |
| I0558 | `web/src/lib/api.ts` | `@@ -876 +908,4 @@ export async function onChatDone(` | `fe285a736e4582f6` | E52 | 6551e6744dbd |
| I0559 | `web/src/lib/api.ts` | `@@ -879 +914,4 @@ export async function playbackStart(fromFrame: number): Promise<void> {` | `5f37a99fb929b9a6` | E52 | 6551e6744dbd |
| I0560 | `web/src/lib/api.ts` | `@@ -882,3 +920,2 @@ export async function playbackStart(fromFrame: number): Promise<void> {` | `de567b355212143b` | E52 | 6551e6744dbd |
| I0561 | `web/src/lib/api.ts` | `@@ -886 +923,2 @@ export async function playbackPause(): Promise<void> {` | `6a958c08d8b26b1f` | E52 | 6551e6744dbd |
| I0562 | `web/src/lib/api.ts` | `@@ -890 +928 @@ export async function playbackPause(): Promise<void> {` | `e1813dbf79b9a968` | E52 | 6551e6744dbd |
| I0563 | `web/src/lib/api.ts` | `@@ -892 +930 @@ export async function playbackStop(): Promise<void> {` | `8dc7986d12bdb56d` | E52 | 6551e6744dbd |
| I0564 | `web/src/lib/api.ts` | `@@ -896 +934 @@ export async function playbackStop(): Promise<void> {` | `21f6ed57bc2e774f` | E52 | 6551e6744dbd |
| I0565 | `web/src/lib/api.ts` | `@@ -898 +936,2 @@ export async function playbackSeek(frame: number): Promise<void> {` | `f16fce6a5f1a857a` | E52 | 6551e6744dbd |
| I0566 | `web/src/lib/api.ts` | `@@ -901 +940 @@ export async function playbackSeek(frame: number): Promise<void> {` | `d93a79627da8ff4c` | E52 | 6551e6744dbd |
| I0567 | `web/src/lib/api.ts` | `@@ -911 +950 @@ export async function onPlaybackFrame(` | `7b0d1c0b60589861` | E52 | 6551e6744dbd |
| I0568 | `web/src/lib/api.ts` | `@@ -916,2 +955,2 @@ export async function onPlaybackFrame(` | `27cf58014f1f4bdd` | E52 | 6551e6744dbd |
| I0569 | `web/src/lib/api.ts` | `@@ -920,0 +960,33 @@ export async function onPlaybackFrame(` | `d04412d1299a6adc` | E52 | 6551e6744dbd |
| I0570 | `web/src/lib/types.ts` | `@@ -405,0 +406 @@ export interface TimelineSnapshot {` | `b88c1ae497f9a2e4` | E52 | 6551e6744dbd |
| I0571 | `web/src/lib/types.ts` | `@@ -408,0 +410,28 @@ export interface TimelineSnapshot {` | `2a67856e5f0446e5` | E52 | 6551e6744dbd |
| I0572 | `web/src/store/editActions.test.ts` | `@@ -63,0 +64,2 @@ const srv = vi.hoisted(() => {` | `5f9f288f868b9b88` | E63 | 7c6dd6156d14 |
| I0573 | `web/src/store/editActions.test.ts` | `@@ -99,0 +102,31 @@ const srv = vi.hoisted(() => {` | `0a8c087bd1c12fc5` | E63 | 7c6dd6156d14 |
| I0574 | `web/src/store/editActions.test.ts` | `@@ -209,0 +243 @@ import {` | `c8ed1ac2fe5c5f49` | E63 | 7c6dd6156d14 |
| I0575 | `web/src/store/editActions.test.ts` | `@@ -300,0 +335,41 @@ describe("addMediaToTimeline", () => {` | `e9d9448ae2f1bc43` | E63 | 7c6dd6156d14; contrib bf4e3a7dc670 |
| I0576 | `web/src/store/editActions.ts` | `@@ -67 +67,9 @@ export async function insertClips(trackIndex: number, atFrame: number, entries:` | `7d4cd3bcf02b0ae6` | E63 | 7c6dd6156d14 |
| I0577 | `web/src/store/projectActions.test.ts` | `@@ -25 +25,15 @@ const srv = vi.hoisted(() => {` | `a4998735393b9eaf` | E52 | 6551e6744dbd |
| I0578 | `web/src/store/projectActions.test.ts` | `@@ -29 +43,7 @@ vi.mock("../lib/api", () => ({` | `793f42aac44fe0bf` | E52 | 6551e6744dbd |
| I0579 | `web/src/store/projectActions.test.ts` | `@@ -33 +53,10 @@ vi.mock("../lib/api", () => ({` | `63d90ed1e707bf18` | E52 | 6551e6744dbd |
| I0580 | `web/src/store/projectActions.test.ts` | `@@ -39,0 +69,4 @@ describe("openProjectPath", () => {` | `05abea1ea485aca7` | E52 | 6551e6744dbd |
| I0581 | `web/src/store/projectActions.test.ts` | `@@ -51,0 +85,39 @@ describe("openProjectPath", () => {` | `693033d43b7cc8c5` | E63 | bf4e3a7dc670; contrib 6551e6744dbd |
| I0582 | `web/src/store/projectActions.ts` | `@@ -16,0 +17 @@ import { t } from "../i18n";` | `f1f7b1b2d0cfd853` | E52 | 6551e6744dbd |
| I0583 | `web/src/store/projectActions.ts` | `@@ -38 +39,5 @@ export async function newProjectAndEnter(): Promise<void> {` | `dc6179896142a823` | E52 | 6551e6744dbd |
| I0584 | `web/src/store/projectActions.ts` | `@@ -39,0 +45 @@ export async function newProjectAndEnter(): Promise<void> {` | `a801d65b82134638` | E63 | bf4e3a7dc670 |
| I0585 | `web/src/store/projectActions.ts` | `@@ -58 +64,5 @@ export async function newProjectAndEnter(): Promise<void> {` | `1a16ded38c546a15` | E52 | 6551e6744dbd |
| I0586 | `web/src/store/projectActions.ts` | `@@ -63,0 +74 @@ export async function newProjectAndEnter(): Promise<void> {` | `01fbe1517716dfe1` | E63 | bf4e3a7dc670 |
| I0587 | `web/src/store/projectActions.ts` | `@@ -87,0 +99 @@ export async function openProjectPath(path: string): Promise<void> {` | `bcf0d613c303139c` | E52 | 6551e6744dbd |
| I0588 | `web/src/store/projectActions.ts` | `@@ -89 +101 @@ export async function openProjectPath(path: string): Promise<void> {` | `0b5a3e2074aa1d3d` | E52 | 6551e6744dbd |
| I0589 | `web/src/store/projectActions.ts` | `@@ -93,0 +106 @@ export async function openProjectPath(path: string): Promise<void> {` | `01fbe1517716dfe1` | E63 | bf4e3a7dc670 |
| I0590 | `web/src/store/projectStore.ts` | `@@ -18,0 +19 @@ interface ProjectState {` | `2969394b3142108f` | E52 | 6551e6744dbd |
| I0591 | `web/src/store/projectStore.ts` | `@@ -28 +29 @@ interface ProjectState {` | `c5f15807f6b5c1df` | E52 | 6551e6744dbd |
| I0592 | `web/src/store/projectStore.ts` | `@@ -36,0 +38 @@ export const useProjectStore = create<ProjectState>((set) => ({` | `7df85a55380888f1` | E52 | 6551e6744dbd |
| I0593 | `web/src/store/projectStore.ts` | `@@ -43 +45,6 @@ export const useProjectStore = create<ProjectState>((set) => ({` | `ffe001a499feae4a` | E52 | 6551e6744dbd |
| I0594 | `web/src/store/sync.test.ts` | `@@ -0,0 +1,77 @@` | `dd399d4a0b6ac6aa` | E63 | bf4e3a7dc670; contrib 6551e6744dbd |
| I0595 | `web/src/store/sync.ts` | `@@ -8,0 +9,2 @@ import { useProjectStore } from "./projectStore";` | `49ed89529d5924aa` | E63 | bf4e3a7dc670; contrib 6551e6744dbd |
| I0596 | `web/src/store/sync.ts` | `@@ -16 +18 @@ async function refreshMirror(): Promise<void> {` | `b061c422769e5796` | E52 | 6551e6744dbd |
| I0597 | `web/src/store/sync.ts` | `@@ -28,2 +30,3 @@ export async function startSync(): Promise<void> {` | `99782cd6c6f85a05` | E52 | 6551e6744dbd |
| I0598 | `web/src/store/sync.ts` | `@@ -33,0 +37 @@ export async function startSync(): Promise<void> {` | `9fd23e4048012339` | E52 | 6551e6744dbd |
| I0599 | `web/src/store/sync.ts` | `@@ -35,0 +40 @@ export async function startSync(): Promise<void> {` | `b4736933d8f0f2dc` | E63 | bf4e3a7dc670 |

## Independent machine validation summary

A separate parser, which does not import the generator, regenerated both parent diffs and parsed this markdown. It verified:

- B IDs are continuous `B0001..B0403`, unique, and total 403; every path/header/fingerprint equals the corresponding source-to-safety hunk.
- Every B disposition is non-empty and belongs to `delivered-exact`, `superseded-by-reviewed-fix`, or `historical-evidence-only`.
- 99 delivered-exact occurrences match and consume same-path fingerprints one-to-one, including duplicate fingerprints.
- I IDs are continuous `I0001..I0599`, unique, and total 599; every row equals the corresponding unmatched source-to-delivery hunk.
- Every B/I evidence code exists in the expanded catalog; every superseded row resolves to concrete replacement tests and an exact reviewer report; every integration row resolves to reviewed evidence.
- The ledger path is absent from both classified parent diffs and has no B/I self-reference.
- The independently regenerated parent counts are 56/403 and 84/698, and the three recorded ledger-excluded patch receipts match the immutable delivery-parent receipts.
