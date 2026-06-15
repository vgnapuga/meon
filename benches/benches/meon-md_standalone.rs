//! Standalone (`find_*`) iterator benchmark.
//!
//! Each standalone iterator scans the raw source for a single element kind with
//! no cross-element context, so it can — by design — yield a different count
//! than the full parse. This bench prints, per corpus, both the full-parse
//! count and the standalone count for every element kind, then times the
//! standalone scan. The composition header (`docs_md::report`) is shared with
//! the full-parse bench so the two are read against the same inputs.
//!
//! Fairness: input `black_box`-ed; the iterator is fully driven and each item
//! `black_box`-ed so the scan is not optimised away; `Throughput::Bytes` gives
//! GiB/s comparable across feature builds.

mod docs_md;

use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use meon_md::MarkdownParser;
use std::time::Duration;

macro_rules! find_bench {
    ($group:ident, $bytes:ident, $full:ident, $method:ident => $field:ident) => {{
        let standalone_count = MarkdownParser::$method($bytes).count();
        println!(
            "    {:<18} full={:>8}  standalone={:>8}",
            stringify!($method),
            $full.$field.len(),
            standalone_count,
        );
        $group.bench_function(stringify!($method), |b| {
            b.iter(|| {
                MarkdownParser::$method(black_box($bytes)).for_each(|x| {
                    black_box(x);
                })
            })
        });
    }};
}

fn bench_standalone(c: &mut Criterion) {
    for (label, doc) in docs_md::corpora() {
        let bytes = doc.as_bytes();

        // Size + composition header (same report as the full-parse bench).
        docs_md::report(label, bytes);

        // Reference counts from one full parse, for full-vs-standalone deltas.
        let full = MarkdownParser::parse(bytes);
        println!("│  full-vs-standalone counts:");

        let mut group = c.benchmark_group(format!("standalone/{label}"));
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.sample_size(docs_md::SAMPLE_SIZE);
        group.measurement_time(Duration::from_secs(docs_md::SAMPLE_TIME));
        group.warm_up_time(Duration::from_secs(docs_md::WARMUP_TIME));

        find_bench!(group, bytes, full, find_codes           => codes);
        find_bench!(group, bytes, full, find_italics         => italics);
        find_bench!(group, bytes, full, find_bolds           => bolds);
        find_bench!(group, bytes, full, find_bold_italics    => bold_italics);
        find_bench!(group, bytes, full, find_autolinks       => autolinks);
        find_bench!(group, bytes, full, find_links           => links);
        find_bench!(group, bytes, full, find_headings        => headings);
        find_bench!(group, bytes, full, find_thematic_breaks => thematic_breaks);
        find_bench!(group, bytes, full, find_fenced_codes    => fenced_codes);
        find_bench!(group, bytes, full, find_blockquotes     => blockquotes);
        find_bench!(group, bytes, full, find_bullet_items    => bullet_items);
        find_bench!(group, bytes, full, find_ordered_items   => ordered_items);

        group.finish();
    }
}

criterion_group!(benches, bench_standalone);
criterion_main!(benches);
