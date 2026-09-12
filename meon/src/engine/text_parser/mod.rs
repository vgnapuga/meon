//! Runtime parsing macros: `parse_text!`, `parse_inline!`, `parse_line!`,
//! `parse_block!`, and `define_standalone_fns!`.
//!
//! These macros are the engine core. They are `#[macro_export]`-ed so that
//! qualified paths emitted by `define_parser!` (e.g. `meon::parse_text!(...)`)
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
/// # Three rule families
///
/// Every grammar element belongs to one family, chosen by where it can begin
/// and end in the source.
///
/// **Inline** elements live inside a run of text and are found by scanning for
/// trigger bytes. `inline` fields hold a user type with several spans
/// (`Vec<T>`, e.g. a link); `inline_simple` fields hold one span each
/// (`Vec<Span>`, e.g. bold, or the `fallback` plain text, optionally merged
/// with `merge_simple = true`).
///
/// **Line** elements cover exactly one line: they start at its first byte and
/// consume it, so no inline scan runs on the line itself. `line(byte, max)`
/// and `line_simple(bytes, min)` both produce `Vec<(Type, Span)>`, matched by
/// [`crate::parse_line!`].
///
/// **Block** elements start on one line and end on a later one, tracked by the
/// active block stack. `block` items (`(pattern)`, `num(...)`) are per-line
/// leaves with metadata, `Vec<(Type, Span)>`. `block_simple` covers `fence`
/// (open fence line through close fence line, inline scanning suppressed
/// inside), `cont` (consecutive lines starting with the marker) and the
/// `fallback` paragraph run, all `Vec<Span>`. Matched by [`crate::parse_block!`].
///
/// # Per-line dispatch
///
/// ```text
/// 1. Blank line          → flush the run, close open `cont` frames, advance.
/// 2. parse_block! peel   → continue or close the frames already open.
/// 3. parse_block! open   → open new fence / cont / block items.
/// 4. parse_line!         → match a whole-line rule.
/// 5. Inline scan         → trailing content after a matched marker, or a
///                          deferred multi-line run (below).
/// ```
///
/// Steps 2–4 short-circuit on the first match. While a fence is the innermost
/// open frame the line is consumed whole and step 5 does not run. If an outer
/// continuation closes mid-line, the line is reprocessed from its start
/// against the shallower stack.
///
/// # Multi-line runs
///
/// A line that matches no rule while no block is open is not scanned at once.
/// Its start is recorded in `para_start` and the loop moves to the next line;
/// the run grows while the following lines also fall through. The run is
/// flushed as **one** `parse_inline!` call — and its paragraph span recorded —
/// at a blank line, at a line where a line or block rule matches, or at the
/// end of input. Inside the run every `eol` is ordinary content, so the
/// inline stack survives it: a grammar with empty `lines {}` / `blocks {}`
/// sections (JSON) gets one run over the whole input, blank lines aside.
///
/// Trailing content after a matched line or block marker is scanned by a
/// single-line `parse_inline!` call; it cannot continue onto another line.
///
/// # Standalone iterators
///
/// The generated `find_*` methods (see [`crate::define_standalone_fns!`]) scan
/// the source independently of this macro. They carry no cross-element state,
/// so they may match bytes the full parse suppresses (a delimiter inside a
/// fence); counts can differ by design.
///
/// # Expansion
///
/// ```text
/// parse_text!(src; sep=…, eol=…, tab=…, escape=…, max_nest=…; <sections>)
///    ├─ @cs   — split the sections into [inline], [lines], [blocks]
///    ├─ @ci   — inline settings: merge_simple, fallback, hard_break, the
///    │          on_trigger byte sets
///    ├─ @cb   — block settings: block_simple rules, block rules, fallback
///    └─ @body — the parsing loop
/// ```
///
/// # Loop state
///
/// - `pos` always advances; the loop is O(n) in the source length.
/// - `_active_stack: [(u8, u8, u8, u32); max_nest]` and `_active_depth` —
///   the open block frames (see [`crate::parse_block!`] for the encoding).
/// - `para_start` — start of the current fallthrough run, `None` when no run
///   is open. Also the run's inline-scan start.
/// - `text_start` — start of pending plain text for the single-line inline
///   calls. Kept equal to `pos` while a run is deferred, so the flushes at the
///   run's close points are no-ops; the run's text is emitted by its own
///   `parse_inline!` call.
///
/// # Context bytes
///
/// | Parameter  | Meaning                                          | Typical value |
/// |------------|--------------------------------------------------|---------------|
/// | `sep`      | Word separator                                   | `b' '`        |
/// | `eol`      | Line terminator                                  | `b'\n'`       |
/// | `tab`      | Tab character                                    | `b'\t'`       |
/// | `escape`   | Escape prefix                                    | `b'\\'`       |
/// | `max_nest` | Nesting cap shared by the inline and block stacks | `1`           |
///
/// `max_nest` is always supplied by `define_parser!`. It bounds the unified
/// inline stack of [`crate::parse_inline!`] (symmetric with `balanced = true`,
/// asymmetric with `balanced = true` or `parse_inside = true`, key_value) and
/// the block stack of [`crate::parse_block!`] (how deep `cont` / `fence`
/// frames nest, and whether a `block` item opens inside an open block). At
/// `max_nest = 1` nothing self-nests and at most one block is open at a time.
/// Rules whose `balanced` and `parse_inside` flags are both `false` never
/// touch the stack.
///
/// # Known limitations
///
/// - Precedence between overlapping inline rules follows declaration order;
///   there is no precedence table.
#[macro_export]
macro_rules! parse_text {
    // `max_nest` is always present: `define_parser!` defaults it to `1`
    // before emitting this call.
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

    // Collect on_trigger(...) { ... } blocks.
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

            let mut _active_stack: [(u8, u8, u8, u32); $maxn] =
                [(0u8, 0u8, 0u8, 0u32); $maxn];
            let mut _active_depth: usize = 0;
            let mut pos: usize = 0;
            let mut para_start: Option<u32> = None;
            let mut text_start: u32 = 0;

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

            macro_rules! flush_para_inline {
                ($end:expr) => {
                    if let Some(s) = para_start {
                        let _ps = s as usize;
                        let _pe = $end as usize;
                        if _ps < _pe {
                            $crate::parse_inline!(
                                state, src, _ps, _pe,
                                $tx, $merge_il, $esc, $sep, $tab, $eol, $maxn, true ; $($ilt)*
                            );
                        }
                    }
                };
            }

            while pos < len {
                let current_byte = match src.get(pos) {
                    Some(&b) => b,
                    None => break,
                };

                if current_byte == $eol {
                    flush_para_inline!(pos);
                    flush_text!(pos);
                    close_para!();

                    let is_fence = if $maxn == 1 {
                        _active_depth > 0 && _active_stack[0].0 == 0u8
                    } else {
                        _active_depth > 0 && _active_stack.get(_active_depth - 1).map_or(false, |stack| stack.0 == 0u8)
                    };

                    if !is_fence {
                        $crate::parse_text!(@close_stack _active_stack, _active_depth, state, src, pos ;
                            block_simple { $($sr)* } block { $($br)* });
                    }
                    pos += 1;
                    continue;
                }

                let current_line_end = $crate::memchr::memchr($eol, &src[pos..])
                    .map(|i| pos + i)
                    .unwrap_or(len);
                let next_line_start = if current_line_end < len { current_line_end + 1 } else { len };

                let mut line_consumed = false;
                let mut line_start_progress = true;
                let mut loop_continue_signal = false;

                while line_start_progress {
                    line_start_progress = false;
                    let _old_depth = _active_depth;

                    match $crate::parse_block!(
                        _active_stack, _active_depth, state, src, pos, current_line_end,
                        sep = $sep, tab = $tab, max_nest = $maxn ;
                        block_simple { $($sr)* } block { $($br)* }
                    ) {
                        Some((opened, cs)) => {
                            if opened {
                                flush_para_inline!(pos);
                                flush_text!(pos);
                                close_para!();
                            }
                            text_start = cs as u32;
                            pos = cs;

                            if cs >= current_line_end {
                                if cs == current_line_end && cs < len { pos += 1; }
                                loop_continue_signal = true;
                            }
                            line_consumed = true;
                            break;
                        }
                        None => {}
                    }

                    if _active_depth != _old_depth {
                        line_start_progress = true;
                        continue;
                    }

                    match $crate::parse_line!(
                        state, src, pos, current_line_end, sep = $sep ; $($ln)*
                    ) {
                        Some(cs) => {
                            flush_para_inline!(pos);
                            flush_text!(pos);
                            close_para!();
                            text_start = cs as u32;
                            pos = cs;

                            if cs >= current_line_end {
                                if cs == current_line_end && cs < len { pos += 1; }
                                loop_continue_signal = true;
                            }
                            line_consumed = true;
                            break;
                        }
                        None => {}
                    }
                }

                if loop_continue_signal { continue; }

                if !line_consumed && _active_depth == 0 {
                    if para_start.is_none() {
                        para_start = Some(pos as u32);
                    }
                    pos = next_line_start;
                    text_start = pos as u32;
                    continue;
                }

                let skip_inline = if $maxn == 1 {
                    _active_depth > 0 && _active_stack[0].0 == 0u8
                } else {
                    _active_depth > 0 && _active_stack.get(_active_depth - 1).map_or(false, |stack| stack.0 == 0u8)
                };

                if skip_inline {
                    pos = next_line_start;
                    continue;
                }

                if pos < current_line_end {
                    let _ = $crate::parse_inline!(
                        state, src, pos, current_line_end,
                        $tx, $merge_il, $esc, $sep, $tab, $eol, $maxn, false ; $($ilt)*
                    );
                }
                pos = next_line_start;
                text_start = pos as u32;
            }

            flush_para_inline!(len);
            flush_text!(len);
            if let Some(s) = para_start {
                state.$para.push($crate::span::Span::new(s, len as u32));
            }
            $crate::parse_text!(@close_stack _active_stack, _active_depth, state, src, len ;
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

    // Close every still-open block frame at end of input (or, gated by the
    // caller, on a blank line), top → down. Each frame's span is dispatched to
    // the right field by `parse_block!`'s `@close_frame` arm, matching the
    // stored byte against each `cont` / `fence` rule.
    (@close_stack $stack:ident, $depth:ident, $st:ident, $src:ident, $pos:expr ;
        block_simple { $($sr:tt)* } block { $($br:tt)* }
    ) => {
        while $depth > 0 {
            $depth -= 1;
            let (_d2, _b2, _c2, _s2) = $stack[$depth];
            $crate::parse_block!(@close_frame
                $st, $src, $pos as u32, _d2, _b2, _s2 ; $($sr)*);
        }
    };
}
