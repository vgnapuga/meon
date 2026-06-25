//! Standalone (`find_*`) iterator benchmark for `meon-json` — structural only,
//! no typing.
//!
//! Each standalone iterator scans the raw source for a single element kind with
//! no cross-element context, so it can — by design — yield a different count
//! than the full parse. For JSON this gap is *wider* than for the flat markdown
//! grammar: `find_objects` / `find_arrays` match only their exact declared
//! delimiter and do **not** track nesting the way `JsonParser::parse` does, and
//! `find_strings` / `find_members` are likewise nesting-insensitive. The point
//! of this bench is the raw per-kind scan rate, not structural fidelity — reach
//! for `find_*` only for a single nesting-insensitive sweep (e.g. "every string
//! in the document"); use the full parse when you need correct containment.
//!
//! This is the structural counterpart to `meon-json_parse`'s `typed` line:
//! there is **no** `type_scalars` post-pass here at all. The composition header
//! (`docs_json::report`) is shared with the parse/compare benches, so all three
//! are read against the same inputs.
//!
//! Fairness: input `black_box`-ed; each iterator is fully driven and every item
//! `black_box`-ed so the scan is not optimised away; `Throughput::Bytes` gives
//! GiB/s comparable across feature builds and across the JSON benches.

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

        group.finish();
    }
}

criterion_group!(benches, bench_standalone);
criterion_main!(benches);
