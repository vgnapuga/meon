//! [`LineMarkerIter`] — line-marker element standalone iterator.

use super::common::*;
/// Iterator over line-marker elements in a byte slice.
///
/// Matches lines that start with 1–`max` consecutive occurrences of `byte`
/// followed by `sep` or end of line. The count is passed to `make` to produce
/// the metadata value `T`. Yields `(meta, span)` where `span` covers the
/// content after the marker and its trailing separator.
///
/// The scan is streaming: one `memchr` pass finds candidate marker bytes and
/// an O(1) previous-byte check keeps only the ones that begin a line, so lines
/// without the marker are never visited at all.
///
/// Obtained via the generated `Parser::find_*` methods; rarely constructed
/// directly.
pub struct LineMarkerIter<'a, T, F>
where
    F: Fn(u8) -> T,
{
    src: &'a [u8],
    byte: u8,
    max: u8,
    eol: u8,
    sep: u8,
    make: F,
    pos: usize,
    _t: std::marker::PhantomData<T>,
}

impl<'a, T, F> LineMarkerIter<'a, T, F>
where
    F: Fn(u8) -> T,
{
    /// Create an iterator over line-marker elements.
    ///
    /// # Parameters
    ///
    /// - `src` — source byte slice to scan.
    /// - `byte` — the marker byte that must appear at the start of a line.
    /// - `max` — maximum number of consecutive `byte` occurrences to count;
    ///   runs longer than `max` do not match.
    /// - `eol` — line terminator byte.
    /// - `sep` — separator byte required immediately after the marker run
    ///   (or end of line).
    /// - `make` — closure that receives the count of marker bytes and constructs
    ///   the metadata value `T`.
    pub fn new(src: &'a [u8], byte: u8, max: u8, eol: u8, sep: u8, make: F) -> Self {
        Self {
            src,
            byte,
            max,
            eol,
            sep,
            make,
            pos: 0,
            _t: std::marker::PhantomData,
        }
    }
}

impl<T, F> Iterator for LineMarkerIter<'_, T, F>
where
    F: Fn(u8) -> T,
{
    type Item = (T, Span);

    fn next(&mut self) -> Option<(T, Span)> {
        let src = self.src;
        let len = src.len();
        loop {
            let p = memchr::memchr(self.byte, &src[self.pos..])? + self.pos;

            // The marker run must begin its line.
            if p > 0 && src[p - 1] != self.eol {
                self.pos = p + 1;
                continue;
            }

            let mut c = 0u8;
            let mut i = p;
            while i < len && src[i] == self.byte && c < self.max {
                c += 1;
                i += 1;
            }

            // The run must terminate the line or be followed by `sep`.
            if i >= len || src[i] == self.eol {
                let meta = (self.make)(c);
                self.pos = if i < len { i + 1 } else { len };
                return Some((meta, Span::new(i as u32, i as u32)));
            }
            if src[i] == self.sep {
                let cs = i + 1;
                let le = find_line_end(src, cs, self.eol);
                let meta = (self.make)(c);
                self.pos = if le < len { le + 1 } else { len };
                return Some((meta, Span::new(cs as u32, le as u32)));
            }

            // Run too long or missing separator: skip it. Deeper bytes of the
            // same run fail the line-start check in O(1).
            self.pos = i + 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stub_make(c: u8) -> u8 {
        c
    }

    // 01. Parses a basic single marker followed by a separator and text content
    #[test]
    fn test_01_basic_single_marker() {
        let src = b"# Hello";
        let mut iter = LineMarkerIter::new(src, b'#', 6, b'\n', b' ', stub_make);

        assert_eq!(iter.next(), Some((1, Span::new(2, 7))));
        assert_eq!(iter.next(), None);
    }

    // 02. Parses multiple consecutive markers within the allowed maximum limit
    #[test]
    fn test_02_multiple_markers_under_max() {
        let src = b"### Heading";
        let mut iter = LineMarkerIter::new(src, b'#', 6, b'\n', b' ', stub_make);

        assert_eq!(iter.next(), Some((3, Span::new(4, 11))));
        assert_eq!(iter.next(), None);
    }

    // 03. Parses consecutive markers that hit the maximum allowed threshold exactly
    #[test]
    fn test_03_markers_at_maximum_limit() {
        let src = b"###### Title";
        let mut iter = LineMarkerIter::new(src, b'#', 6, b'\n', b' ', stub_make);

        assert_eq!(iter.next(), Some((6, Span::new(7, 12))));
        assert_eq!(iter.next(), None);
    }

    // 04. Rejects the line entirely if the marker count exceeds the maximum limit due to trailing non-separator
    #[test]
    fn test_04_markers_exceeding_max_limit() {
        let src = b"####### Heading";
        let mut iter = LineMarkerIter::new(src, b'#', 6, b'\n', b' ', stub_make);

        assert_eq!(iter.next(), None);
    }

    // 05. Successfully matches markers when the line ends immediately with no content or separator
    #[test]
    fn test_05_markers_only_at_line_end() {
        let src = b"###";
        let mut iter = LineMarkerIter::new(src, b'#', 6, b'\n', b' ', stub_make);

        assert_eq!(iter.next(), Some((3, Span::new(3, 3))));
        assert_eq!(iter.next(), None);
    }

    // 06. Rejects the line if the expected separator character is missing right after the markers
    #[test]
    fn test_06_missing_required_separator() {
        let src = b"#NoSeparatorText";
        let mut iter = LineMarkerIter::new(src, b'#', 6, b'\n', b' ', stub_make);

        assert_eq!(iter.next(), None);
    }

    // 07. Skips lines that do not start with the specified marker byte
    #[test]
    fn test_07_line_not_starting_with_marker() {
        let src = b" Not a marker";
        let mut iter = LineMarkerIter::new(src, b'#', 6, b'\n', b' ', stub_make);

        assert_eq!(iter.next(), None);
    }

    // 08. Skips an invalid or non-matching line and continues to parse subsequent valid lines
    #[test]
    fn test_08_skip_invalid_line_to_valid() {
        let src = b"invalid line\n# valid line\nskip again";
        let mut iter = LineMarkerIter::new(src, b'#', 6, b'\n', b' ', stub_make);

        assert_eq!(iter.next(), Some((1, Span::new(15, 25))));
        assert_eq!(iter.next(), None);
    }

    // 09. Returns None immediately when the input slice is completely empty
    #[test]
    fn test_09_empty_input_slice() {
        let src = b"";
        let mut iter = LineMarkerIter::new(src, b'#', 6, b'\n', b' ', stub_make);

        assert_eq!(iter.next(), None);
    }

    // 10. Safely cycles through and ignores consecutive empty lines
    #[test]
    fn test_10_consecutive_empty_lines() {
        let src = b"\n\n\n";
        let mut iter = LineMarkerIter::new(src, b'#', 6, b'\n', b' ', stub_make);

        assert_eq!(iter.next(), None);
    }

    // 11. Processes a valid marker line that ends exactly at the EOF without a trailing newline
    #[test]
    fn test_11_missing_trailing_eol_at_eof() {
        let src = b"## TextAtEof";
        let mut iter = LineMarkerIter::new(src, b'#', 6, b'\n', b' ', stub_make);

        assert_eq!(iter.next(), Some((2, Span::new(3, 12))));
        assert_eq!(iter.next(), None);
    }

    // 12. Rejects lines where the line starts with a separator instead of the marker byte
    #[test]
    fn test_12_line_starts_with_separator_byte() {
        let src = b" # text";
        let mut iter = LineMarkerIter::new(src, b'#', 6, b'\n', b' ', stub_make);

        assert_eq!(iter.next(), None);
    }

    // 13. Validates a line containing only a marker and a single separator with no trailing text
    #[test]
    fn test_13_marker_and_separator_only() {
        let src = b"# ";
        let mut iter = LineMarkerIter::new(src, b'#', 6, b'\n', b' ', stub_make);

        assert_eq!(iter.next(), Some((1, Span::new(2, 2))));
        assert_eq!(iter.next(), None);
    }

    // 14. Skips only the first separator byte, leaving subsequent duplicate separators inside the span
    #[test]
    fn test_14_multiple_consecutive_separators() {
        let src = b"#  two spaces";
        let mut iter = LineMarkerIter::new(src, b'#', 6, b'\n', b' ', stub_make);

        assert_eq!(iter.next(), Some((1, Span::new(2, 13))));
        assert_eq!(iter.next(), None);
    }

    // 15. Extracts a sequence of multiple consecutive valid marker lines with varying marker counts
    #[test]
    fn test_15_multiple_valid_sequential_lines() {
        let src = b"# a\n## b\n### c";
        let mut iter = LineMarkerIter::new(src, b'#', 6, b'\n', b' ', stub_make);

        assert_eq!(iter.next(), Some((1, Span::new(2, 3))));
        assert_eq!(iter.next(), Some((2, Span::new(7, 8))));
        assert_eq!(iter.next(), Some((3, Span::new(13, 14))));
        assert_eq!(iter.next(), None);
    }
}
