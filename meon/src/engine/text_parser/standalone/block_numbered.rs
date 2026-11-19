//! [`BlockNumberedIter`] — numbered block item standalone iterator.

use super::common::*;
/// Iterator over numbered block items in a byte slice.
///
/// Matches lines where, after optional leading whitespace, one to nine bytes
/// satisfying `digit_matches` are followed by a byte satisfying
/// `end_matches` and then `sep` or `tab`. The number and delimiter byte are
/// passed to `make` to construct the metadata value `T`: as in the full
/// parse, each digit byte counts as its value less `b'0'`, which is the
/// number itself for ASCII digits and wraps otherwise. Yields `(meta, span)`
/// where `span` covers the content portion of the line.
///
/// At construction the `end_matches` predicate is probed over all 256 byte
/// values; when it accepts at most three bytes (every grammar in practice) the
/// scan is streaming — one `memchr` pass for the *delimiter* bytes (`.`/`)`),
/// then a backward walk over the digit run and the `sep`/`tab` indentation to
/// confirm the item leads its line. Scanning by the delimiter avoids needing a
/// ten-byte digit search primitive; a stray delimiter in prose is rejected by
/// the digit check in O(1). A predicate accepting more bytes falls back to the
/// line-by-line scan.
///
/// Obtained via the generated `Parser::find_*` methods; rarely constructed
/// directly.
pub struct BlockNumberedIter<'a, T, D, E, F>
where
    D: Fn(u8) -> bool,
    E: Fn(u8) -> bool,
    F: Fn(u32, u8) -> T,
{
    src: &'a [u8],
    breaks: LineBreaks,
    eol: u8,
    sep: u8,
    tab: u8,
    digit_matches: D,
    end_matches: E,
    make: F,
    pos: usize,
    needles: [u8; 3],
    nn: usize,
    streaming: bool,
    /// Some delimiter byte is also a digit byte (streaming scan only).
    overlap: bool,
    _t: std::marker::PhantomData<T>,
}

impl<'a, T, D, E, F> BlockNumberedIter<'a, T, D, E, F>
where
    D: Fn(u8) -> bool,
    E: Fn(u8) -> bool,
    F: Fn(u32, u8) -> T,
{
    /// Create an iterator over numbered block items.
    ///
    /// # Parameters
    ///
    /// - `src` — source byte slice to scan.
    /// - `breaks` — how lines end: an `eol` byte or a [`LineBreaks`].
    /// - `sep` — separator byte; allowed as leading indentation and required
    ///   immediately after the delimiter byte.
    /// - `tab` — tab byte; treated equivalently to `sep`.
    /// - `digit_matches` — predicate that returns `true` for the bytes the
    ///   number consists of (e.g. ASCII digits).
    /// - `end_matches` — predicate that returns `true` for valid delimiter bytes
    ///   following the digit run (e.g. `b'.'` or `b')'`).
    /// - `make` — closure that receives `(number, delimiter_byte)` and constructs
    ///   the metadata value `T`.
    pub fn new(
        src: &'a [u8],
        breaks: impl Into<LineBreaks>,
        sep: u8,
        tab: u8,
        digit_matches: D,
        end_matches: E,
        make: F,
    ) -> Self {
        let breaks: LineBreaks = breaks.into();
        let mut needles = [0u8; 3];
        let (nn, streaming) = match probe_matcher(&end_matches, &mut needles) {
            Some(n) => (n, true),
            None => (0, false),
        };
        let overlap = needles[..nn].iter().any(|&b| digit_matches(b));
        Self {
            src,
            breaks,
            eol: breaks.eol,
            sep,
            tab,
            digit_matches,
            end_matches,
            make,
            pos: 0,
            needles,
            nn,
            streaming,
            overlap,
            _t: std::marker::PhantomData,
        }
    }
}

impl<T, D, E, F> BlockNumberedIter<'_, T, D, E, F>
where
    D: Fn(u8) -> bool,
    E: Fn(u8) -> bool,
    F: Fn(u32, u8) -> T,
{
    /// The number a digit run reads as, as the full parse computes it.
    #[inline]
    fn number(digits: &[u8]) -> u32 {
        digits.iter().fold(0u32, |n, &d| {
            n.wrapping_mul(10).wrapping_add(d.wrapping_sub(b'0') as u32)
        })
    }

    /// Streaming scan: `memchr` for the probed delimiter bytes, a backward
    /// walk over the digit run (at most 9 digits, as in the forward path) and
    /// the `sep`/`tab` indentation, then the same post-delimiter separator
    /// check as the line-by-line path.
    fn next_streaming(&mut self) -> Option<(T, Span)> {
        let src = self.src;
        let len = src.len();
        loop {
            let p = find_any_of(&self.needles, self.nn, &src[self.pos..])? + self.pos;

            // A failed hit is rejected in place (`p + 1`), NOT by skipping to
            // the line end: prose delimiters (`.` after a word) are rare per
            // line, so an O(1) reject per hit beats re-scanning each line's
            // tail for its end. (This is the opposite trade-off from
            // `block_marker`/`line_uniform`, whose needles collide with dense
            // emphasis runs.)
            //
            // No digit right before the delimiter: the common prose case,
            // rejected on one byte.
            if p == 0 || !(self.digit_matches)(src[p - 1]) {
                self.pos = p + 1;
                continue;
            }
            // Walk the rest of the digit run backward.
            let mut k = p - 1;
            let mut dc = 1u8;
            while k > 0 && (self.digit_matches)(src[k - 1]) && dc < 9 {
                k -= 1;
                dc += 1;
            }
            // Only sep/tab indentation may precede the digits on their line,
            // which also rejects a run longer than the 9-digit cap: its tenth
            // digit is no indentation.
            let mut ls = k;
            while ls > 0 && (src[ls - 1] == self.sep || src[ls - 1] == self.tab) {
                ls -= 1;
            }
            if ls > 0 && src[ls - 1] != self.eol {
                self.pos = p + 1;
                continue;
            }
            // A delimiter byte the digit pattern also takes ends the run only
            // at the 9-digit cap: below it the full parse reads on past it.
            if self.overlap && dc < 9 && (self.digit_matches)(src[p]) {
                self.pos = p + 1;
                continue;
            }

            // The delimiter must be followed by a separator, then content.
            let nxt = p + 1;
            if nxt < len && (src[nxt] == self.sep || src[nxt] == self.tab) {
                let num = Self::number(&src[k..p]);
                let cs = nxt + 1;
                let le = find_line_end(src, cs, self.eol);
                let ce = self.breaks.content_end(src, cs, le);
                let meta = (self.make)(num, src[p]);
                self.pos = if le < len { le + 1 } else { len };
                return Some((meta, Span::new(cs as u32, ce as u32)));
            }

            self.pos = p + 1;
        }
    }
}

impl<T, D, E, F> Iterator for BlockNumberedIter<'_, T, D, E, F>
where
    D: Fn(u8) -> bool,
    E: Fn(u8) -> bool,
    F: Fn(u32, u8) -> T,
{
    type Item = (T, Span);

    fn next(&mut self) -> Option<(T, Span)> {
        if self.streaming {
            return self.next_streaming();
        }
        let src = self.src;
        let len = src.len();
        loop {
            if self.pos >= len {
                return None;
            }
            let le = find_line_end(src, self.pos, self.eol);
            let ce = self.breaks.content_end(src, self.pos, le);

            let mut p = self.pos;
            while p < ce && (src[p] == self.sep || src[p] == self.tab) {
                p += 1;
            }

            if p < ce && (self.digit_matches)(src[p]) {
                let mut i = p;
                let mut dc = 0u8;
                while i < ce && (self.digit_matches)(src[i]) && dc < 9 {
                    i += 1;
                    dc += 1;
                }
                let num = Self::number(&src[p..i]);
                if dc > 0 && i < ce && (self.end_matches)(src[i]) {
                    let kind = src[i];
                    i += 1;
                    if i < ce && (src[i] == self.sep || src[i] == self.tab) {
                        let cs = i + 1;
                        let meta = (self.make)(num, kind);
                        let span = Span::new(cs as u32, ce as u32);
                        self.pos = if le < len { le + 1 } else { len };
                        return Some((meta, span));
                    }
                }
            }

            self.pos = if le < len { le + 1 } else { len };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stub_make(num: u32, kind: u8) -> (u32, u8) {
        (num, kind)
    }

    fn ascii_digit(b: u8) -> bool {
        b.is_ascii_digit()
    }

    fn stub_end_matches(b: u8) -> bool {
        b == b'.' || b == b')'
    }

    // 01. Parses a standard ordered list item with a dot delimiter and trailing text
    #[test]
    fn test_01_standard_ordered_item() {
        let src = b"1. item\n";
        let mut iter = BlockNumberedIter::new(
            src,
            b'\n',
            b' ',
            b'\t',
            ascii_digit,
            stub_end_matches,
            stub_make,
        );

        assert_eq!(iter.next(), Some(((1, b'.'), Span::new(3, 7))));
        assert_eq!(iter.next(), None);
    }

    // 02. Successfully skips leading spaces before parsing the item number
    #[test]
    fn test_02_leading_spaces_skipped() {
        let src = b"  2. item";
        let mut iter = BlockNumberedIter::new(
            src,
            b'\n',
            b' ',
            b'\t',
            ascii_digit,
            stub_end_matches,
            stub_make,
        );

        assert_eq!(iter.next(), Some(((2, b'.'), Span::new(5, 9))));
        assert_eq!(iter.next(), None);
    }

    // 03. Validates alternative delimiter suffix matched via the predicate closure
    #[test]
    fn test_03_alternative_delimiter_suffix() {
        let src = b"9) test";
        let mut iter = BlockNumberedIter::new(
            src,
            b'\n',
            b' ',
            b'\t',
            ascii_digit,
            stub_end_matches,
            stub_make,
        );

        assert_eq!(iter.next(), Some(((9, b')'), Span::new(3, 7))));
        assert_eq!(iter.next(), None);
    }

    // 04. Parses a multi-digit item number that stays safely within the 9-digit ceiling
    #[test]
    fn test_04_multi_digit_number() {
        let src = b"12345. text";
        let mut iter = BlockNumberedIter::new(
            src,
            b'\n',
            b' ',
            b'\t',
            ascii_digit,
            stub_end_matches,
            stub_make,
        );

        assert_eq!(iter.next(), Some(((12345, b'.'), Span::new(7, 11))));
        assert_eq!(iter.next(), None);
    }

    // 05. Rejects numbers containing 10 or more digits due to the digit count restriction
    #[test]
    fn test_05_digit_count_overflow_boundary_rejected() {
        let src = b"1234567890. text";
        let mut iter = BlockNumberedIter::new(
            src,
            b'\n',
            b' ',
            b'\t',
            ascii_digit,
            stub_end_matches,
            stub_make,
        );

        assert_eq!(iter.next(), None);
    }

    // 06. Rejects a line if there is no valid whitespace following the delimiter suffix
    #[test]
    fn test_06_missing_whitespace_after_delimiter_rejected() {
        let src = b"1.item";
        let mut iter = BlockNumberedIter::new(
            src,
            b'\n',
            b' ',
            b'\t',
            ascii_digit,
            stub_end_matches,
            stub_make,
        );

        assert_eq!(iter.next(), None);
    }

    // 07. Processes and extracts multiple sequential numbered items across different lines
    #[test]
    fn test_07_multiple_items_across_lines() {
        let src = b"1. a\n2. b";
        let mut iter = BlockNumberedIter::new(
            src,
            b'\n',
            b' ',
            b'\t',
            ascii_digit,
            stub_end_matches,
            stub_make,
        );

        assert_eq!(iter.next(), Some(((1, b'.'), Span::new(3, 4))));
        assert_eq!(iter.next(), Some(((2, b'.'), Span::new(8, 9))));
        assert_eq!(iter.next(), None);
    }

    // 08. Accepts an item with an empty content section containing only the trailing whitespace
    #[test]
    fn test_08_empty_content_after_delimiter() {
        let src = b"1. ";
        let mut iter = BlockNumberedIter::new(
            src,
            b'\n',
            b' ',
            b'\t',
            ascii_digit,
            stub_end_matches,
            stub_make,
        );

        assert_eq!(iter.next(), Some(((1, b'.'), Span::new(3, 3))));
        assert_eq!(iter.next(), None);
    }

    // 09. Skips lines that completely lack an initial digit marker sequence
    #[test]
    fn test_09_skips_non_numbered_lines() {
        let src = b"hello\n1. item";
        let mut iter = BlockNumberedIter::new(
            src,
            b'\n',
            b' ',
            b'\t',
            ascii_digit,
            stub_end_matches,
            stub_make,
        );

        assert_eq!(iter.next(), Some(((1, b'.'), Span::new(9, 13))));
        assert_eq!(iter.next(), None);
    }

    // 10. Rejects lines where digits appear but are preceded by unallowed text characters
    #[test]
    fn test_10_digits_after_text_rejected() {
        let src = b"abc 1. item";
        let mut iter = BlockNumberedIter::new(
            src,
            b'\n',
            b' ',
            b'\t',
            ascii_digit,
            stub_end_matches,
            stub_make,
        );

        assert_eq!(iter.next(), None);
    }

    // 11. Returns None immediately when initialized with a completely empty input slice
    #[test]
    fn test_11_empty_source_slice() {
        let src = b"";
        let mut iter = BlockNumberedIter::new(
            src,
            b'\n',
            b' ',
            b'\t',
            ascii_digit,
            stub_end_matches,
            stub_make,
        );

        assert_eq!(iter.next(), None);
    }

    // 12. Confirms that zero is processed correctly as a valid item index number
    #[test]
    fn test_12_zero_as_valid_number() {
        let src = b"0. item";
        let mut iter = BlockNumberedIter::new(
            src,
            b'\n',
            b' ',
            b'\t',
            ascii_digit,
            stub_end_matches,
            stub_make,
        );

        assert_eq!(iter.next(), Some(((0, b'.'), Span::new(3, 7))));
        assert_eq!(iter.next(), None);
    }

    // 13. Includes extra spaces following the first separator character inside the returned span
    #[test]
    fn test_13_multiple_spaces_after_delimiter() {
        let src = b"1.  item";
        let mut iter = BlockNumberedIter::new(
            src,
            b'\n',
            b' ',
            b'\t',
            ascii_digit,
            stub_end_matches,
            stub_make,
        );

        assert_eq!(iter.next(), Some(((1, b'.'), Span::new(3, 8))));
        assert_eq!(iter.next(), None);
    }

    // 14. Supports tab characters for both initial line indentation and item separation
    #[test]
    fn test_14_tabs_as_whitespace_and_separator() {
        let src = b"\t1.\titem";
        let mut iter = BlockNumberedIter::new(
            src,
            b'\n',
            b' ',
            b'\t',
            ascii_digit,
            stub_end_matches,
            stub_make,
        );

        assert_eq!(iter.next(), Some(((1, b'.'), Span::new(4, 8))));
        assert_eq!(iter.next(), None);
    }

    // 15. Rejects a line when the line end cuts off exactly at the delimiter suffix
    #[test]
    fn test_15_eof_immediately_after_delimiter_rejected() {
        let src = b"1.";
        let mut iter = BlockNumberedIter::new(
            src,
            b'\n',
            b' ',
            b'\t',
            ascii_digit,
            stub_end_matches,
            stub_make,
        );

        assert_eq!(iter.next(), None);
    }
    // ---- Per-line fallback path (matcher accepting more than 3 bytes) --- //

    fn wide_end_matches(b: u8) -> bool {
        matches!(b, b'.' | b')' | b':' | b'/')
    }

    // 16. A delimiter predicate accepting four bytes forces the line-by-line
    //     fallback, which must produce the same result as the streaming path
    #[test]
    fn test_16_fallback_standard_item() {
        let src = b"1. a\n22) b";
        let mut iter = BlockNumberedIter::new(
            src,
            b'\n',
            b' ',
            b'\t',
            ascii_digit,
            wide_end_matches,
            stub_make,
        );
        assert_eq!(iter.next(), Some(((1, b'.'), Span::new(3, 4))));
        assert_eq!(iter.next(), Some(((22, b')'), Span::new(9, 10))));
        assert_eq!(iter.next(), None);
    }

    // 17. Fallback: indentation, missing digits, a digit run without its
    //     delimiter, and a delimiter without its separator all skip the line
    #[test]
    fn test_17_fallback_skips() {
        let src = b"plain.\n12x\n3.nosep\n  4: ok";
        let mut iter = BlockNumberedIter::new(
            src,
            b'\n',
            b' ',
            b'\t',
            ascii_digit,
            wide_end_matches,
            stub_make,
        );
        assert_eq!(iter.next(), Some(((4, b':'), Span::new(24, 26))));
        assert_eq!(iter.next(), None);
    }

    // 18. Fallback: empty input terminates cleanly
    #[test]
    fn test_18_fallback_empty() {
        let mut iter = BlockNumberedIter::new(
            b"",
            b'\n',
            b' ',
            b'\t',
            ascii_digit,
            wide_end_matches,
            stub_make,
        );
        assert_eq!(iter.next(), None);
    }

    // 19. A digit pattern narrower than ASCII digits: `0` is no digit, so
    //     `10.` and `0.` open nothing, in both scans
    #[test]
    fn test_19_digit_pattern_range() {
        let src = b"1. a\n10. b\n9) c\n0. d";
        let nonzero = |b: u8| (b'1'..=b'9').contains(&b);
        let want = vec![((1, b'.'), Span::new(3, 4)), ((9, b')'), Span::new(14, 15))];
        let stream = BlockNumberedIter::new(
            src,
            b'\n',
            b' ',
            b'\t',
            nonzero,
            stub_end_matches,
            stub_make,
        );
        assert_eq!(stream.collect::<Vec<_>>(), want);
        let lines = BlockNumberedIter::new(
            src,
            b'\n',
            b' ',
            b'\t',
            nonzero,
            wide_end_matches,
            stub_make,
        );
        assert_eq!(lines.collect::<Vec<_>>(), want);
    }

    // 20. Non-digit bytes count as their value less `b'0'`, wrapping: nine
    //     `z`s overflow `u32` and yield the wrapped number, no panic
    #[test]
    fn test_20_wrapping_number() {
        let src = b"zzzzzzzzz. x";
        let lower = |b: u8| b.is_ascii_lowercase();
        let want = (0..9).fold(0u32, |n, _| n.wrapping_mul(10).wrapping_add(74));
        let mut iter =
            BlockNumberedIter::new(src, b'\n', b' ', b'\t', lower, stub_end_matches, stub_make);
        assert_eq!(iter.next(), Some(((want, b'.'), Span::new(11, 12))));
        assert_eq!(iter.next(), None);
    }

    // 21. A delimiter the digit pattern also takes ends the run only at the
    //     9-digit cap: the streaming scan agrees with the line scan
    #[test]
    fn test_21_delimiter_inside_digit_pattern() {
        let src = b"abx y\naaaaaaaax y\naaaaaaaaax z";
        let lower = |b: u8| b.is_ascii_lowercase();
        let x = |b: u8| b == b'x';
        let x_wide = |b: u8| b == b'x' || wide_end_matches(b);
        let stream: Vec<_> =
            BlockNumberedIter::new(src, b'\n', b' ', b'\t', lower, x, stub_make).collect();
        let lines: Vec<_> =
            BlockNumberedIter::new(src, b'\n', b' ', b'\t', lower, x_wide, stub_make).collect();
        assert_eq!(
            stream.iter().map(|(_, s)| *s).collect::<Vec<_>>(),
            vec![Span::new(29, 30)]
        );
        assert_eq!(stream, lines);
    }

    #[allow(unused)]
    mod generated {
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub struct Num {
            pub num: u32,
            pub kind: u8,
        }

        // Three end bytes: the finder streams by them.
        mod narrow {
            use super::Num;
            crate::define_parser!(Narrow {
                sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\', max_nest = 4;
                inline { fallback => texts [10]; }
                blocks {
                    block {
                        num(b'1'..=b'9', end = b'.' | b')' | b':') |n, k|:
                            Num { num: n, kind: k } => nums [40];
                    }
                    fallback => paragraphs [80];
                }
            });
        }

        // Eleven end bytes: the finder scans line by line.
        mod wide {
            use super::Num;
            crate::define_parser!(Wide {
                sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\', max_nest = 4;
                inline { fallback => texts [10]; }
                blocks {
                    block {
                        num(b'a'..=b'z', end = b'0'..=b'9' | b':') |n, k|:
                            Num { num: n, kind: k } => nums [40];
                    }
                    fallback => paragraphs [80];
                }
            });
        }

        mod letters {
            use super::Num;
            crate::define_parser!(Letters {
                sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\', max_nest = 4;
                inline { fallback => texts [10]; }
                blocks {
                    block_simple { cont(b'>') => quotes [40]; }
                    block {
                        num(b'A'..=b'F' | b'K' | b'M', end = b':' | b';') |n, k|:
                            Num { num: n, kind: k } => nums [40];
                    }
                    fallback => paragraphs [80];
                }
            });
        }

        // 24. Raw numbered spans use the declared digit pattern, preserve
        //     separators and content, and exclude indentation and containers.
        #[test]
        fn test_24_generated_raw_uses_declared_digits() {
            for (src, raw, clean) in [
                (&b"AB: x"[..], &b"AB: x"[..], &b"x"[..]),
                (b"KM; y", b"KM; y", b"y"),
                (b"  AB;\tx", b"AB;\tx", b"x"),
                (b"> AB: x", b"AB: x", b"x"),
                (b"ABC: ", b"ABC: ", b""),
                (b"ABC:  x", b"ABC:  x", b" x"),
                (b"AB: x\n", b"AB: x", b"x"),
                (b"ABCDEFABC; x", b"ABCDEFABC; x", b"x"),
            ] {
                let c = letters::LettersParser::parse_unchecked(src);
                assert_eq!(c.nums.len(), 1, "{src:?}");
                assert!(c.nums_raw().eq([raw]));
                assert!(c.nums_clean().eq([clean]));
            }
            let c = letters::LettersParser::parse_unchecked(b"AB: x");
            assert_eq!(c.nums[0].0.num, 188);
            assert_eq!(c.nums[0].0.kind, b':');
        }

        // 25. Digit-like terminators and restricted decimal patterns keep
        //     their full raw marker, including the original end byte.
        #[test]
        fn test_25_generated_raw_patterns_and_ends() {
            let c = wide::WideParser::parse_unchecked(b"  ab5 x\nq: y");
            assert!(c.nums_raw().eq([&b"ab5 x"[..], &b"q: y"[..]]));
            assert!(c.nums_clean().eq([&b"x"[..], &b"y"[..]]));
            let c = narrow::NarrowParser::parse_unchecked(b"10. no\n12) yes\n9: ");
            assert!(c.nums_raw().eq([&b"12) yes"[..], &b"9: "[..]]));
        }

        fn items(
            it: impl IntoIterator<Item = (Num, crate::span::Span)>,
        ) -> Vec<(u32, u8, u32, u32)> {
            it.into_iter()
                .map(|(m, s)| (m.num, m.kind, s.start, s.end))
                .collect()
        }

        // 22. Digit and end patterns reach the finders as written, and the
        //     finders yield the full parse's items
        #[test]
        fn test_22_generated_patterns() {
            let src: &[u8] = b"1. a\n10. b\n9: c\n  3) d\n0. e\nab5 f\nzzzzzzzzz: g\nq. h";
            let full = narrow::NarrowParser::parse_unchecked(src);
            let want = vec![(1, b'.', 3, 4), (9, b':', 14, 15), (3, b')', 21, 22)];
            assert_eq!(items(full.nums.iter().copied()), want);
            assert_eq!(items(narrow::NarrowParser::find_nums_unchecked(src)), want);
            let ctx = narrow::NarrowParser::context_unchecked(src);
            assert_eq!(
                items(narrow::NarrowParser::find_context_nums_unchecked(src, &ctx)),
                want
            );

            let full = wide::WideParser::parse_unchecked(src);
            let zs = (0..9).fold(0u32, |n, _| n.wrapping_mul(10).wrapping_add(74));
            let want = vec![(49 * 10 + 50, b'5', 32, 33), (zs, b':', 45, 46)];
            assert_eq!(items(full.nums.iter().copied()), want);
            assert_eq!(items(wide::WideParser::find_nums_unchecked(src)), want);
            let ctx = wide::WideParser::context_unchecked(src);
            assert_eq!(
                items(wide::WideParser::find_context_nums_unchecked(src, &ctx)),
                want
            );
        }
    }

    // 23. Streaming: a run longer than nine digits opens nothing, a
    //     nine-digit run does
    #[test]
    fn test_23_streaming_digit_cap() {
        let src = b"1234567890. a\n123456789. b";
        let iter = BlockNumberedIter::new(
            src,
            b'\n',
            b' ',
            b'\t',
            ascii_digit,
            stub_end_matches,
            stub_make,
        );
        assert_eq!(
            iter.collect::<Vec<_>>(),
            vec![((123456789, b'.'), Span::new(25, 26))]
        );
    }
}
