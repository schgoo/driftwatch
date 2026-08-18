set windows-shell := ["pwsh.exe", "-NoLogo", "-NoProfile", "-NonInteractive", "-Command"]

_default:
    @just --list

# Build the workspace
build:
    cd rust && cargo build --workspace --all-targets

# Run all tests
test:
    cd rust && cargo test --workspace

# Run clippy (warnings as errors)
clippy:
    cd rust && cargo clippy --workspace --all-targets -- -D warnings

# Check formatting
format-check:
    cd rust && cargo fmt -- --check

# Apply formatting
format:
    cd rust && cargo fmt

# Run cargo-evaluate (deterministic + semantic lint packs). Part of the harness
# — a merge gate, not advisory. Rule exceptions live in rust/evaluate.toml.
evaluate:
    # `prefer_strum_derive` is a native rule not covered by evaluate.toml's
    # `[config] disabled`; `Value`'s Display is recursive and cannot be derived
    # by strum, so it is allowed here explicitly.
    cd rust && cargo evaluate -A evaluate::prefer_strum_derive

# Full pre-PR gate: build, test, clippy, format, and evaluate.
check: build test clippy format-check evaluate
