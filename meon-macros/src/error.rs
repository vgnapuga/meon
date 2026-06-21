//! Minimal located-error type for the grammar front-end.
//!
//! On malformed grammar the front-end returns an [`Error`] carrying the span of
//! the offending token. [`Error::to_compile_error`] turns it into a
//! `compile_error!` invocation spanned at that token, so the user gets a located
//! diagnostic instead of a proc-macro panic.

use proc_macro2::{Span, TokenStream as TS2};
use quote::quote_spanned;

/// A grammar error located at a specific token span.
#[derive(Debug)]
pub(crate) struct Error {
    span: Span,
    msg: String,
}

/// Front-end result type.
pub(crate) type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Create an error at `span` with message `msg`.
    pub(crate) fn new(span: Span, msg: impl Into<String>) -> Self {
        Self {
            span,
            msg: msg.into(),
        }
    }

    /// Render as a `compile_error!{ "..." }` invocation spanned at the offending
    /// token, suitable as the whole expansion of the proc-macro.
    pub(crate) fn to_compile_error(&self) -> TS2 {
        let msg = &self.msg;
        quote_spanned! { self.span => ::core::compile_error!{ #msg } }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ce(msg: &str) -> String {
        Error::new(Span::call_site(), msg)
            .to_compile_error()
            .to_string()
    }

    // 01. to_compile_error emits a compile_error! invocation
    #[test]
    fn test_01_emits_compile_error() {
        assert!(ce("boom").contains("compile_error"));
    }

    // 02. The original message text is embedded in the output
    #[test]
    fn test_02_embeds_message() {
        assert!(ce("expected literal (fence min)").contains("expected literal"));
    }

    // 03. Error::new accepts a &str message
    #[test]
    fn test_03_accepts_str() {
        let _e = Error::new(Span::call_site(), "msg");
    }

    // 04. Error::new accepts an owned String message
    #[test]
    fn test_04_accepts_string() {
        let _e = Error::new(Span::call_site(), String::from("msg"));
    }

    // 05. Distinct messages render to distinct output
    #[test]
    fn test_05_distinct_messages() {
        assert_ne!(ce("alpha"), ce("beta"));
    }

    // 06. The Result alias resolves Ok values
    #[test]
    fn test_06_result_ok() {
        let r: Result<u32> = Ok(7);
        assert!(r.is_ok());
    }

    // 07. The Result alias carries Err values
    #[test]
    fn test_07_result_err() {
        let r: Result<u32> = Err(Error::new(Span::call_site(), "nope"));
        assert!(r.is_err());
    }

    // 08. An empty message still produces a well-formed compile_error
    #[test]
    fn test_08_empty_message() {
        assert!(ce("").contains("compile_error"));
    }

    // 09. The message is embedded as a quoted string literal
    #[test]
    fn test_09_message_is_string_literal() {
        assert!(ce("hi").contains("\"hi\""));
    }

    // 10. The rendered error is a non-empty token stream
    #[test]
    fn test_10_non_empty_stream() {
        assert!(
            !Error::new(Span::call_site(), "x")
                .to_compile_error()
                .is_empty()
        );
    }
}
