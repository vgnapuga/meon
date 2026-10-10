# meon-md — Cross-parser comparison

Throughput of [`meon-md`](https://github.com/vgnapuga/meon/blob/main/meon-md/README.md)
(built on the [`meon`](https://github.com/vgnapuga/meon/blob/main/meon/README.md)
engine) next to two CommonMark parsers, on the same corpora as the intra-engine
benches.

> **Three parsers, two different jobs.** `meon-md` is, by design, **not**
> CommonMark-compliant — it parses a Markdown subset into flat, type-indexed
> span vectors (O(1) access per element kind, single-type extraction via
> `find_*`, zero-copy spans). `pulldown-cmark` and `comrak` are full CommonMark
> and produce an event stream / an AST. A throughput gap is the difference
> between those jobs. `Throughput::Bytes` measures how fast the input is
> consumed, since the three produce different things.

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
* [***ARCHITECTURE.md***](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE.md)
* [***BENCHMARKS.md***](https://github.com/vgnapuga/meon/blob/main/benches/README.md)
  * ***MD_COMPARE.md***    <--
  * [***JSON_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/JSON_COMPARE.md)
* [***FUZZING.md***](https://github.com/vgnapuga/meon/blob/main/fuzz/README.md)

---

## What is measured

One binary, `meon-md_compare`. Per corpus (`plain` / `hot` / `heavy`), three
parsers over identical input, each `black_box`-ed:

| Line             | Call                                             | What it produces                                          |
|------------------|--------------------------------------------------|-----------------------------------------------------------|
| `meon-md`        | `MarkdownParser::parse`                          | Flat, type-indexed span table for a Markdown subset.      |
| `pulldown-cmark` | `Parser::new(s)`, iterator fully drained         | Full CommonMark event stream, parse-only, no rendering.   |
| `comrak`         | `parse_document(&arena, s, &Options::default())` | Full CommonMark AST, no rendering. The upper bound.       |

`pulldown-cmark` is the closest in shape to meon's single pass (a forward event
stream, no owned tree). `comrak` is the upper bound: it builds an owned AST.

The same per-corpus composition report as the intra-engine benches is printed
before timing.

---

## Two different jobs

- **CommonMark non-compliance is deliberate.** `meon-md` targets a Markdown
  subset on purpose; it is not, and does not aim to be, a CommonMark parser.
  Its output is a flat, type-indexed span table — O(1) access per element kind,
  one-type extraction via `find_*`, zero-copy spans. A tree can be built on top
  of those spans if a consumer needs one. The comparators do the full CommonMark
  job and hand back an event stream / AST. The figures compare those two
  designs.

- **Feature delta.** The comparators handle reference-style links, raw HTML,
  HTML entities, indented code blocks, setext headings, link/emphasis
  precedence, tight/loose lists and more — none of which `meon-md` does, by
  design. They pay for that surface on every parse; meon does not.

- **Corpus bias.** The `plain` / `hot` / `heavy` corpora are written for
  `meon-md`'s feature set, so they under-exercise the CommonMark features the
  comparators still handle. Real CommonMark documents shift the comparators'
  cost relative to what is shown here.

- **Synthetic-data upper bound.** The corpora are programmatic and uniform.
  Treat every figure as an upper-bound estimate, not expected production
  throughput.

- **Build-flag / SIMD parity.** meon uses AVX2 only under `--features avx2` +
  `RUSTFLAGS="-C target-cpu=native"`; on stable it runs the scalar SWAR path.
  `pulldown-cmark` has its own opt-in `simd` scanner (not enabled by default
  here, see [Running](#running)); `comrak` is scalar. Every results block below
  states the exact build it was taken under; only rows built with comparable
  flags belong side by side.

- **Output shapes differ.** SoA spans vs an event stream vs an AST.
  `Throughput::Bytes` normalises by input size — it answers "how fast is the
  input consumed", since the three produce different things.

- **End-to-end cost.** Timed regions include each parser's own allocations
  (meon's output `Vec`s, comrak's arena). comrak gets a fresh arena per
  iteration; pulldown's event iterator is fully drained so nothing is skipped
  lazily. Corpus generation and the `&str` view are outside the timed region.

---

## Running

Inside `nix develop`:

```sh
# Stable, scalar (meon SWAR path, pulldown scalar, comrak scalar):
cargo bench --bench meon-md_compare

# Nightly, meon AVX2 path tuned for the host CPU:
RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_compare --features avx2
```

Dependency build flags (in `benches/Cargo.toml`), chosen to keep the comparators
on their parse-only path:

- `pulldown-cmark` - `default-features = false` (drops `html` rendering). To
  give pulldown its SIMD scanner for a fairer AVX row, add `features = ["simd"]`
  and note it in the results block.
- `comrak` - `default-features = false` (drops `syntect` / rendering deps; keeps
  `parse_document`, `Arena`, `Options`).

Hardware and Criterion knobs are shared with the intra-engine benches — see
*Test hardware* in
[***BENCHMARKS.md***](https://github.com/vgnapuga/meon/blob/main/benches/README.md)
and the knobs in `benches/benches/docs_md.rs`.

---

## Corpora

Each base document is tiled `REPEAT_COUNT` times so the working set exceeds
cache. The `small` and `big` runs differ only in `REPEAT_COUNT`.

| Corpus  | Shape                                                                  | Stresses                                                          |
|---------|------------------------------------------------------------------------|-------------------------------------------------------------------|
| `plain` | Prose only, no markup.                                                 | Fallback/text path, line loop. Ceiling case (near-pure scanning). |
| `hot`   | Light, evenly spread markup (~one of each common inline per paragraph).| Typical real-world document.                                      |
| `heavy` | Dense: headings, rules, quotes, fences, lists, nested inline.          | Every rule family at once, including nesting. Stress case.        |

> **Synthetic data notice.** All three corpora are generated programmatically
> with uniform, predictable structure. Real-world documents typically have
> **lower element density** and less regular patterns than `hot` or `heavy`.
> Treat the numbers as upper-bound estimates for your specific workload, not
> as expected production throughput.

### Corpus composition

**small (REPEAT_COUNT = 10)**

```
┌─ corpus: plain
│  size:          2.80 MiB  (2937800 bytes)
│  elements:         2     (0.0 per KiB)
│  span mem:      0.00 MiB  (~0.0% of input, 8 B/span lower bound)
│
│          headings:         0    thematic_breaks:         0         paragraphs:         1
│       blockquotes:         0       fenced_codes:         0       bullet_items:         0
│     ordered_items:         0              bolds:         0            italics:         0
│      bold_italics:         0              codes:         0              links:         0
│         autolinks:         0        hard_breaks:         0              texts:         1
└─

┌─ corpus: hot
│  size:          0.75 MiB  (790600 bytes)
│  elements:     65000     (84.2 per KiB)
│  span mem:      0.50 MiB  (~65.8% of input, 8 B/span lower bound)
│
│          headings:      5000    thematic_breaks:         0         paragraphs:      5000
│       blockquotes:         0       fenced_codes:         0       bullet_items:         0
│     ordered_items:         0              bolds:      5000            italics:      5000
│      bold_italics:         0              codes:      5000              links:      5000
│         autolinks:      5000        hard_breaks:         0              texts:     30000
└─

┌─ corpus: heavy
│  size:          1.47 MiB  (1541020 bytes)
│  elements:    140000     (93.0 per KiB)
│  span mem:      1.07 MiB  (~72.7% of input, 8 B/span lower bound)
│
│          headings:      2000    thematic_breaks:      2000         paragraphs:      4000
│       blockquotes:      4000       fenced_codes:      2000       bullet_items:      6000
│     ordered_items:      4000              bolds:     12000            italics:     12000
│      bold_italics:      6000              codes:     10000              links:      6000
│         autolinks:      4000        hard_breaks:         0              texts:     66000
└─
```

**big (REPEAT_COUNT = 1000, exceeds L3 cache)**

```
┌─ corpus: plain
│  size:        280.17 MiB  (293780000 bytes)
│  elements:         2     (0.0 per KiB)
│  span mem:      0.00 MiB  (~0.0% of input, 8 B/span lower bound)
│
│          headings:         0    thematic_breaks:         0         paragraphs:         1
│       blockquotes:         0       fenced_codes:         0       bullet_items:         0
│     ordered_items:         0              bolds:         0            italics:         0
│      bold_italics:         0              codes:         0              links:         0
│         autolinks:         0        hard_breaks:         0              texts:         1
└─

┌─ corpus: hot
│  size:         75.40 MiB  (79060000 bytes)
│  elements:   6500000     (84.2 per KiB)
│  span mem:     49.59 MiB  (~65.8% of input, 8 B/span lower bound)
│
│          headings:    500000    thematic_breaks:         0         paragraphs:    500000
│       blockquotes:         0       fenced_codes:         0       bullet_items:         0
│     ordered_items:         0              bolds:    500000            italics:    500000
│      bold_italics:         0              codes:    500000              links:    500000
│         autolinks:    500000        hard_breaks:         0              texts:   3000000
└─

┌─ corpus: heavy
│  size:        146.96 MiB  (154102000 bytes)
│  elements:  14000000     (93.0 per KiB)
│  span mem:    106.81 MiB  (~72.7% of input, 8 B/span lower bound)
│
│          headings:    200000    thematic_breaks:    200000         paragraphs:    400000
│       blockquotes:    400000       fenced_codes:    200000       bullet_items:    600000
│     ordered_items:    400000              bolds:   1200000            italics:   1200000
│      bold_italics:    600000              codes:   1000000              links:    600000
│         autolinks:    400000        hard_breaks:         0              texts:   6600000
└─
```

---

## Results

> Throughput (`thrpt`) is the headline. Compare a cell only against the same
> corpus in the same build block. Each cell is the Criterion `time` / `thrpt`
> triple (low / median / high).

### stable - `cargo bench --bench meon-md_compare`

**small (fits in cache):**

| Corpus  | `meon-md`                  | `pulldown-cmark`           | `comrak`                   |
|---------|----------------------------|----------------------------|----------------------------|
| `plain` | [1.0006 ms]=[2.7345 GiB/s] | [3.7007 ms]=[757.07 MiB/s] | [9.9962 ms]=[280.28 MiB/s] |
| `hot`   | [658.15 µs]=[1.1187 GiB/s] | [4.8029 ms]=[156.98 MiB/s] | [16.587 ms]=[45.457 MiB/s] |
| `heavy` | [1.5525 ms]=[946.59 MiB/s] | [12.602 ms]=[116.62 MiB/s] | [43.289 ms]=[33.949 MiB/s] |

<details>
    <summary>full log</summary>

    ┌─ corpus: plain
    │  size:          2.80 MiB  (2937800 bytes)
    │  elements:         2     (0.0 per KiB)
    │  span mem:      0.00 MiB  (~0.0% of input, 8 B/span lower bound)
    │
    │          headings:         0    thematic_breaks:         0         paragraphs:         1
    │       blockquotes:         0       fenced_codes:         0       bullet_items:         0
    │     ordered_items:         0              bolds:         0            italics:         0
    │      bold_italics:         0              codes:         0              links:         0
    │         autolinks:         0        hard_breaks:         0              texts:         1
    └─
    compare/plain/meon-md   time:   [999.76 µs 1.0006 ms 1.0018 ms]
                            thrpt:  [2.7312 GiB/s 2.7345 GiB/s 2.7367 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) high mild
      1 (5.00%) high severe
    compare/plain/pulldown-cmark
                            time:   [3.6967 ms 3.7007 ms 3.7055 ms]
                            thrpt:  [756.09 MiB/s 757.07 MiB/s 757.89 MiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      1 (5.00%) low mild
      3 (15.00%) high severe
    compare/plain/comrak    time:   [9.9693 ms 9.9962 ms 10.017 ms]
                            thrpt:  [279.68 MiB/s 280.28 MiB/s 281.03 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    
    
    ┌─ corpus: hot
    │  size:          0.75 MiB  (790600 bytes)
    │  elements:     65000     (84.2 per KiB)
    │  span mem:      0.50 MiB  (~65.8% of input, 8 B/span lower bound)
    │
    │          headings:      5000    thematic_breaks:         0         paragraphs:      5000
    │       blockquotes:         0       fenced_codes:         0       bullet_items:         0
    │     ordered_items:         0              bolds:      5000            italics:      5000
    │      bold_italics:         0              codes:      5000              links:      5000
    │         autolinks:      5000        hard_breaks:         0              texts:     30000
    └─
    compare/hot/meon-md     time:   [657.53 µs 658.15 µs 658.75 µs]
                            thrpt:  [1.1177 GiB/s 1.1187 GiB/s 1.1198 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    compare/hot/pulldown-cmark
                            time:   [4.7964 ms 4.8029 ms 4.8112 ms]
                            thrpt:  [156.71 MiB/s 156.98 MiB/s 157.20 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    compare/hot/comrak      time:   [16.541 ms 16.587 ms 16.628 ms]
                            thrpt:  [45.343 MiB/s 45.457 MiB/s 45.583 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    
    
    ┌─ corpus: heavy
    │  size:          1.47 MiB  (1541020 bytes)
    │  elements:    140000     (93.0 per KiB)
    │  span mem:      1.07 MiB  (~72.7% of input, 8 B/span lower bound)
    │
    │          headings:      2000    thematic_breaks:      2000         paragraphs:      4000
    │       blockquotes:      4000       fenced_codes:      2000       bullet_items:      6000
    │     ordered_items:      4000              bolds:     12000            italics:     12000
    │      bold_italics:      6000              codes:     10000              links:      6000
    │         autolinks:      4000        hard_breaks:         0              texts:     66000
    └─
    compare/heavy/meon-md   time:   [1.5519 ms 1.5525 ms 1.5534 ms]
                            thrpt:  [946.05 MiB/s 946.59 MiB/s 947.01 MiB/s]
    compare/heavy/pulldown-cmark
                            time:   [12.578 ms 12.602 ms 12.622 ms]
                            thrpt:  [116.44 MiB/s 116.62 MiB/s 116.84 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    compare/heavy/comrak    time:   [43.241 ms 43.289 ms 43.352 ms]
                            thrpt:  [33.900 MiB/s 33.949 MiB/s 33.987 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
</details

**big (exceeds L3 cache):**

| Corpus  | `meon-md`                  | `pulldown-cmark`           | `comrak`                  |
|---------|----------------------------|----------------------------|---------------------------|
| `plain` | [103.27 ms]=[2.6494 GiB/s] | [579.15 ms]=[483.76 MiB/s] | [2.3552 s]=[118.96 MiB/s] |
| `hot`   | [66.557 ms]=[1.1063 GiB/s] | [853.62 ms]=[88.327 MiB/s] | [3.2433 s]=[23.247 MiB/s] |
| `heavy` | [154.99 ms]=[948.23 MiB/s] | [2.0413 s]=[71.996 MiB/s]  | [7.4457 s]=[19.738 MiB/s] |

<details>
    <summary>full log</summary>

    ┌─ corpus: plain
    │  size:        280.17 MiB  (293780000 bytes)
    │  elements:         2     (0.0 per KiB)
    │  span mem:      0.00 MiB  (~0.0% of input, 8 B/span lower bound)
    │
    │          headings:         0    thematic_breaks:         0         paragraphs:         1
    │       blockquotes:         0       fenced_codes:         0       bullet_items:         0
    │     ordered_items:         0              bolds:         0            italics:         0
    │      bold_italics:         0              codes:         0              links:         0
    │         autolinks:         0        hard_breaks:         0              texts:         1
    └─
    compare/plain/meon-md   time:   [102.68 ms 103.27 ms 103.89 ms]
                            thrpt:  [2.6336 GiB/s 2.6494 GiB/s 2.6646 GiB/s]
    Benchmarking compare/plain/pulldown-cmark: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 11.5s, or reduce sample count to 10.
    compare/plain/pulldown-cmark
                            time:   [576.69 ms 579.15 ms 581.54 ms]
                            thrpt:  [481.77 MiB/s 483.76 MiB/s 485.83 MiB/s]
    Benchmarking compare/plain/comrak: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 46.3s, or reduce sample count to 10.
    compare/plain/comrak    time:   [2.3454 s 2.3552 s 2.3645 s]
                            thrpt:  [118.49 MiB/s 118.96 MiB/s 119.46 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
    2 (10.00%) low mild
    
    
    ┌─ corpus: hot
    │  size:         75.40 MiB  (79060000 bytes)
    │  elements:   6500000     (84.2 per KiB)
    │  span mem:     49.59 MiB  (~65.8% of input, 8 B/span lower bound)
    │
    │          headings:    500000    thematic_breaks:         0         paragraphs:    500000
    │       blockquotes:         0       fenced_codes:         0       bullet_items:         0
    │     ordered_items:         0              bolds:    500000            italics:    500000
    │      bold_italics:         0              codes:    500000              links:    500000
    │         autolinks:    500000        hard_breaks:         0              texts:   3000000
    └─
    Benchmarking compare/hot/meon-md: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 14.0s, enable flat sampling, or reduce sample count to 10.
    compare/hot/meon-md     time:   [66.426 ms 66.557 ms 66.686 ms]
                            thrpt:  [1.1041 GiB/s 1.1063 GiB/s 1.1085 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
    1 (5.00%) high mild
    Benchmarking compare/hot/pulldown-cmark: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 17.1s, or reduce sample count to 10.
    compare/hot/pulldown-cmark
                            time:   [851.79 ms 853.62 ms 855.34 ms]
                            thrpt:  [88.149 MiB/s 88.327 MiB/s 88.516 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
    2 (10.00%) low mild
    Benchmarking compare/hot/comrak: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 64.3s, or reduce sample count to 10.
    compare/hot/comrak      time:   [3.2334 s 3.2433 s 3.2536 s]
                            thrpt:  [23.173 MiB/s 23.247 MiB/s 23.319 MiB/s]
    
    
    ┌─ corpus: heavy
    │  size:        146.96 MiB  (154102000 bytes)
    │  elements:  14000000     (93.0 per KiB)
    │  span mem:    106.81 MiB  (~72.7% of input, 8 B/span lower bound)
    │
    │          headings:    200000    thematic_breaks:    200000         paragraphs:    400000
    │       blockquotes:    400000       fenced_codes:    200000       bullet_items:    600000
    │     ordered_items:    400000              bolds:   1200000            italics:   1200000
    │      bold_italics:    600000              codes:   1000000              links:    600000
    │         autolinks:    400000        hard_breaks:         0              texts:   6600000
    └─
    compare/heavy/meon-md   time:   [154.45 ms 154.99 ms 155.47 ms]
                            thrpt:  [945.27 MiB/s 948.23 MiB/s 951.54 MiB/s]
    Found 3 outliers among 20 measurements (15.00%)
    2 (10.00%) low severe
    1 (5.00%) high severe
    Benchmarking compare/heavy/pulldown-cmark: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 41.7s, or reduce sample count to 10.
    compare/heavy/pulldown-cmark
                            time:   [2.0375 s 2.0413 s 2.0455 s]
                            thrpt:  [71.846 MiB/s 71.996 MiB/s 72.131 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
    2 (10.00%) high mild
    Benchmarking compare/heavy/comrak: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 148.5s, or reduce sample count to 10.
    compare/heavy/comrak    time:   [7.3905 s 7.4457 s 7.5023 s]
                            thrpt:  [19.589 MiB/s 19.738 MiB/s 19.885 MiB/s]
    Found 4 outliers among 20 measurements (20.00%)
    1 (5.00%) low severe
    1 (5.00%) high mild
    2 (10.00%) high severe
</details>

### nightly - `RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_compare --features avx2`

> meon on AVX2; `pulldown-cmark` and `comrak` scalar (no `simd` feature). The
> meon column is AVX2 against scalar comparators — not a like-for-like SIMD row.

**small (fits in cache):**

| Corpus  | `meon-md`                  | `pulldown-cmark`           | `comrak`                   |
|---------|----------------------------|----------------------------|----------------------------|
| `plain` | [596.41 µs]=[4.5875 GiB/s] | [3.1806 ms]=[880.88 MiB/s] | [9.6308 ms]=[290.91 MiB/s] |
| `hot`   | [562.51 µs]=[1.3090 GiB/s] | [5.3109 ms]=[141.97 MiB/s] | [15.623 ms]=[48.262 MiB/s] |
| `heavy` | [1.3374 ms]=[1.0731 GiB/s] | [13.439 ms]=[109.35 MiB/s] | [42.209 ms]=[34.818 MiB/s] |

<details>
    <summary>full log</summary>

    ┌─ corpus: plain
    │  size:          2.80 MiB  (2937800 bytes)
    │  elements:         2     (0.0 per KiB)
    │  span mem:      0.00 MiB  (~0.0% of input, 8 B/span lower bound)
    │
    │          headings:         0    thematic_breaks:         0         paragraphs:         1
    │       blockquotes:         0       fenced_codes:         0       bullet_items:         0
    │     ordered_items:         0              bolds:         0            italics:         0
    │      bold_italics:         0              codes:         0              links:         0
    │         autolinks:         0        hard_breaks:         0              texts:         1
    └─
    compare/plain/meon-md   time:   [596.31 µs 596.41 µs 596.49 µs]
                            thrpt:  [4.5869 GiB/s 4.5875 GiB/s 4.5883 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    compare/plain/pulldown-cmark
                            time:   [3.1770 ms 3.1806 ms 3.1850 ms]
                            thrpt:  [879.67 MiB/s 880.88 MiB/s 881.87 MiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      3 (15.00%) high mild
      1 (5.00%) high severe
    compare/plain/comrak    time:   [9.5825 ms 9.6308 ms 9.6986 ms]
                            thrpt:  [288.88 MiB/s 290.91 MiB/s 292.38 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    
    
    ┌─ corpus: hot
    │  size:          0.75 MiB  (790600 bytes)
    │  elements:     65000     (84.2 per KiB)
    │  span mem:      0.50 MiB  (~65.8% of input, 8 B/span lower bound)
    │
    │          headings:      5000    thematic_breaks:         0         paragraphs:      5000
    │       blockquotes:         0       fenced_codes:         0       bullet_items:         0
    │     ordered_items:         0              bolds:      5000            italics:      5000
    │      bold_italics:         0              codes:      5000              links:      5000
    │         autolinks:      5000        hard_breaks:         0              texts:     30000
    └─
    compare/hot/meon-md     time:   [562.23 µs 562.51 µs 562.82 µs]
                            thrpt:  [1.3082 GiB/s 1.3090 GiB/s 1.3096 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    compare/hot/pulldown-cmark
                            time:   [5.2962 ms 5.3109 ms 5.3276 ms]
                            thrpt:  [141.52 MiB/s 141.97 MiB/s 142.36 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    compare/hot/comrak      time:   [15.550 ms 15.623 ms 15.715 ms]
                            thrpt:  [47.978 MiB/s 48.262 MiB/s 48.487 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    
    
    ┌─ corpus: heavy
    │  size:          1.47 MiB  (1541020 bytes)
    │  elements:    140000     (93.0 per KiB)
    │  span mem:      1.07 MiB  (~72.7% of input, 8 B/span lower bound)
    │
    │          headings:      2000    thematic_breaks:      2000         paragraphs:      4000
    │       blockquotes:      4000       fenced_codes:      2000       bullet_items:      6000
    │     ordered_items:      4000              bolds:     12000            italics:     12000
    │      bold_italics:      6000              codes:     10000              links:      6000
    │         autolinks:      4000        hard_breaks:         0              texts:     66000
    └─
    compare/heavy/meon-md   time:   [1.3367 ms 1.3374 ms 1.3381 ms]
                            thrpt:  [1.0725 GiB/s 1.0731 GiB/s 1.0737 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    compare/heavy/pulldown-cmark
                            time:   [13.422 ms 13.439 ms 13.459 ms]
                            thrpt:  [109.19 MiB/s 109.35 MiB/s 109.50 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    compare/heavy/comrak    time:   [42.178 ms 42.209 ms 42.246 ms]
                            thrpt:  [34.787 MiB/s 34.818 MiB/s 34.844 MiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low mild
      2 (10.00%) high severe
</details>

**big (exceeds L3 cache):**

| Corpus  | `meon-md`                  | `pulldown-cmark`           | `comrak`                  |
|---------|----------------------------|----------------------------|---------------------------|
| `plain` | [63.279 ms]=[4.3238 GiB/s] | [531.95 ms]=[526.69 MiB/s] | [2.2354 s]=[125.33 MiB/s] |
| `hot`   | [56.797 ms]=[1.2964 GiB/s] | [886.41 ms]=[85.059 MiB/s] | [3.1578 s]=[23.877 MiB/s] |
| `heavy` | [134.38 ms]=[1.0680 GiB/s] | [2.0877 s]=[70.396 MiB/s]  | [7.3278 s]=[20.056 MiB/s] |

<details>
    <summary>full log</summary>

    ┌─ corpus: plain
    │  size:        280.17 MiB  (293780000 bytes)
    │  elements:         2     (0.0 per KiB)
    │  span mem:      0.00 MiB  (~0.0% of input, 8 B/span lower bound)
    │
    │          headings:         0    thematic_breaks:         0         paragraphs:         1
    │       blockquotes:         0       fenced_codes:         0       bullet_items:         0
    │     ordered_items:         0              bolds:         0            italics:         0
    │      bold_italics:         0              codes:         0              links:         0
    │         autolinks:         0        hard_breaks:         0              texts:         1
    └─
    Benchmarking compare/plain/meon-md: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 13.5s, enable flat sampling, or reduce sample count to 10.
    compare/plain/meon-md   time:   [63.188 ms 63.279 ms 63.439 ms]
                            thrpt:  [4.3129 GiB/s 4.3238 GiB/s 4.3300 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) high mild
      2 (10.00%) high severe
    Benchmarking compare/plain/pulldown-cmark: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 10.6s, or reduce sample count to 10.
    compare/plain/pulldown-cmark
                            time:   [530.98 ms 531.95 ms 533.25 ms]
                            thrpt:  [525.40 MiB/s 526.69 MiB/s 527.65 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    Benchmarking compare/plain/comrak: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 44.5s, or reduce sample count to 10.
    compare/plain/comrak    time:   [2.2191 s 2.2354 s 2.2553 s]
                            thrpt:  [124.23 MiB/s 125.33 MiB/s 126.25 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    
    
    ┌─ corpus: hot
    │  size:         75.40 MiB  (79060000 bytes)
    │  elements:   6500000     (84.2 per KiB)
    │  span mem:     49.59 MiB  (~65.8% of input, 8 B/span lower bound)
    │
    │          headings:    500000    thematic_breaks:         0         paragraphs:    500000
    │       blockquotes:         0       fenced_codes:         0       bullet_items:         0
    │     ordered_items:         0              bolds:    500000            italics:    500000
    │      bold_italics:         0              codes:    500000              links:    500000
    │         autolinks:    500000        hard_breaks:         0              texts:   3000000
    └─
    Benchmarking compare/hot/meon-md: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 11.9s, enable flat sampling, or reduce sample count to 10.
    compare/hot/meon-md     time:   [56.721 ms 56.797 ms 56.896 ms]
                            thrpt:  [1.2941 GiB/s 1.2964 GiB/s 1.2981 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) high mild
      1 (5.00%) high severe
    Benchmarking compare/hot/pulldown-cmark: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 17.5s, or reduce sample count to 10.
    compare/hot/pulldown-cmark
                            time:   [885.61 ms 886.41 ms 887.36 ms]
                            thrpt:  [84.968 MiB/s 85.059 MiB/s 85.136 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    Benchmarking compare/hot/comrak: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 63.1s, or reduce sample count to 10.
    compare/hot/comrak      time:   [3.1435 s 3.1578 s 3.1726 s]
                            thrpt:  [23.765 MiB/s 23.877 MiB/s 23.985 MiB/s]
    
    
    ┌─ corpus: heavy
    │  size:        146.96 MiB  (154102000 bytes)
    │  elements:  14000000     (93.0 per KiB)
    │  span mem:    106.81 MiB  (~72.7% of input, 8 B/span lower bound)
    │
    │          headings:    200000    thematic_breaks:    200000         paragraphs:    400000
    │       blockquotes:    400000       fenced_codes:    200000       bullet_items:    600000
    │     ordered_items:    400000              bolds:   1200000            italics:   1200000
    │      bold_italics:    600000              codes:   1000000              links:    600000
    │         autolinks:    400000        hard_breaks:         0              texts:   6600000
    └─
    compare/heavy/meon-md   time:   [134.25 ms 134.38 ms 134.50 ms]
                            thrpt:  [1.0671 GiB/s 1.0680 GiB/s 1.0690 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) low mild
    Benchmarking compare/heavy/pulldown-cmark: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 41.1s, or reduce sample count to 10.
    compare/heavy/pulldown-cmark
                            time:   [2.0810 s 2.0877 s 2.0950 s]
                            thrpt:  [70.148 MiB/s 70.396 MiB/s 70.622 MiB/s]
    Benchmarking compare/heavy/comrak: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 148.1s, or reduce sample count to 10.
    compare/heavy/comrak    time:   [7.3107 s 7.3278 s 7.3474 s]
                            thrpt:  [20.002 MiB/s 20.056 MiB/s 20.103 MiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      1 (5.00%) low mild
      1 (5.00%) high mild
      2 (10.00%) high severe
</details>

---

## Scaling from small to big

The clearest expression of the architecture difference is how each parser holds
up as the input grows past cache (stable build, median `thrpt`):

| Parser              | `plain`              | `hot`                | `heavy`            |
|---------------------|----------------------|----------------------|--------------------|
| `meon-md` - stable  | 2.73 -> 2.65 GiB/s   | 1.12 -> 1.11 GiB/s   | 947 -> 948 MiB/s   |
| `meon-md` - nightly | 4.59 -> 4.32 GiB/s   | 1.31 -> 1.3 GiB/s    | 1.07 -> 1.07 GiB/s |
| `pulldown-cmark`    | 757 -> 483 MiB/s     | 157 -> 88 MiB/s      | 117 -> 72 MiB/s    |
| `comrak`            | 280 -> 118 MiB/s     | 45 -> 23 MiB/s       | 34 -> 20 MiB/s     |

- **`meon-md` holds throughput essentially flat** from small to big (`plain` and
  `heavy` even tick up). The output is a compact, contiguous span table (`u32`
  pairs), so the working set stays cache-friendly as the document grows.
- **`pulldown-cmark` loses ~34–44%** at big — event-stream bookkeeping plus a
  growing working set push past cache.
- **`comrak` loses ~43–51%** and is slowest in absolute terms throughout — it
  materialises an owned AST, so allocation and pointer-chasing dominate as the
  document grows.

A flat span table degrades far less with scale than an event stream or an owned
tree. The AVX2 run shows the same pattern.

---

## meon-md standalone extraction (no comparator equivalent)

`find_*` scans the raw source for **one** element kind only — e.g. every bold
span — with no cross-element context. `pulldown-cmark` and `comrak` have no
equivalent: pulling just the bold spans from them means walking the full event
stream or AST. The numbers below are meon-only; they are here because per-type
extraction is part of the architecture difference this document is about.

Each line reports `full` vs `standalone` counts. By design they can differ: a
standalone scan has no fence/escape context (see
[`ARCHITECTURE.md §12`](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE.md#12-standalone-iterators)).
Shown for both `small` and `big`.

### stable - `cargo bench --bench meon-md_standalone`

**small (fits in cache):**

#### Standalone - `find_*`
| Function               | `plain`                    | `hot`                      | `heavy`                    |
|------------------------|----------------------------|----------------------------|----------------------------|
| `find_codes`           | [27.517 µs]=[99.432 GiB/s] | [75.258 µs]=[9.7838 GiB/s] | [184.85 µs]=[7.7640 GiB/s] |
| `find_italics`         | [27.680 µs]=[98.844 GiB/s] | [122.81 µs]=[5.9953 GiB/s] | [398.10 µs]=[3.6051 GiB/s] |
| `find_bolds`           | [29.375 µs]=[93.141 GiB/s] | [124.20 µs]=[5.9285 GiB/s] | [404.03 µs]=[3.5522 GiB/s] |
| `find_bold_italics`    | [27.272 µs]=[100.32 GiB/s] | [114.53 µs]=[6.4288 GiB/s] | [379.62 µs]=[3.7805 GiB/s] |
| `find_autolinks`       | [26.475 µs]=[103.34 GiB/s] | [73.876 µs]=[9.9667 GiB/s] | [65.575 µs]=[21.886 GiB/s] |
| `find_links`           | [27.391 µs]=[99.888 GiB/s] | [116.26 µs]=[6.3331 GiB/s] | [141.82 µs]=[10.120 GiB/s] |
| `find_headings`        | [28.898 µs]=[94.678 GiB/s] | [77.694 µs]=[9.4769 GiB/s] | [41.098 µs]=[34.921 GiB/s] |
| `find_thematic_breaks` | [39.893 µs]=[68.585 GiB/s] | [72.626 µs]=[10.138 GiB/s] | [281.87 µs]=[5.0917 GiB/s] |
| `find_fenced_codes`    | [27.780 µs]=[98.490 GiB/s] | [64.703 µs]=[11.380 GiB/s] | [190.47 µs]=[7.5349 GiB/s] |
| `find_blockquotes`     | [27.789 µs]=[98.458 GiB/s] | [38.040 µs]=[19.356 GiB/s] | [118.69 µs]=[12.092 GiB/s] |
| `find_bullet_items`    | [40.901 µs]=[66.894 GiB/s] | [71.744 µs]=[10.263 GiB/s] | [267.79 µs]=[5.3593 GiB/s] |
| `find_ordered_items`   | [271.01 µs]=[10.096 GiB/s] | [95.513 µs]=[7.7089 GiB/s] | [217.27 µs]=[6.6055 GiB/s] |
 
#### Context-aware - `context()` + `find_context_*`
| Function                       | `plain`                    | `hot`                      | `heavy`                    |
|--------------------------------|----------------------------|----------------------------|----------------------------|
| `context`                      | [34.453 µs]=[79.413 GiB/s] | [158.58 µs]=[4.6432 GiB/s] | [295.22 µs]=[4.8613 GiB/s] |
| `find_context_italics`         | [27.740 µs]=[98.632 GiB/s] | [138.05 µs]=[5.3336 GiB/s] | [460.41 µs]=[3.1172 GiB/s] |
| `find_context_bolds`           | [31.400 µs]=[87.136 GiB/s] | [141.69 µs]=[5.1967 GiB/s] | [455.90 µs]=[3.1480 GiB/s] |
| `find_context_bold_italics`    | [27.967 µs]=[97.833 GiB/s] | [121.77 µs]=[6.0467 GiB/s] | [438.58 µs]=[3.2724 GiB/s] |
| `find_context_headings`        | [29.135 µs]=[93.910 GiB/s] | [82.407 µs]=[8.9350 GiB/s] | [44.675 µs]=[32.125 GiB/s] |
| `find_context_thematic_breaks` | [46.137 µs]=[59.302 GiB/s] | [72.551 µs]=[10.149 GiB/s] | [285.01 µs]=[5.0356 GiB/s] |
| `find_context_blockquotes`     | [28.149 µs]=[97.198 GiB/s] | [37.992 µs]=[19.380 GiB/s] | [120.99 µs]=[11.862 GiB/s] |
| `find_context_bullet_items`    | [40.463 µs]=[67.618 GiB/s] | [71.828 µs]=[10.251 GiB/s] | [277.44 µs]=[5.1729 GiB/s] |
| `find_context_ordered_items`   | [274.52 µs]=[9.9666 GiB/s] | [96.308 µs]=[7.6453 GiB/s] | [231.58 µs]=[6.1974 GiB/s] |
 
#### Cold context - `find_context_*_cold`
| Function                            | `plain`                    | `hot`                      | `heavy`                    |
|-------------------------------------|----------------------------|----------------------------|----------------------------|
| `find_context_italics_cold`         | [63.397 µs]=[43.157 GiB/s] | [296.58 µs]=[2.4826 GiB/s] | [759.32 µs]=[1.8901 GiB/s] |
| `find_context_bolds_cold`           | [74.200 µs]=[36.874 GiB/s] | [305.41 µs]=[2.4108 GiB/s] | [761.17 µs]=[1.8855 GiB/s] |
| `find_context_bold_italics_cold`    | [63.170 µs]=[43.313 GiB/s] | [280.23 µs]=[2.6275 GiB/s] | [738.22 µs]=[1.9441 GiB/s] |
| `find_context_headings_cold`        | [63.641 µs]=[42.992 GiB/s] | [238.06 µs]=[3.0930 GiB/s] | [342.61 µs]=[4.1890 GiB/s] |
| `find_context_thematic_breaks_cold` | [77.057 µs]=[35.507 GiB/s] | [231.18 µs]=[3.1850 GiB/s] | [579.94 µs]=[2.4747 GiB/s] |
| `find_context_blockquotes_cold`     | [63.719 µs]=[42.939 GiB/s] | [196.37 µs]=[3.7496 GiB/s] | [418.55 µs]=[3.4290 GiB/s] |
| `find_context_bullet_items_cold`    | [77.417 µs]=[35.342 GiB/s] | [230.30 µs]=[3.1972 GiB/s] | [571.30 µs]=[2.5121 GiB/s] |
| `find_context_ordered_items_cold`   | [314.20 µs]=[8.7080 GiB/s] | [253.75 µs]=[2.9017 GiB/s] | [532.47 µs]=[2.6954 GiB/s] |
 


<details>
    <summary>plain full log</summary>

    ```
    ┌─ corpus: plain
    │  size:          2.80 MiB  (2937800 bytes)
    │  elements:         2     (0.0 per KiB)
    │  span mem:      0.00 MiB  (~0.0% of input, 8 B/span lower bound)
    │
    │          headings:         0    thematic_breaks:         0         paragraphs:         1
    │       blockquotes:         0       fenced_codes:         0       bullet_items:         0
    │     ordered_items:         0              bolds:         0            italics:         0
    │      bold_italics:         0              codes:         0              links:         0
    │         autolinks:         0        hard_breaks:         0              texts:         1
    └─
    │  full-vs-standalone counts:
        find_codes         full=       0  standalone=       0
    standalone/plain/find_codes
                            time:   [26.920 µs 27.517 µs 28.155 µs]
                            thrpt:  [97.178 GiB/s 99.432 GiB/s 101.64 GiB/s]
        find_italics       full=       0  standalone=       0
    standalone/plain/find_italics
                            time:   [26.894 µs 27.680 µs 28.565 µs]
                            thrpt:  [95.782 GiB/s 98.844 GiB/s 101.73 GiB/s]
        find_bolds         full=       0  standalone=       0
    standalone/plain/find_bolds
                            time:   [28.180 µs 29.375 µs 30.483 µs]
                            thrpt:  [89.755 GiB/s 93.141 GiB/s 97.091 GiB/s]
        find_bold_italics  full=       0  standalone=       0
    standalone/plain/find_bold_italics
                            time:   [26.729 µs 27.272 µs 28.015 µs]
                            thrpt:  [97.662 GiB/s 100.32 GiB/s 102.36 GiB/s]
        find_autolinks     full=       0  standalone=       0
    standalone/plain/find_autolinks
                            time:   [26.110 µs 26.475 µs 26.787 µs]
                            thrpt:  [102.14 GiB/s 103.34 GiB/s 104.79 GiB/s]
        find_links         full=       0  standalone=       0
    standalone/plain/find_links
                            time:   [27.127 µs 27.391 µs 27.725 µs]
                            thrpt:  [98.685 GiB/s 99.888 GiB/s 100.86 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_headings      full=       0  standalone=       0
    standalone/plain/find_headings
                            time:   [27.833 µs 28.898 µs 30.385 µs]
                            thrpt:  [90.047 GiB/s 94.678 GiB/s 98.304 GiB/s]
        find_thematic_breaks full=       0  standalone=       0
    standalone/plain/find_thematic_breaks
                            time:   [39.245 µs 39.893 µs 40.703 µs]
                            thrpt:  [67.220 GiB/s 68.585 GiB/s 69.717 GiB/s]
        find_fenced_codes  full=       0  standalone=       0
    standalone/plain/find_fenced_codes
                            time:   [27.136 µs 27.780 µs 28.315 µs]
                            thrpt:  [96.629 GiB/s 98.490 GiB/s 100.83 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_blockquotes   full=       0  standalone=       0
    standalone/plain/find_blockquotes
                            time:   [27.173 µs 27.789 µs 28.516 µs]
                            thrpt:  [95.947 GiB/s 98.458 GiB/s 100.69 GiB/s]
        find_bullet_items  full=       0  standalone=       0
    standalone/plain/find_bullet_items
                            time:   [40.182 µs 40.901 µs 41.400 µs]
                            thrpt:  [66.088 GiB/s 66.894 GiB/s 68.091 GiB/s]
        find_ordered_items full=       0  standalone=       0
    standalone/plain/find_ordered_items
                            time:   [270.76 µs 271.01 µs 271.31 µs]
                            thrpt:  [10.084 GiB/s 10.096 GiB/s 10.105 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    │  context regions: 0
    standalone/plain/context
                            time:   [34.049 µs 34.453 µs 34.872 µs]
                            thrpt:  [78.459 GiB/s 79.413 GiB/s 80.357 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    │  full-vs-context-aware counts:
        find_context_italics full=       0  context-aware=       0
    standalone/plain/find_context_italics
                            time:   [27.116 µs 27.740 µs 28.371 µs]
                            thrpt:  [96.439 GiB/s 98.632 GiB/s 100.90 GiB/s]
        find_context_bolds full=       0  context-aware=       0
    standalone/plain/find_context_bolds
                            time:   [29.153 µs 31.400 µs 33.575 µs]
                            thrpt:  [81.490 GiB/s 87.136 GiB/s 93.853 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) high mild
      1 (5.00%) high severe
        find_context_bold_italics full=       0  context-aware=       0
    standalone/plain/find_context_bold_italics
                            time:   [27.392 µs 27.967 µs 28.635 µs]
                            thrpt:  [95.548 GiB/s 97.833 GiB/s 99.885 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_context_headings full=       0  context-aware=       0
    standalone/plain/find_context_headings
                            time:   [28.504 µs 29.135 µs 29.794 µs]
                            thrpt:  [91.832 GiB/s 93.910 GiB/s 95.987 GiB/s]
        find_context_thematic_breaks full=       0  context-aware=       0
    standalone/plain/find_context_thematic_breaks
                            time:   [44.936 µs 46.137 µs 47.724 µs]
                            thrpt:  [57.331 GiB/s 59.302 GiB/s 60.888 GiB/s]
        find_context_blockquotes full=       0  context-aware=       0
    standalone/plain/find_context_blockquotes
                            time:   [27.378 µs 28.149 µs 28.981 µs]
                            thrpt:  [94.409 GiB/s 97.198 GiB/s 99.935 GiB/s]
        find_context_bullet_items full=       0  context-aware=       0
    standalone/plain/find_context_bullet_items
                            time:   [39.625 µs 40.463 µs 41.280 µs]
                            thrpt:  [66.280 GiB/s 67.618 GiB/s 69.048 GiB/s]
        find_context_ordered_items full=       0  context-aware=       0
    standalone/plain/find_context_ordered_items
                            time:   [273.78 µs 274.52 µs 275.52 µs]
                            thrpt:  [9.9303 GiB/s 9.9666 GiB/s 9.9935 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      3 (15.00%) high mild
    standalone/plain/find_context_italics_cold
                            time:   [62.134 µs 63.397 µs 64.512 µs]
                            thrpt:  [42.411 GiB/s 43.157 GiB/s 44.034 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    standalone/plain/find_context_bolds_cold
                            time:   [71.036 µs 74.200 µs 76.956 µs]
                            thrpt:  [35.553 GiB/s 36.874 GiB/s 38.516 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low mild
      2 (10.00%) high mild
    standalone/plain/find_context_bold_italics_cold
                            time:   [61.894 µs 63.170 µs 64.578 µs]
                            thrpt:  [42.368 GiB/s 43.313 GiB/s 44.205 GiB/s]
    standalone/plain/find_context_headings_cold
                            time:   [62.142 µs 63.641 µs 64.927 µs]
                            thrpt:  [42.140 GiB/s 42.992 GiB/s 44.029 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low mild
      2 (10.00%) high mild
    standalone/plain/find_context_thematic_breaks_cold
                            time:   [75.243 µs 77.057 µs 79.017 µs]
                            thrpt:  [34.626 GiB/s 35.507 GiB/s 36.363 GiB/s]
    standalone/plain/find_context_blockquotes_cold
                            time:   [62.755 µs 63.719 µs 64.895 µs]
                            thrpt:  [42.161 GiB/s 42.939 GiB/s 43.599 GiB/s]
    standalone/plain/find_context_bullet_items_cold
                            time:   [76.038 µs 77.417 µs 78.973 µs]
                            thrpt:  [34.645 GiB/s 35.342 GiB/s 35.982 GiB/s]
    standalone/plain/find_context_ordered_items_cold
                            time:   [310.47 µs 314.20 µs 317.79 µs]
                            thrpt:  [8.6095 GiB/s 8.7080 GiB/s 8.8127 GiB/s]
    ```
</details>

<details>
    <summary>hot full log</summary>

    ```
    ┌─ corpus: hot
    │  size:          0.75 MiB  (790600 bytes)
    │  elements:     65000     (84.2 per KiB)
    │  span mem:      0.50 MiB  (~65.8% of input, 8 B/span lower bound)
    │
    │          headings:      5000    thematic_breaks:         0         paragraphs:      5000
    │       blockquotes:         0       fenced_codes:         0       bullet_items:         0
    │     ordered_items:         0              bolds:      5000            italics:      5000
    │      bold_italics:         0              codes:      5000              links:      5000
    │         autolinks:      5000        hard_breaks:         0              texts:     30000
    └─
    │  full-vs-standalone counts:
        find_codes         full=    5000  standalone=    5000
    standalone/hot/find_codes
                            time:   [75.218 µs 75.258 µs 75.295 µs]
                            thrpt:  [9.7789 GiB/s 9.7838 GiB/s 9.7889 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low mild
      2 (10.00%) high severe
        find_italics       full=    5000  standalone=    5000
    standalone/hot/find_italics
                            time:   [122.71 µs 122.81 µs 122.96 µs]
                            thrpt:  [5.9881 GiB/s 5.9953 GiB/s 6.0002 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      3 (15.00%) high severe
        find_bolds         full=    5000  standalone=    5000
    standalone/hot/find_bolds
                            time:   [124.14 µs 124.20 µs 124.28 µs]
                            thrpt:  [5.9247 GiB/s 5.9285 GiB/s 5.9314 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_bold_italics  full=       0  standalone=       0
    standalone/hot/find_bold_italics
                            time:   [114.46 µs 114.53 µs 114.61 µs]
                            thrpt:  [6.4246 GiB/s 6.4288 GiB/s 6.4330 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_autolinks     full=    5000  standalone=    5000
    standalone/hot/find_autolinks
                            time:   [73.811 µs 73.876 µs 73.946 µs]
                            thrpt:  [9.9574 GiB/s 9.9667 GiB/s 9.9755 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_links         full=    5000  standalone=    5000
    standalone/hot/find_links
                            time:   [116.15 µs 116.26 µs 116.43 µs]
                            thrpt:  [6.3241 GiB/s 6.3331 GiB/s 6.3392 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_headings      full=    5000  standalone=    5000
    standalone/hot/find_headings
                            time:   [77.649 µs 77.694 µs 77.746 µs]
                            thrpt:  [9.4706 GiB/s 9.4769 GiB/s 9.4824 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) high mild
      2 (10.00%) high severe
        find_thematic_breaks full=       0  standalone=       0
    standalone/hot/find_thematic_breaks
                            time:   [72.578 µs 72.626 µs 72.665 µs]
                            thrpt:  [10.133 GiB/s 10.138 GiB/s 10.145 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_fenced_codes  full=       0  standalone=       0
    standalone/hot/find_fenced_codes
                            time:   [64.665 µs 64.703 µs 64.743 µs]
                            thrpt:  [11.373 GiB/s 11.380 GiB/s 11.386 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_blockquotes   full=       0  standalone=       0
    standalone/hot/find_blockquotes
                            time:   [37.967 µs 38.040 µs 38.130 µs]
                            thrpt:  [19.310 GiB/s 19.356 GiB/s 19.393 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_bullet_items  full=       0  standalone=       0
    standalone/hot/find_bullet_items
                            time:   [71.702 µs 71.744 µs 71.791 µs]
                            thrpt:  [10.256 GiB/s 10.263 GiB/s 10.269 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_ordered_items full=       0  standalone=       0
    standalone/hot/find_ordered_items
                            time:   [94.974 µs 95.513 µs 95.927 µs]
                            thrpt:  [7.6757 GiB/s 7.7089 GiB/s 7.7527 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      3 (15.00%) high mild
    │  context regions: 10000
    standalone/hot/context  time:   [158.46 µs 158.58 µs 158.71 µs]
                            thrpt:  [4.6394 GiB/s 4.6432 GiB/s 4.6468 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    │  full-vs-context-aware counts:
        find_context_italics full=    5000  context-aware=    5000
    standalone/hot/find_context_italics
                            time:   [137.87 µs 138.05 µs 138.25 µs]
                            thrpt:  [5.3258 GiB/s 5.3336 GiB/s 5.3406 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_context_bolds full=    5000  context-aware=    5000
    standalone/hot/find_context_bolds
                            time:   [141.52 µs 141.69 µs 141.89 µs]
                            thrpt:  [5.1892 GiB/s 5.1967 GiB/s 5.2027 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_context_bold_italics full=       0  context-aware=       0
    standalone/hot/find_context_bold_italics
                            time:   [121.70 µs 121.77 µs 121.85 µs]
                            thrpt:  [6.0426 GiB/s 6.0467 GiB/s 6.0502 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_context_headings full=    5000  context-aware=    5000
    standalone/hot/find_context_headings
                            time:   [82.229 µs 82.407 µs 82.585 µs]
                            thrpt:  [8.9157 GiB/s 8.9350 GiB/s 8.9543 GiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      2 (10.00%) low mild
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_context_thematic_breaks full=       0  context-aware=       0
    standalone/hot/find_context_thematic_breaks
                            time:   [72.515 µs 72.551 µs 72.605 µs]
                            thrpt:  [10.141 GiB/s 10.149 GiB/s 10.154 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      3 (15.00%) high severe
        find_context_blockquotes full=       0  context-aware=       0
    standalone/hot/find_context_blockquotes
                            time:   [37.960 µs 37.992 µs 38.031 µs]
                            thrpt:  [19.360 GiB/s 19.380 GiB/s 19.397 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_context_bullet_items full=       0  context-aware=       0
    standalone/hot/find_context_bullet_items
                            time:   [71.760 µs 71.828 µs 71.896 µs]
                            thrpt:  [10.241 GiB/s 10.251 GiB/s 10.261 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      3 (15.00%) high severe
        find_context_ordered_items full=       0  context-aware=       0
    standalone/hot/find_context_ordered_items
                            time:   [96.234 µs 96.308 µs 96.427 µs]
                            thrpt:  [7.6359 GiB/s 7.6453 GiB/s 7.6512 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    standalone/hot/find_context_italics_cold
                            time:   [296.31 µs 296.58 µs 297.06 µs]
                            thrpt:  [2.4786 GiB/s 2.4826 GiB/s 2.4849 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
    standalone/hot/find_context_bolds_cold
                            time:   [304.76 µs 305.41 µs 305.89 µs]
                            thrpt:  [2.4071 GiB/s 2.4108 GiB/s 2.4160 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
    standalone/hot/find_context_bold_italics_cold
                            time:   [279.92 µs 280.23 µs 280.54 µs]
                            thrpt:  [2.6246 GiB/s 2.6275 GiB/s 2.6304 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    standalone/hot/find_context_headings_cold
                            time:   [237.91 µs 238.06 µs 238.19 µs]
                            thrpt:  [3.0912 GiB/s 3.0930 GiB/s 3.0948 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    standalone/hot/find_context_thematic_breaks_cold
                            time:   [230.94 µs 231.18 µs 231.49 µs]
                            thrpt:  [3.1808 GiB/s 3.1850 GiB/s 3.1883 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    standalone/hot/find_context_blockquotes_cold
                            time:   [196.19 µs 196.37 µs 196.54 µs]
                            thrpt:  [3.7463 GiB/s 3.7496 GiB/s 3.7531 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
    standalone/hot/find_context_bullet_items_cold
                            time:   [230.11 µs 230.30 µs 230.50 µs]
                            thrpt:  [3.1944 GiB/s 3.1972 GiB/s 3.1998 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    standalone/hot/find_context_ordered_items_cold
                            time:   [253.51 µs 253.75 µs 254.03 µs]
                            thrpt:  [2.8985 GiB/s 2.9017 GiB/s 2.9045 GiB/s]
    ```
</details>

<details>
    <summary>heavy full log</summary>

    ```
    ┌─ corpus: heavy
    │  size:          1.47 MiB  (1541020 bytes)
    │  elements:    140000     (93.0 per KiB)
    │  span mem:      1.07 MiB  (~72.7% of input, 8 B/span lower bound)
    │
    │          headings:      2000    thematic_breaks:      2000         paragraphs:      4000
    │       blockquotes:      4000       fenced_codes:      2000       bullet_items:      6000
    │     ordered_items:      4000              bolds:     12000            italics:     12000
    │      bold_italics:      6000              codes:     10000              links:      6000
    │         autolinks:      4000        hard_breaks:         0              texts:     66000
    └─
    │  full-vs-standalone counts:
        find_codes         full=   10000  standalone=   10000
    standalone/heavy/find_codes
                            time:   [184.07 µs 184.85 µs 185.76 µs]
                            thrpt:  [7.7260 GiB/s 7.7640 GiB/s 7.7971 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
        find_italics       full=   12000  standalone=   12000
    standalone/heavy/find_italics
                            time:   [396.05 µs 398.10 µs 399.26 µs]
                            thrpt:  [3.5946 GiB/s 3.6051 GiB/s 3.6237 GiB/s]
        find_bolds         full=   12000  standalone=   12000
    standalone/heavy/find_bolds
                            time:   [402.67 µs 404.03 µs 404.76 µs]
                            thrpt:  [3.5458 GiB/s 3.5522 GiB/s 3.5642 GiB/s]
        find_bold_italics  full=    6000  standalone=    6000
    standalone/heavy/find_bold_italics
                            time:   [379.45 µs 379.62 µs 379.78 µs]
                            thrpt:  [3.7789 GiB/s 3.7805 GiB/s 3.7823 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) high mild
      1 (5.00%) high severe
        find_autolinks     full=    4000  standalone=    4000
    standalone/heavy/find_autolinks
                            time:   [65.437 µs 65.575 µs 65.744 µs]
                            thrpt:  [21.830 GiB/s 21.886 GiB/s 21.932 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_links         full=    6000  standalone=    6000
    standalone/heavy/find_links
                            time:   [141.70 µs 141.82 µs 141.94 µs]
                            thrpt:  [10.111 GiB/s 10.120 GiB/s 10.128 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_headings      full=    2000  standalone=    2000
    standalone/heavy/find_headings
                            time:   [41.067 µs 41.098 µs 41.135 µs]
                            thrpt:  [34.890 GiB/s 34.921 GiB/s 34.947 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_thematic_breaks full=    2000  standalone=    2000
    standalone/heavy/find_thematic_breaks
                            time:   [281.68 µs 281.87 µs 282.04 µs]
                            thrpt:  [5.0886 GiB/s 5.0917 GiB/s 5.0950 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_fenced_codes  full=    2000  standalone=    2000
    standalone/heavy/find_fenced_codes
                            time:   [190.22 µs 190.47 µs 190.90 µs]
                            thrpt:  [7.5182 GiB/s 7.5349 GiB/s 7.5450 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) high mild
      2 (10.00%) high severe
        find_blockquotes   full=    4000  standalone=    4000
    standalone/heavy/find_blockquotes
                            time:   [118.55 µs 118.69 µs 118.86 µs]
                            thrpt:  [12.075 GiB/s 12.092 GiB/s 12.106 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_bullet_items  full=    6000  standalone=    6000
    standalone/heavy/find_bullet_items
                            time:   [267.71 µs 267.79 µs 267.86 µs]
                            thrpt:  [5.3579 GiB/s 5.3593 GiB/s 5.3609 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_ordered_items full=    4000  standalone=    4000
    standalone/heavy/find_ordered_items
                            time:   [217.20 µs 217.27 µs 217.34 µs]
                            thrpt:  [6.6035 GiB/s 6.6055 GiB/s 6.6076 GiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      1 (5.00%) low mild
      2 (10.00%) high mild
      1 (5.00%) high severe
    │  context regions: 16000
    standalone/heavy/context
                            time:   [293.79 µs 295.22 µs 296.63 µs]
                            thrpt:  [4.8384 GiB/s 4.8613 GiB/s 4.8851 GiB/s]
    │  full-vs-context-aware counts:
        find_context_italics full=   12000  context-aware=   12000
    standalone/heavy/find_context_italics
                            time:   [460.08 µs 460.41 µs 460.67 µs]
                            thrpt:  [3.1155 GiB/s 3.1172 GiB/s 3.1194 GiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      3 (15.00%) low severe
      1 (5.00%) low mild
        find_context_bolds full=   12000  context-aware=   12000
    standalone/heavy/find_context_bolds
                            time:   [454.66 µs 455.90 µs 457.66 µs]
                            thrpt:  [3.1359 GiB/s 3.1480 GiB/s 3.1566 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_context_bold_italics full=    6000  context-aware=    6000
    standalone/heavy/find_context_bold_italics
                            time:   [437.81 µs 438.58 µs 439.27 µs]
                            thrpt:  [3.2672 GiB/s 3.2724 GiB/s 3.2781 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
        find_context_headings full=    2000  context-aware=    2000
    standalone/heavy/find_context_headings
                            time:   [44.078 µs 44.675 µs 45.065 µs]
                            thrpt:  [31.847 GiB/s 32.125 GiB/s 32.560 GiB/s]
        find_context_thematic_breaks full=    2000  context-aware=    2000
    standalone/heavy/find_context_thematic_breaks
                            time:   [284.93 µs 285.01 µs 285.09 µs]
                            thrpt:  [5.0341 GiB/s 5.0356 GiB/s 5.0370 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_context_blockquotes full=    4000  context-aware=    4000
    standalone/heavy/find_context_blockquotes
                            time:   [120.83 µs 120.99 µs 121.16 µs]
                            thrpt:  [11.845 GiB/s 11.862 GiB/s 11.878 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_context_bullet_items full=    6000  context-aware=    6000
    standalone/heavy/find_context_bullet_items
                            time:   [277.32 µs 277.44 µs 277.58 µs]
                            thrpt:  [5.1704 GiB/s 5.1729 GiB/s 5.1753 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_context_ordered_items full=    4000  context-aware=    4000
    standalone/heavy/find_context_ordered_items
                            time:   [230.53 µs 231.58 µs 232.70 µs]
                            thrpt:  [6.1674 GiB/s 6.1974 GiB/s 6.2255 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      3 (15.00%) high severe
    standalone/heavy/find_context_italics_cold
                            time:   [758.57 µs 759.32 µs 760.27 µs]
                            thrpt:  [1.8877 GiB/s 1.8901 GiB/s 1.8920 GiB/s]
    standalone/heavy/find_context_bolds_cold
                            time:   [756.10 µs 761.17 µs 765.41 µs]
                            thrpt:  [1.8751 GiB/s 1.8855 GiB/s 1.8981 GiB/s]
    standalone/heavy/find_context_bold_italics_cold
                            time:   [736.68 µs 738.22 µs 739.44 µs]
                            thrpt:  [1.9409 GiB/s 1.9441 GiB/s 1.9482 GiB/s]
    standalone/heavy/find_context_headings_cold
                            time:   [342.32 µs 342.61 µs 342.88 µs]
                            thrpt:  [4.1857 GiB/s 4.1890 GiB/s 4.1925 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    standalone/heavy/find_context_thematic_breaks_cold
                            time:   [578.99 µs 579.94 µs 580.50 µs]
                            thrpt:  [2.4723 GiB/s 2.4747 GiB/s 2.4788 GiB/s]
    standalone/heavy/find_context_blockquotes_cold
                            time:   [417.86 µs 418.55 µs 419.18 µs]
                            thrpt:  [3.4238 GiB/s 3.4290 GiB/s 3.4346 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    standalone/heavy/find_context_bullet_items_cold
                            time:   [570.12 µs 571.30 µs 572.44 µs]
                            thrpt:  [2.5071 GiB/s 2.5121 GiB/s 2.5174 GiB/s]
    standalone/heavy/find_context_ordered_items_cold
                            time:   [531.23 µs 532.47 µs 533.37 µs]
                            thrpt:  [2.6908 GiB/s 2.6954 GiB/s 2.7016 GiB/s]
    ```
</details>

**big (exceeds L3 cache):**

#### Standalone - `find_*`
| Function               | `plain`                    | `hot`                      | `heavy`                    |
|------------------------|----------------------------|----------------------------|----------------------------|
| `find_codes`           | [11.132 ms]=[24.578 GiB/s] | [7.9964 ms]=[9.2079 GiB/s] | [19.855 ms]=[7.2284 GiB/s] |
| `find_italics`         | [11.090 ms]=[24.671 GiB/s] | [12.517 ms]=[5.8823 GiB/s] | [41.090 ms]=[3.4928 GiB/s] |
| `find_bolds`           | [11.177 ms]=[24.479 GiB/s] | [12.733 ms]=[5.7824 GiB/s] | [40.476 ms]=[3.5458 GiB/s] |
| `find_bold_italics`    | [11.128 ms]=[24.586 GiB/s] | [11.804 ms]=[6.2376 GiB/s] | [38.294 ms]=[3.7478 GiB/s] |
| `find_autolinks`       | [11.125 ms]=[24.594 GiB/s] | [7.8717 ms]=[9.3538 GiB/s] | [9.3776 ms]=[15.304 GiB/s] |
| `find_links`           | [11.002 ms]=[24.868 GiB/s] | [11.908 ms]=[6.1832 GiB/s] | [15.699 ms]=[9.1418 GiB/s] |
| `find_headings`        | [11.123 ms]=[24.598 GiB/s] | [8.1425 ms]=[9.0428 GiB/s] | [8.2278 ms]=[17.443 GiB/s] |
| `find_thematic_breaks` | [12.860 ms]=[21.276 GiB/s] | [7.7317 ms]=[9.5232 GiB/s] | [29.326 ms]=[4.8939 GiB/s] |
| `find_fenced_codes`    | [11.128 ms]=[24.586 GiB/s] | [7.1928 ms]=[10.237 GiB/s] | [20.358 ms]=[7.0497 GiB/s] |
| `find_blockquotes`     | [11.193 ms]=[24.445 GiB/s] | [4.7678 ms]=[15.443 GiB/s] | [14.086 ms]=[10.188 GiB/s] |
| `find_bullet_items`    | [12.857 ms]=[21.280 GiB/s] | [7.6009 ms]=[9.6871 GiB/s] | [27.914 ms]=[5.1414 GiB/s] |
| `find_ordered_items`   | [28.866 ms]=[9.4785 GiB/s] | [10.048 ms]=[7.3279 GiB/s] | [23.064 ms]=[6.2227 GiB/s] |

#### Context-aware - `context()` + `find_context_*`
| Function                       | `plain`                    | `hot`                      | `heavy`                    |
|--------------------------------|----------------------------|----------------------------|----------------------------|
| `context`                      | [12.175 ms]=[22.473 GiB/s] | [16.162 ms]=[4.5558 GiB/s] | [31.023 ms]=[4.6262 GiB/s] |
| `find_context_italics`         | [11.068 ms]=[24.721 GiB/s] | [14.281 ms]=[5.1556 GiB/s] | [46.510 ms]=[3.0858 GiB/s] |
| `find_context_bolds`           | [11.016 ms]=[24.836 GiB/s] | [14.523 ms]=[5.0699 GiB/s] | [46.518 ms]=[3.0852 GiB/s] |
| `find_context_bold_italics`    | [11.123 ms]=[24.598 GiB/s] | [12.597 ms]=[5.8449 GiB/s] | [44.393 ms]=[3.2329 GiB/s] |
| `find_context_headings`        | [11.036 ms]=[24.791 GiB/s] | [8.5795 ms]=[8.5821 GiB/s] | [8.7261 ms]=[16.447 GiB/s] |
| `find_context_thematic_breaks` | [12.917 ms]=[21.182 GiB/s] | [7.7220 ms]=[9.5351 GiB/s] | [29.643 ms]=[4.8416 GiB/s] |
| `find_context_blockquotes`     | [11.003 ms]=[24.867 GiB/s] | [4.7600 ms]=[15.469 GiB/s] | [14.165 ms]=[10.132 GiB/s] |
| `find_context_bullet_items`    | [12.859 ms]=[21.277 GiB/s] | [7.6796 ms]=[9.5878 GiB/s] | [28.960 ms]=[4.9558 GiB/s] |
| `find_context_ordered_items`   | [28.890 ms]=[9.4706 GiB/s] | [10.406 ms]=[7.0756 GiB/s] | [24.417 ms]=[5.8778 GiB/s] |

#### Cold context - `find_context_*_cold`
| Function                            | `plain`                    | `hot`                      | `heavy`                    |
|-------------------------------------|----------------------------|----------------------------|----------------------------|
| `find_context_italics_cold`         | [23.286 ms]=[11.750 GiB/s] | [30.341 ms]=[2.4268 GiB/s] | [76.602 ms]=[1.8736 GiB/s] |
| `find_context_bolds_cold`           | [23.442 ms]=[11.672 GiB/s] | [30.660 ms]=[2.4015 GiB/s] | [77.130 ms]=[1.8607 GiB/s] |
| `find_context_bold_italics_cold`    | [23.283 ms]=[11.751 GiB/s] | [28.778 ms]=[2.5586 GiB/s] | [74.743 ms]=[1.9202 GiB/s] |
| `find_context_headings_cold`        | [23.296 ms]=[11.745 GiB/s] | [24.677 ms]=[2.9838 GiB/s] | [39.594 ms]=[3.6247 GiB/s] |
| `find_context_thematic_breaks_cold` | [25.329 ms]=[10.802 GiB/s] | [23.898 ms]=[3.0810 GiB/s] | [60.460 ms]=[2.3738 GiB/s] |
| `find_context_blockquotes_cold`     | [23.437 ms]=[11.674 GiB/s] | [20.975 ms]=[3.5103 GiB/s] | [45.132 ms]=[3.1799 GiB/s] |
| `find_context_bullet_items_cold`    | [25.060 ms]=[10.918 GiB/s] | [23.769 ms]=[3.0977 GiB/s] | [59.341 ms]=[2.4185 GiB/s] |
| `find_context_ordered_items_cold`   | [41.106 ms]=[6.6561 GiB/s] | [26.269 ms]=[2.8029 GiB/s] | [55.434 ms]=[2.5890 GiB/s] |

<details>
    <summary>plain full log</summary>

    ```
    ┌─ corpus: plain
    │  size:        280.17 MiB  (293780000 bytes)
    │  elements:         2     (0.0 per KiB)
    │  span mem:      0.00 MiB  (~0.0% of input, 8 B/span lower bound)
    │
    │          headings:         0    thematic_breaks:         0         paragraphs:         1
    │       blockquotes:         0       fenced_codes:         0       bullet_items:         0
    │     ordered_items:         0              bolds:         0            italics:         0
    │      bold_italics:         0              codes:         0              links:         0
    │         autolinks:         0        hard_breaks:         0              texts:         1
    └─
    │  full-vs-standalone counts:
        find_codes         full=       0  standalone=       0
    standalone/plain/find_codes
                            time:   [11.122 ms 11.132 ms 11.146 ms]
                            thrpt:  [24.548 GiB/s 24.578 GiB/s 24.600 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_italics       full=       0  standalone=       0
    standalone/plain/find_italics
                            time:   [11.026 ms 11.090 ms 11.154 ms]
                            thrpt:  [24.530 GiB/s 24.671 GiB/s 24.814 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      3 (15.00%) high severe
        find_bolds         full=       0  standalone=       0
    standalone/plain/find_bolds
                            time:   [11.124 ms 11.177 ms 11.231 ms]
                            thrpt:  [24.361 GiB/s 24.479 GiB/s 24.595 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_bold_italics  full=       0  standalone=       0
    standalone/plain/find_bold_italics
                            time:   [11.119 ms 11.128 ms 11.137 ms]
                            thrpt:  [24.568 GiB/s 24.586 GiB/s 24.607 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_autolinks     full=       0  standalone=       0
    standalone/plain/find_autolinks
                            time:   [11.117 ms 11.125 ms 11.132 ms]
                            thrpt:  [24.577 GiB/s 24.594 GiB/s 24.610 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low mild
      2 (10.00%) high severe
        find_links         full=       0  standalone=       0
    standalone/plain/find_links
                            time:   [10.990 ms 11.002 ms 11.017 ms]
                            thrpt:  [24.834 GiB/s 24.868 GiB/s 24.896 GiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      2 (10.00%) low mild
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_headings      full=       0  standalone=       0
    standalone/plain/find_headings
                            time:   [11.113 ms 11.123 ms 11.135 ms]
                            thrpt:  [24.572 GiB/s 24.598 GiB/s 24.619 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) high mild
      1 (5.00%) high severe
        find_thematic_breaks full=       0  standalone=       0
    standalone/plain/find_thematic_breaks
                            time:   [12.842 ms 12.860 ms 12.880 ms]
                            thrpt:  [21.243 GiB/s 21.276 GiB/s 21.305 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_fenced_codes  full=       0  standalone=       0
    standalone/plain/find_fenced_codes
                            time:   [11.119 ms 11.128 ms 11.137 ms]
                            thrpt:  [24.567 GiB/s 24.586 GiB/s 24.608 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) high mild
      1 (5.00%) high severe
        find_blockquotes   full=       0  standalone=       0
    standalone/plain/find_blockquotes
                            time:   [11.137 ms 11.193 ms 11.275 ms]
                            thrpt:  [24.267 GiB/s 24.445 GiB/s 24.567 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
        find_bullet_items  full=       0  standalone=       0
    standalone/plain/find_bullet_items
                            time:   [12.844 ms 12.857 ms 12.869 ms]
                            thrpt:  [21.261 GiB/s 21.280 GiB/s 21.302 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
        find_ordered_items full=       0  standalone=       0
    standalone/plain/find_ordered_items
                            time:   [28.841 ms 28.866 ms 28.895 ms]
                            thrpt:  [9.4688 GiB/s 9.4785 GiB/s 9.4865 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    │  context regions: 0
    standalone/plain/context
                            time:   [12.166 ms 12.175 ms 12.183 ms]
                            thrpt:  [22.457 GiB/s 22.473 GiB/s 22.489 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    │  full-vs-context-aware counts:
        find_context_italics full=       0  context-aware=       0
    standalone/plain/find_context_italics
                            time:   [11.010 ms 11.068 ms 11.138 ms]
                            thrpt:  [24.565 GiB/s 24.721 GiB/s 24.850 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_context_bolds full=       0  context-aware=       0
    standalone/plain/find_context_bolds
                            time:   [11.000 ms 11.016 ms 11.037 ms]
                            thrpt:  [24.789 GiB/s 24.836 GiB/s 24.874 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_context_bold_italics full=       0  context-aware=       0
    standalone/plain/find_context_bold_italics
                            time:   [11.112 ms 11.123 ms 11.138 ms]
                            thrpt:  [24.566 GiB/s 24.598 GiB/s 24.623 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) high mild
      1 (5.00%) high severe
        find_context_headings full=       0  context-aware=       0
    standalone/plain/find_context_headings
                            time:   [10.996 ms 11.036 ms 11.095 ms]
                            thrpt:  [24.661 GiB/s 24.791 GiB/s 24.882 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
        find_context_thematic_breaks full=       0  context-aware=       0
    standalone/plain/find_context_thematic_breaks
                            time:   [12.887 ms 12.917 ms 12.948 ms]
                            thrpt:  [21.131 GiB/s 21.182 GiB/s 21.232 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) high mild
      1 (5.00%) high severe
        find_context_blockquotes full=       0  context-aware=       0
    standalone/plain/find_context_blockquotes
                            time:   [10.989 ms 11.003 ms 11.023 ms]
                            thrpt:  [24.822 GiB/s 24.867 GiB/s 24.899 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_context_bullet_items full=       0  context-aware=       0
    standalone/plain/find_context_bullet_items
                            time:   [12.847 ms 12.859 ms 12.871 ms]
                            thrpt:  [21.258 GiB/s 21.277 GiB/s 21.297 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) high mild
      2 (10.00%) high severe
        find_context_ordered_items full=       0  context-aware=       0
    standalone/plain/find_context_ordered_items
                            time:   [28.841 ms 28.890 ms 28.958 ms]
                            thrpt:  [9.4483 GiB/s 9.4706 GiB/s 9.4867 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    standalone/plain/find_context_italics_cold
                            time:   [23.263 ms 23.286 ms 23.308 ms]
                            thrpt:  [11.738 GiB/s 11.750 GiB/s 11.761 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    standalone/plain/find_context_bolds_cold
                            time:   [23.404 ms 23.442 ms 23.483 ms]
                            thrpt:  [11.651 GiB/s 11.672 GiB/s 11.690 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    standalone/plain/find_context_bold_italics_cold
                            time:   [23.272 ms 23.283 ms 23.295 ms]
                            thrpt:  [11.745 GiB/s 11.751 GiB/s 11.757 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) high mild
      2 (10.00%) high severe
    standalone/plain/find_context_headings_cold
                            time:   [23.265 ms 23.296 ms 23.323 ms]
                            thrpt:  [11.731 GiB/s 11.745 GiB/s 11.760 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
    standalone/plain/find_context_thematic_breaks_cold
                            time:   [25.296 ms 25.329 ms 25.372 ms]
                            thrpt:  [10.784 GiB/s 10.802 GiB/s 10.816 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    standalone/plain/find_context_blockquotes_cold
                            time:   [23.398 ms 23.437 ms 23.480 ms]
                            thrpt:  [11.653 GiB/s 11.674 GiB/s 11.693 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    standalone/plain/find_context_bullet_items_cold
                            time:   [25.033 ms 25.060 ms 25.102 ms]
                            thrpt:  [10.900 GiB/s 10.918 GiB/s 10.930 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    standalone/plain/find_context_ordered_items_cold
                            time:   [41.058 ms 41.106 ms 41.175 ms]
                            thrpt:  [6.6450 GiB/s 6.6561 GiB/s 6.6639 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
    ```
</details>

<details>
    <summary>hot full log</summary>
    
    ```
    ┌─ corpus: hot
    │  size:         75.40 MiB  (79060000 bytes)
    │  elements:   6500000     (84.2 per KiB)
    │  span mem:     49.59 MiB  (~65.8% of input, 8 B/span lower bound)
    │
    │          headings:    500000    thematic_breaks:         0         paragraphs:    500000
    │       blockquotes:         0       fenced_codes:         0       bullet_items:         0
    │     ordered_items:         0              bolds:    500000            italics:    500000
    │      bold_italics:         0              codes:    500000              links:    500000
    │         autolinks:    500000        hard_breaks:         0              texts:   3000000
    └─
    │  full-vs-standalone counts:
        find_codes         full=  500000  standalone=  500000
    standalone/hot/find_codes
                            time:   [7.9922 ms 7.9964 ms 8.0025 ms]
                            thrpt:  [9.2009 GiB/s 9.2079 GiB/s 9.2128 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
        find_italics       full=  500000  standalone=  500000
    standalone/hot/find_italics
                            time:   [12.512 ms 12.517 ms 12.523 ms]
                            thrpt:  [5.8798 GiB/s 5.8823 GiB/s 5.8846 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_bolds         full=  500000  standalone=  500000
    standalone/hot/find_bolds
                            time:   [12.719 ms 12.733 ms 12.748 ms]
                            thrpt:  [5.7756 GiB/s 5.7824 GiB/s 5.7892 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_bold_italics  full=       0  standalone=       0
    standalone/hot/find_bold_italics
                            time:   [11.784 ms 11.804 ms 11.834 ms]
                            thrpt:  [6.2217 GiB/s 6.2376 GiB/s 6.2485 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_autolinks     full=  500000  standalone=  500000
    standalone/hot/find_autolinks
                            time:   [7.8626 ms 7.8717 ms 7.8850 ms]
                            thrpt:  [9.3380 GiB/s 9.3538 GiB/s 9.3646 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) high mild
      2 (10.00%) high severe
        find_links         full=  500000  standalone=  500000
    standalone/hot/find_links
                            time:   [11.902 ms 11.908 ms 11.915 ms]
                            thrpt:  [6.1796 GiB/s 6.1832 GiB/s 6.1862 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_headings      full=  500000  standalone=  500000
    standalone/hot/find_headings
                            time:   [8.1349 ms 8.1425 ms 8.1510 ms]
                            thrpt:  [9.0333 GiB/s 9.0428 GiB/s 9.0512 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_thematic_breaks full=       0  standalone=       0
    standalone/hot/find_thematic_breaks
                            time:   [7.7165 ms 7.7317 ms 7.7500 ms]
                            thrpt:  [9.5007 GiB/s 9.5232 GiB/s 9.5419 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_fenced_codes  full=       0  standalone=       0
    standalone/hot/find_fenced_codes
                            time:   [7.1899 ms 7.1928 ms 7.1960 ms]
                            thrpt:  [10.232 GiB/s 10.237 GiB/s 10.241 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
        find_blockquotes   full=       0  standalone=       0
    standalone/hot/find_blockquotes
                            time:   [4.7635 ms 4.7678 ms 4.7727 ms]
                            thrpt:  [15.427 GiB/s 15.443 GiB/s 15.457 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
        find_bullet_items  full=       0  standalone=       0
    standalone/hot/find_bullet_items
                            time:   [7.5929 ms 7.6009 ms 7.6152 ms]
                            thrpt:  [9.6689 GiB/s 9.6871 GiB/s 9.6973 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_ordered_items full=       0  standalone=       0
    standalone/hot/find_ordered_items
                            time:   [10.032 ms 10.048 ms 10.066 ms]
                            thrpt:  [7.3146 GiB/s 7.3279 GiB/s 7.3398 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) high mild
      1 (5.00%) high severe
    │  context regions: 1000000
    standalone/hot/context  time:   [16.140 ms 16.162 ms 16.189 ms]
                            thrpt:  [4.5481 GiB/s 4.5558 GiB/s 4.5620 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    │  full-vs-context-aware counts:
        find_context_italics full=  500000  context-aware=  500000
    standalone/hot/find_context_italics
                            time:   [14.194 ms 14.281 ms 14.339 ms]
                            thrpt:  [5.1349 GiB/s 5.1556 GiB/s 5.1873 GiB/s]
        find_context_bolds full=  500000  context-aware=  500000
    standalone/hot/find_context_bolds
                            time:   [14.507 ms 14.523 ms 14.540 ms]
                            thrpt:  [5.0639 GiB/s 5.0699 GiB/s 5.0756 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) high mild
      2 (10.00%) high severe
        find_context_bold_italics full=       0  context-aware=       0
    standalone/hot/find_context_bold_italics
                            time:   [12.569 ms 12.597 ms 12.645 ms]
                            thrpt:  [5.8227 GiB/s 5.8449 GiB/s 5.8583 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
        find_context_headings full=  500000  context-aware=  500000
    standalone/hot/find_context_headings
                            time:   [8.5742 ms 8.5795 ms 8.5854 ms]
                            thrpt:  [8.5763 GiB/s 8.5821 GiB/s 8.5874 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
        find_context_thematic_breaks full=       0  context-aware=       0
    standalone/hot/find_context_thematic_breaks
                            time:   [7.7169 ms 7.7220 ms 7.7273 ms]
                            thrpt:  [9.5286 GiB/s 9.5351 GiB/s 9.5414 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_context_blockquotes full=       0  context-aware=       0
    standalone/hot/find_context_blockquotes
                            time:   [4.7562 ms 4.7600 ms 4.7645 ms]
                            thrpt:  [15.454 GiB/s 15.469 GiB/s 15.481 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
        find_context_bullet_items full=       0  context-aware=       0
    standalone/hot/find_context_bullet_items
                            time:   [7.6691 ms 7.6796 ms 7.6971 ms]
                            thrpt:  [9.5659 GiB/s 9.5878 GiB/s 9.6009 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      3 (15.00%) high severe
        find_context_ordered_items full=       0  context-aware=       0
    standalone/hot/find_context_ordered_items
                            time:   [10.401 ms 10.406 ms 10.412 ms]
                            thrpt:  [7.0719 GiB/s 7.0756 GiB/s 7.0792 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    standalone/hot/find_context_italics_cold
                            time:   [30.313 ms 30.341 ms 30.374 ms]
                            thrpt:  [2.4241 GiB/s 2.4268 GiB/s 2.4290 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    standalone/hot/find_context_bolds_cold
                            time:   [30.591 ms 30.660 ms 30.732 ms]
                            thrpt:  [2.3959 GiB/s 2.4015 GiB/s 2.4069 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    standalone/hot/find_context_bold_italics_cold
                            time:   [28.686 ms 28.778 ms 28.890 ms]
                            thrpt:  [2.5487 GiB/s 2.5586 GiB/s 2.5668 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    standalone/hot/find_context_headings_cold
                            time:   [24.638 ms 24.677 ms 24.708 ms]
                            thrpt:  [2.9800 GiB/s 2.9838 GiB/s 2.9884 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    standalone/hot/find_context_thematic_breaks_cold
                            time:   [23.878 ms 23.898 ms 23.918 ms]
                            thrpt:  [3.0784 GiB/s 3.0810 GiB/s 3.0836 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    standalone/hot/find_context_blockquotes_cold
                            time:   [20.928 ms 20.975 ms 21.030 ms]
                            thrpt:  [3.5011 GiB/s 3.5103 GiB/s 3.5182 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    standalone/hot/find_context_bullet_items_cold
                            time:   [23.753 ms 23.769 ms 23.787 ms]
                            thrpt:  [3.0954 GiB/s 3.0977 GiB/s 3.0998 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    standalone/hot/find_context_ordered_items_cold
                            time:   [26.257 ms 26.269 ms 26.283 ms]
                            thrpt:  [2.8015 GiB/s 2.8029 GiB/s 2.8043 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    ```
</details>

<details>
    <summary>heavy full log</summary>
    
    ```
    ┌─ corpus: heavy
    │  size:        146.96 MiB  (154102000 bytes)
    │  elements:  14000000     (93.0 per KiB)
    │  span mem:    106.81 MiB  (~72.7% of input, 8 B/span lower bound)
    │
    │          headings:    200000    thematic_breaks:    200000         paragraphs:    400000
    │       blockquotes:    400000       fenced_codes:    200000       bullet_items:    600000
    │     ordered_items:    400000              bolds:   1200000            italics:   1200000
    │      bold_italics:    600000              codes:   1000000              links:    600000
    │         autolinks:    400000        hard_breaks:         0              texts:   6600000
    └─
    │  full-vs-standalone counts:
        find_codes         full= 1000000  standalone= 1000000
    standalone/heavy/find_codes
                            time:   [19.816 ms 19.855 ms 19.903 ms]
                            thrpt:  [7.2110 GiB/s 7.2284 GiB/s 7.2426 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_italics       full= 1200000  standalone= 1200000
    standalone/heavy/find_italics
                            time:   [40.874 ms 41.090 ms 41.254 ms]
                            thrpt:  [3.4789 GiB/s 3.4928 GiB/s 3.5113 GiB/s]
        find_bolds         full= 1200000  standalone= 1200000
    standalone/heavy/find_bolds
                            time:   [40.460 ms 40.476 ms 40.491 ms]
                            thrpt:  [3.5444 GiB/s 3.5458 GiB/s 3.5471 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low mild
      1 (5.00%) high severe
        find_bold_italics  full=  600000  standalone=  600000
    standalone/heavy/find_bold_italics
                            time:   [38.288 ms 38.294 ms 38.300 ms]
                            thrpt:  [3.7473 GiB/s 3.7478 GiB/s 3.7484 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low severe
      1 (5.00%) low mild
      1 (5.00%) high severe
        find_autolinks     full=  400000  standalone=  400000
    standalone/heavy/find_autolinks
                            time:   [9.3684 ms 9.3776 ms 9.3858 ms]
                            thrpt:  [15.291 GiB/s 15.304 GiB/s 15.319 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_links         full=  600000  standalone=  600000
    standalone/heavy/find_links
                            time:   [15.691 ms 15.699 ms 15.709 ms]
                            thrpt:  [9.1361 GiB/s 9.1418 GiB/s 9.1466 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_headings      full=  200000  standalone=  200000
    standalone/heavy/find_headings
                            time:   [8.2200 ms 8.2278 ms 8.2376 ms]
                            thrpt:  [17.422 GiB/s 17.443 GiB/s 17.460 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_thematic_breaks full=  200000  standalone=  200000
    standalone/heavy/find_thematic_breaks
                            time:   [29.288 ms 29.326 ms 29.384 ms]
                            thrpt:  [4.8843 GiB/s 4.8939 GiB/s 4.9003 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_fenced_codes  full=  200000  standalone=  200000
    standalone/heavy/find_fenced_codes
                            time:   [20.326 ms 20.358 ms 20.400 ms]
                            thrpt:  [7.0353 GiB/s 7.0497 GiB/s 7.0607 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_blockquotes   full=  400000  standalone=  400000
    standalone/heavy/find_blockquotes
                            time:   [14.052 ms 14.086 ms 14.114 ms]
                            thrpt:  [10.169 GiB/s 10.188 GiB/s 10.214 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_bullet_items  full=  600000  standalone=  600000
    standalone/heavy/find_bullet_items
                            time:   [27.849 ms 27.914 ms 28.005 ms]
                            thrpt:  [5.1248 GiB/s 5.1414 GiB/s 5.1535 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_ordered_items full=  400000  standalone=  400000
    standalone/heavy/find_ordered_items
                            time:   [23.018 ms 23.064 ms 23.103 ms]
                            thrpt:  [6.2120 GiB/s 6.2227 GiB/s 6.2350 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    │  context regions: 1600000
    standalone/heavy/context
                            time:   [30.932 ms 31.023 ms 31.127 ms]
                            thrpt:  [4.6108 GiB/s 4.6262 GiB/s 4.6398 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    │  full-vs-context-aware counts:
        find_context_italics full= 1200000  context-aware= 1200000
    standalone/heavy/find_context_italics
                            time:   [46.423 ms 46.510 ms 46.601 ms]
                            thrpt:  [3.0797 GiB/s 3.0858 GiB/s 3.0916 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
        find_context_bolds full= 1200000  context-aware= 1200000
    standalone/heavy/find_context_bolds
                            time:   [46.440 ms 46.518 ms 46.605 ms]
                            thrpt:  [3.0795 GiB/s 3.0852 GiB/s 3.0904 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
        find_context_bold_italics full=  600000  context-aware=  600000
    standalone/heavy/find_context_bold_italics
                            time:   [44.326 ms 44.393 ms 44.447 ms]
                            thrpt:  [3.2290 GiB/s 3.2329 GiB/s 3.2378 GiB/s]
        find_context_headings full=  200000  context-aware=  200000
    standalone/heavy/find_context_headings
                            time:   [8.6883 ms 8.7261 ms 8.7717 ms]
                            thrpt:  [16.362 GiB/s 16.447 GiB/s 16.519 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
        find_context_thematic_breaks full=  200000  context-aware=  200000
    standalone/heavy/find_context_thematic_breaks
                            time:   [29.594 ms 29.643 ms 29.700 ms]
                            thrpt:  [4.8323 GiB/s 4.8416 GiB/s 4.8496 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) high mild
      1 (5.00%) high severe
        find_context_blockquotes full=  400000  context-aware=  400000
    standalone/heavy/find_context_blockquotes
                            time:   [14.128 ms 14.165 ms 14.201 ms]
                            thrpt:  [10.106 GiB/s 10.132 GiB/s 10.158 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_context_bullet_items full=  600000  context-aware=  600000
    standalone/heavy/find_context_bullet_items
                            time:   [28.893 ms 28.960 ms 29.026 ms]
                            thrpt:  [4.9444 GiB/s 4.9558 GiB/s 4.9673 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_context_ordered_items full=  400000  context-aware=  400000
    standalone/heavy/find_context_ordered_items
                            time:   [24.281 ms 24.417 ms 24.520 ms]
                            thrpt:  [5.8530 GiB/s 5.8778 GiB/s 5.9107 GiB/s]
    Benchmarking standalone/heavy/find_context_italics_cold: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 16.2s, enable flat sampling, or reduce sample count to 10.
    standalone/heavy/find_context_italics_cold
                            time:   [76.433 ms 76.602 ms 76.816 ms]
                            thrpt:  [1.8683 GiB/s 1.8736 GiB/s 1.8777 GiB/s]
    Benchmarking standalone/heavy/find_context_bolds_cold: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 16.3s, enable flat sampling, or reduce sample count to 10.
    standalone/heavy/find_context_bolds_cold
                            time:   [76.864 ms 77.130 ms 77.403 ms]
                            thrpt:  [1.8542 GiB/s 1.8607 GiB/s 1.8672 GiB/s]
    Benchmarking standalone/heavy/find_context_bold_italics_cold: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 15.8s, enable flat sampling, or reduce sample count to 10.
    standalone/heavy/find_context_bold_italics_cold
                            time:   [74.518 ms 74.743 ms 75.029 ms]
                            thrpt:  [1.9128 GiB/s 1.9202 GiB/s 1.9260 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    standalone/heavy/find_context_headings_cold
                            time:   [39.519 ms 39.594 ms 39.689 ms]
                            thrpt:  [3.6161 GiB/s 3.6247 GiB/s 3.6316 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    Benchmarking standalone/heavy/find_context_thematic_breaks_cold: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 12.7s, enable flat sampling, or reduce sample count to 10.
    standalone/heavy/find_context_thematic_breaks_cold
                            time:   [60.380 ms 60.460 ms 60.565 ms]
                            thrpt:  [2.3697 GiB/s 2.3738 GiB/s 2.3769 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    standalone/heavy/find_context_blockquotes_cold
                            time:   [45.039 ms 45.132 ms 45.264 ms]
                            thrpt:  [3.1707 GiB/s 3.1799 GiB/s 3.1865 GiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      3 (15.00%) high mild
      1 (5.00%) high severe
    Benchmarking standalone/heavy/find_context_bullet_items_cold: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 12.5s, enable flat sampling, or reduce sample count to 10.
    standalone/heavy/find_context_bullet_items_cold
                            time:   [59.264 ms 59.341 ms 59.429 ms]
                            thrpt:  [2.4150 GiB/s 2.4185 GiB/s 2.4217 GiB/s]
    Benchmarking standalone/heavy/find_context_ordered_items_cold: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 11.6s, enable flat sampling, or reduce sample count to 10.
    standalone/heavy/find_context_ordered_items_cold
                            time:   [55.351 ms 55.434 ms 55.546 ms]
                            thrpt:  [2.5838 GiB/s 2.5890 GiB/s 2.5929 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    ```
</details>

---

## meon-md context-aware extraction (`context()` + `find_context_*`)

`context(source)` builds the opaque-region map — fenced blocks, code spans,
autolinks — in one streaming pass; `find_context_*` is the same standalone
matcher with candidate delimiters inside those regions skipped. Every rule
that is not itself opaque gets a variant; the opaque sources keep only their
context-free `find_*`. This closes the fence/opacity divergence the section
above documents — the `full` vs `context-aware` counts are reported alongside
(see
[`ARCHITECTURE.md §12`](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE.md#12-standalone-iterators)).

Three groups per corpus:

- `context` — building the `ParseContext` alone. The map is built once per
  source and shared by every `find_context_*` over it, so this cost amortises
  across element kinds.
- `find_context_*` — the scan against a prebuilt map; the per-candidate
  overhead relative to the context-free `find_*` above.
- `find_context_*_cold` — map build plus scan in a single call: the one-shot
  price when no map is reused.

---

## Reading the numbers

- The figures show an architectural difference (flat type-indexed spans vs event
  stream vs AST) and `meon-md`'s deliberate Markdown-subset scope. A consumer
  that needs a tree can build one over meon's spans.
- Compare a cell only against the same corpus in the same build block.
- `pulldown-cmark` is the closest-shape pair; `comrak` is the upper bound (it
  owns a tree). The gap between them brackets the cost of AST construction over a
  pure event stream.
- **Scaling is the real signal.** meon holds throughput flat from small to big;
  the comparators lose 34–51%. A flat span table is what stays cache-resident.
- The corpora are written for `meon-md`'s subset; a real-world CommonMark
  workload shifts the comparators' cost relative to what is shown.
