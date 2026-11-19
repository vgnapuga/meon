//! Line-level element parser — the `parse_line!` macro.
//!
//! # Overview
//!
//! `parse_line!` scans a single logical line (already delimited by `parse_text!`)
//! and tries to match it against a set of *line rules*, which are matched at
//! the line start; the first rule that matches wins and no other is tried.
//!
//! It is called by `parse_text!` after block handling when no block rule
//! claimed the line, before scanning its remaining content for inline elements.
//!
//! # Return value
//!
//! Returns `Some(cs)` where `cs` is where the line's content starts: past a
//! `line` rule's marker run and its separator, for `parse_text!` to scan
//! inline up to the line end; the line end for a `line_simple` rule, which
//! consumes the line whole. `None` if no rule matched; the caller
//! (`parse_text!`) then treats the line as paragraph text.
//!
//! # Span limit
//!
//! A line rule carries an optional `max_line_span` cap. A matching span
//! longer than the cap is not emitted in the rule's field: the span itself
//! goes to the grammar's `blocks { fallback => ... }` field, while the line
//! keeps its normal control flow (so trailing content is still scanned for
//! inline elements). With no cap, the rule pushes exactly as before.
//!
//! # Grammar integration
//!
//! The macro consumes the `lines { ... }` section of a `define_parser!` grammar
//! after `strip` has removed the `=> field [N]` annotations. Two rule kinds are
//! supported:
//!
//! - `line(byte, max = N) |var|: Type { ... } => field;`
//!   Matches lines that start with 1–N consecutive occurrences of `byte`
//!   followed by the separator byte or the line end. The count is bound to
//!   `var` and passed to the type constructor; the span is the rest of the
//!   line. Example: ATX headings (`# ... ######`).
//!
//! - `line_simple(pattern, min = N) |var|: Type { ... } => field;`
//!   Matches lines composed entirely of one repeated byte `pattern` matches
//!   (interleaved with the separator), with at least `N` occurrences. That
//!   byte is bound to `var`. Example: thematic breaks (`---`, `***`, `___`).
//!
//! Rules are tried in declaration order; the first match wins.
#[doc(hidden)]
#[macro_export]
macro_rules! parse_line {
    ($state:ident, $src:ident, $pos:expr, $le:expr, sep = $sep:literal ;
     $($rules:tt)*) => {
        $crate::parse_line!(
            $state, $src, $pos, $le, sep = $sep,
            max_line = [], para = _limits_unused ;
            $($rules)*
        )
    };

    ($state:ident, $src:ident, $pos:expr, $le:expr, sep = $sep:literal,
     max_line = $max_line:tt, para = $para:ident ;
     $($rules:tt)*) => {{
        let src: &[u8] = $src;
        let pos: usize  = $pos;
        let le:  usize  = $le;
        let mut _r: Option<usize> = None;
        $crate::parse_line!(@emit _r, $state, src, pos, le, $sep, $max_line, $para ; $($rules)*);
        _r
    }};

    (@emit $res:ident, $st:ident, $src:ident, $pos:ident, $le:ident, $sep:literal,
     $max_line:tt, $para:ident ;
        line($byte:literal, max = $max:literal) |$n:ident|: $meta:expr => $field:ident
        $($rest:tt)*
    ) => {
        if $res.is_none() && $pos < $le && $src[$pos] == $byte {
            let mut _i: usize = $pos;
            let mut _c: u8    = 0;
            while _i < $le && $src[_i] == $byte && _c < $max {
                _c += 1;
                _i += 1;
            }
            if _c > 0 && (_i >= $le || $src[_i] == $sep) {
                if _i < $le { _i += 1; }
                let $n    = _c;
                let _meta = $meta;
                $crate::parse_line!(@push $max_line, $st, $field, $para, _meta,
                    $crate::span::Span::new(_i as u32, $le as u32));
                $res = Some(_i);
            }
        }
        $crate::parse_line!(@emit $res, $st, $src, $pos, $le, $sep, $max_line, $para ; $($rest)*)
    };

    (@emit $res:ident, $st:ident, $src:ident, $pos:ident, $le:ident, $sep:literal,
     $max_line:tt, $para:ident ;
        line_simple($pat:pat, min = $min:literal) |$b:ident|: $meta:expr => $field:ident
        $($rest:tt)*
    ) => {
        if $res.is_none() && $pos < $le {
            let _delim = $src[$pos];
            if matches!(_delim, $pat) {
                let mut _count: u32 = 0;
                let mut _valid      = true;
                for &_c in &$src[$pos..$le] {
                    if _c == _delim    { _count += 1; }
                    else if _c != $sep { _valid = false; break; }
                }
                if _valid && _count >= $min {
                    let $b    = _delim;
                    let _meta = $meta;
                    $crate::parse_line!(@push $max_line, $st, $field, $para, _meta,
                        $crate::span::Span::new($pos as u32, $le as u32));
                    $res = Some($le);
                }
            }
        }
        $crate::parse_line!(@emit $res, $st, $src, $pos, $le, $sep, $max_line, $para ; $($rest)*)
    };

    (@push [], $st:ident, $field:ident, $para:ident, $meta:expr, $span:expr) => {
        $crate::paste::paste! {
            $st.[<push_ $field>](($meta, $span));
        }
    };
    (@push [$n:literal], $st:ident, $field:ident, $para:ident, $meta:expr, $span:expr) => {{
        let _span = $span;
        if (_span.end - _span.start) > $n as u32 {
            $crate::paste::paste! {
                $st.[<push_ $para>](_span);
            }
        } else {
            $crate::paste::paste! {
                $st.[<push_ $field>](($meta, _span));
            }
        }
    }};

    (@emit $r:ident, $st:ident, $src:ident, $pos:ident, $le:ident, $sep:literal,
     $max_line:tt, $para:ident ; , $($rest:tt)*) => {
        $crate::parse_line!(@emit $r, $st, $src, $pos, $le, $sep, $max_line, $para ; $($rest)*)
    };
    (@emit $r:ident, $st:ident, $src:ident, $pos:ident, $le:ident, $sep:literal,
     $max_line:tt, $para:ident ; ; $($rest:tt)*) => {
        $crate::parse_line!(@emit $r, $st, $src, $pos, $le, $sep, $max_line, $para ; $($rest)*)
    };
    (@emit $r:ident, $st:ident, $src:ident, $pos:ident, $le:ident, $sep:literal,
     $max_line:tt, $para:ident ;) => {};
}
