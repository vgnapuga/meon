//! Shared utilities for standalone iterators.
//!
//! Contains the primitive operations used by every iterator in this module:
//!
//! - [`find_line_end`] — locate the end of the current line.
//! - [`next_in_paragraph`] — the paragraph-bounded close search shared by
//!   the inline iterators that search for a close and by the context
//!   builder, and
//!   [`scan_paragraph`], the same search telling where a failed one stopped,
//!   with [`scan_paragraph_by`] for a search of several bytes.
//! - [`count_escape`] — count consecutive escape bytes preceding a position,
//!   used to determine whether a delimiter is escaped (odd count) or not (even).
//! - [`probe_matcher`] / [`find_any_of`] — turn an arbitrary byte predicate
//!   into a `memchr`-family streaming search.
//!
//! All functions operate on raw byte slices.

pub use crate::span::Span;

use super::context::ByteSet;

/// How lines end, for the scans that read lines and paragraphs: the `eol`
/// byte; with `crlf`, a `\r` right before it, which is part of the line end
/// and of no line content; and the bytes a blank line may hold besides — the
/// grammar's `sep` and `tab` — so that a line of them alone ends a paragraph
/// as an empty line does.
///
/// A bare `eol` byte converts into one with no `crlf` and no blank bytes:
/// only an empty line is blank then.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineBreaks {
    /// The line terminator byte.
    pub eol: u8,
    /// Is a `\r` right before `eol` part of the line end?
    pub crlf: bool,
    /// The bytes a blank line may hold.
    pub blank: ByteSet,
}

impl LineBreaks {
    /// Line ends at `eol`, `\r\n`-style when `crlf`, with a line of the
    /// bytes in `blank` alone counted blank.
    pub const fn new(eol: u8, crlf: bool, blank: &[u8]) -> Self {
        Self {
            eol,
            crlf,
            blank: ByteSet::from_bytes(blank),
        }
    }

    /// Where the content of the line `[ls, le)` ends: before a `\r` right
    /// before an actual `eol` at `le` when `crlf` is set; a CR at EOF is content.
    #[inline(always)]
    pub fn content_end(&self, src: &[u8], ls: usize, le: usize) -> usize {
        if self.crlf && le < src.len() && src[le] == self.eol && le > ls && src[le - 1] == b'\r' {
            le - 1
        } else {
            le
        }
    }

    /// Is the line starting at `ls` blank: empty, or of `blank` bytes alone,
    /// up to its line end or the end of input?
    #[inline(always)]
    pub fn blank_at(&self, src: &[u8], ls: usize) -> bool {
        let len = src.len();
        let mut i = ls;
        while i < len && self.blank.contains(src[i]) {
            i += 1;
        }
        i >= len
            || src[i] == self.eol
            || (self.crlf && src[i] == b'\r' && i + 1 < len && src[i + 1] == self.eol)
    }

    /// Does a paragraph end at the `eol` at `q`: with the end of input, or a
    /// blank line, right after it?
    #[inline(always)]
    pub fn ends_paragraph(&self, src: &[u8], q: usize) -> bool {
        q + 1 >= src.len() || self.blank_at(src, q + 1)
    }
}

impl From<u8> for LineBreaks {
    fn from(eol: u8) -> Self {
        Self::new(eol, false, &[])
    }
}

/// Return the byte offset of the next `eol` byte at or after `from`, or
/// `src.len()` if none is found. This is the exclusive end of the current line.
#[inline(always)]
pub fn find_line_end(src: &[u8], from: usize, eol: u8) -> usize {
    memchr::memchr(eol, &src[from..])
        .map(|i| from + i)
        .unwrap_or(src.len())
}

/// The next `needle` at or after `from` within the current paragraph, or
/// `None` when the paragraph ends first.
///
/// A single `eol` is crossed as ordinary content; a blank line (see
/// [`LineBreaks::blank_at`]) or the end of input ends the paragraph. The
/// returned position is a candidate only: the caller validates it (escape,
/// run length, context coverage) and resumes from a position of its choice.
///
/// `enter_line` is called with the start of every line the search enters.
/// `Some(next)` continues the search from `next` — `next == ls` for plain
/// continuation, or a later position to skip an opaque region leading the
/// line — and `None` ends the paragraph, e.g. at a line that opens a fence.
/// Pass `Some` when line starts need no special treatment.
#[inline(always)]
pub fn next_in_paragraph(
    src: &[u8],
    from: usize,
    needle: u8,
    breaks: impl Into<LineBreaks>,
    enter_line: impl FnMut(usize) -> Option<usize>,
) -> Option<usize> {
    scan_paragraph(src, from, needle, breaks, enter_line).ok()
}

/// [`next_in_paragraph`], telling on failure where the paragraph ends:
/// `Err(stop)` when no `needle` lies in `[from, stop)` and `stop` ends the
/// paragraph — the `eol` before a blank line, the `eol` before a line
/// `enter_line` refuses, or the end of input. Bytes `enter_line` skips are
/// not searched.
///
/// A close search that fails can keep `[from, stop)`: an opener of the same
/// rule in it lies in the same paragraph, and its own search would fail
/// too. That turns a run of unclosed openers from one search each into one
/// search in all.
#[inline(always)]
pub fn scan_paragraph(
    src: &[u8],
    from: usize,
    needle: u8,
    breaks: impl Into<LineBreaks>,
    enter_line: impl FnMut(usize) -> Option<usize>,
) -> Result<usize, usize> {
    let breaks: LineBreaks = breaks.into();
    let eol = breaks.eol;
    scan_paragraph_by(
        src,
        from,
        breaks,
        |h| memchr::memchr2(needle, eol, h),
        enter_line,
    )
}

/// [`scan_paragraph`] for the bytes `find` stops at: `find` returns the
/// offset of the first `eol` or needle byte in a haystack, and the search
/// returns the first needle byte it meets.
#[inline(always)]
pub fn scan_paragraph_by(
    src: &[u8],
    from: usize,
    breaks: impl Into<LineBreaks>,
    find: impl Fn(&[u8]) -> Option<usize>,
    mut enter_line: impl FnMut(usize) -> Option<usize>,
) -> Result<usize, usize> {
    let breaks: LineBreaks = breaks.into();
    let len = src.len();
    let mut j = from;
    loop {
        let Some(r) = find(&src[j..]) else {
            return Err(len);
        };
        let q = j + r;
        if src[q] != breaks.eol {
            return Ok(q);
        }
        if breaks.ends_paragraph(src, q) {
            return Err(q);
        }
        j = enter_line(q + 1).ok_or(q)?;
    }
}

/// Probe an arbitrary byte predicate over all 256 values and collect the
/// accepted bytes into `buf`.
///
/// Returns `Some(n)` when the predicate accepts at most three distinct bytes —
/// few enough to drive a `memchr`-family streaming search — or `None` when it
/// accepts more, in which case the caller falls back to line-by-line scanning.
/// Runs once per iterator construction; the predicate closures generated by
/// the grammar are pure byte tests, so the probe is exact.
#[inline]
pub fn probe_matcher(matches: impl Fn(u8) -> bool, buf: &mut [u8; 3]) -> Option<usize> {
    let mut n = 0usize;
    for b in 0..=255u8 {
        if matches(b) {
            if n == 3 {
                return None;
            }
            buf[n] = b;
            n += 1;
        }
    }
    Some(n)
}

/// `memchr` over `n` needle bytes (`n <= 3`), dispatched by count.
#[inline(always)]
pub fn find_any_of(needles: &[u8; 3], n: usize, hay: &[u8]) -> Option<usize> {
    match n {
        0 => None,
        1 => memchr::memchr(needles[0], hay),
        2 => memchr::memchr2(needles[0], needles[1], hay),
        _ => memchr::memchr3(needles[0], needles[1], needles[2], hay),
    }
}

/// Count the number of consecutive `escape` bytes immediately preceding `pos`.
///
/// Used to determine whether a delimiter at `pos` is escaped: an odd count
/// means the delimiter is suppressed; an even count (including zero) means it
/// is active.
#[inline(always)]
pub fn count_escape(src: &[u8], pos: usize, escape: u8) -> u32 {
    let mut n = 0u32;
    let mut k = pos;
    while k > 0 && src[k - 1] == escape {
        n += 1;
        k -= 1;
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- find_line_end tests ---

    // 01. Finds the exact index of the newline character when it exists in the slice
    #[test]
    fn test_01_find_line_end_normal() {
        let src = b"abc\ndef";
        assert_eq!(find_line_end(src, 0, b'\n'), 3);
    }

    // 02. Returns the total slice length when the target end-of-line byte is missing
    #[test]
    fn test_02_find_line_end_missing_eol() {
        let src = b"abcdef";
        assert_eq!(find_line_end(src, 0, b'\n'), 6);
    }

    // 03. Correctly scans from a specific middle offset rather than the slice start
    #[test]
    fn test_03_find_line_end_from_offset() {
        let src = b"abc\ndef\n";
        assert_eq!(find_line_end(src, 4, b'\n'), 7);
    }

    // 04. Handles an empty source slice safely by returning zero immediately
    #[test]
    fn test_04_find_line_end_empty_slice() {
        let src = b"";
        assert_eq!(find_line_end(src, 0, b'\n'), 0);
    }

    // 05. Identifies the line end at index zero if the first byte matches the delimiter
    #[test]
    fn test_05_find_line_end_immediate_match() {
        let src = b"\nabc";
        assert_eq!(find_line_end(src, 0, b'\n'), 0);
    }

    // --- next_in_paragraph tests ---

    // 16. The needle on the same line is found at its position
    #[test]
    fn test_16_next_in_paragraph_same_line() {
        assert_eq!(next_in_paragraph(b"ab*c", 0, b'*', b'\n', Some), Some(2));
    }

    // 17. A single eol is crossed and the needle on the next line is found
    #[test]
    fn test_17_next_in_paragraph_crosses_single_eol() {
        assert_eq!(next_in_paragraph(b"ab\nc*", 0, b'*', b'\n', Some), Some(4));
    }

    // 18. An empty line ends the paragraph before the needle
    #[test]
    fn test_18_next_in_paragraph_empty_line_ends() {
        assert_eq!(next_in_paragraph(b"ab\n\n*", 0, b'*', b'\n', Some), None);
    }

    // 19. End of input right after an eol, or with no needle, ends the search
    #[test]
    fn test_19_next_in_paragraph_eol_at_end() {
        assert_eq!(next_in_paragraph(b"ab\n", 0, b'*', b'\n', Some), None);
        assert_eq!(next_in_paragraph(b"ab", 0, b'*', b'\n', Some), None);
        assert_eq!(next_in_paragraph(b"", 0, b'*', b'\n', Some), None);
    }

    // 20. A needle at `from` itself is returned
    #[test]
    fn test_20_next_in_paragraph_at_from() {
        assert_eq!(next_in_paragraph(b"*a", 0, b'*', b'\n', Some), Some(0));
        assert_eq!(next_in_paragraph(b"a*", 1, b'*', b'\n', Some), Some(1));
    }

    // 21. `enter_line` returning None ends the paragraph at that line
    #[test]
    fn test_21_next_in_paragraph_enter_line_ends() {
        let src = b"a\n```\n*";
        let r = next_in_paragraph(src, 0, b'*', b'\n', |ls| {
            if src[ls] == b'`' { None } else { Some(ls) }
        });
        assert_eq!(r, None);
    }

    // 22. `enter_line` may skip forward past an opaque region at the line start
    #[test]
    fn test_22_next_in_paragraph_enter_line_skips() {
        let src = b"a\n`*`*";
        let r = next_in_paragraph(src, 0, b'*', b'\n', |ls| {
            if src[ls] == b'`' {
                Some(ls + 3)
            } else {
                Some(ls)
            }
        });
        assert_eq!(r, Some(5));
    }

    // --- probe_matcher / find_any_of tests ---

    // 06. Collects the accepted bytes of a small predicate in ascending order
    #[test]
    fn test_06_probe_matcher_small_set() {
        let mut buf = [0u8; 3];
        let n = probe_matcher(|b| b == b'-' || b == b'*' || b == b'+', &mut buf);
        assert_eq!(n, Some(3));
        assert_eq!(buf, [b'*', b'+', b'-']);
    }

    // 07. Returns None for a predicate accepting more than three bytes
    #[test]
    fn test_07_probe_matcher_large_set() {
        let mut buf = [0u8; 3];
        assert_eq!(probe_matcher(|b| b.is_ascii_digit(), &mut buf), None);
    }

    // 08. An always-false predicate probes to an empty needle set
    #[test]
    fn test_08_probe_matcher_empty_set() {
        let mut buf = [0u8; 3];
        assert_eq!(probe_matcher(|_| false, &mut buf), Some(0));
    }

    // 09. find_any_of dispatches correctly for one, two, and three needles
    #[test]
    fn test_09_find_any_of_dispatch() {
        let hay = b"xxabc";
        assert_eq!(find_any_of(&[b'c', 0, 0], 1, hay), Some(4));
        assert_eq!(find_any_of(&[b'b', b'c', 0], 2, hay), Some(3));
        assert_eq!(find_any_of(b"abc", 3, hay), Some(2));
    }

    // 10. An empty needle set never matches
    #[test]
    fn test_10_find_any_of_empty() {
        assert_eq!(find_any_of(&[0, 0, 0], 0, b"anything"), None);
    }

    // --- count_escape tests ---

    // 11. Returns zero when there are no escape characters preceding the position index
    #[test]
    fn test_11_count_escape_none() {
        let src = b"abc";
        assert_eq!(count_escape(src, 1, b'\\'), 0);
    }

    // 12. Counts a single isolated escape character located right before the target position
    #[test]
    fn test_12_count_escape_single() {
        let src = b"a\\b";
        assert_eq!(count_escape(src, 2, b'\\'), 1);
    }

    // 13. Accumulates multiple sequential escape markers preceding the target index cleanly
    #[test]
    fn test_13_count_escape_multiple() {
        let src = b"a\\\\\\b";
        assert_eq!(count_escape(src, 4, b'\\'), 3);
    }

    // 14. Evaluates to zero immediately when the position parameter points to index zero
    #[test]
    fn test_14_count_escape_at_start_index() {
        let src = b"\\abc";
        assert_eq!(count_escape(src, 0, b'\\'), 0);
    }

    // 15. Discontinues counting when a non-escape character breaks the backwards sequence chain
    #[test]
    fn test_15_count_escape_stopped_by_normal_char() {
        let src = b"x\\\\y";
        assert_eq!(count_escape(src, 3, b'\\'), 2);
    }

    // 16. A failed scan tells where the paragraph ends
    #[test]
    fn test_16_scan_paragraph_stop() {
        assert_eq!(scan_paragraph(b"ab>c", 0, b'>', b'\n', Some), Ok(2));
        assert_eq!(scan_paragraph(b"ab\nc\n\nd>", 0, b'>', b'\n', Some), Err(4));
        assert_eq!(scan_paragraph(b"ab\ncd", 0, b'>', b'\n', Some), Err(5));
        assert_eq!(scan_paragraph(b"ab\n", 0, b'>', b'\n', Some), Err(2));
        assert_eq!(scan_paragraph(b"ab\ncd>", 0, b'>', b'\n', |_| None), Err(2));
    }

    // 17. CR belongs to the line end only when an actual eol follows it.
    #[test]
    fn test_17_content_end_requires_eol() {
        for eol in *b"\n|" {
            let breaks = LineBreaks::new(eol, true, b" \t");
            assert_eq!(breaks.content_end(b"x\r", 0, 2), 2);
            let src = [b'x', b'\r', eol];
            assert_eq!(breaks.content_end(&src, 0, 2), 1);
            assert_eq!(breaks.content_end(b"x\ry", 0, 2), 2);
            assert_eq!(breaks.content_end(b"", 0, 0), 0);
        }
    }

    // 18. A bare CR at EOF makes a line nonblank, with or without padding.
    #[test]
    fn test_18_blank_line_requires_eol_after_cr() {
        let breaks = LineBreaks::new(b'\n', true, b" \t");
        for src in [&b"\r"[..], b" \t\r", b"\rx"] {
            assert!(!breaks.blank_at(src, 0));
        }
        for src in [&b""[..], b" \t", b"\r\n", b" \t\r\n"] {
            assert!(breaks.blank_at(src, 0));
        }
        assert!(!breaks.ends_paragraph(b"x\n\r", 1));
        assert!(breaks.ends_paragraph(b"x\n\r\n", 1));
    }
}
