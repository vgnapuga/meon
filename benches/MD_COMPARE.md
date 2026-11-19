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
cache. The `small` and `big` runs differ only in `REPEAT_COUNT`, selected
by the `small` / `big` Cargo features of the `benches` package.

| Corpus  | Shape                                                                  | Stresses                                                          |
|---------|------------------------------------------------------------------------|-------------------------------------------------------------------|
| `plain` | Prose only, no markup.                                                 | Fallback/text path, line loop. Ceiling case (near-pure scanning). |
| `hot`   | Light, evenly spread markup (~one of each common inline per paragraph).| Bit more complicated than typical real-world document.            |
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
│          headings:         0 √    thematic_breaks:         0 √         paragraphs:         1 √
│       blockquotes:         0 √       fenced_codes:         0 √       bullet_items:         0 √
│     ordered_items:         0 √              bolds:         0 √            italics:         0 √
│      bold_italics:         0 √              codes:         0 √              links:         0 √
│         autolinks:         0 √        hard_breaks:         0 √              texts:         1 √
└─

┌─ corpus: hot
│  size:          0.79 MiB  (830600 bytes)
│  elements:     70000     (86.3 per KiB)
│  span mem:      0.53 MiB  (~67.4% of input, 8 B/span lower bound)
│
│          headings:      5000 √    thematic_breaks:         0 √         paragraphs:      5000 √
│       blockquotes:         0 √       fenced_codes:         0 √       bullet_items:         0 √
│     ordered_items:         0 √              bolds:      5000 √            italics:      5000 √
│      bold_italics:         0 √              codes:      5000 √              links:      5000 √
│         autolinks:      5000 √        hard_breaks:         0 √              texts:     35000 √
└─

┌─ corpus: heavy
│  size:          1.50 MiB  (1575020 bytes)
│  elements:    144000     (93.6 per KiB)
│  span mem:      1.10 MiB  (~73.1% of input, 8 B/span lower bound)
│
│          headings:      2000 √    thematic_breaks:      2000 √         paragraphs:      4000 √
│       blockquotes:      4000 √       fenced_codes:      4000 √       bullet_items:      6000 √
│     ordered_items:      4000 √              bolds:     12000 √            italics:     12000 √
│      bold_italics:      6000 √              codes:     10000 √              links:      6000 √
│         autolinks:      4000 √        hard_breaks:         0 √              texts:     68000 √
└─
```

**big (REPEAT_COUNT = 1000, exceeds L3 cache)**

```
┌─ corpus: plain
│  size:        280.17 MiB  (293780000 bytes)
│  elements:         2     (0.0 per KiB)
│  span mem:      0.00 MiB  (~0.0% of input, 8 B/span lower bound)
│
│          headings:         0 √    thematic_breaks:         0 √         paragraphs:         1 √
│       blockquotes:         0 √       fenced_codes:         0 √       bullet_items:         0 √
│     ordered_items:         0 √              bolds:         0 √            italics:         0 √
│      bold_italics:         0 √              codes:         0 √              links:         0 √
│         autolinks:         0 √        hard_breaks:         0 √              texts:         1 √
└─

┌─ corpus: hot
│  size:         79.21 MiB  (83060000 bytes)
│  elements:   7000000     (86.3 per KiB)
│  span mem:     53.41 MiB  (~67.4% of input, 8 B/span lower bound)
│
│          headings:    500000 √    thematic_breaks:         0 √         paragraphs:    500000 √
│       blockquotes:         0 √       fenced_codes:         0 √       bullet_items:         0 √
│     ordered_items:         0 √              bolds:    500000 √            italics:    500000 √
│      bold_italics:         0 √              codes:    500000 √              links:    500000 √
│         autolinks:    500000 √        hard_breaks:         0 √              texts:   3500000 √
└─

┌─ corpus: heavy
│  size:        150.21 MiB  (157502000 bytes)
│  elements:  14400000     (93.6 per KiB)
│  span mem:    109.86 MiB  (~73.1% of input, 8 B/span lower bound)
│
│          headings:    200000 √    thematic_breaks:    200000 √         paragraphs:    400000 √
│       blockquotes:    400000 √       fenced_codes:    400000 √       bullet_items:    600000 √
│     ordered_items:    400000 √              bolds:   1200000 √            italics:   1200000 √
│      bold_italics:    600000 √              codes:   1000000 √              links:    600000 √
│         autolinks:    400000 √        hard_breaks:         0 √              texts:   6800000 √
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
| `plain` | [956.38 µs]=[2.8608 GiB/s] | [3.7007 ms]=[757.07 MiB/s] | [9.9962 ms]=[280.28 MiB/s] |
| `hot`   | [837.40 µs]=[945.93 MiB/s] | [4.8029 ms]=[156.98 MiB/s] | [16.587 ms]=[45.457 MiB/s] |
| `heavy` | [1.8387 ms]=[816.93 MiB/s] | [12.602 ms]=[116.62 MiB/s] | [43.289 ms]=[33.949 MiB/s] |

<details>
    <summary>full log</summary>

    ```
    ┌─ corpus: plain
    │  size:          2.80 MiB  (2937800 bytes)
    │  elements:         2     (0.0 per KiB)
    │  span mem:      0.00 MiB  (~0.0% of input, 8 B/span lower bound)
    │
    │          headings:         0 √    thematic_breaks:         0 √         paragraphs:         1 √
    │       blockquotes:         0 √       fenced_codes:         0 √       bullet_items:         0 √
    │     ordered_items:         0 √              bolds:         0 √            italics:         0 √
    │      bold_italics:         0 √              codes:         0 √              links:         0 √
    │         autolinks:         0 √        hard_breaks:         0 √              texts:         1 √
    └─
    compare/plain/meon-md   time:   [951.19 µs 956.38 µs 961.32 µs]
                            thrpt:  [2.8461 GiB/s 2.8608 GiB/s 2.8764 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    compare/plain/pulldown-cmark
                            time:   [3.0871 ms 3.0933 ms 3.1042 ms]
                            thrpt:  [902.56 MiB/s 905.72 MiB/s 907.55 MiB/s]
    compare/plain/comrak    time:   [12.015 ms 12.295 ms 12.645 ms]
                            thrpt:  [221.57 MiB/s 227.87 MiB/s 233.18 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    
    
    ┌─ corpus: hot
    │  size:          0.79 MiB  (830600 bytes)
    │  elements:     70000     (86.3 per KiB)
    │  span mem:      0.53 MiB  (~67.4% of input, 8 B/span lower bound)
    │
    │          headings:      5000 √    thematic_breaks:         0 √         paragraphs:      5000 √
    │       blockquotes:         0 √       fenced_codes:         0 √       bullet_items:         0 √
    │     ordered_items:         0 √              bolds:      5000 √            italics:      5000 √
    │      bold_italics:         0 √              codes:      5000 √              links:      5000 √
    │         autolinks:      5000 √        hard_breaks:         0 √              texts:     35000 √
    └─
    compare/hot/meon-md     time:   [834.99 µs 837.40 µs 839.41 µs]
                            thrpt:  [943.66 MiB/s 945.93 MiB/s 948.66 MiB/s]
    compare/hot/pulldown-cmark
                            time:   [5.1102 ms 5.1572 ms 5.2223 ms]
                            thrpt:  [151.68 MiB/s 153.59 MiB/s 155.01 MiB/s]
    compare/hot/comrak      time:   [19.002 ms 19.185 ms 19.524 ms]
                            thrpt:  [40.572 MiB/s 41.289 MiB/s 41.685 MiB/s]
    
    
    ┌─ corpus: heavy
    │  size:          1.50 MiB  (1575020 bytes)
    │  elements:    144000     (93.6 per KiB)
    │  span mem:      1.10 MiB  (~73.1% of input, 8 B/span lower bound)
    │
    │          headings:      2000 √    thematic_breaks:      2000 √         paragraphs:      4000 √
    │       blockquotes:      4000 √       fenced_codes:      4000 √       bullet_items:      6000 √
    │     ordered_items:      4000 √              bolds:     12000 √            italics:     12000 √
    │      bold_italics:      6000 √              codes:     10000 √              links:      6000 √
    │         autolinks:      4000 √        hard_breaks:         0 √              texts:     68000 √
    └─
    compare/heavy/meon-md   time:   [1.8137 ms 1.8387 ms 1.8577 ms]
                            thrpt:  [808.55 MiB/s 816.93 MiB/s 828.16 MiB/s]
    compare/heavy/pulldown-cmark
                            time:   [13.576 ms 13.637 ms 13.727 ms]
                            thrpt:  [109.42 MiB/s 110.15 MiB/s 110.64 MiB/s]
    Benchmarking compare/heavy/comrak: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 10.3s, enable flat sampling, or reduce sample count to 10.
    compare/heavy/comrak    time:   [48.751 ms 50.614 ms 52.242 ms]
                            thrpt:  [28.752 MiB/s 29.677 MiB/s 30.810 MiB/s]
    ```

</details

**big (exceeds L3 cache):**

| Corpus  | `meon-md`                  | `pulldown-cmark`           | `comrak`                  |
|---------|----------------------------|----------------------------|---------------------------|
| `plain` | [101.46 ms]=[2.6967 GiB/s] | [579.15 ms]=[483.76 MiB/s] | [2.3552 s]=[118.96 MiB/s] |
| `hot`   | [85.646 ms]=[924.87 MiB/s] | [853.62 ms]=[88.327 MiB/s] | [3.2433 s]=[23.247 MiB/s] |
| `heavy` | [181.72 ms]=[826.57 MiB/s] | [2.0413 s]=[71.996 MiB/s]  | [7.4457 s]=[19.738 MiB/s] |

<details>
    <summary>full log</summary>

    ```
    ┌─ corpus: plain
    │  size:        280.17 MiB  (293780000 bytes)
    │  elements:         2     (0.0 per KiB)
    │  span mem:      0.00 MiB  (~0.0% of input, 8 B/span lower bound)
    │
    │          headings:         0 √    thematic_breaks:         0 √         paragraphs:         1 √
    │       blockquotes:         0 √       fenced_codes:         0 √       bullet_items:         0 √
    │     ordered_items:         0 √              bolds:         0 √            italics:         0 √
    │      bold_italics:         0 √              codes:         0 √              links:         0 √
    │         autolinks:         0 √        hard_breaks:         0 √              texts:         1 √
    └─
    compare/plain/meon-md   time:   [100.91 ms 101.46 ms 102.06 ms]
                            thrpt:  [2.6807 GiB/s 2.6967 GiB/s 2.7113 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    Benchmarking compare/plain/pulldown-cmark: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 10.7s, or reduce sample count to 10.
    compare/plain/pulldown-cmark
                            time:   [520.02 ms 522.44 ms 525.25 ms]
                            thrpt:  [533.41 MiB/s 536.27 MiB/s 538.77 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    Benchmarking compare/plain/comrak: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 48.0s, or reduce sample count to 10.
    compare/plain/comrak    time:   [2.4075 s 2.4272 s 2.4489 s]
                            thrpt:  [114.41 MiB/s 115.43 MiB/s 116.37 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    
    
    ┌─ corpus: hot
    │  size:         79.21 MiB  (83060000 bytes)
    │  elements:   7000000     (86.3 per KiB)
    │  span mem:     53.41 MiB  (~67.4% of input, 8 B/span lower bound)
    │
    │          headings:    500000 √    thematic_breaks:         0 √         paragraphs:    500000 √
    │       blockquotes:         0 √       fenced_codes:         0 √       bullet_items:         0 √
    │     ordered_items:         0 √              bolds:    500000 √            italics:    500000 √
    │      bold_italics:         0 √              codes:    500000 √              links:    500000 √
    │         autolinks:    500000 √        hard_breaks:         0 √              texts:   3500000 √
    └─
    Benchmarking compare/hot/meon-md: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 17.7s, enable flat sampling, or reduce sample count to 10.
    compare/hot/meon-md     time:   [85.119 ms 85.646 ms 86.174 ms]
                            thrpt:  [919.21 MiB/s 924.87 MiB/s 930.60 MiB/s]
    Benchmarking compare/hot/pulldown-cmark: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 18.4s, or reduce sample count to 10.
    compare/hot/pulldown-cmark
                            time:   [919.24 ms 925.28 ms 931.72 ms]
                            thrpt:  [85.017 MiB/s 85.609 MiB/s 86.171 MiB/s]
    Benchmarking compare/hot/comrak: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 70.5s, or reduce sample count to 10.
    compare/hot/comrak      time:   [3.4097 s 3.4252 s 3.4410 s]
                            thrpt:  [23.020 MiB/s 23.126 MiB/s 23.232 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    
    
    ┌─ corpus: heavy
    │  size:        150.21 MiB  (157502000 bytes)
    │  elements:  14400000     (93.6 per KiB)
    │  span mem:    109.86 MiB  (~73.1% of input, 8 B/span lower bound)
    │
    │          headings:    200000 √    thematic_breaks:    200000 √         paragraphs:    400000 √
    │       blockquotes:    400000 √       fenced_codes:    400000 √       bullet_items:    600000 √
    │     ordered_items:    400000 √              bolds:   1200000 √            italics:   1200000 √
    │      bold_italics:    600000 √              codes:   1000000 √              links:    600000 √
    │         autolinks:    400000 √        hard_breaks:         0 √              texts:   6800000 √
    └─
    compare/heavy/meon-md   time:   [181.47 ms 181.72 ms 182.01 ms]
                            thrpt:  [825.25 MiB/s 826.57 MiB/s 827.70 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    Benchmarking compare/heavy/pulldown-cmark: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 42.6s, or reduce sample count to 10.
    compare/heavy/pulldown-cmark
                            time:   [2.1210 s 2.1231 s 2.1257 s]
                            thrpt:  [70.662 MiB/s 70.747 MiB/s 70.819 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    Benchmarking compare/heavy/comrak: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 161.1s, or reduce sample count to 10.
    compare/heavy/comrak    time:   [7.7215 s 7.7436 s 7.7715 s]
                            thrpt:  [19.328 MiB/s 19.397 MiB/s 19.453 MiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      2 (10.00%) high mild
      2 (10.00%) high severe
    ```

</details>

### nightly - `RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_compare --features avx2`

> meon on AVX2; `pulldown-cmark` and `comrak` scalar (no `simd` feature). The
> meon column is AVX2 against scalar comparators — not a like-for-like SIMD row.

**small (fits in cache):**

| Corpus  | `meon-md`                  | `pulldown-cmark`           | `comrak`                   |
|---------|----------------------------|----------------------------|----------------------------|
| `plain` | [575.25 µs]=[4.7563 GiB/s] | [3.1806 ms]=[880.88 MiB/s] | [9.6308 ms]=[290.91 MiB/s] |
| `hot`   | [635.61 µs]=[1.2170 GiB/s] | [5.3109 ms]=[141.97 MiB/s] | [15.623 ms]=[48.262 MiB/s] |
| `heavy` | [1.5851 ms]=[947.59 MiB/s] | [13.439 ms]=[109.35 MiB/s] | [42.209 ms]=[34.818 MiB/s] |

<details>
    <summary>full log</summary>

    ```
    ┌─ corpus: plain
    │  size:          2.80 MiB  (2937800 bytes)
    │  elements:         2     (0.0 per KiB)
    │  span mem:      0.00 MiB  (~0.0% of input, 8 B/span lower bound)
    │
    │          headings:         0 √    thematic_breaks:         0 √         paragraphs:         1 √
    │       blockquotes:         0 √       fenced_codes:         0 √       bullet_items:         0 √
    │     ordered_items:         0 √              bolds:         0 √            italics:         0 √
    │      bold_italics:         0 √              codes:         0 √              links:         0 √
    │         autolinks:         0 √        hard_breaks:         0 √              texts:         1 √
    └─
    compare/plain/meon-md   time:   [573.65 µs 575.25 µs 576.67 µs]
                            thrpt:  [4.7445 GiB/s 4.7563 GiB/s 4.7695 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
    2 (10.00%) high mild
    compare/plain/pulldown-cmark
                            time:   [3.8400 ms 3.8829 ms 3.9496 ms]
                            thrpt:  [709.37 MiB/s 721.55 MiB/s 729.60 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
    1 (5.00%) high severe
    compare/plain/comrak    time:   [11.437 ms 11.476 ms 11.508 ms]
                            thrpt:  [243.45 MiB/s 244.13 MiB/s 244.97 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
    1 (5.00%) high severe
    
    
    ┌─ corpus: hot
    │  size:          0.79 MiB  (830600 bytes)
    │  elements:     70000     (86.3 per KiB)
    │  span mem:      0.53 MiB  (~67.4% of input, 8 B/span lower bound)
    │
    │          headings:      5000 √    thematic_breaks:         0 √         paragraphs:      5000 √
    │       blockquotes:         0 √       fenced_codes:         0 √       bullet_items:         0 √
    │     ordered_items:         0 √              bolds:      5000 √            italics:      5000 √
    │      bold_italics:         0 √              codes:      5000 √              links:      5000 √
    │         autolinks:      5000 √        hard_breaks:         0 √              texts:     35000 √
    └─
    compare/hot/meon-md     time:   [633.13 µs 635.61 µs 638.68 µs]
                            thrpt:  [1.2112 GiB/s 1.2170 GiB/s 1.2218 GiB/s]
    compare/hot/pulldown-cmark
                            time:   [5.2325 ms 5.2546 ms 5.2780 ms]
                            thrpt:  [150.08 MiB/s 150.75 MiB/s 151.38 MiB/s]
    compare/hot/comrak      time:   [19.888 ms 20.366 ms 20.849 ms]
                            thrpt:  [37.992 MiB/s 38.894 MiB/s 39.830 MiB/s]
    
    
    ┌─ corpus: heavy
    │  size:          1.50 MiB  (1575020 bytes)
    │  elements:    144000     (93.6 per KiB)
    │  span mem:      1.10 MiB  (~73.1% of input, 8 B/span lower bound)
    │
    │          headings:      2000 √    thematic_breaks:      2000 √         paragraphs:      4000 √
    │       blockquotes:      4000 √       fenced_codes:      4000 √       bullet_items:      6000 √
    │     ordered_items:      4000 √              bolds:     12000 √            italics:     12000 √
    │      bold_italics:      6000 √              codes:     10000 √              links:      6000 √
    │         autolinks:      4000 √        hard_breaks:         0 √              texts:     68000 √
    └─
    compare/heavy/meon-md   time:   [1.5799 ms 1.5851 ms 1.5926 ms]
                            thrpt:  [943.12 MiB/s 947.59 MiB/s 950.74 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
    2 (10.00%) high mild
    compare/heavy/pulldown-cmark
                            time:   [14.106 ms 14.238 ms 14.357 ms]
                            thrpt:  [104.62 MiB/s 105.49 MiB/s 106.48 MiB/s]
    Benchmarking compare/heavy/comrak: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 10.2s, enable flat sampling, or reduce sample count to 10.
    compare/heavy/comrak    time:   [45.411 ms 46.254 ms 47.688 ms]
                            thrpt:  [31.497 MiB/s 32.474 MiB/s 33.077 MiB/s]
    ```

</details>

**big (exceeds L3 cache):**

| Corpus  | `meon-md`                  | `pulldown-cmark`           | `comrak`                  |
|---------|----------------------------|----------------------------|---------------------------|
| `plain` | [64.499 ms]=[4.2420 GiB/s] | [531.95 ms]=[526.69 MiB/s] | [2.2354 s]=[125.33 MiB/s] |
| `hot`   | [63.711 ms]=[1.2142 GiB/s] | [886.41 ms]=[85.059 MiB/s] | [3.1578 s]=[23.877 MiB/s] |
| `heavy` | [158.23 ms]=[949.30 MiB/s] | [2.0877 s]=[70.396 MiB/s]  | [7.3278 s]=[20.056 MiB/s] |

<details>
    <summary>full log</summary>

    ```
    ┌─ corpus: plain
    │  size:        280.17 MiB  (293780000 bytes)
    │  elements:         2     (0.0 per KiB)
    │  span mem:      0.00 MiB  (~0.0% of input, 8 B/span lower bound)
    │
    │          headings:         0 √    thematic_breaks:         0 √         paragraphs:         1 √
    │       blockquotes:         0 √       fenced_codes:         0 √       bullet_items:         0 √
    │     ordered_items:         0 √              bolds:         0 √            italics:         0 √
    │      bold_italics:         0 √              codes:         0 √              links:         0 √
    │         autolinks:         0 √        hard_breaks:         0 √              texts:         1 √
    └─
    Benchmarking compare/plain/meon-md: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 13.1s, enable flat sampling, or reduce sample count to 10.
    compare/plain/meon-md   time:   [63.809 ms 64.499 ms 65.042 ms]
                            thrpt:  [4.2066 GiB/s 4.2420 GiB/s 4.2879 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low severe
      2 (10.00%) low mild
    Benchmarking compare/plain/pulldown-cmark: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 12.2s, or reduce sample count to 10.
    compare/plain/pulldown-cmark
                            time:   [620.88 ms 622.49 ms 624.10 ms]
                            thrpt:  [448.92 MiB/s 450.08 MiB/s 451.25 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    Benchmarking compare/plain/comrak: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 50.1s, or reduce sample count to 10.
    compare/plain/comrak    time:   [2.3810 s 2.4111 s 2.4433 s]
                            thrpt:  [114.67 MiB/s 116.20 MiB/s 117.67 MiB/s]
    
    
    ┌─ corpus: hot
    │  size:         79.21 MiB  (83060000 bytes)
    │  elements:   7000000     (86.3 per KiB)
    │  span mem:     53.41 MiB  (~67.4% of input, 8 B/span lower bound)
    │
    │          headings:    500000 √    thematic_breaks:         0 √         paragraphs:    500000 √
    │       blockquotes:         0 √       fenced_codes:         0 √       bullet_items:         0 √
    │     ordered_items:         0 √              bolds:    500000 √            italics:    500000 √
    │      bold_italics:         0 √              codes:    500000 √              links:    500000 √
    │         autolinks:    500000 √        hard_breaks:         0 √              texts:   3500000 √
    └─
    Benchmarking compare/hot/meon-md: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 13.3s, enable flat sampling, or reduce sample count to 10.
    compare/hot/meon-md     time:   [63.661 ms 63.711 ms 63.759 ms]
                            thrpt:  [1.2133 GiB/s 1.2142 GiB/s 1.2151 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    Benchmarking compare/hot/pulldown-cmark: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 18.7s, or reduce sample count to 10.
    compare/hot/pulldown-cmark
                            time:   [934.44 ms 937.15 ms 940.91 ms]
                            thrpt:  [84.187 MiB/s 84.525 MiB/s 84.770 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    Benchmarking compare/hot/comrak: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 68.4s, or reduce sample count to 10.
    compare/hot/comrak      time:   [3.3913 s 3.4041 s 3.4171 s]
                            thrpt:  [23.181 MiB/s 23.270 MiB/s 23.358 MiB/s]
    
    
    ┌─ corpus: heavy
    │  size:        150.21 MiB  (157502000 bytes)
    │  elements:  14400000     (93.6 per KiB)
    │  span mem:    109.86 MiB  (~73.1% of input, 8 B/span lower bound)
    │
    │          headings:    200000 √    thematic_breaks:    200000 √         paragraphs:    400000 √
    │       blockquotes:    400000 √       fenced_codes:    400000 √       bullet_items:    600000 √
    │     ordered_items:    400000 √              bolds:   1200000 √            italics:   1200000 √
    │      bold_italics:    600000 √              codes:   1000000 √              links:    600000 √
    │         autolinks:    400000 √        hard_breaks:         0 √              texts:   6800000 √
    └─
    compare/heavy/meon-md   time:   [157.92 ms 158.23 ms 158.65 ms]
                            thrpt:  [946.78 MiB/s 949.30 MiB/s 951.14 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
    Benchmarking compare/heavy/pulldown-cmark: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 44.0s, or reduce sample count to 10.
    compare/heavy/pulldown-cmark
                            time:   [2.1925 s 2.1953 s 2.1983 s]
                            thrpt:  [68.329 MiB/s 68.423 MiB/s 68.510 MiB/s]
    Benchmarking compare/heavy/comrak: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 170.1s, or reduce sample count to 10.
    compare/heavy/comrak    time:   [7.6907 s 7.8430 s 7.9962 s]
                            thrpt:  [18.785 MiB/s 19.152 MiB/s 19.531 MiB/s]
    ```

</details>

---
  
## Scaling from small to big

The clearest expression of the architecture difference is how each parser holds
up as the input grows past cache (stable build, median `thrpt`):

| Parser              | `plain`              | `hot`                | `heavy`            |
|---------------------|----------------------|----------------------|--------------------|
| `meon-md` - stable  | 2.86 -> 2.70 GiB/s   | 946 -> 925 MiB/s     | 817 -> 827 MiB/s   |
| `meon-md` - nightly | 4.76 -> 4.24 GiB/s   | 1.22 -> 1.21 GiB/s   | 948 -> 949 MiB/s   |
| `pulldown-cmark`    | 757 -> 483 MiB/s     | 157 -> 88 MiB/s      | 117 -> 72 MiB/s    |
| `comrak`            | 280 -> 118 MiB/s     | 45 -> 23 MiB/s       | 34 -> 20 MiB/s     |

- **`meon-md` holds throughput nearly flat** from small to big: stable stays
  within ~6% (`heavy` even ticks up slightly), nightly within ~11%. The output
  is a compact, contiguous span table (`u32` pairs), so the working set stays
  cache-friendly as the document grows.
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
| `find_codes`           | [32.546 µs]=[84.068 GiB/s] | [93.259 µs]=[8.2947 GiB/s] | [248.97 µs]=[5.8917 GiB/s] |
| `find_italics`         | [32.822 µs]=[83.359 GiB/s] | [155.36 µs]=[4.9791 GiB/s] | [448.06 µs]=[3.2738 GiB/s] |
| `find_bolds`           | [32.576 µs]=[83.988 GiB/s] | [156.86 µs]=[4.9315 GiB/s] | [452.70 µs]=[3.2402 GiB/s] |
| `find_bold_italics`    | [32.644 µs]=[83.813 GiB/s] | [131.54 µs]=[5.8806 GiB/s] | [422.71 µs]=[3.4701 GiB/s] |
| `find_autolinks`       | [32.782 µs]=[83.461 GiB/s] | [94.618 µs]=[8.1756 GiB/s] | [83.369 µs]=[17.595 GiB/s] |
| `find_links`           | [33.499 µs]=[81.675 GiB/s] | [215.14 µs]=[3.5956 GiB/s] | [263.68 µs]=[5.5629 GiB/s] |
| `find_headings`        | [30.879 µs]=[88.606 GiB/s] | [97.621 µs]=[7.9241 GiB/s] | [48.143 µs]=[30.469 GiB/s] |
| `find_thematic_breaks` | [43.521 µs]=[62.867 GiB/s] | [79.091 µs]=[9.7806 GiB/s] | [305.50 µs]=[4.8015 GiB/s] |
| `find_fenced_codes`    | [31.822 µs]=[85.979 GiB/s] | [67.912 µs]=[11.391 GiB/s] | [257.60 µs]=[5.6943 GiB/s] |
| `find_blockquotes`     | [31.697 µs]=[86.319 GiB/s] | [41.781 µs]=[18.515 GiB/s] | [124.47 µs]=[11.785 GiB/s] |
| `find_bullet_items`    | [43.673 µs]=[62.648 GiB/s] | [79.023 µs]=[9.7890 GiB/s] | [288.22 µs]=[5.0894 GiB/s] |
| `find_ordered_items`   | [265.99 µs]=[10.286 GiB/s] | [100.86 µs]=[7.6696 GiB/s] | [232.45 µs]=[6.3105 GiB/s] |
 
#### Context-aware - `context()` + `find_context_*`
| Function                       | `plain`                    | `hot`                      | `heavy`                    |
|--------------------------------|----------------------------|----------------------------|----------------------------|
| `context`                      | [44.594 µs]=[61.354 GiB/s] | [429.82 µs]=[1.7997 GiB/s] | [711.78 µs]=[2.0608 GiB/s] |
| `find_context_italics`         | [31.006 µs]=[88.241 GiB/s] | [186.56 µs]=[4.1465 GiB/s] | [524.54 µs]=[2.7964 GiB/s] |
| `find_context_bolds`           | [31.894 µs]=[85.786 GiB/s] | [190.60 µs]=[4.0585 GiB/s] | [591.65 µs]=[2.4793 GiB/s] |
| `find_context_bold_italics`    | [33.678 µs]=[81.241 GiB/s] | [156.30 µs]=[4.9492 GiB/s] | [493.05 µs]=[2.9751 GiB/s] |
| `find_context_headings`        | [31.677 µs]=[86.373 GiB/s] | [118.64 µs]=[6.5204 GiB/s] | [63.283 µs]=[23.179 GiB/s] |
| `find_context_thematic_breaks` | [44.085 µs]=[62.063 GiB/s] | [79.026 µs]=[9.7886 GiB/s] | [348.08 µs]=[4.2141 GiB/s] |
| `find_context_blockquotes`     | [32.417 µs]=[84.400 GiB/s] | [41.413 µs]=[18.679 GiB/s] | [136.05 µs]=[10.782 GiB/s] |
| `find_context_bullet_items`    | [44.068 µs]=[62.086 GiB/s] | [79.096 µs]=[9.7800 GiB/s] | [337.59 µs]=[4.3450 GiB/s] |
| `find_context_ordered_items`   | [266.10 µs]=[10.282 GiB/s] | [100.59 µs]=[7.6903 GiB/s] | [281.77 µs]=[5.2059 GiB/s] |
 
#### Cold context - `find_context_*_cold`
| Function                            | `plain`                    | `hot`                      | `heavy`                    |
|-------------------------------------|----------------------------|----------------------------|----------------------------|
| `find_context_italics_cold`         | [76.245 µs]=[35.885 GiB/s] | [622.62 µs]=[1.2424 GiB/s] | [1.2350 ms]=[1.1877 GiB/s] |
| `find_context_bolds_cold`           | [76.559 µs]=[35.738 GiB/s] | [616.74 µs]=[1.2543 GiB/s] | [1.3116 ms]=[1.1183 GiB/s] |
| `find_context_bold_italics_cold`    | [76.009 µs]=[35.996 GiB/s] | [591.81 µs]=[1.3071 GiB/s] | [1.2100 ms]=[1.2123 GiB/s] |
| `find_context_headings_cold`        | [75.213 µs]=[36.377 GiB/s] | [550.35 µs]=[1.4056 GiB/s] | [779.42 µs]=[1.8820 GiB/s] |
| `find_context_thematic_breaks_cold` | [87.079 µs]=[31.420 GiB/s] | [507.32 µs]=[1.5248 GiB/s] | [1.0611 ms]=[1.3824 GiB/s] |
| `find_context_blockquotes_cold`     | [74.546 µs]=[36.702 GiB/s] | [471.04 µs]=[1.6422 GiB/s] | [850.28 µs]=[1.7251 GiB/s] |
| `find_context_bullet_items_cold`    | [87.950 µs]=[31.109 GiB/s] | [512.25 µs]=[1.5101 GiB/s] | [1.0567 ms]=[1.3882 GiB/s] |
| `find_context_ordered_items_cold`   | [310.17 µs]=[8.8210 GiB/s] | [534.77 µs]=[1.4465 GiB/s] | [1.0021 ms]=[1.4638 GiB/s] |
 


<details>
    <summary>plain full log</summary>

    ```
    ┌─ corpus: plain
    │  size:          2.80 MiB  (2937800 bytes)
    │  elements:         2     (0.0 per KiB)
    │  span mem:      0.00 MiB  (~0.0% of input, 8 B/span lower bound)
    │
    │          headings:         0 √    thematic_breaks:         0 √         paragraphs:         1 √
    │       blockquotes:         0 √       fenced_codes:         0 √       bullet_items:         0 √
    │     ordered_items:         0 √              bolds:         0 √            italics:         0 √
    │      bold_italics:         0 √              codes:         0 √              links:         0 √
    │         autolinks:         0 √        hard_breaks:         0 √              texts:         1 √
    └─
    │  full-vs-standalone counts:
        find_codes         full=       0  standalone=       0
    standalone/plain/find_codes
                            time:   [31.965 µs 32.546 µs 32.924 µs]
                            thrpt:  [83.100 GiB/s 84.068 GiB/s 85.594 GiB/s]
        find_italics       full=       0  standalone=       0
    standalone/plain/find_italics
                            time:   [32.371 µs 32.822 µs 33.295 µs]
                            thrpt:  [82.175 GiB/s 83.359 GiB/s 84.522 GiB/s]
        find_bolds         full=       0  standalone=       0
    standalone/plain/find_bolds
                            time:   [32.109 µs 32.576 µs 32.967 µs]
                            thrpt:  [82.994 GiB/s 83.988 GiB/s 85.212 GiB/s]
        find_bold_italics  full=       0  standalone=       0
    standalone/plain/find_bold_italics
                            time:   [31.860 µs 32.644 µs 33.232 µs]
                            thrpt:  [82.332 GiB/s 83.813 GiB/s 85.876 GiB/s]
        find_autolinks     full=       0  standalone=       0
    standalone/plain/find_autolinks
                            time:   [32.316 µs 32.782 µs 33.212 µs]
                            thrpt:  [82.382 GiB/s 83.461 GiB/s 84.666 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low severe
      2 (10.00%) low mild
        find_links         full=       0  standalone=       0
    standalone/plain/find_links
                            time:   [33.298 µs 33.499 µs 33.620 µs]
                            thrpt:  [81.382 GiB/s 81.675 GiB/s 82.168 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) low mild
        find_headings      full=       0  standalone=       0
    standalone/plain/find_headings
                            time:   [30.052 µs 30.879 µs 31.891 µs]
                            thrpt:  [85.793 GiB/s 88.606 GiB/s 91.045 GiB/s]
        find_thematic_breaks full=       0  standalone=       0
    standalone/plain/find_thematic_breaks
                            time:   [42.843 µs 43.521 µs 44.039 µs]
                            thrpt:  [62.128 GiB/s 62.867 GiB/s 63.861 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) low mild
      1 (5.00%) high mild
        find_fenced_codes  full=       0  standalone=       0
    standalone/plain/find_fenced_codes
                            time:   [31.389 µs 31.822 µs 32.463 µs]
                            thrpt:  [84.281 GiB/s 85.979 GiB/s 87.166 GiB/s]
        find_blockquotes   full=       0  standalone=       0
    standalone/plain/find_blockquotes
                            time:   [31.198 µs 31.697 µs 32.304 µs]
                            thrpt:  [84.697 GiB/s 86.319 GiB/s 87.699 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low mild
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_bullet_items  full=       0  standalone=       0
    standalone/plain/find_bullet_items
                            time:   [43.076 µs 43.673 µs 44.305 µs]
                            thrpt:  [61.755 GiB/s 62.648 GiB/s 63.516 GiB/s]
        find_ordered_items full=       0  standalone=       0
    standalone/plain/find_ordered_items
                            time:   [265.47 µs 265.99 µs 266.63 µs]
                            thrpt:  [10.261 GiB/s 10.286 GiB/s 10.306 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low severe
      1 (5.00%) low mild
      1 (5.00%) high mild
    │  context regions: 0
    standalone/plain/context
                            time:   [44.108 µs 44.594 µs 45.000 µs]
                            thrpt:  [60.801 GiB/s 61.354 GiB/s 62.030 GiB/s]
    │  full-vs-context-aware counts:
        find_context_italics full=       0  context-aware=       0
    standalone/plain/find_context_italics
                            time:   [30.411 µs 31.006 µs 31.706 µs]
                            thrpt:  [86.295 GiB/s 88.241 GiB/s 89.969 GiB/s]
        find_context_bolds full=       0  context-aware=       0
    standalone/plain/find_context_bolds
                            time:   [30.698 µs 31.894 µs 33.254 µs]
                            thrpt:  [82.278 GiB/s 85.786 GiB/s 89.129 GiB/s]
        find_context_bold_italics full=       0  context-aware=       0
    standalone/plain/find_context_bold_italics
                            time:   [33.123 µs 33.678 µs 34.116 µs]
                            thrpt:  [80.199 GiB/s 81.241 GiB/s 82.602 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low severe
      2 (10.00%) low mild
        find_context_headings full=       0  context-aware=       0
    standalone/plain/find_context_headings
                            time:   [30.866 µs 31.677 µs 32.457 µs]
                            thrpt:  [84.297 GiB/s 86.373 GiB/s 88.642 GiB/s]
        find_context_thematic_breaks full=       0  context-aware=       0
    standalone/plain/find_context_thematic_breaks
                            time:   [43.615 µs 44.085 µs 44.563 µs]
                            thrpt:  [61.397 GiB/s 62.063 GiB/s 62.732 GiB/s]
        find_context_blockquotes full=       0  context-aware=       0
    standalone/plain/find_context_blockquotes
                            time:   [31.810 µs 32.417 µs 32.906 µs]
                            thrpt:  [83.146 GiB/s 84.400 GiB/s 86.013 GiB/s]
        find_context_bullet_items full=       0  context-aware=       0
    standalone/plain/find_context_bullet_items
                            time:   [43.541 µs 44.068 µs 44.606 µs]
                            thrpt:  [61.338 GiB/s 62.086 GiB/s 62.838 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
        find_context_ordered_items full=       0  context-aware=       0
    standalone/plain/find_context_ordered_items
                            time:   [265.50 µs 266.10 µs 266.73 µs]
                            thrpt:  [10.258 GiB/s 10.282 GiB/s 10.305 GiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      4 (20.00%) high severe
    standalone/plain/find_context_italics_cold
                            time:   [75.614 µs 76.245 µs 77.087 µs]
                            thrpt:  [35.493 GiB/s 35.885 GiB/s 36.184 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low mild
      2 (10.00%) high mild
    standalone/plain/find_context_bolds_cold
                            time:   [75.532 µs 76.559 µs 77.796 µs]
                            thrpt:  [35.169 GiB/s 35.738 GiB/s 36.223 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
    standalone/plain/find_context_bold_italics_cold
                            time:   [75.009 µs 76.009 µs 76.806 µs]
                            thrpt:  [35.623 GiB/s 35.996 GiB/s 36.476 GiB/s]
    standalone/plain/find_context_headings_cold
                            time:   [73.608 µs 75.213 µs 76.719 µs]
                            thrpt:  [35.663 GiB/s 36.377 GiB/s 37.170 GiB/s]
    standalone/plain/find_context_thematic_breaks_cold
                            time:   [86.274 µs 87.079 µs 88.013 µs]
                            thrpt:  [31.087 GiB/s 31.420 GiB/s 31.714 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) low mild
    standalone/plain/find_context_blockquotes_cold
                            time:   [73.675 µs 74.546 µs 75.207 µs]
                            thrpt:  [36.380 GiB/s 36.702 GiB/s 37.137 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low mild
      1 (5.00%) high mild
    standalone/plain/find_context_bullet_items_cold
                            time:   [87.215 µs 87.950 µs 88.892 µs]
                            thrpt:  [30.779 GiB/s 31.109 GiB/s 31.371 GiB/s]
    standalone/plain/find_context_ordered_items_cold
                            time:   [308.31 µs 310.17 µs 311.99 µs]
                            thrpt:  [8.7697 GiB/s 8.8210 GiB/s 8.8744 GiB/s]
    ```

</details>

<details>
    <summary>hot full log</summary>

    ```
    ┌─ corpus: hot
    │  size:          0.79 MiB  (830600 bytes)
    │  elements:     70000     (86.3 per KiB)
    │  span mem:      0.53 MiB  (~67.4% of input, 8 B/span lower bound)
    │
    │          headings:      5000 √    thematic_breaks:         0 √         paragraphs:      5000 √
    │       blockquotes:         0 √       fenced_codes:         0 √       bullet_items:         0 √
    │     ordered_items:         0 √              bolds:      5000 √            italics:      5000 √
    │      bold_italics:         0 √              codes:      5000 √              links:      5000 √
    │         autolinks:      5000 √        hard_breaks:         0 √              texts:     35000 √
    └─
    │  full-vs-standalone counts:
        find_codes         full=    5000  standalone=    5000
    standalone/hot/find_codes
                            time:   [92.982 µs 93.259 µs 93.556 µs]
                            thrpt:  [8.2684 GiB/s 8.2947 GiB/s 8.3195 GiB/s]
        find_italics       full=    5000  standalone=    5000
    standalone/hot/find_italics
                            time:   [154.44 µs 155.36 µs 156.11 µs]
                            thrpt:  [4.9552 GiB/s 4.9791 GiB/s 5.0087 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      3 (15.00%) low mild
        find_bolds         full=    5000  standalone=    5000
    standalone/hot/find_bolds
                            time:   [156.10 µs 156.86 µs 157.69 µs]
                            thrpt:  [4.9056 GiB/s 4.9315 GiB/s 4.9554 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
        find_bold_italics  full=       0  standalone=       0
    standalone/hot/find_bold_italics
                            time:   [130.90 µs 131.54 µs 132.50 µs]
                            thrpt:  [5.8382 GiB/s 5.8806 GiB/s 5.9097 GiB/s]
        find_autolinks     full=    5000  standalone=    5000
    standalone/hot/find_autolinks
                            time:   [94.371 µs 94.618 µs 94.942 µs]
                            thrpt:  [8.1477 GiB/s 8.1756 GiB/s 8.1969 GiB/s]
        find_links         full=    5000  standalone=    5000
    standalone/hot/find_links
                            time:   [213.97 µs 215.14 µs 216.59 µs]
                            thrpt:  [3.5715 GiB/s 3.5956 GiB/s 3.6153 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_headings      full=    5000  standalone=    5000
    standalone/hot/find_headings
                            time:   [97.493 µs 97.621 µs 97.786 µs]
                            thrpt:  [7.9107 GiB/s 7.9241 GiB/s 7.9345 GiB/s]
        find_thematic_breaks full=       0  standalone=       0
    standalone/hot/find_thematic_breaks
                            time:   [78.888 µs 79.091 µs 79.323 µs]
                            thrpt:  [9.7519 GiB/s 9.7806 GiB/s 9.8058 GiB/s]
        find_fenced_codes  full=       0  standalone=       0
    standalone/hot/find_fenced_codes
                            time:   [67.739 µs 67.912 µs 68.099 µs]
                            thrpt:  [11.359 GiB/s 11.391 GiB/s 11.420 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
        find_blockquotes   full=       0  standalone=       0
    standalone/hot/find_blockquotes
                            time:   [41.661 µs 41.781 µs 41.902 µs]
                            thrpt:  [18.461 GiB/s 18.515 GiB/s 18.568 GiB/s]
        find_bullet_items  full=       0  standalone=       0
    standalone/hot/find_bullet_items
                            time:   [78.649 µs 79.023 µs 79.431 µs]
                            thrpt:  [9.7387 GiB/s 9.7890 GiB/s 9.8355 GiB/s]
        find_ordered_items full=       0  standalone=       0
    standalone/hot/find_ordered_items
                            time:   [100.44 µs 100.86 µs 101.18 µs]
                            thrpt:  [7.6455 GiB/s 7.6696 GiB/s 7.7019 GiB/s]
    │  context regions: 15000
    standalone/hot/context  time:   [428.69 µs 429.82 µs 431.01 µs]
                            thrpt:  [1.7948 GiB/s 1.7997 GiB/s 1.8045 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low mild
      1 (5.00%) high mild
    │  full-vs-context-aware counts:
        find_context_italics full=    5000  context-aware=    5000
    standalone/hot/find_context_italics
                            time:   [185.99 µs 186.56 µs 187.22 µs]
                            thrpt:  [4.1319 GiB/s 4.1465 GiB/s 4.1592 GiB/s]
        find_context_bolds full=    5000  context-aware=    5000
    standalone/hot/find_context_bolds
                            time:   [190.20 µs 190.60 µs 191.00 µs]
                            thrpt:  [4.0500 GiB/s 4.0585 GiB/s 4.0670 GiB/s]
        find_context_bold_italics full=       0  context-aware=       0
    standalone/hot/find_context_bold_italics
                            time:   [155.66 µs 156.30 µs 157.04 µs]
                            thrpt:  [4.9259 GiB/s 4.9492 GiB/s 4.9694 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low mild
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_context_headings full=    5000  context-aware=    5000
    standalone/hot/find_context_headings
                            time:   [118.26 µs 118.64 µs 119.24 µs]
                            thrpt:  [6.4876 GiB/s 6.5204 GiB/s 6.5410 GiB/s]
        find_context_thematic_breaks full=       0  context-aware=       0
    standalone/hot/find_context_thematic_breaks
                            time:   [78.725 µs 79.026 µs 79.332 µs]
                            thrpt:  [9.7509 GiB/s 9.7886 GiB/s 9.8261 GiB/s]
        find_context_blockquotes full=       0  context-aware=       0
    standalone/hot/find_context_blockquotes
                            time:   [41.218 µs 41.413 µs 41.609 µs]
                            thrpt:  [18.591 GiB/s 18.679 GiB/s 18.767 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
        find_context_bullet_items full=       0  context-aware=       0
    standalone/hot/find_context_bullet_items
                            time:   [78.774 µs 79.096 µs 79.385 µs]
                            thrpt:  [9.7443 GiB/s 9.7800 GiB/s 9.8199 GiB/s]
        find_context_ordered_items full=       0  context-aware=       0
    standalone/hot/find_context_ordered_items
                            time:   [100.31 µs 100.59 µs 100.93 µs]
                            thrpt:  [7.6642 GiB/s 7.6903 GiB/s 7.7115 GiB/s]
    standalone/hot/find_context_italics_cold
                            time:   [621.55 µs 622.62 µs 623.66 µs]
                            thrpt:  [1.2403 GiB/s 1.2424 GiB/s 1.2446 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
    standalone/hot/find_context_bolds_cold
                            time:   [615.76 µs 616.74 µs 617.73 µs]
                            thrpt:  [1.2523 GiB/s 1.2543 GiB/s 1.2563 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    standalone/hot/find_context_bold_italics_cold
                            time:   [590.53 µs 591.81 µs 593.10 µs]
                            thrpt:  [1.3043 GiB/s 1.3071 GiB/s 1.3099 GiB/s]
    standalone/hot/find_context_headings_cold
                            time:   [548.47 µs 550.35 µs 552.97 µs]
                            thrpt:  [1.3989 GiB/s 1.4056 GiB/s 1.4104 GiB/s]
    standalone/hot/find_context_thematic_breaks_cold
                            time:   [506.10 µs 507.32 µs 508.56 µs]
                            thrpt:  [1.5211 GiB/s 1.5248 GiB/s 1.5285 GiB/s]
    Found 5 outliers among 20 measurements (25.00%)
      2 (10.00%) low mild
      2 (10.00%) high mild
      1 (5.00%) high severe
    standalone/hot/find_context_blockquotes_cold
                            time:   [468.92 µs 471.04 µs 472.82 µs]
                            thrpt:  [1.6360 GiB/s 1.6422 GiB/s 1.6497 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    standalone/hot/find_context_bullet_items_cold
                            time:   [510.82 µs 512.25 µs 513.27 µs]
                            thrpt:  [1.5071 GiB/s 1.5101 GiB/s 1.5143 GiB/s]
    standalone/hot/find_context_ordered_items_cold
                            time:   [530.52 µs 534.77 µs 538.32 µs]
                            thrpt:  [1.4370 GiB/s 1.4465 GiB/s 1.4581 GiB/s]
    ```

</details>

<details>
    <summary>heavy full log</summary>

    ```
    ┌─ corpus: heavy
    │  size:          1.50 MiB  (1575020 bytes)
    │  elements:    144000     (93.6 per KiB)
    │  span mem:      1.10 MiB  (~73.1% of input, 8 B/span lower bound)
    │
    │          headings:      2000 √    thematic_breaks:      2000 √         paragraphs:      4000 √
    │       blockquotes:      4000 √       fenced_codes:      4000 √       bullet_items:      6000 √
    │     ordered_items:      4000 √              bolds:     12000 √            italics:     12000 √
    │      bold_italics:      6000 √              codes:     10000 √              links:      6000 √
    │         autolinks:      4000 √        hard_breaks:         0 √              texts:     68000 √
    └─
    │  full-vs-standalone counts:
        find_codes         full=   10000  standalone=   10000
    standalone/heavy/find_codes
                            time:   [247.96 µs 248.97 µs 250.15 µs]
                            thrpt:  [5.8638 GiB/s 5.8917 GiB/s 5.9156 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_italics       full=   12000  standalone=   12000
    standalone/heavy/find_italics
                            time:   [447.21 µs 448.06 µs 449.35 µs]
                            thrpt:  [3.2644 GiB/s 3.2738 GiB/s 3.2800 GiB/s]
        find_bolds         full=   12000  standalone=   12000
    standalone/heavy/find_bolds
                            time:   [450.18 µs 452.70 µs 455.45 µs]
                            thrpt:  [3.2207 GiB/s 3.2402 GiB/s 3.2583 GiB/s]
        find_bold_italics  full=    6000  standalone=    6000
    standalone/heavy/find_bold_italics
                            time:   [421.43 µs 422.71 µs 424.40 µs]
                            thrpt:  [3.4563 GiB/s 3.4701 GiB/s 3.4807 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low mild
      2 (10.00%) high mild
        find_autolinks     full=    4000  standalone=    4000
    standalone/heavy/find_autolinks
                            time:   [82.902 µs 83.369 µs 83.721 µs]
                            thrpt:  [17.521 GiB/s 17.595 GiB/s 17.694 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
        find_links         full=    6000  standalone=    6000
    standalone/heavy/find_links
                            time:   [263.37 µs 263.68 µs 264.05 µs]
                            thrpt:  [5.5551 GiB/s 5.5629 GiB/s 5.5696 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low mild
      1 (5.00%) high mild
        find_headings      full=    2000  standalone=    2000
    standalone/heavy/find_headings
                            time:   [47.953 µs 48.143 µs 48.282 µs]
                            thrpt:  [30.381 GiB/s 30.469 GiB/s 30.589 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
        find_thematic_breaks full=    2000  standalone=    2000
    standalone/heavy/find_thematic_breaks
                            time:   [304.96 µs 305.50 µs 306.31 µs]
                            thrpt:  [4.7887 GiB/s 4.8015 GiB/s 4.8099 GiB/s]
        find_fenced_codes  full=    4000  standalone=    4000
    standalone/heavy/find_fenced_codes
                            time:   [257.35 µs 257.60 µs 257.94 µs]
                            thrpt:  [5.6867 GiB/s 5.6943 GiB/s 5.6999 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low severe
        find_blockquotes   full=    4000  standalone=    4000
    standalone/heavy/find_blockquotes
                            time:   [123.87 µs 124.47 µs 125.15 µs]
                            thrpt:  [11.721 GiB/s 11.785 GiB/s 11.842 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
        find_bullet_items  full=    6000  standalone=    6000
    standalone/heavy/find_bullet_items
                            time:   [286.81 µs 288.22 µs 289.79 µs]
                            thrpt:  [5.0617 GiB/s 5.0894 GiB/s 5.1144 GiB/s]
        find_ordered_items full=    4000  standalone=    4000
    standalone/heavy/find_ordered_items
                            time:   [231.75 µs 232.45 µs 233.23 µs]
                            thrpt:  [6.2894 GiB/s 6.3105 GiB/s 6.3293 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    │  context regions: 24000
    standalone/heavy/context
                            time:   [709.29 µs 711.78 µs 714.39 µs]
                            thrpt:  [2.0533 GiB/s 2.0608 GiB/s 2.0681 GiB/s]
    │  full-vs-context-aware counts:
        find_context_italics full=   12000  context-aware=   12000
    standalone/heavy/find_context_italics
                            time:   [521.72 µs 524.54 µs 528.55 µs]
                            thrpt:  [2.7753 GiB/s 2.7964 GiB/s 2.8116 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
        find_context_bolds full=   12000  context-aware=   12000
    standalone/heavy/find_context_bolds
                            time:   [589.45 µs 591.65 µs 593.61 µs]
                            thrpt:  [2.4711 GiB/s 2.4793 GiB/s 2.4885 GiB/s]
        find_context_bold_italics full=    6000  context-aware=    6000
    standalone/heavy/find_context_bold_italics
                            time:   [491.04 µs 493.05 µs 495.65 µs]
                            thrpt:  [2.9594 GiB/s 2.9751 GiB/s 2.9873 GiB/s]
        find_context_headings full=    2000  context-aware=    2000
    standalone/heavy/find_context_headings
                            time:   [63.038 µs 63.283 µs 63.506 µs]
                            thrpt:  [23.098 GiB/s 23.179 GiB/s 23.269 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) low mild
        find_context_thematic_breaks full=    2000  context-aware=    2000
    standalone/heavy/find_context_thematic_breaks
                            time:   [346.75 µs 348.08 µs 349.32 µs]
                            thrpt:  [4.1992 GiB/s 4.2141 GiB/s 4.2303 GiB/s]
        find_context_blockquotes full=    4000  context-aware=    4000
    standalone/heavy/find_context_blockquotes
                            time:   [135.70 µs 136.05 µs 136.37 µs]
                            thrpt:  [10.756 GiB/s 10.782 GiB/s 10.809 GiB/s]
        find_context_bullet_items full=    6000  context-aware=    6000
    standalone/heavy/find_context_bullet_items
                            time:   [336.75 µs 337.59 µs 338.50 µs]
                            thrpt:  [4.3333 GiB/s 4.3450 GiB/s 4.3559 GiB/s]
        find_context_ordered_items full=    4000  context-aware=    4000
    standalone/heavy/find_context_ordered_items
                            time:   [281.31 µs 281.77 µs 282.15 µs]
                            thrpt:  [5.1988 GiB/s 5.2059 GiB/s 5.2144 GiB/s]
    standalone/heavy/find_context_italics_cold
                            time:   [1.2277 ms 1.2350 ms 1.2415 ms]
                            thrpt:  [1.1816 GiB/s 1.1877 GiB/s 1.1948 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    standalone/heavy/find_context_bolds_cold
                            time:   [1.3044 ms 1.3116 ms 1.3188 ms]
                            thrpt:  [1.1123 GiB/s 1.1183 GiB/s 1.1246 GiB/s]
    standalone/heavy/find_context_bold_italics_cold
                            time:   [1.2069 ms 1.2100 ms 1.2132 ms]
                            thrpt:  [1.2091 GiB/s 1.2123 GiB/s 1.2154 GiB/s]
    standalone/heavy/find_context_headings_cold
                            time:   [778.68 µs 779.42 µs 780.10 µs]
                            thrpt:  [1.8803 GiB/s 1.8820 GiB/s 1.8838 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    standalone/heavy/find_context_thematic_breaks_cold
                            time:   [1.0546 ms 1.0611 ms 1.0696 ms]
                            thrpt:  [1.3715 GiB/s 1.3824 GiB/s 1.3909 GiB/s]
    standalone/heavy/find_context_blockquotes_cold
                            time:   [845.24 µs 850.28 µs 855.23 µs]
                            thrpt:  [1.7151 GiB/s 1.7251 GiB/s 1.7354 GiB/s]
    standalone/heavy/find_context_bullet_items_cold
                            time:   [1.0492 ms 1.0567 ms 1.0622 ms]
                            thrpt:  [1.3809 GiB/s 1.3882 GiB/s 1.3980 GiB/s]
    standalone/heavy/find_context_ordered_items_cold
                            time:   [998.14 µs 1.0021 ms 1.0064 ms]
                            thrpt:  [1.4575 GiB/s 1.4638 GiB/s 1.4696 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    ```

</details>

**big (exceeds L3 cache):**

#### Standalone - `find_*`
| Function               | `plain`                    | `hot`                      | `heavy`                    |
|------------------------|----------------------------|----------------------------|----------------------------|
| `find_codes`           | [13.801 ms]=[19.825 GiB/s] | [10.244 ms]=[7.5515 GiB/s] | [26.026 ms]=[5.6360 GiB/s] |
| `find_italics`         | [13.968 ms]=[19.588 GiB/s] | [16.234 ms]=[4.7651 GiB/s] | [45.804 ms]=[3.2024 GiB/s] |
| `find_bolds`           | [13.924 ms]=[19.649 GiB/s] | [16.285 ms]=[4.7500 GiB/s] | [45.803 ms]=[3.2025 GiB/s] |
| `find_bold_italics`    | [13.887 ms]=[19.703 GiB/s] | [14.018 ms]=[5.5183 GiB/s] | [42.798 ms]=[3.4274 GiB/s] |
| `find_autolinks`       | [13.769 ms]=[19.871 GiB/s] | [10.530 ms]=[7.3460 GiB/s] | [12.670 ms]=[11.578 GiB/s] |
| `find_links`           | [13.655 ms]=[20.038 GiB/s] | [21.898 ms]=[3.5326 GiB/s] | [27.946 ms]=[5.2489 GiB/s] |
| `find_headings`        | [13.951 ms]=[19.611 GiB/s] | [10.697 ms]=[7.2317 GiB/s] | [10.331 ms]=[14.199 GiB/s] |
| `find_thematic_breaks` | [16.209 ms]=[16.880 GiB/s] | [8.9587 ms]=[8.6347 GiB/s] | [31.726 ms]=[4.6235 GiB/s] |
| `find_fenced_codes`    | [14.086 ms]=[19.424 GiB/s] | [8.1030 ms]=[9.5466 GiB/s] | [27.115 ms]=[5.4098 GiB/s] |
| `find_blockquotes`     | [14.076 ms]=[19.437 GiB/s] | [6.3757 ms]=[12.133 GiB/s] | [15.577 ms]=[9.4167 GiB/s] |
| `find_bullet_items`    | [16.270 ms]=[16.817 GiB/s] | [8.9393 ms]=[8.6534 GiB/s] | [29.950 ms]=[4.8976 GiB/s] |
| `find_ordered_items`   | [30.888 ms]=[8.8579 GiB/s] | [10.887 ms]=[7.1053 GiB/s] | [24.813 ms]=[5.9116 GiB/s] |

#### Context-aware - `context()` + `find_context_*`
| Function                       | `plain`                    | `hot`                      | `heavy`                    |
|--------------------------------|----------------------------|----------------------------|----------------------------|
| `context`                      | [15.984 ms]=[17.118 GiB/s] | [43.526 ms]=[1.7772 GiB/s] | [82.238 ms]=[1.7837 GiB/s] |
| `find_context_italics`         | [13.766 ms]=[19.876 GiB/s] | [19.322 ms]=[4.0035 GiB/s] | [52.944 ms]=[2.7705 GiB/s] |
| `find_context_bolds`           | [13.977 ms]=[19.576 GiB/s] | [19.414 ms]=[3.9845 GiB/s] | [58.991 ms]=[2.4866 GiB/s] |
| `find_context_bold_italics`    | [14.244 ms]=[19.208 GiB/s] | [16.232 ms]=[4.7657 GiB/s] | [49.093 ms]=[2.9879 GiB/s] |
| `find_context_headings`        | [14.255 ms]=[19.194 GiB/s] | [12.729 ms]=[6.0771 GiB/s] | [11.575 ms]=[12.673 GiB/s] |
| `find_context_thematic_breaks` | [16.248 ms]=[16.839 GiB/s] | [8.9999 ms]=[8.5952 GiB/s] | [35.987 ms]=[4.0760 GiB/s] |
| `find_context_blockquotes`     | [14.183 ms]=[19.291 GiB/s] | [6.3694 ms]=[12.145 GiB/s] | [16.194 ms]=[9.0578 GiB/s] |
| `find_context_bullet_items`    | [16.126 ms]=[16.966 GiB/s] | [8.9724 ms]=[8.6215 GiB/s] | [34.648 ms]=[4.2336 GiB/s] |
| `find_context_ordered_items`   | [30.690 ms]=[8.9152 GiB/s] | [10.860 ms]=[7.1228 GiB/s] | [29.263 ms]=[5.0126 GiB/s] |

#### Cold context - `find_context_*_cold`
| Function                            | `plain`                    | `hot`                      | `heavy`                    |
|-------------------------------------|----------------------------|----------------------------|----------------------------|
| `find_context_italics_cold`         | [29.885 ms]=[9.1552 GiB/s] | [62.666 ms]=[1.2344 GiB/s] | [134.24 ms]=[1.0927 GiB/s] |
| `find_context_bolds_cold`           | [30.261 ms]=[9.0416 GiB/s] | [63.165 ms]=[1.2247 GiB/s] | [140.79 ms]=[1.0419 GiB/s] |
| `find_context_bold_italics_cold`    | [30.357 ms]=[9.0128 GiB/s] | [59.733 ms]=[1.2950 GiB/s] | [131.86 ms]=[1.1125 GiB/s] |
| `find_context_headings_cold`        | [30.680 ms]=[8.9181 GiB/s] | [56.265 ms]=[1.3748 GiB/s] | [94.238 ms]=[1.5565 GiB/s] |
| `find_context_thematic_breaks_cold` | [32.724 ms]=[8.3609 GiB/s] | [52.146 ms]=[1.4834 GiB/s] | [118.59 ms]=[1.2369 GiB/s] |
| `find_context_blockquotes_cold`     | [30.341 ms]=[9.0175 GiB/s] | [49.449 ms]=[1.5644 GiB/s] | [98.389 ms]=[1.4909 GiB/s] |
| `find_context_bullet_items_cold`    | [31.657 ms]=[8.6427 GiB/s] | [52.063 ms]=[1.4858 GiB/s] | [116.40 ms]=[1.2602 GiB/s] |
| `find_context_ordered_items_cold`   | [48.285 ms]=[5.6665 GiB/s] | [54.543 ms]=[1.4183 GiB/s] | [111.63 ms]=[1.3140 GiB/s] |

<details>
    <summary>plain full log</summary>

    ```
    ┌─ corpus: plain
    │  size:        280.17 MiB  (293780000 bytes)
    │  elements:         2     (0.0 per KiB)
    │  span mem:      0.00 MiB  (~0.0% of input, 8 B/span lower bound)
    │
    │          headings:         0 √    thematic_breaks:         0 √         paragraphs:         1 √
    │       blockquotes:         0 √       fenced_codes:         0 √       bullet_items:         0 √
    │     ordered_items:         0 √              bolds:         0 √            italics:         0 √
    │      bold_italics:         0 √              codes:         0 √              links:         0 √
    │         autolinks:         0 √        hard_breaks:         0 √              texts:         1 √
    └─
    │  full-vs-standalone counts:
        find_codes         full=       0  standalone=       0
    standalone/plain/find_codes
                            time:   [13.736 ms 13.801 ms 13.886 ms]
                            thrpt:  [19.704 GiB/s 19.825 GiB/s 19.919 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_italics       full=       0  standalone=       0
    standalone/plain/find_italics
                            time:   [13.867 ms 13.968 ms 14.077 ms]
                            thrpt:  [19.436 GiB/s 19.588 GiB/s 19.731 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_bolds         full=       0  standalone=       0
    standalone/plain/find_bolds
                            time:   [13.839 ms 13.924 ms 14.011 ms]
                            thrpt:  [19.528 GiB/s 19.649 GiB/s 19.770 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_bold_italics  full=       0  standalone=       0
    standalone/plain/find_bold_italics
                            time:   [13.810 ms 13.887 ms 13.965 ms]
                            thrpt:  [19.592 GiB/s 19.703 GiB/s 19.812 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_autolinks     full=       0  standalone=       0
    standalone/plain/find_autolinks
                            time:   [13.672 ms 13.769 ms 13.890 ms]
                            thrpt:  [19.698 GiB/s 19.871 GiB/s 20.012 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_links         full=       0  standalone=       0
    standalone/plain/find_links
                            time:   [13.558 ms 13.655 ms 13.761 ms]
                            thrpt:  [19.883 GiB/s 20.038 GiB/s 20.180 GiB/s]
        find_headings      full=       0  standalone=       0
    standalone/plain/find_headings
                            time:   [13.867 ms 13.951 ms 14.055 ms]
                            thrpt:  [19.466 GiB/s 19.611 GiB/s 19.731 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
        find_thematic_breaks full=       0  standalone=       0
    standalone/plain/find_thematic_breaks
                            time:   [16.102 ms 16.209 ms 16.321 ms]
                            thrpt:  [16.764 GiB/s 16.880 GiB/s 16.992 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
        find_fenced_codes  full=       0  standalone=       0
    standalone/plain/find_fenced_codes
                            time:   [13.980 ms 14.086 ms 14.201 ms]
                            thrpt:  [19.267 GiB/s 19.424 GiB/s 19.572 GiB/s]
        find_blockquotes   full=       0  standalone=       0
    standalone/plain/find_blockquotes
                            time:   [13.981 ms 14.076 ms 14.180 ms]
                            thrpt:  [19.295 GiB/s 19.437 GiB/s 19.569 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
        find_bullet_items  full=       0  standalone=       0
    standalone/plain/find_bullet_items
                            time:   [16.154 ms 16.270 ms 16.383 ms]
                            thrpt:  [16.700 GiB/s 16.817 GiB/s 16.938 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_ordered_items full=       0  standalone=       0
    standalone/plain/find_ordered_items
                            time:   [30.793 ms 30.888 ms 30.990 ms]
                            thrpt:  [8.8288 GiB/s 8.8579 GiB/s 8.8853 GiB/s]
    │  context regions: 0
    standalone/plain/context
                            time:   [15.877 ms 15.984 ms 16.129 ms]
                            thrpt:  [16.964 GiB/s 17.118 GiB/s 17.232 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    │  full-vs-context-aware counts:
        find_context_italics full=       0  context-aware=       0
    standalone/plain/find_context_italics
                            time:   [13.647 ms 13.766 ms 13.879 ms]
                            thrpt:  [19.714 GiB/s 19.876 GiB/s 20.049 GiB/s]
        find_context_bolds full=       0  context-aware=       0
    standalone/plain/find_context_bolds
                            time:   [13.893 ms 13.977 ms 14.054 ms]
                            thrpt:  [19.468 GiB/s 19.576 GiB/s 19.693 GiB/s]
        find_context_bold_italics full=       0  context-aware=       0
    standalone/plain/find_context_bold_italics
                            time:   [14.119 ms 14.244 ms 14.384 ms]
                            thrpt:  [19.022 GiB/s 19.208 GiB/s 19.378 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_context_headings full=       0  context-aware=       0
    standalone/plain/find_context_headings
                            time:   [14.140 ms 14.255 ms 14.362 ms]
                            thrpt:  [19.050 GiB/s 19.194 GiB/s 19.350 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_context_thematic_breaks full=       0  context-aware=       0
    standalone/plain/find_context_thematic_breaks
                            time:   [16.101 ms 16.248 ms 16.402 ms]
                            thrpt:  [16.681 GiB/s 16.839 GiB/s 16.993 GiB/s]
        find_context_blockquotes full=       0  context-aware=       0
    standalone/plain/find_context_blockquotes
                            time:   [14.062 ms 14.183 ms 14.316 ms]
                            thrpt:  [19.111 GiB/s 19.291 GiB/s 19.457 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_context_bullet_items full=       0  context-aware=       0
    standalone/plain/find_context_bullet_items
                            time:   [15.998 ms 16.126 ms 16.253 ms]
                            thrpt:  [16.834 GiB/s 16.966 GiB/s 17.102 GiB/s]
        find_context_ordered_items full=       0  context-aware=       0
    standalone/plain/find_context_ordered_items
                            time:   [30.599 ms 30.690 ms 30.809 ms]
                            thrpt:  [8.8806 GiB/s 8.9152 GiB/s 8.9416 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) high mild
      2 (10.00%) high severe
    standalone/plain/find_context_italics_cold
                            time:   [29.571 ms 29.885 ms 30.194 ms]
                            thrpt:  [9.0617 GiB/s 9.1552 GiB/s 9.2524 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    standalone/plain/find_context_bolds_cold
                            time:   [30.102 ms 30.261 ms 30.397 ms]
                            thrpt:  [9.0009 GiB/s 9.0416 GiB/s 9.0891 GiB/s]
    standalone/plain/find_context_bold_italics_cold
                            time:   [30.092 ms 30.357 ms 30.630 ms]
                            thrpt:  [8.9325 GiB/s 9.0128 GiB/s 9.0922 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    standalone/plain/find_context_headings_cold
                            time:   [30.389 ms 30.680 ms 31.011 ms]
                            thrpt:  [8.8228 GiB/s 8.9181 GiB/s 9.0035 GiB/s]
    standalone/plain/find_context_thematic_breaks_cold
                            time:   [32.583 ms 32.724 ms 32.885 ms]
                            thrpt:  [8.3201 GiB/s 8.3609 GiB/s 8.3971 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
    standalone/plain/find_context_blockquotes_cold
                            time:   [30.190 ms 30.341 ms 30.539 ms]
                            thrpt:  [8.9591 GiB/s 9.0175 GiB/s 9.0627 GiB/s]
    standalone/plain/find_context_bullet_items_cold
                            time:   [31.466 ms 31.657 ms 31.883 ms]
                            thrpt:  [8.5815 GiB/s 8.6427 GiB/s 8.6951 GiB/s]
    
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 10.1s, enable flat sampling, or reduce sample count to 10.
    standalone/plain/find_context_ordered_items_cold
                            time:   [48.124 ms 48.285 ms 48.459 ms]
                            thrpt:  [5.6460 GiB/s 5.6665 GiB/s 5.6854 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low mild
      1 (5.00%) high mild
    ```

</details>

<details>
    <summary>hot full log</summary>

    ```
    ┌─ corpus: hot
    │  size:         79.21 MiB  (83060000 bytes)
    │  elements:   7000000     (86.3 per KiB)
    │  span mem:     53.41 MiB  (~67.4% of input, 8 B/span lower bound)
    │
    │          headings:    500000 √    thematic_breaks:         0 √         paragraphs:    500000 √
    │       blockquotes:         0 √       fenced_codes:         0 √       bullet_items:         0 √
    │     ordered_items:         0 √              bolds:    500000 √            italics:    500000 √
    │      bold_italics:         0 √              codes:    500000 √              links:    500000 √
    │         autolinks:    500000 √        hard_breaks:         0 √              texts:   3500000 √
    └─
    │  full-vs-standalone counts:
        find_codes         full=  500000  standalone=  500000
    standalone/hot/find_codes
                            time:   [10.216 ms 10.244 ms 10.278 ms]
                            thrpt:  [7.5263 GiB/s 7.5515 GiB/s 7.5723 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_italics       full=  500000  standalone=  500000
    standalone/hot/find_italics
                            time:   [16.199 ms 16.234 ms 16.280 ms]
                            thrpt:  [4.7516 GiB/s 4.7651 GiB/s 4.7753 GiB/s]
        find_bolds         full=  500000  standalone=  500000
    standalone/hot/find_bolds
                            time:   [16.083 ms 16.285 ms 16.436 ms]
                            thrpt:  [4.7065 GiB/s 4.7500 GiB/s 4.8097 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) low mild
        find_bold_italics  full=       0  standalone=       0
    standalone/hot/find_bold_italics
                            time:   [13.862 ms 14.018 ms 14.167 ms]
                            thrpt:  [5.4602 GiB/s 5.5183 GiB/s 5.5805 GiB/s]
        find_autolinks     full=  500000  standalone=  500000
    standalone/hot/find_autolinks
                            time:   [10.476 ms 10.530 ms 10.586 ms]
                            thrpt:  [7.3076 GiB/s 7.3460 GiB/s 7.3837 GiB/s]
        find_links         full=  500000  standalone=  500000
    standalone/hot/find_links
                            time:   [21.820 ms 21.898 ms 21.981 ms]
                            thrpt:  [3.5192 GiB/s 3.5326 GiB/s 3.5452 GiB/s]
        find_headings      full=  500000  standalone=  500000
    standalone/hot/find_headings
                            time:   [10.656 ms 10.697 ms 10.742 ms]
                            thrpt:  [7.2015 GiB/s 7.2317 GiB/s 7.2596 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_thematic_breaks full=       0  standalone=       0
    standalone/hot/find_thematic_breaks
                            time:   [8.9382 ms 8.9587 ms 8.9811 ms]
                            thrpt:  [8.6132 GiB/s 8.6347 GiB/s 8.6545 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_fenced_codes  full=       0  standalone=       0
    standalone/hot/find_fenced_codes
                            time:   [8.0819 ms 8.1030 ms 8.1252 ms]
                            thrpt:  [9.5205 GiB/s 9.5466 GiB/s 9.5714 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_blockquotes   full=       0  standalone=       0
    standalone/hot/find_blockquotes
                            time:   [6.3555 ms 6.3757 ms 6.3947 ms]
                            thrpt:  [12.097 GiB/s 12.133 GiB/s 12.171 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_bullet_items  full=       0  standalone=       0
    standalone/hot/find_bullet_items
                            time:   [8.9225 ms 8.9393 ms 8.9563 ms]
                            thrpt:  [8.6370 GiB/s 8.6534 GiB/s 8.6697 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_ordered_items full=       0  standalone=       0
    standalone/hot/find_ordered_items
                            time:   [10.846 ms 10.887 ms 10.933 ms]
                            thrpt:  [7.0751 GiB/s 7.1053 GiB/s 7.1320 GiB/s]
    │  context regions: 1500000
    standalone/hot/context  time:   [43.437 ms 43.526 ms 43.605 ms]
                            thrpt:  [1.7740 GiB/s 1.7772 GiB/s 1.7809 GiB/s]
    │  full-vs-context-aware counts:
        find_context_italics full=  500000  context-aware=  500000
    standalone/hot/find_context_italics
                            time:   [19.227 ms 19.322 ms 19.443 ms]
                            thrpt:  [3.9785 GiB/s 4.0035 GiB/s 4.0232 GiB/s]
        find_context_bolds full=  500000  context-aware=  500000
    standalone/hot/find_context_bolds
                            time:   [19.350 ms 19.414 ms 19.494 ms]
                            thrpt:  [3.9682 GiB/s 3.9845 GiB/s 3.9978 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low mild
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_context_bold_italics full=       0  context-aware=       0
    standalone/hot/find_context_bold_italics
                            time:   [16.186 ms 16.232 ms 16.287 ms]
                            thrpt:  [4.7495 GiB/s 4.7657 GiB/s 4.7792 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_context_headings full=  500000  context-aware=  500000
    standalone/hot/find_context_headings
                            time:   [12.694 ms 12.729 ms 12.762 ms]
                            thrpt:  [6.0616 GiB/s 6.0771 GiB/s 6.0937 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
        find_context_thematic_breaks full=       0  context-aware=       0
    standalone/hot/find_context_thematic_breaks
                            time:   [8.9779 ms 8.9999 ms 9.0182 ms]
                            thrpt:  [8.5777 GiB/s 8.5952 GiB/s 8.6162 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_context_blockquotes full=       0  context-aware=       0
    standalone/hot/find_context_blockquotes
                            time:   [6.3422 ms 6.3694 ms 6.3981 ms]
                            thrpt:  [12.090 GiB/s 12.145 GiB/s 12.197 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_context_bullet_items full=       0  context-aware=       0
    standalone/hot/find_context_bullet_items
                            time:   [8.9478 ms 8.9724 ms 8.9930 ms]
                            thrpt:  [8.6017 GiB/s 8.6215 GiB/s 8.6452 GiB/s]
        find_context_ordered_items full=       0  context-aware=       0
    standalone/hot/find_context_ordered_items
                            time:   [10.833 ms 10.860 ms 10.897 ms]
                            thrpt:  [7.0988 GiB/s 7.1228 GiB/s 7.1410 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 13.2s, enable flat sampling, or reduce sample count to 10.
    standalone/hot/find_context_italics_cold
                            time:   [62.355 ms 62.666 ms 63.065 ms]
                            thrpt:  [1.2266 GiB/s 1.2344 GiB/s 1.2406 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 13.3s, enable flat sampling, or reduce sample count to 10.
    standalone/hot/find_context_bolds_cold
                            time:   [62.907 ms 63.165 ms 63.475 ms]
                            thrpt:  [1.2187 GiB/s 1.2247 GiB/s 1.2297 GiB/s]
    
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 12.7s, enable flat sampling, or reduce sample count to 10.
    standalone/hot/find_context_bold_italics_cold
                            time:   [59.432 ms 59.733 ms 59.996 ms]
                            thrpt:  [1.2893 GiB/s 1.2950 GiB/s 1.3016 GiB/s]
    
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 11.9s, enable flat sampling, or reduce sample count to 10.
    standalone/hot/find_context_headings_cold
                            time:   [56.116 ms 56.265 ms 56.426 ms]
                            thrpt:  [1.3709 GiB/s 1.3748 GiB/s 1.3785 GiB/s]
    
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 11.1s, enable flat sampling, or reduce sample count to 10.
    standalone/hot/find_context_thematic_breaks_cold
                            time:   [51.805 ms 52.146 ms 52.453 ms]
                            thrpt:  [1.4748 GiB/s 1.4834 GiB/s 1.4932 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 10.4s, enable flat sampling, or reduce sample count to 10.
    standalone/hot/find_context_blockquotes_cold
                            time:   [49.161 ms 49.449 ms 49.686 ms]
                            thrpt:  [1.5569 GiB/s 1.5644 GiB/s 1.5735 GiB/s]
    
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 10.9s, enable flat sampling, or reduce sample count to 10.
    standalone/hot/find_context_bullet_items_cold
                            time:   [51.783 ms 52.063 ms 52.409 ms]
                            thrpt:  [1.4760 GiB/s 1.4858 GiB/s 1.4938 GiB/s]
    
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 11.4s, enable flat sampling, or reduce sample count to 10.
    standalone/hot/find_context_ordered_items_cold
                            time:   [54.408 ms 54.543 ms 54.693 ms]
                            thrpt:  [1.4144 GiB/s 1.4183 GiB/s 1.4218 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    ```

</details>

<details>
    <summary>heavy full log</summary>

    ```
    ┌─ corpus: heavy
    │  size:        150.21 MiB  (157502000 bytes)
    │  elements:  14400000     (93.6 per KiB)
    │  span mem:    109.86 MiB  (~73.1% of input, 8 B/span lower bound)
    │
    │          headings:    200000 √    thematic_breaks:    200000 √         paragraphs:    400000 √
    │       blockquotes:    400000 √       fenced_codes:    400000 √       bullet_items:    600000 √
    │     ordered_items:    400000 √              bolds:   1200000 √            italics:   1200000 √
    │      bold_italics:    600000 √              codes:   1000000 √              links:    600000 √
    │         autolinks:    400000 √        hard_breaks:         0 √              texts:   6800000 √
    └─
    │  full-vs-standalone counts:
        find_codes         full= 1000000  standalone= 1000000
    standalone/heavy/find_codes
                            time:   [25.949 ms 26.026 ms 26.098 ms]
                            thrpt:  [5.6206 GiB/s 5.6360 GiB/s 5.6528 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
        find_italics       full= 1200000  standalone= 1200000
    standalone/heavy/find_italics
                            time:   [45.514 ms 45.804 ms 46.160 ms]
                            thrpt:  [3.1778 GiB/s 3.2024 GiB/s 3.2229 GiB/s]
        find_bolds         full= 1200000  standalone= 1200000
    standalone/heavy/find_bolds
                            time:   [45.581 ms 45.803 ms 46.008 ms]
                            thrpt:  [3.1882 GiB/s 3.2025 GiB/s 3.2181 GiB/s]
        find_bold_italics  full=  600000  standalone=  600000
    standalone/heavy/find_bold_italics
                            time:   [42.650 ms 42.798 ms 42.979 ms]
                            thrpt:  [3.4130 GiB/s 3.4274 GiB/s 3.4393 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_autolinks     full=  400000  standalone=  400000
    standalone/heavy/find_autolinks
                            time:   [12.617 ms 12.670 ms 12.720 ms]
                            thrpt:  [11.532 GiB/s 11.578 GiB/s 11.626 GiB/s]
        find_links         full=  600000  standalone=  600000
    standalone/heavy/find_links
                            time:   [27.848 ms 27.946 ms 28.060 ms]
                            thrpt:  [5.2276 GiB/s 5.2489 GiB/s 5.2674 GiB/s]
        find_headings      full=  200000  standalone=  200000
    standalone/heavy/find_headings
                            time:   [10.244 ms 10.331 ms 10.421 ms]
                            thrpt:  [14.076 GiB/s 14.199 GiB/s 14.320 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_thematic_breaks full=  200000  standalone=  200000
    standalone/heavy/find_thematic_breaks
                            time:   [31.606 ms 31.726 ms 31.868 ms]
                            thrpt:  [4.6028 GiB/s 4.6235 GiB/s 4.6411 GiB/s]
        find_fenced_codes  full=  400000  standalone=  400000
    standalone/heavy/find_fenced_codes
                            time:   [27.051 ms 27.115 ms 27.185 ms]
                            thrpt:  [5.3959 GiB/s 5.4098 GiB/s 5.4226 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_blockquotes   full=  400000  standalone=  400000
    standalone/heavy/find_blockquotes
                            time:   [15.549 ms 15.577 ms 15.606 ms]
                            thrpt:  [9.3990 GiB/s 9.4167 GiB/s 9.4335 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_bullet_items  full=  600000  standalone=  600000
    standalone/heavy/find_bullet_items
                            time:   [29.854 ms 29.950 ms 30.061 ms]
                            thrpt:  [4.8796 GiB/s 4.8976 GiB/s 4.9134 GiB/s]
        find_ordered_items full=  400000  standalone=  400000
    standalone/heavy/find_ordered_items
                            time:   [24.699 ms 24.813 ms 24.931 ms]
                            thrpt:  [5.8837 GiB/s 5.9116 GiB/s 5.9389 GiB/s]
    │  context regions: 2400000
    
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 17.1s, enable flat sampling, or reduce sample count to 10.
    standalone/heavy/context
                            time:   [81.973 ms 82.238 ms 82.519 ms]
                            thrpt:  [1.7776 GiB/s 1.7837 GiB/s 1.7894 GiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      3 (15.00%) low mild
      1 (5.00%) high mild
    │  full-vs-context-aware counts:
        find_context_italics full= 1200000  context-aware= 1200000
    
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 11.2s, enable flat sampling, or reduce sample count to 10.
    standalone/heavy/find_context_italics
                            time:   [52.766 ms 52.944 ms 53.118 ms]
                            thrpt:  [2.7615 GiB/s 2.7705 GiB/s 2.7799 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_context_bolds full= 1200000  context-aware= 1200000
    
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 12.4s, enable flat sampling, or reduce sample count to 10.
    standalone/heavy/find_context_bolds
                            time:   [58.771 ms 58.991 ms 59.296 ms]
                            thrpt:  [2.4738 GiB/s 2.4866 GiB/s 2.4959 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_context_bold_italics full=  600000  context-aware=  600000
    
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 10.4s, enable flat sampling, or reduce sample count to 10.
    standalone/heavy/find_context_bold_italics
                            time:   [48.909 ms 49.093 ms 49.330 ms]
                            thrpt:  [2.9735 GiB/s 2.9879 GiB/s 2.9992 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) high mild
      1 (5.00%) high severe
        find_context_headings full=  200000  context-aware=  200000
    standalone/heavy/find_context_headings
                            time:   [11.496 ms 11.575 ms 11.637 ms]
                            thrpt:  [12.605 GiB/s 12.673 GiB/s 12.760 GiB/s]
        find_context_thematic_breaks full=  200000  context-aware=  200000
    standalone/heavy/find_context_thematic_breaks
                            time:   [35.882 ms 35.987 ms 36.088 ms]
                            thrpt:  [4.0647 GiB/s 4.0760 GiB/s 4.0880 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low mild
      2 (10.00%) high mild
        find_context_blockquotes full=  400000  context-aware=  400000
    standalone/heavy/find_context_blockquotes
                            time:   [16.144 ms 16.194 ms 16.247 ms]
                            thrpt:  [9.0285 GiB/s 9.0578 GiB/s 9.0859 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_context_bullet_items full=  600000  context-aware=  600000
    standalone/heavy/find_context_bullet_items
                            time:   [34.561 ms 34.648 ms 34.726 ms]
                            thrpt:  [4.2241 GiB/s 4.2336 GiB/s 4.2442 GiB/s]
        find_context_ordered_items full=  400000  context-aware=  400000
    standalone/heavy/find_context_ordered_items
                            time:   [29.189 ms 29.263 ms 29.372 ms]
                            thrpt:  [4.9941 GiB/s 5.0126 GiB/s 5.0254 GiB/s]
    standalone/heavy/find_context_italics_cold
                            time:   [133.74 ms 134.24 ms 134.78 ms]
                            thrpt:  [1.0884 GiB/s 1.0927 GiB/s 1.0968 GiB/s]
    standalone/heavy/find_context_bolds_cold
                            time:   [140.25 ms 140.79 ms 141.36 ms]
                            thrpt:  [1.0377 GiB/s 1.0419 GiB/s 1.0458 GiB/s]
    standalone/heavy/find_context_bold_italics_cold
                            time:   [131.47 ms 131.86 ms 132.26 ms]
                            thrpt:  [1.1090 GiB/s 1.1125 GiB/s 1.1157 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 19.7s, enable flat sampling, or reduce sample count to 10.
    standalone/heavy/find_context_headings_cold
                            time:   [93.897 ms 94.238 ms 94.552 ms]
                            thrpt:  [1.5514 GiB/s 1.5565 GiB/s 1.5622 GiB/s]
    standalone/heavy/find_context_thematic_breaks_cold
                            time:   [118.28 ms 118.59 ms 118.94 ms]
                            thrpt:  [1.2332 GiB/s 1.2369 GiB/s 1.2402 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low severe
      2 (10.00%) high severe
    standalone/heavy/find_context_blockquotes_cold
                            time:   [98.121 ms 98.389 ms 98.671 ms]
                            thrpt:  [1.4866 GiB/s 1.4909 GiB/s 1.4949 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    standalone/heavy/find_context_bullet_items_cold
                            time:   [115.96 ms 116.40 ms 116.82 ms]
                            thrpt:  [1.2557 GiB/s 1.2602 GiB/s 1.2649 GiB/s]
    standalone/heavy/find_context_ordered_items_cold
                            time:   [110.88 ms 111.63 ms 112.47 ms]
                            thrpt:  [1.3043 GiB/s 1.3140 GiB/s 1.3229 GiB/s]
    ```

</details>

### nightly - `RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_standalone --features avx2`

**small (fits in cache):**

#### Standalone - `find_*`
| Function               | `plain`                    | `hot`                      | `heavy`                    |
|------------------------|----------------------------|----------------------------|----------------------------|
| `find_codes`           | [32.412 µs]=[84.415 GiB/s] | [73.470 µs]=[10.529 GiB/s] | [202.18 µs]=[7.2551 GiB/s] |
| `find_italics`         | [31.427 µs]=[87.059 GiB/s] | [130.69 µs]=[5.9188 GiB/s] | [364.12 µs]=[4.0285 GiB/s] |
| `find_bolds`           | [32.581 µs]=[83.977 GiB/s] | [134.54 µs]=[5.7497 GiB/s] | [371.62 µs]=[3.9472 GiB/s] |
| `find_bold_italics`    | [33.416 µs]=[81.877 GiB/s] | [119.28 µs]=[6.4854 GiB/s] | [355.40 µs]=[4.1274 GiB/s] |
| `find_autolinks`       | [32.344 µs]=[84.591 GiB/s] | [81.891 µs]=[9.4462 GiB/s] | [75.411 µs]=[19.452 GiB/s] |
| `find_links`           | [31.496 µs]=[86.869 GiB/s] | [174.97 µs]=[4.4210 GiB/s] | [215.31 µs]=[6.8127 GiB/s] |
| `find_headings`        | [33.000 µs]=[82.911 GiB/s] | [76.109 µs]=[10.164 GiB/s] | [42.708 µs]=[34.346 GiB/s] |
| `find_thematic_breaks` | [44.775 µs]=[61.106 GiB/s] | [72.713 µs]=[10.639 GiB/s] | [280.86 µs]=[5.2227 GiB/s] |
| `find_fenced_codes`    | [32.867 µs]=[83.246 GiB/s] | [62.290 µs]=[12.419 GiB/s] | [227.76 µs]=[6.4404 GiB/s] |
| `find_blockquotes`     | [31.855 µs]=[85.890 GiB/s] | [37.787 µs]=[20.472 GiB/s] | [105.12 µs]=[13.954 GiB/s] |
| `find_bullet_items`    | [44.467 µs]=[61.529 GiB/s] | [69.481 µs]=[11.133 GiB/s] | [232.74 µs]=[6.3024 GiB/s] |
| `find_ordered_items`   | [254.53 µs]=[10.749 GiB/s] | [95.971 µs]=[8.0603 GiB/s] | [204.42 µs]=[7.1758 GiB/s] |
 
#### Context-aware - `context()` + `find_context_*`
| Function                       | `plain`                    | `hot`                      | `heavy`                    |
|--------------------------------|----------------------------|----------------------------|----------------------------|
| `context`                      | [44.897 µs]=[60.941 GiB/s] | [340.12 µs]=[2.2744 GiB/s] | [550.20 µs]=[2.6660 GiB/s] |
| `find_context_italics`         | [33.242 µs]=[82.307 GiB/s] | [151.02 µs]=[5.1222 GiB/s] | [418.05 µs]=[3.5088 GiB/s] |
| `find_context_bolds`           | [32.756 µs]=[83.528 GiB/s] | [154.76 µs]=[4.9984 GiB/s] | [487.54 µs]=[3.0087 GiB/s] |
| `find_context_bold_italics`    | [32.773 µs]=[83.485 GiB/s] | [129.38 µs]=[5.9792 GiB/s] | [395.22 µs]=[3.7115 GiB/s] |
| `find_context_headings`        | [32.146 µs]=[85.112 GiB/s] | [102.70 µs]=[7.5320 GiB/s] | [64.596 µs]=[22.708 GiB/s] |
| `find_context_thematic_breaks` | [43.954 µs]=[62.247 GiB/s] | [72.676 µs]=[10.644 GiB/s] | [322.36 µs]=[4.5503 GiB/s] |
| `find_context_blockquotes`     | [33.033 µs]=[82.828 GiB/s] | [37.847 µs]=[20.439 GiB/s] | [119.98 µs]=[12.226 GiB/s] |
| `find_context_bullet_items`    | [43.864 µs]=[62.376 GiB/s] | [69.449 µs]=[11.138 GiB/s] | [284.53 µs]=[5.1554 GiB/s] |
| `find_context_ordered_items`   | [257.74 µs]=[10.616 GiB/s] | [96.862 µs]=[7.9862 GiB/s] | [255.62 µs]=[5.7383 GiB/s] |
 
#### Cold context - `find_context_*_cold`
| Function                            | `plain`                    | `hot`                      | `heavy`                    |
|-------------------------------------|----------------------------|----------------------------|----------------------------|
| `find_context_italics_cold`         | [76.800 µs]=[35.625 GiB/s] | [493.30 µs]=[1.5681 GiB/s] | [965.91 µs]=[1.5186 GiB/s] |
| `find_context_bolds_cold`           | [76.069 µs]=[35.968 GiB/s] | [490.87 µs]=[1.5759 GiB/s] | [1.0383 ms]=[1.4127 GiB/s] |
| `find_context_bold_italics_cold`    | [77.652 µs]=[35.235 GiB/s] | [468.77 µs]=[1.6502 GiB/s] | [950.88 µs]=[1.5426 GiB/s] |
| `find_context_headings_cold`        | [79.153 µs]=[34.566 GiB/s] | [445.49 µs]=[1.7364 GiB/s] | [615.31 µs]=[2.3839 GiB/s] |
| `find_context_thematic_breaks_cold` | [90.746 µs]=[30.150 GiB/s] | [412.21 µs]=[1.8766 GiB/s] | [878.77 µs]=[1.6692 GiB/s] |
| `find_context_blockquotes_cold`     | [76.424 µs]=[35.801 GiB/s] | [375.85 µs]=[2.0582 GiB/s] | [668.04 µs]=[2.1958 GiB/s] |
| `find_context_bullet_items_cold`    | [87.774 µs]=[31.171 GiB/s] | [407.15 µs]=[1.8999 GiB/s] | [841.59 µs]=[1.7430 GiB/s] |
| `find_context_ordered_items_cold`   | [299.15 µs]=[9.1462 GiB/s] | [433.15 µs]=[1.7859 GiB/s] | [802.63 µs]=[1.8276 GiB/s] |
 


<details>
    <summary>plain full log</summary>

    ```
    ┌─ corpus: plain
    │  size:          2.80 MiB  (2937800 bytes)
    │  elements:         2     (0.0 per KiB)
    │  span mem:      0.00 MiB  (~0.0% of input, 8 B/span lower bound)
    │
    │          headings:         0 √    thematic_breaks:         0 √         paragraphs:         1 √
    │       blockquotes:         0 √       fenced_codes:         0 √       bullet_items:         0 √
    │     ordered_items:         0 √              bolds:         0 √            italics:         0 √
    │      bold_italics:         0 √              codes:         0 √              links:         0 √
    │         autolinks:         0 √        hard_breaks:         0 √              texts:         1 √
    └─
    │  full-vs-standalone counts:
        find_codes         full=       0  standalone=       0
    standalone/plain/find_codes
                            time:   [31.905 µs 32.412 µs 32.854 µs]
                            thrpt:  [83.279 GiB/s 84.415 GiB/s 85.756 GiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      3 (15.00%) low mild
      1 (5.00%) high mild
        find_italics       full=       0  standalone=       0
    standalone/plain/find_italics
                            time:   [30.911 µs 31.427 µs 31.741 µs]
                            thrpt:  [86.199 GiB/s 87.059 GiB/s 88.512 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      3 (15.00%) high mild
        find_bolds         full=       0  standalone=       0
    standalone/plain/find_bolds
                            time:   [31.988 µs 32.581 µs 33.373 µs]
                            thrpt:  [81.983 GiB/s 83.977 GiB/s 85.534 GiB/s]
        find_bold_italics  full=       0  standalone=       0
    standalone/plain/find_bold_italics
                            time:   [32.919 µs 33.416 µs 33.865 µs]
                            thrpt:  [80.792 GiB/s 81.877 GiB/s 83.114 GiB/s]
        find_autolinks     full=       0  standalone=       0
    standalone/plain/find_autolinks
                            time:   [31.861 µs 32.344 µs 32.932 µs]
                            thrpt:  [83.082 GiB/s 84.591 GiB/s 85.875 GiB/s]
        find_links         full=       0  standalone=       0
    standalone/plain/find_links
                            time:   [31.288 µs 31.496 µs 31.692 µs]
                            thrpt:  [86.331 GiB/s 86.869 GiB/s 87.448 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_headings      full=       0  standalone=       0
    standalone/plain/find_headings
                            time:   [32.413 µs 33.000 µs 33.460 µs]
                            thrpt:  [81.771 GiB/s 82.911 GiB/s 84.413 GiB/s]
        find_thematic_breaks full=       0  standalone=       0
    standalone/plain/find_thematic_breaks
                            time:   [44.231 µs 44.775 µs 45.275 µs]
                            thrpt:  [60.431 GiB/s 61.106 GiB/s 61.858 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
        find_fenced_codes  full=       0  standalone=       0
    standalone/plain/find_fenced_codes
                            time:   [32.251 µs 32.867 µs 33.351 µs]
                            thrpt:  [82.038 GiB/s 83.246 GiB/s 84.835 GiB/s]
        find_blockquotes   full=       0  standalone=       0
    standalone/plain/find_blockquotes
                            time:   [31.089 µs 31.855 µs 32.770 µs]
                            thrpt:  [83.493 GiB/s 85.890 GiB/s 88.006 GiB/s]
        find_bullet_items  full=       0  standalone=       0
    standalone/plain/find_bullet_items
                            time:   [44.025 µs 44.467 µs 44.951 µs]
                            thrpt:  [60.867 GiB/s 61.529 GiB/s 62.148 GiB/s]
        find_ordered_items full=       0  standalone=       0
    standalone/plain/find_ordered_items
                            time:   [254.04 µs 254.53 µs 255.11 µs]
                            thrpt:  [10.725 GiB/s 10.749 GiB/s 10.770 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    │  context regions: 0
    standalone/plain/context
                            time:   [44.106 µs 44.897 µs 45.514 µs]
                            thrpt:  [60.114 GiB/s 60.941 GiB/s 62.033 GiB/s]
    │  full-vs-context-aware counts:
        find_context_italics full=       0  context-aware=       0
    standalone/plain/find_context_italics
                            time:   [32.630 µs 33.242 µs 33.736 µs]
                            thrpt:  [81.100 GiB/s 82.307 GiB/s 83.851 GiB/s]
        find_context_bolds full=       0  context-aware=       0
    standalone/plain/find_context_bolds
                            time:   [32.312 µs 32.756 µs 33.252 µs]
                            thrpt:  [82.282 GiB/s 83.528 GiB/s 84.677 GiB/s]
        find_context_bold_italics full=       0  context-aware=       0
    standalone/plain/find_context_bold_italics
                            time:   [32.133 µs 32.773 µs 33.202 µs]
                            thrpt:  [82.407 GiB/s 83.485 GiB/s 85.148 GiB/s]
    Found 5 outliers among 20 measurements (25.00%)
      5 (25.00%) high mild
        find_context_headings full=       0  context-aware=       0
    standalone/plain/find_context_headings
                            time:   [32.015 µs 32.146 µs 32.377 µs]
                            thrpt:  [84.505 GiB/s 85.112 GiB/s 85.461 GiB/s]
        find_context_thematic_breaks full=       0  context-aware=       0
    standalone/plain/find_context_thematic_breaks
                            time:   [43.608 µs 43.954 µs 44.251 µs]
                            thrpt:  [61.830 GiB/s 62.247 GiB/s 62.741 GiB/s]
        find_context_blockquotes full=       0  context-aware=       0
    standalone/plain/find_context_blockquotes
                            time:   [32.517 µs 33.033 µs 33.486 µs]
                            thrpt:  [81.708 GiB/s 82.828 GiB/s 84.142 GiB/s]
        find_context_bullet_items full=       0  context-aware=       0
    standalone/plain/find_context_bullet_items
                            time:   [43.223 µs 43.864 µs 44.271 µs]
                            thrpt:  [61.802 GiB/s 62.376 GiB/s 63.300 GiB/s]
        find_context_ordered_items full=       0  context-aware=       0
    standalone/plain/find_context_ordered_items
                            time:   [255.67 µs 257.74 µs 260.67 µs]
                            thrpt:  [10.496 GiB/s 10.616 GiB/s 10.702 GiB/s]
    standalone/plain/find_context_italics_cold
                            time:   [75.598 µs 76.800 µs 77.777 µs]
                            thrpt:  [35.178 GiB/s 35.625 GiB/s 36.192 GiB/s]
    standalone/plain/find_context_bolds_cold
                            time:   [75.222 µs 76.069 µs 77.074 µs]
                            thrpt:  [35.499 GiB/s 35.968 GiB/s 36.373 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) low mild
    standalone/plain/find_context_bold_italics_cold
                            time:   [76.901 µs 77.652 µs 78.568 µs]
                            thrpt:  [34.824 GiB/s 35.235 GiB/s 35.579 GiB/s]
    standalone/plain/find_context_headings_cold
                            time:   [78.527 µs 79.153 µs 79.625 µs]
                            thrpt:  [34.362 GiB/s 34.566 GiB/s 34.842 GiB/s]
    standalone/plain/find_context_thematic_breaks_cold
                            time:   [89.954 µs 90.746 µs 91.435 µs]
                            thrpt:  [29.923 GiB/s 30.150 GiB/s 30.416 GiB/s]
    standalone/plain/find_context_blockquotes_cold
                            time:   [75.885 µs 76.424 µs 76.830 µs]
                            thrpt:  [35.612 GiB/s 35.801 GiB/s 36.055 GiB/s]
    standalone/plain/find_context_bullet_items_cold
                            time:   [86.658 µs 87.774 µs 88.778 µs]
                            thrpt:  [30.819 GiB/s 31.171 GiB/s 31.573 GiB/s]
    standalone/plain/find_context_ordered_items_cold
                            time:   [297.24 µs 299.15 µs 301.26 µs]
                            thrpt:  [9.0819 GiB/s 9.1462 GiB/s 9.2048 GiB/s]
    ```

</details>

<details>
    <summary>hot full log</summary>

    ```
    ┌─ corpus: hot
    │  size:          0.79 MiB  (830600 bytes)
    │  elements:     70000     (86.3 per KiB)
    │  span mem:      0.53 MiB  (~67.4% of input, 8 B/span lower bound)
    │
    │          headings:      5000 √    thematic_breaks:         0 √         paragraphs:      5000 √
    │       blockquotes:         0 √       fenced_codes:         0 √       bullet_items:         0 √
    │     ordered_items:         0 √              bolds:      5000 √            italics:      5000 √
    │      bold_italics:         0 √              codes:      5000 √              links:      5000 √
    │         autolinks:      5000 √        hard_breaks:         0 √              texts:     35000 √
    └─
    │  full-vs-standalone counts:
        find_codes         full=    5000  standalone=    5000
    standalone/hot/find_codes
                            time:   [73.097 µs 73.470 µs 74.029 µs]
                            thrpt:  [10.449 GiB/s 10.529 GiB/s 10.583 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_italics       full=    5000  standalone=    5000
    standalone/hot/find_italics
                            time:   [130.38 µs 130.69 µs 130.98 µs]
                            thrpt:  [5.9059 GiB/s 5.9188 GiB/s 5.9330 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low mild
      1 (5.00%) high severe
        find_bolds         full=    5000  standalone=    5000
    standalone/hot/find_bolds
                            time:   [133.75 µs 134.54 µs 135.20 µs]
                            thrpt:  [5.7216 GiB/s 5.7497 GiB/s 5.7835 GiB/s]
        find_bold_italics  full=       0  standalone=       0
    standalone/hot/find_bold_italics
                            time:   [118.30 µs 119.28 µs 120.27 µs]
                            thrpt:  [6.4318 GiB/s 6.4854 GiB/s 6.5389 GiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      2 (10.00%) low severe
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_autolinks     full=    5000  standalone=    5000
    standalone/hot/find_autolinks
                            time:   [81.633 µs 81.891 µs 82.129 µs]
                            thrpt:  [9.4188 GiB/s 9.4462 GiB/s 9.4761 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_links         full=    5000  standalone=    5000
    standalone/hot/find_links
                            time:   [173.97 µs 174.97 µs 175.71 µs]
                            thrpt:  [4.4024 GiB/s 4.4210 GiB/s 4.4466 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_headings      full=    5000  standalone=    5000
    standalone/hot/find_headings
                            time:   [75.865 µs 76.109 µs 76.379 µs]
                            thrpt:  [10.128 GiB/s 10.164 GiB/s 10.197 GiB/s]
        find_thematic_breaks full=       0  standalone=       0
    standalone/hot/find_thematic_breaks
                            time:   [72.633 µs 72.713 µs 72.800 µs]
                            thrpt:  [10.626 GiB/s 10.639 GiB/s 10.650 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
        find_fenced_codes  full=       0  standalone=       0
    standalone/hot/find_fenced_codes
                            time:   [62.210 µs 62.290 µs 62.378 µs]
                            thrpt:  [12.401 GiB/s 12.419 GiB/s 12.435 GiB/s]
        find_blockquotes   full=       0  standalone=       0
    standalone/hot/find_blockquotes
                            time:   [37.651 µs 37.787 µs 37.923 µs]
                            thrpt:  [20.398 GiB/s 20.472 GiB/s 20.545 GiB/s]
        find_bullet_items  full=       0  standalone=       0
    standalone/hot/find_bullet_items
                            time:   [69.380 µs 69.481 µs 69.604 µs]
                            thrpt:  [11.114 GiB/s 11.133 GiB/s 11.150 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_ordered_items full=       0  standalone=       0
    standalone/hot/find_ordered_items
                            time:   [95.864 µs 95.971 µs 96.073 µs]
                            thrpt:  [8.0518 GiB/s 8.0603 GiB/s 8.0693 GiB/s]
    │  context regions: 15000
    standalone/hot/context  time:   [339.10 µs 340.12 µs 341.28 µs]
                            thrpt:  [2.2666 GiB/s 2.2744 GiB/s 2.2812 GiB/s]
    │  full-vs-context-aware counts:
        find_context_italics full=    5000  context-aware=    5000
    standalone/hot/find_context_italics
                            time:   [150.75 µs 151.02 µs 151.33 µs]
                            thrpt:  [5.1117 GiB/s 5.1222 GiB/s 5.1314 GiB/s]
        find_context_bolds full=    5000  context-aware=    5000
    standalone/hot/find_context_bolds
                            time:   [154.29 µs 154.76 µs 155.31 µs]
                            thrpt:  [4.9806 GiB/s 4.9984 GiB/s 5.0136 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_context_bold_italics full=       0  context-aware=       0
    standalone/hot/find_context_bold_italics
                            time:   [128.46 µs 129.38 µs 130.26 µs]
                            thrpt:  [5.9386 GiB/s 5.9792 GiB/s 6.0219 GiB/s]
        find_context_headings full=    5000  context-aware=    5000
    standalone/hot/find_context_headings
                            time:   [102.21 µs 102.70 µs 103.27 µs]
                            thrpt:  [7.4909 GiB/s 7.5320 GiB/s 7.5680 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
        find_context_thematic_breaks full=       0  context-aware=       0
    standalone/hot/find_context_thematic_breaks
                            time:   [72.593 µs 72.676 µs 72.754 µs]
                            thrpt:  [10.632 GiB/s 10.644 GiB/s 10.656 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_context_blockquotes full=       0  context-aware=       0
    standalone/hot/find_context_blockquotes
                            time:   [37.668 µs 37.847 µs 38.027 µs]
                            thrpt:  [20.342 GiB/s 20.439 GiB/s 20.536 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
        find_context_bullet_items full=       0  context-aware=       0
    standalone/hot/find_context_bullet_items
                            time:   [69.387 µs 69.449 µs 69.509 µs]
                            thrpt:  [11.129 GiB/s 11.138 GiB/s 11.148 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_context_ordered_items full=       0  context-aware=       0
    standalone/hot/find_context_ordered_items
                            time:   [96.411 µs 96.862 µs 97.284 µs]
                            thrpt:  [7.9515 GiB/s 7.9862 GiB/s 8.0235 GiB/s]
    standalone/hot/find_context_italics_cold
                            time:   [492.25 µs 493.30 µs 494.16 µs]
                            thrpt:  [1.5654 GiB/s 1.5681 GiB/s 1.5715 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    standalone/hot/find_context_bolds_cold
                            time:   [489.20 µs 490.87 µs 492.93 µs]
                            thrpt:  [1.5693 GiB/s 1.5759 GiB/s 1.5813 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    standalone/hot/find_context_bold_italics_cold
                            time:   [467.35 µs 468.77 µs 470.07 µs]
                            thrpt:  [1.6456 GiB/s 1.6502 GiB/s 1.6552 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
    standalone/hot/find_context_headings_cold
                            time:   [443.08 µs 445.49 µs 447.56 µs]
                            thrpt:  [1.7284 GiB/s 1.7364 GiB/s 1.7459 GiB/s]
    standalone/hot/find_context_thematic_breaks_cold
                            time:   [410.95 µs 412.21 µs 413.61 µs]
                            thrpt:  [1.8703 GiB/s 1.8766 GiB/s 1.8824 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    standalone/hot/find_context_blockquotes_cold
                            time:   [374.02 µs 375.85 µs 377.52 µs]
                            thrpt:  [2.0490 GiB/s 2.0582 GiB/s 2.0682 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) low mild
      1 (5.00%) high mild
    standalone/hot/find_context_bullet_items_cold
                            time:   [405.57 µs 407.15 µs 409.04 µs]
                            thrpt:  [1.8911 GiB/s 1.8999 GiB/s 1.9073 GiB/s]
    standalone/hot/find_context_ordered_items_cold
                            time:   [431.56 µs 433.15 µs 435.15 µs]
                            thrpt:  [1.7777 GiB/s 1.7859 GiB/s 1.7925 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    ```

</details>

<details>
    <summary>heavy full log</summary>

    ```
    ┌─ corpus: heavy
    │  size:          1.50 MiB  (1575020 bytes)
    │  elements:    144000     (93.6 per KiB)
    │  span mem:      1.10 MiB  (~73.1% of input, 8 B/span lower bound)
    │
    │          headings:      2000 √    thematic_breaks:      2000 √         paragraphs:      4000 √
    │       blockquotes:      4000 √       fenced_codes:      4000 √       bullet_items:      6000 √
    │     ordered_items:      4000 √              bolds:     12000 √            italics:     12000 √
    │      bold_italics:      6000 √              codes:     10000 √              links:      6000 √
    │         autolinks:      4000 √        hard_breaks:         0 √              texts:     68000 √
    └─
    │  full-vs-standalone counts:
        find_codes         full=   10000  standalone=   10000
    standalone/heavy/find_codes
                            time:   [201.41 µs 202.18 µs 203.04 µs]
                            thrpt:  [7.2244 GiB/s 7.2551 GiB/s 7.2830 GiB/s]
        find_italics       full=   12000  standalone=   12000
    standalone/heavy/find_italics
                            time:   [362.03 µs 364.12 µs 365.97 µs]
                            thrpt:  [4.0082 GiB/s 4.0285 GiB/s 4.0517 GiB/s]
        find_bolds         full=   12000  standalone=   12000
    standalone/heavy/find_bolds
                            time:   [369.76 µs 371.62 µs 373.28 µs]
                            thrpt:  [3.9296 GiB/s 3.9472 GiB/s 3.9670 GiB/s]
        find_bold_italics  full=    6000  standalone=    6000
    standalone/heavy/find_bold_italics
                            time:   [354.23 µs 355.40 µs 356.65 µs]
                            thrpt:  [4.1129 GiB/s 4.1274 GiB/s 4.1409 GiB/s]
        find_autolinks     full=    4000  standalone=    4000
    standalone/heavy/find_autolinks
                            time:   [75.192 µs 75.411 µs 75.636 µs]
                            thrpt:  [19.394 GiB/s 19.452 GiB/s 19.508 GiB/s]
        find_links         full=    6000  standalone=    6000
    standalone/heavy/find_links
                            time:   [214.41 µs 215.31 µs 216.16 µs]
                            thrpt:  [6.7860 GiB/s 6.8127 GiB/s 6.8414 GiB/s]
        find_headings      full=    2000  standalone=    2000
    standalone/heavy/find_headings
                            time:   [42.504 µs 42.708 µs 42.917 µs]
                            thrpt:  [34.179 GiB/s 34.346 GiB/s 34.511 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) low mild
        find_thematic_breaks full=    2000  standalone=    2000
    standalone/heavy/find_thematic_breaks
                            time:   [279.50 µs 280.86 µs 282.15 µs]
                            thrpt:  [5.1988 GiB/s 5.2227 GiB/s 5.2481 GiB/s]
        find_fenced_codes  full=    4000  standalone=    4000
    standalone/heavy/find_fenced_codes
                            time:   [227.35 µs 227.76 µs 228.31 µs]
                            thrpt:  [6.4249 GiB/s 6.4404 GiB/s 6.4519 GiB/s]
        find_blockquotes   full=    4000  standalone=    4000
    standalone/heavy/find_blockquotes
                            time:   [104.83 µs 105.12 µs 105.52 µs]
                            thrpt:  [13.901 GiB/s 13.954 GiB/s 13.992 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_bullet_items  full=    6000  standalone=    6000
    standalone/heavy/find_bullet_items
                            time:   [232.32 µs 232.74 µs 233.17 µs]
                            thrpt:  [6.2910 GiB/s 6.3024 GiB/s 6.3139 GiB/s]
        find_ordered_items full=    4000  standalone=    4000
    standalone/heavy/find_ordered_items
                            time:   [204.02 µs 204.42 µs 204.84 µs]
                            thrpt:  [7.1609 GiB/s 7.1758 GiB/s 7.1898 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low mild
      1 (5.00%) high mild
    │  context regions: 24000
    standalone/heavy/context
                            time:   [549.35 µs 550.20 µs 550.97 µs]
                            thrpt:  [2.6623 GiB/s 2.6660 GiB/s 2.6701 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      3 (15.00%) high mild
    │  full-vs-context-aware counts:
        find_context_italics full=   12000  context-aware=   12000
    standalone/heavy/find_context_italics
                            time:   [415.65 µs 418.05 µs 419.94 µs]
                            thrpt:  [3.4930 GiB/s 3.5088 GiB/s 3.5290 GiB/s]
        find_context_bolds full=   12000  context-aware=   12000
    standalone/heavy/find_context_bolds
                            time:   [485.85 µs 487.54 µs 488.76 µs]
                            thrpt:  [3.0012 GiB/s 3.0087 GiB/s 3.0192 GiB/s]
        find_context_bold_italics full=    6000  context-aware=    6000
    standalone/heavy/find_context_bold_italics
                            time:   [394.39 µs 395.22 µs 396.11 µs]
                            thrpt:  [3.7031 GiB/s 3.7115 GiB/s 3.7193 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low mild
      1 (5.00%) high mild
        find_context_headings full=    2000  context-aware=    2000
    standalone/heavy/find_context_headings
                            time:   [64.429 µs 64.596 µs 64.799 µs]
                            thrpt:  [22.637 GiB/s 22.708 GiB/s 22.767 GiB/s]
        find_context_thematic_breaks full=    2000  context-aware=    2000
    standalone/heavy/find_context_thematic_breaks
                            time:   [321.44 µs 322.36 µs 323.58 µs]
                            thrpt:  [4.5332 GiB/s 4.5503 GiB/s 4.5634 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_context_blockquotes full=    4000  context-aware=    4000
    standalone/heavy/find_context_blockquotes
                            time:   [119.34 µs 119.98 µs 120.61 µs]
                            thrpt:  [12.162 GiB/s 12.226 GiB/s 12.292 GiB/s]
        find_context_bullet_items full=    6000  context-aware=    6000
    standalone/heavy/find_context_bullet_items
                            time:   [283.70 µs 284.53 µs 285.57 µs]
                            thrpt:  [5.1366 GiB/s 5.1554 GiB/s 5.1705 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_context_ordered_items full=    4000  context-aware=    4000
    standalone/heavy/find_context_ordered_items
                            time:   [254.77 µs 255.62 µs 257.01 µs]
                            thrpt:  [5.7074 GiB/s 5.7383 GiB/s 5.7576 GiB/s]
    standalone/heavy/find_context_italics_cold
                            time:   [962.67 µs 965.91 µs 969.27 µs]
                            thrpt:  [1.5134 GiB/s 1.5186 GiB/s 1.5237 GiB/s]
    standalone/heavy/find_context_bolds_cold
                            time:   [1.0336 ms 1.0383 ms 1.0447 ms]
                            thrpt:  [1.4041 GiB/s 1.4127 GiB/s 1.4192 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    standalone/heavy/find_context_bold_italics_cold
                            time:   [947.54 µs 950.88 µs 954.66 µs]
                            thrpt:  [1.5365 GiB/s 1.5426 GiB/s 1.5481 GiB/s]
    standalone/heavy/find_context_headings_cold
                            time:   [613.15 µs 615.31 µs 617.38 µs]
                            thrpt:  [2.3759 GiB/s 2.3839 GiB/s 2.3923 GiB/s]
    standalone/heavy/find_context_thematic_breaks_cold
                            time:   [874.85 µs 878.77 µs 882.48 µs]
                            thrpt:  [1.6622 GiB/s 1.6692 GiB/s 1.6767 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    standalone/heavy/find_context_blockquotes_cold
                            time:   [666.22 µs 668.04 µs 670.27 µs]
                            thrpt:  [2.1885 GiB/s 2.1958 GiB/s 2.2018 GiB/s]
    standalone/heavy/find_context_bullet_items_cold
                            time:   [837.52 µs 841.59 µs 847.27 µs]
                            thrpt:  [1.7313 GiB/s 1.7430 GiB/s 1.7514 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    standalone/heavy/find_context_ordered_items_cold
                            time:   [800.34 µs 802.63 µs 805.64 µs]
                            thrpt:  [1.8207 GiB/s 1.8276 GiB/s 1.8328 GiB/s]
    ```

</details>

**big (exceeds L3 cache):**

#### Standalone - `find_*`
| Function               | `plain`                    | `hot`                      | `heavy`                    |
|------------------------|----------------------------|----------------------------|----------------------------|
| `find_codes`           | [13.175 ms]=[20.767 GiB/s] | [8.2300 ms]=[9.3992 GiB/s] | [21.617 ms]=[6.7856 GiB/s] |
| `find_italics`         | [12.934 ms]=[21.154 GiB/s] | [13.605 ms]=[5.6859 GiB/s] | [37.219 ms]=[3.9411 GiB/s] |
| `find_bolds`           | [12.990 ms]=[21.062 GiB/s] | [14.022 ms]=[5.5168 GiB/s] | [37.735 ms]=[3.8872 GiB/s] |
| `find_bold_italics`    | [13.174 ms]=[20.768 GiB/s] | [12.234 ms]=[6.3229 GiB/s] | [36.280 ms]=[4.0432 GiB/s] |
| `find_autolinks`       | [13.145 ms]=[20.815 GiB/s] | [8.2935 ms]=[9.3272 GiB/s] | [11.367 ms]=[12.904 GiB/s] |
| `find_links`           | [13.084 ms]=[20.911 GiB/s] | [17.956 ms]=[4.3082 GiB/s] | [22.946 ms]=[6.3925 GiB/s] |
| `find_headings`        | [13.141 ms]=[20.821 GiB/s] | [8.5320 ms]=[9.0665 GiB/s] | [9.4776 ms]=[15.477 GiB/s] |
| `find_thematic_breaks` | [15.201 ms]=[17.999 GiB/s] | [8.2162 ms]=[9.4150 GiB/s] | [29.118 ms]=[5.0376 GiB/s] |
| `find_fenced_codes`    | [13.177 ms]=[20.763 GiB/s] | [7.4732 ms]=[10.351 GiB/s] | [24.289 ms]=[6.0391 GiB/s] |
| `find_blockquotes`     | [12.846 ms]=[21.298 GiB/s] | [5.8002 ms]=[13.337 GiB/s] | [13.590 ms]=[10.794 GiB/s] |
| `find_bullet_items`    | [14.874 ms]=[18.395 GiB/s] | [8.0561 ms]=[9.6021 GiB/s] | [24.698 ms]=[5.9391 GiB/s] |
| `find_ordered_items`   | [29.266 ms]=[9.3489 GiB/s] | [10.419 ms]=[7.4247 GiB/s] | [22.132 ms]=[6.6276 GiB/s] |
 
#### Context-aware - `context()` + `find_context_*`
| Function                       | `plain`                    | `hot`                      | `heavy`                    |
|--------------------------------|----------------------------|----------------------------|----------------------------|
| `context`                      | [15.183 ms]=[18.020 GiB/s] | [33.915 ms]=[2.2809 GiB/s] | [65.707 ms]=[2.2324 GiB/s] |
| `find_context_italics`         | [13.118 ms]=[20.857 GiB/s] | [15.668 ms]=[4.9371 GiB/s] | [42.457 ms]=[3.4549 GiB/s] |
| `find_context_bolds`           | [13.164 ms]=[20.784 GiB/s] | [16.027 ms]=[4.8265 GiB/s] | [48.991 ms]=[2.9941 GiB/s] |
| `find_context_bold_italics`    | [13.258 ms]=[20.637 GiB/s] | [13.660 ms]=[5.6629 GiB/s] | [40.126 ms]=[3.6556 GiB/s] |
| `find_context_headings`        | [13.172 ms]=[20.771 GiB/s] | [11.093 ms]=[6.9732 GiB/s] | [10.922 ms]=[13.430 GiB/s] |
| `find_context_thematic_breaks` | [14.942 ms]=[18.311 GiB/s] | [8.2459 ms]=[9.3812 GiB/s] | [33.373 ms]=[4.3954 GiB/s] |
| `find_context_blockquotes`     | [13.075 ms]=[20.925 GiB/s] | [5.6160 ms]=[13.774 GiB/s] | [14.857 ms]=[9.8728 GiB/s] |
| `find_context_bullet_items`    | [15.259 ms]=[17.930 GiB/s] | [8.0409 ms]=[9.6203 GiB/s] | [29.672 ms]=[4.9435 GiB/s] |
| `find_context_ordered_items`   | [29.189 ms]=[9.3735 GiB/s] | [10.400 ms]=[7.4383 GiB/s] | [26.738 ms]=[5.4860 GiB/s] |
 
#### Cold context - `find_context_*_cold`
| Function                            | `plain`                    | `hot`                      | `heavy`                    |
|-------------------------------------|----------------------------|----------------------------|----------------------------|
| `find_context_italics_cold`         | [28.347 ms]=[9.6521 GiB/s] | [49.703 ms]=[1.5564 GiB/s] | [108.77 ms]=[1.3486 GiB/s] |
| `find_context_bolds_cold`           | [28.471 ms]=[9.6100 GiB/s] | [50.136 ms]=[1.5429 GiB/s] | [114.90 ms]=[1.2766 GiB/s] |
| `find_context_bold_italics_cold`    | [28.498 ms]=[9.6010 GiB/s] | [47.650 ms]=[1.6234 GiB/s] | [105.29 ms]=[1.3932 GiB/s] |
| `find_context_headings_cold`        | [28.226 ms]=[9.6933 GiB/s] | [45.270 ms]=[1.7088 GiB/s] | [76.753 ms]=[1.9111 GiB/s] |
| `find_context_thematic_breaks_cold` | [29.874 ms]=[9.1585 GiB/s] | [42.016 ms]=[1.8411 GiB/s] | [99.215 ms]=[1.4785 GiB/s] |
| `find_context_blockquotes_cold`     | [28.407 ms]=[9.6316 GiB/s] | [39.418 ms]=[1.9625 GiB/s] | [80.344 ms]=[1.8257 GiB/s] |
| `find_context_bullet_items_cold`    | [33.860 ms]=[8.0804 GiB/s] | [42.147 ms]=[1.8354 GiB/s] | [95.429 ms]=[1.5371 GiB/s] |
| `find_context_ordered_items_cold`   | [44.906 ms]=[6.0928 GiB/s] | [44.436 ms]=[1.7408 GiB/s] | [92.267 ms]=[1.5898 GiB/s] |

<details>
    <summary>plain full log</summary>

    ```
    ┌─ corpus: plain
    │  size:        280.17 MiB  (293780000 bytes)
    │  elements:         2     (0.0 per KiB)
    │  span mem:      0.00 MiB  (~0.0% of input, 8 B/span lower bound)
    │
    │          headings:         0 √    thematic_breaks:         0 √         paragraphs:         1 √
    │       blockquotes:         0 √       fenced_codes:         0 √       bullet_items:         0 √
    │     ordered_items:         0 √              bolds:         0 √            italics:         0 √
    │      bold_italics:         0 √              codes:         0 √              links:         0 √
    │         autolinks:         0 √        hard_breaks:         0 √              texts:         1 √
    └─
    │  full-vs-standalone counts:
        find_codes         full=       0  standalone=       0
    standalone/plain/find_codes
                            time:   [13.085 ms 13.175 ms 13.264 ms]
                            thrpt:  [20.627 GiB/s 20.767 GiB/s 20.909 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
        find_italics       full=       0  standalone=       0
    standalone/plain/find_italics
                            time:   [12.893 ms 12.934 ms 12.976 ms]
                            thrpt:  [21.086 GiB/s 21.154 GiB/s 21.221 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_bolds         full=       0  standalone=       0
    standalone/plain/find_bolds
                            time:   [12.930 ms 12.990 ms 13.048 ms]
                            thrpt:  [20.969 GiB/s 21.062 GiB/s 21.161 GiB/s]
        find_bold_italics  full=       0  standalone=       0
    standalone/plain/find_bold_italics
                            time:   [13.112 ms 13.174 ms 13.235 ms]
                            thrpt:  [20.672 GiB/s 20.768 GiB/s 20.867 GiB/s]
        find_autolinks     full=       0  standalone=       0
    standalone/plain/find_autolinks
                            time:   [13.072 ms 13.145 ms 13.208 ms]
                            thrpt:  [20.715 GiB/s 20.815 GiB/s 20.931 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low mild
      1 (5.00%) high severe
        find_links         full=       0  standalone=       0
    standalone/plain/find_links
                            time:   [13.021 ms 13.084 ms 13.131 ms]
                            thrpt:  [20.837 GiB/s 20.911 GiB/s 21.012 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_headings      full=       0  standalone=       0
    standalone/plain/find_headings
                            time:   [13.054 ms 13.141 ms 13.233 ms]
                            thrpt:  [20.676 GiB/s 20.821 GiB/s 20.960 GiB/s]
        find_thematic_breaks full=       0  standalone=       0
    standalone/plain/find_thematic_breaks
                            time:   [15.135 ms 15.201 ms 15.262 ms]
                            thrpt:  [17.928 GiB/s 17.999 GiB/s 18.078 GiB/s]
        find_fenced_codes  full=       0  standalone=       0
    standalone/plain/find_fenced_codes
                            time:   [13.082 ms 13.177 ms 13.287 ms]
                            thrpt:  [20.592 GiB/s 20.763 GiB/s 20.915 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_blockquotes   full=       0  standalone=       0
    standalone/plain/find_blockquotes
                            time:   [12.772 ms 12.846 ms 12.924 ms]
                            thrpt:  [21.170 GiB/s 21.298 GiB/s 21.421 GiB/s]
        find_bullet_items  full=       0  standalone=       0
    standalone/plain/find_bullet_items
                            time:   [14.781 ms 14.874 ms 14.961 ms]
                            thrpt:  [18.287 GiB/s 18.395 GiB/s 18.510 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_ordered_items full=       0  standalone=       0
    standalone/plain/find_ordered_items
                            time:   [29.203 ms 29.266 ms 29.327 ms]
                            thrpt:  [9.3293 GiB/s 9.3489 GiB/s 9.3690 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    │  context regions: 0
    standalone/plain/context
                            time:   [15.102 ms 15.183 ms 15.278 ms]
                            thrpt:  [17.908 GiB/s 18.020 GiB/s 18.117 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    │  full-vs-context-aware counts:
        find_context_italics full=       0  context-aware=       0
    standalone/plain/find_context_italics
                            time:   [13.046 ms 13.118 ms 13.196 ms]
                            thrpt:  [20.734 GiB/s 20.857 GiB/s 20.973 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_context_bolds full=       0  context-aware=       0
    standalone/plain/find_context_bolds
                            time:   [13.114 ms 13.164 ms 13.207 ms]
                            thrpt:  [20.717 GiB/s 20.784 GiB/s 20.864 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low mild
      1 (5.00%) high severe
        find_context_bold_italics full=       0  context-aware=       0
    standalone/plain/find_context_bold_italics
                            time:   [13.205 ms 13.258 ms 13.307 ms]
                            thrpt:  [20.561 GiB/s 20.637 GiB/s 20.720 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
        find_context_headings full=       0  context-aware=       0
    standalone/plain/find_context_headings
                            time:   [13.105 ms 13.172 ms 13.244 ms]
                            thrpt:  [20.659 GiB/s 20.771 GiB/s 20.878 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_context_thematic_breaks full=       0  context-aware=       0
    standalone/plain/find_context_thematic_breaks
                            time:   [14.880 ms 14.942 ms 15.008 ms]
                            thrpt:  [18.231 GiB/s 18.311 GiB/s 18.388 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low mild
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_context_blockquotes full=       0  context-aware=       0
    standalone/plain/find_context_blockquotes
                            time:   [12.966 ms 13.075 ms 13.179 ms]
                            thrpt:  [20.760 GiB/s 20.925 GiB/s 21.102 GiB/s]
        find_context_bullet_items full=       0  context-aware=       0
    standalone/plain/find_context_bullet_items
                            time:   [15.216 ms 15.259 ms 15.326 ms]
                            thrpt:  [17.852 GiB/s 17.930 GiB/s 17.981 GiB/s]
        find_context_ordered_items full=       0  context-aware=       0
    standalone/plain/find_context_ordered_items
                            time:   [29.089 ms 29.189 ms 29.292 ms]
                            thrpt:  [9.3406 GiB/s 9.3735 GiB/s 9.4057 GiB/s]
    standalone/plain/find_context_italics_cold
                            time:   [28.192 ms 28.347 ms 28.507 ms]
                            thrpt:  [9.5979 GiB/s 9.6521 GiB/s 9.7051 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    standalone/plain/find_context_bolds_cold
                            time:   [28.296 ms 28.471 ms 28.655 ms]
                            thrpt:  [9.5481 GiB/s 9.6100 GiB/s 9.6692 GiB/s]
    standalone/plain/find_context_bold_italics_cold
                            time:   [28.403 ms 28.498 ms 28.591 ms]
                            thrpt:  [9.5695 GiB/s 9.6010 GiB/s 9.6331 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    standalone/plain/find_context_headings_cold
                            time:   [28.085 ms 28.226 ms 28.389 ms]
                            thrpt:  [9.6376 GiB/s 9.6933 GiB/s 9.7421 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    standalone/plain/find_context_thematic_breaks_cold
                            time:   [29.763 ms 29.874 ms 29.970 ms]
                            thrpt:  [9.1293 GiB/s 9.1585 GiB/s 9.1928 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    standalone/plain/find_context_blockquotes_cold
                            time:   [28.168 ms 28.407 ms 28.666 ms]
                            thrpt:  [9.5445 GiB/s 9.6316 GiB/s 9.7132 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low mild
      1 (5.00%) high mild
    standalone/plain/find_context_bullet_items_cold
                            time:   [32.718 ms 33.860 ms 34.874 ms]
                            thrpt:  [7.8456 GiB/s 8.0804 GiB/s 8.3625 GiB/s]
    
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 10.2s, enable flat sampling, or reduce sample count to 10.
    standalone/plain/find_context_ordered_items_cold
                            time:   [44.684 ms 44.906 ms 45.234 ms]
                            thrpt:  [6.0487 GiB/s 6.0928 GiB/s 6.1231 GiB/s]
    ```
  
</details>

<details>
    <summary>hot full log</summary>

    ```
    ┌─ corpus: hot
    │  size:         79.21 MiB  (83060000 bytes)
    │  elements:   7000000     (86.3 per KiB)
    │  span mem:     53.41 MiB  (~67.4% of input, 8 B/span lower bound)
    │
    │          headings:    500000 √    thematic_breaks:         0 √         paragraphs:    500000 √
    │       blockquotes:         0 √       fenced_codes:         0 √       bullet_items:         0 √
    │     ordered_items:         0 √              bolds:    500000 √            italics:    500000 √
    │      bold_italics:         0 √              codes:    500000 √              links:    500000 √
    │         autolinks:    500000 √        hard_breaks:         0 √              texts:   3500000 √
    └─
    │  full-vs-standalone counts:
        find_codes         full=  500000  standalone=  500000
    standalone/hot/find_codes
                            time:   [8.2123 ms 8.2300 ms 8.2474 ms]
                            thrpt:  [9.3794 GiB/s 9.3992 GiB/s 9.4195 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_italics       full=  500000  standalone=  500000
    standalone/hot/find_italics
                            time:   [13.581 ms 13.605 ms 13.630 ms]
                            thrpt:  [5.6752 GiB/s 5.6859 GiB/s 5.6960 GiB/s]
        find_bolds         full=  500000  standalone=  500000
    standalone/hot/find_bolds
                            time:   [13.978 ms 14.022 ms 14.060 ms]
                            thrpt:  [5.5018 GiB/s 5.5168 GiB/s 5.5339 GiB/s]
        find_bold_italics  full=       0  standalone=       0
    standalone/hot/find_bold_italics
                            time:   [12.191 ms 12.234 ms 12.288 ms]
                            thrpt:  [6.2953 GiB/s 6.3229 GiB/s 6.3451 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
        find_autolinks     full=  500000  standalone=  500000
    standalone/hot/find_autolinks
                            time:   [8.2692 ms 8.2935 ms 8.3185 ms]
                            thrpt:  [9.2993 GiB/s 9.3272 GiB/s 9.3547 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low mild
      1 (5.00%) high mild
        find_links         full=  500000  standalone=  500000
    standalone/hot/find_links
                            time:   [17.929 ms 17.956 ms 17.982 ms]
                            thrpt:  [4.3018 GiB/s 4.3082 GiB/s 4.3146 GiB/s]
        find_headings      full=  500000  standalone=  500000
    standalone/hot/find_headings
                            time:   [8.5076 ms 8.5320 ms 8.5568 ms]
                            thrpt:  [9.0403 GiB/s 9.0665 GiB/s 9.0926 GiB/s]
        find_thematic_breaks full=       0  standalone=       0
    standalone/hot/find_thematic_breaks
                            time:   [8.2039 ms 8.2162 ms 8.2295 ms]
                            thrpt:  [9.3998 GiB/s 9.4150 GiB/s 9.4291 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_fenced_codes  full=       0  standalone=       0
    standalone/hot/find_fenced_codes
                            time:   [7.4480 ms 7.4732 ms 7.4992 ms]
                            thrpt:  [10.315 GiB/s 10.351 GiB/s 10.386 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
        find_blockquotes   full=       0  standalone=       0
    standalone/hot/find_blockquotes
                            time:   [5.7706 ms 5.8002 ms 5.8255 ms]
                            thrpt:  [13.279 GiB/s 13.337 GiB/s 13.405 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low mild
      1 (5.00%) high mild
        find_bullet_items  full=       0  standalone=       0
    standalone/hot/find_bullet_items
                            time:   [8.0334 ms 8.0561 ms 8.0787 ms]
                            thrpt:  [9.5753 GiB/s 9.6021 GiB/s 9.6293 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
        find_ordered_items full=       0  standalone=       0
    standalone/hot/find_ordered_items
                            time:   [10.402 ms 10.419 ms 10.437 ms]
                            thrpt:  [7.4120 GiB/s 7.4247 GiB/s 7.4365 GiB/s]
    │  context regions: 1500000
    standalone/hot/context  time:   [33.756 ms 33.915 ms 34.038 ms]
                            thrpt:  [2.2726 GiB/s 2.2809 GiB/s 2.2916 GiB/s]
    │  full-vs-context-aware counts:
        find_context_italics full=  500000  context-aware=  500000
    standalone/hot/find_context_italics
                            time:   [15.617 ms 15.668 ms 15.722 ms]
                            thrpt:  [4.9201 GiB/s 4.9371 GiB/s 4.9533 GiB/s]
        find_context_bolds full=  500000  context-aware=  500000
    standalone/hot/find_context_bolds
                            time:   [15.921 ms 16.027 ms 16.149 ms]
                            thrpt:  [4.7902 GiB/s 4.8265 GiB/s 4.8586 GiB/s]
    Found 5 outliers among 20 measurements (25.00%)
      3 (15.00%) low mild
      2 (10.00%) high severe
        find_context_bold_italics full=       0  context-aware=       0
    standalone/hot/find_context_bold_italics
                            time:   [13.622 ms 13.660 ms 13.708 ms]
                            thrpt:  [5.6429 GiB/s 5.6629 GiB/s 5.6789 GiB/s]
        find_context_headings full=  500000  context-aware=  500000
    standalone/hot/find_context_headings
                            time:   [11.033 ms 11.093 ms 11.159 ms]
                            thrpt:  [6.9321 GiB/s 6.9732 GiB/s 7.0111 GiB/s]
        find_context_thematic_breaks full=       0  context-aware=       0
    standalone/hot/find_context_thematic_breaks
                            time:   [8.2243 ms 8.2459 ms 8.2665 ms]
                            thrpt:  [9.3577 GiB/s 9.3812 GiB/s 9.4058 GiB/s]
        find_context_blockquotes full=       0  context-aware=       0
    standalone/hot/find_context_blockquotes
                            time:   [5.5552 ms 5.6160 ms 5.6817 ms]
                            thrpt:  [13.615 GiB/s 13.774 GiB/s 13.925 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_context_bullet_items full=       0  context-aware=       0
    standalone/hot/find_context_bullet_items
                            time:   [8.0186 ms 8.0409 ms 8.0637 ms]
                            thrpt:  [9.5931 GiB/s 9.6203 GiB/s 9.6471 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
        find_context_ordered_items full=       0  context-aware=       0
    standalone/hot/find_context_ordered_items
                            time:   [10.384 ms 10.400 ms 10.416 ms]
                            thrpt:  [7.4266 GiB/s 7.4383 GiB/s 7.4492 GiB/s]
    
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 10.3s, enable flat sampling, or reduce sample count to 10.
    standalone/hot/find_context_italics_cold
                            time:   [49.457 ms 49.703 ms 49.920 ms]
                            thrpt:  [1.5496 GiB/s 1.5564 GiB/s 1.5641 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 10.5s, enable flat sampling, or reduce sample count to 10.
    standalone/hot/find_context_bolds_cold
                            time:   [50.050 ms 50.136 ms 50.207 ms]
                            thrpt:  [1.5407 GiB/s 1.5429 GiB/s 1.5456 GiB/s]
    standalone/hot/find_context_bold_italics_cold
                            time:   [47.507 ms 47.650 ms 47.794 ms]
                            thrpt:  [1.6185 GiB/s 1.6234 GiB/s 1.6283 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    standalone/hot/find_context_headings_cold
                            time:   [45.110 ms 45.270 ms 45.453 ms]
                            thrpt:  [1.7019 GiB/s 1.7088 GiB/s 1.7148 GiB/s]
    standalone/hot/find_context_thematic_breaks_cold
                            time:   [41.948 ms 42.016 ms 42.111 ms]
                            thrpt:  [1.8369 GiB/s 1.8411 GiB/s 1.8441 GiB/s]
    standalone/hot/find_context_blockquotes_cold
                            time:   [39.292 ms 39.418 ms 39.553 ms]
                            thrpt:  [1.9557 GiB/s 1.9625 GiB/s 1.9688 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    standalone/hot/find_context_bullet_items_cold
                            time:   [42.003 ms 42.147 ms 42.311 ms]
                            thrpt:  [1.8283 GiB/s 1.8354 GiB/s 1.8417 GiB/s]
    standalone/hot/find_context_ordered_items_cold
                            time:   [44.299 ms 44.436 ms 44.550 ms]
                            thrpt:  [1.7364 GiB/s 1.7408 GiB/s 1.7462 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    ```

</details>

<details>
    <summary>heavy full log</summary>

    ```
    ┌─ corpus: heavy
    │  size:        150.21 MiB  (157502000 bytes)
    │  elements:  14400000     (93.6 per KiB)
    │  span mem:    109.86 MiB  (~73.1% of input, 8 B/span lower bound)
    │
    │          headings:    200000 √    thematic_breaks:    200000 √         paragraphs:    400000 √
    │       blockquotes:    400000 √       fenced_codes:    400000 √       bullet_items:    600000 √
    │     ordered_items:    400000 √              bolds:   1200000 √            italics:   1200000 √
    │      bold_italics:    600000 √              codes:   1000000 √              links:    600000 √
    │         autolinks:    400000 √        hard_breaks:         0 √              texts:   6800000 √
    └─
    │  full-vs-standalone counts:
        find_codes         full= 1000000  standalone= 1000000
    standalone/heavy/find_codes
                            time:   [21.540 ms 21.617 ms 21.693 ms]
                            thrpt:  [6.7618 GiB/s 6.7856 GiB/s 6.8100 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_italics       full= 1200000  standalone= 1200000
    standalone/heavy/find_italics
                            time:   [37.165 ms 37.219 ms 37.284 ms]
                            thrpt:  [3.9342 GiB/s 3.9411 GiB/s 3.9469 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_bolds         full= 1200000  standalone= 1200000
    standalone/heavy/find_bolds
                            time:   [37.590 ms 37.735 ms 37.903 ms]
                            thrpt:  [3.8701 GiB/s 3.8872 GiB/s 3.9023 GiB/s]
        find_bold_italics  full=  600000  standalone=  600000
    standalone/heavy/find_bold_italics
                            time:   [36.210 ms 36.280 ms 36.350 ms]
                            thrpt:  [4.0353 GiB/s 4.0432 GiB/s 4.0509 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low mild
      1 (5.00%) high mild
        find_autolinks     full=  400000  standalone=  400000
    standalone/heavy/find_autolinks
                            time:   [11.312 ms 11.367 ms 11.420 ms]
                            thrpt:  [12.845 GiB/s 12.904 GiB/s 12.967 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) low mild
        find_links         full=  600000  standalone=  600000
    standalone/heavy/find_links
                            time:   [22.846 ms 22.946 ms 23.070 ms]
                            thrpt:  [6.3583 GiB/s 6.3925 GiB/s 6.4206 GiB/s]
        find_headings      full=  200000  standalone=  200000
    standalone/heavy/find_headings
                            time:   [9.4386 ms 9.4776 ms 9.5146 ms]
                            thrpt:  [15.417 GiB/s 15.477 GiB/s 15.541 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_thematic_breaks full=  200000  standalone=  200000
    standalone/heavy/find_thematic_breaks
                            time:   [29.061 ms 29.118 ms 29.191 ms]
                            thrpt:  [5.0249 GiB/s 5.0376 GiB/s 5.0475 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_fenced_codes  full=  400000  standalone=  400000
    standalone/heavy/find_fenced_codes
                            time:   [24.202 ms 24.289 ms 24.418 ms]
                            thrpt:  [6.0073 GiB/s 6.0391 GiB/s 6.0609 GiB/s]
        find_blockquotes   full=  400000  standalone=  400000
    standalone/heavy/find_blockquotes
                            time:   [13.561 ms 13.590 ms 13.623 ms]
                            thrpt:  [10.768 GiB/s 10.794 GiB/s 10.817 GiB/s]
        find_bullet_items  full=  600000  standalone=  600000
    standalone/heavy/find_bullet_items
                            time:   [24.669 ms 24.698 ms 24.727 ms]
                            thrpt:  [5.9322 GiB/s 5.9391 GiB/s 5.9462 GiB/s]
        find_ordered_items full=  400000  standalone=  400000
    standalone/heavy/find_ordered_items
                            time:   [22.052 ms 22.132 ms 22.220 ms]
                            thrpt:  [6.6014 GiB/s 6.6276 GiB/s 6.6519 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    │  context regions: 2400000
    
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 13.8s, enable flat sampling, or reduce sample count to 10.
    standalone/heavy/context
                            time:   [65.456 ms 65.707 ms 66.096 ms]
                            thrpt:  [2.2193 GiB/s 2.2324 GiB/s 2.2410 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low mild
      2 (10.00%) high severe
    │  full-vs-context-aware counts:
        find_context_italics full= 1200000  context-aware= 1200000
    standalone/heavy/find_context_italics
                            time:   [42.337 ms 42.457 ms 42.579 ms]
                            thrpt:  [3.4450 GiB/s 3.4549 GiB/s 3.4647 GiB/s]
        find_context_bolds full= 1200000  context-aware= 1200000
    
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 10.3s, enable flat sampling, or reduce sample count to 10.
    standalone/heavy/find_context_bolds
                            time:   [48.916 ms 48.991 ms 49.071 ms]
                            thrpt:  [2.9892 GiB/s 2.9941 GiB/s 2.9987 GiB/s]
        find_context_bold_italics full=  600000  context-aware=  600000
    standalone/heavy/find_context_bold_italics
                            time:   [40.051 ms 40.126 ms 40.232 ms]
                            thrpt:  [3.6460 GiB/s 3.6556 GiB/s 3.6624 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_context_headings full=  200000  context-aware=  200000
    standalone/heavy/find_context_headings
                            time:   [10.879 ms 10.922 ms 10.959 ms]
                            thrpt:  [13.385 GiB/s 13.430 GiB/s 13.483 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_context_thematic_breaks full=  200000  context-aware=  200000
    standalone/heavy/find_context_thematic_breaks
                            time:   [33.317 ms 33.373 ms 33.416 ms]
                            thrpt:  [4.3896 GiB/s 4.3954 GiB/s 4.4028 GiB/s]
        find_context_blockquotes full=  400000  context-aware=  400000
    standalone/heavy/find_context_blockquotes
                            time:   [14.814 ms 14.857 ms 14.900 ms]
                            thrpt:  [9.8443 GiB/s 9.8728 GiB/s 9.9016 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_context_bullet_items full=  600000  context-aware=  600000
    standalone/heavy/find_context_bullet_items
                            time:   [29.615 ms 29.672 ms 29.749 ms]
                            thrpt:  [4.9307 GiB/s 4.9435 GiB/s 4.9530 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_context_ordered_items full=  400000  context-aware=  400000
    standalone/heavy/find_context_ordered_items
                            time:   [26.696 ms 26.738 ms 26.782 ms]
                            thrpt:  [5.4770 GiB/s 5.4860 GiB/s 5.4946 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    standalone/heavy/find_context_italics_cold
                            time:   [108.22 ms 108.77 ms 109.45 ms]
                            thrpt:  [1.3402 GiB/s 1.3486 GiB/s 1.3554 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    standalone/heavy/find_context_bolds_cold
                            time:   [114.62 ms 114.90 ms 115.20 ms]
                            thrpt:  [1.2734 GiB/s 1.2766 GiB/s 1.2797 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low mild
      2 (10.00%) high mild
    standalone/heavy/find_context_bold_italics_cold
                            time:   [104.91 ms 105.29 ms 105.67 ms]
                            thrpt:  [1.3881 GiB/s 1.3932 GiB/s 1.3982 GiB/s]
    
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 16.2s, enable flat sampling, or reduce sample count to 10.
    standalone/heavy/find_context_headings_cold
                            time:   [76.269 ms 76.753 ms 77.313 ms]
                            thrpt:  [1.8973 GiB/s 1.9111 GiB/s 1.9233 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    standalone/heavy/find_context_thematic_breaks_cold
                            time:   [98.805 ms 99.215 ms 99.661 ms]
                            thrpt:  [1.4718 GiB/s 1.4785 GiB/s 1.4846 GiB/s]
    
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 16.9s, enable flat sampling, or reduce sample count to 10.
    standalone/heavy/find_context_blockquotes_cold
                            time:   [80.097 ms 80.344 ms 80.663 ms]
                            thrpt:  [1.8185 GiB/s 1.8257 GiB/s 1.8313 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low mild
      1 (5.00%) high mild
    
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 20.0s, enable flat sampling, or reduce sample count to 10.
    standalone/heavy/find_context_bullet_items_cold
                            time:   [95.077 ms 95.429 ms 95.896 ms]
                            thrpt:  [1.5296 GiB/s 1.5371 GiB/s 1.5428 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      3 (15.00%) high mild
    
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 19.3s, enable flat sampling, or reduce sample count to 10.
    standalone/heavy/find_context_ordered_items_cold
                            time:   [91.932 ms 92.267 ms 92.693 ms]
                            thrpt:  [1.5825 GiB/s 1.5898 GiB/s 1.5956 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    ```

</details>

---

## meon-md context-aware extraction (`context()` + `find_context_*`)

`context(source)` builds the opaque-region map — fenced blocks, code spans,
autolinks and fully opaque links/images — in a forward scan with bounded
searches; `find_context_*` is the same standalone
matcher with candidate delimiters inside those regions skipped and line
structure respected. Transparent symmetric/asymmetric and non-fence line/block
rules get a variant; the opaque sources keep only their
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
