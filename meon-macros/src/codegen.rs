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
    max_nest: &Literal,
    rules: &[StandaloneRule],
) -> TS2 {
    // The context spec: the capacity divisors of every fence and opaque
    // rule, the `parse_inside = false` symmetric/asymmetric rules, the
    // `chained` rules with both components `parse_inside = false`, and the
    // grammar's line rules, each section in declaration order. Consumed by
    // `define_standalone_fns!` to emit `context()`.
    let mut ctx_caps: Vec<TS2> = Vec::new();
    let mut ctx_sym: Vec<TS2> = Vec::new();
    let mut ctx_asym: Vec<TS2> = Vec::new();
    let mut ctx_chained: Vec<TS2> = Vec::new();
    let mut ctx_block_simple: Vec<TS2> = Vec::new();
    let mut ctx_block: Vec<TS2> = Vec::new();
    let mut ctx_lines: Vec<TS2> = Vec::new();
    for rule in rules {
        match rule {
            StandaloneRule::Fence { byte, min, cap, .. } => {
                ctx_caps.push(quote! { #cap });
                ctx_block_simple.push(quote! { fence(#byte, #min) });
            }
            StandaloneRule::Cont { byte, .. } => {
                ctx_block_simple.push(quote! { cont(#byte) });
            }
            StandaloneRule::SymmetricExact {
                byte,
                count,
                opaque: true,
                cap,
                ..
            } => {
                ctx_caps.push(quote! { #cap });
                ctx_sym.push(quote! { (#byte, #count) });
            }
            StandaloneRule::AsymmetricExact {
                open,
                close,
                count,
                opaque: true,
                balanced,
                cap,
                ..
            } => {
                ctx_caps.push(quote! { #cap });
                ctx_asym.push(quote! { (#open, #close, #count, #balanced) });
            }
            StandaloneRule::Chained {
                open1,
                close1,
                open2,
                close2,
                text_flags,
                url_flags,
                cap,
                ..
            } if !text_flags.parse_inside && !url_flags.parse_inside => {
                let (tb, ub) = (text_flags.balanced, url_flags.balanced);
                ctx_caps.push(quote! { #cap });
                ctx_chained.push(quote! { (#open1, #close1, #tb, #open2, #close2, #ub) });
            }
            StandaloneRule::BlockMarker { bytes, .. } => {
                ctx_block.push(quote! { marker(#(#bytes),*) });
            }
            StandaloneRule::BlockNumbered { end_bytes, .. } => {
                ctx_block.push(quote! { num(#(#end_bytes),*) });
            }
            StandaloneRule::LineMarker { byte, max, .. } => {
                ctx_lines.push(quote! { marker(#byte, #max) });
            }
            StandaloneRule::LineUniform { bytes, min, .. } => {
                ctx_lines.push(quote! { uniform([#(#bytes),*], #min) });
            }
            _ => {}
        }
    }
    let ctx_hdr = quote! {
        context {
            caps [ #(#ctx_caps),* ]
            sym [ #(#ctx_sym),* ]
            asym [ #(#ctx_asym),* ]
            chained [ #(#ctx_chained),* ]
            block_simple [ #(#ctx_block_simple),* ]
            block [ #(#ctx_block),* ]
            lines [ #(#ctx_lines),* ]
        }
    };

    let opacity = |opaque: bool| -> TS2 {
        if opaque {
            quote!(opaque)
        } else {
            quote!(transparent)
        }
    };

    // Line-family finders carry their rule's index within its section, the
    // index `LineModel` reports for the rule that claims a line.
    let (mut block_rule, mut line_rule) = (0usize, 0usize);
    let next = |n: &mut usize| {
        let i = Literal::usize_unsuffixed(*n);
        *n += 1;
        i
    };
    let items: Vec<TS2> = rules.iter().map(|rule| match rule {
        StandaloneRule::SymmetricExact { field, byte, count, opaque, .. } => {
            let o = opacity(*opaque);
            quote! { symmetric_exact(#byte, #count, #o) => #field; }
        }
        StandaloneRule::AsymmetricExact { field, open, close, count, opaque, balanced, .. } => {
            let o = opacity(*opaque);
            if *balanced || !*opaque {
                quote! { asymmetric_stack(#open, #close, #balanced, #o) => #field; }
            } else {
                quote! { asymmetric_exact(#open, #close, #count, #o) => #field; }
            }
        }
        StandaloneRule::Chained {
            field, open1, close1, open2, close2, prefix, ty, pf, ff, sf, text_flags, url_flags, ..
        } => {
            let (tb, ub) = (text_flags.balanced, url_flags.balanced);
            let tr = text_flags.parse_inside || url_flags.parse_inside;
            quote! {
                chained(#open1, #close1, #tb, #open2, #close2, #ub, #tr, #prefix, #ty, #pf, #ff, #sf)
                    => #field;
            }
        }
        StandaloneRule::KeyValue { field, eq, end, allow_sep, ty, kf, vf } => {
            let allow = if *allow_sep { quote!(true) } else { quote!(false) };
            quote! { kv(#eq, #end, #allow, #ty, #kf, #vf) => #field; }
        }
        StandaloneRule::LineMarker { field, byte, max, ty, var, body } => {
            let i = next(&mut line_rule);
            quote! { line_marker(#byte, #max, #ty, #var, #i) { #body } => #field; }
        }
        StandaloneRule::LineUniform { field, bytes, min, ty, var, body } => {
            let i = next(&mut line_rule);
            quote! { line_uniform([#(#bytes),*], #min, #ty, #var, #i) { #body } => #field; }
        }
        StandaloneRule::Fence { field, byte, min, .. } =>
            quote! { fence(#byte, #min) => #field; },
        StandaloneRule::Cont { field, byte } =>
            quote! { cont(#byte) => #field; },
        StandaloneRule::BlockMarker { field, bytes, ty, var, body } => {
            let i = next(&mut block_rule);
            quote! { block_marker([#(#bytes),*], #ty, #var, #i) { #body } => #field; }
        }
        StandaloneRule::BlockNumbered { field, end_bytes, ty, num_var, kind_var, body } => {
            let i = next(&mut block_rule);
            quote! { block_numbered([#(#end_bytes),*], #ty, #num_var, #kind_var, #i) { #body } => #field; }
        }
    }).collect();
    quote! { sep=#sep, eol=#eol, tab=#tab, escape=#esc, max_nest=#max_nest; #ctx_hdr #(#items)* }
}
