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

# Check dependency licenses (cargo-deny).
deny:
    cd rust && cargo deny check licenses

# Regenerate per-crate READMEs from lib docs (cargo-doc2readme).
readme:
    cd rust && cargo doc2readme -p annotations --lib --template crates/README.j2 --out crates/annotations/README.md
    cd rust && cargo doc2readme -p annotations-macros --lib --template crates/README.j2 --out crates/annotations-macros/README.md
    cd rust && cargo doc2readme -p contract --lib --template crates/README.j2 --out crates/contract/README.md
    cd rust && cargo doc2readme -p diff --lib --template crates/README.j2 --out crates/diff/README.md
    cd rust && cargo doc2readme -p extract --lib --template crates/README.j2 --out crates/extract/README.md
    cd rust && cargo doc2readme -p runtime --lib --template crates/README.j2 --out crates/runtime/README.md

# Verify per-crate READMEs are in sync with lib docs.
readme-check:
    cd rust && cargo doc2readme -p annotations --lib --template crates/README.j2 --out crates/annotations/README.md --check
    cd rust && cargo doc2readme -p annotations-macros --lib --template crates/README.j2 --out crates/annotations-macros/README.md --check
    cd rust && cargo doc2readme -p contract --lib --template crates/README.j2 --out crates/contract/README.md --check
    cd rust && cargo doc2readme -p diff --lib --template crates/README.j2 --out crates/diff/README.md --check
    cd rust && cargo doc2readme -p extract --lib --template crates/README.j2 --out crates/extract/README.md --check
    cd rust && cargo doc2readme -p runtime --lib --template crates/README.j2 --out crates/runtime/README.md --check

# Measure workspace test coverage (cargo-llvm-cov); fails under the line floor.
coverage:
    cd rust && cargo llvm-cov --workspace --all-features --summary-only --fail-under-lines 85

# Full pre-PR gate: build, test, clippy, format, evaluate, licenses, and READMEs.
check: build test clippy format-check evaluate deny readme-check
