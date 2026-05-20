# Zero_Nine development task runner
# Usage: just <recipe>

test:
    cargo test --all-targets

fmt:
    cargo fmt --all -- --check

clippy:
    cargo clippy --all-targets --all-features -- -D warnings

review:
    cargo insta review

docker-build:
    docker build -t zero-nine .

docker-run:
    docker run --rm zero-nine --help

coverage:
    cargo tarpaulin --all-features --workspace --out Html
