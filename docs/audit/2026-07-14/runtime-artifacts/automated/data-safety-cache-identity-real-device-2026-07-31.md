# Data safety cache identity real-device evidence — 2026-07-31

## Scope

- Plan: `data-safety-implementation.md`, Task 6 `DS-cache-identity-complete`.
- Requirement: `requirement-52da354b46176a99`.
- Boundary: stable lowercase 32-hex file identity from path, Foundation/Swift modification time, and size, with no identity for missing metadata.

## Exact code evidence

- `identity_hex_is_stable_and_lowercase`: PASS, 1/1.
- `identity_hex_matches_swift_for_whole_second_mtime`: PASS, 1/1.
- `file_identity_key_missing_file_is_none`: PASS, 1/1.

The full cache-key module additionally covers every identity component, Foundation subsecond rounding, pre-epoch times, Swift closest-shortest and exponential formatting, independent SHA-256 prefixes, visual versus transcript/embedding seed order, real file metadata, and missing files.

These owning tests already passed at the initial baseline, so Task 6 required runtime evidence closure rather than a production patch; no artificial RED failure was introduced.

## Packaged application result

During Task 3, the packaged application imported `/private/tmp/opentake-task8-music.wav` and visibly rendered its waveform in the media panel. The production `MediaVisualCache` wrote:

`~/Library/Caches/com.opentake.desktop/media-cache/MediaVisualCache/67f1356ac2a7dc98b08d839f7cbb38a5.waveform`

The source file had:

- byte size: `88278`;
- Foundation modification time: `1785475345.638414`;
- absolute path: `/private/tmp/opentake-task8-music.wav`.

An independent Swift process used `Foundation.FileManager` for the same size/date values and `CryptoKit.SHA256`, retaining the first 32 lowercase hex characters. It produced:

- transcript/embedding order `path|mtime|size`: `246a0ff48bba2d5609aff5bd6fa3e7f6`;
- visual order `path|size|mtime`: `67f1356ac2a7dc98b08d839f7cbb38a5`.

The independently calculated visual identity exactly matched the packaged application's real cache filename. The filename is 32 lowercase hex characters, representing the first 16 SHA-256 bytes as required.

## Outcome

Task 6 is complete: focused Rust vectors, missing-file behavior, and an actual packaged waveform cache agree with the independent Swift/Foundation implementation. This closes one data-safety record only; it does not reclassify the remaining data-safety tasks or authorize Beta publication.
