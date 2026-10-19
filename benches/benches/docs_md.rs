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
            "Another line with some [text](url) plain text and `code {}` and <https://link> inline.\n",
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
            "> Blockquote <https://link> with ![text](url) **bold {}** and *italic* content here.\n",
            i
        ));
        doc.push_str(&format!(
            "> Second [text](url) blockquote <https://link> line with `code {}` here.\n",
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
        // A blank line ends the list: without it CommonMark reads the next
        // line as a lazy continuation of the last item's paragraph.
        doc.push('\n');
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
        doc.push_str("```python\n");
        doc.push_str(&format!("def func_{}(): pass\n", i));
        doc.push_str("```\n");
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

/// Columns of the composition table printed by [`report`].
const COLS: usize = 3;

/// Element counts and plain-text bytes of the same input as seen by
/// pulldown-cmark, mapped onto meon-md's element kinds. The reference for the
/// marks in [`report`].
///
/// The mapping follows what each meon-md field holds:
///
/// - `paragraphs` - paragraphs outside block quotes and list items, whose
///   content meon-md keeps in `blockquotes` / the item spans instead.
/// - `bolds` / `italics` / `bold_italics` - `***x***` is one `bold_italics`
///   span in meon-md and an emphasis directly wrapping a strong (or the
///   reverse) in pulldown-cmark; such pairs count once, as `bold_italics`.
/// - `links` - links and images, autolinks excluded; `autolinks` - autolinks.
/// - `texts` - compared by covered bytes, not by count: meon-md merges the
///   lines of a paragraph into one span, pulldown-cmark splits text at every
///   line break. Text inside emphasis, links, images and code blocks is not
///   plain text in either parser. `eol` bytes are left out.
struct Reference {
    /// Count of every kind except `texts`, under the names [`report`] prints.
    counts: Vec<(&'static str, usize)>,
    text_bytes: Vec<bool>,
}

impl Reference {
    fn count(&self, kind: &str) -> Option<usize> {
        self.counts
            .iter()
            .find(|(k, _)| *k == kind)
            .map(|&(_, n)| n)
    }
}

/// Mark the bytes of `range` as plain text, `eol` bytes excepted. Both sides
/// of the `texts` comparison go through here, so they leave out the same bytes.
fn mark_text(bytes: &[u8], range: std::ops::Range<usize>, text_bytes: &mut [bool]) {
    for i in range {
        if bytes[i] != b'\n' {
            text_bytes[i] = true;
        }
    }
}

fn pulldown_reference(text: &str) -> Reference {
    use pulldown_cmark::{CodeBlockKind, Event, LinkType, Parser, Tag, TagEnd};

    // Delimiter width of an emphasis and of a strong.
    const EM: usize = 1;
    const STRONG: usize = 2;

    let mut headings = 0;
    let mut thematic_breaks = 0;
    let mut paragraphs = 0;
    let mut blockquotes = 0;
    let mut fenced_codes = 0;
    let mut bullet_items = 0;
    let mut ordered_items = 0;
    let mut strongs = 0;
    let mut emphases = 0;
    let mut bold_italics = 0;
    let mut codes = 0;
    let mut links = 0;
    let mut autolinks = 0;
    let mut hard_breaks = 0;
    let mut text_bytes = vec![false; text.len()];

    // Open block quotes and list items, open lists (`true` = ordered), open
    // inline elements whose text is not plain text, and an open code block.
    let mut containers = 0usize;
    let mut lists: Vec<bool> = Vec::new();
    let mut inline_depth = 0usize;
    let mut in_code_block = false;
    // The previous event, when it opened an emphasis or a strong: that
    // element's delimiter width and source range.
    let mut prev_open: Option<(usize, std::ops::Range<usize>)> = None;

    for (event, range) in Parser::new(text).into_offset_iter() {
        let opened = match &event {
            Event::Start(Tag::Emphasis) => Some(EM),
            Event::Start(Tag::Strong) => Some(STRONG),
            _ => None,
        };
        // An emphasis and a strong opened back to back, the inner one exactly
        // one outer delimiter in from each side: `***x***`.
        let wraps_directly = match (opened, &prev_open) {
            (Some(width), Some((outer, outer_range))) => {
                width != *outer
                    && range.start == outer_range.start + outer
                    && range.end + outer == outer_range.end
            }
            _ => false,
        };
        if wraps_directly {
            bold_italics += 1;
        }
        prev_open = opened.map(|w| (w, range.clone()));

        match event {
            Event::Start(Tag::Heading { .. }) => headings += 1,
            Event::Rule => thematic_breaks += 1,
            Event::Start(Tag::Paragraph) if containers == 0 => paragraphs += 1,
            Event::Start(Tag::BlockQuote(_)) => {
                blockquotes += 1;
                containers += 1;
            }
            Event::End(TagEnd::BlockQuote(_)) => containers -= 1,
            Event::Start(Tag::CodeBlock(kind)) => {
                if matches!(kind, CodeBlockKind::Fenced(_)) {
                    fenced_codes += 1;
                }
                in_code_block = true;
            }
            Event::End(TagEnd::CodeBlock) => in_code_block = false,
            Event::Start(Tag::List(start)) => lists.push(start.is_some()),
            Event::End(TagEnd::List(_)) => {
                lists.pop();
            }
            Event::Start(Tag::Item) => {
                if lists.last() == Some(&true) {
                    ordered_items += 1;
                } else {
                    bullet_items += 1;
                }
                containers += 1;
            }
            Event::End(TagEnd::Item) => containers -= 1,
            Event::Start(Tag::Strong) => {
                strongs += 1;
                inline_depth += 1;
            }
            Event::Start(Tag::Emphasis) => {
                emphases += 1;
                inline_depth += 1;
            }
            Event::Start(Tag::Link { link_type, .. }) => {
                if matches!(link_type, LinkType::Autolink | LinkType::Email) {
                    autolinks += 1;
                } else {
                    links += 1;
                }
                inline_depth += 1;
            }
            Event::Start(Tag::Image { .. }) => {
                links += 1;
                inline_depth += 1;
            }
            Event::End(TagEnd::Strong | TagEnd::Emphasis | TagEnd::Link | TagEnd::Image) => {
                inline_depth -= 1;
            }
            Event::Code(_) => codes += 1,
            Event::HardBreak => hard_breaks += 1,
            Event::Text(_) if inline_depth == 0 && !in_code_block => {
                mark_text(text.as_bytes(), range, &mut text_bytes);
            }
            _ => {}
        }
    }

    Reference {
        counts: vec![
            ("headings", headings),
            ("thematic_breaks", thematic_breaks),
            ("paragraphs", paragraphs),
            ("blockquotes", blockquotes),
            ("fenced_codes", fenced_codes),
            ("bullet_items", bullet_items),
            ("ordered_items", ordered_items),
            ("bolds", strongs - bold_italics),
            ("italics", emphases - bold_italics),
            ("bold_italics", bold_italics),
            ("codes", codes),
            ("links", links),
            ("autolinks", autolinks),
            ("hard_breaks", hard_breaks),
        ],
        text_bytes,
    }
}

/// Parse `bytes` with meon-md and print a size + element-composition report.
///
/// Printed *before* the timed run so every throughput figure can be tied to the
/// exact amount and kind of structure the parser actually produced on that
/// input. This is the difference between "2.4 GiB/s" and "2.4 GiB/s while
/// emitting N headings, M bolds, ... over X MiB".
///
/// The same input is also parsed with pulldown-cmark, and each count is
/// followed by `✓` or `✗`: whether the two parsers agree on that element kind
/// (see [`Reference`] for how pulldown-cmark's output is mapped). Every
/// disagreement is listed under the table: with both counts, or for `texts`
/// with the number of text bytes found by only one of the two. meon-md parses
/// a Markdown subset, not CommonMark, so a `✗` is not necessarily a meon-md
/// bug - it marks where the two parsers read the corpus differently.
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

    let reference = std::str::from_utf8(bytes).ok().map(pulldown_reference);
    let mut meon_text_bytes = vec![false; bytes.len()];
    for s in &c.texts {
        mark_text(
            bytes,
            s.start as usize..s.end as usize,
            &mut meon_text_bytes,
        );
    }
    // For every kind: `None` when pulldown-cmark could not read the corpus,
    // otherwise `None` on agreement or a description of the difference.
    let checks: Vec<Option<Option<String>>> = counts
        .iter()
        .map(|&(name, n)| {
            let r = reference.as_ref()?;
            Some(match r.count(name) {
                Some(p) => (p != n).then(|| format!("{name}: meon-md {n}, pulldown-cmark {p}")),
                None => {
                    let only = |a: &[bool], b: &[bool]| {
                        a.iter().zip(b).filter(|&(&x, &y)| x && !y).count()
                    };
                    let (m, p) = (
                        only(&meon_text_bytes, &r.text_bytes),
                        only(&r.text_bytes, &meon_text_bytes),
                    );
                    (m + p > 0).then(|| {
                        format!("{name}: {m} bytes only in meon-md, {p} only in pulldown-cmark")
                    })
                }
            })
        })
        .collect();

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
    let cells: Vec<String> = counts
        .iter()
        .zip(&checks)
        .map(|((name, n), check)| {
            let mark = match check {
                Some(None) => "√",
                Some(Some(_)) => "✗",
                None => " ",
            };
            format!("{name:>16}: {n:>9} {mark}")
        })
        .collect();
    for row in cells.chunks(COLS) {
        println!("│  {}", row.join("   "));
    }
    if reference.is_none() {
        println!("│");
        println!("│  pulldown-cmark: corpus is not UTF-8, not checked");
    }
    let differs: Vec<&String> = checks.iter().flatten().flatten().collect();
    if !differs.is_empty() {
        println!("│");
        println!("│  differs from pulldown-cmark:");
        for d in differs {
            println!("│    {d}");
        }
    }
    println!("└─");
}
