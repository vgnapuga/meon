//! Shared utilities for standalone iterators.
//!
//! Contains three primitive operations used by every iterator in this module:
//!
//! - [`find_line_end`] — locate the end of the current line.
//! - [`advance_line`] — move to the start of the next line.
//! - [`count_escape`] — count consecutive escape bytes preceding a position,
//!   used to determine whether a delimiter is escaped (odd count) or not (even).
//!
//! All functions operate on raw byte slices and are marked `#[inline(always)]`.

pub use crate::span::Span;

/// Return the byte offset of the next `eol` byte at or after `from`, or
/// `src.len()` if none is found. This is the exclusive end of the current line.
#[inline(always)]
pub fn find_line_end(src: &[u8], from: usize, eol: u8) -> usize {
    memchr::memchr(eol, &src[from..])
        .map(|i| from + i)
        .unwrap_or(src.len())
}

/// Advance past `line_end` and return `(next_line_start, next_line_end)`.
///
/// Returns `None` if `line_end` is at or past the end of `src`, or if the byte
/// immediately after `line_end` is also past the end (i.e. no next line exists).
#[inline(always)]
pub fn advance_line(src: &[u8], line_end: usize, eol: u8) -> Option<(usize, usize)> {
    let len = src.len();
    if line_end >= len {
        return None;
    }
    let next = line_end + 1;
    if next >= len {
        return None;
    }
    Some((next, find_line_end(src, next, eol)))
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

    // --- advance_line tests ---

    // 06. Advances to the next line successfully and computes its correct end index
    #[test]
    fn test_06_advance_line_standard() {
        let src = b"abc\ndef";
        assert_eq!(advance_line(src, 3, b'\n'), Some((4, 7)));
    }

    // 07. Returns None when the current line end is already at the absolute slice capacity
    #[test]
    fn test_07_advance_line_at_eof_boundary() {
        let src = b"abc";
        assert_eq!(advance_line(src, 3, b'\n'), None);
    }

    // 08. Returns None if advancing by one byte lands exactly on or past the slice boundary
    #[test]
    fn test_08_advance_line_trailing_newline_eof() {
        let src = b"abc\n";
        assert_eq!(advance_line(src, 3, b'\n'), None);
    }

    // 09. Safely steps into an empty line block and returns matching start and end bounds
    #[test]
    fn test_09_advance_line_into_empty_line() {
        let src = b"abc\n\ndef";
        assert_eq!(advance_line(src, 3, b'\n'), Some((4, 4)));
    }

    // 10. Computes the next line parameters up to the end of the file if no delimiter exists
    #[test]
    fn test_10_advance_line_next_has_no_eol() {
        let src = b"abc\ndef";
        assert_eq!(advance_line(src, 3, b'\n'), Some((4, 7)));
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
}
