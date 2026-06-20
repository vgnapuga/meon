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
//!               merge_flag, escape_byte, sep_byte, tab_byte, max_nest ; <rules>)
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
//!
//! # Bounded nesting (`max_nest`)
//!
//! Two rule kinds opt into multi-level tracking, sharing the grammar's
//! `max_nest` depth cap (forwarded from `parse_text!`; defaults to `1`).
//!
//! ## `asymmetric { balanced = …; parse_inside = …; … }`
//!
//! `balanced` and `parse_inside` are independent and both gate the bounded
//! stack — a rule needs only *one* of them `true` to be on it at all:
//!
//! - `balanced` controls *this type's* effective depth cap: the grammar's
//!   `max_nest` when `true` (so it can self-nest, `{ { } }`), or a hard `1`
//!   when `false` — a second open of the same type while one is already
//!   pending is then simply literal, exactly as the pre-nesting,
//!   `balanced = false` memchr path always treated it.
//! - `parse_inside` controls *opacity*, recorded per frame at push time
//!   (`!$api`), not as one engine-wide flag — different types nested inside
//!   each other can have different settings, and only the *innermost*
//!   currently-open frame's flag governs whether other rules fire: a
//!   transparent `{` containing an opaque `[` is transparent right up until
//!   execution actually enters the `[`, then opaque for its content, then
//!   transparent again once back outside it.
//!
//! Only a rule with **both** flags `false` ever runs the original,
//! untouched `if delim == $ao { … memchr/depth-search for $ac … }` block —
//! for any other rule that block is unreachable dead code, since the new
//! check (tried first, for every trigger byte) already intercepted and
//! `continue`d past it.
//!
//! The new check recognises both `$ao` and `$ac` for every stack-eligible
//! rule in the grammar (so different bracket types nest validly with each
//! other) using ordinary bracket matching, **one byte of the run at a
//! time** — `{{` is two separate open events, not one "count = 2" event,
//! because field routing for asymmetric is matched against the literal `1`
//! per character, not against the run's length (unlike `symmetric`, where
//! the run length itself picks the construct, e.g. `*` vs `**`). An open
//! byte pushes a placeholder span (`start == end`) into the rule's field,
//! back-patched on the matching close; this keeps the field sorted by
//! `start` even though an outer frame closes after its inner ones. Beyond a
//! type's effective cap, an extra same-type open increments a one-shot
//! overflow counter instead of pushing a frame — but only when
//! `balanced = true` for that type; `balanced = false` never accumulates
//! overflow, since its cap is 1 and a second same-type open is just
//! literal, full stop. A close byte that doesn't match the current top of
//! stack is left as a literal byte. A frame still open at line end is
//! discarded (`Vec::remove` at its index, not `truncate` — a same-type
//! self-nesting frame can close while an ancestor never does, leaving a
//! properly-closed inner entry at a *higher* index than the still-open
//! outer one, which must survive), exactly as an unclosed bracket used to
//! simply produce no span.
//!
//! **Required grammar change**: the close byte must be listed in the same
//! `on_trigger(...)` set as the open byte — `on_trigger(b'{', b'}')`, not
//! just `on_trigger(b'{')` — since the close is now found by the same scan
//! that finds the open, not by an internal forward search.
//!
//! ## `symmetric { parse_inside = true; balanced = true; … }`
//!
//! `parse_inside = false` and `parse_inside = true, balanced = false` are
//! **entirely unchanged** — both keep their original code paths verbatim.
//!
//! `parse_inside = true, balanced = true` replaces the single pending-slot
//! with a bounded stack of pending frames, shared across every such rule.
//! An occurrence whose `(byte, count)` matches the current top closes it;
//! otherwise, if there is room, it opens a new frame. This is what fixes a
//! real bug in the single-slot version: a *different*-count occurrence of
//! the same byte used to silently overwrite the one pending slot, losing
//! the outer delimiter — `**bold *italic* still-bold**` would never close
//! the bold. With the stack, the inner `*` (count 1) opens its own frame
//! instead of clobbering the outer `**` (count 2). Because open and close
//! look identical for a symmetric delimiter, an *identical* `(byte, count)`
//! pair still cannot self-nest — `**a **b** c**` resolves as two adjacent
//! runs, the same as a flat toggle. A frame still open at line end is
//! discarded, same as the asymmetric stack. Unlike asymmetric, the run
//! length here genuinely picks the construct (`*` vs `**` are different
//! fields), so it is matched as-is, never split byte-by-byte.
#[doc(hidden)]
#[macro_export]
macro_rules! parse_inline {
    ($state:ident, $src:ident, $start:expr, $le:expr,
     $tx:ident, $merge_il:tt, $esc:literal, $sep:literal, $tab:literal, $maxn:literal ;
     $($tail:tt)*) => {
        $crate::parse_inline!(
            @collect ($state, $src, $start, $le, $tx, $merge_il, $esc, $sep, $tab, $maxn)
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
    // Accumulation phase: collect all rule sections into typed buckets.  //
    // ------------------------------------------------------------------ //

    // hard_break rule
    (@collect ($st:ident, $src:ident, $s:expr, $le:expr,
               $tx:ident, $merge_il:tt, $esc:literal, $sep:literal, $tab:literal, $maxn:literal)
     (hard_break: )
     finders  = [$($fi:tt)*]
     sy_rules = [$($sr:tt)*]
     as_rules = [$($ar:tt)*]
     ch_rules = [$($cr:tt)*]
     kv_rules = [$($kv:tt)*]
     tail = [hard_break($hb_esc:literal, $sp:literal, $sp_min:literal) => $hb:ident ; $($rest:tt)*]
    ) => {
        $crate::parse_inline!(
            @collect ($st, $src, $s, $le, $tx, $merge_il, $esc, $sep, $tab, $maxn)
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
               $tx:ident, $merge_il:tt, $esc:literal, $sep:literal, $tab:literal, $maxn:literal)
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
            @collect ($st, $src, $s, $le, $tx, $merge_il, $esc, $sep, $tab, $maxn)
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
                  $tx:ident, $merge_il:tt, $esc:literal, $sep:literal, $tab:literal, $maxn:literal)
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
        $crate::parse_inline!(@body ($st, $src, $s, $le, $tx, $merge_il, $esc, $sep, $tab, $maxn)
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
    // Execution phase: the actual single-pass scan over one line.        //
    // ------------------------------------------------------------------ //

    (
        @body ($state:ident, $src:ident, $start:expr, $le:expr,
               $tx:ident, $merge_il:tt, $esc:literal, $sep:literal, $tab:literal, $maxn:literal)
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
        // Pending symmetric match for the original, untouched
        // parse_inside=true, balanced=false path: (byte, open_pos, open_count, depth).
        let mut pending: Option<(u8, u32, u32, u32)> = None;

        // Bounded stack for symmetric { parse_inside = true; balanced = true; … }.
        // Frame = (byte, count, vec_idx_in_field).
        let mut sym_frames: [(u8, u32, u32); $maxn] = [(0u8, 0u32, 0u32); $maxn];
        let mut sym_depth: u8 = 0u8;

        // Bounded stack for asymmetric rules with `balanced = true` and/or
        // `parse_inside = true` — either flag alone is enough to put a rule
        // on this stack; the two are independent:
        //
        // - `balanced` controls this *type's* effective depth cap: `$maxn`
        //   (the grammar-wide cap) when true, or a hard `1` when false — a
        //   second open of the same type while one is still pending is then
        //   simply literal, exactly as the original `balanced = false`
        //   memchr-based path always treated it.
        // - `parse_inside` controls *opacity*, stored per-frame (`!$api` at
        //   push time) rather than as one engine-wide flag, because
        //   different types can be nested inside each other with different
        //   settings — e.g. a transparent `{` containing an opaque `[`
        //   must still go opaque only once execution is actually inside the
        //   `[`, not for the `{`-only portion around it.
        //
        // Frame = (open_byte, close_byte, per_char_count, vec_idx_in_field,
        // is_opaque). `per_char_count` is always `1` — see the run-splitting
        // note below — stored anyway so the close / discard sides can
        // re-derive which `$af` field to touch via the same
        // `match … { $an => … }` arms the open side used: `$af` is bound
        // inside that inner repetition, so every access to it must stay
        // inside a matching `match`, never used bare.
        let mut asym_frames: [(u8, u8, u32, u32, bool); $maxn] =
            [(0u8, 0u8, 0u32, 0u32, false); $maxn];
        let mut asym_depth: u8 = 0u8;
        // One-shot overflow counter: extra same-type opens beyond the
        // *effective* cap for that type, so the real tracked frame's close
        // isn't mistaken early. Only ever increments when `balanced = true`
        // for the type at the top of stack — `balanced = false` types cap
        // at 1 and never accumulate overflow at all (a second same-type
        // open is just literal, full stop, matching pre-nesting behaviour).
        let mut asym_overflow: u32 = 0u32;

        // Two-phase transparent state for `chained` rules where either
        // component has `parse_inside = true`. The two components (text
        // bracket, url paren) are *sequential*, never simultaneous — phase
        // 2 only ever starts once phase 1 has fully closed — so a single
        // slot per phase is enough, no bounded array. Closing phase 1 never
        // commits anything by itself; it only enables attempting to open
        // phase 2. Only phase 2 actually closing pushes the combined
        // `$ch_ty` struct. This mirrors the original (and still-untouched,
        // for components with `parse_inside = false`) self-contained
        // two-phase search exactly — the only behavioural change introduced
        // by `parse_inside = true` is that other rules can now fire on the
        // bytes being scanned over, with the same orphan-on-failure
        // trade-off already accepted for `asymmetric`'s transparent mode.
        //
        // Scoped to a single active `chained` rule: if a grammar ever
        // declared more than one and two were mid-scan at once, they would
        // alias this same state. Every grammar seen so far declares exactly
        // one, so this is accepted as a documented limitation rather than
        // built out to a per-rule array.
        let mut ch_in_text: bool = false;
        let mut ch_text_opaque: bool = false;
        let mut ch_text_depth: i32 = 0;
        let mut ch_text_start: u32 = 0;
        let mut ch_is_prefix: bool = false;
        let mut ch_real_start: u32 = 0;

        let mut ch_in_url: bool = false;
        let mut ch_url_opaque: bool = false;
        let mut ch_url_depth: i32 = 0;
        let mut ch_url_start: u32 = 0;
        let mut ch_saved_text_end: u32 = 0;

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

            // -------------------------------------------------------------- //
            // asymmetric, balanced and/or transparent: tried first, for      //
            // every trigger byte, before chained/symmetric/the original      //
            // memchr-only asymmetric block below. A rule with both           //
            // `balanced = false` and `parse_inside = false` never matches    //
            // `$abal || $api` here, so this block does nothing for it and    //
            // the original block further down (unmodified) is the only       //
            // thing that ever runs for it.                                   //
            //                                                                //
            // Gated by `!_chained_opaque_active`: an opaque chained phase    //
            // (text or url component with `parse_inside = false`) isn't      //
            // part of asymmetric's shared stack/family, so its opacity must  //
            // suppress asymmetric's own cross-type recognition too — unlike  //
            // asymmetric's own opaque frames, which deliberately do *not*    //
            // suppress *other* asymmetric types (see the stack's own doc     //
            // comment for why that exception is scoped to same-family        //
            // nesting only).                                                 //
            // -------------------------------------------------------------- //
            let _chained_opaque_active =
                (ch_in_text && ch_text_opaque) || (ch_in_url && ch_url_opaque);

            let mut _asym_bal_handled = false;
            if !_chained_opaque_active {
                $(
                    if ($abal || $api) && delim == $ao {
                        if text_start < delim_start as usize {
                            if sym_depth == 0 && asym_depth == 0 && !ch_in_text && !ch_in_url {
                                push_il!($tx, $crate::span::Span::new(text_start as u32, delim_start));
                            };
                        }
                        // Effective depth cap for *this* type: the grammar-wide
                        // max_nest when it may self-nest, otherwise a hard 1.
                        let _cap: usize = if $abal { $maxn } else { 1usize };
                        // Each byte of the run is its own open event — `{{`
                        // is two opens, not one "count = 2" event. Field
                        // routing is matched against the literal `1`, never
                        // against the run's `count`, so an arm declared for
                        // any other count is unreachable for asymmetric.
                        // `text_start` advances *inside* the loop, one byte
                        // at a time, and only past a byte that actually did
                        // something (pushed a frame or counted as
                        // overflow) — a byte that did neither (cap reached
                        // for a different type, or no declared arm at all)
                        // leaves `text_start` exactly where it was, so it is
                        // picked up by a later flush instead of silently
                        // vanishing between two consumed neighbours.
                        for _k in 0..count {
                            let _char_pos = delim_start + _k;
                            let mut _consumed = false;
                            if (asym_depth as usize) < _cap {
                                match 1u32 {
                                    $( $an => {
                                        let _content_start = _char_pos + 1;
                                        let _vidx = $state.$af.len() as u32;
                                        push_il!($af, $crate::span::Span::new(
                                            _content_start, _content_start));
                                        asym_frames[asym_depth as usize] =
                                            ($ao, $ac, 1u32, _vidx, !$api);
                                        asym_depth += 1;
                                        asym_overflow = 0;
                                        _consumed = true;
                                    } )*
                                    _ => {}
                                }
                            } else if $abal && asym_depth > 0
                                && asym_frames[asym_depth as usize - 1].0 == $ao
                            {
                                asym_overflow += 1;
                                _consumed = true;
                            }
                            if _consumed {
                                text_start = (_char_pos + 1) as usize;
                            }
                            // Else literal: text_start stays put.
                        }
                        _asym_bal_handled = true;
                    } else if ($abal || $api) && delim == $ac {
                        for _k in 0..count {
                            let _close_char_pos = delim_start + _k;
                            let mut _consumed = false;
                            if asym_depth > 0
                                && asym_frames[asym_depth as usize - 1].1 == $ac
                            {
                                if asym_overflow > 0 {
                                    asym_overflow -= 1;
                                } else {
                                    let (_ob, _cb, _rc, _vidx, _op) =
                                        asym_frames[asym_depth as usize - 1];
                                    match _rc {
                                        $( $an => {
                                            $state.$af[_vidx as usize].end = _close_char_pos;
                                        } )*
                                        _ => {}
                                    }
                                    asym_depth -= 1;
                                    asym_overflow = 0;
                                }
                                _consumed = true;
                            }
                            if _consumed {
                                text_start = (_close_char_pos + 1) as usize;
                            }
                            // A close character that doesn't match the
                            // open top — empty stack, or a different type
                            // on top — is literal; text_start stays put.
                        }
                        _asym_bal_handled = true;
                    }
                )*
            }
            if _asym_bal_handled {
                continue;
            }

            let _asym_opaque_active =
                asym_depth > 0 && asym_frames[asym_depth as usize - 1].4;

            // -------------------------------------------------------------- //
            // chained, transparent phases: a rule with *either* component's  //
            // `parse_inside = true` uses this state-machine for *both*       //
            // phases (so phase 1 → phase 2 hand-off never needs to switch    //
            // mechanisms mid-match); each phase's own opacity is still       //
            // tracked independently in `ch_text_opaque` / `ch_url_opaque`,   //
            // resolved once at the moment that phase opens. A rule where     //
            // *both* components are `parse_inside = false` never matches     //
            // `$tpi || $upi` here, so this block does nothing for it and the //
            // original two-phase self-contained search further below         //
            // (unmodified) is the only thing that ever runs for it.          //
            //                                                                //
            // Closing the text phase never commits anything by itself — it   //
            // only attempts to open the url phase immediately. Only closing  //
            // the url phase actually pushes the combined `$ch_ty` struct.    //
            // If the byte right after the text close isn't `$uo`, the whole  //
            // match is abandoned and the opening bracket plus everything     //
            // scanned over is preserved as literal text — nothing committed. //
            // -------------------------------------------------------------- //
            let mut _chained_handled = false;
            $(
                if ($tpi || $upi) && !ch_in_text && !ch_in_url
                    && !_asym_opaque_active && delim == $co
                {
                    let _is_prefix = delim_start > 0
                        && src[delim_start as usize - 1] == $cp
                        && {
                            let mut _bs: u32 = 0;
                            let mut _i = delim_start as usize - 1;
                            while _i > $start && src[_i - 1] == $esc { _bs += 1; _i -= 1; }
                            _bs % 2 == 0
                        };
                    let _real_start = if _is_prefix {
                        delim_start as usize - 1
                    } else {
                        delim_start as usize
                    };
                    if text_start < _real_start {
                        if sym_depth == 0 && asym_depth == 0 && !ch_in_text && !ch_in_url {
                            push_il!($tx, $crate::span::Span::new(text_start as u32, _real_start as u32));
                        };
                    }
                    ch_in_text = true;
                    ch_text_opaque = !$tpi;
                    ch_text_depth = 0;
                    ch_text_start = pos as u32;
                    ch_is_prefix = _is_prefix;
                    ch_real_start = _real_start as u32;
                    text_start = pos;
                    _chained_handled = true;
                } else if ch_in_text && delim == $co && $tbal {
                    ch_text_depth += 1;
                    text_start = pos;
                    _chained_handled = true;
                } else if ch_in_text && delim == $cc {
                    if $tbal && ch_text_depth > 0 {
                        ch_text_depth -= 1;
                        text_start = pos;
                        _chained_handled = true;
                    } else {
                        let _ct_end = delim_start;
                        ch_in_text = false;
                        if pos < parse_end && src[pos] == $uo {
                            ch_in_url = true;
                            ch_url_opaque = !$upi;
                            ch_url_depth = 0;
                            ch_url_start = (pos + 1) as u32;
                            ch_saved_text_end = _ct_end;
                            pos += 1;
                            text_start = pos;
                            _chained_handled = true;
                        } else {
                            // No `(` right after `]` — abandon the match.
                            // Preserve the opening bracket and everything
                            // scanned over as literal text; nothing for
                            // this chained rule is committed.
                            if (ch_real_start as usize) < pos {
                                if sym_depth == 0 && asym_depth == 0 && !ch_in_text && !ch_in_url {
                                    push_il!($tx, $crate::span::Span::new(ch_real_start, pos as u32));
                                };
                            }
                            text_start = pos;
                            _chained_handled = true;
                        }
                    }
                } else if ch_in_url && delim == $uo && $ubal {
                    ch_url_depth += 1;
                    text_start = pos;
                    _chained_handled = true;
                } else if ch_in_url && delim == $uc {
                    if $ubal && ch_url_depth > 0 {
                        ch_url_depth -= 1;
                    } else {
                        let _cu_end = delim_start;
                        // Cleared *before* the flush below: this flush is
                        // for text before the whole chained match (mirrors
                        // any other construct's own opening flush), not
                        // content inside it — gating it on this match's
                        // own `ch_in_url` would be self-referential and
                        // wrongly suppress it. Only outer nesting
                        // (sym_depth / asym_depth) is the right signal here.
                        ch_in_url = false;
                        if text_start < ch_real_start as usize {
                            if sym_depth == 0 && asym_depth == 0 && !ch_in_text && !ch_in_url {
                                push_il!($tx, $crate::span::Span::new(text_start as u32, ch_real_start));
                            };
                        }
                        $state.$cf.push($ch_ty {
                            $cpi: ch_is_prefix,
                            $ct: $crate::span::Span::new(ch_text_start, ch_saved_text_end),
                            $cu: $crate::span::Span::new(ch_url_start, _cu_end),
                        });
                    }
                    text_start = pos;
                    _chained_handled = true;
                }
            )*
            if _chained_handled {
                continue;
            }

            if _asym_opaque_active || _chained_opaque_active {
                // The innermost active context (an asymmetric frame, or a
                // chained phase) is opaque — suppress everything else, same
                // as the pre-existing behaviour. A *transparent* innermost
                // context falls through instead, letting
                // chained/symmetric/key_value fire normally on its content.
                continue;
            }

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
                                    if sym_depth == 0 && asym_depth == 0 && !ch_in_text && !ch_in_url {
                                        push_il!($tx, $crate::span::Span::new(
                                        text_start as u32, real_start as u32));
                                    };
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
                        if $bal {
                            // Bounded stack — replaces the single pending
                            // slot for this rule's occurrences only.
                            let _matches_top = sym_depth > 0
                                && sym_frames[sym_depth as usize - 1].0 == $sb
                                && sym_frames[sym_depth as usize - 1].1 == count;

                            if _matches_top {
                                let (_b, _c, _vidx) = sym_frames[sym_depth as usize - 1];
                                let mut _closed = false;
                                match _c {
                                    $( $sn => {
                                        $state.$sf[_vidx as usize].end = delim_start;
                                        _closed = true;
                                    } )*
                                    _ => {}
                                }
                                if _closed {
                                    sym_depth -= 1;
                                    text_start = pos;
                                    continue;
                                } else {
                                    text_start = delim_start as usize;
                                    continue;
                                }
                            } else if (sym_depth as usize) < $maxn {
                                let mut _pushed = false;
                                match count {
                                    $( $sn => {
                                        if text_start < delim_start as usize {
                                            if sym_depth == 0 && asym_depth == 0 && !ch_in_text && !ch_in_url {
                                                push_il!($tx, $crate::span::Span::new(
                                                text_start as u32, delim_start));
                                            };
                                        }
                                        let _vidx = $state.$sf.len() as u32;
                                        push_il!($sf, $crate::span::Span::new(pos as u32, pos as u32));
                                        sym_frames[sym_depth as usize] = ($sb, count, _vidx);
                                        _pushed = true;
                                    } )*
                                    _ => {}
                                }
                                if _pushed {
                                    sym_depth += 1;
                                    text_start = pos;
                                    continue;
                                } else {
                                    if text_start < delim_start as usize {
                                        if sym_depth == 0 && asym_depth == 0 && !ch_in_text && !ch_in_url {
                                            push_il!($tx, $crate::span::Span::new(
                                            text_start as u32, delim_start));
                                        };
                                    }
                                    text_start = delim_start as usize;
                                    continue;
                                }
                            } else {
                                // Beyond cap
                                if text_start < delim_start as usize {
                                    if sym_depth == 0 && asym_depth == 0 && !ch_in_text && !ch_in_url {
                                        push_il!($tx, $crate::span::Span::new(
                                        text_start as u32, delim_start));
                                    };
                                }
                                text_start = delim_start as usize;
                                continue;
                            }
                        } else {
                            // Original single pending-slot mechanism — untouched.
                            if let Some((pb, op, oc, ref mut _depth)) = pending {
                                if pb == $sb && oc == count {
                                    if $bal && *_depth > 0 {
                                        *_depth -= 1;
                                        continue;
                                    }
                                    if (text_start as u32) < op {
                                        if sym_depth == 0 && asym_depth == 0 && !ch_in_text && !ch_in_url {
                                            push_il!($tx, $crate::span::Span::new(text_start as u32, op));
                                        };
                                    }
                                    let clean = $crate::span::Span::new(op + count, delim_start);
                                    match count { $( $sn => { push_il!($sf, clean); } )* _ => {} }
                                    text_start = pos;
                                    pending = None;
                                    continue;
                                }
                            }
                            pending = Some(($sb, delim_start, count, 0u32));
                        }
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
                                if sym_depth == 0 && asym_depth == 0 && !ch_in_text && !ch_in_url {
                                    push_il!($tx, $crate::span::Span::new(
                                    text_start as u32, delim_start));
                                };
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
                            if sym_depth == 0 && asym_depth == 0 && !ch_in_text && !ch_in_url {
                                push_il!($tx, $crate::span::Span::new(
                                text_start as u32, delim_start));
                            };
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
                        if sym_depth == 0 && asym_depth == 0 && !ch_in_text && !ch_in_url {
                            push_il!($tx, $crate::span::Span::new(text_start as u32, ks as u32));
                        };
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

        // ------------------------------------------------------------------ //
        // Discard any frame still open at line end on either stack — remove  //
        // its placeholder rather than leave a dangling, unpatched span.      //
        //                                                                    //
        // Asymmetric uses `remove`, not `truncate`: the same type can        //
        // self-nest (`{ { } }`), so a properly-closed inner frame can sit at //
        // a *higher* vec index than a still-open outer one — e.g. `{a {b} c` //
        // closes "b" (pushed second) before line end while the outer `{`     //
        // never finds its `}`. `truncate(outer_vidx)` would also delete the  //
        // already-finalised "b" entry sitting after it. `remove(vidx)`       //
        // deletes only that one placeholder; processing innermost-first      //
        // (this loop's order) means we always remove the highest still-open  //
        // index first, so no not-yet-processed vidx is invalidated by an     //
        // earlier removal shifting things underneath it.                     //
        //                                                                    //
        // Symmetric keeps `truncate`: an identical (byte, count) cannot      //
        // self-nest (occurrences of the same key always toggle, never open a //
        // second frame while one is pending — see the symmetric dispatch     //
        // above), so each field holds at most one pending placeholder at any //
        // time, and it is always the last entry in that field's Vec.         //
        // ------------------------------------------------------------------ //
        while asym_depth > 0 {
            asym_depth -= 1;
            let (_ob, _ocb, _orc, _ovidx, _oop) = asym_frames[asym_depth as usize];
            $(
                if ($abal || $api) && $ao == _ob {
                    match _orc {
                        $( $an => { $state.$af.remove(_ovidx as usize); } )*
                        _ => {}
                    }
                }
            )*
        }
        while sym_depth > 0 {
            sym_depth -= 1;
            let (_sob, _soc, _svidx) = sym_frames[sym_depth as usize];
            $(
                if $bal && $pi && $sb == _sob {
                    match _soc {
                        $( $sn => { $state.$sf.truncate(_svidx as usize); } )*
                        _ => {}
                    }
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
