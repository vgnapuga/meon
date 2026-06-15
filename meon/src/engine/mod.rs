//! Engine internals: content struct generator and runtime parsing macros.
//!
//! This module is `#[doc(hidden)]` — it is an implementation detail of the
//! `meon` crate and is not part of the stable public API. Grammar authors
//! interact with [`define_parser!`] only; the items here are used exclusively
//! by macro expansions.
//!
//! ## Structure
//!
//! - [`content`] — the `define_content!` macro that generates the paired
//!   `<Name>State` accumulator and `<Name>` content struct for a grammar.
//! - [`text_parser`] — the runtime parsing macros (`parse_text!`,
//!   `parse_inline!`, `parse_line!`, `parse_block!`) and the standalone
//!   iterator types backing the generated `find_*` methods.

pub mod content;
pub use crate::define_content;

pub mod text_parser;
pub use crate::parse_text;
