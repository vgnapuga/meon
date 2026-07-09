# meon

EN | [**RU**](https://github.com/vgnapuga/meon/blob/main/README_RU.md)

> Declarative flat parsing engine for text formats.

* **meon**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon/README.md)
  * [***crates.io***](https://crates.io/crates/meon)
* **meon-macros**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon-macros/README.md)
  * [***crates.io***](https://crates.io/crates/meon-macros)
* **meon-md**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon-md/README.md)
  * [***crates.io***](https://crates.io/crates/meon-md)
* **meon-json**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon-json/README.md)
  * [***crates.io***](https://crates.io/crates/meon-json)

* [***CHANGELOG.md***](https://github.com/vgnapuga/meon/blob/main/CHANGELOG.md)
* [***ARCHITECTURE.md***](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE.md)
* [***BENCHMARKS.md***](https://github.com/vgnapuga/meon/blob/main/benches/README.md)
  * [***MD_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/MD_COMPARE.md)
  * [***JSON_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/JSON_COMPARE.md)
* [***FUZZING.md***](https://github.com/vgnapuga/meon/blob/main/fuzz/README.md)

---

## What is meon

Most text parsers build a tree. `meon`([wiki](https://en.wikipedia.org/wiki/Meon_(philosophy))) builds a table.

You describe a grammar once with `define_parser!` and get back a content struct
where every element kind lives in its own flat `Vec`. Headings in one vec,
bold spans in another, links in a third. No tree traversal, no allocator
pressure from node objects, no virtual dispatch. Just contiguous arrays of
`u32` byte-offset pairs that you iterate at native speed.

---

## Flat output — hardware-friendly by design

A typical parser hands you a heterogeneous AST. To find all bold spans you
walk the tree, match node types, and collect what you need. Cache misses
accumulate as you jump between pointer-linked nodes of varying sizes.

`meon` inverts this. The output is a struct-of-arrays:

```
MarkdownContent {
    source:         &[u8]         ← original bytes, borrowed
    texts:          Vec<Span>     ← all plain text runs
    bolds:          Vec<Span>     ← all bold spans
    italics:        Vec<Span>     ← all italic spans
    codes:          Vec<Span>     ← all inline code spans
    links:          Vec<Link>     ← all links and images
    headings:       Vec<(Heading, Span)>
    fenced_codes:   Vec<Span>
    bullet_items:   Vec<(BulletItem, Span)>
    ...
}
```

All spans are `u32` byte offsets — 8 bytes per span. Access to any element
kind is O(1). Iterating all bold spans is a single forward scan over a
contiguous array. The CPU prefetcher is happy.

---

## Spans — zero-copy access into the source

Every element is represented as a `Span { start: u32, end: u32 }` — a
half-open byte range `[start, end)` into the original source slice. Nothing
is copied. Nothing is decoded unless you ask for it.

```rust
let src = b"**bold** and *italic*\n";
let c = MarkdownParser::parse(src);

// Resolve a span to a string slice — zero copy, borrow from source.
// Returns `None` on invalid UTF-8 instead of panicking.
let text: &str = c.str(c.bolds[0]).unwrap();
assert_eq!(text, "bold");

// Or work with raw bytes, no UTF-8 check
let bytes: &[u8] = c.bytes(c.italics[0]);

// Every field also gets a generated `_clean` (delimiters stripped) and
// `_raw` (delimiters included) accessor — zero-copy byte-slice iterators.
let raw: &[u8] = c.bolds_raw().next().unwrap();
assert_eq!(raw, b"**bold**");
```

The content struct borrows the source for its entire lifetime. When the struct
is dropped the source is released. No intermediate representations persist.

---

## Context-free extraction — parse one type without parsing everything

Every grammar rule generates a `find_*` standalone iterator. It scans the raw
source for one element kind only, with no knowledge of surrounding elements,
active blocks, or paragraph state.

```rust
// Full parse — all element kinds in one pass
let content = MarkdownParser::parse(src);

// Standalone — only bold spans, nothing else computed
for span in MarkdownParser::find_bolds(src) {
    println!("{}", std::str::from_utf8(&src[span.start as usize..span.end as usize]).unwrap());
}

// Headings with metadata — level and content span
for (heading, span) in MarkdownParser::find_headings(src) {
    println!("h{}: {}", heading.level, std::str::from_utf8(&src[span.start as usize..span.end as usize]).unwrap());
}

// Links and images — structured type, two spans inside
for link in MarkdownParser::find_links(src) {
    let text = std::str::from_utf8(&src[link.text.start as usize..link.text.end as usize]).unwrap();
    let url  = std::str::from_utf8(&src[link.url.start  as usize..link.url.end  as usize]).unwrap();
    println!("[{}]({})  image={}", text, url, link.is_image);
}
```

Standalone iterators are faster than a full parse when only one element kind
is needed — they skip all cross-element bookkeeping. The trade-off is that
they operate without context: a bold marker inside a fenced code block will be
matched by `find_bolds` but suppressed by the full parser. This divergence is
by design and documented.

---

## Declarative grammar — one invocation, full parser

The engine has no built-in knowledge of any text format. You describe your
format as a grammar and the engine compiles it into a parser at build time:

```rust
use meon::define_parser;

define_parser!(MyFormat {
    sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\', max_nest = 4;

    inline {
        on_trigger(b'*') {
            symmetric b'*' {
                parse_inside = true;
                balanced     = false;
                1 => italics [40],
                2 => bolds   [40],
            }
        }
        fallback => texts [10];
    }
    lines {
        line(b'#', max = 6) |n|:
            Heading { level: NonZeroU8::new(n).unwrap_or(NonZeroU8::MIN) }
            => headings [200];
    }
    blocks {
        block_simple {
            fence(b'`', min = 3) => fenced_codes [400];
        }
        fallback => paragraphs [80];
    }
});

// Generated:
// MyFormatParser::parse(src) -> MyFormatContent<'_>
// MyFormatParser::find_bolds(src) -> impl Iterator<Item = Span>
// MyFormatParser::find_headings(src) -> impl Iterator<Item = (Heading, Span)>
// MyFormatContent::bolds_clean() -> impl Iterator<Item = &[u8]>
// MyFormatContent::bolds_raw()   -> impl Iterator<Item = &[u8]>
// ... and more
```

Everything — the content struct, the parse method, all find_* iterators, all
accessors — is generated at compile time. There is no runtime dispatch, no
vtable, no grammar interpreter.

---

## Repository structure

```
meon/                 ← workspace root (this file)
├── meon/             ← parsing engine + runtime macros
├── meon-macros/      ← define_parser! proc-macro
├── meon-md/          ← Markdown grammar built on meon
├── meon-json/        ← JSON reader grammar built on meon
├── benches/          ← criterion benchmarks
└── fuzz/             ← cargo-fuzz harness
```

`meon-md` is a concrete grammar that parses a useful subset of Markdown. It
demonstrates that the engine covers real-world complexity, and serves as
the benchmark and fuzz target for the project.

`meon-json` is a second reference grammar — a flat, span-based JSON reader. It
shows the engine is not Markdown-specific: a structurally opposite format —
deep nesting, containers, `key: value` pairs, line breaks as ordinary content
— falls out of the same `define_parser!` primitives, emitting one flat `Vec`
per element kind (objects, arrays, strings, members) instead of a tree.

---

## Feature flags

| Feature  | Effect                                            |
|----------|---------------------------------------------------|
| `avx2`   | 32-byte SIMD search (requires nightly + AVX2 CPU) |
| `avx512` | 64-byte SIMD search (implies `avx2`)              |

Without either flag the crate compiles on stable Rust.

---

## License

`meon` is available under the
[***MIT***](./LICENSE-MIT) *OR* [***APACHE-2.0***](./LICENSE-APACHE) license.
