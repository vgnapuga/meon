//! On-demand scalar typing for [`crate::JsonContent`] — see `lib.rs`'s own
//! module doc for why this is a separate post-pass rather than something the
//! engine does while parsing.

use crate::JsonContent;
use meon::span::Span;

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

/// Trim trailing `sep` / `tab` / `\n` / `\r` from the end of `[start, end)`
/// over `src`.
///
/// # Safety invariant relied on by every `get_unchecked` in this file
///
/// Every index used here comes from a [`Span`] produced by the parser, and
/// the engine bounds every span it emits by the source length throughout
/// parsing — a span's `end` is therefore never greater than `src.len()`.
/// Every `get_unchecked` call below is additionally guarded by the same
/// `< end` / `> start` condition a safe `src[i]` would have checked at
/// runtime; removing the check only removes a redundant re-verification of
/// a bound the adjacent loop condition already enforces.
#[inline]
fn trim_end(src: &[u8], start: usize, mut end: usize) -> usize {
    while end > start {
        // SAFETY: `end > start` just checked; `end <= src.len()` by the
        // span invariant above, so `end - 1` is in bounds.
        let b = unsafe { *src.get_unchecked(end - 1) };
        if b != b' ' && b != b'\t' && b != b'\n' && b != b'\r' {
            break;
        }
        end -= 1;
    }
    end
}

impl<'a> JsonContent<'a> {
    /// Type every member value, every array element, AND every bare
    /// top-level value, in one pass, returning owned span vectors.
    pub fn type_scalars(&self) -> TypedScalars {
        let mut out = TypedScalars::default();
        self.for_each_scalar(|k, span| push_kind(&mut out, k, span));
        out
    }

    /// Type only the values/elements that route to a single [`ScalarKind`].
    #[inline]
    pub fn type_field(&self, want: ScalarKind) -> Vec<Span> {
        let mut v = Vec::with_capacity(self.members.len() / 7);
        self.for_each_scalar(|k, span| {
            if k == want {
                v.push(span);
            }
        });
        v
    }

    /// Visit every scalar leaf this content contains — member values, array
    /// elements, then bare top-level values, in that order.
    fn for_each_scalar(&self, mut visit: impl FnMut(ScalarKind, Span)) {
        let src = self.source;

        for m in &self.members {
            Self::process_scalar(
                src,
                m.value.start as usize,
                m.value.end as usize,
                &mut visit,
            );
        }

        for arr in &self.arrays {
            Self::for_each_array_element(src, arr, &mut visit);
        }

        for sc in &self.scalars {
            Self::process_scalar(src, sc.start as usize, sc.end as usize, &mut visit);
        }
    }

    /// Trim and classify one segment, visiting it if its first byte types.
    /// Unifies member values, top-level scalars, and array elements into a
    /// single inlineable path.
    ///
    /// An `#[inline(always)]` variant was tried here to test whether the
    /// regression measured on `wide_strings` (member/scalar/array-element
    /// classification consolidated into this one shared function, instead
    /// of duplicated separately) was the inlining heuristic declining to
    /// inline this at all three call sites. Measured, not assumed: forcing
    /// it made `wide_strings` *worse*, not better — ruling out "not enough
    /// inlining" as the cause, since more of it moved further in the wrong
    /// direction. Left at plain `#[inline]`; the actual mechanism for the
    /// `wide_strings`-specific regression is not confirmed.
    #[inline]
    fn process_scalar(
        src: &[u8],
        mut start: usize,
        end: usize,
        visit: &mut impl FnMut(ScalarKind, Span),
    ) {
        while start < end {
            // SAFETY: `start < end <= src.len()` (span invariant).
            let b = unsafe { *src.get_unchecked(start) };
            if b != b' ' && b != b'\t' && b != b'\n' && b != b'\r' {
                break;
            }
            start += 1;
        }

        if start < end {
            // SAFETY: `start < end <= src.len()`.
            if let Some(k) = classify(unsafe { *src.get_unchecked(start) }) {
                let e = trim_end(src, start, end);
                visit(k, Span::new(start as u32, e as u32));
            }
        }
    }

    /// Split one array's content on top-level commas and call `visit` for
    /// each element whose first byte classifies. `arr` is content-only
    /// (brackets excluded), so `[arr.start, arr.end)` IS the interior.
    fn for_each_array_element(src: &[u8], arr: &Span, visit: &mut impl FnMut(ScalarKind, Span)) {
        let inner_start = arr.start as usize;
        let inner_end = arr.end as usize;
        if inner_start >= inner_end {
            return;
        }

        let mut seg_start = inner_start;
        let mut depth: u32 = 0;
        let mut i = inner_start;

        while i < inner_end {
            // SAFETY: `i < inner_end <= src.len()` (span invariant above).
            let b = unsafe { *src.get_unchecked(i) };
            match b {
                b'"' => {
                    i += 1;
                    while i < inner_end {
                        // SAFETY: `i < inner_end <= src.len()`.
                        let sb = unsafe { *src.get_unchecked(i) };
                        if sb == b'\\' {
                            i += 2;
                            continue;
                        }
                        if sb == b'"' {
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
                    if depth > 0 {
                        depth -= 1;
                    }
                    i += 1;
                }
                b',' => {
                    if depth == 0 {
                        Self::process_scalar(src, seg_start, i, visit);
                        seg_start = i + 1;
                    }
                    i += 1;
                }
                _ => {
                    i += 1;
                }
            }
        }
        Self::process_scalar(src, seg_start, inner_end, visit);
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
