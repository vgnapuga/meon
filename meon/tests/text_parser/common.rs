use meon::define_content;
use meon::span::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Link {
    pub is_image: bool,
    pub text: Span,
    pub url: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyValue {
    pub key: Span,
    pub value: Span,
}

#[derive(Debug, Clone, Copy)]
pub struct Heading {
    pub level: std::num::NonZeroU8,
}

#[derive(Debug, Clone, Copy)]
pub struct ThematicBreak {
    pub kind: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct BulletItem {
    pub kind: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct OrderedItem {
    pub kind: u8,
    pub num: u32,
}

define_content!(TestContent {
    inline {
        links:      Link     [100],
        key_values: KeyValue [20],
    }
    inline_simple {
        texts        [10],
        bolds        [40],
        italics      [40],
        bold_italics [80],
        codes        [80],
        autolinks    [100],
        objects      [100],
        // Separate fields for the max_nest > 1 mechanism tests, so the
        // existing balanced=false `italics`/`bolds` fixtures above (and
        // their tests, which rely on the original single-pending-slot
        // behaviour) stay completely untouched.
        n_italics    [40],
        n_bolds      [40],
    }
    line {
        headings:        Heading       [200],
        thematic_breaks: ThematicBreak [200],
    }
    block {
        bullet_items:  BulletItem  [80],
        ordered_items: OrderedItem [80],
    }
    block_simple {
        hard_breaks  [500],
        paragraphs   [80],
        blockquotes  [200],
        fenced_codes [400],
    }
});

pub fn txt(src: &[u8], s: Span) -> &str {
    std::str::from_utf8(&src[s.start as usize..s.end as usize]).unwrap()
}

macro_rules! run_inline {
    ($src:expr) => {{
        let src: &[u8] = $src;
        let le = src.len();
        let mut st = ParseState::new(le);
        let consumed = meon::parse_inline!(
            st, src, 0, le, texts, false, b'\\', b' ', b'\t', 1;
            hard_break(b'\\', b' ', 2) => hard_breaks;
            on_trigger(b'=') {
                key_value: KeyValue {
                    eq        = b'=';
                    allow_sep = true;
                    end       = b'\n';
                    key       => key,
                    value     => value,
                } => key_values
            }
            on_trigger(b'*', b'`', b'<', b'[') {
                symmetric b'`' {
                    parse_inside = false;
                    balanced     = false;
                    _ => codes
                }
                symmetric b'*' {
                    parse_inside = true;
                    balanced     = false;
                    1 => italics, 2 => bolds, _ => bold_italics
                }
                asymmetric b'<', b'>' {
                    balanced     = false;
                    parse_inside = false;
                    1 => autolinks
                }
                chained: Link {
                    | b'[', b']' | {
                        parse_inside = false;
                        balanced     = false;
                    } => text,
                    | b'(', b')' | {
                        parse_inside = false;
                        balanced     = false;
                    } => url,
                    prefix | b'!' | => is_image,
                } => links
            }
        );
        (st, consumed)
    }};
}

/// Single-level balanced asymmetric fixture — `max_nest = 1` fixed, so a
/// nested same-type region still collapses to one outer span, exactly as
/// before the nesting stack existed. Kept exactly as the original tests
/// expect; see `run_inline_balanced_nested!` for depth > 1.
///
/// NOTE: `on_trigger` now lists **both** `b'{'` and `b'}'`. The close byte
/// must be visible to the same scan that finds the open byte — the bounded
/// stack no longer does an internal forward search for it, unlike the old
/// single-span mechanism.
macro_rules! run_inline_balanced {
    ($src:expr) => {{
        let src: &[u8] = $src;
        let le = src.len();
        let mut st = ParseState::new(le);
        let consumed = meon::parse_inline!(
            st, src, 0, le, texts, false, b'\\', b' ', b'\t', 1;
            on_trigger(b'{', b'}') {
                asymmetric b'{', b'}' {
                    balanced     = true;
                    parse_inside = false;
                    1 => objects
                }
            }
        );
        (st, consumed)
    }};
}

/// Same grammar as `run_inline_balanced!`, with a caller-supplied
/// `max_nest`, so the same input shape can be exercised at depth 1
/// (collapsing) and depth > 1 (real per-level emission).
macro_rules! run_inline_balanced_nested {
    ($src:expr, $maxn:literal) => {{
        let src: &[u8] = $src;
        let le = src.len();
        let mut st = ParseState::new(le);
        let consumed = meon::parse_inline!(
            st, src, 0, le, texts, false, b'\\', b' ', b'\t', $maxn;
            on_trigger(b'{', b'}') {
                asymmetric b'{', b'}' {
                    balanced     = true;
                    parse_inside = false;
                    1 => objects
                }
            }
        );
        (st, consumed)
    }};
}

/// `symmetric { parse_inside = true; balanced = true; … }` fixture, using
/// `n_italics` / `n_bolds` — separate from the pre-existing `italics` /
/// `bolds` (which stay `balanced = false`, untouched). Exercises the fix for
/// the single-pending-slot overwrite bug: a different-count occurrence of
/// the same byte used to silently clobber the still-pending outer
/// delimiter.
macro_rules! run_inline_sym_nested {
    ($src:expr, $maxn:literal) => {{
        let src: &[u8] = $src;
        let le = src.len();
        let mut st = ParseState::new(le);
        let consumed = meon::parse_inline!(
            st, src, 0, le, texts, false, b'\\', b' ', b'\t', $maxn;
            on_trigger(b'*') {
                symmetric b'*' {
                    parse_inside = true;
                    balanced     = true;
                    1 => n_italics, 2 => n_bolds
                }
            }
        );
        (st, consumed)
    }};
}

macro_rules! run_line {
    ($src:expr, $pos:expr, $le:expr) => {{
        let src: &[u8] = $src;
        let mut st = ParseState::new(src.len());
        let result = meon::parse_line!(
            st, src, $pos, $le, sep = b' ';
            line(b'#', max = 6) |n|:
                Heading {
                    level: std::num::NonZeroU8::new(n).unwrap_or(std::num::NonZeroU8::MIN)
                }
                => headings;
            line_simple(b'-' | b'*' | b'_', min = 3) |b|:
                ThematicBreak { kind: b }
                => thematic_breaks;
        );
        (st, result)
    }};
}

macro_rules! run_block {
    ($active:expr, $src:expr, $pos:expr, $le:expr) => {{
        let src: &[u8] = $src;
        let mut st = ParseState::new(src.len());
        let _tmp = $active;
        let result = meon::parse_block!(
            _tmp, st, src, $pos, $le, sep = b' ', tab = b'\t';
            block_simple {
                fence(b'`', min = 3) => fenced_codes;
                cont(b'>')            => blockquotes;
            }
            block {
                (b'-' | b'*' | b'+') |b|:
                    BulletItem { kind: b }
                    => bullet_items;
                num(b'0'..=b'9', end = b'.' | b')') |n, k|:
                    OrderedItem { kind: k, num: n }
                    => ordered_items;
            }
        );
        (st, result)
    }};
}

macro_rules! run_sym_balanced {
    ($src:expr) => {{
        let src: &[u8] = $src;
        let le = src.len();
        let mut st = ParseState::new(le);
        meon::parse_inline!(
            st, src, 0, le, texts, false, b'\\', b' ', b'\t', 1;
            on_trigger(b'"') {
                symmetric b'"' {
                    parse_inside = false;
                    balanced     = true;
                    _ => codes,
                }
            }
        );
        st
    }};
}

macro_rules! run_chained_balanced {
    ($src:expr, $tbal:tt, $ubal:tt) => {{
        let src: &[u8] = $src;
        let le = src.len();
        let mut st = ParseState::new(le);
        meon::parse_inline!(
            st, src, 0, le, texts, false, b'\\', b' ', b'\t', 1;
            on_trigger(b'[') {
                chained: Link {
                    | b'[', b']' | {
                        parse_inside = false;
                        balanced     = $tbal;
                    } => text,
                    | b'(', b')' | {
                        parse_inside = false;
                        balanced     = $ubal;
                    } => url,
                    prefix | b'!' | => is_image,
                } => links
            }
        );
        st
    }};
}
