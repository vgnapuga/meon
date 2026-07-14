//! [`KvIter`] — key-value pair standalone iterator.

use super::common::*;
/// Iterator over `key = value` pairs in a byte slice.
///
/// One streaming `memchr` pass finds the `eq` bytes; lines without one are
/// never visited. Everything between the preceding word boundary and `eq` is
/// the key; everything between `eq` and the first `end_byte` (or end of line)
/// is the value. When `allow_sep` is `true`, spaces around `eq` are trimmed
/// from both key and value. The pair structure itself stays line-bounded by
/// contract: the value never extends past its line.
///
/// Yields one item per `eq` occurrence via the `make` closure, which receives
/// `(key_span, value_span)` and constructs the output type `T`.
///
/// Obtained via the generated `Parser::find_*` methods; rarely constructed
/// directly.
pub struct KvIter<'a, T, F>
where
    F: Fn(Span, Span) -> T,
{
    src: &'a [u8],
    eq: u8,
    end_byte: u8,
    allow_sep: bool,
    eol: u8,
    sep: u8,
    tab: u8,
    make: F,
    pos: usize,
    _t: std::marker::PhantomData<T>,
}

impl<'a, T, F> KvIter<'a, T, F>
where
    F: Fn(Span, Span) -> T,
{
    /// Create an iterator over `key = value` pairs.
    ///
    /// # Parameters
    ///
    /// - `src` — source byte slice to scan.
    /// - `eq` — the equality byte that separates key from value (e.g. `b'='`).
    /// - `end_byte` — byte that terminates the value (e.g. `b';'` or `b'\n'`);
    ///   if not found the value extends to end of line.
    /// - `allow_sep` — when `true`, spaces around `eq` are trimmed from both
    ///   key and value.
    /// - `eol` — line terminator byte.
    /// - `sep` — word separator byte used for key boundary detection.
    /// - `tab` — tab byte, treated equivalently to `sep` for key boundaries.
    /// - `make` — closure that receives `(key_span, value_span)` and constructs
    ///   the output value `T`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        src: &'a [u8],
        eq: u8,
        end_byte: u8,
        allow_sep: bool,
        eol: u8,
        sep: u8,
        tab: u8,
        make: F,
    ) -> Self {
        Self {
            src,
            eq,
            end_byte,
            allow_sep,
            eol,
            sep,
            tab,
            make,
            pos: 0,
            _t: std::marker::PhantomData,
        }
    }
}

impl<T, F> Iterator for KvIter<'_, T, F>
where
    F: Fn(Span, Span) -> T,
{
    type Item = T;

    fn next(&mut self) -> Option<T> {
        let src = self.src;
        let len = src.len();
        // One streaming pass for the `eq` byte. The backward key walks are
        // bounded by `eol` (line starts) and by `bound` — the start of the
        // unconsumed region, so a key never reaches back into a previous
        // pair's value on the same line, exactly as in the line-by-line scan.
        let bound = self.pos;
        let eq_pos = memchr::memchr(self.eq, &src[self.pos..])? + self.pos;

        let mut key_end = eq_pos;
        if self.allow_sep {
            while key_end > bound && src[key_end - 1] == self.sep {
                key_end -= 1;
            }
        }
        let mut ks = key_end;
        while ks > bound
            && src[ks - 1] != self.sep
            && src[ks - 1] != self.tab
            && src[ks - 1] != self.eol
        {
            ks -= 1;
        }

        let mut val_start = eq_pos + 1;
        if self.allow_sep {
            while val_start < len && src[val_start] == self.sep {
                val_start += 1;
            }
        }
        // The value ends at the first `end_byte` or at its line's end,
        // whichever comes first.
        let (val_end, ate_terminator) =
            match memchr::memchr2(self.end_byte, self.eol, &src[val_start..]) {
                Some(rr) => (val_start + rr, true),
                None => (len, false),
            };

        self.pos = val_end + usize::from(ate_terminator);
        Some((self.make)(
            Span::new(ks as u32, key_end as u32),
            Span::new(val_start as u32, val_end as u32),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stub_make(k: Span, v: Span) -> (Span, Span) {
        (k, v)
    }

    // 01. Parses a clean, standard key-value pair without any surrounding spaces
    #[test]
    fn test_01_basic_kv_pair() {
        let src = b"key=value";
        let mut iter = KvIter::new(src, b'=', b';', true, b'\n', b' ', b'\t', stub_make);

        assert_eq!(iter.next(), Some((Span::new(0, 3), Span::new(4, 9))));
        assert_eq!(iter.next(), None);
    }

    // 02. Trims spaces around the equality sign when allow_sep is explicitly enabled
    #[test]
    fn test_02_trim_spaces_enabled() {
        let src = b"key   =   value";
        let mut iter = KvIter::new(src, b'=', b';', true, b'\n', b' ', b'\t', stub_make);

        assert_eq!(iter.next(), Some((Span::new(0, 3), Span::new(10, 15))));
        assert_eq!(iter.next(), None);
    }

    // 03. Preserves spaces surrounding the equality sign when allow_sep is disabled
    #[test]
    fn test_03_trim_spaces_disabled() {
        let src = b"key = val";
        let mut iter = KvIter::new(src, b'=', b';', false, b'\n', b' ', b'\t', stub_make);

        assert_eq!(iter.next(), Some((Span::new(4, 4), Span::new(5, 9))));
        assert_eq!(iter.next(), None);
    }

    // 04. Parses multiple independent key-value pairs separated by the end_byte on a single line
    #[test]
    fn test_04_multiple_pairs_single_line() {
        let src = b"a=1;b=2";
        let mut iter = KvIter::new(src, b'=', b';', true, b'\n', b' ', b'\t', stub_make);

        assert_eq!(iter.next(), Some((Span::new(0, 1), Span::new(2, 3))));
        assert_eq!(iter.next(), Some((Span::new(4, 5), Span::new(6, 7))));
        assert_eq!(iter.next(), None);
    }

    // 05. Isolates the key correctly when it is preceded by leading space separators on the line
    #[test]
    fn test_05_leading_spaces_before_key() {
        let src = b"   key=value";
        let mut iter = KvIter::new(src, b'=', b';', true, b'\n', b' ', b'\t', stub_make);

        assert_eq!(iter.next(), Some((Span::new(3, 6), Span::new(7, 12))));
        assert_eq!(iter.next(), None);
    }

    // 06. Correctly identifies the key boundary when it is preceded by a tab character
    #[test]
    fn test_06_tab_separator_key_boundary() {
        let src = b"prefix\tkey=value";
        let mut iter = KvIter::new(src, b'=', b';', true, b'\n', b' ', b'\t', stub_make);

        assert_eq!(iter.next(), Some((Span::new(7, 10), Span::new(11, 16))));
        assert_eq!(iter.next(), None);
    }

    // 07. Truncates the value slice precisely at the specified end_byte marker
    #[test]
    fn test_07_value_terminated_by_end_byte() {
        let src = b"key=value;ignored_trailing_stuff";
        let mut iter = KvIter::new(src, b'=', b';', true, b'\n', b' ', b'\t', stub_make);

        assert_eq!(iter.next(), Some((Span::new(0, 3), Span::new(4, 9))));
        assert_eq!(iter.next(), None);
    }

    // 08. Gracefully falls back to the end of the line if no matching end_byte is found for the value
    #[test]
    fn test_08_value_terminated_by_eol() {
        let src = b"key=value_to_eol";
        let mut iter = KvIter::new(src, b'=', b';', true, b'\n', b' ', b'\t', stub_make);

        assert_eq!(iter.next(), Some((Span::new(0, 3), Span::new(4, 16))));
        assert_eq!(iter.next(), None);
    }

    // 09. Skips an entire line if it does not contain the required equality delimiter byte
    #[test]
    fn test_09_skip_line_missing_equality_byte() {
        let src = b"invalid_line_without_delimiter\nvalid=pair";
        let mut iter = KvIter::new(src, b'=', b';', true, b'\n', b' ', b'\t', stub_make);

        assert_eq!(iter.next(), Some((Span::new(31, 36), Span::new(37, 41))));
        assert_eq!(iter.next(), None);
    }

    // 10. Iterates seamlessly through multiple sequential lines containing valid pairs
    #[test]
    fn test_10_multiline_pairs() {
        let src = b"k1=v1\nk2=v2\nk3=v3";
        let mut iter = KvIter::new(src, b'=', b';', true, b'\n', b' ', b'\t', stub_make);

        assert_eq!(iter.next(), Some((Span::new(0, 2), Span::new(3, 5))));
        assert_eq!(iter.next(), Some((Span::new(6, 8), Span::new(9, 11))));
        assert_eq!(iter.next(), Some((Span::new(12, 14), Span::new(15, 17))));
        assert_eq!(iter.next(), None);
    }

    // 11. Properly returns an empty span for the value when the equality sign is at the very end
    #[test]
    fn test_11_empty_value_span() {
        let src = b"key=";
        let mut iter = KvIter::new(src, b'=', b';', true, b'\n', b' ', b'\t', stub_make);

        assert_eq!(iter.next(), Some((Span::new(0, 3), Span::new(4, 4))));
        assert_eq!(iter.next(), None);
    }

    // 12. Returns an empty span for the key if the line starts directly with the equality byte
    #[test]
    fn test_12_empty_key_span() {
        let src = b"=value";
        let mut iter = KvIter::new(src, b'=', b';', true, b'\n', b' ', b'\t', stub_make);

        assert_eq!(iter.next(), Some((Span::new(0, 0), Span::new(1, 6))));
        assert_eq!(iter.next(), None);
    }

    // 13. Safely handles and skips consecutive empty lines without emitting false matches
    #[test]
    fn test_13_consecutive_empty_lines() {
        let src = b"\n\nkey=value\n\n";
        let mut iter = KvIter::new(src, b'=', b';', true, b'\n', b' ', b'\t', stub_make);

        assert_eq!(iter.next(), Some((Span::new(2, 5), Span::new(6, 11))));
        assert_eq!(iter.next(), None);
    }

    // 14. Immediately returns None when initialized with a completely empty source byte slice
    #[test]
    fn test_14_empty_source_slice() {
        let src = b"";
        let mut iter = KvIter::new(src, b'=', b';', true, b'\n', b' ', b'\t', stub_make);

        assert_eq!(iter.next(), None);
    }

    // 15. Correctly handles a line that terminates with the end_byte directly at the end of the line
    #[test]
    fn test_15_end_byte_at_line_boundary() {
        let src = b"key=value;";
        let mut iter = KvIter::new(src, b'=', b';', true, b'\n', b' ', b'\t', stub_make);

        assert_eq!(iter.next(), Some((Span::new(0, 3), Span::new(4, 9))));
        assert_eq!(iter.next(), None);
    }
}
