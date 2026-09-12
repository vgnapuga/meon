//! Canonicalise the stripped `inline { ... }` section before it is handed to
//! the runtime `parse_text!` / `parse_inline!` macros.
//!
//! The front-end ([`crate::collect`]) reads the grammar by keyword: sub-rules
//! and settings may come in any order, with or without a trailing `;` or `,`.
//! The runtime macros are `macro_rules!` patterns and accept exactly one token
//! shape: sub-rules inside `on_trigger { ... }` in the order `symmetric`,
//! `asymmetric`, `chained`, `key_value`; `parse_inside` before `balanced` in a
//! `symmetric` body and after it in an `asymmetric` body; the `prefix` last in
//! a `chained` body; `eq`, `allow_sep`, `end`, `key`, `value` in that order in
//! a `key_value` body; every setting terminated by `;` and every arm by `,`.
//!
//! This pass rewrites what the front-end accepted into that one shape, so the
//! two never disagree: whatever `define_parser!` accepts, the runtime macros
//! accept. Reordering is semantically neutral — the runtime buckets sub-rules
//! by kind and dispatches the kinds in a fixed order, and the relative order
//! *within* a kind is preserved.
//!
//! It runs on the stripped stream (no `[N]` capacities) and only after the
//! front-end has validated the grammar, so every keyword and setting can be
//! assumed present. Anything unrecognised is passed through untouched, after
//! the recognised statements of its block.

use proc_macro2::{
    Delimiter, Group, Ident, Punct, Spacing, Span, TokenStream as TS2, TokenTree as TT,
};

/// Canonicalise an `inline { ... }` section (already stripped of `[N]`).
pub(crate) fn canonicalize_inline(ts: TS2) -> TS2 {
    let tokens: Vec<TT> = ts.into_iter().collect();
    let (lead, units) = split_by_keyword(&tokens, |id| {
        matches!(
            id.to_string().as_str(),
            "merge_simple" | "hard_break" | "fallback" | "on_trigger"
        )
    });
    let mut out = TS2::new();
    out.extend(lead);
    for (kw, unit) in units {
        match kw.as_str() {
            "on_trigger" => {
                // on_trigger ( bytes ) { body }
                for tt in unit {
                    match tt {
                        TT::Group(g) if g.delimiter() == Delimiter::Brace => {
                            out.extend([regroup(&g, canonicalize_on_trigger(g.stream()))]);
                        }
                        other => out.extend([other]),
                    }
                }
            }
            // merge_simple = flag ;   hard_break(..) => field ;   fallback => field ;
            _ => {
                out.extend(unit);
                out.extend([semi()]);
            }
        }
    }
    out
}

/// Canonicalise the body of one `on_trigger(..) { ... }` block: sub-rules
/// sorted by kind, each body canonicalised, no stray separators.
fn canonicalize_on_trigger(ts: TS2) -> TS2 {
    let tokens: Vec<TT> = ts.into_iter().collect();
    let (lead, units) = split_by_keyword(&tokens, |id| {
        matches!(
            id.to_string().as_str(),
            "symmetric" | "asymmetric" | "chained" | "key_value"
        )
    });
    let rank = |kw: &str| match kw {
        "symmetric" => 0,
        "asymmetric" => 1,
        "chained" => 2,
        _ => 3,
    };
    let mut units: Vec<(String, Vec<TT>)> = units;
    // Stable: relative order within one kind is preserved.
    units.sort_by_key(|(kw, _)| rank(kw));

    let mut out = TS2::new();
    out.extend(lead);
    for (kw, unit) in units {
        let body: fn(TS2) -> TS2 = match kw.as_str() {
            "symmetric" => |b| canonicalize_flag_body(b, &["parse_inside", "balanced"]),
            "asymmetric" => |b| canonicalize_flag_body(b, &["balanced", "parse_inside"]),
            "chained" => canonicalize_chained_body,
            _ => canonicalize_kv_body,
        };
        // The first brace group of the unit is the rule body; everything
        // else (`byte`, `open, close`, `: Type`, `=> field`) is kept as is.
        let mut done = false;
        for tt in unit {
            match tt {
                TT::Group(g) if !done && g.delimiter() == Delimiter::Brace => {
                    out.extend([regroup(&g, body(g.stream()))]);
                    done = true;
                }
                other => out.extend([other]),
            }
        }
    }
    out
}

/// Canonicalise a `symmetric` / `asymmetric` body (or a `chained` component's
/// settings block): the named flags first, in `order`, each as
/// `name = value ;`, then every `N => field` arm terminated by `,`, then
/// anything unrecognised.
fn canonicalize_flag_body(ts: TS2, order: &[&str]) -> TS2 {
    let tokens: Vec<TT> = ts.into_iter().collect();
    let mut flags: Vec<(Ident, TT)> = Vec::new();
    let mut arms: Vec<(TT, TT)> = Vec::new();
    let mut rest: Vec<TT> = Vec::new();

    let mut i = 0;
    while i < tokens.len() {
        if let TT::Ident(name) = &tokens[i] {
            if is_alone(&tokens, i + 1, '=') {
                if let Some(value) = tokens.get(i + 2) {
                    flags.push((name.clone(), value.clone()));
                    i += 3;
                    if is_alone(&tokens, i, ';') {
                        i += 1;
                    }
                    continue;
                }
            }
        }
        if is_fat_arrow(&tokens, i + 1) {
            if let Some(field) = tokens.get(i + 3) {
                arms.push((tokens[i].clone(), field.clone()));
                i += 4;
                if is_alone(&tokens, i, ',') || is_alone(&tokens, i, ';') {
                    i += 1;
                }
                continue;
            }
        }
        if is_alone(&tokens, i, ',') || is_alone(&tokens, i, ';') {
            i += 1;
            continue;
        }
        rest.push(tokens[i].clone());
        i += 1;
    }

    let mut out = TS2::new();
    for &want in order {
        if let Some(pos) = flags.iter().position(|(n, _)| n == want) {
            let (name, value) = flags.remove(pos);
            out.extend([TT::Ident(name), eq_alone(), value, semi()]);
        }
    }
    // Flags the caller did not list (none in practice) keep their place.
    for (name, value) in flags {
        out.extend([TT::Ident(name), eq_alone(), value, semi()]);
    }
    for (count, field) in arms {
        out.extend([count, fat_arrow_eq(), fat_arrow_gt(), field, comma()]);
    }
    out.extend(rest);
    out
}

/// Canonicalise a `chained` body: the two `| open, close | { settings } =>
/// field` components in declaration order, then the `prefix | byte | =>
/// field`, each terminated by `,`, each settings block canonicalised.
fn canonicalize_chained_body(ts: TS2) -> TS2 {
    let tokens: Vec<TT> = ts.into_iter().collect();
    let mut components: Vec<Vec<TT>> = Vec::new();
    let mut prefix: Vec<Vec<TT>> = Vec::new();
    let mut rest: Vec<TT> = Vec::new();

    let mut i = 0;
    while i < tokens.len() {
        if is_alone(&tokens, i, '|') {
            // | open , close | { settings } => field
            let start = i;
            let mut j = i;
            while j < tokens.len()
                && !matches!(&tokens[j], TT::Group(g) if g.delimiter() == Delimiter::Brace)
            {
                j += 1;
            }
            if j < tokens.len() && is_fat_arrow(&tokens, j + 1) && tokens.get(j + 3).is_some() {
                let mut unit: Vec<TT> = tokens[start..j].to_vec();
                if let TT::Group(g) = &tokens[j] {
                    unit.push(regroup(
                        g,
                        canonicalize_flag_body(g.stream(), &["parse_inside", "balanced"]),
                    ));
                }
                unit.extend(tokens[j + 1..j + 4].iter().cloned());
                components.push(unit);
                i = j + 4;
                if is_alone(&tokens, i, ',') {
                    i += 1;
                }
                continue;
            }
        }
        if let TT::Ident(id) = &tokens[i] {
            // prefix | byte | => field
            if id == "prefix"
                && is_alone(&tokens, i + 1, '|')
                && is_alone(&tokens, i + 3, '|')
                && is_fat_arrow(&tokens, i + 4)
                && tokens.get(i + 6).is_some()
            {
                prefix.push(tokens[i..i + 7].to_vec());
                i += 7;
                if is_alone(&tokens, i, ',') {
                    i += 1;
                }
                continue;
            }
        }
        if is_alone(&tokens, i, ',') || is_alone(&tokens, i, ';') {
            i += 1;
            continue;
        }
        rest.push(tokens[i].clone());
        i += 1;
    }

    let mut out = TS2::new();
    for unit in components.into_iter().chain(prefix) {
        out.extend(unit);
        out.extend([comma()]);
    }
    out.extend(rest);
    out
}

/// Canonicalise a `key_value` body: `eq = ..;`, `allow_sep = ..;`,
/// `end = ..;`, `key => ..,`, `value => ..,` in that order.
fn canonicalize_kv_body(ts: TS2) -> TS2 {
    let tokens: Vec<TT> = ts.into_iter().collect();
    let (lead, mut units) = split_by_keyword(&tokens, |id| {
        matches!(
            id.to_string().as_str(),
            "eq" | "allow_sep" | "end" | "key" | "value"
        )
    });
    let rank = |kw: &str| match kw {
        "eq" => 0,
        "allow_sep" => 1,
        "end" => 2,
        "key" => 3,
        _ => 4,
    };
    units.sort_by_key(|(kw, _)| rank(kw));

    let mut out = TS2::new();
    out.extend(lead);
    for (kw, unit) in units {
        let terminator = match kw.as_str() {
            "key" | "value" => comma(),
            _ => semi(),
        };
        out.extend(unit);
        out.extend([terminator]);
    }
    out
}

/// Split a flat token list into statements, each starting at a keyword
/// identifier (per `is_kw`) and running up to the next keyword. Trailing `,`
/// and `;` separators are trimmed from every statement. Tokens before the
/// first keyword are returned separately, untouched.
fn split_by_keyword(
    tokens: &[TT],
    is_kw: impl Fn(&Ident) -> bool,
) -> (Vec<TT>, Vec<(String, Vec<TT>)>) {
    // A keyword identifier starts a statement unless it sits in value
    // position — right after `=>` or `=` — as in `key => key` or
    // `value => value`, where the second word is a field name.
    let starts: Vec<usize> = tokens
        .iter()
        .enumerate()
        .filter_map(|(i, tt)| match tt {
            TT::Ident(id) if is_kw(id) && !in_value_position(tokens, i) => Some(i),
            _ => None,
        })
        .collect();

    let lead = match starts.first() {
        Some(&first) => tokens[..first].to_vec(),
        None => tokens.to_vec(),
    };

    let mut units = Vec::new();
    for (n, &start) in starts.iter().enumerate() {
        let end = starts.get(n + 1).copied().unwrap_or(tokens.len());
        let mut unit: Vec<TT> = tokens[start..end].to_vec();
        while unit.len() > 1
            && matches!(unit.last(), Some(TT::Punct(p)) if p.as_char() == ',' || p.as_char() == ';')
        {
            unit.pop();
        }
        let kw = match &tokens[start] {
            TT::Ident(id) => id.to_string(),
            _ => unreachable!("statement starts are identifiers"),
        };
        units.push((kw, unit));
    }
    (lead, units)
}

/// Is the token at `i` the right-hand side of a `=>` or `=`?
fn in_value_position(tokens: &[TT], i: usize) -> bool {
    i > 0
        && matches!(
            &tokens[i - 1],
            TT::Punct(p) if (p.as_char() == '>' && i > 1 && is_fat_arrow(tokens, i - 2))
                || (p.as_char() == '=' && p.spacing() == Spacing::Alone)
        )
}

// ---- token helpers ----------------------------------------------------- //

fn regroup(orig: &Group, inner: TS2) -> TT {
    let mut g = Group::new(orig.delimiter(), inner);
    g.set_span(orig.span());
    TT::Group(g)
}

fn is_alone(tokens: &[TT], i: usize, ch: char) -> bool {
    matches!(tokens.get(i), Some(TT::Punct(p)) if p.as_char() == ch && p.spacing() == Spacing::Alone)
}

fn is_fat_arrow(tokens: &[TT], i: usize) -> bool {
    matches!(
        (tokens.get(i), tokens.get(i + 1)),
        (Some(TT::Punct(a)), Some(TT::Punct(b)))
            if a.as_char() == '=' && a.spacing() == Spacing::Joint && b.as_char() == '>'
    )
}

fn semi() -> TT {
    TT::Punct(Punct::new(';', Spacing::Alone))
}

fn comma() -> TT {
    TT::Punct(Punct::new(',', Spacing::Alone))
}

fn eq_alone() -> TT {
    TT::Punct(Punct::new('=', Spacing::Alone))
}

fn fat_arrow_eq() -> TT {
    TT::Punct(Punct::new('=', Spacing::Joint))
}

fn fat_arrow_gt() -> TT {
    let mut p = Punct::new('>', Spacing::Alone);
    p.set_span(Span::call_site());
    TT::Punct(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    fn eq(input: TS2, expected: TS2) {
        assert_eq!(canonicalize_inline(input).to_string(), expected.to_string());
    }

    // 01. Already-canonical input is returned unchanged
    #[test]
    fn test_01_canonical_unchanged() {
        let g = quote! {
            merge_simple = true ;
            hard_break(b'\\', b' ', 2) => hard_breaks ;
            on_trigger(b'*', b'<') {
                symmetric b'*' { parse_inside = true ; balanced = false ; 1 => italics , 2 => bolds , }
                asymmetric b'<', b'>' { balanced = false ; parse_inside = false ; 1 => autolinks , }
            }
            fallback => texts ;
        };
        eq(g.clone(), g);
    }

    // 02. Sub-rules are sorted into symmetric, asymmetric, chained, key_value
    #[test]
    fn test_02_sub_rules_sorted_by_kind() {
        eq(
            quote! {
                on_trigger(b'*', b'<', b'[', b'=') {
                    key_value: Pair { eq = b'=' ; allow_sep = true ; end = b'\n' ; key => key , value => value , } => pairs
                    chained: Link {
                        | b'[', b']' | { parse_inside = false ; balanced = false ; } => text ,
                        | b'(', b')' | { parse_inside = false ; balanced = false ; } => url ,
                        prefix | b'!' | => is_image ,
                    } => links
                    asymmetric b'<', b'>' { balanced = false ; parse_inside = false ; 1 => autolinks , }
                    symmetric b'*' { parse_inside = true ; balanced = false ; 1 => italics , }
                }
            },
            quote! {
                on_trigger(b'*', b'<', b'[', b'=') {
                    symmetric b'*' { parse_inside = true ; balanced = false ; 1 => italics , }
                    asymmetric b'<', b'>' { balanced = false ; parse_inside = false ; 1 => autolinks , }
                    chained: Link {
                        | b'[', b']' | { parse_inside = false ; balanced = false ; } => text ,
                        | b'(', b')' | { parse_inside = false ; balanced = false ; } => url ,
                        prefix | b'!' | => is_image ,
                    } => links
                    key_value: Pair { eq = b'=' ; allow_sep = true ; end = b'\n' ; key => key , value => value , } => pairs
                }
            },
        );
    }

    // 03. Relative order within one kind is preserved
    #[test]
    fn test_03_same_kind_order_preserved() {
        eq(
            quote! {
                on_trigger(b'`', b'*') {
                    symmetric b'`' { parse_inside = false ; balanced = false ; 1 => codes , }
                    symmetric b'*' { parse_inside = true ; balanced = true ; 1 => italics , }
                }
            },
            quote! {
                on_trigger(b'`', b'*') {
                    symmetric b'`' { parse_inside = false ; balanced = false ; 1 => codes , }
                    symmetric b'*' { parse_inside = true ; balanced = true ; 1 => italics , }
                }
            },
        );
    }

    // 04. Flag order: symmetric wants parse_inside first, asymmetric balanced first
    #[test]
    fn test_04_flag_order_per_kind() {
        eq(
            quote! {
                on_trigger(b'*', b'<') {
                    symmetric b'*' { balanced = true ; parse_inside = true ; 1 => italics , }
                    asymmetric b'<', b'>' { parse_inside = false ; balanced = false ; 1 => autolinks , }
                }
            },
            quote! {
                on_trigger(b'*', b'<') {
                    symmetric b'*' { parse_inside = true ; balanced = true ; 1 => italics , }
                    asymmetric b'<', b'>' { balanced = false ; parse_inside = false ; 1 => autolinks , }
                }
            },
        );
    }

    // 05. Arms may come before flags and use `;` or no separator; they are
    //     emitted after the flags, each terminated by `,`
    #[test]
    fn test_05_arms_after_flags_with_commas() {
        eq(
            quote! {
                on_trigger(b'*') {
                    symmetric b'*' { 1 => italics ; 2 => bolds parse_inside = true ; balanced = false }
                }
            },
            quote! {
                on_trigger(b'*') {
                    symmetric b'*' { parse_inside = true ; balanced = false ; 1 => italics , 2 => bolds , }
                }
            },
        );
    }

    // 06. Top-level statements get their `;`, in place, in declaration order
    #[test]
    fn test_06_top_level_terminators() {
        eq(
            quote! {
                merge_simple = true
                fallback => texts
                hard_break(b'\\', b' ', 2) => hard_breaks
            },
            quote! {
                merge_simple = true ;
                fallback => texts ;
                hard_break(b'\\', b' ', 2) => hard_breaks ;
            },
        );
    }

    // 07. chained: prefix declared first moves last, missing commas are added,
    //     component settings are reordered
    #[test]
    fn test_07_chained_prefix_last_and_commas() {
        eq(
            quote! {
                on_trigger(b'[') {
                    chained: Link {
                        prefix | b'!' | => is_image
                        | b'[', b']' | { balanced = false ; parse_inside = false } => text
                        | b'(', b')' | { parse_inside = false ; balanced = false ; } => url
                    } => links
                }
            },
            quote! {
                on_trigger(b'[') {
                    chained: Link {
                        | b'[', b']' | { parse_inside = false ; balanced = false ; } => text ,
                        | b'(', b')' | { parse_inside = false ; balanced = false ; } => url ,
                        prefix | b'!' | => is_image ,
                    } => links
                }
            },
        );
    }

    // 08. key_value: statements reordered and terminated
    #[test]
    fn test_08_kv_statement_order() {
        eq(
            quote! {
                on_trigger(b'=') {
                    key_value: Pair { value => value ; key => key ; end = b'\n' , allow_sep = true , eq = b'=' } => pairs
                }
            },
            quote! {
                on_trigger(b'=') {
                    key_value: Pair { eq = b'=' ; allow_sep = true ; end = b'\n' ; key => key , value => value , } => pairs
                }
            },
        );
    }

    // 09. Stray separators between sub-rules are dropped
    #[test]
    fn test_09_stray_separators_between_sub_rules() {
        eq(
            quote! {
                on_trigger(b'*', b'<') {
                    asymmetric b'<', b'>' { balanced = false ; parse_inside = false ; 1 => autolinks , } ;
                    symmetric b'*' { parse_inside = true ; balanced = false ; 1 => italics , } ,
                }
            },
            quote! {
                on_trigger(b'*', b'<') {
                    symmetric b'*' { parse_inside = true ; balanced = false ; 1 => italics , }
                    asymmetric b'<', b'>' { balanced = false ; parse_inside = false ; 1 => autolinks , }
                }
            },
        );
    }

    // 10. Idempotent: canonicalising twice equals once
    #[test]
    fn test_10_idempotent() {
        let g = quote! {
            fallback => texts
            on_trigger(b'*', b'<') {
                asymmetric b'<', b'>' { parse_inside = false ; balanced = false ; 1 => autolinks }
                symmetric b'*' { balanced = false ; parse_inside = true ; 1 => italics }
            }
        };
        let once = canonicalize_inline(g);
        let twice = canonicalize_inline(once.clone());
        assert_eq!(once.to_string(), twice.to_string());
    }

    // 11. A `_ => field` catch-all arm is an arm like any other
    #[test]
    fn test_11_underscore_arm() {
        eq(
            quote! {
                on_trigger(b'*') {
                    symmetric b'*' { parse_inside = true ; balanced = false ; 1 => italics , _ => rest }
                }
            },
            quote! {
                on_trigger(b'*') {
                    symmetric b'*' { parse_inside = true ; balanced = false ; 1 => italics , _ => rest , }
                }
            },
        );
    }

    // 12. Multiple on_trigger blocks are each canonicalised independently
    #[test]
    fn test_12_multiple_on_trigger_blocks() {
        eq(
            quote! {
                on_trigger(b'=') {
                    key_value: Pair { end = b'\n' ; eq = b'=' ; allow_sep = true ; key => key , value => value } => pairs
                }
                on_trigger(b'*') {
                    symmetric b'*' { balanced = false ; parse_inside = true ; 1 => italics }
                }
            },
            quote! {
                on_trigger(b'=') {
                    key_value: Pair { eq = b'=' ; allow_sep = true ; end = b'\n' ; key => key , value => value , } => pairs
                }
                on_trigger(b'*') {
                    symmetric b'*' { parse_inside = true ; balanced = false ; 1 => italics , }
                }
            },
        );
    }
}
