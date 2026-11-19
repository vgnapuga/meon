//! Token surgery: unwrap the invisible groups a grammar written inside a
//! `macro_rules!` macro arrives with, and remove the `=> field [N]`
//! annotations from a grammar section so the cleaned stream can be handed to
//! the runtime `parse_*!` macros, which don't understand those annotations.

use proc_macro2::{Delimiter, Group, Spacing, TokenStream as TS2, TokenTree as TT};

/// Splice the tokens of every invisible (`Delimiter::None`) group into the
/// stream around it, at any depth, except inside a rule's metadata body
/// (`|vars|: Type { ... }`).
///
/// A grammar written inside a `macro_rules!` macro receives its `$x:literal`,
/// `$x:pat` and similar fragments wrapped in such groups; unwrapped, they read
/// exactly like the tokens they stand for. A metadata body is Rust code, where
/// a wrapped `$x:expr` must keep its precedence; the runtime macros read it
/// as an `expr` fragment, which does not see invisible groups, so there a
/// group of several tokens becomes a parenthesised one (see
/// [`parenthesize_invisible`]).
pub(crate) fn flatten_invisible(ts: TS2) -> TS2 {
    let mut out = TS2::new();
    // A `|` right before, and a `|:` seen with its body still ahead.
    let mut after_bar = false;
    let mut meta_ahead = false;
    for tt in ts {
        match tt {
            TT::Group(g) if g.delimiter() == Delimiter::None => {
                out.extend(flatten_invisible(g.stream()));
                after_bar = false;
            }
            TT::Group(g) if meta_ahead && g.delimiter() == Delimiter::Brace => {
                let mut ng = Group::new(Delimiter::Brace, parenthesize_invisible(g.stream()));
                ng.set_span(g.span());
                out.extend([TT::Group(ng)]);
                meta_ahead = false;
                after_bar = false;
            }
            TT::Group(g) => {
                let mut ng = Group::new(g.delimiter(), flatten_invisible(g.stream()));
                ng.set_span(g.span());
                out.extend([TT::Group(ng)]);
                after_bar = false;
            }
            TT::Punct(p) => {
                if p.as_char() == ':' && after_bar {
                    meta_ahead = true;
                }
                after_bar = p.as_char() == '|';
                out.extend([TT::Punct(p)]);
            }
            other => {
                after_bar = false;
                out.extend([other]);
            }
        }
    }
    out
}

/// Turn every invisible group of several tokens into a parenthesised group,
/// and splice a one-token one, at any depth.
///
/// An `expr` fragment the runtime macros match does not see invisible
/// groups, so `$x * 2` with `$x` standing for `1 + 1` would read as
/// `1 + 1 * 2`; parenthesised it reads as written. One token has no
/// precedence to keep.
fn parenthesize_invisible(ts: TS2) -> TS2 {
    let mut out = TS2::new();
    for tt in ts {
        match tt {
            TT::Group(g) => {
                let inner = parenthesize_invisible(g.stream());
                if g.delimiter() == Delimiter::None && inner.clone().into_iter().count() <= 1 {
                    out.extend(inner);
                } else {
                    let delim = match g.delimiter() {
                        Delimiter::None => Delimiter::Parenthesis,
                        d => d,
                    };
                    let mut ng = Group::new(delim, inner);
                    ng.set_span(g.span());
                    out.extend([TT::Group(ng)]);
                }
            }
            other => out.extend([other]),
        }
    }
    out
}

/// Recursively strip every `=> field [N]` sequence from `ts`, descending into
/// groups. Everything else is preserved verbatim.
pub(crate) fn strip(ts: TS2) -> TS2 {
    let tokens: Vec<TT> = ts.into_iter().collect();
    let mut out = TS2::new();
    let mut i = 0;
    while i < tokens.len() {
        if is_fat_arrow_at(&tokens, i) {
            out.extend([tokens[i].clone(), tokens[i + 1].clone()]);
            i += 2;
            if i < tokens.len() {
                if let TT::Ident(_) = &tokens[i] {
                    out.extend([tokens[i].clone()]);
                    i += 1;
                    if i < tokens.len() {
                        if let TT::Group(g) = &tokens[i] {
                            if g.delimiter() == Delimiter::Bracket {
                                i += 1;
                            }
                        }
                    }
                }
            }
            continue;
        }
        match &tokens[i] {
            TT::Group(g) => {
                let inner = strip(g.stream());
                let mut ng = Group::new(g.delimiter(), inner);
                ng.set_span(g.span());
                out.extend([TT::Group(ng)]);
            }
            tt => out.extend([tt.clone()]),
        }
        i += 1;
    }
    out
}

/// True if tokens `[i]` and `[i + 1]` form a joint `=>`.
fn is_fat_arrow_at(tokens: &[TT], i: usize) -> bool {
    if i + 1 >= tokens.len() {
        return false;
    }
    match (&tokens[i], &tokens[i + 1]) {
        (TT::Punct(a), TT::Punct(b)) => {
            a.as_char() == '=' && a.spacing() == Spacing::Joint && b.as_char() == '>'
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    fn eq(input: TS2, expected: TS2) {
        assert_eq!(strip(input).to_string(), expected.to_string());
    }

    // 01. A single arm loses its `[N]` capacity but keeps `=> field`
    #[test]
    fn test_01_drops_capacity_keeps_arrow() {
        eq(quote! { 1 => italics [40] }, quote! { 1 => italics });
    }

    // 02. A fallback arm loses `[N]` while the trailing `;` is preserved
    #[test]
    fn test_02_fallback_keeps_semicolon() {
        eq(
            quote! { fallback => texts [10] ; },
            quote! { fallback => texts ; },
        );
    }

    // 03. Capacities nested inside a group are stripped recursively
    #[test]
    fn test_03_nested_in_group() {
        eq(
            quote! { on_trigger(b'*') { 1 => italics [40] } },
            quote! { on_trigger(b'*') { 1 => italics } },
        );
    }

    // 04. Several arms each lose their own capacity
    #[test]
    fn test_04_multiple_arms() {
        eq(
            quote! { 1 => italics [40] , 2 => bolds [40] },
            quote! { 1 => italics , 2 => bolds },
        );
    }

    // 05. A stream with no arrows is returned unchanged
    #[test]
    fn test_05_no_arrow_unchanged() {
        eq(
            quote! { parse_inside = false ; },
            quote! { parse_inside = false ; },
        );
    }

    // 06. `=> field` with no following capacity is already in final form
    #[test]
    fn test_06_arrow_without_cap_idempotent() {
        eq(quote! { 1 => italics }, quote! { 1 => italics });
    }

    // 07. Two levels of nesting are both stripped
    #[test]
    fn test_07_deeply_nested() {
        eq(
            quote! { outer { inner { 1 => codes [80] } } },
            quote! { outer { inner { 1 => codes } } },
        );
    }

    // 08. Only a bracket group after `=> field` is dropped; other groups stay
    #[test]
    fn test_08_only_bracket_after_field_dropped() {
        eq(quote! { 1 => italics (x) }, quote! { 1 => italics (x) });
    }

    // 09. A bracket group that is not preceded by `=> field` is preserved
    #[test]
    fn test_09_unrelated_bracket_preserved() {
        eq(quote! { foo [40] }, quote! { foo [40] });
    }

    // 10. strip is idempotent: stripping twice equals stripping once
    #[test]
    fn test_10_idempotent() {
        let once = strip(quote! { 1 => italics [40] });
        let twice = strip(once.clone());
        assert_eq!(once.to_string(), twice.to_string());
    }

    fn invisible(ts: TS2) -> TT {
        TT::Group(Group::new(Delimiter::None, ts))
    }

    // 11. Invisible groups are unwrapped at any depth outside a metadata
    //     body: a forwarded literal and pattern read as written
    #[test]
    fn test_11_flatten_unwraps_invisible_groups() {
        let mut ts = quote! { max_nest = };
        ts.extend([invisible(quote! { 4 })]);
        let mut args = quote! {};
        args.extend([invisible(quote! { b'~' | b'=' })]);
        args.extend(quote! { , min = 3 });
        ts.extend(quote! { ; fence });
        ts.extend([TT::Group(Group::new(Delimiter::Parenthesis, args))]);
        assert_eq!(
            flatten_invisible(ts).to_string(),
            quote! { max_nest = 4; fence(b'~' | b'=', min = 3) }.to_string()
        );
    }

    // 12. In a metadata body an invisible group of several tokens becomes a
    //     parenthesised one, keeping its precedence; one token is unwrapped
    #[test]
    fn test_12_flatten_parenthesizes_in_metadata_body() {
        let mut body = quote! { kind: };
        body.extend([invisible(quote! { 1 + 1 })]);
        body.extend(quote! { * 2, other: });
        body.extend([invisible(quote! { b })]);
        let mut ts = quote! { (b'-') |b|: Item };
        ts.extend([TT::Group(Group::new(Delimiter::Brace, body))]);
        assert_eq!(
            flatten_invisible(ts).to_string(),
            quote! { (b'-') |b|: Item { kind: (1 + 1) * 2, other: b } }.to_string()
        );
    }
}
