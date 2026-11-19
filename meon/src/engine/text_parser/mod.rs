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
/// 1. Blank line          → flush the run, close every open frame — unless a
///                          fence is the innermost one: the line is then fence
///                          content and every frame stays open — advance. A
///                          line of `sep` and `tab` bytes alone is blank.
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
/// sections (JSON) gets one run over the whole input, blank lines aside. The
/// `eol` that ends the run's last line is not part of the scanned run, the
/// same as for a single-line call; the paragraph span still includes it.
///
/// Lines of `cont` frames alone run the same way: a line whose content
/// follows the markers of the open frames, all of them `cont` frames, starts
/// a run, and the next lines of the same frames alone — the same depth, no
/// frame or item opened, content that is not `sep` and `tab` bytes alone —
/// go on with it. The markers of every line after the first are no content
/// of the run: the inline scan steps past them at each `eol`, and its forward
/// searches skip the bytes among them. An element can still span them, as
/// a pair across two lines of the frames does. The run is flushed at the
/// first line that does not go on with it, or at the end of input.
///
/// Trailing content after any other matched line or block marker is scanned
/// by a single-line `parse_inline!` call; it cannot continue onto another
/// line.
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
/// parse_text!(src; sep=…, eol=…, tab=…, escape=…, max_nest=…, crlf=…; <sections>)
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
/// - `_active_stack: [(u32, u32); max_nest]` and `_active_depth` —
///   the open block frames (see [`crate::parse_block!`] for the encoding).
/// - `para_start` — start of the current fallthrough run, `None` when no run
///   is open. Also the run's inline-scan start.
///
/// Plain text is emitted only by `parse_inline!`: by the run's own call for a
/// deferred run, by the single-line call for content after a matched marker.
/// Bytes outside both — `eol` bytes between runs, lines consumed by a `line`
/// rule or a fence — belong to no fallback span.
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
/// | `crlf`     | A `\r` right before `eol` is part of the line end | `false`       |
///
/// `max_nest` is always supplied by `define_parser!`. It bounds the unified
/// inline stack of [`crate::parse_inline!`] (symmetric and asymmetric with
/// `parse_inside = true`, key_value) and the block stack of
/// [`crate::parse_block!`] (how deep `cont` / `fence` frames nest, and
/// whether a `block` item opens inside an open block). At `max_nest = 1`
/// nothing self-nests and at most one block is open at a time. A `symmetric`
/// or `asymmetric` rule with `parse_inside = false` never touches the stack:
/// it is a forward search, and an `asymmetric` one with `balanced = true`
/// counts its own nesting without bound.
///
/// # Known limitations
///
/// - Precedence between overlapping inline rules follows declaration order;
///   there is no precedence table.
#[macro_export]
macro_rules! parse_text {
    // `max_nest` and `crlf` are always present: `define_parser!` defaults
    // them to `1` and `false` before emitting this call.
    (
        $src:expr ;
        sep = $sep:literal, eol = $eol:literal,
        tab = $tab:literal, escape = $esc:literal, max_nest = $maxn:literal,
        crlf = $crlf:ident,
        max_inline_span = $max_inline:tt,
        max_line_span = $max_line:tt,
        max_block_span = $max_block:tt,
        reparse = $reparse:tt ;
        $($sections:tt)*
    ) => {
        $crate::parse_text!(@cs
            ctx = ($src, $sep, $eol, $tab, $esc, $maxn, $crlf,
                   $max_inline, $max_line, $max_block, $reparse)
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
        ctx=($src:expr, $sep:literal, $eol:literal, $tab:literal, $esc:literal, $maxn:literal,
             $crlf:ident, $max_inline:tt, $max_line:tt, $max_block:tt, $reparse:tt)
        ln=[$($ln:tt)*] bl=[$($bl:tt)*]
        ms=[$merge_il:tt] ftx=[$tx:ident] ilt=[$($ilt:tt)*]
        hb=$hb:tt finders=$finders:tt
        rem=[]
    ) => {
        $crate::parse_text!(@cb
            ctx=($src, $sep, $eol, $tab, $esc, $maxn, $crlf,
                 $max_inline, $max_line, $max_block, $reparse)
            merge_il=$merge_il tx=$tx ilt=[$($ilt)*] ln=[$($ln)*]
            hb=$hb finders=$finders
            sr=[] br=[] fpara=[]
            rem=[$($bl)*])
    };

    (@ci
        ctx=($src:expr, $sep:literal, $eol:literal, $tab:literal, $esc:literal, $maxn:literal,
             $crlf:ident, $max_inline:tt, $max_line:tt, $max_block:tt, $reparse:tt)
        ln=[$($ln:tt)*] bl=[$($bl:tt)*]
        ms=[] ftx=[$tx:ident] ilt=[$($ilt:tt)*]
        hb=$hb:tt finders=$finders:tt
        rem=[]
    ) => {
        $crate::parse_text!(@cb
            ctx=($src, $sep, $eol, $tab, $esc, $maxn, $crlf,
                 $max_inline, $max_line, $max_block, $reparse)
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
        ctx=($src:expr, $sep:literal, $eol:literal, $tab:literal, $esc:literal, $maxn:literal,
             $crlf:ident, $max_inline:tt, $max_line:tt, $max_block:tt, $reparse:tt)
        merge_il=$merge_il:tt tx=$tx:ident ilt=[$($ilt:tt)*] ln=[$($ln:tt)*]
        hb=$hb:tt finders=[$($f:literal)*]
        sr=[$($sr:tt)*] br=[$($br:tt)*] fpara=[$para:ident]
        rem=[]
    ) => {
        $crate::parse_text!(@body
            $src, $sep, $eol, $tab, $esc, $maxn, $crlf,
            $max_inline, $max_line, $max_block, $reparse,
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
            $crlf:ident, $max_inline:tt, $max_line:tt, $max_block:tt, $reparse:tt,
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

            let mut _active_stack: [(u32, u32); $maxn] = [(0u32, 0u32); $maxn];
            let mut _active_depth: usize = 0;
            let mut pos: usize = 0;
            let _reparse_depth: usize = 0;
            let mut para_start: Option<u32> = None;
            // The open `cont` run: where its scan starts, where its last line
            // ends, and the marker bytes of the frames it runs in.
            let mut cont_run: Option<(usize, usize, usize)> = None;
            let mut cont_bytes: [u8; $maxn] = [0u8; $maxn];

            macro_rules! close_para {
                () => {
                    if let Some(s) = para_start.take() {
                        $crate::paste::paste! {
                            state.[<push_ $para>]($crate::span::Span::new(s, pos as u32));
                        }
                    }
                };
            }

            macro_rules! flush_para_inline {
                ($end:expr) => {
                    if let Some(s) = para_start {
                        let _ps = s as usize;
                        let mut _pe = $end as usize;
                        if _pe > _ps && src[_pe - 1] == $eol {
                            _pe -= 1;
                            if $crlf && _pe > _ps && src[_pe - 1] == b'\r' {
                                _pe -= 1;
                            }
                        }
                        if _ps < _pe {
                            scan_para_run(&mut state, src, _ps, _pe);
                        }
                    }
                };
            }

            // The scan of a paragraph run, out of line for the same reason as
            // `scan_cont_run`.
            #[inline(never)]
            fn scan_para_run(state: &mut ParseState, src: &[u8], _ps: usize, _pe: usize) {
                let _reparse_depth: usize = 0;
                $crate::parse_inline!(
                    state, src, _ps, _pe,
                    $tx, $merge_il, $esc, $sep, $tab, $eol, $maxn, true, crlf = $crlf,
                    max_inline = $max_inline, reparse = $reparse, depth = _reparse_depth ;
                    $($ilt)*
                );
            }

            // The scan of a `cont` run, out of line: the loop flushes a run
            // from several places, and one copy of the inline engine serves
            // them all.
            #[inline(never)]
            fn scan_cont_run(
                state: &mut ParseState,
                src: &[u8],
                (_cs, _ce, _cd): (usize, usize, usize),
                cont_bytes: &[u8; $maxn],
            ) {
                let _reparse_depth: usize = 0;
                $crate::parse_inline!(
                    state, src, _cs, _ce,
                    $tx, $merge_il, $esc, $sep, $tab, $eol, $maxn,
                    cont(cont_bytes, _cd), crlf = $crlf,
                    max_inline = $max_inline, reparse = $reparse, depth = _reparse_depth ; $($ilt)*
                );
            }
            macro_rules! flush_cont {
                () => {
                    if let Some(_run) = cont_run.take() {
                        scan_cont_run(&mut state, src, _run, &cont_bytes);
                    }
                };
            }

            // Is `src[$from..$to]` `sep` and `tab` bytes alone?
            macro_rules! blank {
                ($from:expr, $to:expr) => {
                    src[$from..$to].iter().all(|&b| b == $sep || b == $tab)
                };
            }

            while pos < len {
                let current_byte = match src.get(pos) {
                    Some(&b) => b,
                    None => break,
                };

                // The line's `eol`, and where its content ends: before the
                // `\r` that comes right before that `eol` when `crlf` is set.
                let line_eol = if current_byte == $eol {
                    pos
                } else {
                    $crate::memchr::memchr($eol, &src[pos..]).map_or(len, |i| pos + i)
                };
                let current_line_end = if $crlf && line_eol < len && line_eol > pos && src[line_eol - 1] == b'\r' {
                    line_eol - 1
                } else {
                    line_eol
                };
                let next_line_start = if line_eol < len { line_eol + 1 } else { len };

                // A line of `sep` and `tab` bytes alone is as blank as an empty
                // one. Most lines begin with neither, which tells them apart at
                // once.
                if pos == current_line_end
                    || ((current_byte == $sep || current_byte == $tab)
                        && blank!(pos, current_line_end))
                {
                    flush_cont!();
                    flush_para_inline!(pos);
                    close_para!();

                    let is_fence = if $maxn == 1 {
                        _active_depth > 0 && _active_stack[0].0 != 0
                    } else {
                        _active_depth > 0 && _active_stack.get(_active_depth - 1).map_or(false, |stack| stack.0 != 0)
                    };

                    if !is_fence {
                        $crate::parse_text!(@close_stack _active_stack, _active_depth, state, src, pos ;
                            block_simple { $($sr)* } block { $($br)* });
                    }
                    pos = next_line_start;
                    continue;
                }

                // With no block open, a line whose first byte begins no block
                // or line rule goes on the paragraph run as the dispatch below
                // would leave it. A `sep` or `tab` byte may stand before a
                // marker, and takes the dispatch.
                if _active_depth == 0
                    && current_byte != $sep
                    && current_byte != $tab
                    && !$crate::parse_text!(@starts current_byte ;
                        sr=[$($sr)*] br=[$($br)*] ln=[$($ln)*])
                {
                    flush_cont!();
                    if para_start.is_none() {
                        para_start = Some(pos as u32);
                    }
                    pos = next_line_start;
                    continue;
                }

                let line_start = pos;

                let mut line_consumed = false;
                let mut line_start_progress = true;
                let mut loop_continue_signal = false;
                // What the block stack made of the line: whether it opened a
                // block, and where the content starts.
                let mut block_hit: Option<(bool, usize)> = None;

                while line_start_progress {
                    line_start_progress = false;
                    let _old_depth = _active_depth;

                    match $crate::parse_block!(
                        _active_stack, _active_depth, state, src, pos, current_line_end,
                        nl = next_line_start,
                        sep = $sep, tab = $tab, max_nest = $maxn ;
                        block_simple { $($sr)* } block { $($br)* }
                    ) {
                        Some((opened, cs)) => {
                            if opened {
                                flush_para_inline!(pos);
                                close_para!();
                            }
                            block_hit = Some((opened, cs));
                            pos = cs;

                            if cs >= current_line_end {
                                pos = next_line_start;
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
                            close_para!();
                            pos = cs;

                            if cs >= current_line_end {
                                pos = next_line_start;
                                loop_continue_signal = true;
                            }
                            line_consumed = true;
                            break;
                        }
                        None => {}
                    }
                }

                if loop_continue_signal {
                    // Nothing of the line is inline content: a `cont` run ends.
                    flush_cont!();
                    continue;
                }

                // A line of `cont` frames alone — the content right after
                // their markers — goes on with the open `cont` run when it
                // opened none of them, and starts one otherwise. Content of
                // `sep` and `tab` bytes alone ends the run, as a blank line
                // ends a paragraph run.
                if let Some((opened, cs)) = block_hit {
                    let mut _c = line_start;
                    let mut _k = 0usize;
                    while _k < _active_depth && _active_stack[_k].0 == 0 {
                        let _b = src[_active_stack[_k].1 as usize];
                        if _c < current_line_end && src[_c] == _b {
                            _c = if _c + 1 < current_line_end
                                && (src[_c + 1] == $sep || src[_c + 1] == $tab)
                            { _c + 2 } else { _c + 1 };
                        } else {
                            break;
                        }
                        _k += 1;
                    }
                    if _k == _active_depth && _active_depth > 0 && cs == _c {
                        if blank!(cs, current_line_end) {
                            flush_cont!();
                            pos = next_line_start;
                            continue;
                        }
                        match cont_run {
                            Some((_s, _, _d)) if !opened && _d == _active_depth => {
                                cont_run = Some((_s, current_line_end, _d));
                                pos = next_line_start;
                                continue;
                            }
                            _ => flush_cont!(),
                        }
                        // A next line without the outer marker closes the
                        // frames: the run is this line alone, and the line
                        // scan below reads it as the run scan would.
                        if next_line_start < len
                            && _active_stack
                                .first()
                                .is_some_and(|_f| src[next_line_start] == src[_f.1 as usize])
                        {
                            for _j in 0.._active_depth {
                                cont_bytes[_j] = src[_active_stack[_j].1 as usize];
                            }
                            cont_run = Some((cs, current_line_end, _active_depth));
                            pos = next_line_start;
                            continue;
                        }
                    }
                }
                flush_cont!();

                if !line_consumed && _active_depth == 0 {
                    if para_start.is_none() {
                        para_start = Some(pos as u32);
                    }
                    pos = next_line_start;
                    continue;
                }

                let skip_inline = if $maxn == 1 {
                    _active_depth > 0 && _active_stack[0].0 != 0
                } else {
                    _active_depth > 0 && _active_stack.get(_active_depth - 1).map_or(false, |stack| stack.0 != 0)
                };

                if skip_inline {
                    pos = next_line_start;
                    continue;
                }

                if pos < current_line_end {
                    {
                        let state = &mut state;
                        let _ = $crate::parse_inline!(
                            state, src, pos, current_line_end,
                            $tx, $merge_il, $esc, $sep, $tab, $eol, $maxn, false,
                            max_inline = $max_inline, reparse = $reparse,
                            depth = _reparse_depth ; $($ilt)*
                        );
                    }
                }
                pos = next_line_start;
            }

            flush_cont!();
            flush_para_inline!(len);
            if let Some(s) = para_start {
                $crate::paste::paste! {
                    state.[<push_ $para>]($crate::span::Span::new(s, len as u32));
                }
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

    // Can a line beginning with the byte `$b` open a block or line rule?
    // Each rule reads its marker first; a `sep` or `tab` byte before a
    // marker is the caller's to rule out.
    (@starts $b:ident ; sr=[fence($pat:pat, min = $min:literal) => $f:ident $($rest:tt)*]
        br=$br:tt ln=$ln:tt) => {
        matches!($b, $pat) || $crate::parse_text!(@starts $b ; sr=[$($rest)*] br=$br ln=$ln)
    };
    (@starts $b:ident ; sr=[cont($byte:literal) => $f:ident $($rest:tt)*]
        br=$br:tt ln=$ln:tt) => {
        $b == $byte || $crate::parse_text!(@starts $b ; sr=[$($rest)*] br=$br ln=$ln)
    };
    (@starts $b:ident ; sr=[; $($rest:tt)*] br=$br:tt ln=$ln:tt) => {
        $crate::parse_text!(@starts $b ; sr=[$($rest)*] br=$br ln=$ln)
    };
    (@starts $b:ident ; sr=[, $($rest:tt)*] br=$br:tt ln=$ln:tt) => {
        $crate::parse_text!(@starts $b ; sr=[$($rest)*] br=$br ln=$ln)
    };
    (@starts $b:ident ; sr=[]
        br=[($pat:pat) |$v:ident|: $meta:expr => $f:ident $($rest:tt)*] ln=$ln:tt) => {
        matches!($b, $pat) || $crate::parse_text!(@starts $b ; sr=[] br=[$($rest)*] ln=$ln)
    };
    (@starts $b:ident ; sr=[]
        br=[num($dp:pat, end = $ep:pat) |$n:ident, $k:ident|: $meta:expr => $f:ident $($rest:tt)*]
        ln=$ln:tt) => {
        matches!($b, $dp) || $crate::parse_text!(@starts $b ; sr=[] br=[$($rest)*] ln=$ln)
    };
    (@starts $b:ident ; sr=[] br=[; $($rest:tt)*] ln=$ln:tt) => {
        $crate::parse_text!(@starts $b ; sr=[] br=[$($rest)*] ln=$ln)
    };
    (@starts $b:ident ; sr=[] br=[, $($rest:tt)*] ln=$ln:tt) => {
        $crate::parse_text!(@starts $b ; sr=[] br=[$($rest)*] ln=$ln)
    };
    (@starts $b:ident ; sr=[] br=[]
        ln=[line($byte:literal, max = $max:literal) |$n:ident|: $meta:expr => $f:ident $($rest:tt)*]) => {
        $b == $byte || $crate::parse_text!(@starts $b ; sr=[] br=[] ln=[$($rest)*])
    };
    (@starts $b:ident ; sr=[] br=[]
        ln=[line_simple($pat:pat, min = $min:literal) |$v:ident|: $meta:expr => $f:ident $($rest:tt)*]) => {
        matches!($b, $pat) || $crate::parse_text!(@starts $b ; sr=[] br=[] ln=[$($rest)*])
    };
    (@starts $b:ident ; sr=[] br=[] ln=[; $($rest:tt)*]) => {
        $crate::parse_text!(@starts $b ; sr=[] br=[] ln=[$($rest)*])
    };
    (@starts $b:ident ; sr=[] br=[] ln=[, $($rest:tt)*]) => {
        $crate::parse_text!(@starts $b ; sr=[] br=[] ln=[$($rest)*])
    };
    (@starts $b:ident ; sr=[] br=[] ln=[]) => {
        false
    };

    (@hb_check $p:ident, $ts:expr, $src:ident ; []) => {
        ($p, false)
    };

    (@hb_push $st:ident, $le:ident, $hb:ident ;
        [$hb_esc:literal, $sp:literal, $sp_min:literal => $hb_fld:ident]
    ) => {
        if $hb {
            $crate::paste::paste! {
                $st.[<push_ $hb_fld>]($crate::span::Span::new($le as u32, $le as u32));
            }
        }
    };

    (@hb_push $st:ident, $le:ident, $hb:ident ; []) => {};

    (@dispatch $st:ident, $field:ident, $span:expr, $floor:expr, true) => {
        $crate::paste::paste! { $st.[<push_merge_ $field>]($span, $floor); }
    };
    (@dispatch $st:ident, $field:ident, $span:expr, $floor:expr, false) => {
        $crate::paste::paste! { $st.[<push_ $field>]($span); }
    };

    // Close every still-open block frame at end of input (or, gated by the
    // caller, on a blank line), top → down. Each frame's span is dispatched to
    // the field of the rule that opened it by `parse_block!`'s `@close_frame`
    // arm.
    (@close_stack $stack:ident, $depth:ident, $st:ident, $src:ident, $pos:expr ;
        block_simple { $($sr:tt)* } block { $($br:tt)* }
    ) => {
        while $depth > 0 {
            $depth -= 1;
            let (_c2, _s2) = $stack[$depth];
            $crate::parse_block!(@close_frame
                $st, $src, $pos as u32, _c2, _s2 ; $($sr)*);
        }
    };
}
