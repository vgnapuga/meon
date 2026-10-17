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

/// Classify a scalar by its first byte: digits and a leading `-` are numbers,
/// `t`/`f`/`n` the three keyword constants. A first byte of `"`, `{`, `[` (a
/// string or container) — or anything else — yields `None`, so such values
/// are never mis-typed.
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

/// Whitespace trimmed from both ends of a value before it is classified.
#[inline]
fn is_ws(b: u8) -> bool {
    b == b' ' || b == b'\t' || b == b'\n' || b == b'\r'
}

impl JsonContent<'_> {
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
    /// Shared by member values, top-level scalars and array elements.
    ///
    /// Panics if `start..end` is not a valid range of `src`, like slice
    /// indexing does.
    #[inline]
    fn process_scalar(
        src: &[u8],
        start: usize,
        end: usize,
        visit: &mut impl FnMut(ScalarKind, Span),
    ) {
        let seg = &src[start..end];
        let Some(lead) = seg.iter().position(|&b| !is_ws(b)) else {
            return;
        };
        if let Some(k) = classify(seg[lead]) {
            let trail = seg.iter().rposition(|&b| !is_ws(b)).map_or(lead, |p| p + 1);
            visit(k, Span::new((start + lead) as u32, (start + trail) as u32));
        }
    }

    /// Split one array's content on top-level commas and call `visit` for
    /// each element whose first byte classifies. `arr` is content-only
    /// (brackets excluded), so `[arr.start, arr.end)` IS the interior.
    ///
    /// Panics if `arr` is not a valid range of `src`, like slice indexing
    /// does.
    fn for_each_array_element(src: &[u8], arr: &Span, visit: &mut impl FnMut(ScalarKind, Span)) {
        let base = arr.start as usize;
        let end = arr.end as usize;
        if base >= end {
            return;
        }
        let inner = &src[base..end];

        let mut seg_start = 0;
        let mut depth: u32 = 0;
        let mut i = 0;

        while i < inner.len() {
            match inner[i] {
                b'"' => {
                    i += 1;
                    while i < inner.len() {
                        match inner[i] {
                            b'\\' => i += 2,
                            b'"' => {
                                i += 1;
                                break;
                            }
                            _ => i += 1,
                        }
                    }
                }
                b'{' | b'[' => {
                    depth += 1;
                    i += 1;
                }
                b'}' | b']' => {
                    depth = depth.saturating_sub(1);
                    i += 1;
                }
                b',' => {
                    if depth == 0 {
                        Self::process_scalar(src, base + seg_start, base + i, visit);
                        seg_start = i + 1;
                    }
                    i += 1;
                }
                _ => i += 1,
            }
        }
        Self::process_scalar(src, base + seg_start, end, visit);
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
