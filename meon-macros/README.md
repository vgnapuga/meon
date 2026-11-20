# meon-macros

Procedural macro crate for the `meon` parsing engine.

This crate exposes a single public entry point — `define_parser!` — which
compiles a declarative grammar description into a fully working parser at
compile time. It is not intended to be used directly: depend on `meon` instead,
which re-exports `define_parser!` and provides the runtime infrastructure.

* **meon**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon/README.md)
  * [***crates.io***](https://crates.io/crates/meon)
* **meon-macros**    <--
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
  * [***MD_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/MD_COMPARE.md)
  * [***JSON_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/JSON_COMPARE.md)
* [***FUZZING.md***](https://github.com/vgnapuga/meon/blob/main/fuzz/README.md) - *GitHub*

---

## Usage

Do not add `meon-macros` to your `Cargo.toml` directly.

```toml
[dependencies]
meon = "0.7"
```

```rust
use meon::define_parser;

define_parser!(MyFormat {
    sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';

    limits {
        offset = u32;
        max_nest = 4;
        max_inline_span = 0;
        max_line_span = 0;
        max_block_span = 0;
        max_input_bytes = 0;
    }

    inline {
        fallback => texts [10];
    }
    blocks {
        fallback => paragraphs [80];
    }
});

let content = MyFormatParser::parse(b"hello world\n").unwrap();
```

For the full grammar reference see
[`meon/README.md`](https://github.com/vgnapuga/meon/blob/main/meon/README.md).

`limits { ... }` is optional: `offset` selects the span offset type (`u8`, `u16`, `u32`, default `u32`), `max_nest` sets the shared nesting cap, and `max_inline_span`, `max_line_span` and `max_block_span` cap emitted span lengths in bytes. `0` and the offset type's maximum mean absent, so no check or fallback code is generated; a value above the offset maximum is rejected.

`max_input_bytes` caps the source length. `0` and the offset type's maximum mean that maximum; `_` or an absent key means no check and the parser reads only the prefix that fits. When no check is generated, the checked methods return `Ok` directly. A value above the offset maximum is rejected.

---

## What `define_parser!` generates

Given `define_parser!(Name { ... })`, the macro expands into:

**`NameContent<'a>`** — the output struct. One `pub` field per grammar rule,
all borrowing from the original source slice via byte-offset spans whose type defaults to `u32`.

**`NameParser`** — a unit struct with checked and unchecked methods:
- `parse(source: &[u8]) -> Result<NameContent<'_>, InputBytesTooLarge>` — full
  parse with linear search work per fixed grammar.
- `parse_unchecked(source: &[u8]) -> NameContent<'_>` — the same parse without
  the `max_input_bytes` check.
- `find_<field>(source: &[u8]) -> Result<impl Iterator, InputBytesTooLarge>`
  and `find_<field>_unchecked` — standalone per-element iterators, one pair per
  grammar rule that supports it. Context-free and faster than a full parse when
  only one element kind is needed.
- `context(source: &[u8]) -> Result<ParseContext, InputBytesTooLarge>` and
  `context_unchecked` — the opaque-region map (fences plus opaque
  symmetric/asymmetric rules and chained rules with both components opaque),
  built by a forward trigger scan with bounded searches.
- `find_context_<field>(source, &ctx) -> Result<impl Iterator, InputBytesTooLarge>`
  and `find_context_<field>_unchecked` — for transparent symmetric/asymmetric
  and non-fence line/block rules; candidates inside opaque regions are skipped.

**Accessor methods on `NameContent`:**
- `str(span) -> Option<&str>` — span to UTF-8 string, `None` on invalid UTF-8.
- `bytes(span) -> &[u8]` — span to byte slice.
- `<field>_clean()` — iterator over inner content slices (between delimiters).
- `<field>_raw()` — iterator over full slices including delimiter bytes.
  `_raw` is generated only for fields backed by a standalone rule; fallback
  and `hard_break` fields get `_clean` only. For numbered blocks, the number
  is recovered with the grammar's declared digit pattern, including ranges and
  alternatives. `AB: x` therefore stays intact for
  `num(b'A'..=b'F', end = b':')`; `_clean()` returns `x`.

---

## Internal pipeline

`define_parser!` is a thin wrapper around a compile-time pipeline implemented
entirely in `meon-macros`:

```
Grammar DSL tokens
    │
    ▼
[cursor.rs]   hand-rolled TokenStream cursor
    │
    ▼
[collect.rs]  grammar front-end
    │          fills CF (collected fields) + Vec<StandaloneRule>
    │          the only stage that reports grammar errors
    ▼
[strip.rs]    removes => field [N] annotations
    │          so the cleaned tokens pass to runtime macros
    ▼
[normalize.rs] canonicalises the inline section into the one
    │          token shape the runtime macro_rules! patterns match
    ▼
[codegen.rs]  emits define_content!(...) call
[methods.rs]  emits _clean / _raw accessor impl
[codegen.rs]  emits define_standalone_fns! { ... } call
    │
    ▼
Final token stream → rustc
```

All runtime behaviour lives in `meon` (the `parse_text!`, `parse_inline!`,
`parse_line!`, `parse_block!`, and `define_standalone_fns!` declarative
macros). `meon-macros` only produces tokens; it has no runtime footprint.

The front-end is the single authority on the shape a grammar may take. It
reads the grammar by keyword, so sub-rules and settings within a section can
be written in any order and a trailing `;` or `,` is optional; the normalize
pass then rewrites what it accepted into the fixed shape the runtime patterns
require. The four context bytes in the header keep their fixed order
(`sep`, `eol`, `tab`, `escape`), with the optional `max_nest` and `crlf`
after them. Whatever `define_parser!` accepts, the runtime macros accept.

For a detailed description of each stage see
[`ARCHITECTURE.md §4`](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE.md#4-grammar-compilation-pipeline) - *GitHub*.

---

## Error reporting

A malformed grammar emits a located `compile_error!` spanned at the offending
token rather than a proc-macro panic. Example:

```
error: `fence` rule needs `min = N`
 --> src/lib.rs:7:35
  |
7 |     blocks { block_simple { fence(b'`') => fenced_codes [400]; } }
  |                                   ^^^^
```

What the front-end rejects:

- An unknown keyword in a section — the error names the section and lists the
  keywords it does accept.
- A missing `parse_inside` in a symmetric rule; `balanced` on symmetric is
  rejected. Asymmetric rules and chained components require both flags, each
  set to `true` or `false`.
- An asymmetric rule with `balanced = true` or `parse_inside = true` must
  declare exactly one arm, `1 => field`; other counts, wildcard arms and
  multiple arms are rejected.
- An incomplete `chained` (fewer than two components, or no `prefix`) or
  `key_value` (a missing `eq`, `allow_sep`, `end`, `key`, or `value`).
- A missing `[div]` capacity, a missing rule body, and a missing literal in
  any position that requires one (`fence` min, `line` max, `symmetric` byte,
  the context bytes).

---

## Grammars inside `macro_rules!`

Invisible groups around forwarded literal and pattern fragments are unwrapped
where the DSL expects those fragments. Expression fragments in metadata bodies
retain their grouping: `$weight * 2` keeps the expression's original precedence.
Capacities, context settings and pattern arguments may therefore be forwarded
from a declarative macro, as exercised by `tests/ui/pass/macro_grammar.rs`.

## Cross-crate hygiene

All macro calls emitted by the expansion are fully qualified via
`proc_macro_crate::crate_name` so the generated code resolves correctly
regardless of how `meon` is imported or renamed in `Cargo.toml`.

See [`ARCHITECTURE.md §16`](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE.md#16-cross-crate-macro-hygiene) - *GitHub*
for the full explanation.

---

## License

`meon-macros` is available under the
[***MIT***](https://github.com/vgnapuga/meon/blob/main/LICENSE-MIT) *OR* [***APACHE-2.0***](https://github.com/vgnapuga/meon/blob/main/LICENSE-APACHE) license - *GitHub*.
