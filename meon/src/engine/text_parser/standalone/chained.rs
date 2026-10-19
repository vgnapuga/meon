//! [`ChainedIter`] — two-part chained delimiter standalone iterator.

use super::common::*;
/// Iterator over two-part chained delimiter spans in a byte slice.
///
/// Matches the pattern `[prefix]open1...close1 open2...close2`, where `prefix` is
/// an optional single byte (e.g. `!` for image links). The iterator yields one
/// item per match via the `make` closure, which receives `(is_prefix, span1,
/// span2)` and constructs the output type `T`.
///
/// A single streaming scan finds `open1` candidates; the component close
/// searches are **paragraph-bounded**: a component may span single line
/// breaks, but an empty line — two consecutive `eol` bytes — or the end of
/// input aborts the candidate. The `close1`/`open2` junction must still be
/// byte-adjacent: a newline between the two components never matches.
///
/// Escape sequences are respected on `open1` and on the prefix byte.
///
/// What a failed search learns is kept for the openers after it: a close
/// byte absent up to the end of the paragraph, and a first-component close
/// whose match failed after it. A run of unclosed openers costs one search,
/// not one per opener.
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
    /// Where the paragraph of the last failed `close1` search ends: no
    /// `close1` lies between that search's start and here. Openers, and the
    /// url starts after them, only move forward, so each window below began
    /// before every later opener.
    text_dead: usize,
    /// The first `close1` after the start of the last match that failed
    /// after it: an opener before it finds the same one.
    text_found: usize,
    /// As `text_dead`, for `close2`.
    url_dead: usize,
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
    /// - `eol` — line terminator byte; a component may span single line
    ///   breaks, but never an empty line (two consecutive `eol` bytes).
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
            text_dead: 0,
            text_found: 0,
            url_dead: 0,
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
        let len = src.len();
        loop {
            // Open scan: one streaming pass for the first opening byte.
            let p = memchr::memchr(self.open1, &src[self.pos..])? + self.pos;

            if count_escape(src, p, self.escape) % 2 == 1 {
                self.pos = p + 1;
                continue;
            }

            let is_prefix = p > 0
                && src[p - 1] == self.prefix
                && count_escape(src, p - 1, self.escape) % 2 == 0;

            let text_start = p + 1;
            let c1 = if text_start <= self.text_found {
                self.text_found
            } else if text_start < self.text_dead {
                self.pos = p + 1;
                continue;
            } else {
                match scan_paragraph(src, text_start, self.close1, self.eol, Some) {
                    Ok(c1) => c1,
                    Err(stop) => {
                        self.text_dead = stop;
                        self.pos = p + 1;
                        continue;
                    }
                }
            };

            // The two components must be byte-adjacent — a line break between
            // them never matches.
            let nxt = c1 + 1;
            if nxt >= len || src[nxt] != self.open2 {
                self.text_found = c1;
                self.pos = p + 1;
                continue;
            }

            let url_start = nxt + 1;
            let c2 = if url_start < self.url_dead {
                None
            } else {
                match scan_paragraph(src, url_start, self.close2, self.eol, Some) {
                    Ok(c2) => Some(c2),
                    Err(stop) => {
                        self.url_dead = stop;
                        None
                    }
                }
            };
            let Some(c2) = c2 else {
                self.text_found = c1;
                self.pos = p + 1;
                continue;
            };

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

    // 05. A first component left unclosed on its own line closes past a single
    //     newline — components are paragraph-bounded, not line-bounded.
    #[test]
    fn test_05_close1_across_single_newline() {
        let src = b"[unclosed(url\n[x](y)";
        let mut iter = ChainedIter::new(src, b'[', b']', b'(', b')', b'!', b'\n', b'\\', stub_make);

        // close1 for the opener at 0 is the `]` at 16; `(` follows at 17.
        assert_eq!(
            iter.next(),
            Some((false, Span::new(1, 16), Span::new(18, 19)))
        );
        assert_eq!(iter.next(), None);
    }

    // 06. A second component left unclosed on its own line closes past a
    //     single newline too.
    #[test]
    fn test_06_close2_across_single_newline() {
        let src = b"[text](unclosed\n[x](y)";
        let mut iter = ChainedIter::new(src, b'[', b']', b'(', b')', b'!', b'\n', b'\\', stub_make);

        // The url component opened at 7 closes at the `)` at 21, spanning the
        // embedded newline; the inner `[x](y)` is consumed by it.
        assert_eq!(
            iter.next(),
            Some((false, Span::new(1, 5), Span::new(7, 21)))
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

    // 09. The two components must be byte-adjacent: a newline between close1
    //     and open2 never matches, paragraph bounds notwithstanding
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

    // ---- Paragraph-bounded behaviour ------------------------------------ //

    // 16. An empty line aborts an unclosed first component: components never
    //     pair across paragraphs
    #[test]
    fn test_16_empty_line_aborts_component() {
        let src = b"[open\n\nclosed](url)";
        let mut iter = ChainedIter::new(src, b'[', b']', b'(', b')', b'!', b'\n', b'\\', stub_make);

        assert_eq!(iter.next(), None);
    }

    // 17. A whole pair spanning a single newline inside one paragraph matches
    #[test]
    fn test_17_pair_spans_single_newline() {
        let src = b"[multi\nline](url)";
        let mut iter = ChainedIter::new(src, b'[', b']', b'(', b')', b'!', b'\n', b'\\', stub_make);

        assert_eq!(
            iter.next(),
            Some((false, Span::new(1, 11), Span::new(13, 16)))
        );
        assert_eq!(iter.next(), None);
    }

    fn links(src: &[u8]) -> Vec<(u32, u32, u32, u32)> {
        ChainedIter::new(src, b'[', b']', b'(', b')', b'!', b'\n', b'\\', stub_make)
            .map(|(_, t, u)| (t.start, t.end, u.start, u.end))
            .collect()
    }

    // 18. What a failed search learns holds only where it holds: up to the
    //     end of the paragraph for an absent close byte, up to the close it
    //     found for a match that failed after it
    #[test]
    fn test_18_failed_search_windows() {
        // No first close up to the empty line, then a new paragraph.
        assert_eq!(links(b"[a [b\n\n[c](d)"), vec![(8, 9, 11, 12)]);
        // A match failed after `]`; an opener past that close searches anew.
        assert_eq!(links(b"[a] [b](c)"), vec![(5, 6, 8, 9)]);
        // The first opener's close serves the second one too.
        assert!(links(b"[a [b] c").is_empty());
        // No url close up to the empty line, then a new paragraph.
        assert_eq!(links(b"[a](b\n\n[c](d)"), vec![(8, 9, 11, 12)]);
        assert!(links(b"[a](b [c](d").is_empty());
    }

    // 19. A long run of unclosed openers is read once
    #[test]
    fn test_19_unclosed_openers() {
        let mut src = b"[a ".repeat(20_000);
        src.extend_from_slice(b"\n\n[b](c)");
        assert_eq!(links(&src), vec![(60_003, 60_004, 60_006, 60_007)]);
        let mut src = b"[a](".repeat(20_000);
        src.extend_from_slice(b"\n\n[b](c)");
        assert_eq!(links(&src), vec![(80_003, 80_004, 80_006, 80_007)]);
    }
}
