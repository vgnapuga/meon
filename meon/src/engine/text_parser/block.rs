//! Block-level element parser — the `parse_block!` macro.
//!
//! # Overview
//!
//! `parse_block!` handles structured multi-line constructs: fenced code blocks,
//! blockquote continuations, bullet lists, and ordered lists. It is called by
//! `parse_text!` at the start of each line and, in a single call, fully
//! resolves that line's block structure:
//!
//! - **Peel phase**: every currently-open block frame is matched against the
//!   line from the outside in — continuation markers are consumed, a still-open
//!   fence either closes or swallows the line, and a frame whose marker is gone
//!   is closed (together with everything nested inside it).
//! - **Open phase**: at the position left after peeling, new block frames are
//!   opened (as many as nest on one line, e.g. a fence opening inside a
//!   blockquote), bounded by `max_nest`.
//!
//! It is not meant to be called by users directly.
//!
//! # Active block stack (`max_nest`)
//!
//! The active block state is a bounded stack `[(u8, u8, u8, u32); max_nest]`
//! plus a depth counter, sharing the grammar-wide `max_nest` cap with the
//! inline engine (see [`crate::parse_inline!`]). `max_nest = 1` reduces it to a
//! single slot and reproduces the original, single-active-block behaviour
//! exactly: at most one block open at a time, no block opening inside another.
//!
//! | Discriminant (field 0) | Meaning            | Field 1  | Field 2 | Field 3  |
//! |------------------------|--------------------|----------|---------|----------|
//! | `0`                    | Open fence         | `byte`   | `count` | `start`  |
//! | `1`                    | Continuation (`>`) | `byte`   | `0`     | `start`  |
//!
//! Two structural invariants make the stack tractable:
//!
//! - **A fence is always the top frame.** Fence content is opaque — no block
//!   can open inside it — so when a fence is the innermost open block it
//!   consumes the whole line and the open phase never runs; nothing is ever
//!   pushed above a fence.
//! - **Continuations may self-nest.** Unlike an inline symmetric delimiter
//!   (where open and close are indistinguishable, so an identical key can't
//!   nest), a `cont` opens positionally — at the line start, after the outer
//!   markers have been peeled — and closes by *absence* of its marker, so
//!   `> >` is two genuinely nested blockquote frames.
//!
//! `block` items (bullets, ordered) are per-line leaves: they push nothing onto
//! the stack and so consume no depth. They may still open *inside* a `cont`,
//! but only when `max_nest > 1` — at `max_nest = 1` the open phase never runs
//! inside an already-open block, exactly as before.
//!
//! # Return value
//!
//! Returns `Option<(bool, usize)>`:
//! - `Some((true, cs))` — at least one new block was opened; `cs` is the first
//!   content byte (or the next line start, for a fence whose info line is
//!   consumed whole).
//! - `Some((false, cs))` — the active stack consumed/continued this line
//!   without opening anything new; `cs` is where inline scanning should resume
//!   (for a continued `cont`) or the next line start (for a fence line).
//! - `None` — nothing matched and nothing is active, **or** an outer
//!   continuation just closed; in the latter case the depth has changed and
//!   `parse_text!` re-runs from the same line start to reprocess the remainder.
//!
//! # Grammar integration
//!
//! The macro consumes the `blocks { ... }` section after `strip`. Two sub-sections:
//!
//! - `block_simple { ... }` — single-line openers and continuations:
//!   - `fence(byte, min = N) => field;` — fenced code blocks (e.g. ` ``` `).
//!   - `cont(byte) => field;` — line-continuation blocks (e.g. blockquotes `>`).
//!
//! - `block { ... }` — single-line blocks with per-line metadata:
//!   - `(pat) |var|: Type { ... } => field;` — marker-prefixed items (bullet lists).
//!   - `num(digit_pat, end = end_pat) |n, k|: Type { ... } => field;` — numbered
//!     items (ordered lists).
#[doc(hidden)]
#[macro_export]
macro_rules! parse_block {
    (
        $stack:ident, $depth:ident, $state:ident, $src:ident, $pos:expr, $le:expr,
        sep = $sep:literal, tab = $tab:literal, max_nest = $maxn:literal ;
        block_simple { $($sr:tt)* }
        block        { $($br:tt)* }
    ) => {{
        let src: &[u8] = $src;
        let pos: usize = $pos;
        let le:  usize = $le;

        // ---- PEEL: walk the active frames outermost → innermost from `pos`. //
        let mut _cur: usize = pos;
        let mut _fi: usize = 0;
        let mut _peel_broke: bool = false;     // an outer cont closed → reprocess
        let mut _line_consumed: bool = false;  // a fence frame ate the whole line
        let mut _consumed_np: usize = 0;

        while _fi < $depth {
            let (_disc, _ab, _ac, _astart) = $stack[_fi];
            if _disc == 1u8 {
                // cont frame: its marker must be present at `_cur` to continue.
                if _cur < le && src[_cur] == _ab {
                    _cur = if _cur + 1 < le
                        && (src[_cur + 1] == $sep || src[_cur + 1] == $tab)
                    { _cur + 2 } else { _cur + 1 };
                    _fi += 1;
                } else {
                    // Marker gone: this frame and everything nested inside it
                    // close, innermost-first. Their spans end at this line's
                    // start (`pos`), where containment broke. The line itself
                    // is then reprocessed from `pos`.
                    let mut _j = $depth;
                    while _j > _fi {
                        _j -= 1;
                        let (_d2, _b2, _c2, _s2) = $stack[_j];
                        $crate::parse_block!(@close_frame
                            $state, src, pos as u32, _d2, _b2, _s2 ; $($sr)*);
                    }
                    $depth = _fi;
                    _peel_broke = true;
                    break;
                }
            } else {
                // fence frame (always the top): does this line close it?
                let mut _i = _cur;
                let mut _fc: u8 = 0;
                while _i < le && src[_i] == _ab { _fc = _fc.saturating_add(1); _i += 1; }
                if _fc >= _ac && src[_i..le].iter().all(|&b| b == $sep || b == $tab) {
                    let _end = if le < src.len() { le + 1 } else { src.len() };
                    $crate::parse_block!(@close_frame
                        $state, src, _end as u32, _disc, _ab, _astart ; $($sr)*);
                    $depth -= 1;
                }
                // Closed or not, the fence line is fully consumed.
                _consumed_np = if le < src.len() { le + 1 } else { src.len() };
                _line_consumed = true;
                break;
            }
        }

        if _line_consumed {
            Some((false, _consumed_np))
        } else if _peel_broke {
            None
        } else {
            // All active frames (if any) were `cont` and continued; `_cur` is
            // now past their markers. Try to OPEN new block(s) here.
            let _had_frames = $depth > 0;
            // Top level always opens; inside an already-open block only when
            // `max_nest` allows another level. This keeps `max_nest = 1`
            // identical: inside any block, the remainder goes straight to
            // inline, no further block opens.
            let _can_open = !_had_frames || ($depth < $maxn);
            let mut _opened_any = false;
            let mut _open_cur = _cur;

            if _can_open {
                let mut _go = true;
                while _go {
                    _go = false;
                    let mut _or: ::core::option::Option<(bool, usize)> = None;
                    if $depth < $maxn {
                        $crate::parse_block!(@open_simple _or, $stack, $depth, $state,
                            src, _open_cur, le, $sep, $tab ; $($sr)*);
                    }
                    if let Some((_o, _cs)) = _or {
                        _opened_any = true;
                        _open_cur = _cs;
                        _go = true;          // a frame opened — try to nest further
                        continue;
                    }
                    // A leaf `block` item (bullet / num) is itself a nesting
                    // level, so it is gated by the same `max_nest` cap as a
                    // frame: at `max_nest = 1` an item never opens inside an
                    // already-open block — the remainder goes to inline
                    // instead, exactly as the pre-nesting engine did. At the
                    // top level (`depth == 0 < max_nest`) it always opens.
                    let mut _bres: ::core::option::Option<(bool, usize)> = None;
                    if $depth < $maxn {
                        $crate::parse_block!(@open_block _bres, $state,
                            src, _open_cur, le, $sep, $tab ; $($br)*);
                    }
                    if let Some((_o, _cs)) = _bres {
                        _opened_any = true;
                        _open_cur = _cs;
                    }
                }
            }

            if _opened_any {
                Some((true, _open_cur))
            } else if _had_frames {
                Some((false, _open_cur))
            } else {
                None
            }
        }
    }};


    // ------------------------------------------------------------------ //
    // @close_frame: push the span for one frame (by runtime tuple), the  //
    // field chosen by matching the stored byte against each rule.        //
    // ------------------------------------------------------------------ //
    (@close_frame $st:ident, $src:ident, $endpos:expr, $disc:expr, $byte:expr, $start:expr ;
        $($sr:tt)*
    ) => {
        $crate::parse_block!(@close_frame_inner
            $st, $src, $endpos, $disc, $byte, $start ; $($sr)*)
    };
    (@close_frame_inner $st:ident, $src:ident, $endpos:expr, $disc:expr, $byte:expr, $start:expr ;
        cont($cb:literal) => $field:ident $($rest:tt)*
    ) => {
        if $disc == 1u8 && $byte == $cb {
            $st.$field.push($crate::span::Span::new($start, $endpos));
        }
        $crate::parse_block!(@close_frame_inner $st, $src, $endpos, $disc, $byte, $start ; $($rest)*)
    };
    (@close_frame_inner $st:ident, $src:ident, $endpos:expr, $disc:expr, $byte:expr, $start:expr ;
        fence($pat:pat, min = $min:literal) => $field:ident $($rest:tt)*
    ) => {
        if $disc == 0u8 && matches!($byte, $pat) {
            $st.$field.push($crate::span::Span::new($start, $endpos));
        }
        $crate::parse_block!(@close_frame_inner $st, $src, $endpos, $disc, $byte, $start ; $($rest)*)
    };
    (@close_frame_inner $st:ident, $src:ident, $e:expr, $d:expr, $b:expr, $s:expr ; , $($r:tt)*) => {
        $crate::parse_block!(@close_frame_inner $st, $src, $e, $d, $b, $s ; $($r)*)
    };
    (@close_frame_inner $st:ident, $src:ident, $e:expr, $d:expr, $b:expr, $s:expr ; ; $($r:tt)*) => {
        $crate::parse_block!(@close_frame_inner $st, $src, $e, $d, $b, $s ; $($r)*)
    };
    (@close_frame_inner $st:ident, $src:ident, $e:expr, $d:expr, $b:expr, $s:expr ;) => {};


    // ------------------------------------------------------------------ //
    // @open_simple: try to open one fence/cont frame, pushing onto the   //
    // stack. The depth-vs-max_nest gate is applied by the caller.        //
    // ------------------------------------------------------------------ //
    (@open_simple $res:ident, $stack:ident, $depth:ident, $st:ident, $src:ident,
     $pos:ident, $le:ident, $sep:literal, $tab:literal ;
     fence($pat:pat, min = $min:literal) => $field:ident $($rest:tt)*
    ) => {
        if $res.is_none() && $pos < $le && matches!($src[$pos], $pat) {
            let _byte = $src[$pos];
            let mut _i: usize = $pos;
            let mut _c: u8    = 0;
            while _i < $le && $src[_i] == _byte { _c = _c.saturating_add(1); _i += 1; }
            if _c >= $min && $src[_i..$le].iter().all(|&b| b != _byte) {
                $stack[$depth] = (0u8, _byte, _c, $pos as u32);
                $depth += 1;
                let np = if $le < $src.len() { $le + 1 } else { $src.len() };
                $res = Some((true, np));
            }
        }
        $crate::parse_block!(@open_simple $res, $stack, $depth, $st, $src, $pos, $le, $sep, $tab ; $($rest)*)
    };

    (@open_simple $res:ident, $stack:ident, $depth:ident, $st:ident, $src:ident,
     $pos:ident, $le:ident, $sep:literal, $tab:literal ;
     cont($byte:literal) => $field:ident $($rest:tt)*
    ) => {
        if $res.is_none() && $pos < $le && $src[$pos] == $byte {
            $stack[$depth] = (1u8, $byte, 0u8, $pos as u32);
            $depth += 1;
            let cs = if $pos + 1 < $le
                && ($src[$pos + 1] == $sep || $src[$pos + 1] == $tab)
            { $pos + 2 } else { $pos + 1 };
            $res = Some((true, cs));
        }
        $crate::parse_block!(@open_simple $res, $stack, $depth, $st, $src, $pos, $le, $sep, $tab ; $($rest)*)
    };

    (@open_simple $r:ident, $stk:ident, $d:ident, $st:ident, $src:ident,
     $pos:ident, $le:ident, $sep:literal, $tab:literal ; , $($rest:tt)*) => {
        $crate::parse_block!(@open_simple $r, $stk, $d, $st, $src, $pos, $le, $sep, $tab ; $($rest)*)
    };
    (@open_simple $r:ident, $stk:ident, $d:ident, $st:ident, $src:ident,
     $pos:ident, $le:ident, $sep:literal, $tab:literal ; ; $($rest:tt)*) => {
        $crate::parse_block!(@open_simple $r, $stk, $d, $st, $src, $pos, $le, $sep, $tab ; $($rest)*)
    };
    (@open_simple $r:ident, $stk:ident, $d:ident, $st:ident, $src:ident,
     $pos:ident, $le:ident, $sep:literal, $tab:literal ;) => {};


    // ------------------------------------------------------------------ //
    // @open_block: try to open one leaf `block` item (bullet / num).     //
    // Pushes a `(Type, Span)` entry; touches no stack frame.             //
    // ------------------------------------------------------------------ //
    (@open_block $res:ident, $st:ident, $src:ident,
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
        $crate::parse_block!(@open_block $res, $st, $src, $pos, $le, $sep, $tab ; $($rest)*)
    };

    (@open_block $res:ident, $st:ident, $src:ident,
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
        $crate::parse_block!(@open_block $res, $st, $src, $pos, $le, $sep, $tab ; $($rest)*)
    };

    (@open_block $r:ident, $st:ident, $src:ident,
     $pos:ident, $le:ident, $sep:literal, $tab:literal ; , $($rest:tt)*) => {
        $crate::parse_block!(@open_block $r, $st, $src, $pos, $le, $sep, $tab ; $($rest)*)
    };
    (@open_block $r:ident, $st:ident, $src:ident,
     $pos:ident, $le:ident, $sep:literal, $tab:literal ; ; $($rest:tt)*) => {
        $crate::parse_block!(@open_block $r, $st, $src, $pos, $le, $sep, $tab ; $($rest)*)
    };
    (@open_block $r:ident, $st:ident, $src:ident,
     $pos:ident, $le:ident, $sep:literal, $tab:literal ;) => {};
}
