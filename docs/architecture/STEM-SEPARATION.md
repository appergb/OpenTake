# Stem separation execution boundaries

OpenTake currently exposes two explicit execution choices. Only the local path is executable in this release; the hosted path is intentionally fail-closed.

## Local execution

`opentake-center-v1` is a bundled, inspectable DSP profile, not a neural semantic-separation model. On first use OpenTake atomically installs the 104-byte profile under the application model directory and verifies SHA-256 `9c72ab220f370000a702fc11c8071905648a56d1102d9519659a6062abb4b376`. An existing file with a different digest is rejected rather than silently replaced.

The processor decodes the source to stereo 48 kHz PCM and derives centre `(L + R) / 2` and side `(L - R) / 2` signals. Vocals and accompaniment are published as stereo dual-mono WAV assets so each remains audible in the current mono export mixdown. This works well when voice/dialogue is centred and the desired accompaniment has stereo side information. It does not promise semantic separation of centred instruments, reverb, or mono material and must not be described as Demucs/MDX-equivalent.

Local processing is offline. Source media is hashed but never modified or uploaded. Both result files are published and imported atomically, and each media entry records the source asset id, source SHA-256, execution id, model SHA-256, and stem kind.

## Hosted execution

Hosted selection requires all of the following before routing can be considered:

- an explicit provider;
- a provider-qualified model whose prefix matches that provider;
- explicit confirmation that the source audio may be uploaded;
- a configured provider adapter.

The desktop application currently has no stem-separation transport adapter. It rejects incomplete consent/configuration and performs no upload. Adding a hosted adapter later must reuse the provider registry and derived-media provenance path rather than placing credentials, signed URLs, or provider diagnostics in the project.

## Job and failure semantics

The Tauri owner allows one active separation job, emits source-scoped progress, and exposes cancellation. Output is written into a unique project `media/stems-<uuid>` directory via partial files and atomic renames. Cancellation, processing failure, output-probe failure, or import failure removes the job directory. The two derived assets enter the shared media manifest in one persisted batch so a project cannot retain only one side of a successful operation.

The Inspector identifies the local privacy boundary, the hosted upload boundary, active progress, cancellation, typed failures, and the ids/provenance of successful outputs. Derived assets use the same preview, timeline, playback, save/reopen, and export paths as ordinary imported audio.
