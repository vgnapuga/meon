# meon-md — Benchmarks

EN | [**RU**](https://github.com/vgnapuga/meon/blob/main/benches/README_RU.md)

Throughput benchmarks for the [`meon-md`](https://github.com/vgnapuga/meon/blob/main/meon-md/README.md)
reference grammar, built on the [`meon`](https://github.com/vgnapuga/meon/blob/main/meon/README.md)
engine. They exist to track engine performance across changes and feature
flags — **not** to claim a ranking against other parsers (see
[Scope & fairness](#scope--fairness)).

There are two benchmark binaries:

| Bench                | Measures                                           |
|----------------------|----------------------------------------------------|
| `meon-md_parse`      | `MarkdownParser::parse` — full single-pass parse.  |
| `meon-md_standalone` | `find_*` iterators — one element kind, no context. |

Both print a **size + element-composition report** for each corpus *before*
timing, so every throughput figure can be read against the exact amount and
kind of structure the parser actually produced.

* **meon**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon/README.md)
  * [***crates.io***](https://crates.io/crates/meon)
* **meon-macros**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon-macros/README.md)
  * [***crates.io***](https://crates.io/crates/meon-macros)
* **meon-md**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon-md/README.md)
  * [***crates.io***](https://crates.io/crates/meon-md)

* [***ARCHITECTURE.md***](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE.md)
* ***BENCHMARKS.md***    <--
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
> **lower element density** and less regular patterns than `hot` or `heavy`.
> Treat the numbers as upper-bound estimates for your specific workload, not
> as expected production throughput.

---

## Running

Inside `nix develop`:

```sh
# Stable, scalar SWAR path:
cargo bench --bench meon-md_parse
cargo bench --bench meon-md_standalone

# Nightly, AVX2 SIMD path, tuned for the host CPU:
RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_parse      --features avx2
RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_standalone --features avx2
```

Criterion knobs (`SAMPLE_SIZE`, `SAMPLE_TIME`, `WARMUP_TIME`) live in
`benches/benches/docs_md.rs`. Defaults favour a quick local run; raise them for
publication-grade numbers.

---

## Scope & fairness

- **Intra-engine only.** These numbers measure *this* engine over *these*
  corpora. They are meaningful for "did my change regress?" and
  "how much does AVX2 help?", not for a leaderboard.
- **Not comparable to CommonMark parsers as-is.** `meon-md` emits flat spans
  for a Markdown *subset* and does no AST construction, reference-link
  resolution, or rendering. A fair comparison against `pulldown-cmark` /
  `comrak` would pin both to parse-only and document the feature delta.
- **End-to-end cost.** The timed region includes the internal `Vec`
  allocations, because that is what a real caller pays. Input and output are
  `black_box`-ed; document generation is outside the timed region.

---

## Known performance characteristics

**Throughput is not linear at scale.** The parser pre-allocates `Vec` capacity
as `source.len() / div`. When the accumulated output `Vec`s grow large enough
to exceed the CPU's last-level cache, allocation pressure and cache misses
become the bottleneck rather than scanning speed. This is visible in the
small → big comparison below: throughput drops ~30–40% once the working set
no longer fits in cache.

**Mitigation options** (no changes to `meon` required):

- Replace the global allocator with [`mimalloc`](https://crates.io/crates/mimalloc)
  or [`jemallocator`](https://crates.io/crates/jemallocator) in the consuming
  crate. Both are known to reduce allocation overhead at large scale.
- Tune capacity divisors `[div]` in the grammar to better match actual element
  density in your data. Tighter pre-allocation means smaller `Vec`s and less
  cache pressure.

**Throughput scales inversely with `max_nest`, independent of how deeply
content actually nests.** The block-level active-block stack and the inline
engine's bounded symmetric/asymmetric stacks are sized by `max_nest` at
compile time (`[T; max_nest]`), and these arrays are zero-initialised on
*every* call into `parse_block!`/`parse_inline!` — i.e. once per line that
reaches block processing, and once per line containing a trigger byte for
inline processing — regardless of whether that specific line actually uses
any nesting at all. A larger `max_nest` therefore costs more on every such
line, not just on lines that nest deeply.

Measured on the `small` corpora, stable build, `meon-md` rebuilt with
`max_nest = 255` instead of its normal `4`:

| Corpus  | `max_nest = 4`  | `max_nest = 255`   | Δ            |
|---------|-----------------|--------------------|--------------|
| `plain` | 2.5484 GiB/s    | 2.5758 GiB/s       | ~0% (noise)  |
| `hot`   | ~1089 MiB/s¹    | 500.60 MiB/s       | **−54%**     |
| `heavy` | 964.89 MiB/s    | 482.16 MiB/s       | **−50%**     |

¹ `1.0636 GiB/s` converted to MiB/s for direct comparison.

`plain` is unaffected because it contains no trigger bytes at all — its lines
never call `parse_inline!`, so the `max_nest`-sized arrays are never
allocated in the first place. `hot` and `heavy` lose roughly half their
throughput from `max_nest` alone, with no change to the actual content or
nesting depth used — the cost is paid purely for the *larger stack frame*,
not for any nesting that happens.

**Practical takeaway:** set `max_nest` to the smallest value your grammar
actually needs. `meon-md`'s `max_nest = 4` is already a deliberate choice,
not a default — going materially higher costs real throughput on every
inline-bearing line, whether or not anything in that line nests at all.

**AVX-512 is not benchmarked.** The `avx512` feature is implemented
(see [`swar.rs`](https://github.com/vgnapuga/meon/blob/main/meon/src/swar.rs))
but was not tested — AVX-512 hardware was not available during development.
Contributions with real numbers are welcome.

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

---

## Corpus composition

### small (REPEAT_COUNT = 10)

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

### big (REPEAT_COUNT = 1000, exceeds L3 cache)

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

## Results — parse with `max_nest = 4` (`meon-md_parse`)

### stable — `cargo bench --bench meon-md_parse`

**small (fits in cache):**

```
parse/plain/full        time:   [1.0725 ms 1.0736 ms 1.0748 ms]
                        thrpt:  [2.5456 GiB/s 2.5484 GiB/s 2.5511 GiB/s]

parse/hot/full          time:   [690.99 µs 692.26 µs 693.39 µs]
                        thrpt:  [1.0619 GiB/s 1.0636 GiB/s 1.0656 GiB/s]

parse/heavy/full        time:   [1.5211 ms 1.5231 ms 1.5253 ms]
                        thrpt:  [963.50 MiB/s 964.89 MiB/s 966.15 MiB/s]
```

**big (exceeds L3 cache — allocation pressure visible):**

```
parse/plain/full        time:   [104.00 ms 104.15 ms 104.30 ms]
                        thrpt:  [2.6231 GiB/s 2.6269 GiB/s 2.6308 GiB/s]

parse/hot/full          time:   [104.56 ms 104.75 ms 104.95 ms]
                        thrpt:  [718.43 MiB/s 719.78 MiB/s 721.11 MiB/s]

parse/heavy/full        time:   [220.94 ms 221.25 ms 221.58 ms]
                        thrpt:  [663.26 MiB/s 664.23 MiB/s 665.17 MiB/s]
```

> `plain` throughput holds near-constant across scales because the parser
> emits almost no spans (2 total) and `Vec` pressure is negligible.
> `hot` and `heavy` drop ~30–35% once span `Vec`s exceed cache — see
> [Known performance characteristics](#known-performance-characteristics).

---

### nightly — `RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_parse --features avx2`

**small (fits in cache):**

```
parse/plain/full        time:   [617.81 µs 619.98 µs 622.44 µs]
                        thrpt:  [4.3957 GiB/s 4.4131 GiB/s 4.4286 GiB/s]

parse/hot/full          time:   [554.20 µs 554.95 µs 555.65 µs]
                        thrpt:  [1.3251 GiB/s 1.3268 GiB/s 1.3286 GiB/s]

parse/heavy/full        time:   [1.2448 ms 1.2468 ms 1.2488 ms]
                        thrpt:  [1.1493 GiB/s 1.1511 GiB/s 1.1529 GiB/s]
```

**big (exceeds L3 cache):**

```
parse/plain/full        time:   [57.988 ms 58.041 ms 58.094 ms]
                        thrpt:  [4.7097 GiB/s 4.7140 GiB/s 4.7183 GiB/s]

parse/hot/full          time:   [90.720 ms 90.909 ms 91.105 ms]
                        thrpt:  [827.59 MiB/s 829.38 MiB/s 831.10 MiB/s]

parse/heavy/full        time:   [194.77 ms 195.08 ms 195.40 ms]
                        thrpt:  [752.12 MiB/s 753.35 MiB/s 754.54 MiB/s]
```

---

## Results — standalone (`meon-md_standalone`)

Each line reports `full` vs `standalone` counts. By design they can differ:
a standalone scan has no fence/escape context (see
[`ARCHITECTURE.md §12`](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE.md#12-standalone-iterators)).

### stable — `cargo bench --bench meon-md_standalone`

<details>
<summary>plain</summary>

```
find_codes         full=       0  standalone=       0
standalone/plain/find_codes
                    time:   [440.48 µs 441.06 µs 441.63 µs]
                    thrpt:  [6.1953 GiB/s 6.2034 GiB/s 6.2115 GiB/s]

find_italics       full=       0  standalone=       0
standalone/plain/find_italics
                        time:   [437.88 µs 438.45 µs 439.05 µs]
                        thrpt:  [6.2317 GiB/s 6.2402 GiB/s 6.2484 GiB/s]

find_bolds         full=       0  standalone=       0
standalone/plain/find_bolds
                        time:   [432.37 µs 432.63 µs 432.91 µs]
                        thrpt:  [6.3200 GiB/s 6.3243 GiB/s 6.3279 GiB/s]

find_bold_italics  full=       0  standalone=       0
standalone/plain/find_bold_italics
                        time:   [438.11 µs 438.88 µs 439.74 µs]
                        thrpt:  [6.2220 GiB/s 6.2341 GiB/s 6.2451 GiB/s]

find_autolinks     full=       0  standalone=       0
standalone/plain/find_autolinks
                        time:   [448.60 µs 449.13 µs 449.68 µs]
                        thrpt:  [6.0844 GiB/s 6.0918 GiB/s 6.0990 GiB/s]

find_links         full=       0  standalone=       0
standalone/plain/find_links
                        time:   [452.58 µs 453.60 µs 454.77 µs]
                        thrpt:  [6.0163 GiB/s 6.0318 GiB/s 6.0454 GiB/s]

find_headings      full=       0  standalone=       0
standalone/plain/find_headings
                        time:   [271.94 µs 273.35 µs 275.08 µs]
                        thrpt:  [9.9465 GiB/s 10.009 GiB/s 10.061 GiB/s]

find_thematic_breaks full=     0  standalone=       0
standalone/plain/find_thematic_breaks
                        time:   [291.24 µs 291.71 µs 292.13 µs]
                        thrpt:  [9.3659 GiB/s 9.3792 GiB/s 9.3943 GiB/s]

find_fenced_codes  full=       0  standalone=       0
standalone/plain/find_fenced_codes
                        time:   [279.03 µs 279.18 µs 279.34 µs]
                        thrpt:  [9.7946 GiB/s 9.8004 GiB/s 9.8056 GiB/s]

find_blockquotes   full=       0  standalone=       0
standalone/plain/find_blockquotes
                        time:   [270.56 µs 270.80 µs 271.06 µs]
                        thrpt:  [10.094 GiB/s 10.103 GiB/s 10.112 GiB/s]

find_bullet_items  full=       0  standalone=       0
standalone/plain/find_bullet_items
                        time:   [285.24 µs 285.45 µs 285.68 µs]
                        thrpt:  [9.5774 GiB/s 9.5851 GiB/s 9.5921 GiB/s]

find_ordered_items full=       0  standalone=       0
standalone/plain/find_ordered_items
                        time:   [299.77 µs 301.26 µs 302.67 µs]
                        thrpt:  [9.0396 GiB/s 9.0820 GiB/s 9.1271 GiB/s]
```

</details>

<details>
<summary>hot</summary>

```
find_codes         full=    5000  standalone=    5000
standalone/hot/find_codes
                    time:   [240.21 µs 240.54 µs 240.84 µs]
                    thrpt:  [3.0572 GiB/s 3.0611 GiB/s 3.0652 GiB/s]

find_italics       full=    5000  standalone=    5000
standalone/hot/find_italics
                        time:   [286.31 µs 286.64 µs 286.96 µs]
                        thrpt:  [2.5659 GiB/s 2.5688 GiB/s 2.5717 GiB/s]

find_bolds         full=    5000  standalone=    5000
standalone/hot/find_bolds
                        time:   [289.61 µs 290.24 µs 290.93 µs]
                        thrpt:  [2.5309 GiB/s 2.5368 GiB/s 2.5424 GiB/s]

find_bold_italics  full=       0  standalone=       0
standalone/hot/find_bold_italics
                        time:   [278.47 µs 279.03 µs 279.56 µs]
                        thrpt:  [2.6338 GiB/s 2.6388 GiB/s 2.6441 GiB/s]

find_autolinks     full=    5000  standalone=    5000
standalone/hot/find_autolinks
                        time:   [252.97 µs 253.31 µs 253.66 µs]
                        thrpt:  [2.9027 GiB/s 2.9067 GiB/s 2.9107 GiB/s]

find_links         full=    5000  standalone=    5000
standalone/hot/find_links
                        time:   [273.75 µs 274.38 µs 274.94 µs]
                        thrpt:  [2.6780 GiB/s 2.6835 GiB/s 2.6897 GiB/s]

find_headings      full=    5000  standalone=    5000
standalone/hot/find_headings
                        time:   [119.94 µs 120.07 µs 120.21 µs]
                        thrpt:  [6.1249 GiB/s 6.1322 GiB/s 6.1389 GiB/s]

find_thematic_breaks full=     0  standalone=       0
standalone/hot/find_thematic_breaks
                        time:   [126.51 µs 126.67 µs 126.84 µs]
                        thrpt:  [5.8048 GiB/s 5.8129 GiB/s 5.8202 GiB/s]

find_fenced_codes  full=       0  standalone=       0
standalone/hot/find_fenced_codes
                        time:   [124.75 µs 124.80 µs 124.87 µs]
                        thrpt:  [5.8966 GiB/s 5.8998 GiB/s 5.9022 GiB/s]

find_blockquotes   full=       0  standalone=       0
standalone/hot/find_blockquotes
                        time:   [116.87 µs 116.93 µs 117.00 µs]
                        thrpt:  [6.2932 GiB/s 6.2969 GiB/s 6.3004 GiB/s]

find_bullet_items  full=       0  standalone=       0
standalone/hot/find_bullet_items
                        time:   [126.96 µs 127.05 µs 127.14 µs]
                        thrpt:  [5.7912 GiB/s 5.7955 GiB/s 5.7995 GiB/s]

find_ordered_items full=       0  standalone=       0
standalone/hot/find_ordered_items
                        time:   [130.92 µs 131.15 µs 131.41 µs]
                        thrpt:  [5.6031 GiB/s 5.6141 GiB/s 5.6242 GiB/s]
```

</details>

<details>
<summary>heavy</summary>

```
find_codes         full=   10000  standalone=   10000
standalone/heavy/find_codes
                        time:   [566.26 µs 568.25 µs 570.19 µs]
                        thrpt:  [2.5170 GiB/s 2.5256 GiB/s 2.5345 GiB/s]

find_italics       full=   12000  standalone=    12000
standalone/heavy/find_italics
                        time:   [784.27 µs 786.19 µs 788.19 µs]
                        thrpt:  [1.8209 GiB/s 1.8255 GiB/s 1.8300 GiB/s]

find_bolds         full=   12000  standalone=   12000
standalone/heavy/find_bolds
                        time:   [781.20 µs 781.93 µs 782.65 µs]
                        thrpt:  [1.8337 GiB/s 1.8354 GiB/s 1.8372 GiB/s]

find_bold_italics  full=    6000  standalone=    6000
standalone/heavy/find_bold_italics
                        time:   [772.55 µs 774.96 µs 777.47 µs]
                        thrpt:  [1.8460 GiB/s 1.8520 GiB/s 1.8577 GiB/s]

find_autolinks     full=    4000  standalone=    4000
standalone/heavy/find_autolinks
                        time:   [454.43 µs 455.19 µs 455.97 µs]
                        thrpt:  [3.1476 GiB/s 3.1529 GiB/s 3.1582 GiB/s]

find_links         full=    6000  standalone=    6000
standalone/heavy/find_links
                        time:   [532.53 µs 533.12 µs 533.66 µs]
                        thrpt:  [2.6893 GiB/s 2.6921 GiB/s 2.6951 GiB/s]

find_headings      full=    2000  standalone=    2000
standalone/heavy/find_headings
                        time:   [252.99 µs 253.47 µs 253.97 µs]
                        thrpt:  [5.6510 GiB/s 5.6622 GiB/s 5.6729 GiB/s]

find_thematic_breaks full=  2000  standalone=    2000
standalone/heavy/find_thematic_breaks
                        time:   [265.58 µs 265.88 µs 266.21 µs]
                        thrpt:  [5.3913 GiB/s 5.3978 GiB/s 5.4039 GiB/s]

find_fenced_codes  full=    2000  standalone=    2000
standalone/heavy/find_fenced_codes
                        time:   [275.79 µs 276.01 µs 276.25 µs]
                        thrpt:  [5.1953 GiB/s 5.1998 GiB/s 5.2038 GiB/s]

find_blockquotes   full=    4000  standalone=    2000
standalone/heavy/find_blockquotes
                        time:   [260.41 µs 260.68 µs 260.92 µs]
                        thrpt:  [5.5004 GiB/s 5.5056 GiB/s 5.5112 GiB/s]

find_bullet_items  full=    6000  standalone=    6000
standalone/heavy/find_bullet_items
                        time:   [265.47 µs 265.69 µs 265.95 µs]
                        thrpt:  [5.3964 GiB/s 5.4017 GiB/s 5.4063 GiB/s]

find_ordered_items full=    4000  standalone=    4000
standalone/heavy/find_ordered_items
                        time:   [290.72 µs 290.95 µs 291.18 µs]
                        thrpt:  [4.9289 GiB/s 4.9328 GiB/s 4.9367 GiB/s]
```

</details>

---

### nightly — `RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_standalone --features avx2`

<details>
<summary>plain</summary>

```
find_codes         full=       0  standalone=       0
standalone/plain/find_codes
                    time:   [333.56 µs 333.94 µs 334.37 µs]
                    thrpt:  [8.1828 GiB/s 8.1932 GiB/s 8.2026 GiB/s]

find_italics       full=       0  standalone=       0
standalone/plain/find_italics
                        time:   [335.99 µs 336.61 µs 337.31 µs]
                        thrpt:  [8.1114 GiB/s 8.1282 GiB/s 8.1432 GiB/s]

find_bolds         full=       0  standalone=       0
standalone/plain/find_bolds
                        time:   [335.29 µs 335.80 µs 336.35 µs]
                        thrpt:  [8.1344 GiB/s 8.1478 GiB/s 8.1602 GiB/s]

find_bold_italics  full=       0  standalone=       0
standalone/plain/find_bold_italics
                        time:   [339.67 µs 339.93 µs 340.18 µs]
                        thrpt:  [8.0428 GiB/s 8.0487 GiB/s 8.0549 GiB/s]

find_autolinks     full=       0  standalone=       0
standalone/plain/find_autolinks
                        time:   [327.01 µs 327.67 µs 328.40 µs]
                        thrpt:  [8.3315 GiB/s 8.3499 GiB/s 8.3668 GiB/s]

find_links         full=       0  standalone=       0
standalone/plain/find_links
                        time:   [320.73 µs 321.32 µs 321.96 µs]
                        thrpt:  [8.4980 GiB/s 8.5150 GiB/s 8.5306 GiB/s]

find_headings      full=       0  standalone=       0
standalone/plain/find_headings
                        time:   [258.66 µs 258.75 µs 258.85 µs]
                        thrpt:  [10.570 GiB/s 10.574 GiB/s 10.578 GiB/s]

find_thematic_breaks full=       0  standalone=       0
standalone/plain/find_thematic_breaks
                        time:   [268.52 µs 269.30 µs 270.07 µs]
                        thrpt:  [10.131 GiB/s 10.160 GiB/s 10.189 GiB/s]

find_fenced_codes  full=       0  standalone=       0
standalone/plain/find_fenced_codes
                        time:   [269.89 µs 270.01 µs 270.13 µs]
                        thrpt:  [10.129 GiB/s 10.133 GiB/s 10.138 GiB/s]

find_blockquotes   full=       0  standalone=       0
standalone/plain/find_blockquotes
                        time:   [244.94 µs 245.19 µs 245.46 µs]
                        thrpt:  [11.147 GiB/s 11.159 GiB/s 11.170 GiB/s]

find_bullet_items  full=       0  standalone=       0
standalone/plain/find_bullet_items
                        time:   [265.82 µs 265.94 µs 266.06 µs]
                        thrpt:  [10.283 GiB/s 10.288 GiB/s 10.293 GiB/s]

find_ordered_items full=       0  standalone=       0
standalone/plain/find_ordered_items
                        time:   [282.07 µs 282.18 µs 282.28 µs]
                        thrpt:  [9.6926 GiB/s 9.6962 GiB/s 9.6997 GiB/s]
```

</details>

<details>
<summary>hot</summary>

```
find_codes         full=    5000  standalone=    5000
standalone/hot/find_codes
                    time:   [181.66 µs 181.90 µs 182.16 µs]
                    thrpt:  [4.0420 GiB/s 4.0478 GiB/s 4.0532 GiB/s]

find_italics       full=    5000  standalone=    5000
standalone/hot/find_italics
                        time:   [233.95 µs 234.34 µs 234.77 µs]
                        thrpt:  [3.1363 GiB/s 3.1421 GiB/s 3.1473 GiB/s]

find_bolds         full=    5000  standalone=    5000
standalone/hot/find_bolds
                        time:   [230.86 µs 231.41 µs 232.16 µs]
                        thrpt:  [3.1716 GiB/s 3.1817 GiB/s 3.1894 GiB/s]

find_bold_italics  full=       0  standalone=       0
standalone/hot/find_bold_italics
                        time:   [231.28 µs 231.50 µs 231.72 µs]
                        thrpt:  [3.1776 GiB/s 3.1806 GiB/s 3.1835 GiB/s]

find_autolinks     full=    5000  standalone=    5000
standalone/hot/find_autolinks
                        time:   [207.56 µs 207.76 µs 207.99 µs]
                        thrpt:  [3.5401 GiB/s 3.5440 GiB/s 3.5474 GiB/s]

find_links         full=    5000  standalone=    5000
standalone/hot/find_links
                        time:   [205.19 µs 205.35 µs 205.54 µs]
                        thrpt:  [3.5824 GiB/s 3.5856 GiB/s 3.5883 GiB/s]

find_headings      full=    5000  standalone=    5000
standalone/hot/find_headings
                        time:   [110.42 µs 110.46 µs 110.51 µs]
                        thrpt:  [6.6629 GiB/s 6.6657 GiB/s 6.6681 GiB/s]

find_thematic_breaks full=       0  standalone=       0
standalone/hot/find_thematic_breaks
                        time:   [119.30 µs 119.38 µs 119.48 µs]
                        thrpt:  [6.1628 GiB/s 6.1677 GiB/s 6.1721 GiB/s]

find_fenced_codes  full=       0  standalone=       0
standalone/hot/find_fenced_codes
                        time:   [119.83 µs 119.86 µs 119.89 µs]
                        thrpt:  [6.1414 GiB/s 6.1429 GiB/s 6.1444 GiB/s]

find_blockquotes   full=       0  standalone=       0
standalone/hot/find_blockquotes
                        time:   [110.91 µs 110.95 µs 111.00 µs]
                        thrpt:  [6.6334 GiB/s 6.6363 GiB/s 6.6390 GiB/s]

find_bullet_items  full=       0  standalone=       0
standalone/hot/find_bullet_items
                        time:   [120.35 µs 120.46 µs 120.57 µs]
                        thrpt:  [6.1071 GiB/s 6.1125 GiB/s 6.1179 GiB/s]

find_ordered_items full=       0  standalone=       0
standalone/hot/find_ordered_items
                        time:   [124.49 µs 124.54 µs 124.59 µs]
                        thrpt:  [5.9099 GiB/s 5.9121 GiB/s 5.9144 GiB/s]
```

</details>

<details>
<summary>heavy</summary>

```
find_codes         full=   10000  standalone=   10000
standalone/heavy/find_codes
                    time:   [425.79 µs 426.39 µs 426.96 µs]
                    thrpt:  [3.3614 GiB/s 3.3659 GiB/s 3.3706 GiB/s]

find_italics       full=   12000  standalone=   12000
standalone/heavy/find_italics
                        time:   [611.55 µs 612.84 µs 614.52 µs]
                        thrpt:  [2.3355 GiB/s 2.3419 GiB/s 2.3468 GiB/s]

find_bolds         full=   12000  standalone=   12000
standalone/heavy/find_bolds
                        time:   [604.34 µs 604.93 µs 605.61 µs]
                        thrpt:  [2.3698 GiB/s 2.3725 GiB/s 2.3748 GiB/s]

find_bold_italics  full=    6000  standalone=    6000
standalone/heavy/find_bold_italics
                        time:   [617.40 µs 618.15 µs 618.96 µs]
                        thrpt:  [2.3187 GiB/s 2.3217 GiB/s 2.3246 GiB/s]

find_autolinks     full=    4000  standalone=    4000
standalone/heavy/find_autolinks
                        time:   [350.83 µs 351.12 µs 351.44 µs]
                        thrpt:  [4.0837 GiB/s 4.0875 GiB/s 4.0909 GiB/s]

find_links         full=    6000  standalone=    6000
standalone/heavy/find_links
                        time:   [406.19 µs 406.62 µs 407.06 µs]
                        thrpt:  [3.5257 GiB/s 3.5296 GiB/s 3.5333 GiB/s]

find_headings      full=    2000  standalone=    2000
standalone/heavy/find_headings
                        time:   [231.79 µs 231.92 µs 232.06 µs]
                        thrpt:  [6.1845 GiB/s 6.1883 GiB/s 6.1918 GiB/s]

find_thematic_breaks full=    2000  standalone=    2000
standalone/heavy/find_thematic_breaks
                        time:   [246.08 µs 246.35 µs 246.63 µs]
                        thrpt:  [5.8192 GiB/s 5.8258 GiB/s 5.8323 GiB/s]

find_fenced_codes  full=    2000  standalone=    2000
standalone/heavy/find_fenced_codes
                        time:   [253.87 µs 254.03 µs 254.18 µs]
                        thrpt:  [5.6464 GiB/s 5.6498 GiB/s 5.6532 GiB/s]

find_blockquotes   full=    4000  standalone=    2000
standalone/heavy/find_blockquotes
                        time:   [234.90 µs 234.98 µs 235.05 µs]
                        thrpt:  [6.1060 GiB/s 6.1078 GiB/s 6.1097 GiB/s]

find_bullet_items  full=    6000  standalone=    6000
standalone/heavy/find_bullet_items
                        time:   [247.71 µs 247.94 µs 248.16 µs]
                        thrpt:  [5.7834 GiB/s 5.7884 GiB/s 5.7938 GiB/s]

find_ordered_items full=    4000  standalone=    4000
standalone/heavy/find_ordered_items
                        time:   [264.47 µs 264.59 µs 264.73 µs]
                        thrpt:  [5.4213 GiB/s 5.4242 GiB/s 5.4267 GiB/s]
```

</details>

---

## Reading the numbers

- `thrpt` (GiB/s) is the headline; it already accounts for corpus size.
- Compare a number only against the *same corpus* on a *different build*
  (scalar vs AVX2), or against a previous commit on the same machine.
- `plain` is fastest (least work); `heavy` slowest (most elements emitted).
  The composition header explains *why*.
- `plain` throughput is stable across small/big because it emits almost no
  spans. `hot`/`heavy` drop ~30–35% at large scale due to `Vec` pressure —
  see [Known performance characteristics](#known-performance-characteristics).
- Criterion writes HTML reports to `target/criterion/`; the `change:` block
  appears automatically on a second run and is the real regression signal.
