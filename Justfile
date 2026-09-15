set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

default: ci

fmt:
    cargo fmt --all
    taplo fmt

fmt-check:
    cargo fmt --all -- --check
    taplo fmt --check

lint:
    cargo clippy --all-targets --all-features -- -D warnings

test:
    cargo nextest run --all-features
    cargo test --doc

test-plain:
    cargo test --all-features

cov:
    cargo llvm-cov nextest --all-features --lcov --output-path lcov.info

audit:
    cargo deny check

docs:
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features

ci: fmt-check lint test audit docs
