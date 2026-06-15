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
            st, src, 0, le, texts, false, b'\\', b' ', b'\t';
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

macro_rules! run_inline_balanced {
    ($src:expr) => {{
        let src: &[u8] = $src;
        let le = src.len();
        let mut st = ParseState::new(le);
        let consumed = meon::parse_inline!(
            st, src, 0, le, texts, false, b'\\', b' ', b'\t';
            on_trigger(b'{') {
                asymmetric b'{', b'}' {
                    balanced     = true;
                    parse_inside = false;
                    _ => objects
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
            st, src, 0, le, texts, false, b'\\', b' ', b'\t';
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
            st, src, 0, le, texts, false, b'\\', b' ', b'\t';
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
