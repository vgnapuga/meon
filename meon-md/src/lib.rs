// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Nikita Shavrin

//! A fast flat parser for a subset of Markdown, built on the [`meon`] engine.
//!
//! This crate is a demonstration grammar: it shows how [`meon::define_parser!`]
//! can describe a non-trivial text format in a single declarative block and
//! produce a high-performance parser with no hand-written scanning code.
//!
//! # Usage
//!
//! ```rust
//! use meon_md::MarkdownParser;
//!
//! let content = MarkdownParser::parse(b"# Hello\n**world**\n");
//! assert_eq!(content.headings.len(), 1);
//! assert_eq!(content.bolds.len(), 1);
//! ```
//!
//! # Supported elements
//!
//! **Inline:** plain text, bold (`**`), italic (`*`), bold-italic (`***`),
//! inline code (`` ` ``), links (`[text](url)`), images (`![alt](url)`),
//! autolinks (`<url>`), hard breaks.
//!
//! **Line:** ATX headings (`#`–`######`), thematic breaks (`---`, `***`, `___`).
//!
//! **Block:** fenced code blocks (`` ``` ``), blockquotes (`>`), bullet lists
//! (`-`, `*`, `+`), ordered lists (`1.`, `1)`), paragraphs.
//!
//! Lines end at `\n` or `\r\n` (`crlf = true`). A line of spaces and tabs
//! alone is blank.
//!
//! # Limits
//!
//! The grammar sets `max_nest = 4` in its `limits` section, the engine's
//! bounded-nesting depth cap (see [`meon::define_parser!`] for the mechanism).
//! The three span caps — `max_inline_span`, `max_line_span` and
//! `max_block_span` — are written as `0`, which the engine treats as absent:
//! no span is dropped for its length.
//!
//! Two independent rule families opt into `max_nest`:
//!
//! - **Blockquotes and fences** (`cont(b'>')`, `fence(b'`', min = 3)`) share
//!   the block-level active-block stack. A line like `"> > text"` opens two
//!   distinct, properly contained `blockquotes` spans rather than one; a
//!   fenced code block opened on a continuation line inside a blockquote
//!   (`"> ```\n> code\n> ```"`) is scoped to its own fence span without
//!   absorbing the surrounding `>` markers.
//! - **Bold and italic** (`symmetric b'*' { parse_inside = true; ... }`) share
//!   the inline bounded stack. `"**bold *italic* still-bold**"` resolves both
//!   the outer bold and the inner italic as separate, correctly-bounded
//!   spans.
//!
//! `max_nest = 4` means up to four such levels self-nest correctly per
//! family. Past the cap, another emphasis opener remains literal and gets
//! no frame (see [`meon::parse_inline!`]); a fifth block level does not open,
//! and its marker is content of the innermost frame
//! (see [`meon::parse_block!`]).
//!
//! Autolinks are `balanced = false, parse_inside = false`: a forward search
//! for the first `>`. The text and url of `[text](url)` / `![alt](url)` are
//! `balanced = true, parse_inside = false`: brackets and parentheses inside
//! them are counted as bytes, so `[a [b] c](url)` and
//! `[t](https://x.y/a_(b))` are one link each, and nothing else is parsed
//! inside them.
//!
//! The lines of one blockquote are one inline run, as the lines of a
//! paragraph are: `"> **a\n> b**"` is one bold span.
//!
//! # Known limitations
//!
//! - Emphasis precedence rules are not enforced; declaration order wins.
//! - This is not a CommonMark-compliant implementation.
pub mod types;

use meon::define_parser;
pub use meon::span;
use std::num::NonZeroU8;

use types::{
    block::{BulletItem, OrderedItem},
    inline::Link,
    line::{Heading, ThematicBreak},
};

define_parser!(Markdown {
    sep = b' ',
    eol = b'\n',
    tab = b'\t',
    escape = b'\\',
    crlf = true;

    limits {
        max_nest = 4;
        max_inline_span = 0;
        max_line_span = 0;
        max_block_span = 0;
    }

    inline {
        merge_simple = true;

        hard_break(b'\\', b' ', 2) => hard_breaks [500];
        on_trigger(b'*', b'`', b'[', b'<') {
            symmetric b'`' {
                parse_inside = false;
                1 => codes [80],
            }
            symmetric b'*' {
                parse_inside = true;
                1 => italics [40], 2 => bolds [40], 3 => bold_italics [80],
            }
            asymmetric b'<', b'>' {
                balanced     = false;
                parse_inside = false;
                1 => autolinks [100],
            }
            chained: Link {
                | b'[', b']' | {
                    parse_inside = false;
                    balanced     = true;
                } => text,
                | b'(', b')' | {
                    parse_inside = false;
                    balanced     = true;
                } => url,
                prefix | b'!' | => is_image,
            } => links [100]
        }
        fallback => texts [10];
    }
    lines {
        line(b'#', max = 6) |n|:
            Heading { level: NonZeroU8::new(n).unwrap_or(NonZeroU8::MIN) }
            => headings [200];
        line_simple(b'-' | b'*' | b'_', min = 3) |b|:
            ThematicBreak { kind: b }
            => thematic_breaks [200];
    }
    blocks {
        block_simple {
            fence(b'`', min = 3) => fenced_codes [400];
            cont(b'>')           => blockquotes [200];
        }
        block {
            (b'-' | b'*' | b'+') |b|:
                BulletItem { kind: b }
                => bullet_items [80];
            num(b'0'..=b'9', end = b'.' | b')') |n, k|:
                OrderedItem { kind: k, num: n }
                => ordered_items [80];
        }
        fallback => paragraphs [80];
    }
});
