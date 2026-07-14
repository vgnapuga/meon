//! [`BlockNumberedIter`] — numbered block item standalone iterator.

use super::common::*;
/// Iterator over numbered block items in a byte slice.
///
/// Matches lines where, after optional leading whitespace, one or more ASCII
/// digits (up to 9) are followed by a byte satisfying `end_matches` and then
/// `sep` or `tab`. The parsed number and delimiter byte are passed to `make`
/// to construct the metadata value `T`. Yields `(meta, span)` where `span`
/// covers the content portion of the line.
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
pub struct BlockNumberedIter<'a, T, E, F>
where
    E: Fn(u8) -> bool,
    F: Fn(u32, u8) -> T,
{
    src: &'a [u8],
    eol: u8,
    sep: u8,
    tab: u8,
    end_matches: E,
    make: F,
    pos: usize,
    needles: [u8; 3],
    nn: usize,
    streaming: bool,
    _t: std::marker::PhantomData<T>,
}

impl<'a, T, E, F> BlockNumberedIter<'a, T, E, F>
where
    E: Fn(u8) -> bool,
    F: Fn(u32, u8) -> T,
{
    /// Create an iterator over numbered block items.
    ///
    /// # Parameters
    ///
    /// - `src` — source byte slice to scan.
    /// - `eol` — line terminator byte.
    /// - `sep` — separator byte; allowed as leading indentation and required
    ///   immediately after the delimiter byte.
    /// - `tab` — tab byte; treated equivalently to `sep`.
    /// - `end_matches` — predicate that returns `true` for valid delimiter bytes
    ///   following the digit run (e.g. `b'.'` or `b')'`).
    /// - `make` — closure that receives `(number, delimiter_byte)` and constructs
    ///   the metadata value `T`.
    pub fn new(src: &'a [u8], eol: u8, sep: u8, tab: u8, end_matches: E, make: F) -> Self {
        let mut needles = [0u8; 3];
        let (nn, streaming) = match probe_matcher(&end_matches, &mut needles) {
            Some(n) => (n, true),
            None => (0, false),
        };
        Self {
            src,
            eol,
            sep,
            tab,
            end_matches,
            make,
            pos: 0,
            needles,
            nn,
            streaming,
            _t: std::marker::PhantomData,
        }
    }
}

impl<T, E, F> BlockNumberedIter<'_, T, E, F>
where
    E: Fn(u8) -> bool,
    F: Fn(u32, u8) -> T,
{
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
            // Walk the digit run backward from the delimiter.
            let mut k = p;
            let mut dc = 0u8;
            while k > 0 && src[k - 1].is_ascii_digit() && dc < 9 {
                k -= 1;
                dc += 1;
            }
            // No digits at all, or a run longer than the 9-digit cap.
            if dc == 0 || (k > 0 && src[k - 1].is_ascii_digit()) {
                self.pos = p + 1;
                continue;
            }

            // Only sep/tab indentation may precede the digits on their line.
            let mut ls = k;
            while ls > 0 && (src[ls - 1] == self.sep || src[ls - 1] == self.tab) {
                ls -= 1;
            }
            if ls > 0 && src[ls - 1] != self.eol {
                self.pos = p + 1;
                continue;
            }

            // The delimiter must be followed by a separator, then content.
            let nxt = p + 1;
            if nxt < len && (src[nxt] == self.sep || src[nxt] == self.tab) {
                let mut num = 0u32;
                for &d in &src[k..p] {
                    num = num * 10 + (d - b'0') as u32;
                }
                let cs = nxt + 1;
                let le = find_line_end(src, cs, self.eol);
                let meta = (self.make)(num, src[p]);
                self.pos = if le < len { le + 1 } else { len };
                return Some((meta, Span::new(cs as u32, le as u32)));
            }

            self.pos = p + 1;
        }
    }
}

impl<T, E, F> Iterator for BlockNumberedIter<'_, T, E, F>
where
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

            let mut p = self.pos;
            while p < le && (src[p] == self.sep || src[p] == self.tab) {
                p += 1;
            }

            if p < le && src[p].is_ascii_digit() {
                let mut i = p;
                let mut num = 0u32;
                let mut dc = 0u8;
                while i < le && src[i].is_ascii_digit() && dc < 9 {
                    num = num * 10 + (src[i] - b'0') as u32;
                    i += 1;
                    dc += 1;
                }
                if dc > 0 && i < le && (self.end_matches)(src[i]) {
                    let kind = src[i];
                    i += 1;
                    if i < le && (src[i] == self.sep || src[i] == self.tab) {
                        let cs = i + 1;
                        let meta = (self.make)(num, kind);
                        let span = Span::new(cs as u32, le as u32);
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

    fn stub_end_matches(b: u8) -> bool {
        b == b'.' || b == b')'
    }

    // 01. Parses a standard ordered list item with a dot delimiter and trailing text
    #[test]
    fn test_01_standard_ordered_item() {
        let src = b"1. item\n";
        let mut iter = BlockNumberedIter::new(src, b'\n', b' ', b'\t', stub_end_matches, stub_make);

        assert_eq!(iter.next(), Some(((1, b'.'), Span::new(3, 7))));
        assert_eq!(iter.next(), None);
    }

    // 02. Successfully skips leading spaces before parsing the item number
    #[test]
    fn test_02_leading_spaces_skipped() {
        let src = b"  2. item";
        let mut iter = BlockNumberedIter::new(src, b'\n', b' ', b'\t', stub_end_matches, stub_make);

        assert_eq!(iter.next(), Some(((2, b'.'), Span::new(5, 9))));
        assert_eq!(iter.next(), None);
    }

    // 03. Validates alternative delimiter suffix matched via the predicate closure
    #[test]
    fn test_03_alternative_delimiter_suffix() {
        let src = b"9) test";
        let mut iter = BlockNumberedIter::new(src, b'\n', b' ', b'\t', stub_end_matches, stub_make);

        assert_eq!(iter.next(), Some(((9, b')'), Span::new(3, 7))));
        assert_eq!(iter.next(), None);
    }

    // 04. Parses a multi-digit item number that stays safely within the 9-digit ceiling
    #[test]
    fn test_04_multi_digit_number() {
        let src = b"12345. text";
        let mut iter = BlockNumberedIter::new(src, b'\n', b' ', b'\t', stub_end_matches, stub_make);

        assert_eq!(iter.next(), Some(((12345, b'.'), Span::new(7, 11))));
        assert_eq!(iter.next(), None);
    }

    // 05. Rejects numbers containing 10 or more digits due to the digit count restriction
    #[test]
    fn test_05_digit_count_overflow_boundary_rejected() {
        let src = b"1234567890. text";
        let mut iter = BlockNumberedIter::new(src, b'\n', b' ', b'\t', stub_end_matches, stub_make);

        assert_eq!(iter.next(), None);
    }

    // 06. Rejects a line if there is no valid whitespace following the delimiter suffix
    #[test]
    fn test_06_missing_whitespace_after_delimiter_rejected() {
        let src = b"1.item";
        let mut iter = BlockNumberedIter::new(src, b'\n', b' ', b'\t', stub_end_matches, stub_make);

        assert_eq!(iter.next(), None);
    }

    // 07. Processes and extracts multiple sequential numbered items across different lines
    #[test]
    fn test_07_multiple_items_across_lines() {
        let src = b"1. a\n2. b";
        let mut iter = BlockNumberedIter::new(src, b'\n', b' ', b'\t', stub_end_matches, stub_make);

        assert_eq!(iter.next(), Some(((1, b'.'), Span::new(3, 4))));
        assert_eq!(iter.next(), Some(((2, b'.'), Span::new(8, 9))));
        assert_eq!(iter.next(), None);
    }

    // 08. Accepts an item with an empty content section containing only the trailing whitespace
    #[test]
    fn test_08_empty_content_after_delimiter() {
        let src = b"1. ";
        let mut iter = BlockNumberedIter::new(src, b'\n', b' ', b'\t', stub_end_matches, stub_make);

        assert_eq!(iter.next(), Some(((1, b'.'), Span::new(3, 3))));
        assert_eq!(iter.next(), None);
    }

    // 09. Skips lines that completely lack an initial digit marker sequence
    #[test]
    fn test_09_skips_non_numbered_lines() {
        let src = b"hello\n1. item";
        let mut iter = BlockNumberedIter::new(src, b'\n', b' ', b'\t', stub_end_matches, stub_make);

        assert_eq!(iter.next(), Some(((1, b'.'), Span::new(9, 13))));
        assert_eq!(iter.next(), None);
    }

    // 10. Rejects lines where digits appear but are preceded by unallowed text characters
    #[test]
    fn test_10_digits_after_text_rejected() {
        let src = b"abc 1. item";
        let mut iter = BlockNumberedIter::new(src, b'\n', b' ', b'\t', stub_end_matches, stub_make);

        assert_eq!(iter.next(), None);
    }

    // 11. Returns None immediately when initialized with a completely empty input slice
    #[test]
    fn test_11_empty_source_slice() {
        let src = b"";
        let mut iter = BlockNumberedIter::new(src, b'\n', b' ', b'\t', stub_end_matches, stub_make);

        assert_eq!(iter.next(), None);
    }

    // 12. Confirms that zero is processed correctly as a valid item index number
    #[test]
    fn test_12_zero_as_valid_number() {
        let src = b"0. item";
        let mut iter = BlockNumberedIter::new(src, b'\n', b' ', b'\t', stub_end_matches, stub_make);

        assert_eq!(iter.next(), Some(((0, b'.'), Span::new(3, 7))));
        assert_eq!(iter.next(), None);
    }

    // 13. Includes extra spaces following the first separator character inside the returned span
    #[test]
    fn test_13_multiple_spaces_after_delimiter() {
        let src = b"1.  item";
        let mut iter = BlockNumberedIter::new(src, b'\n', b' ', b'\t', stub_end_matches, stub_make);

        assert_eq!(iter.next(), Some(((1, b'.'), Span::new(3, 8))));
        assert_eq!(iter.next(), None);
    }

    // 14. Supports tab characters for both initial line indentation and item separation
    #[test]
    fn test_14_tabs_as_whitespace_and_separator() {
        let src = b"\t1.\titem";
        let mut iter = BlockNumberedIter::new(src, b'\n', b' ', b'\t', stub_end_matches, stub_make);

        assert_eq!(iter.next(), Some(((1, b'.'), Span::new(4, 8))));
        assert_eq!(iter.next(), None);
    }

    // 15. Rejects a line when the line end cuts off exactly at the delimiter suffix
    #[test]
    fn test_15_eof_immediately_after_delimiter_rejected() {
        let src = b"1.";
        let mut iter = BlockNumberedIter::new(src, b'\n', b' ', b'\t', stub_end_matches, stub_make);

        assert_eq!(iter.next(), None);
    }
}
