# Data safety manifest-corruption real-device evidence — 2026-07-31

## Scope

- Plan: `data-safety-implementation.md`, Task 9 `DS-manifest-corruption-conflict`.
- Requirement: `requirement-7908bd026b5a91f6`.
- Boundary: keep `project.json` and present malformed `media.json` strict; allow only malformed optional `generation-log.json` to open in an explicit write-blocked compatibility session.

## Exact code evidence

- `malformed_manifest_is_an_error`: PASS, 1/1.
- `malformed_manifest_contract_matches_authoritative_source`: PASS, 1/1.

The schema contract covers complete current-version open/edit/save/reopen, missing and malformed required timeline, syntactically and structurally malformed media manifests, missing-manifest default and safe creation, legacy `{}` preservation, malformed optional generation-log recovery, error priority, same-path write rejection, Save As rejection before destination creation, and full nofollow tree-receipt equality.

The plan's expected-RED note described the state before the reviewed test was introduced. On the audited branch the exact test already exists (landed with the cross-cutting data-safety gates), so both focused commands are now green; no artificial failure was introduced.

## Packaged malformed-media result

The application was already displaying the valid generation-seed project with four media entries and two text clips. Opening `/private/tmp/opentake-ds-task9-malformed-media.opentake`, whose `media.json` contains invalid JSON, did not replace the live session:

- the same four media entries remained visible;
- the preview still rendered `Generation seed verified`;
- both original timeline clip IDs remained present;
- no new project, journal, staging, or sibling artifact appeared.

The rejected fixture remained exactly two files with SHA-256 values:

- `project.json`: `b3973a664bd87f47496250b9a32cbbd489b2d7d14e0c43fd3ff777415a2bc1e8`;
- `media.json`: `85c606726a48c5b18880fda848fffe4cbce09090f3b96210b37b1c415fb520c1`.

## Packaged malformed-generation result

Opening `/private/tmp/opentake-ds-task9-malformed-generation.opentake` succeeded only as an explicit compatibility recovery session. The UI displayed:

- `兼容性只读模式`;
- one compatibility issue;
- `generation-log.json:invalid-or-unreadable`;
- editing and saving disabled.

Clicking **添加文本** caused no accessibility-tree or timeline change. The empty timeline stayed empty. The preserved file SHA-256 values were:

- `project.json`: `b3973a664bd87f47496250b9a32cbbd489b2d7d14e0c43fd3ff777415a2bc1e8`;
- `media.json`: `8faebdcf603b7f74817d302fe63613b7547581e03e80da3fb31977abe16df1fe`;
- damaged `generation-log.json`: `36b98fe44f4a237ff39f862cc1083270583b9bcb3127d7bf32b67bd4d4bbea46`.

No rejected Save As destination or related sibling artifact existed after the run.

## Regression gate

- both Task 9 focused commands: PASS.
- `cargo fmt --all -- --check`: PASS on the same source tree.
- `cargo test --workspace --no-fail-fast --quiet`: PASS on the same source tree immediately before this evidence-only change; all executed tests passed, with only explicitly ignored tests skipped.
- `git diff --check`: PASS.

## Outcome

Task 9 is complete. Exact persistence tests and the packaged application agree on strict manifest failure, incumbent-session preservation, explicit generation-log recovery, disabled mutation, and unchanged damaged bytes. This closes one data-safety record only; it does not reclassify later tasks or authorize Beta publication.
