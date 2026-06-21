// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2025 Nikita Shavrin

#![cfg_attr(any(feature = "avx2", feature = "avx512"), feature(portable_simd))]
#![warn(missing_docs)]
#![forbid(unsafe_code)]

//! A declarative flat parsing engine for text formats.
//!
//! # Overview
//!
//! `meon` lets you describe a text grammar in a single [`define_parser!`] macro
//! invocation and get back:
//!
//! - a **content struct** with `Vec` fields for every element kind,
//! - a **`...Parser` type** with a `parse(source: &[u8]) -> ...Content<'_>` method
//!   for full single-pass parsing,
//! - **`find_*` standalone iterators** for lazily scanning one element kind
//!   without parsing the whole document.
//!
//! All span endpoints are `u32` byte offsets into the original source slice.
//! Input must not exceed [`span::MAX_INPUT_LEN`] (4 GiB).
//!
//! # Quick start
//!
//! ```rust,ignore
//! use meon::define_parser;
//!
//! define_parser!(Plain {
//!     sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
//!     inline {
//!         fallback => texts [10];
//!     }
//!     blocks {
//!         fallback => paragraphs [80];
//!     }
//! });
//!
//! let content = PlainParser::parse(b"hello world\n");
//! assert_eq!(content.paragraphs.len(), 1);
//! ```
//!
//! # Feature flags
//!
//! | Feature  | Effect                                            |
//! |----------|---------------------------------------------------|
//! | `avx2`   | 32-byte SIMD search (requires nightly + AVX2 CPU) |
//! | `avx512` | 64-byte SIMD search (implies `avx2`)              |
//!
//! Without either flag the crate compiles on stable Rust.
//!
//! # Crate structure
//!
//! - [`span`] — the [`span::Span`] type and [`span::MAX_INPUT_LEN`] constant.
//! - [`define_parser!`] — the grammar macro (re-exported from `meon-macros`).
//! - `find_*` iterators — re-exported from the standalone iterator module;
//!   normally obtained via the generated `Parser::find_*` methods.

#[doc(hidden)]
pub use memchr;
#[doc(hidden)]
pub use paste;

#[doc(hidden)]
pub mod engine;
pub mod span;
#[doc(hidden)]
pub mod swar;

pub use meon_macros::define_parser;

pub use engine::text_parser::standalone::{
    asymmetric::AsymmetricExactIter, block_marker::BlockMarkerIter,
    block_numbered::BlockNumberedIter, chained::ChainedIter, cont::ContIter, fence::FenceIter,
    key_value::KvIter, line_marker::LineMarkerIter, line_uniform::LineUniformIter,
    symmetric::SymmetricExactIter,
};
