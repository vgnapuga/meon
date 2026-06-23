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
//! Earlier versions routed each member value and array element to a typed
//! field (`trues` / `falses` / `nulls` / `nums`) *inside* the engine's hot
//! loop, via a `scalar { ... }` grammar rule. That cost a push per value on
//! the critical path for a feature most callers do not need on every parse —
//! and it baked a JSON-specific concern (first-byte type tagging) into the
//! generic engine.
//!
//! Typing now lives entirely outside the engine, as methods on
//! [`JsonContent`]. The engine emits only structure (`objects`, `arrays`,
//! `strings`, `members`, `scalars`); the caller asks for typing when — and
//! only when — it wants it:
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
use meon::span::Span;

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

/// Typed scalar projection produced on demand by [`JsonContent::type_scalars`]
/// (or one field at a time by [`JsonContent::type_field`]).
///
/// Each vector holds spans into the original source, byte-equal to the
/// member-value / array-element interval they were typed from (whitespace
/// trimmed). Empty vectors mean "no value of that type was present", and cost
/// nothing until `type_scalars` is called.
#[derive(Debug, Default, Clone)]
pub struct TypedScalars {
    pub nums: Vec<Span>,
    pub trues: Vec<Span>,
    pub falses: Vec<Span>,
    pub nulls: Vec<Span>,
}

/// The kind a scalar's first byte routes to. Returned by the classifier so
/// `type_scalars` and `type_field` share one first-byte decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarKind {
    Num,
    True,
    False,
    Null,
}

/// Classify a scalar by its first byte, exactly as the old in-engine `scalar`
/// table did: digits and a leading `-` are numbers, `t`/`f`/`n` the three
/// keyword constants. A first byte of `"`, `{`, `[` (a string or container) —
/// or anything else — yields `None`, so such values are never mis-typed.
#[inline]
fn classify(first: u8) -> Option<ScalarKind> {
    match first {
        b'0'..=b'9' | b'-' => Some(ScalarKind::Num),
        b't' => Some(ScalarKind::True),
        b'f' => Some(ScalarKind::False),
        b'n' => Some(ScalarKind::Null),
        _ => None,
    }
}

/// Trim `sep` / `tab` / `\n` / `\r` from both ends of `[start, end)` over
/// `src`, returning the tightened bounds (or `start == end` if all whitespace).
#[inline]
fn trim(src: &[u8], mut start: usize, mut end: usize) -> (usize, usize) {
    while start < end && matches!(src[start], b' ' | b'\t' | b'\n' | b'\r') {
        start += 1;
    }
    while end > start && matches!(src[end - 1], b' ' | b'\t' | b'\n' | b'\r') {
        end -= 1;
    }
    (start, end)
}

impl<'a> JsonContent<'a> {
    /// Type every member value, every array element, AND every bare
    /// top-level value (`self.scalars` — a value with no enclosing object or
    /// array, e.g. a whole document that is just `42` or `true`), in one
    /// pass, returning owned span vectors. Strings and containers are
    /// skipped (their first byte is `"` / `{` / `[`, which classifies to
    /// `None`), so only bare scalar leaves are emitted — the same set the
    /// old in-engine `scalar` rule produced.
    ///
    /// Member values come straight from `self.members` (one classify per
    /// member). Array elements are recovered by splitting each `arrays` span's
    /// interior on top-level commas; nested containers and quoted strings are
    /// skipped so their inner commas/bytes are not mistaken for element
    /// boundaries or scalars. Top-level scalars come straight from
    /// `self.scalars`, the same way member values do.
    pub fn type_scalars(&self) -> TypedScalars {
        let src = self.source;
        let mut out = TypedScalars::default();

        // --- member values ---
        for m in &self.members {
            let (s, e) = trim(src, m.value.start as usize, m.value.end as usize);
            if s < e {
                if let Some(k) = classify(src[s]) {
                    push_kind(&mut out, k, Span::new(s as u32, e as u32));
                }
            }
        }

        // --- array elements ---
        for arr in &self.arrays {
            self.type_array_elements(arr, &mut out);
        }

        // --- bare top-level values (no enclosing container) ---
        for sc in &self.scalars {
            let (s, e) = trim(src, sc.start as usize, sc.end as usize);
            if s < e {
                if let Some(k) = classify(src[s]) {
                    push_kind(&mut out, k, Span::new(s as u32, e as u32));
                }
            }
        }

        out
    }

    /// Type only the values/elements that route to a single [`ScalarKind`],
    /// returning just that one vector. Useful when a caller wants, say, only
    /// the numbers and does not want to allocate the other three vectors.
    /// Traverses the same three sources as [`type_scalars`], in the same
    /// order — members, then array elements, then bare top-level scalars —
    /// so the result is exactly `type_scalars().<field>` for the matching
    /// kind, just without allocating the other three vectors.
    pub fn type_field(&self, want: ScalarKind) -> Vec<Span> {
        let src = self.source;
        let mut v = Vec::new();

        for m in &self.members {
            let (s, e) = trim(src, m.value.start as usize, m.value.end as usize);
            if s < e && classify(src[s]) == Some(want) {
                v.push(Span::new(s as u32, e as u32));
            }
        }

        // Reuse the full element splitter, then keep only the wanted kind.
        let mut tmp = TypedScalars::default();
        for arr in &self.arrays {
            self.type_array_elements(arr, &mut tmp);
        }
        let extra = match want {
            ScalarKind::Num => tmp.nums,
            ScalarKind::True => tmp.trues,
            ScalarKind::False => tmp.falses,
            ScalarKind::Null => tmp.nulls,
        };
        v.extend(extra);

        for sc in &self.scalars {
            let (s, e) = trim(src, sc.start as usize, sc.end as usize);
            if s < e && classify(src[s]) == Some(want) {
                v.push(Span::new(s as u32, e as u32));
            }
        }

        v
    }

    /// Split one array span's content on top-level commas and classify each
    /// element's first byte. `arr` is `self.arrays[i]`, which is **already
    /// content-only** — the engine's universal asymmetric-field convention
    /// excludes the brackets themselves (see the doc comment at the top of
    /// this file) — so `[arr.start, arr.end)` IS the interior; no further
    /// trimming of a byte from each end is needed or correct here. A depth
    /// counter over `{}`/`[]` plus a string-skip keeps nested containers and
    /// quoted commas from being seen as element separators; an element whose
    /// first byte is `"`/`{`/`[` (string or nested container) classifies to
    /// `None` and is skipped, so only bare leaf scalars are emitted —
    /// matching the old in-engine behaviour exactly.
    fn type_array_elements(&self, arr: &Span, out: &mut TypedScalars) {
        let src = self.source;
        let inner_start = arr.start as usize;
        let inner_end = arr.end as usize;
        if inner_start >= inner_end {
            return;
        }

        let mut seg_start = inner_start;
        let mut depth: i32 = 0;
        let mut i = inner_start;
        while i < inner_end {
            let b = src[i];
            match b {
                b'"' => {
                    // Skip a quoted string wholesale (escape-aware), so a comma
                    // or bracket inside it is not treated as structural.
                    i += 1;
                    while i < inner_end {
                        if src[i] == b'\\' {
                            i += 2;
                            continue;
                        }
                        if src[i] == b'"' {
                            i += 1;
                            break;
                        }
                        i += 1;
                    }
                }
                b'{' | b'[' => {
                    depth += 1;
                    i += 1;
                }
                b'}' | b']' => {
                    depth -= 1;
                    i += 1;
                }
                b',' if depth == 0 => {
                    Self::emit_element(src, seg_start, i, out);
                    seg_start = i + 1;
                    i += 1;
                }
                _ => {
                    i += 1;
                }
            }
        }
        // Final element, from the last comma (or interior start) to the close.
        Self::emit_element(src, seg_start, inner_end, out);
    }

    /// Trim and classify one array-element segment, pushing it to the matching
    /// vector if its first byte types. Empty (all-whitespace, e.g. a `,,` gap)
    /// or string/container-leading segments are skipped.
    #[inline]
    fn emit_element(src: &[u8], start: usize, end: usize, out: &mut TypedScalars) {
        let (s, e) = trim(src, start, end);
        if s < e {
            if let Some(k) = classify(src[s]) {
                push_kind(out, k, Span::new(s as u32, e as u32));
            }
        }
    }
}

/// Route a classified span to its vector.
#[inline]
fn push_kind(out: &mut TypedScalars, k: ScalarKind, span: Span) {
    match k {
        ScalarKind::Num => out.nums.push(span),
        ScalarKind::True => out.trues.push(span),
        ScalarKind::False => out.falses.push(span),
        ScalarKind::Null => out.nulls.push(span),
    }
}
