//! Cross-parser throughput comparison for the `meon-md` grammar.
//!
//! For every corpus this prints the same size + element-composition report as
//! the intra-engine benches (see `docs_md::report`), then times three parsers
//! over the *identical* input:
//!
//! - `meon-md`        — `MarkdownParser::parse`, flat-span structural parse.
//! - `pulldown-cmark` — the `Parser` event iterator, fully consumed. Parse-only,
//!   no rendering — the closest fair pair to meon (a forward event stream, no
//!   AST allocation).
//! - `comrak`         — `parse_document`, building the full CommonMark AST but
//!   NOT rendering. A deliberate upper bound: it does strictly more work.
//!
//! THIS IS NOT A LEADERBOARD. The three do **different amounts of work**:
//! meon-md emits flat spans for a Markdown *subset* (no AST, no reference-link
//! resolution, no rendering); pulldown-cmark/comrak parse full CommonMark. meon
//! will look faster because it does less. `Throughput::Bytes` measures *ingest
//! of different work*, not "equivalent result per second". See
//! `benches/COMPARISON.md` for the full fairness frame (feature delta, corpus
//! bias, SIMD/build-flag parity).
//!
//! Fairness knobs, identical to the intra-engine benches:
//! - Inputs and outputs are `black_box`-ed.
//! - pulldown-cmark's event iterator is fully drained, so the parse cannot be
//!   skipped lazily; comrak's returned AST root is `black_box`-ed.
//! - The `&str` view of each corpus is taken once, outside the timed region.
//! - `Throughput::Bytes` reports GiB/s, comparable across builds on the *same*
//!   corpora.

mod docs_md;

use comrak::{Arena, Options, parse_document};
use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use meon_md::MarkdownParser;
use pulldown_cmark::Parser;
use std::time::Duration;

fn bench_compare(c: &mut Criterion) {
    for (label, doc) in docs_md::corpora() {
        let bytes = doc.as_bytes();
        let text = doc.as_str();

        // Composition report — printed once, before timing (same as the
        // intra-engine parse bench).
        docs_md::report(label, bytes);

        // One-time sanity pass, outside any timed region: confirm all three
        // parsers consume the corpus without panicking. (Markdown parsers do
        // not reject input the way a JSON parser does, but this still guards
        // against a build picking up an incompatible API or a degenerate
        // corpus.)
        {
            let _ = black_box(MarkdownParser::parse(bytes));
            let _ = black_box(Parser::new(text).count());
            let arena = Arena::new();
            let _ = black_box(parse_document(&arena, text, &Options::default()));
        }

        let mut group = c.benchmark_group(format!("compare/{label}"));
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.sample_size(docs_md::SAMPLE_SIZE);
        group.measurement_time(Duration::from_secs(docs_md::SAMPLE_TIME));
        group.warm_up_time(Duration::from_secs(docs_md::WARMUP_TIME));

        // meon-md: flat-span structural parse.
        group.bench_function("meon-md", |b| {
            b.iter(|| {
                let content = MarkdownParser::parse(black_box(bytes));
                black_box(content);
            });
        });

        // pulldown-cmark: parse-only, event iterator fully drained.
        group.bench_function("pulldown-cmark", |b| {
            b.iter(|| {
                let mut events: u64 = 0;
                for ev in Parser::new(black_box(text)) {
                    black_box(&ev);
                    events += 1;
                }
                black_box(events);
            });
        });

        // comrak: full AST build (no rendering). A fresh arena per iteration,
        // so its allocation/free cost is inside the timed region — that is
        // what a real caller pays. Upper bound: strictly more work than the
        // other two.
        group.bench_function("comrak", |b| {
            let opts = Options::default();
            b.iter(|| {
                let arena = Arena::new();
                let root = parse_document(&arena, black_box(text), &opts);
                black_box(root);
            });
        });

        group.finish();
    }
}

criterion_group!(benches, bench_compare);
criterion_main!(benches);
