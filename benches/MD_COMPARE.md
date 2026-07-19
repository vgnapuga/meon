# meon-md — Cross-parser comparison

EN | [**RU**](https://github.com/vgnapuga/meon/blob/main/benches/MD_COMPARE_RU.md)

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

| Corpus  | `meon-md` | `pulldown-cmark` | `comrak` |
|---------|-----------|------------------|----------|
| `plain` | time: [1.0709 ms 1.0725 ms 1.0744 ms] thrpt: [2.5466 GiB/s 2.5512 GiB/s 2.5549 GiB/s] | time: [3.2574 ms 3.2583 ms 3.2594 ms] thrpt: [859.57 MiB/s 859.85 MiB/s 860.12 MiB/s] | time: [14.646 ms 14.685 ms 14.728 ms] thrpt: [190.23 MiB/s 190.78 MiB/s 191.30 MiB/s] |
| `hot`   | time: [680.44 µs 681.17 µs 681.94 µs] thrpt: [1.0797 GiB/s 1.0809 GiB/s 1.0821 GiB/s] | time: [4.8188 ms 4.8231 ms 4.8274 ms] thrpt: [156.19 MiB/s 156.33 MiB/s 156.47 MiB/s] | time: [18.018 ms 18.092 ms 18.171 ms] thrpt: [41.494 MiB/s 41.675 MiB/s 41.846 MiB/s] |
| `heavy` | time: [1.5665 ms 1.5673 ms 1.5682 ms] thrpt: [937.15 MiB/s 937.71 MiB/s 938.17 MiB/s] | time: [13.503 ms 13.538 ms 13.576 ms] thrpt: [108.25 MiB/s 108.55 MiB/s 108.84 MiB/s] | time: [44.485 ms 44.628 ms 44.777 ms] thrpt: [32.821 MiB/s 32.931 MiB/s 33.037 MiB/s] |

**big (exceeds L3 cache):**

| Corpus  | `meon-md` | `pulldown-cmark` | `comrak` |
|---------|-----------|------------------|----------|
| `plain` | time: [101.30 ms 101.41 ms 101.55 ms] thrpt: [2.6943 GiB/s 2.6979 GiB/s 2.7009 GiB/s] | time: [490.65 ms 492.97 ms 495.95 ms] thrpt: [564.92 MiB/s 568.33 MiB/s 571.02 MiB/s] | time: [2.7079 s 2.7419 s 2.7762 s] thrpt: [100.92 MiB/s 102.18 MiB/s 103.46 MiB/s] |
| `hot`   | time: [67.333 ms 68.266 ms 68.775 ms] thrpt: [1.0706 GiB/s 1.0786 GiB/s 1.0935 GiB/s] | time: [849.37 ms 855.51 ms 861.70 ms] thrpt: [87.499 MiB/s 88.132 MiB/s 88.769 MiB/s] | time: [3.6113 s 3.6626 s 3.7174 s] thrpt: [20.282 MiB/s 20.586 MiB/s 20.878 MiB/s] |
| `heavy` | time: [147.91 ms 149.60 ms 151.65 ms] thrpt: [969.10 MiB/s 982.41 MiB/s 993.59 MiB/s] | time: [2.0664 s 2.0760 s 2.0852 s] thrpt: [70.479 MiB/s 70.793 MiB/s 71.120 MiB/s] | time: [7.8153 s 7.8648 s 7.9273 s] thrpt: [18.539 MiB/s 18.686 MiB/s 18.805 MiB/s] |

### nightly - `RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_compare --features avx2`

> meon on AVX2; `pulldown-cmark` and `comrak` scalar (no `simd` feature). The
> meon column is AVX2 against scalar comparators — not a like-for-like SIMD row.

**small (fits in cache):**

| Corpus  | `meon-md` | `pulldown-cmark` | `comrak` |
|---------|-----------|------------------|----------|
| `plain` | time: [629.95 µs 630.73 µs 631.59 µs] thrpt: [4.3320 GiB/s 4.3379 GiB/s 4.3432 GiB/s] | time: [3.8414 ms 3.8427 ms 3.8441 ms] thrpt: [728.84 MiB/s 729.09 MiB/s 729.35 MiB/s] | time: [16.129 ms 16.230 ms 16.329 ms] thrpt: [171.58 MiB/s 172.62 MiB/s 173.71 MiB/s] |
| `hot`   | time: [613.33 µs 614.12 µs 614.94 µs] thrpt: [1.1974 GiB/s 1.1990 GiB/s 1.2005 GiB/s] | time: [4.9585 ms 4.9634 ms 4.9685 ms] thrpt: [151.75 MiB/s 151.91 MiB/s 152.06 MiB/s] | time: [19.804 ms 19.967 ms 20.132 ms] thrpt: [37.452 MiB/s 37.761 MiB/s 38.073 MiB/s] |
| `heavy` | time: [1.3876 ms 1.3894 ms 1.3911 ms] thrpt: [1.0317 GiB/s 1.0330 GiB/s 1.0343 GiB/s] | time: [13.925 ms 13.977 ms 14.029 ms] thrpt: [104.76 MiB/s 105.15 MiB/s 105.54 MiB/s] | time: [49.054 ms 49.363 ms 49.672 ms] thrpt: [29.587 MiB/s 29.772 MiB/s 29.960 MiB/s] |

**big (exceeds L3 cache):**

| Corpus  | `meon-md` | `pulldown-cmark` | `comrak` |
|---------|-----------|------------------|----------|
| `plain` | time: [66.438 ms 66.569 ms 66.753 ms] thrpt: [4.0988 GiB/s 4.1101 GiB/s 4.1182 GiB/s] | time: [588.74 ms 591.30 ms 594.49 ms] thrpt: [471.28 MiB/s 473.82 MiB/s 475.88 MiB/s] | time: [2.8263 s 2.8505 s 2.8776 s] thrpt: [97.363 MiB/s 98.287 MiB/s 99.131 MiB/s] |
| `hot`   | time: [62.814 ms 62.935 ms 63.150 ms] thrpt: [1.1660 GiB/s 1.1699 GiB/s 1.1722 GiB/s] | time: [895.95 ms 901.29 ms 907.51 ms] thrpt: [83.082 MiB/s 83.655 MiB/s 84.154 MiB/s] | time: [3.5513 s 3.6021 s 3.6586 s] thrpt: [20.608 MiB/s 20.931 MiB/s 21.231 MiB/s] |
| `heavy` | time: [135.34 ms 135.72 ms 136.11 ms] thrpt: [1.0544 GiB/s 1.0575 GiB/s 1.0604 GiB/s] | time: [2.1312 s 2.1528 s 2.1730 s] thrpt: [67.631 MiB/s 68.268 MiB/s 68.957 MiB/s] | time: [8.0073 s 8.0551 s 8.1210 s] thrpt: [18.097 MiB/s 18.245 MiB/s 18.354 MiB/s] |

---

## Scaling from small to big

The clearest expression of the architecture difference is how each parser holds
up as the input grows past cache (stable build, median `thrpt`):

| Parser           | `plain`              | `hot`                | `heavy`              |
|------------------|----------------------|----------------------|----------------------|
| `meon-md`        | 2.55 -> 2.70 GiB/s   | 1.081 -> 1.079 GiB/s | 938 -> 982 MiB/s     |
| `pulldown-cmark` | 860 -> 568 MiB/s     | 156 -> 88 MiB/s      | 109 -> 71 MiB/s      |
| `comrak`         | 191 -> 102 MiB/s     | 41.7 -> 20.6 MiB/s   | 32.9 -> 18.7 MiB/s   |

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

**small:**

<details>
<summary>plain</summary>

```
  find_codes         full=       0  standalone=       0
                        time:   [29.389 µs 29.936 µs 30.729 µs]
                        thrpt:  [89.036 GiB/s 91.398 GiB/s 93.098 GiB/s]

  find_italics       full=       0  standalone=       0
                        time:   [30.329 µs 31.297 µs 32.139 µs]
                        thrpt:  [85.132 GiB/s 87.421 GiB/s 90.211 GiB/s]

  find_bolds         full=       0  standalone=       0
                        time:   [29.730 µs 30.453 µs 31.120 µs]
                        thrpt:  [87.919 GiB/s 89.844 GiB/s 92.029 GiB/s]

  find_bold_italics  full=       0  standalone=       0
                        time:   [30.949 µs 31.724 µs 32.508 µs]
                        thrpt:  [84.165 GiB/s 86.245 GiB/s 88.404 GiB/s]

  find_autolinks     full=       0  standalone=       0
                        time:   [30.295 µs 30.990 µs 31.457 µs]
                        thrpt:  [86.976 GiB/s 88.288 GiB/s 90.313 GiB/s]

  find_links         full=       0  standalone=       0
                        time:   [32.045 µs 33.055 µs 33.889 µs]
                        thrpt:  [80.736 GiB/s 82.772 GiB/s 85.381 GiB/s]

  find_headings      full=       0  standalone=       0
                        time:   [30.158 µs 31.011 µs 32.131 µs]
                        thrpt:  [85.152 GiB/s 88.228 GiB/s 90.724 GiB/s]

  find_thematic_breaks full=       0  standalone=       0
                        time:   [40.961 µs 42.137 µs 43.416 µs]
                        thrpt:  [63.019 GiB/s 64.931 GiB/s 66.797 GiB/s]

  find_fenced_codes  full=       0  standalone=       0
                        time:   [28.677 µs 29.334 µs 30.073 µs]
                        thrpt:  [90.981 GiB/s 93.270 GiB/s 95.410 GiB/s]

  find_blockquotes   full=       0  standalone=       0
                        time:   [31.250 µs 32.135 µs 32.846 µs]
                        thrpt:  [83.298 GiB/s 85.143 GiB/s 87.553 GiB/s]

  find_bullet_items  full=       0  standalone=       0
                        time:   [43.963 µs 44.599 µs 45.321 µs]
                        thrpt:  [60.370 GiB/s 61.347 GiB/s 62.235 GiB/s]

  find_ordered_items full=       0  standalone=       0
                        time:   [283.68 µs 283.99 µs 284.30 µs]
                        thrpt:  [9.6238 GiB/s 9.6344 GiB/s 9.6449 GiB/s]
```

</details>

<details>
<summary>hot</summary>

```
  find_codes         full=    5000  standalone=    5000
                        time:   [81.125 µs 81.465 µs 81.794 µs]
                        thrpt:  [9.0019 GiB/s 9.0383 GiB/s 9.0761 GiB/s]

  find_italics       full=    5000  standalone=    5000
                        time:   [130.55 µs 130.86 µs 131.19 µs]
                        thrpt:  [5.6124 GiB/s 5.6267 GiB/s 5.6400 GiB/s]

  find_bolds         full=    5000  standalone=    5000
                        time:   [128.17 µs 128.76 µs 129.28 µs]
                        thrpt:  [5.6955 GiB/s 5.7186 GiB/s 5.7446 GiB/s]

  find_bold_italics  full=       0  standalone=       0
                        time:   [118.25 µs 118.49 µs 118.72 µs]
                        thrpt:  [6.2018 GiB/s 6.2141 GiB/s 6.2266 GiB/s]

  find_autolinks     full=    5000  standalone=    5000
                        time:   [75.876 µs 75.959 µs 76.058 µs]
                        thrpt:  [9.6808 GiB/s 9.6934 GiB/s 9.7040 GiB/s]

  find_links         full=    5000  standalone=    5000
                        time:   [116.28 µs 116.47 µs 116.62 µs]
                        thrpt:  [6.3139 GiB/s 6.3220 GiB/s 6.3321 GiB/s]

  find_headings      full=    5000  standalone=    5000
                        time:   [78.126 µs 78.245 µs 78.385 µs]
                        thrpt:  [9.3934 GiB/s 9.4102 GiB/s 9.4246 GiB/s]

  find_thematic_breaks full=       0  standalone=       0
                        time:   [73.497 µs 73.593 µs 73.695 µs]
                        thrpt:  [9.9912 GiB/s 10.005 GiB/s 10.018 GiB/s]

  find_fenced_codes  full=       0  standalone=       0
                        time:   [65.842 µs 65.894 µs 65.959 µs]
                        thrpt:  [11.163 GiB/s 11.174 GiB/s 11.183 GiB/s]

  find_blockquotes   full=       0  standalone=       0
                        time:   [39.697 µs 39.924 µs 40.065 µs]
                        thrpt:  [18.378 GiB/s 18.443 GiB/s 18.548 GiB/s]

  find_bullet_items  full=       0  standalone=       0
                        time:   [73.426 µs 73.571 µs 73.773 µs]
                        thrpt:  [9.9806 GiB/s 10.008 GiB/s 10.028 GiB/s]

  find_ordered_items full=       0  standalone=       0
                        time:   [100.46 µs 100.73 µs 101.05 µs]
                        thrpt:  [7.2866 GiB/s 7.3099 GiB/s 7.3290 GiB/s]
```

</details>

<details>
<summary>heavy</summary>

```
  find_codes         full=   10000  standalone=   10000
                        time:   [197.92 µs 198.87 µs 199.77 µs]
                        thrpt:  [7.1842 GiB/s 7.2167 GiB/s 7.2515 GiB/s]

  find_italics       full=   12000  standalone=   12000
                        time:   [403.52 µs 404.11 µs 404.53 µs]
                        thrpt:  [3.5478 GiB/s 3.5515 GiB/s 3.5566 GiB/s]

  find_bolds         full=   12000  standalone=   12000
                        time:   [405.40 µs 406.23 µs 407.41 µs]
                        thrpt:  [3.5227 GiB/s 3.5330 GiB/s 3.5401 GiB/s]

  find_bold_italics  full=    6000  standalone=    6000
                        time:   [393.71 µs 394.46 µs 395.45 µs]
                        thrpt:  [3.6292 GiB/s 3.6384 GiB/s 3.6453 GiB/s]

  find_autolinks     full=    4000  standalone=    4000
                        time:   [70.209 µs 70.262 µs 70.328 µs]
                        thrpt:  [20.407 GiB/s 20.426 GiB/s 20.442 GiB/s]

  find_links         full=    6000  standalone=    6000
                        time:   [144.94 µs 145.40 µs 145.90 µs]
                        thrpt:  [9.8367 GiB/s 9.8707 GiB/s 9.9021 GiB/s]

  find_headings      full=    2000  standalone=    2000
                        time:   [43.549 µs 44.030 µs 44.394 µs]
                        thrpt:  [32.329 GiB/s 32.596 GiB/s 32.956 GiB/s]

  find_thematic_breaks full=    2000  standalone=    2000
                        time:   [290.32 µs 290.87 µs 291.63 µs]
                        thrpt:  [4.9212 GiB/s 4.9341 GiB/s 4.9435 GiB/s]

  find_fenced_codes  full=    2000  standalone=    2000
                        time:   [191.45 µs 191.85 µs 192.26 µs]
                        thrpt:  [7.4649 GiB/s 7.4807 GiB/s 7.4963 GiB/s]

  find_blockquotes   full=    4000  standalone=    4000
                        time:   [121.21 µs 121.38 µs 121.59 µs]
                        thrpt:  [11.804 GiB/s 11.824 GiB/s 11.841 GiB/s]

  find_bullet_items  full=    6000  standalone=    6000
                        time:   [273.58 µs 274.41 µs 275.33 µs]
                        thrpt:  [5.2126 GiB/s 5.2302 GiB/s 5.2459 GiB/s]

  find_ordered_items full=    4000  standalone=    4000
                        time:   [231.70 µs 233.23 µs 234.43 µs]
                        thrpt:  [6.1221 GiB/s 6.1535 GiB/s 6.1941 GiB/s]
```

</details>

**big:**

<details>
<summary>plain</summary>

```
  find_codes         full=       0  standalone=       0
                        time:   [11.568 ms 11.691 ms 11.856 ms]
                        thrpt:  [23.078 GiB/s 23.403 GiB/s 23.651 GiB/s]

  find_italics       full=       0  standalone=       0
                        time:   [11.390 ms 11.419 ms 11.458 ms]
                        thrpt:  [23.878 GiB/s 23.960 GiB/s 24.022 GiB/s]

  find_bolds         full=       0  standalone=       0
                        time:   [11.349 ms 11.370 ms 11.386 ms]
                        thrpt:  [24.030 GiB/s 24.065 GiB/s 24.109 GiB/s]

  find_bold_italics  full=       0  standalone=       0
                        time:   [11.507 ms 11.528 ms 11.553 ms]
                        thrpt:  [23.682 GiB/s 23.735 GiB/s 23.778 GiB/s]

  find_autolinks     full=       0  standalone=       0
                        time:   [11.357 ms 11.369 ms 11.386 ms]
                        thrpt:  [24.030 GiB/s 24.067 GiB/s 24.092 GiB/s]

  find_links         full=       0  standalone=       0
                        time:   [11.498 ms 11.520 ms 11.547 ms]
                        thrpt:  [23.694 GiB/s 23.751 GiB/s 23.797 GiB/s]

  find_headings      full=       0  standalone=       0
                        time:   [11.354 ms 11.386 ms 11.426 ms]
                        thrpt:  [23.946 GiB/s 24.031 GiB/s 24.097 GiB/s]

  find_thematic_breaks full=       0  standalone=       0
                        time:   [12.969 ms 12.992 ms 13.014 ms]
                        thrpt:  [21.024 GiB/s 21.059 GiB/s 21.097 GiB/s]

  find_fenced_codes  full=       0  standalone=       0
                        time:   [11.920 ms 12.075 ms 12.192 ms]
                        thrpt:  [22.442 GiB/s 22.659 GiB/s 22.954 GiB/s]

  find_blockquotes   full=       0  standalone=       0
                        time:   [11.840 ms 12.017 ms 12.229 ms]
                        thrpt:  [22.373 GiB/s 22.767 GiB/s 23.108 GiB/s]

  find_bullet_items  full=       0  standalone=       0
                        time:   [12.962 ms 12.989 ms 13.030 ms]
                        thrpt:  [20.998 GiB/s 21.064 GiB/s 21.108 GiB/s]

  find_ordered_items full=       0  standalone=       0
                        time:   [29.960 ms 29.987 ms 30.015 ms]
                        thrpt:  [9.1156 GiB/s 9.1241 GiB/s 9.1322 GiB/s]
```

</details>

<details>
<summary>hot</summary>

```
  find_codes         full=  500000  standalone=  500000
                        time:   [8.6592 ms 8.6730 ms 8.6873 ms]
                        thrpt:  [8.4756 GiB/s 8.4896 GiB/s 8.5032 GiB/s]

  find_italics       full=  500000  standalone=  500000
                        time:   [12.987 ms 13.053 ms 13.098 ms]
                        thrpt:  [5.6214 GiB/s 5.6408 GiB/s 5.6694 GiB/s]

  find_bolds         full=  500000  standalone=  500000
                        time:   [12.950 ms 12.993 ms 13.040 ms]
                        thrpt:  [5.6464 GiB/s 5.6669 GiB/s 5.6855 GiB/s]

  find_bold_italics  full=       0  standalone=       0
                        time:   [12.090 ms 12.116 ms 12.142 ms]
                        thrpt:  [6.0639 GiB/s 6.0773 GiB/s 6.0901 GiB/s]

  find_autolinks     full=  500000  standalone=  500000
                        time:   [8.1075 ms 8.1336 ms 8.1619 ms]
                        thrpt:  [9.0212 GiB/s 9.0526 GiB/s 9.0818 GiB/s]

  find_links         full=  500000  standalone=  500000
                        time:   [11.964 ms 11.997 ms 12.039 ms]
                        thrpt:  [6.1157 GiB/s 6.1372 GiB/s 6.1543 GiB/s]

  find_headings      full=  500000  standalone=  500000
                        time:   [8.1489 ms 8.1570 ms 8.1699 ms]
                        thrpt:  [9.0124 GiB/s 9.0266 GiB/s 9.0356 GiB/s]

  find_thematic_breaks full=       0  standalone=       0
                        time:   [7.7858 ms 7.7955 ms 7.8027 ms]
                        thrpt:  [9.4366 GiB/s 9.4452 GiB/s 9.4570 GiB/s]

  find_fenced_codes  full=       0  standalone=       0
                        time:   [7.2120 ms 7.2166 ms 7.2209 ms]
                        thrpt:  [10.197 GiB/s 10.203 GiB/s 10.209 GiB/s]

  find_blockquotes   full=       0  standalone=       0
                        time:   [4.8409 ms 4.8552 ms 4.8730 ms]
                        thrpt:  [15.110 GiB/s 15.165 GiB/s 15.210 GiB/s]

  find_bullet_items  full=       0  standalone=       0
                        time:   [7.7171 ms 7.7357 ms 7.7563 ms]
                        thrpt:  [9.4930 GiB/s 9.5182 GiB/s 9.5412 GiB/s]

  find_ordered_items full=       0  standalone=       0
                        time:   [10.380 ms 10.393 ms 10.409 ms]
                        thrpt:  [7.0738 GiB/s 7.0845 GiB/s 7.0932 GiB/s]
```

</details>

<details>
<summary>heavy</summary>

```
  find_codes         full= 1000000  standalone= 1000000
                        time:   [20.715 ms 20.756 ms 20.810 ms]
                        thrpt:  [6.8966 GiB/s 6.9147 GiB/s 6.9281 GiB/s]

  find_italics       full= 1200000  standalone= 1200000
                        time:   [40.575 ms 40.614 ms 40.646 ms]
                        thrpt:  [3.5309 GiB/s 3.5337 GiB/s 3.5372 GiB/s]

  find_bolds         full= 1200000  standalone= 1200000
                        time:   [40.753 ms 40.870 ms 41.013 ms]
                        thrpt:  [3.4993 GiB/s 3.5116 GiB/s 3.5217 GiB/s]

  find_bold_italics  full=  600000  standalone=  600000
                        time:   [39.418 ms 39.501 ms 39.569 ms]
                        thrpt:  [3.6271 GiB/s 3.6333 GiB/s 3.6409 GiB/s]

  find_autolinks     full=  400000  standalone=  400000
                        time:   [9.3369 ms 9.3618 ms 9.3945 ms]
                        thrpt:  [15.277 GiB/s 15.330 GiB/s 15.371 GiB/s]

  find_links         full=  600000  standalone=  600000
                        time:   [15.441 ms 15.452 ms 15.462 ms]
                        thrpt:  [9.2819 GiB/s 9.2878 GiB/s 9.2944 GiB/s]

  find_headings      full=  200000  standalone=  200000
                        time:   [8.1436 ms 8.1681 ms 8.1998 ms]
                        thrpt:  [17.503 GiB/s 17.571 GiB/s 17.624 GiB/s]

  find_thematic_breaks full=  200000  standalone=  200000
                        time:   [29.281 ms 29.325 ms 29.356 ms]
                        thrpt:  [4.8888 GiB/s 4.8941 GiB/s 4.9014 GiB/s]

  find_fenced_codes  full=  200000  standalone=  200000
                        time:   [19.911 ms 19.926 ms 19.943 ms]
                        thrpt:  [7.1964 GiB/s 7.2026 GiB/s 7.2081 GiB/s]

  find_blockquotes   full=  400000  standalone=  400000
                        time:   [14.054 ms 14.061 ms 14.072 ms]
                        thrpt:  [10.199 GiB/s 10.207 GiB/s 10.212 GiB/s]

  find_bullet_items  full=  600000  standalone=  600000
                        time:   [27.895 ms 27.910 ms 27.933 ms]
                        thrpt:  [5.1380 GiB/s 5.1422 GiB/s 5.1450 GiB/s]

  find_ordered_items full=  400000  standalone=  400000
                        time:   [23.272 ms 23.309 ms 23.340 ms]
                        thrpt:  [6.1490 GiB/s 6.1572 GiB/s 6.1670 GiB/s]
```

</details>

### nightly - `RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_standalone --features avx2`

**small:**

<details>
<summary>plain</summary>

```
  find_codes         full=       0  standalone=       0
                        time:   [30.076 µs 30.907 µs 31.891 µs]
                        thrpt:  [85.794 GiB/s 88.526 GiB/s 90.970 GiB/s]

  find_italics       full=       0  standalone=       0
                        time:   [30.086 µs 31.193 µs 32.165 µs]
                        thrpt:  [85.062 GiB/s 87.712 GiB/s 90.942 GiB/s]

  find_bolds         full=       0  standalone=       0
                        time:   [28.336 µs 29.180 µs 29.932 µs]
                        thrpt:  [91.407 GiB/s 93.766 GiB/s 96.557 GiB/s]

  find_bold_italics  full=       0  standalone=       0
                        time:   [27.364 µs 27.569 µs 27.796 µs]
                        thrpt:  [98.434 GiB/s 99.245 GiB/s 99.986 GiB/s]

  find_autolinks     full=       0  standalone=       0
                        time:   [32.066 µs 32.508 µs 32.902 µs]
                        thrpt:  [83.156 GiB/s 84.166 GiB/s 85.325 GiB/s]

  find_links         full=       0  standalone=       0
                        time:   [30.337 µs 31.366 µs 32.288 µs]
                        thrpt:  [84.738 GiB/s 87.230 GiB/s 90.188 GiB/s]

  find_headings      full=       0  standalone=       0
                        time:   [29.653 µs 30.269 µs 30.810 µs]
                        thrpt:  [88.804 GiB/s 90.390 GiB/s 92.269 GiB/s]

  find_thematic_breaks full=       0  standalone=       0
                        time:   [39.265 µs 39.817 µs 40.357 µs]
                        thrpt:  [67.796 GiB/s 68.715 GiB/s 69.681 GiB/s]

  find_fenced_codes  full=       0  standalone=       0
                        time:   [29.133 µs 29.935 µs 30.577 µs]
                        thrpt:  [89.481 GiB/s 91.400 GiB/s 93.915 GiB/s]

  find_blockquotes   full=       0  standalone=       0
                        time:   [30.480 µs 31.275 µs 32.023 µs]
                        thrpt:  [85.440 GiB/s 87.483 GiB/s 89.764 GiB/s]

  find_bullet_items  full=       0  standalone=       0
                        time:   [40.381 µs 40.851 µs 41.480 µs]
                        thrpt:  [65.961 GiB/s 66.977 GiB/s 67.755 GiB/s]

  find_ordered_items full=       0  standalone=       0
                        time:   [246.46 µs 247.75 µs 249.66 µs]
                        thrpt:  [10.959 GiB/s 11.044 GiB/s 11.101 GiB/s]
```

</details>

<details>
<summary>hot</summary>

```
  find_codes         full=    5000  standalone=    5000
                        time:   [65.737 µs 65.790 µs 65.866 µs]
                        thrpt:  [11.179 GiB/s 11.192 GiB/s 11.201 GiB/s]

  find_italics       full=    5000  standalone=    5000
                        time:   [110.45 µs 110.48 µs 110.52 µs]
                        thrpt:  [6.6620 GiB/s 6.6645 GiB/s 6.6662 GiB/s]

  find_bolds         full=    5000  standalone=    5000
                        time:   [112.64 µs 112.69 µs 112.74 µs]
                        thrpt:  [6.5309 GiB/s 6.5337 GiB/s 6.5365 GiB/s]

  find_bold_italics  full=       0  standalone=       0
                        time:   [106.95 µs 106.99 µs 107.04 µs]
                        thrpt:  [6.8791 GiB/s 6.8819 GiB/s 6.8846 GiB/s]

  find_autolinks     full=    5000  standalone=    5000
                        time:   [62.476 µs 62.848 µs 63.318 µs]
                        thrpt:  [11.629 GiB/s 11.716 GiB/s 11.785 GiB/s]

  find_links         full=    5000  standalone=    5000
                        time:   [93.579 µs 93.641 µs 93.708 µs]
                        thrpt:  [7.8574 GiB/s 7.8631 GiB/s 7.8683 GiB/s]

  find_headings      full=    5000  standalone=    5000
                        time:   [65.447 µs 65.489 µs 65.520 µs]
                        thrpt:  [11.238 GiB/s 11.243 GiB/s 11.250 GiB/s]

  find_thematic_breaks full=       0  standalone=       0
                        time:   [68.766 µs 68.815 µs 68.854 µs]
                        thrpt:  [10.694 GiB/s 10.700 GiB/s 10.707 GiB/s]

  find_fenced_codes  full=       0  standalone=       0
                        time:   [61.536 µs 61.573 µs 61.622 µs]
                        thrpt:  [11.949 GiB/s 11.958 GiB/s 11.965 GiB/s]

  find_blockquotes   full=       0  standalone=       0
                        time:   [34.659 µs 34.732 µs 34.806 µs]
                        thrpt:  [21.154 GiB/s 21.199 GiB/s 21.244 GiB/s]

  find_bullet_items  full=       0  standalone=       0
                        time:   [65.437 µs 65.469 µs 65.495 µs]
                        thrpt:  [11.242 GiB/s 11.247 GiB/s 11.252 GiB/s]

  find_ordered_items full=       0  standalone=       0
                        time:   [89.715 µs 89.842 µs 90.011 µs]
                        thrpt:  [8.1802 GiB/s 8.1955 GiB/s 8.2072 GiB/s]
```

</details>

<details>
<summary>heavy</summary>

```
  find_codes         full=   10000  standalone=   10000
                        time:   [160.34 µs 160.56 µs 160.70 µs]
                        thrpt:  [8.9306 GiB/s 8.9384 GiB/s 8.9508 GiB/s]

  find_italics       full=   12000  standalone=   12000
                        time:   [345.18 µs 345.37 µs 345.57 µs]
                        thrpt:  [4.1531 GiB/s 4.1555 GiB/s 4.1577 GiB/s]

  find_bolds         full=   12000  standalone=   12000
                        time:   [349.76 µs 349.83 µs 349.91 µs]
                        thrpt:  [4.1015 GiB/s 4.1025 GiB/s 4.1033 GiB/s]

  find_bold_italics  full=    6000  standalone=    6000
                        time:   [342.89 µs 343.05 µs 343.17 µs]
                        thrpt:  [4.1821 GiB/s 4.1836 GiB/s 4.1855 GiB/s]

  find_autolinks     full=    4000  standalone=    4000
                        time:   [58.596 µs 59.101 µs 59.479 µs]
                        thrpt:  [24.129 GiB/s 24.284 GiB/s 24.493 GiB/s]

  find_links         full=    6000  standalone=    6000
                        time:   [116.53 µs 116.58 µs 116.63 µs]
                        thrpt:  [12.306 GiB/s 12.311 GiB/s 12.316 GiB/s]

  find_headings      full=    2000  standalone=    2000
                        time:   [37.493 µs 37.522 µs 37.540 µs]
                        thrpt:  [38.231 GiB/s 38.249 GiB/s 38.279 GiB/s]

  find_thematic_breaks full=    2000  standalone=    2000
                        time:   [262.29 µs 262.36 µs 262.41 µs]
                        thrpt:  [5.4692 GiB/s 5.4703 GiB/s 5.4718 GiB/s]

  find_fenced_codes  full=    2000  standalone=    2000
                        time:   [175.43 µs 175.65 µs 175.84 µs]
                        thrpt:  [8.1618 GiB/s 8.1708 GiB/s 8.1808 GiB/s]

  find_blockquotes   full=    4000  standalone=    4000
                        time:   [100.80 µs 100.89 µs 100.98 µs]
                        thrpt:  [14.212 GiB/s 14.226 GiB/s 14.238 GiB/s]

  find_bullet_items  full=    6000  standalone=    6000
                        time:   [226.50 µs 226.88 µs 227.18 µs]
                        thrpt:  [6.3174 GiB/s 6.3257 GiB/s 6.3364 GiB/s]

  find_ordered_items full=    4000  standalone=    4000
                        time:   [197.08 µs 197.24 µs 197.37 µs]
                        thrpt:  [7.2714 GiB/s 7.2763 GiB/s 7.2822 GiB/s]
```

</details>

**big:**

<details>
<summary>plain</summary>

```
  find_codes         full=       0  standalone=       0
                        time:   [11.383 ms 11.421 ms 11.450 ms]
                        thrpt:  [23.895 GiB/s 23.956 GiB/s 24.035 GiB/s]

  find_italics       full=       0  standalone=       0
                        time:   [11.372 ms 11.399 ms 11.432 ms]
                        thrpt:  [23.934 GiB/s 24.002 GiB/s 24.060 GiB/s]

  find_bolds         full=       0  standalone=       0
                        time:   [11.329 ms 11.353 ms 11.388 ms]
                        thrpt:  [24.025 GiB/s 24.100 GiB/s 24.151 GiB/s]

  find_bold_italics  full=       0  standalone=       0
                        time:   [11.321 ms 11.343 ms 11.369 ms]
                        thrpt:  [24.067 GiB/s 24.121 GiB/s 24.168 GiB/s]

  find_autolinks     full=       0  standalone=       0
                        time:   [11.337 ms 11.357 ms 11.384 ms]
                        thrpt:  [24.034 GiB/s 24.091 GiB/s 24.135 GiB/s]

  find_links         full=       0  standalone=       0
                        time:   [11.475 ms 11.492 ms 11.511 ms]
                        thrpt:  [23.769 GiB/s 23.807 GiB/s 23.843 GiB/s]

  find_headings      full=       0  standalone=       0
                        time:   [11.348 ms 11.362 ms 11.378 ms]
                        thrpt:  [24.047 GiB/s 24.080 GiB/s 24.111 GiB/s]

  find_thematic_breaks full=       0  standalone=       0
                        time:   [12.888 ms 12.916 ms 12.951 ms]
                        thrpt:  [21.125 GiB/s 21.184 GiB/s 21.230 GiB/s]

  find_fenced_codes  full=       0  standalone=       0
                        time:   [11.339 ms 11.366 ms 11.393 ms]
                        thrpt:  [24.014 GiB/s 24.073 GiB/s 24.129 GiB/s]

  find_blockquotes   full=       0  standalone=       0
                        time:   [11.375 ms 11.419 ms 11.456 ms]
                        thrpt:  [23.883 GiB/s 23.960 GiB/s 24.053 GiB/s]

  find_bullet_items  full=       0  standalone=       0
                        time:   [12.870 ms 12.881 ms 12.895 ms]
                        thrpt:  [21.218 GiB/s 21.241 GiB/s 21.260 GiB/s]

  find_ordered_items full=       0  standalone=       0
                        time:   [27.008 ms 27.020 ms 27.035 ms]
                        thrpt:  [10.120 GiB/s 10.126 GiB/s 10.130 GiB/s]
```

</details>

<details>
<summary>hot</summary>

```
  find_codes         full=  500000  standalone=  500000
                        time:   [7.4700 ms 7.4783 ms 7.4878 ms]
                        thrpt:  [9.8334 GiB/s 9.8458 GiB/s 9.8567 GiB/s]

  find_italics       full=  500000  standalone=  500000
                        time:   [11.796 ms 11.802 ms 11.807 ms]
                        thrpt:  [6.2362 GiB/s 6.2390 GiB/s 6.2420 GiB/s]

  find_bolds         full=  500000  standalone=  500000
                        time:   [11.960 ms 11.975 ms 11.995 ms]
                        thrpt:  [6.1386 GiB/s 6.1486 GiB/s 6.1565 GiB/s]

  find_bold_italics  full=       0  standalone=       0
                        time:   [11.364 ms 11.373 ms 11.382 ms]
                        thrpt:  [6.4690 GiB/s 6.4743 GiB/s 6.4793 GiB/s]

  find_autolinks     full=  500000  standalone=  500000
                        time:   [7.4069 ms 7.4524 ms 7.4968 ms]
                        thrpt:  [9.8216 GiB/s 9.8800 GiB/s 9.9408 GiB/s]

  find_links         full=  500000  standalone=  500000
                        time:   [10.058 ms 10.068 ms 10.076 ms]
                        thrpt:  [7.3072 GiB/s 7.3136 GiB/s 7.3208 GiB/s]

  find_headings      full=  500000  standalone=  500000
                        time:   [7.4904 ms 7.5599 ms 7.6439 ms]
                        thrpt:  [9.6326 GiB/s 9.7396 GiB/s 9.8299 GiB/s]

  find_thematic_breaks full=       0  standalone=       0
                        time:   [7.7056 ms 7.7126 ms 7.7186 ms]
                        thrpt:  [9.5394 GiB/s 9.5468 GiB/s 9.5554 GiB/s]

  find_fenced_codes  full=       0  standalone=       0
                        time:   [7.0821 ms 7.0865 ms 7.0904 ms]
                        thrpt:  [10.385 GiB/s 10.390 GiB/s 10.397 GiB/s]

  find_blockquotes   full=       0  standalone=       0
                        time:   [4.5480 ms 4.5571 ms 4.5668 ms]
                        thrpt:  [16.123 GiB/s 16.157 GiB/s 16.190 GiB/s]

  find_bullet_items  full=       0  standalone=       0
                        time:   [7.4247 ms 7.4371 ms 7.4477 ms]
                        thrpt:  [9.8863 GiB/s 9.9005 GiB/s 9.9169 GiB/s]

  find_ordered_items full=       0  standalone=       0
                        time:   [9.8883 ms 9.9442 ms 10.005 ms]
                        thrpt:  [7.3594 GiB/s 7.4044 GiB/s 7.4462 GiB/s]
```

</details>

<details>
<summary>heavy</summary>

```
  find_codes         full= 1000000  standalone= 1000000
                        time:   [16.969 ms 16.991 ms 17.009 ms]
                        thrpt:  [8.4378 GiB/s 8.4470 GiB/s 8.4576 GiB/s]

  find_italics       full= 1200000  standalone= 1200000
                        time:   [34.896 ms 34.935 ms 34.978 ms]
                        thrpt:  [4.1031 GiB/s 4.1082 GiB/s 4.1127 GiB/s]

  find_bolds         full= 1200000  standalone= 1200000
                        time:   [35.445 ms 35.495 ms 35.561 ms]
                        thrpt:  [4.0358 GiB/s 4.0433 GiB/s 4.0491 GiB/s]

  find_bold_italics  full=  600000  standalone=  600000
                        time:   [34.660 ms 34.678 ms 34.691 ms]
                        thrpt:  [4.1371 GiB/s 4.1386 GiB/s 4.1408 GiB/s]

  find_autolinks     full=  400000  standalone=  400000
                        time:   [8.7289 ms 8.7433 ms 8.7592 ms]
                        thrpt:  [16.385 GiB/s 16.415 GiB/s 16.442 GiB/s]

  find_links         full=  600000  standalone=  600000
                        time:   [13.709 ms 13.729 ms 13.752 ms]
                        thrpt:  [10.436 GiB/s 10.454 GiB/s 10.469 GiB/s]

  find_headings      full=  200000  standalone=  200000
                        time:   [7.8347 ms 7.8522 ms 7.8689 ms]
                        thrpt:  [18.239 GiB/s 18.277 GiB/s 18.318 GiB/s]

  find_thematic_breaks full=  200000  standalone=  200000
                        time:   [26.790 ms 26.812 ms 26.846 ms]
                        thrpt:  [5.3459 GiB/s 5.3528 GiB/s 5.3571 GiB/s]

  find_fenced_codes  full=  200000  standalone=  200000
                        time:   [18.492 ms 18.512 ms 18.526 ms]
                        thrpt:  [7.7469 GiB/s 7.7527 GiB/s 7.7609 GiB/s]

  find_blockquotes   full=  400000  standalone=  400000
                        time:   [12.521 ms 12.533 ms 12.547 ms]
                        thrpt:  [11.439 GiB/s 11.451 GiB/s 11.462 GiB/s]

  find_bullet_items  full=  600000  standalone=  600000
                        time:   [23.661 ms 23.678 ms 23.698 ms]
                        thrpt:  [6.0561 GiB/s 6.0612 GiB/s 6.0657 GiB/s]

  find_ordered_items full=  400000  standalone=  400000
                        time:   [20.671 ms 20.692 ms 20.718 ms]
                        thrpt:  [6.9274 GiB/s 6.9359 GiB/s 6.9430 GiB/s]
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

Shown for both `small` and `big`.

### stable - `cargo bench --bench meon-md_standalone`

**small:**

<details>
<summary>plain</summary>

```
  context regions: 0
                        time:   [37.739 µs 38.703 µs 39.792 µs]
                        thrpt:  [68.759 GiB/s 70.693 GiB/s 72.499 GiB/s]

  find_context_italics full=       0  context-aware=       0
                        time:   [31.954 µs 33.106 µs 34.060 µs]
                        thrpt:  [80.329 GiB/s 82.644 GiB/s 85.625 GiB/s]

  find_context_bolds full=       0  context-aware=       0
                        time:   [29.739 µs 30.648 µs 31.475 µs]
                        thrpt:  [86.929 GiB/s 89.273 GiB/s 92.003 GiB/s]

  find_context_bold_italics full=       0  context-aware=       0
                        time:   [29.803 µs 30.784 µs 31.718 µs]
                        thrpt:  [86.262 GiB/s 88.879 GiB/s 91.806 GiB/s]

  find_context_headings full=       0  context-aware=       0
                        time:   [31.480 µs 31.841 µs 32.313 µs]
                        thrpt:  [84.674 GiB/s 85.927 GiB/s 86.913 GiB/s]

  find_context_thematic_breaks full=       0  context-aware=       0
                        time:   [39.708 µs 40.383 µs 41.322 µs]
                        thrpt:  [66.212 GiB/s 67.753 GiB/s 68.904 GiB/s]

  find_context_blockquotes full=       0  context-aware=       0
                        time:   [30.902 µs 31.625 µs 32.287 µs]
                        thrpt:  [84.742 GiB/s 86.516 GiB/s 88.538 GiB/s]

  find_context_bullet_items full=       0  context-aware=       0
                        time:   [41.252 µs 42.353 µs 43.312 µs]
                        thrpt:  [63.170 GiB/s 64.601 GiB/s 66.325 GiB/s]

  find_context_ordered_items full=       0  context-aware=       0
                        time:   [286.25 µs 288.22 µs 290.13 µs]
                        thrpt:  [9.4306 GiB/s 9.4927 GiB/s 9.5584 GiB/s]

  find_context_italics_cold
                        time:   [71.632 µs 73.457 µs 74.998 µs]
                        thrpt:  [36.482 GiB/s 37.247 GiB/s 38.196 GiB/s]

  find_context_bolds_cold
                        time:   [72.945 µs 75.071 µs 76.664 µs]
                        thrpt:  [35.689 GiB/s 36.446 GiB/s 37.508 GiB/s]

  find_context_bold_italics_cold
                        time:   [73.264 µs 75.327 µs 76.807 µs]
                        thrpt:  [35.622 GiB/s 36.322 GiB/s 37.345 GiB/s]

  find_context_headings_cold
                        time:   [69.012 µs 70.988 µs 73.605 µs]
                        thrpt:  [37.172 GiB/s 38.543 GiB/s 39.646 GiB/s]

  find_context_thematic_breaks_cold
                        time:   [80.498 µs 82.085 µs 83.208 µs]
                        thrpt:  [32.882 GiB/s 33.332 GiB/s 33.989 GiB/s]

  find_context_blockquotes_cold
                        time:   [72.193 µs 73.540 µs 75.055 µs]
                        thrpt:  [36.454 GiB/s 37.205 GiB/s 37.899 GiB/s]

  find_context_bullet_items_cold
                        time:   [80.338 µs 82.021 µs 84.272 µs]
                        thrpt:  [32.467 GiB/s 33.358 GiB/s 34.057 GiB/s]

  find_context_ordered_items_cold
                        time:   [338.00 µs 347.62 µs 359.27 µs]
                        thrpt:  [7.6155 GiB/s 7.8708 GiB/s 8.0947 GiB/s]
```

</details>

<details>
<summary>hot</summary>

```
  context regions: 10000
                        time:   [162.62 µs 162.94 µs 163.32 µs]
                        thrpt:  [4.5084 GiB/s 4.5190 GiB/s 4.5278 GiB/s]

  find_context_italics full=    5000  context-aware=    5000
                        time:   [142.99 µs 143.73 µs 144.53 µs]
                        thrpt:  [5.0946 GiB/s 5.1228 GiB/s 5.1493 GiB/s]

  find_context_bolds full=    5000  context-aware=    5000
                        time:   [148.50 µs 148.83 µs 149.25 µs]
                        thrpt:  [4.9334 GiB/s 4.9474 GiB/s 4.9582 GiB/s]

  find_context_bold_italics full=       0  context-aware=       0
                        time:   [124.69 µs 124.82 µs 124.95 µs]
                        thrpt:  [5.8926 GiB/s 5.8991 GiB/s 5.9053 GiB/s]

  find_context_headings full=    5000  context-aware=    5000
                        time:   [81.898 µs 81.954 µs 82.015 µs]
                        thrpt:  [8.9777 GiB/s 8.9844 GiB/s 8.9904 GiB/s]

  find_context_thematic_breaks full=       0  context-aware=       0
                        time:   [73.685 µs 73.858 µs 73.992 µs]
                        thrpt:  [9.9511 GiB/s 9.9692 GiB/s 9.9925 GiB/s]

  find_context_blockquotes full=       0  context-aware=       0
                        time:   [39.516 µs 39.652 µs 39.774 µs]
                        thrpt:  [18.512 GiB/s 18.569 GiB/s 18.633 GiB/s]

  find_context_bullet_items full=       0  context-aware=       0
                        time:   [72.861 µs 73.004 µs 73.133 µs]
                        thrpt:  [10.068 GiB/s 10.086 GiB/s 10.106 GiB/s]

  find_context_ordered_items full=       0  context-aware=       0
                        time:   [98.582 µs 99.153 µs 99.751 µs]
                        thrpt:  [7.3814 GiB/s 7.4260 GiB/s 7.4690 GiB/s]

  find_context_italics_cold
                        time:   [300.65 µs 300.92 µs 301.23 µs]
                        thrpt:  [2.4443 GiB/s 2.4469 GiB/s 2.4490 GiB/s]

  find_context_bolds_cold
                        time:   [304.14 µs 304.25 µs 304.35 µs]
                        thrpt:  [2.4192 GiB/s 2.4201 GiB/s 2.4209 GiB/s]

  find_context_bold_italics_cold
                        time:   [281.22 µs 281.43 µs 281.65 µs]
                        thrpt:  [2.6142 GiB/s 2.6163 GiB/s 2.6182 GiB/s]

  find_context_headings_cold
                        time:   [242.83 µs 243.95 µs 245.56 µs]
                        thrpt:  [2.9984 GiB/s 3.0183 GiB/s 3.0322 GiB/s]

  find_context_thematic_breaks_cold
                        time:   [237.04 µs 237.23 µs 237.49 µs]
                        thrpt:  [3.1003 GiB/s 3.1037 GiB/s 3.1062 GiB/s]

  find_context_blockquotes_cold
                        time:   [201.91 µs 202.11 µs 202.37 µs]
                        thrpt:  [3.6385 GiB/s 3.6431 GiB/s 3.6466 GiB/s]

  find_context_bullet_items_cold
                        time:   [237.63 µs 237.85 µs 238.11 µs]
                        thrpt:  [3.0923 GiB/s 3.0957 GiB/s 3.0985 GiB/s]

  find_context_ordered_items_cold
                        time:   [263.69 µs 264.12 µs 264.65 µs]
                        thrpt:  [2.7822 GiB/s 2.7877 GiB/s 2.7923 GiB/s]
```

</details>

<details>
<summary>heavy</summary>

```
  context regions: 16000
                        time:   [306.52 µs 307.70 µs 308.98 µs]
                        thrpt:  [4.6450 GiB/s 4.6643 GiB/s 4.6822 GiB/s]

  find_context_italics full=   12000  context-aware=   12000
                        time:   [452.76 µs 455.16 µs 457.12 µs]
                        thrpt:  [3.1396 GiB/s 3.1532 GiB/s 3.1699 GiB/s]

  find_context_bolds full=   12000  context-aware=   12000
                        time:   [449.78 µs 450.90 µs 452.18 µs]
                        thrpt:  [3.1739 GiB/s 3.1829 GiB/s 3.1909 GiB/s]

  find_context_bold_italics full=    6000  context-aware=    6000
                        time:   [435.22 µs 435.70 µs 436.26 µs]
                        thrpt:  [3.2898 GiB/s 3.2940 GiB/s 3.2976 GiB/s]

  find_context_headings full=    2000  context-aware=    2000
                        time:   [45.328 µs 45.630 µs 46.036 µs]
                        thrpt:  [31.175 GiB/s 31.452 GiB/s 31.662 GiB/s]

  find_context_thematic_breaks full=    2000  context-aware=    2000
                        time:   [291.00 µs 291.54 µs 292.16 µs]
                        thrpt:  [4.9124 GiB/s 4.9228 GiB/s 4.9319 GiB/s]

  find_context_blockquotes full=    4000  context-aware=    4000
                        time:   [122.71 µs 122.82 µs 122.99 µs]
                        thrpt:  [11.669 GiB/s 11.685 GiB/s 11.696 GiB/s]

  find_context_bullet_items full=    6000  context-aware=    6000
                        time:   [282.36 µs 283.16 µs 284.41 µs]
                        thrpt:  [5.0462 GiB/s 5.0685 GiB/s 5.0828 GiB/s]

  find_context_ordered_items full=    4000  context-aware=    4000
                        time:   [236.18 µs 236.56 µs 237.11 µs]
                        thrpt:  [6.0528 GiB/s 6.0670 GiB/s 6.0766 GiB/s]

  find_context_italics_cold
                        time:   [748.06 µs 750.28 µs 752.63 µs]
                        thrpt:  [1.9069 GiB/s 1.9129 GiB/s 1.9185 GiB/s]

  find_context_bolds_cold
                        time:   [750.07 µs 752.33 µs 755.82 µs]
                        thrpt:  [1.8988 GiB/s 1.9077 GiB/s 1.9134 GiB/s]

  find_context_bold_italics_cold
                        time:   [731.87 µs 732.97 µs 734.76 µs]
                        thrpt:  [1.9533 GiB/s 1.9580 GiB/s 1.9610 GiB/s]

  find_context_headings_cold
                        time:   [351.15 µs 354.38 µs 356.82 µs]
                        thrpt:  [4.0222 GiB/s 4.0498 GiB/s 4.0871 GiB/s]

  find_context_thematic_breaks_cold
                        time:   [599.02 µs 600.65 µs 602.88 µs]
                        thrpt:  [2.3805 GiB/s 2.3894 GiB/s 2.3959 GiB/s]

  find_context_blockquotes_cold
                        time:   [421.86 µs 425.17 µs 428.29 µs]
                        thrpt:  [3.3510 GiB/s 3.3755 GiB/s 3.4020 GiB/s]

  find_context_bullet_items_cold
                        time:   [589.55 µs 591.17 µs 593.05 µs]
                        thrpt:  [2.4200 GiB/s 2.4277 GiB/s 2.4344 GiB/s]

  find_context_ordered_items_cold
                        time:   [543.56 µs 544.87 µs 546.28 µs]
                        thrpt:  [2.6272 GiB/s 2.6340 GiB/s 2.6404 GiB/s]
```

</details>

**big:**

<details>
<summary>plain</summary>

```
  context regions: 0
                        time:   [12.370 ms 12.399 ms 12.428 ms]
                        thrpt:  [22.015 GiB/s 22.067 GiB/s 22.119 GiB/s]

  find_context_italics full=       0  context-aware=       0
                        time:   [11.396 ms 11.412 ms 11.431 ms]
                        thrpt:  [23.934 GiB/s 23.976 GiB/s 24.008 GiB/s]

  find_context_bolds full=       0  context-aware=       0
                        time:   [11.437 ms 11.475 ms 11.528 ms]
                        thrpt:  [23.733 GiB/s 23.843 GiB/s 23.922 GiB/s]

  find_context_bold_italics full=       0  context-aware=       0
                        time:   [11.533 ms 11.553 ms 11.582 ms]
                        thrpt:  [23.623 GiB/s 23.682 GiB/s 23.724 GiB/s]

  find_context_headings full=       0  context-aware=       0
                        time:   [11.404 ms 11.425 ms 11.449 ms]
                        thrpt:  [23.899 GiB/s 23.949 GiB/s 23.993 GiB/s]

  find_context_thematic_breaks full=       0  context-aware=       0
                        time:   [13.108 ms 13.137 ms 13.169 ms]
                        thrpt:  [20.777 GiB/s 20.827 GiB/s 20.873 GiB/s]

  find_context_blockquotes full=       0  context-aware=       0
                        time:   [11.457 ms 11.472 ms 11.484 ms]
                        thrpt:  [23.824 GiB/s 23.851 GiB/s 23.880 GiB/s]

  find_context_bullet_items full=       0  context-aware=       0
                        time:   [13.083 ms 13.102 ms 13.125 ms]
                        thrpt:  [20.846 GiB/s 20.883 GiB/s 20.913 GiB/s]

  find_context_ordered_items full=       0  context-aware=       0
                        time:   [29.961 ms 29.981 ms 29.998 ms]
                        thrpt:  [9.1206 GiB/s 9.1258 GiB/s 9.1319 GiB/s]

  find_context_italics_cold
                        time:   [23.838 ms 23.904 ms 23.988 ms]
                        thrpt:  [11.406 GiB/s 11.446 GiB/s 11.478 GiB/s]

  find_context_bolds_cold
                        time:   [24.186 ms 24.236 ms 24.294 ms]
                        thrpt:  [11.262 GiB/s 11.289 GiB/s 11.313 GiB/s]

  find_context_bold_italics_cold
                        time:   [24.118 ms 24.160 ms 24.204 ms]
                        thrpt:  [11.304 GiB/s 11.325 GiB/s 11.344 GiB/s]

  find_context_headings_cold
                        time:   [23.740 ms 23.792 ms 23.850 ms]
                        thrpt:  [11.472 GiB/s 11.500 GiB/s 11.525 GiB/s]

  find_context_thematic_breaks_cold
                        time:   [25.319 ms 25.369 ms 25.422 ms]
                        thrpt:  [10.762 GiB/s 10.785 GiB/s 10.806 GiB/s]

  find_context_blockquotes_cold
                        time:   [24.201 ms 24.251 ms 24.318 ms]
                        thrpt:  [11.251 GiB/s 11.282 GiB/s 11.305 GiB/s]

  find_context_bullet_items_cold
                        time:   [25.433 ms 25.487 ms 25.556 ms]
                        thrpt:  [10.706 GiB/s 10.735 GiB/s 10.758 GiB/s]

  find_context_ordered_items_cold
                        time:   [42.475 ms 42.536 ms 42.602 ms]
                        thrpt:  [6.4224 GiB/s 6.4323 GiB/s 6.4415 GiB/s]
```

</details>

<details>
<summary>hot</summary>

```
  context regions: 1000000
                        time:   [16.036 ms 16.050 ms 16.064 ms]
                        thrpt:  [4.5835 GiB/s 4.5876 GiB/s 4.5914 GiB/s]

  find_context_italics full=  500000  context-aware=  500000
                        time:   [14.431 ms 14.454 ms 14.471 ms]
                        thrpt:  [5.0881 GiB/s 5.0941 GiB/s 5.1023 GiB/s]

  find_context_bolds full=  500000  context-aware=  500000
                        time:   [15.016 ms 15.055 ms 15.082 ms]
                        thrpt:  [4.8821 GiB/s 4.8909 GiB/s 4.9034 GiB/s]

  find_context_bold_italics full=       0  context-aware=       0
                        time:   [12.628 ms 12.638 ms 12.650 ms]
                        thrpt:  [5.8206 GiB/s 5.8260 GiB/s 5.8308 GiB/s]

  find_context_headings full=  500000  context-aware=  500000
                        time:   [8.6732 ms 8.6934 ms 8.7130 ms]
                        thrpt:  [8.4506 GiB/s 8.4697 GiB/s 8.4894 GiB/s]

  find_context_thematic_breaks full=       0  context-aware=       0
                        time:   [7.8141 ms 7.8217 ms 7.8291 ms]
                        thrpt:  [9.4047 GiB/s 9.4136 GiB/s 9.4227 GiB/s]

  find_context_blockquotes full=       0  context-aware=       0
                        time:   [4.7995 ms 4.8147 ms 4.8311 ms]
                        thrpt:  [15.241 GiB/s 15.293 GiB/s 15.341 GiB/s]

  find_context_bullet_items full=       0  context-aware=       0
                        time:   [7.7026 ms 7.7091 ms 7.7162 ms]
                        thrpt:  [9.5423 GiB/s 9.5511 GiB/s 9.5592 GiB/s]

  find_context_ordered_items full=       0  context-aware=       0
                        time:   [10.325 ms 10.343 ms 10.357 ms]
                        thrpt:  [7.1092 GiB/s 7.1192 GiB/s 7.1309 GiB/s]

  find_context_italics_cold
                        time:   [30.445 ms 30.464 ms 30.480 ms]
                        thrpt:  [2.4157 GiB/s 2.4169 GiB/s 2.4185 GiB/s]

  find_context_bolds_cold
                        time:   [30.908 ms 30.940 ms 30.987 ms]
                        thrpt:  [2.3762 GiB/s 2.3798 GiB/s 2.3822 GiB/s]

  find_context_bold_italics_cold
                        time:   [29.190 ms 29.217 ms 29.243 ms]
                        thrpt:  [2.5179 GiB/s 2.5201 GiB/s 2.5225 GiB/s]

  find_context_headings_cold
                        time:   [24.763 ms 24.787 ms 24.815 ms]
                        thrpt:  [2.9672 GiB/s 2.9705 GiB/s 2.9734 GiB/s]

  find_context_thematic_breaks_cold
                        time:   [23.865 ms 23.882 ms 23.899 ms]
                        thrpt:  [3.0809 GiB/s 3.0831 GiB/s 3.0852 GiB/s]

  find_context_blockquotes_cold
                        time:   [20.947 ms 21.061 ms 21.151 ms]
                        thrpt:  [3.4811 GiB/s 3.4960 GiB/s 3.5150 GiB/s]

  find_context_bullet_items_cold
                        time:   [24.298 ms 24.330 ms 24.364 ms]
                        thrpt:  [3.0221 GiB/s 3.0263 GiB/s 3.0303 GiB/s]

  find_context_ordered_items_cold
                        time:   [26.611 ms 26.630 ms 26.649 ms]
                        thrpt:  [2.7630 GiB/s 2.7650 GiB/s 2.7669 GiB/s]
```

</details>

<details>
<summary>heavy</summary>

```
  context regions: 1600000
                        time:   [30.269 ms 30.316 ms 30.362 ms]
                        thrpt:  [4.7268 GiB/s 4.7341 GiB/s 4.7414 GiB/s]

  find_context_italics full= 1200000  context-aware= 1200000
                        time:   [44.809 ms 44.995 ms 45.169 ms]
                        thrpt:  [3.1774 GiB/s 3.1896 GiB/s 3.2029 GiB/s]

  find_context_bolds full= 1200000  context-aware= 1200000
                        time:   [45.382 ms 45.482 ms 45.633 ms]
                        thrpt:  [3.1451 GiB/s 3.1555 GiB/s 3.1625 GiB/s]

  find_context_bold_italics full=  600000  context-aware=  600000
                        time:   [43.573 ms 43.682 ms 43.795 ms]
                        thrpt:  [3.2770 GiB/s 3.2855 GiB/s 3.2938 GiB/s]

  find_context_headings full=  200000  context-aware=  200000
                        time:   [8.6334 ms 8.6568 ms 8.6869 ms]
                        thrpt:  [16.521 GiB/s 16.579 GiB/s 16.624 GiB/s]

  find_context_thematic_breaks full=  200000  context-aware=  200000
                        time:   [29.643 ms 29.665 ms 29.690 ms]
                        thrpt:  [4.8338 GiB/s 4.8380 GiB/s 4.8416 GiB/s]

  find_context_blockquotes full=  400000  context-aware=  400000
                        time:   [13.925 ms 13.939 ms 13.955 ms]
                        thrpt:  [10.284 GiB/s 10.296 GiB/s 10.307 GiB/s]

  find_context_bullet_items full=  600000  context-aware=  600000
                        time:   [28.558 ms 28.594 ms 28.632 ms]
                        thrpt:  [5.0126 GiB/s 5.0192 GiB/s 5.0255 GiB/s]

  find_context_ordered_items full=  400000  context-aware=  400000
                        time:   [24.069 ms 24.281 ms 24.530 ms]
                        thrpt:  [5.8506 GiB/s 5.9107 GiB/s 5.9628 GiB/s]

  find_context_italics_cold
                        time:   [76.141 ms 76.276 ms 76.364 ms]
                        thrpt:  [1.8794 GiB/s 1.8816 GiB/s 1.8849 GiB/s]

  find_context_bolds_cold
                        time:   [75.479 ms 75.611 ms 75.799 ms]
                        thrpt:  [1.8934 GiB/s 1.8981 GiB/s 1.9014 GiB/s]

  find_context_bold_italics_cold
                        time:   [73.866 ms 74.046 ms 74.245 ms]
                        thrpt:  [1.9330 GiB/s 1.9382 GiB/s 1.9430 GiB/s]

  find_context_headings_cold
                        time:   [39.551 ms 39.580 ms 39.604 ms]
                        thrpt:  [3.6239 GiB/s 3.6261 GiB/s 3.6287 GiB/s]

  find_context_thematic_breaks_cold
                        time:   [59.931 ms 60.084 ms 60.242 ms]
                        thrpt:  [2.3824 GiB/s 2.3886 GiB/s 2.3947 GiB/s]

  find_context_blockquotes_cold
                        time:   [44.866 ms 44.966 ms 45.038 ms]
                        thrpt:  [3.1866 GiB/s 3.1917 GiB/s 3.1988 GiB/s]

  find_context_bullet_items_cold
                        time:   [59.037 ms 59.108 ms 59.210 ms]
                        thrpt:  [2.4239 GiB/s 2.4281 GiB/s 2.4310 GiB/s]

  find_context_ordered_items_cold
                        time:   [55.076 ms 55.155 ms 55.240 ms]
                        thrpt:  [2.5981 GiB/s 2.6021 GiB/s 2.6059 GiB/s]
```

</details>

### nightly - `RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_standalone --features avx2`

**small:**

<details>
<summary>plain</summary>

```
  context regions: 0
                        time:   [35.688 µs 36.546 µs 37.325 µs]
                        thrpt:  [73.302 GiB/s 74.865 GiB/s 76.666 GiB/s]

  find_context_italics full=       0  context-aware=       0
                        time:   [28.993 µs 30.114 µs 31.268 µs]
                        thrpt:  [87.502 GiB/s 90.856 GiB/s 94.370 GiB/s]

  find_context_bolds full=       0  context-aware=       0
                        time:   [28.808 µs 29.443 µs 30.131 µs]
                        thrpt:  [90.805 GiB/s 92.925 GiB/s 94.974 GiB/s]

  find_context_bold_italics full=       0  context-aware=       0
                        time:   [28.119 µs 28.640 µs 29.104 µs]
                        thrpt:  [94.010 GiB/s 95.533 GiB/s 97.302 GiB/s]

  find_context_headings full=       0  context-aware=       0
                        time:   [28.056 µs 28.358 µs 28.624 µs]
                        thrpt:  [95.585 GiB/s 96.483 GiB/s 97.521 GiB/s]

  find_context_thematic_breaks full=       0  context-aware=       0
                        time:   [43.845 µs 44.580 µs 45.479 µs]
                        thrpt:  [60.160 GiB/s 61.374 GiB/s 62.402 GiB/s]

  find_context_blockquotes full=       0  context-aware=       0
                        time:   [28.580 µs 29.380 µs 29.982 µs]
                        thrpt:  [91.255 GiB/s 93.127 GiB/s 95.732 GiB/s]

  find_context_bullet_items full=       0  context-aware=       0
                        time:   [41.407 µs 42.376 µs 43.234 µs]
                        thrpt:  [63.285 GiB/s 64.566 GiB/s 66.077 GiB/s]

  find_context_ordered_items full=       0  context-aware=       0
                        time:   [256.98 µs 257.73 µs 258.12 µs]
                        thrpt:  [10.600 GiB/s 10.616 GiB/s 10.647 GiB/s]

  find_context_italics_cold
                        time:   [65.588 µs 67.706 µs 69.301 µs]
                        thrpt:  [39.481 GiB/s 40.410 GiB/s 41.716 GiB/s]

  find_context_bolds_cold
                        time:   [65.095 µs 66.238 µs 67.402 µs]
                        thrpt:  [40.593 GiB/s 41.306 GiB/s 42.031 GiB/s]

  find_context_bold_italics_cold
                        time:   [64.356 µs 65.812 µs 67.348 µs]
                        thrpt:  [40.626 GiB/s 41.573 GiB/s 42.514 GiB/s]

  find_context_headings_cold
                        time:   [65.134 µs 66.607 µs 68.618 µs]
                        thrpt:  [39.873 GiB/s 41.077 GiB/s 42.006 GiB/s]

  find_context_thematic_breaks_cold
                        time:   [74.991 µs 76.241 µs 77.541 µs]
                        thrpt:  [35.285 GiB/s 35.887 GiB/s 36.485 GiB/s]

  find_context_blockquotes_cold
                        time:   [66.122 µs 67.391 µs 68.829 µs]
                        thrpt:  [39.752 GiB/s 40.599 GiB/s 41.378 GiB/s]

  find_context_bullet_items_cold
                        time:   [75.022 µs 77.162 µs 78.941 µs]
                        thrpt:  [34.659 GiB/s 35.458 GiB/s 36.470 GiB/s]

  find_context_ordered_items_cold
                        time:   [287.19 µs 294.24 µs 302.15 µs]
                        thrpt:  [9.0553 GiB/s 9.2987 GiB/s 9.5270 GiB/s]
```

</details>

<details>
<summary>hot</summary>

```
  context regions: 10000
                        time:   [131.47 µs 131.66 µs 131.92 µs]
                        thrpt:  [5.5813 GiB/s 5.5924 GiB/s 5.6007 GiB/s]

  find_context_italics full=    5000  context-aware=    5000
                        time:   [114.89 µs 114.99 µs 115.07 µs]
                        thrpt:  [6.3987 GiB/s 6.4032 GiB/s 6.4090 GiB/s]

  find_context_bolds full=    5000  context-aware=    5000
                        time:   [116.83 µs 116.92 µs 117.01 µs]
                        thrpt:  [6.2925 GiB/s 6.2974 GiB/s 6.3024 GiB/s]

  find_context_bold_italics full=       0  context-aware=       0
                        time:   [111.48 µs 111.54 µs 111.61 µs]
                        thrpt:  [6.5973 GiB/s 6.6012 GiB/s 6.6049 GiB/s]

  find_context_headings full=    5000  context-aware=    5000
                        time:   [66.846 µs 66.916 µs 66.993 µs]
                        thrpt:  [10.991 GiB/s 11.003 GiB/s 11.015 GiB/s]

  find_context_thematic_breaks full=       0  context-aware=       0
                        time:   [68.813 µs 68.861 µs 68.904 µs]
                        thrpt:  [10.686 GiB/s 10.693 GiB/s 10.700 GiB/s]

  find_context_blockquotes full=       0  context-aware=       0
                        time:   [34.908 µs 34.943 µs 34.995 µs]
                        thrpt:  [21.040 GiB/s 21.072 GiB/s 21.093 GiB/s]

  find_context_bullet_items full=       0  context-aware=       0
                        time:   [65.869 µs 66.018 µs 66.179 µs]
                        thrpt:  [11.126 GiB/s 11.153 GiB/s 11.178 GiB/s]

  find_context_ordered_items full=       0  context-aware=       0
                        time:   [89.662 µs 89.713 µs 89.765 µs]
                        thrpt:  [8.2025 GiB/s 8.2073 GiB/s 8.2119 GiB/s]

  find_context_italics_cold
                        time:   [245.64 µs 245.76 µs 245.89 µs]
                        thrpt:  [2.9945 GiB/s 2.9961 GiB/s 2.9975 GiB/s]

  find_context_bolds_cold
                        time:   [247.22 µs 247.31 µs 247.39 µs]
                        thrpt:  [2.9763 GiB/s 2.9773 GiB/s 2.9783 GiB/s]

  find_context_bold_italics_cold
                        time:   [241.40 µs 241.50 µs 241.63 µs]
                        thrpt:  [3.0472 GiB/s 3.0489 GiB/s 3.0502 GiB/s]

  find_context_headings_cold
                        time:   [197.34 µs 197.51 µs 197.68 µs]
                        thrpt:  [3.7247 GiB/s 3.7279 GiB/s 3.7312 GiB/s]

  find_context_thematic_breaks_cold
                        time:   [200.02 µs 200.16 µs 200.31 µs]
                        thrpt:  [3.6759 GiB/s 3.6786 GiB/s 3.6811 GiB/s]

  find_context_blockquotes_cold
                        time:   [165.83 µs 166.00 µs 166.27 µs]
                        thrpt:  [4.4283 GiB/s 4.4356 GiB/s 4.4401 GiB/s]

  find_context_bullet_items_cold
                        time:   [195.70 µs 195.82 µs 195.93 µs]
                        thrpt:  [3.7580 GiB/s 3.7602 GiB/s 3.7625 GiB/s]

  find_context_ordered_items_cold
                        time:   [221.55 µs 222.10 µs 222.73 µs]
                        thrpt:  [3.3058 GiB/s 3.3151 GiB/s 3.3234 GiB/s]
```

</details>

<details>
<summary>heavy</summary>

```
  context regions: 16000
                        time:   [248.55 µs 249.67 µs 250.39 µs]
                        thrpt:  [5.7318 GiB/s 5.7484 GiB/s 5.7743 GiB/s]

  find_context_italics full=   12000  context-aware=   12000
                        time:   [360.06 µs 360.18 µs 360.27 µs]
                        thrpt:  [3.9836 GiB/s 3.9847 GiB/s 3.9859 GiB/s]

  find_context_bolds full=   12000  context-aware=   12000
                        time:   [364.49 µs 364.59 µs 364.66 µs]
                        thrpt:  [3.9357 GiB/s 3.9365 GiB/s 3.9375 GiB/s]

  find_context_bold_italics full=    6000  context-aware=    6000
                        time:   [353.65 µs 354.66 µs 355.62 µs]
                        thrpt:  [4.0357 GiB/s 4.0467 GiB/s 4.0582 GiB/s]

  find_context_headings full=    2000  context-aware=    2000
                        time:   [39.854 µs 40.405 µs 40.828 µs]
                        thrpt:  [35.152 GiB/s 35.520 GiB/s 36.011 GiB/s]

  find_context_thematic_breaks full=    2000  context-aware=    2000
                        time:   [266.21 µs 266.26 µs 266.30 µs]
                        thrpt:  [5.3893 GiB/s 5.3901 GiB/s 5.3912 GiB/s]

  find_context_blockquotes full=    4000  context-aware=    4000
                        time:   [108.05 µs 108.13 µs 108.23 µs]
                        thrpt:  [13.260 GiB/s 13.273 GiB/s 13.282 GiB/s]

  find_context_bullet_items full=    6000  context-aware=    6000
                        time:   [235.54 µs 235.63 µs 235.70 µs]
                        thrpt:  [6.0890 GiB/s 6.0909 GiB/s 6.0932 GiB/s]

  find_context_ordered_items full=    4000  context-aware=    4000
                        time:   [207.04 µs 207.42 µs 207.71 µs]
                        thrpt:  [6.9095 GiB/s 6.9193 GiB/s 6.9319 GiB/s]

  find_context_italics_cold
                        time:   [607.03 µs 607.40 µs 607.65 µs]
                        thrpt:  [2.3619 GiB/s 2.3629 GiB/s 2.3643 GiB/s]

  find_context_bolds_cold
                        time:   [615.54 µs 617.02 µs 618.71 µs]
                        thrpt:  [2.3196 GiB/s 2.3260 GiB/s 2.3316 GiB/s]

  find_context_bold_italics_cold
                        time:   [606.34 µs 606.87 µs 607.46 µs]
                        thrpt:  [2.3626 GiB/s 2.3649 GiB/s 2.3669 GiB/s]

  find_context_headings_cold
                        time:   [290.09 µs 290.31 µs 290.49 µs]
                        thrpt:  [4.9405 GiB/s 4.9436 GiB/s 4.9473 GiB/s]

  find_context_thematic_breaks_cold
                        time:   [515.56 µs 516.22 µs 516.89 µs]
                        thrpt:  [2.7766 GiB/s 2.7802 GiB/s 2.7837 GiB/s]

  find_context_blockquotes_cold
                        time:   [357.79 µs 357.98 µs 358.15 µs]
                        thrpt:  [4.0072 GiB/s 4.0092 GiB/s 4.0112 GiB/s]

  find_context_bullet_items_cold
                        time:   [482.42 µs 483.33 µs 484.28 µs]
                        thrpt:  [2.9635 GiB/s 2.9694 GiB/s 2.9750 GiB/s]

  find_context_ordered_items_cold
                        time:   [457.58 µs 457.96 µs 458.32 µs]
                        thrpt:  [3.1314 GiB/s 3.1339 GiB/s 3.1364 GiB/s]
```

</details>

**big:**

<details>
<summary>plain</summary>

```
  context regions: 0
                        time:   [12.349 ms 12.367 ms 12.389 ms]
                        thrpt:  [22.085 GiB/s 22.123 GiB/s 22.155 GiB/s]

  find_context_italics full=       0  context-aware=       0
                        time:   [11.371 ms 11.423 ms 11.471 ms]
                        thrpt:  [23.852 GiB/s 23.952 GiB/s 24.062 GiB/s]

  find_context_bolds full=       0  context-aware=       0
                        time:   [11.359 ms 11.376 ms 11.394 ms]
                        thrpt:  [24.012 GiB/s 24.052 GiB/s 24.088 GiB/s]

  find_context_bold_italics full=       0  context-aware=       0
                        time:   [11.378 ms 11.411 ms 11.456 ms]
                        thrpt:  [23.884 GiB/s 23.978 GiB/s 24.046 GiB/s]

  find_context_headings full=       0  context-aware=       0
                        time:   [11.349 ms 11.367 ms 11.396 ms]
                        thrpt:  [24.009 GiB/s 24.069 GiB/s 24.109 GiB/s]

  find_context_thematic_breaks full=       0  context-aware=       0
                        time:   [12.980 ms 13.001 ms 13.024 ms]
                        thrpt:  [21.008 GiB/s 21.045 GiB/s 21.078 GiB/s]

  find_context_blockquotes full=       0  context-aware=       0
                        time:   [11.380 ms 11.410 ms 11.448 ms]
                        thrpt:  [23.900 GiB/s 23.979 GiB/s 24.042 GiB/s]

  find_context_bullet_items full=       0  context-aware=       0
                        time:   [13.023 ms 13.035 ms 13.048 ms]
                        thrpt:  [20.969 GiB/s 20.990 GiB/s 21.010 GiB/s]

  find_context_ordered_items full=       0  context-aware=       0
                        time:   [27.054 ms 27.116 ms 27.187 ms]
                        thrpt:  [10.064 GiB/s 10.090 GiB/s 10.113 GiB/s]

  find_context_italics_cold
                        time:   [23.776 ms 23.806 ms 23.856 ms]
                        thrpt:  [11.469 GiB/s 11.493 GiB/s 11.508 GiB/s]

  find_context_bolds_cold
                        time:   [23.743 ms 23.786 ms 23.834 ms]
                        thrpt:  [11.480 GiB/s 11.503 GiB/s 11.523 GiB/s]

  find_context_bold_italics_cold
                        time:   [23.834 ms 23.968 ms 24.068 ms]
                        thrpt:  [11.368 GiB/s 11.415 GiB/s 11.480 GiB/s]

  find_context_headings_cold
                        time:   [23.825 ms 23.876 ms 23.940 ms]
                        thrpt:  [11.429 GiB/s 11.459 GiB/s 11.484 GiB/s]

  find_context_thematic_breaks_cold
                        time:   [25.473 ms 25.504 ms 25.534 ms]
                        thrpt:  [10.715 GiB/s 10.728 GiB/s 10.741 GiB/s]

  find_context_blockquotes_cold
                        time:   [23.874 ms 23.988 ms 24.077 ms]
                        thrpt:  [11.364 GiB/s 11.406 GiB/s 11.460 GiB/s]

  find_context_bullet_items_cold
                        time:   [25.227 ms 25.268 ms 25.306 ms]
                        thrpt:  [10.812 GiB/s 10.828 GiB/s 10.846 GiB/s]

  find_context_ordered_items_cold
                        time:   [40.171 ms 40.215 ms 40.255 ms]
                        thrpt:  [6.7967 GiB/s 6.8036 GiB/s 6.8110 GiB/s]
```

</details>

<details>
<summary>hot</summary>

```
  context regions: 1000000
                        time:   [13.783 ms 13.799 ms 13.818 ms]
                        thrpt:  [5.3285 GiB/s 5.3358 GiB/s 5.3420 GiB/s]

  find_context_italics full=  500000  context-aware=  500000
                        time:   [12.215 ms 12.233 ms 12.258 ms]
                        thrpt:  [6.0067 GiB/s 6.0189 GiB/s 6.0277 GiB/s]

  find_context_bolds full=  500000  context-aware=  500000
                        time:   [12.373 ms 12.381 ms 12.389 ms]
                        thrpt:  [5.9432 GiB/s 5.9471 GiB/s 5.9511 GiB/s]

  find_context_bold_italics full=       0  context-aware=       0
                        time:   [11.899 ms 11.908 ms 11.920 ms]
                        thrpt:  [6.1772 GiB/s 6.1831 GiB/s 6.1878 GiB/s]

  find_context_headings full=  500000  context-aware=  500000
                        time:   [7.6021 ms 7.6116 ms 7.6236 ms]
                        thrpt:  [9.6583 GiB/s 9.6734 GiB/s 9.6856 GiB/s]

  find_context_thematic_breaks full=       0  context-aware=       0
                        time:   [7.7211 ms 7.7276 ms 7.7352 ms]
                        thrpt:  [9.5189 GiB/s 9.5283 GiB/s 9.5362 GiB/s]

  find_context_blockquotes full=       0  context-aware=       0
                        time:   [4.5527 ms 4.5684 ms 4.5808 ms]
                        thrpt:  [16.074 GiB/s 16.117 GiB/s 16.173 GiB/s]

  find_context_bullet_items full=       0  context-aware=       0
                        time:   [7.3931 ms 7.4012 ms 7.4100 ms]
                        thrpt:  [9.9366 GiB/s 9.9485 GiB/s 9.9593 GiB/s]

  find_context_ordered_items full=       0  context-aware=       0
                        time:   [9.7225 ms 9.7335 ms 9.7497 ms]
                        thrpt:  [7.5521 GiB/s 7.5646 GiB/s 7.5732 GiB/s]

  find_context_italics_cold
                        time:   [26.152 ms 26.171 ms 26.195 ms]
                        thrpt:  [2.8109 GiB/s 2.8134 GiB/s 2.8155 GiB/s]

  find_context_bolds_cold
                        time:   [26.272 ms 26.300 ms 26.325 ms]
                        thrpt:  [2.7970 GiB/s 2.7996 GiB/s 2.8026 GiB/s]

  find_context_bold_italics_cold
                        time:   [25.638 ms 25.658 ms 25.675 ms]
                        thrpt:  [2.8678 GiB/s 2.8697 GiB/s 2.8719 GiB/s]

  find_context_headings_cold
                        time:   [21.313 ms 21.334 ms 21.370 ms]
                        thrpt:  [3.4456 GiB/s 3.4513 GiB/s 3.4547 GiB/s]

  find_context_thematic_breaks_cold
                        time:   [21.463 ms 21.483 ms 21.505 ms]
                        thrpt:  [3.4238 GiB/s 3.4274 GiB/s 3.4306 GiB/s]

  find_context_blockquotes_cold
                        time:   [18.360 ms 18.387 ms 18.422 ms]
                        thrpt:  [3.9969 GiB/s 4.0045 GiB/s 4.0103 GiB/s]

  find_context_bullet_items_cold
                        time:   [21.179 ms 21.190 ms 21.200 ms]
                        thrpt:  [3.4731 GiB/s 3.4747 GiB/s 3.4766 GiB/s]

  find_context_ordered_items_cold
                        time:   [23.610 ms 23.644 ms 23.682 ms]
                        thrpt:  [3.1091 GiB/s 3.1141 GiB/s 3.1186 GiB/s]
```

</details>

<details>
<summary>heavy</summary>

```
  context regions: 1600000
                        time:   [25.678 ms 25.748 ms 25.811 ms]
                        thrpt:  [5.5603 GiB/s 5.5739 GiB/s 5.5892 GiB/s]

  find_context_italics full= 1200000  context-aware= 1200000
                        time:   [36.679 ms 36.705 ms 36.745 ms]
                        thrpt:  [3.9058 GiB/s 3.9101 GiB/s 3.9128 GiB/s]

  find_context_bolds full= 1200000  context-aware= 1200000
                        time:   [36.943 ms 36.961 ms 36.975 ms]
                        thrpt:  [3.8815 GiB/s 3.8830 GiB/s 3.8849 GiB/s]

  find_context_bold_italics full=  600000  context-aware=  600000
                        time:   [35.782 ms 36.026 ms 36.212 ms]
                        thrpt:  [3.9633 GiB/s 3.9838 GiB/s 4.0110 GiB/s]

  find_context_headings full=  200000  context-aware=  200000
                        time:   [8.3653 ms 8.3862 ms 8.4058 ms]
                        thrpt:  [17.074 GiB/s 17.114 GiB/s 17.156 GiB/s]

  find_context_thematic_breaks full=  200000  context-aware=  200000
                        time:   [27.361 ms 27.381 ms 27.408 ms]
                        thrpt:  [5.2364 GiB/s 5.2415 GiB/s 5.2454 GiB/s]

  find_context_blockquotes full=  400000  context-aware=  400000
                        time:   [12.679 ms 12.689 ms 12.700 ms]
                        thrpt:  [11.301 GiB/s 11.310 GiB/s 11.320 GiB/s]

  find_context_bullet_items full=  600000  context-aware=  600000
                        time:   [24.503 ms 24.516 ms 24.527 ms]
                        thrpt:  [5.8514 GiB/s 5.8541 GiB/s 5.8572 GiB/s]

  find_context_ordered_items full=  400000  context-aware=  400000
                        time:   [21.721 ms 21.757 ms 21.790 ms]
                        thrpt:  [6.5863 GiB/s 6.5964 GiB/s 6.6075 GiB/s]

  find_context_italics_cold
                        time:   [62.192 ms 62.241 ms 62.296 ms]
                        thrpt:  [2.3038 GiB/s 2.3059 GiB/s 2.3077 GiB/s]

  find_context_bolds_cold
                        time:   [62.958 ms 63.008 ms 63.064 ms]
                        thrpt:  [2.2758 GiB/s 2.2778 GiB/s 2.2796 GiB/s]

  find_context_bold_italics_cold
                        time:   [61.841 ms 61.925 ms 62.023 ms]
                        thrpt:  [2.3140 GiB/s 2.3176 GiB/s 2.3208 GiB/s]

  find_context_headings_cold
                        time:   [34.058 ms 34.209 ms 34.410 ms]
                        thrpt:  [4.1709 GiB/s 4.1953 GiB/s 4.2140 GiB/s]

  find_context_thematic_breaks_cold
                        time:   [52.877 ms 52.939 ms 52.988 ms]
                        thrpt:  [2.7085 GiB/s 2.7110 GiB/s 2.7142 GiB/s]

  find_context_blockquotes_cold
                        time:   [38.592 ms 38.620 ms 38.645 ms]
                        thrpt:  [3.7138 GiB/s 3.7162 GiB/s 3.7189 GiB/s]

  find_context_bullet_items_cold
                        time:   [50.328 ms 50.443 ms 50.518 ms]
                        thrpt:  [2.8409 GiB/s 2.8452 GiB/s 2.8517 GiB/s]

  find_context_ordered_items_cold
                        time:   [47.369 ms 47.420 ms 47.487 ms]
                        thrpt:  [3.0223 GiB/s 3.0265 GiB/s 3.0298 GiB/s]
```

</details>

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
