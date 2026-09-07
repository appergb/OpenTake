# Contributing to OpenTake

> Status: canonical · Stage: implementation-backed · Updated: 2026-09-06

Contributions are welcome at [appergb/OpenTake](https://github.com/appergb/OpenTake). Discuss large changes in an issue before implementation. In this multi-worktree workspace, use `OpenTake-generation/`; a fresh clone may use its chosen directory name.

Start with [AGENTS.md](AGENTS.md), then read the relevant [module overview and index](docs/modules/INDEX.md). Development rules, upstream references, frame/serialization contracts and build commands have one source: [development conventions](docs/project/conventions.md).

## Validation

Run checks appropriate to the changed behavior from the repository root:

```bash
cargo fmt --all -- --check
cargo clippy
cargo test
pnpm --dir web install --frozen-lockfile
pnpm --dir web test
pnpm --dir web build
python3 scripts/check_docs.py
```

The frontend currently has no `lint` script; its build includes TypeScript checking. Record commands actually run. Native playback, GPU, audio, providers and installer changes need environment-specific evidence; a browser fallback does not validate a desktop package. Keep user edits and retained audit assets intact.

## Documentation and releases

Update the relevant module documentation with code changes. Keep historical dates and release versions. Current work follows the [public Beta plan](docs/plans/active/2026-09-06-public-beta.md); [Beta 6](docs/releases/1.0.0-beta.6.md) is a candidate pending validation and publication. Release/tag/signing operations belong to the release owner.

## License

Contributions use the repository's GPL-3.0-or-later license; retain upstream attribution and third-party notices.
