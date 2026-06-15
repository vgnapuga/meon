//! Element types produced by the grammar.
//!
//! Each type is the metadata half of a `Vec<(Type, Span)>` field in the
//! generated content struct. The `Span` half carries the byte range; the
//! type carries any per-element metadata (level, delimiter kind, number…).

pub mod block;
pub mod inline;
pub mod line;
