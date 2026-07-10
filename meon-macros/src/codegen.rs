//! Back-end: emit the `define_content!` invocation and the standalone DSL that
//! drives `define_standalone_fns!`.

use proc_macro2::{Ident, Literal, TokenStream as TS2};
use quote::quote;

use crate::model::{CF, StandaloneRule};

/// Emit the `<mc>::define_content!(Name { ... })` invocation describing every
/// field (with its type and capacity divisor) collected from the grammar.
///
/// `mc` is the resolved crate path (`crate` when expanded inside `meon` itself,
/// the imported crate name otherwise), so the call is always fully qualified and
/// works from any dependent crate.
pub(crate) fn build_define_content(name: &Ident, cf: &CF, mc: &TS2) -> TS2 {
    let il = cf
        .inline
        .iter()
        .map(|(f, ty, cap)| quote! { #f: #ty [ #cap ], });
    let is = cf
        .inline_simple
        .iter()
        .map(|(f, cap)| quote! { #f [ #cap ], });
    let ln = cf
        .line
        .iter()
        .map(|(f, ty, cap)| quote! { #f: #ty [ #cap ], });
    let bl = cf
        .block
        .iter()
        .map(|(f, ty, cap)| quote! { #f: #ty [ #cap ], });
    let bs = cf
        .block_simple
        .iter()
        .map(|(f, cap)| quote! { #f [ #cap ], });
    quote! {
        #mc::define_content!(#name {
            inline        { #(#il)* }
            inline_simple { #(#is)* }
            line          { #(#ln)* }
            block         { #(#bl)* }
            block_simple  { #(#bs)* }
        });
    }
}

/// Emit the rule list consumed by `define_standalone_fns!`, prefixed with the
/// `sep`/`eol`/`tab`/`escape` context.
pub(crate) fn build_standalone_dsl(
    sep: &Literal,
    eol: &Literal,
    tab: &Literal,
    esc: &Literal,
    rules: &[StandaloneRule],
) -> TS2 {
    // The opaque-region context spec: every fence rule plus every
    // `parse_inside = false` symmetric/asymmetric rule. Consumed by
    // `define_standalone_fns!` to emit `context()`.
    let mut ctx_fences: Vec<TS2> = Vec::new();
    let mut ctx_sym: Vec<TS2> = Vec::new();
    let mut ctx_asym: Vec<TS2> = Vec::new();
    for rule in rules {
        match rule {
            StandaloneRule::Fence { byte, min, .. } => {
                ctx_fences.push(quote! { (#byte, #min) });
            }
            StandaloneRule::SymmetricExact {
                byte,
                count,
                opaque: true,
                ..
            } => {
                ctx_sym.push(quote! { (#byte, #count) });
            }
            StandaloneRule::AsymmetricExact {
                open,
                close,
                count,
                opaque: true,
                ..
            } => {
                ctx_asym.push(quote! { (#open, #close, #count) });
            }
            _ => {}
        }
    }
    let ctx_hdr = quote! {
        context { fences [ #(#ctx_fences),* ] sym [ #(#ctx_sym),* ] asym [ #(#ctx_asym),* ] }
    };

    let opacity = |opaque: bool| -> TS2 {
        if opaque {
            quote!(opaque)
        } else {
            quote!(transparent)
        }
    };

    let items: Vec<TS2> = rules.iter().map(|rule| match rule {
        StandaloneRule::SymmetricExact { field, byte, count, opaque } => {
            let o = opacity(*opaque);
            quote! { symmetric_exact(#byte, #count, #o) => #field; }
        }
        StandaloneRule::AsymmetricExact { field, open, close, count, opaque } => {
            let o = opacity(*opaque);
            quote! { asymmetric_exact(#open, #close, #count, #o) => #field; }
        }
        StandaloneRule::Chained { field, open1, close1, open2, close2, prefix, ty, pf, ff, sf } =>
            quote! { chained(#open1, #close1, #open2, #close2, #prefix, #ty, #pf, #ff, #sf) => #field; },
        StandaloneRule::KeyValue { field, eq, end, allow_sep, ty, kf, vf } => {
            let allow = if *allow_sep { quote!(true) } else { quote!(false) };
            quote! { kv(#eq, #end, #allow, #ty, #kf, #vf) => #field; }
        }
        StandaloneRule::LineMarker { field, byte, max, ty, var, body } =>
            quote! { line_marker(#byte, #max, #ty, #var) { #body } => #field; },
        StandaloneRule::LineUniform { field, bytes, min, ty, var, body } =>
            quote! { line_uniform([#(#bytes),*], #min, #ty, #var) { #body } => #field; },
        StandaloneRule::Fence { field, byte, min } =>
            quote! { fence(#byte, #min) => #field; },
        StandaloneRule::Cont { field, byte } =>
            quote! { cont(#byte) => #field; },
        StandaloneRule::BlockMarker { field, bytes, ty, var, body } =>
            quote! { block_marker([#(#bytes),*], #ty, #var) { #body } => #field; },
        StandaloneRule::BlockNumbered { field, end_bytes, ty, num_var, kind_var, body } =>
            quote! { block_numbered([#(#end_bytes),*], #ty, #num_var, #kind_var) { #body } => #field; },
    }).collect();
    quote! { sep=#sep, eol=#eol, tab=#tab, escape=#esc; #ctx_hdr #(#items)* }
}
