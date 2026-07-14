//! [`AsymmetricExactIter`] — asymmetric exact-delimiter standalone iterator.

use super::common::*;
/// Iterator over asymmetric exact-delimiter spans in a byte slice.
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
/// Escape sequences are respected on the opening run. The closing byte is
/// found by a plain scan (no escape check on close).
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
            let mut j = cs;
            let close = loop {
                let Some(r) = memchr::memchr2(self.close, self.eol, &src[j..]) else {
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

    // 11. A close marker on the next line of the same paragraph now completes
    //     the pair — a single newline no longer separates open from close.
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

    // ---- Paragraph-bounded behaviour (the new contract) ----------------- //

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
}
