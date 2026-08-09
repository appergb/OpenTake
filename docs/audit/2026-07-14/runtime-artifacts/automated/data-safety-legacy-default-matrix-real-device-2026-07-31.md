# Data safety legacy/default matrix real-device evidence — 2026-07-31

## Scope

- Plan: `data-safety-implementation.md`, Task 1 `DS-legacy-default-matrix`.
- Requirement: `requirement-365ac4943b157d3e`.
- Boundary: exhaustive current/legacy/missing/malformed/future compatibility, plus a representative packaged-app open/edit/save/reopen round trip.

## Exact code evidence

All four named owning tests existed at the initial baseline and passed without a production change:

- `applies_clip_defaults_for_omitted_fields`: PASS, 1/1.
- `migrates_legacy_transform_xy_to_center`: PASS, 1/1.
- `migrates_generation_log_legacy_cost_and_version`: PASS, 1/1.
- `exhaustive_legacy_default_matrix`: PASS, 1/1.

The exhaustive test covers current and legacy decoding, omitted defaults, malformed and unknown-future fail-closed branches, read-only/save-as guards, and open/edit/save/reopen persistence. Because the reviewed production boundary was already complete, the initial baseline was GREEN; no artificial failing implementation was introduced merely to satisfy the generated RED wording.

## Packaged application fixture

The debug macOS application opened `/private/tmp/opentake-ds-legacy-real-device.opentake`, created specifically for this acceptance run. The input intentionally omitted project dimensions, frame rate, settings state, track and clip identifiers, track flags, media type/source type, media-manifest version/folders, and generation-log version. It also used legacy transform `x/y` and legacy generation `cost` fields, and pointed at a missing external movie.

The first native-open attempt deliberately selected `/private/tmp` itself and the application visibly rejected it with `missing required project.json in bundle at /private/tmp`, then returned safely to Home. Selecting the actual package opened successfully and rendered:

- 1920 × 1080 at 30 fps;
- a three-second offline-media placeholder with relink recovery;
- synthesized track and clip identities;
- a migrated timeline clip on V1.

## Edit, save, and reopen

Using only the packaged application UI, the run added a text clip and set its content to `Legacy migration verified`, returned to Home to save, and reopened the project from the recent-project card. The reopened preview rendered the exact text, the timeline retained both V1 and V2 clips, and the offline-media recovery surface remained available.

The persisted JSON then proved the full representative migration:

- project defaults: `width=1920`, `height=1080`, `fps=30`, `settingsConfigured=false`;
- track defaults: generated UUIDs, `muted=false`, `hidden=false`, `syncLocked=true`;
- clip defaults: generated UUIDs, `mediaType/sourceClipType=video`, zero trims/fades/crop, unit speed/volume/opacity;
- transform migration: legacy `x=0.1/y=0.2` became `centerX≈0.1/centerY≈0.2`, while width/height remained `0.5`;
- media manifest: `version=1`, `folders=[]`;
- generation log: `version=1`, legacy `cost=0.42` became `costCredits=42`;
- real edit: the added text clip persisted `textContent="Legacy migration verified"` and reopened visibly.

Persisted artifact SHA-256 values after the successful round trip:

| File | SHA-256 |
|---|---|
| `generation-log.json` | `2b802e37cc1eeb8ae818de7d77f3ea6e57300232050c24765704cb879fc71cae` |
| `media.json` | `0d56801a702905b9450fa8ff0854a224560693397f81f67d66394b488b86a8c9` |
| `project.json` | `1ce3f603dd4a12de83733817e7b6aab97a91474ae90b66ed4ca0aa461cc78986` |

## Outcome

Task 1 is complete: exact compatibility tests and the packaged real-device round trip agree on the same migration behavior, including visible invalid-bundle recovery. This closes one data-safety record only; it does not reclassify the remaining data-safety tasks or authorize Beta publication.
