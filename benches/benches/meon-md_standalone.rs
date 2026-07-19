//! Standalone (`find_*`) and context-aware (`find_context_*`) iterator
//! benchmark.
//!
//! Each **context-free** standalone iterator scans the raw source for a
//! single element kind with no cross-element context, so it can — by
//! design — yield a different count than the full parse (e.g. a delimiter
//! inside a fenced block). Each **context-aware** iterator additionally
//! consults a [`meon::ParseContext`] — built by `MarkdownParser::context`
//! from every fenced block and every `parse_inside = false` inline rule in
//! one sequential pass — and skips any candidate whose position falls
//! inside an opaque region. Only rules whose grammar arm declares
//! `parse_inside = true` get a `find_context_*` method at all (`italics` /
//! `bolds` / `bold_italics` here, plus every line/block rule); `codes`,
//! `autolinks`, and `fenced_codes` are themselves the *source* of the
//! context and have no context-aware counterpart. `links` (`chained`) is
//! not covered either — see `meon::define_parser!`'s docs.
//!
//! # Two context-aware numbers, not one
//!
//! `ParseContext::build` is a real, separately-timed cost (the `context`
//! bench below). Whether it belongs in a *single* context-aware call's
//! budget depends entirely on how many context-aware fields you sweep with
//! it — and this bench reports **both** scenarios rather than picking one:
//!
//! - **`find_context_*` (amortized)** — the context is built **once**,
//!   outside the timed loop, and reused by the scan. This is the correct
//!   number when you sweep **several** context-aware fields over the same
//!   source: `context()`'s cost is paid once and shared, so each
//!   additional field only costs its own scan.
//! - **`find_context_*_cold` (single-sweep lower bound)** — `context()` is
//!   rebuilt **inside** the timed loop, every iteration, immediately
//!   before the one scan that uses it. This is the honest cost when you
//!   only need **one** context-aware field: there is no second sweep to
//!   amortize the build against. Do not read the amortized number as a
//!   substitute for this one — for a single field, `_cold` is what you
//!   actually pay.
//!
//! Rule of thumb: `_cold ≈ context + find_context_*` (amortized); the two
//! numbers should sum consistently. If you need `k` context-aware fields,
//! the real total is one `context` build plus `k` amortized scans — closer
//! to the `_cold` number for `k = 1` and to the sum of amortized numbers
//! for `k` large.
//!
//! This bench prints, per corpus, the full-parse count against both the
//! context-free standalone count and — where it exists — the
//! context-aware count, in the same `full=... N=...` shape for both
//! tables. The composition header (`docs_md::report`) is shared with the
//! full-parse bench so all of these are read against the same inputs.
//!
//! Fairness: input `black_box`-ed; every iterator is fully driven and each
//! item `black_box`-ed so the scan is not optimised away; the `context()`
//! build is timed the same way, its result `black_box`-ed;
//! `Throughput::Bytes` gives GiB/s comparable across feature builds.

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

/// Amortized context-aware scan: `ctx` is built once, outside the timed
/// loop, and reused. Correct number when sweeping several context-aware
/// fields over the same source. Prints in the same `full=... N=...` shape
/// as `find_bench!` so the two tables read side by side.
macro_rules! find_context_bench {
    ($group:ident, $bytes:ident, $ctx:ident, $full:ident, $method:ident => $field:ident) => {{
        let context_count = MarkdownParser::$method($bytes, &$ctx).count();
        println!(
            "    {:<18} full={:>8}  context-aware={:>8}",
            stringify!($method),
            $full.$field.len(),
            context_count,
        );
        $group.bench_function(stringify!($method), |b| {
            b.iter(|| {
                MarkdownParser::$method(black_box($bytes), &$ctx).for_each(|x| {
                    black_box(x);
                })
            })
        });
    }};
}

/// Cold (single-sweep) context-aware scan: `context()` is rebuilt on every
/// iteration, immediately before the one scan that consumes it. This is the
/// real lower-bound cost when only one context-aware field is needed —
/// there is no second sweep to amortize the build against.
macro_rules! find_context_cold_bench {
    ($group:ident, $bytes:ident, $method:ident) => {{
        $group.bench_function(concat!(stringify!($method), "_cold"), |b| {
            b.iter(|| {
                let ctx = MarkdownParser::context(black_box($bytes));
                MarkdownParser::$method(black_box($bytes), &ctx).for_each(|x| {
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

        // One-time context build, reused by every amortized find_context_*
        // call below. Timed on its own: this is the number the "_cold"
        // benches re-pay on every single iteration instead.
        let ctx = MarkdownParser::context(bytes);
        println!("│  context regions: {}", ctx.spans().len());
        group.bench_function("context", |b| {
            b.iter(|| {
                black_box(MarkdownParser::context(black_box(bytes)));
            })
        });

        println!("│  full-vs-context-aware counts:");
        find_context_bench!(group, bytes, ctx, full, find_context_italics         => italics);
        find_context_bench!(group, bytes, ctx, full, find_context_bolds          => bolds);
        find_context_bench!(group, bytes, ctx, full, find_context_bold_italics   => bold_italics);
        find_context_bench!(group, bytes, ctx, full, find_context_headings       => headings);
        find_context_bench!(group, bytes, ctx, full, find_context_thematic_breaks => thematic_breaks);
        find_context_bench!(group, bytes, ctx, full, find_context_blockquotes    => blockquotes);
        find_context_bench!(group, bytes, ctx, full, find_context_bullet_items   => bullet_items);
        find_context_bench!(group, bytes, ctx, full, find_context_ordered_items  => ordered_items);

        // Single-sweep (cold) cost: context() rebuilt every iteration,
        // immediately before the one scan that uses it. The real number for
        // "I only need this one context-aware field."
        find_context_cold_bench!(group, bytes, find_context_italics);
        find_context_cold_bench!(group, bytes, find_context_bolds);
        find_context_cold_bench!(group, bytes, find_context_bold_italics);
        find_context_cold_bench!(group, bytes, find_context_headings);
        find_context_cold_bench!(group, bytes, find_context_thematic_breaks);
        find_context_cold_bench!(group, bytes, find_context_blockquotes);
        find_context_cold_bench!(group, bytes, find_context_bullet_items);
        find_context_cold_bench!(group, bytes, find_context_ordered_items);

        group.finish();
    }
}

criterion_group!(benches, bench_standalone);
criterion_main!(benches);
