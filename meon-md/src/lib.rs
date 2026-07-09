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
//! **Block:** fenced code blocks (`` ``` ``/`~~~`), blockquotes (`>`),
//! bullet lists (`-`, `*`, `+`), ordered lists (`1.`, `1)`), paragraphs.
//!
//! # Nesting
//!
//! This grammar sets `max_nest = 4`, the engine's bounded-nesting depth cap
//! (see [`meon::define_parser!`] for the mechanism). Two independent rule
//! families opt into it:
//!
//! - **Blockquotes and fences** (`cont(b'>')`, `fence(b'`', min = 3)`) share
//!   the block-level active-block stack. A line like `"> > text"` opens two
//!   distinct, properly contained `blockquotes` spans rather than one; a
//!   fenced code block opened on a continuation line inside a blockquote
//!   (`"> ```\n> code\n> ```"`) is scoped to its own fence span without
//!   absorbing the surrounding `>` markers.
//! - **Bold and italic** (`symmetric b'*' { parse_inside = true; balanced =
//!   true; ... }`) share the inline bounded stack. `"**bold *italic*
//!   still-bold**"` resolves both the outer bold and the inner italic as
//!   separate, correctly-bounded spans — a different-count inner delimiter
//!   no longer overwrites the engine's single pending slot and silently
//!   loses the outer pair.
//!
//! `max_nest = 4` means up to four such levels self-nest correctly per
//! family; a fifth level of the same construct collapses via the engine's
//! overflow counter (see [`meon::parse_inline!`] and [`meon::parse_block!`]
//! for the exact behaviour at the cap).
//!
//! Autolinks and the `[text](url)` / `![alt](url)` link/image syntax remain
//! `balanced = false, parse_inside = false` by design — they use the
//! original single-pass forward search and do not participate in nesting
//! (e.g. `[a [b] c](url)` does not nest its own brackets).
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
    max_nest = 4;

    inline {
        merge_simple = true;

        hard_break(b'\\', b' ', 2) => hard_breaks [500];
        on_trigger(b'*', b'`', b'[', b'<') {
            symmetric b'`' {
                parse_inside = false;
                balanced     = false;
                1 => codes [80],
            }
            symmetric b'*' {
                parse_inside = true;
                balanced     = true;
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
                    balanced     = false;
                } => text,
                | b'(', b')' | {
                    parse_inside = false;
                    balanced     = false;
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
