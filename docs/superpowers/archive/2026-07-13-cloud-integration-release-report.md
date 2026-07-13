# OpenTake cloud integration release report

Release window: 2026-07-13 to 2026-07-14 (Asia/Shanghai)

## Outcome

This release converges the seven open pull requests #211 through #217 onto one
reviewed repository tree, while preserving every original PR head in the final
commit ancestry. The source convergence starts from the frozen remote `main`
commit `ac50dc896bea821f66c88c6ed50cf9185e4e31d1` and uses local integration
commit `cf52c5e495f9aea6b685aa20d863c5418a010ca5` as the code-complete candidate.
A Windows aggregate CI finding then required the test-gating-only successor
`301b82d2772559ba6fad25cbb2847ebc07baa494`. The documentation-only successor is the
exact tree supplied to the fail-closed cloud publisher. Replacement aggregate
PR #219 then exercised the complete workflow and exposed two additional native
Windows runtime defects. Code successor
`71ba39ec57866346e492c5973196f8806e221710` fixes both defects without changing
the accepted macOS runtime bundle; its documentation-only successors form the final tree
supplied to the next immutable aggregate publication.

The final cloud gate is intentionally stricter than the legacy PR checks: the
aggregate pull request and the final `main` push must each pass all four jobs in
the integrated workflow, including both native Windows security jobs. No
source PR is merged through GitHub's ordinary merge endpoint; their exact heads
become ancestors of the aggregate commit and are then verified as indirectly
merged.

## Frozen remote input

The publisher refuses to proceed if `main`, any PR head, or any PR open state
differs from this snapshot:

| Ref | Frozen SHA |
| --- | --- |
| `main` | `ac50dc896bea821f66c88c6ed50cf9185e4e31d1` |
| PR #211 | `eb6e4294f6f3397a336b15bf0c67ad8007a62f0f` |
| PR #212 | `4986716ae1d8985e1c1886dbd45bda22b2452eb7` |
| PR #213 | `da3e934feb6a4cb4a55f8251172839f67fd23ca8` |
| PR #214 | `89bf38c81016b656d5b0c5ef911b9ee7e7962432` |
| PR #215 | `b9e4954072a2e000e2b3dcc05e0aec3f7c550e4a` |
| PR #216 | `708fd443f1d2c8796b71acb7bc0ad1042dabd88c` |
| PR #217 | `dd9f224079825f066603b5f9949a531813a664b3` |

All seven values were re-read from GitHub immediately before final local
validation and still matched the frozen snapshot.

## Integrated changes

| Source | Integrated result |
| --- | --- |
| PR #211 | Adds GPU pixel-diff coverage for the compositor and preserves the reviewed text-rendering fixtures. |
| PR #212 | Adds the configurable account/login scaffold, verified-origin credential binding, nullable DTO handling, interaction coverage, and half-written credential recovery tests. |
| PR #213 | Adds freeze-frame editing through split and image-clip insertion, with collision-safe generated asset paths. |
| PR #214 | Aligns GPU text rasterization with the upstream layer behavior and retains replayed text tests. |
| PR #215 | Completes the media-library rewrite and global favorites, using retained filesystem capabilities, transactional manifest updates, rollback evidence, project-identity guards, and durable Save As bundle publication/recovery. |
| PR #216 | Adds save-clip/save-range-as-media, progress and operation-scoped cancellation, durable output reservation, zero-frame rejection, project postconditions, and H.264/HEVC/ProRes export coverage. |
| PR #217 | Adds the agent chat panel, streaming session state, tool dispatch, MCP import hardening, and reviewed menu/layout integration. |

The convergence work also includes the reviewed native playback replacement,
schema-compatibility read-only mode, project/media state arbitration, bounded
workers and caches, secure Unix/Windows filesystem capability layers, and two
native Windows CI jobs. The exact runtime code candidate changes 208 files with
55,706 additions and 3,833 deletions relative to frozen `main`; the later
Windows corrections add one test-only platform attribute, repair the
capability-relative native rename API, and restore Windows reserved-output
creation semantics.

## Final playback correction

Real-media validation found a latent false-ready condition in CPAL/CoreAudio:
`build` and `play` returned success but the output callback was never invoked.
The previous code installed that device position as the master clock, so the
renderer repeatedly emitted frame zero while its raw frame counter continued
to increase.

The final correction:

1. increments a callback epoch at the first instruction of each output callback;
2. requires two callbacks within a bounded one-second readiness window, so one
   asynchronous trailing callback cannot prove liveness;
3. keeps the hardware stream continuously running while retained sessions are
   logically muted, avoiding asynchronous WASAPI/CoreAudio pause/play
   acknowledgement races;
4. stages resume as `muted callback proof -> render clock seek/resume -> audio
   unmute`, so no already-consumed audio block is rewound;
5. restores a retained session to paused state if callback readiness fails; and
6. installs an advancing wall clock instead of a dead audio clock when initial
   callback readiness fails.

Two independent reviewers rejected the first attempted fix because it resumed
audio before the render clock seek and could rewind one device block. The
replacement design above was implemented before this release candidate was
accepted.

## Validation evidence

### Full integration baseline

The exact integration merge before the playback correction
(`cfa457c26492d70a8ac8c201277e59e586c0d162`) passed:

- `cargo test --workspace -- --test-threads=1`;
- workspace and minimal Tauri `cargo clippy --all-targets -D warnings`;
- 319 Tauri library tests, 55 core tests, 152 project tests, and 47 media
  library tests in focused independent runs;
- 65 Web test files / 658 tests and the Web production build;
- FFmpeg H.264, HEVC, and ProRes roundtrips;
- three real GPU export integrations and three manual two-second codec probes;
- seven playback integration tests and six playback transport tests; and
- ProRes and GPU color-grade playback probes.

### Playback correction candidate

The exact code candidate `cf52c5e495f9aea6b685aa20d863c5418a010ca5`
passed the following local checks:

- `cargo fmt --all -- --check`;
- all 25 audio unit tests, including callback success, timeout, one-trailing-
  callback rejection, advancing wall-clock fallback, and retained device-clock
  selection;
- all 28 playback command tests, including successful two-phase retained and
  fresh-install resume ordering, plus failed-resume state rollback;
- the full serial Rust workspace suite: 1,987 passed, 0 failed, 7 ignored;
- workspace all-target clippy with warnings denied;
- frozen Web installation, 65 test files / 658 tests, and the Web production
  build;
- the unsigned Tauri macOS application bundle build, with executable SHA-256
  `9c1c4a40325e6bb95a0bc1c99e8f2bd61737800aa94ac31c8d907f454777f9a3`
  and relative-file bundle digest
  `fd36d6eff7b450bf9434c86b3f400076f008bc38726cb09fae6d5d7697db9ba1`;
- the production-style real-media probe path (`prepared audio -> first GPU frame
  -> muted callback proof -> engine resume -> audio commit`);
- three manual playback probes in one run: H.264/AAC safe fallback
  (`frames=78`, `playhead=90`, non-black ratio `0.698`), ProRes playback
  (`52` frames, non-black ratio `1.000`), and GPU color-grade playback
  (`40` frames); and
- publisher syntax plus 13 offline state-machine tests covering mutation
  blocking, atomic state integrity, phase monotonicity, exact CI selection,
  indirect-merge ancestry, ambiguous-response recovery, and non-force main CAS.

These checks were run from a fresh `git archive` with the exact candidate tree.
The documentation-only successor is structurally compared with this code tree before
publication; the aggregate PR and final `main` push then repeat the
repository-defined CI on GitHub-hosted runners.

### Native Windows aggregate corrections

The first aggregate PR, #218, exposed one portable-test compilation error before
either native Windows job could reach its requested security filters. A test in
`decode/pcm.rs` executes `sh -c "exit 7"`, while its `Command` import was already
correctly limited to Unix. The test itself lacked the matching `#[cfg(unix)]`,
so both Windows jobs failed while compiling the full test harness.

Commit `301b82d2772559ba6fad25cbb2847ebc07baa494` adds only that platform
attribute. The Unix regression test passed, all 47 media-library tests passed,
and all-target `opentake-media` clippy with warnings denied passed. A fresh
release bundle from this successor is byte-identical to the installed runtime
binary and preserves the executable and bundle digests above. The failed #218
attempt is retained as audit evidence and must be closed as superseded.

Replacement aggregate PR #219 compiled and ran the full workflow. Its Web and
Rust jobs passed, while both native Windows jobs found independent runtime
defects:

1. the global media library passed a native-style relative-name buffer and
   retained parent handle through Win32
   `SetFileInformationByHandle(FILE_RENAME_INFO)`, while allocating less than
   the SDK's conservative documented buffer minimum; that combination made
   every handle-relative manifest rename fail with `ERROR_INVALID_PARAMETER`
   (87); and
2. project-media output reservation selected Windows `GENERIC_READ |
   GENERIC_WRITE | DELETE` with `access_mode`, but omitted Rust's semantic
   `write(true)` flag, so `OpenOptions` rejected `create_new` before reaching
   the Windows system call.

Code successor `71ba39ec57866346e492c5973196f8806e221710` switches the
library rename to the native `NtSetInformationFile(FileRenameInformation)`
contract, retains both source and parent capabilities, uses a complete
`FILE_RENAME_INFORMATION` buffer, and maps `NTSTATUS` back to an OS error. It
also synchronizes the output reservation's semantic read/write flags while
preserving its DELETE access, `CREATE_NEW`, and delete-sharing denial.

The correction passed formatting, diff checks, all 47 local media-library
tests, all-target media clippy with warnings denied, a focused reserved-output
identity regression, and a minimal `x86_64-pc-windows-msvc` compile probe for
the exact `windows-sys` FFI surface. A full repository cross-build remains
blocked by the third-party MSVC-header limitation below, so a new immutable
aggregate PR must pass both native Windows jobs and all four total jobs before
promotion. Failed #219 is retained as audit evidence and must also be closed as
superseded.

### Platform boundary

macOS cross-checks for the pure Rust project/capability crates on
`x86_64-pc-windows-msvc` pass, including `cargo check` and clippy with warnings
denied for `opentake-project`. A full media/Tauri Windows cross-build cannot be
completed on this host because a third-party C build requires the MSVC SDK
headers (`stdlib.h`). The integrated workflow therefore makes both native
Windows jobs mandatory:

- `Windows (cancel / reparse safety)`; and
- `Windows (library capability security)`.

## Qualified limitations

The current Mac enumerates the built-in speakers as the default two-channel
96 kHz output, but CPAL 0.15.3, 0.16.0, and 0.17.x all produced zero callbacks in
both OpenTake and isolated minimal programs. The strict real-audio qualification
mode (`OPENTAKE_REQUIRE_AUDIO_CALLBACK=1`) therefore fails on this host. This is
not reported as an audio-output pass. The safe fallback mode is verified: video
continues with a monotonic wall clock instead of freezing at frame zero. A host
with a working callback is still required to qualify audible device output.

The first fresh-archive full Rust run encountered the unchanged test
`new_epoch_can_schedule_same_cache_key_after_old_reservation_drops`. Its focused
retry passed, 20 additional repetitions passed, and a complete serial workspace
rerun passed with 1,987 tests. This is recorded as a pre-existing timing flake;
it did not recur in the final serial suite and is not hidden as a clean first
attempt.

The exact candidate bundle was installed at `/Applications/OpenTake.app`; its
installed executable and relative-file bundle digests matched the build output.
The application process and `127.0.0.1:19789` listener remained live, MCP
`initialize` advertised OpenTake 1.0.0, `tools/list` returned the editing tool
surface, and `get_timeline` returned a valid empty 30 fps, 1920 x 1080 timeline.
The previous installed bundle is preserved at
`/Applications/OpenTake.app.pre-cf52-20260714-010105` for rollback.

macOS screenshot capture in the current desktop session returned black frames /
failed single-pixel capture, and Accessibility window enumeration did not expose
the WebView window. Visual acceptance is therefore recorded as permission/API
blocked rather than passed. Bundle identity, process, WebKit page load, loopback
listener, and MCP protocol behavior were verified independently of screenshots.

## Independent review

- The integration conflict/security review accepted exact merge `cfa457c` with
  P0/P1/P2 = 0/0/0.
- The independent implementation review accepted exact merge `cfa457c` with
  P0/P1/P2 = 0/0/0.
- Two additional reviewers independently accepted exact code candidate
  `cf52c5e495f9aea6b685aa20d863c5418a010ca5` / tree
  `03166926f597fb87d15ca254dc358058dc86a749` with Critical/Important/Minor =
  0/0/0 and explicit **Ready to merge: Yes** verdicts. Their fresh-archive
  checks covered all 25 audio tests, all 28 serial playback-command tests,
  clippy with warnings denied, safe real-media fallback, strict audio
  rejection, and dependency-diff integrity.
- A fresh-archive reviewer accepted exact Windows CI correction
  `301b82d2772559ba6fad25cbb2847ebc07baa494` / tree
  `66ecb6ef09e66a077d109d3b3d18ca69ab66cb92` with
  Critical/Important = 0/0 and **Ready for new aggregate publish: Yes**. The
  only minor observation concerns a separate future full-Windows-test `mkfifo`
  fixture and does not affect either mandatory Windows workflow job.
- An independent Windows API review accepted code successor
  `71ba39ec57866346e492c5973196f8806e221710` / tree
  `975bc74f2c0bfd7ae31e0c07161ddac781230297` with
  Critical/Important = 0/0 and **Ready: Yes**, subject to the mandatory native
  Windows CI gate. Its two comment-only minor observations (FFI alignment and
  synchronous handle lifetime) were incorporated before the commit.
- The documentation-only successor is reviewed against that accepted code tree before
  its exact commit/tree is supplied to the cloud publisher.

## Cloud publication and completion criteria

The release publisher creates a complete snapshot tree, an aggregate commit
with ordered parents `main + #211..#217`, and a single aggregate PR. It waits for
every job in `.github/workflows/ci.yml` to complete successfully. Only then does
it create the deterministic two-parent final commit and compare-and-swap
`refs/heads/main` from the frozen old SHA with `force=false`.

Completion requires all of the following to be re-read from GitHub:

- final `main` equals the deterministic expected commit and exact local tree;
- aggregate and final parent order is exact;
- #211 through #217 and the aggregate PR are closed as merged;
- superseded failed aggregate attempts #218 and #219 are closed without being
  represented as successful merges;
- every original PR head is reachable from final `main`;
- aggregate pull-request CI and final `main` push CI each contain all four
  expected jobs and every job is `completed/success`; and
- the publisher's fsync-backed state reaches `complete` without remote drift.

Any mismatch preserves the existing aggregate ref/PR and local audit state,
then stops without force-pushing, weakening branch protection, deleting refs,
or claiming success.
