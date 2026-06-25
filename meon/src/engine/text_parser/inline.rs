//! Inline element parser — the `parse_inline!` macro.
//!
//! # Overview
//!
//! `parse_inline!` drives a single-pass scan over one *run* of source text —
//! either a single line (mid-line continuations after a Line/Block match) or
//! a whole multi-line fallthrough span handed to it as one call by
//! `parse_text!` (see its docs for the run-accumulation mechanism) — and
//! emits spans for every inline element it recognises: emphasis, code spans,
//! links, images, autolinks, key-value pairs and hard breaks.
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
//!               merge_flag, escape_byte, sep_byte, tab_byte, eol_byte,
//!               max_nest, multiline_flag ; <rules>)
//! ```
//!
//! `multiline_flag` (`true`/`false`) gates whether `$eol` participates in the
//! unified trigger search at all — see the "Single unified trigger search"
//! section below for why this exists and what it costs to get wrong in
//! either direction. `parse_text!`'s two call sites pass the correct value
//! for their own case (`true` for the whole-run multi-line call, `false` for
//! the single-line-bounded mid-line-continuation call); a caller invoking
//! `parse_inline!` directly (e.g. the engine's own unit tests) should pass
//! `true` unless it can prove `[start, line_end)` is `\n`-free by
//! construction, the same way `parse_text!`'s single-line call site can.
//!
//! Rules inside the `inline` section are:
//!
//! - `merge_simple = true | false;`  — whether adjacent text spans are merged.
//! - `fallback => field;`            — field that receives plain-text spans.
//! - `hard_break(esc, sp, min) => field;` — hard-break detection, checked at
//!   the scanned run's end and, when the run spans multiple lines, at every
//!   internal `\n` inside it too (see the multi-line section below).
//! - `on_trigger(b1, …) { <inline rules> }` — byte-triggered inline block.
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
//! # Bounded nesting (`max_nest`) — one unified stack, one minimal frame
//!
//! Every construct that needs to track *how deeply it is nested* — asymmetric,
//! symmetric, **and** key_value alike — lives on a **single shared stack**
//! (`frames`), bounded by the grammar's `max_nest` depth cap (forwarded from
//! `parse_text!`; defaults to `1`). A stack entry is a plain
//! `(u8, u8, u32)` tuple — byte, count-at-open, back-patch index — nothing
//! more. There is no stored kind tag and no per-kind dead field: every other
//! fact about a frame (its close byte, its opacity, which output field it
//! routes to) is already known to the grammar at compile time and is
//! recovered, at the point of use, by matching the stored byte against each
//! rule's own literal — the exact idiom the *open* side already used to
//! route a field via `match 1u32 { $( $an => … ) }`; the close/discard/
//! opacity sides now use the same idiom instead of reading a separately
//! stored copy of the same fact.
//!
//! - **byte** — asymmetric/symmetric: the rule's own delimiter byte (its
//!   open byte for asymmetric, since the close byte is recoverable from it
//!   via the grammar's own `$ao`/`$ac` pairing; the delimiter itself for
//!   symmetric, open and close being the same byte). key_value: `$kv_end`,
//!   its terminator byte. A byte value is assumed unique in *meaning* across
//!   every stack-eligible rule in a grammar's `on_trigger` block — the same
//!   assumption every trigger-byte dispatch in this engine already makes;
//!   nothing new is introduced by frames sharing this assumption too.
//! - **count** — the repetition count recorded at open. Asymmetric: always
//!   `1` (each byte of an open run is its own event, per the existing
//!   per-character open loop). Symmetric: the real run length, needed both
//!   to pick which field this is (`1`/`2`/`3` ⇒ italics/bolds/bold_italics)
//!   and to require an exact-count match on close. key_value: unused (`0`).
//! - **vidx** — back-patch index into this frame's own already-pushed
//!   placeholder. Asymmetric/symmetric: the placeholder span (`start ==
//!   end`) pushed at open time, exactly as before — `vidx` is where to
//!   write its `.end` on close, and pushing at open time is what keeps
//!   their own output vectors in open/start order (an outer pair, opened
//!   first, sits before an inner one in the vector even though it closes
//!   last). key_value: **unused** (`0`) — a kv frame's pending key span and
//!   value start live in a small separate scratch array, `kv_pending`,
//!   indexed by this frame's own position in `frames` (not by a stored
//!   index at all). `$kv_f`, the public output vector, is appended to only
//!   once, complete, at the moment a pair actually *closes* — never at open
//!   time with a placeholder. This is the opposite convention from
//!   asymmetric/symmetric, deliberately: it keeps `$kv_f` in *close* order
//!   (an outer pair, closing after every pair nested inside its value,
//!   lands after them in the vector too) rather than open order. Both
//!   orderings are pre-existing, separately tested contracts on their
//!   respective output vectors, not a free choice made here — key_value
//!   sharing `frames`/`fdepth` with the other two kinds for *nesting*
//!   purposes does not mean it shares their *push-timing* convention too.
//!
//! `chained` is *not* on this stack — its two components are strictly
//! sequential (phase 2 only begins once phase 1 has closed), so it is a
//! two-phase transparent state machine needing no stack. The single-pending
//! symmetric mode (`parse_inside = true, balanced = false`) and all the
//! self-contained forward-search paths (greedy symmetric, the legacy
//! `balanced = false, parse_inside = false` asymmetric memchr, the legacy
//! both-opaque chained search) are likewise *not* stack users — they are the
//! original pre-nesting code, left intact.
//!
//! Because the stack is shared, `fdepth` is the single nesting budget for
//! every kind combined, key_value included — every `fdepth == 0` check
//! throughout this macro ("is anything at all currently open") is correct
//! as-is regardless of which kind(s) happen to be on the stack right now;
//! key_value sharing the stack needed no separate depth counter and no
//! separate ordering mechanism, precisely because it pushes and pops through
//! the exact same `frames`/`fdepth` everything else does.
//!
//! ## The fallback-flush invariant
//!
//! Plain text is flushed to the fallback field `$tx` only while the stack is
//! empty and no chained phase is in progress (`fdepth == 0 && !ch_in_text &&
//! !ch_in_url`). Text sitting inside any open construct is that construct's
//! content, not separate top-level prose, even when no more specific rule
//! claims it. Suppressing the flush there loses nothing: those bytes stay
//! covered by whichever enclosing span eventually closes around them.
//!
//! ## `asymmetric { balanced = …; parse_inside = …; … }`
//!
//! `balanced` and `parse_inside` are independent and both gate stack
//! eligibility — a rule needs only *one* of them `true` to be on the stack:
//!
//! - `balanced` controls this type's effective depth cap: `max_nest` when
//!   `true` (so it can self-nest, `{ { } }`), or a hard `1` when `false`.
//! - `parse_inside` controls *opacity*. Not stored per frame — derived at
//!   the point of use by matching the frame's stored open byte against each
//!   rule's own `$ao`/`$api` — so only the innermost open frame's rule
//!   governs whether other rules fire on its content, the same outcome the
//!   pre-unification per-frame flag gave, without carrying the flag itself.
//!
//! A rule with **both** flags `false` never reaches the stack at all; it runs
//! the historical self-contained `memchr` block further down (whose own
//! closing search now also skips escaped candidate close bytes — see
//! `@is_escaped`).
//!
//! Each byte of an open run is its own open event — `{{` is two opens, not
//! one "count = 2" event — because asymmetric field routing matches the
//! literal `1` per character, not the run length. An open pushes a
//! placeholder span (`start == end`), back-patched on the matching close.
//! Beyond a `balanced = true` type's cap an extra same-type open bumps a
//! one-shot overflow counter instead of pushing.
//!
//! **Close cascade + key_value drain.** A close byte runs a single unified
//! pass (never one block per rule — sharing a close byte across rules would
//! otherwise double-pop). Per close character, *before* touching the
//! asymmetric frame, the pass first drains any `key_value` frame sitting on
//! top: that value's container is closing, so the value ends here. Then, if
//! the new top is an asymmetric frame whose open byte's grammar-known close
//! byte matches this character (derived via the same `$ao`/`$ac` pairing the
//! open side used, never a separately stored close byte), that one frame is
//! popped and its placeholder back-patched. Because the stack is strict LIFO
//! and a kv frame is always pushed *after* the container it lives in, a `}}`
//! run finalises the inner pair on the first `}` (then pops its object) and
//! the outer pair on the second — correct nesting falls out of the
//! per-character loop, and the "value committed before the container pops"
//! ordering is automatic.
//!
//! **Required grammar change**: the close byte (and, for `key_value`, the
//! `end` separator) must be listed in the same `on_trigger(...)` set, since
//! they are now found by the same scan that finds the opens.
//!
//! ## `symmetric { parse_inside = true; balanced = true; … }`
//!
//! `parse_inside = true, balanced = false` (single pending slot) is
//! **unchanged** — it keeps its original `pending` slot, off the stack.
//!
//! `parse_inside = false` (greedy mode) keeps its opacity unchanged; only its
//! internal forward search gained escape-awareness (see `@is_escaped`). It is
//! off the stack.
//!
//! `parse_inside = true, balanced = true` pushes onto the unified stack. An
//! occurrence whose `(byte, count)` matches the current top closes it;
//! otherwise, with room, it opens a new frame. An identical `(byte, count)`
//! pair cannot self-nest (open and close look the same), so `**a **b** c**`
//! resolves as two adjacent runs. The run length picks the construct, so it
//! is matched as-is, never split byte-by-byte (unlike asymmetric).
//!
//! ## `chained: T { … }` (no stack)
//!
//! Unchanged from the pre-unification design: both-opaque components run the
//! self-contained two-phase search; a transparent component runs the
//! two-phase transparent state machine. Sequential phases, single slot each.
//!
//! ## Internal `\n` within a multi-line run (no stack, no frame)
//!
//! `parse_text!` may hand `parse_inline!` a span covering several source
//! lines at once (a multi-line fallthrough run — see `parse_text!`'s own
//! docs for when and why). Inside such a span, every `\n` is just another
//! byte; by default it is never specially recognised at all and is scanned
//! over exactly like a space or a letter, at near-zero extra cost (it is one
//! more byte in the unified `find_any` target set, nothing more — *when*
//! `$eol` is in that set at all; see immediately below), which is the entire
//! reason a grammar like JSON's stack survives the line break without any
//! change to its own rules.
//!
//! `$eol` is part of the unified trigger search **only when the caller's
//! `multiline` flag (`$ml`) is `true`** (see the execution-phase find loop).
//! It is not free to include unconditionally: `find_any`'s wide (`N >= 4`)
//! path costs one comparison per target per 8-byte chunk, so every extra
//! target byte is paid on every chunk of every byte scanned, whether or not
//! it ever matches. `parse_text!`'s multi-line call (`flush_para_inline!`)
//! passes `true`, because its span can genuinely contain internal `\n`
//! bytes and needs both the hard-break handling below and the line-break
//! bounding of long trigger-free stretches. Its single-line-bounded call
//! (mid-line continuations after a Line/Block match) passes `false`,
//! because that span's own upper bound is itself the offset a `memchr` call
//! found for the next `\n` — the range is `\n`-free by construction, so
//! searching for `$eol` there could never match anything; excluding it
//! removes a dead per-chunk comparison from every such line with no change
//! in behaviour. A direct caller of `parse_inline!` outside `parse_text!`
//! (e.g. the engine's own unit tests) should pass `true` unless it can make
//! the same construction guarantee itself.
//!
//! When `$ml` is `true` and the search does land on `\n`, what happens next
//! is what `hard_break` gates. When a grammar declares it, an internal `\n`
//! is checked the same way the run's own end is: look backward for the
//! escape byte or `>= min` separator bytes, trim them out of the flushed
//! plain-text span, and emit a zero-length hard-break span at the trim point
//! if matched. This check runs *before* the generic escape-skip logic
//! further below — a backslash immediately before `\n` is the
//! hard-break-via-backslash signal here, consumed by this check itself, not
//! an "escaped eol" in the generic delimiter sense that escape-skip exists
//! for. Either way the scan simply `continue`s afterwards: the unified stack
//! is never drained by an internal `\n`, only by the run's true end. When
//! `hard_break` is *not* declared (regardless of `$ml`), landing on a `\n`
//! falls through every rule arm untouched (no arm triggers on it) and it is
//! scanned over like any other byte.
//!
//! This makes the pre-loop, once-only end-of-run hard-break check further
//! down partially redundant for a multi-line run's *last* internal `\n`
//! (it gets caught here first, on its way past) — harmlessly so, since by
//! the time the end-of-run check runs, the byte immediately before `parse_end`
//! is that very `\n`, which never matches `$hb_esc` or `$sp`. The end-of-run
//! check remains the only mechanism for the genuinely line-break-free cases:
//! true end of input with no trailing newline, and the `$ml = false`
//! single-line-bounded calls, where `$eol` is not even in the search target
//! set, so no internal `\n` is ever landed on in the first place.
//!
//! ## `key_value: T { … }` — shares the same stack, no nesting of its own
//!
//! A `key_value` rule splits, around its `eq` trigger byte, into a key (to
//! the left) and a value (to the right). Both sides are bounded by *foreign*
//! bytes, which is why the rule triggers on the separator, not on an opening
//! byte. The **key has no nesting**: it is computed once, ascending, at `eq`
//! time and never changes again — but it is *not* pushed to the rule's
//! output vector yet. It is parked in `kv_pending`, alongside the value's
//! start, and stays there for as long as the pair's value remains open. The
//! **value is nested**: it is what the stack frame actually tracks, finalised
//! by structure rather than by a forward search. Only when the value
//! actually closes — at the `$kv_end` pre-check, the asymmetric close
//! cascade's kv-drain, or end-of-run — is the *whole* struct (key and value
//! both) pushed into the output vector in one go, complete. This is
//! deliberate, not incidental: it is what keeps the output vector in *close*
//! order (an outer pair, whose value closes after every pair nested inside
//! it, lands after them in the vector too — see `kv_pending`'s own doc
//! comment for why this differs from asymmetric/symmetric's open-order
//! convention on their own output vectors). Sharing the unified `frames` /
//! `fdepth` stack with those two kinds is what gives key_value its nesting
//! for free; it does not also mean key_value adopts their push-timing.
//!
//! **Pushing (the stack frame, not the output vector).** On `eq`, a frame is
//! pushed *only if the current top of stack is not already a key_value
//! frame* (recognised the same way every other stack query recognises a
//! frame's kind here — by matching its stored byte, `$kv_end`, against this
//! rule's own literal). This single condition makes a flat, separator-less
//! line like `a = 1 b = 2` resolve to one pair whose value runs to the line
//! end (the second `eq` lands on top of the first pending value and is
//! therefore value content, not a new key), while in structured input a
//! fresh key only ever appears after the previous pair has been popped (by
//! its `end` separator or by its container closing), so the top is a
//! container, not a kv frame, and the new pair pushes normally.
//!
//! **Key anchor (`_kv_seg_start`).** A quoted key like `"a"` is consumed by
//! the symmetric string rule *before* `eq` is reached, moving `text_start`
//! past it; a `text_start`-bounded back-scan would yield an empty key.
//! `_kv_seg_start` tracks the position just after the most recent
//! key-introducing structural byte (a container open, or a top-level `end`
//! separator) and survives that opaque consumption. The key is back-scanned
//! from `eq` to the previous separator/tab, clamped to never cross
//! `_kv_seg_start`. This handles both a flat space-separated key (the
//! separator stops the scan) and a quoted JSON key (the anchor clamps it, so
//! the key span is `"a"` — quotes included; the unquoted content lives in the
//! string field, recoverable by interval containment).
//!
//! **Finalising the value.** Two points, both with the kv frame on top:
//!
//! - the `end` byte (`$kv_end`, e.g. `,`) arrives while the kv frame is on
//!   top — handled by a pre-check near the loop head, since `end` is claimed
//!   by no other rule;
//! - the container the value lives directly inside closes — handled by the
//!   drain at the head of the asymmetric close cascade, before the container
//!   pops.
//!
//! When the value's container is a *transparent asymmetric* container that is
//! not itself wrapped in a kv pair (e.g. a bare top-level array), an `$kv_end`
//! arriving with no kv frame on top is simply unmatched by any rule here and
//! falls through untouched — it is ordinary content as far as the stack is
//! concerned. Splitting such a container's elements is a concern for whatever
//! reads the container's span afterward, not for this scan.
//!
//! **End of run.** A kv frame still open when the scanned span ends finalises
//! its value to `parse_end` (so a flat `key = value` with no terminator still
//! emits), and advances `text_start` past it so the unconditional final flush
//! does not re-emit the value as plain text. Asymmetric and symmetric frames
//! still open at that point are discarded (no span), as before. Since a run
//! can now span multiple lines (see `parse_text!`'s docs), this is no longer
//! "end of line" in the literal sense — a key_value pair, like any other
//! stack-tracked construct, survives every internal `\n` inside the run and
//! is only ever drained here at the run's true end.
//!
//! **Containment, not equality.** When a value *is* a container, its `value`
//! span covers the whole `[1,2]` / `{…}` including the brackets, whereas the
//! asymmetric field stores only the bracket *content* — so the container span
//! is strictly *inside* the value span, not byte-equal. The projection
//! ("which field's interval contains this one") still holds; it is
//! containment, not equality.
//!
//! **Limitation.** Like `chained`, the kv value state is correct for a single
//! key_value rule per grammar; the key anchor `_kv_seg_start` is shared.
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
        // One minimal frame, one array, one depth, shared by asymmetric,
        // symmetric, AND key_value. `(byte, count, vidx)`:
        //   asymmetric : byte = open byte, count = 1 (always — each byte of
        //                an open run is its own event), vidx = back-patch
        //                index into the placeholder already pushed.
        //   symmetric  : byte = the delimiter, count = run length (picks
        //                the construct), vidx = back-patch index.
        //   key_value  : byte = $kv_end, count = 0 (unused), vidx = 0
        //                (unused — a kv frame's pending key/value-start data
        //                lives in `kv_pending` below, at this SAME position
        //                in `frames`, not behind an index stored here).
        // Nothing else is stored: close byte, opacity, and field routing are
        // all recovered from `byte` at the point of use by matching it
        // against each rule's own compile-time literal (see this file's
        // module docs for why this is sound and not a new assumption).
        //
        // Boundary note: `count` is a single byte, so a symmetric run length
        // beyond 255 would alias against a different, shorter run that
        // happens to truncate to the same `u8` (both open-time storage and
        // close-time comparison truncate the same way via `as u8`, so the
        // two sides stay internally consistent — but a 257-byte run and a
        // 1-byte run of the same delimiter would then wrongly compare equal
        // to each other). No existing grammar's symmetric rules declare run
        // lengths anywhere near this range (markdown's widest is `3`, for
        // bold-italic), so this is a real but currently unreachable edge,
        // stated here rather than left implicit.
        let mut frames: [(u8, u8, u32); $maxn] = [(0u8, 0u8, 0u32); $maxn];

        // Scratch storage for a still-open key_value frame's already-
        // resolved key plus pending value start: `(ks, ke, vs)`, indexed by
        // the SAME position the frame itself occupies in `frames` (not a
        // separately tracked vidx — the third tuple field of a kv frame is
        // simply unused, `0`, since position alone already identifies the
        // slot). This is deliberately kept OUT of `$kv_f`, the public output
        // vector: `$kv_f` is only ever appended to at the moment a pair
        // actually *closes*, complete, in one push — never at open time with
        // a placeholder. That is what makes its order close-order (an outer
        // pair, closing after every pair nested inside it, lands after them
        // in the vector too) rather than open-order. This is the opposite
        // convention from asymmetric/symmetric, whose own output vectors
        // intentionally ARE pushed at open time (placeholder, back-patched)
        // to keep THEM in open/start order — both conventions are existing,
        // separately tested behaviour, not a free choice made here. None of
        // this is a second *nesting* stack: ordering among kv frames and
        // asymmetric/symmetric frames is still decided entirely by `frames`
        // / `fdepth` alone; `kv_pending` only carries data that has nowhere
        // else to live until its frame closes.
        let mut kv_pending: [(u32, u32, u32); $maxn] = [(0u32, 0u32, 0u32); $maxn];

        // `fdepth` is `usize`, not a narrower type, despite being
        // architecturally bounded by `max_nest` (every push site checks
        // `fdepth < $maxn` first, so it provably never exceeds it). That
        // bound was, for a while, used as grounds to make it `u8` — measured
        // and reverted: `fdepth` indexes into `frames`/`kv_pending` many
        // times per interesting byte in the hot loop (`_top_is_kv`,
        // `_top_opaque_active`, the close cascade's kv-drain, the ordinary
        // close path — easily 5-10 accesses), and array indexing needs a
        // pointer-width operand regardless of what width the index variable
        // is declared as. A narrower `fdepth` means every one of those
        // accesses pays a zero-extend (`movzx`) from the narrow type up to
        // `usize` first; `usize` itself needs none. Unlike the frame fields
        // above — which DO live inside an array, multiplied by `max_nest`,
        // where narrowing wins on memory traffic — `fdepth` is a scalar
        // *index*, not an array element, so there is no compensating memory
        // win to offset that per-access cost. The same "scalar, not an
        // array element, no win from narrowing" reasoning that already kept
        // `asym_overflow` / `ch_text_depth` / `ch_url_depth` wide applies
        // here too; it just was not applied to `fdepth` the first time.
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
        // Single pending symmetric slot for parse_inside=true, balanced=false
        // (the original mechanism, off the unified stack).
        let mut pending: Option<(u8, u32, u32, u32)> = None;

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

        loop {
            // ---- Single unified trigger search -------------------------- //
            //
            // ONE `find_any` over the union of every `on_trigger` group's
            // bytes, *conditionally* plus `$eol` — gated by `$ml`, the
            // multiline flag passed in by the caller (`true` for
            // `flush_para_inline!`'s whole-run call, `false` for the
            // single-line-bounded mid-line-continuation call — see
            // `parse_text!`'s two call sites).
            //
            // `find_any`'s cost in the wide (`N >= 4`) path is linear in `N`:
            // one `has_byte` broadcast-compare per target, per 8-byte chunk
            // (see `swar::find_any_wide`). `$eol` is one more target than the
            // grammar's own trigger bytes, i.e. roughly +25% of that cost
            // for a 4-byte `on_trigger` set such as Markdown's — paid on
            // *every* chunk of *every* scanned byte, regardless of whether
            // `\n` ever actually occurs in range.
            //
            // For the single-line-bounded call, it provably never does:
            // `parse_text!` computes this call's own `$le` as the offset of
            // the next `\n` via `memchr`, so `[pos, parse_end)` is by
            // construction `\n`-free. Searching for `$eol` there cannot ever
            // match — it is pure dead weight, paid on every marker line with
            // trailing inline content (headings, bullet items: the `heavy`
            // profile). Dropping it for `$ml = false` removes that cost with
            // no behavioural change: the byte that could never be found is
            // simply no longer searched for.
            //
            // For the multiline run call (`$ml = true`), `$eol` stays in the
            // set: a long trigger-free run needs the search to stop at each
            // line's own `\n` rather than walking to `parse_end`, and an
            // internal `\n` may itself need hard-break handling (see below).
            // This is the genuinely necessary cost for that call's job.
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

            // Whether the current top of the unified stack is a key_value
            // frame — derived once here by byte alone (no stored kind): the
            // frame's stored byte equals some kv_rule's own `$kv_end`
            // literal. Valid for the rest of this outer-loop iteration only
            // (it must be recomputed inside any inner loop that itself pops
            // frames, e.g. the asymmetric close cascade further below, since
            // `fdepth` can change mid-iteration there). Reused by the eol
            // deferral guard immediately below and by the `$kv_end`
            // pre-check further down.
            let _top_is_kv = fdepth > 0 && {
                let _tb = frames[fdepth - 1].0;
                let mut _k = false;
                $( if _tb == $kv_end { _k = true; } )*
                _k
            };

            // ---------------------------------------------------------- //
            // Internal eol within a multi-line run. This whole block is a   //
            // `$(...)*` repetition over the (0-or-1) hard_break rule, so it  //
            // exists at all only when hard_break is declared; without it the //
            // `\n` the unified search just landed on falls straight through  //
            // to the generic dispatch below and is scanned over like any      //
            // other unmatched byte. Checked BEFORE the generic escape-skip     //
            // below: a preceding backslash here is the hard-break-via-          //
            // backslash signal itself ($hb_esc), to be consumed by THIS          //
            // check, not treated as "this eol is escaped" by the generic           //
            // delimiter-escaping rule meant for other bytes. Mirrors the            //
            // up-front end-of-run check, just triggered per-occurrence instead       //
            // of once: trims trailing spaces/escape, emits the hard-break span        //
            // if matched, flushes pending plain text up to the trim point             //
            // (subject to the same stack-emptiness guard as every other flush          //
            // here), then continues the SAME scan — the unified stack is never         //
            // drained by this. The final internal eol of a run (immediately             //
            // before `parse_end`) is also found and handled right here; the up-          //
            // front check further below then sees an eol byte at `parse_end - 1`,        //
            // never matches, and is a no-op for this case — it remains necessary for      //
            // the true-EOF-without-trailing-newline case and for the single-line-bounded   //
            // calls, where no internal eol is ever in range to be found by the search above.
            //
            // Deferral guard: `eol` and a `key_value` rule's `end` byte can be
            // the *same* byte (e.g. a flat `key=value` pair terminated by its
            // own line's `\n` — exactly `run_inline!`'s grammar in the test
            // suite). When the stack's top is an open kv-value frame, that
            // existing, byte-exact mechanism (the `$kv_end` pre-check just
            // below) owns this occurrence — it already `continue`s on a
            // correct match, so deferring here by doing nothing and falling
            // through changes nothing for it. Firing the hard-break path
            // *first* would otherwise swallow the terminator before
            // `$kv_end` ever saw it, silently merging what should have been
            // two values into one. This is a pure stack-state check — it
            // does not need to know what `$kv_end`'s literal byte is.
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
                        if fdepth == 0 && !ch_in_text && !ch_in_url {
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
            // key_value value terminator (`$kv_end`) at the value's own  //
            // level: only fires when a kv frame is currently on top of    //
            // the stack (we are back at the value's depth, nothing deeper  //
            // open). `$kv_end` is claimed by no other rule, so this is a    //
            // standalone pre-check. The value ends *before* the separator.   //
            //                                                                 //
            // When `$kv_end` arrives with no kv frame on top (e.g. a bare      //
            // top-level array's `,` between elements), no rule here claims it:  //
            // it falls through untouched, `pos` having already advanced past    //
            // the delimiter run. Splitting a transparent container's elements    //
            // is a concern for whoever reads that container's span afterward,     //
            // not for this scan — the stack only needs to know when a *value*      //
            // it is tracking ends.                                                  //
            // ---------------------------------------------------------------------//
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
                            if fdepth == 0 && !ch_in_text && !ch_in_url {
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

                            // If the frame directly below a top-of-stack
                            // key_value frame is the container about to close,
                            // the value ends here — commit it BEFORE the
                            // container frame is popped, so LIFO order holds.
                            // kv frames never stack directly (a container
                            // always sits between two of them), so at most one
                            // kv frame can be on top of its closing container.
                            // Both facts — "top is a kv frame" and "the frame
                            // below is a container whose grammar-known close
                            // byte is this delim" — are derived from stored
                            // bytes alone, freshly each sub-iteration (fdepth
                            // can change within this very loop, so a value
                            // computed once before the loop, like `_top_is_kv`
                            // above, would go stale after the first pop).
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
                        if fdepth == 0 && !ch_in_text && !ch_in_url {
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
                                if fdepth == 0 && !ch_in_text && !ch_in_url {
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
                            if fdepth == 0 && !ch_in_text && !ch_in_url {
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

            // --- legacy chained (both components opaque) ---
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
                                _found
                            };
                            if let Some(cu_end) = close_url {
                                let real_start = if is_prefix {
                                    delim_start as usize - 1
                                } else {
                                    delim_start as usize
                                };
                                if text_start < real_start {
                                    if fdepth == 0 && !ch_in_text && !ch_in_url {
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

            // --- symmetric (stack mode, or legacy pending / greedy) ---
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
                                            if fdepth == 0 && !ch_in_text && !ch_in_url {
                                                push_il!($tx, $crate::span::Span::new(
                                                text_start as u32, delim_start));
                                            };
                                        }
                                        let _vidx = $state.$sf.len() as u32;
                                        push_il!($sf, $crate::span::Span::new(pos as u32, pos as u32));
                                        frames[fdepth] = ($sb, count as u8, _vidx);
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
                                        if fdepth == 0 && !ch_in_text && !ch_in_url {
                                            push_il!($tx, $crate::span::Span::new(
                                            text_start as u32, delim_start));
                                        };
                                    }
                                    text_start = delim_start as usize;
                                    continue;
                                }
                            } else {
                                if text_start < delim_start as usize {
                                    if fdepth == 0 && !ch_in_text && !ch_in_url {
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
                                        if fdepth == 0 && !ch_in_text && !ch_in_url {
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
                                if fdepth == 0 && !ch_in_text && !ch_in_url {
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

            // --- legacy asymmetric (balanced=false, parse_inside=false) ---
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
                        _found
                    };
                    if let Some(cp) = close_pos {
                        if text_start < delim_start as usize {
                            if fdepth == 0 && !ch_in_text && !ch_in_url {
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

            // --- key_value: key resolved now, but NOT pushed to $kv_f yet —
            // only `kv_pending` at this frame's own slot. $kv_f is appended
            // to once, complete, at actual close (see kv_pending's own doc
            // comment for why this ordering matters).
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
                            if fdepth == 0 && !ch_in_text && !ch_in_url {
                                push_il!($tx, $crate::span::Span::new(text_start as u32, ks as u32));
                            };
                        }
                        if fdepth < $maxn {
                            kv_pending[fdepth] = (ks as u32, key_end as u32, val_start as u32);
                            frames[fdepth] = ($kv_end, 0u8, 0u32);
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
        // End of run: drain the unified stack, top → down. Kind is derived   //
        // from each frame's stored byte (kv's own `$kv_end`, then asymmetric //
        // rules' `$ao`, else symmetric) — never a stored tag.                //
        //                                                                    //
        //  - key_value frame  : its key and value-start, parked in           //
        //    `kv_pending` since `eq` time, are pushed as the complete        //
        //    struct right here — value finalised to the run's end (so a      //
        //    flat `key = value` with no terminator still emits correctly) —  //
        //    and the text cursor is pushed past it, so the unconditional     //
        //    flush below does not re-emit the value as plain text.           //
        //  - asymmetric frame : discard via `Vec::remove(vidx)` — the same   //
        //    type can self-nest, so a closed inner entry can sit at a higher //
        //    index than a still-open outer one; processing innermost-first   //
        //    removes the highest index first.                                //
        //  - symmetric frame  : discard via `truncate(vidx)` — an identical  //
        //    (byte, count) never self-nests, so each field has at most one   //
        //    pending placeholder, always last.                               //
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

        // Returns `$le` as-is — the boundary passed in, not `$le + 1`. The two
        // call sites in `parse_text!` each know their own boundary's meaning
        // and own the `+1` themselves where it applies: the single-line
        // category (mid-line continuations) still resumes at `current_line_end
        // + 1` (its `$le` sits exactly on the eol byte, by construction); the
        // multi-line category (`flush_para_inline!`) ignores the return value
        // entirely, since its own boundary is never an eol position to skip
        // past — it's a blank line's start, a matched construct's start, or
        // `len`, and the surrounding loop already knows how to proceed from
        // there.
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
