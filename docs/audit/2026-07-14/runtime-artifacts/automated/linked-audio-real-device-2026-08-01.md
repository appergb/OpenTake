# MR-linked-audio-complete packaged-runtime evidence — 2026-08-01

## Scope and package identity

This acceptance run used `target/release/bundle/macos/OpenTake.app`. The final executable SHA-256 was `942aa7d0e432c270146d04a025c5c3632b61333e17f92369d7152295d6bd20c8`; strict deep code-sign verification passed. The bundle remains ad-hoc signed with no Team ID, so this is packaged-runtime evidence rather than Developer ID notarization.

The deterministic source was `/private/tmp/opentake-linked-audio-silent-20260801.mp4`, SHA-256 `ee12524fb8a661a396b2b578e06ae6d2e06cdd2ec80b3e42faa69afe45901b4f`. It is five seconds of H.264 640×360 at 30 fps and has no audio stream. The isolated saved project was `/private/tmp/opentake-linked-audio-real-device-20260801.opentake`.

## Code boundary

- `cargo test -p opentake-agent does_not_link_audio_when_source_has_no_audio` executed `add_clips_does_not_link_audio_when_source_has_no_audio` and `insert_clips_does_not_link_audio_when_source_has_no_audio`: 2 passed, 0 failed.
- `cargo test -p opentake-media video_with_zero_channel_audio_has_no_audio`: 1 passed, 0 failed.
- The owning Agent tests were introduced by `a2f34cb`; the zero-channel probe regression was introduced by `9920468`. Because the inherited implementation already satisfied the contract, the current acceptance baseline was GREEN and no artificial RED was manufactured.

The effective gate is `place_clip`: linked audio requires all four conditions `add_linked_audio`, video target, video source type, and `has_audio`. `resolve_media_kind` carries the manifest value into both Agent command paths. FFprobe audio streams that explicitly report zero channels produce `has_audio=false`; an absent audio stream does likewise in the real fixture.

## Packaged UI and persistence

In the rebuilt desktop application:

1. A new empty project was saved through the native Save panel.
2. The source was imported through the native Import panel. The material library showed exactly one five-second video item.
3. Double-clicking the item added it to the timeline. The accessibility tree showed exactly one track label, `V1`, with one clip. No `A1` or `A2` label or audio clip was created.
4. Playback advanced visibly from `00:00:00` to `00:01:05` of `00:05:00`.
5. The persisted `media.json` entry recorded `type:"video"` and `hasAudio:false`.
6. The persisted `project.json` contained one video track and one video clip. The clip had no serialized `linkGroupId`.
7. Export completed with the visible message `导出完成 · 1280×720 · 150 帧`.

The exported file was `/private/tmp/opentake-linked-audio-real-device-20260801.mp4`, SHA-256 `1b01b65af6d3c6e8524b21a42ff6010de2e7ba69f9f361f858bb416d6d306e96`. Independent FFprobe inspection found a single H.264 video stream at 1280×720, 30 fps, duration 5.000 seconds, and no audio stream. This verifies import, timeline placement, playback, persistence, and export parity for the silent-video boundary.
