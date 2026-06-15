//! Token surgery: remove the `=> field [N]` annotations from a grammar section
//! so the cleaned stream can be handed to the runtime `parse_*!` macros, which
//! don't understand those annotations.

use proc_macro2::{Delimiter, Group, Spacing, TokenStream as TS2, TokenTree as TT};

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
            quote! { memchr(b'*') { 1 => italics [40] } },
            quote! { memchr(b'*') { 1 => italics } },
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
}
