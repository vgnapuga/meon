//! The optional `limits` grammar section.
//!
//! `limits` carries the offset type, the grammar-wide nesting cap, the three
//! optional span length caps and the optional input length cap. `0` and the
//! offset type's maximum mean "no cap" for a span cap, and "the maximum" for
//! `max_input_bytes`. `_` or an absent `max_input_bytes` means no input check.

use proc_macro2::{Literal, Span as PSpan, TokenStream as TS2, TokenTree as TT};
use quote::quote;

use crate::cursor::Cursor;
use crate::error::{Error, Result};

/// The offset type selected by `limits { offset = ...; }`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum OffsetTy {
    U8,
    U16,
    #[default]
    U32,
    U64,
}

impl OffsetTy {
    /// The largest offset this type can store, as a `u64`.
    pub(crate) fn max(self) -> u64 {
        match self {
            Self::U8 => u8::MAX as u64,
            Self::U16 => u16::MAX as u64,
            Self::U32 => u32::MAX as u64,
            Self::U64 => u64::MAX,
        }
    }

    /// The Rust type token.
    pub(crate) fn tokens(self) -> TS2 {
        match self {
            Self::U8 => quote! { u8 },
            Self::U16 => quote! { u16 },
            Self::U32 => quote! { u32 },
            Self::U64 => quote! { u64 },
        }
    }
}

/// The narrowest unsigned type that holds a limit value.
#[derive(Debug, Clone, Copy)]
pub(crate) enum LimitTy {
    U8,
    U16,
    U32,
    U64,
}

/// One limit value. `value` is stored as `u64`; the emitted literal is
/// narrowed to `u8`/`u16`/`u32`/`u64`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Limit {
    pub(crate) value: u64,
    ty: LimitTy,
}

impl Limit {
    fn new(value: u64) -> Self {
        let ty = if value <= u8::MAX as u64 {
            LimitTy::U8
        } else if value <= u16::MAX as u64 {
            LimitTy::U16
        } else if value <= u32::MAX as u64 {
            LimitTy::U32
        } else {
            LimitTy::U64
        };
        Self { value, ty }
    }

    pub(crate) fn literal(&self) -> Literal {
        match self.ty {
            LimitTy::U8 => Literal::u8_unsuffixed(self.value as u8),
            LimitTy::U16 => Literal::u16_unsuffixed(self.value as u16),
            LimitTy::U32 => Literal::u32_unsuffixed(self.value as u32),
            LimitTy::U64 => Literal::u64_unsuffixed(self.value),
        }
    }

    /// The optional runtime-macro argument: `[N]`.
    pub(crate) fn tokens(&self) -> TS2 {
        let lit = self.literal();
        quote! { [ #lit ] }
    }
}

/// Parsed `limits { ... }` contents.
#[derive(Debug, Default)]
pub(crate) struct Limits {
    pub(crate) offset: OffsetTy,
    pub(crate) max_nest: Option<Literal>,
    pub(crate) max_inline_span: Option<Limit>,
    pub(crate) max_line_span: Option<Limit>,
    pub(crate) max_block_span: Option<Limit>,
    pub(crate) max_input_bytes: Option<Limit>,
}

impl Limits {
    /// Parse the body of a `limits { ... }` group.
    pub(crate) fn parse(ts: TS2) -> Result<Self> {
        let mut c = Cursor::new(ts);
        let mut offset = OffsetTy::default();
        let mut raw_nest: Option<Literal> = None;
        let mut raw_inline: Option<(u64, PSpan)> = None;
        let mut raw_line: Option<(u64, PSpan)> = None;
        let mut raw_block: Option<(u64, PSpan)> = None;
        let mut raw_input: Option<(u64, PSpan)> = None;
        let mut seen_offset = false;
        let mut seen_nest = false;
        let mut seen_inline = false;
        let mut seen_line = false;
        let mut seen_block = false;
        let mut seen_input = false;

        while let Some(tt) = c.peek().cloned() {
            match tt {
                TT::Ident(id) => {
                    let name = id.to_string();
                    match name.as_str() {
                        "offset" => {
                            if seen_offset {
                                return Err(Error::new(id.span(), "`offset` is set twice"));
                            }
                            seen_offset = true;
                            offset = parse_offset(&mut c)?;
                        }
                        "max_nest" => {
                            if seen_nest {
                                return Err(Error::new(id.span(), "`max_nest` is set twice"));
                            }
                            let lit = c.named_lit("max_nest")?;
                            if literal_u64(&lit).is_none() {
                                return Err(Error::new(
                                    lit.span(),
                                    "expected a non-negative integer after `max_nest =`",
                                ));
                            }
                            seen_nest = true;
                            raw_nest = Some(lit);
                        }
                        "max_inline_span" => {
                            if seen_inline {
                                return Err(Error::new(
                                    id.span(),
                                    "`max_inline_span` is set twice",
                                ));
                            }
                            seen_inline = true;
                            raw_inline = parse_limit_raw(&mut c, "max_inline_span")?;
                        }
                        "max_line_span" => {
                            if seen_line {
                                return Err(Error::new(id.span(), "`max_line_span` is set twice"));
                            }
                            seen_line = true;
                            raw_line = parse_limit_raw(&mut c, "max_line_span")?;
                        }
                        "max_block_span" => {
                            if seen_block {
                                return Err(Error::new(id.span(), "`max_block_span` is set twice"));
                            }
                            seen_block = true;
                            raw_block = parse_limit_raw(&mut c, "max_block_span")?;
                        }
                        "max_input_bytes" => {
                            if seen_input {
                                return Err(Error::new(
                                    id.span(),
                                    "`max_input_bytes` is set twice",
                                ));
                            }
                            seen_input = true;
                            raw_input = parse_input_raw(&mut c, "max_input_bytes")?;
                        }
                        _ => {
                            return Err(Error::new(
                                id.span(),
                                format!(
                                    "unknown keyword `{name}` in `limits`; expected \
                                     `offset`, `max_nest`, `max_inline_span`, \
                                     `max_line_span`, `max_block_span` or `max_input_bytes`"
                                ),
                            ));
                        }
                    }
                    c.skip(';');
                    c.skip(',');
                }
                TT::Punct(p) if p.as_char() == ';' || p.as_char() == ',' => c.advance(),
                other => {
                    return Err(Error::new(
                        other.span(),
                        format!("expected a `limits` setting, got {other:?}"),
                    ));
                }
            }
        }

        let max = offset.max();
        let cap = |raw: Option<(u64, PSpan)>, name: &str| -> Result<Option<Limit>> {
            match raw {
                None => Ok(None),
                Some((value, span)) => {
                    if value > max {
                        return Err(Error::new(
                            span,
                            format!("`{name}` must not exceed `{}`", max),
                        ));
                    }
                    if value == 0 || value == max {
                        Ok(None)
                    } else {
                        Ok(Some(Limit::new(value)))
                    }
                }
            }
        };

        let max_input_bytes = match (seen_input, raw_input) {
            (false, _) | (true, None) => None,
            (true, Some((value, span))) => {
                if value > max {
                    return Err(Error::new(
                        span,
                        format!("`max_input_bytes` must not exceed `{}`", max),
                    ));
                }
                if value == 0 || value == max {
                    Some(Limit::new(max))
                } else {
                    Some(Limit::new(value))
                }
            }
        };

        Ok(Self {
            offset,
            max_nest: raw_nest,
            max_inline_span: cap(raw_inline, "max_inline_span")?,
            max_line_span: cap(raw_line, "max_line_span")?,
            max_block_span: cap(raw_block, "max_block_span")?,
            max_input_bytes,
        })
    }

    pub(crate) fn inline_tt(&self) -> TS2 {
        self.max_inline_span
            .as_ref()
            .map_or_else(|| quote! { [] }, Limit::tokens)
    }

    pub(crate) fn line_tt(&self) -> TS2 {
        self.max_line_span
            .as_ref()
            .map_or_else(|| quote! { [] }, Limit::tokens)
    }

    pub(crate) fn block_tt(&self) -> TS2 {
        self.max_block_span
            .as_ref()
            .map_or_else(|| quote! { [] }, Limit::tokens)
    }

    pub(crate) fn offset_tt(&self) -> TS2 {
        self.offset.tokens()
    }
}

/// Parse `offset = u8 | u16 | u32 | u64`.
fn parse_offset(c: &mut Cursor) -> Result<OffsetTy> {
    let span = c.span();
    match c.next_tt() {
        Some(TT::Ident(i)) if i == "offset" => {}
        other => {
            return Err(Error::new(
                span,
                format!("expected `offset`, got {other:?}"),
            ));
        }
    }
    c.skip_eq_alone();
    let span = c.span();
    let id = c.next_ident("offset type")?;
    match id.to_string().as_str() {
        "u8" => Ok(OffsetTy::U8),
        "u16" => Ok(OffsetTy::U16),
        "u32" => Ok(OffsetTy::U32),
        "u64" => Ok(OffsetTy::U64),
        other => Err(Error::new(
            span,
            format!("expected `u8`, `u16`, `u32` or `u64` after `offset =`, got `{other}`"),
        )),
    }
}

/// Parse `name = <literal>` and return the raw value and its span.
fn parse_limit_raw(c: &mut Cursor, name: &str) -> Result<Option<(u64, PSpan)>> {
    let lit = c.named_lit(name)?;
    let value = literal_u64(&lit).ok_or_else(|| {
        Error::new(
            lit.span(),
            format!("expected a non-negative integer after `{name} =`"),
        )
    })?;
    Ok(Some((value, lit.span())))
}

/// Parse `max_input_bytes = <literal | _>`.
///
/// `_` returns `None`; a literal returns its raw value.
fn parse_input_raw(c: &mut Cursor, name: &str) -> Result<Option<(u64, PSpan)>> {
    let span = c.span();
    match c.next_tt() {
        Some(TT::Ident(i)) if i == name => {}
        other => {
            return Err(Error::new(
                span,
                format!("expected `{name}`, got {other:?}"),
            ));
        }
    }
    c.skip_eq_alone();
    let span = c.span();
    match c.next_tt() {
        Some(TT::Ident(i)) if i == "_" => Ok(None),
        Some(TT::Punct(p)) if p.as_char() == '_' => Ok(None),
        Some(TT::Literal(lit)) => {
            let value = literal_u64(&lit).ok_or_else(|| {
                Error::new(
                    lit.span(),
                    format!("expected a non-negative integer or `_` after `{name} =`"),
                )
            })?;
            Ok(Some((value, lit.span())))
        }
        other => Err(Error::new(
            span,
            format!("expected a non-negative integer or `_` after `{name} =`, got {other:?}"),
        )),
    }
}

/// Read a decimal integer literal, allowing `_` separators and an integer
/// suffix (`100u16`, `1_024usize`).
pub(crate) fn literal_u64(lit: &Literal) -> Option<u64> {
    let s = lit.to_string();
    let digits_len = s
        .chars()
        .take_while(|ch| ch.is_ascii_digit() || *ch == '_')
        .map(char::len_utf8)
        .sum::<usize>();
    if digits_len == 0 {
        return None;
    }
    let digits = s[..digits_len].replace('_', "");
    if digits.is_empty() {
        return None;
    }
    let rest = &s[digits_len..];
    if !rest.is_empty() && !rest.chars().all(|ch| ch.is_ascii_alphanumeric()) {
        return None;
    }
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    fn parse(ts: TS2) -> Limits {
        Limits::parse(ts).unwrap()
    }

    #[test]
    fn defaults_are_absent() {
        let l = parse(quote! {});
        assert_eq!(l.offset, OffsetTy::U32);
        assert!(l.max_nest.is_none());
        assert!(l.max_inline_span.is_none());
        assert!(l.max_line_span.is_none());
        assert!(l.max_block_span.is_none());
        assert!(l.max_input_bytes.is_none());
    }

    #[test]
    fn values_and_narrow_types() {
        let l = parse(quote! {
            max_nest = 4;
            max_inline_span = 100;
            max_line_span = 70_000;
            max_block_span = 4_294_967_295u32;
        });
        assert!(l.max_nest.is_some());
        assert_eq!(l.max_inline_span.unwrap().value, 100);
        assert_eq!(l.max_line_span.unwrap().value, 70_000);
        assert!(l.max_block_span.is_none());
    }

    #[test]
    fn zero_and_u32_max_are_absent() {
        let l = parse(quote! {
            max_inline_span = 0;
            max_line_span = 4_294_967_295;
            max_block_span = 0;
        });
        assert!(l.max_inline_span.is_none());
        assert!(l.max_line_span.is_none());
        assert!(l.max_block_span.is_none());
    }

    #[test]
    fn offset_selects_the_type() {
        let l = parse(quote! {
            offset = u8;
            max_inline_span = 100;
            max_line_span = 200;
            max_input_bytes = 0;
        });
        assert_eq!(l.offset, OffsetTy::U8);
        assert_eq!(l.max_inline_span.unwrap().value, 100);
        assert_eq!(l.max_line_span.unwrap().value, 200);
        assert_eq!(l.max_input_bytes.unwrap().value, 255);
    }

    #[test]
    fn offset_rejects_values_above_its_max() {
        let e = Limits::parse(quote! {
            offset = u8;
            max_inline_span = 300;
        })
        .unwrap_err()
        .to_compile_error()
        .to_string();
        assert!(e.contains("must not exceed"), "{e}");
    }

    #[test]
    fn offset_unknown_type_is_an_error() {
        let e = Limits::parse(quote! { offset = u128 })
            .unwrap_err()
            .to_compile_error()
            .to_string();
        assert!(e.contains("expected `u8`"), "{e}");
    }

    #[test]
    fn offset_duplicate_is_an_error() {
        let e = Limits::parse(quote! { offset = u8; offset = u16 })
            .unwrap_err()
            .to_compile_error()
            .to_string();
        assert!(e.contains("set twice"), "{e}");
    }

    #[test]
    fn input_limit_values() {
        let l = parse(quote! { max_input_bytes = 1_024u16 });
        assert_eq!(l.max_input_bytes.unwrap().value, 1_024);
    }

    #[test]
    fn input_limit_zero_and_max_mean_the_maximum() {
        let zero = parse(quote! { max_input_bytes = 0 });
        assert_eq!(zero.max_input_bytes.unwrap().value, u32::MAX as u64);
        let max = parse(quote! { max_input_bytes = 4_294_967_295 });
        assert_eq!(max.max_input_bytes.unwrap().value, u32::MAX as u64);
    }

    #[test]
    fn input_limit_underscore_means_no_check() {
        let l = parse(quote! { max_input_bytes = _ });
        assert!(l.max_input_bytes.is_none());
    }

    #[test]
    fn input_limit_over_u32_is_an_error() {
        let e = Limits::parse(quote! { max_input_bytes = 4_294_967_296 })
            .unwrap_err()
            .to_compile_error()
            .to_string();
        assert!(e.contains("must not exceed"), "{e}");
    }

    #[test]
    fn input_limit_duplicate_is_an_error() {
        let e = Limits::parse(quote! { max_input_bytes = 1; max_input_bytes = _ })
            .unwrap_err()
            .to_compile_error()
            .to_string();
        assert!(e.contains("set twice"), "{e}");
    }

    #[test]
    fn over_u32_is_an_error() {
        let e = Limits::parse(quote! { max_inline_span = 4_294_967_296 })
            .unwrap_err()
            .to_compile_error()
            .to_string();
        assert!(e.contains("must not exceed"), "{e}");
    }

    #[test]
    fn duplicate_key_is_an_error() {
        let e = Limits::parse(quote! { max_inline_span = 1; max_inline_span = 2 })
            .unwrap_err()
            .to_compile_error()
            .to_string();
        assert!(e.contains("set twice"), "{e}");
    }

    #[test]
    fn unknown_key_is_an_error() {
        let e = Limits::parse(quote! { max_spans = 1 })
            .unwrap_err()
            .to_compile_error()
            .to_string();
        assert!(e.contains("unknown keyword"), "{e}");
    }

    #[test]
    fn suffix_and_separators_are_read() {
        let l = parse(quote! { max_inline_span = 1_024u16 });
        assert_eq!(l.max_inline_span.unwrap().value, 1_024);
    }
}
