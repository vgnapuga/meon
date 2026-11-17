//! [`ParseContext`] — the opaque-region map consumed by the `find_context_*`
//! standalone iterators — together with [`LineModel`], the grammar's line
//! rules the map is read against, and [`ContextCursor`], its lookup cursor.
//!
//! # Purpose
//!
//! Context-free standalone iterators (`find_*`) are documented to diverge from
//! the full parse: they match delimiters inside fenced blocks and inside
//! opaque inline constructs (code spans, strings, autolinks, links — any rule
//! with `parse_inside = false`), because they carry no cross-element state.
//!
//! A [`ParseContext`] closes most of that gap. It is built by **one**
//! streaming pass over the source — a single unified needle search per
//! iteration (fence bytes and opaque-rule triggers share one deduplicated
//! set, dispatched to `memchr`/`memchr2`/`memchr3` or, beyond three distinct
//! bytes, to the SWAR [`crate::swar::find_any`]) — resolving every opaque
//! construct with the same leftmost-wins semantics the full parser uses.
//!
//! # Lines
//!
//! Where a construct may begin and end depends on the line it sits on, so the
//! pass reads every line it lands on the way `parse_text!` does, through the
//! grammar's [`LineModel`] — its `block_simple`, `block` and `lines` rules.
//! Outside an open fence, a line is one of:
//!
//! - a **fence line**: a `fence` rule opens at the line start, or right after
//!   the line's `cont` markers while `max_nest` leaves room for one more
//!   frame;
//! - a **block line**: it opens or continues `cont` frames (and possibly a
//!   `block` item after them), opens a top-level `block` item, or matches a
//!   `lines` rule. Its inline content is the rest of the line after the
//!   markers, scanned on its own — unless it holds `cont` frames alone: such
//!   lines of the same depth, one after another, are one inline run read
//!   past their markers, which ends at a line of another kind or depth, a
//!   line whose content is `sep` and `tab` bytes alone, or the end of input;
//! - a **paragraph line**: no rule claims it. Consecutive paragraph lines are
//!   one inline run, which ends at an empty line (two consecutive `eol`
//!   bytes, or a line of `sep` and `tab` bytes alone), at a line that is not
//!   a paragraph line, or at the end of input.
//!
//! A hit with a byte no marker consists of before it on its line is content,
//! known without finding the line: only a hit preceded by marker bytes alone,
//! a fence run, or an opener whose close search reaches its line end has its
//! line found and classified, and the pass keeps that line while later hits
//! fall on it. A line a close search enters is told a paragraph line by its
//! first byte when no rule begins with it, and a grammar without line rules
//! skips all of this.
//!
//! # Regions
//!
//! 1. A hit at the fence run of a fence line opens a fenced region that
//!    covers the run through its close line: a later line whose content after
//!    the markers of the `cont` frames holding the fence is a run of at least
//!    the opening count, followed by `sep`/`tab` only. A fence inside `cont`
//!    frames also ends at the start of the first non-blank line that lacks
//!    one of their markers; a blank line never ends a fence. An unclosed
//!    fence covers to the end of input.
//! 2. Any other hit is an **opaque inline rule** trigger, matched inside the
//!    inline run that holds it, escape-aware throughout, exactly-count on
//!    both sides for symmetric rules. A `chained` rule whose components are
//!    both opaque opens at the last byte of a run of its open byte; the text
//!    close, the second open right after it and the second close must all lie in
//!    the run, and the match covers the open byte through the second close. An
//!    `asymmetric` rule with `balanced = true` opens a ring at the first
//!    byte of its open run, which every other byte of its own nests in, as
//!    the full parse does; its match covers that byte through the close of
//!    the ring, and when the ring never closes the next byte tries. A hit
//!    among a line's markers is not a trigger. A
//!    matched construct covers its full extent, delimiters included, and the
//!    scan resumes after it — so overlapping opaque candidates resolve
//!    leftmost-first, like the full parser's trigger loop. A `chained` match
//!    that fails resumes the scan right after its open run, so its text is
//!    scanned like any other.
//!
//! The result is a sorted, non-overlapping `Vec<Span>` of opaque regions,
//! preallocated from the grammar's own capacity divisors (the same `[cap]`
//! hints that size the content vectors), kept together with the
//! [`LineModel`], so the `find_context_*` iterators read lines the same way:
//! inline finders skip candidates inside a region or among a line's markers
//! and keep their close search inside the opener's inline run; line-family
//! finders keep an item only when [`ContextCursor::admits`] its line, and the
//! `cont` finder a frame whose marker no region covers. Note the
//! semantics: the context suppresses **trigger positions**, not enclosing
//! spans — a transparent element's span may still legally *contain* an opaque
//! region, exactly as in the full parse.
//!
//! # Remaining divergence from the full parse (by design)
//!
//! - Line-family `find_context_*` finders filter what their context-free
//!   iterator yields and never add to it: a `block` item after `cont` markers
//!   (`> - item`) is not found. `cont` frames are the context-free
//!   iterator's, kept by the position of their marker: a frame whose marker
//!   sits inside a fence is dropped whole, even when its run goes on past the
//!   fence.
//! - A `chained` rule with a transparent component is no context source. The
//!   full parse makes an opaque component of its match opaque while the match
//!   is in it — up to the end of the inline run when that component never
//!   closes — and those bytes are not covered here. In a grammar that also
//!   has an opaque `chained` rule, an opaque match the full parse never
//!   reaches inside such a match is still covered.
//! - An `asymmetric` finder sees its own rule's frames only. In the full
//!   parse the frames of every stack-kept rule, `key_value` values among
//!   them, share one stack: they all count towards `max_nest`, and a close
//!   whose innermost frame belongs to another rule closes nothing, unless
//!   that frame is a `key_value` value inside the container the close ends.
//!   The finder reads the full parse's spans where the rules nest properly —
//!   every other frame opened inside one of the rule's frames is closed by
//!   that frame's close, or is such a value — and all the frames fit in
//!   `max_nest`.

use super::asymmetric::Rings;
use super::chained::{ChainMemo, chain_match};
use super::common::{
    LineBreaks, Span, count_escape, find_line_end, next_in_paragraph, scan_paragraph,
};

/// One opaque symmetric rule: `(delimiter byte, exact run count)`.
pub type SymSpec = (u8, u32);
/// One opaque asymmetric rule: `(open byte, close byte, exact open count,
/// balanced)`. Every byte of the open run of a `balanced` rule is an open of
/// its own, and its count is `1`.
pub type AsymSpec = (u8, u8, u32, bool);

/// The two components of a `chained` rule: their open and close bytes and
/// their `balanced` flags. The context takes the rules whose components are
/// both `parse_inside = false`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChainSpec {
    /// The first component's open and close bytes.
    pub first: (u8, u8),
    /// The first component's `balanced` flag: each unescaped open byte nests
    /// one level.
    pub first_balanced: bool,
    /// The second component's open and close bytes; the open byte must
    /// follow the first component's close.
    pub second: (u8, u8),
    /// The second component's `balanced` flag.
    pub second_balanced: bool,
}

/// Most distinct needle bytes one search instruction takes; a wider set is
/// searched through a byte table.
const MAX_NEEDLES: usize = 11;

/// One `block_simple` rule of a [`LineModel`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockSimpleRule {
    /// `fence(pattern, min = N)`: a run of at least `min` of one byte out of
    /// `bytes`, with no further such byte on the line, opens a fenced block,
    /// which a run of that same byte closes.
    Fence {
        /// The bytes a fence may consist of.
        bytes: ByteSet,
        /// The shortest opening run.
        min: u32,
    },
    /// `cont(byte)`: a `byte` marker, optionally followed by one `sep`/`tab`,
    /// opens or continues a continuation frame.
    Cont {
        /// The marker byte.
        byte: u8,
    },
}

/// One `block` rule of a [`LineModel`]: a leaf item that may follow
/// `sep`/`tab` indentation and is followed by one `sep` or `tab`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockRule {
    /// `(b1 | b2 | ...)`: one marker byte out of `bytes`.
    Marker {
        /// The marker bytes.
        bytes: ByteSet,
    },
    /// `num(digits, end = ends)`: one to nine bytes out of `digits`, then
    /// one byte out of `ends`.
    Num {
        /// The bytes the number may consist of.
        digits: ByteSet,
        /// The bytes that may end the number.
        ends: ByteSet,
    },
}

/// One `lines` rule of a [`LineModel`], matched at the line start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineRule {
    /// `line(byte, max = N)`: one to `max` `byte`s, then `sep` or the line
    /// end.
    Marker {
        /// The marker byte.
        byte: u8,
        /// The longest marker run.
        max: u8,
    },
    /// `line_simple(b1 | b2 | ..., min = N)`: the whole line is one byte out
    /// of `bytes`, repeated at least `min` times and interleaved with `sep`
    /// only.
    Uniform {
        /// The bytes the line may repeat.
        bytes: ByteSet,
        /// The fewest repetitions.
        min: u32,
    },
}

/// The line rules of a grammar — what `parse_text!` tries at the start of
/// every line — each section in declaration order.
///
/// The generated `Parser::context` fills it from the grammar, and the
/// [`ParseContext`] keeps it, so the `find_context_*` iterators read lines
/// the way the context builder did. The default model has no rules: every
/// line is a paragraph line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineModel {
    /// Line terminator byte.
    pub eol: u8,
    /// Separator byte.
    pub sep: u8,
    /// Tab byte.
    pub tab: u8,
    /// Is a `\r` right before `eol` part of the line end (the grammar's
    /// `crlf`)?
    pub crlf: bool,
    /// The grammar's `max_nest`: the most frames — `cont` frames and a fence
    /// — one line may hold.
    pub max_nest: usize,
    /// The `block_simple` rules (`fence`, `cont`).
    pub block_simple: &'static [BlockSimpleRule],
    /// The `block` rules (marker and `num` items).
    pub block: &'static [BlockRule],
    /// The `lines` rules (`line`, `line_simple`).
    pub lines: &'static [LineRule],
}

impl Default for LineModel {
    fn default() -> Self {
        Self {
            eol: b'\n',
            sep: b' ',
            tab: b'\t',
            crlf: false,
            max_nest: 1,
            block_simple: &[],
            block: &[],
            lines: &[],
        }
    }
}

/// The rule a line-family `find_context_*` finder matches, by its index in
/// declaration order within its section, for [`ContextCursor::admits`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleIndex {
    /// The n-th `block` rule (a marker or `num` item).
    Block(usize),
    /// The n-th `lines` rule (`line` or `line_simple`).
    Line(usize),
}

/// How the full parse reads one line that no open fence swallows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineKind {
    /// A fence of `count` `byte`s opens at `at`, inside `depth` `cont`
    /// frames.
    Fence {
        at: usize,
        byte: u8,
        count: u32,
        depth: usize,
    },
    /// `cont` markers open or continue `depth` frames; inline content
    /// starts at `cs`, past a `block` item's marker when `item`.
    Cont { cs: usize, depth: usize, item: bool },
    /// The `rule`-th `block` rule opens a top-level item; inline content
    /// starts at `cs`.
    Block { cs: usize, rule: usize },
    /// The `rule`-th `lines` rule matches; inline content starts at `cs`,
    /// the line end when the rule consumes the whole line.
    Line { cs: usize, rule: usize },
    /// No rule claims the line: it belongs to a multi-line paragraph run.
    Para,
}

/// A classified line `[ls, le)`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LineSlot {
    pub(crate) ls: usize,
    pub(crate) le: usize,
    pub(crate) kind: LineKind,
}

impl LineSlot {
    /// Where the line's inline content starts; the bytes before it are
    /// markers. A fence line has none: the info line is consumed whole.
    #[inline]
    pub(crate) fn content_start(&self) -> usize {
        match self.kind {
            LineKind::Fence { .. } => self.le,
            LineKind::Cont { cs, .. } | LineKind::Block { cs, .. } | LineKind::Line { cs, .. } => {
                cs
            }
            LineKind::Para => self.ls,
        }
    }
}

/// A set of byte values: the bytes a grammar's byte pattern matches.
///
/// The [`LineModel`] rules hold their patterns this way, so a lookup is one
/// bit test whatever the pattern's shape. A `const` set comes from
/// [`ByteSet::from_bytes`], or from a pattern through the hidden `byte_set!`
/// macro the generated `Parser::context` uses.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct ByteSet([u64; 4]);

impl ByteSet {
    /// The set of no byte.
    pub const EMPTY: Self = Self([0; 4]);

    /// The set of the bytes in `bytes`.
    pub const fn from_bytes(bytes: &[u8]) -> Self {
        let mut set = Self::EMPTY;
        let mut i = 0;
        while i < bytes.len() {
            set = set.with(bytes[i]);
            i += 1;
        }
        set
    }

    /// The set with `b` added.
    #[inline]
    pub const fn with(self, b: u8) -> Self {
        let mut words = self.0;
        words[(b >> 6) as usize] |= 1 << (b & 63);
        Self(words)
    }

    /// Is `b` in the set?
    #[inline(always)]
    pub const fn contains(&self, b: u8) -> bool {
        (self.0[(b >> 6) as usize] >> (b & 63)) & 1 == 1
    }

    /// The bytes in the set, in ascending order.
    pub fn iter(self) -> impl Iterator<Item = u8> {
        (0..=255u8).filter(move |&b| self.contains(b))
    }

    #[inline]
    fn insert(&mut self, b: u8) {
        *self = self.with(b);
    }

    #[inline]
    fn union(&mut self, other: Self) {
        for (w, o) in self.0.iter_mut().zip(other.0) {
            *w |= o;
        }
    }
}

impl core::fmt::Debug for ByteSet {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}

/// Classes of bytes a [`LineModel`]'s rules are made of, computed once per
/// context.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ByteClasses {
    /// What a line's markers may consist of: `sep`, `tab` and every byte a
    /// `cont`, `block` or `lines` rule consumes before the content (a fence
    /// run is no marker before content).
    marks: ByteSet,
    /// What a line some rule claims may begin with: `sep` and `tab` (a
    /// `block` item may be indented) and every rule's first byte.
    starts: ByteSet,
    /// The fence bytes.
    fences: ByteSet,
    /// The first bytes of the `block_simple` rules.
    simple: ByteSet,
    /// What a line a `block_simple` or `block` rule claims may begin with.
    blocks: ByteSet,
}

/// Reads the lines a forward scan's hits fall on, through a [`LineModel`].
///
/// Hits must come in increasing order. A hit that follows a content byte on
/// its line is content, which is known without finding the line: the walk
/// back from the hit stops at the first byte no marker consists of. Only a
/// hit preceded on its line by marker bytes alone has its line classified.
/// The walk remembers the run of marker bytes it crossed, so no byte is
/// walked twice.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LineReader<'m> {
    lines: &'m LineModel,
    rules: bool,
    classes: &'m ByteClasses,
    /// `[lo, hi)` holds marker bytes only; `lo` starts a line when the flag
    /// is set, and follows a content byte otherwise.
    run: (usize, usize, bool),
    /// The last classified line.
    slot: Option<LineSlot>,
}

impl<'m> LineReader<'m> {
    fn new(lines: &'m LineModel, classes: &'m ByteClasses) -> Self {
        Self {
            lines,
            rules: lines.has_rules(),
            classes,
            run: (0, 0, true),
            slot: None,
        }
    }

    /// Is `p` content because a content byte comes right before it, or two
    /// bytes before it with a marker byte in between? The common case of a
    /// delimiter inside a line, told without a walk.
    #[inline(always)]
    fn after_content(&self, src: &[u8], p: usize) -> bool {
        let eol = self.lines.eol;
        let content = |b: u8| b != eol && !self.classes.marks.contains(b);
        p > 0 && (content(src[p - 1]) || (p > 1 && src[p - 1] != eol && content(src[p - 2])))
    }

    /// May a hit on `b` fall among a line's markers? Markers consist of
    /// such bytes only; a fence's info line, the one other place
    /// [`Self::in_markers`] reports, lies inside a fenced region.
    #[inline]
    pub(crate) fn marks(&self, b: u8) -> bool {
        self.classes.marks.contains(b)
    }

    /// Is `p` among the markers of its line, or on a fence's info line?
    #[inline(always)]
    pub(crate) fn in_markers(&mut self, src: &[u8], p: usize) -> bool {
        self.rules
            && !self.after_content(src, p)
            && self.walk(src, p).is_some_and(|l| p < l.content_start())
    }

    /// The classified line holding `p` when only marker bytes come before
    /// `p` on it, or when that line was the last one classified — `p` may
    /// then be a marker, a fence run or content — and `None` when `p`
    /// follows a content byte and is content itself.
    #[inline(always)]
    pub(crate) fn lead(&mut self, src: &[u8], p: usize) -> Option<LineSlot> {
        if !self.rules || self.after_content(src, p) {
            return None;
        }
        self.walk(src, p)
    }

    /// [`Self::lead`] past its quick test: the walk over marker bytes.
    #[inline(never)]
    fn walk(&mut self, src: &[u8], p: usize) -> Option<LineSlot> {
        if let Some(s) = self.slot {
            if s.ls <= p && p <= s.le {
                return Some(s);
            }
        }
        let (lo, hi, from_start) = self.run;
        let eol = self.lines.eol;
        let mut k = p;
        let at_start = loop {
            if k == hi {
                k = lo;
                break from_start;
            }
            if k == 0 || src[k - 1] == eol {
                break true;
            }
            if !self.classes.marks.contains(src[k - 1]) {
                break false;
            }
            k -= 1;
        };
        self.run = (k, p, at_start);
        at_start.then(|| self.read(src, k, p))
    }

    /// Classify the line starting at `ls` that holds `p`, and keep it.
    #[inline(never)]
    fn read(&mut self, src: &[u8], ls: usize, p: usize) -> LineSlot {
        let le = find_line_end(src, p, self.lines.eol);
        let kind = if ls < le && !self.classes.starts.contains(src[ls]) {
            LineKind::Para
        } else {
            self.lines
                .classify(src, ls, self.lines.content_end(src, ls, le))
        };
        let line = LineSlot { ls, le, kind };
        self.slot = Some(line);
        line
    }

    /// Where the close search for an opener at `p` goes on in the line
    /// starting at `ls`, if it goes on there: past the line start of a
    /// paragraph line from a paragraph line, past the markers of a line of
    /// the same `cont` frames alone from such a line, whose content is not
    /// `sep` and `tab` bytes alone. The caller has told the line is not
    /// blank. `run` keeps the opener line's reading across the calls of one
    /// search.
    #[inline(never)]
    pub(crate) fn crosses(
        &mut self,
        src: &[u8],
        p: usize,
        ls: usize,
        run: &mut Option<Run>,
    ) -> Option<usize> {
        match *run.get_or_insert_with(|| self.run_at(src, p)) {
            Run::Para => self.is_para(src, ls).then_some(ls),
            Run::Cont {
                depth: d,
                ls: ols,
                le: ole,
            } => {
                if ls >= src.len() || !self.classes.simple.contains(src[ls]) {
                    return None;
                }
                let le = find_line_end(src, ls, self.lines.eol);
                let le = self.lines.content_end(src, ls, le);
                match self.lines.classify(src, ls, le) {
                    LineKind::Cont {
                        cs,
                        depth,
                        item: false,
                    } if depth == d
                        && self.lines.same_markers(src, (ols, ole), (ls, le), d)
                        && !self.lines.blank(src, cs, le) =>
                    {
                        Some(cs)
                    }
                    _ => None,
                }
            }
            Run::Line => None,
        }
    }

    /// The inline run the line holding `p` is in.
    fn run_at(&mut self, src: &[u8], p: usize) -> Run {
        if !self.rules {
            return Run::Para;
        }
        let line = match self.slot {
            Some(s) if s.ls <= p && p <= s.le => s,
            _ => {
                let ls = memchr::memrchr(self.lines.eol, &src[..p]).map_or(0, |i| i + 1);
                self.read(src, ls, p)
            }
        };
        match line.kind {
            LineKind::Para => Run::Para,
            LineKind::Cont {
                depth, item: false, ..
            } => Run::Cont {
                depth,
                ls: line.ls,
                le: line.le,
            },
            _ => Run::Line,
        }
    }

    /// Is the line holding `p` a paragraph line?
    #[cfg(test)]
    fn para_at(&mut self, src: &[u8], p: usize) -> bool {
        self.run_at(src, p) == Run::Para
    }

    /// Is the line starting at `ls` a paragraph line? A line whose first
    /// byte begins no rule is one without further reading. The caller tells
    /// a blank line first (see [`LineBreaks::ends_paragraph`]).
    fn is_para(&self, src: &[u8], ls: usize) -> bool {
        if ls >= src.len() {
            return true;
        }
        let b = src[ls];
        let le = find_line_end(src, ls, self.lines.eol);
        !self.rules
            || !self.classes.starts.contains(b)
            || self
                .lines
                .classify(src, ls, self.lines.content_end(src, ls, le))
                == LineKind::Para
    }
}

/// The inline run of a line, as far as the lines after it go: a paragraph
/// run, a run of the `depth` `cont` frames alone whose markers lead the line
/// `[ls, le)`, or the line's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Run {
    Para,
    Cont { depth: usize, ls: usize, le: usize },
    Line,
}

impl LineModel {
    /// How the model's lines end, for the paragraph-bounded scans.
    #[inline]
    pub const fn breaks(&self) -> LineBreaks {
        LineBreaks::new(self.eol, self.crlf, &[self.sep, self.tab])
    }

    /// Where the content of the line `[ls, le)`, `le` its `eol` or the end
    /// of input, ends: before a CR followed by an actual `eol` with
    /// `crlf`. A CR at EOF remains content.
    #[inline]
    pub(crate) fn content_end(&self, src: &[u8], ls: usize, le: usize) -> usize {
        if self.crlf && le < src.len() && src[le] == self.eol && le > ls && src[le - 1] == b'\r' {
            le - 1
        } else {
            le
        }
    }

    /// Do the lines `a` and `b`, as `(start, end)`, each led by `depth`
    /// `cont` markers, have the same marker bytes, outermost first?
    fn same_markers(&self, src: &[u8], a: (usize, usize), b: (usize, usize), depth: usize) -> bool {
        let (mut o, mut c) = (a.0, b.0);
        for _ in 0..depth {
            if o >= a.1 || c >= b.1 || src[o] != src[c] {
                return false;
            }
            o = self.past_marker(src, o, a.1);
            c = self.past_marker(src, c, b.1);
        }
        true
    }

    /// Is `src[from..to]` `sep` and `tab` bytes alone?
    #[inline]
    pub(crate) fn blank(&self, src: &[u8], from: usize, to: usize) -> bool {
        src[from..to]
            .iter()
            .all(|&b| b == self.sep || b == self.tab)
    }

    /// Position just past a marker at `p`: the marker byte plus one optional
    /// `sep`/`tab`.
    #[inline]
    fn past_marker(&self, src: &[u8], p: usize, le: usize) -> usize {
        if p + 1 < le && (src[p + 1] == self.sep || src[p + 1] == self.tab) {
            p + 2
        } else {
            p + 1
        }
    }

    /// Classify the line `[ls, le)` as `parse_text!` reads it when no fence
    /// is open over it.
    ///
    /// Mirrors `parse_block!`'s open phase — the `block_simple` rules in
    /// order, nesting while `max_nest` allows, then one `block` item — and,
    /// for a line that holds no frame, `parse_line!`. Frames already open
    /// peel the same `cont` markers the open phase would consume, so the
    /// result does not depend on earlier lines.
    pub(crate) fn classify(&self, src: &[u8], ls: usize, le: usize) -> LineKind {
        let mut cur = ls;
        let mut depth = 0usize;
        'open: while depth < self.max_nest {
            for rule in self.block_simple {
                match *rule {
                    BlockSimpleRule::Fence { bytes, min } => {
                        if let Some((byte, count)) = fence_run(src, cur, le, bytes, min) {
                            return LineKind::Fence {
                                at: cur,
                                byte,
                                count,
                                depth,
                            };
                        }
                    }
                    BlockSimpleRule::Cont { byte } => {
                        if cur < le && src[cur] == byte {
                            depth += 1;
                            cur = self.past_marker(src, cur, le);
                            continue 'open;
                        }
                    }
                }
            }
            break;
        }
        if depth < self.max_nest {
            if let Some((cs, rule)) = self.block_item(src, cur, le) {
                return if depth > 0 {
                    LineKind::Cont {
                        cs,
                        depth,
                        item: true,
                    }
                } else {
                    LineKind::Block { cs, rule }
                };
            }
        }
        if depth > 0 {
            return LineKind::Cont {
                cs: cur,
                depth,
                item: false,
            };
        }
        match self.line_rule(src, ls, le) {
            Some((cs, rule)) => LineKind::Line { cs, rule },
            None => LineKind::Para,
        }
    }

    /// Might the full parse give the line `[ls, le)` to a rule it tries
    /// before `rule`? `false` when no such rule can begin with the line's
    /// first bytes, which leaves the line to `rule` once `rule` matches it;
    /// `true` when only [`Self::classify`] can tell.
    #[inline]
    fn contested(
        &self,
        classes: &ByteClasses,
        src: &[u8],
        ls: usize,
        le: usize,
        rule: RuleIndex,
    ) -> bool {
        if self.max_nest == 0 || ls >= le {
            return true;
        }
        let b = src[ls];
        match rule {
            RuleIndex::Line(i) => {
                classes.blocks.contains(b)
                    || self.lines[..i.min(self.lines.len())]
                        .iter()
                        .any(|r| match *r {
                            LineRule::Marker { byte, .. } => byte == b,
                            LineRule::Uniform { bytes, .. } => bytes.contains(b),
                        })
            }
            RuleIndex::Block(i) => {
                if classes.simple.contains(b) {
                    return true;
                }
                let mut m = ls;
                while m < le && (src[m] == self.sep || src[m] == self.tab) {
                    m += 1;
                }
                m >= le
                    || self.block[..i.min(self.block.len())]
                        .iter()
                        .any(|r| match *r {
                            BlockRule::Marker { bytes } => bytes.contains(src[m]),
                            BlockRule::Num { digits, .. } => digits.contains(src[m]),
                        })
            }
        }
    }

    /// Does the full parse give the line `[ls, le)` to `rule`?
    #[inline(never)]
    fn gives(&self, src: &[u8], ls: usize, le: usize, rule: RuleIndex) -> bool {
        match (self.classify(src, ls, le), rule) {
            (LineKind::Block { rule: r, .. }, RuleIndex::Block(i)) => r == i,
            (LineKind::Line { rule: r, .. }, RuleIndex::Line(i)) => r == i,
            _ => false,
        }
    }

    /// Is `b` the marker of a `cont` rule?
    #[inline]
    pub(crate) fn is_cont(&self, b: u8) -> bool {
        self.block_simple
            .iter()
            .any(|r| matches!(*r, BlockSimpleRule::Cont { byte } if byte == b))
    }

    /// The fence a line starting at `ls` opens with its first byte, as
    /// `(byte, count)`: the first `block_simple` rule in order that matches
    /// there, when it is a fence. The first step of [`Self::classify`].
    #[inline]
    fn fence_at(&self, src: &[u8], ls: usize, le: usize) -> Option<(u8, u32)> {
        if self.max_nest == 0 {
            return None;
        }
        for rule in self.block_simple {
            match *rule {
                BlockSimpleRule::Fence { bytes, min } => {
                    if let Some(found) = fence_run(src, ls, le, bytes, min) {
                        return Some(found);
                    }
                }
                BlockSimpleRule::Cont { byte } => {
                    if ls < le && src[ls] == byte {
                        return None;
                    }
                }
            }
        }
        None
    }

    /// Content start and rule index of the `block` item opening at `pos`, if
    /// any: the first rule in order that matches after the `sep`/`tab`
    /// indentation (`parse_block!`'s `@open_block`).
    fn block_item(&self, src: &[u8], pos: usize, le: usize) -> Option<(usize, usize)> {
        let mut p = pos;
        while p < le && (src[p] == self.sep || src[p] == self.tab) {
            p += 1;
        }
        if p >= le {
            return None;
        }
        let spaced = |i: usize| i < le && (src[i] == self.sep || src[i] == self.tab);
        for (ri, rule) in self.block.iter().enumerate() {
            match *rule {
                BlockRule::Marker { bytes } => {
                    if bytes.contains(src[p]) && spaced(p + 1) {
                        return Some((p + 2, ri));
                    }
                }
                BlockRule::Num { digits, ends } => {
                    let mut i = p;
                    let mut dc = 0u8;
                    while i < le && digits.contains(src[i]) && dc < 9 {
                        i += 1;
                        dc += 1;
                    }
                    if dc > 0 && i < le && ends.contains(src[i]) && spaced(i + 1) {
                        return Some((i + 2, ri));
                    }
                }
            }
        }
        None
    }

    /// Content start and rule index of the first `lines` rule matching the
    /// line `[ls, le)`, if any (`parse_line!`).
    fn line_rule(&self, src: &[u8], ls: usize, le: usize) -> Option<(usize, usize)> {
        if ls >= le {
            return None;
        }
        for (ri, rule) in self.lines.iter().enumerate() {
            match *rule {
                LineRule::Marker { byte, max } => {
                    if src[ls] != byte {
                        continue;
                    }
                    let mut i = ls;
                    let mut c = 0u8;
                    while i < le && src[i] == byte && c < max {
                        c += 1;
                        i += 1;
                    }
                    if c > 0 && (i >= le || src[i] == self.sep) {
                        return Some((if i < le { i + 1 } else { i }, ri));
                    }
                }
                LineRule::Uniform { bytes, min } => {
                    let delim = src[ls];
                    if !bytes.contains(delim) {
                        continue;
                    }
                    let mut count = 0u32;
                    let valid = src[ls..le].iter().all(|&b| {
                        if b == delim {
                            count += 1;
                            true
                        } else {
                            b == self.sep
                        }
                    });
                    if valid && count >= min {
                        return Some((le, ri));
                    }
                }
            }
        }
        None
    }

    /// Does the model hold any rule? Without one every line is a paragraph
    /// line.
    #[inline]
    pub(crate) fn has_rules(&self) -> bool {
        !(self.block_simple.is_empty() && self.block.is_empty() && self.lines.is_empty())
    }

    /// The byte classes of the model's rules.
    fn classes(&self) -> ByteClasses {
        let mut c = ByteClasses::default();
        if !self.has_rules() {
            return c;
        }
        for set in [&mut c.marks, &mut c.starts, &mut c.blocks] {
            set.insert(self.sep);
            set.insert(self.tab);
        }
        for rule in self.block_simple {
            match *rule {
                BlockSimpleRule::Fence { bytes, .. } => {
                    for set in [&mut c.fences, &mut c.simple, &mut c.starts, &mut c.blocks] {
                        set.union(bytes);
                    }
                }
                BlockSimpleRule::Cont { byte } => {
                    c.simple.insert(byte);
                    c.marks.insert(byte);
                    c.starts.insert(byte);
                    c.blocks.insert(byte);
                }
            }
        }
        for rule in self.block {
            let (marker, ends) = match *rule {
                BlockRule::Marker { bytes } => (bytes, ByteSet::EMPTY),
                BlockRule::Num { digits, ends } => (digits, ends),
            };
            for set in [&mut c.marks, &mut c.starts, &mut c.blocks] {
                set.union(marker);
            }
            c.marks.union(ends);
        }
        for rule in self.lines {
            let bytes = match *rule {
                LineRule::Marker { byte, .. } => ByteSet::EMPTY.with(byte),
                LineRule::Uniform { bytes, .. } => bytes,
            };
            c.marks.union(bytes);
            c.starts.union(bytes);
        }
        c
    }

    /// Where a fence opened on the line `[open_ls, open_le)` inside `depth`
    /// `cont` frames ends, as `(region end, resume position)`.
    ///
    /// Mirrors `parse_block!`'s peel phase over the following lines: an empty
    /// line keeps every frame open; a line that lacks the marker of one of
    /// the `depth` frames closes that frame and every frame inside it, the
    /// fence included, at its start, and is read again; a line whose content
    /// after those markers is a run of at least `count` `byte`s followed by
    /// `sep`/`tab` only closes the fence.
    fn nested_fence_end(
        &self,
        src: &[u8],
        open_ls: usize,
        open_le: usize,
        byte: u8,
        count: u32,
        depth: usize,
    ) -> (usize, usize) {
        let len = src.len();
        let mut ls = if open_le < len { open_le + 1 } else { len };
        while ls < len {
            let eol = find_line_end(src, ls, self.eol);
            let le = self.content_end(src, ls, eol);
            if !self.blank(src, ls, le) {
                let (mut o, mut c) = (open_ls, ls);
                for _ in 0..depth {
                    if c < le && src[c] == src[o] {
                        o = self.past_marker(src, o, open_le);
                        c = self.past_marker(src, c, le);
                    } else {
                        return (ls, ls);
                    }
                }
                if fence_closes(src, c, le, byte, count, self.sep, self.tab) {
                    return (eol, if eol < len { eol + 1 } else { len });
                }
            }
            ls = if eol < len { eol + 1 } else { len };
        }
        (len, len)
    }
}

/// A sorted, non-overlapping set of byte ranges covered by opaque constructs,
/// together with the [`LineModel`] they were found with.
///
/// Built once per source via the generated `Parser::context(source)` and then
/// shared (immutably) by any number of `find_context_*` iterators over the
/// same source. See the [module documentation](self) for the construction
/// semantics.
#[derive(Debug, Clone, Default)]
pub struct ParseContext {
    spans: Vec<Span>,
    lines: LineModel,
    classes: ByteClasses,
}

impl ParseContext {
    /// The opaque regions, sorted by `start`, non-overlapping.
    #[inline]
    pub fn spans(&self) -> &[Span] {
        &self.spans
    }

    /// The line rules the context was built with.
    #[inline]
    pub fn lines(&self) -> &LineModel {
        &self.lines
    }

    /// A fresh lookup cursor over this context.
    #[inline]
    pub fn cursor(&self) -> ContextCursor<'_> {
        ContextCursor {
            spans: &self.spans,
            lines: &self.lines,
            classes: &self.classes,
            idx: 0,
        }
    }

    /// A fresh reader of this context's lines, for a forward scan.
    #[inline]
    pub(crate) fn reader(&self) -> LineReader<'_> {
        LineReader::new(&self.lines, &self.classes)
    }

    /// Build the context in one streaming pass over `source`.
    ///
    /// - `escape` — the grammar's escape byte.
    /// - `lines` — the grammar's line rules; its fences are the fence rules
    ///   of the pass.
    /// - `sym` — every `symmetric` arm with `parse_inside = false`.
    /// - `asym` — every `asymmetric` arm with `parse_inside = false`, with
    ///   its rule's `balanced` flag.
    /// - `chained` — every `chained` rule whose components are both
    ///   `parse_inside = false`.
    /// - `cap_hint` — preallocation hint for the region vector; the generated
    ///   `Parser::context` derives it from the grammar's `[cap]` divisors
    ///   (`0` is always safe and merely re-grows).
    ///
    /// All rule sets may be empty; with no fence and no opaque rule the
    /// context has no regions.
    pub fn build(
        source: &[u8],
        escape: u8,
        lines: LineModel,
        sym: &[SymSpec],
        asym: &[AsymSpec],
        chained: &[ChainSpec],
        cap_hint: usize,
    ) -> Self {
        let len = source.len();
        let eol = lines.eol;
        let breaks = lines.breaks();
        let mut spans: Vec<Span> = Vec::with_capacity(cap_hint);

        // One deduplicated needle set — fence bytes and opaque triggers
        // together — so every iteration issues exactly ONE search
        // (memchr/2/3, or SWAR beyond three distinct bytes), never several.
        // A set too wide for those is searched byte by byte through a table.
        let mut needle_buf = [0u8; MAX_NEEDLES];
        let mut needle_set = ByteSet::default();
        let nn = build_needles(
            lines.block_simple,
            sym,
            asym,
            chained,
            &mut needle_buf,
            &mut needle_set,
        );
        let find = |hay: &[u8]| match nn {
            Some(n) => find_needle(&needle_buf[..n], hay),
            None => hay.iter().position(|&b| needle_set.contains(b)),
        };

        let classes = lines.classes();
        let mut reader = LineReader::new(&lines, &classes);
        let mut chain_memo = vec![ChainMemo::default(); chained.len()];
        // Per `asym` rule, where the inline run of its last failed close
        // search ends: no unescaped close byte lies between that search's
        // start, before every later opener, and here.
        let mut asym_dead: Vec<usize> = vec![0; asym.len()];
        // Per `asym` rule with `balanced = true`, its failed ring searches.
        let mut rings: Vec<Rings> = vec![Rings::default(); asym.len()];
        let mut pos: usize = 0;
        while pos < len {
            let Some(r) = find(&source[pos..]) else {
                break;
            };
            let p = pos + r;
            let b = source[p];

            // The fence run of a fence line opens a fenced region; a fence
            // byte among a line's markers or on an info line is no trigger. A
            // byte that is both a fence byte and an inline trigger (a
            // backtick) falls through to the inline rule below when no fence
            // opens. A fence at a line start is told without reading the
            // line further.
            let fence_byte = classes.fences.contains(b);
            if fence_byte && (p == 0 || source[p - 1] == eol) {
                let le = find_line_end(source, p, eol);
                let ce = lines.content_end(source, p, le);
                if let Some((byte, count)) = lines.fence_at(source, p, ce) {
                    let end = fence_close_scan(source, le, len, byte, count, &lines);
                    spans.push(Span::new(p as u32, end as u32));
                    pos = if end < len { end + 1 } else { len };
                    continue;
                }
            }
            if fence_byte {
                if let Some(line) = reader.lead(source, p) {
                    if let LineKind::Fence {
                        at,
                        byte,
                        count,
                        depth,
                    } = line.kind
                    {
                        if p == at {
                            let (end, resume) = if depth == 0 {
                                let end =
                                    fence_close_scan(source, line.le, len, byte, count, &lines);
                                (end, if end < len { end + 1 } else { len })
                            } else {
                                lines.nested_fence_end(source, line.ls, line.le, byte, count, depth)
                            };
                            spans.push(Span::new(p as u32, end as u32));
                            pos = resume;
                            continue;
                        }
                    }
                    if p < line.content_start() {
                        pos = p + 1;
                        continue;
                    }
                }
            }

            if count_escape(source, p, escape) % 2 == 1 {
                pos = p + 1;
                continue;
            }

            let mut run_end = p;
            let mut c = 0u32;
            while run_end < len && source[run_end] == b {
                c += 1;
                run_end += 1;
            }
            // Choose an arm only after measuring the complete opening run.
            // An unsupported count is literal, except that a run beginning
            // among continuation markers must be retried past those markers.
            let Some(rule) = rule_for(b, c, sym, asym, chained) else {
                pos = if lines.is_cont(b) && reader.in_markers(source, p) {
                    p + 1
                } else {
                    run_end
                };
                continue;
            };
            let count = c;
            if !fence_byte && classes.marks.contains(b) && reader.in_markers(source, p) {
                pos = p + 1;
                continue;
            }

            // The close search stays inside the opener's inline run: the rest
            // of a block line, or the run of a paragraph line or of a line of
            // `cont` frames alone. The opener's line is read when the search
            // first meets its end.
            let mut run: Option<Run> = None;
            let enter_line = |ls: usize| reader.crosses(source, p, ls, &mut run);

            match rule {
                Opaque::Sym => {
                    match sym_close(source, run_end, b, count, escape, breaks, enter_line) {
                        Some(close_end) => {
                            spans.push(Span::new(p as u32, close_end as u32));
                            pos = close_end;
                        }
                        None => pos = run_end,
                    }
                }
                Opaque::Asym(ri) if asym[ri].3 => {
                    // Each byte of the run opens a ring in turn, until one
                    // closes.
                    let bytes = (asym[ri].0, asym[ri].1, escape, breaks);
                    let mut enter_line = enter_line;
                    pos = run_end;
                    for o in p..run_end {
                        if let Some(q) = rings[ri].close(source, o, bytes, &mut enter_line) {
                            spans.push(Span::new(o as u32, (q + 1) as u32));
                            pos = q + 1;
                            break;
                        }
                    }
                }
                Opaque::Asym(ri) => {
                    if run_end < asym_dead[ri] {
                        pos = run_end;
                        continue;
                    }
                    match asym_close(source, run_end, asym[ri].1, escape, breaks, enter_line) {
                        Ok(cp) => {
                            spans.push(Span::new(p as u32, (cp + 1) as u32));
                            pos = cp + 1;
                        }
                        Err(stop) => {
                            asym_dead[ri] = stop;
                            pos = run_end;
                        }
                    }
                }
                Opaque::Chain(ri) => {
                    let memo = &mut chain_memo[ri];
                    match chain_match(
                        source,
                        run_end,
                        &chained[ri],
                        escape,
                        breaks,
                        enter_line,
                        memo,
                    )
                    .map(|(_, cu)| cu + 1)
                    {
                        Some(end) => {
                            spans.push(Span::new((run_end - 1) as u32, end as u32));
                            pos = end;
                        }
                        None => pos = run_end,
                    }
                }
            }
        }

        Self {
            spans,
            lines,
            classes,
        }
    }
}

/// Deduplicated union of every fence byte, symmetric marker, asymmetric
/// opener and `chained` text opener, kept whole in `set` and written into
/// `buf` while it fits. Returns the count, or `None` when more than
/// [`MAX_NEEDLES`] bytes do not fit `buf` and the search must go through
/// `set`.
fn build_needles(
    block_simple: &[BlockSimpleRule],
    sym: &[SymSpec],
    asym: &[AsymSpec],
    chained: &[ChainSpec],
    buf: &mut [u8; MAX_NEEDLES],
    set: &mut ByteSet,
) -> Option<usize> {
    let mut n = 0;
    let mut push = |b: u8, n: &mut usize| {
        if !set.contains(b) {
            set.insert(b);
            if *n < MAX_NEEDLES {
                buf[*n] = b;
            }
            *n += 1;
        }
    };
    for rule in block_simple {
        if let BlockSimpleRule::Fence { bytes, .. } = *rule {
            for byte in bytes.iter() {
                push(byte, &mut n);
            }
        }
    }
    for &(b, _) in sym {
        push(b, &mut n);
    }
    for &(o, _, _, _) in asym {
        push(o, &mut n);
    }
    for spec in chained {
        push(spec.first.0, &mut n);
    }
    (n <= MAX_NEEDLES).then_some(n)
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

/// An opaque inline rule, with the rule-set index needed by its search.
#[derive(Debug, Clone, Copy)]
enum Opaque {
    Sym,
    Asym(usize),
    Chain(usize),
}

/// The opaque inline arm owning trigger byte `b` and opening run `count`,
/// looked up in the full parser's forward-search order: `chained`,
/// `symmetric`, `asymmetric`.
/// The engine assumes a byte has one meaning per grammar, so the order only
/// matters for a grammar that breaks that. Counts select exact symmetric
/// and asymmetric arms; balanced asymmetric and chained rules accept every
/// opening run length.
#[inline]
fn rule_for(
    b: u8,
    count: u32,
    sym: &[SymSpec],
    asym: &[AsymSpec],
    chained: &[ChainSpec],
) -> Option<Opaque> {
    if let Some(ri) = chained.iter().position(|spec| spec.first.0 == b) {
        return Some(Opaque::Chain(ri));
    }
    if sym.iter().any(|&(sb, _)| sb == b) {
        return sym
            .iter()
            .any(|&(sb, n)| sb == b && n == count)
            .then_some(Opaque::Sym);
    }
    asym.iter()
        .position(|&(o, _, n, balanced)| o == b && (balanced || n == count))
        .map(Opaque::Asym)
}

/// The byte and run count of a fence of one of `bytes` opening at `pos` on
/// `[pos, le)`, if one opens: at least `min` of that byte and no further one
/// on the info line (`parse_block!`'s `@open_simple` fence arm).
#[inline]
fn fence_run(src: &[u8], pos: usize, le: usize, bytes: ByteSet, min: u32) -> Option<(u8, u32)> {
    if pos >= le || !bytes.contains(src[pos]) {
        return None;
    }
    let byte = src[pos];
    let mut i = pos;
    let mut c: u32 = 0;
    while i < le && src[i] == byte {
        c += 1;
        i += 1;
    }
    (c >= min && src[i..le].iter().all(|&x| x != byte)).then_some((byte, c))
}

/// Does the content `[pos, line_end)` close an open fence of `byte` ×
/// `open_count`?
///
/// Mirrors `parse_block!`'s peel-phase close: run `>= open_count`, remainder
/// all `sep` / `tab`.
#[inline]
fn fence_closes(
    src: &[u8],
    pos: usize,
    line_end: usize,
    byte: u8,
    open_count: u32,
    sep: u8,
    tab: u8,
) -> bool {
    let mut i = pos;
    let mut c: u32 = 0;
    while i < line_end && src[i] == byte {
        c += 1;
        i += 1;
    }
    c >= open_count && src[i..line_end].iter().all(|&b| b == sep || b == tab)
}

/// Streaming search for the closing line of a top-level fence: `memchr` for
/// the fence byte, line-start check, then `parse_block!`'s close condition.
/// Returns the close line's end, or `len` for an unclosed fence. Content
/// lines inside the block are never walked.
fn fence_close_scan(
    src: &[u8],
    open_line_end: usize,
    len: usize,
    byte: u8,
    open_count: u32,
    lines: &LineModel,
) -> usize {
    let eol = lines.eol;
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
        let ce = lines.content_end(src, q, cle);
        if fence_closes(src, q, ce, byte, open_count, lines.sep, lines.tab) {
            return cle;
        }
        search = if cle < len { cle + 1 } else { len };
    }
}

/// Escape-aware symmetric close search from `from`, inside the inline run
/// `enter_line` bounds: it is called with the start of every line the
/// search would enter, and `None` ends the run.
#[inline]
fn sym_close(
    src: &[u8],
    from: usize,
    byte: u8,
    count: u32,
    escape: u8,
    breaks: LineBreaks,
    mut enter_line: impl FnMut(usize) -> Option<usize>,
) -> Option<usize> {
    let len = src.len();
    let mut j = from;
    loop {
        let q = next_in_paragraph(src, j, byte, breaks, &mut enter_line)?;
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

/// Escape-aware asymmetric close search from `from`, inside the inline run
/// `enter_line` bounds, as in [`sym_close`].
#[inline]
fn asym_close(
    src: &[u8],
    from: usize,
    close: u8,
    escape: u8,
    breaks: LineBreaks,
    mut enter_line: impl FnMut(usize) -> Option<usize>,
) -> Result<usize, usize> {
    let mut j = from;
    loop {
        let q = scan_paragraph(src, j, close, breaks, &mut enter_line)?;
        if count_escape(src, q, escape) % 2 == 1 {
            j = q + 1;
            continue;
        }
        return Ok(q);
    }
}

/// A lookup cursor over a [`ParseContext`], made for forward scans.
///
/// Amortized O(1) per query **when queries arrive in non-decreasing position
/// order**; [`Self::covering_end`] requires that order. [`Self::is_covered`]
/// also answers a query behind the last one, stepping back over the regions
/// between the two, so queries that stay close to each other — the
/// innermost-first frames of a `cont` block — stay cheap. Copy a cursor to
/// fork a lookahead (e.g. a close-delimiter search) without disturbing the
/// parent's position: the copy starts at the parent's index, which is valid
/// for any position `>=` the parent's last query.
#[derive(Debug, Clone, Copy)]
pub struct ContextCursor<'m> {
    spans: &'m [Span],
    lines: &'m LineModel,
    classes: &'m ByteClasses,
    idx: usize,
}

impl ContextCursor<'_> {
    /// Is `pos` inside an opaque region? `pos` may lie behind the last
    /// query.
    #[inline]
    pub fn is_covered(&mut self, pos: usize) -> bool {
        while self.idx > 0 && (self.spans[self.idx - 1].end as usize) > pos {
            self.idx -= 1;
        }
        self.covering_end(pos).is_some()
    }

    /// If `pos` is inside an opaque region, return that region's `end` offset
    /// (exclusive) so the caller can jump past it. `None` when uncovered.
    /// `pos` must not lie behind the last query.
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

    /// Does the full parse give the line of `item` to `rule`? `item` is a
    /// span a line-family iterator yielded: it ends at its line's end.
    ///
    /// `false` when the line starts inside an opaque region (a fenced block),
    /// or when the full parse reads the line otherwise: as a `fence` or
    /// `cont` line, or as an item of a rule it tries first — `* * *` is a
    /// bullet item when `*` is a marker, and of two `lines` rules matching
    /// one line the first declared takes it. A context whose model has no
    /// rules admits every line it does not cover. Items come in increasing
    /// order, as a line-family iterator yields them.
    #[inline]
    pub fn admits(&mut self, source: &[u8], item: Span, rule: RuleIndex) -> bool {
        let le = (item.end as usize).min(source.len());
        let mut ls = (item.start as usize).min(le);
        while ls > 0 && source[ls - 1] != self.lines.eol {
            ls -= 1;
        }
        if self.is_covered(ls) {
            return false;
        }
        !self.lines.has_rules()
            || !self.lines.contested(self.classes, source, ls, le, rule)
            || self.lines.gives(source, ls, le, rule)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An example grammar's line rules: ``` fences, `>` blocks, `-`/`*`/`+`
    /// and numbered items, `#` lines and `-`/`*`/`_` uniform lines.
    const EXAMPLE_LINES: LineModel = LineModel {
        eol: b'\n',
        sep: b' ',
        tab: b'\t',
        crlf: false,
        max_nest: 4,
        block_simple: &[
            BlockSimpleRule::Fence {
                bytes: ByteSet::from_bytes(b"`"),
                min: 3,
            },
            BlockSimpleRule::Cont { byte: b'>' },
        ],
        block: &[
            BlockRule::Marker {
                bytes: ByteSet::from_bytes(b"-*+"),
            },
            BlockRule::Num {
                digits: ByteSet::from_bytes(b"0123456789"),
                ends: ByteSet::from_bytes(b".)"),
            },
        ],
        lines: &[
            LineRule::Marker { byte: b'#', max: 6 },
            LineRule::Uniform {
                bytes: ByteSet::from_bytes(b"-*_"),
                min: 3,
            },
        ],
    };

    /// Fence rules only, every other line a paragraph line.
    const fn fences(block_simple: &'static [BlockSimpleRule]) -> LineModel {
        LineModel {
            eol: b'\n',
            sep: b' ',
            tab: b'\t',
            crlf: false,
            max_nest: 1,
            block_simple,
            block: &[],
            lines: &[],
        }
    }

    /// The example grammar's `chained` rule: `[`…`]` then `(`…`)`.
    const EXAMPLE_LINKS: ChainSpec = ChainSpec {
        first: (b'[', b']'),
        first_balanced: false,
        second: (b'(', b')'),
        second_balanced: false,
    };

    fn example_context(src: &[u8]) -> ParseContext {
        // The example grammar's opaque rules: ``` fences, `` ` `` pairs, `<`…`>`
        // and the `chained` rule.
        ParseContext::build(
            src,
            b'\\',
            EXAMPLE_LINES,
            &[(b'`', 1)],
            &[(b'<', b'>', 1, false)],
            &[EXAMPLE_LINKS],
            0,
        )
    }

    fn kinds(src: &[u8], model: LineModel) -> Vec<LineKind> {
        let mut out = Vec::new();
        let mut ls = 0;
        while ls < src.len() {
            let le = find_line_end(src, ls, b'\n');
            out.push(model.classify(src, ls, le));
            ls = le + 1;
        }
        out
    }

    // 01. Empty rule set / empty source produce an empty context
    #[test]
    fn test_01_empty() {
        let m = ParseContext::build(b"abc", b'\\', LineModel::default(), &[], &[], &[], 0);
        assert!(m.spans().is_empty());
        let m = example_context(b"");
        assert!(m.spans().is_empty());
    }

    // 02. A code span covers its full extent, delimiters included
    #[test]
    fn test_02_code_span() {
        let m = example_context(b"a `code` b");
        assert_eq!(m.spans(), &[Span::new(2, 8)]);
    }

    // 03. An autolink covers open through close byte
    #[test]
    fn test_03_autolink() {
        let m = example_context(b"x <http://a> y");
        assert_eq!(m.spans(), &[Span::new(2, 12)]);
    }

    // 04. A fenced block covers open line through close line
    #[test]
    fn test_04_fence() {
        let src = b"before\n```\n**not bold**\n```\nafter";
        let m = example_context(src);
        assert_eq!(m.spans(), &[Span::new(7, 27)]);
    }

    // 05. An unclosed fence covers to end of input
    #[test]
    fn test_05_unclosed_fence() {
        let src = b"a\n```\nrest never closes";
        let m = example_context(src);
        assert_eq!(m.spans(), &[Span::new(2, src.len() as u32)]);
    }

    // 06. Leftmost-wins: an autolink open inside a code span is not a region
    //     of its own; scanning resumes after the code span
    #[test]
    fn test_06_leftmost_wins() {
        let m = example_context(b"`a <b` c> d <e>");
        // code span [0,6); the `<b` inside it is invisible; `c>` alone is no
        // autolink (no `<`), the next real autolink is `<e>` at 12..15.
        assert_eq!(m.spans(), &[Span::new(0, 6), Span::new(12, 15)]);
    }

    // 07. Escaped opening delimiter does not open an opaque region; the next
    //     unescaped delimiter becomes the opener (full-parse semantics)
    #[test]
    fn test_07_escaped_open() {
        let m = example_context(b"a \\`not code` b `real`");
        // `\`` at 3 is literal; the backtick at 12 opens, closing at 16.
        assert_eq!(m.spans(), &[Span::new(12, 17)]);
    }

    // 08. Escaped closing delimiter is skipped by the close search
    #[test]
    fn test_08_escaped_close() {
        let m = example_context(b"`a\\`b` rest");
        assert_eq!(m.spans(), &[Span::new(0, 6)]);
    }

    // 09. Unclosed inline construct covers nothing
    #[test]
    fn test_09_unclosed_inline() {
        let m = example_context(b"a `never closes");
        assert!(m.spans().is_empty());
    }

    // ---- Paragraph-bounded behaviour --------------------------------- //

    // 10. A pair on paragraph lines may span a line break: consecutive
    //     paragraph lines are one inline run
    #[test]
    fn test_10_pair_spans_single_newline() {
        let m = example_context(b"a `x\ny` b");
        assert_eq!(m.spans(), &[Span::new(2, 7)]);
    }

    // 11. An empty line (two consecutive eol bytes) aborts a pending opener:
    //     opaque constructs in different paragraphs never pair.
    #[test]
    fn test_11_empty_line_aborts_open() {
        let m = example_context(b"a `x\n\ny` b");
        assert!(m.spans().is_empty());
    }

    // 12. A fence-opening line aborts a pending opener — a block construct
    //     ends the paragraph — but the fence itself still gets its own
    //     region.
    #[test]
    fn test_12_fence_aborts_pending_open() {
        let m = example_context(b"a `x\n```\ncode\n```\n");
        assert_eq!(m.spans(), &[Span::new(5, 17)]);
    }

    // 13. A fence info line with a trailing fence byte does not open
    #[test]
    fn test_13_fence_info_line_reject() {
        let m = example_context(b"``` info ` tick\ncontent\n```\n");
        // The open line is rejected (` on the info line), so the *inline*
        // code-span rule sees the ``` run (count 3 != 1, no match); third
        // line's ``` opens an (unclosed) fence to EOF.
        assert_eq!(m.spans(), &[Span::new(24, 28)]);
    }

    // 14. Fence close requires at least the open count
    #[test]
    fn test_14_fence_close_count() {
        let src = b"````\nx\n```\n````\ny";
        let m = example_context(src);
        assert_eq!(m.spans(), &[Span::new(0, 15)]);
    }

    // 15. Cursor: monotone queries, covering_end jumps
    #[test]
    fn test_15_cursor() {
        let m = example_context(b"a `b` c `d` e");
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
        let m = example_context(b"a `b` c `d` e");
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
        let m = ParseContext::build(src, b'\\', LineModel::default(), &[(b'"', 1)], &[], &[], 0);
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
        const LINES: LineModel = fences(&[BlockSimpleRule::Fence {
            bytes: ByteSet::from_bytes(b"~"),
            min: 3,
        }]);
        let m = ParseContext::build(src, b'\\', LINES, &[(b'`', 1)], &[], &[], 0);
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
            let m = ParseContext::build(&src, b'\\', LineModel::default(), &sym, &[], &[], 0);
            assert_eq!(m.spans(), &[Span::new(0, 4)], "needle count {n}");
        }
    }

    // 20. More trigger bytes than the needle buffer holds: the scan falls
    //     back to the byte table, and every rule still matches
    #[test]
    fn test_20_needle_overflow_scans_table() {
        let alphabet = *b"qwzjvxkyfgdm";
        let sym: Vec<SymSpec> = alphabet.iter().map(|&b| (b, 1)).collect();
        let m = ParseContext::build(b"mam qbq", b'\\', LineModel::default(), &sym, &[], &[], 0);
        assert_eq!(m.spans(), &[Span::new(0, 3), Span::new(4, 7)]);
    }

    // 21. Two distinct fence rules coexist; a mid-line or short-run fence
    //     byte that opens nothing is ordinary content
    #[test]
    fn test_21_two_fence_rules() {
        let src = b"~~ not\n```\na\n```\n~~~\nb\n~~~\nx ``` y";
        const LINES: LineModel = fences(&[
            BlockSimpleRule::Fence {
                bytes: ByteSet::from_bytes(b"`"),
                min: 3,
            },
            BlockSimpleRule::Fence {
                bytes: ByteSet::from_bytes(b"~"),
                min: 3,
            },
        ]);
        let m = ParseContext::build(src, b'\\', LINES, &[], &[], &[], 0);
        assert_eq!(m.spans(), &[Span::new(7, 16), Span::new(17, 26)]);
    }

    // 22. Close-fence scan: a mid-line fence byte and a close-line with
    //     trailing junk are both skipped before the true close
    #[test]
    fn test_22_fence_close_scan_edges() {
        let src = b"```\ncode ` tick\n``` x\n```\nafter";
        let m = example_context(src);
        assert_eq!(m.spans(), &[Span::new(0, 25)]);
    }

    // 23. Asymmetric: a wrong-length open run and an unclosed opener both
    //     yield nothing; escaped close candidates are skipped
    #[test]
    fn test_23_asym_branches() {
        let m = example_context(b"<<double> x");
        assert!(m.spans().is_empty());
        let m = example_context(b"a <never closes");
        assert!(m.spans().is_empty());
        let m = example_context(b"<a\\>b> c");
        assert_eq!(m.spans(), &[Span::new(0, 6)]);
    }

    // 24. Asymmetric close search is paragraph-bounded: crosses one newline,
    //     aborts on an empty line and on a fence-opening line
    #[test]
    fn test_24_asym_paragraph_bounds() {
        let m = example_context(b"<a\nb> c");
        assert_eq!(m.spans(), &[Span::new(0, 5)]);
        let m = example_context(b"<a\n\nb> c");
        assert!(m.spans().is_empty());
        let m = example_context(b"<a\n```\nx\n```\nb> c");
        assert_eq!(m.spans(), &[Span::new(3, 12)]);
    }

    // 25. Symmetric close search skips a wrong-count closing run
    #[test]
    fn test_25_sym_close_run_mismatch() {
        let m = example_context(b"`a``b` x");
        assert_eq!(m.spans(), &[Span::new(0, 6)]);
    }

    // ---- Line classification ----------------------------------------- //

    // 26. Each line kind of the example grammar: fence, nested fence, cont, cont with an
    //     item, item, numbered item, heading, thematic break, empty heading,
    //     paragraph
    #[test]
    fn test_26_classify_kinds() {
        let src = b"```\n> ```\n> > a\n> - b\n - c\n12. d\n## e\n* * *\n---\n#\nplain\n";
        assert_eq!(
            kinds(src, EXAMPLE_LINES),
            vec![
                LineKind::Fence {
                    at: 0,
                    byte: b'`',
                    count: 3,
                    depth: 0,
                },
                LineKind::Fence {
                    at: 6,
                    byte: b'`',
                    count: 3,
                    depth: 1,
                },
                LineKind::Cont {
                    cs: 14,
                    depth: 2,
                    item: false,
                },
                LineKind::Cont {
                    cs: 20,
                    depth: 1,
                    item: true,
                },
                LineKind::Block { cs: 25, rule: 0 },
                LineKind::Block { cs: 31, rule: 1 },
                LineKind::Line { cs: 36, rule: 0 },
                LineKind::Block { cs: 40, rule: 0 },
                LineKind::Line { cs: 47, rule: 1 },
                LineKind::Line { cs: 49, rule: 0 },
                LineKind::Para,
            ]
        );
    }

    // 27. `max_nest` bounds every frame on a line: past the cap, further
    //     markers, a fence and a block item are inline content
    #[test]
    fn test_27_classify_max_nest() {
        let mut model = EXAMPLE_LINES;
        model.max_nest = 1;
        assert_eq!(
            kinds(b"> ```\n> > a\n> - b\n", model),
            vec![
                LineKind::Cont {
                    cs: 2,
                    depth: 1,
                    item: false,
                },
                LineKind::Cont {
                    cs: 8,
                    depth: 1,
                    item: false,
                },
                LineKind::Cont {
                    cs: 14,
                    depth: 1,
                    item: false,
                },
            ]
        );
        model.max_nest = 0;
        assert_eq!(
            kinds(b"```\n> a\n- b\n# c\n", model),
            vec![
                LineKind::Para,
                LineKind::Para,
                LineKind::Para,
                LineKind::Line { cs: 14, rule: 0 },
            ]
        );
    }

    // 28. Near misses stay paragraph lines: no sep after a marker, a heading
    //     run past `max`, a tab or junk in a uniform line, ten digits, an
    //     indented fence
    #[test]
    fn test_28_classify_near_misses() {
        let src = b"-a\n####### x\n_ _\t_\n--- x\n1234567890. y\n ```\n";
        assert!(
            kinds(src, EXAMPLE_LINES)
                .iter()
                .all(|k| *k == LineKind::Para)
        );
    }

    // 29. Rules match in declaration order within a section, and the
    //     matching rule's index is kept
    #[test]
    fn test_29_classify_declaration_order() {
        const HEAD_FIRST: LineModel = LineModel {
            lines: &[
                LineRule::Marker { byte: b'-', max: 1 },
                LineRule::Uniform {
                    bytes: ByteSet::from_bytes(b"-"),
                    min: 3,
                },
            ],
            ..fences(&[])
        };
        const RULE_FIRST: LineModel = LineModel {
            lines: &[
                LineRule::Uniform {
                    bytes: ByteSet::from_bytes(b"-"),
                    min: 3,
                },
                LineRule::Marker { byte: b'-', max: 1 },
            ],
            ..fences(&[])
        };
        assert_eq!(
            kinds(b"- - -", HEAD_FIRST),
            vec![LineKind::Line { cs: 2, rule: 0 }]
        );
        assert_eq!(
            kinds(b"- - -", RULE_FIRST),
            vec![LineKind::Line { cs: 5, rule: 0 }]
        );
        const MARKERS: LineModel = LineModel {
            block: &[
                BlockRule::Marker {
                    bytes: ByteSet::from_bytes(b"*"),
                },
                BlockRule::Marker {
                    bytes: ByteSet::from_bytes(b"*-"),
                },
            ],
            ..fences(&[])
        };
        assert_eq!(
            kinds(b"* a\n- b\n", MARKERS),
            vec![
                LineKind::Block { cs: 2, rule: 0 },
                LineKind::Block { cs: 6, rule: 1 },
            ]
        );
    }

    // ---- Inline runs on block lines ---------------------------------- //

    // 30. A pair never leaves a paragraph run for a line a rule claims: a
    //     bullet, a heading, a blockquote line
    #[test]
    fn test_30_paragraph_run_ends_at_block_line() {
        assert!(example_context(b"`x\n- y` z\n").spans().is_empty());
        assert_eq!(
            example_context(b"The `bar\n## H `code`\n").spans(),
            &[Span::new(14, 20)]
        );
        assert!(example_context(b"`x\n> y`\n> > z\n").spans().is_empty());
    }

    // 31. On a block item line the inline run is the line: a pair does not
    //     reach the next line, paragraph line or not. Lines of the same
    //     `cont` frames alone are one run, read past their markers; another
    //     depth, an item, or blank content ends it
    #[test]
    fn test_31_block_line_run_is_one_line() {
        assert!(example_context(b"- `a\nb`\n").spans().is_empty());
        assert_eq!(
            example_context(b"- `a` `b\n- c`\n").spans(),
            &[Span::new(2, 5)]
        );
        assert_eq!(example_context(b"> `x\n> y`\n").spans(), &[Span::new(2, 9)]);
        assert_eq!(
            example_context(b"> > `x\n> > y`\n").spans(),
            &[Span::new(4, 13)]
        );
        assert!(example_context(b"> `x\n> > y`\n").spans().is_empty());
        assert!(example_context(b"> - `x\n> y`\n").spans().is_empty());
        assert!(example_context(b"> `x\n> - y`\n").spans().is_empty());
        assert!(example_context(b"> `x\n>  \n> y`\n").spans().is_empty());
        assert!(example_context(b"`x\n   \ny`\n").spans().is_empty());
    }

    // 32. Marker bytes are not triggers, even when a trigger byte doubles as
    //     a marker
    #[test]
    fn test_32_markers_are_not_triggers() {
        const PIPES: LineModel = LineModel {
            block_simple: &[BlockSimpleRule::Cont { byte: b'|' }],
            max_nest: 2,
            ..fences(&[])
        };
        let m = ParseContext::build(
            b"| a | b |\n| | c |\n",
            b'\\',
            PIPES,
            &[(b'|', 1)],
            &[],
            &[],
            0,
        );
        assert_eq!(m.spans(), &[Span::new(4, 9)]);
        // With room for one frame only, the second `|` is content: a run
        // that begins among the markers goes on into content.
        let one = LineModel {
            max_nest: 1,
            ..PIPES
        };
        let m = ParseContext::build(b"||x|\n", b'\\', one, &[(b'|', 1)], &[], &[], 0);
        assert_eq!(m.spans(), &[Span::new(1, 4)]);
    }

    // 33. A fence byte that is also a marker and a trigger is no trigger
    //     among a line's markers, and opens no fence where `max_nest` leaves
    //     no room for a frame
    #[test]
    fn test_33_fence_byte_markers() {
        const PLUS: LineModel = LineModel {
            block_simple: &[BlockSimpleRule::Fence {
                bytes: ByteSet::from_bytes(b"+"),
                min: 3,
            }],
            block: &[BlockRule::Marker {
                bytes: ByteSet::from_bytes(b"+"),
            }],
            ..fences(&[])
        };
        let m = ParseContext::build(b"+ a +\n", b'\\', PLUS, &[(b'+', 1)], &[], &[], 0);
        assert!(m.spans().is_empty());
        let mut flat = EXAMPLE_LINES;
        flat.max_nest = 0;
        let m = ParseContext::build(b"```\n`x`\n```\n", b'\\', flat, &[(b'`', 1)], &[], &[], 0);
        assert_eq!(m.spans(), &[Span::new(4, 7)]);
    }

    // ---- Fences inside continuation frames --------------------------- //

    // 34. A fence after `cont` markers covers its run through the close line
    #[test]
    fn test_34_nested_fence() {
        let m = example_context(b"> ```\n> *x* `y`\n> ```\nafter `z`\n");
        assert_eq!(m.spans(), &[Span::new(2, 21), Span::new(28, 31)]);
    }

    // 35. A line without the frame's marker ends the nested fence at its
    //     start and is read again; an empty line does not
    #[test]
    fn test_35_nested_fence_ends_with_its_frame() {
        let m = example_context(b"> ```\n\n> a\nb `c`\n");
        assert_eq!(m.spans(), &[Span::new(2, 11), Span::new(13, 16)]);
    }

    // 36. The close line must carry the frames' markers: a top-level ```
    //     line closes the frame, not the fence, and opens a fence of its own
    #[test]
    fn test_36_nested_fence_close_needs_markers() {
        let m = example_context(b"> ```\n> a\n```\nb\n");
        assert_eq!(m.spans(), &[Span::new(2, 10), Span::new(10, 16)]);
    }

    // 37. Nested frames must match marker for marker; deeper markers on a
    //     content line are content
    #[test]
    fn test_37_nested_fence_depth() {
        let m = example_context(b"> > ```\n> > > a\n> > ```\n");
        assert_eq!(m.spans(), &[Span::new(4, 23)]);
        let m = example_context(b"> > ```\n> a\n");
        assert_eq!(m.spans(), &[Span::new(4, 8)]);
    }

    // 38. With no room left under `max_nest`, a fence after the markers does
    //     not open
    #[test]
    fn test_38_nested_fence_needs_room() {
        let mut lines = EXAMPLE_LINES;
        lines.max_nest = 1;
        let m = ParseContext::build(
            b"> ```\n> `x`\n> ```\n",
            b'\\',
            lines,
            &[(b'`', 1)],
            &[],
            &[],
            0,
        );
        assert_eq!(m.spans(), &[Span::new(8, 11)]);
    }

    // ---- Random-access lookup and line admission --------------------- //

    // 39. A cursor answers queries in any order, stepping back as needed
    #[test]
    fn test_39_is_covered_any_order() {
        let m = example_context(b"a `b` c `d` e");
        let mut cur = m.cursor();
        for (pos, covered) in [
            (9, true),
            (2, true),
            (12, false),
            (0, false),
            (4, true),
            (5, false),
        ] {
            assert_eq!(cur.is_covered(pos), covered, "pos {pos}");
        }
    }

    // 40. admits: a fenced line is refused; a line a block item claims is
    //     refused to the lines rules; a line goes to its own rule only; a
    //     line starting with an opaque construct is admitted
    #[test]
    fn test_40_admits() {
        let src = b"```\n# a\n```\n* * *\n## `b`\n- `c`\n> d\n";
        let m = example_context(src);
        // The span a line-family iterator yields for `line`: from `skip`
        // bytes in to the line end.
        let item = |line: &[u8], skip: usize| {
            let ls = src
                .windows(line.len())
                .position(|w| w == line)
                .expect("line present");
            Span::new((ls + skip) as u32, (ls + line.len()) as u32)
        };
        let mut cur = m.cursor();
        assert!(!cur.admits(src, item(b"# a", 2), RuleIndex::Line(0)));
        assert!(!cur.admits(src, item(b"* * *", 0), RuleIndex::Line(1)));
        assert!(cur.admits(src, item(b"* * *", 2), RuleIndex::Block(0)));
        assert!(cur.admits(src, item(b"## `b`", 3), RuleIndex::Line(0)));
        assert!(!cur.admits(src, item(b"## `b`", 3), RuleIndex::Line(1)));
        assert!(cur.admits(src, item(b"- `c`", 2), RuleIndex::Block(0)));
        assert!(!cur.admits(src, item(b"- `c`", 2), RuleIndex::Block(1)));
        assert!(!cur.admits(src, item(b"> d", 2), RuleIndex::Block(0)));
        let plain = ParseContext::default();
        assert!(
            plain
                .cursor()
                .admits(b"* * *", Span::new(0, 5), RuleIndex::Line(1))
        );
        // With `max_nest = 0` no block item opens.
        let mut flat = EXAMPLE_LINES;
        flat.max_nest = 0;
        let m = ParseContext::build(b"- x\n", b'\\', flat, &[], &[], &[], 0);
        assert!(
            !m.cursor()
                .admits(b"- x\n", Span::new(2, 3), RuleIndex::Block(0))
        );
    }

    // 41. The reader tells content from markers by the bytes before a hit:
    //     a hit after content is content, a hit after markers alone gets its
    //     line classified, and a marker run is walked once
    #[test]
    fn test_41_reader_lead() {
        let src = b"a *b*\n* * *x\n> > *c\n";
        let m = example_context(src);
        let mut r = m.reader();
        assert!(r.lead(src, 2).is_none());
        assert!(r.lead(src, 4).is_none());
        let line = r.lead(src, 6).expect("bullet line start");
        assert_eq!(line.kind, LineKind::Block { cs: 8, rule: 0 });
        assert_eq!(r.lead(src, 10).map(|l| l.ls), Some(6));
        let line = r.lead(src, 17).expect("after the quote markers");
        assert_eq!(
            line.kind,
            LineKind::Cont {
                cs: 17,
                depth: 2,
                item: false
            }
        );
        assert!(r.para_at(src, 2));
        assert!(!r.para_at(src, 17));
        assert!(!r.is_para(src, 6));
        assert!(r.is_para(src, 0));
        assert!(r.crosses(src, 17, 6, &mut None).is_none());
        assert!(r.crosses(src, 2, 6, &mut None).is_none());
        assert!(r.crosses(src, 2, 13, &mut Some(Run::Para)).is_none());
    }

    // 42. Hits along a long run of marker bytes see what lies before the
    //     run: content, or the line start
    #[test]
    fn test_42_reader_marker_run() {
        let mut src = b"x".to_vec();
        src.extend_from_slice(&b" -".repeat(5000));
        src.push(b'\n');
        let tail = src.len();
        src.extend_from_slice(&b"- ".repeat(5000));
        src.extend_from_slice(b"*y*\n");
        let m = example_context(&src);
        let mut r = m.reader();
        for p in (2..tail - 1).step_by(2) {
            assert!(r.lead(&src, p).is_none(), "pos {p}");
        }
        for p in (tail..tail + 10_000).step_by(2) {
            let line = r.lead(&src, p).expect("only markers before");
            assert_eq!(line.ls, tail);
        }
    }

    // ---- Opaque `chained` rules ----------------------------------------- //

    fn regions(src: &[u8]) -> Vec<(u32, u32)> {
        example_context(src)
            .spans()
            .iter()
            .map(|s| (s.start, s.end))
            .collect()
    }

    fn chain_regions(src: &[u8], spec: ChainSpec) -> Vec<(u32, u32)> {
        ParseContext::build(src, b'\\', EXAMPLE_LINES, &[(b'`', 1)], &[], &[spec], 0)
            .spans()
            .iter()
            .map(|s| (s.start, s.end))
            .collect()
    }

    // 43. A link covers its open byte through the second close
    #[test]
    fn test_43_link_region() {
        assert_eq!(regions(b"a [b *c*](d) e"), vec![(2, 12)]);
        assert_eq!(regions(b"[a](b)[c](d)"), vec![(0, 6), (6, 12)]);
    }

    // 44. Links and code spans resolve leftmost-first; inside a link the
    //     search for its closes is raw
    #[test]
    fn test_44_link_leftmost_first() {
        assert_eq!(regions(b"`[a`](b)"), vec![(0, 4)]);
        assert_eq!(regions(b"[a `b](c)` d"), vec![(0, 9)]);
        assert_eq!(regions(b"<[a>](b)"), vec![(0, 4)]);
    }

    // 45. A link that fails resumes the scan right after its open run
    #[test]
    fn test_45_failed_link_resumes_after_open() {
        // No second open right after the first close.
        assert_eq!(regions(b"[a `b`] (c)"), vec![(3, 6)]);
        // No first close.
        assert_eq!(regions(b"[a `b`"), vec![(3, 6)]);
        // No second close: the code span inside the url is found.
        assert_eq!(regions(b"[a](`x`"), vec![(4, 7)]);
    }

    // 46. Of a run of open bytes the last opens; an escaped open byte does
    //     not, and the prefix byte stays outside the region
    #[test]
    fn test_46_link_open_run_escape_prefix() {
        assert_eq!(regions(b"[[a](b)"), vec![(1, 7)]);
        assert!(regions(b"\\[a](b)").is_empty());
        assert_eq!(regions(b"\\[[a](b)"), vec![(2, 8)]);
        assert_eq!(regions(b"![a](b)"), vec![(1, 7)]);
        // An escaped first close is skipped.
        assert_eq!(regions(b"[a\\](b)](c)"), vec![(0, 11)]);
        // An escaped second close is skipped.
        assert_eq!(regions(b"[a](b\\)c)"), vec![(0, 9)]);
    }

    // 47. Every part of a link lies in the opener's inline run
    #[test]
    fn test_47_link_inline_run() {
        assert_eq!(regions(b"[a\nb](c)"), vec![(0, 8)]);
        assert_eq!(regions(b"[a](b\nc)"), vec![(0, 8)]);
        assert!(regions(b"[a\n\nb](c)").is_empty());
        assert!(regions(b"[a](b\n\nc)").is_empty());
        // A heading line ends the paragraph run.
        assert!(regions(b"[a\n# b](c)").is_empty());
        // A bullet line's run is the line.
        assert!(regions(b"- [a\nb](c)").is_empty());
        assert_eq!(regions(b"- [a](b)\n"), vec![(2, 8)]);
        // The second open must follow the first close on the same line.
        assert!(regions(b"[a]\n(b)").is_empty());
    }

    // 48. With `balanced`, each unescaped open byte nests one level
    #[test]
    fn test_48_link_balanced() {
        let bal = ChainSpec {
            first_balanced: true,
            second_balanced: true,
            ..EXAMPLE_LINKS
        };
        assert_eq!(chain_regions(b"[a [b] c](d (e) f)", bal), vec![(0, 18)]);
        assert!(chain_regions(b"[a [b] c](d (e) f)", EXAMPLE_LINKS).is_empty());
        assert_eq!(chain_regions(b"[a \\[b](c)", bal), vec![(0, 10)]);
        // A close byte met without reaching depth zero is no proof of
        // absence: a later opener inside still balances.
        assert_eq!(chain_regions(b"[a [b](c)", bal), vec![(3, 9)]);
        assert_eq!(chain_regions(b"[a](b (c) [d](e)", bal), vec![(10, 16)]);
        // A balanced first close serves no other opener: one nested inside
        // closes earlier.
        assert_eq!(chain_regions(b"[a [b](x) c] d", bal), vec![(3, 9)]);
    }

    // 49. What a failed search learns holds only where it holds: up to the
    //     end of the inline run for an absent close byte, up to the close it
    //     found for a match that failed after it
    #[test]
    fn test_49_link_memo_windows() {
        // Absent first close up to the empty line, then a new run.
        assert_eq!(regions(b"[a [b\n\n[c](d)"), vec![(7, 13)]);
        // The first opener's match fails after `]`; the second opener lies
        // past that close and searches anew.
        assert_eq!(regions(b"[a] [b](c)"), vec![(4, 10)]);
        // The first opener's first close serves the second one too, and the
        // second open follows neither.
        assert!(regions(b"[a [b] c").is_empty());
        // An unbalanced first component ends at its first close: the link
        // takes the inner opener in.
        assert_eq!(regions(b"[a [b](c)"), vec![(0, 9)]);
        // Absent second close up to the empty line, then a new run.
        assert_eq!(regions(b"[a](b\n\n[c](d)"), vec![(7, 13)]);
        assert!(regions(b"[a](b [c](d").is_empty());
    }

    // 50. A long run of unclosed openers is read once
    #[test]
    fn test_50_link_unclosed_openers() {
        let mut src = b"[x ".repeat(20_000);
        src.extend_from_slice(b"`c`");
        assert_eq!(regions(&src), vec![(60_000, 60_003)]);
        let mut src = b"[x](".repeat(20_000);
        src.extend_from_slice(b"`c`");
        assert_eq!(regions(&src), vec![(80_000, 80_003)]);
    }

    // 51. A balanced component whose open and close bytes are one byte never
    //     closes, and another rule's search still finds that byte
    #[test]
    fn test_51_link_same_open_and_close_byte() {
        let quoted = ChainSpec {
            first: (b'"', b'"'),
            first_balanced: true,
            ..EXAMPLE_LINKS
        };
        let src = b"\"a <b\"";
        let m = ParseContext::build(
            src,
            b'\\',
            LineModel::default(),
            &[],
            &[(b'<', b'"', 1, false)],
            &[quoted],
            0,
        );
        assert_eq!(m.spans(), &[Span::new(3, 6)]);
    }

    // 52. A failed asymmetric close search holds for the openers after it up
    //     to the end of its inline run only
    #[test]
    fn test_52_asym_failed_search_kept_to_run_end() {
        assert_eq!(regions(b"<a <b\n\n<c>"), vec![(7, 10)]);
        assert_eq!(regions(b"<a\n# <b>"), vec![(5, 8)]);
        assert_eq!(regions(b"<a `b` <c\n\n<d>"), vec![(3, 6), (11, 14)]);
        let mut src = b"<a ".repeat(20_000);
        src.extend_from_slice(b"\n\n<b>");
        assert_eq!(regions(&src), vec![(60_002, 60_005)]);
    }

    // ---- Opaque `asymmetric` rules with `balanced = true` --------------- //

    fn frame_regions(src: &[u8], max_nest: usize) -> Vec<(u32, u32)> {
        let lines = LineModel {
            max_nest,
            ..EXAMPLE_LINES
        };
        ParseContext::build(
            src,
            b'\\',
            lines,
            &[(b'`', 1)],
            &[(b'{', b'}', 1, true)],
            &[],
            0,
        )
        .spans()
        .iter()
        .map(|s| (s.start, s.end))
        .collect()
    }

    // 53. The ring covers its first open byte through its close, whatever
    //     nests in it; nothing else matches inside
    #[test]
    fn test_53_balanced_ring_nests() {
        assert_eq!(frame_regions(b"{a {b} c} `d`", 4), vec![(0, 9), (10, 13)]);
        assert_eq!(frame_regions(b"{{a}} `b`", 4), vec![(0, 5), (6, 9)]);
        assert_eq!(frame_regions(b"{a `}` b} c", 4), vec![(0, 5)]);
        assert_eq!(frame_regions(b"{a\\}b} `c`", 4), vec![(0, 6), (7, 10)]);
        assert_eq!(frame_regions(b"\\{a} `b`", 4), vec![(5, 8)]);
    }

    // 54. A ring that never closes covers nothing, and the next open byte
    //     tries; the other rules match inside it
    #[test]
    fn test_54_unclosed_ring_covers_nothing() {
        assert_eq!(frame_regions(b"{a {b} `c`", 4), vec![(3, 6), (7, 10)]);
        assert_eq!(frame_regions(b"{{a} `b`", 4), vec![(1, 4), (5, 8)]);
        assert_eq!(
            frame_regions(b"{a\nb `c`\n\n`d`", 4),
            vec![(5, 8), (10, 13)]
        );
        assert_eq!(frame_regions(b"{a\n# `b`", 4), vec![(5, 8)]);
        assert_eq!(frame_regions(b"- {a\n`b`", 4), vec![(5, 8)]);
        let src = b"{a ".repeat(20_000);
        assert_eq!(frame_regions(&src, 4), vec![]);
        let mut src = vec![b'{'; 100_000];
        src.extend(std::iter::repeat_n(b'}', 50_000));
        assert_eq!(frame_regions(&src, 4), vec![(50_000, 150_000)]);
    }

    // 55. `max_nest` plays no part in a ring
    #[test]
    fn test_55_balanced_ring_ignores_max_nest() {
        assert_eq!(frame_regions(b"{a{b}c} `d`", 1), vec![(0, 7), (8, 11)]);
        assert_eq!(frame_regions(b"{{{a}}} `b`", 2), vec![(0, 7), (8, 11)]);
        assert_eq!(frame_regions(b"{a} `b`", 0), vec![(0, 3), (4, 7)]);
    }

    // 56. A `num` rule's digit pattern decides where a paragraph run ends:
    //     with letters for digits, `ab. y` is an item line, so the code
    //     opener of the line before it has no close
    #[test]
    fn test_56_num_digit_pattern_ends_run() {
        const LINES: LineModel = LineModel {
            eol: b'\n',
            sep: b' ',
            tab: b'\t',
            crlf: false,
            max_nest: 1,
            block_simple: &[],
            block: &[BlockRule::Num {
                digits: ByteSet::from_bytes(b"abcdefghijklmnopqrstuvwxyz"),
                ends: ByteSet::from_bytes(b"."),
            }],
            lines: &[],
        };
        let m = ParseContext::build(b"`x\nab. y`", b'\\', LINES, &[(b'`', 1)], &[], &[], 0);
        assert_eq!(m.spans(), &[]);
        let m = ParseContext::build(b"`x\n12. y`", b'\\', LINES, &[(b'`', 1)], &[], &[], 0);
        assert_eq!(m.spans(), &[Span::new(0, 9)]);
    }

    // 57. A `num` end byte that is also an opaque trigger is a marker on an
    //     item line, not an opener
    #[test]
    fn test_57_num_end_byte_is_marker() {
        const LINES: LineModel = LineModel {
            eol: b'\n',
            sep: b' ',
            tab: b'\t',
            crlf: false,
            max_nest: 1,
            block_simple: &[],
            block: &[BlockRule::Num {
                digits: ByteSet::from_bytes(b"0123456789"),
                ends: ByteSet::from_bytes(b"`"),
            }],
            lines: &[],
        };
        let m = ParseContext::build(b"1` code`", b'\\', LINES, &[(b'`', 1)], &[], &[], 0);
        assert_eq!(m.spans(), &[]);
        let m = ParseContext::build(b"1`code`", b'\\', LINES, &[(b'`', 1)], &[], &[], 0);
        assert_eq!(m.spans(), &[Span::new(1, 7)]);
    }

    // 58. Repeated balanced first openers with a close byte but no second
    //     component cover no context region.
    #[test]
    fn test_58_link_balanced_first_openers_with_close() {
        const N: usize = 1_024;
        let spec = ChainSpec {
            first: (b'{', b'}'),
            first_balanced: true,
            second: (b'<', b'>'),
            second_balanced: false,
        };
        let mut src = [b' '; N * 3 + 1];
        for chunk in src[..N * 3].chunks_exact_mut(3) {
            chunk.copy_from_slice(b"{x ");
        }
        src[N * 3] = b'}';

        let ctx = ParseContext::build(&src, b'\\', EXAMPLE_LINES, &[], &[], &[spec], 0);
        assert!(ctx.spans().is_empty());
    }

    // 59. A close byte met by failed balanced second searches still closes
    //     the last pair, which alone becomes a context region.
    #[test]
    fn test_59_link_balanced_second_openers_with_close() {
        const N: usize = 1_024;
        let spec = ChainSpec {
            first: (b'{', b'}'),
            first_balanced: false,
            second: (b'<', b'>'),
            second_balanced: true,
        };
        let mut src = [b' '; N * 4 + 1];
        for chunk in src[..N * 4].chunks_exact_mut(4) {
            chunk.copy_from_slice(b"{x}<");
        }
        src[N * 4] = b'>';

        let ctx = ParseContext::build(&src, b'\\', EXAMPLE_LINES, &[], &[], &[spec], 0);
        let end = src.len() as u32;
        assert_eq!(ctx.spans(), &[Span::new(end - 5, end)]);
    }

    // 60. Every declared symmetric run count owns its opaque region,
    //     independently of the order of the arms.
    #[test]
    fn test_60_symmetric_arm_selected_by_run_count() {
        for sym in [
            [(b'`', 1), (b'`', 2), (b'`', 3)],
            [(b'`', 3), (b'`', 1), (b'`', 2)],
        ] {
            for src in [&b"`x`"[..], b"``x``", b"```x```"] {
                let ctx = ParseContext::build(src, b'\\', EXAMPLE_LINES, &sym, &[], &[], 0);
                assert_eq!(ctx.spans(), &[Span::new(0, src.len() as u32)]);
            }
            let ctx = ParseContext::build(b"````x````", b'\\', EXAMPLE_LINES, &sym, &[], &[], 0);
            assert!(ctx.spans().is_empty());
        }
    }

    // 61. Exact asymmetric arms also select the declared opening count.
    #[test]
    fn test_61_asymmetric_arm_selected_by_run_count() {
        for asym in [
            [(b'<', b'>', 1, false), (b'<', b'>', 2, false)],
            [(b'<', b'>', 2, false), (b'<', b'>', 1, false)],
        ] {
            for src in [&b"<x>"[..], b"<<x>"] {
                let ctx = ParseContext::build(src, b'\\', EXAMPLE_LINES, &[], &asym, &[], 0);
                assert_eq!(ctx.spans(), &[Span::new(0, src.len() as u32)]);
            }
            let ctx = ParseContext::build(b"<<<x>", b'\\', EXAMPLE_LINES, &[], &asym, &[], 0);
            assert!(ctx.spans().is_empty());
        }
    }
}
