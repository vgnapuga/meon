//! [`ParseContext`] — the opaque-region map consumed by the `find_context_*`
//! standalone iterators, plus [`ContextCursor`], its monotone lookup cursor.
//!
//! # Purpose
//!
//! Context-free standalone iterators (`find_*`) are documented to diverge from
//! the full parse: they match delimiters inside fenced blocks and inside
//! opaque inline constructs (code spans, strings, autolinks — any rule with
//! `parse_inside = false`), because they carry no cross-element state.
//!
//! A [`ParseContext`] closes most of that gap. It is built by **one**
//! streaming pass over the source — a single unified needle search per
//! iteration (fence bytes and opaque-rule triggers share one deduplicated
//! set, dispatched to `memchr`/`memchr2`/`memchr3` or, beyond three distinct
//! bytes, to the SWAR [`crate::swar::find_any`]) — resolving every opaque
//! construct with the same leftmost-wins semantics the full parser uses:
//!
//! 1. A needle hit that is a **fence byte at a line start** is tested against
//!    `parse_block!`'s open condition (`count >= min`, no further fence byte
//!    on the info line); on success the whole region — open line through
//!    close line (`count >= open count`, remainder all `sep`/`tab`), or end
//!    of input for an unclosed fence — is recorded as one opaque region, and
//!    the scan resumes after it.
//! 2. Any other hit is an **opaque inline rule** trigger, matched
//!    **paragraph-bounded**: a match may span a single line break,
//!    exactly-count on both sides for symmetric rules, escape-aware
//!    throughout. A pending opener aborts on an empty line (two consecutive
//!    `eol` bytes), a line that opens a fence (a block construct ends the
//!    paragraph), or end of input; this mirrors the full parser and the
//!    context-aware standalone iterators. A matched construct covers its full
//!    extent, delimiters included, and the scan resumes after it — so
//!    overlapping opaque candidates resolve leftmost-first, like the full
//!    parser's trigger loop.
//!
//! The result is a sorted, non-overlapping `Vec<Span>` of opaque regions,
//! preallocated from the grammar's own capacity divisors (the same `[cap]`
//! hints that size the content vectors). A `find_context_*` iterator then
//! skips any *candidate delimiter* whose position falls inside one of them.
//! Note the semantics: the context suppresses **trigger positions**, not
//! enclosing spans — a bold span may still legally *contain* a code span,
//! exactly as in the full parse.
//!
//! # Remaining divergence from the full parse (by design)
//!
//! - Block-marker context other than fences is not modelled: a `>`
//!   continuation marker byte is not covered (it is never a trigger byte in
//!   practice), and inline runs are not segmented by blockquote peeling.
//! - This construction's own closing search is escape-aware (matching the
//!   full parser); the context-free `find_*` close search is not.

use super::common::{Span, count_escape, find_line_end};

/// One opaque symmetric rule: `(delimiter byte, exact run count)`.
pub type SymSpec = (u8, u32);
/// One opaque asymmetric rule: `(open byte, close byte, exact open count)`.
pub type AsymSpec = (u8, u8, u32);
/// One fence rule: `(fence byte, minimum run length)`.
pub type FenceSpec = (u8, u8);

/// Maximum distinct needle bytes: up to 3 fence bytes plus up to 8 opaque
/// triggers, deduplicated.
const MAX_NEEDLES: usize = 11;

/// A sorted, non-overlapping set of byte ranges covered by opaque constructs.
///
/// Built once per source via the generated `Parser::context(source)` and then
/// shared (immutably) by any number of `find_context_*` iterators over the
/// same source. See the [module documentation](self) for the construction
/// semantics.
#[derive(Debug, Clone, Default)]
pub struct ParseContext {
    spans: Vec<Span>,
}

impl ParseContext {
    /// The opaque regions, sorted by `start`, non-overlapping.
    #[inline]
    pub fn spans(&self) -> &[Span] {
        &self.spans
    }

    /// A fresh monotone lookup cursor over this context.
    #[inline]
    pub fn cursor(&self) -> ContextCursor<'_> {
        ContextCursor {
            spans: &self.spans,
            idx: 0,
        }
    }

    /// Build the context in one streaming pass over `source`.
    ///
    /// - `fences` — every `fence(byte, min)` rule of the grammar.
    /// - `sym` — every `symmetric` arm with `parse_inside = false`.
    /// - `asym` — every `asymmetric` arm with `parse_inside = false`.
    /// - `cap_hint` — preallocation hint for the region vector; the generated
    ///   `Parser::context` derives it from the grammar's `[cap]` divisors
    ///   (`0` is always safe and merely re-grows).
    ///
    /// All slices may be empty; an empty rule set yields an empty context.
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        source: &[u8],
        eol: u8,
        escape: u8,
        sep: u8,
        tab: u8,
        fences: &[FenceSpec],
        sym: &[SymSpec],
        asym: &[AsymSpec],
        cap_hint: usize,
    ) -> Self {
        let len = source.len();
        let mut spans: Vec<Span> = Vec::with_capacity(cap_hint);

        // One deduplicated needle set — fence bytes and opaque triggers
        // together — so every iteration issues exactly ONE search
        // (memchr/2/3, or SWAR beyond three distinct bytes), never several.
        let mut needle_buf = [0u8; MAX_NEEDLES];
        let nn = build_needles(fences, sym, asym, &mut needle_buf);
        let needles = &needle_buf[..nn];

        let mut pos: usize = 0;
        while pos < len {
            let Some(r) = find_needle(needles, &source[pos..]) else {
                break;
            };
            let p = pos + r;
            let b = source[p];

            // Fence first, and only at a line start — exactly the full
            // parser's precedence. (A byte may be both a fence byte and an
            // inline trigger, e.g. a backtick; a rejected open falls through
            // to the inline rule below, mirroring `parse_block!`.)
            if is_fence_byte(b, fences) && (p == 0 || source[p - 1] == eol) {
                let le = find_line_end(source, p, eol);
                if let Some(open_count) = fence_opens(source, p, le, fences) {
                    let end = fence_close_scan(source, le, len, b, open_count, eol, sep, tab);
                    spans.push(Span::new(p as u32, end as u32));
                    pos = if end < len { end + 1 } else { len };
                    continue;
                }
            }

            // Inline opaque rule owning this byte, if any (a pure fence byte
            // that failed to open is ordinary content).
            let Some((is_sym, ri)) = rule_for(b, sym, asym) else {
                pos = p + 1;
                continue;
            };

            if count_escape(source, p, escape) % 2 == 1 {
                pos = p + 1;
                continue;
            }

            if is_sym {
                let (byte, count) = sym[ri];
                let mut run_end = p;
                let mut c = 0u32;
                while run_end < len && source[run_end] == byte {
                    c += 1;
                    run_end += 1;
                }
                if c != count {
                    pos = run_end;
                    continue;
                }
                match sym_close(source, run_end, len, byte, count, escape, eol, fences) {
                    Some(close_end) => {
                        spans.push(Span::new(p as u32, close_end as u32));
                        pos = close_end;
                    }
                    None => pos = run_end,
                }
            } else {
                let (open, close, count) = asym[ri];
                let mut run_end = p;
                let mut c = 0u32;
                while run_end < len && source[run_end] == open {
                    c += 1;
                    run_end += 1;
                }
                if c != count {
                    pos = run_end;
                    continue;
                }
                match asym_close(source, run_end, len, close, escape, eol, fences) {
                    Some(cp) => {
                        spans.push(Span::new(p as u32, (cp + 1) as u32));
                        pos = cp + 1;
                    }
                    None => pos = run_end,
                }
            }
        }

        Self { spans }
    }
}

/// Deduplicated union of every fence byte, symmetric marker and asymmetric
/// opener, written into `buf`. Returns the count written. Overflow beyond
/// [`MAX_NEEDLES`] is silently dropped (a dropped needle only means its rule
/// is missed, never unsound); no known grammar comes close.
fn build_needles(
    fences: &[FenceSpec],
    sym: &[SymSpec],
    asym: &[AsymSpec],
    buf: &mut [u8; MAX_NEEDLES],
) -> usize {
    let mut n = 0;
    let mut push = |b: u8, n: &mut usize| {
        if *n < MAX_NEEDLES && !buf[..*n].contains(&b) {
            buf[*n] = b;
            *n += 1;
        }
    };
    for &(fb, _) in fences {
        push(fb, &mut n);
    }
    for &(b, _) in sym {
        push(b, &mut n);
    }
    for &(o, _, _) in asym {
        push(o, &mut n);
    }
    n
}

/// One search over `hay` for any of the `needles`: `memchr`-family for up to
/// three distinct bytes, SWAR [`crate::swar::find_any`] beyond.
#[inline]
fn find_needle(needles: &[u8], hay: &[u8]) -> Option<usize> {
    let n = needles;
    match n.len() {
        0 => None,
        1 => memchr::memchr(n[0], hay),
        2 => memchr::memchr2(n[0], n[1], hay),
        3 => memchr::memchr3(n[0], n[1], n[2], hay),
        4 => crate::swar::find_any([n[0], n[1], n[2], n[3]], hay),
        5 => crate::swar::find_any([n[0], n[1], n[2], n[3], n[4]], hay),
        6 => crate::swar::find_any([n[0], n[1], n[2], n[3], n[4], n[5]], hay),
        7 => crate::swar::find_any([n[0], n[1], n[2], n[3], n[4], n[5], n[6]], hay),
        8 => crate::swar::find_any([n[0], n[1], n[2], n[3], n[4], n[5], n[6], n[7]], hay),
        9 => crate::swar::find_any([n[0], n[1], n[2], n[3], n[4], n[5], n[6], n[7], n[8]], hay),
        10 => crate::swar::find_any(
            [n[0], n[1], n[2], n[3], n[4], n[5], n[6], n[7], n[8], n[9]],
            hay,
        ),
        _ => crate::swar::find_any(
            [
                n[0], n[1], n[2], n[3], n[4], n[5], n[6], n[7], n[8], n[9], n[10],
            ],
            hay,
        ),
    }
}

/// Is `b` one of the grammar's fence bytes?
#[inline]
fn is_fence_byte(b: u8, fences: &[FenceSpec]) -> bool {
    fences.iter().any(|&(fb, _)| fb == b)
}

/// The opaque inline rule owning trigger byte `b`, as `(is_symmetric, index)`.
/// Ties cannot occur across rules — the engine assumes a byte has one meaning
/// per grammar.
#[inline]
fn rule_for(b: u8, sym: &[SymSpec], asym: &[AsymSpec]) -> Option<(bool, usize)> {
    for (ri, &(sb, _)) in sym.iter().enumerate() {
        if sb == b {
            return Some((true, ri));
        }
    }
    for (ri, &(o, _, _)) in asym.iter().enumerate() {
        if o == b {
            return Some((false, ri));
        }
    }
    None
}

/// Does a fence open at `pos` on `[pos, line_end)`? Returns the run count.
///
/// Mirrors `parse_block!`'s `@open_simple` fence arm: run length `>= min` and
/// no further fence byte on the info line.
#[inline]
fn fence_opens(src: &[u8], pos: usize, line_end: usize, fences: &[FenceSpec]) -> Option<u8> {
    if pos >= line_end {
        return None;
    }
    let b = src[pos];
    for &(fb, fmin) in fences {
        if b == fb {
            let mut i = pos;
            let mut c: u8 = 0;
            while i < line_end && src[i] == fb {
                c = c.saturating_add(1);
                i += 1;
            }
            if c >= fmin && src[i..line_end].iter().all(|&x| x != fb) {
                return Some(c);
            }
            return None;
        }
    }
    None
}

/// Does the line `[pos, line_end)` close an open fence of `byte` × `open_count`?
///
/// Mirrors `parse_block!`'s peel-phase close: run `>= open_count`, remainder
/// all `sep` / `tab`.
#[inline]
fn fence_closes(
    src: &[u8],
    pos: usize,
    line_end: usize,
    byte: u8,
    open_count: u8,
    sep: u8,
    tab: u8,
) -> bool {
    let mut i = pos;
    let mut c: u8 = 0;
    while i < line_end && src[i] == byte {
        c = c.saturating_add(1);
        i += 1;
    }
    c >= open_count && src[i..line_end].iter().all(|&b| b == sep || b == tab)
}

/// Streaming search for the closing fence line: `memchr` for the fence byte,
/// line-start check, then `parse_block!`'s close condition. Returns the close
/// line's end, or `len` for an unclosed fence. Content lines inside the block
/// are never walked.
#[allow(clippy::too_many_arguments)]
fn fence_close_scan(
    src: &[u8],
    open_line_end: usize,
    len: usize,
    byte: u8,
    open_count: u8,
    eol: u8,
    sep: u8,
    tab: u8,
) -> usize {
    let mut search = if open_line_end < len {
        open_line_end + 1
    } else {
        len
    };
    loop {
        let Some(r) = memchr::memchr(byte, &src[search..]) else {
            return len;
        };
        let q = search + r;
        if q > 0 && src[q - 1] != eol {
            search = q + 1;
            continue;
        }
        let cle = find_line_end(src, q, eol);
        if fence_closes(src, q, cle, byte, open_count, sep, tab) {
            return cle;
        }
        search = if cle < len { cle + 1 } else { len };
    }
}

/// Would the line starting at `ls` end the paragraph by opening a fence?
#[inline]
fn fence_opens_line(src: &[u8], ls: usize, eol: u8, fences: &[FenceSpec]) -> bool {
    if !is_fence_byte(src[ls], fences) {
        return false;
    }
    let le = find_line_end(src, ls, eol);
    fence_opens(src, ls, le, fences).is_some()
}

/// Escape-aware, paragraph-bounded symmetric close search starting at `from`.
/// Crosses a single `eol` (a pair may span one line break within a
/// paragraph), aborting — like the full parser and the context-aware
/// standalone iterators — on an empty line, a fence-opening line (a block
/// construct ends the paragraph), or end of input.
#[allow(clippy::too_many_arguments)]
#[inline]
fn sym_close(
    src: &[u8],
    from: usize,
    len: usize,
    byte: u8,
    count: u32,
    escape: u8,
    eol: u8,
    fences: &[FenceSpec],
) -> Option<usize> {
    let mut j = from;
    loop {
        let r = memchr::memchr2(byte, eol, &src[j..])?;
        let q = j + r;
        if src[q] == eol {
            if q + 1 >= len || src[q + 1] == eol || fence_opens_line(src, q + 1, eol, fences) {
                return None;
            }
            j = q + 1;
            continue;
        }
        if count_escape(src, q, escape) % 2 == 1 {
            j = q + 1;
            continue;
        }
        let mut cc = 0u32;
        let mut tmp = q;
        while tmp < len && src[tmp] == byte {
            cc += 1;
            tmp += 1;
        }
        if cc == count {
            return Some(tmp);
        }
        j = tmp;
    }
}

/// Escape-aware, paragraph-bounded asymmetric close search starting at
/// `from`. Same crossing/abort rules as [`sym_close`].
#[inline]
fn asym_close(
    src: &[u8],
    from: usize,
    len: usize,
    close: u8,
    escape: u8,
    eol: u8,
    fences: &[FenceSpec],
) -> Option<usize> {
    let mut j = from;
    loop {
        let r = memchr::memchr2(close, eol, &src[j..])?;
        let q = j + r;
        if src[q] == eol {
            if q + 1 >= len || src[q + 1] == eol || fence_opens_line(src, q + 1, eol, fences) {
                return None;
            }
            j = q + 1;
            continue;
        }
        if count_escape(src, q, escape) % 2 == 1 {
            j = q + 1;
            continue;
        }
        return Some(q);
    }
}

/// A monotone lookup cursor over a [`ParseContext`].
///
/// Amortized O(1) per query **provided queries arrive in non-decreasing
/// position order** — the cursor only ever moves forward. Copy a cursor to
/// fork a lookahead (e.g. a close-delimiter search) without disturbing the
/// parent's position: the copy starts at the parent's index, which is valid
/// for any position `>=` the parent's last query.
#[derive(Debug, Clone, Copy)]
pub struct ContextCursor<'m> {
    spans: &'m [Span],
    idx: usize,
}

impl ContextCursor<'_> {
    /// Is `pos` inside an opaque region?
    #[inline]
    pub fn is_covered(&mut self, pos: usize) -> bool {
        self.covering_end(pos).is_some()
    }

    /// If `pos` is inside an opaque region, return that region's `end` offset
    /// (exclusive) so the caller can jump past it. `None` when uncovered.
    #[inline]
    pub fn covering_end(&mut self, pos: usize) -> Option<usize> {
        while self.idx < self.spans.len() && (self.spans[self.idx].end as usize) <= pos {
            self.idx += 1;
        }
        match self.spans.get(self.idx) {
            Some(s) if (s.start as usize) <= pos => Some(s.end as usize),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn md_context(src: &[u8]) -> ParseContext {
        // meon-md's opaque rule set: ``` fences, `code` spans, <autolinks>.
        ParseContext::build(
            src,
            b'\n',
            b'\\',
            b' ',
            b'\t',
            &[(b'`', 3)],
            &[(b'`', 1)],
            &[(b'<', b'>', 1)],
            0,
        )
    }

    // 01. Empty rule set / empty source produce an empty context
    #[test]
    fn test_01_empty() {
        let m = ParseContext::build(b"abc", b'\n', b'\\', b' ', b'\t', &[], &[], &[], 0);
        assert!(m.spans().is_empty());
        let m = md_context(b"");
        assert!(m.spans().is_empty());
    }

    // 02. A code span covers its full extent, delimiters included
    #[test]
    fn test_02_code_span() {
        let m = md_context(b"a `code` b");
        assert_eq!(m.spans(), &[Span::new(2, 8)]);
    }

    // 03. An autolink covers open through close byte
    #[test]
    fn test_03_autolink() {
        let m = md_context(b"x <http://a> y");
        assert_eq!(m.spans(), &[Span::new(2, 12)]);
    }

    // 04. A fenced block covers open line through close line
    #[test]
    fn test_04_fence() {
        let src = b"before\n```\n**not bold**\n```\nafter";
        let m = md_context(src);
        assert_eq!(m.spans(), &[Span::new(7, 27)]);
    }

    // 05. An unclosed fence covers to end of input
    #[test]
    fn test_05_unclosed_fence() {
        let src = b"a\n```\nrest never closes";
        let m = md_context(src);
        assert_eq!(m.spans(), &[Span::new(2, src.len() as u32)]);
    }

    // 06. Leftmost-wins: an autolink open inside a code span is not a region
    //     of its own; scanning resumes after the code span
    #[test]
    fn test_06_leftmost_wins() {
        let m = md_context(b"`a <b` c> d <e>");
        // code span [0,6); the `<b` inside it is invisible; `c>` alone is no
        // autolink (no `<`), the next real autolink is `<e>` at 12..15.
        assert_eq!(m.spans(), &[Span::new(0, 6), Span::new(12, 15)]);
    }

    // 07. Escaped opening delimiter does not open an opaque region; the next
    //     unescaped delimiter becomes the opener (full-parse semantics)
    #[test]
    fn test_07_escaped_open() {
        let m = md_context(b"a \\`not code` b `real`");
        // `\`` at 3 is literal; the backtick at 12 opens, closing at 16.
        assert_eq!(m.spans(), &[Span::new(12, 17)]);
    }

    // 08. Escaped closing delimiter is skipped by the close search
    #[test]
    fn test_08_escaped_close() {
        let m = md_context(b"`a\\`b` rest");
        assert_eq!(m.spans(), &[Span::new(0, 6)]);
    }

    // 09. Unclosed inline construct covers nothing
    #[test]
    fn test_09_unclosed_inline() {
        let m = md_context(b"a `never closes");
        assert!(m.spans().is_empty());
    }

    // ---- Paragraph-bounded behaviour --------------------------------- //

    // 10. Opaque matching is paragraph-bounded: a pair may span a single line
    //     break within one paragraph, exactly like the context-aware
    //     standalone iterators.
    #[test]
    fn test_10_pair_spans_single_newline() {
        let m = md_context(b"a `x\ny` b");
        assert_eq!(m.spans(), &[Span::new(2, 7)]);
    }

    // 11. An empty line (two consecutive eol bytes) aborts a pending opener:
    //     opaque constructs in different paragraphs never pair.
    #[test]
    fn test_11_empty_line_aborts_open() {
        let m = md_context(b"a `x\n\ny` b");
        assert!(m.spans().is_empty());
    }

    // 12. A fence-opening line aborts a pending opener — a block construct
    //     ends the paragraph — but the fence itself still gets its own
    //     region.
    #[test]
    fn test_12_fence_aborts_pending_open() {
        let m = md_context(b"a `x\n```\ncode\n```\n");
        assert_eq!(m.spans(), &[Span::new(5, 17)]);
    }

    // 13. A fence info line with a trailing fence byte does not open
    #[test]
    fn test_13_fence_info_line_reject() {
        let m = md_context(b"``` info ` tick\ncontent\n```\n");
        // The open line is rejected (` on the info line), so the *inline*
        // code-span rule sees the ``` run (count 3 != 1, no match); third
        // line's ``` opens an (unclosed) fence to EOF.
        assert_eq!(m.spans(), &[Span::new(24, 28)]);
    }

    // 14. Fence close requires at least the open count
    #[test]
    fn test_14_fence_close_count() {
        let src = b"````\nx\n```\n````\ny";
        let m = md_context(src);
        assert_eq!(m.spans(), &[Span::new(0, 15)]);
    }

    // 15. Cursor: monotone queries, covering_end jumps
    #[test]
    fn test_15_cursor() {
        let m = md_context(b"a `b` c `d` e");
        let mut cur = m.cursor();
        assert!(!cur.is_covered(0));
        assert_eq!(cur.covering_end(2), Some(5));
        assert_eq!(cur.covering_end(4), Some(5));
        assert!(!cur.is_covered(6));
        assert_eq!(cur.covering_end(9), Some(11));
        assert!(!cur.is_covered(12));
    }

    // 16. Cursor copies fork independently
    #[test]
    fn test_16_cursor_copy() {
        let m = md_context(b"a `b` c `d` e");
        let mut cur = m.cursor();
        assert!(!cur.is_covered(1));
        let mut fork = cur;
        assert_eq!(fork.covering_end(9), Some(11));
        // Parent still answers correctly for positions >= its own last query.
        assert_eq!(cur.covering_end(2), Some(5));
    }

    // 17. JSON-shaped rule set: strings only, no fences
    #[test]
    fn test_17_json_strings() {
        let src = br#"{"a": "x, {not open}", "b": 1}"#;
        let m = ParseContext::build(src, b'\n', b'\\', b' ', b'\t', &[], &[(b'"', 1)], &[], 0);
        // "a" -> [1,4), "x, {not open}" -> [6,21), "b" -> [23,26)
        assert_eq!(
            m.spans(),
            &[Span::new(1, 4), Span::new(6, 21), Span::new(23, 26)]
        );
    }

    // 18. A fence byte distinct from every trigger (`~~~` fence with backtick
    //     code spans) exercises the pure-fence-byte needle path
    #[test]
    fn test_18_distinct_fence_byte() {
        let src = b"`c`\n~~~\n`hidden`\n~~~\n`d`";
        let m = ParseContext::build(
            src,
            b'\n',
            b'\\',
            b' ',
            b'\t',
            &[(b'~', 3)],
            &[(b'`', 1)],
            &[],
            0,
        );
        assert_eq!(
            m.spans(),
            &[Span::new(0, 3), Span::new(4, 20), Span::new(21, 24)]
        );
    }
    // ---- Wide needle sets, fence-byte edges, asymmetric branches -------- //

    // 19. Every find_needle arm from 4 through 11+ needles finds a pair of
    //     the last (rarest) trigger byte — the SWAR dispatch works end to end
    #[test]
    fn test_19_wide_needle_sets() {
        let alphabet = *b"qwzjvxkyfgd";
        for n in 4..=11usize {
            let sym: Vec<SymSpec> = alphabet[..n].iter().map(|&b| (b, 1)).collect();
            let last = alphabet[n - 1];
            let src = [last, b'h', b'i', last, b' ', b'.'];
            let m = ParseContext::build(&src, b'\n', b'\\', b' ', b'\t', &[], &sym, &[], 0);
            assert_eq!(m.spans(), &[Span::new(0, 4)], "needle count {n}");
        }
    }

    // 20. Needles beyond the fixed buffer are dropped: the overflowing rule
    //     simply never matches, everything else still does
    #[test]
    fn test_20_needle_overflow_dropped() {
        let alphabet = *b"qwzjvxkyfgdm";
        let sym: Vec<SymSpec> = alphabet.iter().map(|&b| (b, 1)).collect();
        let m = ParseContext::build(b"mam qbq", b'\n', b'\\', b' ', b'\t', &[], &sym, &[], 0);
        assert_eq!(m.spans(), &[Span::new(4, 7)]);
    }

    // 21. Two distinct fence rules coexist; a mid-line or short-run fence
    //     byte that opens nothing is ordinary content
    #[test]
    fn test_21_two_fence_rules() {
        let src = b"~~ not\n```\na\n```\n~~~\nb\n~~~\nx ``` y";
        let m = ParseContext::build(
            src,
            b'\n',
            b'\\',
            b' ',
            b'\t',
            &[(b'`', 3), (b'~', 3)],
            &[],
            &[],
            0,
        );
        assert_eq!(m.spans(), &[Span::new(7, 16), Span::new(17, 26)]);
    }

    // 22. Close-fence scan: a mid-line fence byte and a close-line with
    //     trailing junk are both skipped before the true close
    #[test]
    fn test_22_fence_close_scan_edges() {
        let src = b"```\ncode ` tick\n``` x\n```\nafter";
        let m = md_context(src);
        assert_eq!(m.spans(), &[Span::new(0, 25)]);
    }

    // 23. Asymmetric: a wrong-length open run and an unclosed opener both
    //     yield nothing; escaped close candidates are skipped
    #[test]
    fn test_23_asym_branches() {
        let m = md_context(b"<<double> x");
        assert!(m.spans().is_empty());
        let m = md_context(b"a <never closes");
        assert!(m.spans().is_empty());
        let m = md_context(b"<a\\>b> c");
        assert_eq!(m.spans(), &[Span::new(0, 6)]);
    }

    // 24. Asymmetric close search is paragraph-bounded: crosses one newline,
    //     aborts on an empty line and on a fence-opening line
    #[test]
    fn test_24_asym_paragraph_bounds() {
        let m = md_context(b"<a\nb> c");
        assert_eq!(m.spans(), &[Span::new(0, 5)]);
        let m = md_context(b"<a\n\nb> c");
        assert!(m.spans().is_empty());
        let m = md_context(b"<a\n```\nx\n```\nb> c");
        assert_eq!(m.spans(), &[Span::new(3, 12)]);
    }

    // 25. Symmetric close search skips a wrong-count closing run
    #[test]
    fn test_25_sym_close_run_mismatch() {
        let m = md_context(b"`a``b` x");
        assert_eq!(m.spans(), &[Span::new(0, 6)]);
    }
}
