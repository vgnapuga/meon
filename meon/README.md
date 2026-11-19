# meon

A declarative flat parsing engine for text formats.

You describe a grammar once with `define_parser!` and get back a fully working
parser: a `parse` method for a full scan with bounded forward searches, and
`find_*` standalone iterators for lazily extracting one element kind at a time.

```toml
[dependencies]
meon = "0.7"
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
* **meon-json**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon-json/README.md)
  * [***crates.io***](https://crates.io/crates/meon-json)

* [***FAQ.md***](https://github.com/vgnapuga/meon/blob/main/FAQ.md) - *GitHub*
* [***CHANGELOG.md***](https://github.com/vgnapuga/meon/blob/main/CHANGELOG.md) - *GitHub*
* [***ARCHITECTURE.md***](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE.md) - *GitHub*
* [***BENCHMARKS.md***](https://github.com/vgnapuga/meon/blob/main/benches/README.md) - *GitHub*
  * [***MD_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/MD_COMPARE.md) - *GitHub*
  * [***JSON_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/JSON_COMPARE.md) - *GitHub*
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
  - `parse(source: &[u8]) -> NameContent<'_>` — full parse with linear
    search work per fixed grammar.
  - `find_*(source: &[u8]) -> impl Iterator` — one per rule, context-free,
    faster when you only need one element kind.
  - `context(source: &[u8]) -> ParseContext` — the opaque-region map (fences
    plus opaque symmetric/asymmetric and fully opaque chained rules), built
    by a forward trigger scan with bounded searches.
  - `find_context_*(source, &ctx) -> impl Iterator` — for transparent
    symmetric/asymmetric and non-fence line/block rules; the same matcher with candidates inside opaque regions skipped.
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

Four context bytes are required and apply to every rule. `max_nest` and
`crlf` are optional grammar-wide settings:

| Key        | Meaning                                                        |
|------------|----------------------------------------------------------------|
| `sep`      | Word separator (typically space)                               |
| `eol`      | Line terminator (typically `\n`)                               |
| `tab`      | Tab character                                                  |
| `escape`   | Escape prefix that suppresses the next byte                    |
| `crlf`     | Optional. Recognise CR before `eol` as a line-ending byte; default `false`. |
| `max_nest` | Optional. Bounded self-nesting depth cap, shared by            |
|            | stack-based inline rules and `key_value`                      |
|            | values (below) and by block-level `cont`/`fence` nesting.      |
|            | Default `1`: no self-nesting, at most one block active.        |
|            | `sep = ..., eol = ..., tab = ..., escape = ..., max_nest = 4;` |

The optional `crlf` setting defaults to `false`. With `crlf = true`, only a
CR immediately followed by an actual `eol` is part of the line ending. A bare CR, including at EOF, remains content.

---

Byte patterns in `line_simple`, `fence`, `block (pattern)` and both arguments
of `num` are Rust patterns, including literals, alternatives, ranges and
constants. The same patterns reach full parsing, standalone iterators and
context construction. A grammar emitted by `macro_rules!` may pass literal,
pattern and expression fragments without changing their precedence.

## Rule families

Rules are divided into three families based on where in the source an element
can begin and end.

### Inline rules — within an inline run

Inline elements are found by scanning for trigger bytes within an inline run,
which may span paragraph or same-marker continuation lines. Two sub-kinds:

**`inline_simple`** — a single `Span` with no extra metadata.
Declared via `fallback` or inside `on_trigger` as match arms.
Field type: `Vec<Span>`.

**`inline`** — a user-defined struct with multiple `Span` fields.
Declared via `chained` or `key_value` inside `on_trigger`.
Field type: `Vec<T>`.

---

#### `on_trigger(b1, b2, ...) { ... }`

Declares a set of trigger bytes. When any of them is found on a line the
block contributes to the engine's fixed kind dispatch. Declaration order is
preserved within a kind; the dispatch is not a format-specific precedence table.

---

##### `symmetric byte { ... }` — same open and close delimiter

```
on_trigger(b'*') {
    symmetric b'*' {
        parse_inside = true;   // transparent bounded-stack matching
        1 => italics  [40],    // count => field [capacity_divisor]
        2 => bolds    [40],
        3 => bold_italics [80],
    }
}
```

- `parse_inside = true` uses the bounded inline stack. A matching byte and
  count on the top frame closes it; another declared count opens a nested
  frame when the stack has room. Content remains visible to other rules.
- `parse_inside = false` searches forward for the next unescaped run of
  exactly the opening count. Runs of other lengths remain content, and a
  successful match is opaque to other rules.
- `balanced` is not accepted for `symmetric`; writing it is a compile error.
  `parse_inside` and run counts decide matching. Identical byte/count pairs
  close rather than self-nest.
- Each `N => field [div]` arm captures runs of exactly N bytes → `Vec<Span>`.
- `_ => field [div]` captures other counts in the full parse; standalone
  finders are generated for explicit numeric count arms.

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

- With `parse_inside = true`, the rule uses the bounded inline stack.
  `balanced = true` permits self-nesting up to `max_nest`; with `false`,
  another open byte of the same pending type is literal. Each opening byte
  is an event, and the rule declares the `1` arm.
- With `parse_inside = false`, matching is an opaque forward search. An
  unbalanced rule selects an exact opening-run count and closes at the first
  unescaped close byte. A balanced rule declares the `1` arm, counts every
  unescaped open byte as another level, and closes when the depth returns to
  zero. Its own depth does not consume the bounded inline stack.
- A transparent rule needs its close byte in `on_trigger`; an opaque forward
  search finds its close internally, so only its opening byte is required.
- Different asymmetric rules may share a close byte when their opening bytes
  differ; stack closing uses the pending frame's own opening byte.

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
  instead of the self-contained forward search an opaque component uses.
  This is scoped to a single active `chained` rule per grammar; a grammar
  declaring two such rules with overlapping in-progress matches is not
  supported.
- The output type `T` must be defined by the grammar author with fields named
  after the `=> field` identifiers.
- With both components opaque, a failed chain involving a balanced component
  lazily builds a close index for the remaining inline run. Later balanced
  queries take constant time and preserve valid inner matches. Search work
  stays linear per rule. One reusable buffer holds the index and its working
  stack links: four bytes per indexed byte per balanced component, up to eight
  when both are balanced. Successful chains before a failure allocate no
  search buffer; fully unbalanced rules use scalar memos.
- `balanced` counts a component's own unescaped opening and closing bytes;
  the second opener must immediately follow the first close. Of an initial
  opening run, only its last byte opens the chain.
- Field type: `Vec<T>`.

---

##### `key_value: Type { ... }` — `key = value` pairs

```
on_trigger(b'=') {
    key_value: KeyValue {
        eq        = b'=';
        allow_sep = true;   // skip one separator after eq; trim key padding
        end       = b'\n';  // value terminates here (or at eol)
        key   => key,
        value => value,
    } => key_values [20]
}
```

- The `eq` byte **must** be listed in the `on_trigger(...)` set; the `end`
  byte is added to that set automatically — do not list it yourself.
- The value participates in the unified bounded-nesting stack: it survives
  nested `symmetric`/`asymmetric` containers opened inside it (up to the
  grammar's `max_nest`) and survives internal `eol` bytes within a run. The
  pair is emitted only when its value actually closes — at `end`, at the
  close of the container the value lives in, or at the run's end — so `Vec<T>`
  stays in value-close order.
- With `allow_sep = true`, one immediate `sep` or `tab` after `eq` is
  skipped; further bytes remain in the value. If `sep` is also `end`, initial
  separator padding does not end the value before its first nonseparator.
- A refused open when the shared stack is full remains literal text.
- The output type `T` must be defined with fields named after the `=> key` /
  `=> value` identifiers. Field type: `Vec<T>`.
- Only one `key_value` rule per grammar is supported (see Known limitations).

---

##### `fallback => field [div]` — plain text

Captures runs of bytes that triggered no other inline rule.

```
fallback => texts [10];
```

With `merge_simple = true`, adjacent fallback spans (gap at most one byte)
merge only within the same inline run. Separate paragraphs or block/line
items do not merge across their boundaries.

`fallback` also keeps the literal bytes of a construct that never closes,
including its opener. With a transparent symmetric `*` rule, `*a\nb\n`
yields no italic and fallback `*a\nb`; the run's final `eol` is not inline
content. Elements that close inside an unmatched outer construct survive.
For example, ``**a `c` b`` retains the code span and fallback `**a ` and ` b`.

---

##### `hard_break(esc, sp, min) => field [div]` — trailing hard break

```
hard_break(b'\\', b' ', 2) => hard_breaks [500];
```

Detects a trailing hard-break marker at end of line: either the `esc` byte, or
at least `min` consecutive `sp` bytes. Emits a zero-length `Span`.

---

### Line rules — whole line consumed

A matching line consumes the line-rule decision; its content after the
marker is scanned for inline elements. A uniform line has no content to scan.
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

##### `line_simple(pattern, min = N) |var|: Type { ... } => field [div]`

Matches lines composed entirely of one repeated byte accepted by `pattern`
(interleaved with `sep`), appearing at least `min` times. `var` receives that
byte. A mixture of two different accepted bytes is not a match.

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

##### `fence(pattern, min = N) => field [div]`

Opens with at least N consecutive copies of one byte accepted by `pattern`,
with no further copy of that byte in the info string. It closes with the
same byte and at least the opening count, followed only by `sep` or `tab`. The entire
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
satisfying `end_pat` and then `sep` or `tab`. `n` receives the number accumulated as
`n.wrapping_mul(10).wrapping_add(byte.wrapping_sub(b'0') as u32)` for
each digit byte; this is decimal for ASCII digits and wraps for other
patterns. `k` receives the original delimiter byte. `_raw()` uses the
declared digit pattern to include the original number and marker.

```
num(b'0'..=b'9', end = b'.' | b')') |n, k|:
    OrderedItem { kind: k, num: n }
    => ordered_items [80];
```

##### `fallback => field [div]`

Lines matching no other block or line rule are grouped into paragraph spans.
A line containing only `sep` and `tab` is blank and ends the run.

```
fallback => paragraphs [80];
```

---

## Inline runs span multiple lines

Inline scanning does not operate per physical line. A run of lines that
match no `line`/`block`/`fence`/`cont` rule is handed to the inline scanner
as **one** multi-line span. Inside that run an `eol` is ordinary content: an
open `symmetric`/`asymmetric` frame or a pending `key_value` value survives
the line break unless its own delimiter closes it sooner.

A run is bounded — and the inline stack is drained — at a blank line, at a
line where a `line`/`block` rule matches, or at end of input. A grammar with
empty `lines {}` / `blocks {}` sections (e.g. a JSON-shaped grammar)
therefore gets one run covering the whole input, blank lines aside, so its
nesting survives every internal `\n` at no extra cost. A grammar with real
`lines`/`blocks` rules is bounded by them: multi-line inline spanning
happens only inside a single paragraph.

---

## Capacity divisors

Every field carries `[div]`. The field's `Vec` starts unallocated and
reserves `source.len() / div` elements when it receives its first element, so
a rule that never matches a given document costs nothing. A divisor of `10`
means roughly one element per 10 bytes. Tune based on expected element
density in real inputs.

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

Standalone iterators scan the raw source as a byte stream — one
`memchr`-family search per candidate, so marker-free stretches are never
walked. Inline pair matching is paragraph-bounded (a pair may span a single
line break; an empty line or a line of `sep`/`tab` alone aborts it), and same-type block nesting (`cont`)
matches the full parse up to the grammar's `max_nest`. They may still yield
spans that the full parser would suppress (e.g. delimiters inside a fence);
counts can differ from the full parse — this is by design. Standalone
iterators follow their rule's balancing and transparency settings. Exact
opaque symmetric/asymmetric arms select their declared opening-run count;
opaque balanced asymmetric rules count nested opening and closing bytes.

For transparent symmetric/asymmetric and non-fence line/block rules, a
context-aware variant is also generated. `key_value` and `chained` have raw
finders only:

```rust
// One streaming pass builds the opaque-region map (fences plus every
// parse_inside = false rule), shared by all context-aware finders.
let ctx = MyParser::context(source);
for span in MyParser::find_context_italics(source, &ctx) {
    // candidates inside fences, code spans, etc. are skipped
}
```

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

- Block nesting and stack-based inline self-nesting are bounded by
  `max_nest`, default `1`. Opaque balanced forward searches count their own
  depth outside that stack; `max_nest` is not a span or input-size limit.
- A `chained` rule with a transparent component (`parse_inside = true`)
  tracks only one in-progress match per grammar; two such rules with
  overlapping matches are not supported.
- Inline scanning runs over a multi-line run (see "Inline runs span multiple
  lines"), not a single line: the unified stack persists across internal
  `eol` bytes. A run is still bounded by a blank line, so a blank line
  *inside* an open construct closes the run and discards that construct (its
  text is kept as `fallback`) — a JSON-shaped grammar must not contain blank
  lines mid-value. Overlapping inline kinds are dispatched in the engine's fixed order;
  declaration order is preserved within a kind. This is not a format-specific
  precedence table.
- Only one `key_value` rule per grammar is supported; its key-segment anchor
  is shared, so a second `key_value` rule is not tracked independently.

---

## License

`meon` is available under the
[***MIT***](https://github.com/vgnapuga/meon/blob/main/LICENSE-MIT) *OR* [***APACHE-2.0***](https://github.com/vgnapuga/meon/blob/main/LICENSE-APACHE) license - *GitHub*.
