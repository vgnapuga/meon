//! Line element types: [`Heading`] and [`ThematicBreak`].

/// A heading element carrying its nesting level.
#[derive(Debug, Clone, Copy)]
pub struct Heading {
    /// Heading level, 1–6. Never zero.
    pub level: std::num::NonZeroU8,
}

/// A thematic break (horizontal rule).
#[derive(Debug, Clone, Copy)]
pub struct ThematicBreak {
    /// The ASCII byte of the delimiter character (`b'-'`, `b'*'`, or `b'_'`).
    pub kind: u8,
}
