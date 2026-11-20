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

### Cold and warmed scenarios

`meon-md_parse` is the **isolated** scenario: only `meon-md` is timed, and no
cross-parser sanity pass or other parser's timed benchmark runs before it. The
shared composition report still runs before timing, but the measured process is
otherwise meon-only.

`meon-md_compare` is the **warmed** scenario: all three parsers run in the same
process, and a one-time sanity pass runs all three before timing. The allocator,
pages and caches are therefore shared and already warm. `pulldown-cmark` and
`comrak` are measured under the same warmed conditions as `meon-md`, so the
three-way comparison inside `_compare` is internally consistent.

Absolute numbers from `_parse` and `_compare` answer different questions and are
not interchangeable. Use `_parse` for an isolated single-parser run and
`_compare` for the warmed multi-parser scenario.

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

**small (`--features benches/small`):**

| Corpus  | `meon-md`                  | `pulldown-cmark`           | `comrak`                   |
|---------|----------------------------|----------------------------|----------------------------|
| `plain` | [890.17 µs]=[3.0736 GiB/s] | [3.1841 ms]=[879.92 MiB/s] | [10.763 ms]=[260.32 MiB/s] |
| `hot`   | [807.64 µs]=[980.79 MiB/s] | [4.9859 ms]=[158.87 MiB/s] | [18.617 ms]=[42.549 MiB/s] |
| `heavy` | [1.8638 ms]=[805.92 MiB/s] | [13.088 ms]=[114.76 MiB/s] | [46.466 ms]=[32.326 MiB/s] |

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
    compare/plain/meon-md   time:   [889.23 µs 890.17 µs 891.17 µs]
                            thrpt:  [3.0702 GiB/s 3.0736 GiB/s 3.0769 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    compare/plain/pulldown-cmark
                            time:   [3.1790 ms 3.1841 ms 3.1903 ms]
                            thrpt:  [878.20 MiB/s 879.92 MiB/s 881.32 MiB/s]
    compare/plain/comrak    time:   [10.739 ms 10.763 ms 10.783 ms]
                            thrpt:  [259.84 MiB/s 260.32 MiB/s 260.90 MiB/s]
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
    compare/hot/meon-md     time:   [806.83 µs 807.64 µs 808.67 µs]
                            thrpt:  [979.54 MiB/s 980.79 MiB/s 981.77 MiB/s]
    compare/hot/pulldown-cmark
                            time:   [4.9795 ms 4.9859 ms 4.9963 ms]
                            thrpt:  [158.54 MiB/s 158.87 MiB/s 159.08 MiB/s]
    compare/hot/comrak      time:   [18.578 ms 18.617 ms 18.666 ms]
                            thrpt:  [42.437 MiB/s 42.549 MiB/s 42.637 MiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low mild
      2 (10.00%) high severe
    
    
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
    compare/heavy/meon-md   time:   [1.8619 ms 1.8638 ms 1.8654 ms]
                            thrpt:  [805.22 MiB/s 805.92 MiB/s 806.73 MiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) low mild
      1 (5.00%) high severe
    compare/heavy/pulldown-cmark
                            time:   [13.071 ms 13.088 ms 13.104 ms]
                            thrpt:  [114.63 MiB/s 114.76 MiB/s 114.92 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    compare/heavy/comrak    time:   [46.400 ms 46.466 ms 46.542 ms]
                            thrpt:  [32.273 MiB/s 32.326 MiB/s 32.372 MiB/s]
    ```

</details

**big (`--features benches/big`):**

| Corpus  | `meon-md`                  | `pulldown-cmark`           | `comrak`                  |
|---------|----------------------------|----------------------------|---------------------------|
| `plain` | [92.553 ms]=[2.9562 GiB/s] | [522.84 ms]=[535.87 MiB/s] | [2.3472 s]=[119.36 MiB/s] |
| `hot`   | [81.157 ms]=[976.04 MiB/s] | [916.39 ms]=[86.440 MiB/s] | [3.4414 s]=[23.018 MiB/s] |
| `heavy` | [185.88 ms]=[808.08 MiB/s] | [2.1525 s]=[69.781 MiB/s]  | [7.8217 s]=[19.204 MiB/s] |

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
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 19.7s, enable flat sampling, or reduce sample count to 10.
    compare/plain/meon-md   time:   [92.480 ms 92.553 ms 92.626 ms]
                            thrpt:  [2.9539 GiB/s 2.9562 GiB/s 2.9585 GiB/s]
                     change:
                            time:   [+10290% +10307% +10321%] (p = 0.00 < 0.05)
                            thrpt:  [-99.040% -99.039% -99.038%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low severe
      1 (5.00%) low mild
    Benchmarking compare/plain/pulldown-cmark: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 10.4s, or reduce sample count to 10.
    compare/plain/pulldown-cmark
                            time:   [521.82 ms 522.84 ms 524.05 ms]
                            thrpt:  [534.62 MiB/s 535.87 MiB/s 536.91 MiB/s]
                     change:
                            time:   [+16264% +16302% +16345%] (p = 0.00 < 0.05)
                            thrpt:  [-99.392% -99.390% -99.389%]
                            Performance has regressed.
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) high mild
      2 (10.00%) high severe
    Benchmarking compare/plain/comrak: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 45.4s, or reduce sample count to 10.
    compare/plain/comrak    time:   [2.3357 s 2.3472 s 2.3582 s]
                            thrpt:  [118.80 MiB/s 119.36 MiB/s 119.95 MiB/s]
                     change:
                            time:   [+21514% +21695% +21849%] (p = 0.00 < 0.05)
                            thrpt:  [-99.544% -99.541% -99.537%]
                            Performance has regressed.
    
    
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
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 17.0s, enable flat sampling, or reduce sample count to 10.
    compare/hot/meon-md     time:   [81.065 ms 81.157 ms 81.299 ms]
                            thrpt:  [974.33 MiB/s 976.04 MiB/s 977.14 MiB/s]
                     change:
                            time:   [+9925.3% +9945.0% +9965.0%] (p = 0.00 < 0.05)
                            thrpt:  [-99.006% -99.004% -99.003%]
                            Performance has regressed.
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) high mild
      1 (5.00%) high severe
    Benchmarking compare/hot/pulldown-cmark: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 18.2s, or reduce sample count to 10.
    compare/hot/pulldown-cmark
                            time:   [913.66 ms 916.39 ms 919.57 ms]
                            thrpt:  [86.141 MiB/s 86.440 MiB/s 86.697 MiB/s]
                     change:
                            time:   [+18166% +18228% +18303%] (p = 0.00 < 0.05)
                            thrpt:  [-99.457% -99.454% -99.453%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    Benchmarking compare/hot/comrak: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 70.9s, or reduce sample count to 10.
    compare/hot/comrak      time:   [3.4322 s 3.4414 s 3.4503 s]
                            thrpt:  [22.958 MiB/s 23.018 MiB/s 23.079 MiB/s]
                     change:
                            time:   [+18140% +18301% +18440%] (p = 0.00 < 0.05)
                            thrpt:  [-99.461% -99.457% -99.452%]
                            Performance has regressed.
    
    
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
    compare/heavy/meon-md   time:   [185.67 ms 185.88 ms 186.11 ms]
                            thrpt:  [807.09 MiB/s 808.08 MiB/s 809.00 MiB/s]
                     change:
                            time:   [+9860.0% +9875.4% +9889.7%] (p = 0.00 < 0.05)
                            thrpt:  [-98.999% -98.998% -98.996%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    Benchmarking compare/heavy/pulldown-cmark: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 42.9s, or reduce sample count to 10.
    compare/heavy/pulldown-cmark
                            time:   [2.1499 s 2.1525 s 2.1555 s]
                            thrpt:  [69.685 MiB/s 69.781 MiB/s 69.866 MiB/s]
                     change:
                            time:   [+16267% +16328% +16375%] (p = 0.00 < 0.05)
                            thrpt:  [-99.393% -99.391% -99.389%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    Benchmarking compare/heavy/comrak: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 150.6s, or reduce sample count to 10.
    compare/heavy/comrak    time:   [7.7544 s 7.8217 s 7.8866 s]
                            thrpt:  [19.046 MiB/s 19.204 MiB/s 19.370 MiB/s]
                     change:
                            time:   [+16585% +16714% +16853%] (p = 0.00 < 0.05)
                            thrpt:  [-99.410% -99.405% -99.401%]
                            Performance has regressed.
    Found 5 outliers among 20 measurements (25.00%)
      2 (10.00%) low severe
      3 (15.00%) high severe
    ```

</details>

### nightly - `RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_compare --features meon/avx2`

> meon on AVX2; `pulldown-cmark` and `comrak` scalar (no `simd` feature). The
> meon column is AVX2 against scalar comparators — not a like-for-like SIMD row.

**small (`--features benches/small`):**

| Corpus  | `meon-md`                  | `pulldown-cmark`           | `comrak`                   |
|---------|----------------------------|----------------------------|----------------------------|
| `plain` | [620.12 µs]=[4.4121 GiB/s] | [3.7218 ms]=[752.79 MiB/s] | [10.781 ms]=[259.86 MiB/s] |
| `hot`   | [629.83 µs]=[1.2282 GiB/s] | [5.1178 ms]=[154.78 MiB/s] | [18.606 ms]=[42.573 MiB/s] |
| `heavy` | [1.6149 ms]=[930.10 MiB/s] | [13.171 ms]=[114.04 MiB/s] | [45.161 ms]=[33.260 MiB/s] |

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
    compare/plain/meon-md   time:   [619.43 µs 620.12 µs 620.87 µs]
                            thrpt:  [4.4068 GiB/s 4.4121 GiB/s 4.4171 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    compare/plain/pulldown-cmark
                            time:   [3.7183 ms 3.7218 ms 3.7269 ms]
                            thrpt:  [751.75 MiB/s 752.79 MiB/s 753.49 MiB/s]
    compare/plain/comrak    time:   [10.752 ms 10.781 ms 10.813 ms]
                            thrpt:  [259.11 MiB/s 259.86 MiB/s 260.57 MiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) high mild
      2 (10.00%) high severe
    
    
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
    compare/hot/meon-md     time:   [629.31 µs 629.83 µs 630.55 µs]
                            thrpt:  [1.2268 GiB/s 1.2282 GiB/s 1.2292 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
    compare/hot/pulldown-cmark
                            time:   [5.1114 ms 5.1178 ms 5.1257 ms]
                            thrpt:  [154.54 MiB/s 154.78 MiB/s 154.97 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    compare/hot/comrak      time:   [18.544 ms 18.606 ms 18.664 ms]
                            thrpt:  [42.441 MiB/s 42.573 MiB/s 42.715 MiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low severe
      1 (5.00%) low mild
      1 (5.00%) high severe
    
    
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
    compare/heavy/meon-md   time:   [1.6138 ms 1.6149 ms 1.6161 ms]
                            thrpt:  [929.44 MiB/s 930.10 MiB/s 930.76 MiB/s]
    compare/heavy/pulldown-cmark
                            time:   [13.156 ms 13.171 ms 13.186 ms]
                            thrpt:  [113.91 MiB/s 114.04 MiB/s 114.17 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    compare/heavy/comrak    time:   [45.122 ms 45.161 ms 45.197 ms]
                            thrpt:  [33.233 MiB/s 33.260 MiB/s 33.289 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    ```

</details>

**big (`--features benches/big`):**

| Corpus  | `meon-md`                  | `pulldown-cmark`           | `comrak`                  |
|---------|----------------------------|----------------------------|---------------------------|
| `plain` | [65.477 ms]=[4.1786 GiB/s] | [527.45 ms]=[531.17 MiB/s] | [2.2925 s]=[122.21 MiB/s] |
| `hot`   | [62.939 ms]=[1.2291 GiB/s] | [948.93 ms]=[83.475 MiB/s] | [3.3856 s]=[23.397 MiB/s] |
| `heavy` | [158.05 ms]=[950.39 MiB/s] | [2.1538 s]=[69.741 MiB/s]  | [7.6604 s]=[19.608 MiB/s] |

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
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 13.8s, enable flat sampling, or reduce sample count to 10.
    compare/plain/meon-md   time:   [65.418 ms 65.477 ms 65.531 ms]
                            thrpt:  [4.1752 GiB/s 4.1786 GiB/s 4.1824 GiB/s]
                     change:
                            time:   [+10433% +10451% +10467%] (p = 0.00 < 0.05)
                            thrpt:  [-99.054% -99.052% -99.051%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low severe
      1 (5.00%) low mild
    Benchmarking compare/plain/pulldown-cmark: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 10.5s, or reduce sample count to 10.
    compare/plain/pulldown-cmark
                            time:   [526.38 ms 527.45 ms 528.84 ms]
                            thrpt:  [529.78 MiB/s 531.17 MiB/s 532.26 MiB/s]
                     change:
                            time:   [+13978% +14022% +14067%] (p = 0.00 < 0.05)
                            thrpt:  [-99.294% -99.292% -99.290%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    Benchmarking compare/plain/comrak: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 45.3s, or reduce sample count to 10.
    compare/plain/comrak    time:   [2.2777 s 2.2925 s 2.3079 s]
                            thrpt:  [121.40 MiB/s 122.21 MiB/s 123.01 MiB/s]
                     change:
                            time:   [+20755% +20988% +21216%] (p = 0.00 < 0.05)
                            thrpt:  [-99.531% -99.526% -99.520%]
                            Performance has regressed.
    
    
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
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 13.2s, enable flat sampling, or reduce sample count to 10.
    compare/hot/meon-md     time:   [62.852 ms 62.939 ms 63.010 ms]
                            thrpt:  [1.2277 GiB/s 1.2291 GiB/s 1.2308 GiB/s]
                     change:
                            time:   [+9857.5% +9873.8% +9889.1%] (p = 0.00 < 0.05)
                            thrpt:  [-98.999% -98.997% -98.996%]
                            Performance has regressed.
    Benchmarking compare/hot/pulldown-cmark: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 19.0s, or reduce sample count to 10.
    compare/hot/pulldown-cmark
                            time:   [946.89 ms 948.93 ms 952.09 ms]
                            thrpt:  [83.198 MiB/s 83.475 MiB/s 83.655 MiB/s]
                     change:
                            time:   [+18342% +18405% +18473%] (p = 0.00 < 0.05)
                            thrpt:  [-99.462% -99.460% -99.458%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    Benchmarking compare/hot/comrak: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 67.8s, or reduce sample count to 10.
    compare/hot/comrak      time:   [3.3777 s 3.3856 s 3.3937 s]
                            thrpt:  [23.341 MiB/s 23.397 MiB/s 23.452 MiB/s]
                     change:
                            time:   [+17915% +18057% +18175%] (p = 0.00 < 0.05)
                            thrpt:  [-99.453% -99.449% -99.445%]
                            Performance has regressed.
    
    
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
    compare/heavy/meon-md   time:   [158.00 ms 158.05 ms 158.10 ms]
                            thrpt:  [950.09 MiB/s 950.39 MiB/s 950.67 MiB/s]
                     change:
                            time:   [+9681.0% +9689.6% +9697.8%] (p = 0.00 < 0.05)
                            thrpt:  [-98.979% -98.979% -98.978%]
                            Performance has regressed.
    Benchmarking compare/heavy/pulldown-cmark: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 43.0s, or reduce sample count to 10.
    compare/heavy/pulldown-cmark
                            time:   [2.1521 s 2.1538 s 2.1558 s]
                            thrpt:  [69.676 MiB/s 69.741 MiB/s 69.796 MiB/s]
                     change:
                            time:   [+16184% +16237% +16280%] (p = 0.00 < 0.05)
                            thrpt:  [-99.389% -99.388% -99.386%]
                            Performance has regressed.
    Found 5 outliers among 20 measurements (25.00%)
      1 (5.00%) low severe
      2 (10.00%) low mild
      1 (5.00%) high mild
      1 (5.00%) high severe
    Benchmarking compare/heavy/comrak: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 154.9s, or reduce sample count to 10.
    compare/heavy/comrak    time:   [7.5950 s 7.6604 s 7.7210 s]
                            thrpt:  [19.454 MiB/s 19.608 MiB/s 19.777 MiB/s]
                     change:
                            time:   [+16697% +16842% +16993%] (p = 0.00 < 0.05)
                            thrpt:  [-99.415% -99.410% -99.405%]
                            Performance has regressed.
    Found 4 outliers among 20 measurements (20.00%)
      2 (10.00%) low severe
      2 (10.00%) high severe
    ```

</details>

---
  
## Scaling from small to big

The clearest expression of the architecture difference is how each parser holds
up as the input grows past cache (stable build, median `thrpt`):

| Parser              | `plain`              | `hot`                | `heavy`            |
|---------------------|----------------------|----------------------|--------------------|
| `meon-md` - stable  | 3.07 -> 2.96 GiB/s   | 981 -> 976 MiB/s     | 806 -> 808 MiB/s   |
| `meon-md` - nightly | 4.41 -> 4.18 GiB/s   | 1.23 -> 1.23 GiB/s   | 930 -> 950 MiB/s   |
| `pulldown-cmark`    | 880 -> 536 MiB/s     | 159 -> 86 MiB/s      | 115 -> 70 MiB/s    |
| `comrak`            | 260 -> 119 MiB/s     | 43 -> 23 MiB/s       | 32 -> 19 MiB/s     |

- **`meon-md` holds throughput nearly flat** from small to big: stable stays
  within ~4% (`heavy` even ticks up slightly), nightly within ~6%. The output
  is a compact, contiguous span table (`u32` pairs), so the working set stays
  cache-friendly as the document grows.
- **`pulldown-cmark` loses ~39–46%** at big — event-stream bookkeeping plus a
  growing working set push past cache.
- **`comrak` loses ~41–54%** and is slowest in absolute terms throughout — it
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

**small (`--features benches/small`):**

#### Standalone - `find_*`
| Function               | `plain`                    | `hot`                      | `heavy`                    |
|------------------------|----------------------------|----------------------------|----------------------------|
| `find_codes`           | [29.554 µs]=[92.577 GiB/s] | [88.294 µs]=[8.7612 GiB/s] | [237.94 µs]=[6.1649 GiB/s] |
| `find_italics`         | [31.335 µs]=[87.314 GiB/s] | [147.20 µs]=[5.2551 GiB/s] | [423.07 µs]=[3.4672 GiB/s] |
| `find_bolds`           | [29.678 µs]=[92.191 GiB/s] | [151.86 µs]=[5.0939 GiB/s] | [423.72 µs]=[3.4619 GiB/s] |
| `find_bold_italics`    | [31.107 µs]=[87.955 GiB/s] | [126.61 µs]=[6.1096 GiB/s] | [397.31 µs]=[3.6920 GiB/s] |
| `find_autolinks`       | [29.855 µs]=[91.645 GiB/s] | [91.243 µs]=[8.4780 GiB/s] | [77.281 µs]=[18.981 GiB/s] |
| `find_links`           | [30.547 µs]=[89.569 GiB/s] | [207.19 µs]=[3.7335 GiB/s] | [254.35 µs]=[5.7671 GiB/s] |
| `find_headings`        | [33.416 µs]=[81.877 GiB/s] | [90.962 µs]=[8.5042 GiB/s] | [45.081 µs]=[32.538 GiB/s] |
| `find_thematic_breaks` | [42.974 µs]=[63.668 GiB/s] | [78.608 µs]=[9.8407 GiB/s] | [295.79 µs]=[4.9591 GiB/s] |
| `find_fenced_codes`    | [31.670 µs]=[86.391 GiB/s] | [66.707 µs]=[11.596 GiB/s] | [249.60 µs]=[5.8769 GiB/s] |
| `find_blockquotes`     | [32.498 µs]=[84.190 GiB/s] | [38.530 µs]=[20.077 GiB/s] | [119.84 µs]=[12.240 GiB/s] |
| `find_bullet_items`    | [43.286 µs]=[63.209 GiB/s] | [75.913 µs]=[10.190 GiB/s] | [276.21 µs]=[5.3106 GiB/s] |
| `find_ordered_items`   | [275.29 µs]=[9.9389 GiB/s] | [101.07 µs]=[7.6538 GiB/s] | [228.44 µs]=[6.4212 GiB/s] |
 
#### Context-aware - `context()` + `find_context_*`
| Function                       | `plain`                    | `hot`                      | `heavy`                    |
|--------------------------------|----------------------------|----------------------------|----------------------------|
| `context`                      | [47.959 µs]=[57.050 GiB/s] | [507.89 µs]=[1.5231 GiB/s] | [852.85 µs]=[1.7199 GiB/s] |
| `find_context_italics`         | [31.171 µs]=[87.775 GiB/s] | [180.76 µs]=[4.2794 GiB/s] | [507.01 µs]=[2.8931 GiB/s] |
| `find_context_bolds`           | [31.498 µs]=[86.863 GiB/s] | [183.65 µs]=[4.2122 GiB/s] | [568.92 µs]=[2.5783 GiB/s] |
| `find_context_bold_italics`    | [31.965 µs]=[85.596 GiB/s] | [151.84 µs]=[5.0946 GiB/s] | [476.28 µs]=[3.0798 GiB/s] |
| `find_context_headings`        | [28.353 µs]=[96.498 GiB/s] | [115.52 µs]=[6.6965 GiB/s] | [62.814 µs]=[23.352 GiB/s] |
| `find_context_thematic_breaks` | [42.475 µs]=[64.416 GiB/s] | [77.943 µs]=[9.9247 GiB/s] | [337.68 µs]=[4.3439 GiB/s] |
| `find_context_blockquotes`     | [33.067 µs]=[82.741 GiB/s] | [38.716 µs]=[19.980 GiB/s] | [128.74 µs]=[11.394 GiB/s] |
| `find_context_bullet_items`    | [42.960 µs]=[63.689 GiB/s] | [76.394 µs]=[10.126 GiB/s] | [323.29 µs]=[4.5373 GiB/s] |
| `find_context_ordered_items`   | [275.22 µs]=[9.9414 GiB/s] | [101.25 µs]=[7.6401 GiB/s] | [272.31 µs]=[5.3867 GiB/s] |
 
#### Cold context - `find_context_*_cold`
| Function                            | `plain`                    | `hot`                      | `heavy`                    |
|-------------------------------------|----------------------------|----------------------------|----------------------------|
| `find_context_italics_cold`         | [77.963 µs]=[35.094 GiB/s] | [690.65 µs]=[1.1200 GiB/s] | [1.3478 ms]=[1.0883 GiB/s] |
| `find_context_bolds_cold`           | [79.470 µs]=[34.428 GiB/s] | [693.30 µs]=[1.1158 GiB/s] | [1.4058 ms]=[1.0434 GiB/s] |
| `find_context_bold_italics_cold`    | [75.353 µs]=[36.310 GiB/s] | [662.74 µs]=[1.1672 GiB/s] | [1.3207 ms]=[1.1107 GiB/s] |
| `find_context_headings_cold`        | [84.787 µs]=[32.270 GiB/s] | [630.75 µs]=[1.2264 GiB/s] | [907.05 µs]=[1.6172 GiB/s] |
| `find_context_thematic_breaks_cold` | [85.891 µs]=[31.855 GiB/s] | [584.06 µs]=[1.3244 GiB/s] | [1.1784 ms]=[1.2448 GiB/s] |
| `find_context_blockquotes_cold`     | [79.582 µs]=[34.380 GiB/s] | [545.22 µs]=[1.4188 GiB/s] | [977.95 µs]=[1.4999 GiB/s] |
| `find_context_bullet_items_cold`    | [82.141 µs]=[33.309 GiB/s] | [585.30 µs]=[1.3216 GiB/s] | [1.1650 ms]=[1.2591 GiB/s] |
| `find_context_ordered_items_cold`   | [346.44 µs]=[7.8976 GiB/s] | [610.11 µs]=[1.2679 GiB/s] | [1.1153 ms]=[1.3152 GiB/s] |
 


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
                            time:   [28.746 µs 29.554 µs 30.320 µs]
                            thrpt:  [90.240 GiB/s 92.577 GiB/s 95.179 GiB/s]
        find_italics       full=       0  standalone=       0
    standalone/plain/find_italics
                            time:   [30.083 µs 31.335 µs 32.690 µs]
                            thrpt:  [83.698 GiB/s 87.314 GiB/s 90.949 GiB/s]
        find_bolds         full=       0  standalone=       0
    standalone/plain/find_bolds
                            time:   [28.766 µs 29.678 µs 30.880 µs]
                            thrpt:  [88.603 GiB/s 92.191 GiB/s 95.114 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
        find_bold_italics  full=       0  standalone=       0
    standalone/plain/find_bold_italics
                            time:   [29.936 µs 31.107 µs 32.318 µs]
                            thrpt:  [84.660 GiB/s 87.955 GiB/s 91.395 GiB/s]
        find_autolinks     full=       0  standalone=       0
    standalone/plain/find_autolinks
                            time:   [29.118 µs 29.855 µs 30.906 µs]
                            thrpt:  [88.529 GiB/s 91.645 GiB/s 93.963 GiB/s]
        find_links         full=       0  standalone=       0
    standalone/plain/find_links
                            time:   [29.406 µs 30.547 µs 31.584 µs]
                            thrpt:  [86.627 GiB/s 89.569 GiB/s 93.044 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_headings      full=       0  standalone=       0
    standalone/plain/find_headings
                            time:   [32.230 µs 33.416 µs 34.895 µs]
                            thrpt:  [78.408 GiB/s 81.877 GiB/s 84.890 GiB/s]
        find_thematic_breaks full=       0  standalone=       0
    standalone/plain/find_thematic_breaks
                            time:   [42.812 µs 42.974 µs 43.120 µs]
                            thrpt:  [63.452 GiB/s 63.668 GiB/s 63.908 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_fenced_codes  full=       0  standalone=       0
    standalone/plain/find_fenced_codes
                            time:   [30.542 µs 31.670 µs 32.827 µs]
                            thrpt:  [83.347 GiB/s 86.391 GiB/s 89.581 GiB/s]
        find_blockquotes   full=       0  standalone=       0
    standalone/plain/find_blockquotes
                            time:   [30.736 µs 32.498 µs 33.961 µs]
                            thrpt:  [80.564 GiB/s 84.190 GiB/s 89.017 GiB/s]
        find_bullet_items  full=       0  standalone=       0
    standalone/plain/find_bullet_items
                            time:   [42.094 µs 43.286 µs 44.309 µs]
                            thrpt:  [61.749 GiB/s 63.209 GiB/s 64.998 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) low mild
        find_ordered_items full=       0  standalone=       0
    standalone/plain/find_ordered_items
                            time:   [275.10 µs 275.29 µs 275.49 µs]
                            thrpt:  [9.9314 GiB/s 9.9389 GiB/s 9.9455 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    │  context regions: 0
    standalone/plain/context
                            time:   [47.358 µs 47.959 µs 48.641 µs]
                            thrpt:  [56.250 GiB/s 57.050 GiB/s 57.773 GiB/s]
    │  full-vs-context-aware counts:
        find_context_italics full=       0  context-aware=       0
    standalone/plain/find_context_italics
                            time:   [30.194 µs 31.171 µs 32.121 µs]
                            thrpt:  [85.179 GiB/s 87.775 GiB/s 90.616 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_context_bolds full=       0  context-aware=       0
    standalone/plain/find_context_bolds
                            time:   [30.551 µs 31.498 µs 32.238 µs]
                            thrpt:  [84.870 GiB/s 86.863 GiB/s 89.555 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
        find_context_bold_italics full=       0  context-aware=       0
    standalone/plain/find_context_bold_italics
                            time:   [30.603 µs 31.965 µs 33.590 µs]
                            thrpt:  [81.454 GiB/s 85.596 GiB/s 89.404 GiB/s]
        find_context_headings full=       0  context-aware=       0
    standalone/plain/find_context_headings
                            time:   [27.945 µs 28.353 µs 28.850 µs]
                            thrpt:  [94.836 GiB/s 96.498 GiB/s 97.908 GiB/s]
        find_context_thematic_breaks full=       0  context-aware=       0
    standalone/plain/find_context_thematic_breaks
                            time:   [41.660 µs 42.475 µs 43.164 µs]
                            thrpt:  [63.387 GiB/s 64.416 GiB/s 65.676 GiB/s]
        find_context_blockquotes full=       0  context-aware=       0
    standalone/plain/find_context_blockquotes
                            time:   [32.633 µs 33.067 µs 33.722 µs]
                            thrpt:  [81.135 GiB/s 82.741 GiB/s 83.843 GiB/s]
        find_context_bullet_items full=       0  context-aware=       0
    standalone/plain/find_context_bullet_items
                            time:   [41.933 µs 42.960 µs 44.106 µs]
                            thrpt:  [62.034 GiB/s 63.689 GiB/s 65.248 GiB/s]
        find_context_ordered_items full=       0  context-aware=       0
    standalone/plain/find_context_ordered_items
                            time:   [274.83 µs 275.22 µs 275.73 µs]
                            thrpt:  [9.9228 GiB/s 9.9414 GiB/s 9.9554 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    standalone/plain/find_context_italics_cold
                            time:   [74.742 µs 77.963 µs 80.519 µs]
                            thrpt:  [33.980 GiB/s 35.094 GiB/s 36.606 GiB/s]
    standalone/plain/find_context_bolds_cold
                            time:   [76.367 µs 79.470 µs 83.266 µs]
                            thrpt:  [32.859 GiB/s 34.428 GiB/s 35.827 GiB/s]
    standalone/plain/find_context_bold_italics_cold
                            time:   [73.316 µs 75.353 µs 77.467 µs]
                            thrpt:  [35.319 GiB/s 36.310 GiB/s 37.319 GiB/s]
    standalone/plain/find_context_headings_cold
                            time:   [81.464 µs 84.787 µs 87.361 µs]
                            thrpt:  [31.319 GiB/s 32.270 GiB/s 33.586 GiB/s]
    standalone/plain/find_context_thematic_breaks_cold
                            time:   [83.766 µs 85.891 µs 88.953 µs]
                            thrpt:  [30.758 GiB/s 31.855 GiB/s 32.663 GiB/s]
    standalone/plain/find_context_blockquotes_cold
                            time:   [78.085 µs 79.582 µs 81.353 µs]
                            thrpt:  [33.632 GiB/s 34.380 GiB/s 35.039 GiB/s]
    standalone/plain/find_context_bullet_items_cold
                            time:   [80.176 µs 82.141 µs 85.281 µs]
                            thrpt:  [32.082 GiB/s 33.309 GiB/s 34.125 GiB/s]
    standalone/plain/find_context_ordered_items_cold
                            time:   [337.90 µs 346.44 µs 355.48 µs]
                            thrpt:  [7.6968 GiB/s 7.8976 GiB/s 8.0972 GiB/s]
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
                            time:   [88.190 µs 88.294 µs 88.415 µs]
                            thrpt:  [8.7491 GiB/s 8.7612 GiB/s 8.7714 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_italics       full=    5000  standalone=    5000
    standalone/hot/find_italics
                            time:   [147.12 µs 147.20 µs 147.28 µs]
                            thrpt:  [5.2524 GiB/s 5.2551 GiB/s 5.2582 GiB/s]
        find_bolds         full=    5000  standalone=    5000
    standalone/hot/find_bolds
                            time:   [151.76 µs 151.86 µs 152.01 µs]
                            thrpt:  [5.0887 GiB/s 5.0939 GiB/s 5.0973 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_bold_italics  full=       0  standalone=       0
    standalone/hot/find_bold_italics
                            time:   [125.27 µs 126.61 µs 128.33 µs]
                            thrpt:  [6.0278 GiB/s 6.1096 GiB/s 6.1750 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low mild
      2 (10.00%) high severe
        find_autolinks     full=    5000  standalone=    5000
    standalone/hot/find_autolinks
                            time:   [91.036 µs 91.243 µs 91.372 µs]
                            thrpt:  [8.4660 GiB/s 8.4780 GiB/s 8.4973 GiB/s]
        find_links         full=    5000  standalone=    5000
    standalone/hot/find_links
                            time:   [207.03 µs 207.19 µs 207.34 µs]
                            thrpt:  [3.7309 GiB/s 3.7335 GiB/s 3.7365 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
        find_headings      full=    5000  standalone=    5000
    standalone/hot/find_headings
                            time:   [90.888 µs 90.962 µs 91.048 µs]
                            thrpt:  [8.4961 GiB/s 8.5042 GiB/s 8.5111 GiB/s]
        find_thematic_breaks full=       0  standalone=       0
    standalone/hot/find_thematic_breaks
                            time:   [78.548 µs 78.608 µs 78.670 µs]
                            thrpt:  [9.8329 GiB/s 9.8407 GiB/s 9.8482 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) low mild
        find_fenced_codes  full=       0  standalone=       0
    standalone/hot/find_fenced_codes
                            time:   [66.521 µs 66.707 µs 66.933 µs]
                            thrpt:  [11.557 GiB/s 11.596 GiB/s 11.629 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      3 (15.00%) high severe
        find_blockquotes   full=       0  standalone=       0
    standalone/hot/find_blockquotes
                            time:   [38.488 µs 38.530 µs 38.578 µs]
                            thrpt:  [20.052 GiB/s 20.077 GiB/s 20.099 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_bullet_items  full=       0  standalone=       0
    standalone/hot/find_bullet_items
                            time:   [75.849 µs 75.913 µs 75.967 µs]
                            thrpt:  [10.183 GiB/s 10.190 GiB/s 10.199 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_ordered_items full=       0  standalone=       0
    standalone/hot/find_ordered_items
                            time:   [101.03 µs 101.07 µs 101.11 µs]
                            thrpt:  [7.6505 GiB/s 7.6538 GiB/s 7.6565 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    │  context regions: 15000
    standalone/hot/context  time:   [507.72 µs 507.89 µs 508.04 µs]
                            thrpt:  [1.5226 GiB/s 1.5231 GiB/s 1.5236 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    │  full-vs-context-aware counts:
        find_context_italics full=    5000  context-aware=    5000
    standalone/hot/find_context_italics
                            time:   [180.65 µs 180.76 µs 180.90 µs]
                            thrpt:  [4.2762 GiB/s 4.2794 GiB/s 4.2820 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_context_bolds full=    5000  context-aware=    5000
    standalone/hot/find_context_bolds
                            time:   [183.56 µs 183.65 µs 183.77 µs]
                            thrpt:  [4.2094 GiB/s 4.2122 GiB/s 4.2143 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_context_bold_italics full=       0  context-aware=       0
    standalone/hot/find_context_bold_italics
                            time:   [151.67 µs 151.84 µs 152.06 µs]
                            thrpt:  [5.0872 GiB/s 5.0946 GiB/s 5.1003 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_context_headings full=    5000  context-aware=    5000
    standalone/hot/find_context_headings
                            time:   [115.44 µs 115.52 µs 115.58 µs]
                            thrpt:  [6.6928 GiB/s 6.6965 GiB/s 6.7008 GiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      2 (10.00%) low mild
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_context_thematic_breaks full=       0  context-aware=       0
    standalone/hot/find_context_thematic_breaks
                            time:   [77.891 µs 77.943 µs 77.991 µs]
                            thrpt:  [9.9185 GiB/s 9.9247 GiB/s 9.9313 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
        find_context_blockquotes full=       0  context-aware=       0
    standalone/hot/find_context_blockquotes
                            time:   [38.687 µs 38.716 µs 38.750 µs]
                            thrpt:  [19.963 GiB/s 19.980 GiB/s 19.995 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
        find_context_bullet_items full=       0  context-aware=       0
    standalone/hot/find_context_bullet_items
                            time:   [76.260 µs 76.394 µs 76.541 µs]
                            thrpt:  [10.106 GiB/s 10.126 GiB/s 10.144 GiB/s]
        find_context_ordered_items full=       0  context-aware=       0
    standalone/hot/find_context_ordered_items
                            time:   [101.19 µs 101.25 µs 101.32 µs]
                            thrpt:  [7.6344 GiB/s 7.6401 GiB/s 7.6447 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    standalone/hot/find_context_italics_cold
                            time:   [690.42 µs 690.65 µs 690.87 µs]
                            thrpt:  [1.1197 GiB/s 1.1200 GiB/s 1.1204 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    standalone/hot/find_context_bolds_cold
                            time:   [692.55 µs 693.30 µs 693.91 µs]
                            thrpt:  [1.1148 GiB/s 1.1158 GiB/s 1.1170 GiB/s]
    standalone/hot/find_context_bold_italics_cold
                            time:   [661.93 µs 662.74 µs 663.87 µs]
                            thrpt:  [1.1652 GiB/s 1.1672 GiB/s 1.1686 GiB/s]
    standalone/hot/find_context_headings_cold
                            time:   [629.94 µs 630.75 µs 631.26 µs]
                            thrpt:  [1.2254 GiB/s 1.2264 GiB/s 1.2280 GiB/s]
    standalone/hot/find_context_thematic_breaks_cold
                            time:   [583.88 µs 584.06 µs 584.26 µs]
                            thrpt:  [1.3240 GiB/s 1.3244 GiB/s 1.3249 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    standalone/hot/find_context_blockquotes_cold
                            time:   [544.70 µs 545.22 µs 545.87 µs]
                            thrpt:  [1.4171 GiB/s 1.4188 GiB/s 1.4202 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    standalone/hot/find_context_bullet_items_cold
                            time:   [585.02 µs 585.30 µs 585.53 µs]
                            thrpt:  [1.3211 GiB/s 1.3216 GiB/s 1.3223 GiB/s]
    standalone/hot/find_context_ordered_items_cold
                            time:   [609.74 µs 610.11 µs 610.50 µs]
                            thrpt:  [1.2671 GiB/s 1.2679 GiB/s 1.2687 GiB/s]
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
                            time:   [237.81 µs 237.94 µs 238.08 µs]
                            thrpt:  [6.1611 GiB/s 6.1649 GiB/s 6.1682 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) low severe
      1 (5.00%) low mild
        find_italics       full=   12000  standalone=   12000
    standalone/heavy/find_italics
                            time:   [422.83 µs 423.07 µs 423.22 µs]
                            thrpt:  [3.4659 GiB/s 3.4672 GiB/s 3.4691 GiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      4 (20.00%) low severe
        find_bolds         full=   12000  standalone=   12000
    standalone/heavy/find_bolds
                            time:   [423.02 µs 423.72 µs 424.26 µs]
                            thrpt:  [3.4574 GiB/s 3.4619 GiB/s 3.4676 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) low mild
        find_bold_italics  full=    6000  standalone=    6000
    standalone/heavy/find_bold_italics
                            time:   [396.61 µs 397.31 µs 397.95 µs]
                            thrpt:  [3.6860 GiB/s 3.6920 GiB/s 3.6985 GiB/s]
        find_autolinks     full=    4000  standalone=    4000
    standalone/heavy/find_autolinks
                            time:   [77.203 µs 77.281 µs 77.346 µs]
                            thrpt:  [18.965 GiB/s 18.981 GiB/s 19.000 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
        find_links         full=    6000  standalone=    6000
    standalone/heavy/find_links
                            time:   [254.19 µs 254.35 µs 254.51 µs]
                            thrpt:  [5.7634 GiB/s 5.7671 GiB/s 5.7708 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) low mild
      1 (5.00%) high mild
        find_headings      full=    2000  standalone=    2000
    standalone/heavy/find_headings
                            time:   [44.775 µs 45.081 µs 45.365 µs]
                            thrpt:  [32.334 GiB/s 32.538 GiB/s 32.760 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) low mild
      1 (5.00%) high mild
        find_thematic_breaks full=    2000  standalone=    2000
    standalone/heavy/find_thematic_breaks
                            time:   [295.60 µs 295.79 µs 295.94 µs]
                            thrpt:  [4.9566 GiB/s 4.9591 GiB/s 4.9622 GiB/s]
        find_fenced_codes  full=    4000  standalone=    4000
    standalone/heavy/find_fenced_codes
                            time:   [249.38 µs 249.60 µs 249.89 µs]
                            thrpt:  [5.8699 GiB/s 5.8769 GiB/s 5.8819 GiB/s]
        find_blockquotes   full=    4000  standalone=    4000
    standalone/heavy/find_blockquotes
                            time:   [119.67 µs 119.84 µs 120.06 µs]
                            thrpt:  [12.217 GiB/s 12.240 GiB/s 12.257 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_bullet_items  full=    6000  standalone=    6000
    standalone/heavy/find_bullet_items
                            time:   [276.05 µs 276.21 µs 276.36 µs]
                            thrpt:  [5.3078 GiB/s 5.3106 GiB/s 5.3136 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low severe
      2 (10.00%) low mild
        find_ordered_items full=    4000  standalone=    4000
    standalone/heavy/find_ordered_items
                            time:   [228.06 µs 228.44 µs 228.70 µs]
                            thrpt:  [6.4139 GiB/s 6.4212 GiB/s 6.4318 GiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      2 (10.00%) low severe
      1 (5.00%) low mild
      1 (5.00%) high mild
    │  context regions: 24000
    standalone/heavy/context
                            time:   [851.18 µs 852.85 µs 854.47 µs]
                            thrpt:  [1.7167 GiB/s 1.7199 GiB/s 1.7233 GiB/s]
    │  full-vs-context-aware counts:
        find_context_italics full=   12000  context-aware=   12000
    standalone/heavy/find_context_italics
                            time:   [506.07 µs 507.01 µs 507.96 µs]
                            thrpt:  [2.8877 GiB/s 2.8931 GiB/s 2.8985 GiB/s]
        find_context_bolds full=   12000  context-aware=   12000
    standalone/heavy/find_context_bolds
                            time:   [567.22 µs 568.92 µs 570.42 µs]
                            thrpt:  [2.5716 GiB/s 2.5783 GiB/s 2.5861 GiB/s]
        find_context_bold_italics full=    6000  context-aware=    6000
    standalone/heavy/find_context_bold_italics
                            time:   [475.20 µs 476.28 µs 477.36 µs]
                            thrpt:  [3.0729 GiB/s 3.0798 GiB/s 3.0868 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
        find_context_headings full=    2000  context-aware=    2000
    standalone/heavy/find_context_headings
                            time:   [62.443 µs 62.814 µs 63.103 µs]
                            thrpt:  [23.245 GiB/s 23.352 GiB/s 23.491 GiB/s]
        find_context_thematic_breaks full=    2000  context-aware=    2000
    standalone/heavy/find_context_thematic_breaks
                            time:   [336.94 µs 337.68 µs 338.39 µs]
                            thrpt:  [4.3348 GiB/s 4.3439 GiB/s 4.3534 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
        find_context_blockquotes full=    4000  context-aware=    4000
    standalone/heavy/find_context_blockquotes
                            time:   [128.55 µs 128.74 µs 129.03 µs]
                            thrpt:  [11.368 GiB/s 11.394 GiB/s 11.411 GiB/s]
        find_context_bullet_items full=    6000  context-aware=    6000
    standalone/heavy/find_context_bullet_items
                            time:   [322.75 µs 323.29 µs 323.99 µs]
                            thrpt:  [4.5275 GiB/s 4.5373 GiB/s 4.5448 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) high mild
      2 (10.00%) high severe
        find_context_ordered_items full=    4000  context-aware=    4000
    standalone/heavy/find_context_ordered_items
                            time:   [271.96 µs 272.31 µs 272.75 µs]
                            thrpt:  [5.3780 GiB/s 5.3867 GiB/s 5.3936 GiB/s]
    standalone/heavy/find_context_italics_cold
                            time:   [1.3463 ms 1.3478 ms 1.3499 ms]
                            thrpt:  [1.0866 GiB/s 1.0883 GiB/s 1.0895 GiB/s]
    standalone/heavy/find_context_bolds_cold
                            time:   [1.4053 ms 1.4058 ms 1.4065 ms]
                            thrpt:  [1.0429 GiB/s 1.0434 GiB/s 1.0438 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      3 (15.00%) high severe
    standalone/heavy/find_context_bold_italics_cold
                            time:   [1.3196 ms 1.3207 ms 1.3222 ms]
                            thrpt:  [1.1094 GiB/s 1.1107 GiB/s 1.1116 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    standalone/heavy/find_context_headings_cold
                            time:   [906.72 µs 907.05 µs 907.34 µs]
                            thrpt:  [1.6167 GiB/s 1.6172 GiB/s 1.6178 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    standalone/heavy/find_context_thematic_breaks_cold
                            time:   [1.1774 ms 1.1784 ms 1.1795 ms]
                            thrpt:  [1.2436 GiB/s 1.2448 GiB/s 1.2458 GiB/s]
    standalone/heavy/find_context_blockquotes_cold
                            time:   [977.65 µs 977.95 µs 978.28 µs]
                            thrpt:  [1.4994 GiB/s 1.4999 GiB/s 1.5004 GiB/s]
    standalone/heavy/find_context_bullet_items_cold
                            time:   [1.1644 ms 1.1650 ms 1.1657 ms]
                            thrpt:  [1.2584 GiB/s 1.2591 GiB/s 1.2598 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
    standalone/heavy/find_context_ordered_items_cold
                            time:   [1.1148 ms 1.1153 ms 1.1160 ms]
                            thrpt:  [1.3144 GiB/s 1.3152 GiB/s 1.3159 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    ```

</details>

**big (`--features benches/big`):**

#### Standalone - `find_*`
| Function               | `plain`                    | `hot`                      | `heavy`                    |
|------------------------|----------------------------|----------------------------|----------------------------|
| `find_codes`           | [12.429 ms]=[22.013 GiB/s] | [10.121 ms]=[7.6428 GiB/s] | [24.901 ms]=[5.8907 GiB/s] |
| `find_italics`         | [12.380 ms]=[22.100 GiB/s] | [15.650 ms]=[4.9427 GiB/s] | [42.877 ms]=[3.4210 GiB/s] |
| `find_bolds`           | [12.436 ms]=[22.001 GiB/s] | [15.860 ms]=[4.8775 GiB/s] | [44.086 ms]=[3.3272 GiB/s] |
| `find_bold_italics`    | [12.422 ms]=[22.026 GiB/s] | [13.362 ms]=[5.7890 GiB/s] | [40.142 ms]=[3.6542 GiB/s] |
| `find_autolinks`       | [12.421 ms]=[22.027 GiB/s] | [10.202 ms]=[7.5822 GiB/s] | [10.853 ms]=[13.515 GiB/s] |
| `find_links`           | [12.484 ms]=[21.916 GiB/s] | [21.093 ms]=[3.6674 GiB/s] | [26.536 ms]=[5.5277 GiB/s] |
| `find_headings`        | [12.452 ms]=[21.973 GiB/s] | [10.147 ms]=[7.6235 GiB/s] | [8.8670 ms]=[16.543 GiB/s] |
| `find_thematic_breaks` | [14.422 ms]=[18.971 GiB/s] | [8.6318 ms]=[8.9617 GiB/s] | [29.924 ms]=[4.9018 GiB/s] |
| `find_fenced_codes`    | [12.315 ms]=[22.217 GiB/s] | [7.7077 ms]=[10.036 GiB/s] | [25.326 ms]=[5.7918 GiB/s] |
| `find_blockquotes`     | [12.344 ms]=[22.165 GiB/s] | [5.3076 ms]=[14.575 GiB/s] | [14.567 ms]=[10.070 GiB/s] |
| `find_bullet_items`    | [14.334 ms]=[19.088 GiB/s] | [8.5440 ms]=[9.0538 GiB/s] | [28.009 ms]=[5.2370 GiB/s] |
| `find_ordered_items`   | [30.654 ms]=[8.9255 GiB/s] | [11.084 ms]=[6.9793 GiB/s] | [23.849 ms]=[6.1505 GiB/s] |

#### Context-aware - `context()` + `find_context_*`
| Function                       | `plain`                    | `hot`                      | `heavy`                    |
|--------------------------------|----------------------------|----------------------------|----------------------------|
| `context`                      | [14.375 ms]=[19.033 GiB/s] | [91.776 ms]=[863.10 MiB/s] | [173.26 ms]=[866.94 MiB/s] |
| `find_context_italics`         | [12.350 ms]=[22.154 GiB/s] | [18.636 ms]=[4.1510 GiB/s] | [50.693 ms]=[2.8936 GiB/s] |
| `find_context_bolds`           | [12.337 ms]=[22.178 GiB/s] | [18.868 ms]=[4.0998 GiB/s] | [57.099 ms]=[2.5689 GiB/s] |
| `find_context_bold_italics`    | [12.350 ms]=[22.154 GiB/s] | [15.883 ms]=[4.8702 GiB/s] | [47.862 ms]=[3.0647 GiB/s] |
| `find_context_headings`        | [12.379 ms]=[22.102 GiB/s] | [12.895 ms]=[5.9991 GiB/s] | [10.333 ms]=[14.195 GiB/s] |
| `find_context_thematic_breaks` | [14.307 ms]=[19.123 GiB/s] | [8.5526 ms]=[9.0447 GiB/s] | [34.122 ms]=[4.2988 GiB/s] |
| `find_context_blockquotes`     | [12.367 ms]=[22.124 GiB/s] | [5.3220 ms]=[14.535 GiB/s] | [15.032 ms]=[9.7584 GiB/s] |
| `find_context_bullet_items`    | [14.433 ms]=[18.957 GiB/s] | [8.5717 ms]=[9.0246 GiB/s] | [32.848 ms]=[4.4656 GiB/s] |
| `find_context_ordered_items`   | [30.699 ms]=[8.9126 GiB/s] | [11.107 ms]=[6.9649 GiB/s] | [28.284 ms]=[5.1861 GiB/s] |

#### Cold context - `find_context_*_cold`
| Function                            | `plain`                    | `hot`                      | `heavy`                    |
|-------------------------------------|----------------------------|----------------------------|----------------------------|
| `find_context_italics_cold`         | [27.110 ms]=[10.092 GiB/s] | [110.03 ms]=[719.90 MiB/s] | [225.03 ms]=[667.49 MiB/s] |
| `find_context_bolds_cold`           | [27.106 ms]=[10.094 GiB/s] | [109.36 ms]=[724.32 MiB/s] | [231.09 ms]=[650.00 MiB/s] |
| `find_context_bold_italics_cold`    | [27.078 ms]=[10.104 GiB/s] | [106.79 ms]=[741.76 MiB/s] | [222.58 ms]=[674.85 MiB/s] |
| `find_context_headings_cold`        | [27.123 ms]=[10.088 GiB/s] | [104.79 ms]=[755.91 MiB/s] | [185.94 ms]=[807.84 MiB/s] |
| `find_context_thematic_breaks_cold` | [29.024 ms]=[9.4267 GiB/s] | [100.26 ms]=[790.06 MiB/s] | [208.21 ms]=[721.42 MiB/s] |
| `find_context_blockquotes_cold`     | [27.116 ms]=[10.090 GiB/s] | [99.576 ms]=[795.50 MiB/s] | [190.07 ms]=[790.24 MiB/s] |
| `find_context_bullet_items_cold`    | [28.980 ms]=[9.4412 GiB/s] | [100.33 ms]=[789.51 MiB/s] | [207.05 ms]=[725.46 MiB/s] |
| `find_context_ordered_items_cold`   | [45.337 ms]=[6.0349 GiB/s] | [103.01 ms]=[769.01 MiB/s] | [202.48 ms]=[741.84 MiB/s] |

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
                            time:   [12.404 ms 12.429 ms 12.461 ms]
                            thrpt:  [21.956 GiB/s 22.013 GiB/s 22.058 GiB/s]
                     change:
                            time:   [+40043% +41171% +42362%] (p = 0.00 < 0.05)
                            thrpt:  [-99.764% -99.758% -99.751%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_italics       full=       0  standalone=       0
    standalone/plain/find_italics
                            time:   [12.366 ms 12.380 ms 12.398 ms]
                            thrpt:  [22.068 GiB/s 22.100 GiB/s 22.125 GiB/s]
                     change:
                            time:   [+38430% +39559% +40672%] (p = 0.00 < 0.05)
                            thrpt:  [-99.755% -99.748% -99.740%]
                            Performance has regressed.
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) high mild
      1 (5.00%) high severe
        find_bolds         full=       0  standalone=       0
    standalone/plain/find_bolds
                            time:   [12.414 ms 12.436 ms 12.457 ms]
                            thrpt:  [21.963 GiB/s 22.001 GiB/s 22.039 GiB/s]
                     change:
                            time:   [+39404% +41418% +43384%] (p = 0.00 < 0.05)
                            thrpt:  [-99.770% -99.759% -99.747%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_bold_italics  full=       0  standalone=       0
    standalone/plain/find_bold_italics
                            time:   [12.401 ms 12.422 ms 12.447 ms]
                            thrpt:  [21.982 GiB/s 22.026 GiB/s 22.063 GiB/s]
                     change:
                            time:   [+40024% +41565% +43126%] (p = 0.00 < 0.05)
                            thrpt:  [-99.769% -99.760% -99.751%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_autolinks     full=       0  standalone=       0
    standalone/plain/find_autolinks
                            time:   [12.408 ms 12.421 ms 12.437 ms]
                            thrpt:  [21.999 GiB/s 22.027 GiB/s 22.051 GiB/s]
                     change:
                            time:   [+40644% +42082% +43491%] (p = 0.00 < 0.05)
                            thrpt:  [-99.771% -99.763% -99.755%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_links         full=       0  standalone=       0
    standalone/plain/find_links
                            time:   [12.448 ms 12.484 ms 12.511 ms]
                            thrpt:  [21.869 GiB/s 21.916 GiB/s 21.980 GiB/s]
                     change:
                            time:   [+40275% +41360% +42389%] (p = 0.00 < 0.05)
                            thrpt:  [-99.765% -99.759% -99.752%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_headings      full=       0  standalone=       0
    standalone/plain/find_headings
                            time:   [12.428 ms 12.452 ms 12.473 ms]
                            thrpt:  [21.936 GiB/s 21.973 GiB/s 22.015 GiB/s]
                     change:
                            time:   [+35627% +36933% +38243%] (p = 0.00 < 0.05)
                            thrpt:  [-99.739% -99.730% -99.720%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_thematic_breaks full=       0  standalone=       0
    standalone/plain/find_thematic_breaks
                            time:   [14.405 ms 14.422 ms 14.442 ms]
                            thrpt:  [18.946 GiB/s 18.971 GiB/s 18.994 GiB/s]
                     change:
                            time:   [+33170% +33495% +33805%] (p = 0.00 < 0.05)
                            thrpt:  [-99.705% -99.702% -99.699%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_fenced_codes  full=       0  standalone=       0
    standalone/plain/find_fenced_codes
                            time:   [12.287 ms 12.315 ms 12.361 ms]
                            thrpt:  [22.135 GiB/s 22.217 GiB/s 22.267 GiB/s]
                     change:
                            time:   [+36163% +37746% +39416%] (p = 0.00 < 0.05)
                            thrpt:  [-99.747% -99.736% -99.724%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_blockquotes   full=       0  standalone=       0
    standalone/plain/find_blockquotes
                            time:   [12.325 ms 12.344 ms 12.363 ms]
                            thrpt:  [22.131 GiB/s 22.165 GiB/s 22.200 GiB/s]
                     change:
                            time:   [+36169% +37795% +39558%] (p = 0.00 < 0.05)
                            thrpt:  [-99.748% -99.736% -99.724%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_bullet_items  full=       0  standalone=       0
    standalone/plain/find_bullet_items
                            time:   [14.315 ms 14.334 ms 14.352 ms]
                            thrpt:  [19.064 GiB/s 19.088 GiB/s 19.114 GiB/s]
                     change:
                            time:   [+31904% +32679% +33555%] (p = 0.00 < 0.05)
                            thrpt:  [-99.703% -99.695% -99.688%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_ordered_items full=       0  standalone=       0
    standalone/plain/find_ordered_items
                            time:   [30.630 ms 30.654 ms 30.675 ms]
                            thrpt:  [8.9195 GiB/s 8.9255 GiB/s 8.9325 GiB/s]
                     change:
                            time:   [+11016% +11038% +11070%] (p = 0.00 < 0.05)
                            thrpt:  [-99.105% -99.102% -99.100%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    │  context regions: 0
    standalone/plain/context
                            time:   [14.359 ms 14.375 ms 14.392 ms]
                            thrpt:  [19.011 GiB/s 19.033 GiB/s 19.055 GiB/s]
                     change:
                            time:   [+29723% +30123% +30524%] (p = 0.00 < 0.05)
                            thrpt:  [-99.673% -99.669% -99.665%]
                            Performance has regressed.
    Found 4 outliers among 20 measurements (20.00%)
      2 (10.00%) low mild
      2 (10.00%) high severe
    │  full-vs-context-aware counts:
        find_context_italics full=       0  context-aware=       0
    standalone/plain/find_context_italics
                            time:   [12.330 ms 12.350 ms 12.369 ms]
                            thrpt:  [22.119 GiB/s 22.154 GiB/s 22.190 GiB/s]
                     change:
                            time:   [+39305% +40261% +41175%] (p = 0.00 < 0.05)
                            thrpt:  [-99.758% -99.752% -99.746%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_context_bolds full=       0  context-aware=       0
    standalone/plain/find_context_bolds
                            time:   [12.300 ms 12.337 ms 12.368 ms]
                            thrpt:  [22.123 GiB/s 22.178 GiB/s 22.245 GiB/s]
                     change:
                            time:   [+37141% +38375% +39736%] (p = 0.00 < 0.05)
                            thrpt:  [-99.749% -99.740% -99.731%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
        find_context_bold_italics full=       0  context-aware=       0
    standalone/plain/find_context_bold_italics
                            time:   [12.336 ms 12.350 ms 12.367 ms]
                            thrpt:  [22.124 GiB/s 22.154 GiB/s 22.179 GiB/s]
                     change:
                            time:   [+36673% +38342% +40052%] (p = 0.00 < 0.05)
                            thrpt:  [-99.751% -99.740% -99.728%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_context_headings full=       0  context-aware=       0
    standalone/plain/find_context_headings
                            time:   [12.358 ms 12.379 ms 12.398 ms]
                            thrpt:  [22.069 GiB/s 22.102 GiB/s 22.140 GiB/s]
                     change:
                            time:   [+41611% +42548% +43451%] (p = 0.00 < 0.05)
                            thrpt:  [-99.770% -99.766% -99.760%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_context_thematic_breaks full=       0  context-aware=       0
    standalone/plain/find_context_thematic_breaks
                            time:   [14.281 ms 14.307 ms 14.328 ms]
                            thrpt:  [19.096 GiB/s 19.123 GiB/s 19.158 GiB/s]
                     change:
                            time:   [+31956% +32795% +33668%] (p = 0.00 < 0.05)
                            thrpt:  [-99.704% -99.696% -99.688%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_context_blockquotes full=       0  context-aware=       0
    standalone/plain/find_context_blockquotes
                            time:   [12.311 ms 12.367 ms 12.410 ms]
                            thrpt:  [22.047 GiB/s 22.124 GiB/s 22.224 GiB/s]
                     change:
                            time:   [+36446% +37028% +37599%] (p = 0.00 < 0.05)
                            thrpt:  [-99.735% -99.731% -99.726%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_context_bullet_items full=       0  context-aware=       0
    standalone/plain/find_context_bullet_items
                            time:   [14.409 ms 14.433 ms 14.457 ms]
                            thrpt:  [18.925 GiB/s 18.957 GiB/s 18.989 GiB/s]
                     change:
                            time:   [+32425% +33423% +34476%] (p = 0.00 < 0.05)
                            thrpt:  [-99.711% -99.702% -99.693%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_context_ordered_items full=       0  context-aware=       0
    standalone/plain/find_context_ordered_items
                            time:   [30.674 ms 30.699 ms 30.724 ms]
                            thrpt:  [8.9051 GiB/s 8.9126 GiB/s 8.9199 GiB/s]
                     change:
                            time:   [+11042% +11055% +11067%] (p = 0.00 < 0.05)
                            thrpt:  [-99.105% -99.104% -99.102%]
                            Performance has regressed.
    standalone/plain/find_context_italics_cold
                            time:   [27.076 ms 27.110 ms 27.147 ms]
                            thrpt:  [10.079 GiB/s 10.092 GiB/s 10.105 GiB/s]
                     change:
                            time:   [+34647% +35461% +36286%] (p = 0.00 < 0.05)
                            thrpt:  [-99.725% -99.719% -99.712%]
                            Performance has regressed.
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low mild
      1 (5.00%) high mild
      1 (5.00%) high severe
    standalone/plain/find_context_bolds_cold
                            time:   [27.061 ms 27.106 ms 27.143 ms]
                            thrpt:  [10.080 GiB/s 10.094 GiB/s 10.111 GiB/s]
                     change:
                            time:   [+32366% +33298% +34304%] (p = 0.00 < 0.05)
                            thrpt:  [-99.709% -99.701% -99.692%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    standalone/plain/find_context_bold_italics_cold
                            time:   [27.023 ms 27.078 ms 27.137 ms]
                            thrpt:  [10.082 GiB/s 10.104 GiB/s 10.125 GiB/s]
                     change:
                            time:   [+34083% +35042% +36023%] (p = 0.00 < 0.05)
                            thrpt:  [-99.723% -99.715% -99.707%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    standalone/plain/find_context_headings_cold
                            time:   [27.090 ms 27.123 ms 27.163 ms]
                            thrpt:  [10.073 GiB/s 10.088 GiB/s 10.100 GiB/s]
                     change:
                            time:   [+32256% +33281% +34346%] (p = 0.00 < 0.05)
                            thrpt:  [-99.710% -99.700% -99.691%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    standalone/plain/find_context_thematic_breaks_cold
                            time:   [28.973 ms 29.024 ms 29.080 ms]
                            thrpt:  [9.4086 GiB/s 9.4267 GiB/s 9.4433 GiB/s]
                     change:
                            time:   [+32001% +32721% +33486%] (p = 0.00 < 0.05)
                            thrpt:  [-99.702% -99.695% -99.688%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    standalone/plain/find_context_blockquotes_cold
                            time:   [27.067 ms 27.116 ms 27.163 ms]
                            thrpt:  [10.073 GiB/s 10.090 GiB/s 10.108 GiB/s]
                     change:
                            time:   [+30986% +32207% +33420%] (p = 0.00 < 0.05)
                            thrpt:  [-99.702% -99.690% -99.678%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low mild
      1 (5.00%) high severe
    standalone/plain/find_context_bullet_items_cold
                            time:   [28.946 ms 28.980 ms 29.016 ms]
                            thrpt:  [9.4294 GiB/s 9.4412 GiB/s 9.4522 GiB/s]
                     change:
                            time:   [+32445% +33727% +34980%] (p = 0.00 < 0.05)
                            thrpt:  [-99.715% -99.704% -99.693%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
    standalone/plain/find_context_ordered_items_cold
                            time:   [45.211 ms 45.337 ms 45.447 ms]
                            thrpt:  [6.0203 GiB/s 6.0349 GiB/s 6.0517 GiB/s]
                     change:
                            time:   [+12741% +12980% +13217%] (p = 0.00 < 0.05)
                            thrpt:  [-99.249% -99.235% -99.221%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
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
                            time:   [10.113 ms 10.121 ms 10.134 ms]
                            thrpt:  [7.6332 GiB/s 7.6428 GiB/s 7.6494 GiB/s]
                     change:
                            time:   [+11346% +11372% +11410%] (p = 0.00 < 0.05)
                            thrpt:  [-99.131% -99.128% -99.126%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_italics       full=  500000  standalone=  500000
    standalone/hot/find_italics
                            time:   [15.624 ms 15.650 ms 15.691 ms]
                            thrpt:  [4.9299 GiB/s 4.9427 GiB/s 4.9510 GiB/s]
                     change:
                            time:   [+10533% +10561% +10589%] (p = 0.00 < 0.05)
                            thrpt:  [-99.064% -99.062% -99.059%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
        find_bolds         full=  500000  standalone=  500000
    standalone/hot/find_bolds
                            time:   [15.850 ms 15.860 ms 15.872 ms]
                            thrpt:  [4.8737 GiB/s 4.8775 GiB/s 4.8805 GiB/s]
                     change:
                            time:   [+10307% +10334% +10359%] (p = 0.00 < 0.05)
                            thrpt:  [-99.044% -99.042% -99.039%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_bold_italics  full=       0  standalone=       0
    standalone/hot/find_bold_italics
                            time:   [13.351 ms 13.362 ms 13.374 ms]
                            thrpt:  [5.7841 GiB/s 5.7890 GiB/s 5.7938 GiB/s]
                     change:
                            time:   [+10455% +10524% +10570%] (p = 0.00 < 0.05)
                            thrpt:  [-99.063% -99.059% -99.053%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_autolinks     full=  500000  standalone=  500000
    standalone/hot/find_autolinks
                            time:   [10.165 ms 10.202 ms 10.236 ms]
                            thrpt:  [7.5571 GiB/s 7.5822 GiB/s 7.6101 GiB/s]
                     change:
                            time:   [+11111% +11156% +11202%] (p = 0.00 < 0.05)
                            thrpt:  [-99.115% -99.112% -99.108%]
                            Performance has regressed.
        find_links         full=  500000  standalone=  500000
    standalone/hot/find_links
                            time:   [21.085 ms 21.093 ms 21.100 ms]
                            thrpt:  [3.6662 GiB/s 3.6674 GiB/s 3.6687 GiB/s]
                     change:
                            time:   [+10074% +10082% +10091%] (p = 0.00 < 0.05)
                            thrpt:  [-99.019% -99.018% -99.017%]
                            Performance has regressed.
    Found 4 outliers among 20 measurements (20.00%)
      3 (15.00%) low mild
      1 (5.00%) high severe
        find_headings      full=  500000  standalone=  500000
    standalone/hot/find_headings
                            time:   [10.142 ms 10.147 ms 10.153 ms]
                            thrpt:  [7.6189 GiB/s 7.6235 GiB/s 7.6275 GiB/s]
                     change:
                            time:   [+11040% +11060% +11093%] (p = 0.00 < 0.05)
                            thrpt:  [-99.107% -99.104% -99.102%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_thematic_breaks full=       0  standalone=       0
    standalone/hot/find_thematic_breaks
                            time:   [8.6214 ms 8.6318 ms 8.6412 ms]
                            thrpt:  [8.9519 GiB/s 8.9617 GiB/s 8.9725 GiB/s]
                     change:
                            time:   [+10853% +10873% +10893%] (p = 0.00 < 0.05)
                            thrpt:  [-99.090% -99.089% -99.087%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_fenced_codes  full=       0  standalone=       0
    standalone/hot/find_fenced_codes
                            time:   [7.6877 ms 7.7077 ms 7.7238 ms]
                            thrpt:  [10.015 GiB/s 10.036 GiB/s 10.062 GiB/s]
                     change:
                            time:   [+11416% +11451% +11493%] (p = 0.00 < 0.05)
                            thrpt:  [-99.137% -99.134% -99.132%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_blockquotes   full=       0  standalone=       0
    standalone/hot/find_blockquotes
                            time:   [5.2953 ms 5.3076 ms 5.3180 ms]
                            thrpt:  [14.546 GiB/s 14.575 GiB/s 14.608 GiB/s]
                     change:
                            time:   [+13633% +13698% +13788%] (p = 0.00 < 0.05)
                            thrpt:  [-99.280% -99.275% -99.272%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
        find_bullet_items  full=       0  standalone=       0
    standalone/hot/find_bullet_items
                            time:   [8.5340 ms 8.5440 ms 8.5540 ms]
                            thrpt:  [9.0432 GiB/s 9.0538 GiB/s 9.0644 GiB/s]
                     change:
                            time:   [+11124% +11156% +11182%] (p = 0.00 < 0.05)
                            thrpt:  [-99.114% -99.112% -99.109%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low mild
      1 (5.00%) high severe
        find_ordered_items full=       0  standalone=       0
    standalone/hot/find_ordered_items
                            time:   [11.072 ms 11.084 ms 11.097 ms]
                            thrpt:  [6.9710 GiB/s 6.9793 GiB/s 6.9863 GiB/s]
                     change:
                            time:   [+10854% +10869% +10886%] (p = 0.00 < 0.05)
                            thrpt:  [-99.090% -99.088% -99.087%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    │  context regions: 1500000
    Benchmarking standalone/hot/context: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 19.5s, enable flat sampling, or reduce sample count to 10.
    standalone/hot/context  time:   [91.488 ms 91.776 ms 92.046 ms]
                            thrpt:  [860.57 MiB/s 863.10 MiB/s 865.82 MiB/s]
                     change:
                            time:   [+17950% +18079% +18250%] (p = 0.00 < 0.05)
                            thrpt:  [-99.455% -99.450% -99.446%]
                            Performance has regressed.
    Found 4 outliers among 20 measurements (20.00%)
      1 (5.00%) low severe
      1 (5.00%) low mild
      2 (10.00%) high severe
    │  full-vs-context-aware counts:
        find_context_italics full=  500000  context-aware=  500000
    standalone/hot/find_context_italics
                            time:   [18.623 ms 18.636 ms 18.647 ms]
                            thrpt:  [4.1483 GiB/s 4.1510 GiB/s 4.1539 GiB/s]
                     change:
                            time:   [+10205% +10213% +10222%] (p = 0.00 < 0.05)
                            thrpt:  [-99.031% -99.030% -99.030%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_context_bolds full=  500000  context-aware=  500000
    standalone/hot/find_context_bolds
                            time:   [18.861 ms 18.868 ms 18.875 ms]
                            thrpt:  [4.0982 GiB/s 4.0998 GiB/s 4.1014 GiB/s]
                     change:
                            time:   [+10141% +10163% +10180%] (p = 0.00 < 0.05)
                            thrpt:  [-99.027% -99.026% -99.024%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_context_bold_italics full=       0  context-aware=       0
    standalone/hot/find_context_bold_italics
                            time:   [15.864 ms 15.883 ms 15.908 ms]
                            thrpt:  [4.8628 GiB/s 4.8702 GiB/s 4.8763 GiB/s]
                     change:
                            time:   [+10351% +10365% +10380%] (p = 0.00 < 0.05)
                            thrpt:  [-99.046% -99.044% -99.043%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
        find_context_headings full=  500000  context-aware=  500000
    standalone/hot/find_context_headings
                            time:   [12.877 ms 12.895 ms 12.908 ms]
                            thrpt:  [5.9927 GiB/s 5.9991 GiB/s 6.0074 GiB/s]
                     change:
                            time:   [+11028% +11045% +11062%] (p = 0.00 < 0.05)
                            thrpt:  [-99.104% -99.103% -99.101%]
                            Performance has regressed.
        find_context_thematic_breaks full=       0  context-aware=       0
    standalone/hot/find_context_thematic_breaks
                            time:   [8.5444 ms 8.5526 ms 8.5607 ms]
                            thrpt:  [9.0362 GiB/s 9.0447 GiB/s 9.0534 GiB/s]
                     change:
                            time:   [+10868% +10893% +10922%] (p = 0.00 < 0.05)
                            thrpt:  [-99.093% -99.090% -99.088%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_context_blockquotes full=       0  context-aware=       0
    standalone/hot/find_context_blockquotes
                            time:   [5.3098 ms 5.3220 ms 5.3369 ms]
                            thrpt:  [14.494 GiB/s 14.535 GiB/s 14.568 GiB/s]
                     change:
                            time:   [+13626% +13746% +13964%] (p = 0.00 < 0.05)
                            thrpt:  [-99.289% -99.278% -99.271%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_context_bullet_items full=       0  context-aware=       0
    standalone/hot/find_context_bullet_items
                            time:   [8.5635 ms 8.5717 ms 8.5792 ms]
                            thrpt:  [9.0166 GiB/s 9.0246 GiB/s 9.0332 GiB/s]
                     change:
                            time:   [+11085% +11117% +11154%] (p = 0.00 < 0.05)
                            thrpt:  [-99.111% -99.109% -99.106%]
                            Performance has regressed.
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low mild
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_context_ordered_items full=       0  context-aware=       0
    standalone/hot/find_context_ordered_items
                            time:   [11.098 ms 11.107 ms 11.117 ms]
                            thrpt:  [6.9585 GiB/s 6.9649 GiB/s 6.9703 GiB/s]
                     change:
                            time:   [+10809% +10859% +10893%] (p = 0.00 < 0.05)
                            thrpt:  [-99.090% -99.088% -99.083%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    standalone/hot/find_context_italics_cold
                            time:   [109.75 ms 110.03 ms 110.34 ms]
                            thrpt:  [717.89 MiB/s 719.90 MiB/s 721.75 MiB/s]
                     change:
                            time:   [+15783% +15824% +15867%] (p = 0.00 < 0.05)
                            thrpt:  [-99.374% -99.372% -99.370%]
                            Performance has regressed.
    standalone/hot/find_context_bolds_cold
                            time:   [109.16 ms 109.36 ms 109.57 ms]
                            thrpt:  [722.92 MiB/s 724.32 MiB/s 725.63 MiB/s]
                     change:
                            time:   [+15636% +15670% +15704%] (p = 0.00 < 0.05)
                            thrpt:  [-99.367% -99.366% -99.365%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    standalone/hot/find_context_bold_italics_cold
                            time:   [106.69 ms 106.79 ms 106.89 ms]
                            thrpt:  [741.04 MiB/s 741.76 MiB/s 742.46 MiB/s]
                     change:
                            time:   [+15971% +15993% +16016%] (p = 0.00 < 0.05)
                            thrpt:  [-99.380% -99.379% -99.378%]
                            Performance has regressed.
    standalone/hot/find_context_headings_cold
                            time:   [104.42 ms 104.79 ms 105.19 ms]
                            thrpt:  [753.04 MiB/s 755.91 MiB/s 758.62 MiB/s]
                     change:
                            time:   [+16473% +16538% +16599%] (p = 0.00 < 0.05)
                            thrpt:  [-99.401% -99.399% -99.397%]
                            Performance has regressed.
    standalone/hot/find_context_thematic_breaks_cold
                            time:   [100.13 ms 100.26 ms 100.41 ms]
                            thrpt:  [788.88 MiB/s 790.06 MiB/s 791.12 MiB/s]
                     change:
                            time:   [+17015% +17054% +17089%] (p = 0.00 < 0.05)
                            thrpt:  [-99.418% -99.417% -99.416%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    standalone/hot/find_context_blockquotes_cold
                            time:   [99.249 ms 99.576 ms 99.851 ms]
                            thrpt:  [793.31 MiB/s 795.50 MiB/s 798.12 MiB/s]
                     change:
                            time:   [+18073% +18137% +18190%] (p = 0.00 < 0.05)
                            thrpt:  [-99.453% -99.452% -99.450%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
    standalone/hot/find_context_bullet_items_cold
                            time:   [100.17 ms 100.33 ms 100.50 ms]
                            thrpt:  [788.15 MiB/s 789.51 MiB/s 790.80 MiB/s]
                     change:
                            time:   [+17015% +17041% +17072%] (p = 0.00 < 0.05)
                            thrpt:  [-99.418% -99.417% -99.416%]
                            Performance has regressed.
    standalone/hot/find_context_ordered_items_cold
                            time:   [102.62 ms 103.01 ms 103.39 ms]
                            thrpt:  [766.13 MiB/s 769.01 MiB/s 771.89 MiB/s]
                     change:
                            time:   [+16726% +16786% +16857%] (p = 0.00 < 0.05)
                            thrpt:  [-99.410% -99.408% -99.406%]
                            Performance has regressed.
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
                            time:   [24.832 ms 24.901 ms 24.970 ms]
                            thrpt:  [5.8744 GiB/s 5.8907 GiB/s 5.9070 GiB/s]
                     change:
                            time:   [+10333% +10360% +10388%] (p = 0.00 < 0.05)
                            thrpt:  [-99.047% -99.044% -99.041%]
                            Performance has regressed.
        find_italics       full= 1200000  standalone= 1200000
    standalone/heavy/find_italics
                            time:   [42.452 ms 42.877 ms 43.323 ms]
                            thrpt:  [3.3859 GiB/s 3.4210 GiB/s 3.4553 GiB/s]
                     change:
                            time:   [+9970.9% +10038% +10109%] (p = 0.00 < 0.05)
                            thrpt:  [-99.020% -99.014% -99.007%]
                            Performance has regressed.
        find_bolds         full= 1200000  standalone= 1200000
    standalone/heavy/find_bolds
                            time:   [43.914 ms 44.086 ms 44.316 ms]
                            thrpt:  [3.3100 GiB/s 3.3272 GiB/s 3.3403 GiB/s]
                     change:
                            time:   [+10303% +10342% +10382%] (p = 0.00 < 0.05)
                            thrpt:  [-99.046% -99.042% -99.039%]
                            Performance has regressed.
        find_bold_italics  full=  600000  standalone=  600000
    standalone/heavy/find_bold_italics
                            time:   [40.067 ms 40.142 ms 40.189 ms]
                            thrpt:  [3.6498 GiB/s 3.6542 GiB/s 3.6610 GiB/s]
                     change:
                            time:   [+9978.4% +10001% +10022%] (p = 0.00 < 0.05)
                            thrpt:  [-99.012% -99.010% -99.008%]
                            Performance has regressed.
        find_autolinks     full=  400000  standalone=  400000
    standalone/heavy/find_autolinks
                            time:   [10.833 ms 10.853 ms 10.874 ms]
                            thrpt:  [13.490 GiB/s 13.515 GiB/s 13.541 GiB/s]
                     change:
                            time:   [+13898% +13964% +14059%] (p = 0.00 < 0.05)
                            thrpt:  [-99.294% -99.289% -99.286%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low mild
      1 (5.00%) high severe
        find_links         full=  600000  standalone=  600000
    standalone/heavy/find_links
                            time:   [26.521 ms 26.536 ms 26.549 ms]
                            thrpt:  [5.5251 GiB/s 5.5277 GiB/s 5.5310 GiB/s]
                     change:
                            time:   [+10318% +10334% +10350%] (p = 0.00 < 0.05)
                            thrpt:  [-99.043% -99.042% -99.040%]
                            Performance has regressed.
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low mild
      2 (10.00%) high severe
        find_headings      full=  200000  standalone=  200000
    standalone/heavy/find_headings
                            time:   [8.8546 ms 8.8670 ms 8.8816 ms]
                            thrpt:  [16.516 GiB/s 16.543 GiB/s 16.566 GiB/s]
                     change:
                            time:   [+19524% +19665% +19883%] (p = 0.00 < 0.05)
                            thrpt:  [-99.500% -99.494% -99.490%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_thematic_breaks full=  200000  standalone=  200000
    standalone/heavy/find_thematic_breaks
                            time:   [29.909 ms 29.924 ms 29.940 ms]
                            thrpt:  [4.8993 GiB/s 4.9018 GiB/s 4.9043 GiB/s]
                     change:
                            time:   [+10009% +10017% +10024%] (p = 0.00 < 0.05)
                            thrpt:  [-99.012% -99.012% -99.011%]
                            Performance has regressed.
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) low mild
      1 (5.00%) high mild
        find_fenced_codes  full=  400000  standalone=  400000
    standalone/heavy/find_fenced_codes
                            time:   [25.297 ms 25.326 ms 25.356 ms]
                            thrpt:  [5.7851 GiB/s 5.7918 GiB/s 5.7985 GiB/s]
                     change:
                            time:   [+10035% +10049% +10067%] (p = 0.00 < 0.05)
                            thrpt:  [-99.016% -99.015% -99.013%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_blockquotes   full=  400000  standalone=  400000
    standalone/heavy/find_blockquotes
                            time:   [14.551 ms 14.567 ms 14.585 ms]
                            thrpt:  [10.057 GiB/s 10.070 GiB/s 10.080 GiB/s]
                     change:
                            time:   [+12036% +12063% +12092%] (p = 0.00 < 0.05)
                            thrpt:  [-99.180% -99.178% -99.176%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_bullet_items  full=  600000  standalone=  600000
    standalone/heavy/find_bullet_items
                            time:   [27.998 ms 28.009 ms 28.021 ms]
                            thrpt:  [5.2348 GiB/s 5.2370 GiB/s 5.2391 GiB/s]
                     change:
                            time:   [+10032% +10043% +10056%] (p = 0.00 < 0.05)
                            thrpt:  [-99.015% -99.014% -99.013%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low mild
      1 (5.00%) high severe
        find_ordered_items full=  400000  standalone=  400000
    standalone/heavy/find_ordered_items
                            time:   [23.821 ms 23.849 ms 23.877 ms]
                            thrpt:  [6.1434 GiB/s 6.1505 GiB/s 6.1578 GiB/s]
                     change:
                            time:   [+10326% +10348% +10371%] (p = 0.00 < 0.05)
                            thrpt:  [-99.045% -99.043% -99.041%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    │  context regions: 2400000
    standalone/heavy/context
                            time:   [172.77 ms 173.26 ms 173.74 ms]
                            thrpt:  [864.53 MiB/s 866.94 MiB/s 869.39 MiB/s]
                     change:
                            time:   [+20163% +20229% +20292%] (p = 0.00 < 0.05)
                            thrpt:  [-99.510% -99.508% -99.507%]
                            Performance has regressed.
    │  full-vs-context-aware counts:
        find_context_italics full= 1200000  context-aware= 1200000
    Benchmarking standalone/heavy/find_context_italics: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 10.6s, enable flat sampling, or reduce sample count to 10.
    standalone/heavy/find_context_italics
                            time:   [50.675 ms 50.693 ms 50.718 ms]
                            thrpt:  [2.8922 GiB/s 2.8936 GiB/s 2.8946 GiB/s]
                     change:
                            time:   [+9876.3% +9901.1% +9926.4%] (p = 0.00 < 0.05)
                            thrpt:  [-99.003% -99.000% -98.998%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
        find_context_bolds full= 1200000  context-aware= 1200000
    Benchmarking standalone/heavy/find_context_bolds: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 12.1s, enable flat sampling, or reduce sample count to 10.
    standalone/heavy/find_context_bolds
                            time:   [57.045 ms 57.099 ms 57.197 ms]
                            thrpt:  [2.5646 GiB/s 2.5689 GiB/s 2.5714 GiB/s]
                     change:
                            time:   [+9965.4% +10001% +10034%] (p = 0.00 < 0.05)
                            thrpt:  [-99.013% -99.010% -99.006%]
                            Performance has regressed.
        find_context_bold_italics full=  600000  context-aware=  600000
    Benchmarking standalone/heavy/find_context_bold_italics: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 10.1s, enable flat sampling, or reduce sample count to 10.
    standalone/heavy/find_context_bold_italics
                            time:   [47.829 ms 47.862 ms 47.923 ms]
                            thrpt:  [3.0608 GiB/s 3.0647 GiB/s 3.0668 GiB/s]
                     change:
                            time:   [+9905.1% +9942.3% +9980.2%] (p = 0.00 < 0.05)
                            thrpt:  [-99.008% -99.004% -99.001%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_context_headings full=  200000  context-aware=  200000
    standalone/heavy/find_context_headings
                            time:   [10.315 ms 10.333 ms 10.353 ms]
                            thrpt:  [14.168 GiB/s 14.195 GiB/s 14.220 GiB/s]
                     change:
                            time:   [+16368% +16479% +16620%] (p = 0.00 < 0.05)
                            thrpt:  [-99.402% -99.397% -99.393%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_context_thematic_breaks full=  200000  context-aware=  200000
    standalone/heavy/find_context_thematic_breaks
                            time:   [34.095 ms 34.122 ms 34.155 ms]
                            thrpt:  [4.2947 GiB/s 4.2988 GiB/s 4.3022 GiB/s]
                     change:
                            time:   [+9983.1% +10001% +10019%] (p = 0.00 < 0.05)
                            thrpt:  [-99.012% -99.010% -99.008%]
                            Performance has regressed.
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) high mild
      1 (5.00%) high severe
        find_context_blockquotes full=  400000  context-aware=  400000
    standalone/heavy/find_context_blockquotes
                            time:   [15.012 ms 15.032 ms 15.059 ms]
                            thrpt:  [9.7410 GiB/s 9.7584 GiB/s 9.7709 GiB/s]
                     change:
                            time:   [+11536% +11568% +11601%] (p = 0.00 < 0.05)
                            thrpt:  [-99.145% -99.143% -99.141%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_context_bullet_items full=  600000  context-aware=  600000
    standalone/heavy/find_context_bullet_items
                            time:   [32.819 ms 32.848 ms 32.868 ms]
                            thrpt:  [4.4629 GiB/s 4.4656 GiB/s 4.4696 GiB/s]
                     change:
                            time:   [+10028% +10046% +10062%] (p = 0.00 < 0.05)
                            thrpt:  [-99.016% -99.014% -99.013%]
                            Performance has regressed.
        find_context_ordered_items full=  400000  context-aware=  400000
    standalone/heavy/find_context_ordered_items
                            time:   [28.252 ms 28.284 ms 28.311 ms]
                            thrpt:  [5.1812 GiB/s 5.1861 GiB/s 5.1920 GiB/s]
                     change:
                            time:   [+10252% +10271% +10295%] (p = 0.00 < 0.05)
                            thrpt:  [-99.038% -99.036% -99.034%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    standalone/heavy/find_context_italics_cold
                            time:   [224.82 ms 225.03 ms 225.20 ms]
                            thrpt:  [666.99 MiB/s 667.49 MiB/s 668.12 MiB/s]
                     change:
                            time:   [+16549% +16573% +16594%] (p = 0.00 < 0.05)
                            thrpt:  [-99.401% -99.400% -99.399%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
    standalone/heavy/find_context_bolds_cold
                            time:   [230.49 ms 231.09 ms 231.66 ms]
                            thrpt:  [648.40 MiB/s 650.00 MiB/s 651.68 MiB/s]
                     change:
                            time:   [+16277% +16325% +16370%] (p = 0.00 < 0.05)
                            thrpt:  [-99.393% -99.391% -99.389%]
                            Performance has regressed.
    Found 5 outliers among 20 measurements (25.00%)
      1 (5.00%) low severe
      1 (5.00%) low mild
      2 (10.00%) high mild
      1 (5.00%) high severe
    standalone/heavy/find_context_bold_italics_cold
                            time:   [222.43 ms 222.58 ms 222.72 ms]
                            thrpt:  [674.41 MiB/s 674.85 MiB/s 675.29 MiB/s]
                     change:
                            time:   [+16714% +16739% +16760%] (p = 0.00 < 0.05)
                            thrpt:  [-99.407% -99.406% -99.405%]
                            Performance has regressed.
    standalone/heavy/find_context_headings_cold
                            time:   [185.56 ms 185.94 ms 186.30 ms]
                            thrpt:  [806.27 MiB/s 807.84 MiB/s 809.46 MiB/s]
                     change:
                            time:   [+20348% +20392% +20437%] (p = 0.00 < 0.05)
                            thrpt:  [-99.513% -99.512% -99.511%]
                            Performance has regressed.
    standalone/heavy/find_context_thematic_breaks_cold
                            time:   [207.77 ms 208.21 ms 208.63 ms]
                            thrpt:  [719.95 MiB/s 721.42 MiB/s 722.95 MiB/s]
                     change:
                            time:   [+17499% +17539% +17578%] (p = 0.00 < 0.05)
                            thrpt:  [-99.434% -99.433% -99.432%]
                            Performance has regressed.
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) low mild
      1 (5.00%) high mild
    standalone/heavy/find_context_blockquotes_cold
                            time:   [189.92 ms 190.07 ms 190.23 ms]
                            thrpt:  [789.59 MiB/s 790.24 MiB/s 790.87 MiB/s]
                     change:
                            time:   [+19311% +19330% +19347%] (p = 0.00 < 0.05)
                            thrpt:  [-99.486% -99.485% -99.485%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    standalone/heavy/find_context_bullet_items_cold
                            time:   [206.73 ms 207.05 ms 207.36 ms]
                            thrpt:  [724.37 MiB/s 725.46 MiB/s 726.59 MiB/s]
                     change:
                            time:   [+17646% +17675% +17703%] (p = 0.00 < 0.05)
                            thrpt:  [-99.438% -99.437% -99.437%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low mild
      1 (5.00%) high mild
    standalone/heavy/find_context_ordered_items_cold
                            time:   [202.30 ms 202.48 ms 202.67 ms]
                            thrpt:  [741.13 MiB/s 741.84 MiB/s 742.50 MiB/s]
                     change:
                            time:   [+18008% +18033% +18058%] (p = 0.00 < 0.05)
                            thrpt:  [-99.449% -99.449% -99.448%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    ```

</details>

### nightly - `RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_standalone --features avx2`

**small (`--features benches/small`):**

#### Standalone - `find_*`
| Function               | `plain`                    | `hot`                      | `heavy`                    |
|------------------------|----------------------------|----------------------------|----------------------------|
| `find_codes`           | [29.178 µs]=[93.769 GiB/s] | [68.952 µs]=[11.219 GiB/s] | [192.51 µs]=[7.6198 GiB/s] |
| `find_italics`         | [29.420 µs]=[93.000 GiB/s] | [123.35 µs]=[6.2710 GiB/s] | [347.58 µs]=[4.2201 GiB/s] |
| `find_bolds`           | [33.449 µs]=[81.796 GiB/s] | [128.12 µs]=[6.0379 GiB/s] | [355.98 µs]=[4.1206 GiB/s] |
| `find_bold_italics`    | [30.468 µs]=[89.800 GiB/s] | [112.18 µs]=[6.8956 GiB/s] | [339.97 µs]=[4.3147 GiB/s] |
| `find_autolinks`       | [35.478 µs]=[77.120 GiB/s] | [68.536 µs]=[11.287 GiB/s] | [63.390 µs]=[23.140 GiB/s] |
| `find_links`           | [34.946 µs]=[78.294 GiB/s] | [167.76 µs]=[4.6112 GiB/s] | [207.61 µs]=[7.0655 GiB/s] |
| `find_headings`        | [33.463 µs]=[81.763 GiB/s] | [74.350 µs]=[10.404 GiB/s] | [39.294 µs]=[37.330 GiB/s] |
| `find_thematic_breaks` | [41.501 µs]=[65.928 GiB/s] | [71.912 µs]=[10.757 GiB/s] | [272.00 µs]=[5.3929 GiB/s] |
| `find_fenced_codes`    | [30.704 µs]=[89.109 GiB/s] | [61.454 µs]=[12.588 GiB/s] | [222.53 µs]=[6.5918 GiB/s] |
| `find_blockquotes`     | [32.917 µs]=[83.119 GiB/s] | [36.645 µs]=[21.110 GiB/s] | [101.90 µs]=[14.395 GiB/s] |
| `find_bullet_items`    | [44.450 µs]=[61.553 GiB/s] | [68.414 µs]=[11.307 GiB/s] | [230.88 µs]=[6.3532 GiB/s] |
| `find_ordered_items`   | [254.62 µs]=[10.746 GiB/s] | [94.613 µs]=[8.1760 GiB/s] | [202.56 µs]=[7.2415 GiB/s] |
 
#### Context-aware - `context()` + `find_context_*`
| Function                       | `plain`                    | `hot`                      | `heavy`                    |
|--------------------------------|----------------------------|----------------------------|----------------------------|
| `context`                      | [41.234 µs]=[66.354 GiB/s] | [428.76 µs]=[1.8042 GiB/s] | [715.76 µs]=[2.0494 GiB/s] |
| `find_context_italics`         | [31.926 µs]=[85.698 GiB/s] | [145.99 µs]=[5.2987 GiB/s] | [402.60 µs]=[3.6434 GiB/s] |
| `find_context_bolds`           | [31.686 µs]=[86.348 GiB/s] | [149.61 µs]=[5.1703 GiB/s] | [470.08 µs]=[3.1204 GiB/s] |
| `find_context_bold_italics`    | [33.013 µs]=[82.877 GiB/s] | [129.36 µs]=[5.9800 GiB/s] | [390.03 µs]=[3.7609 GiB/s] |
| `find_context_headings`        | [31.814 µs]=[86.002 GiB/s] | [98.284 µs]=[7.8706 GiB/s] | [61.427 µs]=[23.879 GiB/s] |
| `find_context_thematic_breaks` | [41.950 µs]=[65.221 GiB/s] | [71.879 µs]=[10.762 GiB/s] | [312.78 µs]=[4.6898 GiB/s] |
| `find_context_blockquotes`     | [31.383 µs]=[87.181 GiB/s] | [36.489 µs]=[21.200 GiB/s] | [118.12 µs]=[12.418 GiB/s] |
| `find_context_bullet_items`    | [42.141 µs]=[64.926 GiB/s] | [68.107 µs]=[11.358 GiB/s] | [279.10 µs]=[5.2556 GiB/s] |
| `find_context_ordered_items`   | [254.54 µs]=[10.749 GiB/s] | [94.537 µs]=[8.1826 GiB/s] | [248.93 µs]=[5.8927 GiB/s] |
 
#### Cold context - `find_context_*_cold`
| Function                            | `plain`                    | `hot`                      | `heavy`                    |
|-------------------------------------|----------------------------|----------------------------|----------------------------|
| `find_context_italics_cold`         | [75.725 µs]=[36.131 GiB/s] | [583.20 µs]=[1.3264 GiB/s] | [1.1433 ms]=[1.2830 GiB/s] |
| `find_context_bolds_cold`           | [77.653 µs]=[35.234 GiB/s] | [589.26 µs]=[1.3128 GiB/s] | [1.2108 ms]=[1.2114 GiB/s] |
| `find_context_bold_italics_cold`    | [80.361 µs]=[34.047 GiB/s] | [569.24 µs]=[1.3589 GiB/s] | [1.1279 ms]=[1.3006 GiB/s] |
| `find_context_headings_cold`        | [76.818 µs]=[35.617 GiB/s] | [529.66 µs]=[1.4605 GiB/s] | [777.92 µs]=[1.8856 GiB/s] |
| `find_context_thematic_breaks_cold` | [89.829 µs]=[30.458 GiB/s] | [499.60 µs]=[1.5483 GiB/s] | [1.0264 ms]=[1.4291 GiB/s] |
| `find_context_blockquotes_cold`     | [69.158 µs]=[39.562 GiB/s] | [464.14 µs]=[1.6666 GiB/s] | [830.89 µs]=[1.7654 GiB/s] |
| `find_context_bullet_items_cold`    | [85.383 µs]=[32.044 GiB/s] | [495.44 µs]=[1.5613 GiB/s] | [994.81 µs]=[1.4745 GiB/s] |
| `find_context_ordered_items_cold`   | [320.49 µs]=[8.5370 GiB/s] | [521.10 µs]=[1.4845 GiB/s] | [965.24 µs]=[1.5197 GiB/s] |
 


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
                            time:   [28.420 µs 29.178 µs 30.265 µs]
                            thrpt:  [90.403 GiB/s 93.769 GiB/s 96.270 GiB/s]
        find_italics       full=       0  standalone=       0
    standalone/plain/find_italics
                            time:   [29.169 µs 29.420 µs 29.750 µs]
                            thrpt:  [91.968 GiB/s 93.000 GiB/s 93.798 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_bolds         full=       0  standalone=       0
    standalone/plain/find_bolds
                            time:   [32.126 µs 33.449 µs 34.688 µs]
                            thrpt:  [78.876 GiB/s 81.796 GiB/s 85.165 GiB/s]
        find_bold_italics  full=       0  standalone=       0
    standalone/plain/find_bold_italics
                            time:   [29.114 µs 30.468 µs 31.781 µs]
                            thrpt:  [86.090 GiB/s 89.800 GiB/s 93.977 GiB/s]
        find_autolinks     full=       0  standalone=       0
    standalone/plain/find_autolinks
                            time:   [33.651 µs 35.478 µs 37.284 µs]
                            thrpt:  [73.384 GiB/s 77.120 GiB/s 81.306 GiB/s]
        find_links         full=       0  standalone=       0
    standalone/plain/find_links
                            time:   [33.982 µs 34.946 µs 35.852 µs]
                            thrpt:  [76.314 GiB/s 78.294 GiB/s 80.514 GiB/s]
        find_headings      full=       0  standalone=       0
    standalone/plain/find_headings
                            time:   [31.797 µs 33.463 µs 35.397 µs]
                            thrpt:  [77.295 GiB/s 81.763 GiB/s 86.046 GiB/s]
        find_thematic_breaks full=       0  standalone=       0
    standalone/plain/find_thematic_breaks
                            time:   [40.379 µs 41.501 µs 42.485 µs]
                            thrpt:  [64.401 GiB/s 65.928 GiB/s 67.760 GiB/s]
        find_fenced_codes  full=       0  standalone=       0
    standalone/plain/find_fenced_codes
                            time:   [29.724 µs 30.704 µs 32.052 µs]
                            thrpt:  [85.362 GiB/s 89.109 GiB/s 92.050 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_blockquotes   full=       0  standalone=       0
    standalone/plain/find_blockquotes
                            time:   [31.356 µs 32.917 µs 34.640 µs]
                            thrpt:  [78.985 GiB/s 83.119 GiB/s 87.257 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_bullet_items  full=       0  standalone=       0
    standalone/plain/find_bullet_items
                            time:   [42.875 µs 44.450 µs 45.628 µs]
                            thrpt:  [59.964 GiB/s 61.553 GiB/s 63.815 GiB/s]
        find_ordered_items full=       0  standalone=       0
    standalone/plain/find_ordered_items
                            time:   [254.48 µs 254.62 µs 254.75 µs]
                            thrpt:  [10.740 GiB/s 10.746 GiB/s 10.751 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) low mild
    │  context regions: 0
    standalone/plain/context
                            time:   [40.588 µs 41.234 µs 41.808 µs]
                            thrpt:  [65.442 GiB/s 66.354 GiB/s 67.410 GiB/s]
    │  full-vs-context-aware counts:
        find_context_italics full=       0  context-aware=       0
    standalone/plain/find_context_italics
                            time:   [30.451 µs 31.926 µs 33.633 µs]
                            thrpt:  [81.349 GiB/s 85.698 GiB/s 89.850 GiB/s]
        find_context_bolds full=       0  context-aware=       0
    standalone/plain/find_context_bolds
                            time:   [30.291 µs 31.686 µs 32.911 µs]
                            thrpt:  [83.134 GiB/s 86.348 GiB/s 90.325 GiB/s]
        find_context_bold_italics full=       0  context-aware=       0
    standalone/plain/find_context_bold_italics
                            time:   [31.620 µs 33.013 µs 34.191 µs]
                            thrpt:  [80.022 GiB/s 82.877 GiB/s 86.530 GiB/s]
        find_context_headings full=       0  context-aware=       0
    standalone/plain/find_context_headings
                            time:   [30.269 µs 31.814 µs 33.332 µs]
                            thrpt:  [82.084 GiB/s 86.002 GiB/s 90.391 GiB/s]
        find_context_thematic_breaks full=       0  context-aware=       0
    standalone/plain/find_context_thematic_breaks
                            time:   [40.700 µs 41.950 µs 43.231 µs]
                            thrpt:  [63.288 GiB/s 65.221 GiB/s 67.225 GiB/s]
        find_context_blockquotes full=       0  context-aware=       0
    standalone/plain/find_context_blockquotes
                            time:   [30.201 µs 31.383 µs 32.302 µs]
                            thrpt:  [84.703 GiB/s 87.181 GiB/s 90.594 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_context_bullet_items full=       0  context-aware=       0
    standalone/plain/find_context_bullet_items
                            time:   [41.021 µs 42.141 µs 43.484 µs]
                            thrpt:  [62.921 GiB/s 64.926 GiB/s 66.699 GiB/s]
        find_context_ordered_items full=       0  context-aware=       0
    standalone/plain/find_context_ordered_items
                            time:   [254.31 µs 254.54 µs 254.78 µs]
                            thrpt:  [10.739 GiB/s 10.749 GiB/s 10.759 GiB/s]
    standalone/plain/find_context_italics_cold
                            time:   [73.650 µs 75.725 µs 78.105 µs]
                            thrpt:  [35.030 GiB/s 36.131 GiB/s 37.149 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    standalone/plain/find_context_bolds_cold
                            time:   [73.923 µs 77.653 µs 81.281 µs]
                            thrpt:  [33.662 GiB/s 35.234 GiB/s 37.012 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      3 (15.00%) low mild
    standalone/plain/find_context_bold_italics_cold
                            time:   [77.354 µs 80.361 µs 83.326 µs]
                            thrpt:  [32.835 GiB/s 34.047 GiB/s 35.370 GiB/s]
    standalone/plain/find_context_headings_cold
                            time:   [73.158 µs 76.818 µs 81.165 µs]
                            thrpt:  [33.710 GiB/s 35.617 GiB/s 37.399 GiB/s]
    standalone/plain/find_context_thematic_breaks_cold
                            time:   [87.044 µs 89.829 µs 92.344 µs]
                            thrpt:  [29.629 GiB/s 30.458 GiB/s 31.433 GiB/s]
    standalone/plain/find_context_blockquotes_cold
                            time:   [68.783 µs 69.158 µs 69.405 µs]
                            thrpt:  [39.421 GiB/s 39.562 GiB/s 39.778 GiB/s]
    standalone/plain/find_context_bullet_items_cold
                            time:   [82.598 µs 85.383 µs 88.593 µs]
                            thrpt:  [30.883 GiB/s 32.044 GiB/s 33.125 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    standalone/plain/find_context_ordered_items_cold
                            time:   [310.36 µs 320.49 µs 333.87 µs]
                            thrpt:  [8.1949 GiB/s 8.5370 GiB/s 8.8156 GiB/s]
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
                            time:   [68.843 µs 68.952 µs 69.094 µs]
                            thrpt:  [11.196 GiB/s 11.219 GiB/s 11.237 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_italics       full=    5000  standalone=    5000
    standalone/hot/find_italics
                            time:   [123.28 µs 123.35 µs 123.41 µs]
                            thrpt:  [6.2683 GiB/s 6.2710 GiB/s 6.2749 GiB/s]
        find_bolds         full=    5000  standalone=    5000
    standalone/hot/find_bolds
                            time:   [128.04 µs 128.12 µs 128.20 µs]
                            thrpt:  [6.0341 GiB/s 6.0379 GiB/s 6.0414 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_bold_italics  full=       0  standalone=       0
    standalone/hot/find_bold_italics
                            time:   [112.10 µs 112.18 µs 112.26 µs]
                            thrpt:  [6.8906 GiB/s 6.8956 GiB/s 6.9005 GiB/s]
        find_autolinks     full=    5000  standalone=    5000
    standalone/hot/find_autolinks
                            time:   [68.499 µs 68.536 µs 68.566 µs]
                            thrpt:  [11.282 GiB/s 11.287 GiB/s 11.293 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) low mild
        find_links         full=    5000  standalone=    5000
    standalone/hot/find_links
                            time:   [167.70 µs 167.76 µs 167.82 µs]
                            thrpt:  [4.6094 GiB/s 4.6112 GiB/s 4.6128 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) low mild
      1 (5.00%) high mild
        find_headings      full=    5000  standalone=    5000
    standalone/hot/find_headings
                            time:   [74.306 µs 74.350 µs 74.384 µs]
                            thrpt:  [10.400 GiB/s 10.404 GiB/s 10.410 GiB/s]
        find_thematic_breaks full=       0  standalone=       0
    standalone/hot/find_thematic_breaks
                            time:   [71.785 µs 71.912 µs 72.007 µs]
                            thrpt:  [10.743 GiB/s 10.757 GiB/s 10.776 GiB/s]
        find_fenced_codes  full=       0  standalone=       0
    standalone/hot/find_fenced_codes
                            time:   [61.235 µs 61.454 µs 61.634 µs]
                            thrpt:  [12.551 GiB/s 12.588 GiB/s 12.633 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      3 (15.00%) high mild
        find_blockquotes   full=       0  standalone=       0
    standalone/hot/find_blockquotes
                            time:   [36.563 µs 36.645 µs 36.755 µs]
                            thrpt:  [21.046 GiB/s 21.110 GiB/s 21.157 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_bullet_items  full=       0  standalone=       0
    standalone/hot/find_bullet_items
                            time:   [68.262 µs 68.414 µs 68.590 µs]
                            thrpt:  [11.278 GiB/s 11.307 GiB/s 11.332 GiB/s]
        find_ordered_items full=       0  standalone=       0
    standalone/hot/find_ordered_items
                            time:   [94.539 µs 94.613 µs 94.692 µs]
                            thrpt:  [8.1692 GiB/s 8.1760 GiB/s 8.1824 GiB/s]
    │  context regions: 15000
    standalone/hot/context  time:   [427.71 µs 428.76 µs 429.85 µs]
                            thrpt:  [1.7996 GiB/s 1.8042 GiB/s 1.8086 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    │  full-vs-context-aware counts:
        find_context_italics full=    5000  context-aware=    5000
    standalone/hot/find_context_italics
                            time:   [145.88 µs 145.99 µs 146.10 µs]
                            thrpt:  [5.2946 GiB/s 5.2987 GiB/s 5.3027 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low mild
      1 (5.00%) high mild
        find_context_bolds full=    5000  context-aware=    5000
    standalone/hot/find_context_bolds
                            time:   [149.48 µs 149.61 µs 149.76 µs]
                            thrpt:  [5.1652 GiB/s 5.1703 GiB/s 5.1750 GiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      2 (10.00%) low mild
      2 (10.00%) high severe
        find_context_bold_italics full=       0  context-aware=       0
    standalone/hot/find_context_bold_italics
                            time:   [129.23 µs 129.36 µs 129.46 µs]
                            thrpt:  [5.9754 GiB/s 5.9800 GiB/s 5.9859 GiB/s]
        find_context_headings full=    5000  context-aware=    5000
    standalone/hot/find_context_headings
                            time:   [97.743 µs 98.284 µs 98.730 µs]
                            thrpt:  [7.8351 GiB/s 7.8706 GiB/s 7.9142 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      3 (15.00%) low mild
        find_context_thematic_breaks full=       0  context-aware=       0
    standalone/hot/find_context_thematic_breaks
                            time:   [71.817 µs 71.879 µs 71.942 µs]
                            thrpt:  [10.753 GiB/s 10.762 GiB/s 10.771 GiB/s]
        find_context_blockquotes full=       0  context-aware=       0
    standalone/hot/find_context_blockquotes
                            time:   [36.429 µs 36.489 µs 36.564 µs]
                            thrpt:  [21.156 GiB/s 21.200 GiB/s 21.235 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      3 (15.00%) high severe
        find_context_bullet_items full=       0  context-aware=       0
    standalone/hot/find_context_bullet_items
                            time:   [68.045 µs 68.107 µs 68.167 µs]
                            thrpt:  [11.348 GiB/s 11.358 GiB/s 11.368 GiB/s]
        find_context_ordered_items full=       0  context-aware=       0
    standalone/hot/find_context_ordered_items
                            time:   [94.451 µs 94.537 µs 94.627 µs]
                            thrpt:  [8.1748 GiB/s 8.1826 GiB/s 8.1900 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    standalone/hot/find_context_italics_cold
                            time:   [582.92 µs 583.20 µs 583.50 µs]
                            thrpt:  [1.3257 GiB/s 1.3264 GiB/s 1.3270 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low mild
      1 (5.00%) high mild
    standalone/hot/find_context_bolds_cold
                            time:   [588.75 µs 589.26 µs 589.57 µs]
                            thrpt:  [1.3121 GiB/s 1.3128 GiB/s 1.3139 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low severe
      1 (5.00%) high mild
    standalone/hot/find_context_bold_italics_cold
                            time:   [568.69 µs 569.24 µs 570.11 µs]
                            thrpt:  [1.3569 GiB/s 1.3589 GiB/s 1.3602 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
    standalone/hot/find_context_headings_cold
                            time:   [529.00 µs 529.66 µs 530.19 µs]
                            thrpt:  [1.4590 GiB/s 1.4605 GiB/s 1.4623 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low severe
      1 (5.00%) low mild
      1 (5.00%) high mild
    standalone/hot/find_context_thematic_breaks_cold
                            time:   [499.38 µs 499.60 µs 499.82 µs]
                            thrpt:  [1.5477 GiB/s 1.5483 GiB/s 1.5490 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
    standalone/hot/find_context_blockquotes_cold
                            time:   [463.90 µs 464.14 µs 464.49 µs]
                            thrpt:  [1.6654 GiB/s 1.6666 GiB/s 1.6675 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    standalone/hot/find_context_bullet_items_cold
                            time:   [495.27 µs 495.44 µs 495.57 µs]
                            thrpt:  [1.5610 GiB/s 1.5613 GiB/s 1.5619 GiB/s]
    standalone/hot/find_context_ordered_items_cold
                            time:   [520.80 µs 521.10 µs 521.39 µs]
                            thrpt:  [1.4837 GiB/s 1.4845 GiB/s 1.4853 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) low mild
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
                            time:   [192.41 µs 192.51 µs 192.61 µs]
                            thrpt:  [7.6156 GiB/s 7.6198 GiB/s 7.6237 GiB/s]
        find_italics       full=   12000  standalone=   12000
    standalone/heavy/find_italics
                            time:   [347.46 µs 347.58 µs 347.70 µs]
                            thrpt:  [4.2187 GiB/s 4.2201 GiB/s 4.2217 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      3 (15.00%) low mild
        find_bolds         full=   12000  standalone=   12000
    standalone/heavy/find_bolds
                            time:   [355.86 µs 355.98 µs 356.12 µs]
                            thrpt:  [4.1190 GiB/s 4.1206 GiB/s 4.1219 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
        find_bold_italics  full=    6000  standalone=    6000
    standalone/heavy/find_bold_italics
                            time:   [339.78 µs 339.97 µs 340.13 µs]
                            thrpt:  [4.3126 GiB/s 4.3147 GiB/s 4.3170 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_autolinks     full=    4000  standalone=    4000
    standalone/heavy/find_autolinks
                            time:   [63.020 µs 63.390 µs 63.916 µs]
                            thrpt:  [22.950 GiB/s 23.140 GiB/s 23.276 GiB/s]
        find_links         full=    6000  standalone=    6000
    standalone/heavy/find_links
                            time:   [207.45 µs 207.61 µs 207.73 µs]
                            thrpt:  [7.0613 GiB/s 7.0655 GiB/s 7.0710 GiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      3 (15.00%) high mild
      1 (5.00%) high severe
        find_headings      full=    2000  standalone=    2000
    standalone/heavy/find_headings
                            time:   [38.828 µs 39.294 µs 39.772 µs]
                            thrpt:  [36.881 GiB/s 37.330 GiB/s 37.778 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
        find_thematic_breaks full=    2000  standalone=    2000
    standalone/heavy/find_thematic_breaks
                            time:   [271.87 µs 272.00 µs 272.13 µs]
                            thrpt:  [5.3903 GiB/s 5.3929 GiB/s 5.3954 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_fenced_codes  full=    4000  standalone=    4000
    standalone/heavy/find_fenced_codes
                            time:   [222.18 µs 222.53 µs 222.88 µs]
                            thrpt:  [6.5812 GiB/s 6.5918 GiB/s 6.6020 GiB/s]
        find_blockquotes   full=    4000  standalone=    4000
    standalone/heavy/find_blockquotes
                            time:   [101.77 µs 101.90 µs 102.01 µs]
                            thrpt:  [14.380 GiB/s 14.395 GiB/s 14.414 GiB/s]
        find_bullet_items  full=    6000  standalone=    6000
    standalone/heavy/find_bullet_items
                            time:   [230.82 µs 230.88 µs 230.95 µs]
                            thrpt:  [6.3514 GiB/s 6.3532 GiB/s 6.3549 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      3 (15.00%) high mild
        find_ordered_items full=    4000  standalone=    4000
    standalone/heavy/find_ordered_items
                            time:   [202.27 µs 202.56 µs 203.01 µs]
                            thrpt:  [7.2255 GiB/s 7.2415 GiB/s 7.2521 GiB/s]
    │  context regions: 24000
    standalone/heavy/context
                            time:   [715.43 µs 715.76 µs 716.10 µs]
                            thrpt:  [2.0484 GiB/s 2.0494 GiB/s 2.0503 GiB/s]
    │  full-vs-context-aware counts:
        find_context_italics full=   12000  context-aware=   12000
    standalone/heavy/find_context_italics
                            time:   [402.48 µs 402.60 µs 402.75 µs]
                            thrpt:  [3.6420 GiB/s 3.6434 GiB/s 3.6445 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_context_bolds full=   12000  context-aware=   12000
    standalone/heavy/find_context_bolds
                            time:   [469.77 µs 470.08 µs 470.29 µs]
                            thrpt:  [3.1191 GiB/s 3.1204 GiB/s 3.1225 GiB/s]
        find_context_bold_italics full=    6000  context-aware=    6000
    standalone/heavy/find_context_bold_italics
                            time:   [389.85 µs 390.03 µs 390.30 µs]
                            thrpt:  [3.7582 GiB/s 3.7609 GiB/s 3.7626 GiB/s]
        find_context_headings full=    2000  context-aware=    2000
    standalone/heavy/find_context_headings
                            time:   [60.987 µs 61.427 µs 61.700 µs]
                            thrpt:  [23.774 GiB/s 23.879 GiB/s 24.052 GiB/s]
        find_context_thematic_breaks full=    2000  context-aware=    2000
    standalone/heavy/find_context_thematic_breaks
                            time:   [312.32 µs 312.78 µs 313.11 µs]
                            thrpt:  [4.6847 GiB/s 4.6898 GiB/s 4.6967 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      3 (15.00%) high mild
        find_context_blockquotes full=    4000  context-aware=    4000
    standalone/heavy/find_context_blockquotes
                            time:   [118.04 µs 118.12 µs 118.18 µs]
                            thrpt:  [12.412 GiB/s 12.418 GiB/s 12.426 GiB/s]
        find_context_bullet_items full=    6000  context-aware=    6000
    standalone/heavy/find_context_bullet_items
                            time:   [278.91 µs 279.10 µs 279.39 µs]
                            thrpt:  [5.2502 GiB/s 5.2556 GiB/s 5.2591 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_context_ordered_items full=    4000  context-aware=    4000
    standalone/heavy/find_context_ordered_items
                            time:   [248.60 µs 248.93 µs 249.25 µs]
                            thrpt:  [5.8852 GiB/s 5.8927 GiB/s 5.9004 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) high mild
      1 (5.00%) high severe
    standalone/heavy/find_context_italics_cold
                            time:   [1.1424 ms 1.1433 ms 1.1445 ms]
                            thrpt:  [1.2816 GiB/s 1.2830 GiB/s 1.2841 GiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      1 (5.00%) low mild
      2 (10.00%) high mild
      1 (5.00%) high severe
    standalone/heavy/find_context_bolds_cold
                            time:   [1.2106 ms 1.2108 ms 1.2111 ms]
                            thrpt:  [1.2112 GiB/s 1.2114 GiB/s 1.2117 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) low severe
    standalone/heavy/find_context_bold_italics_cold
                            time:   [1.1271 ms 1.1279 ms 1.1287 ms]
                            thrpt:  [1.2996 GiB/s 1.3006 GiB/s 1.3014 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    standalone/heavy/find_context_headings_cold
                            time:   [777.36 µs 777.92 µs 778.64 µs]
                            thrpt:  [1.8839 GiB/s 1.8856 GiB/s 1.8870 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) high mild
      2 (10.00%) high severe
    standalone/heavy/find_context_thematic_breaks_cold
                            time:   [1.0260 ms 1.0264 ms 1.0267 ms]
                            thrpt:  [1.4287 GiB/s 1.4291 GiB/s 1.4297 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    standalone/heavy/find_context_blockquotes_cold
                            time:   [830.32 µs 830.89 µs 831.33 µs]
                            thrpt:  [1.7645 GiB/s 1.7654 GiB/s 1.7666 GiB/s]
    standalone/heavy/find_context_bullet_items_cold
                            time:   [994.35 µs 994.81 µs 995.38 µs]
                            thrpt:  [1.4737 GiB/s 1.4745 GiB/s 1.4752 GiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      1 (5.00%) low mild
      2 (10.00%) high mild
      1 (5.00%) high severe
    standalone/heavy/find_context_ordered_items_cold
                            time:   [964.89 µs 965.24 µs 965.60 µs]
                            thrpt:  [1.5191 GiB/s 1.5197 GiB/s 1.5202 GiB/s]
    ```

</details>

**big (`--features benches/big`):**

#### Standalone - `find_*`
| Function               | `plain`                    | `hot`                      | `heavy`                    |
|------------------------|----------------------------|----------------------------|----------------------------|
| `find_codes`           | [11.612 ms]=[23.562 GiB/s] | [7.8351 ms]=[9.8730 GiB/s] | [20.364 ms]=[7.2032 GiB/s] |
| `find_italics`         | [11.619 ms]=[23.547 GiB/s] | [13.099 ms]=[5.9053 GiB/s] | [35.559 ms]=[4.1252 GiB/s] |
| `find_bolds`           | [11.610 ms]=[23.566 GiB/s] | [13.426 ms]=[5.7615 GiB/s] | [36.352 ms]=[4.0351 GiB/s] |
| `find_bold_italics`    | [11.630 ms]=[23.526 GiB/s] | [11.797 ms]=[6.5574 GiB/s] | [34.840 ms]=[4.2102 GiB/s] |
| `find_autolinks`       | [11.646 ms]=[23.494 GiB/s] | [7.8250 ms]=[9.8858 GiB/s] | [9.3116 ms]=[15.753 GiB/s] |
| `find_links`           | [11.615 ms]=[23.557 GiB/s] | [17.201 ms]=[4.4972 GiB/s] | [21.671 ms]=[6.7689 GiB/s] |
| `find_headings`        | [11.617 ms]=[23.551 GiB/s] | [8.1307 ms]=[9.5140 GiB/s] | [8.2296 ms]=[17.824 GiB/s] |
| `find_thematic_breaks` | [13.132 ms]=[20.834 GiB/s] | [7.8523 ms]=[9.8514 GiB/s] | [27.922 ms]=[5.2533 GiB/s] |
| `find_fenced_codes`    | [11.635 ms]=[23.515 GiB/s] | [7.1849 ms]=[10.766 GiB/s] | [23.193 ms]=[6.3247 GiB/s] |
| `find_blockquotes`     | [11.640 ms]=[23.506 GiB/s] | [4.8449 ms]=[15.967 GiB/s] | [12.752 ms]=[11.503 GiB/s] |
| `find_bullet_items`    | [13.115 ms]=[20.863 GiB/s] | [7.6560 ms]=[10.104 GiB/s] | [23.835 ms]=[6.1542 GiB/s] |
| `find_ordered_items`   | [27.725 ms]=[9.8687 GiB/s] | [10.052 ms]=[7.6955 GiB/s] | [21.109 ms]=[6.9488 GiB/s] |
 
#### Context-aware - `context()` + `find_context_*`
| Function                       | `plain`                    | `hot`                      | `heavy`                    |
|--------------------------------|----------------------------|----------------------------|----------------------------|
| `context`                      | [13.204 ms]=[20.722 GiB/s] | [80.995 ms]=[977.98 MiB/s] | [155.65 ms]=[965.01 MiB/s] |
| `find_context_italics`         | [11.610 ms]=[23.566 GiB/s] | [15.145 ms]=[5.1075 GiB/s] | [41.199 ms]=[3.5604 GiB/s] |
| `find_context_bolds`           | [11.578 ms]=[23.632 GiB/s] | [15.770 ms]=[4.9052 GiB/s] | [47.673 ms]=[3.0769 GiB/s] |
| `find_context_bold_italics`    | [11.570 ms]=[23.648 GiB/s] | [13.565 ms]=[5.7026 GiB/s] | [39.618 ms]=[3.7025 GiB/s] |
| `find_context_headings`        | [11.568 ms]=[23.652 GiB/s] | [10.563 ms]=[7.3230 GiB/s] | [9.5651 ms]=[15.335 GiB/s] |
| `find_context_thematic_breaks` | [13.106 ms]=[20.877 GiB/s] | [7.7966 ms]=[9.9217 GiB/s] | [32.362 ms]=[4.5326 GiB/s] |
| `find_context_blockquotes`     | [11.571 ms]=[23.645 GiB/s] | [4.8014 ms]=[16.111 GiB/s] | [13.454 ms]=[10.903 GiB/s] |
| `find_context_bullet_items`    | [13.131 ms]=[20.836 GiB/s] | [7.6478 ms]=[10.115 GiB/s] | [28.501 ms]=[5.1466 GiB/s] |
| `find_context_ordered_items`   | [27.857 ms]=[9.8218 GiB/s] | [9.9981 ms]=[7.7370 GiB/s] | [25.951 ms]=[5.6525 GiB/s] |
 
#### Cold context - `find_context_*_cold`
| Function                            | `plain`                    | `hot`                      | `heavy`                    |
|-------------------------------------|----------------------------|----------------------------|----------------------------|
| `find_context_italics_cold`         | [24.742 ms]=[11.058 GiB/s] | [98.158 ms]=[806.99 MiB/s] | [198.84 ms]=[755.42 MiB/s] |
| `find_context_bolds_cold`           | [24.773 ms]=[11.044 GiB/s] | [97.959 ms]=[808.63 MiB/s] | [205.75 ms]=[730.03 MiB/s] |
| `find_context_bold_italics_cold`    | [24.794 ms]=[11.035 GiB/s] | [96.465 ms]=[821.15 MiB/s] | [196.85 ms]=[763.05 MiB/s] |
| `find_context_headings_cold`        | [24.779 ms]=[11.042 GiB/s] | [91.414 ms]=[866.53 MiB/s] | [166.31 ms]=[903.18 MiB/s] |
| `find_context_thematic_breaks_cold` | [26.277 ms]=[10.412 GiB/s] | [89.825 ms]=[881.85 MiB/s] | [187.40 ms]=[801.51 MiB/s] |
| `find_context_blockquotes_cold`     | [24.791 ms]=[11.036 GiB/s] | [86.891 ms]=[911.63 MiB/s] | [170.06 ms]=[883.23 MiB/s] |
| `find_context_bullet_items_cold`    | [26.320 ms]=[10.395 GiB/s] | [90.149 ms]=[878.68 MiB/s] | [184.17 ms]=[815.58 MiB/s] |
| `find_context_ordered_items_cold`   | [41.139 ms]=[6.6506 GiB/s] | [92.096 ms]=[860.10 MiB/s] | [182.04 ms]=[825.11 MiB/s] |

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
                            time:   [11.603 ms 11.612 ms 11.624 ms]
                            thrpt:  [23.537 GiB/s 23.562 GiB/s 23.581 GiB/s]
                     change:
                            time:   [+36850% +38025% +39266%] (p = 0.00 < 0.05)
                            thrpt:  [-99.746% -99.738% -99.729%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_italics       full=       0  standalone=       0
    standalone/plain/find_italics
                            time:   [11.607 ms 11.619 ms 11.634 ms]
                            thrpt:  [23.518 GiB/s 23.547 GiB/s 23.573 GiB/s]
                     change:
                            time:   [+38895% +39348% +39793%] (p = 0.00 < 0.05)
                            thrpt:  [-99.749% -99.747% -99.744%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_bolds         full=       0  standalone=       0
    standalone/plain/find_bolds
                            time:   [11.593 ms 11.610 ms 11.626 ms]
                            thrpt:  [23.534 GiB/s 23.566 GiB/s 23.600 GiB/s]
                     change:
                            time:   [+31985% +33444% +35090%] (p = 0.00 < 0.05)
                            thrpt:  [-99.716% -99.702% -99.688%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_bold_italics  full=       0  standalone=       0
    standalone/plain/find_bold_italics
                            time:   [11.618 ms 11.630 ms 11.643 ms]
                            thrpt:  [23.500 GiB/s 23.526 GiB/s 23.551 GiB/s]
                     change:
                            time:   [+36182% +37452% +38817%] (p = 0.00 < 0.05)
                            thrpt:  [-99.743% -99.734% -99.724%]
                            Performance has regressed.
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low mild
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_autolinks     full=       0  standalone=       0
    standalone/plain/find_autolinks
                            time:   [11.625 ms 11.646 ms 11.665 ms]
                            thrpt:  [23.455 GiB/s 23.494 GiB/s 23.536 GiB/s]
                     change:
                            time:   [+32679% +34073% +35550%] (p = 0.00 < 0.05)
                            thrpt:  [-99.719% -99.707% -99.695%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_links         full=       0  standalone=       0
    standalone/plain/find_links
                            time:   [11.602 ms 11.615 ms 11.627 ms]
                            thrpt:  [23.531 GiB/s 23.557 GiB/s 23.583 GiB/s]
                     change:
                            time:   [+32108% +32956% +33876%] (p = 0.00 < 0.05)
                            thrpt:  [-99.706% -99.697% -99.690%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_headings      full=       0  standalone=       0
    standalone/plain/find_headings
                            time:   [11.603 ms 11.617 ms 11.632 ms]
                            thrpt:  [23.522 GiB/s 23.551 GiB/s 23.581 GiB/s]
                     change:
                            time:   [+32240% +33651% +35161%] (p = 0.00 < 0.05)
                            thrpt:  [-99.716% -99.704% -99.691%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_thematic_breaks full=       0  standalone=       0
    standalone/plain/find_thematic_breaks
                            time:   [13.109 ms 13.132 ms 13.157 ms]
                            thrpt:  [20.795 GiB/s 20.834 GiB/s 20.872 GiB/s]
                     change:
                            time:   [+31519% +32285% +33070%] (p = 0.00 < 0.05)
                            thrpt:  [-99.699% -99.691% -99.684%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_fenced_codes  full=       0  standalone=       0
    standalone/plain/find_fenced_codes
                            time:   [11.613 ms 11.635 ms 11.658 ms]
                            thrpt:  [23.470 GiB/s 23.515 GiB/s 23.560 GiB/s]
                     change:
                            time:   [+34480% +36366% +38084%] (p = 0.00 < 0.05)
                            thrpt:  [-99.738% -99.726% -99.711%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_blockquotes   full=       0  standalone=       0
    standalone/plain/find_blockquotes
                            time:   [11.626 ms 11.640 ms 11.656 ms]
                            thrpt:  [23.474 GiB/s 23.506 GiB/s 23.535 GiB/s]
                     change:
                            time:   [+34445% +35905% +37398%] (p = 0.00 < 0.05)
                            thrpt:  [-99.733% -99.722% -99.711%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_bullet_items  full=       0  standalone=       0
    standalone/plain/find_bullet_items
                            time:   [13.095 ms 13.115 ms 13.134 ms]
                            thrpt:  [20.832 GiB/s 20.863 GiB/s 20.894 GiB/s]
                     change:
                            time:   [+30141% +31207% +32276%] (p = 0.00 < 0.05)
                            thrpt:  [-99.691% -99.681% -99.669%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
        find_ordered_items full=       0  standalone=       0
    standalone/plain/find_ordered_items
                            time:   [27.711 ms 27.725 ms 27.739 ms]
                            thrpt:  [9.8634 GiB/s 9.8687 GiB/s 9.8736 GiB/s]
                     change:
                            time:   [+10785% +10807% +10840%] (p = 0.00 < 0.05)
                            thrpt:  [-99.086% -99.083% -99.081%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low mild
      1 (5.00%) high severe
    │  context regions: 0
    standalone/plain/context
                            time:   [13.192 ms 13.204 ms 13.219 ms]
                            thrpt:  [20.698 GiB/s 20.722 GiB/s 20.740 GiB/s]
                     change:
                            time:   [+30784% +31552% +32355%] (p = 0.00 < 0.05)
                            thrpt:  [-99.692% -99.684% -99.676%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    │  full-vs-context-aware counts:
        find_context_italics full=       0  context-aware=       0
    standalone/plain/find_context_italics
                            time:   [11.599 ms 11.610 ms 11.624 ms]
                            thrpt:  [23.539 GiB/s 23.566 GiB/s 23.588 GiB/s]
                     change:
                            time:   [+34520% +35648% +36868%] (p = 0.00 < 0.05)
                            thrpt:  [-99.729% -99.720% -99.711%]
                            Performance has regressed.
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) high mild
      1 (5.00%) high severe
        find_context_bolds full=       0  context-aware=       0
    standalone/plain/find_context_bolds
                            time:   [11.556 ms 11.578 ms 11.598 ms]
                            thrpt:  [23.590 GiB/s 23.632 GiB/s 23.676 GiB/s]
                     change:
                            time:   [+35284% +36508% +37844%] (p = 0.00 < 0.05)
                            thrpt:  [-99.736% -99.727% -99.717%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_context_bold_italics full=       0  context-aware=       0
    standalone/plain/find_context_bold_italics
                            time:   [11.554 ms 11.570 ms 11.589 ms]
                            thrpt:  [23.608 GiB/s 23.648 GiB/s 23.680 GiB/s]
                     change:
                            time:   [+33358% +34536% +35886%] (p = 0.00 < 0.05)
                            thrpt:  [-99.722% -99.711% -99.701%]
                            Performance has regressed.
    Found 4 outliers among 20 measurements (20.00%)
      2 (10.00%) high mild
      2 (10.00%) high severe
        find_context_headings full=       0  context-aware=       0
    standalone/plain/find_context_headings
                            time:   [11.557 ms 11.568 ms 11.581 ms]
                            thrpt:  [23.625 GiB/s 23.652 GiB/s 23.675 GiB/s]
                     change:
                            time:   [+35685% +37119% +38586%] (p = 0.00 < 0.05)
                            thrpt:  [-99.742% -99.731% -99.721%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low mild
      1 (5.00%) high severe
        find_context_thematic_breaks full=       0  context-aware=       0
    standalone/plain/find_context_thematic_breaks
                            time:   [13.093 ms 13.106 ms 13.118 ms]
                            thrpt:  [20.858 GiB/s 20.877 GiB/s 20.897 GiB/s]
                     change:
                            time:   [+30444% +31074% +31755%] (p = 0.00 < 0.05)
                            thrpt:  [-99.686% -99.679% -99.673%]
                            Performance has regressed.
    Found 4 outliers among 20 measurements (20.00%)
      1 (5.00%) low mild
      1 (5.00%) high mild
      2 (10.00%) high severe
        find_context_blockquotes full=       0  context-aware=       0
    standalone/plain/find_context_blockquotes
                            time:   [11.549 ms 11.571 ms 11.598 ms]
                            thrpt:  [23.590 GiB/s 23.645 GiB/s 23.690 GiB/s]
                     change:
                            time:   [+37586% +38649% +39695%] (p = 0.00 < 0.05)
                            thrpt:  [-99.749% -99.742% -99.735%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_context_bullet_items full=       0  context-aware=       0
    standalone/plain/find_context_bullet_items
                            time:   [13.095 ms 13.131 ms 13.170 ms]
                            thrpt:  [20.775 GiB/s 20.836 GiB/s 20.893 GiB/s]
                     change:
                            time:   [+29533% +30310% +31093%] (p = 0.00 < 0.05)
                            thrpt:  [-99.679% -99.671% -99.663%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_context_ordered_items full=       0  context-aware=       0
    standalone/plain/find_context_ordered_items
                            time:   [27.836 ms 27.857 ms 27.885 ms]
                            thrpt:  [9.8120 GiB/s 9.8218 GiB/s 9.8291 GiB/s]
                     change:
                            time:   [+10834% +10866% +10920%] (p = 0.00 < 0.05)
                            thrpt:  [-99.093% -99.088% -99.085%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    standalone/plain/find_context_italics_cold
                            time:   [24.700 ms 24.742 ms 24.783 ms]
                            thrpt:  [11.040 GiB/s 11.058 GiB/s 11.077 GiB/s]
                     change:
                            time:   [+32552% +33759% +34930%] (p = 0.00 < 0.05)
                            thrpt:  [-99.715% -99.705% -99.694%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    standalone/plain/find_context_bolds_cold
                            time:   [24.748 ms 24.773 ms 24.804 ms]
                            thrpt:  [11.031 GiB/s 11.044 GiB/s 11.056 GiB/s]
                     change:
                            time:   [+29758% +30785% +32013%] (p = 0.00 < 0.05)
                            thrpt:  [-99.689% -99.676% -99.665%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    standalone/plain/find_context_bold_italics_cold
                            time:   [24.760 ms 24.794 ms 24.838 ms]
                            thrpt:  [11.016 GiB/s 11.035 GiB/s 11.050 GiB/s]
                     change:
                            time:   [+30119% +30911% +31733%] (p = 0.00 < 0.05)
                            thrpt:  [-99.686% -99.678% -99.669%]
                            Performance has regressed.
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low mild
      1 (5.00%) high mild
      1 (5.00%) high severe
    standalone/plain/find_context_headings_cold
                            time:   [24.740 ms 24.779 ms 24.815 ms]
                            thrpt:  [11.026 GiB/s 11.042 GiB/s 11.059 GiB/s]
                     change:
                            time:   [+29563% +30754% +32076%] (p = 0.00 < 0.05)
                            thrpt:  [-99.689% -99.676% -99.663%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    standalone/plain/find_context_thematic_breaks_cold
                            time:   [26.241 ms 26.277 ms 26.322 ms]
                            thrpt:  [10.394 GiB/s 10.412 GiB/s 10.427 GiB/s]
                     change:
                            time:   [+28395% +29149% +29979%] (p = 0.00 < 0.05)
                            thrpt:  [-99.668% -99.658% -99.649%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    standalone/plain/find_context_blockquotes_cold
                            time:   [24.739 ms 24.791 ms 24.832 ms]
                            thrpt:  [11.018 GiB/s 11.036 GiB/s 11.060 GiB/s]
                     change:
                            time:   [+35926% +36332% +36781%] (p = 0.00 < 0.05)
                            thrpt:  [-99.729% -99.726% -99.722%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    standalone/plain/find_context_bullet_items_cold
                            time:   [26.281 ms 26.320 ms 26.376 ms]
                            thrpt:  [10.373 GiB/s 10.395 GiB/s 10.411 GiB/s]
                     change:
                            time:   [+29657% +30607% +31555%] (p = 0.00 < 0.05)
                            thrpt:  [-99.684% -99.674% -99.664%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    standalone/plain/find_context_ordered_items_cold
                            time:   [41.106 ms 41.139 ms 41.180 ms]
                            thrpt:  [6.6441 GiB/s 6.6506 GiB/s 6.6561 GiB/s]
                     change:
                            time:   [+12299% +12648% +13011%] (p = 0.00 < 0.05)
                            thrpt:  [-99.237% -99.216% -99.193%]
                            Performance has regressed.
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) high mild
      1 (5.00%) high severe
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
                            time:   [7.8223 ms 7.8351 ms 7.8457 ms]
                            thrpt:  [9.8596 GiB/s 9.8730 GiB/s 9.8891 GiB/s]
                     change:
                            time:   [+11243% +11277% +11322%] (p = 0.00 < 0.05)
                            thrpt:  [-99.124% -99.121% -99.118%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_italics       full=  500000  standalone=  500000
    standalone/hot/find_italics
                            time:   [13.080 ms 13.099 ms 13.120 ms]
                            thrpt:  [5.8961 GiB/s 5.9053 GiB/s 5.9141 GiB/s]
                     change:
                            time:   [+10516% +10552% +10605%] (p = 0.00 < 0.05)
                            thrpt:  [-99.066% -99.061% -99.058%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_bolds         full=  500000  standalone=  500000
    standalone/hot/find_bolds
                            time:   [13.415 ms 13.426 ms 13.439 ms]
                            thrpt:  [5.7563 GiB/s 5.7615 GiB/s 5.7663 GiB/s]
                     change:
                            time:   [+10368% +10385% +10402%] (p = 0.00 < 0.05)
                            thrpt:  [-99.048% -99.046% -99.045%]
                            Performance has regressed.
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) high mild
      2 (10.00%) high severe
        find_bold_italics  full=       0  standalone=       0
    standalone/hot/find_bold_italics
                            time:   [11.769 ms 11.797 ms 11.834 ms]
                            thrpt:  [6.5365 GiB/s 6.5574 GiB/s 6.5728 GiB/s]
                     change:
                            time:   [+10390% +10414% +10436%] (p = 0.00 < 0.05)
                            thrpt:  [-99.051% -99.049% -99.047%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
        find_autolinks     full=  500000  standalone=  500000
    standalone/hot/find_autolinks
                            time:   [7.8000 ms 7.8250 ms 7.8464 ms]
                            thrpt:  [9.8588 GiB/s 9.8858 GiB/s 9.9174 GiB/s]
                     change:
                            time:   [+11315% +11356% +11412%] (p = 0.00 < 0.05)
                            thrpt:  [-99.131% -99.127% -99.124%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_links         full=  500000  standalone=  500000
    standalone/hot/find_links
                            time:   [17.180 ms 17.201 ms 17.223 ms]
                            thrpt:  [4.4915 GiB/s 4.4972 GiB/s 4.5027 GiB/s]
                     change:
                            time:   [+10138% +10154% +10171%] (p = 0.00 < 0.05)
                            thrpt:  [-99.026% -99.025% -99.023%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_headings      full=  500000  standalone=  500000
    standalone/hot/find_headings
                            time:   [8.1199 ms 8.1307 ms 8.1426 ms]
                            thrpt:  [9.5001 GiB/s 9.5140 GiB/s 9.5267 GiB/s]
                     change:
                            time:   [+10833% +10854% +10885%] (p = 0.00 < 0.05)
                            thrpt:  [-99.090% -99.087% -99.085%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_thematic_breaks full=       0  standalone=       0
    standalone/hot/find_thematic_breaks
                            time:   [7.8432 ms 7.8523 ms 7.8625 ms]
                            thrpt:  [9.8386 GiB/s 9.8514 GiB/s 9.8627 GiB/s]
                     change:
                            time:   [+10830% +10858% +10891%] (p = 0.00 < 0.05)
                            thrpt:  [-99.090% -99.087% -99.085%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_fenced_codes  full=       0  standalone=       0
    standalone/hot/find_fenced_codes
                            time:   [7.1779 ms 7.1849 ms 7.1947 ms]
                            thrpt:  [10.752 GiB/s 10.766 GiB/s 10.777 GiB/s]
                     change:
                            time:   [+11607% +11642% +11680%] (p = 0.00 < 0.05)
                            thrpt:  [-99.151% -99.148% -99.146%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_blockquotes   full=       0  standalone=       0
    standalone/hot/find_blockquotes
                            time:   [4.8296 ms 4.8449 ms 4.8613 ms]
                            thrpt:  [15.913 GiB/s 15.967 GiB/s 16.017 GiB/s]
                     change:
                            time:   [+13101% +13173% +13259%] (p = 0.00 < 0.05)
                            thrpt:  [-99.251% -99.247% -99.242%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_bullet_items  full=       0  standalone=       0
    standalone/hot/find_bullet_items
                            time:   [7.6464 ms 7.6560 ms 7.6672 ms]
                            thrpt:  [10.089 GiB/s 10.104 GiB/s 10.117 GiB/s]
                     change:
                            time:   [+11077% +11107% +11140%] (p = 0.00 < 0.05)
                            thrpt:  [-99.110% -99.108% -99.105%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_ordered_items full=       0  standalone=       0
    standalone/hot/find_ordered_items
                            time:   [10.043 ms 10.052 ms 10.061 ms]
                            thrpt:  [7.6884 GiB/s 7.6955 GiB/s 7.7023 GiB/s]
                     change:
                            time:   [+10522% +10538% +10556%] (p = 0.00 < 0.05)
                            thrpt:  [-99.062% -99.060% -99.059%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    │  context regions: 1500000
    Benchmarking standalone/hot/context: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 17.1s, enable flat sampling, or reduce sample count to 10.
    standalone/hot/context  time:   [80.804 ms 80.995 ms 81.195 ms]
                            thrpt:  [975.58 MiB/s 977.98 MiB/s 980.30 MiB/s]
                     change:
                            time:   [+18731% +18816% +18911%] (p = 0.00 < 0.05)
                            thrpt:  [-99.474% -99.471% -99.469%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    │  full-vs-context-aware counts:
        find_context_italics full=  500000  context-aware=  500000
    standalone/hot/find_context_italics
                            time:   [15.126 ms 15.145 ms 15.168 ms]
                            thrpt:  [5.0999 GiB/s 5.1075 GiB/s 5.1141 GiB/s]
                     change:
                            time:   [+10258% +10279% +10301%] (p = 0.00 < 0.05)
                            thrpt:  [-99.039% -99.037% -99.035%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
        find_context_bolds full=  500000  context-aware=  500000
    standalone/hot/find_context_bolds
                            time:   [15.740 ms 15.770 ms 15.798 ms]
                            thrpt:  [4.8965 GiB/s 4.9052 GiB/s 4.9145 GiB/s]
                     change:
                            time:   [+10304% +10387% +10438%] (p = 0.00 < 0.05)
                            thrpt:  [-99.051% -99.046% -99.039%]
                            Performance has regressed.
    Found 4 outliers among 20 measurements (20.00%)
      2 (10.00%) low mild
      2 (10.00%) high mild
        find_context_bold_italics full=       0  context-aware=       0
    standalone/hot/find_context_bold_italics
                            time:   [13.527 ms 13.565 ms 13.612 ms]
                            thrpt:  [5.6830 GiB/s 5.7026 GiB/s 5.7185 GiB/s]
                     change:
                            time:   [+10386% +10407% +10429%] (p = 0.00 < 0.05)
                            thrpt:  [-99.050% -99.048% -99.046%]
                            Performance has regressed.
        find_context_headings full=  500000  context-aware=  500000
    standalone/hot/find_context_headings
                            time:   [10.559 ms 10.563 ms 10.568 ms]
                            thrpt:  [7.3198 GiB/s 7.3230 GiB/s 7.3262 GiB/s]
                     change:
                            time:   [+10595% +10639% +10689%] (p = 0.00 < 0.05)
                            thrpt:  [-99.073% -99.069% -99.065%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
        find_context_thematic_breaks full=       0  context-aware=       0
    standalone/hot/find_context_thematic_breaks
                            time:   [7.7914 ms 7.7966 ms 7.8038 ms]
                            thrpt:  [9.9126 GiB/s 9.9217 GiB/s 9.9283 GiB/s]
                     change:
                            time:   [+10744% +10769% +10808%] (p = 0.00 < 0.05)
                            thrpt:  [-99.083% -99.080% -99.078%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_context_blockquotes full=       0  context-aware=       0
    standalone/hot/find_context_blockquotes
                            time:   [4.7961 ms 4.8014 ms 4.8080 ms]
                            thrpt:  [16.089 GiB/s 16.111 GiB/s 16.129 GiB/s]
                     change:
                            time:   [+12939% +13036% +13153%] (p = 0.00 < 0.05)
                            thrpt:  [-99.245% -99.239% -99.233%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_context_bullet_items full=       0  context-aware=       0
    standalone/hot/find_context_bullet_items
                            time:   [7.6341 ms 7.6478 ms 7.6597 ms]
                            thrpt:  [10.099 GiB/s 10.115 GiB/s 10.133 GiB/s]
                     change:
                            time:   [+11113% +11158% +11228%] (p = 0.00 < 0.05)
                            thrpt:  [-99.117% -99.112% -99.108%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_context_ordered_items full=       0  context-aware=       0
    standalone/hot/find_context_ordered_items
                            time:   [9.9923 ms 9.9981 ms 10.006 ms]
                            thrpt:  [7.7313 GiB/s 7.7370 GiB/s 7.7415 GiB/s]
                     change:
                            time:   [+10463% +10483% +10504%] (p = 0.00 < 0.05)
                            thrpt:  [-99.057% -99.055% -99.053%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    standalone/hot/find_context_italics_cold
                            time:   [97.789 ms 98.158 ms 98.489 ms]
                            thrpt:  [804.27 MiB/s 806.99 MiB/s 810.03 MiB/s]
                     change:
                            time:   [+16650% +16718% +16777%] (p = 0.00 < 0.05)
                            thrpt:  [-99.407% -99.405% -99.403%]
                            Performance has regressed.
    standalone/hot/find_context_bolds_cold
                            time:   [97.448 ms 97.959 ms 98.430 ms]
                            thrpt:  [804.76 MiB/s 808.63 MiB/s 812.87 MiB/s]
                     change:
                            time:   [+16434% +16521% +16600%] (p = 0.00 < 0.05)
                            thrpt:  [-99.401% -99.398% -99.395%]
                            Performance has regressed.
    Benchmarking standalone/hot/find_context_bold_italics_cold: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 20.0s, enable flat sampling, or reduce sample count to 10.
    standalone/hot/find_context_bold_italics_cold
                            time:   [95.669 ms 96.465 ms 96.979 ms]
                            thrpt:  [816.80 MiB/s 821.15 MiB/s 827.98 MiB/s]
                     change:
                            time:   [+16685% +16837% +17000%] (p = 0.00 < 0.05)
                            thrpt:  [-99.415% -99.410% -99.404%]
                            Performance has regressed.
    Benchmarking standalone/hot/find_context_headings_cold: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 19.2s, enable flat sampling, or reduce sample count to 10.
    standalone/hot/find_context_headings_cold
                            time:   [90.478 ms 91.414 ms 92.355 ms]
                            thrpt:  [857.69 MiB/s 866.53 MiB/s 875.48 MiB/s]
                     change:
                            time:   [+17105% +17243% +17391%] (p = 0.00 < 0.05)
                            thrpt:  [-99.428% -99.423% -99.419%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    Benchmarking standalone/hot/find_context_thematic_breaks_cold: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 19.0s, enable flat sampling, or reduce sample count to 10.
    standalone/hot/find_context_thematic_breaks_cold
                            time:   [89.686 ms 89.825 ms 89.964 ms]
                            thrpt:  [880.49 MiB/s 881.85 MiB/s 883.21 MiB/s]
                     change:
                            time:   [+17832% +17893% +17978%] (p = 0.00 < 0.05)
                            thrpt:  [-99.447% -99.444% -99.442%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    Benchmarking standalone/hot/find_context_blockquotes_cold: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 18.4s, enable flat sampling, or reduce sample count to 10.
    standalone/hot/find_context_blockquotes_cold
                            time:   [86.663 ms 86.891 ms 87.095 ms]
                            thrpt:  [909.49 MiB/s 911.63 MiB/s 914.03 MiB/s]
                     change:
                            time:   [+18548% +18640% +18755%] (p = 0.00 < 0.05)
                            thrpt:  [-99.470% -99.466% -99.464%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    Benchmarking standalone/hot/find_context_bullet_items_cold: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 19.0s, enable flat sampling, or reduce sample count to 10.
    standalone/hot/find_context_bullet_items_cold
                            time:   [89.945 ms 90.149 ms 90.267 ms]
                            thrpt:  [877.53 MiB/s 878.68 MiB/s 880.67 MiB/s]
                     change:
                            time:   [+17946% +18051% +18161%] (p = 0.00 < 0.05)
                            thrpt:  [-99.452% -99.449% -99.446%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    Benchmarking standalone/hot/find_context_ordered_items_cold: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 19.4s, enable flat sampling, or reduce sample count to 10.
    standalone/hot/find_context_ordered_items_cold
                            time:   [91.571 ms 92.096 ms 92.556 ms]
                            thrpt:  [855.83 MiB/s 860.10 MiB/s 865.03 MiB/s]
                     change:
                            time:   [+17524% +17642% +17808%] (p = 0.00 < 0.05)
                            thrpt:  [-99.442% -99.436% -99.433%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
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
                            time:   [20.345 ms 20.364 ms 20.378 ms]
                            thrpt:  [7.1981 GiB/s 7.2032 GiB/s 7.2098 GiB/s]
                     change:
                            time:   [+10465% +10484% +10512%] (p = 0.00 < 0.05)
                            thrpt:  [-99.058% -99.055% -99.053%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_italics       full= 1200000  standalone= 1200000
    standalone/heavy/find_italics
                            time:   [35.551 ms 35.559 ms 35.565 ms]
                            thrpt:  [4.1244 GiB/s 4.1252 GiB/s 4.1261 GiB/s]
                     change:
                            time:   [+10134% +10144% +10156%] (p = 0.00 < 0.05)
                            thrpt:  [-99.025% -99.024% -99.023%]
                            Performance has regressed.
    Found 4 outliers among 20 measurements (20.00%)
      1 (5.00%) low mild
      1 (5.00%) high mild
      2 (10.00%) high severe
        find_bolds         full= 1200000  standalone= 1200000
    standalone/heavy/find_bolds
                            time:   [36.342 ms 36.352 ms 36.363 ms]
                            thrpt:  [4.0339 GiB/s 4.0351 GiB/s 4.0362 GiB/s]
                     change:
                            time:   [+10048% +10092% +10124%] (p = 0.00 < 0.05)
                            thrpt:  [-99.022% -99.019% -99.015%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_bold_italics  full=  600000  standalone=  600000
    standalone/heavy/find_bold_italics
                            time:   [34.832 ms 34.840 ms 34.848 ms]
                            thrpt:  [4.2092 GiB/s 4.2102 GiB/s 4.2112 GiB/s]
                     change:
                            time:   [+10143% +10150% +10159%] (p = 0.00 < 0.05)
                            thrpt:  [-99.025% -99.024% -99.024%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_autolinks     full=  400000  standalone=  400000
    standalone/heavy/find_autolinks
                            time:   [9.2995 ms 9.3116 ms 9.3206 ms]
                            thrpt:  [15.738 GiB/s 15.753 GiB/s 15.773 GiB/s]
                     change:
                            time:   [+14446% +14569% +14706%] (p = 0.00 < 0.05)
                            thrpt:  [-99.325% -99.318% -99.313%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_links         full=  600000  standalone=  600000
    standalone/heavy/find_links
                            time:   [21.662 ms 21.671 ms 21.679 ms]
                            thrpt:  [6.7661 GiB/s 6.7689 GiB/s 6.7716 GiB/s]
                     change:
                            time:   [+10315% +10336% +10358%] (p = 0.00 < 0.05)
                            thrpt:  [-99.044% -99.042% -99.040%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_headings      full=  200000  standalone=  200000
    standalone/heavy/find_headings
                            time:   [8.2180 ms 8.2296 ms 8.2412 ms]
                            thrpt:  [17.799 GiB/s 17.824 GiB/s 17.849 GiB/s]
                     change:
                            time:   [+20876% +21074% +21299%] (p = 0.00 < 0.05)
                            thrpt:  [-99.533% -99.528% -99.523%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
        find_thematic_breaks full=  200000  standalone=  200000
    standalone/heavy/find_thematic_breaks
                            time:   [27.904 ms 27.922 ms 27.938 ms]
                            thrpt:  [5.2503 GiB/s 5.2533 GiB/s 5.2569 GiB/s]
                     change:
                            time:   [+10152% +10167% +10183%] (p = 0.00 < 0.05)
                            thrpt:  [-99.028% -99.026% -99.025%]
                            Performance has regressed.
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low mild
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_fenced_codes  full=  400000  standalone=  400000
    standalone/heavy/find_fenced_codes
                            time:   [23.182 ms 23.193 ms 23.206 ms]
                            thrpt:  [6.3211 GiB/s 6.3247 GiB/s 6.3276 GiB/s]
                     change:
                            time:   [+10288% +10311% +10335%] (p = 0.00 < 0.05)
                            thrpt:  [-99.042% -99.040% -99.037%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_blockquotes   full=  400000  standalone=  400000
    standalone/heavy/find_blockquotes
                            time:   [12.739 ms 12.752 ms 12.766 ms]
                            thrpt:  [11.490 GiB/s 11.503 GiB/s 11.514 GiB/s]
                     change:
                            time:   [+12409% +12431% +12460%] (p = 0.00 < 0.05)
                            thrpt:  [-99.204% -99.202% -99.201%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_bullet_items  full=  600000  standalone=  600000
    standalone/heavy/find_bullet_items
                            time:   [23.822 ms 23.835 ms 23.847 ms]
                            thrpt:  [6.1512 GiB/s 6.1542 GiB/s 6.1576 GiB/s]
                     change:
                            time:   [+10219% +10232% +10250%] (p = 0.00 < 0.05)
                            thrpt:  [-99.034% -99.032% -99.031%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_ordered_items full=  400000  standalone=  400000
    standalone/heavy/find_ordered_items
                            time:   [21.088 ms 21.109 ms 21.136 ms]
                            thrpt:  [6.9401 GiB/s 6.9488 GiB/s 6.9558 GiB/s]
                     change:
                            time:   [+10306% +10331% +10364%] (p = 0.00 < 0.05)
                            thrpt:  [-99.044% -99.041% -99.039%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    │  context regions: 2400000
    standalone/heavy/context
                            time:   [155.52 ms 155.65 ms 155.78 ms]
                            thrpt:  [964.21 MiB/s 965.01 MiB/s 965.83 MiB/s]
                     change:
                            time:   [+21620% +21642% +21662%] (p = 0.00 < 0.05)
                            thrpt:  [-99.540% -99.540% -99.540%]
                            Performance has regressed.
    │  full-vs-context-aware counts:
        find_context_italics full= 1200000  context-aware= 1200000
    standalone/heavy/find_context_italics
                            time:   [41.162 ms 41.199 ms 41.237 ms]
                            thrpt:  [3.5571 GiB/s 3.5604 GiB/s 3.5636 GiB/s]
                     change:
                            time:   [+10136% +10163% +10193%] (p = 0.00 < 0.05)
                            thrpt:  [-99.028% -99.026% -99.023%]
                            Performance has regressed.
        find_context_bolds full= 1200000  context-aware= 1200000
    Benchmarking standalone/heavy/find_context_bolds: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 10.0s, enable flat sampling, or reduce sample count to 10.
    standalone/heavy/find_context_bolds
                            time:   [47.648 ms 47.673 ms 47.715 ms]
                            thrpt:  [3.0742 GiB/s 3.0769 GiB/s 3.0785 GiB/s]
                     change:
                            time:   [+10049% +10067% +10089%] (p = 0.00 < 0.05)
                            thrpt:  [-99.019% -99.016% -99.015%]
                            Performance has regressed.
    Found 4 outliers among 20 measurements (20.00%)
      4 (20.00%) high severe
        find_context_bold_italics full=  600000  context-aware=  600000
    standalone/heavy/find_context_bold_italics
                            time:   [39.606 ms 39.618 ms 39.630 ms]
                            thrpt:  [3.7014 GiB/s 3.7025 GiB/s 3.7036 GiB/s]
                     change:
                            time:   [+10036% +10045% +10053%] (p = 0.00 < 0.05)
                            thrpt:  [-99.015% -99.014% -99.013%]
                            Performance has regressed.
    Found 4 outliers among 20 measurements (20.00%)
      4 (20.00%) low mild
        find_context_headings full=  200000  context-aware=  200000
    standalone/heavy/find_context_headings
                            time:   [9.5604 ms 9.5651 ms 9.5706 ms]
                            thrpt:  [15.327 GiB/s 15.335 GiB/s 15.343 GiB/s]
                     change:
                            time:   [+15546% +15663% +15811%] (p = 0.00 < 0.05)
                            thrpt:  [-99.372% -99.366% -99.361%]
                            Performance has regressed.
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) high mild
      1 (5.00%) high severe
        find_context_thematic_breaks full=  200000  context-aware=  200000
    standalone/heavy/find_context_thematic_breaks
                            time:   [32.341 ms 32.362 ms 32.387 ms]
                            thrpt:  [4.5292 GiB/s 4.5326 GiB/s 4.5356 GiB/s]
                     change:
                            time:   [+10245% +10259% +10277%] (p = 0.00 < 0.05)
                            thrpt:  [-99.036% -99.035% -99.033%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_context_blockquotes full=  400000  context-aware=  400000
    standalone/heavy/find_context_blockquotes
                            time:   [13.440 ms 13.454 ms 13.472 ms]
                            thrpt:  [10.888 GiB/s 10.903 GiB/s 10.914 GiB/s]
                     change:
                            time:   [+11285% +11320% +11364%] (p = 0.00 < 0.05)
                            thrpt:  [-99.128% -99.124% -99.122%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
        find_context_bullet_items full=  600000  context-aware=  600000
    standalone/heavy/find_context_bullet_items
                            time:   [28.481 ms 28.501 ms 28.520 ms]
                            thrpt:  [5.1432 GiB/s 5.1466 GiB/s 5.1503 GiB/s]
                     change:
                            time:   [+10090% +10106% +10123%] (p = 0.00 < 0.05)
                            thrpt:  [-99.022% -99.020% -99.019%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_context_ordered_items full=  400000  context-aware=  400000
    standalone/heavy/find_context_ordered_items
                            time:   [25.925 ms 25.951 ms 25.974 ms]
                            thrpt:  [5.6474 GiB/s 5.6525 GiB/s 5.6582 GiB/s]
                     change:
                            time:   [+10302% +10324% +10346%] (p = 0.00 < 0.05)
                            thrpt:  [-99.043% -99.041% -99.039%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    standalone/heavy/find_context_italics_cold
                            time:   [198.53 ms 198.84 ms 199.13 ms]
                            thrpt:  [754.31 MiB/s 755.42 MiB/s 756.57 MiB/s]
                     change:
                            time:   [+17249% +17282% +17317%] (p = 0.00 < 0.05)
                            thrpt:  [-99.426% -99.425% -99.424%]
                            Performance has regressed.
    standalone/heavy/find_context_bolds_cold
                            time:   [205.46 ms 205.75 ms 206.06 ms]
                            thrpt:  [728.95 MiB/s 730.03 MiB/s 731.06 MiB/s]
                     change:
                            time:   [+16875% +16902% +16930%] (p = 0.00 < 0.05)
                            thrpt:  [-99.413% -99.412% -99.411%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low mild
      1 (5.00%) high mild
    standalone/heavy/find_context_bold_italics_cold
                            time:   [196.30 ms 196.85 ms 197.32 ms]
                            thrpt:  [761.22 MiB/s 763.05 MiB/s 765.18 MiB/s]
                     change:
                            time:   [+17271% +17326% +17373%] (p = 0.00 < 0.05)
                            thrpt:  [-99.428% -99.426% -99.424%]
                            Performance has regressed.
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low severe
      2 (10.00%) low mild
    standalone/heavy/find_context_headings_cold
                            time:   [166.07 ms 166.31 ms 166.54 ms]
                            thrpt:  [901.90 MiB/s 903.18 MiB/s 904.47 MiB/s]
                     change:
                            time:   [+21250% +21283% +21316%] (p = 0.00 < 0.05)
                            thrpt:  [-99.533% -99.532% -99.532%]
                            Performance has regressed.
    standalone/heavy/find_context_thematic_breaks_cold
                            time:   [187.13 ms 187.40 ms 187.69 ms]
                            thrpt:  [800.30 MiB/s 801.51 MiB/s 802.69 MiB/s]
                     change:
                            time:   [+18128% +18154% +18184%] (p = 0.00 < 0.05)
                            thrpt:  [-99.453% -99.452% -99.451%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    standalone/heavy/find_context_blockquotes_cold
                            time:   [169.92 ms 170.06 ms 170.23 ms]
                            thrpt:  [882.39 MiB/s 883.23 MiB/s 883.97 MiB/s]
                     change:
                            time:   [+20353% +20374% +20396%] (p = 0.00 < 0.05)
                            thrpt:  [-99.512% -99.512% -99.511%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    standalone/heavy/find_context_bullet_items_cold
                            time:   [183.86 ms 184.17 ms 184.46 ms]
                            thrpt:  [814.28 MiB/s 815.58 MiB/s 816.94 MiB/s]
                     change:
                            time:   [+18366% +18401% +18434%] (p = 0.00 < 0.05)
                            thrpt:  [-99.460% -99.459% -99.458%]
                            Performance has regressed.
    Found 4 outliers among 20 measurements (20.00%)
      2 (10.00%) low mild
      2 (10.00%) high mild
    standalone/heavy/find_context_ordered_items_cold
                            time:   [181.86 ms 182.04 ms 182.21 ms]
                            thrpt:  [824.34 MiB/s 825.11 MiB/s 825.96 MiB/s]
                     change:
                            time:   [+18736% +18756% +18774%] (p = 0.00 < 0.05)
                            thrpt:  [-99.470% -99.470% -99.469%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
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
