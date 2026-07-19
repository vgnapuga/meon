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
* **meon-json**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon-json/README.md)
  * [***crates.io***](https://crates.io/crates/meon-json)

* [***CHANGELOG.md***](https://github.com/vgnapuga/meon/blob/main/CHANGELOG.md) - *GitHub*
* [***ARCHITECTURE.md***](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE.md) - *GitHub*
* [***BENCHMARKS.md***](https://github.com/vgnapuga/meon/blob/main/benches/README.md) - *GitHub*
  * [***MD_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/MD_COMPARE.md) - *GitHub*
  * [***JSON_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/JSON_COMPARE.md) - *GitHub*
* [***FUZZING.md***](https://github.com/vgnapuga/meon/blob/main/fuzz/README.md) - *GitHub*

---

## Quick start

```toml
[dependencies]
meon-md = "0.3"
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

| Element         | Syntax              | Field             | Type                         |
|-----------------|---------------------|-------------------|------------------------------|
| Heading         | `# ... ######`      | `headings`        | `Vec<(Heading, Span)>`       |
| Thematic break  | `---`, `***`, `___` | `thematic_breaks` | `Vec<(ThematicBreak, Span)>` |

### Block

| Element        | Syntax             | Field           | Type                      |
|----------------|--------------------|-----------------|---------------------------|
| Fenced code    | ` ``` ... ``` `    | `fenced_codes`  | `Vec<Span>`               |
| Blockquote     | `> ...`            | `blockquotes`   | `Vec<Span>`               |
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

Standalone iterators operate without cross-element context: they may yield
spans that the full parser would suppress (e.g. bold markers inside a fenced
code block). Blockquote nesting, however, matches the full parse —
`find_blockquotes` sees `> >` as two nested spans, capped by the grammar's
`max_nest`.

To close the opacity gap, build the context map once and use the
`find_context_*` variants:

```rust
let ctx = MarkdownParser::context(src);
// Bold markers inside code spans, autolinks and fenced blocks are skipped:
for span in MarkdownParser::find_context_bolds(src, &ctx) { /* ... */ }
```

Every non-opaque element kind has one (`find_context_bolds`,
`find_context_headings`, `find_context_blockquotes`, ...); code spans,
autolinks and fenced blocks are the context sources and keep only their
context-free `find_*`. See
[`ARCHITECTURE.md §12`](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE.md#12-standalone-iterators) - *GitHub*
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

## Nesting

This grammar sets `max_nest = 4`. Two independent mechanisms opt into it:

- **Blockquotes and fences** nest up to 4 levels deep. `> > text` opens two
  distinct, correctly-bounded `blockquotes` spans rather than one collapsed
  span that leaks the inner marker into it; a fenced code block opened on a
  continuation line inside a blockquote stays scoped to its own span.
- **Bold and italic** nest up to 4 levels deep. `**bold *italic* still
  bold**` resolves both the outer bold and the inner italic correctly,
  instead of the inner delimiter silently overwriting the outer one.

```rust
let src = b"> > nested quote with **bold *italic* text**\n";
let c = MarkdownParser::parse(src);
assert_eq!(c.blockquotes.len(), 2);
assert_eq!(c.bolds.len(), 1);
assert_eq!(c.italics.len(), 1);
```

Links, images, and autolinks remain non-nesting by design — `[a [b] c](url)`
does not nest its own brackets.

---

## Known limitations

This is a **demonstration grammar**, not a CommonMark-compliant implementation.

- Emphasis spanning multiple lines is not detected.
- Emphasis precedence (CommonMark §6.2) is not enforced — declaration order
  wins.
- Reference-style links, HTML entities, and indented code blocks are not
  supported.
- Nesting depth is capped at `max_nest = 4` for blockquotes/fences and for
  bold/italic; a 5th level of the same construct is left untracked rather
  than represented as its own span.

See
[`ARCHITECTURE.md §17`](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE.md#17-known-limitations-and-deliberate-trade-offs) - *GitHub*
for the full discussion of the engine's remaining trade-offs, including the
bounded `max_nest` cap and the single-active-`chained`-rule constraint.

---

## License

`meon-md` is available under the
[***MIT***](https://github.com/vgnapuga/meon/blob/main/LICENSE-MIT) *OR* [***APACHE-2.0***](https://github.com/vgnapuga/meon/blob/main/LICENSE-APACHE) license - *GitHub*.
