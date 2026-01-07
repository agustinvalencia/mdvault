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

coverage:
    cargo tarpaulin --workspace --all-features --out Html --out Xml

coverage-clean:
    rm -f cobertura.xml tarpaulin-report.html tarpaulin-report.xml tarpaulin-report.json lcov.info coverage.json
