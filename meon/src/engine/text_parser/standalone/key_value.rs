//! [`KvIter`] — key-value pair standalone iterator.

use super::common::*;
/// Iterator over `key = value` pairs in a byte slice.
///
/// It reads the rule the way the full parse does with no other rule's
/// frame open:
///
/// - With `max_nest = 0` no pair opens: the stack has no room for one.
/// - An unescaped `eq` byte opens a pair; the rest of an `eq` run is value
///   content, and so is every `eq` inside an open value.
/// - The key is the word before `eq`: the back-scan stops at `sep`, `tab`,
///   `eol` and at the start of the key segment, which follows the `end` run
///   that closed the previous value. With `allow_sep` the `sep` bytes before
///   `eq` are trimmed from the key, and those after it skipped before the
///   value.
/// - The value ends at the first unescaped `end_byte` after its start, or
///   where its paragraph ends: an empty line — two consecutive `eol` bytes —
///   or the end of input, a trailing `eol` excluded. A single `eol` is value
///   content, unless `end_byte` is `eol`. The rest of the `end` run is text,
///   and the next key segment starts after it.
///
/// Other rules are not seen: in the full parse a container's open starts a
/// key segment and its close ends the value inside it, an opaque construct
/// right before `eq` is the key, and line rules end runs.
///
/// Yields one item per pair via the `make` closure, which receives
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
    escape: u8,
    make: F,
    pos: usize,
    /// Where the key segment starts: after the `end` run that closed the
    /// last value, or at the start of input.
    floor: usize,
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
    ///   if not found the value extends to the end of its paragraph.
    /// - `allow_sep` — when `true`, the `sep` bytes around `eq` are trimmed
    ///   from both key and value.
    /// - `eol` — line terminator byte.
    /// - `sep` — word separator byte used for key boundary detection.
    /// - `tab` — tab byte, treated equivalently to `sep` for key boundaries.
    /// - `escape` — escape prefix byte; an `eq` or `end_byte` preceded by an
    ///   odd number of escape bytes is none.
    /// - `max_nest` — the grammar's `max_nest`.
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
        escape: u8,
        max_nest: usize,
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
            escape,
            make,
            // With no room on the stack, no pair opens.
            pos: if max_nest == 0 { src.len() } else { 0 },
            floor: 0,
            _t: std::marker::PhantomData,
        }
    }

    /// Where the value starting at `from` ends, and where the next key
    /// segment starts: at the first unescaped `end_byte` and past its run,
    /// or at the end of the paragraph.
    #[inline]
    fn value_end(&self, from: usize) -> (usize, usize) {
        let (src, end, eol) = (self.src, self.end_byte, self.eol);
        let mut j = from;
        if end == eol {
            // A single unescaped `eol` ends the value; an empty line or the
            // end of input ends its paragraph.
            loop {
                let Some(r) = memchr::memchr(eol, &src[j..]) else {
                    return (src.len(), src.len());
                };
                let q = j + r;
                if q + 1 >= src.len() || src[q + 1] == eol {
                    return (q, q);
                }
                if count_escape(src, q, self.escape) % 2 == 0 {
                    return (q, q + 1);
                }
                j = q + 1;
            }
        }
        loop {
            match scan_paragraph(src, j, end, eol, Some) {
                Ok(q) if count_escape(src, q, self.escape) % 2 == 1 => j = q + 1,
                Ok(q) => {
                    let mut e = q;
                    while e < src.len() && src[e] == end {
                        e += 1;
                    }
                    return (q, e);
                }
                Err(stop) => return (stop, stop),
            }
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
        loop {
            let eq_pos = memchr::memchr(self.eq, &src[self.pos..])? + self.pos;
            if count_escape(src, eq_pos, self.escape) % 2 == 1 {
                self.pos = eq_pos + 1;
                continue;
            }

            let floor = self.floor;
            let mut key_end = eq_pos;
            if self.allow_sep {
                while key_end > floor && src[key_end - 1] == self.sep {
                    key_end -= 1;
                }
            }
            let mut ks = key_end;
            while ks > floor
                && src[ks - 1] != self.sep
                && src[ks - 1] != self.tab
                && src[ks - 1] != self.eol
            {
                ks -= 1;
            }

            // The first byte of the run is `eq`; the rest is value content.
            let mut val_start = eq_pos + 1;
            if self.allow_sep {
                while val_start < len && src[val_start] == self.sep {
                    val_start += 1;
                }
            }
            let (val_end, next) = self.value_end(val_start);
            self.pos = next;
            self.floor = next;
            return Some((self.make)(
                Span::new(ks as u32, key_end as u32),
                Span::new(val_start as u32, val_end as u32),
            ));
        }
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
        let mut iter = KvIter::new(
            src, b'=', b';', true, b'\n', b' ', b'\t', b'\\', 4, stub_make,
        );

        assert_eq!(iter.next(), Some((Span::new(0, 3), Span::new(4, 9))));
        assert_eq!(iter.next(), None);
    }

    // 02. Trims spaces around the equality sign when allow_sep is explicitly enabled
    #[test]
    fn test_02_trim_spaces_enabled() {
        let src = b"key   =   value";
        let mut iter = KvIter::new(
            src, b'=', b';', true, b'\n', b' ', b'\t', b'\\', 4, stub_make,
        );

        assert_eq!(iter.next(), Some((Span::new(0, 3), Span::new(10, 15))));
        assert_eq!(iter.next(), None);
    }

    // 03. Preserves spaces surrounding the equality sign when allow_sep is disabled
    #[test]
    fn test_03_trim_spaces_disabled() {
        let src = b"key = val";
        let mut iter = KvIter::new(
            src, b'=', b';', false, b'\n', b' ', b'\t', b'\\', 4, stub_make,
        );

        assert_eq!(iter.next(), Some((Span::new(4, 4), Span::new(5, 9))));
        assert_eq!(iter.next(), None);
    }

    // 04. Parses multiple independent key-value pairs separated by the end_byte on a single line
    #[test]
    fn test_04_multiple_pairs_single_line() {
        let src = b"a=1;b=2";
        let mut iter = KvIter::new(
            src, b'=', b';', true, b'\n', b' ', b'\t', b'\\', 4, stub_make,
        );

        assert_eq!(iter.next(), Some((Span::new(0, 1), Span::new(2, 3))));
        assert_eq!(iter.next(), Some((Span::new(4, 5), Span::new(6, 7))));
        assert_eq!(iter.next(), None);
    }

    // 05. Isolates the key correctly when it is preceded by leading space separators on the line
    #[test]
    fn test_05_leading_spaces_before_key() {
        let src = b"   key=value";
        let mut iter = KvIter::new(
            src, b'=', b';', true, b'\n', b' ', b'\t', b'\\', 4, stub_make,
        );

        assert_eq!(iter.next(), Some((Span::new(3, 6), Span::new(7, 12))));
        assert_eq!(iter.next(), None);
    }

    // 06. Correctly identifies the key boundary when it is preceded by a tab character
    #[test]
    fn test_06_tab_separator_key_boundary() {
        let src = b"prefix\tkey=value";
        let mut iter = KvIter::new(
            src, b'=', b';', true, b'\n', b' ', b'\t', b'\\', 4, stub_make,
        );

        assert_eq!(iter.next(), Some((Span::new(7, 10), Span::new(11, 16))));
        assert_eq!(iter.next(), None);
    }

    // 07. Truncates the value slice precisely at the specified end_byte marker
    #[test]
    fn test_07_value_terminated_by_end_byte() {
        let src = b"key=value;ignored_trailing_stuff";
        let mut iter = KvIter::new(
            src, b'=', b';', true, b'\n', b' ', b'\t', b'\\', 4, stub_make,
        );

        assert_eq!(iter.next(), Some((Span::new(0, 3), Span::new(4, 9))));
        assert_eq!(iter.next(), None);
    }

    // 08. Gracefully falls back to the end of the line if no matching end_byte is found for the value
    #[test]
    fn test_08_value_terminated_by_eol() {
        let src = b"key=value_to_eol";
        let mut iter = KvIter::new(
            src, b'=', b';', true, b'\n', b' ', b'\t', b'\\', 4, stub_make,
        );

        assert_eq!(iter.next(), Some((Span::new(0, 3), Span::new(4, 16))));
        assert_eq!(iter.next(), None);
    }

    // 09. Skips an entire line if it does not contain the required equality delimiter byte
    #[test]
    fn test_09_skip_line_missing_equality_byte() {
        let src = b"invalid_line_without_delimiter\nvalid=pair";
        let mut iter = KvIter::new(
            src, b'=', b';', true, b'\n', b' ', b'\t', b'\\', 4, stub_make,
        );

        assert_eq!(iter.next(), Some((Span::new(31, 36), Span::new(37, 41))));
        assert_eq!(iter.next(), None);
    }

    // 10. A value goes on across single line breaks: the lines of one
    //     paragraph are one run, and an `eq` inside a value is content
    #[test]
    fn test_10_multiline_pairs() {
        let src = b"k1=v1\nk2=v2\nk3=v3";
        let mut iter = KvIter::new(
            src, b'=', b';', true, b'\n', b' ', b'\t', b'\\', 4, stub_make,
        );

        assert_eq!(iter.next(), Some((Span::new(0, 2), Span::new(3, 17))));
        assert_eq!(iter.next(), None);
    }

    // 11. Properly returns an empty span for the value when the equality sign is at the very end
    #[test]
    fn test_11_empty_value_span() {
        let src = b"key=";
        let mut iter = KvIter::new(
            src, b'=', b';', true, b'\n', b' ', b'\t', b'\\', 4, stub_make,
        );

        assert_eq!(iter.next(), Some((Span::new(0, 3), Span::new(4, 4))));
        assert_eq!(iter.next(), None);
    }

    // 12. Returns an empty span for the key if the line starts directly with the equality byte
    #[test]
    fn test_12_empty_key_span() {
        let src = b"=value";
        let mut iter = KvIter::new(
            src, b'=', b';', true, b'\n', b' ', b'\t', b'\\', 4, stub_make,
        );

        assert_eq!(iter.next(), Some((Span::new(0, 0), Span::new(1, 6))));
        assert_eq!(iter.next(), None);
    }

    // 13. Safely handles and skips consecutive empty lines without emitting false matches
    #[test]
    fn test_13_consecutive_empty_lines() {
        let src = b"\n\nkey=value\n\n";
        let mut iter = KvIter::new(
            src, b'=', b';', true, b'\n', b' ', b'\t', b'\\', 4, stub_make,
        );

        assert_eq!(iter.next(), Some((Span::new(2, 5), Span::new(6, 11))));
        assert_eq!(iter.next(), None);
    }

    // 14. Immediately returns None when initialized with a completely empty source byte slice
    #[test]
    fn test_14_empty_source_slice() {
        let src = b"";
        let mut iter = KvIter::new(
            src, b'=', b';', true, b'\n', b' ', b'\t', b'\\', 4, stub_make,
        );

        assert_eq!(iter.next(), None);
    }

    // 15. Correctly handles a line that terminates with the end_byte directly at the end of the line
    #[test]
    fn test_15_end_byte_at_line_boundary() {
        let src = b"key=value;";
        let mut iter = KvIter::new(
            src, b'=', b';', true, b'\n', b' ', b'\t', b'\\', 4, stub_make,
        );

        assert_eq!(iter.next(), Some((Span::new(0, 3), Span::new(4, 9))));
        assert_eq!(iter.next(), None);
    }

    type P = ((u32, u32), (u32, u32));

    fn pairs(src: &[u8], end: u8) -> Vec<P> {
        KvIter::new(
            src,
            b'=',
            end,
            true,
            b'\n',
            b' ',
            b'\t',
            b'\\',
            4,
            |k, v| ((k.start, k.end), (v.start, v.end)),
        )
        .collect()
    }

    // 16. An escaped `eq` opens no pair; the rest of its run is read anew
    #[test]
    fn test_16_escaped_eq() {
        assert!(pairs(b"a\\=1", b';').is_empty());
        assert_eq!(pairs(b"a\\==b", b';'), vec![((0, 3), (4, 5))]);
    }

    // 17. An escaped `end_byte` is value content
    #[test]
    fn test_17_escaped_end() {
        assert_eq!(pairs(b"a=x\\;b=1", b';'), vec![((0, 1), (2, 8))]);
    }

    // 18. The next key segment starts past the whole `end` run
    #[test]
    fn test_18_end_run() {
        assert_eq!(
            pairs(b"a=1;;b=2", b';'),
            vec![((0, 1), (2, 3)), ((5, 6), (7, 8))]
        );
        assert_eq!(
            pairs(b"a=1;;;b=2", b';'),
            vec![((0, 1), (2, 3)), ((6, 7), (8, 9))]
        );
        assert_eq!(
            pairs(b"a=;;b=1", b';'),
            vec![((0, 1), (2, 2)), ((4, 5), (6, 7))]
        );
    }

    // 19. A value ends with its paragraph, a trailing `eol` excluded
    #[test]
    fn test_19_paragraph_bounds() {
        assert_eq!(
            pairs(b"a=1\nb=2;c=3", b';'),
            vec![((0, 1), (2, 7)), ((8, 9), (10, 11))]
        );
        assert_eq!(
            pairs(b"a=1\n\nb=2", b';'),
            vec![((0, 1), (2, 3)), ((5, 6), (7, 8))]
        );
        assert_eq!(pairs(b"a=1\n", b';'), vec![((0, 1), (2, 3))]);
    }

    // 20. With `end_byte` the `eol`, a single unescaped line break ends the
    //     value and an escaped one is content
    #[test]
    fn test_20_end_is_eol() {
        assert_eq!(
            pairs(b"a=1\nb=2", b'\n'),
            vec![((0, 1), (2, 3)), ((4, 5), (6, 7))]
        );
        assert_eq!(pairs(b"a=1\\\nb=2", b'\n'), vec![((0, 1), (2, 8))]);
        // An escaped line break before an empty line still ends the
        // paragraph.
        assert_eq!(
            pairs(b"a=1\\\n\nb=2", b'\n'),
            vec![((0, 1), (2, 4)), ((6, 7), (8, 9))]
        );
        assert_eq!(
            pairs(b"a=1\n\nb=2", b'\n'),
            vec![((0, 1), (2, 3)), ((5, 6), (7, 8))]
        );
    }

    // 21. Many unclosed values and many pairs are read once
    #[test]
    fn test_21_linear() {
        let src = b"a=b ".repeat(20_000);
        assert_eq!(pairs(&src, b';'), vec![((0, 1), (2, 80_000))]);
        let src = b"a=b;".repeat(20_000);
        assert_eq!(pairs(&src, b';').len(), 20_000);
    }

    // 22. With `max_nest = 0` no pair opens
    #[test]
    fn test_22_no_room() {
        let got: Vec<_> = KvIter::new(
            b"a=1;b=2", b'=', b';', true, b'\n', b' ', b'\t', b'\\', 0, stub_make,
        )
        .collect();
        assert!(got.is_empty());
    }

    // ---- The generated methods ------------------------------------------ //

    /// The `find_*` method `define_parser!` generates for a `key_value`
    /// rule. The expansion inside this crate trips lints that a dependent
    /// crate never sees.
    #[allow(unused, unused_comparisons, clippy::out_of_bounds_indexing)]
    mod generated {
        use crate::span::Span;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct Pair {
            pub key: Span,
            pub value: Span,
        }

        macro_rules! pairs {
            ($m:ident, $p:ident, $esc:tt, $maxn:tt) => {
                mod $m {
                    use super::Pair;
                    crate::define_parser!($p {
                        sep = b' ', eol = b'\n', tab = b'\t', escape = $esc, max_nest = $maxn;
                        inline {
                            on_trigger(b'=') {
                                key_value: Pair { eq = b'='; allow_sep = true; end = b';'; key => key, value => value, } => pairs [40]
                            }
                            fallback => texts [10];
                        }
                        blocks { fallback => paragraphs [80]; }
                    });
                }
            };
        }

        pairs!(caret, Caret, b'^', 4);
        pairs!(no_room, NoRoom, b'\\', 0);

        // 23. The finder gets the grammar's own escape byte, which escapes
        //     `eq` and `end`
        #[test]
        fn test_23_generated_escape_byte() {
            for src in [&b"a^=1"[..], b"a\\=1", b"a=x^;b=1", b"a=1;;b=2"] {
                let full = caret::CaretParser::parse(src).pairs.to_vec();
                let find: Vec<Pair> = caret::CaretParser::find_pairs(src).collect();
                assert_eq!(find, full, "{:?}", String::from_utf8_lossy(src));
            }
            assert_eq!(caret::CaretParser::find_pairs(b"a^=1").count(), 0);
        }

        // 24. The finder gets the grammar's `max_nest`: with 0, neither the
        //     full parse nor the finder opens a pair
        #[test]
        fn test_24_generated_no_room() {
            let src = b"a=1;b=2";
            assert!(no_room::NoRoomParser::parse(src).pairs.is_empty());
            assert_eq!(no_room::NoRoomParser::find_pairs(src).count(), 0);
        }

        #[derive(Debug, Clone, Copy)]
        pub struct Link {
            pub text: Span,
            pub url: Span,
            pub img: bool,
        }

        // Keys next to every kind of opaque span: a string, an opaque frame
        // and an opaque `chained` match.
        mod quoted {
            use super::{Link, Pair};
            crate::define_parser!(Quoted {
                sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\', max_nest = 4;
                inline {
                    on_trigger(b'[', b'=', b'"', b'{', b'}') {
                        symmetric b'"' { parse_inside = false; balanced = false; 1 => strs [40], }
                        asymmetric b'{', b'}' { balanced = true; parse_inside = false; 1 => objs [40], }
                        chained: Link {
                            | b'[', b']' | { parse_inside = false; balanced = false; } => text,
                            | b'(', b')' | { parse_inside = false; balanced = false; } => url,
                            prefix | b'!' | => img,
                        } => links [40]
                        key_value: Pair { eq = b'='; allow_sep = true; end = b';'; key => key, value => value, } => pairs [40]
                    }
                    fallback => texts [10];
                }
                blocks { fallback => paragraphs [80]; }
            });
        }

        // 25. An opaque span ending right before `eq` is the key whatever
        //     rule made it, and the key after one starts past it
        #[test]
        fn test_25_generated_opaque_keys() {
            use quoted::QuotedParser;
            let keys = |src: &[u8]| -> Vec<(u32, u32)> {
                QuotedParser::parse(src)
                    .pairs
                    .iter()
                    .map(|p| (p.key.start, p.key.end))
                    .collect()
            };
            assert_eq!(keys(b"\"a b\"=1"), vec![(0, 5)]);
            assert_eq!(keys(b"{a b}=1"), vec![(0, 5)]);
            assert_eq!(keys(b"{a {b} c}=1"), vec![(0, 9)]);
            assert_eq!(keys(b"[a b](u)=1"), vec![(0, 8)]);
            assert_eq!(keys(b"![a](u) = 1"), vec![(0, 7)]);
            assert_eq!(keys(b"x [a](u)c=1"), vec![(8, 9)]);
            assert_eq!(keys(b"{a}b=1"), vec![(3, 4)]);
        }
    }
}
