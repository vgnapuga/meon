//! Inline element parser — the `parse_inline!` macro.
//!
//! Scans one *run* of source in a single pass and emits spans for every
//! inline rule of the grammar — `symmetric`, `asymmetric`, `chained` and
//! `key_value` — and for hard breaks. A run is either the trailing
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
//! Every construct that tracks nesting — `symmetric` and `asymmetric` with
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
//! A frame's kind, close byte and output field are recovered by matching
//! `byte` against each rule's literal. This assumes a byte has one
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
//! frame's span covers those bytes. A frame's open run is text until the
//! frame closes. When the frame is still open at the end of the run it is
//! discarded and the text stays: an unmatched opener and everything after it
//! is plain text. A `chained` match with a transparent
//! component holds its text the same way: the text inside it, opener
//! included, is written as usual and rolled back when the match completes,
//! and it stays when the match fails or is still open at the end of the run.
//!
//! # Off-stack constructs
//!
//! - `symmetric` with `parse_inside = false` — a forward search for the next
//!   unescaped run of the same byte and count; a run of another count is
//!   content. The content is opaque to every other rule.
//! - `asymmetric` with `parse_inside = false` — a forward search for the
//!   close byte, which need not be in the `on_trigger` set (see
//!   [`asymmetric`](#asymmetric)). The content is opaque to every other rule.
//! - `chained` — two sequential phases (the first component, then the
//!   second), one slot each. With
//!   both components opaque it is a self-contained forward search; with a
//!   transparent component it is a two-phase state machine that lets other
//!   rules fire on the bytes it scans over. Of a run of the open byte only
//!   the last byte opens the match, and the prefix is the byte before it,
//!   when that byte lies in the run: with `[`, `]`, `(`, `)` and the prefix
//!   `!`, in `![[a](u)` the text is `![` and the match is `[a](u)`, with no
//!   prefix, and a prefix byte that is a block marker before the run is no
//!   prefix.
//!   With `balanced = true` the bytes before it are text too, so they never
//!   nest in the match. Inside a transparent match the bytes of a close run
//!   act one by one. With `balanced = true` each open byte of the component
//!   nests one level and each close byte leaves one; the first close byte
//!   with no level left closes the component, the second component opens only
//!   on the byte right after it, and the rest of the run is read again. With
//!   `balanced = false` the rule's open byte inside its own match is text.
//!   The other `chained` rules leave the match alone: they match inside its
//!   transparent components, like any other rule, and not inside its opaque
//!   ones.
//!
//! In the forward searches a run whose count matches no declared arm is
//! literal text: it opens nothing, and no close is searched for, so nothing
//! after it is skipped.
//!
//! Every forward search skips escaped close candidates. The forward
//! searches (`asymmetric`, opaque `chained` components) remember a close
//! byte they did not find and where they started: a later search for that
//! byte from there on stops at that start, so later openers of the same
//! kind are rejected without a rescan. The first component of an opaque
//! `chained` rule with `balanced = false` also remembers the close it found
//! when the match fails after it, for the openers before that close. With
//! `balanced = true` a search counts depth. One of an `asymmetric` rule that
//! meets close bytes without returning to depth zero remembers the opens
//! inside it that no close balances: a later open byte of the rule in its
//! range fails at once when it is one of them and closes otherwise, so the
//! scan stays linear. A `chained` component's search keeps no such record:
//! a later opener inside it can still balance, and openers like that, `k` of
//! them before the end of an `n`-byte run, cost `O(k·n)`.
//!
//! # `asymmetric`
//!
//! `parse_inside` sets where the rule lives. With `true` it lives on the
//! stack and is transparent: every other rule fires inside its frames.
//! `balanced` then sets how its frames nest: with `true` they nest in one
//! another up to `max_nest`; with `false` the rule holds one frame at a
//! time, which opens at any depth the stack has room for, under other rules'
//! frames too. Its close byte must be listed in the same `on_trigger` set,
//! because closes are found by the same search as opens. Each byte of an
//! open run is its own open event: `{{` is two opens. An open of a
//! `balanced = true` rule beyond the cap, with that rule's own frame on top,
//! bumps a one-shot overflow counter, consumed by the next same-type close.
//! That open and that close are content of the frame on the stack, rolled
//! back or kept with the rest of its text (see "Plain text and open
//! frames"). Any other open the stack cannot take (a second frame of a
//! `balanced = false` rule, a full stack under another rule's frame) is
//! refused and stays literal text.
//!
//! A close byte runs one pass per character: a `key_value` frame on top of
//! the stack is committed first (its value ends where its container closes),
//! then the frame below is popped if its rule's close byte is this
//! character. Two rules may share a close byte; dispatch is by the frame's
//! own open byte. A close byte with another rule's frame on top closes
//! nothing: frames that cross do not close.
//!
//! With `parse_inside = false` the rule is a forward search, off the stack
//! and opaque: no other rule fires inside its match, and `max_nest` plays no
//! part. With `balanced = false` a run of `n` open bytes selects the `n` arm
//! and the first unescaped close byte closes it. With `balanced = true` the
//! search counts the rule's own bytes, as bytes only: each byte of an open
//! run is an open of its own, the first one opens the outer ring and every
//! other open byte nests in it, and the close byte that returns the count to
//! zero closes the ring. The element is the ring, one per ring, whatever
//! nests in it. A ring that never closes is text, and the next open byte
//! tries, the rest of its own run included.
//!
//! # `symmetric` on the stack
//!
//! With `parse_inside = true`, a run whose `(byte, count)` matches the top
//! frame closes it; otherwise, with room, it opens a new frame, so runs of
//! different counts of one byte nest in one another. An identical
//! `(byte, count)` never nests directly in itself, so `**a **b** c**` is two
//! adjacent runs, but it opens again above a frame of another count: in
//! `*a **b *c* d** e` the second `*` opens a second count-1 frame. A frame
//! below the top one is never closed: frames that cross do not close. The
//! run length selects the field arm; a run with no arm, or one the full stack
//! cannot take, is literal text. The rule has no `balanced` setting: its open
//! and close are one byte, so a run of one count never nests in itself.
//!
//! # Internal `eol`
//!
//! In a multi-line run `eol` is an ordinary byte for every rule; the stack
//! survives it. When `hard_break` is declared, each internal `eol` is checked
//! like the run's end: an `esc` byte before it that is not itself escaped,
//! or a run of at least `min` `sp` bytes, is the marker. The marker is
//! trimmed from the flushed text and a zero-length hard-break span is
//! emitted; fewer `sp` bytes stay text. As at the run's end, where the scan
//! stops before the marker, no other rule reads a marker byte: a delimiter
//! run that reaches the marker stops where the marker starts. When the top
//! of the stack is a `key_value` frame the `eol` is left to the `end`-byte
//! check instead, because `end` and `eol` may be the same byte.
//!
//! # `key_value`
//!
//! The rule triggers on `eq`. The key is found by scanning back from `eq`,
//! clamped to the start of the current key segment: the position after the
//! last open run or close of a stack frame, or after the `end` run that
//! closed the previous value, so a key takes no byte of an element before
//! it, nor of an open frame's opener. When an opaque span ends right before
//! `eq`, or before the `sep` run trimmed when `allow_sep` — the match of an
//! opaque `symmetric` rule (a quoted key), of an opaque `asymmetric` rule, or
//! of a `chained` rule with both components opaque, its prefix included —
//! that span with its delimiters is the key, `sep` bytes inside it included.
//! Otherwise the key is the word before `eq`: the scan stops at `sep`, `tab`,
//! `eol` and at the end of an earlier opaque span. The value starts after `eq` (skipping
//! `sep` when `allow_sep`) and is tracked by a stack frame; a second `eq`
//! while a `key_value` frame is on top is value content, and so is the rest
//! of an `eq` run. With the stack full, `eq` opens no frame and the key, `eq`
//! and value stay text. The pair is pushed when the value closes: at `end`,
//! whose run keeps its other bytes as text and starts the next key segment
//! after itself, when the enclosing container closes, or at the end of the
//! run. An `end` byte the value start skipped as `sep` closes nothing. When
//! the value is a container, the value span includes the brackets and the
//! container's own span lies strictly inside it.
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
    // A multi-line run whose lines after the first are led by the `cont`
    // markers `$cs[..$cd]`, outermost first (see `parse_text!`).
    ($state:ident, $src:ident, $start:expr, $le:expr,
     $tx:ident, $merge_il:tt, $esc:literal, $sep:literal, $tab:literal, $eol:literal, $maxn:literal,
     cont($cs:ident, $cd:expr), crlf = $crl:ident ;
     $($tail:tt)*) => {
        $crate::parse_inline!(
            @collect ($state, $src, $start, $le, $tx, $merge_il, $esc, $sep, $tab, $eol, $maxn, true,
                      [$cs, $cd], $crl)
            (hard_break: )
            finders  = []
            sy_rules = []
            as_rules = []
            ch_rules = []
            kv_rules = []
            tail = [$($tail)*]
        )
    };
    // A run whose internal `eol` bytes, with `crlf = true`, take the `\r`
    // right before them as part of the line end.
    ($state:ident, $src:ident, $start:expr, $le:expr,
     $tx:ident, $merge_il:tt, $esc:literal, $sep:literal, $tab:literal, $eol:literal, $maxn:literal, $ml:tt,
     crlf = $crl:ident ;
     $($tail:tt)*) => {
        $crate::parse_inline!(
            @collect ($state, $src, $start, $le, $tx, $merge_il, $esc, $sep, $tab, $eol, $maxn, $ml, [], $crl)
            (hard_break: )
            finders  = []
            sy_rules = []
            as_rules = []
            ch_rules = []
            kv_rules = []
            tail = [$($tail)*]
        )
    };
    ($state:ident, $src:ident, $start:expr, $le:expr,
     $tx:ident, $merge_il:tt, $esc:literal, $sep:literal, $tab:literal, $eol:literal, $maxn:literal, $ml:tt ;
     $($tail:tt)*) => {
        $crate::parse_inline!(
            @collect ($state, $src, $start, $le, $tx, $merge_il, $esc, $sep, $tab, $eol, $maxn, $ml, [], false)
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
               $tx:ident, $merge_il:tt, $esc:literal, $sep:literal, $tab:literal, $eol:literal, $maxn:literal, $ml:tt, $holes:tt, $crl:tt)
     (hard_break: )
     finders  = [$($fi:tt)*]
     sy_rules = [$($sr:tt)*]
     as_rules = [$($ar:tt)*]
     ch_rules = [$($cr:tt)*]
     kv_rules = [$($kv:tt)*]
     tail = [hard_break($hb_esc:literal, $sp:literal, $sp_min:literal) => $hb:ident ; $($rest:tt)*]
    ) => {
        $crate::parse_inline!(
            @collect ($st, $src, $s, $le, $tx, $merge_il, $esc, $sep, $tab, $eol, $maxn, $ml, $holes, $crl)
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
               $tx:ident, $merge_il:tt, $esc:literal, $sep:literal, $tab:literal, $eol:literal, $maxn:literal, $ml:tt, $holes:tt, $crl:tt)
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
            @collect ($st, $src, $s, $le, $tx, $merge_il, $esc, $sep, $tab, $eol, $maxn, $ml, $holes, $crl)
            (hard_break: $($hb)*)
            finders  = [$($fi)* { $($fn_b),+ $(, $kv_end)* }]
            sy_rules = [$($sr)* $( ($sb, $pi, { $( $sn => $sf ),* }) )*]
            as_rules = [$($ar)* $( ($ao, $ac, $abal, $api, { $( $an => $af ),* }) )*]
            ch_rules = [$($cr)* $( ($co, $cc, $tpi, $tbal, $uo, $uc, $upi, $ubal, $cp, $cpi => $ct, $cu, $ch_ty, $cf) )*]
            kv_rules = [$($kv)* $( ($kv_eq, $kv_allow, $kv_end, $kv_kf, $kv_vf, $kv_ty, $kv_f) )*]
            tail = [$($rest)*]
        )
    };

    // Transition: all sections consumed — flatten buckets and enter @body.
    (
        @collect ($st:ident, $src:ident, $s:expr, $le:expr,
                  $tx:ident, $merge_il:tt, $esc:literal, $sep:literal, $tab:literal, $eol:literal, $maxn:literal, $ml:tt, $holes:tt, $crl:tt)
        (hard_break: $($hb:tt)*)
        finders  = [$($fi:tt)*]
        sy_rules = [$( ($sb:literal, $pi:tt, { $( $sn:tt => $sf:ident ),* }) )*]
        as_rules = [$( ($ao:literal, $ac:literal, $abal:tt, $api:tt, { $( $an:tt => $af:ident ),* }) )*]
        ch_rules = [$( ($co:literal, $cc:literal, $tpi:tt, $tbal:tt, $uo:literal, $uc:literal,
                        $upi:tt, $ubal:tt, $cp:literal, $cpi:ident => $ct:ident, $cu:ident,
                        $ch_ty:ident, $cf:ident) )*]
        kv_rules = [$( ($kv_eq:literal, $kv_allow:tt, $kv_end:literal,
                        $kv_kf:ident, $kv_vf:ident, $kv_ty:ident, $kv_f:ident) )*]
        tail = []
    ) => {
        $crate::parse_inline!(@body ($st, $src, $s, $le, $tx, $merge_il, $esc, $sep, $tab, $eol, $maxn, $ml, $holes, $crl)
            (hard_break: $($hb)*)
            finders  = [$($fi)*]
            sy_rules = [$( $sb, $pi, { $( $sn => $sf ),* } )*]
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
               $tx:ident, $merge_il:tt, $esc:literal, $sep:literal, $tab:literal, $eol:literal, $maxn:literal, $ml:tt, $holes:tt, $crl:tt)
        (hard_break: $($hb_esc:literal, $sp:literal, $sp_min:literal => $hb:ident)*)
        finders  = [$( { $($fn_b:literal),+ } )*]
        sy_rules = [$( $sb:literal, $pi:tt, { $( $sn:tt => $sf:ident ),* } )*]
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
        // Every `on_trigger` byte.
        #[allow(dead_code)]
        const _TRIGGERS: &[u8] = &[$($($fn_b),+),*];
        // Does the grammar have a `key_value` rule? Without one, the tests of
        // the top frame for it fold away.
        const _HAS_KV: bool = false $( || { let _ = $kv_end; true } )*;
        // With `crlf`, the `\r` right before an internal `eol` is part of the
        // line end, and so of no element or text.
        #[allow(dead_code)]
        const _CRLF: bool = $crl;
        // Where the content of the line ending at the `eol` at `$e` ends.
        #[allow(unused_macros)]
        macro_rules! content_end {
            ($e:expr) => {{
                let _e: usize = $e;
                if _CRLF && _e > $start && src[_e - 1] == b'\r' { _e - 1 } else { _e }
            }};
        }
        // Is `eol` a `key_value` rule's `end`? The `end` check owns it then.
        #[allow(dead_code)]
        const _KV_EOL: bool = false $( || $kv_end == $eol )*;

        // The line prefixes of a `cont` run: every line after the first starts
        // with the markers of the `_cd` frames the run is in, which belong to
        // no element. `skip_markers!` moves a line start past them; `in_hole!`
        // tells a position among them. Both are no-ops for any other run,
        // where `_cd` is `0`.
        let _cd: usize = $crate::parse_inline!(@cdepth $holes);
        #[allow(unused_macros)]
        macro_rules! skip_markers {
            ($ls:ident) => {
                $crate::parse_inline!(@cskip $holes, src, $ls, parse_end, $sep, $tab)
            };
        }
        #[allow(unused_macros)]
        macro_rules! in_hole {
            ($p:expr) => {
                $crate::parse_inline!(@inhole $holes, src, $p, $start, parse_end, $eol, $sep, $tab)
            };
        }

        macro_rules! push_il {
            ($field:ident, $span:expr) => {
                $crate::parse_text!(@dispatch $state, $field, $span, $start as u32, $merge_il)
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
        // Close byte and field routing are recovered from `byte` by matching
        // it against each rule's literal.
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
        // Delimiter-inclusive bounds of the last opaque span emitted in this
        // run. A key that ends where this span ends is the span itself.
        let mut _kv_opaque: Option<(usize, usize)> = None;

        // Hard-break detection at the end of the run: an `esc` byte that is
        // not itself escaped, or at least `min` `sp` bytes, is the marker and
        // is left out of the scan. Fewer `sp` bytes stay text.
        let _hb = 'hb: {
            $(
                let _mk = $crate::parse_inline!(@hb_mark src, parse_end, $start, $esc,
                    $hb_esc, $sp, $sp_min);
                if _mk < parse_end {
                    parse_end = _mk;
                    break 'hb true;
                }
            )*
            false
        };

        let mut pos: usize = $start;
        let mut text_start: usize = $start;
        // Taken when a transparent `chained` match opens: the fallback
        // snapshot (`hold_base!`) the match rolls back to when it completes,
        // the text cursor and the stack depth. `_ch_low` is the lowest depth
        // the stack has reached since (see `pop_frame!`).
        let mut _ch_base: (u32, u32) = (0u32, 0u32);
        let mut _ch_text_start: usize = $start;
        let mut _ch_depth: usize = 0;
        let mut _ch_low: usize = 0;
        // Pop the top frame on a normal close, lower `_ch_low` when the stack
        // drops below it, and roll the fallback vector back to the frame's
        // snapshot. A frame open since before a chained match rolls back
        // below the match's own snapshot, which then follows it down.
        #[allow(unused_macros)]
        macro_rules! pop_frame {
            () => {{
                fdepth -= 1;
                if fdepth < _ch_low {
                    _ch_low = fdepth;
                }
                release_held!(held_base[fdepth]);
                if held_base[fdepth] < _ch_base {
                    _ch_base = held_base[fdepth];
                }
            }};
        }

        // Two-phase transparent state for `chained` (off the stack).
        let mut ch_in_first: bool = false;
        let mut ch_first_opaque: bool = false;
        let mut ch_first_depth: u32 = 0;
        let mut ch_first_start: u32 = 0;
        let mut ch_is_prefix: bool = false;
        let mut ch_real_start: u32 = 0;

        let mut ch_in_second: bool = false;
        let mut ch_second_opaque: bool = false;
        let mut ch_second_depth: u32 = 0;
        let mut ch_second_start: u32 = 0;
        let mut ch_saved_first_end: u32 = 0;

        // Close bytes known to be absent (unescaped) from part of the run.
        // The forward searches below (asymmetric with `balanced = false` and
        // `parse_inside = false`; opaque `chained` components) scan up to
        // `parse_end`, which is fixed, so a search that meets no close byte
        // tells that it is absent from its start on.
        // `_dead_close` holds, one bit per byte value, the bytes absent from
        // the scan position on: their search started there, and `pos` moves
        // back only to read the rest of a delimiter run again, never before a
        // search already made. The second component's search starts past the
        // first component, ahead of the scan: its byte and start go to
        // `_dead_ahead`, and a later search stops where that one started.
        let mut _dead_close: [u64; 4] = [0u64; 4];
        let mut _dead_ahead: (u8, usize) = (0u8, usize::MAX);
        // End of the range a search for `$b` from `$s` still has to scan:
        // `$s` itself when the byte is known absent from there on.
        #[allow(unused_macros)]
        macro_rules! dead_limit {
            ($b:expr, $s:expr) => {{
                let _s: usize = $s;
                if (_dead_close[($b as usize) >> 6] >> (($b as usize) & 63)) & 1 == 1 {
                    _s
                } else if _dead_ahead.0 == $b && _dead_ahead.1 < parse_end {
                    if _dead_ahead.1 > _s { _dead_ahead.1 } else { _s }
                } else {
                    parse_end
                }
            }};
        }
        // Record that `$b` is absent from `$s` on, keeping the earliest start
        // known. An entry `_dead_ahead` drops for another byte joins
        // `_dead_close` when the scan has reached its start.
        #[allow(unused_macros)]
        macro_rules! dead_close_set {
            ($b:expr, $s:expr) => {{
                let _s: usize = $s;
                let _w = ($b as usize) >> 6;
                let _m = 1u64 << (($b as usize) & 63);
                if _dead_close[_w] & _m == 0 {
                    if _s <= pos {
                        _dead_close[_w] |= _m;
                    } else if _dead_ahead.0 != $b || _dead_ahead.1 > _s {
                        if _dead_ahead.0 != $b && _dead_ahead.1 <= pos {
                            let _ob = _dead_ahead.0 as usize;
                            _dead_close[_ob >> 6] |= 1u64 << (_ob & 63);
                        }
                        _dead_ahead = ($b, _s);
                    }
                }
            }};
        }
        // Failed searches of `balanced = true` opaque asymmetric rules: per
        // slot, one for each such rule, the rule's open byte, where the search
        // stopped (`0` marks a free slot) and a cursor into `_bal_open`, the
        // opens inside the search that no close balances, ascending. An
        // opener of the rule inside that range fails when it is one of them
        // and closes otherwise, so no range is searched twice in vain. A
        // grammar without such rules has no slot.
        const _NBAL: usize = 0 $( + (($abal && !$api) as usize) )*;
        let mut _bal_memo: [(u8, usize, usize); _NBAL] = [(0u8, 0usize, 0usize); _NBAL];
        let mut _bal_open: [::std::vec::Vec<u32>; _NBAL] =
            ::core::array::from_fn(|_| ::std::vec::Vec::new());
        let mut _bal_next: usize = 0;
        // Close bytes found: `(byte, from, at)` says that the first unescaped
        // byte at or after any position in `from..=at` is at `at`. An opaque
        // `chained` match that fails after its first close fills it: a later
        // opener of the same rule before that close searches for it again.
        // Four close bytes are kept; `from == usize::MAX` marks a free slot.
        let mut _found_close: [(u8, usize, usize); 4] = [(0u8, usize::MAX, 0usize); 4];
        let mut _found_next: usize = 0;
        // The close a search for `$b` from `$s` finds, when already known.
        #[allow(unused_macros)]
        macro_rules! found_close {
            ($b:expr, $s:expr) => {{
                let _s: usize = $s;
                let mut _r: Option<usize> = None;
                for _e in _found_close.iter() {
                    if _e.0 == $b && _e.1 <= _s && _s <= _e.2 {
                        _r = Some(_e.2);
                        break;
                    }
                }
                _r
            }};
        }
        // Record that a search for `$b` from `$s` found it at `$p`. A byte
        // keeps its slot; a new byte takes a free slot, or the oldest one.
        #[allow(unused_macros)]
        macro_rules! found_close_set {
            ($b:expr, $s:expr, $p:expr) => {{
                let (_s, _p): (usize, usize) = ($s, $p);
                let mut _k = usize::MAX;
                for (_i, _e) in _found_close.iter().enumerate() {
                    if _e.0 == $b && _e.1 != usize::MAX {
                        _k = _i;
                        break;
                    }
                }
                if _k == usize::MAX {
                    _k = _found_next;
                    _found_next = (_found_next + 1) & 3;
                    _found_close[_k] = ($b, _s, _p);
                } else if _found_close[_k].2 == _p {
                    if _s < _found_close[_k].1 {
                        _found_close[_k].1 = _s;
                    }
                } else {
                    _found_close[_k] = ($b, _s, _p);
                }
            }};
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
            let _top_is_kv = _HAS_KV && fdepth > 0 && {
                let _tb = frames[fdepth - 1].0;
                let mut _k = false;
                $( if _tb == $kv_end { _k = true; } )*
                _k
            };

            // ---------------------------------------------------------------- //
            // Internal eol of a multi-line run. Present only when `hard_break` //
            // is declared; otherwise the eol falls through the dispatch below  //
            // as an unmatched byte. Runs before the escape check: an `esc`     //
            // byte before eol that is not itself escaped is the hard-break     //
            // marker, as is a run of at least `min` `sp` bytes. The marker is  //
            // trimmed, the hard-break span emitted, text flushed up to the     //
            // trim point and the scan continues; the stack is not drained.     //
            // Fewer `sp` bytes stay text. The end-of-run check above handles   //
            // input that ends without an eol and the single-line calls.        //
            //                                                                  //
            // Skipped when a kv frame is on top: `end` and `eol` may be the    //
            // same byte, and the `$kv_end` check below owns that occurrence.   //
            // ---------------------------------------------------------------- //
            $(
                if src[pos] == $eol && !_top_is_kv {
                    let _ce = content_end!(pos);
                    let _ep = $crate::parse_inline!(@hb_mark src, _ce, $start, $esc,
                        $hb_esc, $sp, $sp_min);
                    let _ehb = _ep < _ce;
                    if text_start < _ep {
                        push_il!($tx, $crate::span::Span::new(text_start as u32, _ep as u32));
                    }
                    if _ehb {
                        $crate::paste::paste! {
                            $state.[<push_ $hb>]($crate::span::Span::new(_ep as u32, _ep as u32));
                        }
                    }
                    pos += 1;
                    skip_markers!(pos);
                    text_start = pos;
                    if _cd > 0 {
                        _kv_seg_start = pos;
                    }
                    continue;
                }
            )*
            // The same for a `cont` run with no `hard_break`, or with a
            // `key_value` frame on top whose `end` is no `eol`: the text before
            // the `eol` is flushed and the scan goes on past the next line's
            // markers.
            if _cd > 0 && src[pos] == $eol && !(_top_is_kv && _KV_EOL) {
                let _ce = content_end!(pos);
                if text_start < _ce {
                    push_il!($tx, $crate::span::Span::new(text_start as u32, _ce as u32));
                }
                pos += 1;
                skip_markers!(pos);
                text_start = pos;
                _kv_seg_start = pos;
                continue;
            }

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
            // A run that reaches an internal `eol` stops at that eol's
            // hard-break marker, as the scan stops at the marker before the
            // run's end: no other rule reads a marker byte. A run that is all
            // marker leaves the scan at the eol. Only a grammar whose marker
            // bytes are trigger bytes has such runs.
            $(
                const _HB_CLIP: bool = {
                    let mut _f = false;
                    let mut _i = 0;
                    while _i < _TRIGGERS.len() {
                        if _TRIGGERS[_i] == $sp || _TRIGGERS[_i] == $hb_esc {
                            _f = true;
                        }
                        _i += 1;
                    }
                    _f
                };
                if _HB_CLIP
                    && (delim == $sp || delim == $hb_esc)
                    && pos < parse_end
                    && (src[pos] == $eol
                        || (_CRLF && src[pos] == b'\r' && pos + 1 < parse_end
                            && src[pos + 1] == $eol))
                    && !_top_is_kv
                {
                    let _mk = $crate::parse_inline!(@hb_mark src, pos, $start, $esc,
                        $hb_esc, $sp, $sp_min);
                    if _mk <= delim_start as usize {
                        continue;
                    }
                    if _mk < pos {
                        count -= (pos - _mk) as u32;
                        pos = _mk;
                    }
                }
            )*

            // ---------------------------------------------------------- //
            // key_value terminator (`$kv_end`) at the value's own level:  //
            // fires only with a kv frame on top of the stack. The value    //
            // ends before the separator. With no kv frame on top the byte   //
            // is left alone (a bare array's `,` between elements is         //
            // content). When `end` is also `sep`, the bytes skipped before  //
            // the value starts are not a terminator.                        //
            // -----------------------------------------------------------------//
            let mut _kv_hit = false;
            $(
                if delim == $kv_end
                    && fdepth > 0
                    && frames[fdepth - 1].0 == $kv_end
                    && delim_start >= kv_pending[fdepth - 1].2
                {
                    let (_ks, _ke, _vs) = kv_pending[fdepth - 1];
                    $crate::paste::paste! {
                        $state.[<push_ $kv_f>]($kv_ty {
                            $kv_kf: $crate::span::Span::new(_ks, _ke),
                            $kv_vf: $crate::span::Span::new(_vs, if delim == $eol {
                                (content_end!(delim_start as usize) as u32).max(_vs)
                            } else {
                                delim_start
                            }),
                        });
                    }
                    pop_frame!();
                    _kv_seg_start = pos;
                    _kv_hit = true;
                }
            )*
            if _kv_hit {
                // The first byte of the run ends the value; the rest of the
                // run is read again as content. The next key segment starts
                // past the whole run, and past the next line's markers when
                // the value ended at an `eol` of a `cont` run.
                text_start = delim_start as usize + 1;
                if _cd > 0 && delim == $eol {
                    skip_markers!(text_start);
                    _kv_seg_start = text_start;
                }
                pos = text_start;
                continue;
            }

            // ---------------------------------------------------------- //
            // asymmetric (kind 0): unified open + unified close cascade.  //
            // ---------------------------------------------------------- //
            let _chained_opaque_active =
                (ch_in_first && ch_first_opaque) || (ch_in_second && ch_second_opaque);

            let mut _asym_bal_handled = false;
            if !_chained_opaque_active {
                // --- open side ---
                $(
                    if $api && delim == $ao {
                        // The text before the run is flushed and the text
                        // cursor moves to the run: an open byte stays text
                        // until its frame closes, and one the stack refuses
                        // stays literal text.
                        if text_start < delim_start as usize {
                            push_il!($tx, $crate::span::Span::new(text_start as u32, delim_start));
                        }
                        text_start = delim_start as usize;
                        for _k in 0..count {
                            let _char_pos = delim_start + _k;
                            // `balanced = false`: one frame of the rule at a
                            // time, at any depth the stack has room for.
                            if fdepth < $maxn
                                && ($abal || !frames[..fdepth].iter().any(|_f| _f.0 == $ao))
                            {
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
                                    } )*
                                    _ => {}
                                }
                            } else if $abal && fdepth > 0
                                && frames[fdepth - 1].0 == $ao
                            {
                                asym_overflow += 1;
                            }
                        }
                        // A key begins after the run.
                        _kv_seg_start = pos;
                        _asym_bal_handled = true;
                    }
                )*

                // --- close side: single unified pass, with kv drain. ---
                if !_asym_bal_handled {
                    let mut _asym_is_close_byte = false;
                    $( if $api && delim == $ac { _asym_is_close_byte = true; } )*

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
                                    $( if $api && _below_b == $ao && $ac == delim {
                                        _below_closes_here = true;
                                    } )*
                                    if _below_closes_here {
                                        let (_ks, _ke, _vs) = kv_pending[fdepth - 1];
                                        $(
                                            if _top_b == $kv_end {
                                                $crate::paste::paste! {
                                                    $state.[<push_ $kv_f>]($kv_ty {
                                                        $kv_kf: $crate::span::Span::new(_ks, _ke),
                                                        $kv_vf: $crate::span::Span::new(_vs, _close_char_pos),
                                                    });
                                                }
                                            }
                                        )*
                                        pop_frame!();
                                    }
                                }
                            }

                            if fdepth > 0 {
                                let _ob = frames[fdepth - 1].0;
                                let mut _closes_here = false;
                                $( if $api && _ob == $ao && $ac == delim {
                                    _closes_here = true;
                                } )*
                                if _closes_here {
                                    if asym_overflow > 0 {
                                        // The close pairs with an open absorbed
                                        // past the cap: like that open, it is
                                        // content of the frame on the stack.
                                        asym_overflow -= 1;
                                    } else {
                                        let _vidx = frames[fdepth - 1].2;
                                        $(
                                            if $api && _ob == $ao {
                                                match 1u32 {
                                                    $( $an => {
                                                        $state.$af[_vidx as usize].end = _close_char_pos;
                                                    } )*
                                                    _ => {}
                                                }
                                            }
                                        )*
                                        pop_frame!();
                                        asym_overflow = 0;
                                        text_start = (_close_char_pos + 1) as usize;
                                        _kv_seg_start = text_start;
                                    }
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

            // ---------------------------------------------------------- //
            // chained, transparent phases (off the stack).               //
            // ---------------------------------------------------------- //
            // The phase state belongs to the rule with a transparent
            // component; the other `chained` rules leave it alone.
            let mut _chained_handled = false;
            $(
                if ($tpi || $upi) && !ch_in_first && !ch_in_second && delim == $co
                {
                    // The last byte of the run opens the match; the bytes
                    // before it are text.
                    let _open = pos - 1;
                    let _is_prefix = _open > $start
                        && src[_open - 1] == $cp
                        && !in_hole!(_open - 1)
                        && {
                            let mut _bs: u32 = 0;
                            let mut _i = _open - 1;
                            while _i > $start && src[_i - 1] == $esc { _bs += 1; _i -= 1; }
                            _bs % 2 == 0
                        };
                    let _real_start = if _is_prefix { _open - 1 } else { _open };
                    // The text cursor stays: text flushed inside the match,
                    // opener included, is rolled back to this snapshot when
                    // the match completes and stays when it fails.
                    _ch_base = hold_base!();
                    _ch_text_start = text_start;
                    _ch_depth = fdepth;
                    _ch_low = fdepth;
                    ch_in_first = true;
                    ch_first_opaque = !$tpi;
                    ch_first_depth = 0;
                    ch_first_start = pos as u32;
                    ch_is_prefix = _is_prefix;
                    ch_real_start = _real_start as u32;
                    _chained_handled = true;
                } else if ($tpi || $upi) && ch_in_first && delim == $co && $tbal {
                    // Each byte of the run nests one level.
                    ch_first_depth += count;
                    _chained_handled = true;
                } else if ($tpi || $upi) && ch_in_first && delim == $cc {
                    // Each byte of the run closes one nested level; the next
                    // one closes the first component, and the second component
                    // must open on the byte right after it.
                    if count <= ch_first_depth {
                        ch_first_depth -= count;
                    } else {
                        let _ct_end = delim_start + ch_first_depth;
                        let _next = _ct_end as usize + 1;
                        ch_first_depth = 0;
                        ch_in_first = false;
                        if _next < parse_end && src[_next] == $uo {
                            ch_in_second = true;
                            ch_second_opaque = !$upi;
                            ch_second_depth = 0;
                            ch_second_start = (_next + 1) as u32;
                            ch_saved_first_end = _ct_end;
                            pos = _next + 1;
                        } else {
                            // The match fails and its bytes, opener included,
                            // stay text. The rest of the run is read again.
                            pos = _next;
                        }
                    }
                    _chained_handled = true;
                } else if ($tpi || $upi) && ch_in_second && delim == $uo && $ubal {
                    ch_second_depth += count;
                    _chained_handled = true;
                } else if ($tpi || $upi) && ch_in_second && delim == $uc {
                    // As for the first component; the rest of the run after
                    // the closing byte is read again.
                    if count <= ch_second_depth {
                        ch_second_depth -= count;
                    } else {
                        let _cu_end = delim_start + ch_second_depth;
                        ch_second_depth = 0;
                        ch_in_second = false;
                        pos = _cu_end as usize + 1;
                        // The match covers the text flushed inside it: roll it
                        // back and emit the text before the opener once, unless
                        // a frame open at the match's open has closed inside
                        // it and owns that text. A frame opened since and
                        // still open gets the snapshot of the vector as it now
                        // ends.
                        release_held!(_ch_base);
                        if _ch_low >= _ch_depth && _ch_text_start < ch_real_start as usize {
                            push_il!($tx, $crate::span::Span::new(_ch_text_start as u32, ch_real_start));
                        }
                        let _after = hold_base!();
                        for _k in _ch_low..fdepth {
                            held_base[_k] = _after;
                        }
                        $crate::paste::paste! {
                            $state.[<push_ $cf>]($ch_ty {
                                $cpi: ch_is_prefix,
                                $ct: $crate::span::Span::new(ch_first_start, ch_saved_first_end),
                                $cu: $crate::span::Span::new(ch_second_start, _cu_end),
                            });
                        }
                        text_start = pos;
                    }
                    _chained_handled = true;
                }
            )*
            if _chained_handled {
                continue;
            }

            if _chained_opaque_active {
                continue;
            }

            // --- chained, both components opaque: self-contained forward search ---
            // A rule with a transparent component has its open bytes read by
            // the phases above: inside its own match they open nothing.
            $(
                if !$tpi && !$upi && delim == $co {
                    // As above: the last byte of the run opens the match.
                    let _open = pos - 1;
                    let is_prefix = _open > $start
                        && src[_open - 1] == $cp
                        && !in_hole!(_open - 1)
                        && {
                            let mut _bs: u32 = 0;
                            let mut _i = _open - 1;
                            while _i > $start && src[_i - 1] == $esc { _bs += 1; _i -= 1; }
                            _bs % 2 == 0
                        };
                    let mut _i = pos;
                    let close_first: Option<usize> = if $tbal {
                        // Past the point where the close byte is known absent
                        // the depth cannot return to zero. A search that meets
                        // no close byte records it.
                        let _lim = dead_limit!($cc, pos);
                        let mut _depth: i32 = 1;
                        let mut _met = false;
                        let mut _found: Option<usize> = None;
                        while let Some(_h) = $crate::memchr::memchr2($co, $cc, &src[_i.._lim]) {
                            let _q = _i + _h;
                            _i = _q + 1;
                            if $crate::parse_inline!(@is_escaped src, _q, $start, $esc)
                                || in_hole!(_q)
                            {
                                continue;
                            }
                            // A close byte that is also the open byte nests,
                            // yet the byte is present.
                            if src[_q] == $cc { _met = true; }
                            if src[_q] == $co { _depth += 1; }
                            else {
                                _depth -= 1;
                                if _depth == 0 { _found = Some(_q); break; }
                            }
                        }
                        if !_met {
                            dead_close_set!($cc, pos);
                        }
                        _found
                    } else if let Some(_at) = found_close!($cc, pos) {
                        Some(_at)
                    } else {
                        let _lim = dead_limit!($cc, pos);
                        let mut _found: Option<usize> = None;
                        while let Some(_h) = $crate::memchr::memchr($cc, &src[_i.._lim]) {
                            let _q = _i + _h;
                            _i = _q + 1;
                            if !$crate::parse_inline!(@is_escaped src, _q, $start, $esc)
                                && !in_hole!(_q)
                            {
                                _found = Some(_q);
                                break;
                            }
                        }
                        if _found.is_none() {
                            dead_close_set!($cc, pos);
                        }
                        _found
                    };
                    if let Some(ct_end) = close_first {
                        let next = ct_end + 1;
                        if next < parse_end && src[next] == $uo {
                            let mut _j = next + 1;
                            let close_second: Option<usize> = if $ubal {
                                // As for the first component.
                                let _lim = dead_limit!($uc, next + 1);
                                let mut _depth: i32 = 1;
                                let mut _met = false;
                                let mut _found: Option<usize> = None;
                                while let Some(_h) =
                                    $crate::memchr::memchr2($uo, $uc, &src[_j.._lim])
                                {
                                    let _q = _j + _h;
                                    _j = _q + 1;
                                    if $crate::parse_inline!(@is_escaped src, _q, $start, $esc)
                                        || in_hole!(_q)
                                    {
                                        continue;
                                    }
                                    if src[_q] == $uc { _met = true; }
                                    if src[_q] == $uo { _depth += 1; }
                                    else {
                                        _depth -= 1;
                                        if _depth == 0 { _found = Some(_q); break; }
                                    }
                                }
                                if !_met {
                                    dead_close_set!($uc, next + 1);
                                }
                                _found
                            } else {
                                let _lim = dead_limit!($uc, next + 1);
                                let mut _found: Option<usize> = None;
                                while let Some(_h) = $crate::memchr::memchr($uc, &src[_j.._lim]) {
                                    let _q = _j + _h;
                                    _j = _q + 1;
                                    if !$crate::parse_inline!(@is_escaped src, _q, $start, $esc)
                                        && !in_hole!(_q)
                                    {
                                        _found = Some(_q);
                                        break;
                                    }
                                }
                                if _found.is_none() {
                                    dead_close_set!($uc, next + 1);
                                }
                                _found
                            };
                            if let Some(cu_end) = close_second {
                                let real_start = if is_prefix { _open - 1 } else { _open };
                                if text_start < real_start {
                                    push_il!($tx, $crate::span::Span::new(
                                    text_start as u32, real_start as u32));
                                }
                                $crate::paste::paste! {
                                    $state.[<push_ $cf>]($ch_ty {
                                        $cpi: is_prefix,
                                        $ct:  $crate::span::Span::new(pos as u32, ct_end as u32),
                                        $cu:  $crate::span::Span::new((next + 1) as u32, cu_end as u32),
                                    });
                                }
                                pos = cu_end + 1;
                                text_start = pos;
                                _kv_opaque = Some((real_start, pos));
                                continue;
                            }
                        }
                        // The match failed after its first close: an opener of
                        // this rule before that close finds the same one.
                        if !$tbal {
                            found_close_set!($cc, pos, ct_end);
                        }
                    }
                    continue;
                }
            )*

            // --- symmetric: on the stack, or a forward search when opaque ---
            $(
                if delim == $sb {
                    if $pi {
                        let _matches_top = fdepth > 0
                            && frames[fdepth - 1].0 == $sb
                            && frames[fdepth - 1].1 == count as u8;

                        if _matches_top {
                            let _vidx = frames[fdepth - 1].2;
                            match count {
                                $( $sn => {
                                    $state.$sf[_vidx as usize].end = delim_start;
                                    pop_frame!();
                                    text_start = pos;
                                    _kv_seg_start = pos;
                                } )*
                                _ => {}
                            }
                        } else if fdepth < $maxn {
                            // The run stays text until its frame closes. A
                            // run with no arm for its count, or with the
                            // stack full, is literal text.
                            match count {
                                $( $sn => {
                                    if text_start < delim_start as usize {
                                        push_il!($tx, $crate::span::Span::new(
                                        text_start as u32, delim_start));
                                    }
                                    let _vidx = $state.$sf.len() as u32;
                                    push_il!($sf, $crate::span::Span::new(pos as u32, pos as u32));
                                    frames[fdepth] = ($sb, count as u8, _vidx);
                                    held_base[fdepth] = hold_base!();
                                    fdepth += 1;
                                    text_start = delim_start as usize;
                                    _kv_seg_start = pos;
                                } )*
                                _ => {}
                            }
                        }
                    } else {
                        // A run with no arm for its count is literal text, not
                        // an opener: no forward search, nothing skipped.
                        if !(match count { $( $sn => true, )* _ => false }) {
                            continue;
                        }
                        // The close is the next unescaped run of the same
                        // count; a run of another count is content.
                        let cs = pos;
                        let mut _i = pos;
                        let mut close: Option<(usize, usize)> = None;
                        loop {
                            match $crate::memchr::memchr($sb, &src[_i..parse_end]) {
                                None => break,
                                Some(r) => {
                                    let p = _i + r;
                                    if $crate::parse_inline!(@is_escaped src, p, $start, $esc)
                                        || in_hole!(p)
                                    {
                                        _i = p + 1;
                                        continue;
                                    }
                                    let mut c: u32 = 0;
                                    let mut tmp = p;
                                    while tmp < parse_end && src[tmp] == $sb {
                                        c += 1; tmp += 1;
                                    }
                                    if c == count { close = Some((p, tmp)); break; }
                                    _i = tmp;
                                }
                            }
                        }
                        if let Some((p, end)) = close {
                            if text_start < delim_start as usize {
                                push_il!($tx, $crate::span::Span::new(
                                text_start as u32, delim_start));
                            }
                            let clean = $crate::span::Span::new(cs as u32, p as u32);
                            match count { $( $sn => { push_il!($sf, clean); } )* _ => {} }
                            pos = end;
                            text_start = end;
                            _kv_opaque = Some((delim_start as usize, end));
                        }
                    }
                    continue;
                }
            )*

            // --- asymmetric, parse_inside = false: forward search ---
            $(
                if !$api && $abal && delim == $ao {
                    // Each byte of the run is an open of its own: the first one
                    // opens the outer ring, the rest nest in it, and when the
                    // ring never closes the next byte tries. A run that opens
                    // no ring is text.
                    let mut _ring: Option<(usize, usize)> = None;
                    let mut _o = delim_start as usize;
                    while _o < pos {
                        let _lim = dead_limit!($ac, _o + 1);
                        // A failed search of this rule over `_o` tells whether
                        // `_o` balances.
                        let mut _m = usize::MAX;
                        for (_k, _e) in _bal_memo.iter().enumerate() {
                            if _e.0 == $ao && _o < _e.1 {
                                _m = _k;
                                break;
                            }
                        }
                        let mut _fails = _lim <= _o + 1;
                        if !_fails && _m != usize::MAX {
                            let _v = &_bal_open[_m];
                            let mut _c = _bal_memo[_m].2;
                            while _c < _v.len() && (_v[_c] as usize) < _o {
                                _c += 1;
                            }
                            _bal_memo[_m].2 = _c;
                            _fails = _c < _v.len() && _v[_c] as usize == _o;
                        }
                        if !_fails {
                            // A search no failed one covers records the opens
                            // inside it that no close balances: on the stack of
                            // a slot of its own while it runs, ascending when it
                            // ends.
                            let _rec = _m == usize::MAX;
                            let mut _r = 0usize;
                            if _rec {
                                _r = _bal_next;
                                for (_k, _e) in _bal_memo.iter().enumerate() {
                                    if _e.0 == $ao {
                                        _r = _k;
                                        break;
                                    }
                                }
                                if _r == _bal_next {
                                    _bal_next = (_bal_next + 1) % _NBAL.max(1);
                                }
                                _bal_memo[_r] = (0u8, 0usize, 0usize);
                                _bal_open[_r].clear();
                            }
                            let mut _depth: usize = 1;
                            let mut _met = false;
                            let mut _i = _o + 1;
                            while let Some(_h) = $crate::memchr::memchr2($ao, $ac, &src[_i.._lim]) {
                                let _q = _i + _h;
                                _i = _q + 1;
                                if $crate::parse_inline!(@is_escaped src, _q, $start, $esc)
                                    || in_hole!(_q)
                                {
                                    continue;
                                }
                                // A close byte that is also the open byte
                                // nests, yet the byte is present.
                                if src[_q] == $ac {
                                    _met = true;
                                }
                                if src[_q] == $ao {
                                    _depth += 1;
                                    if _rec {
                                        _bal_open[_r].push(_q as u32);
                                    }
                                } else {
                                    _depth -= 1;
                                    if _depth == 0 {
                                        _ring = Some((_o, _q));
                                        break;
                                    }
                                    if _rec {
                                        _bal_open[_r].pop();
                                    }
                                }
                            }
                            if _ring.is_some() {
                                break;
                            }
                            if !_met {
                                dead_close_set!($ac, _o + 1);
                            } else if _rec {
                                _bal_memo[_r] = ($ao, _lim, 0usize);
                            }
                        }
                        _o += 1;
                    }
                    if let Some((_o, cp)) = _ring {
                        if text_start < _o {
                            push_il!($tx, $crate::span::Span::new(text_start as u32, _o as u32));
                        }
                        let clean = $crate::span::Span::new((_o + 1) as u32, cp as u32);
                        match 1u32 { $( $an => { push_il!($af, clean); } )* _ => {} }
                        pos = cp + 1;
                        text_start = pos;
                        _kv_opaque = Some((_o, pos));
                    }
                    continue;
                }
                if !$api && delim == $ao {
                    // A run with no arm for its count is literal text, not an
                    // opener: no forward search, nothing skipped.
                    if !(match count { $( $an => true, )* _ => false }) {
                        continue;
                    }
                    let cs = pos;
                    let close_pos: Option<usize> = {
                        let _lim = dead_limit!($ac, pos);
                        let mut _i = pos;
                        let mut _found: Option<usize> = None;
                        loop {
                            match $crate::memchr::memchr($ac, &src[_i.._lim]) {
                                None => break,
                                Some(r) => {
                                    let p = _i + r;
                                    if $crate::parse_inline!(@is_escaped src, p, $start, $esc)
                                        || in_hole!(p)
                                    {
                                        _i = p + 1;
                                    } else {
                                        _found = Some(p);
                                        break;
                                    }
                                }
                            }
                        }
                        if _found.is_none() {
                            dead_close_set!($ac, pos);
                        }
                        _found
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
                        _kv_opaque = Some((delim_start as usize, pos));
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
                    // With the stack full no frame can track the pair: the
                    // key, `eq` and value stay in the current text.
                    if !_top_is_kv && fdepth < $maxn {
                        // Key: back-scan from `eq`, clamped to _kv_seg_start.
                        let mut key_end = delim_start as usize;
                        if $kv_allow {
                            while key_end > _kv_seg_start && src[key_end - 1] == $sep {
                                key_end -= 1;
                            }
                        }
                        let mut ks = key_end;
                        let mut key_floor = _kv_seg_start;
                        let mut key_is_opaque = false;
                        if let Some((_os, _oe)) = _kv_opaque {
                            if _oe == key_end && _os >= _kv_seg_start {
                                ks = _os;
                                key_is_opaque = true;
                            } else if _oe > key_floor && _oe <= key_end {
                                key_floor = _oe;
                            }
                        }
                        if !key_is_opaque {
                            while ks > key_floor
                                && src[ks - 1] != $sep
                                && src[ks - 1] != $tab
                                && src[ks - 1] != $eol
                            {
                                ks -= 1;
                            }
                        }
                        // The first byte of the run is `eq`; the rest of the
                        // run is value content.
                        let _eq_end = delim_start as usize + 1;
                        let mut val_start = _eq_end;
                        if $kv_allow {
                            while val_start < parse_end && src[val_start] == $sep {
                                val_start += 1;
                            }
                        }
                        if text_start < ks {
                            push_il!($tx, $crate::span::Span::new(text_start as u32, ks as u32));
                        }
                        kv_pending[fdepth] = (ks as u32, key_end as u32, val_start as u32);
                        frames[fdepth] = ($kv_end, 0u8, 0u32);
                        held_base[fdepth] = hold_base!();
                        fdepth += 1;
                        text_start = _eq_end;
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
        //  - asymmetric and symmetric : discard via `Vec::remove(vidx)`; a     //
        //    closed inner entry may sit above a still-open outer one in the    //
        //    same field. Frames go top-down, so a removal never shifts the     //
        //    placeholder of a frame still to be drained.                       //
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
                    $crate::paste::paste! {
                        $state.[<push_ $kv_f>]($kv_ty {
                            $kv_kf: $crate::span::Span::new(_ks, _ke),
                            $kv_vf: $crate::span::Span::new(_vs, parse_end as u32),
                        });
                    }
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
                    if $api && _fb == $ao {
                        match 1u32 {
                            $( $an => { $state.$af.remove(_fv as usize); } )*
                            _ => {}
                        }
                        _matched_asym = true;
                    }
                )*
                if !_matched_asym {
                    $(
                        if $pi && _fb == $sb {
                            match _fc {
                                $( $sn => { $state.$sf.remove(_fv as usize); } )*
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
            $crate::paste::paste! {
                $state.[<push_ $hb>]($crate::span::Span::new(parse_end as u32, parse_end as u32));
            }
        } )*

        // Returns `$le` unchanged. The single-line call site resumes at `$le + 1`
        // itself; the multi-line call site ignores the value.
        $le
    }};

    // ------------------------------------------------------------------ //
    // @cdepth / @cskip: how many markers lead a line of a run, and where  //
    // the content of the line starting at `$ls` begins past them: each    //
    // marker byte of `$cs[..$cd]`, then one optional `sep`/`tab`.         //
    // ------------------------------------------------------------------ //
    (@cdepth []) => { 0usize };
    (@cdepth [$cs:ident, $cd:expr]) => { $cd };
    (@cskip [], $src:ident, $ls:ident, $end:expr, $sep:literal, $tab:literal) => {};
    (@cskip [$cs:ident, $cd:expr], $src:ident, $ls:ident, $end:expr, $sep:literal, $tab:literal) => {{
        let _end: usize = $end;
        let mut _k: usize = 0;
        while _k < $cd {
            if $ls < _end && $src[$ls] == $cs[_k] {
                $ls += if $ls + 1 < _end && ($src[$ls + 1] == $sep || $src[$ls + 1] == $tab) {
                    2
                } else {
                    1
                };
            } else {
                break;
            }
            _k += 1;
        }
    }};
    // @inhole: is `$p` among the markers leading a line of the run after
    // its first? A prefix is at most a marker and a `sep` per frame, so the
    // line start lies at most that far back.
    (@inhole [], $src:ident, $p:expr, $start:expr, $end:expr, $eol:literal, $sep:literal, $tab:literal) => {
        false
    };
    (@inhole [$cs:ident, $cd:expr], $src:ident, $p:expr, $start:expr, $end:expr,
     $eol:literal, $sep:literal, $tab:literal) => {{
        let _p: usize = $p;
        let _lo = _p.saturating_sub(2 * $cd + 1).max($start);
        let mut _j = _p;
        let mut _in = false;
        while _j > _lo {
            _j -= 1;
            if $src[_j] == $eol {
                let mut _ls = _j + 1;
                $crate::parse_inline!(@cskip [$cs, $cd], $src, _ls, $end, $sep, $tab);
                _in = _p < _ls;
                break;
            }
        }
        _in
    }};

    // ------------------------------------------------------------------ //
    // @hb_mark: where the hard-break marker before the line end `$e` of a //
    // run starting at `$start` begins — an `esc` byte that is not itself  //
    // escaped, or a run of at least `min` `sp` bytes — or `$e` itself     //
    // when there is none.                                                //
    // ------------------------------------------------------------------ //
    (@hb_mark $src:ident, $e:expr, $start:expr, $esc:literal,
     $hb_esc:literal, $sp:literal, $sp_min:literal) => {{
        let _e: usize = $e;
        let mut _mk = _e;
        if _e > $start {
            if $src[_e - 1] == $hb_esc
                && !$crate::parse_inline!(@is_escaped $src, _e - 1, $start, $esc)
            {
                _mk = _e - 1;
            } else {
                let mut _p = _e;
                let mut _n: u32 = 0;
                while _p > $start && $src[_p - 1] == $sp {
                    _n += 1; _p -= 1;
                }
                if _n >= $sp_min {
                    _mk = _p;
                }
            }
        }
        _mk
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
