//! Full single-pass parse benchmark.
//!
//! For every corpus this prints a size + element-composition report **before**
//! the timed run (see `docs_md::report`), then measures `MarkdownParser::parse`
//! end to end — including the internal `Vec` allocations, because that is what a
//! real caller pays. Document generation happens outside the timed region.
//!
//! Fairness notes:
//! - The input is `black_box`-ed so the optimiser cannot precompute results.
//! - The returned content is `black_box`-ed so the parse cannot be eliminated
//!   as dead code.
//! - `Throughput::Bytes` makes Criterion report GiB/s, comparable across the
//!   scalar (stable) and `--features avx2` (nightly) builds on the *same*
//!   corpora.
//!
//! NOTE: this is an intra-engine benchmark. Comparing the raw number against a
//! full CommonMark parser (pulldown-cmark, comrak) is *not* apples to apples:
//! meon-md emits flat spans for a Markdown subset and does no AST construction,
//! reference resolution, or rendering. A fair cross-parser comparison would
//! pin both to parse-only and document the feature delta.

mod docs_md;

use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use meon_md::MarkdownParser;
use std::time::Duration;

fn bench_parse(c: &mut Criterion) {
    for (label, doc) in docs_md::corpora() {
        let bytes = doc.as_bytes();

        // Composition report — printed once, before timing.
        docs_md::report(label, bytes);

        let mut group = c.benchmark_group(format!("parse/{label}"));
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.sample_size(docs_md::SAMPLE_SIZE);
        group.measurement_time(Duration::from_secs(docs_md::SAMPLE_TIME));
        group.warm_up_time(Duration::from_secs(docs_md::WARMUP_TIME));

        group.bench_function("full", |b| {
            b.iter(|| {
                let content = MarkdownParser::parse(black_box(bytes));
                black_box(content);
            });
        });

        group.finish();
    }
}

criterion_group!(benches, bench_parse);
criterion_main!(benches);
