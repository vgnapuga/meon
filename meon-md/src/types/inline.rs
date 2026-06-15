//! Inline element types: [`Link`] and [`KeyValue`].

use meon::span::Span;

/// A link or image element with separate text and url spans.
///
/// `is_image` is `true` when the element was preceded by `!`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Link {
    /// Whether this is an image (`!` prefix was present).
    pub is_image: bool,
    /// Span of the link text (between `[` and `]`).
    pub text: Span,
    /// Span of the link url (between `(` and `)`).
    pub url: Span,
}

/// A `key = value` pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyValue {
    /// Span of the key portion.
    pub key: Span,
    /// Span of the value portion.
    pub value: Span,
}
