//! Hand-rolled cursor over a `TokenStream`.
//!
//! A thin reader with peek/advance and a handful of grammar-aware helpers
//! (`next_group`, `arrow_field_cap`, `collect_lit_alternation`, …) shared by the
//! front-end.
//!
//! The structural methods (`next_ident`, `next_group`, `named_lit`,
//! `arrow_field_cap`, `expect_lit`) return [`Result`]: on malformed input they
//! yield an [`Error`] carrying the span of the offending token rather than
//! panicking. The cursor tracks the span of the last consumed token so that
//! errors hit at end-of-stream still point somewhere useful.

use proc_macro2::{
    Delimiter, Group, Ident, Literal, Spacing, Span, TokenStream as TS2, TokenTree as TT,
};

use crate::error::{Error, Result};

/// A position into a flat vector of token trees.
pub(crate) struct Cursor {
    tokens: Vec<TT>,
    pub(crate) pos: usize,
    last_span: Span,
}

impl Cursor {
    /// Create a cursor over `ts`, positioned at the first token.
    pub(crate) fn new(ts: TS2) -> Self {
        Self {
            tokens: ts.into_iter().collect(),
            pos: 0,
            last_span: Span::call_site(),
        }
    }

    /// Span of the current token, or the last consumed token's span at EOF.
    pub(crate) fn span(&self) -> Span {
        self.peek().map(|t| t.span()).unwrap_or(self.last_span)
    }

    /// Borrow the current token without consuming it.
    pub(crate) fn peek(&self) -> Option<&TT> {
        self.tokens.get(self.pos)
    }

    /// Return the current token's text if it is an identifier.
    pub(crate) fn peek_str(&self) -> Option<String> {
        match self.peek() {
            Some(TT::Ident(i)) => Some(i.to_string()),
            _ => None,
        }
    }

    /// Advance past the current token, saturating at the end.
    pub(crate) fn advance(&mut self) {
        if let Some(tt) = self.tokens.get(self.pos) {
            self.last_span = tt.span();
            self.pos += 1;
        }
    }

    /// Consume and return the current token, if any.
    pub(crate) fn next_tt(&mut self) -> Option<TT> {
        let t = self.tokens.get(self.pos).cloned();
        if let Some(ref tt) = t {
            self.last_span = tt.span();
            self.pos += 1;
        }
        t
    }

    /// Consume an identifier, or error with `ctx` at the offending span.
    pub(crate) fn next_ident(&mut self, ctx: &str) -> Result<Ident> {
        let span = self.span();
        match self.next_tt() {
            Some(TT::Ident(i)) => Ok(i),
            other => Err(Error::new(
                span,
                format!("expected ident ({ctx}), got {other:?}"),
            )),
        }
    }

    /// Consume a delimited group of the given kind, or error with `ctx`.
    pub(crate) fn next_group(&mut self, delim: Delimiter, ctx: &str) -> Result<Group> {
        let span = self.span();
        match self.next_tt() {
            Some(TT::Group(g)) if g.delimiter() == delim => Ok(g),
            other => Err(Error::new(
                span,
                format!("expected group {delim:?} ({ctx}), got {other:?}"),
            )),
        }
    }

    /// Consume the current token if it is a literal (no error on miss).
    pub(crate) fn next_lit(&mut self) -> Option<Literal> {
        if let Some(TT::Literal(_)) = self.peek() {
            if let Some(TT::Literal(l)) = self.next_tt() {
                return Some(l);
            }
        }
        None
    }

    /// Consume a literal, or error with `ctx` at the offending span.
    pub(crate) fn expect_lit(&mut self, ctx: &str) -> Result<Literal> {
        let span = self.span();
        self.next_lit()
            .ok_or_else(|| Error::new(span, format!("expected literal ({ctx})")))
    }

    /// Consume a `lit | lit | …` alternation of literals.
    pub(crate) fn collect_lit_alternation(&mut self) -> Vec<Literal> {
        let mut lits = Vec::new();
        while let Some(TT::Literal(l)) = self.peek().cloned() {
            self.advance();
            lits.push(l);
            if let Some(TT::Punct(p)) = self.peek() {
                if p.as_char() == '|' {
                    self.advance();
                    continue;
                }
            }
            break;
        }
        lits
    }

    /// Consume a `|a, b, …|` capture list, returning the bound identifiers.
    pub(crate) fn skip_pipe_vars_returning(&mut self) -> Vec<Ident> {
        let mut vars = Vec::new();
        if let Some(TT::Punct(p)) = self.peek() {
            if p.as_char() == '|' {
                self.advance();
                loop {
                    match self.peek().cloned() {
                        Some(TT::Punct(p)) if p.as_char() == '|' => {
                            self.advance();
                            break;
                        }
                        Some(TT::Ident(id)) => {
                            self.advance();
                            vars.push(id);
                        }
                        None => break,
                        _ => {
                            self.advance();
                        }
                    }
                }
            }
        }
        vars
    }

    /// Consume `=> field [N]`, returning the field identifier and capacity `N`.
    pub(crate) fn arrow_field_cap(&mut self, ctx: &str) -> Result<(Ident, Literal)> {
        if !self.is_fat_arrow() {
            return Err(Error::new(self.span(), format!("expected `=>` ({ctx})")));
        }
        self.pos += 2;
        let f = self.next_ident(&format!("field name ({ctx})"))?;
        let span = self.span();
        let cap = match self.next_tt() {
            Some(TT::Group(g)) if g.delimiter() == Delimiter::Bracket => {
                let mut inner = g.stream().into_iter();
                match inner.next() {
                    Some(TT::Literal(l)) => l,
                    other => {
                        return Err(Error::new(
                            span,
                            format!("expected literal in [N] ({ctx}), got {other:?}"),
                        ));
                    }
                }
            }
            other => {
                return Err(Error::new(
                    span,
                    format!("expected [N] ({ctx}), got {other:?}"),
                ));
            }
        };
        Ok((f, cap))
    }

    /// Consume `name = <literal>` and return the literal.
    pub(crate) fn named_lit(&mut self, name: &str) -> Result<Literal> {
        let span = self.span();
        match self.next_tt() {
            Some(TT::Ident(i)) if i == name => {}
            other => {
                return Err(Error::new(
                    span,
                    format!("expected `{name}`, got {other:?}"),
                ));
            }
        }
        self.skip_eq_alone();
        let span = self.span();
        match self.next_tt() {
            Some(TT::Literal(l)) => Ok(l),
            other => Err(Error::new(
                span,
                format!("expected literal after `{name} =`, got {other:?}"),
            )),
        }
    }

    /// True if the next two tokens form a joint `=>`.
    pub(crate) fn is_fat_arrow(&self) -> bool {
        match (self.tokens.get(self.pos), self.tokens.get(self.pos + 1)) {
            (Some(TT::Punct(a)), Some(TT::Punct(b))) => {
                a.as_char() == '=' && a.spacing() == Spacing::Joint && b.as_char() == '>'
            }
            _ => false,
        }
    }

    /// Skip a standalone `=` (not the `=` of `=>`).
    pub(crate) fn skip_eq_alone(&mut self) {
        if let Some(TT::Punct(p)) = self.peek() {
            if p.as_char() == '=' && p.spacing() == Spacing::Alone {
                self.advance();
            }
        }
    }

    /// Skip a single `:`.
    pub(crate) fn skip_colon(&mut self) {
        if let Some(TT::Punct(p)) = self.peek() {
            if p.as_char() == ':' {
                self.advance();
            }
        }
    }

    /// Skip a single punctuation token `ch` if present.
    pub(crate) fn skip(&mut self, ch: char) {
        if let Some(TT::Punct(p)) = self.peek() {
            if p.as_char() == ch {
                self.advance();
            }
        }
    }

    /// Collect tokens up to (but not consuming) the next brace group.
    pub(crate) fn collect_until_brace(&mut self) -> TS2 {
        let mut out = TS2::new();
        while let Some(tt) = self.peek() {
            if let TT::Group(g) = tt {
                if g.delimiter() == Delimiter::Brace {
                    break;
                }
            }
            out.extend([self.next_tt().unwrap()]);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(s: &str) -> TS2 {
        s.parse().unwrap()
    }

    fn err_msg(e: Error) -> String {
        e.to_compile_error().to_string()
    }

    // 01. A fresh cursor peeks the first token without consuming it
    #[test]
    fn test_01_peek_does_not_consume() {
        let c = Cursor::new(ts("alpha beta"));
        assert_eq!(c.peek_str().as_deref(), Some("alpha"));
        assert_eq!(c.peek_str().as_deref(), Some("alpha"));
        assert_eq!(c.pos, 0);
    }

    // 02. advance moves past exactly one token
    #[test]
    fn test_02_advance_moves_one() {
        let mut c = Cursor::new(ts("alpha beta"));
        c.advance();
        assert_eq!(c.peek_str().as_deref(), Some("beta"));
    }

    // 03. peek_str returns None when the current token is not an identifier
    #[test]
    fn test_03_peek_str_non_ident() {
        let c = Cursor::new(ts("123 alpha"));
        assert_eq!(c.peek_str(), None);
    }

    // 04. next_ident consumes and returns an identifier
    #[test]
    fn test_04_next_ident_ok() {
        let mut c = Cursor::new(ts("name rest"));
        let id = c.next_ident("ctx").unwrap();
        assert_eq!(id.to_string(), "name");
        assert_eq!(c.peek_str().as_deref(), Some("rest"));
    }

    // 05. next_ident errors on a non-identifier with a located message
    #[test]
    fn test_05_next_ident_err() {
        let mut c = Cursor::new(ts("123"));
        let e = c.next_ident("parser name").unwrap_err();
        assert!(err_msg(e).contains("expected ident"));
    }

    // 06. next_group accepts a brace group and exposes its inner stream
    #[test]
    fn test_06_next_group_brace_ok() {
        let mut c = Cursor::new(ts("{ inner }"));
        let g = c.next_group(Delimiter::Brace, "body").unwrap();
        assert!(!g.stream().is_empty());
    }

    // 07. next_group errors when the delimiter kind does not match
    #[test]
    fn test_07_next_group_wrong_delim() {
        let mut c = Cursor::new(ts("( inner )"));
        let e = c.next_group(Delimiter::Brace, "body").unwrap_err();
        assert!(err_msg(e).contains("expected group"));
    }

    // 08. next_lit returns Some for a literal and None for a non-literal
    #[test]
    fn test_08_next_lit_some_and_none() {
        assert!(Cursor::new(ts("42")).next_lit().is_some());
        assert!(Cursor::new(ts("ident")).next_lit().is_none());
    }

    // 09. expect_lit consumes a literal or errors with a located message
    #[test]
    fn test_09_expect_lit() {
        assert!(Cursor::new(ts("b'*'")).expect_lit("byte").is_ok());
        let e = Cursor::new(ts("ident")).expect_lit("byte").unwrap_err();
        assert!(err_msg(e).contains("expected literal"));
    }

    // 10. collect_lit_alternation gathers a `a | b | c` run of literals
    #[test]
    fn test_10_lit_alternation() {
        let mut c = Cursor::new(ts("b'-' | b'*' | b'_'"));
        assert_eq!(c.collect_lit_alternation().len(), 3);
    }

    // 11. collect_lit_alternation stops at the first non-`|` token, leaving it
    #[test]
    fn test_11_lit_alternation_stops() {
        let mut c = Cursor::new(ts("b'-' | b'*' , rest"));
        assert_eq!(c.collect_lit_alternation().len(), 2);
        assert!(matches!(c.peek(), Some(TT::Punct(_))));
    }

    // 12. skip_pipe_vars_returning extracts the identifiers of a `|a, b|` list
    #[test]
    fn test_12_pipe_vars() {
        let mut c = Cursor::new(ts("|n, k| rest"));
        let vars = c.skip_pipe_vars_returning();
        assert_eq!(vars.len(), 2);
        assert_eq!(vars[0].to_string(), "n");
        assert_eq!(vars[1].to_string(), "k");
    }

    // 13. is_fat_arrow recognises a joined `=>` and rejects a lone `=`
    #[test]
    fn test_13_is_fat_arrow() {
        assert!(Cursor::new(ts("=> field")).is_fat_arrow());
        assert!(!Cursor::new(ts("= rest")).is_fat_arrow());
    }

    // 14. arrow_field_cap parses `=> field [N]` into the field and capacity
    #[test]
    fn test_14_arrow_field_cap_ok() {
        let mut c = Cursor::new(ts("=> italics [40]"));
        let (f, cap) = c.arrow_field_cap("ctx").unwrap();
        assert_eq!(f.to_string(), "italics");
        assert_eq!(cap.to_string(), "40");
    }

    // 15. arrow_field_cap errors when the `=>` is absent
    #[test]
    fn test_15_arrow_field_cap_no_arrow() {
        let mut c = Cursor::new(ts("italics [40]"));
        let e = c.arrow_field_cap("ctx").unwrap_err();
        assert!(err_msg(e).contains("expected `=>`"));
    }

    // 16. arrow_field_cap errors when the `[N]` capacity group is missing
    #[test]
    fn test_16_arrow_field_cap_no_cap() {
        let mut c = Cursor::new(ts("=> italics rest"));
        let e = c.arrow_field_cap("ctx").unwrap_err();
        assert!(err_msg(e).contains("[N]"));
    }

    // 17. named_lit consumes `name = <literal>` and returns the literal
    #[test]
    fn test_17_named_lit_ok() {
        let mut c = Cursor::new(ts("sep = b' '"));
        assert_eq!(c.named_lit("sep").unwrap().to_string(), "b' '");
    }

    // 18. named_lit errors when the leading name does not match
    #[test]
    fn test_18_named_lit_wrong_name() {
        let mut c = Cursor::new(ts("eol = b'\\n'"));
        let e = c.named_lit("sep").unwrap_err();
        assert!(err_msg(e).contains("expected `sep`"));
    }

    // 19. skip / skip_colon / skip_eq_alone drop only the requested punctuation
    #[test]
    fn test_19_skips() {
        let mut c = Cursor::new(ts(", : = rest"));
        c.skip(',');
        c.skip_colon();
        c.skip_eq_alone();
        assert_eq!(c.peek_str().as_deref(), Some("rest"));
    }

    // 20. collect_until_brace gathers everything up to (not into) the next brace
    #[test]
    fn test_20_collect_until_brace() {
        let mut c = Cursor::new(ts("Heading { level } tail"));
        assert_eq!(c.collect_until_brace().to_string(), "Heading");
        assert!(matches!(c.peek(), Some(TT::Group(_))));
    }
}
