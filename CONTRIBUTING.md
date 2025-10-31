# Contributing

## Toolchain
- Rust stable (pinned via rust-toolchain.toml)
- `cargo fmt`, `cargo clippy`, `cargo test` must pass

## Branch & PR
- Feature branches → PR to `main`
- Keep commits small and focused

## Style
- Edition 2024, `clippy::pedantic` in crates
- Avoid introducing deps in Phase 0; we add them as we progress
