//! [`BlockMarkerIter`] — marker-prefixed block item standalone iterator.

use super::common::*;
/// Iterator over marker-prefixed block items in a byte slice.
///
/// Matches lines where, after optional leading `sep`/`tab` whitespace, a single
/// byte satisfying `matches` is followed by `sep` or `tab`. The item content
/// starts after that separator. Yields `(meta, span)` where `meta` is produced
/// by the `make` closure from the marker byte, and `span` covers the content
/// portion of the line.
///
/// Obtained via the generated `Parser::find_*` methods; rarely constructed
/// directly.
pub struct BlockMarkerIter<'a, T, M, F>
where
    M: Fn(u8) -> bool,
    F: Fn(u8) -> T,
{
    src: &'a [u8],
    eol: u8,
    sep: u8,
    tab: u8,
    matches: M,
    make: F,
    pos: usize,
    _t: std::marker::PhantomData<T>,
}

impl<'a, T, M, F> BlockMarkerIter<'a, T, M, F>
where
    M: Fn(u8) -> bool,
    F: Fn(u8) -> T,
{
    /// Create an iterator over marker-prefixed block items.
    ///
    /// # Parameters
    ///
    /// - `src` — source byte slice to scan.
    /// - `eol` — line terminator byte.
    /// - `sep` — separator byte; allowed as leading indentation and required
    ///   immediately after the marker byte.
    /// - `tab` — tab byte; treated equivalently to `sep` for indentation and
    ///   post-marker separation.
    /// - `matches` — predicate that returns `true` for valid marker bytes.
    /// - `make` — closure that receives the marker byte and constructs the
    ///   metadata value `T`.
    pub fn new(src: &'a [u8], eol: u8, sep: u8, tab: u8, matches: M, make: F) -> Self {
        Self {
            src,
            eol,
            sep,
            tab,
            matches,
            make,
            pos: 0,
            _t: std::marker::PhantomData,
        }
    }
}

impl<T, M, F> Iterator for BlockMarkerIter<'_, T, M, F>
where
    M: Fn(u8) -> bool,
    F: Fn(u8) -> T,
{
    type Item = (T, Span);

    fn next(&mut self) -> Option<(T, Span)> {
        let src = self.src;
        let len = src.len();
        loop {
            if self.pos >= len {
                return None;
            }
            let le = find_line_end(src, self.pos, self.eol);

            let mut p = self.pos;
            while p < le && (src[p] == self.sep || src[p] == self.tab) {
                p += 1;
            }

            if p < le && (self.matches)(src[p]) {
                let nxt = p + 1;
                if nxt < le && (src[nxt] == self.sep || src[nxt] == self.tab) {
                    let cs = nxt + 1;
                    let meta = (self.make)(src[p]);
                    let span = Span::new(cs as u32, le as u32);
                    self.pos = if le < len { le + 1 } else { len };
                    return Some((meta, span));
                }
            }

            self.pos = if le < len { le + 1 } else { len };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stub_make(kind: u8) -> u8 {
        kind
    }

    fn stub_matches(b: u8) -> bool {
        b == b'*' || b == b'-' || b == b'+'
    }

    // 01. Parses a standard markdown list item with a trailing newline
    #[test]
    fn test_01_standard_list_item() {
        let src = b"* item\n";
        let mut iter = BlockMarkerIter::new(src, b'\n', b' ', b'\t', stub_matches, stub_make);

        assert_eq!(iter.next(), Some((b'*', Span::new(2, 6))));
        assert_eq!(iter.next(), None);
    }

    // 02. Skips leading space indentation before identifying the block marker
    #[test]
    fn test_02_leading_spaces_skipped() {
        let src = b"  * item";
        let mut iter = BlockMarkerIter::new(src, b'\n', b' ', b'\t', stub_matches, stub_make);

        assert_eq!(iter.next(), Some((b'*', Span::new(4, 8))));
        assert_eq!(iter.next(), None);
    }

    // 03. Correctly matches an alternative valid marker character from the closure
    #[test]
    fn test_03_alternative_marker_matched() {
        let src = b"- item";
        let mut iter = BlockMarkerIter::new(src, b'\n', b' ', b'\t', stub_matches, stub_make);

        assert_eq!(iter.next(), Some((b'-', Span::new(2, 6))));
        assert_eq!(iter.next(), None);
    }

    // 04. Uses a tab character as a valid separator after the block marker
    #[test]
    fn test_04_tab_as_separator() {
        let src = b"*\titem";
        let mut iter = BlockMarkerIter::new(src, b'\n', b' ', b'\t', stub_matches, stub_make);

        assert_eq!(iter.next(), Some((b'*', Span::new(2, 6))));
        assert_eq!(iter.next(), None);
    }

    // 05. Skips leading tab characters used for line indentation
    #[test]
    fn test_05_tabs_as_indentation_skipped() {
        let src = b"\t\t* item";
        let mut iter = BlockMarkerIter::new(src, b'\n', b' ', b'\t', stub_matches, stub_make);

        assert_eq!(iter.next(), Some((b'*', Span::new(4, 8))));
        assert_eq!(iter.next(), None);
    }

    // 06. Rejects a marker sequence that completely lacks a trailing whitespace separator
    #[test]
    fn test_06_missing_separator_after_marker_rejected() {
        let src = b"*item";
        let mut iter = BlockMarkerIter::new(src, b'\n', b' ', b'\t', stub_matches, stub_make);

        assert_eq!(iter.next(), None);
    }

    // 07. Extracts multiple distinct marker items sequentially across line boundaries
    #[test]
    fn test_07_multiple_items_across_lines() {
        let src = b"* a\n- b";
        let mut iter = BlockMarkerIter::new(src, b'\n', b' ', b'\t', stub_matches, stub_make);

        assert_eq!(iter.next(), Some((b'*', Span::new(2, 3))));
        assert_eq!(iter.next(), Some((b'-', Span::new(6, 7))));
        assert_eq!(iter.next(), None);
    }

    // 08. Captures a valid block item that contains an empty content payload segment
    #[test]
    fn test_08_empty_content_after_separator() {
        let src = b"* ";
        let mut iter = BlockMarkerIter::new(src, b'\n', b' ', b'\t', stub_matches, stub_make);

        assert_eq!(iter.next(), Some((b'*', Span::new(2, 2))));
        assert_eq!(iter.next(), None);
    }

    // 09. Skips lines containing raw text that do not begin with a valid block marker
    #[test]
    fn test_09_skips_non_marker_lines() {
        let src = b"plain text\n* item";
        let mut iter = BlockMarkerIter::new(src, b'\n', b' ', b'\t', stub_matches, stub_make);

        assert_eq!(iter.next(), Some((b'*', Span::new(13, 17))));
        assert_eq!(iter.next(), None);
    }

    // 10. Rejects an inline marker character preceded by unallowed alphanumeric text
    #[test]
    fn test_10_marker_preceded_by_text_rejected() {
        let src = b"abc * item";
        let mut iter = BlockMarkerIter::new(src, b'\n', b' ', b'\t', stub_matches, stub_make);

        assert_eq!(iter.next(), None);
    }

    // 11. Returns None immediately when supplied with a completely empty source slice
    #[test]
    fn test_11_empty_source_slice() {
        let src = b"";
        let mut iter = BlockMarkerIter::new(src, b'\n', b' ', b'\t', stub_matches, stub_make);

        assert_eq!(iter.next(), None);
    }

    // 12. Includes any secondary whitespace bytes directly inside the generated span offset
    #[test]
    fn test_12_extra_spaces_included_in_span() {
        let src = b"*   item";
        let mut iter = BlockMarkerIter::new(src, b'\n', b' ', b'\t', stub_matches, stub_make);

        assert_eq!(iter.next(), Some((b'*', Span::new(2, 8))));
        assert_eq!(iter.next(), None);
    }

    // 13. Rejects a line when the source stream terminates precisely at the marker byte
    #[test]
    fn test_13_eof_immediately_at_marker_rejected() {
        let src = b"*";
        let mut iter = BlockMarkerIter::new(src, b'\n', b' ', b'\t', stub_matches, stub_make);

        assert_eq!(iter.next(), None);
    }

    // 14. Rejects consecutive consecutive marker characters without an intermediate space
    #[test]
    fn test_14_consecutive_markers_without_space_rejected() {
        let src = b"** item";
        let mut iter = BlockMarkerIter::new(src, b'\n', b' ', b'\t', stub_matches, stub_make);

        assert_eq!(iter.next(), None);
    }

    // 15. Skips a blank line composed entirely of whitespace characters safely
    #[test]
    fn test_15_whitespace_only_line_skipped() {
        let src = b"   \n+ item";
        let mut iter = BlockMarkerIter::new(src, b'\n', b' ', b'\t', stub_matches, stub_make);

        assert_eq!(iter.next(), Some((b'+', Span::new(6, 10))));
        assert_eq!(iter.next(), None);
    }
}
