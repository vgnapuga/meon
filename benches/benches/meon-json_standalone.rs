//! Standalone (`find_*`) and context-aware (`find_context_*`) iterator
//! benchmark for `meon-json` — structural only, no typing.
//!
//! Each **context-free** standalone iterator scans the raw source for a
//! single element kind with no cross-element context, so it can — by
//! design — yield a different count than the full parse. For JSON this gap
//! is *wider* than for the flat markdown grammar: `find_objects` /
//! `find_arrays` match only their exact declared delimiter and do **not**
//! track nesting the way `JsonParser::parse` does, and are additionally
//! vulnerable to a bracket that lives inside a string's content — the
//! context-free close search cannot tell it apart from a real one.
//!
//! Each **context-aware** iterator additionally consults a
//! [`meon::ParseContext`] — built by `JsonParser::context` from the
//! grammar's only opaque rule, `strings` — and skips any candidate whose
//! position falls inside a string. This closes the string-collision gap
//! but not the nesting gap: `find_context_objects` / `find_context_arrays`
//! still match only the exact delimiter and remain nesting-insensitive.
//! Only `objects` and `arrays` get a `find_context_*` method at all —
//! `strings` is itself the source of the context, and `members`
//! (`key_value`) is not covered — see `meon::define_parser!`'s docs.
//!
//! # Two context-aware numbers, not one
//!
//! `ParseContext::build` is a real, separately-timed cost (the `context`
//! bench below). Whether it belongs in a *single* context-aware call's
//! budget depends entirely on how many context-aware fields you sweep with
//! it — and this bench reports **both** scenarios rather than picking one:
//!
//! - **`find_context_*` (amortized)** — the context is built **once**,
//!   outside the timed loop, and reused by the scan. Correct number when
//!   sweeping **both** `objects` and `arrays` over the same source: the
//!   build is paid once and shared.
//! - **`find_context_*_cold` (single-sweep lower bound)** — `context()` is
//!   rebuilt **inside** the timed loop, every iteration, immediately
//!   before the one scan that uses it. This is the honest cost when you
//!   only need **one** context-aware field — there is no second sweep to
//!   amortize the build against. Do not read the amortized number as a
//!   substitute for this one.
//!
//! Rule of thumb: `_cold ≈ context + find_context_*` (amortized); the two
//! numbers should sum consistently.
//!
//! This bench prints, per corpus, the full-parse count against both the
//! context-free standalone count and the context-aware count, in the same
//! `full=... N=...` shape for both tables. The point of every sweep in this
//! file — context-free and context-aware alike — is the raw per-kind scan
//! rate, not structural fidelity; use the full parse when you need correct
//! containment.
//!
//! This is the structural counterpart to `meon-json_parse`'s `typed` line:
//! there is **no** `type_scalars` post-pass here at all. The composition
//! header (`docs_json::report`) is shared with the parse/compare benches,
//! so all of these are read against the same inputs.
//!
//! Fairness: input `black_box`-ed; every iterator is fully driven and each
//! item `black_box`-ed so the scan is not optimised away; the `context()`
//! build is timed the same way, its result `black_box`-ed;
//! `Throughput::Bytes` gives GiB/s comparable across feature builds and
//! across the JSON benches.

mod docs_json;

use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use meon_json::JsonParser;
use std::time::Duration;

macro_rules! find_bench {
    ($group:ident, $bytes:ident, $full:ident, $method:ident => $field:ident) => {{
        let standalone_count = JsonParser::$method($bytes).count();
        println!(
            "    {:<14} full={:>9}  standalone={:>9}",
            stringify!($method),
            $full.$field.len(),
            standalone_count,
        );
        $group.bench_function(stringify!($method), |b| {
            b.iter(|| {
                JsonParser::$method(black_box($bytes)).for_each(|x| {
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
        let context_count = JsonParser::$method($bytes, &$ctx).count();
        println!(
            "    {:<14} full={:>9}  context-aware={:>9}",
            stringify!($method),
            $full.$field.len(),
            context_count,
        );
        $group.bench_function(stringify!($method), |b| {
            b.iter(|| {
                JsonParser::$method(black_box($bytes), &$ctx).for_each(|x| {
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
                let ctx = JsonParser::context(black_box($bytes));
                JsonParser::$method(black_box($bytes), &ctx).for_each(|x| {
                    black_box(x);
                })
            })
        });
    }};
}

fn bench_standalone(c: &mut Criterion) {
    for (label, doc) in docs_json::corpora() {
        let bytes = doc.as_bytes();

        // Size + composition header (same report as the parse/compare benches).
        docs_json::report(label, bytes);

        // Reference counts from one full parse, for full-vs-standalone deltas.
        let full = JsonParser::parse(bytes);
        println!("│  full-vs-standalone counts:");

        let mut group = c.benchmark_group(format!("json-standalone/{label}"));
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.sample_size(docs_json::SAMPLE_SIZE);
        group.measurement_time(Duration::from_secs(docs_json::SAMPLE_TIME));
        group.warm_up_time(Duration::from_secs(docs_json::WARMUP_TIME));

        find_bench!(group, bytes, full, find_objects => objects);
        find_bench!(group, bytes, full, find_arrays  => arrays);
        find_bench!(group, bytes, full, find_strings => strings);
        find_bench!(group, bytes, full, find_members => members);

        // One-time context build, reused by every amortized find_context_*
        // call below. Timed on its own: this is the number the "_cold"
        // benches re-pay on every single iteration instead.
        let ctx = JsonParser::context(bytes);
        println!("│  context regions (strings): {}", ctx.spans().len());
        group.bench_function("context", |b| {
            b.iter(|| {
                black_box(JsonParser::context(black_box(bytes)));
            })
        });

        println!("│  full-vs-context-aware counts:");
        find_context_bench!(group, bytes, ctx, full, find_context_objects => objects);
        find_context_bench!(group, bytes, ctx, full, find_context_arrays  => arrays);

        // Single-sweep (cold) cost: context() rebuilt every iteration,
        // immediately before the one scan that uses it. The real number for
        // "I only need this one context-aware field."
        find_context_cold_bench!(group, bytes, find_context_objects);
        find_context_cold_bench!(group, bytes, find_context_arrays);

        group.finish();
    }
}

criterion_group!(benches, bench_standalone);
criterion_main!(benches);
