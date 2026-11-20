//! Block-level element parser — the `parse_block!` macro.
//!
//! # Overview
//!
//! `parse_block!` handles the grammar's block rules: `fence` and `cont` frames,
//! which span lines, and `block` items — marker and `num` items — which are
//! one line each (fenced code, blockquotes and list items, for instance). It
//! is called by
//! `parse_text!` at the start of each line and, in a single call, fully
//! resolves that line's block structure:
//!
//! - **Peel phase**: every currently-open block frame is matched against the
//!   line from the outside in — continuation markers are consumed, a still-open
//!   fence either closes or swallows the line, and a frame whose marker is gone
//!   is closed (together with everything nested inside it).
//! - **Open phase**: at the position left after peeling, new block frames are
//!   opened (as many as nest on one line, e.g. a fence opening inside a
//!   `cont` frame), bounded by `max_nest`.
//!
//! It is not meant to be called by users directly.
//!
//! # Active block stack (`max_nest`)
//!
//! The active block state is a bounded stack `[(usize, usize); max_nest]` of
//! `(count, start)` frames plus a depth counter, sharing the grammar-wide
//! `max_nest` cap with the inline engine (see [`crate::parse_inline!`]). At
//! `max_nest = 1` it is a single slot: at most one block open at a time, no
//! block opening inside another.
//!
//! | Frame              | `count`                  | `start`                   |
//! |--------------------|--------------------------|---------------------------|
//! | Open fence         | opening run length, `≥ 1`| position of the run       |
//! | Continuation (`>`) | `0`                      | position of the marker    |
//!
//! The frame's byte is `src[start]`, and the rule that owns it is the one
//! that opened it: the first `cont` rule of that byte for a continuation,
//! the first `fence` rule whose pattern takes the byte and whose `min` the
//! count reaches for a fence. A frame is two `usize` values (16 bytes on
//! 64-bit), and a fence counts its run exactly at any length.
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
//!   `> >` (with `cont(b'>')`) is two genuinely nested frames.
//!
//! `block` items (marker and `num` items) are per-line leaves: they push nothing onto
//! the stack and so consume no depth. They may still open *inside* a `cont`,
//! but only when `max_nest > 1`; at `max_nest = 1` the open phase never runs
//! inside an already-open block.
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
//!   - `fence(pat, min = N) => field;` — fenced code blocks (e.g. ` ``` `): a
//!     run of at least `N` of one byte `pat` matches, with no further such
//!     byte on the line, opens; a run of at least as many of that same byte,
//!     followed by `sep`/`tab` only, closes — so with `b'`' | b'~'` a
//!     ` ``` ` block is not closed by `~~~`.
//!   - `cont(byte) => field;` — line-continuation blocks (e.g. blockquotes `>`).
//!
//! - `block { ... }` — single-line blocks with per-line metadata:
//!   - `(pat) |var|: Type { ... } => field;` — marker-prefixed items (bullet lists).
//!   - `num(digit_pat, end = end_pat) |n, k|: Type { ... } => field;` — numbered
//!     items (ordered lists): one to nine bytes `digit_pat` matches, each
//!     counting as its value less `b'0'` (wrapping for bytes other than ASCII
//!     digits), then one byte `end_pat` matches.
//!
//! Every `pat` is a Rust byte pattern, read with `matches!`: literals,
//! alternations, ranges, constants.
#[doc(hidden)]
#[macro_export]
macro_rules! parse_block {
    // A line whose `eol` comes right after its content: the next line starts
    // past it.
    (
        $stack:ident, $depth:ident, $state:ident, $src:ident, $pos:expr, $le:expr,
        sep = $sep:literal, tab = $tab:literal, max_nest = $maxn:literal ;
        block_simple { $($sr:tt)* }
        block        { $($br:tt)* }
    ) => {{
        let _le: usize = $le;
        $crate::parse_block!(
            $stack, $depth, $state, $src, $pos, _le,
            nl = if _le < $src.len() { _le + 1 } else { $src.len() },
            sep = $sep, tab = $tab, max_nest = $maxn,
            max_block = [], max_inline = [], para = _limits_unused,
            reparse = [], eol = b'\n' ;
            block_simple { $($sr)* }
            block        { $($br)* }
        )
    }};
    (
        $stack:ident, $depth:ident, $state:ident, $src:ident, $pos:expr, $le:expr,
        sep = $sep:literal, tab = $tab:literal, max_nest = $maxn:literal,
        max_block = $max_block:tt, max_inline = $max_inline:tt,
        para = $para:ident, reparse = $reparse:tt, eol = $eol:literal ;
        block_simple { $($sr:tt)* }
        block        { $($br:tt)* }
    ) => {{
        let _le: usize = $le;
        $crate::parse_block!(
            $stack, $depth, $state, $src, $pos, _le,
            nl = if _le < $src.len() { _le + 1 } else { $src.len() },
            sep = $sep, tab = $tab, max_nest = $maxn,
            max_block = $max_block, max_inline = $max_inline,
            para = $para, reparse = $reparse, eol = $eol ;
            block_simple { $($sr)* }
            block        { $($br)* }
        )
    }};
    (
        $stack:ident, $depth:ident, $state:ident, $src:ident, $pos:expr, $le:expr, nl = $nl:expr,
        sep = $sep:literal, tab = $tab:literal, max_nest = $maxn:literal,
        max_block = $max_block:tt, max_inline = $max_inline:tt,
        para = $para:ident, reparse = $reparse:tt, eol = $eol:literal ;
        block_simple { $($sr:tt)* }
        block        { $($br:tt)* }
    ) => {{
        let src: &[u8] = $src;
        macro_rules! __span {
            ($a:expr, $b:expr) => {
                $crate::span::Span::<usize>::from_usize($a as usize, $b as usize)
            };
            ($a:expr, $b:expr,) => {
                __span!($a, $b)
            };
        }
        let pos: usize = $pos;
        let le:  usize = ($le as usize).min($crate::span::Span::<usize>::MAX_INPUT_LEN);
        // Where the next line starts: past the line's `eol`, and past
        // whatever the caller counts as part of the line end before it.
        let nl:  usize = $nl;

        // ---- PEEL: walk the active frames outermost → innermost from `pos`. //
        let mut _cur: usize = pos;
        let mut _fi: usize = 0;
        let mut _peel_broke: bool = false;     // an outer cont closed → reprocess
        let mut _line_consumed: bool = false;  // a fence frame ate the whole line
        let mut _consumed_np: usize = 0;

        while _fi < $depth {
            let (_ac, _astart) = $stack[_fi];
            let _ab = src[_astart as usize];
            if _ac == 0 {
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
                        let (_c2, _s2) = $stack[_j];
                        $crate::parse_block!(@close_frame
                            $state, src, pos, _c2, _s2,
                            max_block = $max_block,
                            para = $para, reparse = $reparse, eol = $eol,
                            closed = false, contb = &[], contd = 0 ; $($sr)*);
                    }
                    $depth = _fi;
                    _peel_broke = true;
                    break;
                }
            } else {
                // fence frame (always the top): does this line close it?
                let mut _i = _cur;
                let mut _fc: usize = 0;
                while _i < le && src[_i] == _ab { _fc += 1; _i += 1; }
                if _fc >= _ac && src[_i..le].iter().all(|&b| b == $sep || b == $tab) {
                    let mut _cont_bytes: [u8; $maxn] = [0u8; $maxn];
                    let mut _cont_depth = 0usize;
                    while _cont_depth < $depth - 1 && $stack[_cont_depth].0 == 0 {
                        _cont_bytes[_cont_depth] = src[$stack[_cont_depth].1 as usize];
                        _cont_depth += 1;
                    }
                    $crate::parse_block!(@close_frame
                        $state, src, nl, _ac, _astart,
                        max_block = $max_block,
                        para = $para, reparse = $reparse, eol = $eol,
                        closed = true, contb = &_cont_bytes, contd = _cont_depth ; $($sr)*);
                    $depth -= 1;
                }
                // Closed or not, the fence line is fully consumed.
                _consumed_np = nl;
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
                            src, _open_cur, le, nl, $sep, $tab ; $($sr)*);
                    }
                    if let Some((_o, _cs)) = _or {
                        _opened_any = true;
                        _open_cur = _cs;
                        _go = true;          // a frame opened — try to nest further
                        continue;
                    }
                    // A leaf `block` item (marker / num) counts as a nesting
                    // level and is gated by the same `max_nest` cap as a
                    // frame: at `max_nest = 1` an item never opens inside an
                    // already-open block and the remainder goes to inline. At
                    // the top level (`depth == 0 < max_nest`) it always opens.
                    let mut _bres: ::core::option::Option<(bool, usize)> = None;
                    if $depth < $maxn {
                        $crate::parse_block!(@open_block _bres, $state,
                            src, _open_cur, le, $sep, $tab,
                            max_block = $max_block, para = $para ; $($br)*);
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
    // @close_frame: push the span for one `(count, start)` frame into the //
    // field of the rule that opened it — the first rule, in order, that  //
    // would open a frame of that byte and count.                         //
    // ------------------------------------------------------------------ //
    (@choose [], $normal:block, $limited:block) => {
        $normal
    };
    (@choose [$n:literal], $normal:block, $limited:block) => {
        $limited
    };
    (@over [], $s:expr, $e:expr) => {
        false
    };
    (@over [$n:literal], $s:expr, $e:expr) => {
        ((($e) as usize) - (($s) as usize)) > $n as usize
    };
    (@reparse [], $st:ident, $src:ident, $s:expr, $e:expr, $cb:expr, $cd:expr) => {};
    (@reparse [$f:ident], $st:ident, $src:ident, $s:expr, $e:expr, $cb:expr, $cd:expr) => {
        $f(&mut $st, $src, $s, $e, 0, $cb, $cd, true);
    };
    (@push_block [], $st:ident, $field:ident, $para:ident, $meta:expr, $span:expr) => {
        $crate::paste::paste! {
            $st.[<push_ $field>](($meta, $span));
        }
    };
    (@push_block [$n:literal], $st:ident, $field:ident, $para:ident, $meta:expr, $span:expr) => {{
        let _span = $span;
        if _span.len_usize() > $n as usize {
            $crate::paste::paste! {
                $st.[<push_ $para>](_span);
            }
        } else {
            $crate::paste::paste! {
                $st.[<push_ $field>](($meta, _span));
            }
        }
    }};
    (@push_simple_block [], $st:ident, $field:ident, $para:ident, $span:expr) => {
        $crate::paste::paste! {
            $st.[<push_ $field>]($span);
        }
    };
    (@push_simple_block [$n:literal], $st:ident, $field:ident, $para:ident, $span:expr) => {{
        let _span = $span;
        if _span.len_usize() > $n as usize {
            $crate::paste::paste! {
                $st.[<push_ $para>](_span);
            }
        } else {
            $crate::paste::paste! {
                $st.[<push_ $field>]($span);
            }
        }
    }};

    (@close_frame $st:ident, $src:ident, $endpos:expr, $count:expr, $start:expr,
        max_block = $max_block:tt, para = $para:ident, reparse = $reparse:tt,
        eol = $eol:literal, closed = $closed:ident,
        contb = $contb:expr, contd = $contd:expr ;
        $($sr:tt)*
    ) => {{
        let _fcount: usize = $count;
        let _fstart: usize = $start;
        let _fbyte: u8 = $src[_fstart as usize];
        let mut _fdone = false;
        $crate::parse_block!(@close_frame_inner
            $st, $src, _fdone, $endpos, _fcount, _fbyte, _fstart,
            max_block = $max_block, para = $para, reparse = $reparse,
            eol = $eol, closed = $closed, contb = $contb, contd = $contd ; $($sr)*);
    }};
    (@close_frame_inner $st:ident, $src:ident, $done:ident, $endpos:expr, $count:ident, $byte:ident, $start:ident,
        max_block = $max_block:tt, para = $para:ident, reparse = $reparse:tt,
        eol = $eol:literal, closed = $closed:ident,
        contb = $contb:expr, contd = $contd:expr ;
        cont($cb:literal) => $field:ident $($rest:tt)*
    ) => {
        if !$done && $count == 0 && $byte == $cb {
            $done = true;
            let _span = __span!($start, $endpos);
            $crate::parse_block!(@push_simple_block $max_block, $st, $field, $para, _span);
        }
        $crate::parse_block!(@close_frame_inner $st, $src, $done, $endpos, $count, $byte, $start,
            max_block = $max_block, para = $para, reparse = $reparse,
            eol = $eol, closed = $closed, contb = $contb, contd = $contd ; $($rest)*)
    };
    (@close_frame_inner $st:ident, $src:ident, $done:ident, $endpos:expr, $count:ident, $byte:ident, $start:ident,
        max_block = $max_block:tt, para = $para:ident, reparse = $reparse:tt,
        eol = $eol:literal, closed = $closed:ident,
        contb = $contb:expr, contd = $contd:expr ;
        fence($pat:pat, min = $min:literal) => $field:ident $($rest:tt)*
    ) => {
        if !$done && $count != 0 && matches!($byte, $pat) && $count >= $min {
            $done = true;
            let _span = __span!($start, $endpos);
            $crate::parse_block!(@push_simple_block $max_block, $st, $field, $para, _span);
            $crate::parse_block!(@choose $max_block, {}, {
            if $crate::parse_block!(@over $max_block, _span.start, _span.end) {
                let _cs = {
                    let mut _i = _span.start as usize;
                    let _end = _span.end as usize;
                    while _i < _end && $src[_i] != $eol {
                        _i += 1;
                    }
                    if _i < _end { _i + 1 } else { _end }
                };
                let _ce = if $closed {
                    let mut _i = (_span.end as usize).saturating_sub(1);
                    while _i > _cs && $src[_i - 1] != $eol {
                        _i -= 1;
                    }
                    _i
                } else {
                    _span.end as usize
                };
                if _cs < _ce {
                    $crate::parse_block!(@reparse $reparse, $st, $src, _cs, _ce, $contb, $contd);
                }
            }
            });
        }
        $crate::parse_block!(@close_frame_inner $st, $src, $done, $endpos, $count, $byte, $start,
            max_block = $max_block, para = $para, reparse = $reparse,
            eol = $eol, closed = $closed, contb = $contb, contd = $contd ; $($rest)*)
    };
    (@close_frame_inner $st:ident, $src:ident, $done:ident, $e:expr, $c:ident, $b:ident, $s:ident,
        max_block = $max_block:tt, para = $para:ident, reparse = $reparse:tt,
        eol = $eol:literal, closed = $closed:ident,
        contb = $contb:expr, contd = $contd:expr ; , $($r:tt)*) => {
        $crate::parse_block!(@close_frame_inner $st, $src, $done, $e, $c, $b, $s,
            max_block = $max_block, para = $para, reparse = $reparse,
            eol = $eol, closed = $closed, contb = $contb, contd = $contd ; $($r)*)
    };
    (@close_frame_inner $st:ident, $src:ident, $done:ident, $e:expr, $c:ident, $b:ident, $s:ident,
        max_block = $max_block:tt, para = $para:ident, reparse = $reparse:tt,
        eol = $eol:literal, closed = $closed:ident,
        contb = $contb:expr, contd = $contd:expr ; ; $($r:tt)*) => {
        $crate::parse_block!(@close_frame_inner $st, $src, $done, $e, $c, $b, $s,
            max_block = $max_block, para = $para, reparse = $reparse,
            eol = $eol, closed = $closed, contb = $contb, contd = $contd ; $($r)*)
    };
    (@close_frame_inner $st:ident, $src:ident, $done:ident, $e:expr, $c:ident, $b:ident, $s:ident,
        max_block = $max_block:tt, para = $para:ident, reparse = $reparse:tt,
        eol = $eol:literal, closed = $closed:ident,
        contb = $contb:expr, contd = $contd:expr ;) => {
        let _ = $done;
    };


    // ------------------------------------------------------------------ //
    // @open_simple: try to open one fence/cont frame, pushing onto the   //
    // stack. The depth-vs-max_nest gate is applied by the caller.        //
    // ------------------------------------------------------------------ //
    (@open_simple $res:ident, $stack:ident, $depth:ident, $st:ident, $src:ident,
     $pos:ident, $le:ident, $nl:ident, $sep:literal, $tab:literal ;
     fence($pat:pat, min = $min:literal) => $field:ident $($rest:tt)*
    ) => {
        if $res.is_none() && $pos < $le && matches!($src[$pos], $pat) {
            let _byte = $src[$pos];
            let mut _i: usize = $pos;
            let mut _c: usize = 0;
            while _i < $le && $src[_i] == _byte { _c += 1; _i += 1; }
            if _c >= $min && $src[_i..$le].iter().all(|&b| b != _byte) {
                $stack[$depth] = (_c as usize, $pos);
                $depth += 1;
                $res = Some((true, $nl));
            }
        }
        $crate::parse_block!(@open_simple $res, $stack, $depth, $st, $src, $pos, $le, $nl, $sep, $tab ; $($rest)*)
    };

    (@open_simple $res:ident, $stack:ident, $depth:ident, $st:ident, $src:ident,
     $pos:ident, $le:ident, $nl:ident, $sep:literal, $tab:literal ;
     cont($byte:literal) => $field:ident $($rest:tt)*
    ) => {
        if $res.is_none() && $pos < $le && $src[$pos] == $byte {
            $stack[$depth] = (0, $pos);
            $depth += 1;
            let cs = if $pos + 1 < $le
                && ($src[$pos + 1] == $sep || $src[$pos + 1] == $tab)
            { $pos + 2 } else { $pos + 1 };
            $res = Some((true, cs));
        }
        $crate::parse_block!(@open_simple $res, $stack, $depth, $st, $src, $pos, $le, $nl, $sep, $tab ; $($rest)*)
    };

    (@open_simple $r:ident, $stk:ident, $d:ident, $st:ident, $src:ident,
     $pos:ident, $le:ident, $nl:ident, $sep:literal, $tab:literal ; , $($rest:tt)*) => {
        $crate::parse_block!(@open_simple $r, $stk, $d, $st, $src, $pos, $le, $nl, $sep, $tab ; $($rest)*)
    };
    (@open_simple $r:ident, $stk:ident, $d:ident, $st:ident, $src:ident,
     $pos:ident, $le:ident, $nl:ident, $sep:literal, $tab:literal ; ; $($rest:tt)*) => {
        $crate::parse_block!(@open_simple $r, $stk, $d, $st, $src, $pos, $le, $nl, $sep, $tab ; $($rest)*)
    };
    (@open_simple $r:ident, $stk:ident, $d:ident, $st:ident, $src:ident,
     $pos:ident, $le:ident, $nl:ident, $sep:literal, $tab:literal ;) => {};


    // ------------------------------------------------------------------ //
    // @open_block: try to open one leaf `block` item (marker / num).     //
    // Pushes a `(Type, Span)` entry; touches no stack frame.             //
    // ------------------------------------------------------------------ //
    (@open_block $res:ident, $st:ident, $src:ident,
     $pos:ident, $le:ident, $sep:literal, $tab:literal,
     max_block = $max_block:tt, para = $para:ident ;
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
                    let _span = __span!(cs as u32, $le as u32);
                    $crate::parse_block!(@push_block $max_block, $st, $field, $para, _meta, _span);
                    $res = Some((true, cs));
                }
            }
        }
        $crate::parse_block!(@open_block $res, $st, $src, $pos, $le, $sep, $tab,
            max_block = $max_block, para = $para ; $($rest)*)
    };

    (@open_block $res:ident, $st:ident, $src:ident,
     $pos:ident, $le:ident, $sep:literal, $tab:literal,
     max_block = $max_block:tt, para = $para:ident ;
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
                    // Each byte counts as its value less `b'0'`: the
                    // number itself for ASCII digits, wrapping otherwise.
                    _num = _num
                        .wrapping_mul(10)
                        .wrapping_add($src[_i].wrapping_sub(b'0') as u32);
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
                        let _span = __span!(cs as u32, $le as u32);
                        $crate::parse_block!(@push_block $max_block, $st, $field, $para, _meta, _span);
                        $res = Some((true, cs));
                    }
                }
            }
        }
        $crate::parse_block!(@open_block $res, $st, $src, $pos, $le, $sep, $tab,
            max_block = $max_block, para = $para ; $($rest)*)
    };

    (@open_block $r:ident, $st:ident, $src:ident,
     $pos:ident, $le:ident, $sep:literal, $tab:literal,
     max_block = $max_block:tt, para = $para:ident ; , $($rest:tt)*) => {
        $crate::parse_block!(@open_block $r, $st, $src, $pos, $le, $sep, $tab,
            max_block = $max_block, para = $para ; $($rest)*)
    };
    (@open_block $r:ident, $st:ident, $src:ident,
     $pos:ident, $le:ident, $sep:literal, $tab:literal,
     max_block = $max_block:tt, para = $para:ident ; ; $($rest:tt)*) => {
        $crate::parse_block!(@open_block $r, $st, $src, $pos, $le, $sep, $tab,
            max_block = $max_block, para = $para ; $($rest)*)
    };
    (@open_block $r:ident, $st:ident, $src:ident,
     $pos:ident, $le:ident, $sep:literal, $tab:literal,
     max_block = $max_block:tt, para = $para:ident ;) => {};

}
