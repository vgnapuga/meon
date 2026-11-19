//! Front-end: walk the grammar token sections (`inline`, `lines`, `blocks`) and
//! fill a [`CF`] with field declarations plus a list of [`StandaloneRule`]s.
//!
//! # Entry points
//!
//! - [`collect_inline`] — processes the `inline { ... }` section.
//! - [`collect_lines`]  — processes the `lines { ... }` section.
//! - [`collect_blocks`] — processes the `blocks { ... }` section.
//!
//! All three return [`Result`] so a malformed grammar surfaces as a located
//! `compile_error!` rather than a panic.
//!
//! # Single authority on grammar shape
//!
//! This front-end is the only parser of the DSL that reports errors. The
//! runtime `parse_*!` macros pattern-match the same tokens again, but they
//! only ever see a stream that passed here and was then canonicalised by
//! [`crate::normalize`]. Three rules follow from that:
//!
//! - **Unknown keywords are errors.** An identifier in statement position that
//!   this module does not recognise (a typo, a keyword from the wrong section)
//!   is reported here, located, with the list of what was expected.
//! - **Everything the runtime needs is required.** A `symmetric` /
//!   `asymmetric` body and every `chained` component must state both
//!   `parse_inside` and `balanced`; a `chained` rule needs its two components
//!   and its `prefix`; a `key_value` rule needs `eq`, `allow_sep`, `end`,
//!   `key` and `value`. A missing one is a located error, not a silently
//!   skipped standalone rule.
//! - **No arm that can never match.** An `asymmetric` rule with `balanced`
//!   or `parse_inside` set opens one frame per open byte, so its only arm is
//!   `1 => field`; any other arm, a second one included, is a located error.
//!
//! Order and separators, on the other hand, are *not* enforced here: settings
//! and sub-rules may come in any order, with or without `;` / `,`, and the
//! canonicalisation pass rewrites them into the shape the runtime expects.

use proc_macro2::{Delimiter, Ident, Literal, Span as PS, TokenStream as TS2, TokenTree as TT};

use crate::cursor::Cursor;
use crate::error::{Error, Result};
use crate::model::{CF, ChainedFlags, StandaloneRule};

/// Walk the `inline { ... }` section.
pub(crate) fn collect_inline(ts: TS2, cf: &mut CF) -> Result<()> {
    let mut c = Cursor::new(ts);
    while let Some(tt) = c.peek().cloned() {
        if let TT::Ident(id) = tt {
            match id.to_string().as_str() {
                "merge_simple" => {
                    c.advance();
                    c.skip_eq_alone();
                    cf.merge_simple = Some(c.next_ident("merge_simple value")?);
                    c.skip(';');
                }
                "hard_break" => {
                    c.advance();
                    c.next_group(Delimiter::Parenthesis, "hard_break args")?;
                    let (f, cap) = c.arrow_field_cap("hard_break")?;
                    c.skip(';');
                    cf.block_simple.push((f, cap));
                }
                "fallback" => {
                    c.advance();
                    let (f, cap) = c.arrow_field_cap("inline fallback")?;
                    c.skip(';');
                    cf.inline_fallback = Some(f.clone());
                    cf.inline_simple.push((f, cap));
                }
                "on_trigger" => {
                    c.advance();
                    c.next_group(Delimiter::Parenthesis, "on_trigger bytes")?;
                    let body = c.next_group(Delimiter::Brace, "on_trigger body")?;
                    collect_on_trigger(body.stream(), cf)?;
                }
                _ => {
                    return Err(unknown_keyword(
                        &id,
                        "inline",
                        "`merge_simple`, `hard_break`, `on_trigger` or `fallback`",
                    ));
                }
            }
        } else {
            c.advance();
        }
    }
    Ok(())
}

/// Located error for an identifier in statement position that the section
/// does not recognise.
fn unknown_keyword(id: &Ident, section: &str, expected: &str) -> Error {
    Error::new(
        id.span(),
        format!("unknown keyword `{id}` in `{section}`; expected {expected}"),
    )
}

/// Read a required `name = true|false;` flag from a rule body, or error at
/// `span` naming the rule kind.
fn require_flag(ts: TS2, name: &str, span: PS, ctx: &str) -> Result<bool> {
    flag_value(ts, name).ok_or_else(|| {
        Error::new(
            span,
            format!("`{ctx}` body is missing `{name} = true|false;`"),
        )
    })
}

/// Walk an `on_trigger(..) { ... }` body.
///
/// Contains `symmetric`, `asymmetric`, `chained` and `key_value` sub-rules.
fn collect_on_trigger(ts: TS2, cf: &mut CF) -> Result<()> {
    let mut c = Cursor::new(ts);
    while let Some(tt) = c.peek().cloned() {
        if let TT::Ident(id) = tt {
            match id.to_string().as_str() {
                "symmetric" => {
                    c.advance();
                    let byte_lit = c.expect_lit("symmetric byte")?;
                    let body = c.next_group(Delimiter::Brace, "symmetric body")?;
                    if let Some(span) = setting_span(body.stream(), "balanced") {
                        return Err(Error::new(
                            span,
                            "`symmetric` takes no `balanced`: it opens and closes with one \
                             byte, so a run never nests in a run of its own count",
                        ));
                    }
                    collect_match_arms(body.stream(), &["parse_inside"], cf)?;
                    let parse_inside =
                        require_flag(body.stream(), "parse_inside", body.span(), "symmetric")?;
                    collect_symmetric_standalone(body.stream(), byte_lit, !parse_inside, cf)?;
                }
                "asymmetric" => {
                    c.advance();
                    let open_lit = c.expect_lit("asymmetric open")?;
                    c.skip(',');
                    let close_lit = c.expect_lit("asymmetric close")?;
                    let body = c.next_group(Delimiter::Brace, "asymmetric body")?;
                    collect_match_arms(body.stream(), &["balanced", "parse_inside"], cf)?;
                    let parse_inside =
                        require_flag(body.stream(), "parse_inside", body.span(), "asymmetric")?;
                    let balanced =
                        require_flag(body.stream(), "balanced", body.span(), "asymmetric")?;
                    collect_asymmetric_standalone(
                        body.stream(),
                        open_lit,
                        close_lit,
                        !parse_inside,
                        balanced,
                        cf,
                    )?;
                }
                "chained" => {
                    c.advance();
                    c.skip_colon();
                    let ty = c.collect_until_brace();
                    let inner = c.next_group(Delimiter::Brace, "chained inner")?;
                    let (f, cap) = c.arrow_field_cap("chained")?;
                    cf.inline.push((f.clone(), ty.clone(), cap.clone()));
                    collect_chained_standalone(inner.stream(), ty, f, cap, cf)?;
                }
                "key_value" => {
                    c.advance();
                    c.skip_colon();
                    let ty = c.collect_until_brace();
                    let inner = c.next_group(Delimiter::Brace, "key_value inner")?;
                    let (f, cap) = c.arrow_field_cap("key_value")?;
                    cf.inline.push((f.clone(), ty.clone(), cap));
                    collect_kv_standalone(inner.stream(), ty, f, cf)?;
                }
                _ => {
                    return Err(unknown_keyword(
                        &id,
                        "on_trigger",
                        "`symmetric`, `asymmetric`, `chained` or `key_value`",
                    ));
                }
            }
        } else {
            c.advance();
        }
    }
    Ok(())
}

/// Read the boolean value of a `name = true|false;` flag inside a rule body.
/// Returns `None` when the flag is absent or its value is not `true` / `false`.
/// The span of a `name = ...` setting in a rule body, if the body has one.
fn setting_span(ts: TS2, name: &str) -> Option<PS> {
    ts.into_iter().find_map(|tt| match tt {
        TT::Ident(id) if id == name => Some(id.span()),
        _ => None,
    })
}

fn flag_value(ts: TS2, name: &str) -> Option<bool> {
    let mut c = Cursor::new(ts);
    while let Some(tt) = c.peek().cloned() {
        if let TT::Ident(id) = &tt {
            if id == name {
                c.advance();
                c.skip_eq_alone();
                return match c.peek() {
                    Some(TT::Ident(v)) if v == "true" => Some(true),
                    Some(TT::Ident(v)) if v == "false" => Some(false),
                    _ => None,
                };
            }
        }
        c.advance();
    }
    None
}

/// Extract the `N => field` exact-count arm of a `symmetric` block, if present.
fn collect_symmetric_standalone(ts: TS2, byte: Literal, opaque: bool, cf: &mut CF) -> Result<()> {
    let mut c = Cursor::new(ts);
    while c.peek().is_some() {
        match c.peek().cloned() {
            Some(TT::Ident(id))
                if matches!(id.to_string().as_str(), "parse_inside" | "balanced") =>
            {
                c.advance();
                c.skip_eq_alone();
                c.advance();
                c.skip(';');
            }
            Some(TT::Ident(id)) if id == "_" => {
                c.advance();
                if c.is_fat_arrow() {
                    c.pos += 2;
                    c.next_ident("wc field")?;
                    let _ = c.next_tt();
                }
                c.skip(',');
            }
            Some(TT::Literal(count)) => {
                c.advance();
                if c.is_fat_arrow() {
                    let (f, cap) = c.arrow_field_cap("sym exact")?;
                    c.skip(',');
                    cf.standalone.push(StandaloneRule::SymmetricExact {
                        field: f,
                        byte: byte.clone(),
                        count,
                        opaque,
                        cap,
                    });
                }
            }
            Some(TT::Punct(p)) if p.as_char() == ',' || p.as_char() == ';' => {
                c.advance();
            }
            _ => {
                c.advance();
            }
        }
    }
    Ok(())
}

/// Extract the `N => field` exact-count arms of an `asymmetric` block.
///
/// A rule with `balanced` or `parse_inside` set reads each opening byte
/// separately, in a depth-counting forward search or on the bounded stack.
/// Only a `1 =>` arm can match, and any other arm is an error.
fn collect_asymmetric_standalone(
    ts: TS2,
    open: Literal,
    close: Literal,
    opaque: bool,
    balanced: bool,
    cf: &mut CF,
) -> Result<()> {
    let stack = balanced || !opaque;
    let mut arms = 0usize;
    let one_arm = |span| {
        Error::new(
            span,
            "an `asymmetric` rule with `balanced = true` or `parse_inside = true` opens one \
             frame per open byte, so its only arm is `1 => field`",
        )
    };
    let mut c = Cursor::new(ts);
    while c.peek().is_some() {
        match c.peek().cloned() {
            Some(TT::Ident(id))
                if matches!(id.to_string().as_str(), "balanced" | "parse_inside") =>
            {
                c.advance();
                c.skip_eq_alone();
                c.advance();
                c.skip(';');
            }
            Some(TT::Ident(id)) if id == "_" => {
                if stack {
                    return Err(one_arm(id.span()));
                }
                c.advance();
                if c.is_fat_arrow() {
                    c.pos += 2;
                    c.next_ident("wc field")?;
                    let _ = c.next_tt();
                }
                c.skip(',');
            }
            Some(TT::Literal(count)) => {
                c.advance();
                if c.is_fat_arrow() {
                    arms += 1;
                    if stack && (arms > 1 || int_value(&count) != Some(1)) {
                        return Err(one_arm(count.span()));
                    }
                    let (f, cap) = c.arrow_field_cap("asym exact")?;
                    c.skip(',');
                    cf.standalone.push(StandaloneRule::AsymmetricExact {
                        field: f,
                        open: open.clone(),
                        close: close.clone(),
                        count,
                        opaque,
                        balanced,
                        cap,
                    });
                }
            }
            Some(TT::Punct(p)) if p.as_char() == ',' || p.as_char() == ';' => {
                c.advance();
            }
            _ => {
                c.advance();
            }
        }
    }
    Ok(())
}

/// The value of an integer literal token, as the compiler reads it: `1`,
/// `1u32`, `1_u32`, `0x1`, `0o1`, `0b1`. `None` for any other literal.
fn int_value(lit: &Literal) -> Option<u128> {
    let s = lit.to_string().replace('_', "");
    let (digits, radix) = match s.get(..2) {
        Some("0x") => (&s[2..], 16),
        Some("0o") => (&s[2..], 8),
        Some("0b") => (&s[2..], 2),
        _ => (s.as_str(), 10),
    };
    let end = digits
        .find(|ch: char| !ch.is_digit(radix))
        .unwrap_or(digits.len());
    let (num, suffix) = digits.split_at(end);
    let suffixes = [
        "", "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
    ];
    if num.is_empty() || !suffixes.contains(&suffix) {
        return None;
    }
    u128::from_str_radix(num, radix).ok()
}

/// Extract the two delimiter pairs, their settings and the prefix of a
/// `chained` block.
fn collect_chained_standalone(
    ts: TS2,
    ty: TS2,
    outer_field: Ident,
    cap: Literal,
    cf: &mut CF,
) -> Result<()> {
    let mut c = Cursor::new(ts);
    let mut open1: Option<Literal> = None;
    let mut close1: Option<Literal> = None;
    let mut open2: Option<Literal> = None;
    let mut close2: Option<Literal> = None;
    let mut flags1: Option<ChainedFlags> = None;
    let mut flags2: Option<ChainedFlags> = None;
    let mut prefix_lit: Option<Literal> = None;
    let mut ff: Option<Ident> = None;
    let mut sf: Option<Ident> = None;
    let mut pf: Option<Ident> = None;

    while c.peek().is_some() {
        match c.peek().cloned() {
            Some(TT::Punct(p)) if p.as_char() == '|' => {
                c.advance();
                let o = c.expect_lit("chained open")?;
                c.skip(',');
                let cl = c.expect_lit("chained close")?;
                if let Some(TT::Punct(p)) = c.peek() {
                    if p.as_char() == '|' {
                        c.advance();
                    }
                }
                let settings = c.next_group(Delimiter::Brace, "chained settings")?;
                let flags = ChainedFlags {
                    parse_inside: require_flag(
                        settings.stream(),
                        "parse_inside",
                        settings.span(),
                        "chained component",
                    )?,
                    balanced: require_flag(
                        settings.stream(),
                        "balanced",
                        settings.span(),
                        "chained component",
                    )?,
                };
                if c.is_fat_arrow() {
                    c.pos += 2;
                    let fi = c.next_ident("chained field")?;
                    c.skip(',');
                    if open1.is_none() {
                        open1 = Some(o);
                        close1 = Some(cl);
                        flags1 = Some(flags);
                        ff = Some(fi);
                    } else {
                        open2 = Some(o);
                        close2 = Some(cl);
                        flags2 = Some(flags);
                        sf = Some(fi);
                    }
                }
            }
            Some(TT::Ident(id)) if id == "prefix" => {
                c.advance();
                if let Some(TT::Punct(p)) = c.peek() {
                    if p.as_char() == '|' {
                        c.advance();
                        let pl = c.expect_lit("prefix byte")?;
                        prefix_lit = Some(pl);
                        if let Some(TT::Punct(p)) = c.peek() {
                            if p.as_char() == '|' {
                                c.advance();
                            }
                        }
                        if c.is_fat_arrow() {
                            c.pos += 2;
                            let pi = c.next_ident("prefix field")?;
                            c.skip(',');
                            pf = Some(pi);
                        }
                    }
                }
            }
            _ => {
                c.advance();
            }
        }
    }

    match (
        (open1, close1, flags1),
        (open2, close2, flags2),
        prefix_lit,
        pf,
        ff,
        sf,
    ) {
        (
            (Some(o1), Some(c1), Some(f1)),
            (Some(o2), Some(c2), Some(f2)),
            Some(p),
            Some(pfi),
            Some(ffi),
            Some(sfi),
        ) => {
            cf.standalone.push(StandaloneRule::Chained {
                field: outer_field,
                open1: o1,
                close1: c1,
                open2: o2,
                close2: c2,
                prefix: p,
                ty,
                pf: pfi,
                ff: ffi,
                sf: sfi,
                first_flags: f1,
                second_flags: f2,
                cap,
            });
            Ok(())
        }
        _ => Err(Error::new(
            outer_field.span(),
            "`chained` rule needs two `| open, close | { ... } => field` components \
             and a `prefix | byte | => field`",
        )),
    }
}

/// Extract the settings of a `key_value` block.
fn collect_kv_standalone(ts: TS2, ty: TS2, outer_field: Ident, cf: &mut CF) -> Result<()> {
    let mut c = Cursor::new(ts);
    let mut eq_lit: Option<Literal> = None;
    let mut end_lit: Option<Literal> = None;
    let mut allow_sep: Option<bool> = None;
    let mut kf: Option<Ident> = None;
    let mut vf: Option<Ident> = None;

    while c.peek().is_some() {
        match c.peek().cloned() {
            Some(TT::Ident(id)) => match id.to_string().as_str() {
                "eq" => {
                    c.advance();
                    c.skip_eq_alone();
                    eq_lit = c.next_lit();
                    c.skip(';');
                }
                "allow_sep" => {
                    c.advance();
                    c.skip_eq_alone();
                    allow_sep = match c.next_tt() {
                        Some(TT::Ident(v)) if v == "true" => Some(true),
                        Some(TT::Ident(v)) if v == "false" => Some(false),
                        _ => None,
                    };
                    c.skip(';');
                }
                "end" => {
                    c.advance();
                    c.skip_eq_alone();
                    end_lit = c.next_lit();
                    c.skip(';');
                }
                "key" => {
                    c.advance();
                    if c.is_fat_arrow() {
                        c.pos += 2;
                        kf = Some(c.next_ident("kv key")?);
                    }
                    c.skip(',');
                }
                "value" => {
                    c.advance();
                    if c.is_fat_arrow() {
                        c.pos += 2;
                        vf = Some(c.next_ident("kv value")?);
                    }
                    c.skip(',');
                }
                _ => {
                    c.advance();
                }
            },
            _ => {
                c.advance();
            }
        }
    }

    match (eq_lit, allow_sep, end_lit, kf, vf) {
        (Some(eq), Some(allow_sep), Some(end), Some(k), Some(v)) => {
            cf.standalone.push(StandaloneRule::KeyValue {
                field: outer_field,
                eq,
                end,
                allow_sep,
                ty,
                kf: k,
                vf: v,
            });
            Ok(())
        }
        _ => Err(Error::new(
            outer_field.span(),
            "`key_value` rule needs `eq = byte;`, `allow_sep = true|false;`, `end = byte;`, \
             `key => field,` and `value => field,`",
        )),
    }
}

/// Collect the `... => field [N]` arms of a symmetric/asymmetric block.
///
/// Named settings listed in `skip_settings` are consumed and discarded.
fn collect_match_arms(ts: TS2, skip_settings: &[&str], cf: &mut CF) -> Result<()> {
    let mut c = Cursor::new(ts);
    while c.peek().is_some() {
        match c.peek().cloned() {
            Some(TT::Ident(id)) if skip_settings.iter().any(|&s| id == s) => {
                c.advance();
                c.skip_eq_alone();
                c.advance();
                c.skip(';');
            }
            Some(TT::Literal(_)) | Some(TT::Ident(_)) => {
                c.advance();
                if c.is_fat_arrow() {
                    let (f, cap) = c.arrow_field_cap("match arm")?;
                    c.skip(',');
                    cf.inline_simple.push((f, cap));
                }
            }
            Some(TT::Punct(p)) if p.as_char() == ',' || p.as_char() == ';' => {
                c.advance();
            }
            _ => {
                c.advance();
            }
        }
    }
    Ok(())
}

/// Walk the `lines { ... }` section (`line` / `line_simple`).
pub(crate) fn collect_lines(ts: TS2, cf: &mut CF) -> Result<()> {
    let mut c = Cursor::new(ts);
    while let Some(tt) = c.peek().cloned() {
        if let TT::Ident(id) = tt {
            let is_simple = match id.to_string().as_str() {
                "line" => false,
                "line_simple" => true,
                _ => {
                    return Err(unknown_keyword(&id, "lines", "`line` or `line_simple`"));
                }
            };
            c.advance();
            let args = c.next_group(Delimiter::Parenthesis, "line args")?;
            let vars = c.skip_pipe_vars_returning();
            c.skip_colon();
            let ty = c.collect_until_brace();
            let body_group = c.next_group(Delimiter::Brace, "line type body")?;
            let (f, cap) = c.arrow_field_cap("line field")?;
            c.skip(';');
            cf.line.push((f.clone(), ty.clone(), cap));

            let var = vars
                .into_iter()
                .next()
                .unwrap_or_else(|| Ident::new("_v", PS::call_site()));
            let body = body_group.stream();
            if is_simple {
                let (pat, min) = parse_min_args(args.stream(), args.span(), "line_simple")?;
                cf.standalone.push(StandaloneRule::LineUniform {
                    field: f,
                    pat,
                    min,
                    ty,
                    var,
                    body,
                });
            } else {
                let (byte, max) = parse_line_marker_args(args.stream())?;
                cf.standalone.push(StandaloneRule::LineMarker {
                    field: f,
                    byte,
                    max,
                    ty,
                    var,
                    body,
                });
            }
        } else {
            c.advance();
        }
    }
    Ok(())
}

/// Walk the `blocks { ... }` section (`block_simple` / `block` / `fallback`).
pub(crate) fn collect_blocks(ts: TS2, cf: &mut CF) -> Result<()> {
    let mut c = Cursor::new(ts);
    while c.peek().is_some() {
        match c.peek().cloned() {
            Some(TT::Ident(id)) => match id.to_string().as_str() {
                "block_simple" => {
                    c.advance();
                    let body = c.next_group(Delimiter::Brace, "block_simple body")?;
                    collect_block_simple(body.stream(), cf)?;
                }
                "block" => {
                    c.advance();
                    let body = c.next_group(Delimiter::Brace, "block body")?;
                    collect_block(body.stream(), cf)?;
                }
                "fallback" => {
                    c.advance();
                    let (f, cap) = c.arrow_field_cap("blocks fallback")?;
                    c.skip(';');
                    cf.block_simple.push((f, cap));
                }
                _ => {
                    return Err(unknown_keyword(
                        &id,
                        "blocks",
                        "`block_simple`, `block` or `fallback`",
                    ));
                }
            },
            _ => {
                c.advance();
            }
        }
    }
    Ok(())
}

/// Walk a `block_simple { ... }` body (`fence` / `cont`).
fn collect_block_simple(ts: TS2, cf: &mut CF) -> Result<()> {
    let mut c = Cursor::new(ts);
    while let Some(tt) = c.peek().cloned() {
        if let TT::Ident(id) = tt {
            let kind = id.to_string();
            if matches!(kind.as_str(), "fence" | "cont") {
                c.advance();
                let args = c.next_group(Delimiter::Parenthesis, "fence/cont args")?;
                let (f, cap) = c.arrow_field_cap("block_simple field")?;
                c.skip(';');
                cf.block_simple.push((f.clone(), cap.clone()));
                if kind == "fence" {
                    let (pat, min) = parse_min_args(args.stream(), args.span(), "fence")?;
                    cf.standalone.push(StandaloneRule::Fence {
                        field: f,
                        pat,
                        min,
                        cap,
                    });
                } else {
                    let byte = parse_cont_args(args.stream())?;
                    cf.standalone.push(StandaloneRule::Cont { field: f, byte });
                }
            } else {
                return Err(unknown_keyword(&id, "block_simple", "`fence` or `cont`"));
            }
        } else {
            c.advance();
        }
    }
    Ok(())
}

/// Walk a `block { ... }` body (marker `(...)` blocks and `num(...)` blocks).
fn collect_block(ts: TS2, cf: &mut CF) -> Result<()> {
    let mut c = Cursor::new(ts);
    while c.peek().is_some() {
        match c.peek().cloned() {
            Some(TT::Group(g)) if g.delimiter() == Delimiter::Parenthesis => {
                c.advance();
                let pat_stream = g.stream();
                let vars = c.skip_pipe_vars_returning();
                c.skip_colon();
                let ty = c.collect_until_brace();
                let body_group = c.next_group(Delimiter::Brace, "block type body")?;
                let (f, cap) = c.arrow_field_cap("block field")?;
                c.skip(';');
                cf.block.push((f.clone(), ty.clone(), cap));
                let pat = Cursor::new(pat_stream).collect_pattern("block marker")?;
                let var = vars
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| Ident::new("_b", PS::call_site()));
                cf.standalone.push(StandaloneRule::BlockMarker {
                    field: f,
                    pat,
                    ty,
                    var,
                    body: body_group.stream(),
                });
            }
            Some(TT::Ident(id)) if id == "num" => {
                c.advance();
                let num_args = c.next_group(Delimiter::Parenthesis, "num args")?;
                let vars = c.skip_pipe_vars_returning();
                c.skip_colon();
                let ty = c.collect_until_brace();
                let body_group = c.next_group(Delimiter::Brace, "num type body")?;
                let (f, cap) = c.arrow_field_cap("num field")?;
                c.skip(';');
                cf.block.push((f.clone(), ty.clone(), cap));
                let (digits, ends) = parse_num_args(num_args.stream(), num_args.span())?;
                let mut vi = vars.into_iter();
                let num_var = vi
                    .next()
                    .unwrap_or_else(|| Ident::new("_n", PS::call_site()));
                let kind_var = vi
                    .next()
                    .unwrap_or_else(|| Ident::new("_k", PS::call_site()));
                cf.standalone.push(StandaloneRule::BlockNumbered {
                    field: f,
                    digits,
                    ends,
                    ty,
                    num_var,
                    kind_var,
                    body: body_group.stream(),
                });
            }
            Some(TT::Ident(id)) => {
                return Err(unknown_keyword(
                    &id,
                    "block",
                    "`num` or a `(pattern)` marker",
                ));
            }
            _ => {
                c.advance();
            }
        }
    }
    Ok(())
}

fn parse_line_marker_args(ts: TS2) -> Result<(Literal, Literal)> {
    let mut c = Cursor::new(ts);
    let byte = c.expect_lit("line marker byte")?;
    c.skip(',');
    c.advance();
    c.skip_eq_alone();
    Ok((byte, c.expect_lit("line marker max")?))
}

/// The byte pattern and minimum run of `line_simple(pat, min = N)` and
/// `fence(pat, min = N)`.
fn parse_min_args(ts: TS2, span: PS, rule: &str) -> Result<(TS2, Literal)> {
    let mut c = Cursor::new(ts);
    let pat = c.collect_pattern(&format!("{rule} bytes"))?;
    c.skip(',');
    if c.peek_str().as_deref() != Some("min") {
        return Err(Error::new(span, format!("`{rule}` rule needs `min = N`")));
    }
    c.advance();
    c.skip_eq_alone();
    Ok((pat, c.expect_lit(&format!("{rule} min"))?))
}

fn parse_cont_args(ts: TS2) -> Result<Literal> {
    Cursor::new(ts).expect_lit("cont byte")
}

/// The digit and end patterns of `num(digits, end = ends)`.
fn parse_num_args(ts: TS2, span: PS) -> Result<(TS2, TS2)> {
    let mut c = Cursor::new(ts);
    let digits = c.collect_pattern("num digits")?;
    c.skip(',');
    if c.peek_str().as_deref() != Some("end") {
        return Err(Error::new(span, "`num` rule needs `end = pattern`"));
    }
    c.advance();
    c.skip_eq_alone();
    Ok((digits, c.collect_pattern("num end")?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::standalone_field;
    use quote::quote;

    fn ci(s: TS2) -> CF {
        let mut cf = CF::default();
        collect_inline(s, &mut cf).unwrap();
        cf
    }
    fn cl(s: TS2) -> CF {
        let mut cf = CF::default();
        collect_lines(s, &mut cf).unwrap();
        cf
    }
    fn cb(s: TS2) -> CF {
        let mut cf = CF::default();
        collect_blocks(s, &mut cf).unwrap();
        cf
    }

    // 01. An inline fallback yields a single inline_simple field
    #[test]
    fn test_01_inline_fallback() {
        let cf = ci(quote! { fallback => texts [10]; });
        assert_eq!(cf.inline_simple.len(), 1);
        assert_eq!(cf.inline_simple[0].0.to_string(), "texts");
    }

    // 02. on_trigger: a symmetric exact arm produces a SymmetricExact standalone rule
    #[test]
    fn test_02_on_trigger_symmetric_exact() {
        let cf = ci(quote! {
            on_trigger(b'*') {
                symmetric b'*' { parse_inside = true; 1 => italics [40], }
            }
        });
        assert_eq!(cf.standalone.len(), 1);
        assert_eq!(standalone_field(&cf.standalone[0]).to_string(), "italics");
    }

    // 03. on_trigger: multiple symmetric counts produce one arm and one rule each
    #[test]
    fn test_03_on_trigger_symmetric_multiple_counts() {
        let cf = ci(quote! {
            on_trigger(b'*') {
                symmetric b'*' {
                    parse_inside = true;
                    1 => italics [40], 2 => bolds [40], 3 => bold_italics [80],
                }
            }
        });
        assert_eq!(cf.inline_simple.len(), 3);
        assert_eq!(cf.standalone.len(), 3);
    }

    // 04. on_trigger: asymmetric arm produces an AsymmetricExact standalone rule
    #[test]
    fn test_04_on_trigger_asymmetric_exact() {
        let cf = ci(quote! {
            on_trigger(b'<') {
                asymmetric b'<', b'>' { balanced = false; parse_inside = false; 1 => autolinks [100], }
            }
        });
        assert_eq!(standalone_field(&cf.standalone[0]).to_string(), "autolinks");
    }

    // 05. on_trigger: chained block produces a typed inline field and a Chained rule
    #[test]
    fn test_05_on_trigger_chained() {
        let cf = ci(quote! {
            on_trigger(b'[') {
                chained: Link {
                    | b'[', b']' | { parse_inside = false; balanced = false; } => text,
                    | b'(', b')' | { parse_inside = false; balanced = false; } => url,
                    prefix | b'!' | => is_image,
                } => links [100]
            }
        });
        assert_eq!(cf.inline.len(), 1);
        assert_eq!(cf.inline[0].0.to_string(), "links");
        assert_eq!(standalone_field(&cf.standalone[0]).to_string(), "links");
        let StandaloneRule::Chained {
            first_flags,
            second_flags,
            cap,
            ..
        } = &cf.standalone[0]
        else {
            panic!("expected a Chained rule");
        };
        assert!(!first_flags.parse_inside && !first_flags.balanced);
        assert!(!second_flags.parse_inside && !second_flags.balanced);
        assert_eq!(cap.to_string(), "100");
    }

    // 05b. The settings of each chained component are kept apart
    #[test]
    fn test_05b_chained_component_flags() {
        let cf = ci(quote! {
            on_trigger(b'[') {
                chained: Link {
                    | b'[', b']' | { parse_inside = true; balanced = true; } => text,
                    | b'(', b')' | { balanced = false; parse_inside = false; } => url,
                    prefix | b'!' | => is_image,
                } => links [50]
            }
        });
        let StandaloneRule::Chained {
            first_flags,
            second_flags,
            ..
        } = &cf.standalone[0]
        else {
            panic!("expected a Chained rule");
        };
        assert!(first_flags.parse_inside && first_flags.balanced);
        assert!(!second_flags.parse_inside && !second_flags.balanced);
    }

    // 06. on_trigger: key_value block produces a typed inline field and a KeyValue rule
    #[test]
    fn test_06_on_trigger_key_value() {
        let cf = ci(quote! {
            on_trigger(b'=') {
                key_value: KeyValue {
                    eq = b'='; allow_sep = true; end = b'\n';
                    key => key, value => value,
                } => key_values [20]
            }
        });
        assert_eq!(cf.inline[0].0.to_string(), "key_values");
        assert_eq!(
            standalone_field(&cf.standalone[0]).to_string(),
            "key_values"
        );
    }

    // 07. merge_simple is consumed and produces no field
    #[test]
    fn test_07_merge_simple_consumed() {
        let cf = ci(quote! { merge_simple = true; fallback => texts [10]; });
        assert_eq!(cf.inline_simple.len(), 1);
    }

    // 08. The old `memchr` spelling is an unknown keyword, reported here
    #[test]
    fn test_08_memchr_is_unknown_keyword() {
        let e = err_inline(quote! {
            memchr(b'*') {
                symmetric b'*' { parse_inside = true; 1 => italics [40], }
            }
        });
        assert!(e.contains("unknown keyword `memchr` in `inline`"), "{e}");
        assert!(e.contains("on_trigger"), "{e}");
    }

    fn err_inline(ts: TS2) -> String {
        let mut cf = CF::default();
        collect_inline(ts, &mut cf)
            .unwrap_err()
            .to_compile_error()
            .to_string()
    }

    // 16. Unknown keywords in every section are located errors
    #[test]
    fn test_16_unknown_keywords_per_section() {
        let e = err_inline(
            quote! { on_trigger(b'*') { symetric b'*' { parse_inside = true; balanced = false; 1 => italics [40], } } },
        );
        assert!(
            e.contains("unknown keyword `symetric` in `on_trigger`"),
            "{e}"
        );

        let mut cf = CF::default();
        let e = collect_lines(
            quote! { lines(b'#', max = 6) |n|: H { level: n } => headings [200]; },
            &mut cf,
        )
        .unwrap_err()
        .to_compile_error()
        .to_string();
        assert!(e.contains("unknown keyword `lines` in `lines`"), "{e}");

        let mut cf = CF::default();
        let e = collect_blocks(
            quote! { fence(b'`', min = 3) => fenced_codes [400]; },
            &mut cf,
        )
        .unwrap_err()
        .to_compile_error()
        .to_string();
        assert!(e.contains("unknown keyword `fence` in `blocks`"), "{e}");

        let mut cf = CF::default();
        let e = collect_blocks(
            quote! { block_simple { fences(b'`', min = 3) => fenced_codes [400]; } },
            &mut cf,
        )
        .unwrap_err()
        .to_compile_error()
        .to_string();
        assert!(
            e.contains("unknown keyword `fences` in `block_simple`"),
            "{e}"
        );

        let mut cf = CF::default();
        let e = collect_blocks(
            quote! { block { number(b'0'..=b'9', end = b'.') |n, k|: O { k, n } => items [80]; } },
            &mut cf,
        )
        .unwrap_err()
        .to_compile_error()
        .to_string();
        assert!(e.contains("unknown keyword `number` in `block`"), "{e}");
    }

    // 17. A symmetric body must state `parse_inside`, and takes no
    //     `balanced`; an asymmetric body must state both, as true|false
    #[test]
    fn test_17_missing_or_invalid_flags() {
        let e = err_inline(quote! { on_trigger(b'*') { symmetric b'*' { 1 => italics [40], } } });
        assert!(
            e.contains("`symmetric` body is missing `parse_inside = true|false;`"),
            "{e}"
        );
        let e = err_inline(quote! {
            on_trigger(b'*') { symmetric b'*' { parse_inside = true; balanced = false; 1 => italics [40], } }
        });
        assert!(e.contains("`symmetric` takes no `balanced`"), "{e}");

        let e = err_inline(
            quote! { on_trigger(b'<') { asymmetric b'<', b'>' { balanced = false; 1 => autolinks [100], } } },
        );
        assert!(
            e.contains("`asymmetric` body is missing `parse_inside = true|false;`"),
            "{e}"
        );

        let e = err_inline(
            quote! { on_trigger(b'*') { symmetric b'*' { parse_inside = maybe; 1 => italics [40], } } },
        );
        assert!(e.contains("missing `parse_inside = true|false;`"), "{e}");
    }

    // 18. Flags may come in any order and the opacity is read correctly
    #[test]
    fn test_18_flags_any_order() {
        let cf = ci(quote! {
            on_trigger(b'`') {
                symmetric b'`' { parse_inside = false; 1 => codes [80], }
            }
        });
        assert!(matches!(
            cf.standalone[0],
            StandaloneRule::SymmetricExact { opaque: true, .. }
        ));
    }

    // 19. A chained rule without its prefix, and a key_value rule without
    //     `end`, are errors instead of silently emitting no finder
    #[test]
    fn test_19_incomplete_chained_and_kv() {
        let e = err_inline(quote! {
            on_trigger(b'[') {
                chained: Link {
                    | b'[', b']' | { parse_inside = false; balanced = false; } => text,
                    | b'(', b')' | { parse_inside = false; balanced = false; } => url,
                } => links [100]
            }
        });
        assert!(e.contains("`chained` rule needs"), "{e}");

        let e = err_inline(quote! {
            on_trigger(b'=') {
                key_value: Pair { eq = b'='; allow_sep = true; key => key, value => value, } => pairs [20]
            }
        });
        assert!(e.contains("`key_value` rule needs"), "{e}");

        let e = err_inline(quote! {
            on_trigger(b'[') {
                chained: Link {
                    | b'[', b']' | { parse_inside = false; } => text,
                    | b'(', b')' | { parse_inside = false; balanced = false; } => url,
                    prefix | b'!' | => is_image,
                } => links [100]
            }
        });
        assert!(
            e.contains("`chained component` body is missing `balanced"),
            "{e}"
        );
    }

    // 20. An asymmetric rule kept on the stack has only a `1 =>` arm and
    //     carries its `balanced` flag; a forward-searched one may have others
    #[test]
    fn test_20_asymmetric_stack_arms() {
        for flags in [
            quote! { balanced = true; parse_inside = false; },
            quote! { balanced = false; parse_inside = true; },
        ] {
            for arm in [
                quote! { 2 => pairs [40], },
                quote! { _ => pairs [40], },
                quote! { 1 => pairs [40], },
            ] {
                let e = err_inline(quote! {
                    on_trigger(b'{', b'}') { asymmetric b'{', b'}' { #flags 1 => objs [40], #arm } }
                });
                assert!(e.contains("so its only arm is `1 => field`"), "{e}");
            }
        }
        let cf = ci(quote! {
            on_trigger(b'{', b'}') {
                asymmetric b'{', b'}' { parse_inside = true; balanced = true; 1 => objs [40], }
            }
        });
        assert!(matches!(
            cf.standalone[..],
            [StandaloneRule::AsymmetricExact {
                balanced: true,
                opaque: false,
                ..
            }]
        ));
        let cf = ci(quote! {
            on_trigger(b'<') {
                asymmetric b'<', b'>' { balanced = false; parse_inside = false; 1 => one [40], 2 => two [40], }
            }
        });
        assert_eq!(cf.standalone.len(), 2);
        // Any spelling of the value 1 is that arm.
        for arm in [
            quote! { 1u32 },
            quote! { 1_u32 },
            quote! { 0x1 },
            quote! { 01 },
        ] {
            let cf = ci(quote! {
                on_trigger(b'{', b'}') {
                    asymmetric b'{', b'}' { balanced = true; parse_inside = true; #arm => objs [40], }
                }
            });
            assert_eq!(cf.standalone.len(), 1);
        }
    }

    // 21. An integer literal's value, as the compiler reads it
    #[test]
    fn test_21_int_value() {
        let v = |s: &str| int_value(&s.parse::<Literal>().unwrap());
        for (s, n) in [
            ("1", 1),
            ("1u32", 1),
            ("1_000", 1000),
            ("0x1f", 31),
            ("0o17", 15),
            ("0b101", 5),
            ("7usize", 7),
        ] {
            assert_eq!(v(s), Some(n), "{s}");
        }
        for s in ["1.0", "b'1'", "\"1\"", "1f32"] {
            assert_eq!(v(s), None, "{s}");
        }
    }

    // 09. A line marker produces a LineMarker rule and a cf.line entry
    #[test]
    fn test_09_line_marker() {
        let cf = cl(quote! {
            line(b'#', max = 6) |n|: Heading { level: n } => headings [200];
        });
        assert_eq!(cf.line.len(), 1);
        assert_eq!(cf.line[0].0.to_string(), "headings");
        assert_eq!(standalone_field(&cf.standalone[0]).to_string(), "headings");
    }

    // 10. A line_simple produces a LineUniform rule
    #[test]
    fn test_10_line_uniform() {
        let cf = cl(quote! {
            line_simple(b'-' | b'*' | b'_', min = 3) |b|: ThematicBreak { kind: b } => thematic_breaks [200];
        });
        assert_eq!(
            standalone_field(&cf.standalone[0]).to_string(),
            "thematic_breaks"
        );
    }

    // 11. block_simple with fence + cont produces two fields and two rules
    #[test]
    fn test_11_block_simple_fence_cont() {
        let cf = cb(quote! {
            block_simple {
                fence(b'`', min = 3) => fenced_codes [400];
                cont(b'>') => blockquotes [200];
            }
        });
        assert_eq!(cf.block_simple.len(), 2);
        assert_eq!(cf.standalone.len(), 2);
    }

    // 12. A marker block produces a BlockMarker rule and a cf.block entry
    #[test]
    fn test_12_block_marker() {
        let cf = cb(quote! {
            block { (b'-' | b'*' | b'+') |b|: BulletItem { kind: b } => bullet_items [80]; }
        });
        assert_eq!(cf.block[0].0.to_string(), "bullet_items");
        assert_eq!(
            standalone_field(&cf.standalone[0]).to_string(),
            "bullet_items"
        );
    }

    // 13. A num block produces a BlockNumbered rule
    #[test]
    fn test_13_block_numbered() {
        let cf = cb(quote! {
            block {
                num(b'0'..=b'9', end = b'.' | b')') |n, k|:
                    OrderedItem { kind: k, num: n } => ordered_items [80];
            }
        });
        assert_eq!(
            standalone_field(&cf.standalone[0]).to_string(),
            "ordered_items"
        );
    }

    // 14. A blocks fallback produces a block_simple field and no standalone rule
    #[test]
    fn test_14_blocks_fallback() {
        let cf = cb(quote! { fallback => paragraphs [80]; });
        assert_eq!(cf.block_simple.len(), 1);
        assert_eq!(cf.block_simple[0].0.to_string(), "paragraphs");
        assert_eq!(cf.standalone.len(), 0);
    }

    // 15. An inline arm without a `[N]` capacity is rejected
    #[test]
    fn test_15_inline_missing_cap_err() {
        let mut cf = CF::default();
        assert!(collect_inline(quote! { fallback => texts; }, &mut cf).is_err());
    }

    // 22. Block and line byte patterns are passed on as written: several
    //     fence bytes, digit and end ranges, a range marker, a range run
    #[test]
    fn test_22_patterns_passed_as_written() {
        let cf = cb(quote! {
            block_simple { fence(b'`' | b'~', min = 3) => codes [40]; }
            block {
                (b'x'..=b'z' | b'+') |b|: Item { kind: b } => items [40];
                num(b'1'..=b'9', end = b'.' | b'a'..=b'c') |n, k|: Num { num: n, kind: k } => nums [40];
            }
        });
        let pats: Vec<String> = cf
            .standalone
            .iter()
            .map(|r| match r {
                StandaloneRule::Fence { pat, min, .. } => format!("{pat} / {min}"),
                StandaloneRule::BlockMarker { pat, .. } => pat.to_string(),
                StandaloneRule::BlockNumbered { digits, ends, .. } => format!("{digits} / {ends}"),
                _ => unreachable!(),
            })
            .collect();
        let want = [
            quote! { b'`' | b'~' }.to_string() + " / 3",
            quote! { b'x'..=b'z' | b'+' }.to_string(),
            quote! { b'1'..=b'9' }.to_string() + " / " + &quote! { b'.' | b'a'..=b'c' }.to_string(),
        ];
        assert_eq!(pats, want);

        let cf = cl(quote! {
            line_simple(b'0'..=b'9' | b'=', min = 3) |b|: Rule { kind: b } => rules [40];
        });
        let StandaloneRule::LineUniform { pat, .. } = &cf.standalone[0] else {
            unreachable!()
        };
        assert_eq!(pat.to_string(), quote! { b'0'..=b'9' | b'=' }.to_string());
    }

    // 23. A minimum given under any other key, or bare, is an error, as is a
    //     `num` without `end`
    #[test]
    fn test_23_min_and_end_keys_required() {
        let err = |r: Result<()>| r.unwrap_err().to_compile_error().to_string();
        let mut cf = CF::default();
        let e = err(collect_blocks(
            quote! { block_simple { fence(b'`', max = 3) => codes [40]; } },
            &mut cf,
        ));
        assert!(e.contains("`fence` rule needs `min = N`"), "{e}");
        let e = err(collect_lines(
            quote! { line_simple(b'-', 3) |b|: Rule { kind: b } => rules [40]; },
            &mut cf,
        ));
        assert!(e.contains("`line_simple` rule needs `min = N`"), "{e}");
        let e = err(collect_blocks(
            quote! { block { num(b'0'..=b'9', b'.') |n, k|: Num { num: n, kind: k } => nums [40]; } },
            &mut cf,
        ));
        assert!(e.contains("`num` rule needs `end = pattern`"), "{e}");
    }
}
