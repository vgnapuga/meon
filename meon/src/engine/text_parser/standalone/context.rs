//! [`ParseContext`] — the opaque-region map consumed by the `find_context_*`
//! standalone iterators, plus [`ContextCursor`], its monotone lookup cursor.
//!
//! # Purpose
//!
//! Context-free standalone iterators (`find_*`) are documented to diverge from
//! the full parse: they match delimiters inside fenced blocks and inside
//! opaque inline constructs (code spans, strings, autolinks — any rule with
//! `parse_inside = false`), because they carry no cross-element state.
//!
//! A [`ParseContext`] closes most of that gap. It is built by **one**
//! sequential left-to-right pass over the source that resolves every opaque
//! construct with the same leftmost-wins semantics the full parser uses:
//!
//! 1. **Fences** are tracked line by line, mirroring `parse_block!`'s open
//!    condition (`count >= min`, no further fence byte on the info line) and
//!    close condition (`count >= open count`, remainder all `sep`/`tab`).
//!    The whole region — open line through close line, or end of input for an
//!    unclosed fence — is recorded as an opaque region.
//! 2. On non-fence lines, **opaque inline rules** are matched with an
//!    escape-aware forward search bounded by the line, exactly-count on both
//!    sides for symmetric rules. A matched construct covers its full extent,
//!    delimiters included, and the scan resumes after it — so overlapping
//!    opaque candidates resolve leftmost-first, like the full parser's
//!    trigger loop.
//!
//! The result is a sorted, non-overlapping `Vec<Span>` of opaque regions. A
//! `find_context_*` iterator then skips any *candidate delimiter* whose
//! position falls inside one of them. Note the semantics: the context
//! suppresses **trigger positions**, not enclosing spans — a bold span may
//! still legally *contain* a code span, exactly as in the full parse.
//!
//! # Remaining divergence from the full parse (by design)
//!
//! - Opaque inline matching here is line-bounded, like all standalone
//!   matching; the full parser can match an opaque construct across a line
//!   break inside one multi-line paragraph run.
//! - Block-marker context other than fences is not modelled: a `>`
//!   continuation marker byte is not covered (it is never a trigger byte in
//!   practice), and inline runs are not segmented by blockquote peeling.
//! - This construction's own closing search is escape-aware (matching the
//!   full parser); the context-free `find_*` close search is not. The
//!   context-aware and context-free variants of the *same* rule therefore use
//!   identical matchers — only the region map differs.

use super::common::{Span, count_escape, find_line_end};

/// One opaque symmetric rule: `(delimiter byte, exact run count)`.
pub type SymSpec = (u8, u32);
/// One opaque asymmetric rule: `(open byte, close byte, exact open count)`.
pub type AsymSpec = (u8, u8, u32);
/// One fence rule: `(fence byte, minimum run length)`.
pub type FenceSpec = (u8, u8);

/// A sorted, non-overlapping set of byte ranges covered by opaque constructs.
///
/// Built once per source via the generated `Parser::context(source)` and then
/// shared (immutably) by any number of `find_context_*` iterators over the
/// same source. See the [module documentation](self) for the construction
/// semantics.
#[derive(Debug, Clone, Default)]
pub struct ParseContext {
    spans: Vec<Span>,
}

impl ParseContext {
    /// The opaque regions, sorted by `start`, non-overlapping.
    #[inline]
    pub fn spans(&self) -> &[Span] {
        &self.spans
    }

    /// A fresh monotone lookup cursor over this context.
    #[inline]
    pub fn cursor(&self) -> ContextCursor<'_> {
        ContextCursor {
            spans: &self.spans,
            idx: 0,
        }
    }

    /// Build the context in one sequential pass over `source`.
    ///
    /// - `fences` — every `fence(byte, min)` rule of the grammar.
    /// - `sym` — every `symmetric` arm with `parse_inside = false`.
    /// - `asym` — every `asymmetric` arm with `parse_inside = false`.
    ///
    /// All slices may be empty; an empty rule set yields an empty context.
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        source: &[u8],
        eol: u8,
        escape: u8,
        sep: u8,
        tab: u8,
        fences: &[FenceSpec],
        sym: &[SymSpec],
        asym: &[AsymSpec],
    ) -> Self {
        let len = source.len();
        let mut spans: Vec<Span> = Vec::new();
        let mut pos: usize = 0;

        while pos < len {
            let line_end = find_line_end(source, pos, eol);

            if let Some(open_count) = fence_opens(source, pos, line_end, fences) {
                // Cover from the open line's start to the close line's end
                // (or end of input for an unclosed fence).
                let fb = source[pos];
                let mut l = if line_end < len { line_end + 1 } else { len };
                let mut end = len;
                while l < len {
                    let le = find_line_end(source, l, eol);
                    if fence_closes(source, l, le, fb, open_count, sep, tab) {
                        end = le;
                        break;
                    }
                    l = if le < len { le + 1 } else { len };
                }
                spans.push(Span::new(pos as u32, end as u32));
                pos = if end < len { end + 1 } else { len };
                continue;
            }

            // Non-fence line: scan for opaque inline constructs, leftmost-first.
            let mut i = pos;
            while i < line_end {
                let Some((q, is_sym, ri)) = earliest_opaque(source, i, line_end, sym, asym) else {
                    break;
                };

                if count_escape(source, q, escape) % 2 == 1 {
                    i = q + 1;
                    continue;
                }

                if is_sym {
                    let (byte, count) = sym[ri];
                    let mut run_end = q;
                    let mut c = 0u32;
                    while run_end < line_end && source[run_end] == byte {
                        c += 1;
                        run_end += 1;
                    }
                    if c != count {
                        i = run_end;
                        continue;
                    }
                    match sym_close(source, run_end, line_end, byte, count, escape) {
                        Some(close_end) => {
                            spans.push(Span::new(q as u32, close_end as u32));
                            i = close_end;
                        }
                        None => i = run_end,
                    }
                } else {
                    let (open, close, count) = asym[ri];
                    let mut run_end = q;
                    let mut c = 0u32;
                    while run_end < line_end && source[run_end] == open {
                        c += 1;
                        run_end += 1;
                    }
                    if c != count {
                        i = run_end;
                        continue;
                    }
                    match asym_close(source, run_end, line_end, close, escape) {
                        Some(cp) => {
                            spans.push(Span::new(q as u32, (cp + 1) as u32));
                            i = cp + 1;
                        }
                        None => i = run_end,
                    }
                }
            }

            pos = if line_end < len { line_end + 1 } else { len };
        }

        Self { spans }
    }
}

/// Does a fence open at `pos` on `[pos, line_end)`? Returns the run count.
///
/// Mirrors `parse_block!`'s `@open_simple` fence arm: run length `>= min` and
/// no further fence byte on the info line.
#[inline]
fn fence_opens(src: &[u8], pos: usize, line_end: usize, fences: &[FenceSpec]) -> Option<u8> {
    if pos >= line_end {
        return None;
    }
    let b = src[pos];
    for &(fb, fmin) in fences {
        if b == fb {
            let mut i = pos;
            let mut c: u8 = 0;
            while i < line_end && src[i] == fb {
                c = c.saturating_add(1);
                i += 1;
            }
            if c >= fmin && src[i..line_end].iter().all(|&x| x != fb) {
                return Some(c);
            }
            return None;
        }
    }
    None
}

/// Does the line `[pos, line_end)` close an open fence of `byte` × `open_count`?
///
/// Mirrors `parse_block!`'s peel-phase close: run `>= open_count`, remainder
/// all `sep` / `tab`.
#[inline]
fn fence_closes(
    src: &[u8],
    pos: usize,
    line_end: usize,
    byte: u8,
    open_count: u8,
    sep: u8,
    tab: u8,
) -> bool {
    let mut i = pos;
    let mut c: u8 = 0;
    while i < line_end && src[i] == byte {
        c = c.saturating_add(1);
        i += 1;
    }
    c >= open_count && src[i..line_end].iter().all(|&b| b == sep || b == tab)
}

/// Earliest opaque trigger in `[from, line_end)` across every rule.
///
/// Returns `(offset, is_symmetric, rule_index)`. Ties cannot occur across
/// rules — the engine assumes a byte has one meaning per grammar.
#[inline]
fn earliest_opaque(
    src: &[u8],
    from: usize,
    line_end: usize,
    sym: &[SymSpec],
    asym: &[AsymSpec],
) -> Option<(usize, bool, usize)> {
    let hay = &src[from..line_end];
    let mut best: Option<(usize, bool, usize)> = None;
    for (ri, &(b, _)) in sym.iter().enumerate() {
        if let Some(r) = memchr::memchr(b, hay) {
            if best.is_none_or(|(bq, _, _)| from + r < bq) {
                best = Some((from + r, true, ri));
            }
        }
    }
    for (ri, &(o, _, _)) in asym.iter().enumerate() {
        if let Some(r) = memchr::memchr(o, hay) {
            if best.is_none_or(|(bq, _, _)| from + r < bq) {
                best = Some((from + r, false, ri));
            }
        }
    }
    best
}

/// Escape-aware exact-count symmetric close search in `[from, line_end)`.
/// Returns the offset one past the closing run.
#[inline]
fn sym_close(
    src: &[u8],
    from: usize,
    line_end: usize,
    byte: u8,
    count: u32,
    escape: u8,
) -> Option<usize> {
    let mut j = from;
    while j < line_end {
        let r = memchr::memchr(byte, &src[j..line_end])?;
        let cp = j + r;
        if count_escape(src, cp, escape) % 2 == 1 {
            j = cp + 1;
            continue;
        }
        let mut cc = 0u32;
        let mut tmp = cp;
        while tmp < line_end && src[tmp] == byte {
            cc += 1;
            tmp += 1;
        }
        if cc == count {
            return Some(tmp);
        }
        j = tmp;
    }
    None
}

/// Escape-aware asymmetric close search in `[from, line_end)`.
/// Returns the offset of the closing byte itself.
#[inline]
fn asym_close(src: &[u8], from: usize, line_end: usize, close: u8, escape: u8) -> Option<usize> {
    let mut j = from;
    while j < line_end {
        let r = memchr::memchr(close, &src[j..line_end])?;
        let cp = j + r;
        if count_escape(src, cp, escape) % 2 == 1 {
            j = cp + 1;
            continue;
        }
        return Some(cp);
    }
    None
}

/// A monotone lookup cursor over a [`ParseContext`].
///
/// Amortized O(1) per query **provided queries arrive in non-decreasing
/// position order** — the cursor only ever moves forward. Copy a cursor to
/// fork a lookahead (e.g. a close-delimiter search) without disturbing the
/// parent's position: the copy starts at the parent's index, which is valid
/// for any position `>=` the parent's last query.
#[derive(Debug, Clone, Copy)]
pub struct ContextCursor<'m> {
    spans: &'m [Span],
    idx: usize,
}

impl ContextCursor<'_> {
    /// Is `pos` inside an opaque region?
    #[inline]
    pub fn is_covered(&mut self, pos: usize) -> bool {
        self.covering_end(pos).is_some()
    }

    /// If `pos` is inside an opaque region, return that region's `end` offset
    /// (exclusive) so the caller can jump past it. `None` when uncovered.
    #[inline]
    pub fn covering_end(&mut self, pos: usize) -> Option<usize> {
        while self.idx < self.spans.len() && (self.spans[self.idx].end as usize) <= pos {
            self.idx += 1;
        }
        match self.spans.get(self.idx) {
            Some(s) if (s.start as usize) <= pos => Some(s.end as usize),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn md_context(src: &[u8]) -> ParseContext {
        // meon-md's opaque rule set: ``` fences, `code` spans, <autolinks>.
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

    // 01. Empty rule set / empty source produce an empty context
    #[test]
    fn test_01_empty() {
        let m = ParseContext::build(b"abc", b'\n', b'\\', b' ', b'\t', &[], &[], &[]);
        assert!(m.spans().is_empty());
        let m = md_context(b"");
        assert!(m.spans().is_empty());
    }

    // 02. A code span covers its full extent, delimiters included
    #[test]
    fn test_02_code_span() {
        let m = md_context(b"a `code` b");
        assert_eq!(m.spans(), &[Span::new(2, 8)]);
    }

    // 03. An autolink covers open through close byte
    #[test]
    fn test_03_autolink() {
        let m = md_context(b"x <http://a> y");
        assert_eq!(m.spans(), &[Span::new(2, 12)]);
    }

    // 04. A fenced block covers open line through close line
    #[test]
    fn test_04_fence() {
        let src = b"before\n```\n**not bold**\n```\nafter";
        let m = md_context(src);
        assert_eq!(m.spans(), &[Span::new(7, 27)]);
    }

    // 05. An unclosed fence covers to end of input
    #[test]
    fn test_05_unclosed_fence() {
        let src = b"a\n```\nrest never closes";
        let m = md_context(src);
        assert_eq!(m.spans(), &[Span::new(2, src.len() as u32)]);
    }

    // 06. Leftmost-wins: an autolink open inside a code span is not a region
    //     of its own; scanning resumes after the code span
    #[test]
    fn test_06_leftmost_wins() {
        let m = md_context(b"`a <b` c> d <e>");
        // code span [0,6); the `<b` inside it is invisible; `c>` alone is no
        // autolink (no `<`), the next real autolink is `<e>` at 12..15.
        assert_eq!(m.spans(), &[Span::new(0, 6), Span::new(12, 15)]);
    }

    // 07. Escaped opening delimiter does not open an opaque region; the next
    //     unescaped delimiter becomes the opener (full-parse semantics)
    #[test]
    fn test_07_escaped_open() {
        let m = md_context(b"a \\`not code` b `real`");
        // `\`` at 3 is literal; the backtick at 12 opens, closing at 16.
        assert_eq!(m.spans(), &[Span::new(12, 17)]);
    }

    // 08. Escaped closing delimiter is skipped by the close search
    #[test]
    fn test_08_escaped_close() {
        let m = md_context(b"`a\\`b` rest");
        assert_eq!(m.spans(), &[Span::new(0, 6)]);
    }

    // 09. Unclosed inline construct covers nothing
    #[test]
    fn test_09_unclosed_inline() {
        let m = md_context(b"a `never closes");
        assert!(m.spans().is_empty());
    }

    // 10. Opaque matching is line-bounded
    #[test]
    fn test_10_line_bounded() {
        let m = md_context(b"a `x\ny` b");
        assert!(m.spans().is_empty());
    }

    // 11. A fence info line with a trailing fence byte does not open
    #[test]
    fn test_11_fence_info_line_reject() {
        let m = md_context(b"``` info ` tick\ncontent\n```\n");
        // The open line is rejected (` on the info line), so the *inline*
        // code-span rule sees the ``` run (count 3 != 1, no match); third
        // line's ``` opens an (unclosed) fence to EOF.
        assert_eq!(m.spans(), &[Span::new(24, 28)]);
    }

    // 12. Fence close requires at least the open count
    #[test]
    fn test_12_fence_close_count() {
        let src = b"````\nx\n```\n````\ny";
        let m = md_context(src);
        assert_eq!(m.spans(), &[Span::new(0, 15)]);
    }

    // 13. Cursor: monotone queries, covering_end jumps
    #[test]
    fn test_13_cursor() {
        let m = md_context(b"a `b` c `d` e");
        let mut cur = m.cursor();
        assert!(!cur.is_covered(0));
        assert_eq!(cur.covering_end(2), Some(5));
        assert_eq!(cur.covering_end(4), Some(5));
        assert!(!cur.is_covered(6));
        assert_eq!(cur.covering_end(9), Some(11));
        assert!(!cur.is_covered(12));
    }

    // 14. Cursor copies fork independently
    #[test]
    fn test_14_cursor_copy() {
        let m = md_context(b"a `b` c `d` e");
        let mut cur = m.cursor();
        assert!(!cur.is_covered(1));
        let mut fork = cur;
        assert_eq!(fork.covering_end(9), Some(11));
        // Parent still answers correctly for positions >= its own last query.
        assert_eq!(cur.covering_end(2), Some(5));
    }

    // 15. JSON-shaped rule set: strings only, no fences
    #[test]
    fn test_15_json_strings() {
        let src = br#"{"a": "x, {not open}", "b": 1}"#;
        let m = ParseContext::build(src, b'\n', b'\\', b' ', b'\t', &[], &[(b'"', 1)], &[]);
        // "a" -> [1,4), "x, {not open}" -> [6,21), "b" -> [23,26)
        assert_eq!(
            m.spans(),
            &[Span::new(1, 4), Span::new(6, 21), Span::new(23, 26)]
        );
    }
}
