# Apple Silicon FFmpeg license replacement — 2026-08-01

## Scope

This evidence closes the Apple Silicon `--enable-nonfree` sidecar blocker for the
first local Beta candidate. It does not close the native Windows or Intel macOS
sidecar checks, Developer ID signing, notarization, or final packaged-app
verification.

## Pinned supply

The Apple Silicon lock records pin both the downloaded archive and the exact
extracted executable:

| Tool | Source | Archive SHA-256 | Executable SHA-256 |
| --- | --- | --- | --- |
| FFmpeg 7.0 | `https://www.osxexperts.net/ffmpeg7arm.zip` | `563111a239fe70d2e5c84a5382204a7d0bf0a332385a92a44baff36d313e27f2` | `326895b16940f238d76e902fc71150f10c388c281985756f9850ff800a2f1499` |
| FFprobe 7.0 | `https://www.osxexperts.net/ffprobe7arm.zip` | `e5ae34ee2f0b3594892a695fd733646904bbc7eb40af3b359ed91538ddcb5513` | `307e09bc01bd72bde5f441a1a6df68769da3b2b6e431accfbfc9cf3893ad00c4` |

Both executables report a GPL configuration without `--enable-nonfree`; their
`-L` output does not contain FFmpeg's non-redistributable warning.

## Fail-closed supply verification

`scripts/provision_ffmpeg_sidecars.py` now verifies the archive hash, requires
the one pinned ZIP member, enforces an extraction-size ceiling, verifies the
executable hash, and runs the executable license/configuration checks before it
can replace the destination. Verification rejects `--enable-nonfree` and the
phrase `not legally redistributable`.

The Python regression suite passed all four cases covering lock validation,
pinned ZIP extraction, archive mismatch rejection, and license rejection.

## Media compatibility gate

With the two pinned Apple Silicon executables selected explicitly through
`OPENTAKE_FFMPEG` and `OPENTAKE_FFPROBE`, `cargo test -p opentake-media
--no-fail-fast` passed. This covered 401 unit tests (one separately gated test
ignored), 13 FFmpeg integration tests (one external-fixture test ignored), plus
the denoise, facade, HDR, loudness, proxy, and stems test binaries. The real
sidecar paths exercised probing, RGBA frame decoding, PCM extraction,
thumbnails, waveforms, H.264, H.265, ProRes, HDR, and proxy generation.

The final Beta gate must still recheck the pinned source hashes before signing,
then prove the embedded executables retain the expected version and license
metadata, pass code-sign validation, and work with an empty `PATH`. macOS code
signing mutates Mach-O bytes, so the post-signing executable hash is not expected
to equal the pre-signing supply hash.
