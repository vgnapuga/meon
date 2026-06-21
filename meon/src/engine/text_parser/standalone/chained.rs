//! [`ChainedIter`] — two-part chained delimiter standalone iterator.

use super::common::*;
/// Iterator over two-part chained delimiter spans in a byte slice.
///
/// Matches the pattern `[prefix]open1...close1 open2...close2`, where `prefix` is
/// an optional single byte (e.g. `!` for image links). The iterator yields one
/// item per match via the `make` closure, which receives `(is_prefix, span1,
/// span2)` and constructs the output type `T`.
///
/// Matching is line-bounded. Escape sequences are respected on `open1` and on
/// the prefix byte.
///
/// Obtained via the generated `Parser::find_*` methods; rarely constructed
/// directly.
pub struct ChainedIter<'a, T, F>
where
    F: Fn(bool, Span, Span) -> T,
{
    src: &'a [u8],
    open1: u8,
    close1: u8,
    open2: u8,
    close2: u8,
    prefix: u8,
    eol: u8,
    escape: u8,
    make: F,
    pos: usize,
    line_end: usize,
    _t: std::marker::PhantomData<T>,
}

impl<'a, T, F> ChainedIter<'a, T, F>
where
    F: Fn(bool, Span, Span) -> T,
{
    /// Create an iterator over two-part chained delimiter spans.
    ///
    /// # Parameters
    ///
    /// - `src` — source byte slice to scan.
    /// - `open1` / `close1` — opening and closing bytes of the first component
    ///   (e.g. `b'['` / `b']'` for the text part of a link).
    /// - `open2` / `close2` — opening and closing bytes of the second component
    ///   (e.g. `b'('` / `b')'` for the url part).
    /// - `prefix` — optional single byte immediately before `open1` that sets
    ///   the `is_prefix` flag passed to `make` (e.g. `b'!'` for images).
    /// - `eol` — line terminator byte; matching never crosses a line boundary.
    /// - `escape` — escape prefix byte; suppresses `open1` and `prefix` when
    ///   preceded by an odd number of escape bytes.
    /// - `make` — closure that receives `(is_prefix, span1, span2)` and
    ///   constructs the output value `T`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        src: &'a [u8],
        open1: u8,
        close1: u8,
        open2: u8,
        close2: u8,
        prefix: u8,
        eol: u8,
        escape: u8,
        make: F,
    ) -> Self {
        let line_end = find_line_end(src, 0, eol);
        Self {
            src,
            open1,
            close1,
            open2,
            close2,
            prefix,
            eol,
            escape,
            make,
            pos: 0,
            line_end,
            _t: std::marker::PhantomData,
        }
    }
}

impl<T, F> Iterator for ChainedIter<'_, T, F>
where
    F: Fn(bool, Span, Span) -> T,
{
    type Item = T;

    fn next(&mut self) -> Option<T> {
        let src = self.src;
        loop {
            while self.pos >= self.line_end {
                let (next, end) = advance_line(src, self.line_end, self.eol)?;
                self.pos = next;
                self.line_end = end;
            }

            let Some(r) = memchr::memchr(self.open1, &src[self.pos..self.line_end]) else {
                self.pos = self.line_end;
                continue;
            };
            let p = self.pos + r;

            if count_escape(src, p, self.escape) % 2 == 1 {
                self.pos = p + 1;
                continue;
            }

            let is_prefix = p > 0
                && src[p - 1] == self.prefix
                && count_escape(src, p - 1, self.escape) % 2 == 0;

            let text_start = p + 1;
            let Some(r1) = memchr::memchr(self.close1, &src[text_start..self.line_end]) else {
                self.pos = p + 1;
                continue;
            };
            let c1 = text_start + r1;
            let nxt = c1 + 1;
            if nxt >= self.line_end || src[nxt] != self.open2 {
                self.pos = p + 1;
                continue;
            }

            let url_start = nxt + 1;
            let Some(r2) = memchr::memchr(self.close2, &src[url_start..self.line_end]) else {
                self.pos = p + 1;
                continue;
            };
            let c2 = url_start + r2;

            self.pos = c2 + 1;
            return Some((self.make)(
                is_prefix,
                Span::new(text_start as u32, c1 as u32),
                Span::new(url_start as u32, c2 as u32),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stub_make(is_prefix: bool, text: Span, url: Span) -> (bool, Span, Span) {
        (is_prefix, text, url)
    }

    // 01. Parses a clean, standard link pattern without any prefix
    #[test]
    fn test_01_standard_link() {
        let src = b"[text](url)";
        let mut iter = ChainedIter::new(src, b'[', b']', b'(', b')', b'!', b'\n', b'\\', stub_make);

        assert_eq!(
            iter.next(),
            Some((false, Span::new(1, 5), Span::new(7, 10)))
        );
        assert_eq!(iter.next(), None);
    }

    // 02. Detects the unescaped prefix byte matching an image pattern
    #[test]
    fn test_02_image_with_prefix() {
        let src = b"![img](lnk)";
        let mut iter = ChainedIter::new(src, b'[', b']', b'(', b')', b'!', b'\n', b'\\', stub_make);

        assert_eq!(iter.next(), Some((true, Span::new(2, 5), Span::new(7, 10))));
        assert_eq!(iter.next(), None);
    }

    // 03. Skips an open marker if it is preceded by an odd number of escape characters
    #[test]
    fn test_03_escaped_open_marker_skipped() {
        let src = b"\\[skip](no)[x](y)";
        let mut iter = ChainedIter::new(src, b'[', b']', b'(', b')', b'!', b'\n', b'\\', stub_make);

        assert_eq!(
            iter.next(),
            Some((false, Span::new(12, 13), Span::new(15, 16)))
        );
        assert_eq!(iter.next(), None);
    }

    // 04. Rejects patterns where the second open marker does not immediately follow the first close marker
    #[test]
    fn test_04_broken_chain_with_spaces_skipped() {
        let src = b"[a] (b)[x](y)";
        let mut iter = ChainedIter::new(src, b'[', b']', b'(', b')', b'!', b'\n', b'\\', stub_make);

        assert_eq!(
            iter.next(),
            Some((false, Span::new(8, 9), Span::new(11, 12)))
        );
        assert_eq!(iter.next(), None);
    }

    // 05. Recovers on a subsequent line if the first line contains an unclosed first component
    #[test]
    fn test_05_missing_close1_on_line() {
        let src = b"[unclosed(url\n[x](y)";
        let mut iter = ChainedIter::new(src, b'[', b']', b'(', b')', b'!', b'\n', b'\\', stub_make);

        assert_eq!(
            iter.next(),
            Some((false, Span::new(15, 16), Span::new(18, 19)))
        );
        assert_eq!(iter.next(), None);
    }

    // 06. Recovers on a subsequent line if the first line contains an unclosed second component
    #[test]
    fn test_06_missing_close2_on_line() {
        let src = b"[text](unclosed\n[x](y)";
        let mut iter = ChainedIter::new(src, b'[', b']', b'(', b')', b'!', b'\n', b'\\', stub_make);

        assert_eq!(
            iter.next(),
            Some((false, Span::new(17, 18), Span::new(20, 21)))
        );
        assert_eq!(iter.next(), None);
    }

    // 07. Extracts multiple independent valid chained pairs from a single line sequential stream
    #[test]
    fn test_07_multiple_pairs_on_single_line() {
        let src = b"[a](b)[c](d)";
        let mut iter = ChainedIter::new(src, b'[', b']', b'(', b')', b'!', b'\n', b'\\', stub_make);

        assert_eq!(iter.next(), Some((false, Span::new(1, 2), Span::new(4, 5))));
        assert_eq!(
            iter.next(),
            Some((false, Span::new(7, 8), Span::new(10, 11)))
        );
        assert_eq!(iter.next(), None);
    }

    // 08. Demonstrates flat parsing logic where an inner open bracket becomes part of the text span
    #[test]
    fn test_08_flat_inner_open_marker() {
        let src = b"[skip [a](b)";
        let mut iter = ChainedIter::new(src, b'[', b']', b'(', b')', b'!', b'\n', b'\\', stub_make);

        assert_eq!(
            iter.next(),
            Some((false, Span::new(1, 8), Span::new(10, 11)))
        );
        assert_eq!(iter.next(), None);
    }

    // 09. Rejects patterns split across a newline boundary as parsing is strictly line-bound
    #[test]
    fn test_09_newline_separating_components_fails() {
        let src = b"[a]\n(b)";
        let mut iter = ChainedIter::new(src, b'[', b']', b'(', b')', b'!', b'\n', b'\\', stub_make);

        assert_eq!(iter.next(), None);
    }

    // 10. Handles empty inner text and empty inner url segments correctly
    #[test]
    fn test_10_empty_contents() {
        let src = b"[](b)";
        let mut iter = ChainedIter::new(src, b'[', b']', b'(', b')', b'!', b'\n', b'\\', stub_make);

        assert_eq!(iter.next(), Some((false, Span::new(1, 1), Span::new(3, 4))));
        assert_eq!(iter.next(), None);
    }

    // 11. Treats prefix as false if the prefix character itself is escaped
    #[test]
    fn test_11_escaped_prefix_evaluated_as_false() {
        let src = b"\\![a](b)";
        let mut iter = ChainedIter::new(src, b'[', b']', b'(', b')', b'!', b'\n', b'\\', stub_make);

        assert_eq!(iter.next(), Some((false, Span::new(3, 4), Span::new(6, 7))));
        assert_eq!(iter.next(), None);
    }

    // 12. Correctly processes independent matching chains across multiple sequential lines
    #[test]
    fn test_12_multiline_processing() {
        let src = b"[a](b)\n![c](d)";
        let mut iter = ChainedIter::new(src, b'[', b']', b'(', b')', b'!', b'\n', b'\\', stub_make);

        assert_eq!(iter.next(), Some((false, Span::new(1, 2), Span::new(4, 5))));
        assert_eq!(
            iter.next(),
            Some((true, Span::new(9, 10), Span::new(12, 13)))
        );
        assert_eq!(iter.next(), None);
    }

    // 13. Returns None immediately when supplied with a completely empty input slice
    #[test]
    fn test_13_empty_source_slice() {
        let src = b"";
        let mut iter = ChainedIter::new(src, b'[', b']', b'(', b')', b'!', b'\n', b'\\', stub_make);

        assert_eq!(iter.next(), None);
    }

    // 14. Prevents underflow boundaries and safely sets prefix to false when pattern starts at index 0
    #[test]
    fn test_14_pattern_at_start_index_zero() {
        let src = b"[a](b)";
        let mut iter = ChainedIter::new(src, b'[', b']', b'(', b')', b'!', b'\n', b'\\', stub_make);

        assert_eq!(iter.next(), Some((false, Span::new(1, 2), Span::new(4, 5))));
        assert_eq!(iter.next(), None);
    }

    // 15. Ignores trailing garbage text on the line after successfully extracting a valid pattern match
    #[test]
    fn test_15_trailing_garbage_ignored() {
        let src = b"[a](b)random_garbage_text";
        let mut iter = ChainedIter::new(src, b'[', b']', b'(', b')', b'!', b'\n', b'\\', stub_make);

        assert_eq!(iter.next(), Some((false, Span::new(1, 2), Span::new(4, 5))));
        assert_eq!(iter.next(), None);
    }
}
