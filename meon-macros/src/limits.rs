//! The optional `limits` grammar section.
//!
//! `limits` carries the grammar-wide nesting cap and the three optional span
//! length caps. An absent cap (or `0`, or `u32::MAX`) means "no cap": the
//! runtime macros are handed an empty `[]` token and generate no check for it.

use proc_macro2::{Literal, TokenStream as TS2, TokenTree as TT};
use quote::quote;

use crate::cursor::Cursor;
use crate::error::{Error, Result};

/// The narrowest unsigned type that holds a limit value.
#[derive(Clone, Copy)]
pub(crate) enum LimitTy {
    U8,
    U16,
    U32,
}

/// One span-length cap. `value` is never `0` or `u32::MAX`; those normalize to
/// "no cap" in [`Limits::parse`].
#[derive(Clone, Copy)]
pub(crate) struct Limit {
    pub(crate) value: u32,
    ty: LimitTy,
}

impl Limit {
    fn new(value: u32) -> Self {
        let ty = if value <= u8::MAX as u32 {
            LimitTy::U8
        } else if value <= u16::MAX as u32 {
            LimitTy::U16
        } else {
            LimitTy::U32
        };
        Self { value, ty }
    }

    fn literal(&self) -> Literal {
        match self.ty {
            LimitTy::U8 => Literal::u8_unsuffixed(self.value as u8),
            LimitTy::U16 => Literal::u16_unsuffixed(self.value as u16),
            LimitTy::U32 => Literal::u32_unsuffixed(self.value),
        }
    }

    /// The optional runtime-macro argument: `[N]`.
    pub(crate) fn tokens(&self) -> TS2 {
        let lit = self.literal();
        quote! { [ #lit ] }
    }
}

/// Parsed `limits { ... }` contents.
#[derive(Default)]
pub(crate) struct Limits {
    pub(crate) max_nest: Option<Literal>,
    pub(crate) max_inline_span: Option<Limit>,
    pub(crate) max_line_span: Option<Limit>,
    pub(crate) max_block_span: Option<Limit>,
}

impl Limits {
    /// Parse the body of a `limits { ... }` group.
    pub(crate) fn parse(ts: TS2) -> Result<Self> {
        let mut c = Cursor::new(ts);
        let mut out = Self::default();
        let mut seen_nest = false;
        let mut seen_inline = false;
        let mut seen_line = false;
        let mut seen_block = false;

        while let Some(tt) = c.peek().cloned() {
            match tt {
                TT::Ident(id) => {
                    let name = id.to_string();
                    match name.as_str() {
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
                            out.max_nest = Some(lit);
                        }
                        "max_inline_span" => {
                            if seen_inline {
                                return Err(Error::new(
                                    id.span(),
                                    "`max_inline_span` is set twice",
                                ));
                            }
                            seen_inline = true;
                            out.max_inline_span = parse_limit(&mut c, "max_inline_span")?;
                        }
                        "max_line_span" => {
                            if seen_line {
                                return Err(Error::new(id.span(), "`max_line_span` is set twice"));
                            }
                            seen_line = true;
                            out.max_line_span = parse_limit(&mut c, "max_line_span")?;
                        }
                        "max_block_span" => {
                            if seen_block {
                                return Err(Error::new(id.span(), "`max_block_span` is set twice"));
                            }
                            seen_block = true;
                            out.max_block_span = parse_limit(&mut c, "max_block_span")?;
                        }
                        _ => {
                            return Err(Error::new(
                                id.span(),
                                format!(
                                    "unknown keyword `{name}` in `limits`; expected \
                                     `max_nest`, `max_inline_span`, `max_line_span` or \
                                     `max_block_span`"
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

        Ok(out)
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
}

fn parse_limit(c: &mut Cursor, name: &str) -> Result<Option<Limit>> {
    let lit = c.named_lit(name)?;
    let value = literal_u64(&lit).ok_or_else(|| {
        Error::new(
            lit.span(),
            format!("expected a non-negative integer after `{name} =`"),
        )
    })?;
    if value > u32::MAX as u64 {
        return Err(Error::new(
            lit.span(),
            format!("`{name}` must not exceed `u32::MAX`"),
        ));
    }
    if value == 0 || value == u32::MAX as u64 {
        return Ok(None);
    }
    Ok(Some(Limit::new(value as u32)))
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
