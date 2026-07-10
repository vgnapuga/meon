#![allow(dead_code)]
//! Shared corpus generators and reporting helpers for the meon-md benchmarks.
//!
//! Three corpora model deliberately different document shapes so a throughput
//! number can be read *in context* instead of as one headline figure:
//!
//! - `plain` — prose only, no markup. Exercises the fallback/text path and the
//!   line loop with almost no element emission. This is the "ceiling" case:
//!   nearly pure scanning.
//! - `hot`   — light, evenly-spread markup (roughly one of each common inline
//!   element per paragraph). Representative of typical real documents.
//! - `heavy` — dense markup: headings, rules, quotes, fences, lists and nested
//!   inline together. Exercises every rule family at once; the "stress" case.
//!   This corpus specifically includes nested constructs (a doubly-nested
//!   blockquote line, and a bold span containing a nested italic) so the
//!   measurement also reflects the engine's bounded-nesting stack machinery
//!   (`max_nest = 4` in this grammar), not only flat element density — a
//!   purely flat heavy corpus would never touch that code path at all.
//!
//! `REPEAT_COUNT` tiles each base document so the working set is large enough
//! for a stable measurement and clearly larger than L2/L3.

pub const REPEAT_COUNT: usize = 10;

// Criterion knobs. Tuned for a sane local run time while staying statistically
// stable. For publication-grade numbers raise SAMPLE_SIZE / SAMPLE_TIME.
pub const WARMUP_TIME: u64 = 3;
pub const SAMPLE_TIME: u64 = 10;
pub const SAMPLE_SIZE: usize = 20;

pub fn doc_plain() -> String {
    let mut doc = String::new();
    for i in 0..2000 {
        doc.push_str(&format!(
            "This is a plain text paragraph number {} with no formatting whatsoever.\n",
            i
        ));
        doc.push_str(&format!(
            "Another sentence in paragraph {} continuing with more plain words here.\n",
            i
        ));
    }
    doc
}

pub fn doc_hot() -> String {
    let mut doc = String::new();
    for i in 0..500 {
        doc.push_str(&format!("## Section {}\n", i));
        doc.push_str(&format!(
            "Plain text paragraph with **bold {}** and *italic {}* words.\n",
            i, i
        ));
        doc.push_str(&format!(
            "Another line with some [text](url) plain text and `code {}` and <link> inline.\n",
            i
        ));
        doc.push('\n');
    }
    doc
}

pub fn doc_heavy() -> String {
    let mut doc = String::new();
    for i in 0..200 {
        doc.push_str(&format!("# Title {}\n", i));
        doc.push_str("---\n");
        doc.push_str(&format!(
            "Text with key = value\n**bold** and *italic* [text](url) and ***bold italic*** and `code` all together {}.  ",
            i
        ));
        doc.push_str(&format!(
            "> Blockquote <link> with ![text](url) **bold {}** and *italic* content here.\n",
            i
        ));
        doc.push_str(&format!(
            "> Second [text](url) blockquote <link> line with `code {}` here.\n",
            i
        ));
        // Nested blockquote: continues the still-open `>` continuation from
        // the two lines above and opens a *second* level on top of it,
        // exercising the block-level bounded stack (`cont` nesting) rather
        // than just a flat single-level continuation.
        doc.push_str(&format!(
            "> > Nested quote note {} stays under the same outer quote with *italic* style.\n",
            i
        ));
        doc.push_str("```rust\n");
        doc.push_str(&format!("fn function_{}() {{ let x = 42; }}\n", i));
        doc.push_str("```\n");
        doc.push_str(&format!(
            "- Bullet item **bold {}** with *italic* text\n",
            i
        ));
        doc.push_str(&format!("- Second bullet `code {}` item here\n", i));
        doc.push_str(&format!("- Third bullet ***bold italic {}*** item\n", i));
        doc.push_str(&format!(
            "{}. Ordered item with **bold** content\n",
            i % 9 + 1
        ));
        doc.push_str(&format!("{}. Second ordered `code` item here\n", i % 9 + 2));
        doc.push_str(&format!(
            "Mixed line **bold** then *italic* then `code` then ***bi*** end {}.\n",
            i
        ));
        // Nested emphasis: a different-count inner delimiter (`*`) inside an
        // outer `**` pair, exercising the inline-level bounded symmetric
        // stack rather than two independently-opened-and-closed spans.
        doc.push_str(&format!(
            "Nested emphasis check {}: **outer bold *inner italic* still outer bold** done.\n",
            i
        ));
        doc.push_str("~~~python\n");
        doc.push_str(&format!("def func_{}(): pass\n", i));
        doc.push_str("~~~\n");
        doc.push('\n');
    }
    doc
}

/// All benchmark corpora, already tiled `REPEAT_COUNT` times.
pub fn corpora() -> Vec<(&'static str, String)> {
    vec![
        // ("plain", doc_plain().repeat(REPEAT_COUNT)),
        ("hot", doc_hot().repeat(REPEAT_COUNT)),
        ("heavy", doc_heavy().repeat(REPEAT_COUNT)),
    ]
}

/// Parse `bytes` once and print a size + element-composition report.
///
/// Printed *before* the timed run so every throughput figure can be tied to the
/// exact amount and kind of structure the parser actually produced on that
/// input. This is the difference between "2.4 GiB/s" and "2.4 GiB/s while
/// emitting N headings, M bolds, ... over X MiB".
pub fn report(label: &str, bytes: &[u8]) {
    use meon_md::MarkdownParser;
    let c = MarkdownParser::parse(bytes);

    // Order: block/line structure first, then inline, then text runs.
    let counts: [(&str, usize); 15] = [
        ("headings", c.headings.len()),
        ("thematic_breaks", c.thematic_breaks.len()),
        ("paragraphs", c.paragraphs.len()),
        ("blockquotes", c.blockquotes.len()),
        ("fenced_codes", c.fenced_codes.len()),
        ("bullet_items", c.bullet_items.len()),
        ("ordered_items", c.ordered_items.len()),
        ("bolds", c.bolds.len()),
        ("italics", c.italics.len()),
        ("bold_italics", c.bold_italics.len()),
        ("codes", c.codes.len()),
        ("links", c.links.len()),
        ("autolinks", c.autolinks.len()),
        ("hard_breaks", c.hard_breaks.len()),
        ("texts", c.texts.len()),
    ];

    let total: usize = counts.iter().map(|(_, n)| n).sum();
    let len = bytes.len();
    let mib = len as f64 / (1024.0 * 1024.0);
    let density = total as f64 / (len as f64 / 1024.0);
    // Span is 2×u32 = 8 bytes; lower bound, since Vec<(T, Span)> entries are
    // larger. Gives a feel for output footprint relative to input size.
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
