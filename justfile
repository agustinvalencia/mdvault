install: 
    cargo install --path crates/cli

doctor: 
    cargo run --bin markadd -- doctor

ci: fmt lint test

test:
    cargo test --all

fmt:
    cargo fmt

lint:
    cargo clippy --all-targets --all-features -- -D warnings

golden-update:
    INSTA_UPDATE=auto cargo test -p markadd
