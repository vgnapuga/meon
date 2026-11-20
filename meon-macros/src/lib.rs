// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Nikita Shavrin

//! Procedural macro crate for the declarative parser engine.
//!
//! The single entry point is [`define_parser!`]. It consumes a grammar
//! description and expands to: the content struct (via `define_content!`), a
//! `...Parser` type whose `parse` method drives the runtime `parse_text!` macro,
//! the standalone `find_*` iterator functions, and the `_clean` / `_raw`
//! accessor methods.
//!
//! Internally the macro is a small three-stage pipeline:
//!
//! * `cursor` — a hand-rolled token-stream reader used by every stage;
//! * `collect` — the front-end: walks the grammar tokens and fills a
//!   `model::CF` plus a list of `model::StandaloneRule`s; the only place
//!   that reports grammar errors;
//! * `strip` / `normalize` — token surgery: the invisible groups of
//!   `macro_rules!` fragments unwrapped, and on the sections handed to the
//!   runtime macros, capacities removed and the `inline` section rewritten
//!   into the one shape those macros pattern-match;
//! * `codegen` / `methods` — the back-end: turns that data into tokens.
//!
//! The front-end returns `error::Result`; a malformed grammar surfaces as a
//! located `compile_error!` instead of a proc-macro panic. Only [`define_parser`]
//! lives here because `#[proc_macro]` entry points must reside in the crate root.

mod codegen;
mod collect;
mod cursor;
mod error;
mod limits;
mod methods;
mod model;
mod normalize;
mod strip;

use proc_macro::TokenStream;
use proc_macro2::{Delimiter, Ident, Literal, Span as PS, TokenStream as TS2, TokenTree as TT};
use quote::quote;

use crate::codegen::{build_define_content, build_standalone_dsl};
use crate::collect::{collect_blocks, collect_inline, collect_lines};
use crate::cursor::Cursor;
use crate::error::{Error, Result};
use crate::limits::{Limits, literal_u64};
use crate::methods::build_content_methods;
use crate::model::{CF, crate_path};
use crate::normalize::canonicalize_inline;
use crate::strip::{flatten_invisible, strip};

/// Generate a parser from a declarative grammar.
///
/// # Overview
///
/// `define_parser!` takes a grammar description and expands it into:
///
/// - `<Name>Content<'a>` — the output struct with one `pub` field per grammar
///   rule, borrowing the source slice for its lifetime.
/// - `<Name>Parser` — a unit struct with checked and unchecked methods:
///   - `parse(source: &[u8]) -> Result<<Name>Content<'_>, InputBytesTooLarge>`
///     — full single-pass parse;
///   - `parse_unchecked(source: &[u8]) -> <Name>Content<'_>` — the same
///     parse without the `max_input_bytes` check;
///   - `find_<field>` / `find_<field>_unchecked` — standalone per-element
///     iterators, checked and unchecked;
///   - `context` / `context_unchecked` and `find_context_<field>` /
///     `find_context_<field>_unchecked` — the opaque-region map and its
///     context-aware finders, checked and unchecked.
/// - Accessor methods `<field>_clean` and `<field>_raw` on `<Name>Content`
///   for ergonomic span-to-slice conversion.
///
/// # Grammar syntax
///
/// ```text
/// define_parser!(Name {
///     sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
///
///     limits {
///         max_nest = 4;
///         max_inline_span = 1_024;
///         max_line_span = 0;
///         max_block_span = 0;
///         max_input_bytes = 0;
///     }
///
///     inline  { ... }
///     lines   { ... }
///     blocks  { ... }
/// });
/// ```
///
/// ## Context bytes
///
/// | Key         | Meaning                                                        |
/// |-------------|----------------------------------------------------------------|
/// | `sep`       | Word separator (typically space)                               |
/// | `eol`       | Line terminator (typically `\n`)                               |
/// | `tab`       | Tab character                                                  |
/// | `escape`    | Escape prefix that suppresses the next byte                    |
/// | `crlf`      | Optional, default `false`. With `true`, a `\r` right before    |
/// |             | `eol` is part of the line end: no line content, text or        |
/// |             | element takes it, and a line of it alone, or of it after       |
/// |             | `sep`/`tab` bytes, is blank. A `\r` anywhere else is a byte    |
/// |             | like any other.                                                |
///
/// `max_nest` is also accepted in the header for existing grammars, but
/// `limits` is the current home for it. When both are present they must
/// agree.
///
/// ## `limits { ... }` section
///
/// Optional. Settings may be written in any order; `;` and `,` are optional.
///
/// | Key               | Meaning                                                     |
/// |-------------------|-------------------------------------------------------------|
/// | `offset`          | Default `u32`. Span offset type: `u8`, `u16`, `u32` or `u64`.|
/// | `max_nest`        | Default `1`. Shared nesting cap of the inline stack and     |
/// |                   | the block stack (`cont` / `fence`). At `1` nothing nests.   |
/// | `max_inline_span` | Default absent. Longest emitted inline span, in bytes.      |
/// | `max_line_span`   | Default absent. Longest emitted line span, in bytes.        |
/// | `max_block_span`  | Default absent. Longest emitted block span, in bytes.       |
/// | `max_input_bytes` | Default absent. Longest accepted source, in bytes.          |
///
/// `offset` selects the type of every generated `Span` and the hard upper
/// bound on the input. `u8`/`u16`/`u32` use that type for span offsets; `u64`
/// uses `u64` spans with `usize` positions internally. The checked methods
/// reject input longer than the selected type only when `max_input_bytes` is
/// set; otherwise they read the prefix that fits.
///
/// The span caps are measured on the clean span — `end - start` of the
/// stored `Span`, without delimiters. `0` and the selected offset type's
/// maximum mean absent: no check is generated. A value above that maximum is
/// a compile error. When a cap is absent, the corresponding checks and
/// fallback code are not generated.
///
/// `max_input_bytes` follows different rules. `0` and the selected offset
/// type's maximum mean that maximum: the checked methods reject a larger
/// source. `_` or an absent key means no check at all. A value above the
/// selected maximum is a compile error. The check runs before any parser
/// state or `Vec` is allocated.
///
/// A span longer than its cap is not emitted:
///
/// - an inline transparent span keeps its inner spans and its text;
/// - an inline `parse_inside = false` span is dropped and its interior is
///   reparsed;
/// - a `chained` element is dropped and its interior is reparsed;
/// - a `key_value` pair is dropped and is not reparsed;
/// - a line or block item span goes to `blocks { fallback => ... }`;
/// - an over-long fence is reparsed as paragraph runs and no longer opaque.
///
/// A line of `sep` and `tab` bytes alone is as blank as an empty line: it
/// ends a paragraph and closes the open blocks, outside a fence.
///
/// ## `inline { ... }` section
///
/// Declares elements that appear inside lines. All inline rules are triggered
/// by specific bytes declared in `on_trigger`; unmatched bytes fall through to
/// `fallback`.
///
/// ```text
/// inline {
///     merge_simple = true;          // coalesce adjacent fallback spans
///
///     hard_break(esc, sp, min) => field [div];
///     // Detects trailing hard-break sequences: an `esc` byte not itself
///     // escaped OR ≥ `min` consecutive `sp` bytes at end of line. Emits a
///     // zero-length Span; fewer `sp` bytes stay text.
///
///     on_trigger(b1, b2, ...) {
///         symmetric byte {
///             parse_inside = true | false; // true: frames on the stack;
///                                          // false: opaque forward search
///             N => field [div],   // exact count N of `byte` → Span
///         }
///         asymmetric open, close {
///             balanced     = true | false; // counts nested `open`/`close`
///             parse_inside = true | false; // true: frames on the stack;
///                                          // false: opaque forward search
///             N => field [div],   // exact count N of `open`; with either
///                                 // flag true each open byte is an open
///                                 // of its own, and `1 =>` is the only arm
///         }
///         chained: Type {
///             | open1, close1 | { parse_inside = ...; balanced = ...; } => text_field,
///             | open2, close2 | { parse_inside = ...; balanced = ...; } => url_field,
///             prefix | byte | => prefix_field,
///         } => field [div]
///         key_value: Type {
///             eq        = byte;
///             allow_sep = true | false;
///             end       = byte;
///             key   => key_field,
///             value => value_field,
///         } => field [div]
///     }
///
///     fallback => field [div];    // plain-text runs → Vec<Span>
/// }
/// ```
///
/// ## `lines { ... }` section
///
/// Declares whole-line elements, tried at the start of a line no block rule
/// claims.
///
/// ```text
/// lines {
///     line(byte, max = N) |var|: Type { ... } => field [div];
///     // Matches 1–N leading `byte` bytes followed by `sep` or the line end.
///     // `var` receives the count. The Span is the rest of the line, which is
///     // scanned for inline rules. Produces Vec<(Type, Span)>.
///
///     line_simple(pattern, min = N) |var|: Type { ... } => field [div];
///     // Matches a line composed entirely of one byte `pattern` matches
///     // (interleaved with `sep`), at least `min` times. `var` receives that
///     // byte. The whole line is consumed; no inline scan.
///     // Produces Vec<(Type, Span)>.
/// }
/// ```
///
/// ## `blocks { ... }` section
///
/// Declares multi-line constructs and single-line items with metadata.
///
/// ```text
/// blocks {
///     block_simple {
///         fence(pattern, min = N) => field [div];
///         // Opens on a line starting with ≥ N of one byte `pattern` matches
///         // (and no further such byte on the line); closes on a line of at
///         // least as many of that same byte. Entire range is one Span.
///         // Inline scanning suppressed while fence is active.
///
///         cont(byte) => field [div];
///         // Groups consecutive lines starting with `byte` into one Span.
///     }
///
///     block {
///         (pattern) |var|: Type { ... } => field [div];
///         // Single-line item: marker byte matching `pattern`, followed by
///         // `sep` or `tab`. `var` receives the marker byte.
///         // Produces Vec<(Type, Span)>.
///
///         num(digit_pat, end = end_pat) |n, k|: Type { ... } => field [div];
///         // Single-line numbered item: one to nine bytes matching
///         // `digit_pat`, then a byte matching `end_pat`, then `sep` or `tab`.
///         // `n` receives the number (each byte counts as its value less
///         // `b'0'`), `k` the delimiter byte. Produces Vec<(Type, Span)>.
///     }
///
///     fallback => field [div];
///     // Lines matching no other block rule are grouped into paragraph Spans.
/// }
/// ```
///
/// ## Capacity divisors `[div]`
///
/// Each field carries `[div]` — the initial `Vec` capacity is
/// `source.len() / div`. Tune based on expected element density:
/// a divisor of `10` means roughly one element per 10 bytes.
///
/// ## Inside `macro_rules!`
///
/// A grammar may be written inside a `macro_rules!` macro and take its
/// settings, bytes, patterns, capacities and metadata from the macro's
/// fragments (`$n:literal`, `$p:pat`, `$e:expr`, ...): a fragment reads as
/// the tokens it stands for, and an `$e:expr` in a metadata body keeps its
/// precedence.
///
/// # Errors
///
/// A malformed grammar emits a located `compile_error!` at the offending token
/// rather than panicking. The error message includes the parsing context
/// (e.g. `` "`fence` rule needs `min = N`" ``).
///
/// # Cross-crate hygiene
///
/// All macro calls emitted by the expansion are fully qualified via
/// `proc_macro_crate::crate_name` so the generated code works correctly
/// whether `define_parser!` is called inside `meon` itself or from a dependent
/// crate.
#[proc_macro]
pub fn define_parser(input: TokenStream) -> TokenStream {
    match expand(TS2::from(input)) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// The inline rules `parse_inline!` consumes: the canonicalised section
/// without `merge_simple` and the fallback field, which `parse_text!` reads
/// separately.
fn inline_rules(tokens: &[TT]) -> TS2 {
    let mut out = TS2::new();
    let mut i = 0usize;
    while i < tokens.len() {
        if let TT::Ident(id) = &tokens[i] {
            if id == "merge_simple" || id == "fallback" {
                i += 1;
                while i < tokens.len()
                    && !matches!(&tokens[i], TT::Punct(p) if p.as_char() == ';' || p.as_char() == ',')
                {
                    i += 1;
                }
                if i < tokens.len() {
                    i += 1;
                }
                continue;
            }
        }
        out.extend([tokens[i].clone()]);
        i += 1;
    }
    out
}

/// Fallible core of [`define_parser`].
fn expand(input: TS2) -> Result<TS2> {
    let mut c = Cursor::new(flatten_invisible(input));

    let name = c.next_ident("parser name")?;
    let body_g = c.next_group(Delimiter::Brace, "parser body")?;
    let content_name = Ident::new(&format!("{}Content", name), name.span());
    let parser_name = Ident::new(&format!("{}Parser", name), name.span());

    let mut bc = Cursor::new(body_g.stream());
    let sep = bc.named_lit("sep")?;
    bc.skip(',');
    let eol = bc.named_lit("eol")?;
    bc.skip(',');
    let tab = bc.named_lit("tab")?;
    bc.skip(',');
    let esc = bc.named_lit("escape")?;

    // Two optional context settings may follow `escape`, before the header's
    // closing `;`, in either order: `max_nest = N` (absent ⇒ `1`) and
    // `crlf = true | false` (absent ⇒ `false`).
    let mut max_nest: Option<Literal> = None;
    let mut crlf: Option<Ident> = None;
    while matches!(bc.peek(), Some(TT::Punct(p)) if p.as_char() == ',') {
        bc.advance();
        let span = bc.span();
        match bc.peek_str().as_deref() {
            Some("max_nest") if max_nest.is_none() => max_nest = Some(bc.named_lit("max_nest")?),
            Some("crlf") if crlf.is_none() => crlf = Some(bc.named_flag("crlf")?),
            Some(name @ ("max_nest" | "crlf")) => {
                return Err(Error::new(span, format!("`{name}` is set twice")));
            }
            _ => {
                return Err(Error::new(
                    span,
                    format!("expected `max_nest` or `crlf`, got {:?}", bc.peek()),
                ));
            }
        }
    }
    let crlf = crlf.unwrap_or_else(|| Ident::new("false", PS::call_site()));
    bc.skip(';');

    let mut inline_ts = TS2::new();
    let mut lines_ts = TS2::new();
    let mut blocks_ts = TS2::new();
    let mut limits = Limits::default();
    let mut limits_seen = false;

    loop {
        match bc.peek_str().as_deref() {
            Some("limits") => {
                let span = bc.span();
                if limits_seen {
                    return Err(Error::new(span, "`limits` is set twice"));
                }
                bc.advance();
                limits = Limits::parse(bc.next_group(Delimiter::Brace, "limits")?.stream())?;
                limits_seen = true;
            }
            Some("inline") => {
                bc.advance();
                inline_ts = bc.next_group(Delimiter::Brace, "inline")?.stream();
            }
            Some("lines") => {
                bc.advance();
                lines_ts = bc.next_group(Delimiter::Brace, "lines")?.stream();
            }
            Some("blocks") => {
                bc.advance();
                blocks_ts = bc.next_group(Delimiter::Brace, "blocks")?.stream();
            }
            _ => break,
        }
    }

    // `max_nest` may be written in the header or in `limits`. The header form
    // is kept for existing grammars; when both are present they must agree.
    let max_nest = match (max_nest, limits.max_nest.take()) {
        (Some(header), Some(section)) => {
            if literal_u64(&header) != literal_u64(&section) {
                return Err(Error::new(
                    section.span(),
                    "`max_nest` differs from the value in the grammar header",
                ));
            }
            header
        }
        (Some(header), None) => header,
        (None, Some(section)) => section,
        (None, None) => Literal::usize_unsuffixed(1),
    };
    // Resolve the runtime crate path once; used for every qualified macro call
    // in the expansion so it works correctly from any dependent crate.
    let mc = crate_path();

    let off_tt = limits.offset_tt();
    let lim_inline = limits.inline_tt();
    let lim_line = limits.line_tt();
    let lim_block = limits.block_tt();
    let input_limit = limits.max_input_bytes.as_ref().map(|limit| limit.literal());
    let check_tt = if input_limit.is_some() {
        quote! { [__meon_check_input] }
    } else {
        quote! { [] }
    };
    let check_call = if input_limit.is_some() {
        quote! { Self::__meon_check_input(source)?; }
    } else {
        TS2::new()
    };
    let check_fn = if let Some(lit) = &input_limit {
        quote! {
            #[inline]
            fn __meon_check_input(
                source: &[u8],
            ) -> ::core::result::Result<(), #mc::InputBytesTooLarge<#off_tt>> {
                if source.len() > #lit as usize {
                    ::core::result::Result::Err(#mc::InputBytesTooLarge {
                        len: source.len(),
                        max: #lit,
                    })
                } else {
                    ::core::result::Result::Ok(())
                }
            }
        }
    } else {
        TS2::new()
    };

    let mut cf = CF::default();
    collect_inline(inline_ts.clone(), &mut cf)?;
    collect_lines(lines_ts.clone(), &mut cf)?;
    collect_blocks(blocks_ts.clone(), &mut cf)?;

    let pt_inline: Vec<_> = canonicalize_inline(strip(inline_ts)).into_iter().collect();
    let pt_inline_rules = inline_rules(&pt_inline);
    let pt_lines: Vec<_> = strip(lines_ts).into_iter().collect();
    let pt_blocks: Vec<_> = strip(blocks_ts).into_iter().collect();

    // A reparse helper is emitted only when an inline cap is configured. It
    // scans an oversized opaque construct's interior with the same grammar.
    let reparse_helper = Ident::new(&format!("__meon_reparse_{}", name), name.span());
    let has_inline_limit = limits.max_inline_span.is_some() || limits.max_block_span.is_some();
    let reparse_fn = if let (Some(tx), true) = (&cf.inline_fallback, has_inline_limit) {
        let merge = cf
            .merge_simple
            .clone()
            .unwrap_or_else(|| Ident::new("false", PS::call_site()));
        quote! {
            #[allow(dead_code, non_snake_case, clippy::too_many_arguments)]
            fn #reparse_helper(
                state: &mut ParseState,
                src: &[u8],
                start: usize,
                end: usize,
                _reparse_depth: usize,
                cont_bytes: &[u8],
                cd: usize,
                split: bool,
            ) {
                if _reparse_depth > #max_nest {
                    return;
                }
                if split {
                    let mut pos = start;
                    let mut run_start: Option<usize> = None;
                    let trim_end = |s: usize, mut e: usize| -> usize {
                        if e > s && src[e - 1] == #eol {
                            e -= 1;
                            if #crlf && e > s && src[e - 1] == b'\r' {
                                e -= 1;
                            }
                        }
                        e
                    };
                    while pos < end {
                        let line_end = #mc::memchr::memchr(#eol, &src[pos..end])
                            .map_or(end, |i| pos + i);
                        let content_end = if #crlf
                            && line_end > pos
                            && src[line_end - 1] == b'\r'
                        {
                            line_end - 1
                        } else {
                            line_end
                        };
                        let mut content = pos;
                        let mut k = 0;
                        while k < cd
                            && content < content_end
                            && src[content] == cont_bytes[k]
                        {
                            content += if content + 1 < content_end
                                && (src[content + 1] == #sep || src[content + 1] == #tab)
                            {
                                2
                            } else {
                                1
                            };
                            k += 1;
                        }
                        let blank = content == content_end
                            || src[content..content_end]
                                .iter()
                                .all(|&b| b == #sep || b == #tab);
                        if blank {
                            if let Some(s) = run_start.take() {
                                #reparse_helper(
                                    state, src, s, trim_end(s, pos), _reparse_depth,
                                    cont_bytes, cd, false,
                                );
                            }
                        } else if run_start.is_none() {
                            run_start = Some(pos);
                        }
                        pos = if line_end < end { line_end + 1 } else { end };
                    }
                    if let Some(s) = run_start {
                        #reparse_helper(
                            state, src, s, trim_end(s, end), _reparse_depth,
                            cont_bytes, cd, false,
                        );
                    }
                    return;
                }
                let mut start = start;
                let mut _k = 0;
                while _k < cd && start < end && src[start] == cont_bytes[_k] {
                    start += if start + 1 < end
                        && (src[start + 1] == #sep || src[start + 1] == #tab)
                    {
                        2
                    } else {
                        1
                    };
                    _k += 1;
                }
                #mc::parse_inline!(@off #off_tt,
                    state, src, start, end, #tx, #merge, #esc, #sep, #tab, #eol, #max_nest,
                    cont(cont_bytes, cd), crlf = #crlf, max_inline = #lim_inline,
                    reparse = [ #reparse_helper ], depth = _reparse_depth ;
                    #pt_inline_rules
                );
            }
        }
    } else {
        TS2::new()
    };
    let reparse_tt = if has_inline_limit {
        quote! { [ #reparse_helper ] }
    } else {
        quote! { [] }
    };

    let dc = build_define_content(&content_name, &off_tt, &cf, &mc);
    let cm = build_content_methods(&content_name, &off_tt, &cf, &sep, &eol, &tab, &esc);
    let dsl = build_standalone_dsl(
        &sep,
        &eol,
        &tab,
        &esc,
        &max_nest,
        &crlf,
        &lim_inline,
        &lim_line,
        &lim_block,
        &check_tt,
        &off_tt,
        &cf.standalone,
    );

    Ok(quote! {
        #dc
        #reparse_fn

        #[allow(missing_docs)]
        pub struct #parser_name;

        #[allow(missing_docs)]
        impl #parser_name {
            #check_fn

            pub fn parse(
                source: &[u8],
            ) -> ::core::result::Result<#content_name<'_>, #mc::InputBytesTooLarge<#off_tt>> {
                #check_call
                ::core::result::Result::Ok(Self::parse_unchecked(source))
            }

            pub fn parse_unchecked(source: &[u8]) -> #content_name<'_> {
                #mc::parse_text!(
                    source;
                    off = #off_tt,
                    sep = #sep, eol = #eol, tab = #tab, escape = #esc,
                    max_nest = #max_nest, crlf = #crlf,
                    max_inline_span = #lim_inline,
                    max_line_span = #lim_line,
                    max_block_span = #lim_block,
                    reparse = #reparse_tt;
                    inline { #(#pt_inline)* }
                    lines  { #(#pt_lines)* }
                    blocks { #(#pt_blocks)* }
                )
            }

            #mc::define_standalone_fns! { #dsl }
        }

        #cm
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A structurally well-formed grammar (the referenced types need not exist:
    /// `expand` only builds tokens, it does not compile them).
    fn valid() -> TS2 {
        quote! {
            Demo {
                sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
                inline {
                    merge_simple = true;
                    on_trigger(b'*', b'`') {
                        symmetric b'`' {
                            parse_inside = false;
                            1 => codes [80],
                        }
                        symmetric b'*' {
                            parse_inside = true;
                            1 => italics [40], 2 => bolds [40],
                        }
                    }
                    fallback => texts [10];
                }
                lines {
                    line(b'#', max = 6) |n|:
                        Heading { level: n }
                        => headings [200];
                }
                blocks {
                    block_simple {
                        cont(b'>') => blockquotes [200];
                    }
                    fallback => paragraphs [80];
                }
            }
        }
    }

    fn expand_str(g: TS2) -> String {
        expand(g).unwrap().to_string()
    }

    fn expand_err(g: TS2) -> String {
        expand(g).unwrap_err().to_compile_error().to_string()
    }

    // 01. A well-formed grammar expands without error
    #[test]
    fn test_01_valid_grammar_ok() {
        assert!(expand(valid()).is_ok());
    }

    // 02. The expansion declares the `<Name>Parser` type
    #[test]
    fn test_02_emits_parser_type() {
        assert!(expand_str(valid()).contains("DemoParser"));
    }

    // 03. The expansion references the `<Name>Content` type
    #[test]
    fn test_03_emits_content_type() {
        assert!(expand_str(valid()).contains("DemoContent"));
    }

    // 04. The expansion wires the runtime parse_text! macro (qualified)
    #[test]
    fn test_04_emits_parse_text() {
        assert!(expand_str(valid()).contains("parse_text"));
    }

    // 05. The expansion wires the standalone finder generator (qualified)
    #[test]
    fn test_05_emits_standalone_fns() {
        assert!(expand_str(valid()).contains("define_standalone_fns"));
    }

    // 06. define_content! call is fully qualified via the crate path
    #[test]
    fn test_06_define_content_qualified() {
        let s = expand_str(valid());
        // The call must appear as `<path>::define_content!`, not bare.
        // We check that `define_content` is always preceded by `::`.
        let idx = s.find("define_content").expect("define_content missing");
        assert!(
            s[..idx].ends_with(":: "),
            "define_content! must be qualified (preceded by ::), got: ...{}...",
            &s[idx.saturating_sub(20)..idx + 20]
        );
    }

    // 07. parse_text! call is fully qualified via the crate path
    #[test]
    fn test_07_parse_text_qualified() {
        let s = expand_str(valid());
        let idx = s.find("parse_text").expect("parse_text missing");
        assert!(
            s[..idx].ends_with(":: "),
            "parse_text! must be qualified (preceded by ::), got: ...{}...",
            &s[idx.saturating_sub(20)..idx + 20]
        );
    }

    // 08. Empty input errors on the missing parser name
    #[test]
    fn test_08_empty_input_err() {
        let e = expand_err(quote! {});
        assert!(e.contains("expected ident"));
    }

    // 09. A bare name with no body errors on the missing brace group
    #[test]
    fn test_09_name_without_body_err() {
        let e = expand_err(quote! { Demo });
        assert!(e.contains("expected group"));
    }

    // 10. A truncated context section errors on the missing `eol`
    #[test]
    fn test_10_context_truncated_err() {
        let e = expand_err(quote! { Demo { sep = b' ' } });
        assert!(e.contains("eol"));
    }

    // 11. A misspelled context keyword errors on the expected `sep`
    #[test]
    fn test_11_wrong_context_keyword_err() {
        let e = expand_err(quote! { Demo { sap = b' ' } });
        assert!(e.contains("expected `sep`"));
    }

    // 12. An inline arm without a `[N]` capacity errors
    #[test]
    fn test_12_inline_missing_cap_err() {
        let e = expand_err(quote! {
            Demo {
                sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
                inline { fallback => texts; }
            }
        });
        assert!(e.contains("[N]"));
    }

    // 13. A `symmetric` block without its byte literal errors
    #[test]
    fn test_13_symmetric_missing_byte_err() {
        let e = expand_err(quote! {
            Demo {
                sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
                inline { on_trigger(b'*') { symmetric { 1 => bolds [40], } } }
            }
        });
        assert!(e.contains("symmetric byte"));
    }

    // 14. A `line` marker without `max = N` errors
    #[test]
    fn test_14_line_missing_max_err() {
        let e = expand_err(quote! {
            Demo {
                sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
                lines { line(b'#') |n|: Heading { level: n } => headings [200]; }
            }
        });
        assert!(e.contains("line marker max"));
    }

    // 15. A `fence` without `min = N` errors
    #[test]
    fn test_15_fence_missing_min_err() {
        let e = expand_err(quote! {
            Demo {
                sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
                blocks { block_simple { fence(b'`') => fenced_codes [400]; } }
            }
        });
        assert!(e.contains("`fence` rule needs `min = N`"), "{e}");
    }

    // 16. max_nest, when present, is forwarded verbatim into the parse_text!
    //     call instead of the implicit default of 1.
    #[test]
    fn test_16_explicit_max_nest_forwarded() {
        let g = quote! {
            Demo {
                sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\', max_nest = 4;
            inline { fallback => texts [10]; }
            }
        };
        let s = expand_str(g);
        assert!(s.contains("max_nest"));
        let idx = s.find("max_nest").expect("max_nest missing");
        // The literal 4 must appear shortly after the max_nest keyword.
        assert!(s[idx..idx + 30].contains('4'));
    }

    // 18. Sub-rules declared out of order are emitted in the runtime's order
    #[test]
    fn test_18_out_of_order_sub_rules_are_canonicalised() {
        let g = quote! {
            Demo {
                sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
                inline {
                    on_trigger(b'*', b'<') {
                        asymmetric b'<', b'>' { parse_inside = false; balanced = false; 1 => autolinks [100], }
                        symmetric b'*' { parse_inside = true; 1 => italics [40], }
                    }
                    fallback => texts [10];
                }
            }
        };
        let s = expand_str(g);
        let sym = s.find("symmetric b'*'").expect("symmetric missing");
        let asym = s.find("asymmetric b'<'").expect("asymmetric missing");
        assert!(
            sym < asym,
            "symmetric must precede asymmetric in the expansion"
        );
        // Flag order follows the runtime pattern per kind.
        assert!(s.contains("symmetric b'*' { parse_inside = true ; "), "{s}");
        assert!(
            s.contains("asymmetric b'<' , b'>' { balanced = false ; parse_inside = false ; "),
            "{s}"
        );
    }

    // 19. An unknown keyword inside `inline` is a located front-end error
    #[test]
    fn test_19_unknown_inline_keyword_err() {
        let e = expand_err(quote! {
            Demo {
                sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
                inline { memchr(b'*') { symmetric b'*' { parse_inside = true; 1 => italics [40], } } }
            }
        });
        assert!(e.contains("unknown keyword `memchr`"), "{e}");
    }

    // 20. A symmetric body with `balanced` is a located front-end error
    #[test]
    fn test_20_symmetric_balanced_err() {
        let e = expand_err(quote! {
            Demo {
                sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
                inline { on_trigger(b'*') { symmetric b'*' { parse_inside = true; balanced = true; 1 => italics [40], } } }
            }
        });
        assert!(e.contains("`symmetric` takes no `balanced`"), "{e}");
    }

    // 17. max_nest, when absent, still appears in the expansion with the
    //     implicit default value of 1 (parse_text! always receives it).
    #[test]
    fn test_17_absent_max_nest_defaults_to_one() {
        let s = expand_str(valid());
        let idx = s.find("max_nest").expect("max_nest missing from expansion");
        assert!(
            s[idx..idx + 20].contains('1'),
            "expected default max_nest = 1, got: ...{}...",
            &s[idx..idx + 20]
        );
    }

    // 24. A set input limit emits the check and the unchecked parse
    #[test]
    fn test_24_input_limit_emits_check() {
        let g = quote! {
            Demo {
                sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
                limits { max_input_bytes = 8; }
                inline { fallback => texts [10]; }
            }
        };
        let s = expand_str(g);
        assert!(s.contains("__meon_check_input"), "{s}");
        assert!(s.contains("parse_unchecked"), "{s}");
    }

    // 25. `_` means no check code is generated
    #[test]
    fn test_25_input_limit_underscore_emits_no_check() {
        let g = quote! {
            Demo {
                sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
                limits { max_input_bytes = _; }
                inline { fallback => texts [10]; }
            }
        };
        let s = expand_str(g);
        assert!(!s.contains("__meon_check_input"), "{s}");
        assert!(s.contains("parse_unchecked"), "{s}");
    }

    // 26. An absent key emits no check code
    #[test]
    fn test_26_absent_input_limit_emits_no_check() {
        let s = expand_str(valid());
        assert!(!s.contains("__meon_check_input"), "{s}");
    }

    // 27. `0` means the maximum input limit
    #[test]
    fn test_27_input_limit_zero_is_the_maximum() {
        let g = quote! {
            Demo {
                sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
                limits { max_input_bytes = 0; }
                inline { fallback => texts [10]; }
            }
        };
        let s = expand_str(g);
        assert!(s.contains("__meon_check_input"), "{s}");
        assert!(s.contains("4294967295"), "{s}");
    }

    // 21. A `chained` rule with both components opaque feeds `context()`; one
    //     with a transparent component does not
    #[test]
    fn test_21_chained_context_source() {
        let grammar = |text_inside: bool| {
            quote! {
                Demo {
                    sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
                    inline {
                        on_trigger(b'[') {
                            chained: Link {
                                | b'[', b']' | { parse_inside = #text_inside; balanced = false; } => text,
                                | b'(', b')' | { parse_inside = false; balanced = true; } => url,
                                prefix | b'!' | => is_image,
                            } => links [70]
                        }
                        fallback => texts [10];
                    }
                }
            }
        };
        let compact = |s: String| s.split_whitespace().collect::<String>();
        let opaque = compact(expand_str(grammar(false)));
        assert!(
            opaque.contains("chained[(b'[',b']',false,b'(',b')',true)]"),
            "{opaque}"
        );
        assert!(opaque.contains("caps[70]"), "{opaque}");
        let mixed = compact(expand_str(grammar(true)));
        assert!(mixed.contains("chained[]"), "{mixed}");
        assert!(mixed.contains("caps[]"), "{mixed}");
        // The finder gets both components' `balanced` flags and whether
        // either is transparent.
        assert!(
            opaque.contains("chained(b'[',b']',false,b'(',b')',true,false,b'!',"),
            "{opaque}"
        );
        assert!(
            mixed.contains("chained(b'[',b']',false,b'(',b')',true,true,b'!',"),
            "{mixed}"
        );
    }

    // 22. `crlf`, set or not, reaches `parse_text!` and the finders; it may
    //     come before `max_nest`
    #[test]
    fn test_22_crlf_forwarded() {
        let g = quote! {
            Demo {
                sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\', crlf = true, max_nest = 2;
                inline { fallback => texts [10]; }
            }
        };
        let s = expand_str(g);
        assert_eq!(s.matches("max_nest = 2").count(), 2, "{s}");
        assert!(s.contains("crlf = true"), "{s}");
        let s = expand_str(valid());
        assert!(s.contains("crlf = false"), "{s}");
    }

    // 23. A context setting set twice, an unknown one, or a `crlf` that is
    //     no flag, is a located error
    #[test]
    fn test_23_context_setting_errs() {
        let e = expand_err(quote! {
            Demo { sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\', crlf = true, crlf = false; }
        });
        assert!(e.contains("`crlf` is set twice"), "{e}");
        let e = expand_err(quote! {
            Demo { sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\', depth = 2; }
        });
        assert!(e.contains("expected `max_nest` or `crlf`"), "{e}");
        let e = expand_err(quote! {
            Demo { sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\', crlf = 1; }
        });
        assert!(
            e.contains("expected `true` or `false` after `crlf =`"),
            "{e}"
        );
    }
}
