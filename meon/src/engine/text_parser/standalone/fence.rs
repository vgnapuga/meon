//! [`FenceIter`] — fenced block standalone iterator.

use super::common::*;
/// Iterator over fenced block spans in a byte slice.
///
/// A fence opens when a line starts with at least `min` consecutive occurrences
/// of `byte` and contains no further `byte` after the opening run. It closes on
/// the next line that starts with at least as many `byte` bytes as the opener,
/// followed only by `sep` or `tab`. If no closing fence is found the span
/// extends to the end of the source.
///
/// The returned [`Span`] covers the entire block from the opening fence byte to
/// the end of the closing fence line (inclusive).
///
/// The scan is streaming: one `memchr` pass finds candidate fence bytes and an
/// O(1) previous-byte check keeps only the ones that begin a line — both for
/// opening and for closing fences — so content lines inside and outside blocks
/// are never walked byte-by-byte.
///
/// Obtained via the generated `Parser::find_*` methods; rarely constructed
/// directly.
pub struct FenceIter<'a> {
    src: &'a [u8],
    byte: u8,
    min: u8,
    eol: u8,
    sep: u8,
    tab: u8,
    pos: usize,
}

impl<'a> FenceIter<'a> {
    /// Create an iterator over fenced block spans.
    ///
    /// # Parameters
    ///
    /// - `src` — source byte slice to scan.
    /// - `byte` — the fence character (e.g. `` b'`' `` or `b'~'`).
    /// - `min` — minimum number of consecutive `byte` bytes required to open
    ///   or close a fence.
    /// - `eol` — line terminator byte.
    /// - `sep` — separator byte allowed (with `tab`) on a closing fence line
    ///   after the fence run.
    /// - `tab` — tab byte allowed on a closing fence line after the fence run.
    pub fn new(src: &'a [u8], byte: u8, min: u8, eol: u8, sep: u8, tab: u8) -> Self {
        Self {
            src,
            byte,
            min,
            eol,
            sep,
            tab,
            pos: 0,
        }
    }
}

impl Iterator for FenceIter<'_> {
    type Item = Span;

    fn next(&mut self) -> Option<Span> {
        let src = self.src;
        let len = src.len();
        loop {
            let p = memchr::memchr(self.byte, &src[self.pos..])? + self.pos;

            // An opening fence must begin its line.
            if p > 0 && src[p - 1] != self.eol {
                self.pos = p + 1;
                continue;
            }

            let mut c = 0u8;
            let mut i = p;
            while i < len && src[i] == self.byte {
                c = c.saturating_add(1);
                i += 1;
            }
            let le = find_line_end(src, i, self.eol);

            if c < self.min || src[i..le].contains(&self.byte) {
                // Not an opener, and nothing later on this line can be one
                // (line start is required): resume on the next line.
                self.pos = if le < len { le + 1 } else { len };
                continue;
            }

            let fence_start = p as u32;
            let fence_count = c;
            let mut search = if le < len { le + 1 } else { len };

            // Close search: only fence bytes at line starts are candidates;
            // content lines are skipped by the memchr stream entirely.
            let fence_end = loop {
                let Some(r) = memchr::memchr(self.byte, &src[search..]) else {
                    break len;
                };
                let q = search + r;
                if q > 0 && src[q - 1] != self.eol {
                    search = q + 1;
                    continue;
                }
                let mut cc = 0u8;
                let mut j = q;
                while j < len && src[j] == self.byte {
                    cc = cc.saturating_add(1);
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
        let mut iter = FenceIter::new(src, b'`', 3, b'\n', b' ', b'\t');

        assert_eq!(iter.next(), Some(Span::new(0, 15)));
        assert_eq!(iter.next(), None);
    }

    // 02. Successfully matches an opening fence line that contains an info string
    #[test]
    fn test_02_opening_fence_with_info_string() {
        let src = b"```rust\nlet x = 1;\n```";
        let mut iter = FenceIter::new(src, b'`', 3, b'\n', b' ', b'\t');

        assert_eq!(iter.next(), Some(Span::new(0, 22)));
        assert_eq!(iter.next(), None);
    }

    // 03. Accepts a closing fence line that contains allowed trailing spaces and tabs
    #[test]
    fn test_03_closing_fence_with_allowed_whitespace() {
        let src = b"```\ntext\n``` \t ";
        let mut iter = FenceIter::new(src, b'`', 3, b'\n', b' ', b'\t');

        assert_eq!(iter.next(), Some(Span::new(0, 15)));
        assert_eq!(iter.next(), None);
    }

    // 04. Matches a closing fence that has more consecutive bytes than the opening fence
    #[test]
    fn test_04_closing_fence_longer_than_opening() {
        let src = b"```\ncontent\n````";
        let mut iter = FenceIter::new(src, b'`', 3, b'\n', b' ', b'\t');

        assert_eq!(iter.next(), Some(Span::new(0, 16)));
        assert_eq!(iter.next(), None);
    }

    // 05. Rejects a closing fence if it has fewer consecutive bytes than the opening fence
    #[test]
    fn test_05_closing_fence_shorter_than_opening() {
        let src = b"````\ncontent\n```";
        let mut iter = FenceIter::new(src, b'`', 3, b'\n', b' ', b'\t');

        assert_eq!(iter.next(), Some(Span::new(0, 16)));
        assert_eq!(iter.next(), None);
    }

    // 06. Rejects a closing fence if it contains unallowed trailing text characters
    #[test]
    fn test_06_closing_fence_with_trailing_text() {
        let src = b"```\ncontent\n``` error";
        let mut iter = FenceIter::new(src, b'`', 3, b'\n', b' ', b'\t');

        assert_eq!(iter.next(), Some(Span::new(0, 21)));
        assert_eq!(iter.next(), None);
    }

    // 07. Ignores a potential opening fence line if its length is below the min threshold
    #[test]
    fn test_07_opening_fence_below_minimum_length() {
        let src = b"``\ncontent\n``";
        let mut iter = FenceIter::new(src, b'`', 3, b'\n', b' ', b'\t');

        assert_eq!(iter.next(), None);
    }

    // 08. Consumes everything until EOF when an opening fence lacks a valid closing fence
    #[test]
    fn test_08_unclosed_fence_consumes_to_eof() {
        let src = b"```\ncontent without close";
        let mut iter = FenceIter::new(src, b'`', 3, b'\n', b' ', b'\t');

        assert_eq!(iter.next(), Some(Span::new(0, 25)));
        assert_eq!(iter.next(), None);
    }

    // 09. Sequentially extracts multiple distinct closed fence blocks from the same source
    #[test]
    fn test_09_multiple_independent_fence_blocks() {
        let src = b"```\nfirst\n```\n```\nsecond\n```";
        let mut iter = FenceIter::new(src, b'`', 3, b'\n', b' ', b'\t');

        assert_eq!(iter.next(), Some(Span::new(0, 14)));
        assert_eq!(iter.next(), Some(Span::new(14, 28)));
        assert_eq!(iter.next(), None);
    }

    // 10. Immediately returns None when the input byte slice is completely empty
    #[test]
    fn test_10_empty_source_slice() {
        let src = b"";
        let mut iter = FenceIter::new(src, b'`', 3, b'\n', b' ', b'\t');

        assert_eq!(iter.next(), None);
    }

    // 11. Skips regular text lines and empty lines before finding a valid opening fence
    #[test]
    fn test_11_skips_leading_non_fence_lines() {
        let src = b"regular text\n\n```\ncontent\n```";
        let mut iter = FenceIter::new(src, b'`', 3, b'\n', b' ', b'\t');

        assert_eq!(iter.next(), Some(Span::new(14, 29)));
        assert_eq!(iter.next(), None);
    }

    // 12. Rejects an opening fence line if it is indented or starts with a space character
    #[test]
    fn test_12_indented_opening_fence_rejected() {
        let src = b" ```\ncontent\n```";
        let mut iter = FenceIter::new(src, b'`', 3, b'\n', b' ', b'\t');

        assert_eq!(iter.next(), Some(Span::new(13, 16)));
        assert_eq!(iter.next(), None);
    }

    // 13. Rejects an opening fence line if the specified marker byte appears again inside the info string
    #[test]
    fn test_13_opening_fence_with_marker_in_info_string() {
        let src = b"```rust`info\ncontent\n```";
        let mut iter = FenceIter::new(src, b'`', 3, b'\n', b' ', b'\t');

        assert_eq!(iter.next(), Some(Span::new(21, 24)));
        assert_eq!(iter.next(), None);
    }

    // 14. Matches a fence block that satisfies the minimum length threshold exactly on the edge
    #[test]
    fn test_14_exact_minimum_length_fence() {
        let src = b"~~~\ncontent\n~~~";
        let mut iter = FenceIter::new(src, b'~', 3, b'\n', b' ', b'\t');

        assert_eq!(iter.next(), Some(Span::new(0, 15)));
        assert_eq!(iter.next(), None);
    }

    // 15. Verifies fence extraction behavior when there is no trailing newline character at EOF
    #[test]
    fn test_15_no_trailing_newline_at_eof() {
        let src = b"```\ncontent\n```";
        let mut iter = FenceIter::new(src, b'`', 3, b'\n', b' ', b'\t');

        assert_eq!(iter.next(), Some(Span::new(0, 15)));
        assert_eq!(iter.next(), None);
    }
}
