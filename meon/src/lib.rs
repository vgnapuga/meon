// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Nikita Shavrin

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
//! - a **`...Parser` type** with a `parse(source: &[u8]) ->
//!   Result<...Content<'_>, InputBytesTooLarge>` method for full single-pass
//!   parsing, plus `parse_unchecked` for the same parse without the grammar's
//!   input length check,
//! - **`find_*` standalone iterators** for lazily scanning one element kind
//!   without parsing the whole document,
//! - a **`context(source)` builder** producing the [`ParseContext`] opaque-region
//!   map, plus **`find_context_*` iterators** that skip candidates inside
//!   fenced blocks and `parse_inside = false` constructs and read lines the
//!   way the full parse does.
//!
//! All span endpoints are byte offsets into the original source slice.
//! Their type defaults to `u32` and can be selected per grammar with
//! `limits { offset = u8 | u16 | u32 | u64; }`.
//! A grammar can set `max_input_bytes` in its `limits` section to reject a
//! source longer than the limit with [`InputBytesTooLarge`]; `0` and
//! `u32::MAX` mean the maximum (`u32::MAX`), while an absent key or `_` means
//! no check. Without a check, input longer than [`span::MAX_INPUT_LEN`]
//! (4 GiB) silently truncates span endpoints.
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
//! let content = PlainParser::parse(b"hello world\n").unwrap();
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
//! - [`error`] — [`InputBytesTooLarge`], returned by the checked methods.
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
pub mod error;
pub mod span;
#[doc(hidden)]
pub mod swar;

pub use error::InputBytesTooLarge;
pub use meon_macros::define_parser;

pub use engine::text_parser::standalone::{
    asymmetric::{AsymmetricBalancedIter, AsymmetricExactIter, AsymmetricStackIter},
    block_marker::BlockMarkerIter,
    block_numbered::BlockNumberedIter,
    chained::ChainedIter,
    common::LineBreaks,
    cont::ContIter,
    fence::FenceIter,
    key_value::KvIter,
    line_marker::LineMarkerIter,
    line_uniform::LineUniformIter,
    symmetric::SymmetricExactIter,
};

pub use engine::text_parser::standalone::{
    context::{
        BlockRule, BlockSimpleRule, ByteSet, ChainSpec, ContextCursor, LineModel, LineRule,
        ParseContext, RuleIndex,
    },
    context_iter::{ContextAsymmetricStackIter, ContextSymmetricExactIter},
};
