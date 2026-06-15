# meon — Architecture

EN | [**RU**](./ARCHITECTURE_RU.md)

This document describes the internal design of the `meon` parsing engine: how
the grammar DSL is compiled, how the runtime executes, how data flows from
source bytes to output spans, and where the known trade-offs live.

It is written for contributors and anyone integrating the engine at a level
deeper than the public API.

* **meon**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon/README.md)
  * [***crates.io***](https://crates.io/crates/meon)
* **meon-macros**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon-macros/README.md)
  * [***crates.io***](https://crates.io/crates/meon-macros)
* **meon-md**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon-md/README.md)
  * [***crates.io***](https://crates.io/crates/meon-md)

* ***ARCHITECTURE.md***    <--
* [***BENCHMARKS.md***](https://github.com/vgnapuga/meon/blob/main/benches/README.md)
* [***FUZZING.md***](https://github.com/vgnapuga/meon/blob/main/fuzz/README.md)

---

## Table of contents

1. [Repository layout](#1-repository-layout)
2. [Design goals and constraints](#2-design-goals-and-constraints)
3. [The two-crate split](#3-the-two-crate-split)
4. [Grammar compilation pipeline](#4-grammar-compilation-pipeline)
5. [Intermediate representation](#5-intermediate-representation)
6. [Code generation](#6-code-generation)
7. [Runtime architecture](#7-runtime-architecture)
8. [Content struct and state accumulator](#8-content-struct-and-state-accumulator)
9. [Inline parsing](#9-inline-parsing)
10. [Line parsing](#10-line-parsing)
11. [Block parsing](#11-block-parsing)
12. [Standalone iterators](#12-standalone-iterators)
13. [Multi-byte search — SWAR and SIMD](#13-multi-byte-search--swar-and-simd)
14. [Span representation](#14-span-representation)
15. [Accessor methods](#15-accessor-methods)
16. [Cross-crate macro hygiene](#16-cross-crate-macro-hygiene)
17. [Known limitations and deliberate trade-offs](#17-known-limitations-and-deliberate-trade-offs)
18. [Extension points](#18-extension-points)

---

## 1. Repository layout

```
meon/                          ← workspace root
├── meon/                      ← parsing engine (this crate)
│   ├── src/
│   │   ├── lib.rs             ← public re-exports, crate-level docs
│   │   ├── span.rs            ← Span type and MAX_INPUT_LEN
│   │   ├── swar.rs            ← find_any: multi-byte search primitive
│   │   └── engine/
│   │       ├── content.rs     ← define_content! macro
│   │       ├── mod.rs
│   │       └── text_parser/
│   │           ├── mod.rs     ← parse_text! macro
│   │           ├── inline.rs  ← parse_inline! macro
│   │           ├── line.rs    ← parse_line! macro
│   │           ├── block.rs   ← parse_block! macro
│   │           └── standalone/
│   │               ├── mod.rs            ← define_standalone_fns! + module docs
│   │               ├── common.rs         ← shared iterator utilities
│   │               ├── symmetric.rs
│   │               ├── asymmetric.rs
│   │               ├── chained.rs
│   │               ├── key_value.rs
│   │               ├── fence.rs
│   │               ├── cont.rs
│   │               ├── block_marker.rs
│   │               ├── block_numbered.rs
│   │               ├── line_marker.rs
│   │               └── line_uniform.rs
│   └── tests/
│       ├── text_parser/       ← white-box mechanism tests
│       ├── trybuild.rs        ← UI tests for define_parser!
│       └── ui/
│
├── meon-macros/               ← proc-macro crate
│   └── src/
│       ├── lib.rs             ← define_parser! entry point
│       ├── cursor.rs          ← hand-rolled token stream cursor
│       ├── collect.rs         ← grammar front-end (DSL → CF)
│       ├── codegen.rs         ← back-end (CF → define_content! call)
│       ├── methods.rs         ← back-end (CF → accessor impl)
│       ├── model.rs           ← CF, StandaloneRule, crate_path()
│       ├── error.rs           ← located error type
│       └── strip.rs           ← token surgery (remove [N] annotations)
│
├── meon-md/                   ← Markdown grammar built on meon
├── benches/                   ← criterion benchmarks
└── fuzz/                      ← cargo-fuzz harness
```

---

## 2. Design goals and constraints

**Single forward pass.** The full parser (`parse_text!`) scans the source
exactly once, left-to-right. There is no backtracking, no look-ahead beyond
the current line, and no heap allocation for parser state (the active-block
slot is a single stack-allocated `Option<(u8, u8, u8, u32)>`).

**Flat SoA output.** The content struct stores one `Vec` per element kind.
All spans are `u32` byte offsets into the original source slice, which is
borrowed for the content lifetime. This layout gives O(1) access per element
kind and is cache-friendly for downstream processing.

**Declarative grammar, zero hand-written scanning.** The grammar author writes
one `define_parser!` invocation. Every parser method — `parse`, `find_*`,
`_clean`, `_raw` — is generated at compile time. There is no runtime dispatch
and no vtable.

**Stable-first.** The default build compiles on stable Rust. SIMD acceleration
is opt-in via Cargo feature flags that require nightly.

**No unsafe.** The crate contains no `unsafe` code. Span arithmetic uses
`saturating_sub` and `.min(len)` at the boundaries that could otherwise
underflow or overflow.

---

## 3. The two-crate split

The engine is split across two crates for a structural reason: proc-macro
crates must be compiled as a different kind of library (dylib loaded by the
compiler), and they cannot export ordinary items alongside `#[proc_macro]`
entry points.

```
meon-macros   — proc-macro crate
  define_parser!  (the single public entry point)
  ↓  emits tokens that call:
meon          — runtime crate
  define_content!         (declarative macro, #[macro_export])
  parse_text!             (declarative macro, #[macro_export])
  parse_inline!           (declarative macro, #[macro_export])
  parse_line!             (declarative macro, #[macro_export])
  parse_block!            (declarative macro, #[macro_export])
  define_standalone_fns!  (declarative macro, #[macro_export])
  *Iter structs           (standalone iterator implementations)
```

Grammar authors depend on `meon` (which re-exports `define_parser!` from
`meon-macros`) and only `meon`. The split is invisible from the outside.

---

## 4. Grammar compilation pipeline

When the compiler expands `define_parser!(Name { … })` the following stages
run in sequence, entirely at compile time:

```
Source tokens (grammar DSL)
        │
        ▼
  [cursor.rs] hand-rolled TokenStream cursor
        │  walks the token soup section by section
        ▼
  [collect.rs] grammar front-end
        │  fills CF (collected fields) and Vec<StandaloneRule>
        ▼
  [strip.rs] token surgery
        │  removes => field [N] annotations from grammar sections
        │  so the cleaned tokens can be passed to runtime macros
        ▼
  [codegen.rs] back-end: content struct emission
        │  emits: define_content!(Name { … })
        ▼
  [methods.rs] back-end: accessor emission
        │  emits: impl<'a> NameContent<'a> { str, bytes, *_clean, *_raw }
        ▼
  [codegen.rs] back-end: standalone DSL emission
        │  emits: define_standalone_fns! { … }
        ▼
  Final token stream handed back to rustc
```

The proc-macro itself (`expand` in `lib.rs`) is thin: it drives the cursor,
calls the three collect functions, calls strip, and assembles the output from
the three back-end builders.

---

## 5. Intermediate representation

The collected fields (CF) is a plain struct accumulated by the front-end:

```rust
struct CF {
    inline:       Vec<(Ident, TokenStream, Literal)>,  // field, Type, div
    inline_simple: Vec<(Ident, Literal)>,              // field, div
    line:         Vec<(Ident, TokenStream, Literal)>,
    block:        Vec<(Ident, TokenStream, Literal)>,
    block_simple: Vec<(Ident, Literal)>,
    standalone:   Vec<StandaloneRule>,
}
```

`StandaloneRule` is an enum with one variant per grammar construct that
supports standalone scanning:

```
SymmetricExact  { field, byte, count }
AsymmetricExact { field, open, close, count }
Chained         { field, open1, close1, open2, close2, prefix, ty, pf, ff, sf }
KeyValue        { field, eq, end, allow_sep, ty, kf, vf }
LineMarker      { field, byte, max, ty, var, body }
LineUniform     { field, bytes, min, ty, var, body }
Fence           { field, byte, min }
Cont            { field, byte }
BlockMarker     { field, bytes, ty, var, body }
BlockNumbered   { field, end_bytes, ty, num_var, kind_var, body }
```

Each variant carries exactly the literals and identifiers needed to emit both
the `find_*` method (via `define_standalone_fns!`) and the `_clean` / `_raw`
accessors (via `methods.rs`). Nothing more is stored.

---

## 6. Code generation

### 6.1 `define_content!`

`build_define_content` emits a single call to the runtime macro:

```
mc::define_content!(Name {
    inline        { field: Type [div], … }
    inline_simple { field [div], … }
    line          { field: Type [div], … }
    block         { field: Type [div], … }
    block_simple  { field [div], … }
});
```

`mc` is the resolved runtime crate path (`crate` when expanding inside `meon`
itself, the imported name otherwise). See §16 for the hygiene mechanism.

`define_content!` itself (a declarative macro in `engine/content.rs`) expands
this into two types:

- `NameState` — mutable accumulator, all fields `pub(crate)`.
- `NameContent<'a>` — public output struct borrowing the source.

It also emits `push_<field>` and `push_merge_<field>` methods on `NameState`
for every `inline_simple` field. The merge variant coalesces adjacent spans
(gap ≤ 1 byte) into a single entry.

### 6.2 Parser struct and `parse` method

The `define_parser!` expansion emits:

```rust
pub struct NameParser;

impl NameParser {
    pub fn parse(source: &[u8]) -> NameContent<'_> {
        mc::parse_text!(
            source;
            sep = …, eol = …, tab = …, escape = …;
            inline  { /* stripped grammar */ }
            lines   { /* stripped grammar */ }
            blocks  { /* stripped grammar */ }
        )
    }

    mc::define_standalone_fns! { sep=…, eol=…, tab=…, escape=…; … }
}
```

The grammar sections passed to `parse_text!` have been stripped of their
`=> field [N]` annotations by `strip.rs` — those annotations are only
meaningful to the proc-macro front-end and would be syntax errors inside the
runtime macros.

### 6.3 Accessor methods

`build_content_methods` emits one `_clean` / `_raw` pair per `StandaloneRule`
and one `_clean` accessor for every simple field that has no standalone rule.

`_clean` returns the inner content slice (between delimiters).
`_raw` returns the full slice including delimiter bytes, using
`saturating_sub` at the low boundary and `.min(source.len())` at the high
boundary to prevent out-of-bounds access.

---

## 7. Runtime architecture

`parse_text!` is a multi-stage declarative macro that operates in compile-time
accumulation phases before emitting the actual O(n) parsing loop.

### Accumulation stages (compile time)

```
parse_text!(src; sep=…, eol=…, tab=…, escape=…; <sections>)
    │
    ├─ @cs  — split raw sections into typed buckets [inline], [lines], [blocks]
    │
    ├─ @ci  — walk the inline bucket:
    │         extract merge_simple flag
    │         extract fallback field name
    │         extract hard_break rule
    │         collect on_trigger byte sets → finders list
    │         collect on_trigger bodies → ilt (inline token list)
    │
    ├─ @cb  — walk the blocks bucket:
    │         extract block_simple rules → sr (simple rules)
    │         extract block rules        → br (block rules)
    │         extract fallback paragraph field
    │
    └─ @body — emit the actual parsing loop
               with all resolved buckets inlined as tt fragments
```

Every stage is a separate macro arm identified by an `@`-prefixed internal
token. The final `@body` arm receives everything as flat `tt` fragments and
emits a single Rust block expression that is the body of `parse`.

### Main loop (runtime)

```
pos = 0
while pos < len:

    if at_line_start:
        if blank line → flush paragraph, close active cont block, advance
        find current_line_end

        loop:
            try parse_block! active arm
                → Some((false, cs))  — active block consumed this line
            try parse_block! open arm
                → Some((true,  cs))  — new block opened
            try parse_line!
                → Some(cs)           — whole-line rule matched
            break if no progress

        if any of the above matched → continue to next line

    if active fence (discriminant 0) → skip to next line (no inline scan)

    find next trigger byte or eol using find_any([$eol, $($f),*], …)

    if eol hit:
        check hard-break, flush text, advance
    else (trigger byte hit):
        call parse_inline! for this line
        advance to next line
```

The loop invariant is that `pos` always advances. The hard-break check and
all text flushing happen at eol boundaries, not inside `parse_inline!`.

---

## 8. Content struct and state accumulator

The content struct has five field categories, each with a distinct storage
layout that directly reflects parsing semantics:

| Section        | Field type              | Populated by        |
|----------------|-------------------------|---------------------|
| `inline`       | `Vec<T>`                | `parse_inline!`     |
| `inline_simple`| `Vec<Span>`             | `parse_inline!`     |
| `line`         | `Vec<(T, Span)>`        | `parse_line!`       |
| `block`        | `Vec<(T, Span)>`        | `parse_block!`      |
| `block_simple` | `Vec<Span>`             | `parse_block!`      |

`Vec<T>` — user-defined type with multiple `Span` fields (links, key-values).
`Vec<Span>` — single byte range, no extra metadata.
`Vec<(T, Span)>` — per-element metadata paired with a content span.

The `NameState` accumulator is pre-allocated with `source.len() / div`
capacity per field. At the end of the parse, `into_content(source)` moves all
vecs into the public struct and attaches the source reference. There is no
copying of span data.

---

## 9. Inline parsing

`parse_inline!` drives a single-pass scan over one logical line. It is called
by `parse_text!` when a trigger byte is found before the eol.

### Accumulation phases (compile time)

The macro first collects all rules from the `inline { … }` section into typed
buckets:

```
finders  — list of on_trigger byte sets (one per on_trigger block)
sy_rules — symmetric rules
as_rules — asymmetric rules
ch_rules — chained rules
kv_rules — key_value rules
hb       — hard_break rule (at most one)
```

Then it transitions to `@body` with all buckets flattened into tt fragments.

### Execution (runtime)

At the start of each line, hard-break detection trims the line end: if the
last byte is the escape byte, or if there are ≥ min trailing sep bytes, the
effective line end is shortened and a hard-break flag is set.

The main inner loop:

```
pos = start of line
text_start = start of line
pending: Option<(byte, open_pos, open_count, depth)> = None

loop:
    find next trigger byte using find_any(finders, src[pos..line_end])
    if none → break

    check escape (odd number of preceding escape bytes → skip)

    count consecutive delimiter bytes (count, delim_start)

    try chained rules (open1 delimiter)
    try symmetric rules (matching delimiter)
    try asymmetric rules (open delimiter)
    try key_value rules (eq delimiter)

flush remaining text_start..line_end
emit hard-break if flagged
```

**Symmetric `parse_inside = true` (pending mode):** the first run of `count`
bytes sets `pending`. The next run of the same byte and same count closes the
span. Content between them is scanned for inline elements by the outer loop
(since the loop continues after setting pending). This handles nesting like
`*italic with **bold** inside*`.

**Symmetric `parse_inside = false` (greedy mode):** used for code spans. On
finding an opening run of `count` bytes, the macro immediately scans forward
for a matching closing run using `memchr`. If found, the span is emitted and
the outer loop jumps past it. Content inside is not scanned.

**Balanced mode:** when `balanced = true`, nested pairs of the same delimiter
increment/decrement a depth counter before the outer close is accepted.

### `find_any` dispatch

The trigger byte search uses a single call to `swar::find_any` with a
const-size array. The compiler monomorphises this at compile time:

```
N=1 → memchr::memchr
N=2 → memchr::memchr2
N=3 → memchr::memchr3
N≥4 → SWAR / SIMD loop (see §13)
```

Multiple `on_trigger` blocks each contribute their bytes to the finders list.
The inner loop finds the minimum offset across all finder results to locate
the earliest trigger byte.

---

## 10. Line parsing

`parse_line!` is called at the start of each new line, before block and inline
processing. It tries each rule in declaration order and returns `Some(cs)` on
the first match, where `cs` is the byte offset of the first content byte (for
`line`) or the end of the line (for `line_simple`).

**`line(byte, max = N)`:** counts consecutive `byte` bytes from `pos`. If the
count is in [1, max] and is followed by `sep` or end of line, a match is
recorded. The span covers everything after the marker and its sep.

**`line_simple(b1 | b2 | …, min = N)`:** reads the first byte. If it matches
the pattern, the entire line is validated: every byte must be either the same
delimiter or `sep`. If valid and the delimiter count ≥ min, a match is
recorded.

Both rules use a simple fallthrough: if the first rule does not match, the
next is tried. If no rule matches, `None` is returned and `parse_text!` falls
through to block/inline processing.

---

## 11. Block parsing

`parse_block!` operates in two phases on every line:

### Active phase (`@active`)

If `active` is `Some(…)`, the line belongs to an open block:

- **Fence (discriminant 0):** check if the line is a closing fence (≥
  `flen` fence bytes, rest is `sep`/`tab` only). If yes, push span and clear
  active. Return `Some((false, next_line))` in all cases — fenced content is
  never passed to inline scanning.
- **Cont (discriminant 1):** check if the line starts with the continuation
  byte. If yes, return `Some((false, cs))` advancing past the marker. If no,
  push span and clear active, return `None` so the line is re-processed by
  the open phase.

### Open phase (`@open_simple` then `@open_block`)

`@open_simple` tries `fence` and `cont` rules to start a new multi-line block.
`@open_block` tries marker and numbered rules to open a new single-line item.

These phases are tried only if the active phase returned `None`. The first
match wins.

### Active block encoding

The active slot `Option<(u8, u8, u8, u32)>` encodes:

```
(0, fence_byte, fence_count, start_offset)  — open fence
(1, cont_byte,  0,           start_offset)  — open continuation
```

Only one block can be active at a time. This is a deliberate constraint (see
§17).

---

## 12. Standalone iterators

Every rule that supports standalone scanning generates a `find_*` method via
`define_standalone_fns!`. Each method constructs one of the iterator structs
from `engine/text_parser/standalone/`.

All standalone iterators share the same contract:

- They scan the raw source independently.
- They carry no cross-element state (no active block slot, no paragraph
  tracking, no inline trigger dispatch).
- They may match bytes that `parse_text!` would suppress (e.g. a delimiter
  inside a fenced block).
- Their output can differ from the full parse by design.

### Iterator structure

Every iterator stores the parameters passed to its `new` constructor plus a
`pos` cursor and, for line-bounded iterators, a `line_end` cursor.

The `next` method loops: advance `line_end` when exhausted, find the next
candidate using `memchr`, validate the candidate, return `Some(span)` on
success or continue the loop on failure.

Iterators use three shared utilities from `standalone/common.rs`:

- `find_line_end(src, from, eol)` — locate end of current line.
- `advance_line(src, line_end, eol)` — move to next line, return `None` at EOF.
- `count_escape(src, pos, escape)` — count consecutive escape bytes before
  `pos`, used to detect escaped delimiters.

### Iterator types

| Type                | Matching rule         | Item type       |
|---------------------|-----------------------|-----------------|
| `SymmetricExactIter`| `symmetric N =>`      | `Span`          |
| `AsymmetricExactIter`| `asymmetric N =>`    | `Span`          |
| `ChainedIter`       | `chained`             | `T`             |
| `KvIter`            | `key_value`           | `T`             |
| `LineMarkerIter`    | `line`                | `(T, Span)`     |
| `LineUniformIter`   | `line_simple`         | `(T, Span)`     |
| `FenceIter`         | `fence`               | `Span`          |
| `ContIter`          | `cont`                | `Span`          |
| `BlockMarkerIter`   | `block (pattern)`     | `(T, Span)`     |
| `BlockNumberedIter` | `block num(…)`        | `(T, Span)`     |

---

## 13. Multi-byte search — SWAR and SIMD

The `swar` module provides `find_any<const N>`, the single search primitive
used throughout the engine.

### Dispatch

```rust
pub fn find_any<const N: usize>(targets: [u8; N], src: &[u8]) -> Option<usize> {
    match N {
        1 => memchr::memchr(targets[0], src),
        2 => memchr::memchr2(targets[0], targets[1], src),
        3 => memchr::memchr3(targets[0], targets[1], targets[2], src),
        _ => find_any_wide(targets, src),
    }
}
```

The `match N` is folded at monomorphisation time — no branch is emitted at
runtime. For N=1..3 the highly optimised `memchr` crate handles everything.
For N≥4 the engine's own `find_any_wide` is used.

### SWAR fallback (stable, default)

For N≥4, `find_any_wide` processes 8 bytes per iteration using a single `u64`
word:

```
ONES  = 0x0101_0101_0101_0101   (broadcast multiplier)
HIGHS = 0x8080_8080_8080_8080   (high-bit mask)

broadcast(b)          = b as u64 * ONES
has_byte(chunk, cast) = (chunk ^ cast).wrapping_sub(ONES) & !(chunk ^ cast) & HIGHS
```

`has_byte` returns a mask with the high bit set in each byte-lane that equals
the target. If any lane matches, the trailing-zero count divided by 8 gives
the byte index.

For multiple targets, masks are OR-ed:

```rust
let mut mask = 0u64;
for &bcast in &bcasts {
    mask |= has_byte(chunk, bcast);
}
```

A scalar tail handles the remaining < 8 bytes.

### SIMD acceleration (nightly, opt-in)

With `--features avx2` or `--features avx512`, the crate gates on
`feature(portable_simd)` and replaces the SWAR loop with a wider SIMD loop:

```
avx2:   32-byte Simd<u8, 32> lanes
avx512: 64-byte Simd<u8, 64> lanes, then 32-byte, then 8-byte
```

The SIMD loop uses `simd_eq` + bitmask extraction. The SWAR path is compiled
away entirely when a SIMD feature is active.

---

## 14. Span representation

```rust
pub struct Span {
    pub start: u32,
    pub end: u32,
}
```

A half-open byte range `[start, end)`. All values produced by the parser
satisfy `start <= end <= source.len()`.

`u32` was chosen over `usize` to halve the span size (8 bytes vs 16 bytes
on 64-bit). This matters when a document produces tens of thousands of spans —
the vecs stay in fewer cache lines. The trade-off is the 4 GiB input limit
(`MAX_INPUT_LEN = u32::MAX as usize`), which exceeds any realistic document.

Zero-length spans (`start == end`) are used for position-only markers such as
hard-break anchors.

The fuzz harness explicitly asserts `start <= end && end <= source.len()` for
every span of every element kind after each parse. This is the primary
correctness invariant.

---

## 15. Accessor methods

Every content struct gets the following methods generated by `methods.rs`:

**`str(span) -> Option<&str>`** — slice the source by span, validate UTF-8.
Returns `None` on invalid UTF-8 instead of panicking. Panicking on
user-controlled input from a library is unsound.

**`bytes(span) -> &[u8]`** — slice the source by span, no UTF-8 check.

**`<field>_clean()`** — iterator over inner content slices (between delimiters).

**`<field>_raw()`** — iterator over full slices including delimiter bytes. Uses
`saturating_sub` on the low boundary and `.min(source.len())` on the high
boundary to prevent out-of-bounds indexing.

The raw accessor logic differs per rule type:

- `SymmetricExact`: `start - count .. end + count`
- `AsymmetricExact`: `start - count .. end + 1`
- `Chained`: `ff.start - 1 - prefix_len .. sf.end + 1`
- `BlockMarker`: `start - 2 .. end` (marker + sep)
- `BlockNumbered`: walks backwards past sep/tab and digit bytes

---

## 16. Cross-crate macro hygiene

The proc-macro emits calls to `meon`'s declarative macros from arbitrary
dependent crates. Unqualified macro names would fail to resolve. The solution:

### `crate_path()`

`model.rs` resolves the runtime crate name at proc-macro expansion time:

```rust
pub(crate) fn crate_path() -> TokenStream {
    match crate_name("meon") {
        Ok(FoundCrate::Itself)       => quote! { crate },
        Ok(FoundCrate::Name(name))   => { let i = Ident::new(&name, …); quote! { #i } },
        Err(_)                        => quote! { crate },
    }
}
```

`proc_macro_crate::crate_name("meon")` reads the calling crate's `Cargo.toml`
at compile time and returns the name under which `meon` was imported (handling
renames via `package = "meon"`).

### Emission

Every macro call in the generated code is prefixed with the resolved path:

```rust
let mc = crate_path();
quote! {
    #mc::define_content!(…);
    #mc::parse_text!(…);
    #mc::define_standalone_fns! { … }
}
```

### `$crate` in declarative macros

Inside `define_content!` and `parse_text!`, references to other items from
the `meon` crate use `$crate::`:

```rust
$crate::span::Span
$crate::parse_text!(@dispatch …)
$crate::swar::find_any(…)
```

This is standard declarative macro hygiene and works regardless of how `meon`
is imported.

### External crate dependencies in macro expansions

`paste` and `memchr` are used inside `define_content!` and `parse_text!`
without `$crate::` qualification — declarative macros cannot forward external
crate paths through `$crate`. The engine re-exports both:

```rust
// meon/src/lib.rs
#[doc(hidden)] pub use paste;
#[doc(hidden)] pub use memchr;
```

And references them as `$crate::paste::paste!` and `$crate::memchr::memchr`
inside the macros. Grammar crates only need to depend on `meon`; `paste` and
`memchr` are transitive.

---

## 17. Known limitations and deliberate trade-offs

### Single active-block slot

`parse_text!` holds at most one open block in `active: Option<(u8, u8, u8, u32)>`.
This means nested block constructs — a continuation block containing a fence,
or a fence containing a continuation — cannot be represented simultaneously.

Concretely, `> \`\`\`` (blockquote containing fenced code) produces an
incorrect span: the fence opens and the continuation state is lost. Nested
`> >` (blockquote inside blockquote) leaks inner content into the outer span.

The trade-off is deliberate: a single stack-allocated tuple fits in a register,
produces no heap allocation, and covers the vast majority of real documents.
A correct solution would require a stack of active blocks, adding allocation
and complexity.

### Context-free inline scanning

`parse_inline!` receives the cleaned line content (after block detection) and
scans it independently of surrounding lines. There is no cross-line inline
state. This means:

- Emphasis spanning multiple lines is not detected.
- Precedence between overlapping inline rules follows declaration order, not
  any precedence table.

### Standalone vs full-parse divergence

Standalone iterators (`find_*`) produce different results from the full parse
in several cases:

- A delimiter inside a fenced block is suppressed by the full parser (which
  tracks the active fence slot) but matched by the standalone iterator (which
  has no such state).
- An escaped delimiter is suppressed by the full parser's escape check but
  may be matched by the standalone iterator if its escape logic differs.

This is documented and by design. Standalone iterators trade correctness for
speed in the single-element-kind case.

### 4 GiB input limit

Spans store byte offsets as `u32`. Inputs larger than `u32::MAX` bytes would
produce silently truncated spans. The fuzz harness guards against this with an
early return. In practice no text document approaches this limit.

---

## 18. Extension points

### Adding a new grammar crate

Create a new crate that depends on `meon`. Call `define_parser!` once with the
new grammar. The crate becomes entirely self-contained — it has its own
`NameParser`, `NameContent`, and `find_*` methods with no shared mutable state.

### Adding a new rule kind

Adding a new rule kind requires changes in both crates:

**meon-macros:**
1. Add a variant to `StandaloneRule` in `model.rs`.
2. Parse the new syntax in `collect.rs` (the appropriate `collect_*` function).
3. Emit the standalone DSL entry in `codegen.rs` (`build_standalone_dsl`).
4. Emit the `_clean` / `_raw` accessors in `methods.rs`.

**meon:**
1. Add a matching arm to `define_standalone_fns!` in `standalone/mod.rs`.
2. Implement the iterator struct in a new file under `standalone/`.
3. Re-export the struct from `lib.rs`.
4. Add the new syntax to the appropriate runtime macro (`parse_inline!`,
   `parse_line!`, or `parse_block!`).

### SIMD backends

New SIMD backends can be added to `swar.rs` by adding a new Cargo feature,
gating on `portable_simd`, and adding a `search_simd!` invocation inside
`find_any_wide`. The N=1..3 dispatch to `memchr` is unaffected.

### Replacing the allocator

`meon` uses the global allocator for all `Vec` allocations. Replacing it with
`mimalloc` or another allocator is the responsibility of the binary/grammar
crate, not the engine. No changes to `meon` are needed.

---

## License

`meon` is available under the
[***GNU Affero General Public License v3.0 (AGPL-3.0)***](https://github.com/vgnapuga/meon/blob/main/LICENSE).

If the AGPL-3.0 terms are incompatible with your use case, a commercial
license is available — see [***COMMERCIAL.md***](https://github.com/vgnapuga/meon/blob/main/COMMERCIAL.md).

By contributing, you agree to the [***Contributor License Agreement***](https://github.com/vgnapuga/meon/blob/main/CLA.md).
