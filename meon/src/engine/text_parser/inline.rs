//! Inline element parser — the `parse_inline!` macro.
//!
//! Scans one *run* of source in a single pass and emits spans for every
//! inline element it recognises: emphasis, code spans, links, images,
//! autolinks, key-value pairs and hard breaks. A run is either the trailing
//! content of a line that matched a line/block rule, or a whole multi-line
//! fallthrough span handed over by `parse_text!` (see its docs for how runs
//! are accumulated). Called by `parse_text!` only.
//!
//! # Entry point
//!
//! ```text
//! parse_inline!(state, src, start, end, fallback_field,
//!               merge_flag, escape_byte, sep_byte, tab_byte, eol_byte,
//!               max_nest, multiline_flag ; <rules>)
//! ```
//!
//! `multiline_flag` says whether `eol` joins the trigger search. `parse_text!`
//! passes `true` for a multi-line run and `false` for a single-line run,
//! whose range is `eol`-free by construction. A direct caller (the engine's
//! own unit tests) passes `true` unless it can guarantee the same.
//!
//! # Rules
//!
//! The macro consumes the `inline { ... }` section after the proc-macro has
//! stripped the `[N]` capacities and canonicalised the token order:
//!
//! - `merge_simple = true | false;` — coalesce adjacent fallback spans.
//! - `fallback => field;` — the field that receives plain-text spans.
//! - `hard_break(esc, sp, min) => field;` — hard-break detection at the end
//!   of the run and, in a multi-line run, at every internal `eol`.
//! - `on_trigger(b1, …) { <rules> }` — a set of trigger bytes and the rules
//!   tried when one of them is found: `symmetric`, `asymmetric`, `chained`,
//!   `key_value`.
//!
//! The union of every `on_trigger` set (plus `eol` in multi-line mode) is
//! searched with [`crate::swar::find_any`]; bytes between hits are never
//! visited.
//!
//! # The unified stack
//!
//! Every construct that tracks nesting — `symmetric` with `parse_inside =
//! true, balanced = true`, `asymmetric` with `balanced = true` or
//! `parse_inside = true`, and `key_value` — lives on one bounded stack,
//! `frames: [(u8, u8, u32); max_nest]`, with a single depth counter
//! `fdepth`. A frame stores `(byte, count, vidx)` and no kind tag:
//!
//! | kind         | `byte`         | `count`    | `vidx`                          |
//! |--------------|----------------|------------|---------------------------------|
//! | `asymmetric` | the open byte  | `1`        | index of the placeholder span   |
//! | `symmetric`  | the delimiter  | run length | index of the placeholder span   |
//! | `key_value`  | the `end` byte | `0`        | unused; data is in `kv_pending` |
//!
//! A frame's kind, close byte, opacity and output field are recovered by
//! matching `byte` against each rule's literal. This assumes a byte has one
//! meaning across the stack-eligible rules of a grammar, the same assumption
//! every trigger-byte dispatch in the engine makes.
//!
//! Asymmetric and symmetric frames push a placeholder span at open and
//! back-patch its `end` at close, so their output vectors are in open order.
//! A `key_value` pair is pushed complete when its value closes, so its
//! `Vec<T>` is in close order: an outer pair lands after every pair nested in
//! its value.
//!
//! ## Plain text and open frames
//!
//! Plain text found while a frame is open is written to the fallback field
//! as usual, and every frame records the fallback vector's length at its
//! open. When the frame closes the vector is rolled back to that length: the
//! frame's span covers those bytes. When the frame is still open at the end
//! of the run it is discarded and the text stays: everything after an
//! unmatched opener is plain text. Text inside an in-progress `chained` match
//! is not flushed; it belongs to the match.
//!
//! # Off-stack constructs
//!
//! - `symmetric` with `parse_inside = true, balanced = false` — a single
//!   `pending` slot: the first run opens, the next run of the same byte and
//!   count closes; a different-count run replaces the slot.
//! - `symmetric` with `parse_inside = false` — a forward search for the
//!   closing run; the content is opaque to every other rule. With
//!   `balanced = true` a doubled run is skipped as literal content.
//! - `asymmetric` with `balanced = false, parse_inside = false` — a forward
//!   `memchr` search for the close byte; the close byte need not be in the
//!   `on_trigger` set.
//! - `chained` — two sequential phases (text, then url), one slot each. With
//!   both components opaque it is a self-contained forward search; with a
//!   transparent component it is a two-phase state machine that lets other
//!   rules fire on the bytes it scans over.
//!
//! Every forward search skips escaped close candidates. The `memchr`-style
//! searches (off-stack `asymmetric`, opaque `chained` components with
//! `balanced = false`) remember a close byte that is absent from the rest of
//! the run, so later openers of the same kind are rejected without a rescan.
//!
//! # `asymmetric`
//!
//! `balanced` sets the type's depth cap: `max_nest` when `true`, `1` when
//! `false`. `parse_inside` sets opacity: with `false`, no other rule fires
//! inside the frame. Either flag `true` puts the rule on the stack; its
//! close byte must then be listed in the same `on_trigger` set, because
//! closes are found by the same search as opens. Each byte of an open run is
//! its own open event: `{{` is two opens. An open beyond the cap bumps a
//! one-shot overflow counter, consumed by the next same-type close.
//!
//! A close byte runs one pass per character: a `key_value` frame on top of
//! the stack is committed first (its value ends where its container closes),
//! then the frame below is popped if its rule's close byte is this
//! character. Two rules may share a close byte; dispatch is by the frame's
//! own open byte.
//!
//! # `symmetric` on the stack
//!
//! With `parse_inside = true, balanced = true`, a run whose `(byte, count)`
//! matches the top frame closes it; otherwise, with room, it opens a new
//! frame. An identical `(byte, count)` never self-nests, so `**a **b** c**`
//! is two adjacent runs. The run length selects the field arm.
//!
//! # Internal `eol`
//!
//! In a multi-line run `eol` is an ordinary byte for every rule; the stack
//! survives it. When `hard_break` is declared, each internal `eol` is checked
//! like the run's end: trailing `sp` bytes or an `esc` byte before it are
//! trimmed from the flushed text and a zero-length hard-break span is
//! emitted. When the top of the stack is a `key_value` frame the `eol` is
//! left to the `end`-byte check instead, because `end` and `eol` may be the
//! same byte.
//!
//! # `key_value`
//!
//! The rule triggers on `eq`. The key is found by scanning back from `eq` to
//! the previous `sep`/`tab`, clamped to the start of the current key segment
//! (the position after the last container open or `end` byte), so a quoted
//! key consumed by an opaque rule is still covered. The value starts after
//! `eq` (skipping `sep` when `allow_sep`) and is tracked by a stack frame; a
//! second `eq` while a `key_value` frame is on top is value content. The
//! pair is pushed when the value closes: at `end`, when the enclosing
//! container closes, or at the end of the run. When the value is a
//! container, the value span includes the brackets and the container's own
//! span lies strictly inside it.
//!
//! # Limitations
//!
//! - One `chained` rule with a transparent component per grammar: the phase
//!   state is shared.
//! - One `key_value` rule per grammar: the key-segment anchor and the
//!   top-is-kv test are shared.
#[doc(hidden)]
#[macro_export]
macro_rules! parse_inline {
    ($state:ident, $src:ident, $start:expr, $le:expr,
     $tx:ident, $merge_il:tt, $esc:literal, $sep:literal, $tab:literal, $eol:literal, $maxn:literal, $ml:tt ;
     $($tail:tt)*) => {
        $crate::parse_inline!(
            @collect ($state, $src, $start, $le, $tx, $merge_il, $esc, $sep, $tab, $eol, $maxn, $ml)
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
               $tx:ident, $merge_il:tt, $esc:literal, $sep:literal, $tab:literal, $eol:literal, $maxn:literal, $ml:tt)
     (hard_break: )
     finders  = [$($fi:tt)*]
     sy_rules = [$($sr:tt)*]
     as_rules = [$($ar:tt)*]
     ch_rules = [$($cr:tt)*]
     kv_rules = [$($kv:tt)*]
     tail = [hard_break($hb_esc:literal, $sp:literal, $sp_min:literal) => $hb:ident ; $($rest:tt)*]
    ) => {
        $crate::parse_inline!(
            @collect ($st, $src, $s, $le, $tx, $merge_il, $esc, $sep, $tab, $eol, $maxn, $ml)
            (hard_break: $hb_esc, $sp, $sp_min => $hb)
            finders  = [$($fi)*]
            sy_rules = [$($sr)*]
            as_rules = [$($ar)*]
            ch_rules = [$($cr)*]
            kv_rules = [$($kv)*]
            tail = [$($rest)*]
        )
    };

    // on_trigger(...) { ... }
    (@collect ($st:ident, $src:ident, $s:expr, $le:expr,
               $tx:ident, $merge_il:tt, $esc:literal, $sep:literal, $tab:literal, $eol:literal, $maxn:literal, $ml:tt)
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
            @collect ($st, $src, $s, $le, $tx, $merge_il, $esc, $sep, $tab, $eol, $maxn, $ml)
            (hard_break: $($hb)*)
            finders  = [$($fi)* { $($fn_b),+ $(, $kv_end)* }]
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
                  $tx:ident, $merge_il:tt, $esc:literal, $sep:literal, $tab:literal, $eol:literal, $maxn:literal, $ml:tt)
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
        $crate::parse_inline!(@body ($st, $src, $s, $le, $tx, $merge_il, $esc, $sep, $tab, $eol, $maxn, $ml)
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
    // Execution phase: the actual single-pass scan over one run.          //
    // ------------------------------------------------------------------ //

    (
        @body ($state:ident, $src:ident, $start:expr, $le:expr,
               $tx:ident, $merge_il:tt, $esc:literal, $sep:literal, $tab:literal, $eol:literal, $maxn:literal, $ml:tt)
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

        // ---- The unified inline stack ---------------------------------- //
        //
        // One frame layout shared by asymmetric, symmetric and key_value:
        // `(byte, count, vidx)`.
        //   asymmetric : byte = open byte, count = 1 (each byte of an open
        //                run is its own event), vidx = placeholder index.
        //   symmetric  : byte = the delimiter, count = run length (selects
        //                the field arm), vidx = placeholder index.
        //   key_value  : byte = $kv_end, count and vidx unused; the frame's
        //                data lives in `kv_pending` at the same index.
        // Close byte, opacity and field routing are recovered from `byte` by
        // matching it against each rule's literal.
        //
        // `count` is a `u8`: a run longer than 255 bytes aliases with the run
        // whose length is equal modulo 256. No grammar declares counts near
        // that range.
        let mut frames: [(u8, u8, u32); $maxn] = [(0u8, 0u8, 0u32); $maxn];

        // Key span and value start of each open key_value frame, at the
        // frame's own stack index. The pair is pushed to `$kv_f` only when
        // the value closes, which keeps `$kv_f` in close order.
        let mut kv_pending: [(u32, u32, u32); $maxn] = [(0u32, 0u32, 0u32); $maxn];

        // Per-frame snapshot of the fallback vector `$tx` at the frame's
        // open: `(len, end-of-last-span)`. Text flushed while the frame is
        // open is rolled back to this snapshot when the frame closes
        // (`release_held!`); a frame discarded at the end of the run leaves
        // it in place. The second field restores the previous last span's
        // `end`, which `push_merge_*` may have extended into the held region.
        let mut held_base: [(u32, u32); $maxn] = [(0u32, 0u32); $maxn];

        #[allow(unused_macros)]
        macro_rules! hold_base {
            () => {
                (
                    $state.$tx.len() as u32,
                    $state.$tx.last().map_or(0u32, |s| s.end),
                )
            };
        }
        #[allow(unused_macros)]
        macro_rules! release_held {
            ($base:expr) => {{
                let (_hl, _he): (u32, u32) = $base;
                $state.$tx.truncate(_hl as usize);
                if _hl > 0 {
                    if let Some(_last) = $state.$tx.last_mut() {
                        _last.end = _he;
                    }
                }
            }};
        }

        // `fdepth` indexes `frames` and `kv_pending` several times per
        // trigger byte; a narrower type would cost a zero-extend on every
        // access and save nothing, since it is a scalar, not an array element.
        let mut fdepth: usize = 0;
        // One-shot overflow counter for `balanced = true` asymmetric opens
        // beyond the cap, so the real tracked frame's close isn't mistaken
        // early.
        let mut asym_overflow: u32 = 0u32;
        // Anchor for the start of the current key segment (see kv docs).
        let mut _kv_seg_start: usize = $start;

        // Hard-break detection.
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
        // Single pending slot for `parse_inside = true`, `balanced = false`:
        // `(byte, open position, count)`.
        let mut pending: Option<(u8, u32, u32)> = None;

        // Two-phase transparent state for `chained` (off the stack).
        let mut ch_in_text: bool = false;
        let mut ch_text_opaque: bool = false;
        let mut ch_text_depth: u32 = 0;
        let mut ch_text_start: u32 = 0;
        let mut ch_is_prefix: bool = false;
        let mut ch_real_start: u32 = 0;

        let mut ch_in_url: bool = false;
        let mut ch_url_opaque: bool = false;
        let mut ch_url_depth: u32 = 0;
        let mut ch_url_start: u32 = 0;
        let mut ch_saved_text_end: u32 = 0;

        // Close bytes known to be absent (unescaped) from the rest of the
        // run. The forward searches below (asymmetric with `balanced = false`
        // and `parse_inside = false`; opaque `chained` components with
        // `balanced = false`) scan from the current position to `parse_end`.
        // `pos` only moves forward and `parse_end` is fixed, so a failed
        // search for a byte cannot succeed later in this run: the byte is
        // recorded and later openers of the same kind are rejected without a
        // rescan. One bit per byte value.
        let mut _dead_close: [u64; 4] = [0u64; 4];
        #[allow(unused_macros)]
        macro_rules! dead_close_has {
            ($b:expr) => {
                (_dead_close[($b as usize) >> 6] >> (($b as usize) & 63)) & 1 == 1
            };
        }
        #[allow(unused_macros)]
        macro_rules! dead_close_set {
            ($b:expr) => {
                _dead_close[($b as usize) >> 6] |= 1u64 << (($b as usize) & 63)
            };
        }

        loop {
            // ---- Single unified trigger search -------------------------- //
            //
            // One `find_any` over the union of every `on_trigger` set, plus
            // `$eol` when the caller's multiline flag is set. `find_any`'s wide
            // path costs one compare per target per chunk, so `$eol` is searched
            // only where it can occur: the single-line call's range is eol-free
            // by construction, while a multi-line run needs the search to stop
            // at each line for hard-break handling.
            let found: Option<usize> = if $ml {
                $crate::swar::find_any(
                    [$eol $(, $($fn_b),+)*],
                    &src[pos..parse_end],
                ).map(|r| pos + r)
            } else {
                $crate::swar::find_any(
                    [$($($fn_b),+),*],
                    &src[pos..parse_end],
                ).map(|r| pos + r)
            };

            let Some(_hit) = found else { break };
            pos = _hit;

            // Is the top frame a key_value frame? Derived from its stored byte.
            // Valid for this outer iteration only: loops that pop frames
            // recompute it.
            let _top_is_kv = fdepth > 0 && {
                let _tb = frames[fdepth - 1].0;
                let mut _k = false;
                $( if _tb == $kv_end { _k = true; } )*
                _k
            };

            // ---------------------------------------------------------- //
            // Internal eol of a multi-line run. Present only when            //
            // `hard_break` is declared; otherwise the eol falls through the   //
            // dispatch below as an unmatched byte. Runs before the escape      //
            // check: a backslash before eol is the hard-break marker, not an    //
            // escape. Trims trailing `sp` / `esc` bytes, emits the hard-break    //
            // span, flushes text up to the trim point and continues the scan;    //
            // the stack is not drained. The end-of-run check further down still   //
            // handles input that ends without an eol and the single-line calls.    //
            //                                                                       //
            // Skipped when a kv frame is on top: `end` and `eol` may be the same     //
            // byte, and the `$kv_end` check below owns that occurrence.               //
            // ------------------------------------------------------------------------//
            $(
                if src[pos] == $eol && !_top_is_kv {
                    let mut _ep = pos;
                    let mut _ehb = false;
                    if _ep > $start {
                        if src[_ep - 1] == $hb_esc {
                            _ep -= 1;
                            _ehb = true;
                        } else {
                            let mut _en: u32 = 0;
                            while _ep > $start && src[_ep - 1] == $sp {
                                _en += 1; _ep -= 1;
                            }
                            if _en >= $sp_min { _ehb = true; }
                        }
                    }
                    if text_start < _ep {
                        if !ch_in_text && !ch_in_url {
                            push_il!($tx, $crate::span::Span::new(text_start as u32, _ep as u32));
                        }
                    }
                    if _ehb {
                        $state.$hb.push($crate::span::Span::new(_ep as u32, _ep as u32));
                    }
                    pos += 1;
                    text_start = pos;
                    continue;
                }
            )*

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

            // ---------------------------------------------------------- //
            // key_value terminator (`$kv_end`) at the value's own level:  //
            // fires only with a kv frame on top of the stack. The value    //
            // ends before the separator. With no kv frame on top the byte   //
            // is left alone (a bare array's `,` between elements is         //
            // content).                                                      //
            // -----------------------------------------------------------------//
            let mut _kv_hit = false;
            $(
                if delim == $kv_end
                    && fdepth > 0
                    && frames[fdepth - 1].0 == $kv_end
                {
                    let (_ks, _ke, _vs) = kv_pending[fdepth - 1];
                    $state.$kv_f.push($kv_ty {
                        $kv_kf: $crate::span::Span::new(_ks, _ke),
                        $kv_vf: $crate::span::Span::new(_vs, delim_start),
                    });
                    fdepth -= 1;
                    release_held!(held_base[fdepth]);
                    _kv_seg_start = pos;
                    _kv_hit = true;
                }
            )*
            if _kv_hit {
                text_start = pos;
                continue;
            }

            // ---------------------------------------------------------- //
            // asymmetric (kind 0): unified open + unified close cascade.  //
            // ---------------------------------------------------------- //
            let _chained_opaque_active =
                (ch_in_text && ch_text_opaque) || (ch_in_url && ch_url_opaque);

            let mut _asym_bal_handled = false;
            if !_chained_opaque_active {
                // --- open side ---
                $(
                    if ($abal || $api) && delim == $ao {
                        if text_start < delim_start as usize {
                            if !ch_in_text && !ch_in_url {
                                push_il!($tx, $crate::span::Span::new(text_start as u32, delim_start));
                            };
                        }
                        let _cap: usize = if $abal { $maxn } else { 1usize };
                        for _k in 0..count {
                            let _char_pos = delim_start + _k;
                            let mut _consumed = false;
                            if fdepth < $maxn && fdepth < _cap {
                                match 1u32 {
                                    $( $an => {
                                        let _content_start = _char_pos + 1;
                                        let _vidx = $state.$af.len() as u32;
                                        push_il!($af, $crate::span::Span::new(
                                            _content_start, _content_start));
                                        frames[fdepth] = ($ao, 1u8, _vidx);
                                        held_base[fdepth] = hold_base!();
                                        fdepth += 1;
                                        asym_overflow = 0;
                                        _consumed = true;
                                    } )*
                                    _ => {}
                                }
                            } else if $abal && fdepth > 0
                                && frames[fdepth - 1].0 == $ao
                            {
                                asym_overflow += 1;
                                _consumed = true;
                            }
                            if _consumed {
                                text_start = (_char_pos + 1) as usize;
                            }
                        }
                        // A container just opened: a key may begin right
                        // after it. Anchor the next key segment past the run.
                        _kv_seg_start = pos;
                        _asym_bal_handled = true;
                    }
                )*

                // --- close side: single unified pass, with kv drain. ---
                if !_asym_bal_handled {
                    let mut _asym_is_close_byte = false;
                    $( if ($abal || $api) && delim == $ac { _asym_is_close_byte = true; } )*

                    if _asym_is_close_byte {
                        for _k in 0..count {
                            let _close_char_pos = delim_start + _k;

                            // A key_value frame directly above the closing container is
                            // committed first, before the container pops. Recomputed per
                            // character: `fdepth` changes inside this loop.
                            if fdepth >= 2 {
                                let _top_b = frames[fdepth - 1].0;
                                let mut _top_is_kv_now = false;
                                $( if _top_b == $kv_end { _top_is_kv_now = true; } )*
                                if _top_is_kv_now {
                                    let _below_b = frames[fdepth - 2].0;
                                    let mut _below_closes_here = false;
                                    $( if ($abal || $api) && _below_b == $ao && $ac == delim {
                                        _below_closes_here = true;
                                    } )*
                                    if _below_closes_here {
                                        let (_ks, _ke, _vs) = kv_pending[fdepth - 1];
                                        $(
                                            if _top_b == $kv_end {
                                                $state.$kv_f.push($kv_ty {
                                                    $kv_kf: $crate::span::Span::new(_ks, _ke),
                                                    $kv_vf: $crate::span::Span::new(_vs, _close_char_pos),
                                                });
                                            }
                                        )*
                                        fdepth -= 1;
                                        release_held!(held_base[fdepth]);
                                    }
                                }
                            }

                            if fdepth > 0 {
                                let _ob = frames[fdepth - 1].0;
                                let mut _closes_here = false;
                                $( if ($abal || $api) && _ob == $ao && $ac == delim {
                                    _closes_here = true;
                                } )*
                                if _closes_here {
                                    if asym_overflow > 0 {
                                        asym_overflow -= 1;
                                    } else {
                                        let _vidx = frames[fdepth - 1].2;
                                        $(
                                            if ($abal || $api) && _ob == $ao {
                                                match 1u32 {
                                                    $( $an => {
                                                        $state.$af[_vidx as usize].end = _close_char_pos;
                                                    } )*
                                                    _ => {}
                                                }
                                            }
                                        )*
                                        fdepth -= 1;
                                        release_held!(held_base[fdepth]);
                                        asym_overflow = 0;
                                    }
                                    text_start = (_close_char_pos + 1) as usize;
                                }
                            }
                        }
                        _asym_bal_handled = true;
                    }
                }
            }
            if _asym_bal_handled {
                continue;
            }

            let _top_opaque_active = fdepth > 0 && {
                let _tb = frames[fdepth - 1].0;
                let mut _op = false;
                $( if ($abal || $api) && _tb == $ao { _op = !$api; } )*
                _op
            };

            // ---------------------------------------------------------- //
            // chained, transparent phases (off the stack).               //
            // ---------------------------------------------------------- //
            let mut _chained_handled = false;
            $(
                if ($tpi || $upi) && !ch_in_text && !ch_in_url
                    && !_top_opaque_active && delim == $co
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
                        if !ch_in_text && !ch_in_url {
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
                            if (ch_real_start as usize) < pos {
                                if !ch_in_text && !ch_in_url {
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
                        ch_in_url = false;
                        if text_start < ch_real_start as usize {
                            if !ch_in_text && !ch_in_url {
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

            if _top_opaque_active || _chained_opaque_active {
                continue;
            }

            // --- chained, both components opaque: self-contained forward search ---
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
                            if !$crate::parse_inline!(@is_escaped src, _i, $start, $esc) {
                                if src[_i] == $co { _depth += 1; }
                                else if src[_i] == $cc {
                                    _depth -= 1;
                                    if _depth == 0 { _found = Some(_i); break; }
                                }
                            }
                            _i += 1;
                        }
                        _found
                    } else if dead_close_has!($cc) {
                        None
                    } else {
                        let mut _found: Option<usize> = None;
                        while _i < parse_end {
                            if src[_i] == $cc
                                && !$crate::parse_inline!(@is_escaped src, _i, $start, $esc)
                            {
                                _found = Some(_i);
                                break;
                            }
                            _i += 1;
                        }
                        if _found.is_none() {
                            dead_close_set!($cc);
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
                                    if !$crate::parse_inline!(@is_escaped src, _j, $start, $esc) {
                                        if src[_j] == $uo { _depth += 1; }
                                        else if src[_j] == $uc {
                                            _depth -= 1;
                                            if _depth == 0 { _found = Some(_j); break; }
                                        }
                                    }
                                    _j += 1;
                                }
                                _found
                            } else if dead_close_has!($uc) {
                                None
                            } else {
                                let mut _found: Option<usize> = None;
                                while _j < parse_end {
                                    if src[_j] == $uc
                                        && !$crate::parse_inline!(@is_escaped src, _j, $start, $esc)
                                    {
                                        _found = Some(_j);
                                        break;
                                    }
                                    _j += 1;
                                }
                                if _found.is_none() {
                                    dead_close_set!($uc);
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
                                    if !ch_in_text && !ch_in_url {
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

            // --- symmetric: stack mode, pending slot, or greedy forward search ---
            $(
                if delim == $sb {
                    if $pi {
                        if $bal {
                            let _matches_top = fdepth > 0
                                && frames[fdepth - 1].0 == $sb
                                && frames[fdepth - 1].1 == count as u8;

                            if _matches_top {
                                let _vidx = frames[fdepth - 1].2;
                                let mut _closed = false;
                                match count {
                                    $( $sn => {
                                        $state.$sf[_vidx as usize].end = delim_start;
                                        _closed = true;
                                    } )*
                                    _ => {}
                                }
                                if _closed {
                                    fdepth -= 1;
                                    release_held!(held_base[fdepth]);
                                    text_start = pos;
                                    continue;
                                } else {
                                    text_start = delim_start as usize;
                                    continue;
                                }
                            } else if fdepth < $maxn {
                                let mut _pushed = false;
                                match count {
                                    $( $sn => {
                                        if text_start < delim_start as usize {
                                            if !ch_in_text && !ch_in_url {
                                                push_il!($tx, $crate::span::Span::new(
                                                text_start as u32, delim_start));
                                            };
                                        }
                                        let _vidx = $state.$sf.len() as u32;
                                        push_il!($sf, $crate::span::Span::new(pos as u32, pos as u32));
                                        frames[fdepth] = ($sb, count as u8, _vidx);
                                        held_base[fdepth] = hold_base!();
                                        _pushed = true;
                                    } )*
                                    _ => {}
                                }
                                if _pushed {
                                    fdepth += 1;
                                    text_start = pos;
                                    continue;
                                } else {
                                    if text_start < delim_start as usize {
                                        if !ch_in_text && !ch_in_url {
                                            push_il!($tx, $crate::span::Span::new(
                                            text_start as u32, delim_start));
                                        };
                                    }
                                    text_start = delim_start as usize;
                                    continue;
                                }
                            } else {
                                if text_start < delim_start as usize {
                                    if !ch_in_text && !ch_in_url {
                                        push_il!($tx, $crate::span::Span::new(
                                        text_start as u32, delim_start));
                                    };
                                }
                                text_start = delim_start as usize;
                                continue;
                            }
                        } else {
                            // Single pending slot: `parse_inside = true`, `balanced = false`.
                            if let Some((pb, op, oc)) = pending {
                                if pb == $sb && oc == count {
                                    if (text_start as u32) < op {
                                        if !ch_in_text && !ch_in_url {
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
                            pending = Some(($sb, delim_start, count));
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
                                        if $crate::parse_inline!(@is_escaped src, p, $start, $esc) {
                                            _i = p + 1;
                                            continue;
                                        }
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
                                        if $crate::parse_inline!(@is_escaped src, p, $start, $esc) {
                                            _i = p + 1;
                                            continue;
                                        }
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
                                if !ch_in_text && !ch_in_url {
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

            // --- asymmetric, balanced = false and parse_inside = false: forward search ---
            $(
                if delim == $ao {
                    let cs = pos;
                    let close_pos: Option<usize> = if $abal {
                        let mut depth: usize = 1;
                        let mut _i = pos;
                        let mut found = None;
                        while _i < parse_end {
                            if !$crate::parse_inline!(@is_escaped src, _i, $start, $esc) {
                                if src[_i] == $ao { depth += 1; }
                                else if src[_i] == $ac {
                                    depth -= 1;
                                    if depth == 0 { found = Some(_i); break; }
                                }
                            }
                            _i += 1;
                        }
                        found
                    } else if dead_close_has!($ac) {
                        None
                    } else {
                        let mut _i = pos;
                        let mut _found: Option<usize> = None;
                        loop {
                            match $crate::memchr::memchr($ac, &src[_i..parse_end]) {
                                None => break,
                                Some(r) => {
                                    let p = _i + r;
                                    if $crate::parse_inline!(@is_escaped src, p, $start, $esc) {
                                        _i = p + 1;
                                    } else {
                                        _found = Some(p);
                                        break;
                                    }
                                }
                            }
                        }
                        if _found.is_none() {
                            dead_close_set!($ac);
                        }
                        _found
                    };
                    if let Some(cp) = close_pos {
                        if text_start < delim_start as usize {
                            if !ch_in_text && !ch_in_url {
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

            // --- key_value: resolve the key and park it in `kv_pending`; the pair
            // is pushed when the value closes.
            $(
                if delim == $kv_eq {
                    // Only open a new value frame if the top of stack is not
                    // already a kv frame. If it is, this `eq` is content of
                    // the still-open value (flat, separator-less multi-eq).
                    if !_top_is_kv {
                        // Key: back-scan from `eq`, clamped to _kv_seg_start.
                        let mut key_end = delim_start as usize;
                        if $kv_allow {
                            while key_end > _kv_seg_start && src[key_end - 1] == $sep {
                                key_end -= 1;
                            }
                        }
                        let mut ks = key_end;
                        while ks > _kv_seg_start
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
                        if text_start < ks {
                            if !ch_in_text && !ch_in_url {
                                push_il!($tx, $crate::span::Span::new(text_start as u32, ks as u32));
                            };
                        }
                        if fdepth < $maxn {
                            kv_pending[fdepth] = (ks as u32, key_end as u32, val_start as u32);
                            frames[fdepth] = ($kv_end, 0u8, 0u32);
                            held_base[fdepth] = hold_base!();
                            fdepth += 1;
                        }
                        // else: depth cap reached — pair untracked, `eq`
                        // absorbed.
                        text_start = pos;
                    }
                    continue;
                }
            )*
        }

        // ------------------------------------------------------------------ //
        // End of run: drain the stack top → down, kind derived from the stored //
        // byte.                                                                //
        //  - key_value  : push the pair complete, value ending at the run's    //
        //    end, and move the text cursor past it.                            //
        //  - asymmetric : discard via `Vec::remove(vidx)`; a closed inner       //
        //    entry may sit above a still-open outer one.                        //
        //  - symmetric  : discard via `truncate(vidx)`; the placeholder is       //
        //    always the field's last entry.                                     //
        // The outermost discarded frame's held text stays in the fallback      //
        // field (see `held_base`).                                             //
        // ------------------------------------------------------------------ //
        while fdepth > 0 {
            fdepth -= 1;
            let (_fb, _fc, _fv) = frames[fdepth];

            let mut _matched_kv = false;
            $(
                if _fb == $kv_end {
                    let (_ks, _ke, _vs) = kv_pending[fdepth];
                    $state.$kv_f.push($kv_ty {
                        $kv_kf: $crate::span::Span::new(_ks, _ke),
                        $kv_vf: $crate::span::Span::new(_vs, parse_end as u32),
                    });
                    _matched_kv = true;
                }
            )*

            if _matched_kv {
                release_held!(held_base[fdepth]);
                if parse_end > text_start {
                    text_start = parse_end;
                }
            } else {
                let mut _matched_asym = false;
                $(
                    if ($abal || $api) && _fb == $ao {
                        match 1u32 {
                            $( $an => { $state.$af.remove(_fv as usize); } )*
                            _ => {}
                        }
                        _matched_asym = true;
                    }
                )*
                if !_matched_asym {
                    $(
                        if $bal && $pi && _fb == $sb {
                            match _fc {
                                $( $sn => { $state.$sf.truncate(_fv as usize); } )*
                                _ => {}
                            }
                        }
                    )*
                }
            }
        }

        // Flush any remaining plain text before the scanned span's end.
        if text_start < parse_end {
            push_il!($tx, $crate::span::Span::new(text_start as u32, parse_end as u32));
        }
        // Emit hard-break marker if detected.
        $( if _hb {
            $state.$hb.push($crate::span::Span::new(parse_end as u32, parse_end as u32));
        } )*

        // Returns `$le` unchanged. The single-line call site resumes at `$le + 1`
        // itself; the multi-line call site ignores the value.
        $le
    }};

    // ------------------------------------------------------------------ //
    // @is_escaped: shared escape-check for a single candidate position.  //
    // ------------------------------------------------------------------ //
    (@is_escaped $src:ident, $pos:expr, $start:expr, $esc:literal) => {{
        let _p = $pos;
        _p > $start && {
            let mut _bs: u32 = 0;
            let mut _ei = _p;
            while _ei > $start && $src[_ei - 1] == $esc { _bs += 1; _ei -= 1; }
            _bs % 2 == 1
        }
    }};
}
