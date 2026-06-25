#![allow(dead_code)]
//! Shared corpus generators and reporting helpers for the meon-json benchmarks.
//!
//! Unlike the markdown corpora, a JSON corpus must be **one valid document**
//! (the comparators parse a single document, not a concatenation). Each profile
//! is therefore one top-level array whose element REPEAT_count scales with `REPEAT_COUNT`:
//!
//! - `numbers` — a flat array of numbers / bools / nulls. Scalar-heavy,
//!   near-pure scanning; the ceiling case. This is where meon's *structural*
//!   pass does least (it never parses a number) and a validating parser does
//!   most (it parses and validates every number).
//! - `objects` — an array of flat objects with mixed-typed fields
//!   (`id` / `name` / `active` / `ratio` / `tag`). A typical API payload.
//! - `nested` — an array of moderately nested objects (objects-in-objects,
//!   small arrays, an escaped string). Exercises the unified nesting stack and
//!   the string rule.

use meon_json::JsonParser;

/// Scales every corpus. `REPEAT_COUNT` top-level array elements (×30 for `numbers`,
/// which are cheaper per element). Raise it for a `big` run that exceeds cache.
pub const REPEAT_COUNT: usize = 10_000;

// Criterion knobs, shared with the markdown benches' values.
pub const WARMUP_TIME: u64 = 3;
pub const SAMPLE_TIME: u64 = 10;
pub const SAMPLE_SIZE: usize = 200;

pub fn doc_numbers() -> String {
    let mut s = String::with_capacity(REPEAT_COUNT * 30 * 8);
    s.push('[');
    for i in 0..REPEAT_COUNT * 30 {
        if i > 0 {
            s.push(',');
        }
        match i % 6 {
            0 => s.push_str(&i.to_string()),
            1 => {
                s.push('-');
                s.push_str(&i.to_string());
            }
            2 => s.push_str(&format!("{}.{}", i, i % 100)),
            3 => s.push_str("true"),
            4 => s.push_str("false"),
            _ => s.push_str("null"),
        }
    }
    s.push(']');
    s
}

pub fn doc_objects() -> String {
    let mut s = String::with_capacity(REPEAT_COUNT * 2 * 80);
    s.push('[');
    for i in 0..REPEAT_COUNT * 2 {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!(
            r#"{{"id":{i},"name":"item {i}","active":{},"ratio":{i}.5,"tag":null}}"#,
            if i % 2 == 0 { "true" } else { "false" }
        ));
    }
    s.push(']');
    s
}

pub fn doc_nested() -> String {
    let mut s = String::with_capacity(REPEAT_COUNT * 130);
    s.push('[');
    for i in 0..REPEAT_COUNT {
        if i > 0 {
            s.push(',');
        }
        // The `\\` inside this raw string is a literal backslash pair, i.e. a
        // valid JSON escaped backslash in the generated document.
        s.push_str(&format!(
            r#"{{"id":{i},"meta":{{"path":"C:\\dir\\f{i}","size":{i},"ok":true}},"tags":["a","b",{i}],"deep":{{"a":{{"b":{{"c":{i}}}}}}}}}"#
        ));
    }
    s.push(']');
    s
}

pub fn corpora() -> Vec<(&'static str, String)> {
    vec![
        ("numbers", doc_numbers()),
        ("objects", doc_objects()),
        ("nested", doc_nested()),
    ]
}

/// Parse once with meon-json and print a size + structural + typed composition
/// report, so every throughput figure can be read against the exact structure
/// the parser produced. Runs once, before timing.
pub fn report(label: &str, bytes: &[u8]) {
    let c = JsonParser::parse(bytes);
    let t = c.type_scalars();

    let size = bytes.len();
    let structural = c.objects.len() + c.arrays.len() + c.strings.len() + c.members.len();
    let per_kib = if size > 0 {
        structural as f64 / (size as f64 / 1024.0)
    } else {
        0.0
    };

    println!("┌─ corpus: {label}");
    println!(
        "│  size:        {:8.2} MiB  ({} bytes)",
        size as f64 / (1024.0 * 1024.0),
        size
    );
    println!(
        "│  structural:  {:8}     ({:.1} per KiB)",
        structural, per_kib
    );
    println!("│");
    println!(
        "│      objects: {:9}      arrays: {:9}     strings: {:9}",
        c.objects.len(),
        c.arrays.len(),
        c.strings.len()
    );
    println!(
        "│      members: {:9}     scalars: {:9}       loose: {:9}",
        c.members.len(),
        c.scalars.len(),
        c.loose.len()
    );
    println!(
        "│  typed: nums: {:9}       trues: {:9}      falses: {:9}     nulls: {:9}",
        t.nums.len(),
        t.trues.len(),
        t.falses.len(),
        t.nulls.len()
    );
    println!("└─");
}
