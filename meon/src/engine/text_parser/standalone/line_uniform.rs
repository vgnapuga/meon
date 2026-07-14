//! [`LineUniformIter`] — uniform-line element standalone iterator.

use super::common::*;
/// Iterator over uniform-line elements in a byte slice.
///
/// Matches lines composed entirely of one delimiter byte (interleaved with `sep`),
/// where the delimiter satisfies `matches` and appears at least `min` times.
/// The delimiter byte is passed to `make` to produce the metadata value `T`.
/// Yields `(meta, span)` where `span` covers the entire matched line.
///
/// At construction the `matches` predicate is probed over all 256 byte values;
/// when it accepts at most three bytes (every grammar in practice) the scan is
/// streaming — one `memchr` pass for the delimiter bytes plus an O(1)
/// line-start check — so lines without a delimiter are never visited. A
/// predicate accepting more bytes falls back to the line-by-line scan.
///
/// Obtained via the generated `Parser::find_*` methods; rarely constructed
/// directly.
pub struct LineUniformIter<'a, T, M, F>
where
    M: Fn(u8) -> bool,
    F: Fn(u8) -> T,
{
    src: &'a [u8],
    min: u32,
    eol: u8,
    sep: u8,
    matches: M,
    make: F,
    pos: usize,
    needles: [u8; 3],
    nn: usize,
    streaming: bool,
    _t: std::marker::PhantomData<T>,
}

impl<'a, T, M, F> LineUniformIter<'a, T, M, F>
where
    M: Fn(u8) -> bool,
    F: Fn(u8) -> T,
{
    /// Create an iterator over uniform-line elements.
    ///
    /// # Parameters
    ///
    /// - `src` — source byte slice to scan.
    /// - `min` — minimum number of delimiter byte occurrences required on a
    ///   matching line.
    /// - `eol` — line terminator byte.
    /// - `sep` — separator byte; allowed between delimiter occurrences on a
    ///   matching line.
    /// - `matches` — predicate that returns `true` for valid delimiter bytes;
    ///   all delimiter occurrences on a line must be the same byte.
    /// - `make` — closure that receives the delimiter byte and constructs the
    ///   metadata value `T`.
    pub fn new(src: &'a [u8], min: u32, eol: u8, sep: u8, matches: M, make: F) -> Self {
        let mut needles = [0u8; 3];
        let (nn, streaming) = match probe_matcher(&matches, &mut needles) {
            Some(n) => (n, true),
            None => (0, false),
        };
        Self {
            src,
            min,
            eol,
            sep,
            matches,
            make,
            pos: 0,
            needles,
            nn,
            streaming,
            _t: std::marker::PhantomData,
        }
    }
}

impl<T, M, F> LineUniformIter<'_, T, M, F>
where
    M: Fn(u8) -> bool,
    F: Fn(u8) -> T,
{
    /// Streaming scan: `memchr` for the probed delimiter bytes, line-start
    /// check, then a single forward walk that both validates the line and
    /// finds its end.
    fn next_streaming(&mut self) -> Option<(T, Span)> {
        let src = self.src;
        let len = src.len();
        loop {
            let p = find_any_of(&self.needles, self.nn, &src[self.pos..])? + self.pos;

            // The delimiter must begin its line. A hit mid-line rules the
            // whole line out (a valid line consists of delimiter/`sep` only,
            // so no later hit on it can be a line start): skip to the next
            // line instead of rejecting the remaining hits one by one.
            if p > 0 && src[p - 1] != self.eol {
                let le = find_line_end(src, p, self.eol);
                self.pos = if le < len { le + 1 } else { len };
                continue;
            }

            let delim = src[p];
            let mut count = 0u32;
            let mut i = p;
            let mut valid = true;
            while i < len {
                let b = src[i];
                if b == self.eol {
                    break;
                }
                if b == delim {
                    count += 1;
                } else if b != self.sep {
                    valid = false;
                    break;
                }
                i += 1;
            }

            if valid && count >= self.min {
                let meta = (self.make)(delim);
                let span = Span::new(p as u32, i as u32);
                self.pos = if i < len { i + 1 } else { len };
                return Some((meta, span));
            }

            // Invalid or short line: no other hit on it can match — skip it.
            let le = find_line_end(src, i, self.eol);
            self.pos = if le < len { le + 1 } else { len };
        }
    }
}

impl<T, M, F> Iterator for LineUniformIter<'_, T, M, F>
where
    M: Fn(u8) -> bool,
    F: Fn(u8) -> T,
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

            if self.pos < le {
                let delim = src[self.pos];
                if (self.matches)(delim) {
                    let mut count = 0u32;
                    let mut valid = true;
                    for &b in &src[self.pos..le] {
                        if b == delim {
                            count += 1;
                        } else if b != self.sep {
                            valid = false;
                            break;
                        }
                    }
                    if valid && count >= self.min {
                        let meta = (self.make)(delim);
                        let span = Span::new(self.pos as u32, le as u32);
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

    // Helper closures for testing
    fn stab_matches(b: u8) -> bool {
        b == b'=' || b == b'-'
    }

    fn stab_make(b: u8) -> u8 {
        b
    }

    // 01. Matches a perfect uniform single line containing only the delimiter
    #[test]
    fn test_01_basic_uniform_line() {
        let src = b"====";
        let mut iter = LineUniformIter::new(src, 3, b'\n', b' ', stab_matches, stab_make);

        assert_eq!(iter.next(), Some((b'=', Span::new(0, 4))));
        assert_eq!(iter.next(), None);
    }

    // 02. Matches a line where delimiters are correctly interleaved with the allowed separator
    #[test]
    fn test_02_interleaved_separators() {
        let src = b"= = = =";
        let mut iter = LineUniformIter::new(src, 3, b'\n', b' ', stab_matches, stab_make);

        assert_eq!(iter.next(), Some((b'=', Span::new(0, 7))));
        assert_eq!(iter.next(), None);
    }

    // 03. Returns None when the line contains an invalid character that is neither delim nor sep
    #[test]
    fn test_03_invalid_character_disruption() {
        let src = b"==x==";
        let mut iter = LineUniformIter::new(src, 3, b'\n', b' ', stab_matches, stab_make);

        assert_eq!(iter.next(), None);
    }

    // 04. Skips an invalid line and successfully moves on to parse subsequent valid lines
    #[test]
    fn test_04_skip_invalid_line_to_valid() {
        let src = b"===\n==x==\n===";
        let mut iter = LineUniformIter::new(src, 3, b'\n', b' ', stab_matches, stab_make);

        assert_eq!(iter.next(), Some((b'=', Span::new(0, 3))));
        assert_eq!(iter.next(), Some((b'=', Span::new(10, 13))));
        assert_eq!(iter.next(), None);
    }

    // 05. Rejects a line if the total count of the delimiter is strictly less than the min threshold
    #[test]
    fn test_05_below_min_threshold() {
        let src = b"==";
        let mut iter = LineUniformIter::new(src, 3, b'\n', b' ', stab_matches, stab_make);

        assert_eq!(iter.next(), None);
    }

    // 06. Rejects a uniform line if the closure matches returns false for its delimiter
    #[test]
    fn test_06_delimiter_rejected_by_closure() {
        let src = b"####";
        let mut iter = LineUniformIter::new(src, 3, b'\n', b' ', stab_matches, stab_make);

        assert_eq!(iter.next(), None);
    }

    // 07. Gracefully handles completely empty input data by immediately returning None
    #[test]
    fn test_07_empty_input() {
        let src = b"";
        let mut iter = LineUniformIter::new(src, 3, b'\n', b' ', stab_matches, stab_make);

        assert_eq!(iter.next(), None);
    }

    // 08. Safely skips sequential empty lines without panicking or matching them
    #[test]
    fn test_08_consecutive_empty_lines() {
        let src = b"\n\n\n";
        let mut iter = LineUniformIter::new(src, 3, b'\n', b' ', stab_matches, stab_make);

        assert_eq!(iter.next(), None);
    }

    // 09. Correctly extracts different kinds of valid delimiters across multiple lines
    #[test]
    fn test_09_multiple_distinct_delimiters() {
        let src = b"====\n----\n====";
        let mut iter = LineUniformIter::new(src, 3, b'\n', b' ', stab_matches, stab_make);

        assert_eq!(iter.next(), Some((b'=', Span::new(0, 4))));
        assert_eq!(iter.next(), Some((b'-', Span::new(5, 9))));
        assert_eq!(iter.next(), Some((b'=', Span::new(10, 14))));
        assert_eq!(iter.next(), None);
    }

    // 10. Successfully matches a valid uniform line that terminates exactly at EOF without an EOL character
    #[test]
    fn test_10_missing_trailing_eol_at_eof() {
        let src = b"====";
        let mut iter = LineUniformIter::new(src, 3, b'\n', b' ', stab_matches, stab_make);

        assert_eq!(iter.next(), Some((b'=', Span::new(0, 4))));
        assert_eq!(iter.next(), None);
    }

    // 11. Skips over empty lines embedded between otherwise valid uniform lines
    #[test]
    fn test_11_empty_lines_interleaved() {
        let src = b"====\n\n----";
        let mut iter = LineUniformIter::new(src, 3, b'\n', b' ', stab_matches, stab_make);

        assert_eq!(iter.next(), Some((b'=', Span::new(0, 4))));
        assert_eq!(iter.next(), Some((b'-', Span::new(6, 10))));
        assert_eq!(iter.next(), None);
    }

    // 12. Rejects a line starting with a separator if that separator fails the matches predicate
    #[test]
    fn test_12_line_starting_with_separator() {
        let src = b" ====";
        let mut iter = LineUniformIter::new(src, 3, b'\n', b' ', stab_matches, stab_make);

        assert_eq!(iter.next(), None);
    }

    // 13. Successfully matches a line when the allowed separator characters are trailing at the end
    #[test]
    fn test_13_trailing_separators_on_valid_line() {
        let src = b"====    ";
        let mut iter = LineUniformIter::new(src, 3, b'\n', b' ', stab_matches, stab_make);

        assert_eq!(iter.next(), Some((b'=', Span::new(0, 8))));
        assert_eq!(iter.next(), None);
    }

    // 14. Matches a line that meets the minimum count threshold exactly on the edge
    #[test]
    fn test_14_exact_minimum_count_match() {
        let src = b"===";
        let mut iter = LineUniformIter::new(src, 3, b'\n', b' ', stab_matches, stab_make);

        assert_eq!(iter.next(), Some((b'=', Span::new(0, 3))));
        assert_eq!(iter.next(), None);
    }

    // 15. Verifies that the internal position tracker advances correctly when iterating through matches
    #[test]
    fn test_15_position_advancement_and_exhaustion() {
        let src = b"===\n---";
        let mut iter = LineUniformIter::new(src, 3, b'\n', b' ', stab_matches, stab_make);

        assert!(iter.next().is_some());
        assert!(iter.next().is_some());
        assert!(iter.next().is_none());
    }
}
