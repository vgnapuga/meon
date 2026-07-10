//! Context-aware standalone iterators: [`ContextSymmetricExactIter`] and
//! [`ContextAsymmetricExactIter`].
//!
//! Each is byte-for-byte the same matcher as its context-free sibling
//! ([`super::symmetric::SymmetricExactIter`] /
//! [`super::asymmetric::AsymmetricExactIter`]) with exactly one addition: a
//! candidate delimiter whose position lies inside a
//! [`ParseContext`](super::context::ParseContext) opaque region is skipped,
//! and the scan resumes after that region.
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

use super::common::{Span, advance_line, count_escape, find_line_end};
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
    line_end: usize,
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
        let line_end = find_line_end(src, 0, eol);
        Self {
            src,
            byte,
            count,
            eol,
            escape,
            pos: 0,
            line_end,
            cur: ctx.cursor(),
        }
    }
}

impl Iterator for ContextSymmetricExactIter<'_> {
    type Item = Span;

    fn next(&mut self) -> Option<Span> {
        let src = self.src;
        loop {
            // Skipping past a covered region may land `pos` several lines
            // ahead; keep the larger of the jump target and each next line
            // start while re-syncing.
            while self.pos >= self.line_end {
                let (next, end) = advance_line(src, self.line_end, self.eol)?;
                self.line_end = end;
                self.pos = self.pos.max(next);
            }

            let Some(r) = memchr::memchr(self.byte, &src[self.pos..self.line_end]) else {
                self.pos = self.line_end;
                continue;
            };
            let p = self.pos + r;

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
            while end < self.line_end && src[end] == self.byte {
                c += 1;
                end += 1;
            }

            if c != self.count {
                self.pos = end;
                continue;
            }

            let cs = end;
            let mut j = cs;
            // Fork the cursor for the close lookahead: close candidates are
            // >= `p`, for which the parent cursor's index is already valid.
            let mut ccur = self.cur;
            let close = loop {
                if j >= self.line_end {
                    break None;
                }
                let Some(r2) = memchr::memchr(self.byte, &src[j..self.line_end]) else {
                    break None;
                };
                let cp = j + r2;
                if let Some(ce) = ccur.covering_end(cp) {
                    j = ce;
                    continue;
                }
                let mut cc = 0u32;
                let mut tmp = cp;
                while tmp < self.line_end && src[tmp] == self.byte {
                    cc += 1;
                    tmp += 1;
                }
                if cc == self.count {
                    break Some((cp, tmp));
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
    line_end: usize,
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
        let line_end = find_line_end(src, 0, eol);
        Self {
            src,
            open,
            close,
            count,
            eol,
            escape,
            pos: 0,
            line_end,
            cur: ctx.cursor(),
        }
    }
}

impl Iterator for ContextAsymmetricExactIter<'_> {
    type Item = Span;

    fn next(&mut self) -> Option<Span> {
        let src = self.src;
        loop {
            while self.pos >= self.line_end {
                let (next, end) = advance_line(src, self.line_end, self.eol)?;
                self.line_end = end;
                self.pos = self.pos.max(next);
            }

            let Some(r) = memchr::memchr(self.open, &src[self.pos..self.line_end]) else {
                self.pos = self.line_end;
                continue;
            };
            let p = self.pos + r;

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
            while end < self.line_end && src[end] == self.open {
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
                if j >= self.line_end {
                    break None;
                }
                let Some(r2) = memchr::memchr(self.close, &src[j..self.line_end]) else {
                    break None;
                };
                let cp = j + r2;
                if let Some(ce) = ccur.covering_end(cp) {
                    j = ce;
                    continue;
                }
                break Some(cp);
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

    // 08. A jump across several lines (fence) re-syncs the line loop
    #[test]
    fn test_08_multiline_jump_resync() {
        let src = b"x\n```\na\nb\nc\n```\n**z**";
        let ctx = md_context(src);
        let spans: Vec<_> =
            ContextSymmetricExactIter::new(src, b'*', 2, b'\n', b'\\', &ctx).collect();
        assert_eq!(spans, vec![Span::new(18, 19)]);
    }
}
