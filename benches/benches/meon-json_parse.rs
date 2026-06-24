mod docs_json;

use criterion::{BatchSize, Criterion, Throughput, black_box, criterion_group, criterion_main};
use meon_json::JsonParser;
use std::time::Duration;

/// # Scope of this benchmark — read before trusting the numbers
///
/// Two distinct things are measured here for `meon`, and one for
/// `simd-json`, and they are **not** all doing the same amount of work:
///
/// - `meon-structural` — `JsonParser::parse` alone. This is meon-json's
///   structural pass only: spans for objects/arrays/strings/members, no
///   scalar typing, no string unescaping, no number validation. In
///   simd-json's own terminology this is roughly equivalent to *Stage 1*
///   (structural indexing) alone.
/// - `meon-typed` — `JsonParser::parse` followed by `JsonContent::type_scalars`.
///   This adds first-byte scalar classification (number / true / false /
///   null) on top of the structural pass — closer to simd-json's *Stage 2*
///   in spirit, but **still does not unescape string content** the way
///   simd-json's tape does. This is the fairer of the two to compare
///   against `simd-json`, but still not byte-for-byte the same work.
/// - `simd-json-tape` — `simd_json::to_tape`. This is simd-json's full
///   Stage 1 *and* Stage 2: structural indexing, number parsing/validation,
///   AND string unescaping, all in one call. There is no public simd-json
///   API at this level that does Stage 1 alone, so a perfectly
///   apples-to-apples comparison isn't available — `meon-typed` is the
///   closest honest approximation, not an exact match.
///
/// If you only look at one comparison, use `meon-typed` vs `simd-json-tape`,
/// and keep in mind simd-json is still doing strictly more work (string
/// unescaping) that has no `meon` equivalent on either line above.
///
/// # The simd-json buffer
///
/// `simd_json::to_tape` takes `&mut [u8]` because simd-json's Stage 2
/// unescapes string content **in place**, overwriting bytes inside the same
/// buffer it was given. Calling it again on that same, now-mutated buffer
/// does not parse the original document a second time — it parses whatever
/// Stage 2 already turned it into. `iter_batched` below gives every
/// iteration its own fresh clone of the original bytes specifically to
/// avoid this; the clone itself is excluded from the timed region.
fn bench_parse(c: &mut Criterion) {
    for (label, doc) in docs_json::corpora() {
        let bytes = doc.as_bytes();
        docs_json::report(label, bytes);

        // One-time correctness check, outside any timed region: if this
        // ever fails, every `simd-json-tape` number below is meaningless —
        // either every iteration silently measured an error path, or it was
        // parsing already-mutated bytes. Fail loudly here instead of
        // discovering it by eyeballing suspiciously fast numbers later.
        {
            let mut check_buf = bytes.to_vec();
            let result = simd_json::to_tape(&mut check_buf);
            assert!(
                result.is_ok(),
                "simd_json::to_tape failed on a fresh, unmutated copy of \
                 corpus {label:?} — fix the input before trusting any \
                 simd-json-tape benchmark number for it: {:?}",
                result.err()
            );
        }

        let mut group = c.benchmark_group(format!("parse/{label}"));
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.sample_size(docs_json::SAMPLE_SIZE);
        group.measurement_time(Duration::from_secs(docs_json::SAMPLE_TIME));
        group.warm_up_time(Duration::from_secs(docs_json::WARMUP_TIME));

        // Structural pass only — see the module doc comment above for what
        // this does and does not do relative to simd-json's stages.
        group.bench_function("meon-structural", |b| {
            b.iter(|| {
                let content = JsonParser::parse(black_box(bytes));
                black_box(content);
            });
        });

        // Structural pass + first-byte scalar typing — the fairer of the
        // two `meon` variants to set against `simd-json-tape`, though still
        // not unescaping strings the way simd-json does.
        group.bench_function("meon-typed", |b| {
            b.iter(|| {
                let content = JsonParser::parse(black_box(bytes));
                let typed = content.type_scalars();
                black_box(typed);
            });
        });

        group.bench_function("simd-json-tape", |b| {
            b.iter_batched(
                || bytes.to_vec(),
                |mut buf| {
                    // `Tape<'_>` borrows from `buf`, and `buf` is owned by —
                    // and dies at the end of — this closure, so `tape` can
                    // never be returned out of it (a borrow can't outlive
                    // what it borrows from). `black_box` here as a
                    // statement, not a tail expression, still forces the
                    // call to actually happen (that's `black_box`'s whole
                    // job — it does not need its result kept alive to do
                    // that); the closure itself returns `()`.
                    let tape = simd_json::to_tape(black_box(&mut buf));
                    _ = black_box(tape);
                },
                BatchSize::SmallInput,
            );
        });

        group.finish();
    }
}

criterion_group!(benches, bench_parse);
criterion_main!(benches);
