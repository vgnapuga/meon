// The front-end accepts sub-rules and settings in any order, with or without
// trailing separators; the canonicalisation pass rewrites them into the one
// shape the runtime macros pattern-match. This grammar declares everything in
// the "wrong" order and must both compile and parse like the tidy spelling.
use meon::define_parser;

pub struct Link {
    pub is_image: bool,
    pub text: meon::span::Span,
    pub url: meon::span::Span,
}
pub struct Pair {
    pub key: meon::span::Span,
    pub value: meon::span::Span,
}

define_parser!(Messy {
    sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\', max_nest = 4;
    inline {
        fallback => texts [10]
        on_trigger(b'*', b'`', b'<', b'[', b'=') {
            key_value: Pair {
                value => value
                key => key
                end = b'\n'
                allow_sep = true
                eq = b'='
            } => pairs [50]
            chained: Link {
                prefix | b'!' | => is_image
                | b'[', b']' | { balanced = false; parse_inside = false } => text
                | b'(', b')' | { parse_inside = false; balanced = false; } => url
            } => links [100]
            asymmetric b'<', b'>' { parse_inside = false; balanced = false; 1 => autolinks [100] }
            symmetric b'*' { 1 => italics [40]; 2 => bolds [40] balanced = true; parse_inside = true }
            symmetric b'`' { balanced = false; parse_inside = false; 1 => codes [80], }
        }
        merge_simple = true
    }
    blocks {
        fallback => paragraphs [80];
    }
});

fn main() {
    let src = b"a *i* **b** `c` <u> ![t](x) k = v\n";
    let c = MessyParser::parse(src);
    assert_eq!(c.italics.len(), 1);
    assert_eq!(c.bolds.len(), 1);
    assert_eq!(c.codes.len(), 1);
    assert_eq!(c.autolinks.len(), 1);
    assert_eq!(c.links.len(), 1);
    assert!(c.links[0].is_image);
    assert_eq!(c.pairs.len(), 1);
    assert_eq!(c.str(c.pairs[0].key).unwrap(), "k");
}
