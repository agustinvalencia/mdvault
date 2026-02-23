install:
    cargo install --path crates/cli

doctor:
    cargo run --bin markadd -- doctor

ci: fmt lint test

test:
    cargo test --all --all-features -- --nocapture

fmt:
    cargo fmt --all -- --check

fmt-fix:
    cargo fmt --all

lint:
    cargo clippy --all-targets --all-features -- -D warnings

golden-update:
    INSTA_UPDATE=auto cargo test -p markadd

coverage:
    cargo tarpaulin --workspace --all-features --timeout 120 --out Xml --out Lcov --out json

coverage-clean:
    rm -f cobertura.xml tarpaulin-report.html tarpaulin-report.xml tarpaulin-report.json tarpaulin-report.md lcov.info coverage.json
