//! The `#[watch_dep("name")]` `let`-binding rewrite.
//!
//! Applied to a `let` inside a `#[watch_operation]` body, this is purely
//! observational. Mirroring `#[watch_operation]`'s parameter handling, each
//! argument of the dependency call is emitted as its own `name.<arg>` event —
//! named by the argument identifier (a leading `&`/`&mut` is peeled), or
//! positionally as `name.arg{i}` when the argument is not a bare identifier. It
//! then evaluates the ORIGINAL right-hand side (the real dependency call) and
//! emits `name.response` on the `Ok` path or `name.error` on the `Err` path.
//! Unlike the `SpecGate` `#[spec_mock]` it adapts from, there is no table
//! lookup, no substitution, and no `Default::default()` short-circuit — the real
//! call always runs and its `Result` is bound unchanged to the original pattern.
//!
//! The initializer must be a direct function or method call, optionally with a
//! single trailing `?`: `let x = dep.call(a)?;` is supported and the `?` is
//! preserved verbatim after the observation, so `From`-conversion and error
//! propagation are unchanged (the `Result` is observed by borrow, then the real
//! binding/`?` is applied). Other shapes — `.await`, chained combinators like
//! `dep.get(k).unwrap()`, and non-call initializers — are unsupported and
//! produce a `compile_error!` rather than a silent no-op.
//!
//! The real call is kept verbatim so its type inference (e.g. a literal
//! argument's expected integer type) is preserved; each argument expression is
//! consequently emitted by reference and then re-evaluated in the call, so
//! arguments are expected to be side-effect-free — the usual case (identifiers,
//! literals, field accesses).

use syn::{Block, Expr, Local, Stmt, parse_quote};

use crate::shared::{NameArg, rt};

/// Diagnostic emitted when `#[watch_dep]` is attached to an unsupported
/// initializer shape.
const UNSUPPORTED_SHAPE: &str = "#[watch_dep] expects a direct function or method call as the initializer (an optional trailing `?` is allowed); `.await`, chained calls, and other initializer shapes are not supported";

/// If `attrs` carry a `#[watch_dep("name")]`, return the name.
pub fn take_dep_name(attrs: &[syn::Attribute]) -> Option<String> {
    for a in attrs {
        if a.path().is_ident("watch_dep")
            && let Ok(NameArg(name)) = a.parse_args::<NameArg>()
        {
            return Some(name);
        }
    }
    None
}

/// Expand a `#[watch_dep("name")]` `let` into one input event per call argument
/// plus response/error emission around the real dependency call. An optional
/// single trailing `?` on the initializer is supported and preserved. Returns
/// `None` when the binding has no initializer or the initializer (after peeling
/// one `?`) is not a direct function or method call (leaving the `let`
/// untouched).
pub fn expand_dep_let(local: &Local, dep_name: &str) -> Option<Vec<Stmt>> {
    let init = local.init.as_ref()?;

    // Peel an optional single trailing `?` so `let x = dep.call(a)?;` is
    // instrumented; the `?` is re-applied verbatim after observation.
    let (inner, is_try): (&Expr, bool) = match &*init.expr {
        Expr::Try(t) => (&t.expr, true),
        other => (other, false),
    };

    let inputs = dep_inputs(inner)?;
    let rt = rt();
    let pat = &local.pat;
    let response_name = format!("{dep_name}.response");
    let error_name = format!("{dep_name}.error");

    let mut stmts: Vec<Stmt> = Vec::with_capacity(inputs.len() + 1);
    for (name, expr) in &inputs {
        let event_name = format!("{dep_name}.{name}");
        stmts.push(parse_quote! {
            #rt::emit_event_v(#event_name, #rt::ToValue::to_value(&(#expr)));
        });
    }

    // Observe the real `Result` by borrow, then apply the original binding and
    // (if present) `?` so `From`-conversion and propagation stay verbatim.
    let maybe_q: Option<syn::Token![?]> = is_try.then(|| parse_quote!(?));
    let call: Block = parse_quote!({
        let __dw_res = (#inner);
        match &__dw_res {
            ::core::result::Result::Ok(__dw_v) => {
                #rt::emit_event_v(#response_name, #rt::ToValue::to_value(__dw_v));
            }
            ::core::result::Result::Err(__dw_e) => {
                #rt::emit_event_v(
                    #error_name,
                    #rt::Value::String(::std::format!("{}", __dw_e)),
                );
            }
        };
        let #pat = __dw_res #maybe_q;
    });
    stmts.extend(call.stmts);
    Some(stmts)
}

/// Each argument of the dependency call paired with the event-name suffix it
/// emits under. Returns `None` when the initializer is not a direct function or
/// method call.
fn dep_inputs(e: &Expr) -> Option<Vec<(String, &Expr)>> {
    let args = match e {
        Expr::MethodCall(mc) => &mc.args,
        Expr::Call(c) => &c.args,
        _ => return None,
    };
    Some(
        args.iter()
            .enumerate()
            .map(|(i, a)| (arg_name(i, a), a))
            .collect(),
    )
}

/// The event-name suffix for the `i`th argument: its identifier when it is a
/// bare path (peeling a leading `&`/`&mut`), otherwise the positional `arg{i}`.
fn arg_name(i: usize, a: &Expr) -> String {
    match a {
        Expr::Reference(r) => arg_name(i, &r.expr),
        Expr::Path(p) => match p.path.get_ident() {
            Some(id) => id.to_string(),
            None => format!("arg{i}"),
        },
        _ => format!("arg{i}"),
    }
}

/// Body-instrumentation hook: rewrite a `#[watch_dep]`-tagged `let`. On the
/// happy path returns the expanded statements. When the initializer shape is
/// unsupported, strips the inert helper attribute and prepends a
/// `compile_error!` so the diagnostic is clear instead of an unstable
/// statement-attribute error. Untagged `let`s pass through unchanged.
pub fn rewrite_local(local: Local) -> Vec<Stmt> {
    let Some(name) = take_dep_name(&local.attrs) else {
        return vec![Stmt::Local(local)];
    };
    if let Some(stmts) = expand_dep_let(&local, &name) {
        return stmts;
    }
    let mut stripped = local;
    stripped.attrs.retain(|a| !a.path().is_ident("watch_dep"));
    let msg = syn::LitStr::new(UNSUPPORTED_SHAPE, proc_macro2::Span::call_site());
    let err: Stmt = parse_quote! { ::core::compile_error!(#msg); };
    vec![err, Stmt::Local(stripped)]
}
