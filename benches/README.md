# meon — Benchmarks

EN | [**RU**](https://github.com/vgnapuga/meon/blob/main/benches/README_RU.md)

Throughput benchmarks for the [`meon-md`](https://github.com/vgnapuga/meon/blob/main/meon-md/README.md)
and [`meon-json`](https://github.com/vgnapuga/meon/blob/main/meon-json/README.md)
reference grammars, built on the [`meon`](https://github.com/vgnapuga/meon/blob/main/meon/README.md)
engine. They exist to track engine performance across changes and feature
flags — **not** to claim a ranking against other parsers (see
[Scope & fairness](#scope--fairness)).

| Bench                 | Measures                                                             |
|-----------------------|----------------------------------------------------------------------|
| `meon-md_parse`       | `MarkdownParser::parse` — full single-pass parse.                    |
| `meon-md_standalone`  | `find_*` iterators — one element kind, no context.                   |
| `meon-md_compare`     | meon-md vs `pulldown-cmark` / `comrak` — cross-parser throughput.    |
| `meon-json_parse`     | `JsonParser::parse` (+ `type_scalars`) — structural / typed parse.   |
| `meon-json_compare`   | meon-json vs `simd-json` / `sonic-rs` — cross-parser throughput.     |

Per-corpus composition reports, full result tables, and the cross-parser
numbers live in their own documents — this file is the overview, how-to-run,
fairness frame and test hardware. For the cross-parser comparisons see
[***MD_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/MD_COMPARE.md)
(Markdown) and
[***JSON_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/JSON_COMPARE.md)
(JSON).

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
* ***BENCHMARKS.md***    <--
  * [***MD_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/MD_COMPARE.md)
  * [***JSON_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/JSON_COMPARE.md)
* [***FUZZING.md***](https://github.com/vgnapuga/meon/blob/main/fuzz/README.md)

---

## Corpora

### Markdown corpora (`meon-md_*`)

Each base document is tiled `REPEAT_COUNT` times (default `10`) so the working
set comfortably exceeds cache.

| Corpus  | Shape                                                                  | Stresses                                                          |
|---------|------------------------------------------------------------------------|-------------------------------------------------------------------|
| `plain` | Prose only, no markup.                                                 | Fallback/text path, line loop. Ceiling case (near-pure scanning). |
| `hot`   | Light, evenly spread markup (~one of each common inline per paragraph).| Typical real-world document.                                      |
| `heavy` | Dense: headings, rules, quotes, fences, lists, nested inline.          | Every rule family at once, including nesting. Stress case.        |

> **Synthetic data notice.** These corpora are generated programmatically with
> uniform, predictable structure. Real-world documents typically have **lower
> element density** than `hot` or `heavy` — and lower density means less
> per-element work, so real throughput usually sits **at or above** the
> `hot`/`heavy` numbers. Read `hot`/`heavy` as a conservative lower bound, with
> `plain` (markup-free) as the ceiling, and your workload somewhere in between.

Exact per-corpus element counts are in
[***MD_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/MD_COMPARE.md).

### JSON corpora (`meon-json_*`)

Each corpus is one valid top-level JSON array, scaled by `COUNT`
(`benches/benches/docs_json.rs`); the `small` and `big` runs differ only in
`COUNT`.

| Corpus    | Shape                                                                  | Stresses                                                                 |
|-----------|------------------------------------------------------------------------|--------------------------------------------------------------------------|
| `numbers` | Flat array of numbers / bools / nulls.                                 | Scalar scanning. meon does least; validating parsers parse every number. |
| `objects` | Array of flat objects with mixed-typed fields.                         | Members, keys, typed scalars. A typical API payload.                     |
| `nested`  | Array of moderately nested objects (objects-in-objects, small arrays). | The unified nesting stack and the string rule.                           |

> **Synthetic data notice.** These corpora are generated programmatically with
> uniform structure; real JSON is less regular. Treat the figures as a
> demonstration of the architectural difference (a structural reader vs a
> validating parser), not as expected production throughput.

Exact per-corpus composition is in
[***JSON_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/JSON_COMPARE.md).

---

## Running

Inside `nix develop`:

```sh
# Stable, scalar SWAR path:
cargo bench --bench meon-md_parse
cargo bench --bench meon-md_standalone
cargo bench --bench meon-md_compare
cargo bench --bench meon-json_parse
cargo bench --bench meon-json_compare

# Nightly, AVX2 SIMD path, tuned for the host CPU:
RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_parse        --features avx2
RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_standalone   --features avx2
RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_compare      --features avx2
RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-json_parse      --features avx2
RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-json_compare    --features avx2
```

Criterion knobs (`SAMPLE_SIZE`, `SAMPLE_TIME`, `WARMUP_TIME`) live in
`benches/benches/docs_md.rs` and `benches/benches/docs_json.rs`. Defaults favour
a quick local run; raise them for publication-grade numbers.

---

## Scope & fairness

- **Intra-engine first.** `meon-md_parse` / `meon-md_standalone` /
  `meon-json_parse` measure *this* engine over *these* corpora. They are
  meaningful for "did my change regress?" and "how much does AVX2 help?", not
  for a leaderboard.
- **Cross-parser comparisons are architectural, not rankings.** `meon-md` emits
  flat spans for a Markdown *subset* (no AST, reference-link resolution, or
  rendering); `meon-json` is a *structural reader* (no validation, number
  parsing, or string unescaping). The comparisons — against `pulldown-cmark` /
  `comrak`
  ([***MD_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/MD_COMPARE.md))
  and against `simd-json` / `sonic-rs`
  ([***JSON_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/JSON_COMPARE.md))
  — are framed there as architectural differences, not quality rankings.
- **End-to-end cost.** The timed region includes the internal `Vec`
  allocations, because that is what a real caller pays. Input and output are
  `black_box`-ed; document generation is outside the timed region.

---

## Performance notes

- **`max_nest` has a flat per-line cost.** The block-level active-block stack
  and the inline engine's bounded stacks are `[T; max_nest]` arrays
  zero-initialised on *every* `parse_block!` / `parse_inline!` call, regardless
  of whether that line actually nests. Set `max_nest` to the smallest value
  your grammar needs (`meon-md` uses `4`, `meon-json` uses `64`); a larger cap
  costs throughput on every inline-bearing line whether or not anything nests.
- **AVX-512 is implemented but not benchmarked.** The `avx512` feature exists
  (see [`swar.rs`](https://github.com/vgnapuga/meon/blob/main/meon/src/swar.rs))
  but no AVX-512 hardware was available during development. Contributions with
  real numbers are welcome.

---

## Test hardware

```
CPU:             AMD Ryzen 5 5625U (Zen 3)
RAM:             16 GB
OS:              NixOS 25.05
rustc (stable):  1.86.0
rustc (nightly): 1.98.0-nightly
Environment:     nix develop (isolated shell)
```
