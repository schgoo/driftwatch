set windows-shell := ["pwsh.exe", "-NoLogo", "-NoProfile", "-NonInteractive", "-Command"]

_default:
    @just --list

# Build the workspace
build:
    cd rust && cargo build --workspace --all-targets

# Run all tests (both feature configs: default/off-identity and trace-on)
test:
    cd rust && cargo test --workspace
    cd rust && cargo test --workspace --all-features

# Run clippy (warnings as errors), both feature configs
clippy:
    cd rust && cargo clippy --workspace --all-targets -- -D warnings
    cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings

# Check formatting
format-check:
    cd rust && cargo fmt -- --check

# Apply formatting
format:
    cd rust && cargo fmt

# Run cargo-evaluate (deterministic + semantic lint packs). Part of the harness
# — a merge gate, not advisory. Rule exceptions live in rust/evaluate.toml.
evaluate:
    cd rust && cargo evaluate

# Full pre-PR gate: build, test, clippy, format, and evaluate.
check: build test clippy format-check evaluate
