# meon-md

EN | [**RU**](https://github.com/vgnapuga/meon/blob/main/meon-md/README_RU.md) - *GitHub*

A fast flat parser for a subset of Markdown, built on the
[`meon`](https://github.com/vgnapuga/meon/blob/main/meon/README.md) declarative
parsing engine.

`meon-md` is both a ready-to-use Markdown parser and a reference grammar
demonstrating what `meon` can express in a single `define_parser!` invocation.

* **meon**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon/README.md)
  * [***crates.io***](https://crates.io/crates/meon)
* **meon-macros**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon-macros/README.md)
  * [***crates.io***](https://crates.io/crates/meon-macros)
* **meon-md**    <--
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon-md/README.md)
  * [***crates.io***](https://crates.io/crates/meon-md)

* [***ARCHITECTURE.md***](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE.md) - *GitHub*
* [***BENCHMARKS.md***](https://github.com/vgnapuga/meon/blob/main/benches/README.md) - *GitHub*
* [***FUZZING.md***](https://github.com/vgnapuga/meon/blob/main/fuzz/README.md) - *GitHub*

---

## Quick start

```toml
[dependencies]
meon-md = "0.1"
```

```rust
use meon_md::MarkdownParser;

let src = b"# Hello\n**world** and *italic* with `code`\n";
let c = MarkdownParser::parse(src);

// Access by element kind — O(1) per kind
println!("headings:  {}", c.headings.len());
println!("bolds:     {}", c.bolds.len());
println!("italics:   {}", c.italics.len());

// Resolve a span to a string slice
if let Some((_, span)) = c.headings.first() {
    println!("heading text: {}", c.str(*span).unwrap());
}
```

---

## Supported elements

### Inline

| Element       | Syntax                    | Field          | Type          |
|---------------|---------------------------|----------------|---------------|
| Plain text    | any unmatched bytes       | `texts`        | `Vec<Span>`   |
| Bold          | `**text**`                | `bolds`        | `Vec<Span>`   |
| Italic        | `*text*`                  | `italics`      | `Vec<Span>`   |
| Bold italic   | `***text***`              | `bold_italics` | `Vec<Span>`   |
| Inline code   | `` `code` ``              | `codes`        | `Vec<Span>`   |
| Link          | `[text](url)`             | `links`        | `Vec<Link>`   |
| Image         | `![alt](url)`             | `links`        | `Vec<Link>`   |
| Autolink      | `<url>`                   | `autolinks`    | `Vec<Span>`   |
| Hard break    | `\` or `·· ` at line end  | `hard_breaks`  | `Vec<Span>`   |

### Line

| Element         | Syntax              | Field             | Type                    |
|-----------------|---------------------|-------------------|-------------------------|
| Heading         | `# … ######`       | `headings`        | `Vec<(Heading, Span)>`  |
| Thematic break  | `---`, `***`, `___` | `thematic_breaks` | `Vec<(ThematicBreak, Span)>` |

### Block

| Element        | Syntax             | Field           | Type                      |
|----------------|--------------------|-----------------|---------------------------|
| Fenced code    | ` ``` … ``` `      | `fenced_codes`  | `Vec<Span>`               |
| Blockquote     | `> …`              | `blockquotes`   | `Vec<Span>`               |
| Bullet item    | `- / * / +`        | `bullet_items`  | `Vec<(BulletItem, Span)>` |
| Ordered item   | `1. / 1)`          | `ordered_items` | `Vec<(OrderedItem, Span)>`|
| Paragraph      | fallback           | `paragraphs`    | `Vec<Span>`               |

---

## Output types

```rust
// Span — a half-open byte range [start, end) into the source slice
pub struct Span { pub start: u32, pub end: u32 }

// Link — carries both text and url spans plus an image flag
pub struct Link {
    pub is_image: bool,
    pub text: Span,
    pub url:  Span,
}

// Heading — nesting level 1–6
pub struct Heading { pub level: NonZeroU8 }

// ThematicBreak — ASCII byte of the delimiter (b'-', b'*', or b'_')
pub struct ThematicBreak { pub kind: u8 }

// BulletItem — ASCII byte of the marker (b'-', b'*', or b'+')
pub struct BulletItem { pub kind: u8 }

// OrderedItem — parsed number and delimiter byte (b'.' or b')')
pub struct OrderedItem { pub kind: u8, pub num: u32 }
```

---

## Working with spans

The content struct borrows the original source. Use the built-in helpers to
resolve spans:

```rust
let src = b"**bold** and *italic*\n";
let c = MarkdownParser::parse(src);

// str() returns None on invalid UTF-8 instead of panicking
if let Some(text) = c.str(c.bolds[0]) {
    println!("bold: {text}");   // → "bold"
}

// bytes() for raw byte access
let raw: &[u8] = c.bytes(c.italics[0]);

// _clean() iterator — inner content without delimiter bytes
for text in c.bolds_clean() {
    println!("{}", std::str::from_utf8(text).unwrap());
}

// _raw() iterator — full slice including delimiter bytes
for raw in c.bolds_raw() {
    println!("{}", std::str::from_utf8(raw).unwrap());  // → "**bold**"
}
```

---

## Standalone iterators

Every element kind has a `find_*` method that scans the source without a full
parse. Use it when you only need one element kind from a large document:

```rust
use meon_md::MarkdownParser;

let src = long_document.as_bytes();

// ~2–5× faster than a full parse for a single element kind
for span in MarkdownParser::find_bolds(src) {
    println!("{}", std::str::from_utf8(&src[span.start as usize..span.end as usize]).unwrap());
}

for link in MarkdownParser::find_links(src) {
    // link.text, link.url, link.is_image
}

for (heading, span) in MarkdownParser::find_headings(src) {
    println!("h{}: {}", heading.level, std::str::from_utf8(&src[span.start as usize..span.end as usize]).unwrap());
}
```

Standalone iterators operate without cross-element context. They may yield
spans that the full parser would suppress (e.g. bold markers inside a fenced
code block). See
[`ARCHITECTURE.md §12`](https://github.com/vgnapuga/meon/blob/main/meon/ARCHITECTURE.md#12-standalone-iterators)
for details.

---

## Feature flags

Inherited from `meon`:

| Feature  | Effect                                            |
|----------|---------------------------------------------------|
| `avx2`   | 32-byte SIMD search (requires nightly + AVX2 CPU) |
| `avx512` | 64-byte SIMD search (implies `avx2`)              |

```toml
[dependencies]
meon-md = { version = "0.1", features = ["avx2"] }
```

---

## Known limitations

This is a **demonstration grammar**, not a CommonMark-compliant implementation.

- A blockquote containing a fenced code block (`` > ``` ``) produces an
  incorrect span — the fence opens and the blockquote state is lost.
- Nested blockquotes (`> >`) leak inner content into the outer span.
- Emphasis spanning multiple lines is not detected.
- Emphasis precedence (CommonMark §6.2) is not enforced — declaration order
  wins.
- Reference-style links, HTML entities, and indented code blocks are not
  supported.

These are consequences of the single-forward-pass, single-active-block-slot
design of the `meon` engine. See
[`ARCHITECTURE.md §17`](https://github.com/vgnapuga/meon/blob/main/meon/ARCHITECTURE.md#17-known-limitations-and-deliberate-trade-offs)
for the full discussion.

---

## License

`meon-md` is available under the
[***GNU Affero General Public License v3.0 (AGPL-3.0)***](https://github.com/vgnapuga/meon/blob/main/LICENSE) - *GitHub*.

If the AGPL-3.0 terms are incompatible with your use case, a commercial
license is available — see [***COMMERCIAL.md***](https://github.com/vgnapuga/meon/blob/main/COMMERCIAL.md) - *GitHub*.

By contributing, you agree to the [***Contributor License Agreement***](https://github.com/vgnapuga/meon/blob/main/CLA.md) - *GitHub*.
