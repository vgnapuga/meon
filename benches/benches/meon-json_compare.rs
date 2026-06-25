//! Cross-parser JSON throughput comparison.
//!
//! Four lines per corpus, each `black_box`-ed:
//! - `meon-structural` — `JsonParser::parse`: flat span table only.
//! - `meon-typed`      — `parse` + `type_scalars`: + first-byte scalar typing.
//! - `simd-json`       — `to_tape`: Stage 1 + Stage 2 (structural + number
//!   parsing + string unescaping) in one pass.
//! - `sonic-rs`        — `from_slice::<Value>`: full parse into an owned value.
//!
//! THIS IS NOT A LEADERBOARD. The four do **different amounts of work**:
//! `meon-structural` emits spans and validates nothing; `meon-typed` adds only
//! first-byte classification; `simd-json` and `sonic-rs` validate, parse every
//! number, and unescape every string. `Throughput::Bytes` measures ingest of
//! different work. See `benches/JSON_COMPARE.md` for the fairness frame.
//!
//! `to_tape` unescapes in place, so simd-json gets a fresh clone per iteration
//! (excluded from the timed region via `iter_batched`); the other three read
//! the original immutable bytes.

mod docs_json;

use criterion::{BatchSize, Criterion, Throughput, black_box, criterion_group, criterion_main};
use meon_json::JsonParser;
use std::time::Duration;

fn bench_compare(c: &mut Criterion) {
    for (label, doc) in docs_json::corpora() {
        let bytes = doc.as_bytes();

        docs_json::report(label, bytes);

        // One-time correctness gate, outside any timed region: the validating
        // parsers must accept the corpus. If a corpus is ever made invalid,
        // every simd-json / sonic-rs number for it would silently measure an
        // error path instead of a parse. Fail loudly here.
        {
            let mut check = bytes.to_vec();
            assert!(
                simd_json::to_tape(&mut check).is_ok(),
                "simd_json rejected corpus {label:?}"
            );
            assert!(
                sonic_rs::from_slice::<sonic_rs::Value>(bytes).is_ok(),
                "sonic_rs rejected corpus {label:?}"
            );
        }

        let mut group = c.benchmark_group(format!("json-compare/{label}"));
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.sample_size(docs_json::SAMPLE_SIZE);
        group.measurement_time(Duration::from_secs(docs_json::SAMPLE_TIME));
        group.warm_up_time(Duration::from_secs(docs_json::WARMUP_TIME));

        // meon-json: structural span table only.
        group.bench_function("meon-structural", |b| {
            b.iter(|| {
                let content = JsonParser::parse(black_box(bytes));
                black_box(content);
            });
        });

        // meon-json: structural + first-byte scalar typing.
        group.bench_function("meon-typed", |b| {
            b.iter(|| {
                let content = JsonParser::parse(black_box(bytes));
                let typed = content.type_scalars();
                black_box(typed);
            });
        });

        // simd-json: full Stage 1 + Stage 2 into a tape. Mutates the buffer in
        // place, so each iteration is handed a fresh clone; the clone is the
        // `iter_batched` setup and is NOT timed.
        group.bench_function("simd-json", |b| {
            b.iter_batched(
                || bytes.to_vec(),
                |mut buf| {
                    let tape = simd_json::to_tape(black_box(&mut buf));
                    _ = black_box(tape);
                },
                BatchSize::SmallInput,
            );
        });

        // sonic-rs: full parse into an owned `Value`.
        group.bench_function("sonic-rs", |b| {
            b.iter(|| {
                let value = sonic_rs::from_slice::<sonic_rs::Value>(black_box(bytes));
                _ = black_box(value);
            });
        });

        group.finish();
    }
}

criterion_group!(benches, bench_compare);
criterion_main!(benches);
