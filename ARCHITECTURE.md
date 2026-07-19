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
* **meon-json**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon-json/README.md)
  * [***crates.io***](https://crates.io/crates/meon-json)


* [***CHANGELOG.md***](https://github.com/vgnapuga/meon/blob/main/CHANGELOG.md)
* ***ARCHITECTURE.md***    <--
* [***BENCHMARKS.md***](https://github.com/vgnapuga/meon/blob/main/benches/README.md)
  * [***MD_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/MD_COMPARE.md)
  * [***JSON_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/JSON_COMPARE.md)
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
├── meon-json/                 ← JSON reader grammar built on meon
├── benches/                   ← criterion benchmarks
└── fuzz/                      ← cargo-fuzz harness
```

---

## 2. Design goals and constraints

**Single forward pass.** The full parser (`parse_text!`) scans the source
exactly once, left-to-right. There is no backtracking and no heap allocation
for parser state — the active-block state and the inline engine's unified
bounded-nesting stack (one stack shared by symmetric, asymmetric and
`key_value` rules) are all fixed-size, stack-allocated arrays sized by the
grammar's `max_nest` setting (see §9, §11, §17). Inline scanning may run over a
multi-line fallthrough run rather than a single line (§7, §9), but it still
streams strictly forward — a run is accumulated, never re-scanned.

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

When the compiler expands `define_parser!(Name { ... })` the following stages
run in sequence, entirely at compile time:

```
Source tokens (grammar DSL)
        │
        ▼
  [cursor.rs] hand-rolled TokenStream cursor
        │  walks the token soup section by section (including the optional
        │  `max_nest` context value, alongside sep/eol/tab/escape)
        ▼
  [collect.rs] grammar front-end
        │  fills CF (collected fields) and Vec<StandaloneRule>
        ▼
  [strip.rs] token surgery
        │  removes => field [N] annotations from grammar sections
        │  so the cleaned tokens can be passed to runtime macros
        ▼
  [codegen.rs] back-end: content struct emission
        │  emits: define_content!(Name { ... })
        ▼
  [methods.rs] back-end: accessor emission
        │  emits: impl<'a> NameContent<'a> { str, bytes, *_clean, *_raw }
        ▼
  [codegen.rs] back-end: standalone DSL emission
        │  emits: define_standalone_fns! { ... }
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

Note that `max_nest` itself is *not* part of `CF` — it is parsed directly in
`lib.rs`'s `expand()` as a standalone `Literal`, alongside `sep`/`eol`/`tab`/
`escape`, and forwarded straight through to `parse_text!`'s call site rather
than being threaded through the collected-fields structure.

---

## 6. Code generation

### 6.1 `define_content!`

`build_define_content` emits a single call to the runtime macro:

```
mc::define_content!(Name {
    inline        { field: Type [div], ... }
    inline_simple { field [div], ... }
    line          { field: Type [div], ... }
    block         { field: Type [div], ... }
    block_simple  { field [div], ... }
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
            sep = ..., eol = ..., tab = ..., escape = ..., max_nest = ...;
            inline  { /* stripped grammar */ }
            lines   { /* stripped grammar */ }
            blocks  { /* stripped grammar */ }
        )
    }

    mc::define_standalone_fns! { sep=..., eol=..., tab=..., escape=...; ... }
}
```

`max_nest` is always forwarded explicitly to `parse_text!`, even when the
grammar omitted it — `lib.rs`'s `expand()` defaults the literal to `1` before
emitting this call, rather than relying on `parse_text!`'s own default arm.

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
parse_text!(src; sep=..., eol=..., tab=..., escape=..., max_nest=...; <sections>)
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
        if blank line → flush paragraph, close active block frames, advance
        find current_line_end

        loop:
            try parse_block! active arm
                → Some((false, cs))  — active stack consumed/continued this line
            try parse_block! open arm
                → Some((true,  cs))  — new block opened (possibly several, nested)
            try parse_line!
                → Some(cs)           — whole-line rule matched
            if the active stack's depth changed without a match (an outer
                continuation just closed mid-line) → retry from the same
                line start against the now-shallower stack
            break if no progress

        if any of the above matched → continue to next line

    if innermost active frame is a fence → skip to next line (no inline scan)

    if the line matched no line/block rule and no block is active:
        defer — record/extend the current fallthrough run (para_start) and
        advance to the next line WITHOUT inline-scanning it yet
    else (trailing content after a matched line/block marker):
        inline-scan that content with a single-line-bounded parse_inline! call

    a deferred fallthrough run is flushed as ONE multi-line parse_inline! call —
    and its paragraph span recorded — when it closes: at a blank line, at a line
    where a line/block rule matches, or at end of input (see §9, "Multi-line runs")
```

The loop invariant is that `pos` always advances. A fallthrough run is scanned
once over its whole multi-line extent, not per line, so the unified inline
stack persists across the `\n` bytes inside it. The hard-break check and text
flushing happen at run boundaries, not inside the per-character `parse_inline!`
loop.

---

## 8. Content struct and state accumulator

The content struct has five field categories, each with a distinct storage
layout that directly reflects parsing semantics:

| Section        | Field type              | Populated by        |
|----------------|--------------------------|---------------------|
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

`parse_inline!` drives a single-pass scan over one **run** of source — either a
single line (mid-line content after a `line`/`block` match) or a whole
multi-line fallthrough span handed to it by `parse_text!` as one call (see §7
and "Multi-line runs" below). It is not meant to be called directly.

### Accumulation phases (compile time)

The macro collects the `inline { ... }` section into typed buckets:

```
finders  — on_trigger byte sets (each key_value's `end` byte auto-added)
sy_rules — symmetric rules
as_rules — asymmetric rules
ch_rules — chained rules
kv_rules — key_value rules
hb       — hard_break rule (at most one)
```

Then it transitions to `@body` with all buckets flattened into tt fragments.

### The unified nesting stack

Before nesting existed, symmetric used one pending slot and asymmetric one
forward search. The engine now hosts **every stack-eligible construct —
symmetric, asymmetric, AND key_value — on one shared stack**, bounded by the
grammar-wide `max_nest` (forwarded from `parse_text!`; default `1`):

```
frames:        [(u8, u8, u32); max_nest]   // (byte, count, vidx)
fdepth:        usize                        // single budget for ALL kinds
kv_pending:    [(u32, u32, u32); max_nest]  // (key_start, key_end, value_start)
asym_overflow: u32                          // one-shot counter for balanced opens past the cap
```

A frame stores only `(byte, count, vidx)` — **there is no kind tag**. Each
frame's kind, close byte, opacity, and field routing are recovered at the point
of use by matching the stored `byte` against each rule's own compile-time
literal — the same idiom the open side already used to route a field. This
rests on the engine-wide assumption that a byte has one meaning across the
stack-eligible rules of an `on_trigger` block.

- **asymmetric** — `byte` = the rule's open byte; `count` = 1 always (each byte
  of an open run is its own event); `vidx` = index of the placeholder span
  pushed at open and back-patched on close. Opacity is derived from the stored
  open byte, not stored per frame.
- **symmetric** (`parse_inside = true, balanced = true`) — `byte` = the
  delimiter; `count` = run length (picks the field 1/2/3 → italics/bolds/
  bold_italics, and is matched exactly on close); `vidx` = placeholder index.
- **key_value** — `byte` = the rule's `end` byte; `count` = 0 (unused);
  `vidx` = 0 (unused). Pending data lives in `kv_pending` at the frame's own
  stack position. Unlike the other two, a kv pair is **not** pushed at open;
  it is appended complete only when its value closes — keeping `Vec<T>` in
  value-close order (an outer pair lands after every pair nested in its value),
  the opposite of the open-order convention asymmetric/symmetric use for their
  placeholder-at-open vectors.

`fdepth` is the single budget for all three kinds combined; every "is anything
open" check is `fdepth == 0` regardless of which kinds are on the stack.

### Off-stack constructs (original pre-nesting paths, intact)

These never touch the unified stack and run their original code:

- **symmetric `parse_inside = true, balanced = false`** — the single `pending`
  slot.
- **symmetric `parse_inside = false`** (greedy, code spans) — forward search;
  gained escape-awareness only.
- **asymmetric `balanced = false, parse_inside = false`** (autolinks) — the
  `memchr`/depth forward search; its close byte is NOT required in `on_trigger`.
- **chained** — a two-phase transparent state machine (or the original opaque
  two-phase forward search). Its phases are strictly sequential — phase 2 only
  starts once phase 1 has fully closed — so one slot per phase suffices: no
  stack, no `max_nest` consumed.

### Closing — one unified pass with a key_value drain

A close byte runs a **single** pass, never one block per rule (which would
double-pop a shared close byte). Per close character: first drain any
`key_value` frame on top (its container is closing, so the value ends here,
committed before the container pops); then, if the new top is an asymmetric
frame whose grammar-known close byte matches this character, pop it and
back-patch its placeholder. Dispatch is by the frame's **own recorded open
byte**, so two rules sharing a close byte (`(`/`)` and `[`/`)`) close the
correct frame. Because the stack is strict LIFO and a kv frame always sits
above the container it lives in, a `}}` run finalises the inner pair then its
object, then the outer pair — correct nesting falls out of the per-character
loop.

Grammar consequence: once a rule is on the stack, its close byte (and a
`key_value`'s `end`) must be in the same `on_trigger` set, since closes are
found by the same `find_any` scan as opens. `key_value`'s `end` is auto-added;
an asymmetric close must currently be listed manually (see §17).

### End-of-run drain

A frame still open when the run ends is drained top→down, kind recovered by
stored byte:

- **key_value** — pushed complete, value finalised to the run end (so a
  terminator-less `key = value` still emits); the text cursor is advanced past
  it so the final plain-text flush does not re-emit the value.
- **asymmetric** — discarded via `Vec::remove(vidx)` (a closed inner
  self-nesting entry can sit at a *higher* index than a still-open outer one).
- **symmetric** — discarded via `truncate(vidx)` (an identical `(byte, count)`
  never self-nests, so each field has at most one live placeholder, always
  last).

### Bounded-cap overflow

Beyond `max_nest`, an extra same-type **asymmetric** open with `balanced = true`
bumps `asym_overflow` instead of pushing; the next same-type close consumes one
overflow unit instead of popping, so the real tracked frame's close isn't
mistaken early. A **symmetric** open past the cap, and a **key_value** `eq` past
the cap, are simply absorbed (no frame, the pair/run untracked).

### Multi-line runs

`parse_text!` may hand `parse_inline!` a span covering several source lines as
one call (§7). Inside such a run an `eol` is ordinary content: the unified
stack persists across it, so an open container or a pending `key_value` value
survives the line break and only drains at the run's true end. `eol` joins the
unified `find_any` trigger set **only** when the multiline call passes
`multiline = true`; the single-line-bounded call passes `false` (its span is
`\n`-free by construction, so searching for `eol` there would be a dead
per-chunk comparison). When `hard_break` is declared, an internal `\n` in a
multi-line run is checked for a hard break the same way the run's end is.

### `find_any` dispatch

The trigger search is a single `swar::find_any` call with a const-size array,
monomorphised at compile time:

```
N=1 → memchr::memchr
N=2 → memchr::memchr2
N=3 → memchr::memchr3
N≥4 → SWAR / SIMD loop (see §13)
```

Multiple `on_trigger` blocks each contribute their bytes; the inner loop takes
the minimum offset across finder results to locate the earliest trigger byte.
`eol` is included as one extra target only on multi-line calls (see above).

---

## 10. Line parsing

`parse_line!` is called at the start of each new line, before block and inline
processing. It tries each rule in declaration order and returns `Some(cs)` on
the first match, where `cs` is the byte offset of the first content byte (for
`line`) or the end of the line (for `line_simple`).

**`line(byte, max = N)`:** counts consecutive `byte` bytes from `pos`. If the
count is in [1, max] and is followed by `sep` or end of line, a match is
recorded. The span covers everything after the marker and its sep.

**`line_simple(b1 | b2 | ..., min = N)`:** reads the first byte. If it matches
the pattern, the entire line is validated: every byte must be either the same
delimiter or `sep`. If valid and the delimiter count ≥ min, a match is
recorded.

Both rules use a simple fallthrough: if the first rule does not match, the
next is tried. If no rule matches, `None` is returned and `parse_text!` falls
through to block/inline processing.

---

## 11. Block parsing

`parse_block!` resolves a line's full block structure in one call, via two
phases:

### Peel phase

Every currently-open frame is matched against the line from the outside in:
continuation markers are consumed, a still-open fence either closes or
swallows the line, and a frame whose marker is gone is closed — together
with everything nested inside it.

### Open phase

At the position left after peeling, new frames are opened — as many as nest
on a single line (e.g. a fence opening inside an already-open blockquote) —
bounded by `max_nest`.

### Active block stack (`max_nest`)

The active block state is a bounded stack `[(u8, u8, u8, u32); max_nest]`
plus a depth counter, sharing the grammar-wide `max_nest` cap with the inline
engine (§9). `max_nest = 1` reduces it to a single slot and reproduces the
original, single-active-block behaviour exactly: at most one block open at a
time, no block opening inside another.

| Discriminant (field 0) | Meaning            | Field 1  | Field 2 | Field 3  |
|-------------------------|---------------------|----------|---------|----------|
| `0`                     | Open fence          | `byte`   | `count` | `start`  |
| `1`                     | Continuation (`>`)  | `byte`   | `0`     | `start`  |

Two structural invariants make the stack tractable:

- **A fence is always the top frame.** Fence content is opaque — no block
  can open inside it — so when a fence is the innermost open block it
  consumes the whole line and the open phase never runs; nothing is ever
  pushed above a fence.
- **Continuations may self-nest.** Unlike an inline symmetric delimiter
  (where open and close are indistinguishable, so an identical key can't
  nest), a `cont` opens positionally — at the line start, after the outer
  markers have been peeled — and closes by *absence* of its marker, so
  `> >` is two genuinely nested blockquote frames.

`block` items (bullets, ordered lists) are per-line leaves: they push nothing
onto the stack and so consume no depth. They may still open *inside* a
`cont`, but only when `max_nest > 1` — at `max_nest = 1` the open phase never
runs inside an already-open block, exactly as before.

If an outer continuation's marker is gone mid-line while inner frames were
still open, those inner frames close together with it (innermost-first), the
depth drops, and `parse_text!`'s main loop re-runs from the same line start
so the remainder of the line is reprocessed fresh against the now-shallower
stack — see §7's main-loop pseudocode.

### Return value

`Some((true, cs))` — at least one new block was opened; `cs` is the first
content byte (or the next line start, for a fence whose info line is
consumed whole). `Some((false, cs))` — the active stack consumed/continued
this line without opening anything new. `None` — nothing matched and nothing
is active, **or** an outer continuation just closed; in the latter case the
caller re-runs from the same line start to reprocess the remainder.

---

## 12. Standalone iterators

Every rule that supports standalone scanning generates a `find_*` method via
`define_standalone_fns!`. Each method constructs one of the iterator structs
from `engine/text_parser/standalone/`.

All standalone iterators share the same contract:

- They scan the raw source as a **byte stream**: one `memchr`-family search
  finds the next candidate marker, and only then is its neighbourhood walked.
  Lines without a marker are never visited; there is no per-line loop.
- They carry no cross-element state (no paragraph tracking, no inline trigger
  dispatch) — with one exception: `ContIter` keeps a bounded frame stack so
  same-type block nesting (`> >` opening two blockquote frames) matches the
  full parse, capped by the grammar's `max_nest` exactly like `parse_block!`.
- Inline pair matching is **paragraph-bounded**: a pair may span a single
  line break; an empty line (two consecutive `eol` bytes) or end of input
  aborts a pending opener. `symmetric`/`asymmetric` rules match only the
  *exact* count declared, ignoring `balanced` entirely.
- They may still match bytes that `parse_text!` would suppress (e.g. a
  delimiter inside a fenced block); output can differ from the full parse by
  design. The `find_context_*` variants (below) close most of that gap.

### Iterator structure

Every iterator stores the parameters passed to its `new` constructor plus a
`pos` cursor.

The `next` method loops: one `memchr`/`memchr2`/`memchr3` search from `pos`
finds the next candidate marker; an O(1) neighbour check validates its
position (previous byte for line-start rules, a backward indentation walk
for block rules); the candidate is then matched in place and `Some(...)`
returned, or the loop continues.

Rules whose marker set is an arbitrary predicate (`line_simple`,
`block (pattern)`, `block num(...)`) probe the predicate over all 256 byte
values at construction: up to three accepted bytes drive the streaming
search, more fall back to a line-by-line scan.

Iterators use shared utilities from `standalone/common.rs`:

- `find_line_end(src, from, eol)` — locate end of current line.
- `count_escape(src, pos, escape)` — count consecutive escape bytes before
  `pos`, used to detect escaped delimiters.
- `probe_matcher(matches, buf)` / `find_any_of(needles, n, hay)` — turn a
  byte predicate into a `memchr`-family streaming search.

### Iterator types

| Type                | Matching rule         | Item type       |
|---------------------|------------------------|-----------------|
| `SymmetricExactIter`| `symmetric N =>`      | `Span`          |
| `AsymmetricExactIter`| `asymmetric N =>`    | `Span`          |
| `ChainedIter`       | `chained`             | `T`             |
| `KvIter`            | `key_value`           | `T`             |
| `LineMarkerIter`    | `line`                | `(T, Span)`     |
| `LineUniformIter`   | `line_simple`         | `(T, Span)`     |
| `FenceIter`         | `fence`               | `Span`          |
| `ContIter`          | `cont`                | `Span`          |
| `BlockMarkerIter`   | `block (pattern)`     | `(T, Span)`     |
| `BlockNumberedIter` | `block num(...)`        | `(T, Span)`     |
| `ContextSymmetricExactIter`  | `symmetric N =>` (transparent)  | `Span` |
| `ContextAsymmetricExactIter` | `asymmetric N =>` (transparent) | `Span` |

### Context-aware variants — `ParseContext` and `find_context_*`

Rules with `parse_inside = false` (code spans, strings, autolinks) and
fences are *opaque*: the full parser never matches anything inside them.
The generated `Parser::context(source)` builds a `ParseContext` — a sorted,
non-overlapping set of every opaque region — in one streaming pass whose
needle set unifies the fence bytes and the opaque triggers (one
`memchr`/`memchr2`/`memchr3` search per iteration up to three distinct
bytes, `swar::find_any` beyond). The region vector is preallocated from the
grammar's own `[cap]` divisors. Opaque inline matching inside the builder is
escape-aware and paragraph-bounded, and a fence-opening line ends the
paragraph, mirroring the full parser.

Every *transparent* rule additionally generates
`find_context_*(source, &ctx)`: the same matcher with candidate delimiters
inside opaque regions skipped. Line/block rules post-filter their
context-free iterator through a monotone `ContextCursor` (amortized O(1)
per query); `symmetric`/`asymmetric` rules use the dedicated iterators
above, whose close search also skips covered candidates and aborts at a
fence — a block construct ends the paragraph. Opaque rules themselves get
no `find_context_*`: they are the source of the context, not a consumer.
The context suppresses **candidate positions**, not enclosing spans, so a
bold span may still legally contain a code span, exactly as in the full
parse.

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

These accessors are entirely separate from the full-parse bounded-nesting
mechanism described in §9/§11 — they operate on the (un-nested, exact-count)
`StandaloneRule` data, consistent with §12's note that standalone scanning
ignores `balanced`/`parse_inside` altogether.

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
        Ok(FoundCrate::Name(name))   => { let i = Ident::new(&name, ...); quote! { #i } },
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
    #mc::define_content!(...);
    #mc::parse_text!(...);
    #mc::define_standalone_fns! { ... }
}
```

### `$crate` in declarative macros

Inside `define_content!` and `parse_text!`, references to other items from
the `meon` crate use `$crate::`:

```rust
$crate::span::Span
$crate::parse_text!(@dispatch ...)
$crate::swar::find_any(...)
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

### Bounded nesting depth (`max_nest`)

Both the block-level active-block stack (§11) and the inline-level unified
nesting stack — shared by symmetric, asymmetric AND key_value rules (§9) —
share one grammar-wide `max_nest` setting. Its default, `1`, reproduces the
original single-slot/single-pending behaviour exactly: at most one block active
at a time, no self-nesting for `balanced` rules. Setting it higher resolves
what used to be hard limitations — a blockquote containing a fenced code block,
or a blockquote nested inside another, used to leak content into the wrong
span; a different-count inner emphasis delimiter used to silently overwrite the
single pending slot and lose the outer pair entirely.

`max_nest` is a hard cap, not an unbounded stack — constructs nested deeper
than `max_nest` are not specially tracked:

- For blocks, depth beyond `max_nest` simply stops opening new frames; a
  fifth level of blockquote nesting at `max_nest = 4` is left for whatever
  processing follows (typically literal or inline content) rather than being
  represented as a fifth span.
- For inline `asymmetric`/`symmetric` rules with `balanced = true`, an extra
  same-type open beyond the cap increments a one-shot overflow counter and is
  treated as literal content instead of opening a frame; the overflow is
  consumed by the next same-type close, so the real tracked frame's close
  isn't mistaken early. A `key_value` `eq` past the cap is absorbed (the pair
  untracked).

The trade-off is the same in spirit as the original single-slot design: a
small, fixed-size, stack-allocated array — sized by the grammar's own
`max_nest`, not by input length — avoids heap allocation entirely and keeps
the common case (`max_nest = 1`, the default) at the same cost as before,
while letting a grammar opt into deeper nesting only where it actually needs
it.

### `chained` rules are scoped to one active match per grammar

A `chained` rule with a transparent component (`parse_inside = true` on
either the text or url side) tracks its in-progress match in a small set of
local variables shared across every `chained` rule in the grammar, not a
per-rule array. Two `chained` rules with overlapping in-progress transparent
matches would alias this shared state. Every grammar seen so far declares
exactly one `chained` rule, so this is accepted as a documented limitation
rather than built out further.

### `key_value` is scoped to one rule per grammar

A `key_value` frame is distinguished on the unified stack only by its stored
`end` byte, and the key-segment anchor plus the "top-is-kv" test are shared
across every `key_value` rule. Two such rules — especially sharing an `end`
byte — cannot be told apart at close/drain, and the shared anchor/top-test
conflate them. One `key_value` rule per grammar is supported. This is a
consequence of the minimal frame carrying no kind tag (§9), not a fundamental
limit: a per-rule discriminator (e.g. stored in the kv frame's otherwise-unused
`count` slot) would lift it.

### Inline runs span multiple lines, bounded by blank lines

`parse_inline!` is handed a fallthrough run that may span several physical
lines (§9), not a single line. The unified stack persists across internal `eol`
bytes, so emphasis, a container, or a `key_value` value may span a line break
within one run. The run — and the stack — is bounded by a blank line, by a line
where a `line`/`block` rule matches, or by end of input. Consequences:

- A blank line *inside* an open construct closes the run and discards that
  construct: a JSON-shaped grammar (empty `lines`/`blocks`) must not contain
  blank lines mid-value.
- Precedence between overlapping inline rules still follows declaration order,
  not a precedence table.

### Standalone vs full-parse divergence

Standalone iterators (`find_*`) produce different results from the full parse
in several cases:

- A delimiter inside a fenced block is suppressed by the full parser (which
  tracks the active-block stack) but matched by the standalone iterator
  (which has no such state).
- An escaped delimiter is suppressed by the full parser's escape check but
  may be matched by the standalone iterator if its escape logic differs.
- Standalone `symmetric`/`asymmetric` rules match only the exact declared
  count and never participate in bounded nesting at all, regardless of the
  grammar's `max_nest` or that rule's own `balanced`/`parse_inside` settings.

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

If the new rule kind should support nesting, follow the existing `balanced`/
`parse_inside` pattern already used by `symmetric`/`asymmetric` (§9) rather
than inventing a new mechanism — reuse the grammar-wide `max_nest` cap and the
unified `frames` stack with its single close pass, so it composes correctly
with rules that already nest.

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
[***MIT***](./LICENSE-MIT) *OR* [***APACHE-2.0***](./LICENSE-APACHE) license.
