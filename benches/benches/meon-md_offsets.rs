//! Offset-width comparison benchmark.
//!
//! The same Markdown grammar and the same small corpora are parsed with
//! `Span<u16>`, `Span<u32>` and `Span<u64>` offsets. The corpora are
//! truncated to fit `u16`, so every parser sees the same bytes.
//!
//! This measures the offset type itself: identical rules, identical input,
//! only the generated `Span<T>` width changes. Timing is scan-dominated, so
//! with a 64 KiB corpus the output vectors stay in cache and the time
//! difference is small; the `output heap` line shows the memory difference
//! directly.

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

macro_rules! output_mem {
    ($c:expr, $off:ty, $link:ty) => {{
        let c = $c;
        let mut heap = 0usize;
        heap += c.headings.capacity() * std::mem::size_of::<(Heading, Span<$off>)>();
        heap += c.thematic_breaks.capacity() * std::mem::size_of::<(ThematicBreak, Span<$off>)>();
        heap += c.paragraphs.capacity() * std::mem::size_of::<Span<$off>>();
        heap += c.blockquotes.capacity() * std::mem::size_of::<Span<$off>>();
        heap += c.fenced_codes.capacity() * std::mem::size_of::<Span<$off>>();
        heap += c.bullet_items.capacity() * std::mem::size_of::<(BulletItem, Span<$off>)>();
        heap += c.ordered_items.capacity() * std::mem::size_of::<(OrderedItem, Span<$off>)>();
        heap += c.bolds.capacity() * std::mem::size_of::<Span<$off>>();
        heap += c.italics.capacity() * std::mem::size_of::<Span<$off>>();
        heap += c.bold_italics.capacity() * std::mem::size_of::<Span<$off>>();
        heap += c.codes.capacity() * std::mem::size_of::<Span<$off>>();
        heap += c.links.capacity() * std::mem::size_of::<$link>();
        heap += c.autolinks.capacity() * std::mem::size_of::<Span<$off>>();
        heap += c.hard_breaks.capacity() * std::mem::size_of::<Span<$off>>();
        heap += c.texts.capacity() * std::mem::size_of::<Span<$off>>();

        let mut elements = 0usize;
        elements += c.headings.len();
        elements += c.thematic_breaks.len();
        elements += c.paragraphs.len();
        elements += c.blockquotes.len();
        elements += c.fenced_codes.len();
        elements += c.bullet_items.len();
        elements += c.ordered_items.len();
        elements += c.bolds.len();
        elements += c.italics.len();
        elements += c.bold_italics.len();
        elements += c.codes.len();
        elements += c.links.len();
        elements += c.autolinks.len();
        elements += c.hard_breaks.len();
        elements += c.texts.len();
        (heap, elements)
    }};
}

fn report_output(label: &str, bytes: &[u8]) {
    let c16 = u16::MarkdownU16Parser::parse(bytes).unwrap();
    let c32 = u32::MarkdownU32Parser::parse(bytes).unwrap();
    let c64 = u64::MarkdownU64Parser::parse(bytes).unwrap();

    let (m16, n16) = output_mem!(&c16, u16, LinkU16);
    let (m32, n32) = output_mem!(&c32, u32, LinkU32);
    let (m64, n64) = output_mem!(&c64, u64, LinkU64);

    let pct = |m: usize| 100.0 * m as f64 / bytes.len() as f64;
    println!(
        "│  output heap: u16 {} B ({:.1}%), u32 {} B ({:.1}%), u64 {} B ({:.1}%)",
        m16,
        pct(m16),
        m32,
        pct(m32),
        m64,
        pct(m64)
    );
    println!("│  output elements: u16 {n16}, u32 {n32}, u64 {n64}  ({label})");
}

const LARGE_TARGET_BYTES: usize = 32 * 1024 * 1024;

/// The same three base documents, repeated past a typical L3 cache.
fn large_corpora() -> Vec<(&'static str, String)> {
    [
        ("plain", docs_md::doc_plain()),
        ("hot", docs_md::doc_hot()),
        ("heavy", docs_md::doc_heavy()),
    ]
    .into_iter()
    .map(|(label, base)| {
        let reps = LARGE_TARGET_BYTES.div_ceil(base.len());
        (label, base.repeat(reps))
    })
    .collect()
}

fn report_large(label: &str, bytes: &[u8]) {
    let c32 = u32::MarkdownU32Parser::parse(bytes).unwrap();
    let c64 = u64::MarkdownU64Parser::parse(bytes).unwrap();

    let (m32, n32) = output_mem!(&c32, u32, LinkU32);
    let (m64, n64) = output_mem!(&c64, u64, LinkU64);
    let pct = |m: usize| 100.0 * m as f64 / bytes.len() as f64;

    println!(
        "│  large {label}: {:.1} MiB, output heap u32 {} B ({:.1}%), u64 {} B ({:.1}%); elements u32 {n32}, u64 {n64}",
        bytes.len() as f64 / (1024.0 * 1024.0),
        m32,
        pct(m32),
        m64,
        pct(m64)
    );
}

fn bench_large(c: &mut Criterion) {
    for (label, doc) in large_corpora() {
        let bytes = doc.as_bytes();

        report_large(label, bytes);

        let mut group = c.benchmark_group(format!("offsets-large/{label}"));
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.sample_size(docs_md::SAMPLE_SIZE);
        group.measurement_time(Duration::from_secs(docs_md::SAMPLE_TIME));
        group.warm_up_time(Duration::from_secs(docs_md::WARMUP_TIME));

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

fn bench_offsets(c: &mut Criterion) {
    for (label, doc) in docs_md::corpora_u16() {
        let bytes = doc.as_bytes();

        report_output(label, bytes);
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

criterion_group!(benches, bench_offsets, bench_large);
criterion_main!(benches);
