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
//! * [`cursor`] — a hand-rolled token-stream reader used by every stage;
//! * [`collect`] — the front-end: walks the grammar tokens and fills a
//!   [`model::CF`] plus a list of [`model::StandaloneRule`]s;
//! * [`codegen`] / [`methods`] — the back-end: turns that data into tokens.
//!
//! The front-end returns [`error::Result`]; a malformed grammar surfaces as a
//! located `compile_error!` instead of a proc-macro panic. Only [`define_parser`]
//! lives here because `#[proc_macro]` entry points must reside in the crate root.

mod codegen;
mod collect;
mod cursor;
mod error;
mod methods;
mod model;
mod strip;

use proc_macro::TokenStream;
use proc_macro2::{Delimiter, Ident, Literal, TokenStream as TS2, TokenTree as TT};
use quote::quote;

use crate::codegen::{build_define_content, build_standalone_dsl};
use crate::collect::{collect_blocks, collect_inline, collect_lines};
use crate::cursor::Cursor;
use crate::error::Result;
use crate::methods::build_content_methods;
use crate::model::{CF, crate_path};
use crate::strip::strip;

/// Generate a parser from a declarative grammar.
///
/// # Overview
///
/// `define_parser!` takes a grammar description and expands it into:
///
/// - `<Name>Content<'a>` — the output struct with one `pub` field per grammar
///   rule, borrowing the source slice for its lifetime.
/// - `<Name>Parser` — a unit struct with two kinds of methods:
///   - `parse(source: &[u8]) -> <Name>Content<'_>` — full single-pass parse.
///   - `find_<field>(source: &[u8]) -> impl Iterator` — standalone per-element
///     iterators that scan without full-parse context.
/// - Accessor methods `<field>_clean` and `<field>_raw` on `<Name>Content`
///   for ergonomic span-to-slice conversion.
///
/// # Grammar syntax
///
/// ```text
/// define_parser!(Name {
///     sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
///
///     inline  { ... }
///     lines   { ... }
///     blocks  { ... }
/// });
/// ```
///
/// ## Context bytes (required header, plus one optional)
///
/// | Key         | Meaning                                                        |
/// |-------------|----------------------------------------------------------------|
/// | `sep`       | Word separator (typically space)                               |
/// | `eol`       | Line terminator (typically `\n`)                               |
/// | `tab`       | Tab character                                                  |
/// | `escape`    | Escape prefix that suppresses the next byte                    |
/// | `max_nest`  | Optional. Bounded nesting depth cap forwarded to               |
/// |             | `parse_inline!`'s two stacks — `symmetric` with                |
/// |             | `parse_inside = true; balanced = true;` and                    |
/// |             | `asymmetric` with `balanced = true` and/or                     |
/// |             | `parse_inside = true`. A grammar-wide setting,                 |
/// |             | declared alongside the other context bytes — not               |
/// |             | inside `inline { ... }`.                                       |
/// |             |                                                                |
/// |             | **Absent => `1`**, which reproduces pre-nesting behaviour      |
/// |             | exactly (single pending slot / single outer span, no           |
/// |             | self-nesting). This is the default and is also the fast        |
/// |             | path: at `max_nest = 1`, every grammar rule whose own          |
/// |             | `balanced` and `parse_inside` flags are both `false` skips     |
/// |             | the bounded-stack machinery entirely and runs the original,    |
/// |             | unmodified single-pass scan — there is no per-iteration cost   |
/// |             | from the nesting feature unless a rule actually opts into it.  |
/// |             |                                                                |
/// |             | To opt in to deeper, type-aware nesting (e.g. for `{ [ ] }`    |
/// |             | style structures, or `**bold *italic* bold**`), set it         |
/// |             | explicitly to the deepest level your grammar needs, alongside  |
/// |             | the other context bytes, e.g.:                                 |
/// |             | `sep = ..., eol = ..., tab = ..., escape = ..., max_nest = 4;` |
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
///     // Detects trailing hard-break sequences: `esc` byte OR ≥ `min`
///     // consecutive `sp` bytes at end of line. Emits a zero-length Span.
///
///     on_trigger(b1, b2, ...) {
///         symmetric byte {
///             parse_inside = true | false;
///             balanced     = true | false;
///             N => field [div],   // exact count N of `byte` → Span
///         }
///         asymmetric open, close {
///             balanced     = true | false;
///             parse_inside = true | false;
///             N => field [div],
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
/// Declares whole-line elements. A matching line is consumed entirely; inline
/// scanning is skipped for it.
///
/// ```text
/// lines {
///     line(byte, max = N) |var|: Type { ... } => field [div];
///     // Matches 1–N leading `byte` bytes followed by `sep` or EOL.
///     // `var` receives the count. Produces Vec<(Type, Span)>.
///
///     line_simple(b1 | b2 | ..., min = N) |var|: Type { ... } => field [div];
///     // Matches a line composed entirely of one delimiter byte (interleaved
///     // with `sep`), at least `min` times. `var` receives the delimiter byte.
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
///         fence(byte, min = N) => field [div];
///         // Opens on a line starting with ≥ N `byte` bytes; closes on a
///         // matching fence line. Entire range is one Span. Inline scanning
///         // suppressed while fence is active.
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
///         // Single-line numbered item: digit run followed by byte matching
///         // `end_pat`. `n` receives the parsed number, `k` the delimiter byte.
///         // Produces Vec<(Type, Span)>.
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
/// # Errors
///
/// A malformed grammar emits a located `compile_error!` at the offending token
/// rather than panicking. The error message includes the parsing context
/// (e.g. `"expected literal (fence min)"`).
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

/// Fallible core of [`define_parser`].
fn expand(input: TS2) -> Result<TS2> {
    let mut c = Cursor::new(input);

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

    // `max_nest` is an optional fifth context setting, alongside
    // sep/eol/tab/escape — a grammar-wide value, not specific to `inline`,
    // even though it currently only bounds `parse_inline!`'s two stacks.
    // `, max_nest = N` after `escape`, before the header's closing `;`.
    // Absent ⇒ `1`, which reproduces pre-nesting behaviour exactly.
    let max_nest: Literal = if matches!(bc.peek(), Some(TT::Punct(p)) if p.as_char() == ',') {
        bc.advance();
        bc.named_lit("max_nest")?
    } else {
        Literal::usize_unsuffixed(1)
    };
    bc.skip(';');

    let mut inline_ts = TS2::new();
    let mut lines_ts = TS2::new();
    let mut blocks_ts = TS2::new();

    loop {
        match bc.peek_str().as_deref() {
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

    let mut cf = CF::default();
    collect_inline(inline_ts.clone(), &mut cf)?;
    collect_lines(lines_ts.clone(), &mut cf)?;
    collect_blocks(blocks_ts.clone(), &mut cf)?;

    let pt_inline: Vec<_> = strip(inline_ts).into_iter().collect();
    let pt_lines: Vec<_> = strip(lines_ts).into_iter().collect();
    let pt_blocks: Vec<_> = strip(blocks_ts).into_iter().collect();

    // Resolve the runtime crate path once; used for every qualified macro call
    // in the expansion so it works correctly from any dependent crate.
    let mc = crate_path();

    let dc = build_define_content(&content_name, &cf, &mc);
    let cm = build_content_methods(&content_name, &cf, &sep, &eol, &tab, &esc);
    let dsl = build_standalone_dsl(&sep, &eol, &tab, &esc, &cf.standalone);

    Ok(quote! {
        #dc

        #[allow(missing_docs)]
        pub struct #parser_name;

        #[allow(missing_docs)]
        impl #parser_name {
            pub fn parse(source: &[u8]) -> #content_name<'_> {
                #mc::parse_text!(
                    source;
                    sep = #sep, eol = #eol, tab = #tab, escape = #esc,
                    max_nest = #max_nest;
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
                    memchr(b'*', b'`') {
                        symmetric b'`' {
                            parse_inside = false;
                            balanced = false;
                            1 => codes [80],
                        }
                        symmetric b'*' {
                            parse_inside = true;
                            balanced = false;
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
                inline { memchr(b'*') { symmetric { 1 => bolds [40], } } }
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
        assert!(e.contains("fence min"));
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
}
