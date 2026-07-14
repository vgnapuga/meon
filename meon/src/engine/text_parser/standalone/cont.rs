//! [`ContIter`] — line-continuation block standalone iterator, nesting-aware.

use super::common::*;

/// Physical storage ceiling for the frame stack — the size of the fixed,
/// alloc-free `open`/`pending` arrays. The *behavioural* nesting cap is the
/// grammar's `max_nest`, passed to [`ContIter::new`] and clamped to this bound;
/// no realistic grammar nests 32 deep, so in practice the effective cap equals
/// `max_nest` exactly and this constant is never the binding limit.
const MAX_DEPTH: usize = 32;

/// Iterator over line-continuation block spans in a byte slice, **including
/// same-type self-nesting** on one line (`> >` opens two frames).
///
/// A `cont` marker self-nests positionally: on each line the number of leading
/// marker bytes (each optionally followed by one `sep`/`tab`) is the line's
/// continuation depth, capped at `max_nest`. A frame at depth `d` spans the
/// maximal run of consecutive lines whose depth is `>= d`; it opens at the
/// position of the `d`-th marker on the first line of that run and closes at
/// the start of the first line whose depth drops below `d` (or end of source).
/// This mirrors [`crate::parse_block!`]'s peel/open/close machinery restricted
/// to a single `cont` rule — including its `max_nest` cap: markers past the
/// `max_nest`-th on a line are ordinary content, exactly as in the full parse.
/// So — unlike the old flat grouper — `find_*` now sees the same nesting the
/// full parse does: `> > a` yields two spans, not one.
///
/// Frames closing at the same point are emitted **innermost-first**, matching
/// the full parser's close order.
///
/// This is the one standalone family that carries genuine cross-line state (a
/// small bounded frame stack). Line rules can never nest, and inline rules
/// already nest via the shared engine stack; only same-type block continuation
/// needed this. Everything else in this module remains a stateless forward
/// scan.
///
/// Obtained via the generated `Parser::find_*` methods; rarely constructed
/// directly.
pub struct ContIter<'a> {
    src: &'a [u8],
    byte: u8,
    eol: u8,
    sep: u8,
    tab: u8,
    /// Effective nesting cap: `min(max_nest, MAX_DEPTH)`. No line opens more
    /// than this many frames; deeper markers are treated as content.
    cap: usize,
    /// Current line-start scan position.
    pos: usize,
    /// Start offsets of currently-open frames, outermost first.
    open: [u32; MAX_DEPTH],
    open_len: usize,
    /// Closed spans buffered in emit (innermost-first) order, drained by `next`.
    pending: [Span; MAX_DEPTH],
    pending_len: usize,
    pending_idx: usize,
}

impl<'a> ContIter<'a> {
    /// Create a nesting-aware continuation-block iterator.
    ///
    /// # Parameters
    ///
    /// - `src` — source byte slice to scan.
    /// - `byte` — the continuation marker byte (e.g. `b'>'`); a line's depth is
    ///   the number of leading `byte` markers, each optionally followed by one
    ///   `sep`/`tab`.
    /// - `eol` — line terminator byte.
    /// - `sep` / `tab` — the separator bytes a single marker may consume after
    ///   itself, so `"> > x"` reads as depth 2 rather than one marker plus
    ///   content. Same convention as [`crate::parse_block!`].
    /// - `max_nest` — the grammar's bounded-nesting cap. At most this many
    ///   frames open per line (clamped to the physical [`MAX_DEPTH`] storage
    ///   ceiling); markers beyond it are content, matching the full parse.
    pub fn new(src: &'a [u8], byte: u8, eol: u8, sep: u8, tab: u8, max_nest: usize) -> Self {
        Self {
            src,
            byte,
            eol,
            sep,
            tab,
            cap: max_nest.min(MAX_DEPTH),
            pos: 0,
            open: [0u32; MAX_DEPTH],
            open_len: 0,
            pending: [Span::new(0, 0); MAX_DEPTH],
            pending_len: 0,
            pending_idx: 0,
        }
    }

    /// Position just past a marker at `p` (marker byte + one optional `sep`/`tab`).
    #[inline]
    fn consume_marker(&self, p: usize, le: usize) -> usize {
        if p + 1 < le && (self.src[p + 1] == self.sep || self.src[p + 1] == self.tab) {
            p + 2
        } else {
            p + 1
        }
    }

    /// Close frames `[from, open_len)` at `end`, innermost-first, into `pending`.
    #[inline]
    fn close_from(&mut self, from: usize, end: u32) {
        let mut i = self.open_len;
        while i > from {
            i -= 1;
            self.pending[self.pending_len] = Span::new(self.open[i], end);
            self.pending_len += 1;
        }
        self.open_len = from;
    }
}

impl Iterator for ContIter<'_> {
    type Item = Span;

    fn next(&mut self) -> Option<Span> {
        let src = self.src;
        let len = src.len();
        loop {
            // Drain buffered closed spans first.
            if self.pending_idx < self.pending_len {
                let s = self.pending[self.pending_idx];
                self.pending_idx += 1;
                return Some(s);
            }
            self.pending_len = 0;
            self.pending_idx = 0;

            // End of input: flush every still-open frame, innermost-first.
            if self.open_len > 0 && self.pos >= len {
                self.close_from(0, len as u32);
                continue;
            }

            // Streaming scan: the next marker byte anywhere ahead. Lines
            // without a marker are never walked.
            let hit = memchr::memchr(self.byte, &src[self.pos..]).map(|r| r + self.pos);

            let ls = if self.open_len > 0 {
                // Frames are open, and `pos` is the start of the line that
                // must continue them. A hit anywhere past `pos` — or no hit
                // at all — means that line does not begin with the marker, so
                // every frame closes at its start; the hit (if any) is then
                // re-examined as a fresh opener.
                match hit {
                    Some(q) if q == self.pos => q,
                    _ => {
                        self.close_from(0, self.pos as u32);
                        self.pos = hit.unwrap_or(len);
                        continue;
                    }
                }
            } else {
                let q = hit?;
                // A fresh run must begin its line.
                if q > 0 && src[q - 1] != self.eol {
                    self.pos = q + 1;
                    continue;
                }
                q
            };

            // Process the marker line at `ls` (a line start by construction):
            // peel the open frames, then open new ones — the same
            // peel/open/close machinery as `parse_block!`, re-peeling the
            // line whenever a missing marker closed part of the stack.
            let le = find_line_end(src, ls, self.eol);
            loop {
                let mut cur = ls;
                let mut fi = 0;
                let mut broke = false;
                while fi < self.open_len {
                    if cur < le && src[cur] == self.byte {
                        cur = self.consume_marker(cur, le);
                        fi += 1;
                    } else {
                        self.close_from(fi, ls as u32);
                        broke = true;
                        break;
                    }
                }
                if !broke {
                    // Open phase: further markers open new frames, bounded by
                    // the grammar's `max_nest` cap. Markers past the cap are
                    // ordinary content, matching the full parse.
                    while cur < le && src[cur] == self.byte && self.open_len < self.cap {
                        self.open[self.open_len] = cur as u32;
                        self.open_len += 1;
                        cur = self.consume_marker(cur, le);
                    }
                    break;
                }
            }

            self.pos = if le < len { le + 1 } else { len };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A generous cap so the general fixtures exercise nesting without the cap
    // ever binding; the cap itself is covered by test_23.
    fn iter(src: &[u8]) -> ContIter<'_> {
        ContIter::new(src, b'>', b'\n', b' ', b'\t', MAX_DEPTH)
    }

    // 01. Parses a single matching line that spans until the end of the input without a trailing newline
    #[test]
    fn test_01_single_matching_line() {
        let mut it = iter(b"> abc");
        assert_eq!(it.next(), Some(Span::new(0, 5)));
        assert_eq!(it.next(), None);
    }

    // 02. Parses a single matching line including its trailing newline byte
    #[test]
    fn test_02_single_matching_line_with_newline() {
        let mut it = iter(b"> abc\n");
        assert_eq!(it.next(), Some(Span::new(0, 6)));
        assert_eq!(it.next(), None);
    }

    // 03. Groups multiple consecutive matching lines into a single continuous span
    #[test]
    fn test_03_multi_line_continuous_block() {
        let mut it = iter(b"> a\n> b\n");
        assert_eq!(it.next(), Some(Span::new(0, 8)));
        assert_eq!(it.next(), None);
    }

    // 04. Skips an initial non-matching line and correctly captures the subsequent matching block
    #[test]
    fn test_04_skip_leading_non_matching_lines() {
        let mut it = iter(b"abc\n> def");
        assert_eq!(it.next(), Some(Span::new(4, 9)));
        assert_eq!(it.next(), None);
    }

    // 05. Extracts two separate matching blocks that are split by an empty non-matching line
    #[test]
    fn test_05_two_independent_matching_blocks() {
        let mut it = iter(b"> a\n\n> b");
        assert_eq!(it.next(), Some(Span::new(0, 4)));
        assert_eq!(it.next(), Some(Span::new(5, 8)));
        assert_eq!(it.next(), None);
    }

    // 06. Skips a line containing the target byte internally if it does not start with it
    #[test]
    fn test_06_matching_byte_inside_line_ignored() {
        let mut it = iter(b" a>\n> b");
        assert_eq!(it.next(), Some(Span::new(4, 7)));
        assert_eq!(it.next(), None);
    }

    // 07. Returns None immediately when initialized with a completely empty source byte slice
    #[test]
    fn test_07_empty_source_slice() {
        let mut it = iter(b"");
        assert_eq!(it.next(), None);
    }

    // 08. Captures a block consisting of exactly one isolated matching byte marker
    #[test]
    fn test_08_single_matching_byte_only() {
        let mut it = iter(b">");
        assert_eq!(it.next(), Some(Span::new(0, 1)));
        assert_eq!(it.next(), None);
    }

    // 09. Skips multiple consecutive non-matching lines before locating a valid block start
    #[test]
    fn test_09_multiple_leading_non_matching_lines() {
        let mut it = iter(b"a\nb\nc\n> d");
        assert_eq!(it.next(), Some(Span::new(6, 9)));
        assert_eq!(it.next(), None);
    }

    // 10. Accumulates consecutive lines that contain only the matching byte marker and a newline
    #[test]
    fn test_10_continuous_block_with_empty_matching_lines() {
        let mut it = iter(b">\n>\n> a");
        assert_eq!(it.next(), Some(Span::new(0, 7)));
        assert_eq!(it.next(), None);
    }

    // 11. Properly stops the block span at the newline, skipping a trailing empty line at the end
    #[test]
    fn test_11_trailing_empty_line_after_matching_block() {
        let mut it = iter(b"> a\n\n");
        assert_eq!(it.next(), Some(Span::new(0, 4)));
        assert_eq!(it.next(), None);
    }

    // 12. Skips a leading non-matching line and accurately aggregates multiple trailing matching lines
    #[test]
    fn test_12_skip_non_matching_then_multi_line_block() {
        let mut it = iter(b"abc\n>def\n>ghi");
        assert_eq!(it.next(), Some(Span::new(4, 13)));
        assert_eq!(it.next(), None);
    }

    // 13. Collects multiple matching lines but excludes subsequent non-matching trailing lines
    #[test]
    fn test_13_multi_line_block_followed_by_non_matching() {
        let mut it = iter(b"> 1\n> 2\nnot");
        assert_eq!(it.next(), Some(Span::new(0, 8)));
        assert_eq!(it.next(), None);
    }

    // 14. Integrates Windows-style CRLF sequences correctly within line-end span boundary calculations
    #[test]
    fn test_14_windows_style_crlf_continuation() {
        let mut it = iter(b"> a\r\n> b\r\n");
        assert_eq!(it.next(), Some(Span::new(0, 10)));
        assert_eq!(it.next(), None);
    }

    // 15. Extracts a matching line that exists at the very end of the file following a non-matching line
    #[test]
    fn test_15_matching_byte_at_very_end_after_newline() {
        let mut it = iter(b"abc\n>");
        assert_eq!(it.next(), Some(Span::new(4, 5)));
        assert_eq!(it.next(), None);
    }

    // ---- Nesting-aware behaviour (the new contract) -------------------- //

    // 16. `> >` on one line opens two frames: inner then outer, both to run end
    #[test]
    fn test_16_same_line_nesting_two_frames() {
        // "> > a" — outer marker at 0, inner marker at 2, both span to EOF.
        let mut it = iter(b"> > a");
        assert_eq!(it.next(), Some(Span::new(2, 5))); // inner, innermost-first
        assert_eq!(it.next(), Some(Span::new(0, 5))); // outer
        assert_eq!(it.next(), None);
    }

    // 17. The heavy-corpus shape: depths [1, 1, 2] over three lines, then a
    //     non-cont line closes the run. Two spans (inner + outer).
    #[test]
    fn test_17_depths_1_1_2_then_close() {
        let src = b"> a\n> b\n> > c\nplain\n";
        // run spans lines 0..2; the non-cont "plain" line starts at offset 14,
        // so both frames close there (end = start of the breaking line, which
        // includes the preceding newline — same convention as leaf test_13).
        // outer opens at 0; inner opens at the 2nd '>' of line 2, offset 10.
        let mut it = iter(src);
        assert_eq!(it.next(), Some(Span::new(10, 14))); // inner
        assert_eq!(it.next(), Some(Span::new(0, 14))); // outer
        assert_eq!(it.next(), None);
    }

    // 18. Depth drop mid-run: [2, 1] — inner closes at line 2 start, outer
    //     spans the whole run.
    #[test]
    fn test_18_depth_drop_closes_inner_early() {
        let src = b"> > a\n> b\n";
        // inner opens at offset 2 (line 0), closes at line 1 start = 6.
        // outer opens at 0, spans to EOF = 10.
        let mut it = iter(src);
        assert_eq!(it.next(), Some(Span::new(2, 6))); // inner, closed early
        assert_eq!(it.next(), Some(Span::new(0, 10))); // outer, whole run
        assert_eq!(it.next(), None);
    }

    // 19. Oscillating depth [2, 1, 2] yields three spans: two inner runs plus
    //     the single outer run.
    #[test]
    fn test_19_oscillating_depth() {
        let src = b"> > a\n> b\n> > c\n";
        // line0 bytes 0..6, line1 6..10, line2 10..16; EOF = 16 (trailing '\n').
        // inner#1: opens line0 @2, closes line1 start = 6.
        // inner#2: opens line2 @12, closes EOF = 16.
        // outer:   opens line0 @0, closes EOF = 16.
        let mut it = iter(src);
        assert_eq!(it.next(), Some(Span::new(2, 6))); // inner#1 (closed at line1)
        assert_eq!(it.next(), Some(Span::new(12, 16))); // inner#2 (flushed at EOF, innermost-first)
        assert_eq!(it.next(), Some(Span::new(0, 16))); // outer
        assert_eq!(it.next(), None);
    }

    // 20. Back-to-back markers without a separator (`>>x`) still nest.
    #[test]
    fn test_20_markers_without_separator() {
        let mut it = iter(b">>x");
        assert_eq!(it.next(), Some(Span::new(1, 3))); // inner (marker at 1)
        assert_eq!(it.next(), Some(Span::new(0, 3))); // outer (marker at 0)
        assert_eq!(it.next(), None);
    }

    // 21. Triple depth `> > > deep` opens three frames, flushed innermost-first
    #[test]
    fn test_21_triple_depth() {
        // markers at 0, 2, 4; all span to EOF = 11.
        let mut it = iter(b"> > > deep\n");
        assert_eq!(it.next(), Some(Span::new(4, 11)));
        assert_eq!(it.next(), Some(Span::new(2, 11)));
        assert_eq!(it.next(), Some(Span::new(0, 11)));
        assert_eq!(it.next(), None);
    }

    // 22. A mixed multi-run document: two independent nested runs separated by a
    //     plain line, each with its own inner frame that closes before the outer
    #[test]
    fn test_22_mixed_multi_run() {
        //  intro\n  > a\n  > > b\n  > c\n  plain\n  > > d\n  tail\n
        //  0..5     6..9  10..15   16..19 20..25   26..31   32..
        // run 1: outer opens @6, inner opens @12 (line "> > b"); inner closes at
        //        "> c" start = 16, outer closes at "plain" start = 20.
        // run 2: outer @26, inner @28; both close at "tail" start = 32.
        let mut it = iter(b"intro\n> a\n> > b\n> c\nplain\n> > d\ntail\n");
        assert_eq!(it.next(), Some(Span::new(12, 16))); // run1 inner
        assert_eq!(it.next(), Some(Span::new(6, 20))); // run1 outer
        assert_eq!(it.next(), Some(Span::new(28, 32))); // run2 inner
        assert_eq!(it.next(), Some(Span::new(26, 32))); // run2 outer
        assert_eq!(it.next(), None);
    }

    // 23. `max_nest` caps opening: with cap 2, `> > > > x` opens only two
    //     frames; the 3rd and 4th markers are content, matching the full
    //     parse's behaviour at its own `max_nest`.
    #[test]
    fn test_23_max_nest_caps_opening() {
        // markers at 0, 2, 4, 6; cap 2 opens frames at 0 and 2 only.
        let mut it = ContIter::new(b"> > > > x\n", b'>', b'\n', b' ', b'\t', 2);
        assert_eq!(it.next(), Some(Span::new(2, 10))); // inner (2nd marker)
        assert_eq!(it.next(), Some(Span::new(0, 10))); // outer (1st marker)
        assert_eq!(it.next(), None);
    }
}
