//! The `#[watch_dep("name")]` `let`-binding rewrite.
//!
//! Applied to a `let` inside a `#[watch_operation]` body; purely observational.
//! For `let x = dep.call(a, &b)?;` it expands to:
//!
//! - one `name.<arg>` input event per argument (by identifier, `&`/`&mut`
//!   peeled; else positional `name.arg{i}`);
//! - the original call, run verbatim and bound to a temp;
//! - `name.response` (Ok) or `name.error` (Err `Display`), observed by borrow;
//! - the original pattern and any trailing `?`, re-applied unchanged.
//!
//! The real call always runs and its `Result` binds unchanged: no table, no
//! substitution. The `?` is observed before it unwraps, so an `Err` is recorded
//! then propagated. Arguments are re-evaluated in the call, so they must be
//! side-effect-free (identifiers, literals, field accesses).
//!
//! Only a direct function or method call (optionally one trailing `?`) is
//! supported; `.await`, chained combinators, and non-call initializers produce a
//! `compile_error!`.

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

/// Expand a `#[watch_dep("name")]` `let` into per-argument input events plus
/// response/error emission around the real call (trailing `?` preserved).
/// `None` when there is no initializer, or it is not a call after peeling `?`.
pub fn expand_dep_let(local: &Local, dep_name: &str) -> Option<Vec<Stmt>> {
    let init = local.init.as_ref()?;

    // Peel a trailing `?`; re-applied verbatim after observation.
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

    // Observe the `Result` by borrow, then apply the original binding and `?`.
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

/// Each call argument paired with its event-name suffix. `None` when the
/// initializer is not a direct function or method call.
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

/// Rewrite a `#[watch_dep]`-tagged `let`: expanded statements on success;
/// unsupported shapes strip the attribute and prepend a `compile_error!`.
/// Untagged `let`s pass through unchanged.
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
