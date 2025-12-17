doctor: 
    cargo run --bin markadd -- doctor

golden-update:
    INSTA_UPDATE=auto cargo test -p markadd

ci: fmt lint test

test:
    cargo test --all

fmt:
    cargo fmt

lint:
    cargo check

