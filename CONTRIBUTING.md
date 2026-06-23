# Contributing to OpenTake

We welcome contributions! Please open an [Issue](https://github.com/appergb/OpenTake/issues) for discussion before submitting large PRs.

## Development Setup

```bash
# Prerequisites: Rust >= 1.82, Node.js >= 20, pnpm, FFmpeg >= 6.0
cargo build
cargo test
cd web && pnpm install && pnpm build
```

## Code Style

- Rust: `cargo fmt` + `cargo clippy`
- TypeScript: `pnpm run lint`

## License

By contributing, you agree that your contributions will be licensed under GPL-3.0.
