//! [`ChainedIter`] — two-part chained delimiter standalone iterator — and
//! the forward search of an opaque `chained` rule, shared with the context
//! builder.

use super::common::*;
use super::context::ChainSpec;

/// What the searches of one `chained` rule learned, for the openers after
/// them: the full parse keeps the same, so a run of unclosed openers costs
/// one search, not one per opener.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ChainMemo {
    /// `(start, stop)`: no unescaped first close byte lies in `[start, stop)`,
    /// and `stop` ends the inline run.
    first_dead: Option<(usize, usize)>,
    /// `(start, close)`: the first unescaped first close byte from `start` on
    /// is at `close`, and the match after it failed. Kept for a text
    /// component with `balanced = false` only.
    first_found: Option<(usize, usize)>,
    /// As `first_dead`, for the second close byte.
    second_dead: Option<(usize, usize)>,
}

/// The text and second close bytes of the opaque `chained` match whose text
/// component starts at `from`, right after the open byte, or `None` when it
/// fails: the first close, the second open on the byte right after it, and the
/// second close, all in the opener's inline run (`enter_line` as for
/// [`next_in_paragraph`]).
#[inline]
pub(crate) fn chain_match(
    src: &[u8],
    from: usize,
    spec: &ChainSpec,
    escape: u8,
    breaks: LineBreaks,
    mut enter_line: impl FnMut(usize) -> Option<usize>,
    memo: &mut ChainMemo,
) -> Option<(usize, usize)> {
    let (open, close) = spec.first;
    let found = memo.first_found.filter(|&(s, c)| s <= from && from <= c);
    let dead = memo.first_dead.filter(|&(s, e)| s <= from && from < e);
    let ct = match (found, dead) {
        (Some((_, c)), _) => c,
        (None, Some(_)) => return None,
        (None, None) => {
            let bal = spec.first_balanced;
            match component_close(src, from, open, close, bal, escape, breaks, &mut enter_line) {
                Ok(c) => c,
                Err((stop, met)) => {
                    if !met {
                        memo.first_dead = Some((from, stop));
                    }
                    return None;
                }
            }
        }
    };
    let (uo, uc) = spec.second;
    let next = ct + 1;
    if next < src.len() && src[next] == uo {
        let second_from = next + 1;
        if !memo
            .second_dead
            .is_some_and(|(s, e)| s <= second_from && second_from < e)
        {
            let bal = spec.second_balanced;
            match component_close(
                src,
                second_from,
                uo,
                uc,
                bal,
                escape,
                breaks,
                &mut enter_line,
            ) {
                Ok(cu) => return Some((ct, cu)),
                Err((stop, met)) => {
                    if !met {
                        memo.second_dead = Some((second_from, stop));
                    }
                }
            }
        }
    }
    // The match failed after its first close: an opener before that close
    // finds the same one.
    if !spec.first_balanced {
        memo.first_found = Some((from, ct));
    }
    None
}

/// The close byte of a `chained` component whose content starts at `from`,
/// in the inline run `enter_line` bounds: the first unescaped `close`, or,
/// with `balanced`, the one that brings the depth back to zero, each
/// unescaped `open` nesting one level. On failure, where the run ends and
/// whether an unescaped `close` was met on the way.
#[inline]
#[allow(clippy::too_many_arguments)]
pub(crate) fn component_close(
    src: &[u8],
    from: usize,
    open: u8,
    close: u8,
    balanced: bool,
    escape: u8,
    breaks: LineBreaks,
    mut enter_line: impl FnMut(usize) -> Option<usize>,
) -> Result<usize, (usize, bool)> {
    let eol = breaks.eol;
    if !balanced {
        // The first unescaped close; an `Err` met none.
        let mut j = from;
        return loop {
            match scan_paragraph(src, j, close, breaks, &mut enter_line) {
                Ok(q) if count_escape(src, q, escape) % 2 == 1 => j = q + 1,
                Ok(q) => break Ok(q),
                Err(stop) => break Err((stop, false)),
            }
        };
    }
    let len = src.len();
    let mut depth = 1u32;
    let mut met = false;
    let mut j = from;
    loop {
        let Some(r) = memchr::memchr3(open, close, eol, &src[j..]) else {
            return Err((len, met));
        };
        let q = j + r;
        if src[q] == eol {
            if breaks.ends_paragraph(src, q) {
                return Err((q, met));
            }
            match enter_line(q + 1) {
                Some(next) => j = next,
                None => return Err((q, met)),
            }
            continue;
        }
        if count_escape(src, q, escape) % 2 == 0 {
            // A close byte that is also the open byte nests, yet it is met.
            met |= src[q] == close;
            if src[q] == open {
                depth += 1;
            } else {
                depth -= 1;
                if depth == 0 {
                    return Ok(q);
                }
            }
        }
        j = q + 1;
    }
}

/// Where a transparent match stands: in no match, in its first component, or
/// in its second component. `depth` counts the levels a `balanced` component
/// has nested.
#[derive(Debug, Clone, Copy)]
enum Phase {
    Idle,
    First {
        prefix: bool,
        start: usize,
        depth: u32,
    },
    Second {
        prefix: bool,
        first: (usize, usize),
        start: usize,
        depth: u32,
    },
}

/// Iterator over the matches of a `chained` rule in a byte slice.
///
/// Matches the pattern `[prefix]open1...close1 open2...close2`, where `prefix` is
/// an optional single byte (e.g. `!` for image links). The iterator yields one
/// item per match via the `make` closure, which receives `(is_prefix, span1,
/// span2)` and constructs the output type `T`.
///
/// It reads the rule the way the full parse does:
///
/// - Of an unescaped run of `open1` bytes the last one opens the match, and
///   the prefix is the byte before it: with `[`, `]`, `(`, `)` and the
///   prefix `!`, in `![[a](u)` the match is `[a](u)`, with no prefix.
/// - A component closes at its first unescaped close byte, or, with
///   `balanced`, at the one that brings it back to the level it opened at:
///   each unescaped open byte nests one level. The second component opens only
///   on the byte right after the first close.
/// - A rule whose components are both opaque (`transparent = false`) is a
///   forward search per opener: a match that fails resumes the scan right
///   after its open run, so a later opener inside it may match.
/// - A rule with a transparent component is one match at a time, read in
///   one pass: a match that fails at its first close resumes the scan right
///   after that close, and one that never closes is dropped where its
///   paragraph ends. Openers inside a match are never retried, and inside a
///   component with `balanced = false` an `open1` byte is text.
///
/// A match stays inside one paragraph: a component may span single line
/// breaks, but a blank line (see [`LineBreaks`]) or the end of input ends
/// it. Other rules are not seen: in the full parse an opaque
/// construct opened before a transparent component's close byte hides it,
/// and other rules' line structure ends paragraphs.
///
/// What a failed search learns is kept for the openers after it: a close
/// byte absent up to the end of the paragraph, and, for a first component
/// with `balanced = false`, a first close whose match failed after it. A run
/// of unclosed openers costs one search, not one per opener. A search of a
/// `balanced` component that meets close bytes is not kept, as a later
/// opener may balance where it did not: `k` such openers in an `n`-byte
/// paragraph cost `O(k·n)`, as in the full parse.
///
/// Obtained via the generated `Parser::find_*` methods; rarely constructed
/// directly.
pub struct ChainedIter<'a, T, F>
where
    F: Fn(bool, Span, Span) -> T,
{
    src: &'a [u8],
    spec: ChainSpec,
    prefix: u8,
    transparent: bool,
    breaks: LineBreaks,
    escape: u8,
    make: F,
    pos: usize,
    memo: ChainMemo,
    phase: Phase,
    _t: std::marker::PhantomData<T>,
}

impl<'a, T, F> ChainedIter<'a, T, F>
where
    F: Fn(bool, Span, Span) -> T,
{
    /// Create an iterator over a `chained` rule's matches.
    ///
    /// # Parameters
    ///
    /// - `src` — source byte slice to scan.
    /// - `spec` — the open and close bytes of the two components (e.g.
    ///   `[`/`]` for the text part of a link, `(`/`)` for the url part) and
    ///   their `balanced` flags.
    /// - `prefix` — optional single byte immediately before `open1` that sets
    ///   the `is_prefix` flag passed to `make` (e.g. `b'!'` for images).
    /// - `transparent` — does either component have `parse_inside = true`?
    /// - `breaks` — how lines end: an `eol` byte or a [`LineBreaks`]; a
    ///   component may span single line breaks, but never a blank line.
    /// - `escape` — escape prefix byte; a delimiter or prefix byte preceded
    ///   by an odd number of escape bytes is none.
    /// - `make` — closure that receives `(is_prefix, span1, span2)` and
    ///   constructs the output value `T`.
    pub fn new(
        src: &'a [u8],
        spec: ChainSpec,
        prefix: u8,
        transparent: bool,
        breaks: impl Into<LineBreaks>,
        escape: u8,
        make: F,
    ) -> Self {
        Self {
            src,
            spec,
            prefix,
            transparent,
            breaks: breaks.into(),
            escape,
            make,
            pos: 0,
            memo: ChainMemo::default(),
            phase: Phase::Idle,
            _t: std::marker::PhantomData,
        }
    }

    /// Does an unescaped prefix byte stand right before the opener at `open`?
    #[inline]
    fn prefixed(&self, open: usize) -> bool {
        open > 0
            && self.src[open - 1] == self.prefix
            && count_escape(self.src, open - 1, self.escape) % 2 == 0
    }

    /// The end of the run of `src[p]` that starts at `p`.
    #[inline]
    fn run_end(&self, p: usize) -> usize {
        let b = self.src[p];
        let mut e = p;
        while e < self.src.len() && self.src[e] == b {
            e += 1;
        }
        e
    }

    /// The next match of a rule whose components are both opaque.
    fn next_opaque(&mut self) -> Option<T> {
        let src = self.src;
        let open = self.spec.first.0;
        loop {
            let p = memchr::memchr(open, &src[self.pos..])? + self.pos;
            if count_escape(src, p, self.escape) % 2 == 1 {
                self.pos = p + 1;
                continue;
            }
            let from = self.run_end(p);
            self.pos = from;
            let m = chain_match(
                src,
                from,
                &self.spec,
                self.escape,
                self.breaks,
                Some,
                &mut self.memo,
            );
            if let Some((ct, cu)) = m {
                self.pos = cu + 1;
                let prefix = self.prefixed(from - 1);
                return Some((self.make)(
                    prefix,
                    Span::new(from as u32, ct as u32),
                    Span::new((ct + 2) as u32, cu as u32),
                ));
            }
        }
    }

    /// The next match of a rule with a transparent component: the full
    /// parse's two-phase machine, one match at a time.
    fn next_transparent(&mut self) -> Option<T> {
        let src = self.src;
        let len = src.len();
        let ChainSpec {
            first: (co, cc),
            first_balanced: tbal,
            second: (uo, uc),
            second_balanced: ubal,
        } = self.spec;
        let eol = self.breaks.eol;
        loop {
            let hay = &src[self.pos..];
            let hit = match self.phase {
                Phase::Idle => memchr::memchr(co, hay),
                Phase::First { .. } if tbal => memchr::memchr3(co, cc, eol, hay),
                Phase::First { .. } => memchr::memchr2(cc, eol, hay),
                Phase::Second { .. } if ubal => memchr::memchr3(uo, uc, eol, hay),
                Phase::Second { .. } => memchr::memchr2(uc, eol, hay),
            };
            let Some(r) = hit else {
                self.pos = len;
                self.phase = Phase::Idle;
                return None;
            };
            let q = self.pos + r;
            let b = src[q];
            // The match ends with its paragraph: a single `eol` is content.
            if b == eol && !matches!(self.phase, Phase::Idle) {
                if self.breaks.ends_paragraph(src, q) {
                    self.phase = Phase::Idle;
                }
                self.pos = q + 1;
                continue;
            }
            if count_escape(src, q, self.escape) % 2 == 1 {
                self.pos = q + 1;
                continue;
            }
            let e = self.run_end(q);
            let count = (e - q) as u32;
            self.pos = e;
            match self.phase {
                Phase::Idle => {
                    self.phase = Phase::First {
                        prefix: self.prefixed(e - 1),
                        start: e,
                        depth: 0,
                    };
                }
                Phase::First {
                    prefix,
                    start,
                    depth,
                } => {
                    if tbal && b == co {
                        self.phase = Phase::First {
                            prefix,
                            start,
                            depth: depth + count,
                        };
                    } else if count <= depth {
                        self.phase = Phase::First {
                            prefix,
                            start,
                            depth: depth - count,
                        };
                    } else {
                        // The rest of the close run is read again.
                        let ct = q + depth as usize;
                        let next = ct + 1;
                        if next < len && src[next] == uo {
                            self.phase = Phase::Second {
                                prefix,
                                first: (start, ct),
                                start: next + 1,
                                depth: 0,
                            };
                            self.pos = next + 1;
                        } else {
                            self.phase = Phase::Idle;
                            self.pos = next;
                        }
                    }
                }
                Phase::Second {
                    prefix,
                    first,
                    start,
                    depth,
                } => {
                    if ubal && b == uo {
                        self.phase = Phase::Second {
                            prefix,
                            first,
                            start,
                            depth: depth + count,
                        };
                    } else if count <= depth {
                        self.phase = Phase::Second {
                            prefix,
                            first,
                            start,
                            depth: depth - count,
                        };
                    } else {
                        let cu = q + depth as usize;
                        self.phase = Phase::Idle;
                        self.pos = cu + 1;
                        return Some((self.make)(
                            prefix,
                            Span::new(first.0 as u32, first.1 as u32),
                            Span::new(start as u32, cu as u32),
                        ));
                    }
                }
            }
        }
    }
}

impl<T, F> Iterator for ChainedIter<'_, T, F>
where
    F: Fn(bool, Span, Span) -> T,
{
    type Item = T;

    fn next(&mut self) -> Option<T> {
        if self.transparent {
            self.next_transparent()
        } else {
            self.next_opaque()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINK: ChainSpec = ChainSpec {
        first: (b'[', b']'),
        first_balanced: false,
        second: (b'(', b')'),
        second_balanced: false,
    };

    fn stub_make(is_prefix: bool, text: Span, url: Span) -> (bool, Span, Span) {
        (is_prefix, text, url)
    }

    // 01. Parses a clean, standard link pattern without any prefix
    #[test]
    fn test_01_standard_link() {
        let src = b"[text](url)";
        let mut iter = ChainedIter::new(src, LINK, b'!', false, b'\n', b'\\', stub_make);

        assert_eq!(
            iter.next(),
            Some((false, Span::new(1, 5), Span::new(7, 10)))
        );
        assert_eq!(iter.next(), None);
    }

    // 02. Detects the unescaped prefix byte matching an image pattern
    #[test]
    fn test_02_image_with_prefix() {
        let src = b"![img](lnk)";
        let mut iter = ChainedIter::new(src, LINK, b'!', false, b'\n', b'\\', stub_make);

        assert_eq!(iter.next(), Some((true, Span::new(2, 5), Span::new(7, 10))));
        assert_eq!(iter.next(), None);
    }

    // 03. Skips an open marker if it is preceded by an odd number of escape characters
    #[test]
    fn test_03_escaped_open_marker_skipped() {
        let src = b"\\[skip](no)[x](y)";
        let mut iter = ChainedIter::new(src, LINK, b'!', false, b'\n', b'\\', stub_make);

        assert_eq!(
            iter.next(),
            Some((false, Span::new(12, 13), Span::new(15, 16)))
        );
        assert_eq!(iter.next(), None);
    }

    // 04. Rejects patterns where the second open marker does not immediately follow the first close marker
    #[test]
    fn test_04_broken_chain_with_spaces_skipped() {
        let src = b"[a] (b)[x](y)";
        let mut iter = ChainedIter::new(src, LINK, b'!', false, b'\n', b'\\', stub_make);

        assert_eq!(
            iter.next(),
            Some((false, Span::new(8, 9), Span::new(11, 12)))
        );
        assert_eq!(iter.next(), None);
    }

    // 05. A first component left unclosed on its own line closes past a single
    //     newline — components are paragraph-bounded, not line-bounded.
    #[test]
    fn test_05_close1_across_single_newline() {
        let src = b"[unclosed(url\n[x](y)";
        let mut iter = ChainedIter::new(src, LINK, b'!', false, b'\n', b'\\', stub_make);

        // close1 for the opener at 0 is the `]` at 16; `(` follows at 17.
        assert_eq!(
            iter.next(),
            Some((false, Span::new(1, 16), Span::new(18, 19)))
        );
        assert_eq!(iter.next(), None);
    }

    // 06. A second component left unclosed on its own line closes past a
    //     single newline too.
    #[test]
    fn test_06_close2_across_single_newline() {
        let src = b"[text](unclosed\n[x](y)";
        let mut iter = ChainedIter::new(src, LINK, b'!', false, b'\n', b'\\', stub_make);

        // The second component opened at 7 closes at the `)` at 21, spanning the
        // embedded newline; the inner `[x](y)` is consumed by it.
        assert_eq!(
            iter.next(),
            Some((false, Span::new(1, 5), Span::new(7, 21)))
        );
        assert_eq!(iter.next(), None);
    }

    // 07. Extracts multiple independent valid chained pairs from a single line sequential stream
    #[test]
    fn test_07_multiple_pairs_on_single_line() {
        let src = b"[a](b)[c](d)";
        let mut iter = ChainedIter::new(src, LINK, b'!', false, b'\n', b'\\', stub_make);

        assert_eq!(iter.next(), Some((false, Span::new(1, 2), Span::new(4, 5))));
        assert_eq!(
            iter.next(),
            Some((false, Span::new(7, 8), Span::new(10, 11)))
        );
        assert_eq!(iter.next(), None);
    }

    // 08. Demonstrates flat parsing logic where an inner open bracket becomes part of the text span
    #[test]
    fn test_08_flat_inner_open_marker() {
        let src = b"[skip [a](b)";
        let mut iter = ChainedIter::new(src, LINK, b'!', false, b'\n', b'\\', stub_make);

        assert_eq!(
            iter.next(),
            Some((false, Span::new(1, 8), Span::new(10, 11)))
        );
        assert_eq!(iter.next(), None);
    }

    // 09. The two components must be byte-adjacent: a newline between close1
    //     and open2 never matches, paragraph bounds notwithstanding
    #[test]
    fn test_09_newline_separating_components_fails() {
        let src = b"[a]\n(b)";
        let mut iter = ChainedIter::new(src, LINK, b'!', false, b'\n', b'\\', stub_make);

        assert_eq!(iter.next(), None);
    }

    // 10. Handles empty inner text and empty inner url segments correctly
    #[test]
    fn test_10_empty_contents() {
        let src = b"[](b)";
        let mut iter = ChainedIter::new(src, LINK, b'!', false, b'\n', b'\\', stub_make);

        assert_eq!(iter.next(), Some((false, Span::new(1, 1), Span::new(3, 4))));
        assert_eq!(iter.next(), None);
    }

    // 11. Treats prefix as false if the prefix character itself is escaped
    #[test]
    fn test_11_escaped_prefix_evaluated_as_false() {
        let src = b"\\![a](b)";
        let mut iter = ChainedIter::new(src, LINK, b'!', false, b'\n', b'\\', stub_make);

        assert_eq!(iter.next(), Some((false, Span::new(3, 4), Span::new(6, 7))));
        assert_eq!(iter.next(), None);
    }

    // 12. Correctly processes independent matching chains across multiple sequential lines
    #[test]
    fn test_12_multiline_processing() {
        let src = b"[a](b)\n![c](d)";
        let mut iter = ChainedIter::new(src, LINK, b'!', false, b'\n', b'\\', stub_make);

        assert_eq!(iter.next(), Some((false, Span::new(1, 2), Span::new(4, 5))));
        assert_eq!(
            iter.next(),
            Some((true, Span::new(9, 10), Span::new(12, 13)))
        );
        assert_eq!(iter.next(), None);
    }

    // 13. Returns None immediately when supplied with a completely empty input slice
    #[test]
    fn test_13_empty_source_slice() {
        let src = b"";
        let mut iter = ChainedIter::new(src, LINK, b'!', false, b'\n', b'\\', stub_make);

        assert_eq!(iter.next(), None);
    }

    // 14. Prevents underflow boundaries and safely sets prefix to false when pattern starts at index 0
    #[test]
    fn test_14_pattern_at_start_index_zero() {
        let src = b"[a](b)";
        let mut iter = ChainedIter::new(src, LINK, b'!', false, b'\n', b'\\', stub_make);

        assert_eq!(iter.next(), Some((false, Span::new(1, 2), Span::new(4, 5))));
        assert_eq!(iter.next(), None);
    }

    // 15. Ignores trailing garbage text on the line after successfully extracting a valid pattern match
    #[test]
    fn test_15_trailing_garbage_ignored() {
        let src = b"[a](b)random_garbage_text";
        let mut iter = ChainedIter::new(src, LINK, b'!', false, b'\n', b'\\', stub_make);

        assert_eq!(iter.next(), Some((false, Span::new(1, 2), Span::new(4, 5))));
        assert_eq!(iter.next(), None);
    }

    // ---- Paragraph-bounded behaviour ------------------------------------ //

    // 16. An empty line aborts an unclosed first component: components never
    //     pair across paragraphs
    #[test]
    fn test_16_empty_line_aborts_component() {
        let src = b"[open\n\nclosed](url)";
        let mut iter = ChainedIter::new(src, LINK, b'!', false, b'\n', b'\\', stub_make);

        assert_eq!(iter.next(), None);
    }

    // 17. A whole pair spanning a single newline inside one paragraph matches
    #[test]
    fn test_17_pair_spans_single_newline() {
        let src = b"[multi\nline](url)";
        let mut iter = ChainedIter::new(src, LINK, b'!', false, b'\n', b'\\', stub_make);

        assert_eq!(
            iter.next(),
            Some((false, Span::new(1, 11), Span::new(13, 16)))
        );
        assert_eq!(iter.next(), None);
    }

    fn links(src: &[u8]) -> Vec<(u32, u32, u32, u32)> {
        ChainedIter::new(src, LINK, b'!', false, b'\n', b'\\', stub_make)
            .map(|(_, t, u)| (t.start, t.end, u.start, u.end))
            .collect()
    }

    // 18. What a failed search learns holds only where it holds: up to the
    //     end of the paragraph for an absent close byte, up to the close it
    //     found for a match that failed after it
    #[test]
    fn test_18_failed_search_windows() {
        // No first close up to the empty line, then a new paragraph.
        assert_eq!(links(b"[a [b\n\n[c](d)"), vec![(8, 9, 11, 12)]);
        // A match failed after `]`; an opener past that close searches anew.
        assert_eq!(links(b"[a] [b](c)"), vec![(5, 6, 8, 9)]);
        // The first opener's close serves the second one too.
        assert!(links(b"[a [b] c").is_empty());
        // No second close up to the empty line, then a new paragraph.
        assert_eq!(links(b"[a](b\n\n[c](d)"), vec![(8, 9, 11, 12)]);
        assert!(links(b"[a](b [c](d").is_empty());
    }

    // 19. A long run of unclosed openers is read once
    #[test]
    fn test_19_unclosed_openers() {
        let mut src = b"[a ".repeat(20_000);
        src.extend_from_slice(b"\n\n[b](c)");
        assert_eq!(links(&src), vec![(60_003, 60_004, 60_006, 60_007)]);
        let mut src = b"[a](".repeat(20_000);
        src.extend_from_slice(b"\n\n[b](c)");
        assert_eq!(links(&src), vec![(80_003, 80_004, 80_006, 80_007)]);
    }

    // ---- The rule read as the full parse reads it ----------------------- //

    type L = (bool, (u32, u32), (u32, u32));

    fn read(src: &[u8], balanced: bool, transparent: bool) -> Vec<L> {
        let spec = ChainSpec {
            first_balanced: balanced,
            second_balanced: balanced,
            ..LINK
        };
        ChainedIter::new(src, spec, b'!', transparent, b'\n', b'\\', |p, t, u| {
            (p, (t.start, t.end), (u.start, u.end))
        })
        .collect()
    }

    /// Every combination of `balanced` and `transparent` gives `want`.
    fn all(src: &[u8], want: &[L]) {
        for (b, t) in [(false, false), (true, false), (false, true), (true, true)] {
            assert_eq!(
                read(src, b, t),
                want,
                "{:?} balanced {b} transparent {t}",
                src
            );
        }
    }

    // 20. Of a run of the open byte the last one opens, and the prefix is the
    //     byte before it
    #[test]
    fn test_20_last_byte_of_open_run() {
        all(b"[[a](u)", &[(false, (2, 3), (5, 6))]);
        all(b"![[a](u)", &[(false, (3, 4), (6, 7))]);
        all(b"!![a](u)", &[(true, (3, 4), (6, 7))]);
        all(b"\\![a](u)", &[(false, (3, 4), (6, 7))]);
    }

    // 21. An escaped close byte closes nothing
    #[test]
    fn test_21_escaped_closes() {
        all(b"[a\\](u)", &[]);
        all(b"[a\\]b](u)", &[(false, (1, 5), (7, 8))]);
        all(b"[a](u\\)v)", &[(false, (1, 2), (4, 8))]);
    }

    // 22. A balanced component nests on its open byte
    #[test]
    fn test_22_balanced_components() {
        for t in [false, true] {
            assert_eq!(
                read(b"[a [b] c](u)", true, t),
                vec![(false, (1, 8), (10, 11))]
            );
            assert_eq!(read(b"[a [b] c](u)", false, t), vec![]);
            assert_eq!(read(b"[a](u(v)w)", true, t), vec![(false, (1, 2), (4, 9))]);
            assert_eq!(read(b"[a](u(v)w)", false, t), vec![(false, (1, 2), (4, 7))]);
            assert_eq!(read(b"[a](((u)", true, t), vec![]);
        }
    }

    // 23. A failed opaque match resumes right after its open run; a
    //     transparent one never returns to the openers inside it
    #[test]
    fn test_23_failure_recovery() {
        assert_eq!(
            read(b"[a [b](u)", true, false),
            vec![(false, (4, 5), (7, 8))]
        );
        assert_eq!(read(b"[a [b](u)", true, true), vec![]);
        assert_eq!(
            read(b"[a [b](u)", false, false),
            vec![(false, (1, 5), (7, 8))]
        );
        assert_eq!(
            read(b"[a [b](u)", false, true),
            vec![(false, (1, 5), (7, 8))]
        );
        all(b"[a] [b](u)", &[(false, (5, 6), (8, 9))]);
        all(b"[a]]](u)", &[]);
        all(b"[a\n\nb](u) [c](d)", &[(false, (11, 12), (14, 15))]);
    }

    fn quoted(src: &[u8], balanced: bool, transparent: bool) -> Vec<L> {
        let spec = ChainSpec {
            first: (b'"', b'"'),
            first_balanced: balanced,
            ..LINK
        };
        ChainedIter::new(src, spec, b'!', transparent, b'\n', b'\\', |p, t, u| {
            (p, (t.start, t.end), (u.start, u.end))
        })
        .collect()
    }

    // 24. A component whose open and close are one byte closes at the next
    //     run when unbalanced, and never when balanced
    #[test]
    fn test_24_one_byte_component() {
        for t in [false, true] {
            assert_eq!(quoted(b"\"a\"(u)", false, t), vec![(false, (1, 2), (4, 5))]);
            assert_eq!(quoted(b"\"a\"(u)", true, t), vec![]);
            assert_eq!(quoted(b"\"a\"\"(u)", false, t), vec![]);
            assert_eq!(
                quoted(b"\"\"a\"(u)", false, t),
                vec![(false, (2, 3), (5, 6))]
            );
        }
    }

    // 25. A transparent match is read in one pass, however many openers
    #[test]
    fn test_25_transparent_linear() {
        let mut src = b"[a ".repeat(20_000);
        src.extend_from_slice(b"](u)");
        assert_eq!(
            read(&src, false, true),
            vec![(false, (1, 60_000), (60_002, 60_003))]
        );
        assert_eq!(read(&src, true, true), vec![]);
    }

    // 26. The rest of a second close run is read again, so a url that closes on
    //     the first open byte leaves the next byte of its run to open a match
    #[test]
    fn test_26_second_close_run_read_again() {
        let spec = ChainSpec {
            second: (b'(', b'['),
            ..LINK
        };
        for t in [false, true] {
            let got: Vec<L> =
                ChainedIter::new(b"[a](u[[b](c[", spec, b'!', t, b'\n', b'\\', |p, x, u| {
                    (p, (x.start, x.end), (u.start, u.end))
                })
                .collect();
            assert_eq!(
                got,
                vec![(false, (1, 2), (4, 5)), (false, (7, 8), (10, 11))]
            );
        }
    }

    // ---- The generated methods ------------------------------------------ //

    /// The `find_*` and `context()` methods `define_parser!` generates for a
    /// `chained` rule. The expansion inside this crate trips lints that a
    /// dependent crate never sees.
    #[allow(unused)]
    mod generated {
        use crate::span::Span;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct Link {
            pub is_image: bool,
            pub text: Span,
            pub url: Span,
        }

        type L = (bool, (u32, u32), (u32, u32));

        fn link(l: &Link) -> L {
            (
                l.is_image,
                (l.text.start, l.text.end),
                (l.url.start, l.url.end),
            )
        }

        fn regions(ctx: &crate::ParseContext) -> Vec<(u32, u32)> {
            ctx.spans().iter().map(|s| (s.start, s.end)).collect()
        }

        macro_rules! links {
            ($m:ident, $p:ident, $trig:tt, $tpi:tt, $tbal:tt, $upi:tt, $ubal:tt) => {
                mod $m {
                    use super::Link;
                    crate::define_parser!($p {
                        sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\', max_nest = 4;
                        inline {
                            on_trigger $trig {
                                chained: Link {
                                    | b'[', b']' | { parse_inside = $tpi; balanced = $tbal; } => text,
                                    | b'(', b')' | { parse_inside = $upi; balanced = $ubal; } => url,
                                    prefix | b'!' | => is_image,
                                } => links [50]
                            }
                            fallback => texts [10];
                        }
                        blocks { fallback => paragraphs [80]; }
                    });
                }
            };
        }

        links!(opaque, Opaque, (b'['), false, true, false, false);
        links!(
            mixed,
            Mixed,
            (b'[', b']', b'(', b')'),
            true,
            false,
            false,
            false
        );
        links!(
            nested,
            Nested,
            (b'[', b']', b'(', b')'),
            true,
            true,
            true,
            true
        );

        use mixed::MixedParser;
        use nested::NestedParser;
        use opaque::OpaqueParser;

        // 27. An opaque rule's matches are context regions, from the open
        //     byte through the second close, each component with its own
        //     `balanced`
        #[test]
        fn test_27_generated_opaque_rule_is_context_source() {
            for src in [
                &b"[a [b] c](d (e) f)"[..],
                b"x ![a](b) [c [d]](e) [f](g",
                b"[a\nb](c)\n\n[d](e\n\nf)",
            ] {
                let full = OpaqueParser::parse(src);
                let want: Vec<(u32, u32)> = full
                    .links
                    .iter()
                    .map(|l| (l.text.start - 1, l.url.end + 1))
                    .collect();
                assert_eq!(regions(&OpaqueParser::context(src)), want);
            }
            // The first component nests, the second component does not.
            assert_eq!(
                regions(&OpaqueParser::context(b"[a [b] c](d (e) f)")),
                vec![(0, 15)]
            );
        }

        // 28. A rule with a transparent component is no context source
        #[test]
        fn test_28_generated_transparent_rule_is_no_context_source() {
            let src = b"[a](b)";
            assert_eq!(MixedParser::parse(src).links.len(), 1);
            assert!(MixedParser::context(src).spans().is_empty());
        }

        // 29. The finder gets the rule's flags: a failed opaque match resumes
        //     after its open run, a transparent one never returns to the
        //     openers inside it
        #[test]
        fn test_29_generated_finder_follows_the_rule() {
            for src in [
                &b"[a [b](u)"[..],
                b"[a [b] c](u)",
                b"[a](u(v)w)",
                b"![[a](u)",
                b"[a\\]b](u)",
            ] {
                let full: Vec<L> = OpaqueParser::parse(src).links.iter().map(link).collect();
                let find: Vec<L> = OpaqueParser::find_links(src).map(|l| link(&l)).collect();
                assert_eq!(find, full, "{:?}", String::from_utf8_lossy(src));
                let full: Vec<L> = NestedParser::parse(src).links.iter().map(link).collect();
                let find: Vec<L> = NestedParser::find_links(src).map(|l| link(&l)).collect();
                assert_eq!(find, full, "{:?}", String::from_utf8_lossy(src));
            }
            let find: Vec<L> = OpaqueParser::find_links(b"[a [b](u)")
                .map(|l| link(&l))
                .collect();
            assert_eq!(find, vec![(false, (4, 5), (7, 8))]);
            assert_eq!(NestedParser::find_links(b"[a [b](u)").count(), 0);
        }

        mod marked {
            use super::Link;
            crate::define_parser!(Marked {
                sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\', max_nest = 2;
                inline {
                    on_trigger(b'[') {
                        chained: Link {
                            | b'[', b']' | { parse_inside = false; balanced = false; } => text,
                            | b'(', b')' | { parse_inside = false; balanced = false; } => url,
                            prefix | b'!' | => is_image,
                        } => links [50]
                    }
                    fallback => texts [10];
                }
                blocks { block_simple { cont(b'!') => bangs [40]; } fallback => paragraphs [80]; }
            });
        }

        mod marked_tr {
            use super::Link;
            crate::define_parser!(MarkedTr {
                sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\', max_nest = 2;
                inline {
                    on_trigger(b'[', b']', b'(', b')') {
                        chained: Link {
                            | b'[', b']' | { parse_inside = true; balanced = false; } => text,
                            | b'(', b')' | { parse_inside = false; balanced = false; } => url,
                            prefix | b'!' | => is_image,
                        } => links [50]
                    }
                    fallback => texts [10];
                }
                blocks { block_simple { cont(b'!') => bangs [40]; } fallback => paragraphs [80]; }
            });
        }

        // 30. The prefix is read inside the inline run only: a `!` that is a
        //     block marker before the run makes no image, for an opaque rule
        //     and a transparent one alike
        #[test]
        fn test_30_generated_prefix_inside_the_run() {
            use marked::MarkedParser;
            use marked_tr::MarkedTrParser;
            let opaque = |src: &[u8]| -> Vec<L> {
                MarkedParser::parse(src).links.iter().map(link).collect()
            };
            let transparent = |src: &[u8]| -> Vec<L> {
                MarkedTrParser::parse(src).links.iter().map(link).collect()
            };
            for links in [&opaque as &dyn Fn(&[u8]) -> Vec<L>, &transparent] {
                assert_eq!(links(b"![a](u)"), vec![(false, (2, 3), (5, 6))]);
                assert_eq!(links(b"!x![a](u)"), vec![(true, (4, 5), (7, 8))]);
                assert_eq!(links(b"x ![a](u)"), vec![(true, (4, 5), (7, 8))]);
            }
        }
    }
}
