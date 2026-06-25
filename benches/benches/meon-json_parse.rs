//! Intra-engine JSON parse benchmark.
//!
//! Two lines per corpus, both `black_box`-ed end to end:
//! - `structural` — `JsonParser::parse`: the flat span table only (no scalar
//!   typing, no number parsing, no string unescaping).
//! - `typed`      — `parse` + `JsonContent::type_scalars`: first-byte scalar
//!   classification on top of the structural pass.
//!
//! This is an intra-engine benchmark (meon-json against itself). For the
//! cross-parser comparison against simd-json and others see `meon-json_compare`.

mod docs_json;

use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use meon_json::JsonParser;
use std::time::Duration;

fn bench_parse(c: &mut Criterion) {
    for (label, doc) in docs_json::corpora() {
        let bytes = doc.as_bytes();

        docs_json::report(label, bytes);

        let mut group = c.benchmark_group(format!("json-parse/{label}"));
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.sample_size(docs_json::SAMPLE_SIZE);
        group.measurement_time(Duration::from_secs(docs_json::SAMPLE_TIME));
        group.warm_up_time(Duration::from_secs(docs_json::WARMUP_TIME));

        group.bench_function("structural", |b| {
            b.iter(|| {
                let content = JsonParser::parse(black_box(bytes));
                black_box(content);
            });
        });

        group.bench_function("typed", |b| {
            b.iter(|| {
                let content = JsonParser::parse(black_box(bytes));
                let typed = content.type_scalars();
                black_box(typed);
            });
        });

        group.finish();
    }
}

criterion_group!(benches, bench_parse);
criterion_main!(benches);
