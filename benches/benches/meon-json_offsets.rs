//! Offset-width comparison benchmark for the `meon-json` grammar.
//!
//! The same JSON grammar and the same corpora are parsed with `Span<u16>`,
//! `Span<u32>` and `Span<u64>` offsets. The small corpora are sized to fit
//! `u16`; the large corpora are grown past a typical L3 cache and use only
//! `u32`/`u64`.

mod docs_json;

use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use meon::span::Span;
use std::time::Duration;

macro_rules! member_type {
    ($name:ident, $off:ident) => {
        #[derive(Debug, Clone, Copy)]
        pub struct $name {
            pub key: Span<$off>,
            pub value: Span<$off>,
        }
    };
}

member_type!(MemberU16, u16);
member_type!(MemberU32, u32);
member_type!(MemberU64, u64);

macro_rules! json_parser {
    ($name:ident, $off:ident, $member:ident, $mod:ident) => {
        pub mod $mod {
            use super::*;

            meon::define_parser!($name {
                sep = b' ',
                eol = b'\n',
                tab = b'\t',
                escape = b'\\';

                limits {
                    offset = $off;
                    max_nest = 64;
                    max_inline_span = 0;
                    max_line_span = 0;
                    max_block_span = 0;
                }

                inline {
                    merge_simple = false;
                    on_trigger(b'{', b'}', b'[', b']', b'"', b':') {
                        symmetric b'"' {
                            parse_inside = false;
                            1 => strings [16],
                        }
                        asymmetric b'{', b'}' {
                            balanced     = true;
                            parse_inside = true;
                            1 => objects [32],
                        }
                        asymmetric b'[', b']' {
                            balanced     = true;
                            parse_inside = true;
                            1 => arrays [32],
                        }
                        key_value: $member {
                            eq        = b':';
                            allow_sep = true;
                            end       = b',';
                            key   => key,
                            value => value,
                        } => members [16]
                    }
                    fallback => scalars [16];
                }
                blocks {
                    fallback => loose [256];
                }
            });
        }
    };
}

json_parser!(JsonU16, u16, MemberU16, u16);
json_parser!(JsonU32, u32, MemberU32, u32);
json_parser!(JsonU64, u64, MemberU64, u64);

macro_rules! output_mem {
    ($c:expr, $off:ty, $member:ty) => {{
        let c = $c;
        let mut heap = 0usize;
        heap += c.objects.capacity() * std::mem::size_of::<Span<$off>>();
        heap += c.arrays.capacity() * std::mem::size_of::<Span<$off>>();
        heap += c.strings.capacity() * std::mem::size_of::<Span<$off>>();
        heap += c.members.capacity() * std::mem::size_of::<$member>();
        heap += c.scalars.capacity() * std::mem::size_of::<Span<$off>>();
        heap += c.loose.capacity() * std::mem::size_of::<Span<$off>>();

        let mut elements = 0usize;
        elements += c.objects.len();
        elements += c.arrays.len();
        elements += c.strings.len();
        elements += c.members.len();
        elements += c.scalars.len();
        elements += c.loose.len();
        (heap, elements)
    }};
}

fn report_output(label: &str, bytes: &[u8]) {
    let c16 = u16::JsonU16Parser::parse(bytes).unwrap();
    let c32 = u32::JsonU32Parser::parse(bytes).unwrap();
    let c64 = u64::JsonU64Parser::parse(bytes).unwrap();

    let (m16, n16) = output_mem!(&c16, u16, MemberU16);
    let (m32, n32) = output_mem!(&c32, u32, MemberU32);
    let (m64, n64) = output_mem!(&c64, u64, MemberU64);

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

fn report_large(label: &str, bytes: &[u8]) {
    let c32 = u32::JsonU32Parser::parse(bytes).unwrap();
    let c64 = u64::JsonU64Parser::parse(bytes).unwrap();

    let (m32, n32) = output_mem!(&c32, u32, MemberU32);
    let (m64, n64) = output_mem!(&c64, u64, MemberU64);
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

fn bench_offsets(c: &mut Criterion) {
    for (label, doc) in docs_json::corpora_u16() {
        let bytes = doc.as_bytes();

        report_output(label, bytes);
        docs_json::report(label, bytes);

        let mut group = c.benchmark_group(format!("json-offsets/{label}"));
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.sample_size(docs_json::SAMPLE_SIZE);
        group.measurement_time(Duration::from_secs(docs_json::SAMPLE_TIME));
        group.warm_up_time(Duration::from_secs(docs_json::WARMUP_TIME));

        group.bench_function("u16", |b| {
            b.iter(|| {
                let content = u16::JsonU16Parser::parse(black_box(bytes)).unwrap();
                black_box(content);
            });
        });
        group.bench_function("u32", |b| {
            b.iter(|| {
                let content = u32::JsonU32Parser::parse(black_box(bytes)).unwrap();
                black_box(content);
            });
        });
        group.bench_function("u64", |b| {
            b.iter(|| {
                let content = u64::JsonU64Parser::parse(black_box(bytes)).unwrap();
                black_box(content);
            });
        });

        group.finish();
    }
}

const LARGE_TARGET_BYTES: usize = 32 * 1024 * 1024;

fn bench_large(c: &mut Criterion) {
    for (label, doc) in docs_json::corpora_large(LARGE_TARGET_BYTES) {
        let bytes = doc.as_bytes();

        report_large(label, bytes);

        let mut group = c.benchmark_group(format!("json-offsets-large/{label}"));
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.sample_size(docs_json::SAMPLE_SIZE);
        group.measurement_time(Duration::from_secs(docs_json::SAMPLE_TIME));
        group.warm_up_time(Duration::from_secs(docs_json::WARMUP_TIME));

        group.bench_function("u32", |b| {
            b.iter(|| {
                let content = u32::JsonU32Parser::parse(black_box(bytes)).unwrap();
                black_box(content);
            });
        });
        group.bench_function("u64", |b| {
            b.iter(|| {
                let content = u64::JsonU64Parser::parse(black_box(bytes)).unwrap();
                black_box(content);
            });
        });

        group.finish();
    }
}

criterion_group!(benches, bench_offsets, bench_large);
criterion_main!(benches);
