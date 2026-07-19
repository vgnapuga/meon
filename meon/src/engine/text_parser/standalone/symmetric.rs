//! [`SymmetricExactIter`] — symmetric exact-delimiter standalone iterator.

use super::common::*;
/// Iterator over symmetric exact-delimiter spans in a byte slice.
///
/// A single streaming scan over the source: opening runs are found with one
/// `memchr` pass (line boundaries are irrelevant while nothing is pending),
/// and both the opening and closing run must consist of exactly `count`
/// consecutive bytes. The returned [`Span`] covers the inner content between
/// the delimiters, excluding the delimiter bytes themselves.
///
/// Matching is **paragraph-bounded**: the closing run may sit past a single
/// `eol` (the pair spans lines of one paragraph, as in the full parse), but an
/// empty line — two consecutive `eol` bytes — or the end of input aborts the
/// pending opener.
///
/// Escape sequences are respected on the opening delimiter: an odd number of
/// preceding `escape` bytes suppresses the match. The closing delimiter is
/// found by a plain scan (no escape check on close).
///
/// Obtained via the generated `Parser::find_*` methods; rarely constructed
/// directly.
pub struct SymmetricExactIter<'a> {
    src: &'a [u8],
    byte: u8,
    count: u32,
    eol: u8,
    escape: u8,
    pos: usize,
}

impl<'a> SymmetricExactIter<'a> {
    /// Create an iterator over symmetric exact-delimiter spans.
    ///
    /// # Parameters
    ///
    /// - `src` — source byte slice to scan.
    /// - `byte` — the delimiter byte (e.g. `b'*'`).
    /// - `count` — exact number of consecutive delimiter bytes required for
    ///   both the opening and closing run.
    /// - `eol` — line terminator byte; a pair may span single line breaks, but
    ///   never an empty line (two consecutive `eol` bytes).
    /// - `escape` — escape prefix byte; an odd number of preceding escape bytes
    ///   suppresses the opening delimiter.
    pub fn new(src: &'a [u8], byte: u8, count: u32, eol: u8, escape: u8) -> Self {
        Self {
            src,
            byte,
            count,
            eol,
            escape,
            pos: 0,
        }
    }
}

impl Iterator for SymmetricExactIter<'_> {
    type Item = Span;

    fn next(&mut self) -> Option<Span> {
        let src = self.src;
        let len = src.len();
        loop {
            // Open scan: one streaming pass for the delimiter byte.
            let p = memchr::memchr(self.byte, &src[self.pos..])? + self.pos;

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

            // Close scan, bounded by the end of the paragraph: a single `eol`
            // is ordinary content, two in a row (an empty line) — or the end
            // of input — abort the pending opener.
            let cs = end;
            let mut j = cs;
            let close = loop {
                let Some(r) = memchr::memchr2(self.byte, self.eol, &src[j..]) else {
                    break None;
                };
                let q = j + r;
                if src[q] == self.eol {
                    if q + 1 >= len || src[q + 1] == self.eol {
                        break None;
                    }
                    j = q + 1;
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

#[cfg(test)]
mod tests {
    use super::*;

    // 01. Basic single match: one line, one valid symmetric token
    #[test]
    fn test_01_basic_single_match() {
        let src = b"abc *content* def";
        let mut iter = SymmetricExactIter::new(src, b'*', 1, b'\n', b'\\');

        assert_eq!(iter.next(), Some(Span::new(5, 12)));
        assert_eq!(iter.next(), None);
    }

    // 02. Complete absence of the searched byte in the string
    #[test]
    fn test_02_no_matches() {
        let src = b"just some text without symbols";
        let mut iter = SymmetricExactIter::new(src, b'"', 1, b'\n', b'\\');

        assert_eq!(iter.next(), None);
    }

    // 03. Multiple independent valid tokens on a single line
    #[test]
    fn test_03_multiple_matches_single_line() {
        let src = b"|first| middle |second|";
        let mut iter = SymmetricExactIter::new(src, b'|', 1, b'\n', b'\\');

        assert_eq!(iter.next(), Some(Span::new(1, 6)));
        assert_eq!(iter.next(), Some(Span::new(16, 22)));
        assert_eq!(iter.next(), None);
    }

    // 04. Verify the count parameter (e.g., matches double markers like ** for bold, ignores single *)
    #[test]
    fn test_04_exact_count_match() {
        let src = b"text **bold** and *italic*";
        let mut iter = SymmetricExactIter::new(src, b'*', 2, b'\n', b'\\');

        assert_eq!(iter.next(), Some(Span::new(7, 11)));
        assert_eq!(iter.next(), None);
    }

    // 05. Ignore byte sequences longer than the expected count (e.g., *** is skipped when looking for **)
    #[test]
    fn test_05_count_mismatch_too_long() {
        let src = b"text ***triple*** and **double**";
        let mut iter = SymmetricExactIter::new(src, b'*', 2, b'\n', b'\\');

        assert_eq!(iter.next(), Some(Span::new(24, 30)));
        assert_eq!(iter.next(), None);
    }

    // 06. Ignore byte sequences shorter than the expected count (single markers skipped, double processed)
    #[test]
    fn test_06_count_mismatch_too_short() {
        let src = b"text *single* and **double**";
        let mut iter = SymmetricExactIter::new(src, b'*', 2, b'\n', b'\\');

        assert_eq!(iter.next(), Some(Span::new(20, 26)));
        assert_eq!(iter.next(), None);
    }

    // 07. Escaping specific: the first unescaped byte becomes the opening marker and matches with the next available pair
    #[test]
    fn test_07_escaped_start_byte() {
        let src = b"escaped \\*match* and unescaped *match*";
        let mut iter = SymmetricExactIter::new(src, b'*', 1, b'\n', b'\\');

        assert_eq!(iter.next(), Some(Span::new(16, 31)));
        assert_eq!(iter.next(), None);
    }

    // 08. Even number of escape characters means the escape character itself is escaped, so the marker remains active
    #[test]
    fn test_08_double_escaped_start_byte() {
        let src = b"double_escaped \\\\*match*";
        let mut iter = SymmetricExactIter::new(src, b'*', 1, b'\n', b'\\');

        assert_eq!(iter.next(), Some(Span::new(18, 23)));
        assert_eq!(iter.next(), None);
    }

    // 09. Handling line boundaries across multiline text processing
    #[test]
    fn test_09_multiline_behavior() {
        let src = b"line1 *one*\nline2\nline3 *two*";
        let mut iter = SymmetricExactIter::new(src, b'*', 1, b'\n', b'\\');

        assert_eq!(iter.next(), Some(Span::new(7, 10)));
        assert_eq!(iter.next(), Some(Span::new(25, 28)));
        assert_eq!(iter.next(), None);
    }

    // 10. An opener left unclosed on its own line pairs with the next closing
    //     run past a single newline — matching is paragraph-bounded, not
    //     line-bounded.
    #[test]
    fn test_10_pairs_across_single_newline() {
        let src = b"this *is open ended without close\nnext line *valid*";
        let mut iter = SymmetricExactIter::new(src, b'*', 1, b'\n', b'\\');

        assert_eq!(iter.next(), Some(Span::new(6, 44)));
        assert_eq!(iter.next(), None);
    }

    // 11. Greedy consumption: redundant consecutive markers are completely discarded if the total length mismatches count
    #[test]
    fn test_11_empty_content_between_markers() {
        let src = b"empty **** here";
        let mut iter = SymmetricExactIter::new(src, b'*', 2, b'\n', b'\\');

        assert_eq!(iter.next(), None);
    }

    // 12. Mismatch in closing marker count (valid opening marker but invalid closing count leads to an aborted span)
    #[test]
    fn test_12_closing_marker_count_mismatch() {
        let src = b"start **content* end";
        let mut iter = SymmetricExactIter::new(src, b'*', 2, b'\n', b'\\');

        assert_eq!(iter.next(), None);
    }

    // 13. Markers located at the exact boundaries of the line (start and end of the slice)
    #[test]
    fn test_13_boundary_edges_of_line() {
        let src = b"*content*";
        let mut iter = SymmetricExactIter::new(src, b'*', 1, b'\n', b'\\');

        assert_eq!(iter.next(), Some(Span::new(1, 8)));
        assert_eq!(iter.next(), None);
    }

    // 14. Multiple empty lines preceding a line containing a valid token
    #[test]
    fn test_14_multiple_empty_lines_advance() {
        let src = b"\n\n\n   *match* \n\n";
        let mut iter = SymmetricExactIter::new(src, b'*', 1, b'\n', b'\\');

        assert_eq!(iter.next(), Some(Span::new(7, 12)));
        assert_eq!(iter.next(), None);
    }

    // 15. Specific implementation behavior: the closing marker loop does not validate escape sequences
    #[test]
    fn test_15_closing_marker_ignores_escapes() {
        let src = b"test *content\\*";
        let mut iter = SymmetricExactIter::new(src, b'*', 1, b'\n', b'\\');

        assert_eq!(iter.next(), Some(Span::new(6, 14)));
        assert_eq!(iter.next(), None);
    }

    // ---- Paragraph-bounded behaviour (the new contract) ----------------- //

    // 16. An empty line (two consecutive eol bytes) aborts a pending opener:
    //     delimiters in different paragraphs never pair.
    #[test]
    fn test_16_empty_line_aborts_pending_open() {
        let src = b"para one *open\n\nclosed* in para two";
        let mut iter = SymmetricExactIter::new(src, b'*', 1, b'\n', b'\\');

        assert_eq!(iter.next(), None);
    }

    // 17. A pair split across a single newline within one paragraph matches,
    //     and the span includes the embedded newline.
    #[test]
    fn test_17_pair_spans_single_newline() {
        let src = b"*multi\nline*";
        let mut iter = SymmetricExactIter::new(src, b'*', 1, b'\n', b'\\');

        assert_eq!(iter.next(), Some(Span::new(1, 11)));
        assert_eq!(iter.next(), None);
    }

    // 18. An opener at the very end of input (nothing after it) never pairs.
    #[test]
    fn test_18_open_at_eof_unclosed() {
        let src = b"tail *";
        let mut iter = SymmetricExactIter::new(src, b'*', 1, b'\n', b'\\');

        assert_eq!(iter.next(), None);
    }

    // 19. After a paragraph break aborts one opener, a fresh pair in the next
    //     paragraph still matches normally.
    #[test]
    fn test_19_fresh_pair_after_paragraph_break() {
        let src = b"*a\n\n*b*";
        let mut iter = SymmetricExactIter::new(src, b'*', 1, b'\n', b'\\');

        assert_eq!(iter.next(), Some(Span::new(5, 6)));
        assert_eq!(iter.next(), None);
    }
}
