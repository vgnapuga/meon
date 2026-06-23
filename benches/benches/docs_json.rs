#![allow(dead_code)]
//! JSON benchmark corpora for `meon-json`.
//!
//! # Why these five
//!
//! The previous corpora (`plain`/`hot`/`heavy`) were three variations of the
//! same shape — `format!`-built records — so all three stressed the *same*
//! engine path (key_value finalisation + scalar routing) at slightly different
//! densities. They could not tell apart "the kv path got slower" from "the
//! string scan got slower" from "the nesting stack got slower", because every
//! corpus exercised all of them at once in the same proportion.
//!
//! These five are each dominated by **one** axis of work, so a regression (or
//! a win) shows up in the corpus that owns that axis and stays flat in the
//! others — turning the bench suite into a crude profiler of its own:
//!
//! | corpus           | dominant cost                              | stresses |
//! |------------------|--------------------------------------------|----------|
//! | `flat_records`   | `key_value` finalisation, scalar routing   | members/scalars push rate |
//! | `wide_strings`   | the symmetric-string forward `memchr` scan | string body skipping |
//! | `number_heavy`   | scalar first-byte routing on bare arrays   | array-element typing in the close cascade |
//! | `deep_nest`      | the unified stack (`frames`/`fdepth`)      | push/pop depth, container back-patching |
//! | `mixed_realistic`| a balanced real-API payload                | everything, in realistic proportion |
//!
//! # Sizing
//!
//! Each `doc_*` returns one self-contained document. `corpora()` repeats it
//! `REPEAT_COUNT` times **as independent sibling documents inside one outer
//! array**, not via `String::repeat` (which concatenated whole top-level
//! documents back-to-back — invalid JSON that only `meon`'s `loose` fallback
//! tolerated, and unrepresentative of any real input). Wrapping the repeats in
//! one array keeps the whole corpus a single valid document while still
//! scaling the byte volume for cache-pressure measurement.

pub const REPEAT_COUNT: usize = 10;
pub const WARMUP_TIME: u64 = 3;
pub const SAMPLE_TIME: u64 = 10;
pub const SAMPLE_SIZE: usize = 200;

// --------------------------------------------------------------------------
// flat_records — array of shallow objects. The "API list response" shape.
//
// Dominated by key_value finalisation and scalar routing: every object is a
// burst of `"k":v` pairs whose values are bare scalars (number / bool), so the
// hot path is the kv pre-check + scalar first-byte match firing once per pair,
// thousands of times. Minimal nesting (depth 2), short strings. This is the
// closest analogue to the old `plain`, deliberately kept so a comparison point
// survives, but named for what it actually stresses.
// --------------------------------------------------------------------------
pub fn doc_flat_records() -> String {
    let mut doc = String::with_capacity(64 * 1024);
    doc.push_str("{\"records\":[");
    for i in 0..2000 {
        if i > 0 {
            doc.push(',');
        }
        // 6 bare-scalar values per record: 2 ints, 1 float, 2 bools, 1 null.
        doc.push_str(&format!(
            "{{\"id\":{},\"seq\":{},\"ratio\":{:.3},\"ok\":true,\"stale\":false,\"note\":null}}",
            i,
            i * 7,
            i as f64 * 0.01
        ));
    }
    doc.push_str("]}");
    doc
}

// --------------------------------------------------------------------------
// wide_strings — objects whose values are long string bodies.
//
// Dominated by the symmetric-string rule's forward `memchr` to the closing
// quote: each value is a long run of non-structural bytes that the engine must
// skip as one opaque string, not scan byte-by-byte. This isolates string-body
// throughput from structural density — few members, but most bytes live inside
// quotes. A regression in how strings are skipped shows up here and nowhere
// else. Includes some escaped quotes so the escape-aware close search is on the
// hot path too.
// --------------------------------------------------------------------------
pub fn doc_wide_strings() -> String {
    let mut doc = String::with_capacity(64 * 1024);
    doc.push_str("{\"documents\":[");
    let lorem = "the quick brown fox jumps over the lazy dog while \
                 carrying a \\\"quoted\\\" payload and assorted punctuation, \
                 commas; colons: and other bytes that are not structural ";
    for i in 0..400 {
        if i > 0 {
            doc.push(',');
        }
        // One short key, one very long string value (~250+ bytes each).
        doc.push_str(&format!("{{\"id\":{},\"body\":\"{}{}\"}}", i, lorem, lorem));
    }
    doc.push_str("]}");
    doc
}

// --------------------------------------------------------------------------
// number_heavy — large flat arrays of bare numbers. Telemetry / vectors.
//
// Dominated by array-element scalar routing: every element is a bare number
// typed at its `,` separator (and the last at `]`) in the close cascade's
// per-element path. No keys, no strings, minimal object overhead — just the
// element-routing loop firing on a dense stream of numbers. This is the array
// analogue of `flat_records`' member routing, isolated from the kv machinery.
// --------------------------------------------------------------------------
pub fn doc_number_heavy() -> String {
    let mut doc = String::with_capacity(64 * 1024);
    doc.push_str("{\"series\":[");
    for s in 0..40 {
        if s > 0 {
            doc.push(',');
        }
        doc.push_str("{\"samples\":[");
        for j in 0..200 {
            if j > 0 {
                doc.push(',');
            }
            // Mix of ints, negatives and floats — all route to `nums` by first
            // byte (digit or '-'), exercising both scalar arms.
            if j % 3 == 0 {
                doc.push_str(&format!("-{}", j));
            } else if j % 3 == 1 {
                doc.push_str(&format!("{}.{}", j, s));
            } else {
                doc.push_str(&format!("{}", j * 31));
            }
        }
        doc.push_str("]}");
    }
    doc.push_str("]}");
    doc
}

// --------------------------------------------------------------------------
// deep_nest — a deeply nested container spine with small leaves.
//
// Dominated by the unified stack: each level is a fresh container push, and the
// closing run pops them all in a cascade, back-patching every placeholder span.
// Leaves are tiny, so structural push/pop and depth bookkeeping dominate over
// scalar/string work. This is the corpus that would catch a regression in
// `frames`/`fdepth` handling or in the close cascade — invisible to the flat
// corpora, which never nest past depth 2-3. Depth stays under the grammar's
// `max_nest = 64`.
// --------------------------------------------------------------------------
pub fn doc_deep_nest() -> String {
    let mut doc = String::with_capacity(16 * 1024);
    const DEPTH: usize = 40;
    // Build a left-leaning spine: {"c":{"c":{"c": ... [leaves] ... }}}
    for _ in 0..DEPTH {
        doc.push_str("{\"c\":");
    }
    // A small mixed leaf array at the bottom, so the innermost level still has
    // real element work, then unwind.
    doc.push_str("[1,true,null,\"x\",[2,3,[4,[5]]]]");
    for _ in 0..DEPTH {
        doc.push('}');
    }
    doc
}

// --------------------------------------------------------------------------
// mixed_realistic — a payload shaped like a real API response.
//
// Objects with a mix of: short scalar fields, a couple of string fields, a
// nested metadata object, and a small array of heterogeneous elements. No
// single axis dominates — this is the "does the whole thing hold together at a
// realistic blend" corpus, and the one whose absolute number is the most
// meaningful proxy for real-world throughput. The others isolate; this one
// integrates.
// --------------------------------------------------------------------------
pub fn doc_mixed_realistic() -> String {
    let mut doc = String::with_capacity(64 * 1024);
    doc.push_str("{\"results\":[");
    for i in 0..500 {
        if i > 0 {
            doc.push(',');
        }
        doc.push_str(&format!(
            "{{\
\"id\":{},\
\"user\":\"user_{}\",\
\"email\":\"user{}@example.com\",\
\"active\":true,\
\"score\":{:.2},\
\"meta\":{{\"created\":\"2024-01-{:02}\",\"flags\":{},\"verified\":false}},\
\"tags\":[\"alpha\",\"beta\",{}],\
\"parent\":null\
}}",
            i,
            i,
            i,
            i as f64 * 0.5,
            (i % 28) + 1,
            i * 3,
            i
        ));
    }
    doc.push_str("]}");
    doc
}

/// Repeat one document `REPEAT_COUNT` times as independent siblings inside a
/// single outer array, keeping the whole corpus one valid JSON document while
/// scaling byte volume. (Contrast the old `doc.repeat(N)`, which produced N
/// concatenated top-level documents — not valid JSON.)
fn scale(doc: &str) -> String {
    let mut out = String::with_capacity(doc.len() * REPEAT_COUNT + REPEAT_COUNT + 4);
    out.push('[');
    for i in 0..REPEAT_COUNT {
        if i > 0 {
            out.push(',');
        }
        out.push_str(doc);
    }
    out.push(']');
    out
}

pub fn corpora() -> Vec<(&'static str, String)> {
    vec![
        ("flat_records", scale(&doc_flat_records())),
        ("wide_strings", scale(&doc_wide_strings())),
        ("number_heavy", scale(&doc_number_heavy())),
        ("deep_nest", scale(&doc_deep_nest())),
        ("mixed_realistic", scale(&doc_mixed_realistic())),
    ]
}

pub fn report(label: &str, bytes: &[u8]) {
    use meon_json::JsonParser;
    let c = JsonParser::parse(bytes);
    let counts: [(&str, usize); 6] = [
        ("objects", c.objects.len()),
        ("arrays", c.arrays.len()),
        ("strings", c.strings.len()),
        ("members", c.members.len()),
        ("scalars", c.scalars.len()),
        ("loose", c.loose.len()),
    ];
    let total: usize = counts.iter().map(|(_, n)| n).sum();
    let len = bytes.len();
    let mib = len as f64 / (1024.0 * 1024.0);
    let density = total as f64 / (len as f64 / 1024.0);
    const SPAN_BYTES: usize = 8;
    let span_bytes = total * SPAN_BYTES;
    let overhead = span_bytes as f64 / len as f64 * 100.0;
    println!("\n┌─ corpus: {label}");
    println!("│  size:      {mib:>8.2} MiB  ({len} bytes)");
    println!("│  elements:  {total:>8}     ({density:.1} per KiB)");
    println!(
        "│  span mem:  {:>8.2} MiB  (~{overhead:.1}% of input, 8 B/span lower bound)",
        span_bytes as f64 / (1024.0 * 1024.0)
    );
    println!("│");
    for chunk in counts.chunks(3) {
        let row: String = chunk
            .iter()
            .map(|(name, n)| format!("{name:>16}: {n:>9}"))
            .collect::<Vec<_>>()
            .join("   ");
        println!("│  {row}");
    }
    println!("└─");
}
