# rust-analyzer metadata-source PoC report

> **Frozen feasibility artifact.** This spike is tracked under
> `docs/spikes/ra-extract/` (outside the `rust/` workspace; not built by CI).
> It backs Phase R — see `docs/decisions/phase-r-static-resolver.md`. Command
> paths below are quoted as originally run from the repo root when the crate
> still lived at `spikes/ra-extract/`.

Scratch crate: `docs/spikes/ra-extract/` (standalone; not added to `rust/Cargo.toml`). Fixture: `docs/spikes/ra-extract/fixture/`.

Command evidence:

- `cargo run --manifest-path .\spikes\ra-extract\Cargo.toml --locked --quiet`
- `cargo clean --manifest-path .\spikes\ra-extract\Cargo.toml; cargo build --manifest-path .\spikes\ra-extract\Cargo.toml --locked --quiet`
- `cargo tree --manifest-path .\spikes\ra-extract\Cargo.toml --locked --prefix none`

## Results by criterion

1. ✅ RA loads fixture as a library on stable Windows. Evidence: run output `load: ok on stable rustc 1.96.0 ...`; RA loaded 2426 VFS files. Required `cargo_config.sysroot = Some(RustLibSource::Discover)` and `ProcMacroServerChoice::None` for usable HIR/source mapping. With proc-macro expansion enabled, `Semantics::to_def` failed on `#[watch_operation]` items; selection should scan source attrs and resolve pre-expansion syntax.
2. ✅ Foreign-alias error recovery. Evidence: `io::Result<String>` normalized to `Result<String, Error>` with error `std::io::error::Error`; `anyhow::Result<u32>` normalized to `Result<u32, Error>` with error `anyhow::Error`; `fmt::Result` normalized to `Result<(), Error>` / `core::fmt::Error`.
3. PARTIAL Observation type resolution. Evidence: `explicit_result` reports `observation obs: Vec<Receipt>`, produced via `Semantics::type_of_expr`. Caveat: macro arguments are token trees, so the PoC falls back from the `watch_point!` token to the selected binding initializer (`receipts`). A production adapter needs a robust macro-token-to-expression mapping or macro expansion strategy.
4. PARTIAL Cross-component dependency resolution. Evidence: `explicit_result -> inventory` and registry component dependency emitted. Caveat: the PoC uses the macro's explicit `component = "inventory"` selector; resolving the call target inside `watch_dep!` token trees was not robust with proc macros disabled.
5. ✅ Private-item visibility. Evidence: private annotated `billing::private_io` is enumerated and typed.
6. ✅ Join-back precision incl. impl methods. Evidence: `method_charge module=billing fn=method_charge private=false method=true`; RA source/HIR key gives module path + function name for the impl method.
7. ✅ RegistryDocument validation with populated errors/observations. Evidence: local reconstructed registry reports `local registry validation violations: 0`, with errors populated for `io`, `anyhow`, `fmt`, explicit/local Result and observation `obs=Vec<Receipt>`. The real `extract::derive(...)` was also called with leaked scratch `OpMeta`/`TypeMeta`, but returned 6 violations because it reparses stringified TypeRefs and cannot accept already-resolved `TypeRef`s or observations/dependencies. That is seam friction, not a production-code change.
8. ✅ Cost/brittleness. Versions pinned: `ra_ap_hir`, `ra_ap_ide`, `ra_ap_ide_db`, `ra_ap_load-cargo`, `ra_ap_project_model`, `ra_ap_syntax`, `ra_ap_vfs` all `=0.0.349`; latest `0.0.352` requires rustc 1.98, while repo toolchain is rustc 1.96.0. Extra pin: `unicode-ident = =1.0.22`; without it, `ra-ap-rustc_lexer 0.166.0` panicked at compile time because `unicode-ident 1.0.26` and `unicode-properties 0.1.4` had different Unicode versions. Lockfile contains 228 packages; `cargo tree` unique line count observed: 271. Clean build time after cache downloads: 88.77s on this machine. Adapter directly touched 9 RA API surface types/functions: `load_workspace_at`, `RootDatabase`, `Vfs`, `Semantics`, `Function`, `Type`, `Adt`, `Callable`, `ast`.

## Recommendation

GO, with constraints. RA proves the headline value: real type inference normalizes aliases that the current string pipeline cannot, recovering error outcomes for `io::Result`, `anyhow::Result`, local aliases, and `fmt::Result`. Migration should keep annotations as selection/aiming, add an RA-backed resolver behind/adjacent to the `derive()` seam, and change that seam to accept resolved `contract::TypeRef`/operation observations/dependencies directly. That would let Driftwatch replace/delete `extract::type_ref.rs`, most of `return_kind.rs`, the static string-metadata half of `annotations-macros/src/operation.rs` and `watchable.rs`, and `discovery_json`'s contract role. Keep runtime dynamic emission separate.

Risks: RA API/version pinning is brittle and heavy; proc-macro expansion currently hurts HIR join-back for attribute macros, so production should prefer source scanning plus pre-expansion HIR resolution, and separately design robust token-tree mapping for `watch_point!`/`watch_dep!` expression sites.
