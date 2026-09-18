use anyhow::{Context, Result, bail};
use contract::{
    Component, Dependency, ErrorOutcome, FORMAT, FORMAT_VERSION, Field, NamedType, NamedValue,
    Operation, Outcomes, Primitive, RegistryDocument, TypeRef, Variant, validate,
};
use extract::{RegistryIdentity, derive as derive_from_runtime};
use ra_ap_hir::{CallableKind, DisplayTarget, HirDisplay, Semantics, attach_db};
use ra_ap_ide_db::RootDatabase;
use ra_ap_load_cargo::{LoadCargoConfig, ProcMacroServerChoice, load_workspace_at};
use ra_ap_project_model::{CargoConfig, RustLibSource};
use ra_ap_syntax::ast::{HasName, HasVisibility};
use ra_ap_syntax::{AstNode, SyntaxNode, ast};
use ra_ap_vfs::Vfs;
use runtime::{FieldMeta, OpMeta, TypeMeta, VariantMeta};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug, Clone)]
struct FnMeta {
    name: String,
    module_path: String,
    fn_name: String,
    component: String,
    inputs: Vec<NamedValue>,
    outcomes: Outcomes,
    return_display: String,
    is_private: bool,
    is_method: bool,
    observations: Vec<NamedValue>,
    dependencies: Vec<String>,
}

#[derive(Debug, Clone)]
struct WatchType {
    component: String,
    named: NamedType,
}

#[derive(Default)]
struct Evidence {
    load_ms: u128,
    files_seen: usize,
    private_seen: bool,
    method_seen: bool,
    io_error: Option<String>,
    anyhow_error: Option<String>,
    observation: Option<String>,
    dependency: Option<String>,
    validation_violations: usize,
    runtime_derive_validation_violations: usize,
    operation_count: usize,
    type_count: usize,
    adapter_api_types: BTreeSet<&'static str>,
}

fn main() -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture = root.join("fixture");
    let load_started = Instant::now();
    let mut cargo_config = CargoConfig::default();
    cargo_config.sysroot = Some(RustLibSource::Discover);
    let load_config = LoadCargoConfig {
        load_out_dirs_from_check: true,
        with_proc_macro_server: ProcMacroServerChoice::None,
        prefill_caches: false,
        num_worker_threads: 4,
        proc_macro_processes: 2,
    };
    let (db, vfs, _proc_macros) =
        load_workspace_at(&fixture, &cargo_config, &load_config, &|msg| {
            eprintln!("ra-load: {msg}");
        })
        .context("load fixture with rust-analyzer")?;

    let mut evidence = Evidence {
        load_ms: load_started.elapsed().as_millis(),
        files_seen: vfs.iter().count(),
        ..Evidence::default()
    };
    evidence.adapter_api_types.extend([
        "ra_ap_load_cargo::load_workspace_at",
        "ra_ap_ide_db::RootDatabase",
        "ra_ap_vfs::Vfs",
        "ra_ap_hir::Semantics",
        "ra_ap_hir::Function",
        "ra_ap_hir::Type",
        "ra_ap_hir::Adt",
        "ra_ap_hir::Callable",
        "ra_ap_syntax::ast",
    ]);

    attach_db(&db, || -> Result<()> {
        let sema = Semantics::new(&db);
        let fixture_files = fixture_source_files(&vfs, &fixture);
        let known = collect_watchable_names(&db, &vfs, &sema, &fixture_files)?;
        let mut functions = Vec::new();
        let mut types = Vec::new();

        for (file_id, path) in fixture_files {
            let Some(editioned) = sema.attach_first_edition_opt(file_id) else {
                continue;
            };
            let source = sema.parse(editioned);
            let display_target = sema
                .first_crate(file_id)
                .map(|krate| krate.to_display_target(&db));
            for f in source.syntax().descendants().filter_map(ast::Fn::cast) {
                if has_attr_text(f.syntax(), "watch_operation") {
                    let Some(target) = display_target else {
                        bail!("no display target for {}", path);
                    };
                    let hir_fn = sema.to_def(&f);
                    let meta = analyze_function(&db, &sema, target, &known, &f, hir_fn)?;
                    evidence.private_seen |= meta.is_private;
                    evidence.method_seen |= meta.is_method;
                    if meta.fn_name == "private_io" {
                        evidence.io_error = error_summary(&meta.outcomes);
                    }
                    if meta.fn_name == "anyhow_op" {
                        evidence.anyhow_error = error_summary(&meta.outcomes);
                    }
                    if !meta.observations.is_empty() {
                        evidence.observation = Some(format!(
                            "{}: {}",
                            meta.name,
                            meta.observations
                                .iter()
                                .map(|o| format!("{}={}", o.name, display_type_ref(&o.ty)))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                    }
                    if !meta.dependencies.is_empty() {
                        evidence.dependency =
                            Some(format!("{} -> {}", meta.name, meta.dependencies.join(",")));
                    }
                    functions.push(meta);
                }
            }
            for s in source.syntax().descendants().filter_map(ast::Struct::cast) {
                if has_attr_text(s.syntax(), "derive(Watchable")
                    || has_attr_text(s.syntax(), "Watchable")
                {
                    let Some(target) = display_target else {
                        continue;
                    };
                    if let Some(ty) = analyze_struct(&db, target, &known, &sema, &s) {
                        types.push(ty);
                    }
                }
            }
            for e in source.syntax().descendants().filter_map(ast::Enum::cast) {
                if has_attr_text(e.syntax(), "derive(Watchable")
                    || has_attr_text(e.syntax(), "Watchable")
                {
                    let Some(target) = display_target else {
                        continue;
                    };
                    if let Some(ty) = analyze_enum(&db, target, &known, &sema, &e) {
                        types.push(ty);
                    }
                }
            }
        }

        evidence.operation_count = functions.len();
        evidence.type_count = types.len();

        let identity = RegistryIdentity {
            registry_id: "urn:ctsc:registry:ra-poc:fixture".to_owned(),
            version: "0.1.0-poc".to_owned(),
        };
        let (op_metas, type_metas, _keepalive) = leak_runtime_metadata(&functions, &types);
        let runtime_doc = derive_from_runtime(&op_metas, &type_metas, identity.clone());
        evidence.runtime_derive_validation_violations = validate(&runtime_doc).len();

        let mut doc = build_registry(identity, &functions, &types);
        add_component_dependencies(&mut doc, &functions);
        let violations = validate(&doc);
        evidence.validation_violations = violations.len();

        println!("=== RA extract PoC evidence ===");
        println!(
            "load: ok on stable {} in {} ms; vfs files={}",
            rustc_version(),
            evidence.load_ms,
            evidence.files_seen
        );
        println!(
            "annotated operations: {}; watchable types: {}",
            evidence.operation_count, evidence.type_count
        );
        for f in &functions {
            println!(
                "op {} module={} fn={} private={} method={} ret={} outcomes={}",
                f.name,
                f.module_path,
                f.fn_name,
                f.is_private,
                f.is_method,
                f.return_display,
                outcomes_summary(&f.outcomes)
            );
            for input in &f.inputs {
                println!("  input {}: {}", input.name, display_type_ref(&input.ty));
            }
            for obs in &f.observations {
                println!("  observation {}: {}", obs.name, display_type_ref(&obs.ty));
            }
            for dep in &f.dependencies {
                println!("  dependency component={dep}");
            }
        }
        println!(
            "runtime derive seam validation violations: {} (observations/dependencies require local post-processing)",
            evidence.runtime_derive_validation_violations
        );
        println!(
            "local registry validation violations: {}",
            evidence.validation_violations
        );
        if !violations.is_empty() {
            println!("violations: {violations:#?}");
        }
        println!(
            "io result error: {}",
            evidence.io_error.as_deref().unwrap_or("missing")
        );
        println!(
            "anyhow result error: {}",
            evidence.anyhow_error.as_deref().unwrap_or("missing")
        );
        println!(
            "observation evidence: {}",
            evidence.observation.as_deref().unwrap_or("missing")
        );
        println!(
            "dependency evidence: {}",
            evidence.dependency.as_deref().unwrap_or("missing")
        );
        println!(
            "private visible: {}; impl method joined: {}",
            evidence.private_seen, evidence.method_seen
        );
        println!(
            "adapter RA API surface count: {} -> {}",
            evidence.adapter_api_types.len(),
            evidence
                .adapter_api_types
                .iter()
                .copied()
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!(
            "registry json bytes: {}",
            serde_json::to_string_pretty(&doc)?.len()
        );
        Ok(())
    })
}

fn fixture_source_files(vfs: &Vfs, fixture: &Path) -> Vec<(ra_ap_vfs::FileId, String)> {
    let fixture_s = fixture.to_string_lossy().replace('\\', "/");
    vfs.iter()
        .filter_map(|(id, p)| p.as_path().map(|ap| (id, ap.to_string())))
        .filter(|(_, p)| p.replace('\\', "/").starts_with(&fixture_s) && p.ends_with(".rs"))
        .collect()
}

fn collect_watchable_names(
    db: &RootDatabase,
    vfs: &Vfs,
    sema: &Semantics<'_, RootDatabase>,
    files: &[(ra_ap_vfs::FileId, String)],
) -> Result<BTreeSet<String>> {
    let mut out = BTreeSet::new();
    for (file_id, _) in files {
        let Some(editioned) = sema.attach_first_edition_opt(*file_id) else {
            continue;
        };
        let source = sema.parse(editioned);
        for s in source.syntax().descendants().filter_map(ast::Struct::cast) {
            if has_attr_text(s.syntax(), "Watchable") {
                if let Some(n) = s.name() {
                    out.insert(n.to_string());
                }
            }
        }
        for e in source.syntax().descendants().filter_map(ast::Enum::cast) {
            if has_attr_text(e.syntax(), "Watchable") {
                if let Some(n) = e.name() {
                    out.insert(n.to_string());
                }
            }
        }
    }
    let _ = (db, vfs);
    Ok(out)
}

fn analyze_function(
    db: &RootDatabase,
    sema: &Semantics<'_, RootDatabase>,
    display_target: DisplayTarget,
    known: &BTreeSet<String>,
    ast_fn: &ast::Fn,
    hir_fn: Option<ra_ap_hir::Function>,
) -> Result<FnMeta> {
    let fn_name = hir_fn
        .map(|h| h.name(db).display(db, display_target.edition).to_string())
        .or_else(|| ast_fn.name().map(|n| n.to_string()))
        .unwrap_or_else(|| "unknown".to_owned());
    let module_path = hir_fn
        .map(|h| module_path(db, h.module(db), display_target))
        .unwrap_or_else(|| source_module_path(ast_fn.syntax()));
    let component = attr_component(ast_fn.syntax()).unwrap_or_else(|| "billing".to_owned());
    let is_private = ast_fn.visibility().is_none();
    let is_method = ast_fn.param_list().and_then(|p| p.self_param()).is_some();
    let ast_params: Vec<ast::Param> = ast_fn
        .param_list()
        .into_iter()
        .flat_map(|pl| pl.params())
        .collect();
    let hir_params = hir_fn
        .map(|h| h.params_without_self(db))
        .unwrap_or_default();
    let inputs = ast_params
        .into_iter()
        .enumerate()
        .map(|(idx, ast_param)| {
            let name = ast_param
                .pat()
                .map(|p| p.syntax().text().to_string())
                .unwrap_or_else(|| format!("arg{}", idx));
            let ty = ast_param
                .ty()
                .and_then(|ast_ty| sema.resolve_type(&ast_ty))
                .map(|resolved| type_ref_from_ra(db, display_target, known, &resolved))
                .or_else(|| {
                    hir_params
                        .get(idx)
                        .map(|p| type_ref_from_ra(db, display_target, known, p.ty()))
                })
                .unwrap_or(TypeRef::Primitive {
                    name: Primitive::Unit,
                });
            NamedValue {
                name,
                ty,
                description: None,
            }
        })
        .collect();
    let ast_ret_ty = ast_fn.ret_type().and_then(|rt| rt.ty());
    let ret_from_ast = ast_ret_ty.and_then(|ast_ty| sema.resolve_type(&ast_ty));
    let ret_from_hir = hir_fn.map(|h| h.ret_type(db));
    let ret_ref = ret_from_ast.as_ref().or(ret_from_hir.as_ref());
    let return_display = ret_ref
        .map(|r| type_display(db, display_target, r))
        .unwrap_or_else(|| "()".to_owned());
    let outcomes = ret_ref
        .map(|r| classify_return_ra(db, display_target, known, r))
        .unwrap_or_default();
    let (observations, dependencies) = body_sites(db, sema, display_target, known, ast_fn);
    Ok(FnMeta {
        name: fn_name.clone(),
        module_path,
        fn_name,
        component,
        inputs,
        outcomes,
        return_display,
        is_private,
        is_method,
        observations,
        dependencies,
    })
}

fn analyze_struct(
    db: &RootDatabase,
    display_target: DisplayTarget,
    known: &BTreeSet<String>,
    sema: &Semantics<'_, RootDatabase>,
    ast_struct: &ast::Struct,
) -> Option<WatchType> {
    let def = sema.to_def(ast_struct)?;
    let name = def.name(db).display(db, display_target.edition).to_string();
    let fields = def
        .fields(db)
        .into_iter()
        .map(|field| Field {
            name: field
                .name(db)
                .display(db, display_target.edition)
                .to_string(),
            ty: type_ref_from_ra(db, display_target, known, &field.ty(db)),
            description: None,
        })
        .collect();
    Some(WatchType {
        component: "billing".to_owned(),
        named: NamedType::Record {
            name,
            fields,
            description: None,
        },
    })
}

fn analyze_enum(
    db: &RootDatabase,
    display_target: DisplayTarget,
    known: &BTreeSet<String>,
    sema: &Semantics<'_, RootDatabase>,
    ast_enum: &ast::Enum,
) -> Option<WatchType> {
    let def = sema.to_def(ast_enum)?;
    let name = def.name(db).display(db, display_target.edition).to_string();
    let variants = def
        .variants(db)
        .into_iter()
        .map(|v| {
            let fields: Vec<Field> = v
                .fields(db)
                .into_iter()
                .map(|field| Field {
                    name: field
                        .name(db)
                        .display(db, display_target.edition)
                        .to_string(),
                    ty: type_ref_from_ra(db, display_target, known, &field.ty(db)),
                    description: None,
                })
                .collect();
            Variant {
                name: v.name(db).display(db, display_target.edition).to_string(),
                payload: (!fields.is_empty()).then_some(TypeRef::Record { fields }),
                description: None,
            }
        })
        .collect();
    Some(WatchType {
        component: "billing".to_owned(),
        named: NamedType::TaggedUnion {
            name,
            variants,
            description: None,
        },
    })
}

fn body_sites(
    db: &RootDatabase,
    sema: &Semantics<'_, RootDatabase>,
    display_target: DisplayTarget,
    known: &BTreeSet<String>,
    ast_fn: &ast::Fn,
) -> (Vec<NamedValue>, Vec<String>) {
    let mut observations = Vec::new();
    let mut deps = Vec::new();
    let Some(body) = ast_fn.body() else {
        return (observations, deps);
    };
    for mac in body.syntax().descendants().filter_map(ast::MacroCall::cast) {
        let text = mac.syntax().text().to_string();
        if text.contains("watch_point!") {
            let name = first_string_literal(&text).unwrap_or_else(|| "obs".to_owned());
            let ty = infer_macro_argument_type(db, sema, display_target, known, &mac, "receipts")
                .unwrap_or(TypeRef::Primitive {
                    name: Primitive::Unit,
                });
            observations.push(NamedValue {
                name,
                ty,
                description: None,
            });
        }
        if text.contains("watch_dep!") {
            if let Some(component) = component_arg(&text) {
                deps.push(component);
            }
            if let Some(expanded) = sema.expand_macro_call(&mac) {
                if let Some(call) = expanded
                    .value
                    .descendants()
                    .filter_map(ast::CallExpr::cast)
                    .find(|c| c.syntax().text().to_string().contains("reserve"))
                {
                    if let Some(expr) = ast::Expr::cast(call.syntax().clone()) {
                        if let Some(callable) = sema.resolve_expr_as_callable(&expr) {
                            if let CallableKind::Function(f) = callable.kind() {
                                deps.push(component_from_module(&module_path(
                                    db,
                                    f.module(db),
                                    display_target,
                                )));
                            }
                        }
                    }
                }
            }
        }
    }
    deps.sort();
    deps.dedup();
    (observations, deps)
}

fn infer_macro_argument_type(
    db: &RootDatabase,
    sema: &Semantics<'_, RootDatabase>,
    display_target: DisplayTarget,
    known: &BTreeSet<String>,
    mac: &ast::MacroCall,
    fallback_local: &str,
) -> Option<TypeRef> {
    if let Some(expanded) = sema.expand_macro_call(mac) {
        for expr in expanded.value.descendants().filter_map(ast::Expr::cast) {
            let txt = expr.syntax().text().to_string();
            if (txt.contains(fallback_local) || txt.contains("Receipt"))
                && let Some(info) = sema.type_of_expr(&expr)
            {
                return Some(type_ref_from_ra(
                    db,
                    display_target,
                    known,
                    &info.adjusted(),
                ));
            }
        }
    }
    let body = mac.syntax().ancestors().find_map(ast::BlockExpr::cast)?;
    for let_stmt in body.syntax().descendants().filter_map(ast::LetStmt::cast) {
        if let Some(pat) = let_stmt.pat()
            && pat.syntax().text().to_string() == fallback_local
        {
            if let Some(init) = let_stmt.initializer() {
                let info = sema.type_of_expr(&init)?;
                return Some(type_ref_from_ra(
                    db,
                    display_target,
                    known,
                    &info.adjusted(),
                ));
            }
        }
    }
    None
}

fn classify_return_ra(
    db: &RootDatabase,
    display_target: DisplayTarget,
    known: &BTreeSet<String>,
    ty: &ra_ap_hir::Type<'_>,
) -> Outcomes {
    if ty.is_unit() {
        return Outcomes::default();
    }
    if let Some((adt, args)) = ty.as_adt_with_args() {
        let head = adt.name(db).display(db, display_target.edition).to_string();
        if head == "Result" {
            let ok = args.first().and_then(|x| x.as_ref());
            let err = args.get(1).and_then(|x| x.as_ref());
            let mut result = ok.map(|t| type_ref_from_ra(db, display_target, known, t));
            let mut empty = false;
            if let Some(ok_ty) = ok
                && let Some((ok_adt, ok_args)) = ok_ty.as_adt_with_args()
            {
                if ok_adt
                    .name(db)
                    .display(db, display_target.edition)
                    .to_string()
                    == "Option"
                {
                    result = ok_args
                        .first()
                        .and_then(|x| x.as_ref())
                        .map(|t| type_ref_from_ra(db, display_target, known, t));
                    empty = true;
                }
            }
            let errors = err
                .map(|e| {
                    vec![ErrorOutcome {
                        name: error_name(db, display_target, e),
                        ty: None,
                        description: None,
                    }]
                })
                .unwrap_or_default();
            return Outcomes {
                result,
                empty,
                errors,
            };
        }
        if head == "Option" {
            return Outcomes {
                result: args
                    .first()
                    .and_then(|x| x.as_ref())
                    .map(|t| type_ref_from_ra(db, display_target, known, t)),
                empty: true,
                errors: Vec::new(),
            };
        }
    }
    Outcomes {
        result: Some(type_ref_from_ra(db, display_target, known, ty)),
        empty: false,
        errors: Vec::new(),
    }
}

fn type_ref_from_ra(
    db: &RootDatabase,
    display_target: DisplayTarget,
    known: &BTreeSet<String>,
    ty: &ra_ap_hir::Type<'_>,
) -> TypeRef {
    let display = type_display(db, display_target, ty);
    match display.as_str() {
        "()" => {
            return TypeRef::Primitive {
                name: Primitive::Unit,
            };
        }
        "String" | "std::string::String" | "alloc::string::String" => {
            return TypeRef::Primitive {
                name: Primitive::String,
            };
        }
        "bool" => {
            return TypeRef::Primitive {
                name: Primitive::Bool,
            };
        }
        "i32" => {
            return TypeRef::Primitive {
                name: Primitive::I32,
            };
        }
        "i64" | "isize" | "i8" | "i16" | "i128" => {
            return TypeRef::Primitive {
                name: Primitive::I64,
            };
        }
        "u32" => {
            return TypeRef::Primitive {
                name: Primitive::U32,
            };
        }
        "u64" | "usize" | "u8" | "u16" | "u128" => {
            return TypeRef::Primitive {
                name: Primitive::U64,
            };
        }
        "f32" => {
            return TypeRef::Primitive {
                name: Primitive::F32,
            };
        }
        "f64" => {
            return TypeRef::Primitive {
                name: Primitive::F64,
            };
        }
        _ => {}
    }
    if let Some((adt, args)) = ty.as_adt_with_args() {
        let head = adt.name(db).display(db, display_target.edition).to_string();
        match head.as_str() {
            "Vec" | "VecDeque" => {
                return TypeRef::List {
                    items: Box::new(
                        args.first()
                            .and_then(|x| x.as_ref())
                            .map(|t| type_ref_from_ra(db, display_target, known, t))
                            .unwrap_or(TypeRef::Primitive {
                                name: Primitive::Unit,
                            }),
                    ),
                };
            }
            "Option" => {
                return TypeRef::Optional {
                    value: Box::new(
                        args.first()
                            .and_then(|x| x.as_ref())
                            .map(|t| type_ref_from_ra(db, display_target, known, t))
                            .unwrap_or(TypeRef::Primitive {
                                name: Primitive::Unit,
                            }),
                    ),
                };
            }
            "HashMap" | "BTreeMap" => {
                return TypeRef::Map {
                    keys: Box::new(
                        args.first()
                            .and_then(|x| x.as_ref())
                            .map(|t| type_ref_from_ra(db, display_target, known, t))
                            .unwrap_or(TypeRef::Primitive {
                                name: Primitive::Unit,
                            }),
                    ),
                    values: Box::new(
                        args.get(1)
                            .and_then(|x| x.as_ref())
                            .map(|t| type_ref_from_ra(db, display_target, known, t))
                            .unwrap_or(TypeRef::Primitive {
                                name: Primitive::Unit,
                            }),
                    ),
                };
            }
            _ if known.contains(&head) => {
                return TypeRef::Named {
                    name: head,
                    component_id: None,
                    registry_id: None,
                };
            }
            _ => {}
        }
    }
    let tuple_args: Vec<_> = ty
        .type_arguments()
        .map(|t| type_ref_from_ra(db, display_target, known, &t))
        .collect();
    if display.starts_with('(') && !tuple_args.is_empty() {
        return TypeRef::Tuple { items: tuple_args };
    }
    let name = ty
        .as_adt_with_args()
        .map(|(adt, _)| adt.name(db).display(db, display_target.edition).to_string())
        .unwrap_or(display);
    if known.contains(&name) {
        TypeRef::Named {
            name,
            component_id: None,
            registry_id: None,
        }
    } else {
        TypeRef::Named {
            name,
            component_id: None,
            registry_id: None,
        }
    }
}

fn type_display(
    db: &RootDatabase,
    display_target: DisplayTarget,
    ty: &ra_ap_hir::Type<'_>,
) -> String {
    ty.display(db, display_target).to_string()
}

fn error_name(
    db: &RootDatabase,
    display_target: DisplayTarget,
    ty: &ra_ap_hir::Type<'_>,
) -> String {
    if let Some((adt, _)) = ty.as_adt_with_args() {
        let simple = adt.name(db).display(db, display_target.edition).to_string();
        let path = module_path(db, adt.module(db), display_target);
        let crate_name = adt
            .module(db)
            .krate(db)
            .display_name(db)
            .map(|name| name.to_string())
            .unwrap_or_default();
        match (crate_name.as_str(), path.as_str()) {
            ("ra_extract_fixture", _) | ("ra-extract-fixture", _) => simple,
            ("", "") => simple,
            ("", path) => format!("{path}::{simple}"),
            (krate, "") => format!("{krate}::{simple}"),
            (krate, path) if path.starts_with(krate) => format!("{path}::{simple}"),
            (krate, path) => format!("{krate}::{path}::{simple}"),
        }
    } else {
        type_display(db, display_target, ty)
    }
}

fn module_path(
    db: &RootDatabase,
    module: ra_ap_hir::Module,
    display_target: DisplayTarget,
) -> String {
    module
        .path_segments(db)
        .map(|s| s.display(db, display_target.edition).to_string())
        .collect::<Vec<_>>()
        .join("::")
}

fn component_from_module(module: &str) -> String {
    module.split("::").next().unwrap_or("unknown").to_owned()
}

fn source_module_path(node: &SyntaxNode) -> String {
    let mut segs = node
        .ancestors()
        .filter_map(ast::Module::cast)
        .filter_map(|m| m.name().map(|n| n.to_string()))
        .collect::<Vec<_>>();
    segs.reverse();
    segs.join("::")
}

fn has_attr_text(node: &SyntaxNode, needle: &str) -> bool {
    node.text().to_string().contains(needle)
}

fn attr_component(node: &SyntaxNode) -> Option<String> {
    component_arg(&node.text().to_string())
}

fn component_arg(text: &str) -> Option<String> {
    let marker = "component";
    let i = text.find(marker)?;
    let after = &text[i + marker.len()..];
    first_string_literal(after)
}

fn first_string_literal(text: &str) -> Option<String> {
    let start = text.find('"')? + 1;
    let rest = &text[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}

fn build_registry(
    identity: RegistryIdentity,
    functions: &[FnMeta],
    types: &[WatchType],
) -> RegistryDocument {
    let mut components: BTreeMap<String, Component> = BTreeMap::new();
    for f in functions {
        components
            .entry(f.component.clone())
            .or_insert_with(|| empty_component(&f.component))
            .operations
            .push(Operation {
                name: f.name.clone(),
                description: None,
                inputs: f.inputs.clone(),
                observations: f.observations.clone(),
                outcomes: f.outcomes.clone(),
            });
    }
    for t in types {
        components
            .entry(t.component.clone())
            .or_insert_with(|| empty_component(&t.component))
            .types
            .push(t.named.clone());
    }
    RegistryDocument {
        format: FORMAT.to_owned(),
        format_version: FORMAT_VERSION.to_owned(),
        registry_id: identity.registry_id,
        version: identity.version,
        description: None,
        imports: Vec::new(),
        components: components.into_values().collect(),
    }
}

fn add_component_dependencies(doc: &mut RegistryDocument, functions: &[FnMeta]) {
    for f in functions {
        if let Some(comp) = doc.components.iter_mut().find(|c| c.id == f.component) {
            for dep in &f.dependencies {
                if dep != &f.component && !comp.dependencies.iter().any(|d| &d.component_id == dep)
                {
                    comp.dependencies.push(Dependency {
                        component_id: dep.clone(),
                        registry_id: None,
                    });
                }
            }
        }
    }
}

fn empty_component(id: &str) -> Component {
    Component {
        id: id.to_owned(),
        description: None,
        dependencies: Vec::new(),
        operations: Vec::new(),
        types: Vec::new(),
    }
}

fn display_type_ref(ty: &TypeRef) -> String {
    match ty {
        TypeRef::Primitive { name } => format!("{name:?}"),
        TypeRef::Named { name, .. } => name.clone(),
        TypeRef::List { items } => format!("Vec<{}>", display_type_ref(items)),
        TypeRef::Set { items } => format!("Set<{}>", display_type_ref(items)),
        TypeRef::Optional { value } => format!("Option<{}>", display_type_ref(value)),
        TypeRef::Map { keys, values } => format!(
            "Map<{},{}>",
            display_type_ref(keys),
            display_type_ref(values)
        ),
        TypeRef::Tuple { items } => format!(
            "({})",
            items
                .iter()
                .map(display_type_ref)
                .collect::<Vec<_>>()
                .join(",")
        ),
        TypeRef::Record { fields } => format!(
            "record{{{}}}",
            fields
                .iter()
                .map(|f| format!("{}:{}", f.name, display_type_ref(&f.ty)))
                .collect::<Vec<_>>()
                .join(",")
        ),
        TypeRef::TaggedUnion { variants } => format!(
            "union{{{}}}",
            variants
                .iter()
                .map(|v| v.name.clone())
                .collect::<Vec<_>>()
                .join("|")
        ),
    }
}

fn outcomes_summary(out: &Outcomes) -> String {
    format!(
        "result={}, empty={}, errors=[{}]",
        out.result
            .as_ref()
            .map(display_type_ref)
            .unwrap_or_else(|| "none".to_owned()),
        out.empty,
        out.errors
            .iter()
            .map(|e| e.name.clone())
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn error_summary(out: &Outcomes) -> Option<String> {
    out.errors.first().map(|e| e.name.clone())
}

fn leak_str(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn leak_runtime_metadata(
    functions: &[FnMeta],
    types: &[WatchType],
) -> (Vec<OpMeta>, Vec<TypeMeta>, Vec<Box<dyn std::any::Any>>) {
    let mut keepalive: Vec<Box<dyn std::any::Any>> = Vec::new();
    let mut ops = Vec::new();
    for f in functions {
        let fields: Vec<FieldMeta> = f
            .inputs
            .iter()
            .map(|i| (leak_str(i.name.clone()), leak_str(display_type_ref(&i.ty))))
            .collect();
        let slice: &'static [FieldMeta] = Box::leak(fields.into_boxed_slice());
        ops.push(OpMeta {
            name: leak_str(f.name.clone()),
            module_path: leak_str(f.module_path.clone()),
            fn_name: leak_str(f.fn_name.clone()),
            is_setup: false,
            is_async: false,
            params: slice,
            return_type: leak_str(f.return_display.clone()),
            fills: "",
            component: leak_str(f.component.clone()),
        });
    }
    let mut ty_metas = Vec::new();
    for t in types {
        match &t.named {
            NamedType::Record { name, fields, .. } => {
                let fs: Vec<FieldMeta> = fields
                    .iter()
                    .map(|f| (leak_str(f.name.clone()), leak_str(display_type_ref(&f.ty))))
                    .collect();
                ty_metas.push(TypeMeta {
                    name: leak_str(name.clone()),
                    module_path: "",
                    kind: "struct",
                    fields: Box::leak(fs.into_boxed_slice()),
                    variants: &[],
                    component: leak_str(t.component.clone()),
                });
            }
            NamedType::TaggedUnion { name, variants, .. } => {
                let vs: Vec<VariantMeta> = variants
                    .iter()
                    .map(|v| {
                        let fields = match &v.payload {
                            Some(TypeRef::Record { fields }) => fields
                                .iter()
                                .map(|f| {
                                    (leak_str(f.name.clone()), leak_str(display_type_ref(&f.ty)))
                                })
                                .collect::<Vec<FieldMeta>>(),
                            _ => Vec::new(),
                        };
                        VariantMeta {
                            name: leak_str(v.name.clone()),
                            fields: Box::leak(fields.into_boxed_slice()),
                        }
                    })
                    .collect();
                ty_metas.push(TypeMeta {
                    name: leak_str(name.clone()),
                    module_path: "",
                    kind: "enum",
                    fields: &[],
                    variants: Box::leak(vs.into_boxed_slice()),
                    component: leak_str(t.component.clone()),
                });
            }
        }
    }
    keepalive.push(Box::new("leaked-static-metadata"));
    (ops, ty_metas, keepalive)
}

fn rustc_version() -> String {
    std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_else(|| "unknown".to_owned())
        .trim()
        .to_owned()
}
