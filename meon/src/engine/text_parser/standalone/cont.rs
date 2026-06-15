//! [`ContIter`] — line-continuation block standalone iterator.

use super::common::*;
/// Iterator over line-continuation block spans in a byte slice.
///
/// Groups consecutive lines that start with `byte` into a single [`Span`].
/// The span starts at the `byte` on the first matching line and ends at the
/// start of the first non-matching line (or end of source). A blank line or
/// any line not starting with `byte` terminates the current block.
///
/// Obtained via the generated `Parser::find_*` methods; rarely constructed
/// directly.
pub struct ContIter<'a> {
    src: &'a [u8],
    byte: u8,
    eol: u8,
    pos: usize,
}

impl<'a> ContIter<'a> {
    /// Create an iterator over line-continuation block spans.
    ///
    /// # Parameters
    ///
    /// - `src` — source byte slice to scan.
    /// - `byte` — the continuation marker byte; only lines whose first byte
    ///   equals this value are included in a block (e.g. `b'>'`).
    /// - `eol` — line terminator byte.
    pub fn new(src: &'a [u8], byte: u8, eol: u8) -> Self {
        Self {
            src,
            byte,
            eol,
            pos: 0,
        }
    }
}

impl Iterator for ContIter<'_> {
    type Item = Span;

    fn next(&mut self) -> Option<Span> {
        let src = self.src;
        let len = src.len();
        loop {
            if self.pos >= len {
                return None;
            }

            if src[self.pos] == self.byte {
                let start = self.pos as u32;
                loop {
                    let le = find_line_end(src, self.pos, self.eol);
                    self.pos = if le < len { le + 1 } else { len };
                    if self.pos < len && src[self.pos] == self.byte {
                        continue;
                    }
                    break;
                }
                return Some(Span::new(start, self.pos as u32));
            }

            let le = find_line_end(src, self.pos, self.eol);
            self.pos = if le < len { le + 1 } else { len };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 01. Parses a single matching line that spans until the end of the input without a trailing newline
    #[test]
    fn test_01_single_matching_line() {
        let src = b"> abc";
        let mut iter = ContIter::new(src, b'>', b'\n');

        assert_eq!(iter.next(), Some(Span::new(0, 5)));
        assert_eq!(iter.next(), None);
    }

    // 02. Parses a single matching line including its trailing newline byte
    #[test]
    fn test_02_single_matching_line_with_newline() {
        let src = b"> abc\n";
        let mut iter = ContIter::new(src, b'>', b'\n');

        assert_eq!(iter.next(), Some(Span::new(0, 6)));
        assert_eq!(iter.next(), None);
    }

    // 03. Groups multiple consecutive matching lines into a single continuous span
    #[test]
    fn test_03_multi_line_continuous_block() {
        let src = b"> a\n> b\n";
        let mut iter = ContIter::new(src, b'>', b'\n');

        assert_eq!(iter.next(), Some(Span::new(0, 8)));
        assert_eq!(iter.next(), None);
    }

    // 04. Skips an initial non-matching line and correctly captures the subsequent matching block
    #[test]
    fn test_04_skip_leading_non_matching_lines() {
        let src = b"abc\n> def";
        let mut iter = ContIter::new(src, b'>', b'\n');

        assert_eq!(iter.next(), Some(Span::new(4, 9)));
        assert_eq!(iter.next(), None);
    }

    // 05. Extracts two separate matching blocks that are split by an empty non-matching line
    #[test]
    fn test_05_two_independent_matching_blocks() {
        let src = b"> a\n\n> b";
        let mut iter = ContIter::new(src, b'>', b'\n');

        assert_eq!(iter.next(), Some(Span::new(0, 4)));
        assert_eq!(iter.next(), Some(Span::new(5, 8)));
        assert_eq!(iter.next(), None);
    }

    // 06. Skips a line containing the target byte internally if it does not start with it
    #[test]
    fn test_06_matching_byte_inside_line_ignored() {
        let src = b" a>\n> b";
        let mut iter = ContIter::new(src, b'>', b'\n');

        assert_eq!(iter.next(), Some(Span::new(4, 7)));
        assert_eq!(iter.next(), None);
    }

    // 07. Returns None immediately when initialized with a completely empty source byte slice
    #[test]
    fn test_07_empty_source_slice() {
        let src = b"";
        let mut iter = ContIter::new(src, b'>', b'\n');

        assert_eq!(iter.next(), None);
    }

    // 08. Captures a block consisting of exactly one isolated matching byte marker
    #[test]
    fn test_08_single_matching_byte_only() {
        let src = b">";
        let mut iter = ContIter::new(src, b'>', b'\n');

        assert_eq!(iter.next(), Some(Span::new(0, 1)));
        assert_eq!(iter.next(), None);
    }

    // 09. Skips multiple consecutive non-matching lines before locating a valid block start
    #[test]
    fn test_09_multiple_leading_non_matching_lines() {
        let src = b"a\nb\nc\n> d";
        let mut iter = ContIter::new(src, b'>', b'\n');

        assert_eq!(iter.next(), Some(Span::new(6, 9)));
        assert_eq!(iter.next(), None);
    }

    // 10. Accumulates consecutive lines that contain only the matching byte marker and a newline
    #[test]
    fn test_10_continuous_block_with_empty_matching_lines() {
        let src = b">\n>\n> a";
        let mut iter = ContIter::new(src, b'>', b'\n');

        assert_eq!(iter.next(), Some(Span::new(0, 7)));
        assert_eq!(iter.next(), None);
    }

    // 11. Properly stops the block span at the newline, skipping a trailing empty line at the end
    #[test]
    fn test_11_trailing_empty_line_after_matching_block() {
        let src = b"> a\n\n";
        let mut iter = ContIter::new(src, b'>', b'\n');

        assert_eq!(iter.next(), Some(Span::new(0, 4)));
        assert_eq!(iter.next(), None);
    }

    // 12. Skips a leading non-matching line and accurately aggregates multiple trailing matching lines
    #[test]
    fn test_12_skip_non_matching_then_multi_line_block() {
        let src = b"abc\n>def\n>ghi";
        let mut iter = ContIter::new(src, b'>', b'\n');

        assert_eq!(iter.next(), Some(Span::new(4, 13)));
        assert_eq!(iter.next(), None);
    }

    // 13. Collects multiple matching lines but excludes subsequent non-matching trailing lines
    #[test]
    fn test_13_multi_line_block_followed_by_non_matching() {
        let src = b"> 1\n> 2\nnot";
        let mut iter = ContIter::new(src, b'>', b'\n');

        assert_eq!(iter.next(), Some(Span::new(0, 8)));
        assert_eq!(iter.next(), None);
    }

    // 14. Integrates Windows-style CRLF sequences correctly within line-end span boundary calculations
    #[test]
    fn test_14_windows_style_crlf_continuation() {
        let src = b"> a\r\n> b\r\n";
        let mut iter = ContIter::new(src, b'>', b'\n');

        assert_eq!(iter.next(), Some(Span::new(0, 10)));
        assert_eq!(iter.next(), None);
    }

    // 15. Extracts a matching line that exists at the very end of the file following a non-matching line
    #[test]
    fn test_15_matching_byte_at_very_end_after_newline() {
        let src = b"abc\n>";
        let mut iter = ContIter::new(src, b'>', b'\n');

        assert_eq!(iter.next(), Some(Span::new(4, 5)));
        assert_eq!(iter.next(), None);
    }
}
