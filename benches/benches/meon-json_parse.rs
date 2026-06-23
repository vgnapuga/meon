mod docs_json;

use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use meon_json::JsonParser;
use std::time::Duration;

fn bench_parse(c: &mut Criterion) {
    for (label, doc) in docs_json::corpora() {
        let bytes = doc.as_bytes();

        docs_json::report(label, bytes);

        let mut group = c.benchmark_group(format!("parse/{label}"));
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.sample_size(docs_json::SAMPLE_SIZE);
        group.measurement_time(Duration::from_secs(docs_json::SAMPLE_TIME));
        group.warm_up_time(Duration::from_secs(docs_json::WARMUP_TIME));

        group.bench_function("full", |b| {
            b.iter(|| {
                let content = JsonParser::parse(black_box(bytes));
                black_box(content);
            });
        });

        group.finish();
    }
}

criterion_group!(benches, bench_parse);
criterion_main!(benches);
