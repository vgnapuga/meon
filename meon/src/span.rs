//! The [`Span`] type, the [`SpanIndex`] trait and [`MAX_INPUT_LEN`].
//!
//! [`MAX_INPUT_LEN`]: MAX_INPUT_LEN

/// The maximum byte length of a source slice accepted by the default
/// [`Span<u32>`](Span) representation.
///
/// Spans are stored as `u32` offsets by default, so any input larger than
/// this value would silently truncate span endpoints. A grammar can select a
/// different offset type with `limits { offset = u8 | u16 | u32 | u64; }`.
/// Its checked methods reject an input that does not fit only when the
/// grammar sets `max_input_bytes`; otherwise they read the prefix that fits.
///
/// In practice the default limit is 4 GiB, which exceeds any realistic
/// document size.
pub const MAX_INPUT_LEN: usize = u32::MAX as usize;

/// An unsigned integer type that can store a byte offset in a [`Span`].
///
/// Implemented for `u8`, `u16`, `u32` and `u64`. A grammar selects one of
/// these with `limits { offset = ...; }`; the generated `Span<O>` fields then
/// use it.
pub trait SpanIndex: Copy + Ord + core::fmt::Debug {
    /// The largest offset this type can store, as a `usize`.
    const MAX: usize;

    /// Convert the offset to a `usize`.
    fn to_usize(self) -> usize;

    /// Convert a `usize` to this offset type.
    ///
    /// The caller is responsible for `n <= Self::MAX`.
    fn from_usize(n: usize) -> Self;
}

macro_rules! impl_span_index {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl SpanIndex for $ty {
                const MAX: usize = <$ty>::MAX as usize;

                #[inline(always)]
                fn to_usize(self) -> usize {
                    self as usize
                }

                #[inline(always)]
                fn from_usize(n: usize) -> Self {
                    n as $ty
                }
            }
        )+
    };
}

impl_span_index!(u8, u16, u32, u64);

impl SpanIndex for usize {
    const MAX: usize = usize::MAX;

    #[inline(always)]
    fn to_usize(self) -> usize {
        self
    }

    #[inline(always)]
    fn from_usize(n: usize) -> Self {
        n
    }
}

/// A half-open byte range `[start, end)` into a source slice.
///
/// `T` is the offset type. It defaults to `u32`, so existing code that writes
/// [`Span`] keeps the 4 GiB representation. A grammar can select `u8`, `u16`,
/// `u32` or `u64` with `limits { offset = ...; }`.
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
pub struct Span<T = u32> {
    /// Byte offset of the first byte included in the span.
    pub start: T,
    /// Byte offset one past the last byte included in the span.
    pub end: T,
}

impl Span<u32> {
    /// Construct a `u32` span from explicit `start` and `end` byte offsets.
    ///
    /// The caller is responsible for `start <= end <= source.len()`.
    pub const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }
}

impl<T: SpanIndex> Span<T> {
    /// The largest input length this span type can address.
    pub const MAX_INPUT_LEN: usize = T::MAX;

    /// Construct a span from explicit offsets of the selected type.
    pub const fn from_parts(start: T, end: T) -> Self {
        Self { start, end }
    }

    /// Construct a span from `usize` offsets.
    ///
    /// The caller is responsible for `start <= T::MAX` and `end <= T::MAX`.
    #[inline]
    pub fn from_usize(start: usize, end: usize) -> Self {
        Self {
            start: T::from_usize(start),
            end: T::from_usize(end),
        }
    }

    /// The start offset as a `usize`.
    #[inline(always)]
    pub fn start_usize(self) -> usize {
        self.start.to_usize()
    }

    /// The end offset as a `usize`.
    #[inline(always)]
    pub fn end_usize(self) -> usize {
        self.end.to_usize()
    }

    /// The span length as a `usize`.
    #[inline(always)]
    pub fn len_usize(self) -> usize {
        self.end_usize() - self.start_usize()
    }
}
