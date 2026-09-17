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

# Regenerate (bless) the golden corpus from live captures, then review the diff.
[windows]
bless:
    cd rust; $env:DW_BLESS = "1"; cargo test -p golden --features driftwatch

[unix]
bless:
    cd rust && DW_BLESS=1 cargo test -p golden --features driftwatch

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
    cd rust && cargo doc2readme -p artifact --lib --template crates/README.j2 --out crates/artifact/README.md
    cd rust && cargo doc2readme -p contract --lib --template crates/README.j2 --out crates/contract/README.md
    cd rust && cargo doc2readme -p diff --lib --template crates/README.j2 --out crates/diff/README.md
    cd rust && cargo doc2readme -p extract --lib --template crates/README.j2 --out crates/extract/README.md
    cd rust && cargo doc2readme -p golden --lib --template crates/README.j2 --out crates/golden/README.md
    cd rust && cargo doc2readme -p runtime --lib --template crates/README.j2 --out crates/runtime/README.md

# Verify per-crate READMEs are in sync with lib docs.
readme-check:
    cd rust && cargo doc2readme -p annotations --lib --template crates/README.j2 --out crates/annotations/README.md --check
    cd rust && cargo doc2readme -p annotations-macros --lib --template crates/README.j2 --out crates/annotations-macros/README.md --check
    cd rust && cargo doc2readme -p artifact --lib --template crates/README.j2 --out crates/artifact/README.md --check
    cd rust && cargo doc2readme -p contract --lib --template crates/README.j2 --out crates/contract/README.md --check
    cd rust && cargo doc2readme -p diff --lib --template crates/README.j2 --out crates/diff/README.md --check
    cd rust && cargo doc2readme -p extract --lib --template crates/README.j2 --out crates/extract/README.md --check
    cd rust && cargo doc2readme -p golden --lib --template crates/README.j2 --out crates/golden/README.md --check
    cd rust && cargo doc2readme -p runtime --lib --template crates/README.j2 --out crates/runtime/README.md --check

# Measure workspace test coverage (cargo-llvm-cov); fails under the line floor.
coverage:
    cd rust && cargo llvm-cov --workspace --all-features --summary-only --fail-under-lines 85

# On-demand HYBRID mutation testing of the emitter TCB, killed by the runtime
# encoder tests + byte-exact trace goldens. cargo-gamma mutates the runtime
# emitter (rust/gamma.toml); cargo-mutants mutates the annotation proc-macros
# that gamma cannot reach (rust/.cargo/mutants.toml). Slow, non-circular trust-
# anchor measurement — NOT a per-PR gate. Survivors are gaps in the goldens: add
# a golden (preferred) or record a justified exclusion. DW_GOLDEN_DIR points the
# sandboxed gamma build at the repo-root corpus (outside the cargo workspace).
[windows]
mutants:
    cd rust; $env:DW_GOLDEN_DIR = "{{justfile_directory()}}/tests/golden"; cargo gamma run
    cd rust && cargo mutants

[unix]
mutants:
    cd rust && DW_GOLDEN_DIR="{{justfile_directory()}}/tests/golden" cargo gamma run
    cd rust && cargo mutants

# Full pre-PR gate: build, test, clippy, format, licenses, READMEs, and evaluate.
# `evaluate` runs last: it is not a CI gate (unavailable on hosted runners), so a
# known-baseline evaluate finding must not short-circuit the CI-gating checks.
check: build test clippy format-check deny readme-check evaluate
