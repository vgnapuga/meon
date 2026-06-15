//! Inline element parser — the `parse_inline!` macro.
//!
//! # Overview
//!
//! `parse_inline!` drives a single-pass scan over one logical line of source
//! text and emits spans for every inline element it recognises: emphasis,
//! code spans, links, images, autolinks, key-value pairs and hard breaks.
//!
//! It is invoked by `parse_text!` whenever a non-block byte is found on the
//! current line, and is not meant to be called by users directly.
//!
//! # Grammar integration
//!
//! The macro consumes the `inline { … }` section of a `define_parser!`
//! grammar after `strip` has removed the `=> field [N]` annotations.  The
//! entry point is:
//!
//! ```text
//! parse_inline!(state, src, start, line_end, fallback_field,
//!               merge_flag, escape_byte, sep_byte, tab_byte ; <rules>)
//! ```
//!
//! Rules inside the `inline` section are:
//!
//! - `merge_simple = true | false;`  — whether adjacent text spans are merged.
//! - `fallback => field;`            — field that receives plain-text spans.
//! - `hard_break(esc, sp, min) => field;` — trailing hard-break detection.
//! - `on_trigger(b1, …) { <inline rules> }` — byte-triggered inline block
//!   (replaces the old `memchr(…) { … }` syntax).
//!
//! # `on_trigger` dispatch
//!
//! `on_trigger(b1, b2, …)` declares a set of *trigger bytes*. When the
//! scanner finds any of those bytes in the current line it enters the block
//! and tries each rule in order:
//!
//! - `symmetric byte { … }` — paired delimiters with the same open/close byte.
//! - `asymmetric open, close { … }` — paired delimiters with different bytes.
//! - `chained: Type { … }` — two-part delimiters (e.g. `[text](url)`).
//! - `key_value: Type { … }` — `key = value` pairs.
//!
//! The trigger set is searched with [`crate::swar::find_any`], which
//! dispatches to `memchr` / `memchr2` / `memchr3` for 1–3 bytes and to the
//! SWAR/SIMD engine for 4 or more bytes.
#[doc(hidden)]
#[macro_export]
macro_rules! parse_inline {
    ($state:ident, $src:ident, $start:expr, $le:expr,
     $tx:ident, $merge_il:tt, $esc:literal, $sep:literal, $tab:literal ; $($tail:tt)*) => {
        $crate::parse_inline!(
            @collect ($state, $src, $start, $le, $tx, $merge_il, $esc, $sep, $tab)
            (hard_break: )
            finders  = []
            sy_rules = []
            as_rules = []
            ch_rules = []
            kv_rules = []
            tail = [$($tail)*]
        )
    };

    // ------------------------------------------------------------------ //
    // Accumulation phase: collect all rule sections into typed buckets.   //
    // ------------------------------------------------------------------ //

    // hard_break rule
    (@collect ($st:ident, $src:ident, $s:expr, $le:expr,
               $tx:ident, $merge_il:tt, $esc:literal, $sep:literal, $tab:literal)
     (hard_break: )
     finders  = [$($fi:tt)*]
     sy_rules = [$($sr:tt)*]
     as_rules = [$($ar:tt)*]
     ch_rules = [$($cr:tt)*]
     kv_rules = [$($kv:tt)*]
     tail = [hard_break($hb_esc:literal, $sp:literal, $sp_min:literal) => $hb:ident ; $($rest:tt)*]
    ) => {
        $crate::parse_inline!(
            @collect ($st, $src, $s, $le, $tx, $merge_il, $esc, $sep, $tab)
            (hard_break: $hb_esc, $sp, $sp_min => $hb)
            finders  = [$($fi)*]
            sy_rules = [$($sr)*]
            as_rules = [$($ar)*]
            ch_rules = [$($cr)*]
            kv_rules = [$($kv)*]
            tail = [$($rest)*]
        )
    };

    // on_trigger(...) { ... } — new canonical name for the byte-trigger block.
    // Replaces the old `memchr(…) { … }` syntax; semantics are identical.
    (@collect ($st:ident, $src:ident, $s:expr, $le:expr,
               $tx:ident, $merge_il:tt, $esc:literal, $sep:literal, $tab:literal)
     (hard_break: $($hb:tt)*)
     finders  = [$($fi:tt)*]
     sy_rules = [$($sr:tt)*]
     as_rules = [$($ar:tt)*]
     ch_rules = [$($cr:tt)*]
     kv_rules = [$($kv:tt)*]
     tail = [
         on_trigger($($fn_b:literal),+) {
             $( symmetric $sb:literal {
                 parse_inside = $pi:ident ;
                 balanced     = $bal:ident ;
                 $( $sn:tt => $sf:ident ),* $(,)?
             } )*
             $( asymmetric $ao:literal, $ac:literal {
                 balanced     = $abal:ident ;
                 parse_inside = $api:ident ;
                 $( $an:tt => $af:ident ),* $(,)?
             } )*
             $( chained: $ch_ty:ident {
                 | $co:literal, $cc:literal | {
                     parse_inside = $tpi:ident ;
                     balanced     = $tbal:ident ;
                 } => $ct:ident,
                 | $uo:literal, $uc:literal | {
                     parse_inside = $upi:ident ;
                     balanced     = $ubal:ident ;
                 } => $cu:ident,
                 prefix | $cp:literal | => $cpi:ident,
             } => $cf:ident )*
             $( key_value: $kv_ty:ident {
                 eq        = $kv_eq:literal ;
                 allow_sep = $kv_allow:ident ;
                 end       = $kv_end:literal ;
                 key       => $kv_kf:ident ,
                 value     => $kv_vf:ident ,
             } => $kv_f:ident )*
         }
         $($rest:tt)*
     ]
    ) => {
        $crate::parse_inline!(
            @collect ($st, $src, $s, $le, $tx, $merge_il, $esc, $sep, $tab)
            (hard_break: $($hb)*)
            finders  = [$($fi)* { $($fn_b),+ }]
            sy_rules = [$($sr)* $( ($sb, $pi, $bal, { $( $sn => $sf ),* }) )*]
            as_rules = [$($ar)* $( ($ao, $ac, $abal, $api, { $( $an => $af ),* }) )*]
            ch_rules = [$($cr)* $( ($co, $cc, $tpi, $tbal, $uo, $uc, $upi, $ubal, $cp, $cpi => $ct, $cu, $ch_ty, $cf) )*]
            kv_rules = [$($kv)* $( ($kv_eq, $kv_allow, $kv_end, $kv_kf, $kv_vf, $kv_ty, $kv_f) )*]
            tail = [$($rest)*]
        )
    };

    // Transition: all sections consumed — flatten buckets and enter @body.
    (
        @collect ($st:ident, $src:ident, $s:expr, $le:expr,
                  $tx:ident, $merge_il:tt, $esc:literal, $sep:literal, $tab:literal)
        (hard_break: $($hb:tt)*)
        finders  = [$($fi:tt)*]
        sy_rules = [$( ($sb:literal, $pi:tt, $bal:tt, { $( $sn:tt => $sf:ident ),* }) )*]
        as_rules = [$( ($ao:literal, $ac:literal, $abal:tt, $api:tt, { $( $an:tt => $af:ident ),* }) )*]
        ch_rules = [$( ($co:literal, $cc:literal, $tpi:tt, $tbal:tt, $uo:literal, $uc:literal,
                        $upi:tt, $ubal:tt, $cp:literal, $cpi:ident => $ct:ident, $cu:ident,
                        $ch_ty:ident, $cf:ident) )*]
        kv_rules = [$( ($kv_eq:literal, $kv_allow:tt, $kv_end:literal,
                        $kv_kf:ident, $kv_vf:ident, $kv_ty:ident, $kv_f:ident) )*]
        tail = []
    ) => {
        $crate::parse_inline!(@body ($st, $src, $s, $le, $tx, $merge_il, $esc, $sep, $tab)
            (hard_break: $($hb)*)
            finders  = [$($fi)*]
            sy_rules = [$( $sb, $pi, $bal, { $( $sn => $sf ),* } )*]
            as_rules = [$( $ao, $ac, $abal, $api, { $( $an => $af ),* } )*]
            ch_rules = [$( $co, $cc, $tpi, $tbal, $uo, $uc, $upi, $ubal, $cp,
                           $cpi => $ct, $cu, $ch_ty, $cf )*]
            kv_rules = [$( $kv_eq, $kv_allow, $kv_end, $kv_kf, $kv_vf, $kv_ty, $kv_f )*]
        )
    };

    // ------------------------------------------------------------------ //
    // Execution phase: the actual single-pass scan over one line.         //
    // ------------------------------------------------------------------ //

    (
        @body ($state:ident, $src:ident, $start:expr, $le:expr,
               $tx:ident, $merge_il:tt, $esc:literal, $sep:literal, $tab:literal)
        (hard_break: $($hb_esc:literal, $sp:literal, $sp_min:literal => $hb:ident)*)
        finders  = [$( { $($fn_b:literal),+ } )*]
        sy_rules = [$( $sb:literal, $pi:tt, $bal:tt, { $( $sn:tt => $sf:ident ),* } )*]
        as_rules = [$( $ao:literal, $ac:literal, $abal:tt, $api:tt, { $( $an:tt => $af:ident ),* } )*]
        ch_rules = [$( $co:literal, $cc:literal, $tpi:tt, $tbal:tt, $uo:literal, $uc:literal,
                       $upi:tt, $ubal:tt, $cp:literal, $cpi:ident => $ct:ident, $cu:ident,
                       $ch_ty:ident, $cf:ident )*]
        kv_rules = [$( $kv_eq:literal, $kv_allow:tt, $kv_end:literal,
                       $kv_kf:ident, $kv_vf:ident, $kv_ty:ident, $kv_f:ident )*]
    ) => {{
        let src: &[u8] = $src;
        let len = src.len();
        let mut parse_end: usize = $le;

        macro_rules! push_il {
            ($field:ident, $span:expr) => {
                $crate::parse_text!(@dispatch $state, $field, $span, $merge_il)
            };
        }

        // Hard-break detection: trim trailing spaces / backslash before
        // processing the rest of the line.
        let _hb = 'hb: {
            $(
                if parse_end > $start {
                    if src[parse_end - 1] == $hb_esc { parse_end -= 1; break 'hb true; }
                    let mut _n: u32 = 0;
                    while parse_end > $start && src[parse_end - 1] == $sp {
                        _n += 1; parse_end -= 1;
                    }
                    if _n >= $sp_min { break 'hb true; }
                }
            )*
            false
        };

        let mut pos: usize = $start;
        let mut text_start: usize = $start;
        // Pending symmetric match: (byte, open_pos, open_count, depth).
        let mut pending: Option<(u8, u32, u32, u32)> = None;

        loop {
            // Find the next trigger byte using find_any, which dispatches to
            // memchr/memchr2/memchr3 for 1-3 bytes and to SWAR/SIMD for ≥ 4.
            let found: Option<usize> = 'find: {
                let mut best: Option<usize> = None;
                $(
                    $crate::parse_inline!(@do_find $($fn_b),+ ; src, pos, parse_end, best);
                )*
                break 'find best;
            };

            let Some(rel) = found else { break };
            pos += rel;

            // Skip escaped delimiters (odd number of preceding backslashes).
            if pos > $start {
                let mut _bs: u32 = 0;
                let mut _i = pos;
                while _i > $start && src[_i - 1] == $esc { _bs += 1; _i -= 1; }
                if _bs % 2 == 1 { pos += 1; continue; }
            }

            let delim: u8 = src[pos];
            let delim_start: u32 = pos as u32;
            let mut count: u32 = 0;
            while pos < parse_end && src[pos] == delim { count += 1; pos += 1; }

            // --- chained (e.g. [text](url), ![img](url)) ---
            $(
                if delim == $co {
                    let is_prefix = delim_start > 0
                        && src[delim_start as usize - 1] == $cp
                        && {
                            let mut _bs: u32 = 0;
                            let mut _i = delim_start as usize - 1;
                            while _i > $start && src[_i - 1] == $esc { _bs += 1; _i -= 1; }
                            _bs % 2 == 0
                        };
                    let mut _i = pos;
                    let close_text: Option<usize> = if $tbal {
                        let mut _depth: i32 = 1;
                        let mut _found: Option<usize> = None;
                        while _i < parse_end {
                            if src[_i] == $co { _depth += 1; }
                            else if src[_i] == $cc {
                                _depth -= 1;
                                if _depth == 0 { _found = Some(_i); break; }
                            }
                            _i += 1;
                        }
                        _found
                    } else {
                        let mut _found: Option<usize> = None;
                        while _i < parse_end {
                            if src[_i] == $cc { _found = Some(_i); break; }
                            _i += 1;
                        }
                        _found
                    };
                    if let Some(ct_end) = close_text {
                        let next = ct_end + 1;
                        if next < parse_end && src[next] == $uo {
                            let mut _j = next + 1;
                            let close_url: Option<usize> = if $ubal {
                                let mut _depth: i32 = 1;
                                let mut _found: Option<usize> = None;
                                while _j < parse_end {
                                    if src[_j] == $uo { _depth += 1; }
                                    else if src[_j] == $uc {
                                        _depth -= 1;
                                        if _depth == 0 { _found = Some(_j); break; }
                                    }
                                    _j += 1;
                                }
                                _found
                            } else {
                                let mut _found: Option<usize> = None;
                                while _j < parse_end {
                                    if src[_j] == $uc { _found = Some(_j); break; }
                                    _j += 1;
                                }
                                _found
                            };
                            if let Some(cu_end) = close_url {
                                let real_start = if is_prefix {
                                    delim_start as usize - 1
                                } else {
                                    delim_start as usize
                                };
                                if text_start < real_start {
                                    push_il!($tx, $crate::span::Span::new(
                                        text_start as u32, real_start as u32));
                                }
                                $state.$cf.push($ch_ty {
                                    $cpi: is_prefix,
                                    $ct:  $crate::span::Span::new(pos as u32, ct_end as u32),
                                    $cu:  $crate::span::Span::new((next + 1) as u32, cu_end as u32),
                                });
                                pos = cu_end + 1;
                                text_start = pos;
                                continue;
                            }
                        }
                    }
                    continue;
                }
            )*

            // --- symmetric (e.g. *italic*, **bold**, `code`) ---
            $(
                if delim == $sb {
                    if $pi {
                        if let Some((pb, op, oc, ref mut _depth)) = pending {
                            if pb == $sb && oc == count {
                                if $bal && *_depth > 0 {
                                    *_depth -= 1;
                                    continue;
                                }
                                if (text_start as u32) < op {
                                    push_il!($tx, $crate::span::Span::new(text_start as u32, op));
                                }
                                let clean = $crate::span::Span::new(op + count, delim_start);
                                match count { $( $sn => { push_il!($sf, clean); } )* _ => {} }
                                text_start = pos;
                                pending = None;
                                continue;
                            }
                        }
                        pending = Some(($sb, delim_start, count, 0u32));
                    } else {
                        let cs = pos;
                        let mut _i = pos;
                        let close: Option<(usize, usize)> = if $bal {
                            let mut _found: Option<(usize, usize)> = None;
                            loop {
                                match $crate::memchr::memchr($sb, &src[_i..parse_end]) {
                                    None => break,
                                    Some(r) => {
                                        let p = _i + r;
                                        let mut c: u32 = 0;
                                        let mut tmp = p;
                                        while tmp < parse_end && src[tmp] == $sb {
                                            c += 1; tmp += 1;
                                        }
                                        if c == count * 2 {
                                            _i = tmp;
                                        } else if c == count {
                                            _found = Some((p, tmp));
                                            break;
                                        } else {
                                            _i = tmp;
                                        }
                                    }
                                }
                            }
                            _found
                        } else {
                            let mut _found: Option<(usize, usize)> = None;
                            loop {
                                match $crate::memchr::memchr($sb, &src[_i..parse_end]) {
                                    None => break,
                                    Some(r) => {
                                        let p = _i + r;
                                        let mut c: u32 = 0;
                                        let mut tmp = p;
                                        while tmp < parse_end && src[tmp] == $sb {
                                            c += 1; tmp += 1;
                                        }
                                        if c == count { _found = Some((p, tmp)); break; }
                                        _i = tmp;
                                    }
                                }
                            }
                            _found
                        };
                        if let Some((p, end)) = close {
                            if text_start < delim_start as usize {
                                push_il!($tx, $crate::span::Span::new(
                                    text_start as u32, delim_start));
                            }
                            let clean = $crate::span::Span::new(cs as u32, p as u32);
                            match count { $( $sn => { push_il!($sf, clean); } )* _ => {} }
                            pos = end;
                            text_start = end;
                        }
                    }
                    continue;
                }
            )*

            // --- asymmetric (e.g. <autolink>) ---
            $(
                if delim == $ao {
                    let cs = pos;
                    let close_pos: Option<usize> = if $abal {
                        let mut depth: usize = 1;
                        let mut _i = pos;
                        let mut found = None;
                        while _i < parse_end {
                            if src[_i] == $ao { depth += 1; }
                            else if src[_i] == $ac {
                                depth -= 1;
                                if depth == 0 { found = Some(_i); break; }
                            }
                            _i += 1;
                        }
                        found
                    } else {
                        $crate::memchr::memchr($ac, &src[pos..parse_end]).map(|r| pos + r)
                    };
                    if let Some(cp) = close_pos {
                        if text_start < delim_start as usize {
                            push_il!($tx, $crate::span::Span::new(
                                text_start as u32, delim_start));
                        }
                        let clean = $crate::span::Span::new(cs as u32, cp as u32);
                        match count { $( $an => { push_il!($af, clean); } )* _ => {} }
                        pos = cp + 1;
                        text_start = pos;
                    }
                    continue;
                }
            )*

            // --- key_value (e.g. key = value) ---
            $(
                if delim == $kv_eq {
                    let mut key_end = delim_start as usize;
                    if $kv_allow {
                        while key_end > text_start && src[key_end - 1] == $sep {
                            key_end -= 1;
                        }
                    }
                    let mut ks = key_end;
                    while ks > text_start
                        && src[ks - 1] != $sep
                        && src[ks - 1] != $tab
                    {
                        ks -= 1;
                    }
                    let mut val_start = pos;
                    if $kv_allow {
                        while val_start < parse_end && src[val_start] == $sep {
                            val_start += 1;
                        }
                    }
                    let val_end = $crate::memchr::memchr($kv_end, &src[val_start..parse_end])
                        .map(|i| val_start + i)
                        .unwrap_or(parse_end);
                    let _adv = if val_end < parse_end { 1usize } else { 0usize };
                    if text_start < ks {
                        push_il!($tx, $crate::span::Span::new(text_start as u32, ks as u32));
                    }
                    $state.$kv_f.push($kv_ty {
                        $kv_kf: $crate::span::Span::new(ks as u32,        key_end as u32),
                        $kv_vf: $crate::span::Span::new(val_start as u32, val_end  as u32),
                    });
                    pos        = val_end + _adv;
                    text_start = pos;
                    continue;
                }
            )*
        }

        // Flush any remaining plain text before the line end.
        if text_start < parse_end {
            push_il!($tx, $crate::span::Span::new(text_start as u32, parse_end as u32));
        }
        // Emit hard-break marker if detected.
        $( if _hb {
            $state.$hb.push($crate::span::Span::new(parse_end as u32, parse_end as u32));
        } )*

        if $le < len { $le + 1 } else { len }
    }};

    // ------------------------------------------------------------------ //
    // @do_find: single trigger-set search via find_any.                  //
    //                                                                    //
    // Calls find_any with an array literal of the trigger bytes.         //
    // find_any dispatches internally:                                    //
    //   N=1 → memchr, N=2 → memchr2, N=3 → memchr3, N≥4 → SWAR/SIMD.     //
    // ------------------------------------------------------------------ //
    (@do_find $($b:literal),+ ; $src:ident, $pos:ident, $pe:ident, $best:ident) => {
        if let Some(r) = $crate::swar::find_any([$($b),+], &$src[$pos..$pe]) {
            $best = Some(match $best { Some(cur) if cur <= r => cur, _ => r });
        }
    };
}
