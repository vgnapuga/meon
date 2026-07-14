//! Context-aware standalone iterators: [`ContextSymmetricExactIter`] and
//! [`ContextAsymmetricExactIter`].
//!
//! Each is the same streaming matcher as its context-free sibling
//! ([`super::symmetric::SymmetricExactIter`] /
//! [`super::asymmetric::AsymmetricExactIter`]) with two context-driven
//! additions:
//!
//! - a candidate delimiter whose position lies inside a
//!   [`ParseContext`](super::context::ParseContext) opaque region is skipped,
//!   and the scan resumes after that region;
//! - a *multi-line* opaque region (a fenced block — inline regions never
//!   contain an `eol`) encountered during the close search aborts the pending
//!   opener: a block construct ends the paragraph in the full parse, so a
//!   pair never spans a fence.
//!
//! Like the context-free siblings, matching is paragraph-bounded: a pair may
//! span single line breaks, and an empty line (two consecutive `eol` bytes)
//! or the end of input aborts a pending opener.
//!
//! The context suppresses **candidate positions**, not enclosing spans: a
//! returned span may legally *contain* an opaque region (`**a `code` b**`
//! still yields one bold span), exactly as in the full parse.
//!
//! Line-family rules (`line`, `line_simple`, `cont`, `block`, `num`) do not
//! need dedicated context-aware iterators: their matches are
//! position-independent per line, so the generated `find_context_*` methods
//! reuse the context-free iterator and post-filter items whose span start is
//! covered — which is candidate-exact for whole-line constructs.

use super::common::{Span, count_escape};
use super::context::{ContextCursor, ParseContext};

/// Context-aware variant of [`super::symmetric::SymmetricExactIter`].
///
/// Obtained via the generated `Parser::find_context_*(source, &ctx)` methods;
/// rarely constructed directly.
pub struct ContextSymmetricExactIter<'a> {
    src: &'a [u8],
    byte: u8,
    count: u32,
    eol: u8,
    escape: u8,
    pos: usize,
    cur: ContextCursor<'a>,
}

impl<'a> ContextSymmetricExactIter<'a> {
    /// Create a context-aware symmetric exact-delimiter iterator. Parameters
    /// as in the context-free sibling, plus the prebuilt `ctx` (borrowed for
    /// the iterator's lifetime).
    pub fn new(
        src: &'a [u8],
        byte: u8,
        count: u32,
        eol: u8,
        escape: u8,
        ctx: &'a ParseContext,
    ) -> Self {
        Self {
            src,
            byte,
            count,
            eol,
            escape,
            pos: 0,
            cur: ctx.cursor(),
        }
    }
}

impl Iterator for ContextSymmetricExactIter<'_> {
    type Item = Span;

    fn next(&mut self) -> Option<Span> {
        let src = self.src;
        let len = src.len();
        loop {
            // Open scan: one streaming pass for the delimiter byte; covered
            // candidates are skipped past their whole region (this is what
            // steps over fenced blocks in one jump).
            let p = memchr::memchr(self.byte, &src[self.pos..])? + self.pos;

            if let Some(ce) = self.cur.covering_end(p) {
                self.pos = ce;
                continue;
            }

            if count_escape(src, p, self.escape) % 2 == 1 {
                self.pos = p + 1;
                continue;
            }

            let mut c = 0u32;
            let mut end = p;
            while end < len && src[end] == self.byte {
                c += 1;
                end += 1;
            }

            if c != self.count {
                self.pos = end;
                continue;
            }

            // Close scan, paragraph-bounded and opaque-aware. Fork the cursor
            // for the lookahead: close candidates are `>= p`, for which the
            // parent cursor's index is already valid.
            let cs = end;
            let mut j = cs;
            let mut ccur = self.cur;
            let close = loop {
                let Some(r) = memchr::memchr2(self.byte, self.eol, &src[j..]) else {
                    break None;
                };
                let q = j + r;
                if src[q] == self.eol {
                    if q + 1 >= len || src[q + 1] == self.eol {
                        break None;
                    }
                    // A region opening at the next line start is either a
                    // fenced block (multi-line: the paragraph ends, the
                    // opener dies) or an inline region leading the line
                    // (skip it wholesale).
                    if let Some(ce) = ccur.covering_end(q + 1) {
                        if memchr::memchr(self.eol, &src[q + 1..ce]).is_some() {
                            break None;
                        }
                        j = ce;
                        continue;
                    }
                    j = q + 1;
                    continue;
                }
                if let Some(ce) = ccur.covering_end(q) {
                    j = ce;
                    continue;
                }
                let mut cc = 0u32;
                let mut tmp = q;
                while tmp < len && src[tmp] == self.byte {
                    cc += 1;
                    tmp += 1;
                }
                if cc == self.count {
                    break Some((q, tmp));
                }
                j = tmp;
            };

            match close {
                Some((cp, tmp)) => {
                    self.pos = tmp;
                    return Some(Span::new(cs as u32, cp as u32));
                }
                None => {
                    self.pos = end;
                }
            }
        }
    }
}

/// Context-aware variant of [`super::asymmetric::AsymmetricExactIter`].
///
/// Obtained via the generated `Parser::find_context_*(source, &ctx)` methods;
/// rarely constructed directly.
pub struct ContextAsymmetricExactIter<'a> {
    src: &'a [u8],
    open: u8,
    close: u8,
    count: u32,
    eol: u8,
    escape: u8,
    pos: usize,
    cur: ContextCursor<'a>,
}

impl<'a> ContextAsymmetricExactIter<'a> {
    /// Create a context-aware asymmetric exact-delimiter iterator. Parameters
    /// as in the context-free sibling, plus the prebuilt `ctx`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        src: &'a [u8],
        open: u8,
        close: u8,
        count: u32,
        eol: u8,
        escape: u8,
        ctx: &'a ParseContext,
    ) -> Self {
        Self {
            src,
            open,
            close,
            count,
            eol,
            escape,
            pos: 0,
            cur: ctx.cursor(),
        }
    }
}

impl Iterator for ContextAsymmetricExactIter<'_> {
    type Item = Span;

    fn next(&mut self) -> Option<Span> {
        let src = self.src;
        let len = src.len();
        loop {
            let p = memchr::memchr(self.open, &src[self.pos..])? + self.pos;

            if let Some(ce) = self.cur.covering_end(p) {
                self.pos = ce;
                continue;
            }

            if count_escape(src, p, self.escape) % 2 == 1 {
                self.pos = p + 1;
                continue;
            }

            let mut c = 0u32;
            let mut end = p;
            while end < len && src[end] == self.open {
                c += 1;
                end += 1;
            }

            if c != self.count {
                self.pos = end;
                continue;
            }

            let cs = end;
            let mut j = cs;
            let mut ccur = self.cur;
            let close = loop {
                let Some(r) = memchr::memchr2(self.close, self.eol, &src[j..]) else {
                    break None;
                };
                let q = j + r;
                if src[q] == self.eol {
                    if q + 1 >= len || src[q + 1] == self.eol {
                        break None;
                    }
                    if let Some(ce) = ccur.covering_end(q + 1) {
                        if memchr::memchr(self.eol, &src[q + 1..ce]).is_some() {
                            break None;
                        }
                        j = ce;
                        continue;
                    }
                    j = q + 1;
                    continue;
                }
                if let Some(ce) = ccur.covering_end(q) {
                    j = ce;
                    continue;
                }
                break Some(q);
            };

            match close {
                Some(cp) => {
                    self.pos = cp + 1;
                    return Some(Span::new(cs as u32, cp as u32));
                }
                None => {
                    self.pos = end;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn md_context(src: &[u8]) -> ParseContext {
        ParseContext::build(
            src,
            b'\n',
            b'\\',
            b' ',
            b'\t',
            &[(b'`', 3)],
            &[(b'`', 1)],
            &[(b'<', b'>', 1)],
            0,
        )
    }

    // 01. Empty context: identical behaviour to the context-free iterator
    #[test]
    fn test_01_empty_context_matches_context_free() {
        let src = b"a **b** c **d**";
        let ctx = ParseContext::default();
        let aware: Vec<_> =
            ContextSymmetricExactIter::new(src, b'*', 2, b'\n', b'\\', &ctx).collect();
        let free: Vec<_> = crate::SymmetricExactIter::new(src, b'*', 2, b'\n', b'\\').collect();
        assert_eq!(aware, free);
    }

    // 02. A bold pair whose open sits inside a code span is not matched
    #[test]
    fn test_02_open_inside_code_span_skipped() {
        let src = b"`**` not bold, **real**";
        let ctx = md_context(src);
        let spans: Vec<_> =
            ContextSymmetricExactIter::new(src, b'*', 2, b'\n', b'\\', &ctx).collect();
        assert_eq!(spans, vec![Span::new(17, 21)]);
    }

    // 03. A close candidate inside an opaque region is skipped and the true
    //     close (after the region) is found
    #[test]
    fn test_03_close_inside_opaque_region_skipped() {
        let src = b"**a `**` b**";
        let ctx = md_context(src);
        let spans: Vec<_> =
            ContextSymmetricExactIter::new(src, b'*', 2, b'\n', b'\\', &ctx).collect();
        // The `**` inside backticks is covered; the pair closes at the end,
        // containing the code span — exactly the full-parse shape.
        assert_eq!(spans, vec![Span::new(2, 10)]);
    }

    // 04. Everything inside a fenced block is suppressed
    #[test]
    fn test_04_fence_suppression() {
        let src = b"**out**\n```\n**in**\n```\n**out2**";
        let ctx = md_context(src);
        let spans: Vec<_> =
            ContextSymmetricExactIter::new(src, b'*', 2, b'\n', b'\\', &ctx).collect();
        assert_eq!(spans, vec![Span::new(2, 5), Span::new(25, 29)]);
    }

    // 05. Unclosed fence suppresses to end of input
    #[test]
    fn test_05_unclosed_fence() {
        let src = b"**a**\n```\n**never**";
        let ctx = md_context(src);
        let spans: Vec<_> =
            ContextSymmetricExactIter::new(src, b'*', 2, b'\n', b'\\', &ctx).collect();
        assert_eq!(spans, vec![Span::new(2, 3)]);
    }

    // 06. Context-aware asymmetric: open inside an opaque region skipped,
    //     real one found
    #[test]
    fn test_06_asymmetric_open_covered() {
        let src = b"`<x>` and {y}";
        let ctx = md_context(src);
        let spans: Vec<_> =
            ContextAsymmetricExactIter::new(src, b'{', b'}', 1, b'\n', b'\\', &ctx).collect();
        assert_eq!(spans, vec![Span::new(11, 12)]);
        // And the covered autolink itself is not found by a context-aware
        // `<`/`>` scan:
        let auto: Vec<_> =
            ContextAsymmetricExactIter::new(src, b'<', b'>', 1, b'\n', b'\\', &ctx).collect();
        assert!(auto.is_empty());
    }

    // 07. Context-aware asymmetric: close candidate inside an opaque region
    //     skipped
    #[test]
    fn test_07_asymmetric_close_covered() {
        let src = b"{a `}` b} c";
        let ctx = md_context(src);
        let spans: Vec<_> =
            ContextAsymmetricExactIter::new(src, b'{', b'}', 1, b'\n', b'\\', &ctx).collect();
        assert_eq!(spans, vec![Span::new(1, 8)]);
    }

    // 08. A jump across several lines (fence) re-syncs the scan
    #[test]
    fn test_08_multiline_jump_resync() {
        let src = b"x\n```\na\nb\nc\n```\n**z**";
        let ctx = md_context(src);
        let spans: Vec<_> =
            ContextSymmetricExactIter::new(src, b'*', 2, b'\n', b'\\', &ctx).collect();
        assert_eq!(spans, vec![Span::new(18, 19)]);
    }

    // ---- Paragraph-bounded behaviour (the new contract) ----------------- //

    // 09. A pair spanning a single newline closes, with a covered close
    //     candidate on the first line skipped
    #[test]
    fn test_09_pair_spans_newline_with_covered_candidate() {
        let src = b"*a `*` b\nc*";
        let ctx = md_context(src);
        let spans: Vec<_> =
            ContextSymmetricExactIter::new(src, b'*', 1, b'\n', b'\\', &ctx).collect();
        assert_eq!(spans, vec![Span::new(1, 10)]);
    }

    // 10. A fenced block between the opener and any close candidate ends the
    //     paragraph: the pending opener dies, nothing pairs across the fence
    #[test]
    fn test_10_fence_aborts_pending_opener() {
        let src = b"*a\n```\nx\n```\nb*";
        let ctx = md_context(src);
        let spans: Vec<_> =
            ContextSymmetricExactIter::new(src, b'*', 1, b'\n', b'\\', &ctx).collect();
        assert!(spans.is_empty());
    }

    // 11. An empty line aborts a pending opener, exactly as in the
    //     context-free iterator
    #[test]
    fn test_11_empty_line_aborts_pending_opener() {
        let src = b"*a\n\nb*";
        let ctx = md_context(src);
        let spans: Vec<_> =
            ContextSymmetricExactIter::new(src, b'*', 1, b'\n', b'\\', &ctx).collect();
        assert!(spans.is_empty());
    }

    // 12. Asymmetric pair across a single newline with a covered close
    //     candidate skipped
    #[test]
    fn test_12_asymmetric_pair_spans_newline() {
        let src = b"{a `}` b\nc} d";
        let ctx = md_context(src);
        let spans: Vec<_> =
            ContextAsymmetricExactIter::new(src, b'{', b'}', 1, b'\n', b'\\', &ctx).collect();
        assert_eq!(spans, vec![Span::new(1, 10)]);
    }

    // 13. Asymmetric pending opener dies at a fenced block
    #[test]
    fn test_13_asymmetric_fence_aborts_pending_opener() {
        let src = b"{a\n```\nx\n```\nb} c";
        let ctx = md_context(src);
        let spans: Vec<_> =
            ContextAsymmetricExactIter::new(src, b'{', b'}', 1, b'\n', b'\\', &ctx).collect();
        assert!(spans.is_empty());
    }
}
