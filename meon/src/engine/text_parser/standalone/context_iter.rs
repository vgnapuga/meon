//! Context-aware standalone iterators: [`ContextSymmetricExactIter`] and
//! [`ContextAsymmetricExactIter`].
//!
//! Each is the same streaming matcher as its context-free sibling
//! ([`super::symmetric::SymmetricExactIter`] /
//! [`super::asymmetric::AsymmetricExactIter`]), reading the source through a
//! [`ParseContext`](super::context::ParseContext) the way the full parse
//! does:
//!
//! - a candidate delimiter whose position lies inside an opaque region is
//!   skipped, and the scan resumes after that region;
//! - a candidate among a line's markers (a bullet, a heading's `#` run, a
//!   blockquote's `>`) is no delimiter: the full parse scans only the content
//!   after them;
//! - the close search stays inside the opener's inline run, with lines read
//!   through the context's [`LineModel`](super::context::LineModel): from a
//!   paragraph line the run goes on through the following paragraph lines,
//!   up to an empty line (two consecutive `eol` bytes), a line some rule
//!   claims — a fence, a blockquote line, a list item, a heading — or the end
//!   of input; from any other line it is the rest of that line.
//!
//! The context suppresses **candidate positions**, not enclosing spans: a
//! returned span may legally *contain* an opaque region (`**a `code` b**`
//! still yields one bold span), exactly as in the full parse.
//!
//! Line-family rules (`line`, `line_simple`, `cont`, `block`, `num`) do not
//! need dedicated context-aware iterators: their matches are
//! position-independent per line, so the generated `find_context_*` methods
//! reuse the context-free iterator and keep the items whose line the context
//! admits (see
//! [`ContextCursor::admits`](super::context::ContextCursor::admits)) or, for
//! `cont`, whose marker no region covers.

use super::common::{Span, count_escape, next_in_paragraph};
use super::context::{ContextCursor, LineReader, ParseContext};

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
    lines: LineReader<'a>,
    /// May the delimiter byte fall among a line's markers?
    marked: bool,
    /// Is the delimiter byte also a `cont` marker?
    straddles: bool,
}

impl<'a> ContextSymmetricExactIter<'a> {
    /// Create a context-aware symmetric exact-delimiter iterator. Parameters
    /// as in the context-free sibling, plus the prebuilt `ctx` (borrowed for
    /// the iterator's lifetime). `ctx` splits lines with the `eol` of its
    /// [`LineModel`](super::context::LineModel); pass the same byte.
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
            lines: ctx.reader(),
            marked: ctx.reader().marks(byte),
            straddles: ctx.lines().is_cont(byte),
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

            // A line's markers are no delimiters. Only a run that opens is
            // checked: a run of another length is no opener either way,
            // unless the delimiter byte is also a `cont` marker, when the
            // run may begin among the markers and go on into content.
            if c != self.count {
                self.pos = if self.straddles && self.lines.in_markers(src, p) {
                    p + 1
                } else {
                    end
                };
                continue;
            }
            if self.marked && self.lines.in_markers(src, p) {
                self.pos = p + 1;
                continue;
            }

            // Close scan inside the opener's inline run, opaque-aware. Fork
            // the cursor for the lookahead: close candidates are `>= p`, for
            // which the parent cursor's index is already valid. The opener's
            // line is read when the scan first meets its end.
            let mut para: Option<bool> = None;
            let lines = &mut self.lines;
            let cs = end;
            let mut j = cs;
            let mut ccur = self.cur;
            let eol = self.eol;
            let close = loop {
                // Only a paragraph line goes on with a paragraph run; an
                // opaque region leading it is skipped wholesale.
                let enter_line = |ls: usize| {
                    lines
                        .crosses(src, p, ls, &mut para)
                        .then(|| ccur.covering_end(ls).unwrap_or(ls))
                };
                let Some(q) = next_in_paragraph(src, j, self.byte, eol, enter_line) else {
                    break None;
                };
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
    lines: LineReader<'a>,
    /// May the delimiter byte fall among a line's markers?
    marked: bool,
    /// Is the delimiter byte also a `cont` marker?
    straddles: bool,
}

impl<'a> ContextAsymmetricExactIter<'a> {
    /// Create a context-aware asymmetric exact-delimiter iterator. Parameters
    /// as in the context-free sibling, plus the prebuilt `ctx`, as in
    /// [`ContextSymmetricExactIter::new`].
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
            lines: ctx.reader(),
            marked: ctx.reader().marks(open),
            straddles: ctx.lines().is_cont(open),
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

            // A line's markers are no delimiters. Only a run that opens is
            // checked: a run of another length is no opener either way,
            // unless the delimiter byte is also a `cont` marker, when the
            // run may begin among the markers and go on into content.
            if c != self.count {
                self.pos = if self.straddles && self.lines.in_markers(src, p) {
                    p + 1
                } else {
                    end
                };
                continue;
            }
            if self.marked && self.lines.in_markers(src, p) {
                self.pos = p + 1;
                continue;
            }

            let mut para: Option<bool> = None;
            let lines = &mut self.lines;
            let cs = end;
            let mut j = cs;
            let mut ccur = self.cur;
            let eol = self.eol;
            let close = loop {
                let enter_line = |ls: usize| {
                    lines
                        .crosses(src, p, ls, &mut para)
                        .then(|| ccur.covering_end(ls).unwrap_or(ls))
                };
                let Some(q) = next_in_paragraph(src, j, self.close, eol, enter_line) else {
                    break None;
                };
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

    use crate::engine::text_parser::standalone::context::{
        BlockRule, BlockSimpleRule, LineModel, LineRule,
    };

    /// meon-md's line rules.
    const MD_LINES: LineModel = LineModel {
        eol: b'\n',
        sep: b' ',
        tab: b'\t',
        max_nest: 4,
        block_simple: &[
            BlockSimpleRule::Fence { byte: b'`', min: 3 },
            BlockSimpleRule::Cont { byte: b'>' },
        ],
        block: &[
            BlockRule::Marker { bytes: b"-*+" },
            BlockRule::Num { ends: b".)" },
        ],
        lines: &[
            LineRule::Marker { byte: b'#', max: 6 },
            LineRule::Uniform {
                bytes: b"-*_",
                min: 3,
            },
        ],
    };

    fn md_context(src: &[u8]) -> ParseContext {
        ParseContext::build(src, b'\\', MD_LINES, &[(b'`', 1)], &[(b'<', b'>', 1)], 0)
    }

    fn sym(src: &[u8], count: u32) -> Vec<Span> {
        let ctx = md_context(src);
        ContextSymmetricExactIter::new(src, b'*', count, b'\n', b'\\', &ctx).collect()
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

    // ---- Paragraph-bounded behaviour ------------------------------------ //

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
    // 14. Escaped openers are skipped by both context-aware iterators
    #[test]
    fn test_14_escaped_openers_skipped() {
        let src = b"\\*a* *b*";
        let ctx = ParseContext::default();
        let spans: Vec<_> =
            ContextSymmetricExactIter::new(src, b'*', 1, b'\n', b'\\', &ctx).collect();
        assert_eq!(spans, vec![Span::new(4, 5)]);
        let src = b"\\{a} {b}";
        let spans: Vec<_> =
            ContextAsymmetricExactIter::new(src, b'{', b'}', 1, b'\n', b'\\', &ctx).collect();
        assert_eq!(spans, vec![Span::new(6, 7)]);
    }

    // 15. A wrong-length asymmetric open run is skipped
    #[test]
    fn test_15_asym_run_mismatch() {
        let src = b"{{a} {b}";
        let ctx = ParseContext::default();
        let spans: Vec<_> =
            ContextAsymmetricExactIter::new(src, b'{', b'}', 1, b'\n', b'\\', &ctx).collect();
        assert_eq!(spans, vec![Span::new(6, 7)]);
    }

    // 16. Symmetric close search skips a wrong-count closing run and an
    //     inline opaque region leading the next line
    #[test]
    fn test_16_sym_close_edges() {
        let src = b"**a*b**";
        let ctx = ParseContext::default();
        let spans: Vec<_> =
            ContextSymmetricExactIter::new(src, b'*', 2, b'\n', b'\\', &ctx).collect();
        assert_eq!(spans, vec![Span::new(2, 5)]);

        let src = b"*a\n`x` b*";
        let ctx = md_context(src);
        let spans: Vec<_> =
            ContextSymmetricExactIter::new(src, b'*', 1, b'\n', b'\\', &ctx).collect();
        assert_eq!(spans, vec![Span::new(1, 8)]);
    }

    // 17. Asymmetric close search: empty-line abort and an inline opaque
    //     region leading the next line
    #[test]
    fn test_17_asym_close_edges() {
        let src = b"{a\n\nb} {c}";
        let ctx = md_context(src);
        let spans: Vec<_> =
            ContextAsymmetricExactIter::new(src, b'{', b'}', 1, b'\n', b'\\', &ctx).collect();
        assert_eq!(spans, vec![Span::new(8, 9)]);

        let src = b"{a\n`x` b} c";
        let ctx = md_context(src);
        let spans: Vec<_> =
            ContextAsymmetricExactIter::new(src, b'{', b'}', 1, b'\n', b'\\', &ctx).collect();
        assert_eq!(spans, vec![Span::new(1, 8)]);
    }

    // ---- Line structure ---------------------------------------------- //

    // 18. A line's markers are no delimiters: an asterisk bullet, a thematic
    //     break made of the delimiter byte
    #[test]
    fn test_18_markers_are_no_delimiters() {
        assert_eq!(sym(b"* item with *emphasis*", 1), vec![Span::new(13, 21)]);
        assert_eq!(sym(b"***\n***a***", 3), vec![Span::new(7, 8)]);
    }

    // 19. A pair does not go on from a paragraph line into a line a rule
    //     claims; between paragraph lines it does
    #[test]
    fn test_19_paragraph_run_ends_at_block_line() {
        assert!(sym(b"**a\n- b**", 2).is_empty());
        assert!(sym(b"**a\n# b**", 2).is_empty());
        assert!(sym(b"**a\n> b**", 2).is_empty());
        assert_eq!(sym(b"**a\nb**", 2), vec![Span::new(2, 5)]);
    }

    // 20. From a block line the run is the rest of that line
    #[test]
    fn test_20_block_line_run_is_one_line() {
        assert!(sym(b"- **a\nb**", 2).is_empty());
        assert!(sym(b"> **a\n> b**", 2).is_empty());
        assert_eq!(sym(b"- **a** **b\n- c**", 2), vec![Span::new(4, 5)]);
    }

    // 21. An opaque region over several paragraph lines, leading a line the
    //     close search enters, is skipped like any other
    #[test]
    fn test_21_multi_line_region_is_skipped() {
        assert_eq!(sym(b"**a `x\ny\nz` b**", 2), vec![Span::new(2, 13)]);
    }

    // 22. Everything inside a fence within a blockquote is suppressed
    #[test]
    fn test_22_fence_inside_blockquote() {
        assert_eq!(
            sym(b"> ```\n> *x* *y*\n> ```\n*z*", 1),
            vec![Span::new(23, 24)]
        );
    }

    // 23. The asymmetric iterator reads lines the same way
    #[test]
    fn test_23_asymmetric_line_structure() {
        let asym = |src: &[u8]| -> Vec<Span> {
            let ctx = md_context(src);
            ContextAsymmetricExactIter::new(src, b'{', b'}', 1, b'\n', b'\\', &ctx).collect()
        };
        assert!(asym(b"{a\n# b} c").is_empty());
        assert!(asym(b"- {a\nb}").is_empty());
        assert_eq!(asym(b"{a\nb} c"), vec![Span::new(1, 4)]);
    }

    // 24. A delimiter run that begins among a line's markers goes on into
    //     content when the delimiter byte is also a `cont` marker, and a
    //     bullet on a later line is a marker there too
    #[test]
    fn test_24_runs_across_markers() {
        const PIPES: LineModel = LineModel {
            eol: b'\n',
            sep: b' ',
            tab: b'\t',
            max_nest: 1,
            block_simple: &[BlockSimpleRule::Cont { byte: b'|' }],
            block: &[],
            lines: &[],
        };
        let src = b"||x|\n";
        let ctx = ParseContext::build(src, b'\\', PIPES, &[], &[], 0);
        let spans: Vec<_> =
            ContextSymmetricExactIter::new(src, b'|', 1, b'\n', b'\\', &ctx).collect();
        assert_eq!(spans, vec![Span::new(2, 3)]);
        assert_eq!(sym(b"x\n* *y*\n", 1), vec![Span::new(5, 6)]);
    }
}
