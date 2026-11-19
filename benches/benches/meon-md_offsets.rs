//! Offset-width comparison benchmark.
//!
//! The same Markdown grammar and the same small corpora are parsed with
//! `Span<u16>`, `Span<u32>` and `Span<u64>` offsets. The corpora are
//! truncated to fit `u16`, so every parser sees the same bytes.
//!
//! This measures the offset type itself: identical rules, identical input,
//! only the generated `Span<T>` width changes.

mod docs_md;

use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use meon::span::Span;
use meon_md::types::{
    block::{BulletItem, OrderedItem},
    line::{Heading, ThematicBreak},
};
use std::num::NonZeroU8;
use std::time::Duration;

macro_rules! link_type {
    ($name:ident, $off:ident) => {
        #[derive(Debug, Clone, Copy)]
        pub struct $name {
            pub is_image: bool,
            pub text: Span<$off>,
            pub url: Span<$off>,
        }
    };
}

link_type!(LinkU16, u16);
link_type!(LinkU32, u32);
link_type!(LinkU64, u64);

macro_rules! markdown_parser {
    ($name:ident, $off:ident, $link:ident, $mod:ident) => {
        pub mod $mod {
            use super::*;
        meon::define_parser!($name {
            sep = b' ',
            eol = b'\n',
            tab = b'\t',
            escape = b'\\',
            crlf = true;

            limits {
                offset = $off;
                max_nest = 4;
                max_inline_span = 0;
                max_line_span = 0;
                max_block_span = 0;
            }

            inline {
                merge_simple = true;

                hard_break(b'\\', b' ', 2) => hard_breaks [500];
                on_trigger(b'*', b'`', b'[', b'<') {
                    symmetric b'`' {
                        parse_inside = false;
                        1 => codes [80],
                    }
                    symmetric b'*' {
                        parse_inside = true;
                        1 => italics [40], 2 => bolds [40], 3 => bold_italics [80],
                    }
                    asymmetric b'<', b'>' {
                        balanced     = false;
                        parse_inside = false;
                        1 => autolinks [100],
                    }
                    chained: $link {
                        | b'[', b']' | {
                            parse_inside = false;
                            balanced     = true;
                        } => text,
                        | b'(', b')' | {
                            parse_inside = false;
                            balanced     = true;
                        } => url,
                        prefix | b'!' | => is_image,
                    } => links [100]
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
        }
    };
}

markdown_parser!(MarkdownU16, u16, LinkU16, u16);
markdown_parser!(MarkdownU32, u32, LinkU32, u32);
markdown_parser!(MarkdownU64, u64, LinkU64, u64);

fn bench_offsets(c: &mut Criterion) {
    for (label, doc) in docs_md::corpora_u16() {
        let bytes = doc.as_bytes();

        docs_md::report(label, bytes);

        let mut group = c.benchmark_group(format!("offsets/{label}"));
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.sample_size(docs_md::SAMPLE_SIZE);
        group.measurement_time(Duration::from_secs(docs_md::SAMPLE_TIME));
        group.warm_up_time(Duration::from_secs(docs_md::WARMUP_TIME));

        group.bench_function("u16", |b| {
            b.iter(|| {
                let content = u16::MarkdownU16Parser::parse(black_box(bytes)).unwrap();
                black_box(content);
            });
        });
        group.bench_function("u32", |b| {
            b.iter(|| {
                let content = u32::MarkdownU32Parser::parse(black_box(bytes)).unwrap();
                black_box(content);
            });
        });
        group.bench_function("u64", |b| {
            b.iter(|| {
                let content = u64::MarkdownU64Parser::parse(black_box(bytes)).unwrap();
                black_box(content);
            });
        });

        group.finish();
    }
}

criterion_group!(benches, bench_offsets);
criterion_main!(benches);
