# meon-json

EN | [**RU**](https://github.com/vgnapuga/meon/blob/main/meon-json/README_RU.md) - *GitHub*

A fast flat JSON reader, built on the
[`meon`](https://github.com/vgnapuga/meon/blob/main/meon/README.md) declarative
parsing engine.

`meon-json` produces a *table of intervals*, not a tree. The engine emits one
flat `Vec` per element kind (objects, arrays, strings, members); document
structure is recovered by interval containment over the source. Scalar typing
(`nums` / `trues` / `falses` / `nulls`) is an opt-in post-pass, not part of the
hot loop — a caller that never types pays nothing.

* **meon**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon/README.md)
  * [***crates.io***](https://crates.io/crates/meon)
* **meon-macros**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon-macros/README.md)
  * [***crates.io***](https://crates.io/crates/meon-macros)
* **meon-md**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon-md/README.md)
  * [***crates.io***](https://crates.io/crates/meon-md)
* **meon-json**    <--
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
meon-json = "0.1"
```

```rust
use meon_json::JsonParser;

let src = br#"{"name":"Alice","age":30,"tags":["a","b"]}"#;
let c = JsonParser::parse(src);

// Access by element kind — O(1) per kind
println!("objects: {}", c.objects.len());   // 1
println!("arrays:  {}", c.arrays.len());    // 1
println!("members: {}", c.members.len());   // 3

// Resolve member key/value spans to slices — zero copy, borrow from source
for m in &c.members {
    println!("{} = {}", c.str(m.key).unwrap(), c.str(m.value).unwrap());
}
// "name" = "Alice"
// "age"  = 30
// "tags" = ["a","b"]
```

Pretty-printed, multi-line JSON is parsed the same way — the engine's inline
scan runs over one accumulated multi-line run rather than per physical line, so
the structure stack survives every internal `\n` in a document with no blank
lines.

---

## What this is — and what it is not

`meon-json` is a **structural reader**, not a validating parser.

- It reports the structure it *saw* and never returns an error. Malformed
  input — mismatched brackets, trailing garbage, duplicate keys, unterminated
  containers — does not panic; it yields sane partial output (an unterminated
  container is simply discarded, its committed members kept; trailing garbage
  lands in `scalars`).
- It does **not** validate JSON grammar. If you need rejection of invalid
  documents, validate separately — this crate is for fast structural and
  textual extraction over input you already trust or validate elsewhere.
- Scalar typing is **first-byte classification**, not number validation:
  `1abc` is typed as a number because it starts with a digit (see
  [Known limitations](#known-limitations)).

This mirrors the on-demand model of `simd-json`'s second stage: structure is
found once; values are materialised by type only on request.

---

## Output fields

The engine produces these structural fields directly. All spans are `u32`
byte offsets into the borrowed source.

| Element        | Syntax            | Field     | Type          | Notes                                                               |
|----------------|-------------------|-----------|---------------|---------------------------------------------------------------------|
| Object         | `{ ... }`         | `objects` | `Vec<Span>`   | Content only (braces excluded — `objects_raw()` includes them)      |
| Array          | `[ ... ]`         | `arrays`  | `Vec<Span>`   | Content only (brackets excluded — `arrays_raw()` includes them)     |
| String         | `"..."`           | `strings` | `Vec<Span>`   | Content only (quotes excluded); empty `""` emits **no** span        |
| Member         | `key : value`     | `members` | `Vec<Member>` | One per pair; `key` / `value` are raw spans (delimiters included)   |
| Top-level value| bare `42`, `true` | `scalars` | `Vec<Span>`   | Inline fallback: bare values outside any container, and stray tails |
| Document run   | whole input       | `loose`   | `Vec<Span>`   | Block-level fallback; one span over the whole document              |

Individual **array elements** are not tracked as spans by the engine — only
the array's own outer span is. Per-element access is the typing post-pass's
job (it splits each array span on its own top-level commas); see
[Scalar typing](#scalar-typing).

---

## Output type

```rust
// Span — a half-open byte range [start, end) into the source slice
pub struct Span { pub start: u32, pub end: u32 }

// Member — one `key: value` pair. Both fields are RAW source spans:
//   key   — quotes included for a quoted key
//   value — brackets/quotes included when the value is a container or string
pub struct Member {
    pub key:   Span,
    pub value: Span,
}
```

Recover the unescaped or typed content of a member by **interval containment**
against the other fields — e.g. a member whose value is an array is byte-equal
to that array's `arrays_raw()` slice; the bare `arrays` span (content-only) is
strictly *inside* it.

---

## Working with spans

The content struct borrows the original source. Use the built-in helpers:

```rust
let src = br#"{"a":[1,2,3]}"#;
let c = JsonParser::parse(src);

// str() returns None on invalid UTF-8 instead of panicking
println!("{}", c.str(c.members[0].key).unwrap());   // → "a"  (quotes included)

// bytes() for raw byte access, no UTF-8 check
let raw: &[u8] = c.bytes(c.members[0].value);        // → b"[1,2,3]"

// _clean() / _raw() iterators on the structural span fields:
//   _clean — content only (the bare field convention)
//   _raw   — delimiters included
let inner = c.arrays_clean().next().unwrap();        // → b"1,2,3"
let whole = c.arrays_raw().next().unwrap();          // → b"[1,2,3]"
```

> **Raw, untrimmed member values.** Nothing trims a member value. Its span runs
> from just after `:` (skipping at most one immediately-following space) to its
> terminator, verbatim — trailing spaces, tabs, `\r`, and embedded `\n` are all
> part of the raw span. Use the typing post-pass below if you want the clean,
> whitespace-trimmed token instead.

---

## Scalar typing

Typing lives entirely outside the engine, as methods on `JsonContent`. Ask for
it only when you need it:

```rust
use meon_json::{JsonParser, ScalarKind};

let src = br#"{"a":1,"b":true,"c":[2,3],"d":null}"#;
let c = JsonParser::parse(src);

// All four kinds in one cache-friendly pass, owned vectors out
let typed = c.type_scalars();
// typed.nums   → ["1", "2", "3"]   (member value + both array elements)
// typed.trues  → ["true"]
// typed.nulls  → ["null"]
// typed.falses → []

// Or just one kind, without allocating the other three
let nums = c.type_field(ScalarKind::Num);
```

```rust
// Returned by type_scalars(); each Vec holds spans into the source,
// byte-equal to the value/element they were typed from (whitespace trimmed).
pub struct TypedScalars {
    pub nums:   Vec<Span>,
    pub trues:  Vec<Span>,
    pub falses: Vec<Span>,
    pub nulls:  Vec<Span>,
}

pub enum ScalarKind { Num, True, False, Null }
```

Typing classifies **three** sources by first byte, after trimming
`sep` / `tab` / `\n` / `\r` from both ends:

1. member values (`members[i].value`),
2. array elements (each `arrays` span split on its own top-level commas),
3. bare top-level values (`scalars`).

A first byte of `"`, `{`, `[` — or anything unrecognised — classifies to
`None` and is skipped, so strings and containers are never mis-typed. Nothing
is written back into `JsonContent`; it stays an immutable record of exactly
what the engine saw.

---

## Standalone iterators

Inherited from the engine: every structural rule generates a `find_*` method
that scans the source without a full parse. Useful when you only need one kind
from a large document — extracting every string, say:

```rust
use meon_json::JsonParser;

let src = document.as_bytes();
for span in JsonParser::find_strings(src) {
    println!("{}", std::str::from_utf8(&src[span.start as usize..span.end as usize]).unwrap());
}
```

Standalone iterators are context-free: they match their one element kind with
no knowledge of nesting or surrounding structure. For JSON this matters more
than for a flat format — `find_objects` / `find_arrays` match only the exact
declared delimiter and do **not** track nesting the way a full `parse` does.
Prefer the full parse when you need correct containment; reach for `find_*`
only for a single, nesting-insensitive sweep (such as `find_strings`). See
[`ARCHITECTURE.md §12`](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE.md#12-standalone-iterators) - *GitHub*.

---

## Feature flags

Inherited from `meon`:

| Feature  | Effect                                            |
|----------|---------------------------------------------------|
| `avx2`   | 32-byte SIMD search (requires nightly + AVX2 CPU) |
| `avx512` | 64-byte SIMD search (implies `avx2`)              |

```toml
[dependencies]
meon-json = { version = "0.1", features = ["avx2"] }
```

---

## Nesting

This grammar sets `max_nest = 64`: objects and arrays nest up to 64 levels
deep, and `key_value` member values survive arbitrarily deep container
nesting within that cap. A document nested deeper than 64 levels is parsed
without panicking, but the over-cap containers are left untracked rather than
represented as their own spans.

```rust
let pretty = b"{\n  \"a\": {\n    \"b\": { \"c\": 1 }\n  }\n}";
let c = JsonParser::parse(pretty);
assert_eq!(c.objects.len(), 3);
assert_eq!(c.members.len(), 3);
```

---

## Known limitations

This is a structural reader; several behaviours follow deliberately from that.

- **Not a validator.** Invalid input never errors — it yields partial output.
  Mismatched brackets, trailing garbage, duplicate keys, and unterminated
  containers all produce sane, non-panicking results (a duplicate key records
  both members in order; an unterminated container is discarded, its committed
  members kept).
- **Typing is first-byte classification, not number validation.** `1abc` types
  as `Num`; `True` (capitalised) matches no arm and is left untyped. The
  classifier checks one byte, never the rest of the run.
- **Array elements are not engine-tracked.** The engine stores each array's
  outer span only; per-element spans exist solely through the typing post-pass.
- **Member values are raw and untrimmed.** Only a single leading space after
  `:` is skipped (`allow_sep`); trailing whitespace, `\r`, and embedded `\n`
  stay in the raw span. The typing layer trims; the structural field does not.
- **Empty string `""` emits no `strings` span.** Its content is still
  recoverable from the raw member value.
- **Nesting depth is capped at `max_nest = 64`.** Deeper structure is not
  specially tracked.

See
[`ARCHITECTURE.md §17`](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE.md#17-known-limitations-and-deliberate-trade-offs) - *GitHub*
for the engine's remaining trade-offs.

---

## License

`meon-json` is available under the
[***GNU Affero General Public License v3.0 (AGPL-3.0)***](https://github.com/vgnapuga/meon/blob/main/LICENSE) - *GitHub*.

If the AGPL-3.0 terms are incompatible with your use case, a commercial
license is available — see [***COMMERCIAL.md***](https://github.com/vgnapuga/meon/blob/main/COMMERCIAL.md) - *GitHub*.

By contributing, you agree to the [***Contributor License Agreement***](https://github.com/vgnapuga/meon/blob/main/CLA.md) - *GitHub*.
