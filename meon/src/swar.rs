//! Fast multi-byte search primitives used by the parser engine.
//!
//! # Design
//!
//! [`find_any`] is the single public entry point. It searches `src` for the
//! first occurrence of any byte in `targets` and returns its index, or `None`
//! if no byte is found.
//!
//! Internally the implementation is split by the number of target bytes:
//!
//! - **N = 0**: no target byte can ever match — returns `None` immediately
//!   without touching `src`. This case did not arise from any call site
//!   before the `parse_text!` dispatcher stopped folding `eol` into the
//!   inline trigger set (see `text_parser::mod` for why): a grammar with no
//!   `on_trigger` blocks at all now reaches `find_any` with an empty target
//!   array. Guarding it here, rather than at every call site, is what keeps
//!   this function safe to call with any const array size.
//! - **N = 1 – 3**: delegates to the [`memchr`] crate (`memchr`, `memchr2`,
//!   `memchr3`). These routines are hand-tuned with platform SIMD and are the
//!   fastest available option for small sets.
//! - **N ≥ 4**: uses a SWAR (SIMD Within A Register) fallback that processes
//!   eight bytes per iteration inside a single `u64` word, with optional
//!   AVX2 / AVX512 acceleration when the corresponding Cargo features are
//!   enabled.
//!
//! The split avoids the overhead of loading a target array for the common
//! 1–3 byte case while still handling the wider sets that the inline
//! dispatcher needs (e.g. `on_trigger(b'*', b'`', b'[', b'<')`).
//!
//! # Feature flags
//!
//! | Feature   | Effect                                                   |
//! |-----------|----------------------------------------------------------|
//! | `avx2`    | Enables 32-byte SIMD lanes (requires nightly + AVX2 CPU) |
//! | `avx512`  | Enables 64-byte SIMD lanes (implies `avx2`)              |
//!
//! Without either flag the crate compiles on stable Rust and uses the pure
//! SWAR path for N ≥ 4.
//!
//! # Invariants
//!
//! - The function never panics on any input, including `N == 0`.
//! - The returned index, if `Some(i)`, satisfies `src[i] ∈ targets`.
//! - Time complexity is O(n) in the length of `src`.

#[cfg(any(feature = "avx2", feature = "avx512"))]
use std::simd::prelude::*;

/// Multiplier used to broadcast a byte into every byte-lane of a `u64`.
#[cfg(not(any(feature = "avx2", feature = "avx512")))]
const ONES: u64 = 0x0101_0101_0101_0101;

/// Mask that isolates the high bit of every byte-lane in a `u64`.
#[cfg(not(any(feature = "avx2", feature = "avx512")))]
const HIGHS: u64 = 0x8080_8080_8080_8080;

/// Broadcast byte `b` into every byte-lane of a `u64`.
#[cfg(not(any(feature = "avx2", feature = "avx512")))]
#[inline(always)]
fn broadcast(b: u8) -> u64 {
    b as u64 * ONES
}

/// Return a mask with the high bit set in each byte-lane of `chunk` that
/// equals `bcast` (a broadcast of the target byte).
///
/// Uses the classic SWAR identity:
/// `has = (chunk ^ bcast).wrapping_sub(ONES) & !(chunk ^ bcast) & HIGHS`
#[cfg(not(any(feature = "avx2", feature = "avx512")))]
#[inline(always)]
fn has_byte(chunk: u64, bcast: u64) -> u64 {
    let x = chunk ^ bcast;
    x.wrapping_sub(ONES) & !x & HIGHS
}

/// SIMD search loop for `$lanes`-wide vectors.
#[cfg(any(feature = "avx2", feature = "avx512"))]
macro_rules! search_simd {
    ($lanes:literal, $src:ident, $i:ident, $targets:ident) => {
        while $i + $lanes <= $src.len() {
            let chunk = Simd::<u8, $lanes>::from_slice(&$src[$i..$i + $lanes]);
            let mut mask = chunk.simd_eq(Simd::splat($targets[0]));
            for &t in &$targets[1..] {
                mask |= chunk.simd_eq(Simd::splat(t));
            }
            if mask.any() {
                let bits = mask.to_bitmask();
                return Some($i + bits.trailing_zeros() as usize);
            }
            $i += $lanes;
        }
    };
}

/// Search `src` for the first byte that appears in `targets`.
///
/// Returns `Some(index)` of the earliest matching byte, or `None` if no
/// target byte is present in `src` (including the trivial case `N == 0`,
/// where no byte can ever match).
///
/// # Dispatch strategy
///
/// | `N` | Backend                                                  |
/// |-----|----------------------------------------------------------|
/// | 0   | Always `None`, no scan                                  |
/// | 1   | `memchr::memchr`                                         |
/// | 2   | `memchr::memchr2`                                        |
/// | 3   | `memchr::memchr3`                                        |
/// | >= 4| SWAR (`u64`) with optional AVX2/AVX512 (see module docs) |
///
/// The compiler constant-folds the `match N` at monomorphisation time, so
/// only one branch is ever emitted per instantiation — zero overhead.
#[inline(always)]
pub fn find_any<const N: usize>(targets: [u8; N], src: &[u8]) -> Option<usize> {
    match N {
        0 => None,
        1 => memchr::memchr(targets[0], src),
        2 => memchr::memchr2(targets[0], targets[1], src),
        3 => memchr::memchr3(targets[0], targets[1], targets[2], src),
        _ => find_any_wide(targets, src),
    }
}

/// SWAR / SIMD search for `N ≥ 4` target bytes.
///
/// Separated from [`find_any`] so the compiler can inline the memchr
/// fast-paths and only emit the wider loop when actually needed.
#[inline(always)]
fn find_any_wide<const N: usize>(targets: [u8; N], src: &[u8]) -> Option<usize> {
    let len = src.len();
    let mut i = 0;

    #[cfg(feature = "avx512")]
    {
        search_simd!(64, src, i, targets);
        search_simd!(32, src, i, targets);
        search_simd!(8, src, i, targets);
    }

    #[cfg(all(feature = "avx2", not(feature = "avx512")))]
    {
        search_simd!(32, src, i, targets);
        search_simd!(8, src, i, targets);
    }

    #[cfg(not(any(feature = "avx2", feature = "avx512")))]
    {
        let bcasts = targets.map(broadcast);
        while i + 8 <= len {
            let chunk = u64::from_ne_bytes(src[i..i + 8].try_into().unwrap());
            let mut mask = 0u64;
            for &bcast in &bcasts {
                mask |= has_byte(chunk, bcast);
            }
            if mask != 0 {
                #[cfg(target_endian = "little")]
                return Some(i + (mask.trailing_zeros() / 8) as usize);
                #[cfg(target_endian = "big")]
                return Some(i + (mask.leading_zeros() / 8) as usize);
            }
            i += 8;
        }
    }

    // Scalar tail for remaining < 8 bytes.
    while i < len {
        let b = src[i];
        for &t in &targets {
            if b == t {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // 01. Every dispatch arm: 0 needles never match, 1-3 delegate to memchr,
    //     4+ take the wide path
    #[test]
    fn test_01_dispatch_arms() {
        let hay = b"the quick brown fox jumps over the lazy dog";
        assert_eq!(find_any([], hay), None);
        assert_eq!(find_any(*b"q", hay), Some(4));
        assert_eq!(find_any(*b"qz", hay), Some(4));
        assert_eq!(find_any(*b"zqj", hay), Some(4));
        assert_eq!(find_any(*b"zXYq", hay), Some(4));
        assert_eq!(find_any(*b"zXYWq", hay), Some(4));
    }

    // 02. Wide path: matches at the very start, inside the 8-byte tail, and
    //     across word boundaries of a long haystack
    #[test]
    fn test_02_wide_boundaries() {
        let mut hay = vec![b'.'; 100];
        hay[0] = b'a';
        assert_eq!(find_any(*b"abcd", &hay), Some(0));
        let mut hay = vec![b'.'; 100];
        hay[99] = b'd';
        assert_eq!(find_any(*b"abcd", &hay), Some(99));
        let mut hay = vec![b'.'; 13];
        hay[9] = b'b'; // inside the < 8-byte tail after one full word
        assert_eq!(find_any(*b"abcd", &hay), Some(9));
    }

    // 03. Wide path: no match over a long haystack, and empty input
    #[test]
    fn test_03_wide_no_match() {
        let hay = vec![b'.'; 100];
        assert_eq!(find_any(*b"abcd", &hay), None);
        assert_eq!(find_any(*b"abcd", b""), None);
    }

    // 04. Wide path: the earliest of several present needles wins
    #[test]
    fn test_04_wide_earliest_wins() {
        let hay = b"....d..a..";
        assert_eq!(find_any(*b"abcd", hay), Some(4));
    }
}
