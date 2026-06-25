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
* * [***MD_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/MD_COMPARE.md)
* * ***JSON_COMPARE.md***    <--
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
find_objects   full=        0  standalone=        0   thrpt: [43.699 44.230 44.748 GiB/s]
find_arrays    full=        1  standalone=        1   thrpt: [41.567 42.188 42.811 GiB/s]
find_strings   full=        0  standalone=        0   thrpt: [43.884 44.574 45.339 GiB/s]
find_members   full=        0  standalone=        0   thrpt: [43.498 43.750 43.976 GiB/s]
```

</details>

<details>
<summary>objects</summary>

```
find_objects   full=    20000  standalone=    20000   thrpt: [4.3098 4.3217 4.3342 GiB/s]
find_arrays    full=        1  standalone=        1   thrpt: [40.928 41.523 42.177 GiB/s]
find_strings   full=   120000  standalone=   120000   thrpt: [843.98 846.58 849.19 MiB/s]
find_members   full=   100000  standalone=   100000   thrpt: [843.47 844.85 846.23 MiB/s]
```

</details>

<details>
<summary>nested</summary>

```
find_objects   full=    50000  standalone=    20000   thrpt: [3.7569 3.7673 3.7779 GiB/s]
find_arrays    full=    10001  standalone=    10000   thrpt: [6.8639 6.8803 6.8958 GiB/s]
find_strings   full=   130000  standalone=   130000   thrpt: [634.21 636.08 637.99 MiB/s]
find_members   full=   100000  standalone=    60000   thrpt: [1.1102 1.1112 1.1120 GiB/s]
```

</details>

**big:**

<details>
<summary>numbers</summary>

```
find_objects   full=        0  standalone=        0   thrpt: [11.842 11.891 11.938 GiB/s]
find_arrays    full=        1  standalone=        1   thrpt: [11.975 12.007 12.039 GiB/s]
find_strings   full=        0  standalone=        0   thrpt: [11.885 11.937 11.983 GiB/s]
find_members   full=        0  standalone=        0   thrpt: [11.893 11.938 11.981 GiB/s]
```

</details>

<details>
<summary>objects</summary>

```
find_objects   full=  2000000  standalone=  2000000   thrpt: [4.1262 4.1351 4.1439 GiB/s]
find_arrays    full=        1  standalone=        1   thrpt: [12.199 12.240 12.272 GiB/s]
find_strings   full= 12000000  standalone= 12000000   thrpt: [910.84 913.30 915.42 MiB/s]
find_members   full= 10000000  standalone= 10000000   thrpt: [894.30 896.53 898.66 MiB/s]
```

</details>

<details>
<summary>nested</summary>

```
find_objects   full=  5000000  standalone=  2000000   thrpt: [3.5463 3.5585 3.5704 GiB/s]
find_arrays    full=  1000001  standalone=  1000000   thrpt: [5.8169 5.8338 5.8510 GiB/s]
find_strings   full= 13000000  standalone= 13000000   thrpt: [682.84 685.97 689.08 MiB/s]
find_members   full= 10000000  standalone=  6000000   thrpt: [1.1045 1.1127 1.1209 GiB/s]
```

</details>

### nightly - `RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-json_standalone --features avx2`

**small:**

<details>
<summary>numbers</summary>

```
find_objects   full=        0  standalone=        0   thrpt: [45.270 45.800 46.309 GiB/s]
find_arrays    full=        1  standalone=        1   thrpt: [44.885 45.356 45.835 GiB/s]
find_strings   full=        0  standalone=        0   thrpt: [47.595 47.899 48.191 GiB/s]
find_members   full=        0  standalone=        0   thrpt: [44.655 45.183 45.697 GiB/s]
```

</details>

<details>
<summary>objects</summary>

```
find_objects   full=    20000  standalone=    20000   thrpt: [5.1394 5.1423 5.1451 GiB/s]
find_arrays    full=        1  standalone=        1   thrpt: [45.130 45.470 45.801 GiB/s]
find_strings   full=   120000  standalone=   120000   thrpt: [1.0926 1.0947 1.0968 GiB/s]
find_members   full=   100000  standalone=   100000   thrpt: [1.0095 1.0109 1.0122 GiB/s]
```

</details>

<details>
<summary>nested</summary>

```
find_objects   full=    50000  standalone=    20000   thrpt: [4.8394 4.8460 4.8517 GiB/s]
find_arrays    full=    10001  standalone=    10000   thrpt: [7.7273 7.7924 7.8565 GiB/s]
find_strings   full=   130000  standalone=   130000   thrpt: [834.68 837.32 839.41 MiB/s]
find_members   full=   100000  standalone=    60000   thrpt: [1.3276 1.3299 1.3320 GiB/s]
```

</details>

**big:**

<details>
<summary>numbers</summary>

```
find_objects   full=        0  standalone=        0   thrpt: [11.913 11.959 12.002 GiB/s]
find_arrays    full=        1  standalone=        1   thrpt: [11.862 11.934 11.992 GiB/s]
find_strings   full=        0  standalone=        0   thrpt: [11.843 11.900 11.954 GiB/s]
find_members   full=        0  standalone=        0   thrpt: [11.857 11.910 11.960 GiB/s]
```

</details>

<details>
<summary>objects</summary>

```
find_objects   full=  2000000  standalone=  2000000   thrpt: [4.7771 4.7937 4.8087 GiB/s]
find_arrays    full=        1  standalone=        1   thrpt: [12.164 12.200 12.230 GiB/s]
find_strings   full= 12000000  standalone= 12000000   thrpt: [1.1422 1.1448 1.1473 GiB/s]
find_members   full= 10000000  standalone= 10000000   thrpt: [1.0536 1.0558 1.0579 GiB/s]
```

</details>

<details>
<summary>nested</summary>

```
find_objects   full=  5000000  standalone=  2000000   thrpt: [4.2459 4.2578 4.2694 GiB/s]
find_arrays    full=  1000001  standalone=  1000000   thrpt: [6.6552 6.6879 6.7183 GiB/s]
find_strings   full= 13000000  standalone= 13000000   thrpt: [905.56 906.38 907.16 MiB/s]
find_members   full= 10000000  standalone=  6000000   thrpt: [1.3037 1.3115 1.3198 GiB/s]
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
