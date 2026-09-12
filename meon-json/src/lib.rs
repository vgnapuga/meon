// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Nikita Shavrin

//! `meon-json` — a flat, span-based JSON reader built on the `meon` engine.
//!
//! The grammar produces a *table of intervals*, not a tree. Structure is
//! recovered by interval containment over the source, exactly as elsewhere in
//! `meon`:
//!
//! - `objects` / `arrays` — one span per container, **content only**. The
//!   brackets themselves are excluded: this is the same delimiter-excluded
//!   convention every asymmetric inline field in this engine follows (e.g.
//!   Markdown's `bolds` / `italics` store the text inside `**`/`*`, not the
//!   markers themselves). The bracket-inclusive raw slice is available via
//!   the generated `objects_raw()` / `arrays_raw()` accessors. Note this
//!   differs from `members[i].value` when the value *is* a container —
//!   that field is the raw, unprocessed value span and stays bracket-
//!   inclusive, byte-equal to the corresponding `_raw()` form, not to the
//!   bare `objects`/`arrays` span.
//! - `strings` — one span per `"..."` run (the unescaped content lives between
//!   the quotes; the raw span includes them).
//! - `members` — one `Member { key, value }` per `key: value` pair. `key` is
//!   the raw key span (quotes included for a quoted key); `value` is the raw
//!   value span (brackets/quotes included when the value is a container or
//!   string).
//! - `scalars` — top-level fallback: bare text outside any container.
//!
//! ## Scalar typing is a *post-pass*, not part of the engine
//!
//! The engine emits only structure (`objects`, `arrays`, `strings`,
//! `members`, `scalars`). Typing lives outside it, as methods on
//! [`JsonContent`]; the caller asks for it when it wants it:
//!
//! ```ignore
//! let c = JsonParser::parse(input);
//! let typed = c.type_scalars();   // one cache-friendly pass, owned vectors out
//! // typed.nums / typed.trues / typed.falses / typed.nulls : Vec<Span>
//! ```
//!
//! This mirrors the on-demand model of simd-json's second stage: structure is
//! found once, values are materialised by type only on request. Nothing is
//! written back into [`JsonContent`] — it stays an immutable record of exactly
//! what the engine saw, and a caller that never types pays nothing (no empty
//! `nums`/`trues`/... fields sitting in the struct, no allocation).
//!
//! `type_scalars` / `type_field` classify **three** sources by first byte:
//! member values, array elements, and bare top-level values (`scalars` — a
//! document with no enclosing object or array at all, e.g. just `42` or
//! `true`). All three are trimmed and routed the same way; none of the three
//! is special-cased into a separate API. A typed span is byte-equal to the
//! value/element/scalar span it was routed from (after trimming surrounding
//! whitespace), so it still projects back onto the same interval; the type is
//! just "which vector it landed in".

use meon::define_parser;

/// One `"key": value` pair. Both fields are raw source spans; recover typed or
/// unescaped content by interval containment against the other fields.
pub struct Member {
    pub key: meon::span::Span,
    pub value: meon::span::Span,
}

define_parser!(Json {
    sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\', max_nest = 64;
    inline {
        merge_simple = false;
        // The close bytes `}` and `]` MUST be in this trigger set: the engine
        // finds container closes with the same `find_any` scan that finds the
        // opens, so a close byte absent from the set is never seen and the
        // container never closes. (`,` is the key_value `end` separator and is
        // auto-added to the trigger set by the engine from the `key_value`
        // rule's `end =` — it need not be listed here.)
        on_trigger(b'{', b'}', b'[', b']', b'"', b':') {
            symmetric b'"' {
                parse_inside = false;
                balanced     = false;
                1 => strings [16],
            }
            asymmetric b'{', b'}' {
                balanced     = true;
                parse_inside = true;
                1 => objects [32],
            }
            asymmetric b'[', b']' {
                balanced     = true;
                parse_inside = true;
                1 => arrays [32],
            }
            key_value: Member {
                eq        = b':';
                allow_sep = true;
                end       = b',';
                key   => key,
                value => value,
            } => members [16]
        }
        fallback => scalars [16];
    }
    blocks {
        fallback => loose [256];
    }
});

mod typing;
pub use typing::{ScalarKind, TypedScalars};
