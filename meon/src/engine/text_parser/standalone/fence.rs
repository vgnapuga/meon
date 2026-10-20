//! [`FenceIter`] — fenced block standalone iterator.

use super::common::*;
/// Iterator over fenced block spans in a byte slice.
///
/// A fence opens when a line starts with at least `min` consecutive
/// occurrences of one byte `fence_matches` accepts, and contains no further
/// occurrence of that byte after the opening run. It closes on the next line
/// that starts with at least as many of that same byte as the opener,
/// followed only by `sep` or `tab`: in `fence(b'`' | b'~', min = 3)` a fence
/// opened by ```` ``` ```` is not closed by `~~~`. If no closing fence is
/// found the span extends to the end of the source.
///
/// The returned [`Span`] covers the entire block from the opening fence byte to
/// the end of the closing fence line (inclusive).
///
/// At construction the `fence_matches` predicate is probed over all 256 byte
/// values; when it accepts at most three bytes the scan is streaming — one
/// `memchr` pass finds candidate fence bytes and an O(1) previous-byte check
/// keeps only the ones that begin a line — and otherwise it reads the first
/// byte of every line. The close search is a `memchr` pass for the opener's
/// byte either way, so content lines inside a block are never walked
/// byte-by-byte.
///
/// Obtained via the generated `Parser::find_*` methods; rarely constructed
/// directly.
pub struct FenceIter<'a, M>
where
    M: Fn(u8) -> bool,
{
    src: &'a [u8],
    fence_matches: M,
    min: u32,
    eol: u8,
    sep: u8,
    tab: u8,
    pos: usize,
    needles: [u8; 3],
    nn: usize,
    streaming: bool,
}

impl<'a, M> FenceIter<'a, M>
where
    M: Fn(u8) -> bool,
{
    /// Create an iterator over fenced block spans.
    ///
    /// # Parameters
    ///
    /// - `src` — source byte slice to scan.
    /// - `fence_matches` — predicate that returns `true` for the bytes a fence
    ///   may consist of (e.g. `` b'`' `` or `b'~'`).
    /// - `min` — minimum number of consecutive fence bytes required to open
    ///   a fence.
    /// - `eol` — line terminator byte.
    /// - `sep` — separator byte allowed (with `tab`) on a closing fence line
    ///   after the fence run.
    /// - `tab` — tab byte allowed on a closing fence line after the fence run.
    pub fn new(src: &'a [u8], fence_matches: M, min: u32, eol: u8, sep: u8, tab: u8) -> Self {
        let mut needles = [0u8; 3];
        let (nn, streaming) = match probe_matcher(&fence_matches, &mut needles) {
            Some(n) => (n, true),
            None => (0, false),
        };
        Self {
            src,
            fence_matches,
            min,
            eol,
            sep,
            tab,
            pos: 0,
            needles,
            nn,
            streaming,
        }
    }

    /// The next line start at or after `self.pos` whose first byte is a
    /// fence byte. The streaming scan may leave `self.pos` mid-line; the
    /// line scan keeps it at line starts.
    fn next_candidate(&mut self) -> Option<usize> {
        let src = self.src;
        let len = src.len();
        loop {
            if self.streaming {
                let p = find_any_of(&self.needles, self.nn, &src[self.pos..])? + self.pos;
                if p > 0 && src[p - 1] != self.eol {
                    self.pos = p + 1;
                    continue;
                }
                return Some(p);
            }
            if self.pos >= len {
                return None;
            }
            if (self.fence_matches)(src[self.pos]) {
                return Some(self.pos);
            }
            let le = find_line_end(src, self.pos, self.eol);
            self.pos = if le < len { le + 1 } else { len };
        }
    }
}

impl<M> Iterator for FenceIter<'_, M>
where
    M: Fn(u8) -> bool,
{
    type Item = Span;

    fn next(&mut self) -> Option<Span> {
        let src = self.src;
        let len = src.len();
        loop {
            let p = self.next_candidate()?;
            let byte = src[p];

            let mut c = 0u32;
            let mut i = p;
            while i < len && src[i] == byte {
                c += 1;
                i += 1;
            }
            let le = find_line_end(src, i, self.eol);

            if c < self.min || src[i..le].contains(&byte) {
                // Not an opener, and nothing later on this line can be one
                // (line start is required): resume on the next line.
                self.pos = if le < len { le + 1 } else { len };
                continue;
            }

            let fence_start = p as u32;
            let fence_count = c;
            let mut search = if le < len { le + 1 } else { len };

            // Close search: only the opener's byte at line starts is a
            // candidate; content lines are skipped by the memchr stream.
            let fence_end = loop {
                let Some(r) = memchr::memchr(byte, &src[search..]) else {
                    break len;
                };
                let q = search + r;
                if q > 0 && src[q - 1] != self.eol {
                    search = q + 1;
                    continue;
                }
                let mut cc = 0u32;
                let mut j = q;
                while j < len && src[j] == byte {
                    cc += 1;
                    j += 1;
                }
                let cle = find_line_end(src, j, self.eol);
                if cc >= fence_count && src[j..cle].iter().all(|&b| b == self.sep || b == self.tab)
                {
                    break if cle < len { cle + 1 } else { len };
                }
                search = if cle < len { cle + 1 } else { len };
            };

            self.pos = fence_end;
            return Some(Span::new(fence_start, fence_end as u32));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 01. Parses a standard block surrounded by symmetric three-byte fence lines
    #[test]
    fn test_01_standard_fence_block() {
        let src = b"```\ncontent\n```";
        let mut iter = FenceIter::new(src, |b| b == b'`', 3, b'\n', b' ', b'\t');

        assert_eq!(iter.next(), Some(Span::new(0, 15)));
        assert_eq!(iter.next(), None);
    }

    // 02. Successfully matches an opening fence line that contains an info string
    #[test]
    fn test_02_opening_fence_with_info_string() {
        let src = b"```rust\nlet x = 1;\n```";
        let mut iter = FenceIter::new(src, |b| b == b'`', 3, b'\n', b' ', b'\t');

        assert_eq!(iter.next(), Some(Span::new(0, 22)));
        assert_eq!(iter.next(), None);
    }

    // 03. Accepts a closing fence line that contains allowed trailing spaces and tabs
    #[test]
    fn test_03_closing_fence_with_allowed_whitespace() {
        let src = b"```\ntext\n``` \t ";
        let mut iter = FenceIter::new(src, |b| b == b'`', 3, b'\n', b' ', b'\t');

        assert_eq!(iter.next(), Some(Span::new(0, 15)));
        assert_eq!(iter.next(), None);
    }

    // 04. Matches a closing fence that has more consecutive bytes than the opening fence
    #[test]
    fn test_04_closing_fence_longer_than_opening() {
        let src = b"```\ncontent\n````";
        let mut iter = FenceIter::new(src, |b| b == b'`', 3, b'\n', b' ', b'\t');

        assert_eq!(iter.next(), Some(Span::new(0, 16)));
        assert_eq!(iter.next(), None);
    }

    // 05. Rejects a closing fence if it has fewer consecutive bytes than the opening fence
    #[test]
    fn test_05_closing_fence_shorter_than_opening() {
        let src = b"````\ncontent\n```";
        let mut iter = FenceIter::new(src, |b| b == b'`', 3, b'\n', b' ', b'\t');

        assert_eq!(iter.next(), Some(Span::new(0, 16)));
        assert_eq!(iter.next(), None);
    }

    // 06. Rejects a closing fence if it contains unallowed trailing text characters
    #[test]
    fn test_06_closing_fence_with_trailing_text() {
        let src = b"```\ncontent\n``` error";
        let mut iter = FenceIter::new(src, |b| b == b'`', 3, b'\n', b' ', b'\t');

        assert_eq!(iter.next(), Some(Span::new(0, 21)));
        assert_eq!(iter.next(), None);
    }

    // 07. Ignores a potential opening fence line if its length is below the min threshold
    #[test]
    fn test_07_opening_fence_below_minimum_length() {
        let src = b"``\ncontent\n``";
        let mut iter = FenceIter::new(src, |b| b == b'`', 3, b'\n', b' ', b'\t');

        assert_eq!(iter.next(), None);
    }

    // 08. Consumes everything until EOF when an opening fence lacks a valid closing fence
    #[test]
    fn test_08_unclosed_fence_consumes_to_eof() {
        let src = b"```\ncontent without close";
        let mut iter = FenceIter::new(src, |b| b == b'`', 3, b'\n', b' ', b'\t');

        assert_eq!(iter.next(), Some(Span::new(0, 25)));
        assert_eq!(iter.next(), None);
    }

    // 09. Sequentially extracts multiple distinct closed fence blocks from the same source
    #[test]
    fn test_09_multiple_independent_fence_blocks() {
        let src = b"```\nfirst\n```\n```\nsecond\n```";
        let mut iter = FenceIter::new(src, |b| b == b'`', 3, b'\n', b' ', b'\t');

        assert_eq!(iter.next(), Some(Span::new(0, 14)));
        assert_eq!(iter.next(), Some(Span::new(14, 28)));
        assert_eq!(iter.next(), None);
    }

    // 10. Immediately returns None when the input byte slice is completely empty
    #[test]
    fn test_10_empty_source_slice() {
        let src = b"";
        let mut iter = FenceIter::new(src, |b| b == b'`', 3, b'\n', b' ', b'\t');

        assert_eq!(iter.next(), None);
    }

    // 11. Skips regular text lines and empty lines before finding a valid opening fence
    #[test]
    fn test_11_skips_leading_non_fence_lines() {
        let src = b"regular text\n\n```\ncontent\n```";
        let mut iter = FenceIter::new(src, |b| b == b'`', 3, b'\n', b' ', b'\t');

        assert_eq!(iter.next(), Some(Span::new(14, 29)));
        assert_eq!(iter.next(), None);
    }

    // 12. Rejects an opening fence line if it is indented or starts with a space character
    #[test]
    fn test_12_indented_opening_fence_rejected() {
        let src = b" ```\ncontent\n```";
        let mut iter = FenceIter::new(src, |b| b == b'`', 3, b'\n', b' ', b'\t');

        assert_eq!(iter.next(), Some(Span::new(13, 16)));
        assert_eq!(iter.next(), None);
    }

    // 13. Rejects an opening fence line if the specified marker byte appears again inside the info string
    #[test]
    fn test_13_opening_fence_with_marker_in_info_string() {
        let src = b"```rust`info\ncontent\n```";
        let mut iter = FenceIter::new(src, |b| b == b'`', 3, b'\n', b' ', b'\t');

        assert_eq!(iter.next(), Some(Span::new(21, 24)));
        assert_eq!(iter.next(), None);
    }

    // 14. Matches a fence block that satisfies the minimum length threshold exactly on the edge
    #[test]
    fn test_14_exact_minimum_length_fence() {
        let src = b"~~~\ncontent\n~~~";
        let mut iter = FenceIter::new(src, |b| b == b'~', 3, b'\n', b' ', b'\t');

        assert_eq!(iter.next(), Some(Span::new(0, 15)));
        assert_eq!(iter.next(), None);
    }

    // 15. Verifies fence extraction behavior when there is no trailing newline character at EOF
    #[test]
    fn test_15_no_trailing_newline_at_eof() {
        let src = b"```\ncontent\n```";
        let mut iter = FenceIter::new(src, |b| b == b'`', 3, b'\n', b' ', b'\t');

        assert_eq!(iter.next(), Some(Span::new(0, 15)));
        assert_eq!(iter.next(), None);
    }

    // 16. A pattern of several bytes: each fence closes only with the byte
    //     it opened with, so a `~~~` line inside a ``` block is content
    #[test]
    fn test_16_pattern_fence_closes_with_its_own_byte() {
        let src = b"```\na\n~~~\n```\n~~~\nb\n```\n~~~";
        let mut iter = FenceIter::new(src, |b| b == b'`' || b == b'~', 3, b'\n', b' ', b'\t');

        assert_eq!(iter.next(), Some(Span::new(0, 14)));
        assert_eq!(iter.next(), Some(Span::new(14, 27)));
        assert_eq!(iter.next(), None);
    }

    // 17. Run lengths past 255 are counted exactly: a 255-byte line does
    //     not close a 300-byte opener, a 300-byte line does
    #[test]
    fn test_17_long_runs_counted_exactly() {
        let mut src = vec![b'`'; 300];
        src.extend_from_slice(b"\nx\n");
        src.extend(std::iter::repeat_n(b'`', 255));
        src.push(b'\n');
        src.extend(std::iter::repeat_n(b'`', 300));
        let mut iter = FenceIter::new(&src, |b| b == b'`', 3, b'\n', b' ', b'\t');

        assert_eq!(iter.next(), Some(Span::new(0, src.len() as u32)));
        assert_eq!(iter.next(), None);
    }

    #[allow(unused)]
    mod generated {
        #[derive(Debug, Clone, Copy)]
        pub struct Item {
            pub kind: u8,
        }

        crate::define_parser!(Fences {
            sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\', max_nest = 4;
            inline { fallback => texts [10]; }
            blocks {
                block_simple { fence(b'`' | b'~', min = 3) => codes [40]; }
                block {
                    (b'-') |b|: Item { kind: b } => items [40];
                }
                fallback => paragraphs [80];
            }
        });

        fn spans<'a>(it: impl IntoIterator<Item = &'a crate::span::Span>) -> Vec<(u32, u32)> {
            it.into_iter().map(|s| (s.start, s.end)).collect()
        }

        // 18. A fence pattern reaches the finder and the context as written:
        //     both bytes open, each closes only its own fence, and the
        //     finders agree with the full parse
        #[test]
        fn test_18_generated_pattern_fence() {
            let src: &[u8] = b"```\n- a\n~~~\n```\n~~~\n- b\n```\n~~~\n- c";
            let full = FencesParser::parse(src);
            assert_eq!(spans(&full.codes), vec![(0, 16), (16, 32)]);
            let found: Vec<_> = FencesParser::find_codes(src).collect();
            assert_eq!(spans(&found), spans(&full.codes));
            let ctx = FencesParser::context(src);
            let items: Vec<_> = FencesParser::find_context_items(src, &ctx)
                .map(|(_, s)| s)
                .collect();
            assert_eq!(spans(&items), spans(full.items.iter().map(|(_, s)| s)));
            assert_eq!(items.len(), 1);
        }

        // 19. The full parse counts runs past 255 exactly too: a 255-byte
        //     line inside a 300-byte fence is content
        #[test]
        fn test_19_generated_long_runs() {
            let mut src = vec![b'~'; 300];
            src.extend_from_slice(b"\n- a\n");
            src.extend(std::iter::repeat_n(b'~', 255));
            src.extend_from_slice(b"\n- b\n");
            src.extend(std::iter::repeat_n(b'~', 300));
            src.extend_from_slice(b"\n- c");
            let full = FencesParser::parse(&src);
            let close = src.len() as u32 - 3;
            assert_eq!(spans(&full.codes), vec![(0, close)]);
            assert_eq!(full.items.len(), 1);
            let found: Vec<_> = FencesParser::find_codes(&src).collect();
            assert_eq!(spans(&found), spans(&full.codes));
        }

        mod shared {
            crate::define_parser!(Shared {
                sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\', max_nest = 2;
                inline { fallback => texts [10]; }
                blocks {
                    block_simple {
                        fence(b'`', min = 3) => long [40];
                        fence(b'`', min = 2) => short [40];
                        cont(b'>') => first [40];
                        cont(b'>') => second [40];
                    }
                    fallback => paragraphs [80];
                }
            });
        }

        // 20. Rules sharing a byte: a closed frame goes to the one rule that
        //     opened it — the first that takes its byte and count — and to
        //     no other
        #[test]
        fn test_20_frame_goes_to_its_own_rule() {
            let src: &[u8] = b"```\nx\n```\n``\ny\n``\n> z";
            let full = shared::SharedParser::parse(src);
            assert_eq!(spans(&full.long), vec![(0, 10)]);
            assert_eq!(spans(&full.short), vec![(10, 18)]);
            assert_eq!(spans(&full.first), vec![(18, 21)]);
            assert!(full.second.is_empty());
        }
    }
}
