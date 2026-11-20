//! Parse errors returned by the checked parser methods.

use std::fmt;

/// The source is longer than the grammar's `max_input_bytes` limit.
///
/// The checked methods (`parse`, `find_*`, `find_context_*`, `context`)
/// return this error before allocating any parser state. The `_unchecked`
/// variants skip the check and keep the previous behaviour.
///
/// `T` is the grammar's offset type: `u8`, `u16`, `u32` or `u64`. It defaults
/// to `u32`, matching the default [`Span`](crate::span::Span).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputBytesTooLarge<T = u32> {
    /// The source length, in bytes.
    pub len: usize,
    /// The grammar's input limit, in bytes.
    pub max: T,
}

impl<T: fmt::Display> fmt::Display for InputBytesTooLarge<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "input is {} bytes, exceeding the grammar limit of {} bytes",
            self.len, self.max
        )
    }
}

impl<T: fmt::Debug + fmt::Display> std::error::Error for InputBytesTooLarge<T> {}
