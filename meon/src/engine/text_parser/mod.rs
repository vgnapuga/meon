//! Runtime parsing macros: `parse_text!`, `parse_inline!`, `parse_line!`,
//! `parse_block!`, and `define_standalone_fns!`.
//!
//! These macros are the engine core. They are `#[macro_export]`-ed so that
//! qualified paths emitted by `define_parser!` (e.g. `meon::parse_text!(…)`)
//! resolve correctly from any dependent crate. They are marked
//! `#[doc(hidden)]` because they are not part of the stable public API —
//! grammar authors interact with `define_parser!` only.

pub mod standalone;

pub mod inline;
pub use crate::parse_inline;

pub mod line;
pub use crate::parse_line;

pub mod block;
pub use crate::parse_block;

/// Full single-pass text parser — the `parse_text!` macro.
///
/// # Architecture: three rule families
///
/// Every element in the grammar belongs to exactly one of three families.
/// The distinction is structural — it reflects where in the source an element
/// can begin and end, and whether it requires a trigger byte to be detected.
///
/// ## Inline rules
///
/// Inline elements live *inside* a logical line. They are detected by scanning
/// for trigger bytes within the line content and are further divided:
///
/// - **`inline`** — elements that carry user-defined structured types with
///   multiple span fields (e.g. a link with separate `text` and `url` spans).
///   Each occurrence is represented as a value of the user-defined type `T`,
///   stored as `Vec<T>`. Triggered by specific bytes declared in `on_trigger`.
///
/// - **`inline_simple`** — elements represented by a single [`Span`] with no
///   additional metadata (e.g. bold, italic, code, plain text runs). Triggered
///   by specific bytes in `on_trigger`, or collected as the `fallback` when no
///   trigger fires. Adjacent spans can be coalesced via `merge_simple = true`.
///
/// Both inline families are context-free within a line: each line is scanned
/// independently, with no state carried between lines.
///
/// ## Line rules
///
/// Line elements span exactly one logical line — they begin at the first byte
/// of a line and consume it entirely. If a line rule matches, inline scanning
/// is skipped for that line.
///
/// - **`line`** — whole-line elements with per-occurrence metadata (e.g. a
///   heading carries its level). Stored as `Vec<(Type, Span)>` where `Span`
///   covers the content portion after the marker. Matched by `parse_line!`
///   via `line(byte, max = N)` rules.
///
/// - There is no `line_simple` family: whole-line elements without metadata
///   are expressed as `line_simple(bytes, min = N)` rules that still produce
///   a `Vec<(Type, Span)>` entry, where the type carries the matched delimiter
///   byte.
///
/// ## Block rules
///
/// Block elements begin on one line and end on a different (later) line. They
/// maintain state across lines via the active-block slot.
///
/// - **`block`** — per-line elements with metadata that open a new logical
///   item on each matching line (e.g. a bullet list item: marker kind + content
///   span). Each line that matches opens a new item. Stored as `Vec<(Type, Span)>`.
///   Matched by `parse_block!` via `(pattern) |var|` and `num(…)` rules.
///
/// - **`block_simple`** — multi-line constructs with no per-line metadata.
///   Two sub-kinds:
///   - `fence(byte, min)` — opens on a fence line, closes on a matching
///     fence line; the entire range (open fence through close fence) is one
///     `Span`. While a fence is active, inline scanning is suppressed.
///   - `cont(byte)` — groups consecutive lines that begin with `byte` into
///     a single `Span`. Closes when a line does not start with `byte`.
///   - `fallback` — collects runs of lines that match no other block rule
///     into paragraph spans (`Vec<Span>`).
///
/// # Dispatch order per line
///
/// At the start of each new line `parse_text!` runs the following sequence:
///
/// ```text
/// 1. Blank line?  → flush paragraph, close active cont block, advance.
/// 2. parse_block! active arm  → if a fence or cont block is open, handle it.
/// 3. parse_block! open arm    → try to open a new block (bullet, ordered, fence, cont).
/// 4. parse_line!              → try to match a whole-line rule (headings, thematic breaks).
/// 5. Inline scanning (parse_inline!) → scan for trigger bytes; emit inline spans.
/// 6. Fallback                 → plain text collected into inline_simple fallback field.
/// ```
///
/// Steps 2–4 short-circuit: the first match wins and inline scanning is skipped.
/// When a fence is active (discriminant `0`), step 5 is also suppressed.
///
/// # Standalone iterators
///
/// The generated `find_*` methods (via [`define_standalone_fns!`]) operate
/// **outside** the `parse_text!` context entirely. Each standalone iterator
/// scans the raw source independently, with no knowledge of surrounding
/// elements, active blocks, or paragraph state. This means:
///
/// - A standalone iterator may match bytes that `parse_text!` would have
///   suppressed (e.g. content inside a fence, or an escaped delimiter).
/// - Counts from standalone iterators and from full-parse fields can differ
///   by design — the standalone path trades context-sensitivity for speed.
/// - Use standalone iterators when you need only one element kind from a large
///   source and do not need cross-element consistency.
///
/// # Expansion pipeline
///
/// The macro body is a multi-stage token accumulator driven by `@`-prefixed
/// internal arms. Each stage transforms the grammar token soup into typed
/// buckets before the final `@body` arm emits the parsing loop:
///
/// ```text
/// parse_text!(src; sep=…, eol=…, tab=…, escape=…[, max_nest=…]; <sections>)
///    │
///    ├─ @cs  — split raw sections into [inline], [lines], [blocks] buckets
///    ├─ @ci  — extract inline settings: merge_simple flag, fallback field,
///    │         hard_break rule, on_trigger byte sets → finders list
///    ├─ @cb  — extract block settings: block_simple rules, block rules,
///    │         fallback paragraph field
///    └─ @body — emit the actual O(n) parsing loop with all resolved buckets
/// ```
///
/// # Main loop invariants
///
/// - `pos` always advances; the loop terminates in O(n) in source length.
/// - `at_line_start` is `true` whenever `pos` points at the first byte of a
///   new logical line.
/// - `active` — single `Option<(u8, u8, u8, u32)>` slot encoding the open
///   block (see [`parse_block!`] for the encoding). Only one block can be
///   active at a time.
/// - `para_start` — start offset of the current paragraph; `None` when no
///   paragraph is open. Flushed on block transitions and blank lines.
/// - `text_start` — start offset of the pending plain-text run within the
///   current line. Flushed before any inline element is emitted.
///
/// # Context bytes
///
/// | Parameter   | Meaning                                            | Typical value |
/// |-------------|-----------------------------------------------------|---------------|
/// | `sep`       | Word separator                                     | `b' '`        |
/// | `eol`       | Line terminator                                    | `b'\n'`       |
/// | `tab`       | Tab character                                      | `b'\t'`       |
/// | `escape`    | Escape prefix                                      | `b'\\'`       |
/// | `max_nest`  | Bounded nesting depth cap, forwarded to            | `4`           |
/// |             | `parse_inline!` (optional, default `1`)            |               |
///
/// `max_nest` bounds the two *stacks* `parse_inline!` uses — one for
/// `symmetric { parse_inside = true; balanced = true; … }` rules, one for
/// `asymmetric` rules with `balanced = true` and/or `parse_inside = true` —
/// see that macro's own docs for the full mechanism. (A third transparent
/// construct, `chained` with a `parse_inside = true` component, is
/// activated alongside these but tracks only sequential two-phase state, so
/// it consumes no depth from `max_nest`.) Omitting `max_nest` defaults it
/// to `1`, which reproduces the pre-nesting single-pending-slot /
/// single-outer-span behaviour exactly; existing grammars are unaffected
/// until they opt in. Block-level constructs (`cont`, `fence`) do not
/// participate in `max_nest`.
///
/// # Known limitations
///
/// - Single-slot active block state cannot represent nested block constructs
///   (e.g. a continuation block containing a fenced block). This is a
///   deliberate trade-off: the slot fits in a register and eliminates heap
///   allocation for block state.
/// - Inline scanning is context-free within a line; precedence between
///   overlapping inline rules is grammar-defined and resolved by declaration
///   order, not by a precedence table.
#[macro_export]
macro_rules! parse_text {
    // No `max_nest` given — default to `1`, which reproduces the pre-nesting
    // behaviour exactly. Existing call sites — in particular
    // `define_parser!`'s expansion before it was updated to pass
    // `max_nest` itself — keep compiling unchanged.
    (
        $src:expr ;
        sep = $sep:literal, eol = $eol:literal,
        tab = $tab:literal, escape = $esc:literal ;
        $($sections:tt)*
    ) => {
        $crate::parse_text!(
            $src ;
            sep = $sep, eol = $eol, tab = $tab, escape = $esc, max_nest = 1 ;
            $($sections)*
        )
    };

    (
        $src:expr ;
        sep = $sep:literal, eol = $eol:literal,
        tab = $tab:literal, escape = $esc:literal, max_nest = $maxn:literal ;
        $($sections:tt)*
    ) => {
        $crate::parse_text!(@cs
            ctx = ($src, $sep, $eol, $tab, $esc, $maxn)
            il  = []
            ln  = []
            bl  = []
            rem = [$($sections)*]
        )
    };

    (@cs ctx=$ctx:tt il=[$($il:tt)*] ln=$ln:tt bl=$bl:tt
        rem = [inline { $($new:tt)* } $($rest:tt)*]
    ) => {
        $crate::parse_text!(@cs ctx=$ctx
            il=[$($il)* $($new)*] ln=$ln bl=$bl rem=[$($rest)*])
    };

    (@cs ctx=$ctx:tt il=$il:tt ln=[$($ln:tt)*] bl=$bl:tt
        rem = [lines { $($new:tt)* } $($rest:tt)*]
    ) => {
        $crate::parse_text!(@cs ctx=$ctx
            il=$il ln=[$($ln)* $($new)*] bl=$bl rem=[$($rest)*])
    };

    (@cs ctx=$ctx:tt il=$il:tt ln=$ln:tt bl=[$($bl:tt)*]
        rem = [blocks { $($new:tt)* } $($rest:tt)*]
    ) => {
        $crate::parse_text!(@cs ctx=$ctx
            il=$il ln=$ln bl=[$($bl)* $($new)*] rem=[$($rest)*])
    };

    (@cs ctx=$ctx:tt il=[$($il:tt)*] ln=[$($ln:tt)*] bl=[$($bl:tt)*] rem=[]) => {
        $crate::parse_text!(@ci ctx=$ctx
            ln=[$($ln)*] bl=[$($bl)*]
            ms=[] ftx=[] ilt=[]
            hb=[] finders=[]
            rem=[$($il)*])
    };

    (@ci ctx=$ctx:tt ln=$ln:tt bl=$bl:tt
        ms=$ms:tt ftx=$ftx:tt ilt=$ilt:tt hb=$hb:tt finders=$finders:tt
        rem = [merge_simple = $flag:ident ; $($rest:tt)*]
    ) => {
        $crate::parse_text!(@ci ctx=$ctx ln=$ln bl=$bl
            ms=[$flag] ftx=$ftx ilt=$ilt hb=$hb finders=$finders
            rem=[$($rest)*])
    };

    (@ci ctx=$ctx:tt ln=$ln:tt bl=$bl:tt
        ms=$ms:tt ftx=$ftx:tt ilt=$ilt:tt hb=$hb:tt finders=$finders:tt
        rem = [fallback => $tx:ident ; $($rest:tt)*]
    ) => {
        $crate::parse_text!(@ci ctx=$ctx ln=$ln bl=$bl
            ms=$ms ftx=[$tx] ilt=$ilt hb=$hb finders=$finders
            rem=[$($rest)*])
    };

    (@ci ctx=$ctx:tt ln=$ln:tt bl=$bl:tt
        ms=$ms:tt ftx=$ftx:tt ilt=[$($ilt:tt)*] hb=$hb:tt finders=$finders:tt
        rem = [hard_break($hb_esc:literal, $sp:literal, $sp_min:literal) => $hb_fld:ident ; $($rest:tt)*]
    ) => {
        $crate::parse_text!(@ci ctx=$ctx ln=$ln bl=$bl ms=$ms ftx=$ftx
            ilt=[$($ilt)* hard_break($hb_esc, $sp, $sp_min) => $hb_fld ;]
            hb=[$hb_esc, $sp, $sp_min => $hb_fld]
            finders=$finders
            rem=[$($rest)*])
    };

    // Collect on_trigger(…) { … } blocks — the renamed form of memchr(…) { … }.
    (@ci ctx=$ctx:tt ln=$ln:tt bl=$bl:tt
        ms=$ms:tt ftx=$ftx:tt ilt=[$($ilt:tt)*] hb=$hb:tt finders=[$($f:tt)*]
        rem = [on_trigger($($fn_b:literal),+) { $($inner:tt)* } $($rest:tt)*]
    ) => {
        $crate::parse_text!(@ci ctx=$ctx ln=$ln bl=$bl ms=$ms ftx=$ftx
            ilt=[$($ilt)* on_trigger($($fn_b),+) { $($inner)* }]
            hb=$hb finders=[$($f)* $($fn_b)*]
            rem=[$($rest)*])
    };

    (@ci
        ctx=($src:expr, $sep:literal, $eol:literal, $tab:literal, $esc:literal, $maxn:literal)
        ln=[$($ln:tt)*] bl=[$($bl:tt)*]
        ms=[$merge_il:tt] ftx=[$tx:ident] ilt=[$($ilt:tt)*]
        hb=$hb:tt finders=$finders:tt
        rem=[]
    ) => {
        $crate::parse_text!(@cb
            ctx=($src, $sep, $eol, $tab, $esc, $maxn)
            merge_il=$merge_il tx=$tx ilt=[$($ilt)*] ln=[$($ln)*]
            hb=$hb finders=$finders
            sr=[] br=[] fpara=[]
            rem=[$($bl)*])
    };

    (@ci
        ctx=($src:expr, $sep:literal, $eol:literal, $tab:literal, $esc:literal, $maxn:literal)
        ln=[$($ln:tt)*] bl=[$($bl:tt)*]
        ms=[] ftx=[$tx:ident] ilt=[$($ilt:tt)*]
        hb=$hb:tt finders=$finders:tt
        rem=[]
    ) => {
        $crate::parse_text!(@cb
            ctx=($src, $sep, $eol, $tab, $esc, $maxn)
            merge_il=false tx=$tx ilt=[$($ilt)*] ln=[$($ln)*]
            hb=$hb finders=$finders
            sr=[] br=[] fpara=[]
            rem=[$($bl)*])
    };

    (@cb ctx=$ctx:tt merge_il=$merge_il:tt tx=$tx:ident ilt=$ilt:tt ln=$ln:tt
        hb=$hb:tt finders=$finders:tt
        sr=[$($sr:tt)*] br=$br:tt fpara=$fpara:tt
        rem = [block_simple { $($new:tt)* } $($rest:tt)*]
    ) => {
        $crate::parse_text!(@cb ctx=$ctx merge_il=$merge_il tx=$tx ilt=$ilt ln=$ln
            hb=$hb finders=$finders
            sr=[$($sr)* $($new)*] br=$br fpara=$fpara rem=[$($rest)*])
    };

    (@cb ctx=$ctx:tt merge_il=$merge_il:tt tx=$tx:ident ilt=$ilt:tt ln=$ln:tt
        hb=$hb:tt finders=$finders:tt
        sr=$sr:tt br=[$($br:tt)*] fpara=$fpara:tt
        rem = [block { $($new:tt)* } $($rest:tt)*]
    ) => {
        $crate::parse_text!(@cb ctx=$ctx merge_il=$merge_il tx=$tx ilt=$ilt ln=$ln
            hb=$hb finders=$finders
            sr=$sr br=[$($br)* $($new)*] fpara=$fpara rem=[$($rest)*])
    };

    (@cb ctx=$ctx:tt merge_il=$merge_il:tt tx=$tx:ident ilt=$ilt:tt ln=$ln:tt
        hb=$hb:tt finders=$finders:tt
        sr=$sr:tt br=$br:tt fpara=$fpara:tt
        rem = [fallback => $para:ident ; $($rest:tt)*]
    ) => {
        $crate::parse_text!(@cb ctx=$ctx merge_il=$merge_il tx=$tx ilt=$ilt ln=$ln
            hb=$hb finders=$finders
            sr=$sr br=$br fpara=[$para] rem=[$($rest)*])
    };

    (@cb ctx=$ctx:tt merge_il=$merge_il:tt tx=$tx:ident ilt=$ilt:tt ln=$ln:tt
        hb=$hb:tt finders=$finders:tt
        sr=$sr:tt br=$br:tt fpara=$fpara:tt
        rem = [fallback => $para:ident]
    ) => {
        $crate::parse_text!(@cb ctx=$ctx merge_il=$merge_il tx=$tx ilt=$ilt ln=$ln
            hb=$hb finders=$finders
            sr=$sr br=$br fpara=[$para] rem=[])
    };

    (@cb
        ctx=($src:expr, $sep:literal, $eol:literal, $tab:literal, $esc:literal, $maxn:literal)
        merge_il=$merge_il:tt tx=$tx:ident ilt=[$($ilt:tt)*] ln=[$($ln:tt)*]
        hb=$hb:tt finders=[$($f:literal)*]
        sr=[$($sr:tt)*] br=[$($br:tt)*] fpara=[$para:ident]
        rem=[]
    ) => {
        $crate::parse_text!(@body
            $src, $sep, $eol, $tab, $esc, $maxn,
            $tx, $merge_il,
            [$($ilt)*], [$($ln)*],
            [$($sr)*], [$($br)*],
            $para,
            hb = $hb,
            finders = [$($f)*]
        )
    };


    (@body
        $src:expr, $sep:literal, $eol:literal, $tab:literal, $esc:literal, $maxn:literal,
        $tx:ident, $merge_il:tt,
        [$($ilt:tt)*], [$($ln:tt)*],
        [$($sr:tt)*], [$($br:tt)*],
        $para:ident,
        hb = $hb:tt,
        finders = [$($f:literal)*]
    ) => {{
        let src: &[u8] = $src;
        let len: usize = src.len();
        let mut state = ParseState::new(len);

        let mut _active_val: Option<(u8, u8, u8, u32)> = None;
        let active = &mut _active_val;
        let mut pos: usize = 0;
        let mut para_start: Option<u32> = None;
        let mut text_start: u32 = 0;
        let mut at_line_start: bool = true;

        let mut current_line_end: usize = 0;
        let mut line_end_valid: bool = false;

        macro_rules! flush_text {
            ($end:expr) => {
                let _end = $end as u32;
                if text_start < _end {
                    $crate::parse_text!(@dispatch state, $tx,
                        $crate::span::Span::new(text_start, _end), $merge_il);
                }
            };
        }

        macro_rules! close_para {
            () => {
                if let Some(s) = para_start.take() {
                    state.$para.push($crate::span::Span::new(s, pos as u32));
                }
            };
        }

        while pos < len {
            if at_line_start {
                at_line_start = false;

                if src[pos] == $eol {
                    flush_text!(pos);
                    close_para!();
                    $crate::parse_text!(@close_cont active, state, src, pos ;
                        block_simple { $($sr)* } block { $($br)* });
                    pos += 1;
                    at_line_start = true;
                    line_end_valid = false;
                    continue;
                }

                current_line_end = $crate::memchr::memchr($eol, &src[pos..])
                    .map(|i| pos + i)
                    .unwrap_or(len);
                line_end_valid = true;

                let mut line_consumed = false;
                let mut line_start_progress = true;

                while line_start_progress {
                    line_start_progress = false;
                    let old_active = *active;

                    match $crate::parse_block!(
                        active, state, src, pos, current_line_end,
                        sep = $sep, tab = $tab ;
                        block_simple { $($sr)* } block { $($br)* }
                    ) {
                        Some((opened, cs)) => {
                            if opened {
                                flush_text!(pos);
                                close_para!();
                            }
                            text_start = cs as u32;
                            pos = cs;
                            if cs == current_line_end {
                                if cs < len { pos += 1; }
                                at_line_start = true;
                                line_end_valid = false;
                            } else if cs > current_line_end {
                                at_line_start = true;
                                line_end_valid = false;
                            } else {
                                at_line_start = false;
                            }
                            line_consumed = true;
                            break;
                        }
                        None => {}
                    }

                    if *active != old_active {
                        line_start_progress = true;
                        continue;
                    }

                    match $crate::parse_line!(
                        state, src, pos, current_line_end, sep = $sep ; $($ln)*
                    ) {
                        Some(cs) => {
                            flush_text!(pos);
                            close_para!();
                            text_start = cs as u32;
                            pos = cs;
                            if cs == current_line_end {
                                if cs < len { pos += 1; }
                                at_line_start = true;
                                line_end_valid = false;
                            } else if cs > current_line_end {
                                at_line_start = true;
                                line_end_valid = false;
                            } else {
                                at_line_start = false;
                            }
                            line_consumed = true;
                            break;
                        }
                        None => {}
                    }
                }

                if at_line_start { continue; }

                if !line_consumed && active.is_none() {
                    if para_start.is_none() {
                        para_start = Some(pos as u32);
                    }
                }
            } else {
                if !line_end_valid {
                    current_line_end = $crate::memchr::memchr($eol, &src[pos..])
                        .map(|i| pos + i)
                        .unwrap_or(len);
                    line_end_valid = true;
                }
            }

            let skip_inline = matches!(*active, Some((0u8, _, _, _)));

            if skip_inline {
                pos = if current_line_end < len { current_line_end + 1 } else { len };
                at_line_start = true;
                line_end_valid = false;
                continue;
            }

            match $crate::swar::find_any([$eol, $($f),*], &src[pos..len])
                .map(|r| pos + r)
            {
                None => {
                    pos = len;
                }

                Some(_p) if src[_p] == $eol => {
                    let (_le, _hb) = $crate::parse_text!(
                        @hb_check _p, text_start as usize, src ; $hb
                    );
                    flush_text!(_le);
                    $crate::parse_text!(@hb_push state, _le, _hb ; $hb);
                    text_start = (_p + 1) as u32;
                    pos = _p + 1;
                    at_line_start = true;
                    line_end_valid = false;
                }

                Some(_p) => {
                    let new_pos = $crate::parse_inline!(
                        state, src, pos, current_line_end,
                        $tx, $merge_il, $esc, $sep, $tab, $maxn ; $($ilt)*
                    );
                    text_start = new_pos as u32;
                    pos = new_pos;
                    at_line_start = true;
                    line_end_valid = false;
                }
            }
        }

        flush_text!(len);
        if let Some(s) = para_start {
            state.$para.push($crate::span::Span::new(s, len as u32));
        }
        $crate::parse_text!(@close_cont  active, state, src, len ;
            block_simple { $($sr)* } block { $($br)* });
        $crate::parse_text!(@close_fence active, state, src, len ;
            block_simple { $($sr)* } block { $($br)* });

        state.into_content(src)
    }};


    (@hb_check $p:ident, $ts:expr, $src:ident ;
        [$hb_esc:literal, $sp:literal, $sp_min:literal => $hb_fld:ident]
    ) => {{
        let mut _le = $p;
        let mut _hb = false;
        if _le > $ts {
            if $src[_le - 1] == $hb_esc {
                _le -= 1;
                _hb = true;
            } else {
                let mut _n: u32 = 0;
                while _le > $ts && $src[_le - 1] == $sp { _n += 1; _le -= 1; }
                if _n >= $sp_min { _hb = true; }
            }
        }
        (_le, _hb)
    }};

    (@hb_check $p:ident, $ts:expr, $src:ident ; []) => {
        ($p, false)
    };

    (@hb_push $st:ident, $le:ident, $hb:ident ;
        [$hb_esc:literal, $sp:literal, $sp_min:literal => $hb_fld:ident]
    ) => {
        if $hb {
            $st.$hb_fld.push($crate::span::Span::new($le as u32, $le as u32));
        }
    };

    (@hb_push $st:ident, $le:ident, $hb:ident ; []) => {};

    (@dispatch $st:ident, $field:ident, $span:expr, true) => {
        $crate::paste::paste! { $st.[<push_merge_ $field>]($span); }
    };
    (@dispatch $st:ident, $field:ident, $span:expr, false) => {
        $crate::paste::paste! { $st.[<push_ $field>]($span); }
    };

    (@close_cont $active:ident, $st:ident, $src:ident, $pos:expr ;
        block_simple { $($sr:tt)* } block { $($br:tt)* }
    ) => {
        $crate::parse_text!(@close_cont_inner $active, $st, $src, $pos ; $($sr)*)
    };

    (@close_cont_inner $active:ident, $st:ident, $src:ident, $pos:expr ;
        cont($byte:literal) => $field:ident $($rest:tt)*
    ) => {
        if let Some((1u8, ab, _, start)) = *$active {
            if ab == $byte {
                $st.$field.push($crate::span::Span::new(start, $pos as u32));
                *$active = None;
            }
        }
        $crate::parse_text!(@close_cont_inner $active, $st, $src, $pos ; $($rest)*)
    };
    (@close_cont_inner $active:ident, $st:ident, $src:ident, $pos:expr ;
        fence($pat:pat, min = $min:literal) => $field:ident $($rest:tt)*
    ) => {
        $crate::parse_text!(@close_cont_inner $active, $st, $src, $pos ; $($rest)*)
    };
    (@close_cont_inner $a:ident, $st:ident, $src:ident, $p:expr ; , $($r:tt)*) => {
        $crate::parse_text!(@close_cont_inner $a, $st, $src, $p ; $($r)*)
    };
    (@close_cont_inner $a:ident, $st:ident, $src:ident, $p:expr ; ; $($r:tt)*) => {
        $crate::parse_text!(@close_cont_inner $a, $st, $src, $p ; $($r)*)
    };
    (@close_cont_inner $a:ident, $st:ident, $src:ident, $p:expr ;) => {};

    (@close_fence $active:ident, $st:ident, $src:ident, $pos:expr ;
        block_simple { $($sr:tt)* } block { $($br:tt)* }
    ) => {
        $crate::parse_text!(@close_fence_inner $active, $st, $src, $pos ; $($sr)*)
    };

    (@close_fence_inner $active:ident, $st:ident, $src:ident, $pos:expr ;
        fence($pat:pat, min = $min:literal) => $field:ident $($rest:tt)*
    ) => {
        if let Some((0u8, _, _, start)) = *$active {
            $st.$field.push($crate::span::Span::new(start, $pos as u32));
            *$active = None;
        }
        $crate::parse_text!(@close_fence_inner $active, $st, $src, $pos ; $($rest)*)
    };
    (@close_fence_inner $active:ident, $st:ident, $src:ident, $p:expr ;
        cont($byte:literal) => $field:ident $($rest:tt)*
    ) => {
        $crate::parse_text!(@close_fence_inner $active, $st, $src, $p ; $($rest)*)
    };
    (@close_fence_inner $a:ident, $st:ident, $src:ident, $p:expr ; , $($r:tt)*) => {
        $crate::parse_text!(@close_fence_inner $a, $st, $src, $p ; $($r)*)
    };
    (@close_fence_inner $a:ident, $st:ident, $src:ident, $p:expr ; ; $($r:tt)*) => {
        $crate::parse_text!(@close_fence_inner $a, $st, $src, $p ; $($r)*)
    };
    (@close_fence_inner $a:ident, $st:ident, $src:ident, $p:expr ;) => {};
}
