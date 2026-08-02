# Data safety generation-log seed real-device evidence — 2026-07-31

## Scope

- Plan: `data-safety-implementation.md`, Task 5 `DS-generation-seed`.
- Requirements: `requirement-b9010e6717b5d5ea`, `requirement-1f35cc4131f8f0b7`.
- Boundary: missing or malformed optional generation log, deterministic manifest-provenance seed, save, reopen, and idempotence.

## Exact code evidence

- `malformed_generation_log_is_ignored`: PASS, 1/1.
- `missing_generation_log_seeds_manifest_provenance_once`: PASS, 1/1.

The core owning test covers empty and mixed manifests, imported versus generated assets, duplicate provenance, manifest reorder, duplicate asset identities, signed zero timestamps, edit/save/reopen, existing-log precedence, malformed optional logs, compatibility read-only protection, and byte-stable repeated saves.

Both exact tests already passed at the initial baseline, so this task required packaged-app evidence closure rather than a new production patch; no artificial RED failure was introduced.

## Packaged application fixture

`/private/tmp/opentake-ds-generation-seed-real-device.opentake` started with:

- no `generation-log.json`;
- one imported media entry without generation provenance;
- two video entries carrying identical legacy provenance;
- one audio entry carrying different legacy provenance;
- an empty timeline.

The packaged application opened the bundle and visibly rendered all four offline media entries with relink recovery controls. Before editing, a filesystem check confirmed `generation-log.json` was absent.

## Edit, save, and reopen

Using the packaged UI, the run added a text clip with `Generation seed verified` and returned Home to save. The new `generation-log.json` contained exactly two entries:

- one deterministic `legacy-generation:{sha256}` id for `legacy-video-model` at `700000000.0`;
- one different deterministic id for `legacy-audio-model` at `700000010.0`.

The imported asset was excluded and the duplicate video provenance collapsed to one event. The application then reopened the project from Recents: the text rendered in the preview, all four media entries remained, and the timeline contained the saved clip. Returning Home again produced no duplicate event and no byte changes.

Stable SHA-256 values after both the first and second save:

| File | SHA-256 |
|---|---|
| `generation-log.json` | `9aaecae32ecef935036afc9dc918617cc80451912092623384897834ee593702` |
| `project.json` | `ebf598a4150763d70a473cfd47ba5250952f3f1be5ac0a54af5587dbe4cccd8b` |
| `media.json` | `ea8a7397f7f247a3bf6f71c03ff881e102b8adec9ca02c1ed0af7259eb46ddb8` |

The persisted log count remained two and all ids remained unique after the second process-level open/save cycle.

## Outcome

Task 5 is complete: exact tests and a packaged legacy-project round trip agree on deterministic one-time seeding, deduplication, safe persistence, and idempotence. This closes two data-safety records only; it does not reclassify the remaining data-safety tasks or authorize Beta publication.
