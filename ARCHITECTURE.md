# meon — Architecture

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

* [***FAQ.md***](https://github.com/vgnapuga/meon/blob/main/FAQ.md)
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
19. [Span limits](#19-span-limits)

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
│   │               ├── context.rs        ← ParseContext, ContextCursor
│   │               ├── context_iter.rs   ← context-aware inline iterators
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
│       ├── strip.rs           ← token surgery (remove [N] annotations)
│       └── normalize.rs       ← canonicalise inline for the runtime macros
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
All spans are byte offsets into the original source slice, which is
borrowed for the content lifetime. The offset type defaults to `u32` and is
selected by `limits { offset = ...; }`. This layout gives O(1) access per element
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
        │  the only stage that reports grammar errors
        ▼
  [strip.rs] token surgery
        │  removes => field [N] annotations from grammar sections
        │  so the cleaned tokens can be passed to runtime macros
        ▼
  [normalize.rs] canonicalisation of the inline section
        │  rewrites what the front-end accepted into the one token shape
        │  the runtime macro_rules! patterns match
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

The front-end reads the grammar by keyword, so sub-rules and settings may be
declared in any order and a trailing `;` or `,` is optional. The runtime macros
are `macro_rules!` patterns and match exactly one token shape: inside an
`on_trigger { ... }` block the sub-rules in the order `symmetric`,
`asymmetric`, `chained`, `key_value`; only `parse_inside` in a symmetric
body, and `balanced` before `parse_inside` in an asymmetric one; `prefix` last in a
`chained` body; `eq`, `allow_sep`, `end`, `key`, `value` in that order in a
`key_value` body; every setting closed by `;` and every arm by `,`.

`normalize.rs` rewrites the former into the latter, which makes the front-end
the single authority on the shape a grammar may take: whatever
`define_parser!` accepts, the runtime macros accept. The rewrite is
semantically neutral — the runtime buckets sub-rules by kind and dispatches the
kinds in a fixed order, and the relative order *within* a kind is preserved.
It runs on the stripped stream and only after the front-end has validated the
grammar, so every keyword it needs can be assumed present.

The proc-macro itself (`expand` in `lib.rs`) is thin: it drives the cursor,
calls the three collect functions, calls strip and then normalize on the
`inline` tokens, and assembles the output from the three back-end builders.

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

The optional `limits` section is parsed by `limits.rs` from `expand()`. Its
`max_nest` and the three span caps are *not* part of `CF`: they are forwarded
as literals or optional tokens to `parse_text!`, `parse_inline!`,
`parse_line!`, `parse_block!` and the standalone DSL, rather than being
threaded through the collected-fields structure.

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
    // Emitted only when `max_input_bytes` is set.
    fn __meon_check_input(source: &[u8]) -> Result<(), InputBytesTooLarge> {
        if source.len() > MAX as usize {
            Err(InputBytesTooLarge { len: source.len(), max: MAX })
        } else {
            Ok(())
        }
    }

    pub fn parse(
        source: &[u8],
    ) -> Result<NameContent<'_>, InputBytesTooLarge> {
        Self::__meon_check_input(source)?; // omitted when no limit
        Ok(Self::parse_unchecked(source))
    }

    pub fn parse_unchecked(source: &[u8]) -> NameContent<'_> {
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
grammar omitted it — `expand()` defaults the literal to `1` before emitting
this call. The three span caps are forwarded as optional `[]`/`[N]` tokens:
`[]` means no cap and generates no limit code.

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

The `NameState` accumulator holds the source length and one `Vec` per field,
all starting unallocated. `define_content!` generates a `push_<field>` method
per field — the only append path the runtime macros use — which reserves
`source.len() / div` elements the first time that field receives one. A rule
that never matches therefore never allocates: a grammar with a dozen element
kinds costs what the document actually contains, not what the grammar could
describe.

The reservation adds nothing to the common push. `push_<field>` makes the
same `len == capacity` test `Vec::push` makes; with room left it calls
`Vec::push` in the branch where the compiler already knows the vector is not
full, so the second test is dropped and the path is a plain store. A full
vector — including one never allocated — takes an out-of-line `#[cold]`
function that reserves the hint if the capacity is still zero and then
pushes, so `Vec` grows as usual past the hint.

At the end of the parse, `into_content(source)` moves all vecs into the public
struct and attaches the source reference. There is no copying of span data.

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

**Every stack-eligible construct — symmetric, asymmetric, AND key_value — lives
on one shared stack**, bounded by the grammar-wide `max_nest` (forwarded from
`parse_text!`; default `1`):

```
frames:        [(u8, u8, u32); max_nest]   // (byte, count, vidx)
fdepth:        usize                        // single budget for ALL kinds
kv_pending:    [(u32, u32, u32); max_nest]  // (key_start, key_end, value_start)
held_base:     [(u32, u32); max_nest]       // fallback-vector snapshot per frame
asym_overflow: u32                          // one-shot counter for balanced opens past the cap
```

A frame stores only `(byte, count, vidx)` — **there is no kind tag**. Each
frame's kind, close byte and field routing are recovered at the point
of use by matching the stored `byte` against each rule's own compile-time
literal — the same idiom the open side already used to route a field. This
rests on the engine-wide assumption that a byte has one meaning across the
stack-eligible rules of an `on_trigger` block.

- **asymmetric** — `byte` = the rule's open byte; `count` = 1 always (each byte
  of an open run is its own event); `vidx` = index of the placeholder span
  pushed at open and back-patched on close. Opaque asymmetric rules use a
  separate forward search rather than these frames.
- **symmetric** (`parse_inside = true`) — `byte` = the
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

### Off-stack constructs

These never touch the unified stack:

- **symmetric `parse_inside = false`** (greedy, code spans) — escape-aware
  forward search.
- **asymmetric `parse_inside = false`** — a forward search, using exact
  opening counts when unbalanced and depth-counting rings when balanced.
  Its close byte is not required in `on_trigger`; opaque balanced depth
  does not consume `max_nest`.
- **chained** — a two-phase transparent state machine, or a two-phase forward
  search where a component is opaque. Its phases are strictly sequential —
  phase 2 only starts once phase 1 has fully closed — so one slot per phase
  suffices: no stack, no `max_nest` consumed.

### Close bytes known to be absent

Unbalanced opaque asymmetric searches remember absent close bytes in a
per-run bitset. Opaque `chained` rules instead share `ChainMemo` between the
full parse, standalone finders and the context builder. Unbalanced components
retain scalar absent/found-close memos. When nested first components move
second starts backwards, a known absent suffix bounds the new prefix search.

A failure involving a balanced chained component lazily indexes the rest of
its inline run. A forward pass marks unescaped delimiters, excluding line
markers; a reverse pass pairs them. Each cell records the close balancing a
virtual opener after that byte. Cells at closing bytes also store the reverse
stack's parent links, so no separate working stack is allocated. An exact
reservation follows an initial run-end scan. The single reusable `Vec<u32>`
contains one plane per balanced component: four bytes per indexed byte per
plane, up to eight with two planes. Earlier successful chains consume disjoint
ranges, and subsequent balanced queries use constant-time lookups. The search
work is linear per rule. Transparent chained phases continue to use their
existing streaming state machine.

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
- **symmetric** — removes only the unclosed placeholder with `remove(vidx)`,
  preserving any completed inner entry in the same field.

### Plain text of a frame that never closes

An unclosed frame loses its placeholder, not its literal source bytes.
Fallback spans are appended even while frames are open. `held_base[fdepth]`
records the fallback length and previous last-span end; a successful close
rolls back the covered text before emitting the completed element. Discarding
an unclosed frame performs no rollback. Thus `*a\nb\n` keeps fallback
`*a\nb`, including the opener; a closed inner code span survives without
its bytes also being duplicated as fallback. Transparent chained phases use
the same held-text mechanism, including across other frame closes.

### Bounded-cap overflow

Beyond `max_nest`, an extra same-type **asymmetric** open with `balanced = true`
bumps `asym_overflow` instead of pushing; the next same-type close consumes one
overflow unit instead of popping, so the real tracked frame's close isn't
mistaken early. A **symmetric** open past the cap, and a **key_value** `eq` past
the cap, open no frame and remain literal text (the pair/run is untracked).

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

The bytes from all `on_trigger` blocks, including key/value ends, form one
union. Each iteration searches that union once for the earliest trigger byte.
`eol` is included as one extra target only on multi-line calls (see above).

---

## 10. Line parsing

`parse_line!` is called after block handling when no block rule claimed the
line, before scanning its remaining content for inline elements. It tries each rule in declaration order and returns `Some(cs)` on
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
engine (§9). `max_nest = 1` reduces it to a single slot: at most one block
open at a time, no block opening inside another.

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

Standalone iterators follow one rule's own semantics: escapes, opening-run
counts, balanced depth or bounded transparent frames, and blank-line limits.
They do not inspect other rules. A standalone match may therefore occur inside
another rule's fence/string or span a line that another rule would split.
Context-aware iterators additionally consult the shared opaque map and line
reader. This is a difference in available context, not a different escape or
balancing contract.

Line/marker predicates are probed over the byte domain at construction. Up to
three candidate bytes use streaming `memchr` searches; wider sets fall back to
line-by-line scanning. Patterns are preserved as written for `line_simple`,
`fence`, marker blocks and both `num` arguments. `LineBreaks` supplies `eol`,
`crlf`, `sep`/`tab` blank-line recognition, and the callback deciding whether a
search may cross a new line. Context construction uses `LineReader` to respect
paragraph, block-line, fence and same-marker continuation boundaries.

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

Opaque symmetric and exact asymmetric arms are selected after measuring the
opening run, using both byte and count. An unsupported count stays literal;
balanced asymmetric and chained rules accept every opening run length.
Line readers treat a CR as part of a line ending only before an actual `eol`;
a bare CR at EOF is content, not an empty final line.

Transparent symmetric/asymmetric and non-fence line/block rules generate
`find_context_*(source, &ctx)`: the same matcher with candidate delimiters
inside opaque regions skipped. Line/block rules post-filter their
context-free iterator through a monotone `ContextCursor` (amortized O(1)
per query); `symmetric`/`asymmetric` rules use the dedicated iterators
above, whose searches also respect opaque regions and the grammar's line
structure. `key_value` and `chained` have no context-aware consumer. Opaque rules themselves get
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
A grammar that sets `max_input_bytes` to `0` or the offset maximum makes the
checked methods reject input over that limit; an absent key or `_` reads only
the prefix that fits.

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

**`<field>_raw()`** — iterator over full slices including delimiter bytes.
Delimiter arithmetic uses saturating/clamped bounds where applicable; caller-
mutated invalid content spans may still panic through checked slice indexing.

The raw accessor logic differs per rule type:

- `SymmetricExact`: `start - count .. end + count`
- `AsymmetricExact`: `start - count .. end + 1`
- `Chained`: `ff.start - 1 - prefix_len .. sf.end + 1`
- `BlockMarker`: `start - 2 .. end` (marker + sep)
- `BlockNumbered`: walks backwards past sep/tab and digit bytes

Accessor generation uses each rule's collected metadata. It does not rerun a
standalone iterator: `_clean` and `_raw` resolve the content's existing spans.
For numbered items, the backward digit check uses the declared digit pattern,
not an ASCII-only test.

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

Runtime macros refer to dependencies through `$crate` re-exports rather than
requiring grammar crates to import transitive dependency names. The engine
re-exports both `paste` and `memchr`:

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

The block-level active-block stack (§11) and the inline-level unified
nesting stack — shared by symmetric, asymmetric AND key_value rules (§9) —
share one grammar-wide `max_nest` setting. Its default, `1`, degenerates both
stacks to a single slot: at most one block active at a time, no nested stack
frames. Opaque balanced searches count depth outside these stacks and do not
use this cap. That default is what a grammar wants when its format has
no nesting, and it costs the same as a plain slot; raising it is what makes a
blockquote containing a fenced code block, a blockquote inside another, or
`**bold *italic* bold**` resolve into their own separate spans instead of the
inner construct leaking into — or overwriting — the outer one.

`max_nest` is a hard cap, not an unbounded stack — constructs nested deeper
than `max_nest` are not specially tracked:

- For blocks, depth beyond `max_nest` simply stops opening new frames; a
  fifth level of blockquote nesting at `max_nest = 4` is left for whatever
  processing follows (typically literal or inline content) rather than being
  represented as a fifth span.
- For transparent asymmetric rules with `balanced = true`, an extra
  same-type open beyond the cap increments a one-shot overflow counter and is
  treated as literal content instead of opening a frame; the overflow is
  consumed by the next same-type close, so the real tracked frame's close
  isn't mistaken early. A `key_value` `eq` past the cap is absorbed (the pair
  untracked and its bytes literal).

The trade-off is deliberate: a small, fixed-size, stack-allocated array —
sized by the grammar's own `max_nest`, not by input length — avoids heap
allocation for the frame arrays and keeps the common case (`max_nest = 1`, the default) at
single-slot cost, while letting a grammar opt into deeper nesting only where it
actually needs it.

### Transparent `chained` rules share one active match slot

A `chained` rule with a transparent component (`parse_inside = true` on
either the text or url side) tracks its in-progress match in a small set of
local variables shared across every `chained` rule in the grammar, not a
per-rule array. A rule-byte discriminator prevents another chained rule from advancing the
active rule's phases, but there is still only one active transparent match at
a time. Fully opaque rules have independent search memos and are not subject
to that active-phase restriction.

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
  blank lines mid-value. What is discarded is the construct, not its content —
  the unmatched opening bytes and surrounding literal text remain as fallback
  (§9), while completed inner elements survive.
- Inline dispatch uses a fixed order of rule kinds, preserving declaration
  order within a kind; it is not a format-specific precedence table.

### Close bytes of stack-eligible rules must be declared in `on_trigger`

Closes are located by the same `find_any` scan as opens (§9), so a rule that
goes on the unified stack only ever sees its close byte if that byte is in the
same `on_trigger` set. A `key_value`'s `end` byte is added to the set
automatically; an `asymmetric` close byte has to be listed by hand, and a
grammar that omits it gets a rule that opens and never closes. The front-end
does not reject it, because the byte may legitimately belong to another
`on_trigger` block. Off-stack rules are unaffected — they find their close with
their own forward search.

### Standalone vs full-parse divergence

Standalone iterators (`find_*`) implement the same rule's escapes, runs and
nesting as the full parser, but omit interactions with other rules. A delimiter
inside another rule's string or fence may be found; another rule's line or
frame can also change a match's boundary in the full parse. Context-aware
finders use opaque spans and line classification to account for those cases.
Counts can differ by design; ignoring `balanced` or escapes is not that design.
Chained rules have a raw finder only: fully opaque matches are context sources,
while a rule with a transparent component is neither a context source nor a
context-aware consumer.

### 4 GiB input limit

Spans store byte offsets in the selected `offset` type. Inputs larger than its
maximum produce truncated spans unless a limit is set. A grammar can set
`max_input_bytes = 0` (or the offset maximum) to make its checked methods
reject them; an absent key or `_` reads only the prefix that fits. The fuzz harness guards against this with an early return.
In practice no text document approaches this limit.

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

## 19. Span and input limits

The optional `limits { ... }` section carries the offset type, the
grammar-wide nesting cap, three span-length caps and an input-length cap:

| Key               | Meaning                                                     |
|-------------------|-------------------------------------------------------------|
| `offset`          | Span offset type: `u8`, `u16`, `u32` or `u64`; default `u32`.|
| `max_nest`        | Shared cap of the inline and block stacks; default `1`.     |
| `max_inline_span` | Longest emitted inline span, in bytes; default absent.      |
| `max_line_span`   | Longest emitted line span, in bytes; default absent.        |
| `max_block_span`  | Longest emitted block span, in bytes; default absent.       |
| `max_input_bytes` | Longest accepted source, in bytes; default absent.          |

`max_nest` is also accepted in the header for existing grammars; if both are
present they must agree. The span caps are measured on the clean span,
`end - start` of the stored `Span`, without delimiters. `0` and the offset
type's maximum are normalized to absent; a value above the offset maximum is
a compile error.

`max_input_bytes` is not normalized to absent. `0` and the offset type's
maximum both mean that maximum; the checked methods reject a larger source.
`_` or an absent key means no check, and the parser reads only the prefix that
fits. A value above the offset maximum is a compile error. The check runs at the start of every
checked method — `parse`, `find_*`, `find_context_*`, `context` — before any
parser state, context map or `Vec` is allocated. The unchecked variants skip
it.

The front-end emits each cap as an optional token, `[]` or `[N]`. `[]` means
no cap: the runtime macros emit no comparison, no fallback push and no reparse
helper for that level. `[N]` is emitted as the smallest unsigned literal that
holds the value (`u8`, `u16` or `u32`); comparisons are done in `u32`.

### Full parse

- **Inline simple spans** — a span longer than the cap is not emitted. A
  transparent symmetric/asymmetric frame is removed and its plain text stays
  in the fallback field; its completed inner elements remain.
- **Opaque inline spans** (`parse_inside = false`) — the span is dropped and
  its interior is reparsed with the same grammar. The delimiters become text.
  A `chained` element over the cap is dropped and its interior reparsed.
  A `key_value` pair is dropped and is not reparsed; its key and value spans
  and any inner elements remain.
- **Line and block spans** — a line rule, block item or `cont` span over its
  cap is not emitted; the span itself is pushed to the grammar's
  `blocks { fallback => ... }` field, while inline content keeps being
  scanned. A fence over the cap is also reparsed: its content is split into
  paragraph runs at blank lines, `cont` markers of the frames below it are
  skipped, and the fence is no longer opaque.
- **Fallback fields** — `inline` fallback text is never capped. The block
  fallback paragraph is not pushed when its own span exceeds the block cap.

The reparse helper is generated only when a cap needs it. Its recursion depth
is bounded by `max_nest`. It scans an interior as an inline run; for fence
content it first splits the content into paragraph runs and skips the
continuation markers of the frames that held the fence.

### Standalone and context

Every `find_*` and `find_context_*` iterator applies the same caps: a yielded
span longer than its level's cap is skipped. Context-free opaque finders skip
an over-limit match and resume after it; they do not search its interior for
the same kind. `ParseContext` drops an over-limit opaque region and rescans
its interior for nested regions, so context-aware finders see the same
structure the full parse reparses. A fence over `max_block_span` is not added
to the context map and its content is rescanned. Over-limit line and block
items are filtered by their finders.

---

## License

`meon` is available under the
[***MIT***](./LICENSE-MIT) *OR* [***APACHE-2.0***](./LICENSE-APACHE) license.
