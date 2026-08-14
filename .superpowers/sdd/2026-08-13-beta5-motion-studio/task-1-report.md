# Motion Studio Task 1 report

Status: COMPLETE — CodeMirror dependency and license boundary implemented and independently approved.

## Scope

- Added exact runtime dependencies `codemirror@6.0.2`, `@codemirror/lang-html@6.4.12`, `@codemirror/lang-css@6.3.1`, and `@codemirror/theme-one-dark@6.1.3`.
- Added root third-party notices with the official repository, exact version, MIT identity, installed license path, exact copyright, and full published license text.
- Added a fail-closed Python inventory gate that binds `package.json`, the pnpm v9 runtime importer, `packages`, `snapshots`, installed manifest version/license/repository, the pinned published LICENSE SHA-256, and the notice text.
- Added adversarial tests for a missing notice, changed resolved version, runtime-to-dev dependency movement, missing resolution/snapshot record, lookalike repository, truncated license, and accidental deletion of any required direct package.

## TDD evidence

Initial RED: repository validation failed because all four direct packages and `THIRD_PARTY_NOTICES.md` were absent.

Review REDs reproduced:

- removing only the `packages` record or only the `snapshots` record was accepted;
- a lookalike installed repository and truncated two-word MIT file were accepted;
- moving an exact importer stanza from runtime dependencies to dev dependencies was accepted.

Final GREEN:

```text
python3 -B -m unittest scripts/test_check_license_inventory.py
Ran 7 tests — OK

python3 -B scripts/check_license_inventory.py
CodeMirror dependency and license inventory is valid

pnpm -C web install --frozen-lockfile --offline
Lockfile is up to date — exit 0

pnpm -C web licenses list --prod
All direct CodeMirror packages and resolved editor dependencies report MIT — exit 0

pnpm -C web build
TypeScript and Vite build passed; only pre-existing dynamic-import/chunk-size warnings

git diff --check -- <Task 1 files>
exit 0
```

## Review

Independent code review round 1 found three HIGH and one MEDIUM fail-open/test-coupling issues. Re-review found one additional HIGH runtime/dev importer ambiguity. Both review rounds were reproduced with adversarial tests and fixed. Final review verdict: Spec PASS, Quality PASS, APPROVE, zero findings.

## Commit

`build(motion): add licensed CodeMirror editor dependencies`
