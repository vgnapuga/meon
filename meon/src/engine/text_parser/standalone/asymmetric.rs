//! [`AsymmetricExactIter`] — asymmetric exact-delimiter standalone iterator.

use super::common::*;
/// Iterator over asymmetric exact-delimiter spans in a byte slice.
///
/// Scans line by line for sequences where `count` consecutive `open` bytes are
/// followed by content and then a single `close` byte. The returned [`Span`]
/// covers the inner content, excluding both the opening run and the closing byte.
///
/// Escape sequences are respected on the opening run. Matching does not cross
/// line boundaries.
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
    line_end: usize,
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
    /// - `eol` — line terminator byte; matching never crosses a line boundary.
    /// - `escape` — escape prefix byte; an odd number of preceding escape bytes
    ///   suppresses the opening run.
    pub fn new(src: &'a [u8], open: u8, close: u8, count: u32, eol: u8, escape: u8) -> Self {
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
        }
    }
}

impl Iterator for AsymmetricExactIter<'_> {
    type Item = Span;

    fn next(&mut self) -> Option<Span> {
        let src = self.src;
        loop {
            while self.pos >= self.line_end {
                let (next, end) = advance_line(src, self.line_end, self.eol)?;
                self.pos = next;
                self.line_end = end;
            }

            let Some(r) = memchr::memchr(self.open, &src[self.pos..self.line_end]) else {
                self.pos = self.line_end;
                continue;
            };
            let p = self.pos + r;

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
            match memchr::memchr(self.close, &src[cs..self.line_end]) {
                Some(r2) => {
                    let cp = cs + r2;
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

    // 01. Parses a standard matching pattern with the exact count of open markers
    #[test]
    fn test_01_standard_exact_match() {
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

    // 07. Skips an unclosed sequence on the current line and recovers on a subsequent line
    #[test]
    fn test_07_unclosed_sequence_recovers_on_next_line() {
        let src = b"<<unclosed\n<<valid>";
        let mut iter = AsymmetricExactIter::new(src, b'<', b'>', 2, b'\n', b'\\');

        assert_eq!(iter.next(), Some(Span::new(13, 18)));
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

    // 11. Rejects patterns split across a line boundary as parsing cannot cross newlines
    #[test]
    fn test_11_newline_separating_close_marker_fails() {
        let src = b"<<a\n>";
        let mut iter = AsymmetricExactIter::new(src, b'<', b'>', 2, b'\n', b'\\');

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
}
