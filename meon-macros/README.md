# meon-macros

EN | [**RU**](https://github.com/vgnapuga/meon/blob/main/meon-macros/README_RU.md) - *GitHub* 

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

* [***CHANGELOG.md***](https://github.com/vgnapuga/meon/blob/main/CHANGELOG.md)
* [***ARCHITECTURE.md***](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE.md) - *GitHub*
* [***BENCHMARKS.md***](https://github.com/vgnapuga/meon/blob/main/benches/README.md) - *GitHub*
* [***FUZZING.md***](https://github.com/vgnapuga/meon/blob/main/fuzz/README.md) - *GitHub*

---

## Usage

Do not add `meon-macros` to your `Cargo.toml` directly.

```toml
[dependencies]
meon = "0.2"
```

```rust
use meon::define_parser;

define_parser!(MyFormat {
    sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';

    inline {
        fallback => texts [10];
    }
    blocks {
        fallback => paragraphs [80];
    }
});

let content = MyFormatParser::parse(b"hello world\n");
```

For the full grammar reference see
[`meon/README.md`](https://github.com/vgnapuga/meon/blob/main/meon/README.md).

---

## What `define_parser!` generates

Given `define_parser!(Name { ... })`, the macro expands into:

**`NameContent<'a>`** — the output struct. One `pub` field per grammar rule,
all borrowing from the original source slice via `u32` byte-offset spans.

**`NameParser`** — a unit struct with:
- `parse(source: &[u8]) -> NameContent<'_>` — full single-pass O(n) parse.
- `find_<field>(source: &[u8]) -> impl Iterator` — standalone per-element
  iterators, one per grammar rule that supports it. Context-free and faster
  than a full parse when only one element kind is needed.

**Accessor methods on `NameContent`:**
- `str(span) -> Option<&str>` — span to UTF-8 string, `None` on invalid UTF-8.
- `bytes(span) -> &[u8]` — span to byte slice.
- `<field>_clean()` — iterator over inner content slices (between delimiters).
- `<field>_raw()` — iterator over full slices including delimiter bytes.

---

## Internal pipeline

`define_parser!` is a thin wrapper around a three-stage compile-time pipeline
implemented entirely in `meon-macros`:

```
Grammar DSL tokens
    │
    ▼
[cursor.rs]   hand-rolled TokenStream cursor
    │
    ▼
[collect.rs]  grammar front-end
    │          fills CF (collected fields) + Vec<StandaloneRule>
    ▼
[strip.rs]    removes => field [N] annotations
    │          so the cleaned tokens pass to runtime macros
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

For a detailed description of each stage see
[`ARCHITECTURE.md §4`](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE.md#4-grammar-compilation-pipeline) - *GitHub*.

---

## Error reporting

A malformed grammar emits a located `compile_error!` spanned at the offending
token rather than a proc-macro panic. Example:

```
error: expected literal (fence min)
 --> src/lib.rs:7:35
  |
7 |     blocks { block_simple { fence(b'`') => fenced_codes [400]; } }
  |                                   ^^^^
```

---

## Cross-crate hygiene

All macro calls emitted by the expansion are fully qualified via
`proc_macro_crate::crate_name` so the generated code resolves correctly
regardless of how `meon` is imported or renamed in `Cargo.toml`.

See [`ARCHITECTURE.md §16`](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE.md#16-cross-crate-macro-hygiene) - *GitHub*
for the full explanation.

---

## License

`meon-macros` is available under the
[***GNU Affero General Public License v3.0 (AGPL-3.0)***](https://github.com/vgnapuga/meon/blob/main/LICENSE) - *GitHub*.

If the AGPL-3.0 terms are incompatible with your use case, a commercial
license is available — see [***COMMERCIAL.md***](https://github.com/vgnapuga/meon/blob/main/COMMERCIAL.md) - *GitHub*.

By contributing, you agree to the [***Contributor License Agreement***](https://github.com/vgnapuga/meon/blob/main/CLA.md) - *GitHub*.
