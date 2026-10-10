# meon-json — Cross-parser comparison

Throughput of [`meon-json`](https://github.com/vgnapuga/meon/blob/main/meon-json/README.md)
(built on the [`meon`](https://github.com/vgnapuga/meon/blob/main/meon/README.md)
engine) next to two validating JSON parsers, on the same corpora as the
intra-engine benches.

> **Four parsers, two different jobs.** `meon-json` is, by design, a
> **structural reader**: it parses JSON into flat span vectors — no validation,
> no number parsing, no string unescaping. `simd-json` and `sonic-rs` are
> validating parsers that materialise a tape / an owned value — they parse
> every number and unescape every string. A throughput gap is the difference
> between those jobs. `Throughput::Bytes` measures how fast the input is
> consumed, since the four produce different things.

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

## Two different jobs

- **Different outputs.** `meon-structural` emits span vectors and materialises
  nothing. `meon-typed` adds only first-byte classification. `simd-json` and
  `sonic-rs` validate, parse every number to a value, and unescape every string.
  The throughput gap is the cost of that materialisation. A line doing the same
  job would also parse numbers to values and unescape strings — **neither meon
  line does that**, so even `meon-typed` is a different job from a tape or an
  owned value.

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
| `numbers` | Flat array of numbers / bools / nulls.                                                    | Scalar scanning. meon emits span vectors; validating parsers parse every number. |
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

| Corpus    | `meon-structural`          | `meon-typed`               | `simd-json`                | `sonic-rs`                 |
|-----------|----------------------------|----------------------------|----------------------------|----------------------------|
| `numbers` | [2.1805 ms]=[870.11 MiB/s] | [4.7649 ms]=[398.18 MiB/s] | [5.4442 ms]=[348.49 MiB/s] | [2.5050 ms]=[757.39 MiB/s] |
| `objects` | [3.7751 ms]=[367.99 MiB/s] | [5.3719 ms]=[258.60 MiB/s] | [2.4693 ms]=[562.58 MiB/s] | [1.7249 ms]=[805.40 MiB/s] |
| `nested`  | [4.6293 ms]=[244.00 MiB/s] | [5.9410 ms]=[190.13 MiB/s] | [2.5209 ms]=[448.08 MiB/s] | [2.2510 ms]=[501.82 MiB/s] |

<details>
    <summary>full log</summary>

    ```
    ┌─ corpus: numbers
    │  size:            1.90 MiB  (1989441 bytes)
    │  structural:         1     (0.0 per KiB)
    │
    │      objects:         0      arrays:         1     strings:         0
    │      members:         0     scalars:         0       loose:         1
    │  typed: nums:    150000       trues:     50000      falses:     50000     nulls:     50000
    └─
    json-compare/numbers/meon-structural
                            time:   [2.1804 ms 2.1805 ms 2.1807 ms]
                            thrpt:  [870.04 MiB/s 870.11 MiB/s 870.17 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    json-compare/numbers/meon-typed
                            time:   [4.7620 ms 4.7649 ms 4.7698 ms]
                            thrpt:  [397.77 MiB/s 398.18 MiB/s 398.42 MiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      3 (15.00%) high severe
    json-compare/numbers/simd-json
                            time:   [5.1763 ms 5.4442 ms 5.8969 ms]
                            thrpt:  [321.74 MiB/s 348.49 MiB/s 366.54 MiB/s]
    json-compare/numbers/sonic-rs
                            time:   [2.4991 ms 2.5050 ms 2.5105 ms]
                            thrpt:  [755.73 MiB/s 757.39 MiB/s 759.18 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    
    ┌─ corpus: objects
    │  size:            1.39 MiB  (1456671 bytes)
    │  structural:    240001     (168.7 per KiB)
    │
    │      objects:     20000      arrays:         1     strings:    120000
    │      members:    100000     scalars:         0       loose:         1
    │  typed: nums:     40000       trues:     10000      falses:     10000     nulls:     20000
    └─
    json-compare/objects/meon-structural
                            time:   [3.7739 ms 3.7751 ms 3.7766 ms]
                            thrpt:  [367.84 MiB/s 367.99 MiB/s 368.11 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    json-compare/objects/meon-typed
                            time:   [5.3474 ms 5.3719 ms 5.4021 ms]
                            thrpt:  [257.16 MiB/s 258.60 MiB/s 259.79 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
    json-compare/objects/simd-json
                            time:   [2.4157 ms 2.4693 ms 2.5573 ms]
                            thrpt:  [543.23 MiB/s 562.58 MiB/s 575.06 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    json-compare/objects/sonic-rs
                            time:   [1.7215 ms 1.7249 ms 1.7279 ms]
                            thrpt:  [803.96 MiB/s 805.40 MiB/s 806.98 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    
    ┌─ corpus: nested
    │  size:            1.13 MiB  (1184451 bytes)
    │  structural:    290001     (250.7 per KiB)
    │
    │      objects:     50000      arrays:     10001     strings:    130000
    │      members:    100000     scalars:         0       loose:         1
    │  typed: nums:     40000       trues:     10000      falses:         0     nulls:         0
    └─
    json-compare/nested/meon-structural
                            time:   [4.6089 ms 4.6293 ms 4.6404 ms]
                            thrpt:  [243.42 MiB/s 244.00 MiB/s 245.09 MiB/s]
    json-compare/nested/meon-typed
                            time:   [5.9239 ms 5.9410 ms 5.9597 ms]
                            thrpt:  [189.54 MiB/s 190.13 MiB/s 190.68 MiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      3 (15.00%) low mild
    json-compare/nested/simd-json
                            time:   [2.4863 ms 2.5209 ms 2.5818 ms]
                            thrpt:  [437.52 MiB/s 448.08 MiB/s 454.32 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    json-compare/nested/sonic-rs
                            time:   [2.2493 ms 2.2510 ms 2.2537 ms]
                            thrpt:  [501.22 MiB/s 501.82 MiB/s 502.20 MiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      3 (15.00%) high severe
    ```
</details>

**big:**

| Corpus    | `meon-structural`          | `meon-typed`               | `simd-json`                | `sonic-rs`                 |
|-----------|----------------------------|----------------------------|----------------------------|----------------------------|
| `numbers` | [234.76 ms]=[930.06 MiB/s] | [638.13 ms]=[342.16 MiB/s] | [969.17 ms]=[225.28 MiB/s] | [930.41 ms]=[234.67 MiB/s] |
| `objects` | [542.84 ms]=[276.99 MiB/s] | [754.88 ms]=[199.19 MiB/s] | [710.30 ms]=[211.69 MiB/s] | [489.36 ms]=[307.26 MiB/s] |
| `nested`  | [663.89 ms]=[184.51 MiB/s] | [811.91 ms]=[150.87 MiB/s] | [754.96 ms]=[162.25 MiB/s] | [565.59 ms]=[216.58 MiB/s] |

<details>
    <summary>full log</summary>

    ```
    ┌─ corpus: numbers
    │  size:          218.34 MiB  (228944439 bytes)
    │  structural:         1     (0.0 per KiB)
    │
    │      objects:         0      arrays:         1     strings:         0
    │      members:         0     scalars:         0       loose:         1
    │  typed: nums:  15000000       trues:   5000000      falses:   5000000     nulls:   5000000
    └─
    json-compare/numbers/meon-structural
                            time:   [234.66 ms 234.76 ms 234.89 ms]
                            thrpt:  [929.53 MiB/s 930.06 MiB/s 930.43 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
    Benchmarking json-compare/numbers/meon-typed: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 12.8s, or reduce sample count to 10.
    json-compare/numbers/meon-typed
                            time:   [635.76 ms 638.13 ms 640.60 ms]
                            thrpt:  [340.83 MiB/s 342.16 MiB/s 343.43 MiB/s]
    Benchmarking json-compare/numbers/simd-json: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 21.9s, or reduce sample count to 10.
    json-compare/numbers/simd-json
                            time:   [968.12 ms 969.17 ms 970.34 ms]
                            thrpt:  [225.01 MiB/s 225.28 MiB/s 225.53 MiB/s]
    Benchmarking json-compare/numbers/sonic-rs: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 18.4s, or reduce sample count to 10.
    json-compare/numbers/sonic-rs
                            time:   [925.06 ms 930.41 ms 936.07 ms]
                            thrpt:  [233.25 MiB/s 234.67 MiB/s 236.03 MiB/s]
    
    ┌─ corpus: objects
    │  size:          150.36 MiB  (157666671 bytes)
    │  structural:  24000001     (155.9 per KiB)
    │
    │      objects:   2000000      arrays:         1     strings:  12000000
    │      members:  10000000     scalars:         0       loose:         1
    │  typed: nums:   4000000       trues:   1000000      falses:   1000000     nulls:   2000000
    └─
    Benchmarking json-compare/objects/meon-structural: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 10.8s, or reduce sample count to 10.
    json-compare/objects/meon-structural
                            time:   [542.03 ms 542.84 ms 543.67 ms]
                            thrpt:  [276.57 MiB/s 276.99 MiB/s 277.41 MiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) low mild
      1 (5.00%) high mild
    Benchmarking json-compare/objects/meon-typed: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 15.1s, or reduce sample count to 10.
    json-compare/objects/meon-typed
                            time:   [753.63 ms 754.88 ms 756.28 ms]
                            thrpt:  [198.82 MiB/s 199.19 MiB/s 199.52 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    Benchmarking json-compare/objects/simd-json: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 15.8s, or reduce sample count to 10.
    json-compare/objects/simd-json
                            time:   [708.44 ms 710.30 ms 712.18 ms]
                            thrpt:  [211.13 MiB/s 211.69 MiB/s 212.24 MiB/s]
    json-compare/objects/sonic-rs
                            time:   [487.62 ms 489.36 ms 491.45 ms]
                            thrpt:  [305.96 MiB/s 307.26 MiB/s 308.36 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    
    ┌─ corpus: nested
    │  size:          122.49 MiB  (128444451 bytes)
    │  structural:  29000001     (231.2 per KiB)
    │
    │      objects:   5000000      arrays:   1000001     strings:  13000000
    │      members:  10000000     scalars:         0       loose:         1
    │  typed: nums:   4000000       trues:   1000000      falses:         0     nulls:         0
    └─
    Benchmarking json-compare/nested/meon-structural: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 13.0s, or reduce sample count to 10.
    json-compare/nested/meon-structural
                            time:   [662.20 ms 663.89 ms 665.51 ms]
                            thrpt:  [184.06 MiB/s 184.51 MiB/s 184.98 MiB/s]
    Benchmarking json-compare/nested/meon-typed: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 16.1s, or reduce sample count to 10.
    json-compare/nested/meon-typed
                            time:   [810.19 ms 811.91 ms 813.21 ms]
                            thrpt:  [150.63 MiB/s 150.87 MiB/s 151.19 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low severe
      1 (5.00%) low mild
    Benchmarking json-compare/nested/simd-json: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 16.4s, or reduce sample count to 10.
    json-compare/nested/simd-json
                            time:   [753.34 ms 754.96 ms 756.57 ms]
                            thrpt:  [161.91 MiB/s 162.25 MiB/s 162.60 MiB/s]
    Benchmarking json-compare/nested/sonic-rs: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 11.4s, or reduce sample count to 10.
    json-compare/nested/sonic-rs
                            time:   [563.31 ms 565.59 ms 567.84 ms]
                            thrpt:  [215.72 MiB/s 216.58 MiB/s 217.45 MiB/s]
    ```
</details>

### nightly - `RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-json_compare --features avx2`

> meon on AVX2; `simd-json` / `sonic-rs` on their own runtime SIMD.

**small:**

| Corpus    | `meon-structural`          | `meon-typed`               | `simd-json`                | `sonic-rs`                 |
|-----------|----------------------------|----------------------------|----------------------------|----------------------------|
| `numbers` | [2.4632 ms]=[770.24 MiB/s] | [5.1127 ms]=[371.09 MiB/s] | [5.4569 ms]=[347.68 MiB/s] | [2.5321 ms]=[749.28 MiB/s] |
| `objects` | [4.0565 ms]=[342.46 MiB/s] | [5.6322 ms]=[246.65 MiB/s] | [2.4343 ms]=[570.66 MiB/s] | [1.7211 ms]=[807.16 MiB/s] |
| `nested`  | [4.8705 ms]=[231.92 MiB/s] | [6.2523 ms]=[180.67 MiB/s] | [2.5501 ms]=[442.96 MiB/s] | [2.2486 ms]=[502.35 MiB/s] |

<details>
    <summary>full log</summary>

    ```
    ┌─ corpus: numbers
    │  size:            1.90 MiB  (1989441 bytes)
    │  structural:         1     (0.0 per KiB)
    │
    │      objects:         0      arrays:         1     strings:         0
    │      members:         0     scalars:         0       loose:         1
    │  typed: nums:    150000       trues:     50000      falses:     50000     nulls:     50000
    └─
    json-compare/numbers/meon-structural
                            time:   [2.4629 ms 2.4632 ms 2.4637 ms]
                            thrpt:  [770.10 MiB/s 770.24 MiB/s 770.34 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
    json-compare/numbers/meon-typed
                            time:   [5.0852 ms 5.1127 ms 5.1408 ms]
                            thrpt:  [369.06 MiB/s 371.09 MiB/s 373.10 MiB/s]
    Found 5 outliers among 20 measurements (25.00%)
      4 (20.00%) low mild
      1 (5.00%) high mild
    json-compare/numbers/simd-json
                            time:   [5.1884 ms 5.4569 ms 5.9136 ms]
                            thrpt:  [320.83 MiB/s 347.68 MiB/s 365.67 MiB/s]
    json-compare/numbers/sonic-rs
                            time:   [2.5247 ms 2.5321 ms 2.5398 ms]
                            thrpt:  [747.03 MiB/s 749.28 MiB/s 751.48 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    
    ┌─ corpus: objects
    │  size:            1.39 MiB  (1456671 bytes)
    │  structural:    240001     (168.7 per KiB)
    │
    │      objects:     20000      arrays:         1     strings:    120000
    │      members:    100000     scalars:         0       loose:         1
    │  typed: nums:     40000       trues:     10000      falses:     10000     nulls:     20000
    └─
    json-compare/objects/meon-structural
                            time:   [4.0539 ms 4.0565 ms 4.0605 ms]
                            thrpt:  [342.12 MiB/s 342.46 MiB/s 342.68 MiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) high mild
      2 (10.00%) high severe
    json-compare/objects/meon-typed
                            time:   [5.6226 ms 5.6322 ms 5.6429 ms]
                            thrpt:  [246.18 MiB/s 246.65 MiB/s 247.07 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    json-compare/objects/simd-json
                            time:   [2.3969 ms 2.4343 ms 2.5022 ms]
                            thrpt:  [555.19 MiB/s 570.66 MiB/s 579.57 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    json-compare/objects/sonic-rs
                            time:   [1.7162 ms 1.7211 ms 1.7261 ms]
                            thrpt:  [804.79 MiB/s 807.16 MiB/s 809.46 MiB/s]
    Found 5 outliers among 20 measurements (25.00%)
      3 (15.00%) low severe
      2 (10.00%) high severe
    
    ┌─ corpus: nested
    │  size:            1.13 MiB  (1184451 bytes)
    │  structural:    290001     (250.7 per KiB)
    │
    │      objects:     50000      arrays:     10001     strings:    130000
    │      members:    100000     scalars:         0       loose:         1
    │  typed: nums:     40000       trues:     10000      falses:         0     nulls:         0
    └─
    json-compare/nested/meon-structural
                            time:   [4.8685 ms 4.8705 ms 4.8742 ms]
                            thrpt:  [231.75 MiB/s 231.92 MiB/s 232.02 MiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      3 (15.00%) high severe
    json-compare/nested/meon-typed
                            time:   [6.2380 ms 6.2523 ms 6.2705 ms]
                            thrpt:  [180.14 MiB/s 180.67 MiB/s 181.08 MiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) high mild
      2 (10.00%) high severe
    json-compare/nested/simd-json
                            time:   [2.5136 ms 2.5501 ms 2.6184 ms]
                            thrpt:  [431.41 MiB/s 442.96 MiB/s 449.38 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    json-compare/nested/sonic-rs
                            time:   [2.2472 ms 2.2486 ms 2.2507 ms]
                            thrpt:  [501.89 MiB/s 502.35 MiB/s 502.66 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    ```
</details>

**big:**

| Corpus    | `meon-structural`          | `meon-typed`               | `simd-json`                | `sonic-rs`                 |
|-----------|----------------------------|----------------------------|----------------------------|----------------------------|
| `numbers` | [253.95 ms]=[859.78 MiB/s] | [662.64 ms]=[329.50 MiB/s] | [971.66 ms]=[224.71 MiB/s] | [921.40 ms]=[236.96 MiB/s] |
| `objects` | [568.41 ms]=[264.53 MiB/s] | [781.99 ms]=[192.28 MiB/s] | [709.28 ms]=[211.99 MiB/s] | [494.43 ms]=[304.11 MiB/s] |
| `nested`  | [688.82 ms]=[177.83 MiB/s] | [839.24 ms]=[145.96 MiB/s] | [749.09 ms]=[163.52 MiB/s] | [565.11 ms]=[216.76 MiB/s] |

<details>
    <summary>full log</summary>

    ```
    ┌─ corpus: numbers
    │  size:          218.34 MiB  (228944439 bytes)
    │  structural:         1     (0.0 per KiB)
    │
    │      objects:         0      arrays:         1     strings:         0
    │      members:         0     scalars:         0       loose:         1
    │  typed: nums:  15000000       trues:   5000000      falses:   5000000     nulls:   5000000
    └─
    json-compare/numbers/meon-structural
                            time:   [253.74 ms 253.95 ms 254.25 ms]
                            thrpt:  [858.76 MiB/s 859.78 MiB/s 860.49 MiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) high mild
      1 (5.00%) high severe
    Benchmarking json-compare/numbers/meon-typed: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 13.2s, or reduce sample count to 10.
    json-compare/numbers/meon-typed
                            time:   [660.33 ms 662.64 ms 664.81 ms]
                            thrpt:  [328.42 MiB/s 329.50 MiB/s 330.65 MiB/s]
    Benchmarking json-compare/numbers/simd-json: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 22.0s, or reduce sample count to 10.
    json-compare/numbers/simd-json
                            time:   [970.40 ms 971.66 ms 973.05 ms]
                            thrpt:  [224.39 MiB/s 224.71 MiB/s 225.00 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    Benchmarking json-compare/numbers/sonic-rs: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 18.5s, or reduce sample count to 10.
    json-compare/numbers/sonic-rs
                            time:   [920.47 ms 921.40 ms 922.63 ms]
                            thrpt:  [236.65 MiB/s 236.96 MiB/s 237.20 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    
    ┌─ corpus: objects
    │  size:          150.36 MiB  (157666671 bytes)
    │  structural:  24000001     (155.9 per KiB)
    │
    │      objects:   2000000      arrays:         1     strings:  12000000
    │      members:  10000000     scalars:         0       loose:         1
    │  typed: nums:   4000000       trues:   1000000      falses:   1000000     nulls:   2000000
    └─
    Benchmarking json-compare/objects/meon-structural: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 11.4s, or reduce sample count to 10.
    json-compare/objects/meon-structural
                            time:   [567.14 ms 568.41 ms 569.55 ms]
                            thrpt:  [264.00 MiB/s 264.53 MiB/s 265.12 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
    Benchmarking json-compare/objects/meon-typed: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 15.6s, or reduce sample count to 10.
    json-compare/objects/meon-typed
                            time:   [780.91 ms 781.99 ms 783.12 ms]
                            thrpt:  [192.00 MiB/s 192.28 MiB/s 192.55 MiB/s]
    Benchmarking json-compare/objects/simd-json: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 15.7s, or reduce sample count to 10.
    json-compare/objects/simd-json
                            time:   [707.62 ms 709.28 ms 710.88 ms]
                            thrpt:  [211.52 MiB/s 211.99 MiB/s 212.49 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low severe
      1 (5.00%) high severe
    json-compare/objects/sonic-rs
                            time:   [490.53 ms 494.43 ms 498.49 ms]
                            thrpt:  [301.63 MiB/s 304.11 MiB/s 306.53 MiB/s]
    
    ┌─ corpus: nested
    │  size:          122.49 MiB  (128444451 bytes)
    │  structural:  29000001     (231.2 per KiB)
    │
    │      objects:   5000000      arrays:   1000001     strings:  13000000
    │      members:  10000000     scalars:         0       loose:         1
    │  typed: nums:   4000000       trues:   1000000      falses:         0     nulls:         0
    └─
    Benchmarking json-compare/nested/meon-structural: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 13.6s, or reduce sample count to 10.
    json-compare/nested/meon-structural
                            time:   [688.01 ms 688.82 ms 689.90 ms]
                            thrpt:  [177.55 MiB/s 177.83 MiB/s 178.04 MiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) high mild
      1 (5.00%) high severe
    Benchmarking json-compare/nested/meon-typed: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 16.6s, or reduce sample count to 10.
    json-compare/nested/meon-typed
                            time:   [834.48 ms 839.24 ms 843.15 ms]
                            thrpt:  [145.28 MiB/s 145.96 MiB/s 146.79 MiB/s]
    Found 5 outliers among 20 measurements (25.00%)
      3 (15.00%) low severe
      1 (5.00%) low mild
      1 (5.00%) high mild
    Benchmarking json-compare/nested/simd-json: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 16.3s, or reduce sample count to 10.
    json-compare/nested/simd-json
                            time:   [747.28 ms 749.09 ms 750.98 ms]
                            thrpt:  [163.11 MiB/s 163.52 MiB/s 163.92 MiB/s]
    Benchmarking json-compare/nested/sonic-rs: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 11.5s, or reduce sample count to 10.
    json-compare/nested/sonic-rs
                            time:   [564.13 ms 565.11 ms 566.11 ms]
                            thrpt:  [216.38 MiB/s 216.76 MiB/s 217.14 MiB/s]
    ```
</details>

---

## Scaling from small to big

How each parser holds up as the input grows past cache (median `thrpt`, MiB/s;
`simd-json` / `sonic-rs` from the stable run):

| Parser                      | `numbers`     | `objects`     | `nested`      |
|-----------------------------|---------------|---------------|---------------|
| `meon-structural` - stable  | 870 -> 930    | 368 -> 277    | 244 -> 185    |
| `meon-structural` - nightly | 770 -> 860    | 342 -> 265    | 232 -> 178    |
| `meon-typed` - stable       | 398 -> 342    | 259 -> 199    | 190 -> 151    |
| `meon-typed` - nightly      | 371 -> 330    | 247 -> 192    | 181 -> 146    |
| `simd-json`                 | 348 -> 225    | 563 -> 212    | 448 -> 162    |
| `sonic-rs`                  | 757 -> 235    | 805 -> 307    | 502 -> 217    |

- **meon degrades little with scale.** `meon-structural` gains on `numbers`
  (870 -> 930) and loses about a quarter on `objects` / `nested` (-25% / -24%).
  `meon-typed` loses 14-23%. The flat span table stays largely cache-resident.
- **The validating parsers lose more at big.** `simd-json` loses 62% on
  `objects`, 64% on `nested` and 35% on `numbers`. `sonic-rs` loses 57-69% on
  every corpus, most on `numbers`. Both materialise a tape / an owned `Value`,
  and that working set outgrows cache as the document grows.
- **The gap narrows with scale.** At small, `simd-json` and `sonic-rs` lead
  `meon-structural` on `objects` / `nested` by 1.5-2.2x. At big,
  `meon-structural` overtakes `simd-json` (277 vs 212 on `objects`, 185 vs 162
  on `nested`), while `sonic-rs` keeps a smaller lead (307 vs 277, 217 vs 185).
  On `numbers` meon leads at every scale, from 1.15x over `sonic-rs` at small to
  about 4x at big.
- **AVX2 does not speed up meon-json.** The nightly AVX2 rows follow the same
  scaling shape, but they are 3-11% slower than the stable SWAR rows on every
  corpus at both sizes.

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

#### Standalone - `find_*`
| Function       | `numbers`                  | `objects`                  | `nested`                   |
|----------------|----------------------------|----------------------------|----------------------------|
| `find_objects` | [22.495 µs]=[82.366 GiB/s] | [303.27 µs]=[4.4734 GiB/s] | [284.77 µs]=[3.8737 GiB/s] |
| `find_arrays`  | [25.984 µs]=[71.304 GiB/s] | [18.090 µs]=[74.995 GiB/s] | [150.14 µs]=[7.3472 GiB/s] |
| `find_strings` | [22.542 µs]=[82.194 GiB/s] | [1.4992 ms]=[926.59 MiB/s] | [1.6234 ms]=[695.82 MiB/s] |
| `find_members` | [20.783 µs]=[89.148 GiB/s] | [1.6914 ms]=[821.33 MiB/s] | [1.0464 ms]=[1.0542 GiB/s] |

#### Context-aware - `context()` + `find_context_*`
| Function               | `numbers`                  | `objects`                  | `nested`                   |
|------------------------|----------------------------|----------------------------|----------------------------|
| `context`              | [23.111 µs]=[80.171 GiB/s] | [1.6700 ms]=[831.84 MiB/s] | [1.8042 ms]=[626.07 MiB/s] |
| `find_context_objects` | [20.029 µs]=[92.505 GiB/s] | [395.69 µs]=[3.4285 GiB/s] | [353.65 µs]=[3.1192 GiB/s] |
| `find_context_arrays`  | [27.189 µs]=[68.146 GiB/s] | [51.039 µs]=[26.580 GiB/s] | [213.84 µs]=[5.1586 GiB/s] |

#### Cold context - `find_context_*_cold`
| Function                    | `numbers`                  | `objects`                  | `nested`                   |
|-----------------------------|----------------------------|----------------------------|----------------------------|
| `find_context_objects_cold` | [42.409 µs]=[43.689 GiB/s] | [2.0608 ms]=[674.11 MiB/s] | [2.1584 ms]=[523.33 MiB/s] |
| `find_context_arrays_cold`  | [47.290 µs]=[39.180 GiB/s] | [1.7131 ms]=[810.90 MiB/s] | [2.0145 ms]=[560.73 MiB/s] |

<details>
    <summary>numbers full log</summary>

    ```
    ┌─ corpus: numbers
    │  size:            1.90 MiB  (1989441 bytes)
    │  structural:         1     (0.0 per KiB)
    │
    │      objects:         0      arrays:         1     strings:         0
    │      members:         0     scalars:         0       loose:         1
    │  typed: nums:    150000       trues:     50000      falses:     50000     nulls:     50000
    └─
    │  full-vs-standalone counts:
        find_objects   full=        0  standalone=        0
    json-standalone/numbers/find_objects
                            time:   [21.666 µs 22.495 µs 23.408 µs]
                            thrpt:  [79.152 GiB/s 82.366 GiB/s 85.518 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low mild
      1 (5.00%) high mild
        find_arrays    full=        1  standalone=        1
    json-standalone/numbers/find_arrays
                            time:   [25.016 µs 25.984 µs 26.900 µs]
                            thrpt:  [68.878 GiB/s 71.304 GiB/s 74.065 GiB/s]
        find_strings   full=        0  standalone=        0
    json-standalone/numbers/find_strings
                            time:   [21.892 µs 22.542 µs 23.038 µs]
                            thrpt:  [80.426 GiB/s 82.194 GiB/s 84.634 GiB/s]
        find_members   full=        0  standalone=        0
    json-standalone/numbers/find_members
                            time:   [20.046 µs 20.783 µs 21.394 µs]
                            thrpt:  [86.603 GiB/s 89.148 GiB/s 92.427 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    │  context regions (strings): 0
    json-standalone/numbers/context
                            time:   [22.137 µs 23.111 µs 24.090 µs]
                            thrpt:  [76.912 GiB/s 80.171 GiB/s 83.699 GiB/s]
    │  full-vs-context-aware counts:
        find_context_objects full=        0  context-aware=        0
    json-standalone/numbers/find_context_objects
                            time:   [19.197 µs 20.029 µs 20.658 µs]
                            thrpt:  [89.691 GiB/s 92.505 GiB/s 96.518 GiB/s]
        find_context_arrays full=        1  context-aware=        1
    json-standalone/numbers/find_context_arrays
                            time:   [26.725 µs 27.189 µs 27.677 µs]
                            thrpt:  [66.944 GiB/s 68.146 GiB/s 69.328 GiB/s]
    json-standalone/numbers/find_context_objects_cold
                            time:   [40.912 µs 42.409 µs 43.608 µs]
                            thrpt:  [42.488 GiB/s 43.689 GiB/s 45.288 GiB/s]
    json-standalone/numbers/find_context_arrays_cold
                            time:   [45.711 µs 47.290 µs 48.850 µs]
                            thrpt:  [37.928 GiB/s 39.180 GiB/s 40.533 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    ```
</details>

<details>
    <summary>objects full log</summary>

    ```
    ┌─ corpus: objects
    │  size:            1.39 MiB  (1456671 bytes)
    │  structural:    240001     (168.7 per KiB)
    │
    │      objects:     20000      arrays:         1     strings:    120000
    │      members:    100000     scalars:         0       loose:         1
    │  typed: nums:     40000       trues:     10000      falses:     10000     nulls:     20000
    └─
    │  full-vs-standalone counts:
        find_objects   full=    20000  standalone=    20000
    json-standalone/objects/find_objects
                            time:   [302.37 µs 303.27 µs 304.44 µs]
                            thrpt:  [4.4562 GiB/s 4.4734 GiB/s 4.4866 GiB/s]
        find_arrays    full=        1  standalone=        1
    json-standalone/objects/find_arrays
                            time:   [17.590 µs 18.090 µs 18.594 µs]
                            thrpt:  [72.961 GiB/s 74.995 GiB/s 77.125 GiB/s]
        find_strings   full=   120000  standalone=   120000
    json-standalone/objects/find_strings
                            time:   [1.4982 ms 1.4992 ms 1.5005 ms]
                            thrpt:  [925.84 MiB/s 926.59 MiB/s 927.22 MiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) high mild
      2 (10.00%) high severe
        find_members   full=   100000  standalone=   100000
    json-standalone/objects/find_members
                            time:   [1.6902 ms 1.6914 ms 1.6925 ms]
                            thrpt:  [820.77 MiB/s 821.33 MiB/s 821.90 MiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      3 (15.00%) high mild
    │  context regions (strings): 120000
    json-standalone/objects/context
                            time:   [1.6680 ms 1.6700 ms 1.6729 ms]
                            thrpt:  [830.40 MiB/s 831.84 MiB/s 832.83 MiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) high mild
      1 (5.00%) high severe
    │  full-vs-context-aware counts:
        find_context_objects full=    20000  context-aware=    20000
    json-standalone/objects/find_context_objects
                            time:   [394.82 µs 395.69 µs 396.35 µs]
                            thrpt:  [3.4228 GiB/s 3.4285 GiB/s 3.4361 GiB/s]
        find_context_arrays full=        1  context-aware=        1
    json-standalone/objects/find_context_arrays
                            time:   [49.625 µs 51.039 µs 52.391 µs]
                            thrpt:  [25.894 GiB/s 26.580 GiB/s 27.338 GiB/s]
    json-standalone/objects/find_context_objects_cold
                            time:   [2.0597 ms 2.0608 ms 2.0624 ms]
                            thrpt:  [673.58 MiB/s 674.11 MiB/s 674.48 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    json-standalone/objects/find_context_arrays_cold
                            time:   [1.7103 ms 1.7131 ms 1.7180 ms]
                            thrpt:  [808.61 MiB/s 810.90 MiB/s 812.25 MiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) high mild
      1 (5.00%) high severe
    ```
</details>

<details>
    <summary>nested full log</summary>

    ```
    ┌─ corpus: nested
    │  size:            1.13 MiB  (1184451 bytes)
    │  structural:    290001     (250.7 per KiB)
    │
    │      objects:     50000      arrays:     10001     strings:    130000
    │      members:    100000     scalars:         0       loose:         1
    │  typed: nums:     40000       trues:     10000      falses:         0     nulls:         0
    └─
    │  full-vs-standalone counts:
        find_objects   full=    50000  standalone=    20000
    json-standalone/nested/find_objects
                            time:   [284.36 µs 284.77 µs 285.22 µs]
                            thrpt:  [3.8675 GiB/s 3.8737 GiB/s 3.8793 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_arrays    full=    10001  standalone=    10000
    json-standalone/nested/find_arrays
                            time:   [149.93 µs 150.14 µs 150.29 µs]
                            thrpt:  [7.3397 GiB/s 7.3472 GiB/s 7.3574 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_strings   full=   130000  standalone=   130000
    json-standalone/nested/find_strings
                            time:   [1.6220 ms 1.6234 ms 1.6255 ms]
                            thrpt:  [694.92 MiB/s 695.82 MiB/s 696.40 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_members   full=   100000  standalone=    60000
    json-standalone/nested/find_members
                            time:   [1.0453 ms 1.0464 ms 1.0477 ms]
                            thrpt:  [1.0529 GiB/s 1.0542 GiB/s 1.0553 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    │  context regions (strings): 130000
    json-standalone/nested/context
                            time:   [1.8025 ms 1.8042 ms 1.8060 ms]
                            thrpt:  [625.46 MiB/s 626.07 MiB/s 626.66 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    │  full-vs-context-aware counts:
        find_context_objects full=    50000  context-aware=    20000
    json-standalone/nested/find_context_objects
                            time:   [353.35 µs 353.65 µs 353.94 µs]
                            thrpt:  [3.1167 GiB/s 3.1192 GiB/s 3.1218 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_context_arrays full=    10001  context-aware=    10000
    json-standalone/nested/find_context_arrays
                            time:   [213.56 µs 213.84 µs 214.14 µs]
                            thrpt:  [5.1513 GiB/s 5.1586 GiB/s 5.1654 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    json-standalone/nested/find_context_objects_cold
                            time:   [2.1535 ms 2.1584 ms 2.1660 ms]
                            thrpt:  [521.51 MiB/s 523.33 MiB/s 524.52 MiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      3 (15.00%) high mild
      1 (5.00%) high severe
    json-standalone/nested/find_context_arrays_cold
                            time:   [2.0128 ms 2.0145 ms 2.0165 ms]
                            thrpt:  [560.17 MiB/s 560.73 MiB/s 561.20 MiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      3 (15.00%) high mild
    ```
</details>

**big:**

#### Standalone - `find_*`
| Function       | `numbers`                  | `objects`                  | `nested`                   |
|----------------|----------------------------|----------------------------|----------------------------|
| `find_objects` | [9.0211 ms]=[23.636 GiB/s] | [30.069 ms]=[4.8834 GiB/s] | [29.286 ms]=[4.0846 GiB/s] |
| `find_arrays`  | [9.5245 ms]=[22.387 GiB/s] | [6.4081 ms]=[22.915 GiB/s] | [15.210 ms]=[7.8649 GiB/s] |
| `find_strings` | [8.6628 ms]=[24.614 GiB/s] | [150.52 ms]=[998.98 MiB/s] | [162.92 ms]=[751.89 MiB/s] |
| `find_members` | [8.6089 ms]=[24.768 GiB/s] | [169.13 ms]=[889.01 MiB/s] | [106.00 ms]=[1.1285 GiB/s] |

#### Context-aware - `context()` + `find_context_*`
| Function               | `numbers`                  | `objects`                  | `nested`                   |
|------------------------|----------------------------|----------------------------|----------------------------|
| `context`              | [8.8003 ms]=[24.229 GiB/s] | [210.39 ms]=[714.70 MiB/s] | [228.78 ms]=[535.42 MiB/s] |
| `find_context_objects` | [8.6965 ms]=[24.518 GiB/s] | [41.394 ms]=[3.5473 GiB/s] | [36.907 ms]=[3.2412 GiB/s] |
| `find_context_arrays`  | [9.5862 ms]=[22.243 GiB/s] | [11.522 ms]=[12.744 GiB/s] | [22.408 ms]=[5.3384 GiB/s] |

#### Cold context - `find_context_*_cold`
| Function                    | `numbers`                  | `objects`                  | `nested`                   |
|-----------------------------|----------------------------|----------------------------|----------------------------|
| `find_context_objects_cold` | [17.314 ms]=[12.315 GiB/s] | [252.70 ms]=[595.03 MiB/s] | [265.47 ms]=[461.43 MiB/s] |
| `find_context_arrays_cold`  | [18.483 ms]=[11.536 GiB/s] | [223.86 ms]=[671.68 MiB/s] | [250.18 ms]=[489.63 MiB/s] |

<details>
    <summary>numbers full log</summary>

    ```
    ┌─ corpus: numbers
    │  size:          218.34 MiB  (228944439 bytes)
    │  structural:         1     (0.0 per KiB)
    │
    │      objects:         0      arrays:         1     strings:         0
    │      members:         0     scalars:         0       loose:         1
    │  typed: nums:  15000000       trues:   5000000      falses:   5000000     nulls:   5000000
    └─
    │  full-vs-standalone counts:
        find_objects   full=        0  standalone=        0
    json-standalone/numbers/find_objects
                            time:   [8.8112 ms 9.0211 ms 9.3552 ms]
                            thrpt:  [22.792 GiB/s 23.636 GiB/s 24.199 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_arrays    full=        1  standalone=        1
    json-standalone/numbers/find_arrays
                            time:   [9.5121 ms 9.5245 ms 9.5455 ms]
                            thrpt:  [22.337 GiB/s 22.387 GiB/s 22.416 GiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      1 (5.00%) high mild
      3 (15.00%) high severe
        find_strings   full=        0  standalone=        0
    json-standalone/numbers/find_strings
                            time:   [8.6246 ms 8.6628 ms 8.6961 ms]
                            thrpt:  [24.519 GiB/s 24.614 GiB/s 24.722 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_members   full=        0  standalone=        0
    json-standalone/numbers/find_members
                            time:   [8.5686 ms 8.6089 ms 8.6706 ms]
                            thrpt:  [24.591 GiB/s 24.768 GiB/s 24.884 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    │  context regions (strings): 0
    json-standalone/numbers/context
                            time:   [8.7605 ms 8.8003 ms 8.8335 ms]
                            thrpt:  [24.138 GiB/s 24.229 GiB/s 24.339 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    │  full-vs-context-aware counts:
        find_context_objects full=        0  context-aware=        0
    json-standalone/numbers/find_context_objects
                            time:   [8.6467 ms 8.6965 ms 8.7596 ms]
                            thrpt:  [24.341 GiB/s 24.518 GiB/s 24.659 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_context_arrays full=        1  context-aware=        1
    json-standalone/numbers/find_context_arrays
                            time:   [9.5565 ms 9.5862 ms 9.6243 ms]
                            thrpt:  [22.154 GiB/s 22.243 GiB/s 22.312 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    json-standalone/numbers/find_context_objects_cold
                            time:   [17.249 ms 17.314 ms 17.393 ms]
                            thrpt:  [12.259 GiB/s 12.315 GiB/s 12.361 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    json-standalone/numbers/find_context_arrays_cold
                            time:   [18.374 ms 18.483 ms 18.551 ms]
                            thrpt:  [11.494 GiB/s 11.536 GiB/s 11.604 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    ```
</details>

<details>
    <summary>objects full log</summary>

    ```
    ┌─ corpus: objects
    │  size:          150.36 MiB  (157666671 bytes)
    │  structural:  24000001     (155.9 per KiB)
    │
    │      objects:   2000000      arrays:         1     strings:  12000000
    │      members:  10000000     scalars:         0       loose:         1
    │  typed: nums:   4000000       trues:   1000000      falses:   1000000     nulls:   2000000
    └─
    │  full-vs-standalone counts:
        find_objects   full=  2000000  standalone=  2000000
    json-standalone/objects/find_objects
                            time:   [30.037 ms 30.069 ms 30.102 ms]
                            thrpt:  [4.8780 GiB/s 4.8834 GiB/s 4.8886 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_arrays    full=        1  standalone=        1
    json-standalone/objects/find_arrays
                            time:   [6.3972 ms 6.4081 ms 6.4252 ms]
                            thrpt:  [22.854 GiB/s 22.915 GiB/s 22.953 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_strings   full= 12000000  standalone= 12000000
    json-standalone/objects/find_strings
                            time:   [150.29 ms 150.52 ms 150.84 ms]
                            thrpt:  [996.85 MiB/s 998.98 MiB/s 1000.5 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_members   full= 10000000  standalone= 10000000
    json-standalone/objects/find_members
                            time:   [169.03 ms 169.13 ms 169.25 ms]
                            thrpt:  [888.43 MiB/s 889.01 MiB/s 889.55 MiB/s]
    │  context regions (strings): 12000000
    json-standalone/objects/context
                            time:   [210.09 ms 210.39 ms 210.70 ms]
                            thrpt:  [713.62 MiB/s 714.70 MiB/s 715.71 MiB/s]
    │  full-vs-context-aware counts:
        find_context_objects full=  2000000  context-aware=  2000000
    json-standalone/objects/find_context_objects
                            time:   [41.356 ms 41.394 ms 41.438 ms]
                            thrpt:  [3.5436 GiB/s 3.5473 GiB/s 3.5506 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) high mild
      1 (5.00%) high severe
        find_context_arrays full=        1  context-aware=        1
    json-standalone/objects/find_context_arrays
                            time:   [11.498 ms 11.522 ms 11.546 ms]
                            thrpt:  [12.717 GiB/s 12.744 GiB/s 12.770 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    json-standalone/objects/find_context_objects_cold
                            time:   [252.38 ms 252.70 ms 253.13 ms]
                            thrpt:  [594.02 MiB/s 595.03 MiB/s 595.78 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    json-standalone/objects/find_context_arrays_cold
                            time:   [223.60 ms 223.86 ms 224.09 ms]
                            thrpt:  [670.99 MiB/s 671.68 MiB/s 672.45 MiB/s]
    ```
</details>

<details>
    <summary>nested full log</summary>

    ```
    ┌─ corpus: nested
    │  size:          122.49 MiB  (128444451 bytes)
    │  structural:  29000001     (231.2 per KiB)
    │
    │      objects:   5000000      arrays:   1000001     strings:  13000000
    │      members:  10000000     scalars:         0       loose:         1
    │  typed: nums:   4000000       trues:   1000000      falses:         0     nulls:         0
    └─
    │  full-vs-standalone counts:
        find_objects   full=  5000000  standalone=  2000000
    json-standalone/nested/find_objects
                            time:   [29.242 ms 29.286 ms 29.345 ms]
                            thrpt:  [4.0764 GiB/s 4.0846 GiB/s 4.0908 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_arrays    full=  1000001  standalone=  1000000
    json-standalone/nested/find_arrays
                            time:   [15.195 ms 15.210 ms 15.225 ms]
                            thrpt:  [7.8569 GiB/s 7.8649 GiB/s 7.8723 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_strings   full= 13000000  standalone= 13000000
    json-standalone/nested/find_strings
                            time:   [162.75 ms 162.92 ms 163.09 ms]
                            thrpt:  [751.07 MiB/s 751.89 MiB/s 752.64 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_members   full= 10000000  standalone=  6000000
    json-standalone/nested/find_members
                            time:   [105.94 ms 106.00 ms 106.08 ms]
                            thrpt:  [1.1276 GiB/s 1.1285 GiB/s 1.1292 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
    │  context regions (strings): 13000000
    json-standalone/nested/context
                            time:   [228.46 ms 228.78 ms 229.13 ms]
                            thrpt:  [534.61 MiB/s 535.42 MiB/s 536.17 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    │  full-vs-context-aware counts:
        find_context_objects full=  5000000  context-aware=  2000000
    json-standalone/nested/find_context_objects
                            time:   [36.886 ms 36.907 ms 36.931 ms]
                            thrpt:  [3.2391 GiB/s 3.2412 GiB/s 3.2430 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low mild
      1 (5.00%) high severe
        find_context_arrays full=  1000001  context-aware=  1000000
    json-standalone/nested/find_context_arrays
                            time:   [22.254 ms 22.408 ms 22.517 ms]
                            thrpt:  [5.3126 GiB/s 5.3384 GiB/s 5.3754 GiB/s]
    json-standalone/nested/find_context_objects_cold
                            time:   [265.10 ms 265.47 ms 265.91 ms]
                            thrpt:  [460.66 MiB/s 461.43 MiB/s 462.07 MiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) high mild
      1 (5.00%) high severe
    json-standalone/nested/find_context_arrays_cold
                            time:   [249.76 ms 250.18 ms 250.63 ms]
                            thrpt:  [488.75 MiB/s 489.63 MiB/s 490.45 MiB/s]
    ```
</details>

---

## meon-json context-aware extraction (`context()` + `find_context_*`)

For JSON the opaque rule is the string: `context(source)` maps every string
region in one streaming pass, and the `find_context_*` variants skip candidate
delimiters inside them — a `{` inside a string value no longer counts as an
object open. Strings themselves keep only their context-free `find_strings`
(they *are* the context source). Note the scope: the context closes the
string-opacity divergence, not the nesting-insensitivity — a `find_context_*`
scan still matches literal delimiters without tracking depth (see
[`ARCHITECTURE.md §12`](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE.md#12-standalone-iterators)).

Three groups per corpus:

- `context` — building the `ParseContext` alone. The map is built once per
  source and shared by every `find_context_*` over it, so this cost amortises
  across element kinds.
- `find_context_*` — the scan against a prebuilt map, with `full` vs
  `context-aware` counts alongside.
- `find_context_*_cold` — map build plus scan in a single call: the one-shot
  price when no map is reused.

---

## Reading the numbers

- A higher meon number reflects the job it does — span vectors, no validation,
  no number parsing, no unescaping. A consumer that needs typed values or
  decoded strings does that work on top of meon's spans.
- Compare a cell only against the same corpus in the same build block.
- `simd-json` and `sonic-rs` produce usable values directly; meon produces spans
  you project from. The gap is the cost of that materialisation, which your
  workload may or may not need.
- The `numbers` corpus shows the widest gap by construction; `objects` and
  `nested` are closer to a mixed real payload. Scale matters more than the
  small-input headline — see [Scaling from small to big](#scaling-from-small-to-big).
