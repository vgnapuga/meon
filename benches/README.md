# meon-md — Benchmarks

EN | [**RU**](https://github.com/vgnapuga/meon/blob/main/benches/README_RU.md)

Throughput benchmarks for the [`meon-md`](https://github.com/vgnapuga/meon/blob/main/meon-md/README.md)
reference grammar, built on the [`meon`](https://github.com/vgnapuga/meon/blob/main/meon/README.md)
engine. They exist to track engine performance across changes and feature
flags — **not** to claim a ranking against other parsers (see
[Scope & fairness](#scope--fairness)).

| Bench                | Measures                                                                   |
|----------------------|----------------------------------------------------------------------------|
| `meon-md_parse`      | `MarkdownParser::parse` — full single-pass parse.                          |
| `meon-md_standalone` | `find_*` iterators — one element kind, no context.                         |
| `meon-md_compare`    | meon-md vs `pulldown-cmark` / `comrak` — cross-parser throughput.          |

Per-corpus composition reports, full result tables, and the cross-parser
numbers live in their own documents — this file is the overview, how-to-run,
fairness frame and test hardware. For the cross-parser comparison see
[***MD_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/MD_COMPARE.md).

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
* [***FUZZING.md***](https://github.com/vgnapuga/meon/blob/main/fuzz/README.md)

---

## Corpora

Each base document is tiled `REPEAT_COUNT` times (default `10`) so the working
set comfortably exceeds cache.

| Corpus  | Shape                                                                  | Stresses                                                          |
|---------|------------------------------------------------------------------------|-------------------------------------------------------------------|
| `plain` | Prose only, no markup.                                                 | Fallback/text path, line loop. Ceiling case (near-pure scanning). |
| `hot`   | Light, evenly spread markup (~one of each common inline per paragraph).| Typical real-world document.                                      |
| `heavy` | Dense: headings, rules, quotes, fences, lists, nested inline.          | Every rule family at once, including nesting. Stress case.        |

> **Synthetic data notice.** All three corpora are generated programmatically
> with uniform, predictable structure. Real-world documents typically have
> **lower element density** than `hot` or `heavy` — and lower density means
> less per-element work, so real throughput usually sits **at or above** the
> `hot`/`heavy` numbers. Read `hot`/`heavy` as a conservative lower bound, with
> `plain` (markup-free) as the ceiling, and your workload somewhere in between.

Exact per-corpus element counts (the composition report printed before each
run) are in
[***MD_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/MD_COMPARE.md).

---

## Running

Inside `nix develop`:

```sh
# Stable, scalar SWAR path:
cargo bench --bench meon-md_parse
cargo bench --bench meon-md_standalone
cargo bench --bench meon-md_compare

# Nightly, AVX2 SIMD path, tuned for the host CPU:
RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_parse      --features avx2
RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_standalone --features avx2
RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_compare    --features avx2
```

Criterion knobs (`SAMPLE_SIZE`, `SAMPLE_TIME`, `WARMUP_TIME`) live in
`benches/benches/docs_md.rs`. Defaults favour a quick local run; raise them for
publication-grade numbers.

---

## Scope & fairness

- **Intra-engine first.** `meon-md_parse` / `meon-md_standalone` measure *this*
  engine over *these* corpora. They are meaningful for "did my change regress?"
  and "how much does AVX2 help?", not for a leaderboard.
- **Not a CommonMark ranking.** `meon-md` emits flat spans for a Markdown
  *subset* and does no AST construction, reference-link resolution, or
  rendering. The cross-parser comparison against `pulldown-cmark` / `comrak`
  lives in
  [***MD_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/MD_COMPARE.md),
  framed as an architectural difference, not a quality ranking.
- **End-to-end cost.** The timed region includes the internal `Vec`
  allocations, because that is what a real caller pays. Input and output are
  `black_box`-ed; document generation is outside the timed region.

---

## Performance notes

- **`max_nest` has a flat per-line cost.** The block-level active-block stack
  and the inline engine's bounded stacks are `[T; max_nest]` arrays
  zero-initialised on *every* `parse_block!` / `parse_inline!` call, regardless
  of whether that line actually nests. Set `max_nest` to the smallest value
  your grammar needs — `meon-md` uses `4` deliberately; a larger cap costs
  throughput on every inline-bearing line whether or not anything nests.
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
