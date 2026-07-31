# MR-stems packaged-runtime evidence — 2026-08-01

## Scope and package identity

Task 13 was exercised in the rebuilt release bundle at `target/release/bundle/macos/OpenTake.app`. The final executable SHA-256 was `942aa7d0e432c270146d04a025c5c3632b61333e17f92369d7152295d6bd20c8`; strict deep code-sign verification passed. The bundle is ad-hoc signed and has no Team ID, so this evidence is local packaged-runtime verification, not Developer ID notarization or a publishable Beta signature.

The saved test project was `/private/tmp/opentake-stems-real-device-20260801.opentake`. Its deterministic five-second stereo mixture was `/private/tmp/opentake-stems-mix-20260801.wav` (SHA-256 `295804e62dcc5d5f4f8146b78c485ac19823c98529ee025455c2c368d5530524`). The vocal reference SHA-256 was `835ee1c4414a75acc6ce87b9d3f2ad0f37b3f190e9e1958c08c051906524c2c6`; the final mono-compatible accompaniment reference SHA-256 was `444f012257e20c6e4fe4d70e9fcb30c7c87d26e3429498c7226e4b4529c3f495`.

## Privacy, failure and success paths

- Hosted mode visibly required provider/model and upload consent. Submitting without consent returned the localized error `使用托管服务前必须确认上传源音频。`. Supplying labels and consent with no adapter returned `所选托管服务尚未配置可用的音轨分离适配器。`; no upload occurred.
- Local mode displayed `完全在本机处理，音频不会上传。首次使用会安装并校验内置模型。` and completed with execution id `local:opentake-center-v1`.
- The installed model profile was 104 bytes and matched SHA-256 `9c72ab220f370000a702fc11c8071905648a56d1102d9519659a6062abb4b376`.
- The two results appeared as independent derived media assets. Save/reopen retained `generationInput` source/model hashes, selecting either asset exposed its resolved project-relative path and audio preview, and both could be placed on the timeline and played.

The final derived job directory was `media/stems-686f5e60-a666-4768-87e5-f610c9f92d97`. Vocals SHA-256 was `8fc799724bb9ce760fd90447ba348a64b8af6eb4dc0092735731c8a8a3790f76`; accompaniment SHA-256 was `5ef6b291022bce125a10e1954ecaefce5223051a74b352cbd768403da70dda8a`. Direct PCM comparison measured `85.3732 dB` SDR for both vocal channels and `83.8316 dB` for both accompaniment channels.

## Packaged export

Each stem was independently exported through the normal timeline path:

- `/private/tmp/opentake-stems-vocals-export-v2-20260801.mp4`, SHA-256 `8290427162eb6fb94fc591829017646299901b75ceeb42a288585aa129054428`, five seconds, H.264 1920×1080 plus AAC mono 48 kHz, `34.1008 dB` SDR against the vocal reference.
- `/private/tmp/opentake-stems-accompaniment-export-v2-20260801.mp4`, SHA-256 `6031b6badec7a1b57eacbde3563ce92b2e93c497685a89081e08987656b258c2`, five seconds, H.264 1920×1080 plus AAC mono 48 kHz, `25.5688 dB` SDR against the mono-compatible accompaniment reference. Its mean level was `-13.2 dB`, confirming a non-silent export.

The first side-channel implementation cancelled to silence in the mono export path. A focused regression failed at `-3.010 dB` for mono compatibility; publishing both stems as dual-mono fixed the actual export while preserving the deterministic centre/side boundary. This is a compatibility choice, not a claim of semantic source separation.

## Cancellation and cleanup

Cancellation was verified with `/private/tmp/opentake-stems-cancel-1800s-20260801.wav` (330 MiB, SHA-256 `072c31ef023f50d6f8a87ec88e42ca1adceec2f64e1489c866f63bc163aec4b6`). The packaged UI reached `正在分离音轨… 8 %`; `取消分离` returned it to idle. The media manifest remained at nine entries with only the imported source matching that fixture, no matching derived item, no new stem job directory, and no `.partial` or temporary output. A shorter 300-second fixture finished before cancellation could be observed and is not counted as cancellation evidence.

## Runtime defects found and closed

- Selecting the first generated asset initially produced a black WebView through an unstable Zustand selector. A RED Inspector regression reproduced `Maximum update depth exceeded`; deriving generation sections from the stable `items` selector fixed it.
- Project-relative derived assets initially returned no path in `MediaItemDto`. A RED Rust test observed `None`; returning the resolved path fixed direct preview and timeline use.
- The initial anti-phase accompaniment exported silently through the mono mixer. The focused mono-compatibility RED and the packaged export above verify the dual-mono correction.

The local algorithm is an integrity-checked centre/side DSP profile suited to centred voice/dialogue fixtures. It is not a neural Demucs/MDX separator and does not close the broader semantic-separation gap for arbitrary mixes.
