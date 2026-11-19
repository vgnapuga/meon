//! Asymmetric standalone iterators: [`AsymmetricExactIter`] and
//! [`AsymmetricBalancedIter`] for a rule the full parse matches by a forward
//! search, [`AsymmetricStackIter`] for one it keeps on its stack.

use super::common::*;
use super::context::ByteSet;
/// Iterator over the spans of an asymmetric rule the full parse matches by a
/// forward search: one with `balanced = false` and `parse_inside = false`.
///
/// A single streaming scan over the source for sequences where `count`
/// consecutive `open` bytes are followed by content and then a single `close`
/// byte. The returned [`Span`] covers the inner content, excluding both the
/// opening run and the closing byte.
///
/// Matching is **paragraph-bounded**: the closing byte may sit past a single
/// `eol` (the pair spans lines of one paragraph, as in the full parse), but a
/// blank line (see [`LineBreaks`]) or the end of input aborts the pending
/// opener.
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
    breaks: LineBreaks,
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
    /// - `breaks` — how lines end: an `eol` byte or a [`LineBreaks`]; a pair
    ///   may span single line breaks, but never a blank line.
    /// - `escape` — escape prefix byte; an odd number of preceding escape bytes
    ///   suppresses the opening run.
    pub fn new(
        src: &'a [u8],
        open: u8,
        close: u8,
        count: u32,
        breaks: impl Into<LineBreaks>,
        escape: u8,
    ) -> Self {
        Self {
            src,
            open,
            close,
            count,
            breaks: breaks.into(),
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
            // is ordinary content, a blank line — or the end of input —
            // aborts the pending opener.
            let cs = end;
            if cs < self.dead {
                self.pos = end;
                continue;
            }
            let mut j = cs;
            let close = loop {
                match scan_paragraph(src, j, self.close, self.breaks, Some) {
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

/// The failed searches of one `balanced = true` opaque asymmetric rule, in
/// the order its open bytes are tried.
///
/// A search that fails decides every open byte before where it stopped: the
/// ones it left unbalanced fail too, and every other one closes. Keeping
/// them, no open byte there is searched for in vain, and a scan over many
/// unbalanced open bytes stays linear.
#[derive(Debug, Default, Clone)]
pub(crate) struct Rings {
    /// Where the last failed search stopped.
    end: usize,
    /// Did it meet a close byte? Without one every open byte it decides
    /// fails.
    met: bool,
    /// The open bytes it left unbalanced, ascending.
    open: Vec<usize>,
    /// The first of `open` not behind the last open byte asked about.
    cur: usize,
}

impl Rings {
    /// The close of the ring the open byte at `p` opens, as the full parse
    /// reads it: every other unescaped `open` byte nests, every unescaped
    /// `close` byte leaves one level, and the one that leaves the last level
    /// closes the ring. `bytes` is `(open, close, escape, breaks)`. The search
    /// stays in the inline run `enter_line` bounds (see
    /// [`scan_paragraph_by`]). `p` must not move back between calls.
    #[inline]
    pub(crate) fn close(
        &mut self,
        src: &[u8],
        p: usize,
        bytes: (u8, u8, u8, LineBreaks),
        mut enter_line: impl FnMut(usize) -> Option<usize>,
    ) -> Option<usize> {
        let (open, close, escape, breaks) = bytes;
        let eol = breaks.eol;
        let known = p < self.end;
        if known {
            while self.cur < self.open.len() && self.open[self.cur] < p {
                self.cur += 1;
            }
            if !self.met || self.open.get(self.cur) == Some(&p) {
                return None;
            }
        } else {
            self.open.clear();
            self.cur = 0;
        }
        let find = |h: &[u8]| memchr::memchr3(open, close, eol, h);
        let (mut depth, mut met) = (1usize, false);
        let mut j = p + 1;
        loop {
            let q = match scan_paragraph_by(src, j, breaks, find, &mut enter_line) {
                Ok(q) => q,
                Err(stop) => {
                    if !known {
                        (self.end, self.met) = (stop, met);
                    }
                    return None;
                }
            };
            j = q + 1;
            if count_escape(src, q, escape) % 2 == 1 {
                continue;
            }
            // A close byte that is also the open byte nests, yet the byte is
            // present.
            met |= src[q] == close;
            if src[q] == open {
                depth += 1;
                if !known {
                    self.open.push(q);
                }
            } else {
                depth -= 1;
                if depth == 0 {
                    return Some(q);
                }
                if !known {
                    self.open.pop();
                }
            }
        }
    }
}

/// Iterator over the spans of an asymmetric rule the full parse matches by a
/// forward search that counts the rule's own bytes: one with `balanced =
/// true` and `parse_inside = false`.
///
/// Each open byte is an open of its own. The first one opens the outer ring
/// and every other open byte nests in it; the close byte that leaves the
/// last level closes the ring. The returned [`Span`] is the ring's content,
/// one span per ring, whatever nests in it. A ring that never closes yields
/// nothing, and the next open byte tries, the rest of its own run included.
///
/// Matching is **paragraph-bounded**, as for [`AsymmetricExactIter`], and
/// escapes are respected on every byte of the rule. The scan stays linear
/// however many open bytes go unbalanced.
///
/// Obtained via the generated `Parser::find_*` methods; rarely constructed
/// directly.
pub struct AsymmetricBalancedIter<'a> {
    src: &'a [u8],
    open: u8,
    close: u8,
    breaks: LineBreaks,
    escape: u8,
    pos: usize,
    rings: Rings,
}

impl<'a> AsymmetricBalancedIter<'a> {
    /// Create an iterator over the rings of an opaque balanced asymmetric
    /// rule.
    ///
    /// # Parameters
    ///
    /// - `src` — source byte slice to scan.
    /// - `open` — opening delimiter byte (e.g. `b'{'`).
    /// - `close` — closing delimiter byte (e.g. `b'}'`).
    /// - `breaks` — how lines end: an `eol` byte or a [`LineBreaks`]; a ring
    ///   may span single line breaks, but never a blank line.
    /// - `escape` — escape prefix byte.
    pub fn new(
        src: &'a [u8],
        open: u8,
        close: u8,
        breaks: impl Into<LineBreaks>,
        escape: u8,
    ) -> Self {
        Self {
            src,
            open,
            close,
            breaks: breaks.into(),
            escape,
            pos: 0,
            rings: Rings::default(),
        }
    }
}

impl Iterator for AsymmetricBalancedIter<'_> {
    type Item = Span;

    fn next(&mut self) -> Option<Span> {
        let src = self.src;
        let bytes = (self.open, self.close, self.escape, self.breaks);
        loop {
            let p = memchr::memchr(self.open, &src[self.pos..])? + self.pos;
            if count_escape(src, p, self.escape) % 2 == 1 {
                self.pos = p + 1;
                continue;
            }
            // A ring that never closes leaves the next open byte to try.
            if let Some(q) = self.rings.close(src, p, bytes, Some) {
                self.pos = q + 1;
                return Some(Span::new(p as u32 + 1, q as u32));
            }
            self.pos = p + 1;
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

/// Where a forward scan meets blank lines: the `eol` before them, the end
/// of a paragraph. Found ahead of the scan — by one substring search when
/// only an empty line is blank — so a scan that only cares where paragraphs
/// end need not stop at every `eol`.
pub(crate) struct EmptyLines {
    breaks: LineBreaks,
    finder: memchr::memmem::Finder<'static>,
    /// The `eol` before the first blank line at or after where the last
    /// search started, `usize::MAX` when there is none.
    next: usize,
    /// Has a search been made?
    searched: bool,
}

impl EmptyLines {
    pub(crate) fn new(breaks: LineBreaks) -> Self {
        let eol = breaks.eol;
        Self {
            breaks,
            finder: memchr::memmem::Finder::new(&[eol, eol]).into_owned(),
            next: usize::MAX,
            searched: false,
        }
    }

    /// Does a blank line begin in `[from, to)`? `from` must not move
    /// back between calls, nor follow an `eol`.
    #[inline(always)]
    pub(crate) fn between(&mut self, src: &[u8], from: usize, to: usize) -> bool {
        if !self.searched || self.next < from {
            self.searched = true;
            self.next = self.search(src, from);
        }
        self.next < to
    }

    /// The first blank line at or after `from`. A source with no `eol`
    /// there, as minified data often is, takes the plain byte search alone.
    fn search(&self, src: &[u8], from: usize) -> usize {
        let eol = self.breaks.eol;
        let Some(r) = memchr::memchr(eol, &src[from..]) else {
            return usize::MAX;
        };
        let mut q = from + r;
        if self.breaks.crlf || self.breaks.blank != ByteSet::EMPTY {
            // Every line is read for blank bytes.
            loop {
                if q + 1 < src.len() && self.breaks.blank_at(src, q + 1) {
                    return q;
                }
                match memchr::memchr(eol, &src[q + 1..]) {
                    Some(r) => q = q + 1 + r,
                    None => return usize::MAX,
                }
            }
        }
        if src.get(q + 1) == Some(&eol) {
            return q;
        }
        self.finder
            .find(&src[q + 1..])
            .map_or(usize::MAX, |r| q + 1 + r)
    }
}

/// Iterator over the spans of an asymmetric rule the full parse keeps on
/// its stack: one with `parse_inside = true`.
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
/// Frames stay open across a single `eol` and end at a blank line (see
/// [`LineBreaks`]) or at the end of input, which drops the ones still open. Spans come in the order their frames opened. The rule's own
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
    /// - `breaks` — how lines end: an `eol` byte or a [`LineBreaks`]; frames
    ///   stay open across single line breaks, but never across a blank line.
    /// - `escape` — escape prefix byte.
    pub fn new(
        src: &'a [u8],
        open: u8,
        close: u8,
        balanced: bool,
        max_nest: usize,
        breaks: impl Into<LineBreaks>,
        escape: u8,
    ) -> Self {
        Self {
            src,
            open,
            close,
            escape,
            pos: 0,
            stack: FrameStack::new(balanced, max_nest),
            empty: EmptyLines::new(breaks.into()),
        }
    }
}

impl Iterator for AsymmetricStackIter<'_> {
    type Item = Span;

    fn next(&mut self) -> Option<Span> {
        let src = self.src;
        loop {
            // With no frame open only an open byte matters; with frames
            // open a close byte too, and a blank line before it ends them.
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

        #[derive(Debug, Clone, Copy)]
        pub struct Link {
            pub text: crate::span::Span,
            pub url: crate::span::Span,
            pub img: bool,
        }

        // An opaque balanced rule next to a transparent one, an italic, a
        // transparent `chained` rule and a hard break.
        mod opq {
            use super::Link;
            crate::define_parser!(Opq {
                sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\', max_nest = 4;
                inline {
                    hard_break(b'\\', b' ', 2) => hbs [40];
                    on_trigger(b'{', b'}', b'[', b']', b'*', b'(', b')', b'<', b'>') {
                        symmetric b'*' { parse_inside = true; 1 => ems [40], }
                        asymmetric b'{', b'}' { balanced = true; parse_inside = false; 1 => objs [40], }
                        asymmetric b'[', b']' { balanced = true; parse_inside = true; 1 => arrs [40], }
                        chained: Link {
                            | b'(', b')' | { parse_inside = true; balanced = false; } => text,
                            | b'<', b'>' | { parse_inside = true; balanced = false; } => url,
                            prefix | b'!' | => img,
                        } => links [40]
                    }
                    fallback => texts [10];
                }
                blocks { fallback => paragraphs [80]; }
            });
        }
        use opq::OpqParser;

        // 28. Inside an opaque ring only its own rule's bytes count: another
        //     stack rule neither opens nor blocks the ring's close, a ring
        //     nested in it is no element, and the context finder of that
        //     rule agrees
        #[test]
        fn test_28_generated_opaque_ring_isolates() {
            for (src, objs) in [
                (&b"{[a]}"[..], vec![(1, 4)]),
                (b"{[a}]", vec![(1, 3)]),
                (b"{a{b}c}", vec![(1, 6)]),
            ] {
                let full = OpqParser::parse(src);
                assert_eq!(
                    spans(&full.objs),
                    objs,
                    "{:?}",
                    String::from_utf8_lossy(src)
                );
                assert!(full.arrs.is_empty());
                let ctx = OpqParser::context(src);
                assert!(OpqParser::find_context_arrs(src, &ctx).next().is_none());
                assert_eq!(
                    spans(&OpqParser::find_objs(src).collect::<Vec<_>>()),
                    spans(&full.objs)
                );
            }
            assert!(OpqParser::parse(b"{*a*}").ems.is_empty());
            // A ring that never closes is text, and the other rules act there.
            let src = b"{a [b] *c*";
            let full = OpqParser::parse(src);
            assert!(full.objs.is_empty());
            assert_eq!(spans(&full.arrs), vec![(4, 5)]);
            assert_eq!(spans(&full.ems), vec![(8, 9)]);
            let ctx = OpqParser::context(src);
            assert_eq!(
                spans(&OpqParser::find_context_arrs(src, &ctx).collect::<Vec<_>>()),
                spans(&full.arrs)
            );
        }

        // 29. A transparent `chained` match goes on past an opaque ring
        //     inside it: the ring's bytes do not close the component
        #[test]
        fn test_29_generated_chained_over_opaque_ring() {
            let full = OpqParser::parse(b"(a {b) c})<u>");
            assert_eq!(spans(&full.objs), vec![(4, 8)]);
            let links: Vec<_> = full
                .links
                .iter()
                .map(|l| (spans([&l.text]), spans([&l.url])))
                .collect();
            assert_eq!(links, vec![(vec![(1, 9)], vec![(11, 12)])]);
        }

        // 30. The forward search of an opaque ring that closes never sees
        //     the hard breaks inside it; a ring that never closes is plain
        //     text and keeps them
        #[test]
        fn test_30_generated_hard_break_in_opaque_ring() {
            assert!(OpqParser::parse(b"{a\\\nb}").hbs.is_empty());
            assert!(OpqParser::parse(b"{a  \nb}").hbs.is_empty());
            assert_eq!(spans(&OpqParser::parse(b"{a\\\nb").hbs), vec![(2, 2)]);
            assert_eq!(spans(&OpqParser::parse(b"[a\\\nb]").hbs), vec![(2, 2)]);
        }

        mod one {
            crate::define_parser!(One {
                sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\', max_nest = 4;
                inline {
                    on_trigger(b'{', b'}', b'*') {
                        symmetric b'*' { parse_inside = true; 1 => ems [40], }
                        asymmetric b'{', b'}' { balanced = false; parse_inside = true; 1 => objs [40], }
                    }
                    fallback => texts [10];
                }
                blocks { fallback => paragraphs [80]; }
            });
        }

        // 31. `balanced = false` holds one frame of the rule at a time, at
        //     any depth: under another rule's frame it opens, inside its own
        //     it does not, and the finders agree
        #[test]
        fn test_31_generated_unbalanced_opens_under_other_frames() {
            use one::OneParser;
            for (src, objs) in [
                (&b"*a {b} c*"[..], vec![(4, 5)]),
                (b"{a {b} c}", vec![(1, 5)]),
            ] {
                let full = OneParser::parse(src);
                assert_eq!(
                    spans(&full.objs),
                    objs,
                    "{:?}",
                    String::from_utf8_lossy(src)
                );
                let ctx = OneParser::context(src);
                assert_eq!(spans(&OneParser::find_objs(src).collect::<Vec<_>>()), objs);
                assert_eq!(
                    spans(&OneParser::find_context_objs(src, &ctx).collect::<Vec<_>>()),
                    objs
                );
            }
            assert_eq!(spans(&OneParser::parse(b"*a {b} c*").ems), vec![(1, 8)]);
        }
    }

    // ---- Opaque rings (`balanced = true, parse_inside = false`) ------------ //

    fn rings(src: &[u8]) -> Vec<(u32, u32)> {
        AsymmetricBalancedIter::new(src, b'{', b'}', b'\n', b'\\')
            .map(|s| (s.start, s.end))
            .collect()
    }

    // 32. One span per outer ring, whatever nests in it; every byte of an
    //     open run is an open of its own
    #[test]
    fn test_32_ring_per_outer_ring() {
        assert_eq!(rings(b"{a {b} c} {d}"), vec![(1, 8), (11, 12)]);
        assert_eq!(rings(b"{{a}}"), vec![(1, 4)]);
        assert_eq!(rings(b"{}"), vec![(1, 1)]);
        assert_eq!(rings(b"a} {b}"), vec![(4, 5)]);
    }

    // 33. A ring that never closes yields nothing, and the next open byte
    //     tries, the rest of its own run included
    #[test]
    fn test_33_ring_unclosed_next_open_tries() {
        assert_eq!(rings(b"{a {b} c"), vec![(4, 5)]);
        assert_eq!(rings(b"{{a}"), vec![(2, 3)]);
        assert_eq!(rings(b"{a {b {c} d"), vec![(7, 8)]);
        assert!(rings(b"{{a").is_empty());
    }

    // 34. Rings stay in their paragraph, and escaped bytes count for nothing
    #[test]
    fn test_34_ring_paragraph_and_escapes() {
        assert_eq!(rings(b"{a\nb}"), vec![(1, 4)]);
        assert!(rings(b"{a\n\nb}").is_empty());
        assert_eq!(rings(b"{a\n\n{b}"), vec![(5, 6)]);
        assert_eq!(rings(b"{a\\}b}"), vec![(1, 5)]);
        assert_eq!(rings(b"{a\\{b}"), vec![(1, 5)]);
        assert!(rings(b"\\{a}").is_empty());
        assert_eq!(rings(b"\\{{a}"), vec![(3, 4)]);
    }

    // 35. Open bytes no close balances are searched for once: the scan
    //     stays linear
    #[test]
    fn test_35_ring_linear() {
        let mut src = vec![b'{'; 100_000];
        src.extend(std::iter::repeat_n(b'}', 50_000));
        assert_eq!(rings(&src), vec![(50_001, 149_999)]);
        let mut src = b"{".to_vec();
        for _ in 0..50_000 {
            src.extend_from_slice(b"{{x}");
        }
        assert_eq!(rings(&src).len(), 50_000);
        let src = b"{a ".repeat(20_000);
        assert!(rings(&src).is_empty());
    }

    // 36. With blank bytes or `crlf`, a line of them alone ends the open
    //     frames as an empty line does
    #[test]
    fn test_36_stack_blank_lines() {
        let blank = LineBreaks::new(b'\n', true, b" \t");
        let stack = |src: &[u8]| -> Vec<(u32, u32)> {
            AsymmetricStackIter::new(src, b'{', b'}', true, 4, blank, b'\\')
                .map(|s| (s.start, s.end))
                .collect()
        };
        assert!(stack(b"{a\n \t\nb}").is_empty());
        assert!(stack(b"{a\r\n\r\nb}").is_empty());
        assert!(stack(b"{a\r\n \r\nb}").is_empty());
        assert_eq!(stack(b"{a\r\nb}"), vec![(1, 5)]);
        assert_eq!(stack(b"{a\n \t\n{b}"), vec![(7, 8)]);
    }
}
