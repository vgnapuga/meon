//! The [`Span`] type and [`MAX_INPUT_LEN`] constant.

/// The maximum byte length of a source slice accepted by the parser.
///
/// Spans are stored as `u32` offsets, so any input larger than this value
/// would silently truncate span endpoints. The parser does **not** panic on
/// oversized input at runtime — this constant exists so callers can check
/// before passing in a buffer, and so the fuzz harness can assert it explicitly.
///
/// In practice the limit is 4 GiB, which exceeds any realistic document size.
pub const MAX_INPUT_LEN: usize = u32::MAX as usize;

/// A half-open byte range `[start, end)` into a source slice.
///
/// Both endpoints are byte offsets, **not** character indices. All span values
/// produced by the parser satisfy `start <= end <= source.len()`.
///
/// Obtain the corresponding bytes via the content struct's `bytes(span)` helper,
/// or UTF-8 text via `str(span)` (returns `None` on invalid UTF-8).
///
/// A zero-length span (`start == end`) is used for marker-only positions such
/// as hard-break anchors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// Byte offset of the first byte included in the span.
    pub start: u32,
    /// Byte offset one past the last byte included in the span.
    pub end: u32,
}

impl Span {
    /// Construct a span from explicit `start` and `end` byte offsets.
    ///
    /// # Panics
    ///
    /// Does not panic — it is the caller's responsibility to ensure
    /// `start <= end <= source.len()`.
    pub fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }
}
