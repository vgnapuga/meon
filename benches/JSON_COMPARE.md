# meon-json — Cross-parser comparison

EN | [**RU**](https://github.com/vgnapuga/meon/blob/main/benches/JSON_COMPARE_RU.md)

Throughput of [`meon-json`](https://github.com/vgnapuga/meon/blob/main/meon-json/README.md)
(built on the [`meon`](https://github.com/vgnapuga/meon/blob/main/meon/README.md)
engine) next to two validating JSON parsers, on the same corpora as the
intra-engine benches.

> **These numbers demonstrate an architectural difference, not a quality
> ranking.** `meon-json` is, by design, a **structural reader**: it parses JSON
> into a flat span table and validates nothing, parses no numbers, and unescapes
> no strings. `simd-json` and `sonic-rs` are validating parsers that materialise
> a tape / an owned value — they parse every number and unescape every string.
> A throughput gap therefore reflects two different jobs. `Throughput::Bytes`
> measures how fast the input is consumed, since the four produce different
> things.

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
  * [***MD_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/MD_COMPARE.md)
  * ***JSON_COMPARE.md***    <--
* [***FUZZING.md***](https://github.com/vgnapuga/meon/blob/main/fuzz/README.md)

---

## What is measured

One binary, `meon-json_compare`. Per corpus (`numbers` / `objects` / `nested`),
four parsers over identical input, each `black_box`-ed:

| Line              | Call                                | What it does                                                                  |
|-------------------|-------------------------------------|-------------------------------------------------------------------------------|
| `meon-structural` | `JsonParser::parse`                 | Flat span table. No validation, no number parsing, no string unescaping.      |
| `meon-typed`      | `parse` + `type_scalars`            | + first-byte scalar classification. Still no number-value parse, no unescape. |
| `simd-json`       | `simd_json::to_tape`                | Stage 1 + Stage 2 in one pass: structural + number parse/validate + unescape. |
| `sonic-rs`        | `sonic_rs::from_slice::<Value>`     | Full parse into an owned `Value` (validates, parses numbers, unescapes).      |

`simd-json` mutates its input buffer in place (string unescaping), so it is
handed a fresh clone per iteration; that clone is `iter_batched` setup and is
**not** timed. The other three read the original immutable bytes.

The same per-corpus composition report (structural + typed counts) as the
intra-engine benches is printed before timing.

---

## Why these numbers are demonstrative, not a ranking

- **Different amounts of work.** `meon-structural` emits spans and materialises
  nothing. `meon-typed` adds only first-byte classification. `simd-json` and
  `sonic-rs` validate, parse every number to a value, and unescape every string.
  meon does less, so it reads faster. A like-for-like line would also parse
  numbers to values and unescape strings — **neither meon line does that**, so
  even `meon-typed` is not equivalent work to a tape or an owned value.

- **Reader, not validator — deliberate.** `meon-json` does not reject invalid
  JSON; it reports the structure it saw. `simd-json` and `sonic-rs` validate and
  error on malformed input. The comparison is not like-for-like on guarantees.

- **`meon-typed` is first-byte classification, not number validation.** It
  routes a scalar by its first byte (`1abc` types as a number); it never checks
  the rest of the run, parses a numeric value, or decodes a string.

- **Build-flag / SIMD parity.** meon uses AVX2 only under `--features avx2` +
  `RUSTFLAGS="-C target-cpu=native"`; on stable it runs the scalar SWAR path.
  `simd-json` and `sonic-rs` do their own runtime SIMD detection and use it on
  capable hardware regardless of meon's flag. A scalar-meon row next to the SIMD
  comparators is not a like-for-like SIMD comparison — each results block states
  the meon build it was taken under.

- **Output shapes differ.** SoA spans vs a tape vs an owned `Value`.
  `Throughput::Bytes` normalises by input size — it answers "how fast is the
  input consumed", since the four produce different things.

- **End-to-end cost, and a hidden meon edge.** Timed regions include each
  parser's own allocations (meon's `Vec`s, the simd-json tape, the sonic-rs
  value). The clone `simd-json` needs (to preserve the original, since it
  unescapes in place) is excluded from timing — so meon's zero-copy,
  non-mutating read is not credited here. If your use case must keep the
  original bytes, add that clone to `simd-json`'s cost.

- **Corpus bias.** The corpora are synthetic. The `numbers` corpus maximises the
  gap — meon never parses a number while the validating parsers parse and
  validate every one — so read each corpus on its own terms, not as one headline.

---

## Running

Inside `nix develop`:

```sh
# Stable, meon scalar SWAR path (simd-json / sonic-rs use runtime SIMD):
cargo bench --bench meon-json_compare

# Nightly, meon AVX2 path tuned for the host CPU:
RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-json_compare --features avx2
```

`simd-json` and `sonic-rs` detect and use SIMD at runtime; no Cargo feature is
needed for them. Only meon's AVX2 path is gated behind `--features avx2`.

Hardware and Criterion knobs are shared with the intra-engine benches — see
*Test hardware* in
[***BENCHMARKS.md***](https://github.com/vgnapuga/meon/blob/main/benches/README.md)
and the knobs in `benches/benches/docs_json.rs`.

---

## Corpora

Each corpus is one valid top-level JSON array, scaled by `COUNT`
(`benches/benches/docs_json.rs`). The `small` and `big` runs differ only in
`COUNT`.

| Corpus    | Shape                                                                                     | Stresses                                                                 |
|-----------|-------------------------------------------------------------------------------------------|--------------------------------------------------------------------------|
| `numbers` | Flat array of numbers / bools / nulls.                                                    | Scalar scanning. meon does least; validating parsers parse every number. |
| `objects` | Array of flat objects with mixed-typed fields (`id`/`name`/`active`/...).                 | Members, keys, typed scalars. A typical API payload.                     |
| `nested`  | Array of moderately nested objects (objects-in-objects, small arrays, an escaped string). | The unified nesting stack and the string rule.                           |

> **Synthetic data notice.** All three corpora are generated programmatically
> with uniform, predictable structure. Real-world JSON typically has less
> regular structure than these — treat the figures as a demonstration of the
> architectural difference, not as expected production throughput.

### Corpus composition

**small:**

```
┌─ corpus: numbers
│  size:            1.90 MiB  (1989441 bytes)
│  structural:         1     (0.0 per KiB)
│
│      objects:         0      arrays:         1     strings:         0
│      members:         0     scalars:         0       loose:         1
│  typed: nums:    150000       trues:     50000      falses:     50000     nulls:     50000
└─
┌─ corpus: objects
│  size:            1.39 MiB  (1456671 bytes)
│  structural:    240001     (168.7 per KiB)
│
│      objects:     20000      arrays:         1     strings:    120000
│      members:    100000     scalars:         0       loose:         1
│  typed: nums:     40000       trues:     10000      falses:     10000     nulls:     20000
└─
┌─ corpus: nested
│  size:            1.13 MiB  (1184451 bytes)
│  structural:    290001     (250.7 per KiB)
│
│      objects:     50000      arrays:     10001     strings:    130000
│      members:    100000     scalars:         0       loose:         1
│  typed: nums:     40000       trues:     10000      falses:         0     nulls:         0
└─
```

**big:**

```
┌─ corpus: numbers
│  size:          218.34 MiB  (228944439 bytes)
│  structural:         1     (0.0 per KiB)
│
│      objects:         0      arrays:         1     strings:         0
│      members:         0     scalars:         0       loose:         1
│  typed: nums:  15000000       trues:   5000000      falses:   5000000     nulls:   5000000
└─
┌─ corpus: objects
│  size:          150.36 MiB  (157666671 bytes)
│  structural:  24000001     (155.9 per KiB)
│
│      objects:   2000000      arrays:         1     strings:  12000000
│      members:  10000000     scalars:         0       loose:         1
│  typed: nums:   4000000       trues:   1000000      falses:   1000000     nulls:   2000000
└─
┌─ corpus: nested
│  size:          122.49 MiB  (128444451 bytes)
│  structural:  29000001     (231.2 per KiB)
│
│      objects:   5000000      arrays:   1000001     strings:  13000000
│      members:  10000000     scalars:         0       loose:         1
│  typed: nums:   4000000       trues:   1000000      falses:         0     nulls:         0
└─
```

---

## Results

> Throughput (`thrpt`) is the headline. Compare a cell only against the same
> corpus in the same build block. Each cell is the Criterion `time` / `thrpt`
> triple (low / median / high).

### stable - `cargo bench --bench meon-json_compare`

**small:**

| Corpus    | `meon-structural` | `meon-typed` | `simd-json` | `sonic-rs` |
|-----------|-------------------|--------------|-------------|------------|
| `numbers` | time:   [2.3261 ms 2.3282 ms 2.3303 ms] thrpt:  [814.18 MiB/s 814.92 MiB/s 815.64 MiB/s] | time:   [4.8090 ms 4.8169 ms 4.8248 ms] thrpt:  [393.24 MiB/s 393.88 MiB/s 394.52 MiB/s] | time:   [8.1397 ms 8.1854 ms 8.2325 ms] thrpt:  [230.46 MiB/s 231.79 MiB/s 233.09 MiB/s] | time:   [2.7498 ms 2.7667 ms 2.7842 ms] thrpt:  [681.44 MiB/s 685.75 MiB/s 689.98 MiB/s] |
| `objects` | time:   [3.8821 ms 3.8903 ms 3.8987 ms] thrpt:  [356.33 MiB/s 357.09 MiB/s 357.85 MiB/s] | time:   [5.1094 ms 5.1192 ms 5.1290 ms] thrpt:  [270.85 MiB/s 271.37 MiB/s 271.89 MiB/s] | time:   [1.9563 ms 1.9619 ms 1.9681 ms] thrpt:  [705.84 MiB/s 708.09 MiB/s 710.11 MiB/s] | time:   [1.7473 ms 1.7530 ms 1.7607 ms] thrpt:  [788.99 MiB/s 792.45 MiB/s 795.03 MiB/s] |
| `nested`  | time:   [4.6466 ms 4.6579 ms 4.6714 ms] thrpt:  [241.81 MiB/s 242.51 MiB/s 243.10 MiB/s] | time:   [5.6025 ms 5.6145 ms 5.6275 ms] thrpt:  [200.72 MiB/s 201.19 MiB/s 201.62 MiB/s] | time:   [2.1602 ms 2.1631 ms 2.1669 ms] thrpt:  [521.29 MiB/s 522.21 MiB/s 522.90 MiB/s] | time:   [2.2674 ms 2.2686 ms 2.2698 ms] thrpt:  [497.65 MiB/s 497.92 MiB/s 498.18 MiB/s] |

**big:**

| Corpus    | `meon-structural` | `meon-typed` | `simd-json` | `sonic-rs` |
|-----------|-------------------|--------------|-------------|------------|
| `numbers` | time:   [247.45 ms 247.83 ms 248.17 ms] thrpt:  [879.81 MiB/s 881.00 MiB/s 882.34 MiB/s] | time:   [621.91 ms 624.78 ms 627.49 ms] thrpt:  [347.96 MiB/s 349.47 MiB/s 351.08 MiB/s] | time:   [936.42 ms 937.71 ms 938.87 ms] thrpt:  [232.55 MiB/s 232.84 MiB/s 233.16 MiB/s] | time:   [896.16 ms 897.18 ms 898.18 ms] thrpt:  [243.09 MiB/s 243.36 MiB/s 243.64 MiB/s] |
| `objects` | time:   [522.66 ms 523.26 ms 523.87 ms] thrpt:  [287.02 MiB/s 287.36 MiB/s 287.69 MiB/s] | time:   [698.37 ms 699.36 ms 700.64 ms] thrpt:  [214.61 MiB/s 215.00 MiB/s 215.31 MiB/s] | time:   [680.63 ms 681.85 ms 683.01 ms] thrpt:  [220.15 MiB/s 220.52 MiB/s 220.92 MiB/s] | time:   [465.96 ms 466.57 ms 467.18 ms] thrpt:  [321.85 MiB/s 322.27 MiB/s 322.69 MiB/s] |
| `nested`  | time:   [631.63 ms 638.39 ms 644.40 ms] thrpt:  [190.09 MiB/s 191.88 MiB/s 193.93 MiB/s] | time:   [751.56 ms 755.80 ms 760.03 ms] thrpt:  [161.17 MiB/s 162.07 MiB/s 162.99 MiB/s] | time:   [711.34 ms 713.21 ms 715.77 ms] thrpt:  [171.14 MiB/s 171.75 MiB/s 172.20 MiB/s] | time:   [538.68 ms 539.50 ms 540.27 ms] thrpt:  [226.73 MiB/s 227.05 MiB/s 227.40 MiB/s] |

### nightly - `RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-json_compare --features avx2`

> meon on AVX2; `simd-json` / `sonic-rs` on their own runtime SIMD.

**small:**

| Corpus    | `meon-structural` | `meon-typed` | `simd-json` | `sonic-rs` |
|-----------|-------------------|--------------|-------------|------------|
| `numbers` | time:   [2.0152 ms 2.0161 ms 2.0171 ms] thrpt:  [940.58 MiB/s 941.05 MiB/s 941.49 MiB/s] | time:   [4.2121 ms 4.2186 ms 4.2256 ms] thrpt:  [449.00 MiB/s 449.74 MiB/s 450.43 MiB/s] | time:   [7.7677 ms 7.7922 ms 7.8179 ms] thrpt:  [242.68 MiB/s 243.48 MiB/s 244.25 MiB/s] | time:   [2.6897 ms 2.7041 ms 2.7197 ms] thrpt:  [697.61 MiB/s 701.62 MiB/s 705.39 MiB/s] |
| `objects` | time:   [3.7105 ms 3.7142 ms 3.7182 ms] thrpt:  [373.62 MiB/s 374.02 MiB/s 374.40 MiB/s] | time:   [5.2340 ms 5.2390 ms 5.2442 ms] thrpt:  [264.90 MiB/s 265.16 MiB/s 265.42 MiB/s] | time:   [1.8550 ms 1.8619 ms 1.8694 ms] thrpt:  [743.14 MiB/s 746.12 MiB/s 748.91 MiB/s] | time:   [1.6545 ms 1.6566 ms 1.6590 ms] thrpt:  [837.36 MiB/s 838.57 MiB/s 839.63 MiB/s] |
| `nested`  | time:   [4.3796 ms 4.3852 ms 4.3908 ms] thrpt:  [257.26 MiB/s 257.59 MiB/s 257.92 MiB/s] | time:   [5.6681 ms 5.6761 ms 5.6847 ms] thrpt:  [198.71 MiB/s 199.01 MiB/s 199.29 MiB/s] | time:   [2.1097 ms 2.1134 ms 2.1176 ms] thrpt:  [533.42 MiB/s 534.49 MiB/s 535.42 MiB/s] | time:   [2.1642 ms 2.1662 ms 2.1682 ms] thrpt:  [520.97 MiB/s 521.46 MiB/s 521.93 MiB/s] |

**big:**

| Corpus    | `meon-structural` | `meon-typed` | `simd-json` | `sonic-rs` |
|-----------|-------------------|--------------|-------------|------------|
| `numbers` | time:   [208.97 ms 209.28 ms 209.85 ms] thrpt:  [1.0161 GiB/s 1.0188 GiB/s 1.0204 GiB/s] | time:   [565.28 ms 567.39 ms 569.32 ms] thrpt:  [383.51 MiB/s 384.81 MiB/s 386.25 MiB/s] | time:   [909.24 ms 912.25 ms 915.57 ms] thrpt:  [238.47 MiB/s 239.34 MiB/s 240.13 MiB/s] | time:   [910.24 ms 915.12 ms 919.94 ms] thrpt:  [237.34 MiB/s 238.59 MiB/s 239.87 MiB/s] |
| `objects` | time:   [504.00 ms 504.50 ms 505.08 ms] thrpt:  [297.70 MiB/s 298.04 MiB/s 298.34 MiB/s] | time:   [714.67 ms 719.04 ms 723.33 ms] thrpt:  [207.88 MiB/s 209.12 MiB/s 210.39 MiB/s] | time:   [671.77 ms 672.50 ms 673.24 ms] thrpt:  [223.34 MiB/s 223.59 MiB/s 223.83 MiB/s] | time:   [460.27 ms 461.38 ms 462.47 ms] thrpt:  [325.13 MiB/s 325.90 MiB/s 326.68 MiB/s] |
| `nested`  | time:   [604.64 ms 606.10 ms 607.43 ms] thrpt:  [201.66 MiB/s 202.10 MiB/s 202.59 MiB/s] | time:   [766.82 ms 768.29 ms 769.73 ms] thrpt:  [159.14 MiB/s 159.44 MiB/s 159.74 MiB/s] | time:   [708.05 ms 712.20 ms 716.15 ms] thrpt:  [171.05 MiB/s 172.00 MiB/s 173.00 MiB/s] | time:   [529.42 ms 530.75 ms 532.59 ms] thrpt:  [230.00 MiB/s 230.79 MiB/s 231.37 MiB/s] |

---

## Scaling from small to big

How each parser holds up as the input grows past cache (stable build, median
`thrpt`, MiB/s):

| Parser            | `numbers`     | `objects`     | `nested`      |
|-------------------|---------------|---------------|---------------|
| `meon-structural` | 815 -> 881    | 357 -> 287    | 243 -> 192    |
| `meon-typed`      | 394 -> 349    | 271 -> 215    | 201 -> 162    |
| `simd-json`       | 232 -> 233    | 708 -> 221    | 522 -> 172    |
| `sonic-rs`        | 686 -> 243    | 792 -> 322    | 498 -> 227    |

- **meon degrades little with scale.** `meon-structural` even gains on `numbers`
  (815 -> 881) and loses only ~20% on `objects`/`nested`; `meon-typed` tracks it.
  The flat span table stays largely cache-resident.
- **The validating parsers collapse on structured corpora at big.** `simd-json`
  loses ~69% on `objects` and ~67% on `nested`; `sonic-rs` loses ~55–65% across
  the board — materialising a tape / owned `Value`, their working set blows
  cache as the document grows. (`simd-json` holds on `numbers`, where its tape
  stays compact; `sonic-rs` drops there too.)
- **Rankings flip with scale.** At small the validating parsers lead
  `objects`/`nested` by 2–3x; at big that lead is gone or inverted — e.g. on
  `objects`, `meon-structural` overtakes `simd-json` (287 vs 221), and on
  `numbers` meon leads at every scale and widens at big. A flat span table
  degrades far less than a materialised tape or owned value. The AVX2 run shows
  the same pattern.

---

## meon-json standalone extraction (no comparator equivalent)

`find_*` scans the raw source for **one** element kind only — e.g. every string
— with no cross-element context. `simd-json` and `sonic-rs` have no equivalent:
pulling just the strings from them means materialising the whole tape / owned
`Value` first. The numbers below are meon-only; they are here because
single-kind extraction is part of the architecture difference this document is
about.

Each line reports `full` vs `standalone` counts. For JSON they diverge more than
for a flat format, because `find_*` is **nesting-insensitive**: `find_objects`
matches only the literal `{` delimiter and does not track depth, so on `nested`
it sees the 2M / 20k top-level objects rather than the 5M / 50k a full parse
resolves, and `find_members` under-counts the same way. `find_strings` is exact
(string content has no nesting), which is why it is the recommended single-sweep
use. This is the documented trade-off — reach for `find_*` only for a
nesting-insensitive sweep; use the full `parse` when you need correct
containment (see
[`ARCHITECTURE.md §12`](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE.md#12-standalone-iterators)).
Shown for both `small` and `big`.

### stable - `cargo bench --bench meon-json_standalone`

**small:**

<details>
<summary>numbers</summary>

```
  find_objects   full=        0  standalone=        0
                        time:   [40.710 µs 41.302 µs 41.911 µs]
                        thrpt:  [44.208 GiB/s 44.860 GiB/s 45.512 GiB/s]
                        
  find_arrays    full=        1  standalone=        1
                        time:   [42.002 µs 42.610 µs 43.206 µs]
                        thrpt:  [42.883 GiB/s 43.483 GiB/s 44.113 GiB/s]
                          
  find_strings   full=        0  standalone=        0
                        time:   [39.991 µs 40.544 µs 41.095 µs]
                        thrpt:  [45.087 GiB/s 45.699 GiB/s 46.331 GiB/s]
                          
  find_members   full=        0  standalone=        0
                        time:   [39.493 µs 40.121 µs 40.807 µs]
                        thrpt:  [45.404 GiB/s 46.180 GiB/s 46.915 GiB/s]
```

</details>

<details>
<summary>objects</summary>

```
  find_objects   full=    20000  standalone=    20000
                        time:   [300.02 µs 300.09 µs 300.15 µs]
                        thrpt:  [4.5198 GiB/s 4.5208 GiB/s 4.5217 GiB/s]

  find_arrays    full=        1  standalone=        1
                        time:   [29.547 µs 29.951 µs 30.349 µs]
                        thrpt:  [44.701 GiB/s 45.296 GiB/s 45.915 GiB/s]
                        
  find_strings   full=   120000  standalone=   120000
                        time:   [1.5835 ms 1.5840 ms 1.5847 ms]
                        thrpt:  [876.61 MiB/s 876.99 MiB/s 877.29 MiB/s]
                        
  find_members   full=   100000  standalone=   100000
                        time:   [1.6096 ms 1.6099 ms 1.6101 ms]
                        thrpt:  [862.79 MiB/s 862.93 MiB/s 863.05 MiB/s]
```

</details>

<details>
<summary>nested</summary>

```
  find_objects   full=    50000  standalone=    20000
                        time:   [281.29 µs 281.49 µs 281.70 µs]
                        thrpt:  [3.9159 GiB/s 3.9188 GiB/s 3.9216 GiB/s]
                        
  find_arrays    full=    10001  standalone=    10000
                        time:   [152.18 µs 152.88 µs 153.73 µs]
                        thrpt:  [7.1756 GiB/s 7.2157 GiB/s 7.2488 GiB/s]
                        
  find_strings   full=   130000  standalone=   130000
                        time:   [1.7137 ms 1.7147 ms 1.7158 ms]
                        thrpt:  [658.32 MiB/s 658.76 MiB/s 659.15 MiB/s]

  find_members   full=   100000  standalone=    60000
                        time:   [987.10 µs 987.18 µs 987.27 µs]
                        thrpt:  [1.1173 GiB/s 1.1174 GiB/s 1.1175 GiB/s]
```

</details>

**big:**

<details>
<summary>numbers</summary>

```
  find_objects   full=        0  standalone=        0
                        time:   [17.893 ms 17.931 ms 17.983 ms]
                        thrpt:  [11.857 GiB/s 11.891 GiB/s 11.916 GiB/s]

  find_arrays    full=        1  standalone=        1
                        time:   [19.018 ms 19.261 ms 19.465 ms]
                        thrpt:  [10.954 GiB/s 11.070 GiB/s 11.211 GiB/s]

  find_strings   full=        0  standalone=        0
                        time:   [18.032 ms 18.253 ms 18.491 ms]
                        thrpt:  [11.531 GiB/s 11.681 GiB/s 11.824 GiB/s]

  find_members   full=        0  standalone=        0
                        time:   [17.792 ms 17.819 ms 17.856 ms]
                        thrpt:  [11.941 GiB/s 11.966 GiB/s 11.984 GiB/s]
```

</details>

<details>
<summary>objects</summary>

```
  find_objects   full=  2000000  standalone=  2000000
                        time:   [35.591 ms 35.685 ms 35.755 ms]
                        thrpt:  [4.1068 GiB/s 4.1149 GiB/s 4.1257 GiB/s]

  find_arrays    full=        1  standalone=        1
                        time:   [12.167 ms 12.189 ms 12.213 ms]
                        thrpt:  [12.023 GiB/s 12.047 GiB/s 12.069 GiB/s]

  find_strings   full= 12000000  standalone= 12000000
                        time:   [169.23 ms 169.41 ms 169.61 ms]
                        thrpt:  [886.54 MiB/s 887.55 MiB/s 888.53 MiB/s]

  find_members   full= 10000000  standalone= 10000000
                        time:   [170.28 ms 170.62 ms 171.02 ms]
                        thrpt:  [879.21 MiB/s 881.27 MiB/s 883.05 MiB/s]
```

</details>

<details>
<summary>nested</summary>

```
  find_objects   full=  5000000  standalone=  2000000
                        time:   [33.383 ms 33.458 ms 33.588 ms]
                        thrpt:  [3.5615 GiB/s 3.5754 GiB/s 3.5834 GiB/s]

  find_arrays    full=  1000001  standalone=  1000000
                        time:   [20.383 ms 20.417 ms 20.446 ms]
                        thrpt:  [5.8508 GiB/s 5.8590 GiB/s 5.8687 GiB/s]

  find_strings   full= 13000000  standalone= 13000000
                        time:   [182.25 ms 182.61 ms 183.08 ms]
                        thrpt:  [669.08 MiB/s 670.78 MiB/s 672.12 MiB/s]

  find_members   full= 10000000  standalone=  6000000
                        time:   [106.44 ms 106.58 ms 106.72 ms]
                        thrpt:  [1.1210 GiB/s 1.1224 GiB/s 1.1238 GiB/s]
```

</details>

### nightly - `RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-json_standalone --features avx2`

**small:**

<details>
<summary>numbers</summary>

```
  find_objects   full=        0  standalone=        0
                        time:   [39.971 µs 40.444 µs 40.991 µs]
                        thrpt:  [45.201 GiB/s 45.812 GiB/s 46.354 GiB/s]

  find_arrays    full=        1  standalone=        1
                        time:   [41.829 µs 42.189 µs 42.562 µs]
                        thrpt:  [43.532 GiB/s 43.917 GiB/s 44.295 GiB/s]

  find_strings   full=        0  standalone=        0
                        time:   [41.789 µs 42.252 µs 42.694 µs]
                        thrpt:  [43.397 GiB/s 43.851 GiB/s 44.337 GiB/s]

  find_members   full=        0  standalone=        0
                        time:   [41.508 µs 42.182 µs 42.866 µs]
                        thrpt:  [43.224 GiB/s 43.924 GiB/s 44.637 GiB/s]
```

</details>

<details>
<summary>objects</summary>

```
  find_objects   full=    20000  standalone=    20000
                        time:   [248.93 µs 249.15 µs 249.37 µs]
                        thrpt:  [5.4402 GiB/s 5.4451 GiB/s 5.4499 GiB/s]

  find_arrays    full=        1  standalone=        1
                        time:   [29.019 µs 29.330 µs 29.629 µs]
                        thrpt:  [45.787 GiB/s 46.255 GiB/s 46.750 GiB/s]

  find_strings   full=   120000  standalone=   120000
                        time:   [1.2349 ms 1.2364 ms 1.2379 ms]
                        thrpt:  [1.0960 GiB/s 1.0973 GiB/s 1.0986 GiB/s]

  find_members   full=   100000  standalone=   100000
                        time:   [1.3471 ms 1.3489 ms 1.3508 ms]
                        thrpt:  [1.0043 GiB/s 1.0057 GiB/s 1.0071 GiB/s]
```

</details>

<details>
<summary>nested</summary>

```
  find_objects   full=    50000  standalone=    20000
                        time:   [232.58 µs 232.94 µs 233.28 µs]
                        thrpt:  [4.7287 GiB/s 4.7356 GiB/s 4.7429 GiB/s]

  find_arrays    full=    10001  standalone=    10000
                        time:   [142.54 µs 145.18 µs 148.29 µs]
                        thrpt:  [7.4390 GiB/s 7.5980 GiB/s 7.7389 GiB/s]

  find_strings   full=   130000  standalone=   130000
                        time:   [1.3239 ms 1.3257 ms 1.3275 ms]
                        thrpt:  [850.91 MiB/s 852.08 MiB/s 853.23 MiB/s]

  find_members   full=   100000  standalone=    60000
                        time:   [835.83 µs 836.77 µs 837.81 µs]
                        thrpt:  [1.3167 GiB/s 1.3183 GiB/s 1.3198 GiB/s]
```

</details>

**big:**

<details>
<summary>numbers</summary>

```
  find_objects   full=        0  standalone=        0
                        time:   [17.841 ms 17.958 ms 18.166 ms]
                        thrpt:  [11.737 GiB/s 11.873 GiB/s 11.951 GiB/s]

  find_arrays    full=        1  standalone=        1
                        time:   [17.802 ms 17.854 ms 17.921 ms]
                        thrpt:  [11.898 GiB/s 11.942 GiB/s 11.977 GiB/s]

  find_strings   full=        0  standalone=        0
                        time:   [17.843 ms 17.874 ms 17.901 ms]
                        thrpt:  [11.911 GiB/s 11.929 GiB/s 11.950 GiB/s]

  find_members   full=        0  standalone=        0
                        time:   [17.746 ms 17.779 ms 17.816 ms]
                        thrpt:  [11.968 GiB/s 11.993 GiB/s 12.015 GiB/s]
```

</details>

<details>
<summary>objects</summary>

```
  find_objects   full=  2000000  standalone=  2000000
                        time:   [30.389 ms 30.474 ms 30.555 ms]
                        thrpt:  [4.8057 GiB/s 4.8185 GiB/s 4.8319 GiB/s]

  find_arrays    full=        1  standalone=        1
                        time:   [12.100 ms 12.160 ms 12.238 ms]
                        thrpt:  [11.999 GiB/s 12.075 GiB/s 12.135 GiB/s]

  find_strings   full= 12000000  standalone= 12000000
                        time:   [128.84 ms 129.81 ms 130.91 ms]
                        thrpt:  [1.1217 GiB/s 1.1312 GiB/s 1.1397 GiB/s]

  find_members   full= 10000000  standalone= 10000000
                        time:   [141.47 ms 142.95 ms 144.72 ms]
                        thrpt:  [1.0146 GiB/s 1.0272 GiB/s 1.0379 GiB/s]
```

</details>

<details>
<summary>nested</summary>

```
  find_objects   full=  5000000  standalone=  2000000
                        time:   [28.129 ms 28.213 ms 28.330 ms]
                        thrpt:  [4.2225 GiB/s 4.2399 GiB/s 4.2527 GiB/s]

  find_arrays    full=  1000001  standalone=  1000000
                        time:   [17.688 ms 17.795 ms 17.935 ms]
                        thrpt:  [6.6698 GiB/s 6.7223 GiB/s 6.7628 GiB/s]

  find_strings   full= 13000000  standalone= 13000000
                        time:   [136.25 ms 136.81 ms 137.58 ms]
                        thrpt:  [890.34 MiB/s 895.39 MiB/s 899.07 MiB/s]

  find_members   full= 10000000  standalone=  6000000
                        time:   [90.492 ms 90.885 ms 91.262 ms]
                        thrpt:  [1.3108 GiB/s 1.3162 GiB/s 1.3219 GiB/s]
```

</details>

---

## Reading the numbers

- A higher meon number is "did less work" (no validation, no number parsing, no
  unescaping), not "is the better parser". A consumer that needs typed values or
  decoded strings does that work on top of meon's spans.
- Compare a cell only against the same corpus in the same build block.
- `simd-json` and `sonic-rs` produce usable values directly; meon produces spans
  you project from. The gap is the cost of that materialisation, which your
  workload may or may not need.
- The `numbers` corpus shows the widest gap by construction; `objects` and
  `nested` are closer to a mixed real payload. Scale matters more than the
  small-input headline — see [Scaling from small to big](#scaling-from-small-to-big).
