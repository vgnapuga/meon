# meon

EN | [**RU**](https://github.com/vgnapuga/meon/blob/main/meon/README_RU.md) - *GitHub*

A declarative flat parsing engine for text formats.

You describe a grammar once with `define_parser!` and get back a fully working
parser: a `parse` method for a full single-pass scan, and `find_*` standalone
iterators for lazily extracting one element kind at a time.

```toml
[dependencies]
meon = "0.1"
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

`define_parser!(Name { … })` expands into:

- `NameContent<'a>` — the output struct. Every grammar rule produces one `pub`
  field. All fields borrow from the original source slice.
- `NameParser` — a unit struct with:
  - `parse(source: &[u8]) -> NameContent<'_>` — full single-pass parse, O(n).
  - `find_*(source: &[u8]) -> impl Iterator` — one per rule, context-free,
    faster when you only need one element kind.
- `_clean` / `_raw` accessor methods on `NameContent` for ergonomic
  span-to-slice conversion.

Spans are `u32` byte offsets. Input must not exceed 4 GiB (`span::MAX_INPUT_LEN`).

## Grammar syntax

```
define_parser!(Name {
    sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';

    inline  { … }
    lines   { … }
    blocks  { … }
});
```

The four context bytes are required and apply to every rule:

| Key      | Meaning                                     |
|----------|---------------------------------------------|
| `sep`    | Word separator (typically space)            |
| `eol`    | Line terminator (typically `\n`)            |
| `tab`    | Tab character                               |
| `escape` | Escape prefix that suppresses the next byte |

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

#### `on_trigger(b1, b2, …) { … }`

Declares a set of trigger bytes. When any of them is found on a line the
block is entered and rules are tried in declaration order.

---

##### `symmetric byte { … }` — same open and close delimiter

```
on_trigger(b'*') {
    symmetric b'*' {
        parse_inside = true;   // pending match: first run opens, next closes
        balanced     = false;  // balanced: nested pairs skip the outer close
        1 => italics  [40],    // count => field [capacity_divisor]
        2 => bolds    [40],
        3 => bold_italics [80],
    }
}
```

- `parse_inside = true` — the scanner remembers the opening run and closes on
  the next matching count (`pending` mode).
- `parse_inside = false` — the scanner immediately searches forward for the
  closing run (`greedy` mode, used for code spans).
- `balanced = true` — nested pairs of the same delimiter are skipped over
  before closing.
- Each `N => field [div]` arm captures runs of exactly N bytes → `Vec<Span>`.
- `_ => field [div]` captures any count not matched by the explicit arms.

---

##### `asymmetric open, close { … }` — different open and close bytes

```
on_trigger(b'<') {
    asymmetric b'<', b'>' {
        balanced     = false;
        parse_inside = false;
        1 => autolinks [100],
    }
}
```

- `balanced = true` — nested open/close pairs are tracked before closing.
- `parse_inside` is accepted for compatibility but has no effect on asymmetric
  matching.
- Count arms work the same as in `symmetric`.

---

##### `chained: Type { … }` — two-part delimiter (e.g. links)

```
on_trigger(b'[') {
    chained: Link {
        | b'[', b']' | { parse_inside = false; balanced = false; } => text,
        | b'(', b')' | { parse_inside = false; balanced = false; } => url,
        prefix | b'!' | => is_image,
    } => links [100]
}
```

Matches the pattern `[prefix]open1…close1 open2…close2`.

- Two `| open, close |` pairs define the two components.
- `prefix | byte |` declares an optional single byte immediately before `open1`
  that sets a boolean field (`is_image` in the example).
- The output type `T` must be defined by the grammar author with fields named
  after the `=> field` identifiers.
- Field type: `Vec<T>`.

---

##### `key_value: Type { … }` — `key = value` pairs

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

##### `line(byte, max = N) |var|: Type { … } => field [div]`

Matches lines that start with 1–N consecutive occurrences of `byte` followed
by `sep` or end of line. `var` receives the count.

```
line(b'#', max = 6) |n|:
    Heading { level: NonZeroU8::new(n).unwrap_or(NonZeroU8::MIN) }
    => headings [200];
```

---

##### `line_simple(b1 | b2 | …, min = N) |var|: Type { … } => field [div]`

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

#### `block_simple { … }` — multi-line spans, no per-line metadata

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
when a line does not start with `byte`.

```
cont(b'>') => blockquotes [200];
```

---

#### `block { … }` — per-line items with metadata

Field type: `Vec<(Type, Span)>` — one entry per matching line.

##### `(pattern) |var|: Type { … } => field [div]`

Matches lines where, after optional leading whitespace, a single byte
satisfying `pattern` is followed by `sep` or `tab`. `var` receives the marker
byte.

```
(b'-' | b'*' | b'+') |b|:
    BulletItem { kind: b }
    => bullet_items [80];
```

##### `num(digit_pat, end = end_pat) |n, k|: Type { … } => field [div]`

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

- Only one block can be active at a time. Nested block constructs (e.g. a
  continuation block containing a fenced block) are not representable.
- Inline scanning is context-free within a line. Precedence between
  overlapping inline rules is resolved by declaration order.

---

## License

`meon` is available under the
[***GNU Affero General Public License v3.0 (AGPL-3.0)***](https://github.com/vgnapuga/meon/blob/main/LICENSE) - *GitHub*.

If the AGPL-3.0 terms are incompatible with your use case, a commercial
license is available — see [***COMMERCIAL.md***](https://github.com/vgnapuga/meon/blob/main/COMMERCIAL.md) - *GitHub*.

By contributing, you agree to the [***Contributor License Agreement***](https://github.com/vgnapuga/meon/blob/main/CLA.md) - *GitHub*.
