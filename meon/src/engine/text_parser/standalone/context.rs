//! [`ParseContext`] — the opaque-region map consumed by the `find_context_*`
//! standalone iterators — together with [`LineModel`], the grammar's line
//! rules the map is read against, and [`ContextCursor`], its lookup cursor.
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
//!   markers, scanned on its own;
//! - a **paragraph line**: no rule claims it. Consecutive paragraph lines are
//!   one inline run, which ends at an empty line (two consecutive `eol`
//!   bytes), at a line that is not a paragraph line, or at the end of input.
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
//!    frames also ends at the start of the first non-empty line that lacks
//!    one of their markers; an empty line never ends a fence. An unclosed
//!    fence covers to the end of input.
//! 2. Any other hit is an **opaque inline rule** trigger, matched inside the
//!    inline run that holds it, escape-aware throughout, exactly-count on
//!    both sides for symmetric rules. A hit among a line's markers is not a
//!    trigger. A matched construct covers its full extent, delimiters
//!    included, and the scan resumes after it — so overlapping opaque
//!    candidates resolve leftmost-first, like the full parser's trigger loop.
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
//! spans — a bold span may still legally *contain* a code span, exactly as in
//! the full parse.
//!
//! # Remaining divergence from the full parse (by design)
//!
//! - Line-family `find_context_*` finders filter what their context-free
//!   iterator yields and never add to it: a `block` item after `cont` markers
//!   (`> - item`) is not found. `cont` frames are the context-free
//!   iterator's, kept by the position of their marker: a frame whose marker
//!   sits inside a fence is dropped whole, even when its run goes on past the
//!   fence.
//! - This construction's own closing search is escape-aware (matching the
//!   full parser); the context-free `find_*` close search is not.

use super::common::{Span, count_escape, find_line_end, next_in_paragraph};

/// One opaque symmetric rule: `(delimiter byte, exact run count)`.
pub type SymSpec = (u8, u32);
/// One opaque asymmetric rule: `(open byte, close byte, exact open count)`.
pub type AsymSpec = (u8, u8, u32);

/// Maximum distinct needle bytes: up to 3 fence bytes plus up to 8 opaque
/// triggers, deduplicated.
const MAX_NEEDLES: usize = 11;

/// One `block_simple` rule of a [`LineModel`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockSimpleRule {
    /// `fence(byte, min = N)`: a run of at least `min` `byte`s with no further
    /// `byte` on the line opens a fenced block.
    Fence {
        /// The fence byte.
        byte: u8,
        /// The shortest opening run.
        min: u8,
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
        bytes: &'static [u8],
    },
    /// `num(..., end = e1 | e2 | ...)`: one to nine ASCII digits, then one
    /// byte out of `ends`.
    Num {
        /// The bytes that may end the number.
        ends: &'static [u8],
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
        bytes: &'static [u8],
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
        count: u8,
        depth: usize,
    },
    /// `cont` markers open or continue frames; inline content starts at
    /// `cs`.
    Cont { cs: usize },
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
            LineKind::Cont { cs } | LineKind::Block { cs, .. } | LineKind::Line { cs, .. } => cs,
            LineKind::Para => self.ls,
        }
    }
}

/// A set of byte values.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ByteSet([u64; 4]);

impl ByteSet {
    #[inline]
    fn insert(&mut self, b: u8) {
        self.0[usize::from(b >> 6)] |= 1 << (b & 63);
    }

    #[inline(always)]
    fn contains(&self, b: u8) -> bool {
        (self.0[usize::from(b >> 6)] >> (b & 63)) & 1 == 1
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
            self.lines.classify(src, ls, le)
        };
        let line = LineSlot { ls, le, kind };
        self.slot = Some(line);
        line
    }

    /// May the close search for an opener at `p` go on into the line
    /// starting at `ls`? Only from a paragraph line into a paragraph line.
    /// `para` keeps the opener line's reading across the calls of one
    /// search.
    #[inline(never)]
    pub(crate) fn crosses(
        &mut self,
        src: &[u8],
        p: usize,
        ls: usize,
        para: &mut Option<bool>,
    ) -> bool {
        *para.get_or_insert_with(|| self.para_at(src, p)) && self.is_para(src, ls)
    }

    /// Is the line holding `p` a paragraph line?
    fn para_at(&mut self, src: &[u8], p: usize) -> bool {
        if !self.rules {
            return true;
        }
        if let Some(s) = self.slot {
            if s.ls <= p && p <= s.le {
                return s.kind == LineKind::Para;
            }
        }
        let ls = memchr::memrchr(self.lines.eol, &src[..p]).map_or(0, |i| i + 1);
        self.read(src, ls, p).kind == LineKind::Para
    }

    /// Is the line starting at `ls` a paragraph line? A line whose first
    /// byte begins no rule is one without further reading.
    fn is_para(&self, src: &[u8], ls: usize) -> bool {
        !self.rules
            || ls >= src.len()
            || !self.classes.starts.contains(src[ls])
            || self
                .lines
                .classify(src, ls, find_line_end(src, ls, self.lines.eol))
                == LineKind::Para
    }
}

impl LineModel {
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
                    BlockSimpleRule::Fence { byte, min } => {
                        if let Some(count) = fence_run(src, cur, le, byte, min) {
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
                    LineKind::Cont { cs }
                } else {
                    LineKind::Block { cs, rule }
                };
            }
        }
        if depth > 0 {
            return LineKind::Cont { cs: cur };
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
                            LineRule::Uniform { bytes, .. } => bytes.contains(&b),
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
                            BlockRule::Marker { bytes } => bytes.contains(&src[m]),
                            BlockRule::Num { .. } => src[m].is_ascii_digit(),
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
    fn fence_at(&self, src: &[u8], ls: usize, le: usize) -> Option<(u8, u8)> {
        if self.max_nest == 0 {
            return None;
        }
        for rule in self.block_simple {
            match *rule {
                BlockSimpleRule::Fence { byte, min } => {
                    if let Some(count) = fence_run(src, ls, le, byte, min) {
                        return Some((byte, count));
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
                    if bytes.contains(&src[p]) && spaced(p + 1) {
                        return Some((p + 2, ri));
                    }
                }
                BlockRule::Num { ends } => {
                    let mut i = p;
                    let mut dc = 0u8;
                    while i < le && src[i].is_ascii_digit() && dc < 9 {
                        i += 1;
                        dc += 1;
                    }
                    if dc > 0 && i < le && ends.contains(&src[i]) && spaced(i + 1) {
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
                    if !bytes.contains(&delim) {
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
    fn has_rules(&self) -> bool {
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
                BlockSimpleRule::Fence { byte, .. } => {
                    c.fences.insert(byte);
                    c.simple.insert(byte);
                    c.starts.insert(byte);
                    c.blocks.insert(byte);
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
            let (marker, ends): (&[u8], &[u8]) = match *rule {
                BlockRule::Marker { bytes } => (bytes, &[]),
                BlockRule::Num { ends } => (b"0123456789", ends),
            };
            for &b in marker {
                c.marks.insert(b);
                c.starts.insert(b);
                c.blocks.insert(b);
            }
            for &b in ends {
                c.marks.insert(b);
            }
        }
        for rule in self.lines {
            let bytes: &[u8] = match *rule {
                LineRule::Marker { ref byte, .. } => core::slice::from_ref(byte),
                LineRule::Uniform { bytes, .. } => bytes,
            };
            for &b in bytes {
                c.marks.insert(b);
                c.starts.insert(b);
            }
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
        count: u8,
        depth: usize,
    ) -> (usize, usize) {
        let len = src.len();
        let mut ls = if open_le < len { open_le + 1 } else { len };
        while ls < len {
            let le = find_line_end(src, ls, self.eol);
            if ls < le {
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
                    return (le, if le < len { le + 1 } else { len });
                }
            }
            ls = if le < len { le + 1 } else { len };
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
    /// - `asym` — every `asymmetric` arm with `parse_inside = false`.
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
        cap_hint: usize,
    ) -> Self {
        let len = source.len();
        let eol = lines.eol;
        let mut spans: Vec<Span> = Vec::with_capacity(cap_hint);

        // One deduplicated needle set — fence bytes and opaque triggers
        // together — so every iteration issues exactly ONE search
        // (memchr/2/3, or SWAR beyond three distinct bytes), never several.
        let mut needle_buf = [0u8; MAX_NEEDLES];
        let nn = build_needles(lines.block_simple, sym, asym, &mut needle_buf);
        let needles = &needle_buf[..nn];

        let classes = lines.classes();
        let mut reader = LineReader::new(&lines, &classes);
        let mut pos: usize = 0;
        while pos < len {
            let Some(r) = find_needle(needles, &source[pos..]) else {
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
                if let Some((byte, count)) = lines.fence_at(source, p, le) {
                    let end =
                        fence_close_scan(source, le, len, byte, count, eol, lines.sep, lines.tab);
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
                                let end = fence_close_scan(
                                    source, line.le, len, byte, count, eol, lines.sep, lines.tab,
                                );
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

            // Inline opaque rule owning this byte, if any (a pure fence byte
            // that opens nothing is ordinary content).
            let Some((is_sym, ri)) = rule_for(b, sym, asym) else {
                pos = p + 1;
                continue;
            };

            if count_escape(source, p, escape) % 2 == 1 {
                pos = p + 1;
                continue;
            }

            let count = if is_sym { sym[ri].1 } else { asym[ri].2 };
            let mut run_end = p;
            let mut c = 0u32;
            while run_end < len && source[run_end] == b {
                c += 1;
                run_end += 1;
            }

            // A line's markers are no triggers. Only a run that opens is
            // checked, and only for a byte markers may consist of (a fence
            // byte was checked above): a run of another length opens nothing
            // either way, unless the trigger byte is also a `cont` marker,
            // when the run may begin among the markers and go on into
            // content.
            if c != count {
                pos = if lines.is_cont(b) && reader.in_markers(source, p) {
                    p + 1
                } else {
                    run_end
                };
                continue;
            }
            if !fence_byte && classes.marks.contains(b) && reader.in_markers(source, p) {
                pos = p + 1;
                continue;
            }

            // The close search stays inside the opener's inline run: the rest
            // of a block line, or the paragraph run of a paragraph line. The
            // opener's line is read when the search first meets its end.
            let mut para: Option<bool> = None;
            let enter_line = |ls: usize| reader.crosses(source, p, ls, &mut para).then_some(ls);

            if is_sym {
                match sym_close(source, run_end, b, count, escape, eol, enter_line) {
                    Some(close_end) => {
                        spans.push(Span::new(p as u32, close_end as u32));
                        pos = close_end;
                    }
                    None => pos = run_end,
                }
            } else {
                match asym_close(source, run_end, asym[ri].1, escape, eol, enter_line) {
                    Some(cp) => {
                        spans.push(Span::new(p as u32, (cp + 1) as u32));
                        pos = cp + 1;
                    }
                    None => pos = run_end,
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

/// Deduplicated union of every fence byte, symmetric marker and asymmetric
/// opener, written into `buf`. Returns the count written. Overflow beyond
/// [`MAX_NEEDLES`] is silently dropped (a dropped needle only means its rule
/// is missed, never unsound); no known grammar comes close.
fn build_needles(
    block_simple: &[BlockSimpleRule],
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
    for rule in block_simple {
        if let BlockSimpleRule::Fence { byte, .. } = *rule {
            push(byte, &mut n);
        }
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

/// The run count of a fence of `byte` opening at `pos` on `[pos, le)`, if one
/// opens: at least `min` bytes and no further `byte` on the info line
/// (`parse_block!`'s `@open_simple` fence arm).
#[inline]
fn fence_run(src: &[u8], pos: usize, le: usize, byte: u8, min: u8) -> Option<u8> {
    if pos >= le || src[pos] != byte {
        return None;
    }
    let mut i = pos;
    let mut c: u8 = 0;
    while i < le && src[i] == byte {
        c = c.saturating_add(1);
        i += 1;
    }
    (c >= min && src[i..le].iter().all(|&x| x != byte)).then_some(c)
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

/// Streaming search for the closing line of a top-level fence: `memchr` for
/// the fence byte, line-start check, then `parse_block!`'s close condition.
/// Returns the close line's end, or `len` for an unclosed fence. Content
/// lines inside the block are never walked.
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
    eol: u8,
    mut enter_line: impl FnMut(usize) -> Option<usize>,
) -> Option<usize> {
    let len = src.len();
    let mut j = from;
    loop {
        let q = next_in_paragraph(src, j, byte, eol, &mut enter_line)?;
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
    eol: u8,
    mut enter_line: impl FnMut(usize) -> Option<usize>,
) -> Option<usize> {
    let mut j = from;
    loop {
        let q = next_in_paragraph(src, j, close, eol, &mut enter_line)?;
        if count_escape(src, q, escape) % 2 == 1 {
            j = q + 1;
            continue;
        }
        return Some(q);
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

    /// meon-md's line rules.
    const MD_LINES: LineModel = LineModel {
        eol: b'\n',
        sep: b' ',
        tab: b'\t',
        max_nest: 4,
        block_simple: &[
            BlockSimpleRule::Fence { byte: b'`', min: 3 },
            BlockSimpleRule::Cont { byte: b'>' },
        ],
        block: &[
            BlockRule::Marker { bytes: b"-*+" },
            BlockRule::Num { ends: b".)" },
        ],
        lines: &[
            LineRule::Marker { byte: b'#', max: 6 },
            LineRule::Uniform {
                bytes: b"-*_",
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
            max_nest: 1,
            block_simple,
            block: &[],
            lines: &[],
        }
    }

    fn md_context(src: &[u8]) -> ParseContext {
        // meon-md's opaque rule set: ``` fences, `code` spans, <autolinks>.
        ParseContext::build(src, b'\\', MD_LINES, &[(b'`', 1)], &[(b'<', b'>', 1)], 0)
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
        let m = ParseContext::build(b"abc", b'\\', LineModel::default(), &[], &[], 0);
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

    // 10. A pair on paragraph lines may span a line break: consecutive
    //     paragraph lines are one inline run
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
        let m = ParseContext::build(src, b'\\', LineModel::default(), &[(b'"', 1)], &[], 0);
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
        const LINES: LineModel = fences(&[BlockSimpleRule::Fence { byte: b'~', min: 3 }]);
        let m = ParseContext::build(src, b'\\', LINES, &[(b'`', 1)], &[], 0);
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
            let m = ParseContext::build(&src, b'\\', LineModel::default(), &sym, &[], 0);
            assert_eq!(m.spans(), &[Span::new(0, 4)], "needle count {n}");
        }
    }

    // 20. Needles beyond the fixed buffer are dropped: the overflowing rule
    //     simply never matches, everything else still does
    #[test]
    fn test_20_needle_overflow_dropped() {
        let alphabet = *b"qwzjvxkyfgdm";
        let sym: Vec<SymSpec> = alphabet.iter().map(|&b| (b, 1)).collect();
        let m = ParseContext::build(b"mam qbq", b'\\', LineModel::default(), &sym, &[], 0);
        assert_eq!(m.spans(), &[Span::new(4, 7)]);
    }

    // 21. Two distinct fence rules coexist; a mid-line or short-run fence
    //     byte that opens nothing is ordinary content
    #[test]
    fn test_21_two_fence_rules() {
        let src = b"~~ not\n```\na\n```\n~~~\nb\n~~~\nx ``` y";
        const LINES: LineModel = fences(&[
            BlockSimpleRule::Fence { byte: b'`', min: 3 },
            BlockSimpleRule::Fence { byte: b'~', min: 3 },
        ]);
        let m = ParseContext::build(src, b'\\', LINES, &[], &[], 0);
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

    // ---- Line classification ----------------------------------------- //

    // 26. Each line kind of meon-md: fence, nested fence, cont, cont with an
    //     item, item, numbered item, heading, thematic break, empty heading,
    //     paragraph
    #[test]
    fn test_26_classify_kinds() {
        let src = b"```\n> ```\n> > a\n> - b\n - c\n12. d\n## e\n* * *\n---\n#\nplain\n";
        assert_eq!(
            kinds(src, MD_LINES),
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
                LineKind::Cont { cs: 14 },
                LineKind::Cont { cs: 20 },
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
        let mut model = MD_LINES;
        model.max_nest = 1;
        assert_eq!(
            kinds(b"> ```\n> > a\n> - b\n", model),
            vec![
                LineKind::Cont { cs: 2 },
                LineKind::Cont { cs: 8 },
                LineKind::Cont { cs: 14 },
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
        assert!(kinds(src, MD_LINES).iter().all(|k| *k == LineKind::Para));
    }

    // 29. Rules match in declaration order within a section, and the
    //     matching rule's index is kept
    #[test]
    fn test_29_classify_declaration_order() {
        const HEAD_FIRST: LineModel = LineModel {
            lines: &[
                LineRule::Marker { byte: b'-', max: 1 },
                LineRule::Uniform {
                    bytes: b"-",
                    min: 3,
                },
            ],
            ..fences(&[])
        };
        const RULE_FIRST: LineModel = LineModel {
            lines: &[
                LineRule::Uniform {
                    bytes: b"-",
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
                BlockRule::Marker { bytes: b"*" },
                BlockRule::Marker { bytes: b"*-" },
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
        assert!(md_context(b"`x\n- y` z\n").spans().is_empty());
        assert_eq!(
            md_context(b"The `bar\n## H `code`\n").spans(),
            &[Span::new(14, 20)]
        );
        assert!(md_context(b"`x\n> y`\n> > z\n").spans().is_empty());
    }

    // 31. On a block line the inline run is the line: a pair does not reach
    //     the next line, paragraph line or not
    #[test]
    fn test_31_block_line_run_is_one_line() {
        assert!(md_context(b"- `a\nb`\n").spans().is_empty());
        assert!(md_context(b"> `x\n> y`\n").spans().is_empty());
        assert_eq!(md_context(b"- `a` `b\n- c`\n").spans(), &[Span::new(2, 5)]);
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
        let m = ParseContext::build(b"| a | b |\n| | c |\n", b'\\', PIPES, &[(b'|', 1)], &[], 0);
        assert_eq!(m.spans(), &[Span::new(4, 9)]);
        // With room for one frame only, the second `|` is content: a run
        // that begins among the markers goes on into content.
        let one = LineModel {
            max_nest: 1,
            ..PIPES
        };
        let m = ParseContext::build(b"||x|\n", b'\\', one, &[(b'|', 1)], &[], 0);
        assert_eq!(m.spans(), &[Span::new(1, 4)]);
    }

    // 33. A fence byte that is also a marker and a trigger is no trigger
    //     among a line's markers, and opens no fence where `max_nest` leaves
    //     no room for a frame
    #[test]
    fn test_33_fence_byte_markers() {
        const PLUS: LineModel = LineModel {
            block_simple: &[BlockSimpleRule::Fence { byte: b'+', min: 3 }],
            block: &[BlockRule::Marker { bytes: b"+" }],
            ..fences(&[])
        };
        let m = ParseContext::build(b"+ a +\n", b'\\', PLUS, &[(b'+', 1)], &[], 0);
        assert!(m.spans().is_empty());
        let mut flat = MD_LINES;
        flat.max_nest = 0;
        let m = ParseContext::build(b"```\n`x`\n```\n", b'\\', flat, &[(b'`', 1)], &[], 0);
        assert_eq!(m.spans(), &[Span::new(4, 7)]);
    }

    // ---- Fences inside continuation frames --------------------------- //

    // 34. A fence after `cont` markers covers its run through the close line
    #[test]
    fn test_34_nested_fence() {
        let m = md_context(b"> ```\n> *x* `y`\n> ```\nafter `z`\n");
        assert_eq!(m.spans(), &[Span::new(2, 21), Span::new(28, 31)]);
    }

    // 35. A line without the frame's marker ends the nested fence at its
    //     start and is read again; an empty line does not
    #[test]
    fn test_35_nested_fence_ends_with_its_frame() {
        let m = md_context(b"> ```\n\n> a\nb `c`\n");
        assert_eq!(m.spans(), &[Span::new(2, 11), Span::new(13, 16)]);
    }

    // 36. The close line must carry the frames' markers: a top-level ```
    //     line closes the frame, not the fence, and opens a fence of its own
    #[test]
    fn test_36_nested_fence_close_needs_markers() {
        let m = md_context(b"> ```\n> a\n```\nb\n");
        assert_eq!(m.spans(), &[Span::new(2, 10), Span::new(10, 16)]);
    }

    // 37. Nested frames must match marker for marker; deeper markers on a
    //     content line are content
    #[test]
    fn test_37_nested_fence_depth() {
        let m = md_context(b"> > ```\n> > > a\n> > ```\n");
        assert_eq!(m.spans(), &[Span::new(4, 23)]);
        let m = md_context(b"> > ```\n> a\n");
        assert_eq!(m.spans(), &[Span::new(4, 8)]);
    }

    // 38. With no room left under `max_nest`, a fence after the markers does
    //     not open
    #[test]
    fn test_38_nested_fence_needs_room() {
        let mut lines = MD_LINES;
        lines.max_nest = 1;
        let m = ParseContext::build(b"> ```\n> `x`\n> ```\n", b'\\', lines, &[(b'`', 1)], &[], 0);
        assert_eq!(m.spans(), &[Span::new(8, 11)]);
    }

    // ---- Random-access lookup and line admission --------------------- //

    // 39. A cursor answers queries in any order, stepping back as needed
    #[test]
    fn test_39_is_covered_any_order() {
        let m = md_context(b"a `b` c `d` e");
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
        let m = md_context(src);
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
        let mut flat = MD_LINES;
        flat.max_nest = 0;
        let m = ParseContext::build(b"- x\n", b'\\', flat, &[], &[], 0);
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
        let m = md_context(src);
        let mut r = m.reader();
        assert!(r.lead(src, 2).is_none());
        assert!(r.lead(src, 4).is_none());
        let line = r.lead(src, 6).expect("bullet line start");
        assert_eq!(line.kind, LineKind::Block { cs: 8, rule: 0 });
        assert_eq!(r.lead(src, 10).map(|l| l.ls), Some(6));
        let line = r.lead(src, 17).expect("after the quote markers");
        assert_eq!(line.kind, LineKind::Cont { cs: 17 });
        assert!(r.para_at(src, 2));
        assert!(!r.para_at(src, 17));
        assert!(!r.is_para(src, 6));
        assert!(r.is_para(src, 0));
        assert!(!r.crosses(src, 17, 6, &mut None));
        assert!(!r.crosses(src, 2, 6, &mut None));
        assert!(!r.crosses(src, 2, 13, &mut Some(true)));
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
        let m = md_context(&src);
        let mut r = m.reader();
        for p in (2..tail - 1).step_by(2) {
            assert!(r.lead(&src, p).is_none(), "pos {p}");
        }
        for p in (tail..tail + 10_000).step_by(2) {
            let line = r.lead(&src, p).expect("only markers before");
            assert_eq!(line.ls, tail);
        }
    }
}
