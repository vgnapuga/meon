//! Block-level element parser — the `parse_block!` macro.
//!
//! # Overview
//!
//! `parse_block!` handles structured multi-line constructs: fenced code blocks,
//! blockquote continuations, bullet lists, and ordered lists. It is called by
//! `parse_text!` at the start of each line and operates in two distinct phases:
//!
//! - **Active phase** (`@active`): if there is an open block (fence or
//!   continuation), the line is consumed by that block. Returns `Some((false, cs))`.
//! - **Open phase** (`@open_simple` / `@open_block`): tries to open a new block
//!   on the current line. Returns `Some((true, cs))` on success, `None` if no
//!   rule matched.
//!
//! It is not meant to be called by users directly.
//!
//! # Active block state
//!
//! A single `Option<(u8, u8, u8, u32)>` slot encodes the active block:
//!
//! | Discriminant (field 0) | Meaning            | Field 1  | Field 2 | Field 3  |
//! |------------------------|--------------------|----------|---------|----------|
//! | `0`                    | Open fence         | `byte`   | `count` | `start`  |
//! | `1`                    | Continuation (`>`) | `byte`   | `0`     | `start`  |
//!
//! Only one block can be active at a time. This is a known limitation: nested
//! constructs such as `> \`\`\`` cannot be represented simultaneously.
//!
//! # Return value
//!
//! Returns `Option<(bool, usize)>`:
//! - `Some((true, cs))` — a new block was opened; `cs` is the first content byte.
//! - `Some((false, cs))` — the active block consumed this line; `cs` is the next
//!   line start.
//! - `None` — no block rule matched and no block is active.
//!
//! # Grammar integration
//!
//! The macro consumes the `blocks { … }` section after `strip`. Two sub-sections:
//!
//! - `block_simple { … }` — single-line openers and continuations:
//!   - `fence(byte, min = N) => field;` — fenced code blocks (e.g. ` ``` `).
//!   - `cont(byte) => field;` — line-continuation blocks (e.g. blockquotes `>`).
//!
//! - `block { … }` — single-line blocks with per-line metadata:
//!   - `(pat) |var|: Type { … } => field;` — marker-prefixed items (bullet lists).
//!   - `num(digit_pat, end = end_pat) |n, k|: Type { … } => field;` — numbered
//!     items (ordered lists).
#[doc(hidden)]
#[macro_export]
macro_rules! parse_block {
    (
        $active:ident, $state:ident, $src:ident, $pos:expr, $le:expr,
        sep = $sep:literal, tab = $tab:literal ;
        block_simple { $($sr:tt)* }
        block        { $($br:tt)* }
    ) => {{
        let src: &[u8] = $src;
        let pos: usize = $pos;
        let le:  usize = $le;

        let mut _ar: Option<(bool, usize)> = None;
        $crate::parse_block!(@active _ar, $active, $state, src, pos, le, $sep, $tab ; $($sr)*);

        if _ar.is_some() { _ar } else {
            let mut _or: Option<(bool, usize)> = None;
            $crate::parse_block!(@open_simple _or, $active, $state, src, pos, le, $sep, $tab ; $($sr)*);
            if _or.is_none() {
                $crate::parse_block!(@open_block _or, $active, $state, src, pos, le, $sep, $tab ; $($br)*);
            }
            _or
        }
    }};


    (@active $res:ident, $active:ident, $st:ident, $src:ident,
     $pos:ident, $le:ident, $sep:literal, $tab:literal ;
     fence($pat:pat, min = $min:literal) => $field:ident $($rest:tt)*
    ) => {
        if $res.is_none() {
            if let Some((0u8, fbyte, flen, fstart)) = *$active {
                if matches!(fbyte, $pat) {
                    let mut _i: usize = $pos;
                    let mut _c: u8    = 0;
                    while _i < $le && $src[_i] == fbyte { _c = _c.saturating_add(1); _i += 1; }
                    if _c >= flen && $src[_i..$le].iter().all(|&b| b == $sep || b == $tab) {
                        let end = if $le < $src.len() { $le + 1 } else { $src.len() };
                        $st.$field.push($crate::span::Span::new(fstart, end as u32));
                        *$active = None;
                    }
                    let np = if $le < $src.len() { $le + 1 } else { $src.len() };
                    $res = Some((false, np));
                }
            }
        }
        $crate::parse_block!(@active $res, $active, $st, $src, $pos, $le, $sep, $tab ; $($rest)*)
    };

    (@active $res:ident, $active:ident, $st:ident, $src:ident,
     $pos:ident, $le:ident, $sep:literal, $tab:literal ;
     cont($byte:literal) => $field:ident $($rest:tt)*
    ) => {
        if $res.is_none() {
            if let Some((1u8, ab, _, cstart)) = *$active {
                if ab == $byte {
                    if $pos < $le && $src[$pos] == $byte {
                        let cs = if $pos + 1 < $le
                            && ($src[$pos + 1] == $sep || $src[$pos + 1] == $tab)
                        {
                            $pos + 2
                        } else {
                            $pos + 1
                        };
                        $res = Some((false, cs));
                    } else {
                        $st.$field.push($crate::span::Span::new(cstart, $pos as u32));
                        *$active = None;
                    }
                }
            }
        }
        $crate::parse_block!(@active $res, $active, $st, $src, $pos, $le, $sep, $tab ; $($rest)*)
    };

    (@active $r:ident, $a:ident, $st:ident, $src:ident,
     $pos:ident, $le:ident, $sep:literal, $tab:literal ; , $($rest:tt)*) => {
        $crate::parse_block!(@active $r, $a, $st, $src, $pos, $le, $sep, $tab ; $($rest)*)
    };
    (@active $r:ident, $a:ident, $st:ident, $src:ident,
     $pos:ident, $le:ident, $sep:literal, $tab:literal ; ; $($rest:tt)*) => {
        $crate::parse_block!(@active $r, $a, $st, $src, $pos, $le, $sep, $tab ; $($rest)*)
    };
    (@active $r:ident, $a:ident, $st:ident, $src:ident,
     $pos:ident, $le:ident, $sep:literal, $tab:literal ;) => {};


    (@open_simple $res:ident, $active:ident, $st:ident, $src:ident,
     $pos:ident, $le:ident, $sep:literal, $tab:literal ;
     fence($pat:pat, min = $min:literal) => $field:ident $($rest:tt)*
    ) => {
        if $res.is_none() && $pos < $le && matches!($src[$pos], $pat) {
            let _byte = $src[$pos];
            let mut _i: usize = $pos;
            let mut _c: u8    = 0;
            while _i < $le && $src[_i] == _byte { _c = _c.saturating_add(1); _i += 1; }
            if _c >= $min && $src[_i..$le].iter().all(|&b| b != _byte) {
                *$active = Some((0u8, _byte, _c, $pos as u32));
                let np = if $le < $src.len() { $le + 1 } else { $src.len() };
                $res = Some((true, np));
            }
        }
        $crate::parse_block!(@open_simple $res, $active, $st, $src, $pos, $le, $sep, $tab ; $($rest)*)
    };

    (@open_simple $res:ident, $active:ident, $st:ident, $src:ident,
     $pos:ident, $le:ident, $sep:literal, $tab:literal ;
     cont($byte:literal) => $field:ident $($rest:tt)*
    ) => {
        if $res.is_none() && $pos < $le && $src[$pos] == $byte {
            *$active = Some((1u8, $byte, 0u8, $pos as u32));
            let cs = if $pos + 1 < $le
                && ($src[$pos + 1] == $sep || $src[$pos + 1] == $tab)
            {
                $pos + 2
            } else {
                $pos + 1
            };
            $res = Some((true, cs));
        }
        $crate::parse_block!(@open_simple $res, $active, $st, $src, $pos, $le, $sep, $tab ; $($rest)*)
    };

    (@open_simple $r:ident, $a:ident, $st:ident, $src:ident,
     $pos:ident, $le:ident, $sep:literal, $tab:literal ; , $($rest:tt)*) => {
        $crate::parse_block!(@open_simple $r, $a, $st, $src, $pos, $le, $sep, $tab ; $($rest)*)
    };
    (@open_simple $r:ident, $a:ident, $st:ident, $src:ident,
     $pos:ident, $le:ident, $sep:literal, $tab:literal ; ; $($rest:tt)*) => {
        $crate::parse_block!(@open_simple $r, $a, $st, $src, $pos, $le, $sep, $tab ; $($rest)*)
    };
    (@open_simple $r:ident, $a:ident, $st:ident, $src:ident,
     $pos:ident, $le:ident, $sep:literal, $tab:literal ;) => {};


    (@open_block $res:ident, $active:ident, $st:ident, $src:ident,
     $pos:ident, $le:ident, $sep:literal, $tab:literal ;
     ($pat:pat) |$b:ident|: $meta:expr => $field:ident $($rest:tt)*
    ) => {
        if $res.is_none() && $pos < $le {
            let mut _p = $pos;
            while _p < $le && ($src[_p] == $sep || $src[_p] == $tab) { _p += 1; }
            if _p < $le && matches!($src[_p], $pat) {
                let next = _p + 1;
                if next < $le && ($src[next] == $sep || $src[next] == $tab) {
                    let cs    = next + 1;
                    let $b    = $src[_p];
                    let _meta = $meta;
                    $st.$field.push((_meta, $crate::span::Span::new(cs as u32, $le as u32)));
                    $res = Some((true, cs));
                }
            }
        }
        $crate::parse_block!(@open_block $res, $active, $st, $src, $pos, $le, $sep, $tab ; $($rest)*)
    };

    (@open_block $res:ident, $active:ident, $st:ident, $src:ident,
     $pos:ident, $le:ident, $sep:literal, $tab:literal ;
     num($dp:pat, end = $ep:pat) |$n:ident, $k:ident|: $meta:expr => $field:ident $($rest:tt)*
    ) => {
        if $res.is_none() && $pos < $le {
            let mut _p = $pos;
            while _p < $le && ($src[_p] == $sep || $src[_p] == $tab) { _p += 1; }
            if _p < $le && matches!($src[_p], $dp) {
                let mut _i: usize = _p;
                let mut _num: u32 = 0;
                let mut _dc: u8   = 0;
                while _i < $le && matches!($src[_i], $dp) && _dc < 9 {
                    _num = _num * 10 + ($src[_i] - b'0') as u32;
                    _i += 1; _dc += 1;
                }
                if _dc > 0 && _i < $le && matches!($src[_i], $ep) {
                    let _end = $src[_i];
                    _i += 1;
                    if _i < $le && ($src[_i] == $sep || $src[_i] == $tab) {
                        let cs    = _i + 1;
                        let $n    = _num;
                        let $k    = _end;
                        let _meta = $meta;
                        $st.$field.push((_meta, $crate::span::Span::new(cs as u32, $le as u32)));
                        $res = Some((true, cs));
                    }
                }
            }
        }
        $crate::parse_block!(@open_block $res, $active, $st, $src, $pos, $le, $sep, $tab ; $($rest)*)
    };

    (@open_block $r:ident, $a:ident, $st:ident, $src:ident,
     $pos:ident, $le:ident, $sep:literal, $tab:literal ; , $($rest:tt)*) => {
        $crate::parse_block!(@open_block $r, $a, $st, $src, $pos, $le, $sep, $tab ; $($rest)*)
    };
    (@open_block $r:ident, $a:ident, $st:ident, $src:ident,
     $pos:ident, $le:ident, $sep:literal, $tab:literal ; ; $($rest:tt)*) => {
        $crate::parse_block!(@open_block $r, $a, $st, $src, $pos, $le, $sep, $tab ; $($rest)*)
    };
    (@open_block $r:ident, $a:ident, $st:ident, $src:ident,
     $pos:ident, $le:ident, $sep:literal, $tab:literal ;) => {};
}
