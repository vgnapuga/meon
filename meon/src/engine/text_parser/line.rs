//! Line-level element parser — the `parse_line!` macro.
//!
//! # Overview
//!
//! `parse_line!` scans a single logical line (already delimited by `parse_text!`)
//! and tries to match it against a set of *line rules*. Unlike inline parsing,
//! line rules are **whole-line**: if a rule matches, it consumes the entire line
//! and returns a continuation skip; no other rule is tried.
//!
//! It is called by `parse_text!` at the start of each new line, before inline
//! scanning begins, and is not meant to be called by users directly.
//!
//! # Return value
//!
//! Returns `Some(cs)` where `cs` is the byte offset of the first byte after the
//! matched line (i.e. the start of the next line), or `None` if no rule matched.
//! The caller (`parse_text!`) uses `None` to fall through to block and inline
//! processing.
//!
//! # Grammar integration
//!
//! The macro consumes the `lines { ... }` section of a `define_parser!` grammar
//! after `strip` has removed the `=> field [N]` annotations. Two rule kinds are
//! supported:
//!
//! - `line(byte, max = N) |var|: Type { ... } => field;`
//!   Matches lines that start with 1–N consecutive occurrences of `byte`
//!   followed by the separator byte. The count is bound to `var` and passed
//!   to the type constructor. Example: ATX headings (`# ... ######`).
//!
//! - `line_simple(b1 | b2 | ..., min = N) |var|: Type { ... } => field;`
//!   Matches lines composed entirely of one repeated delimiter byte (interleaved
//!   with the separator), with at least `N` occurrences. The delimiter byte is
//!   bound to `var`. Example: thematic breaks (`---`, `***`, `___`).
//!
//! Rules are tried in declaration order; the first match wins.
#[doc(hidden)]
#[macro_export]
macro_rules! parse_line {
    ($state:ident, $src:ident, $pos:expr, $le:expr, sep = $sep:literal ; $($rules:tt)*) => {{
        let src: &[u8] = $src;
        let pos: usize  = $pos;
        let le:  usize  = $le;
        let mut _r: Option<usize> = None;
        $crate::parse_line!(@emit _r, $state, src, pos, le, $sep ; $($rules)*);
        _r
    }};

    (@emit $res:ident, $st:ident, $src:ident, $pos:ident, $le:ident, $sep:literal ;
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
                $st.$field.push((_meta, $crate::span::Span::new(_i as u32, $le as u32)));
                $res = Some(_i);
            }
        }
        $crate::parse_line!(@emit $res, $st, $src, $pos, $le, $sep ; $($rest)*)
    };

    (@emit $res:ident, $st:ident, $src:ident, $pos:ident, $le:ident, $sep:literal ;
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
                    $st.$field.push((_meta, $crate::span::Span::new($pos as u32, $le as u32)));
                    $res = Some($le);
                }
            }
        }
        $crate::parse_line!(@emit $res, $st, $src, $pos, $le, $sep ; $($rest)*)
    };

    (@emit $r:ident, $st:ident, $src:ident, $pos:ident, $le:ident, $sep:literal ; , $($rest:tt)*) => {
        $crate::parse_line!(@emit $r, $st, $src, $pos, $le, $sep ; $($rest)*)
    };
    (@emit $r:ident, $st:ident, $src:ident, $pos:ident, $le:ident, $sep:literal ; ; $($rest:tt)*) => {
        $crate::parse_line!(@emit $r, $st, $src, $pos, $le, $sep ; $($rest)*)
    };
    (@emit $r:ident, $st:ident, $src:ident, $pos:ident, $le:ident, $sep:literal ;) => {};
}
