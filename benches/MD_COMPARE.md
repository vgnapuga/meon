# meon-md — Cross-parser comparison

EN | [**RU**](https://github.com/vgnapuga/meon/blob/main/benches/MD_COMPARE_RU.md)

Throughput of [`meon-md`](https://github.com/vgnapuga/meon/blob/main/meon-md/README.md)
(built on the [`meon`](https://github.com/vgnapuga/meon/blob/main/meon/README.md)
engine) next to two CommonMark parsers, on the same corpora as the intra-engine
benches.

> **These numbers demonstrate an architectural difference, not a quality
> ranking.** `meon-md` is, by design, **not** CommonMark-compliant — it parses
> a Markdown subset into a flat, type-indexed span table (O(1) access per
> element kind, single-type extraction via `find_*`, zero-copy spans).
> `pulldown-cmark` and `comrak` are full CommonMark and produce an event stream
> / an AST. A throughput gap therefore reflects two different architectures with
> different goals. `Throughput::Bytes` measures how fast the input is consumed,
> since the three produce different things.

* **meon**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon/README.md)
  * [***crates.io***](https://crates.io/crates/meon)
* **meon-macros**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon-macros/README.md)
  * [***crates.io***](https://crates.io/crates/meon-macros)
* **meon-md**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon-md/README.md)
  * [***crates.io***](https://crates.io/crates/meon-md)

* [***CHANGELOG.md***](https://github.com/vgnapuga/meon/blob/main/CHANGELOG.md)
* [***ARCHITECTURE.md***](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE.md)
* [***BENCHMARKS.md***](https://github.com/vgnapuga/meon/blob/main/benches/README.md)
* * ***MD_COMPARE.md***    <--
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

## Why these numbers are demonstrative, not a ranking

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
Measured on the `small` corpora.

### stable - `cargo bench --bench meon-md_standalone`

<details>
<summary>plain</summary>

```
find_codes         full=       0  standalone=       0   thrpt: [6.1953 6.2034 6.2115 GiB/s]
find_italics       full=       0  standalone=       0   thrpt: [6.2317 6.2402 6.2484 GiB/s]
find_bolds         full=       0  standalone=       0   thrpt: [6.3200 6.3243 6.3279 GiB/s]
find_bold_italics  full=       0  standalone=       0   thrpt: [6.2220 6.2341 6.2451 GiB/s]
find_autolinks     full=       0  standalone=       0   thrpt: [6.0844 6.0918 6.0990 GiB/s]
find_links         full=       0  standalone=       0   thrpt: [6.0163 6.0318 6.0454 GiB/s]
find_headings      full=       0  standalone=       0   thrpt: [9.9465 10.009 10.061 GiB/s]
find_thematic_breaks full=     0  standalone=       0   thrpt: [9.3659 9.3792 9.3943 GiB/s]
find_fenced_codes  full=       0  standalone=       0   thrpt: [9.7946 9.8004 9.8056 GiB/s]
find_blockquotes   full=       0  standalone=       0   thrpt: [10.094 10.103 10.112 GiB/s]
find_bullet_items  full=       0  standalone=       0   thrpt: [9.5774 9.5851 9.5921 GiB/s]
find_ordered_items full=       0  standalone=       0   thrpt: [9.0396 9.0820 9.1271 GiB/s]
```

</details>

<details>
<summary>hot</summary>

```
find_codes         full=    5000  standalone=    5000   thrpt: [3.0572 3.0611 3.0652 GiB/s]
find_italics       full=    5000  standalone=    5000   thrpt: [2.5659 2.5688 2.5717 GiB/s]
find_bolds         full=    5000  standalone=    5000   thrpt: [2.5309 2.5368 2.5424 GiB/s]
find_bold_italics  full=       0  standalone=       0   thrpt: [2.6338 2.6388 2.6441 GiB/s]
find_autolinks     full=    5000  standalone=    5000   thrpt: [2.9027 2.9067 2.9107 GiB/s]
find_links         full=    5000  standalone=    5000   thrpt: [2.6780 2.6835 2.6897 GiB/s]
find_headings      full=    5000  standalone=    5000   thrpt: [6.1249 6.1322 6.1389 GiB/s]
find_thematic_breaks full=     0  standalone=       0   thrpt: [5.8048 5.8129 5.8202 GiB/s]
find_fenced_codes  full=       0  standalone=       0   thrpt: [5.8966 5.8998 5.9022 GiB/s]
find_blockquotes   full=       0  standalone=       0   thrpt: [6.2932 6.2969 6.3004 GiB/s]
find_bullet_items  full=       0  standalone=       0   thrpt: [5.7912 5.7955 5.7995 GiB/s]
find_ordered_items full=       0  standalone=       0   thrpt: [5.6031 5.6141 5.6242 GiB/s]
```

</details>

<details>
<summary>heavy</summary>

```
find_codes         full=   10000  standalone=   10000   thrpt: [2.5170 2.5256 2.5345 GiB/s]
find_italics       full=   12000  standalone=   12000   thrpt: [1.8209 1.8255 1.8300 GiB/s]
find_bolds         full=   12000  standalone=   12000   thrpt: [1.8337 1.8354 1.8372 GiB/s]
find_bold_italics  full=    6000  standalone=    6000   thrpt: [1.8460 1.8520 1.8577 GiB/s]
find_autolinks     full=    4000  standalone=    4000   thrpt: [3.1476 3.1529 3.1582 GiB/s]
find_links         full=    6000  standalone=    6000   thrpt: [2.6893 2.6921 2.6951 GiB/s]
find_headings      full=    2000  standalone=    2000   thrpt: [5.6510 5.6622 5.6729 GiB/s]
find_thematic_breaks full=  2000  standalone=    2000   thrpt: [5.3913 5.3978 5.4039 GiB/s]
find_fenced_codes  full=    2000  standalone=    2000   thrpt: [5.1953 5.1998 5.2038 GiB/s]
find_blockquotes   full=    4000  standalone=    2000   thrpt: [5.5004 5.5056 5.5112 GiB/s]
find_bullet_items  full=    6000  standalone=    6000   thrpt: [5.3964 5.4017 5.4063 GiB/s]
find_ordered_items full=    4000  standalone=    4000   thrpt: [4.9289 4.9328 4.9367 GiB/s]
```

</details>

### nightly - `RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_standalone --features avx2`

<details>
<summary>plain</summary>

```
find_codes         full=       0  standalone=       0   thrpt: [8.1828 8.1932 8.2026 GiB/s]
find_italics       full=       0  standalone=       0   thrpt: [8.1114 8.1282 8.1432 GiB/s]
find_bolds         full=       0  standalone=       0   thrpt: [8.1344 8.1478 8.1602 GiB/s]
find_bold_italics  full=       0  standalone=       0   thrpt: [8.0428 8.0487 8.0549 GiB/s]
find_autolinks     full=       0  standalone=       0   thrpt: [8.3315 8.3499 8.3668 GiB/s]
find_links         full=       0  standalone=       0   thrpt: [8.4980 8.5150 8.5306 GiB/s]
find_headings      full=       0  standalone=       0   thrpt: [10.570 10.574 10.578 GiB/s]
find_thematic_breaks full=     0  standalone=       0   thrpt: [10.131 10.160 10.189 GiB/s]
find_fenced_codes  full=       0  standalone=       0   thrpt: [10.129 10.133 10.138 GiB/s]
find_blockquotes   full=       0  standalone=       0   thrpt: [11.147 11.159 11.170 GiB/s]
find_bullet_items  full=       0  standalone=       0   thrpt: [10.283 10.288 10.293 GiB/s]
find_ordered_items full=       0  standalone=       0   thrpt: [9.6926 9.6962 9.6997 GiB/s]
```

</details>

<details>
<summary>hot</summary>

```
find_codes         full=    5000  standalone=    5000   thrpt: [4.0420 4.0478 4.0532 GiB/s]
find_italics       full=    5000  standalone=    5000   thrpt: [3.1363 3.1421 3.1473 GiB/s]
find_bolds         full=    5000  standalone=    5000   thrpt: [3.1716 3.1817 3.1894 GiB/s]
find_bold_italics  full=       0  standalone=       0   thrpt: [3.1776 3.1806 3.1835 GiB/s]
find_autolinks     full=    5000  standalone=    5000   thrpt: [3.5401 3.5440 3.5474 GiB/s]
find_links         full=    5000  standalone=    5000   thrpt: [3.5824 3.5856 3.5883 GiB/s]
find_headings      full=    5000  standalone=    5000   thrpt: [6.6629 6.6657 6.6681 GiB/s]
find_thematic_breaks full=     0  standalone=       0   thrpt: [6.1628 6.1677 6.1721 GiB/s]
find_fenced_codes  full=       0  standalone=       0   thrpt: [6.1414 6.1429 6.1444 GiB/s]
find_blockquotes   full=       0  standalone=       0   thrpt: [6.6334 6.6363 6.6390 GiB/s]
find_bullet_items  full=       0  standalone=       0   thrpt: [6.1071 6.1125 6.1179 GiB/s]
find_ordered_items full=       0  standalone=       0   thrpt: [5.9099 5.9121 5.9144 GiB/s]
```

</details>

<details>
<summary>heavy</summary>

```
find_codes         full=   10000  standalone=   10000   thrpt: [3.3614 3.3659 3.3706 GiB/s]
find_italics       full=   12000  standalone=   12000   thrpt: [2.3355 2.3419 2.3468 GiB/s]
find_bolds         full=   12000  standalone=   12000   thrpt: [2.3698 2.3725 2.3748 GiB/s]
find_bold_italics  full=    6000  standalone=    6000   thrpt: [2.3187 2.3217 2.3246 GiB/s]
find_autolinks     full=    4000  standalone=    4000   thrpt: [4.0837 4.0875 4.0909 GiB/s]
find_links         full=    6000  standalone=    6000   thrpt: [3.5257 3.5296 3.5333 GiB/s]
find_headings      full=    2000  standalone=    2000   thrpt: [6.1845 6.1883 6.1918 GiB/s]
find_thematic_breaks full=  2000  standalone=    2000   thrpt: [5.8192 5.8258 5.8323 GiB/s]
find_fenced_codes  full=    2000  standalone=    2000   thrpt: [5.6464 5.6498 5.6532 GiB/s]
find_blockquotes   full=    4000  standalone=    2000   thrpt: [6.1060 6.1078 6.1097 GiB/s]
find_bullet_items  full=    6000  standalone=    6000   thrpt: [5.7834 5.7884 5.7938 GiB/s]
find_ordered_items full=    4000  standalone=    4000   thrpt: [5.4213 5.4242 5.4267 GiB/s]
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
