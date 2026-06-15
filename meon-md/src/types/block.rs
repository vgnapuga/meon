//! Block element types: [`BulletItem`] and [`OrderedItem`].

/// A bullet list item.
#[derive(Debug, Clone, Copy)]
pub struct BulletItem {
    /// The ASCII byte of the marker character (`b'-'`, `b'*'`, or `b'+'`).
    pub kind: u8,
}

/// An ordered list item.
#[derive(Debug, Clone, Copy)]
pub struct OrderedItem {
    /// The ASCII byte of the delimiter character (`b'.'` or `b')'`).
    pub kind: u8,
    /// The parsed item number.
    pub num: u32,
}
