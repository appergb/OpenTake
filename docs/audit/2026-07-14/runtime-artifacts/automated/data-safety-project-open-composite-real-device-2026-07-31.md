# Data safety project-open composite real-device evidence — 2026-07-31

## Scope

- Plan: `data-safety-implementation.md`, Task 8 `DS-project-open-composite-headings`.
- Requirements: `requirement-6748d221ef0d9a4c`, `requirement-9335bc98b18f8d8d`, `requirement-706e744a85684655`, and `requirement-b38818cf815e0f1e`.
- Boundary: validate and prepare a complete project candidate, admit playback/prewarm, atomically publish one runtime snapshot, then expose it consistently to media, render, playback, captions, and Agent/MCP consumers.

## Exact code evidence

All nine reviewed owning tests passed exactly:

- `exhaustive_legacy_default_matrix`: PASS, 1/1.
- `missing_generation_log_seeds_manifest_provenance_once`: PASS, 1/1.
- `project_open_composite_acceptance`: PASS, 1/1.
- `deferred_apply_rejects_version_and_project_drift_without_mutation`: PASS, 1/1.
- `app_core_media_path_stress_never_mixes_project_snapshots`: PASS, 1/1.
- `caption_commit_rejects_stale_project_revision`: PASS, 1/1.
- `project_open_mapped_boundaries_composite_acceptance`: PASS, 1/1.
- `prewarm_rejection_restores_active_playback_without_project_publish`: PASS, 1/1.
- `transcript_batch_resolution_uses_one_snapshot_and_authoritative_types`: PASS, 1/1.

Together these tests exercise current and legacy input, missing optional files, malformed media rejection without mutation, generation-log seeding, byte-stable save/reopen, prepared-versus-committed publication, stale deferred writes, concurrent Agent path reads, caption revision drift, prewarm rejection and incumbent playback recovery, plus consistent UI-media/render/playback/Agent projections from one production runtime snapshot.

The owning implementation and tests were already green at the audited baseline; no artificial RED failure was introduced.

## Packaged application evidence

The packaged debug application supplied complementary user-visible coverage through two independent real bundle round trips:

1. `/private/tmp/opentake-ds-legacy-real-device.opentake` opened with omitted legacy fields, synthesized defaults and IDs, migrated transforms and generation cost, an offline-media recovery surface, and a visible timeline. A text edit saved and reopened with exact persisted semantics.
2. `/private/tmp/opentake-ds-generation-seed-real-device.opentake` opened with four manifest assets and no generation log. The UI exposed every media entry, deterministically seeded two unique generation records, excluded imported provenance, saved a text edit, and reopened byte-stably without duplicate history.

The deliberately invalid selection of `/private/tmp` failed visibly with `missing required project.json in bundle at /private/tmp` and returned safely to Home. The malformed generation fixture also failed without replacing the live session; after correcting the fixture, the same packaged process opened successfully.

While the second project remained open, the live production MCP server read the same two desktop timeline clips, added a third clip, and undid it. The desktop UI immediately converged back to those same two clips. This directly verifies that desktop and Agent consumers observe the committed shared core session, not separate placeholder state.

Detailed child evidence is retained in:

- `data-safety-legacy-default-matrix-real-device-2026-07-31.md`;
- `data-safety-generation-seed-real-device-2026-07-31.md`;
- `data-safety-shared-core-command-real-device-2026-07-31.md`.

## Regression gate

- all nine Task 8 focused commands: PASS.
- `cargo fmt --all -- --check`: PASS on the same source tree.
- `cargo test --workspace --no-fail-fast --quiet`: PASS on the same source tree immediately before this evidence-only change; all executed tests passed, with only explicitly ignored tests skipped.
- `git diff --check`: PASS.

## Outcome

Task 8 is complete. Deterministic failure injection and two packaged-app round trips agree that project open is validated before publication, failure preserves the incumbent session and bytes, and every mapped consumer reads one committed snapshot. This closes four composite records only; it does not reclassify later data-safety tasks or authorize Beta publication.
