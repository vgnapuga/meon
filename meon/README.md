# meon

EN | [**RU**](https://github.com/vgnapuga/meon/blob/main/meon/README_RU.md) - *GitHub*

A declarative flat parsing engine for text formats.

You describe a grammar once with `define_parser!` and get back a fully working
parser: a `parse` method for a full single-pass scan, and `find_*` standalone
iterators for lazily extracting one element kind at a time.

```toml
[dependencies]
meon = "0.2"
```

* **meon**    <--
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon/README.md)
  * [***crates.io***](https://crates.io/crates/meon)
* **meon-macros**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon-macros/README.md)
  * [***crates.io***](https://crates.io/crates/meon-macros)
* **meon-md**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon-md/README.md)
  * [***crates.io***](https://crates.io/crates/meon-md)

* [***CHANGELOG.md***](https://github.com/vgnapuga/meon/blob/main/CHANGELOG.md)
* [***ARCHITECTURE.md***](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE.md) - *GitHub*
* [***BENCHMARKS.md***](https://github.com/vgnapuga/meon/blob/main/benches/README.md) - *GitHub*
* [***FUZZING.md***](https://github.com/vgnapuga/meon/blob/main/fuzz/README.md) - *GitHub*

## Quick start

```rust
use meon::define_parser;

define_parser!(Plain {
    sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';

    inline {
        fallback => texts [10];
    }
    blocks {
        fallback => paragraphs [80];
    }
});

let content = PlainParser::parse(b"hello world\n");
assert_eq!(content.paragraphs.len(), 1);
assert_eq!(content.texts.len(), 1);
```

## How it works

`define_parser!(Name { ... })` expands into:

- `NameContent<'a>` — the output struct. Every grammar rule produces one `pub`
  field. All fields borrow from the original source slice.
- `NameParser` — a unit struct with:
  - `parse(source: &[u8]) -> NameContent<'_>` — full single-pass parse, O(n).
  - `find_*(source: &[u8]) -> impl Iterator` — one per rule, context-free,
    faster when you only need one element kind.
- `str(span) -> Option<&str>` / `bytes(span) -> &[u8]` and `_clean` / `_raw`
  accessor methods on `NameContent` for ergonomic span-to-slice conversion.

Spans are `u32` byte offsets. Input must not exceed 4 GiB (`span::MAX_INPUT_LEN`).

## Grammar syntax

```
define_parser!(Name {
    sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';

    inline  { ... }
    lines   { ... }
    blocks  { ... }
});
```

Four context bytes are required and apply to every rule. A fifth,
`max_nest`, is optional and grammar-wide:

| Key        | Meaning                                                        |
|------------|----------------------------------------------------------------|
| `sep`      | Word separator (typically space)                               |
| `eol`      | Line terminator (typically `\n`)                               |
| `tab`      | Tab character                                                  |
| `escape`   | Escape prefix that suppresses the next byte                    |
| `max_nest` | Optional. Bounded self-nesting depth cap, shared by            |
|            | `balanced` symmetric/asymmetric rules (below) and by           |
|            | block-level `cont`/`fence` nesting. Default `1`,               |
|            | which reproduces the original, non-nesting behaviour           |
|            | exactly: no self-nesting, at most one block active.            |
|            | `sep = ..., eol = ..., tab = ..., escape = ..., max_nest = 4;` |

---

## Rule families

Rules are divided into three families based on where in the source an element
can begin and end.

### Inline rules — inside a line

Inline elements are found by scanning for trigger bytes within the content of a
line. Two sub-kinds:

**`inline_simple`** — a single `Span` with no extra metadata.
Declared via `fallback` or inside `on_trigger` as match arms.
Field type: `Vec<Span>`.

**`inline`** — a user-defined struct with multiple `Span` fields.
Declared via `chained` or `key_value` inside `on_trigger`.
Field type: `Vec<T>`.

---

#### `on_trigger(b1, b2, ...) { ... }`

Declares a set of trigger bytes. When any of them is found on a line the
block is entered and rules are tried in declaration order.

---

##### `symmetric byte { ... }` — same open and close delimiter

```
on_trigger(b'*') {
    symmetric b'*' {
        parse_inside = true;   // pending match: first run opens, next closes
        balanced     = true;   // self-nest different-count occurrences up to max_nest
        1 => italics  [40],    // count => field [capacity_divisor]
        2 => bolds    [40],
        3 => bold_italics [80],
    }
}
```

- `parse_inside = true` — the scanner remembers the opening run and closes on
  the next matching count (`pending` mode). Content between open and close is
  itself scanned for inline elements, which is what lets a construct like
  `**bold *italic* text**` recognise the nested italic at all.
- `parse_inside = false` — the scanner immediately searches forward for the
  closing run (`greedy` mode, used for code spans). Content inside is opaque
  to every other rule.
- `balanced`'s meaning depends on `parse_inside`:
  - With `parse_inside = true`: `balanced = false` means a different-count
    occurrence of the same byte, while one is already pending, overwrites
    the pending slot — no nesting, matching the original single-slot
    behaviour exactly. `balanced = true` instead opens a *bounded stack* of
    pending frames (sized by the grammar's `max_nest`), so a different-count
    occurrence opens its own frame instead of overwriting the outer one —
    both levels resolve and are emitted to their own fields, separately. An
    *identical* `(byte, count)` pair still cannot self-nest, since open and
    close look the same for a symmetric delimiter (`**a **b** c**` resolves
    as two adjacent runs, not as nesting).
  - With `parse_inside = false` (greedy mode): `balanced = true` instead
    means a *doubled* run of the same delimiter found while searching forward
    is treated as escaped/literal content rather than the close, and the
    search continues past it — unrelated to the stack above.
- Each `N => field [div]` arm captures runs of exactly N bytes → `Vec<Span>`.
- `_ => field [div]` captures any count not matched by the explicit arms.

---

##### `asymmetric open, close { ... }` — different open and close bytes

```
on_trigger(b'{', b'}') {
    asymmetric b'{', b'}' {
        balanced     = true;
        parse_inside = false;
        1 => objects [100],
    }
}
```

- `balanced` and `parse_inside` are independent settings — either one alone
  is enough to put a rule on the bounded stack, sized by the grammar's
  `max_nest`:
  - `balanced` sets this *type's* own effective depth cap: `max_nest` if
    `true` (so `{ { } }` self-nests), or a hard `1` if `false` — a second
    open of the same type while one is already pending is then simply
    literal, matching the original behaviour for that type exactly.
  - `parse_inside` controls *opacity*: `false` keeps the content between
    open and close invisible to every other rule (used for autolinks, so a
    URL's own `(`/`)` aren't mistaken for something else); `true` makes it
    transparent, so other rules — including different bracket types on this
    same stack — can fire on the bytes in between.
- The close byte is matched against the *frame's own* recorded open byte, not
  against whichever rule happens to be checked first — so two different
  `asymmetric` rules declared in the same `on_trigger` block may safely share
  a close byte (e.g. `(`/`)` and `[`/`)`), as long as their open bytes
  differ.
- Once `balanced` or `parse_inside` is `true`, the close byte **must** be
  listed in the same `on_trigger(...)` set as the open byte —
  `on_trigger(b'{', b'}')`, not just `on_trigger(b'{')` — since closing is
  found by the same scan that finds the opening, not by an internal forward
  search.
- Count arms work the same as in `symmetric`, except each byte of a
  multi-byte run is its own event — `{{` is two opens, not one "count = 2"
  event.

---

##### `chained: Type { ... }` — two-part delimiter (e.g. links)

```
on_trigger(b'[') {
    chained: Link {
        | b'[', b']' | { parse_inside = false; balanced = false; } => text,
        | b'(', b')' | { parse_inside = false; balanced = false; } => url,
        prefix | b'!' | => is_image,
    } => links [100]
}
```

Matches the pattern `[prefix]open1...close1 open2...close2`.

- Two `| open, close |` pairs define the two components.
- `prefix | byte |` declares an optional single byte immediately before `open1`
  that sets a boolean field (`is_image` in the example).
- `parse_inside = true` on either component makes that component
  transparent — other rules can fire on the bytes scanned over for it —
  instead of the original, fully-opaque self-contained forward search. This
  is scoped to a single active `chained` rule per grammar; a grammar
  declaring two such rules with overlapping in-progress matches is not
  supported.
- The output type `T` must be defined by the grammar author with fields named
  after the `=> field` identifiers.
- Field type: `Vec<T>`.

---

##### `key_value: Type { ... }` — `key = value` pairs

```
on_trigger(b'=') {
    key_value: KeyValue {
        eq        = b'=';
        allow_sep = true;   // trim spaces around eq
        end       = b'\n';  // value terminates here (or at eol)
        key   => key,
        value => value,
    } => key_values [20]
}
```

- Field type: `Vec<T>`.

---

##### `fallback => field [div]` — plain text

Captures runs of bytes that triggered no other inline rule.

```
fallback => texts [10];
```

Adjacent spans can be merged (gap ≤ 1 byte) by setting `merge_simple = true`
at the top of the `inline` section.

---

##### `hard_break(esc, sp, min) => field [div]` — trailing hard break

```
hard_break(b'\\', b' ', 2) => hard_breaks [500];
```

Detects a trailing hard-break marker at end of line: either the `esc` byte, or
at least `min` consecutive `sp` bytes. Emits a zero-length `Span`.

---

### Line rules — whole line consumed

A matching line is consumed entirely; inline scanning is skipped for it.
Both line rule kinds produce `Vec<(Type, Span)>` where `Span` covers the
content portion after the marker.

---

##### `line(byte, max = N) |var|: Type { ... } => field [div]`

Matches lines that start with 1–N consecutive occurrences of `byte` followed
by `sep` or end of line. `var` receives the count.

```
line(b'#', max = 6) |n|:
    Heading { level: NonZeroU8::new(n).unwrap_or(NonZeroU8::MIN) }
    => headings [200];
```

---

##### `line_simple(b1 | b2 | ..., min = N) |var|: Type { ... } => field [div]`

Matches lines composed entirely of one repeated delimiter byte (interleaved
with `sep`), appearing at least `min` times. `var` receives the delimiter byte.

```
line_simple(b'-' | b'*' | b'_', min = 3) |b|:
    ThematicBreak { kind: b }
    => thematic_breaks [200];
```

---

### Block rules — span multiple lines

Block elements begin on one line and end on a later line.

---

#### `block_simple { ... }` — multi-line spans, no per-line metadata

Field type: `Vec<Span>`.

##### `fence(byte, min = N) => field [div]`

Opens when a line starts with ≥ N consecutive `byte` bytes and contains no
further `byte` in the info string. Closes on the next line that starts with
at least as many `byte` bytes, followed only by `sep` or `tab`. The entire
range (open fence through close fence) is one `Span`. **Inline scanning is
suppressed while a fence is active.**

```
fence(b'`', min = 3) => fenced_codes [400];
```

##### `cont(byte) => field [div]`

Groups consecutive lines that start with `byte` into a single `Span`. Closes
when a line does not start with `byte`. With `max_nest > 1`, a `cont` may
self-nest — `> > text` opens two distinct, correctly-bounded spans, since the
marker is checked positionally on each peel and another `cont` rule may open
right after it on the same line.

```
cont(b'>') => blockquotes [200];
```

---

#### `block { ... }` — per-line items with metadata

Field type: `Vec<(Type, Span)>` — one entry per matching line.

##### `(pattern) |var|: Type { ... } => field [div]`

Matches lines where, after optional leading whitespace, a single byte
satisfying `pattern` is followed by `sep` or `tab`. `var` receives the marker
byte.

```
(b'-' | b'*' | b'+') |b|:
    BulletItem { kind: b }
    => bullet_items [80];
```

##### `num(digit_pat, end = end_pat) |n, k|: Type { ... } => field [div]`

Matches lines where a digit run (up to 9 digits) is followed by a byte
satisfying `end_pat` and then `sep` or `tab`. `n` receives the parsed number,
`k` the delimiter byte.

```
num(b'0'..=b'9', end = b'.' | b')') |n, k|:
    OrderedItem { kind: k, num: n }
    => ordered_items [80];
```

##### `fallback => field [div]`

Lines matching no other block rule are grouped into paragraph spans.

```
fallback => paragraphs [80];
```

---

## Capacity divisors

Every field carries `[div]`. The initial `Vec` capacity is
`source.len() / div`. A divisor of `10` means roughly one element per 10 bytes.
Tune based on expected element density in real inputs.

---

## Standalone iterators

Every rule that supports it generates a `find_*` method:

```rust
// instead of a full parse:
let content = MyParser::parse(source);

// scan for one element kind only:
for span in MyParser::find_italics(source) {
    println!("{}", std::str::from_utf8(&source[span.start as usize..span.end as usize]).unwrap());
}
```

Standalone iterators scan the raw source without any cross-element context.
They may yield spans that the full parser would suppress (e.g. delimiters
inside a fence). Counts can differ from the full parse — this is by design.
Standalone `symmetric`/`asymmetric` rules match only the exact declared
count and never participate in the bounded-nesting stack, regardless of the
grammar's `max_nest` or that rule's own `balanced`/`parse_inside` settings.

---

## Feature flags

| Feature  | Effect                                            |
|----------|---------------------------------------------------|
| `avx2`   | 32-byte SIMD search (requires nightly + AVX2 CPU) |
| `avx512` | 64-byte SIMD search (implies `avx2`)              |

Without either flag the crate compiles on stable Rust using a SWAR
(SIMD-Within-A-Register) fallback for multi-byte search.

---

## Known limitations

- Self-nesting (blockquotes/fences at the block level, `balanced`
  symmetric/asymmetric rules at the inline level) is bounded by the
  grammar's `max_nest` setting — default `1`, meaning no self-nesting at
  all. Constructs nested deeper than `max_nest` are not specially tracked;
  see the context-bytes table above.
- A `chained` rule with a transparent component (`parse_inside = true`)
  tracks only one in-progress match per grammar; two such rules with
  overlapping matches are not supported.
- Inline scanning is context-free within a line — there is no cross-line
  inline state, regardless of `max_nest`, which only bounds nesting *within*
  one line. Precedence between overlapping inline rules is resolved by
  declaration order, not a precedence table.

---

## License

`meon` is available under the
[***GNU Affero General Public License v3.0 (AGPL-3.0)***](https://github.com/vgnapuga/meon/blob/main/LICENSE) - *GitHub*.

If the AGPL-3.0 terms are incompatible with your use case, a commercial
license is available — see [***COMMERCIAL.md***](https://github.com/vgnapuga/meon/blob/main/COMMERCIAL.md) - *GitHub*.

By contributing, you agree to the [***Contributor License Agreement***](https://github.com/vgnapuga/meon/blob/main/CLA.md) - *GitHub*.
