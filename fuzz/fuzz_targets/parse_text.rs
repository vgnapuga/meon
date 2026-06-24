#![no_main]
use libfuzzer_sys::fuzz_target;
use meon::define_parser;
use meon::span::MAX_INPUT_LEN;
use std::num::NonZeroU8;

// Types for the fuzz-only grammar below — same shapes as meon-md's own
// `types` module, redeclared here because each `define_parser!` call binds
// its own concrete types to its own fields.
pub struct Heading {
    pub level: NonZeroU8,
}
pub struct ThematicBreak {
    pub kind: u8,
}
pub struct BulletItem {
    pub kind: u8,
}
pub struct OrderedItem {
    pub kind: u8,
    pub num: u32,
}
pub struct Link {
    pub is_image: bool,
    pub text: meon::span::Span,
    pub url: meon::span::Span,
}
pub struct Pair {
    pub key: meon::span::Span,
    pub value: meon::span::Span,
}

// `meon_md`'s actual grammar, verbatim, with exactly one addition: a
// `key_value` rule inside the same `on_trigger` block as `*`/`` ` ``/`<`/`[`,
// and `:` added to that trigger set (`,` is auto-added by the engine from
// `key_value`'s own `end =`). Fuzz-only: `meon-md`'s real, published
// grammar does not declare this — `:` firing on ordinary prose ("Note:",
// "3:00", a URL after `http`) on every real parse is not an acceptable
// cost for the production crate just to get this coverage. Everything else
// below is copied unchanged from `meon-md/src/lib.rs`.
define_parser!(MdKvFuzz {
    sep = b' ',
    eol = b'\n',
    tab = b'\t',
    escape = b'\\',
    max_nest = 4;

    inline {
        merge_simple = true;

        hard_break(b'\\', b' ', 2) => hard_breaks [500];
        on_trigger(b'*', b'`', b'[', b'<', b':') {
            symmetric b'`' {
                parse_inside = false;
                balanced     = false;
                1 => codes [80],
            }
            symmetric b'*' {
                parse_inside = true;
                balanced     = true;
                1 => italics [40], 2 => bolds [40], 3 => bold_italics [80],
            }
            asymmetric b'<', b'>' {
                balanced     = false;
                parse_inside = false;
                1 => autolinks [100],
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
            } => links [100]
            key_value: Pair {
                eq        = b':';
                allow_sep = true;
                end       = b',';
                key   => key,
                value => value,
            } => pairs [50]
        }
        fallback => texts [10];
    }
    lines {
        line(b'#', max = 6) |n|:
            Heading { level: NonZeroU8::new(n).unwrap_or(NonZeroU8::MIN) }
            => headings [200];
        line_simple(b'-' | b'*' | b'_', min = 3) |b|:
            ThematicBreak { kind: b }
            => thematic_breaks [200];
    }
    blocks {
        block_simple {
            fence(b'`', min = 3) => fenced_codes [400];
            cont(b'>')           => blockquotes [200];
        }
        block {
            (b'-' | b'*' | b'+') |b|:
                BulletItem { kind: b }
                => bullet_items [80];
            num(b'0'..=b'9', end = b'.' | b')') |n, k|:
                OrderedItem { kind: k, num: n }
                => ordered_items [80];
        }
        fallback => paragraphs [80];
    }
});

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT_LEN {
        return;
    }

    // Same rule set `meon-md`'s real grammar declares, copied verbatim,
    // plus `key_value` sharing the unified `frames`/`fdepth` stack with
    // its own balanced=true symmetric (`*`) and asymmetric (`<`,`>`) stack
    // users. One parser, one pass over the input.
    let kv = MdKvFuzzParser::parse(data);
    let kv_check = |start: u32, end: u32| {
        assert!(start <= end);
        let _ = &kv.source[start as usize..end as usize];
    };
    for &s in &kv.texts {
        kv_check(s.start, s.end);
    }
    for &s in &kv.bolds {
        kv_check(s.start, s.end);
    }
    for &s in &kv.italics {
        kv_check(s.start, s.end);
    }
    for &s in &kv.bold_italics {
        kv_check(s.start, s.end);
    }
    for &s in &kv.codes {
        kv_check(s.start, s.end);
    }
    for &s in &kv.autolinks {
        kv_check(s.start, s.end);
    }
    for &s in &kv.hard_breaks {
        assert!(s.start == s.end);
        kv_check(s.start, s.end);
    }
    for l in &kv.links {
        kv_check(l.text.start, l.text.end);
        kv_check(l.url.start, l.url.end);
    }
    for p in &kv.pairs {
        kv_check(p.key.start, p.key.end);
        kv_check(p.value.start, p.value.end);
    }
    for &s in &kv.paragraphs {
        kv_check(s.start, s.end);
    }
    for &s in &kv.blockquotes {
        kv_check(s.start, s.end);
    }
    for &s in &kv.fenced_codes {
        kv_check(s.start, s.end);
    }
    for (_, s) in &kv.headings {
        kv_check(s.start, s.end);
    }
    for (_, s) in &kv.thematic_breaks {
        kv_check(s.start, s.end);
    }
    for (_, s) in &kv.bullet_items {
        kv_check(s.start, s.end);
    }
    for (_, s) in &kv.ordered_items {
        kv_check(s.start, s.end);
    }
});
