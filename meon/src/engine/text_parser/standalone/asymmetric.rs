//! Asymmetric standalone iterators: [`AsymmetricExactIter`] for a rule the
//! full parse matches by a forward search, [`AsymmetricStackIter`] for one it
//! keeps on its stack.

use super::common::*;
/// Iterator over the spans of an asymmetric rule the full parse matches by a
/// forward search: one with `balanced = false` and `parse_inside = false`.
///
/// A single streaming scan over the source for sequences where `count`
/// consecutive `open` bytes are followed by content and then a single `close`
/// byte. The returned [`Span`] covers the inner content, excluding both the
/// opening run and the closing byte.
///
/// Matching is **paragraph-bounded**: the closing byte may sit past a single
/// `eol` (the pair spans lines of one paragraph, as in the full parse), but an
/// empty line — two consecutive `eol` bytes — or the end of input aborts the
/// pending opener.
///
/// Escape sequences are respected on the opening run and on the closing
/// byte, as in the full parse: a byte preceded by an odd number of `escape`
/// bytes is no delimiter. A search that finds no
/// close is remembered up to the end of its paragraph, so the openers after
/// it there are rejected without a search of their own: the scan stays
/// linear however many openers go unclosed.
///
/// Obtained via the generated `Parser::find_*` methods; rarely constructed
/// directly.
pub struct AsymmetricExactIter<'a> {
    src: &'a [u8],
    open: u8,
    close: u8,
    count: u32,
    eol: u8,
    escape: u8,
    pos: usize,
    /// Where the paragraph of the last failed close search ends: no
    /// unescaped `close` lies between that search's start and here. Openers only move forward,
    /// so the search started before every later opener.
    dead: usize,
}

impl<'a> AsymmetricExactIter<'a> {
    /// Create an iterator over asymmetric exact-delimiter spans.
    ///
    /// # Parameters
    ///
    /// - `src` — source byte slice to scan.
    /// - `open` — opening delimiter byte (e.g. `b'<'`).
    /// - `close` — closing delimiter byte (e.g. `b'>'`).
    /// - `count` — exact number of consecutive `open` bytes required.
    /// - `eol` — line terminator byte; a pair may span single line breaks, but
    ///   never an empty line (two consecutive `eol` bytes).
    /// - `escape` — escape prefix byte; an odd number of preceding escape bytes
    ///   suppresses the opening run.
    pub fn new(src: &'a [u8], open: u8, close: u8, count: u32, eol: u8, escape: u8) -> Self {
        Self {
            src,
            open,
            close,
            count,
            eol,
            escape,
            pos: 0,
            dead: 0,
        }
    }
}

impl Iterator for AsymmetricExactIter<'_> {
    type Item = Span;

    fn next(&mut self) -> Option<Span> {
        let src = self.src;
        let len = src.len();
        loop {
            // Open scan: one streaming pass for the opening byte.
            let p = memchr::memchr(self.open, &src[self.pos..])? + self.pos;

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

            // Close scan, bounded by the end of the paragraph: a single `eol`
            // is ordinary content, two in a row (an empty line) — or the end
            // of input — abort the pending opener.
            let cs = end;
            if cs < self.dead {
                self.pos = end;
                continue;
            }
            let mut j = cs;
            let close = loop {
                match scan_paragraph(src, j, self.close, self.eol, Some) {
                    Ok(q) if count_escape(src, q, self.escape) % 2 == 1 => j = q + 1,
                    found => break found,
                }
            };
            match close {
                Ok(cp) => {
                    self.pos = cp + 1;
                    return Some(Span::new(cs as u32, cp as u32));
                }
                Err(stop) => {
                    self.dead = stop;
                    self.pos = end;
                }
            }
        }
    }
}

/// End of a placeholder span: its frame is open, or was dropped unclosed.
const OPEN: u32 = u32::MAX;

/// The frames one asymmetric rule keeps on the full parse's stack, and the
/// spans they open, in open order.
///
/// An open takes a frame while fewer than `cap` are open, and records a
/// placeholder span; a close completes the innermost one. Past the cap, an
/// open of a `balanced` rule is counted instead, and the next close is
/// dropped with it. A frame left open when its run ends is dropped, and so
/// is its span. Once no frame is open every span is final, and they are
/// handed out in open order.
pub(crate) struct FrameStack {
    cap: usize,
    balanced: bool,
    /// Indices into `out` of the open frames' spans, innermost last.
    frames: Vec<usize>,
    /// Opens past the cap that the next closes pair with: non-zero only
    /// while `cap` frames are open, as only a pop or the run's end makes
    /// room.
    overflow: u32,
    out: Vec<Span>,
    /// The next span of `out` to hand out.
    next: usize,
}

impl FrameStack {
    /// A stack for a rule with the `balanced` flag in a grammar of
    /// `max_nest`: `max_nest` frames when balanced, one otherwise.
    pub(crate) fn new(balanced: bool, max_nest: usize) -> Self {
        Self {
            cap: if balanced { max_nest } else { max_nest.min(1) },
            balanced,
            frames: Vec::new(),
            overflow: 0,
            out: Vec::new(),
            next: 0,
        }
    }

    /// Are no frames open?
    #[inline(always)]
    pub(crate) fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// The open byte at `at`. Returns whether a frame opened.
    #[inline(always)]
    pub(crate) fn open(&mut self, at: usize) -> bool {
        if self.frames.len() < self.cap {
            self.frames.push(self.out.len());
            self.out.push(Span::new(at as u32 + 1, OPEN));
            true
        } else {
            if self.balanced && !self.frames.is_empty() {
                self.overflow += 1;
            }
            false
        }
    }

    /// The close byte at `at`.
    #[inline(always)]
    pub(crate) fn close(&mut self, at: usize) {
        if let Some(&top) = self.frames.last() {
            if self.overflow > 0 {
                self.overflow -= 1;
            } else {
                self.out[top].end = at as u32;
                self.frames.pop();
            }
        }
    }

    /// The run ends: the open frames are dropped.
    #[inline]
    pub(crate) fn end_run(&mut self) {
        self.frames.clear();
        self.overflow = 0;
    }

    /// The next span, in open order, while no frame is open.
    #[inline(always)]
    pub(crate) fn pop_closed(&mut self) -> Option<Span> {
        debug_assert!(self.frames.is_empty());
        while self.next < self.out.len() {
            let s = self.out[self.next];
            self.next += 1;
            if s.end != OPEN {
                return Some(s);
            }
        }
        self.out.clear();
        self.next = 0;
        None
    }

    /// Read the run of `src[p]` at `p`, a delimiter, as one open or close
    /// per byte; returns its end and whether the outermost frame opened in
    /// it.
    #[inline(always)]
    pub(crate) fn read_run(&mut self, src: &[u8], p: usize, open: u8) -> (usize, bool) {
        let b = src[p];
        let mut k = p;
        let mut outermost = false;
        if b == open {
            while k < src.len() && src[k] == b {
                let first = self.frames.is_empty();
                outermost |= self.open(k) && first;
                k += 1;
            }
        } else {
            while k < src.len() && src[k] == b {
                self.close(k);
                k += 1;
            }
        }
        (k, outermost)
    }
}

/// Where a forward scan meets empty lines: their first `eol`, the end of
/// a paragraph. Found ahead of the scan by one substring search, so a scan
/// that only cares where paragraphs end need not stop at every `eol`.
pub(crate) struct EmptyLines {
    eol: u8,
    finder: memchr::memmem::Finder<'static>,
    /// The first `eol` of the first empty line at or after where the last
    /// search started, `usize::MAX` when there is none.
    next: usize,
    /// Has a search been made?
    searched: bool,
}

impl EmptyLines {
    pub(crate) fn new(eol: u8) -> Self {
        Self {
            eol,
            finder: memchr::memmem::Finder::new(&[eol, eol]).into_owned(),
            next: usize::MAX,
            searched: false,
        }
    }

    /// Does an empty line begin in `[from, to)`? `from` must not move
    /// back between calls, nor follow an `eol`.
    #[inline(always)]
    pub(crate) fn between(&mut self, src: &[u8], from: usize, to: usize) -> bool {
        if !self.searched || self.next < from {
            self.searched = true;
            self.next = self.search(src, from);
        }
        self.next < to
    }

    /// The first empty line at or after `from`. A source with no `eol`
    /// there, as minified data often is, takes the plain byte search alone.
    fn search(&self, src: &[u8], from: usize) -> usize {
        let Some(r) = memchr::memchr(self.eol, &src[from..]) else {
            return usize::MAX;
        };
        let q = from + r;
        if src.get(q + 1) == Some(&self.eol) {
            return q;
        }
        self.finder
            .find(&src[q + 1..])
            .map_or(usize::MAX, |r| q + 1 + r)
    }
}

/// Iterator over the spans of an asymmetric rule the full parse keeps on
/// its stack: one with `balanced = true` or `parse_inside = true`.
///
/// Each byte of an open run is an open of its own and each byte of a close
/// run a close of its own, as in the full parse: a close completes the
/// innermost open frame. The rule nests up to `max_nest` frames when
/// `balanced`, and takes one otherwise (none with `max_nest = 0`), an open
/// past it staying text. Past
/// the cap an open of a `balanced` rule pairs with the next close, and both
/// are dropped. A delimiter byte preceded by an odd number of `escape`
/// bytes is no delimiter; the rest of its run is read as a run of its own.
///
/// Frames stay open across a single `eol` and end at an empty line — two
/// consecutive `eol` bytes — or at the end of input, which drops the ones
/// still open. Spans come in the order their frames opened. The rule's own
/// frames are the only ones this iterator sees: other rules' frames, which
/// in the full parse share the stack and its `max_nest` and may stand
/// between an open and its close, are not modelled.
///
/// Obtained via the generated `Parser::find_*` methods; rarely constructed
/// directly.
pub struct AsymmetricStackIter<'a> {
    src: &'a [u8],
    open: u8,
    close: u8,
    escape: u8,
    pos: usize,
    stack: FrameStack,
    empty: EmptyLines,
}

impl<'a> AsymmetricStackIter<'a> {
    /// Create an iterator over the spans of a stack-kept asymmetric rule.
    ///
    /// # Parameters
    ///
    /// - `src` — source byte slice to scan.
    /// - `open` — opening delimiter byte (e.g. `b'{'`).
    /// - `close` — closing delimiter byte (e.g. `b'}'`).
    /// - `balanced` — the rule's `balanced` flag: nest up to `max_nest`
    ///   frames, or take one while `max_nest` allows any.
    /// - `max_nest` — the grammar's `max_nest`.
    /// - `eol` — line terminator byte; frames stay open across single line
    ///   breaks, but never across an empty line.
    /// - `escape` — escape prefix byte.
    pub fn new(
        src: &'a [u8],
        open: u8,
        close: u8,
        balanced: bool,
        max_nest: usize,
        eol: u8,
        escape: u8,
    ) -> Self {
        Self {
            src,
            open,
            close,
            escape,
            pos: 0,
            stack: FrameStack::new(balanced, max_nest),
            empty: EmptyLines::new(eol),
        }
    }
}

impl Iterator for AsymmetricStackIter<'_> {
    type Item = Span;

    fn next(&mut self) -> Option<Span> {
        let src = self.src;
        loop {
            // With no frame open only an open byte matters; with frames
            // open a close byte too, and an empty line before it ends them.
            let p = if self.stack.is_empty() {
                if let Some(s) = self.stack.pop_closed() {
                    return Some(s);
                }
                let Some(r) = memchr::memchr(self.open, &src[self.pos..]) else {
                    self.pos = src.len();
                    return None;
                };
                self.pos + r
            } else {
                let Some(r) = memchr::memchr2(self.open, self.close, &src[self.pos..]) else {
                    self.stack.end_run();
                    self.pos = src.len();
                    continue;
                };
                let p = self.pos + r;
                if self.empty.between(src, self.pos, p) {
                    self.stack.end_run();
                }
                p
            };
            if count_escape(src, p, self.escape) % 2 == 1 {
                self.pos = p + 1;
                continue;
            }
            self.pos = self.stack.read_run(src, p, self.open).0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 01. Parses a standard valid sequence with exact open marker count
    #[test]
    fn test_01_standard_valid_sequence() {
        let src = b"<<text>";
        let mut iter = AsymmetricExactIter::new(src, b'<', b'>', 2, b'\n', b'\\');

        assert_eq!(iter.next(), Some(Span::new(2, 6)));
        assert_eq!(iter.next(), None);
    }

    // 02. Skips an escaped open marker component and successfully parses the next valid one
    #[test]
    fn test_02_escaped_open_marker_skipped() {
        let src = b"\\<<text><<valid>";
        let mut iter = AsymmetricExactIter::new(src, b'<', b'>', 2, b'\n', b'\\');

        assert_eq!(iter.next(), Some(Span::new(10, 15)));
        assert_eq!(iter.next(), None);
    }

    // 03. Rejects the sequence if the open marker count is lower than required
    #[test]
    fn test_03_fewer_open_markers_rejected() {
        let src = b"<text>";
        let mut iter = AsymmetricExactIter::new(src, b'<', b'>', 2, b'\n', b'\\');

        assert_eq!(iter.next(), None);
    }

    // 04. Rejects the sequence if the open marker count is higher than required
    #[test]
    fn test_04_more_open_markers_rejected() {
        let src = b"<<<text>";
        let mut iter = AsymmetricExactIter::new(src, b'<', b'>', 2, b'\n', b'\\');

        assert_eq!(iter.next(), None);
    }

    // 05. Extracts multiple independent valid spans sequentially from a single line
    #[test]
    fn test_05_multiple_matches_on_single_line() {
        let src = b"<<a><<b_c>";
        let mut iter = AsymmetricExactIter::new(src, b'<', b'>', 2, b'\n', b'\\');

        assert_eq!(iter.next(), Some(Span::new(2, 3)));
        assert_eq!(iter.next(), Some(Span::new(6, 9)));
        assert_eq!(iter.next(), None);
    }

    // 06. Allows an internal open marker character to safely exist within the captured inner span
    #[test]
    fn test_06_open_marker_inside_content_body() {
        let src = b"<<a<b_c>";
        let mut iter = AsymmetricExactIter::new(src, b'<', b'>', 2, b'\n', b'\\');

        assert_eq!(iter.next(), Some(Span::new(2, 7)));
        assert_eq!(iter.next(), None);
    }

    // 07. An opener left unclosed on its own line pairs with the next closing
    //     byte past a single newline — matching is paragraph-bounded, not
    //     line-bounded. The embedded newline stays inside the span.
    #[test]
    fn test_07_unclosed_pairs_across_single_newline() {
        let src = b"<<unclosed\n<<valid>";
        let mut iter = AsymmetricExactIter::new(src, b'<', b'>', 2, b'\n', b'\\');

        assert_eq!(iter.next(), Some(Span::new(2, 18)));
        assert_eq!(iter.next(), None);
    }

    // 08. Successfully processes an empty content block yielding an empty inner span
    #[test]
    fn test_08_empty_content_span() {
        let src = b"<<>";
        let mut iter = AsymmetricExactIter::new(src, b'<', b'>', 2, b'\n', b'\\');

        assert_eq!(iter.next(), Some(Span::new(2, 2)));
        assert_eq!(iter.next(), None);
    }

    // 09. Returns None immediately when the iterator is provided with an empty slice
    #[test]
    fn test_09_empty_source_slice() {
        let src = b"";
        let mut iter = AsymmetricExactIter::new(src, b'<', b'>', 2, b'\n', b'\\');

        assert_eq!(iter.next(), None);
    }

    // 10. Parses a valid matching chain located directly at the absolute start index zero
    #[test]
    fn test_10_match_at_start_boundary() {
        let src = b"<<x>";
        let mut iter = AsymmetricExactIter::new(src, b'<', b'>', 2, b'\n', b'\\');

        assert_eq!(iter.next(), Some(Span::new(2, 3)));
        assert_eq!(iter.next(), None);
    }

    // 11. A close marker on the next line of the same paragraph completes the
    //     pair: a single newline does not separate open from close.
    #[test]
    fn test_11_close_marker_across_single_newline() {
        let src = b"<<a\n>";
        let mut iter = AsymmetricExactIter::new(src, b'<', b'>', 2, b'\n', b'\\');

        assert_eq!(iter.next(), Some(Span::new(2, 4)));
        assert_eq!(iter.next(), None);
    }

    // 12. Validates the sequence when it is preceded by a non-escaping even number of escape characters
    #[test]
    fn test_12_even_escapes_treated_as_valid_match() {
        let src = b"\\\\<<a_b>";
        let mut iter = AsymmetricExactIter::new(src, b'<', b'>', 2, b'\n', b'\\');

        assert_eq!(iter.next(), Some(Span::new(4, 7)));
        assert_eq!(iter.next(), None);
    }

    // 13. Ignores trailing garbage text on the line following a successful match block
    #[test]
    fn test_13_trailing_garbage_ignored() {
        let src = b"<<abc>def";
        let mut iter = AsymmetricExactIter::new(src, b'<', b'>', 2, b'\n', b'\\');

        assert_eq!(iter.next(), Some(Span::new(2, 5)));
        assert_eq!(iter.next(), None);
    }

    // 14. Verifies precise matching configuration where the exact target count parameter is one
    #[test]
    fn test_14_exact_count_of_one() {
        let src = b"<abc>";
        let mut iter = AsymmetricExactIter::new(src, b'<', b'>', 1, b'\n', b'\\');

        assert_eq!(iter.next(), Some(Span::new(1, 4)));
        assert_eq!(iter.next(), None);
    }

    // 15. Processes independent valid matches distributed across multiple distinct line boundaries
    #[test]
    fn test_15_multiline_independent_matches() {
        let src = b"<<a>\n<<b_c>";
        let mut iter = AsymmetricExactIter::new(src, b'<', b'>', 2, b'\n', b'\\');

        assert_eq!(iter.next(), Some(Span::new(2, 3)));
        assert_eq!(iter.next(), Some(Span::new(7, 10)));
        assert_eq!(iter.next(), None);
    }

    // ---- Paragraph-bounded behaviour ------------------------------------ //

    // 16. An empty line (two consecutive eol bytes) aborts a pending opener:
    //     delimiters in different paragraphs never pair.
    #[test]
    fn test_16_empty_line_aborts_pending_open() {
        let src = b"<<open\n\nclose> other";
        let mut iter = AsymmetricExactIter::new(src, b'<', b'>', 2, b'\n', b'\\');

        assert_eq!(iter.next(), None);
    }

    // 17. After a paragraph break aborts one opener, a fresh pair in the next
    //     paragraph still matches normally.
    #[test]
    fn test_17_fresh_pair_after_paragraph_break() {
        let src = b"<<a\n\n<<b>";
        let mut iter = AsymmetricExactIter::new(src, b'<', b'>', 2, b'\n', b'\\');

        assert_eq!(iter.next(), Some(Span::new(7, 8)));
        assert_eq!(iter.next(), None);
    }

    fn spans(src: &[u8]) -> Vec<(u32, u32)> {
        AsymmetricExactIter::new(src, b'<', b'>', 1, b'\n', b'\\')
            .map(|s| (s.start, s.end))
            .collect()
    }

    // 18. An opener after one whose search failed finds no close in the same
    //     paragraph, and searches anew in the next
    #[test]
    fn test_18_failed_search_kept_to_paragraph_end() {
        assert_eq!(spans(b"<a <b\n\n<c>"), vec![(8, 9)]);
        assert_eq!(spans(b"<a\n<b\n\n<c <d>"), vec![(8, 12)]);
        assert_eq!(spans(b"<a <b>"), vec![(1, 5)]);
    }

    // 19. A long run of unclosed openers is read once
    #[test]
    fn test_19_unclosed_openers() {
        let mut src = b"<a ".repeat(20_000);
        src.extend_from_slice(b"\n\n<b>");
        assert_eq!(spans(&src), vec![(60_003, 60_004)]);
    }

    // 20. An escaped close byte is no close
    #[test]
    fn test_20_escaped_close() {
        assert_eq!(spans(b"<a\\>b>"), vec![(1, 5)]);
        assert!(spans(b"<a\\>").is_empty());
        assert_eq!(spans(b"<a\\\\>b>"), vec![(1, 4)]);
        assert_eq!(spans(b"<a\\> <b>"), vec![(1, 7)]);
    }

    // ---- Stack-kept rules ----------------------------------------------- //

    fn stack(src: &[u8], balanced: bool, max_nest: usize) -> Vec<(u32, u32)> {
        AsymmetricStackIter::new(src, b'{', b'}', balanced, max_nest, b'\n', b'\\')
            .map(|s| (s.start, s.end))
            .collect()
    }

    // 21. A balanced rule nests: a close completes the innermost open frame,
    //     and spans come in the order their frames opened
    #[test]
    fn test_21_stack_nests() {
        assert_eq!(
            stack(br#"{"a":{"b":1},"c":2}"#, true, 4),
            vec![(1, 18), (6, 11)]
        );
        assert_eq!(stack(b"{a{b}}{c}", true, 4), vec![(1, 5), (3, 4), (7, 8)]);
        let arrays: Vec<_> =
            AsymmetricStackIter::new(b"[1,[2,[3]],4]", b'[', b']', true, 4, b'\n', b'\\')
                .map(|s| (s.start, s.end))
                .collect();
        assert_eq!(arrays, vec![(1, 12), (4, 9), (7, 8)]);
    }

    // 22. Each byte of an open run opens on its own; an unbalanced rule takes
    //     one frame, and the opens past it are text
    #[test]
    fn test_22_stack_open_run_bytes() {
        assert_eq!(stack(b"{{a}}", true, 4), vec![(1, 4), (2, 3)]);
        assert_eq!(stack(b"{{a}", false, 4), vec![(1, 3)]);
        assert_eq!(stack(b"{a {b} c}", false, 4), vec![(1, 5)]);
        assert_eq!(stack(b"}}{a}}", false, 4), vec![(3, 4)]);
    }

    // 23. Past `max_nest`, an open of a balanced rule pairs with the next
    //     close and both are dropped
    #[test]
    fn test_23_stack_overflow_pairs() {
        assert_eq!(stack(b"{a{b}c}", true, 1), vec![(1, 6)]);
        assert!(stack(b"{a{b}", true, 1).is_empty());
        assert_eq!(stack(b"{a{b{c}d}e}", true, 1), vec![(1, 10)]);
        assert_eq!(stack(b"{a{b{c}d}e}", true, 2), vec![(1, 10), (3, 8)]);
        assert!(stack(b"{a}", true, 0).is_empty());
        assert!(stack(b"{a}", false, 0).is_empty());
        // The opens past the cap end with the paragraph too.
        assert_eq!(stack(b"{a{b\n\n{c}", true, 1), vec![(7, 8)]);
    }

    // 24. Frames still open where the paragraph ends are dropped, and the
    //     ones inside them that closed are kept
    #[test]
    fn test_24_stack_unclosed_dropped() {
        assert_eq!(stack(b"{a {b}", true, 4), vec![(4, 5)]);
        assert_eq!(stack(b"{a\nb}", true, 4), vec![(1, 4)]);
        assert!(stack(b"{a\n\nb}", true, 4).is_empty());
        assert_eq!(stack(b"{a {b}\n\n{c}", true, 4), vec![(4, 5), (9, 10)]);
        assert_eq!(stack(b"{{a\n", true, 4), vec![]);
    }

    // 25. An escaped delimiter is none; the rest of its run is a run of its
    //     own
    #[test]
    fn test_25_stack_escapes() {
        assert_eq!(stack(b"{a\\}b}", true, 4), vec![(1, 5)]);
        assert!(stack(b"\\{a}", true, 4).is_empty());
        assert_eq!(stack(b"\\{{a}", true, 4), vec![(3, 4)]);
        assert_eq!(stack(b"{a\\}}", true, 4), vec![(1, 4)]);
        assert_eq!(stack(b"\\\\{a}", true, 4), vec![(3, 4)]);
    }

    // 26. A deep or long input is read once
    #[test]
    fn test_26_stack_linear() {
        let mut src = b"{".repeat(30_000);
        src.extend_from_slice(&b"}".repeat(30_000));
        assert_eq!(stack(&src, true, 64).len(), 64);
        let src = b"{a ".repeat(20_000);
        assert!(stack(&src, true, 64).is_empty());
        let src = b"{a}".repeat(20_000);
        assert_eq!(stack(&src, false, 1).len(), 20_000);
    }

    // ---- The generated methods ------------------------------------------ //

    /// The `find_*` and `find_context_*` methods `define_parser!` generates
    /// for a stack-kept rule. The expansion inside this crate trips lints
    /// that a dependent crate never sees.
    #[allow(unused)]
    mod generated {
        macro_rules! objects {
            ($m:ident, $p:ident, $bal:tt, $maxn:tt) => {
                mod $m {
                    crate::define_parser!($p {
                        sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\', max_nest = $maxn;
                        inline {
                            on_trigger(b'{', b'}') {
                                asymmetric b'{', b'}' { balanced = $bal; parse_inside = true; 1 => objs [40], }
                            }
                            fallback => texts [10];
                        }
                        blocks { fallback => paragraphs [80]; }
                    });
                }
            };
        }

        objects!(cap1, Cap1, true, 1);
        objects!(cap2, Cap2, true, 2);
        objects!(flat, Flat, false, 4);

        fn spans<'a>(it: impl IntoIterator<Item = &'a crate::span::Span>) -> Vec<(u32, u32)> {
            it.into_iter().map(|s| (s.start, s.end)).collect()
        }

        // 27. The finders get the rule's `balanced` flag and the grammar's
        //     `max_nest`, and yield the full parse's spans
        #[test]
        fn test_27_generated_flags_and_cap() {
            macro_rules! check {
                ($m:ident :: $p:ident, $src:expr, $want:expr) => {{
                    let src: &[u8] = $src;
                    let full = spans(&$m::$p::parse(src).objs);
                    let ctx = $m::$p::context(src);
                    assert_eq!(full, $want);
                    assert_eq!(spans(&$m::$p::find_objs(src).collect::<Vec<_>>()), full);
                    assert_eq!(
                        spans(&$m::$p::find_context_objs(src, &ctx).collect::<Vec<_>>()),
                        full
                    );
                }};
            }
            check!(cap1::Cap1Parser, b"{a{b}c}", vec![(1, 6)]);
            check!(cap2::Cap2Parser, b"{a{b{c}d}e}", vec![(1, 10), (3, 8)]);
            check!(flat::FlatParser, b"{a {b} c}", vec![(1, 5)]);
            check!(flat::FlatParser, b"{{a}", vec![(1, 3)]);
        }
    }
}
