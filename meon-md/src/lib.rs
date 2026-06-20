// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2025 Nikita Shavrin

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
//! # Known limitations
//!
//! - A blockquote containing a fenced code block (`> \`\`\``) produces an
//!   incorrect span due to the single-slot active block state.
//! - Nested blockquotes (`> >`) leak content into the outer span.
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
