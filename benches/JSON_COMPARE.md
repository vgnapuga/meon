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

### Cold and warmed scenarios

`meon-json_parse` is the **isolated** scenario: only `meon-json` is timed, and
no cross-parser validation gate or other parser's timed benchmark runs before
it. The shared composition report still runs before timing.

`meon-json_compare` is the **warmed** scenario: all four parsers run in the same
process, and a one-time validation gate runs `simd-json` and `sonic-rs` before
timing. The allocator, pages and caches are therefore shared and already warm.
`simd-json` and `sonic-rs` are measured under the same warmed conditions as the
two `meon-json` lines, so the four-way comparison inside `_compare` is
internally consistent.

Absolute numbers from `_parse` and `_compare` answer different questions and are
not interchangeable.

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

Each corpus is one valid top-level JSON array, scaled by `REPEAT_COUNT`
(`benches/benches/docs_json.rs`). The `small` and `big` runs differ only in
`REPEAT_COUNT`, selected by the `small` / `big` Cargo features of the
`benches` package.

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
| `numbers` | [2.2451 ms]=[845.08 MiB/s] | [4.9663 ms]=[382.03 MiB/s] | [5.7800 ms]=[328.25 MiB/s] | [2.7076 ms]=[700.71 MiB/s] |
| `objects` | [3.4378 ms]=[404.09 MiB/s] | [4.9514 ms]=[280.57 MiB/s] | [2.4674 ms]=[563.02 MiB/s] | [1.7689 ms]=[785.34 MiB/s] |
| `nested`  | [4.3160 ms]=[261.72 MiB/s] | [5.7318 ms]=[197.07 MiB/s] | [2.5757 ms]=[438.56 MiB/s] | [2.2734 ms]=[496.87 MiB/s] |

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
                            time:   [2.2314 ms 2.2451 ms 2.2588 ms]
                            thrpt:  [839.96 MiB/s 845.08 MiB/s 850.28 MiB/s]
    json-compare/numbers/meon-typed
                            time:   [4.9397 ms 4.9663 ms 5.0099 ms]
                            thrpt:  [378.71 MiB/s 382.03 MiB/s 384.09 MiB/s]
    json-compare/numbers/simd-json
                            time:   [5.4272 ms 5.7800 ms 6.3323 ms]
                            thrpt:  [299.62 MiB/s 328.25 MiB/s 349.59 MiB/s]
    json-compare/numbers/sonic-rs
                            time:   [2.7033 ms 2.7076 ms 2.7127 ms]
                            thrpt:  [699.42 MiB/s 700.71 MiB/s 701.85 MiB/s]
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
                            time:   [3.4362 ms 3.4378 ms 3.4395 ms]
                            thrpt:  [403.89 MiB/s 404.09 MiB/s 404.28 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    json-compare/objects/meon-typed
                            time:   [4.9492 ms 4.9514 ms 4.9544 ms]
                            thrpt:  [280.40 MiB/s 280.57 MiB/s 280.69 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    json-compare/objects/simd-json
                            time:   [2.4337 ms 2.4674 ms 2.5289 ms]
                            thrpt:  [549.32 MiB/s 563.02 MiB/s 570.80 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    json-compare/objects/sonic-rs
                            time:   [1.7643 ms 1.7689 ms 1.7732 ms]
                            thrpt:  [783.43 MiB/s 785.34 MiB/s 787.39 MiB/s]
    
    ┌─ corpus: nested
    │  size:            1.13 MiB  (1184451 bytes)
    │  structural:    290001     (250.7 per KiB)
    │
    │      objects:     50000      arrays:     10001     strings:    130000
    │      members:    100000     scalars:         0       loose:         1
    │  typed: nums:     40000       trues:     10000      falses:         0     nulls:         0
    └─
    json-compare/nested/meon-structural
                            time:   [4.2942 ms 4.3160 ms 4.3420 ms]
                            thrpt:  [260.15 MiB/s 261.72 MiB/s 263.05 MiB/s]
    json-compare/nested/meon-typed
                            time:   [5.7214 ms 5.7318 ms 5.7458 ms]
                            thrpt:  [196.59 MiB/s 197.07 MiB/s 197.43 MiB/s]
    json-compare/nested/simd-json
                            time:   [2.5415 ms 2.5757 ms 2.6372 ms]
                            thrpt:  [428.33 MiB/s 438.56 MiB/s 444.46 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    json-compare/nested/sonic-rs
                            time:   [2.2691 ms 2.2734 ms 2.2774 ms]
                            thrpt:  [495.99 MiB/s 496.87 MiB/s 497.81 MiB/s]
    ```

</details>

**big:**

| Corpus    | `meon-structural`          | `meon-typed`               | `simd-json`                | `sonic-rs`                 |
|-----------|----------------------------|----------------------------|----------------------------|----------------------------|
| `numbers` | [238.84 ms]=[914.14 MiB/s] | [667.88 ms]=[326.91 MiB/s] | [963.20 ms]=[226.68 MiB/s] | [925.72 ms]=[235.86 MiB/s] |
| `objects` | [505.02 ms]=[297.73 MiB/s] | [712.92 ms]=[210.91 MiB/s] | [720.11 ms]=[208.80 MiB/s] | [487.11 ms]=[308.68 MiB/s] |
| `nested`  | [637.12 ms]=[192.26 MiB/s] | [812.96 ms]=[150.68 MiB/s] | [762.81 ms]=[160.58 MiB/s] | [569.69 ms]=[215.02 MiB/s] |

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
                            time:   [238.77 ms 238.84 ms 238.93 ms]
                            thrpt:  [913.82 MiB/s 914.14 MiB/s 914.44 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    Benchmarking json-compare/numbers/meon-typed: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 13.2s, or reduce sample count to 10.
    json-compare/numbers/meon-typed
                            time:   [666.79 ms 667.88 ms 669.02 ms]
                            thrpt:  [326.35 MiB/s 326.91 MiB/s 327.45 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low severe
      1 (5.00%) high severe
    Benchmarking json-compare/numbers/simd-json: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 22.1s, or reduce sample count to 10.
    json-compare/numbers/simd-json
                            time:   [961.50 ms 963.20 ms 965.30 ms]
                            thrpt:  [226.19 MiB/s 226.68 MiB/s 227.08 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    Benchmarking json-compare/numbers/sonic-rs: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 18.6s, or reduce sample count to 10.
    json-compare/numbers/sonic-rs
                            time:   [924.15 ms 925.72 ms 927.99 ms]
                            thrpt:  [235.28 MiB/s 235.86 MiB/s 236.26 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
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
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 10.1s, or reduce sample count to 10.
    json-compare/objects/meon-structural
                            time:   [502.91 ms 505.02 ms 507.75 ms]
                            thrpt:  [296.14 MiB/s 297.73 MiB/s 298.98 MiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      1 (5.00%) low severe
      1 (5.00%) high mild
      2 (10.00%) high severe
    Benchmarking json-compare/objects/meon-typed: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 14.2s, or reduce sample count to 10.
    json-compare/objects/meon-typed
                            time:   [711.61 ms 712.92 ms 714.93 ms]
                            thrpt:  [210.32 MiB/s 210.91 MiB/s 211.30 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    Benchmarking json-compare/objects/simd-json: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 16.1s, or reduce sample count to 10.
    json-compare/objects/simd-json
                            time:   [718.49 ms 720.11 ms 722.00 ms]
                            thrpt:  [208.26 MiB/s 208.80 MiB/s 209.28 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low mild
      1 (5.00%) high severe
    json-compare/objects/sonic-rs
                            time:   [486.29 ms 487.11 ms 487.98 ms]
                            thrpt:  [308.13 MiB/s 308.68 MiB/s 309.20 MiB/s]
    
    ┌─ corpus: nested
    │  size:          122.49 MiB  (128444451 bytes)
    │  structural:  29000001     (231.2 per KiB)
    │
    │      objects:   5000000      arrays:   1000001     strings:  13000000
    │      members:  10000000     scalars:         0       loose:         1
    │  typed: nums:   4000000       trues:   1000000      falses:         0     nulls:         0
    └─
    Benchmarking json-compare/nested/meon-structural: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 12.4s, or reduce sample count to 10.
    json-compare/nested/meon-structural
                            time:   [632.91 ms 637.12 ms 641.38 ms]
                            thrpt:  [190.99 MiB/s 192.26 MiB/s 193.54 MiB/s]
    Benchmarking json-compare/nested/meon-typed: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 16.5s, or reduce sample count to 10.
    json-compare/nested/meon-typed
                            time:   [811.09 ms 812.96 ms 815.06 ms]
                            thrpt:  [150.29 MiB/s 150.68 MiB/s 151.02 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    Benchmarking json-compare/nested/simd-json: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 16.6s, or reduce sample count to 10.
    json-compare/nested/simd-json
                            time:   [761.05 ms 762.81 ms 765.56 ms]
                            thrpt:  [160.01 MiB/s 160.58 MiB/s 160.95 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low mild
      1 (5.00%) high severe
    Benchmarking json-compare/nested/sonic-rs: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 11.5s, or reduce sample count to 10.
    json-compare/nested/sonic-rs
                            time:   [567.70 ms 569.69 ms 571.83 ms]
                            thrpt:  [214.21 MiB/s 215.02 MiB/s 215.77 MiB/s]
    ```

</details>

### nightly - `RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-json_compare --features avx2`

> meon on AVX2; `simd-json` / `sonic-rs` on their own runtime SIMD.

**small:**

| Corpus    | `meon-structural`          | `meon-typed`               | `simd-json`                | `sonic-rs`                 |
|-----------|----------------------------|----------------------------|----------------------------|----------------------------|
| `numbers` | [2.0481 ms]=[926.34 MiB/s] | [4.4171 ms]=[429.53 MiB/s] | [5.4591 ms]=[347.55 MiB/s] | [2.7974 ms]=[678.23 MiB/s] |
| `objects` | [3.2179 ms]=[431.71 MiB/s] | [4.4993 ms]=[308.76 MiB/s] | [2.4126 ms]=[575.82 MiB/s] | [1.6667 ms]=[833.50 MiB/s] |
| `nested`  | [4.1345 ms]=[273.21 MiB/s] | [5.3322 ms]=[211.84 MiB/s] | [2.4784 ms]=[455.77 MiB/s] | [2.1685 ms]=[520.92 MiB/s] |

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
                            time:   [2.0470 ms 2.0481 ms 2.0494 ms]
                            thrpt:  [925.77 MiB/s 926.34 MiB/s 926.87 MiB/s]
    json-compare/numbers/meon-typed
                            time:   [4.3996 ms 4.4171 ms 4.4374 ms]
                            thrpt:  [427.57 MiB/s 429.53 MiB/s 431.24 MiB/s]
    json-compare/numbers/simd-json
                            time:   [5.1836 ms 5.4591 ms 5.9255 ms]
                            thrpt:  [320.19 MiB/s 347.55 MiB/s 366.02 MiB/s]
    json-compare/numbers/sonic-rs
                            time:   [2.7898 ms 2.7974 ms 2.8074 ms]
                            thrpt:  [675.82 MiB/s 678.23 MiB/s 680.09 MiB/s]
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
                            time:   [3.2117 ms 3.2179 ms 3.2231 ms]
                            thrpt:  [431.01 MiB/s 431.71 MiB/s 432.54 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    json-compare/objects/meon-typed
                            time:   [4.4809 ms 4.4993 ms 4.5240 ms]
                            thrpt:  [307.07 MiB/s 308.76 MiB/s 310.02 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    json-compare/objects/simd-json
                            time:   [2.3781 ms 2.4126 ms 2.4730 ms]
                            thrpt:  [561.75 MiB/s 575.82 MiB/s 584.15 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    json-compare/objects/sonic-rs
                            time:   [1.6607 ms 1.6667 ms 1.6708 ms]
                            thrpt:  [831.43 MiB/s 833.50 MiB/s 836.52 MiB/s]
    
    ┌─ corpus: nested
    │  size:            1.13 MiB  (1184451 bytes)
    │  structural:    290001     (250.7 per KiB)
    │
    │      objects:     50000      arrays:     10001     strings:    130000
    │      members:    100000     scalars:         0       loose:         1
    │  typed: nums:     40000       trues:     10000      falses:         0     nulls:         0
    └─
    json-compare/nested/meon-structural
                            time:   [4.1321 ms 4.1345 ms 4.1372 ms]
                            thrpt:  [273.03 MiB/s 273.21 MiB/s 273.37 MiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      3 (15.00%) high severe
    json-compare/nested/meon-typed
                            time:   [5.3142 ms 5.3322 ms 5.3475 ms]
                            thrpt:  [211.23 MiB/s 211.84 MiB/s 212.56 MiB/s]
    json-compare/nested/simd-json
                            time:   [2.4441 ms 2.4784 ms 2.5398 ms]
                            thrpt:  [444.75 MiB/s 455.77 MiB/s 462.17 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    json-compare/nested/sonic-rs
                            time:   [2.1677 ms 2.1685 ms 2.1690 ms]
                            thrpt:  [520.78 MiB/s 520.92 MiB/s 521.09 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    ```

</details>

**big:**

| Corpus    | `meon-structural`          | `meon-typed`               | `simd-json`                | `sonic-rs`                 |
|-----------|----------------------------|----------------------------|----------------------------|----------------------------|
| `numbers` | [212.00 ms]=[1.0058 GiB/s] | [585.34 ms]=[373.01 MiB/s] | [936.86 ms]=[233.05 MiB/s] | [954.75 ms]=[228.69 MiB/s] |
| `objects` | [497.92 ms]=[301.98 MiB/s] | [666.11 ms]=[225.73 MiB/s] | [708.57 ms]=[212.21 MiB/s] | [486.62 ms]=[308.99 MiB/s] |
| `nested`  | [615.67 ms]=[198.96 MiB/s] | [752.93 ms]=[162.69 MiB/s] | [744.03 ms]=[164.64 MiB/s] | [562.11 ms]=[217.92 MiB/s] |

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
                            time:   [211.91 ms 212.00 ms 212.11 ms]
                            thrpt:  [1.0053 GiB/s 1.0058 GiB/s 1.0062 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    Benchmarking json-compare/numbers/meon-typed: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 11.7s, or reduce sample count to 10.
    json-compare/numbers/meon-typed
                            time:   [584.37 ms 585.34 ms 586.43 ms]
                            thrpt:  [372.32 MiB/s 373.01 MiB/s 373.63 MiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low mild
      2 (10.00%) high severe
    Benchmarking json-compare/numbers/simd-json: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 21.4s, or reduce sample count to 10.
    json-compare/numbers/simd-json
                            time:   [935.75 ms 936.86 ms 938.07 ms]
                            thrpt:  [232.75 MiB/s 233.05 MiB/s 233.33 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    Benchmarking json-compare/numbers/sonic-rs: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 18.7s, or reduce sample count to 10.
    json-compare/numbers/sonic-rs
                            time:   [944.10 ms 954.75 ms 967.92 ms]
                            thrpt:  [225.57 MiB/s 228.69 MiB/s 231.27 MiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      4 (20.00%) high severe
    
    ┌─ corpus: objects
    │  size:          150.36 MiB  (157666671 bytes)
    │  structural:  24000001     (155.9 per KiB)
    │
    │      objects:   2000000      arrays:         1     strings:  12000000
    │      members:  10000000     scalars:         0       loose:         1
    │  typed: nums:   4000000       trues:   1000000      falses:   1000000     nulls:   2000000
    └─
    json-compare/objects/meon-structural
                            time:   [494.70 ms 497.92 ms 500.89 ms]
                            thrpt:  [300.19 MiB/s 301.98 MiB/s 303.95 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
    Benchmarking json-compare/objects/meon-typed: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 13.4s, or reduce sample count to 10.
    json-compare/objects/meon-typed
                            time:   [664.81 ms 666.11 ms 667.98 ms]
                            thrpt:  [225.10 MiB/s 225.73 MiB/s 226.17 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    Benchmarking json-compare/objects/simd-json: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 15.7s, or reduce sample count to 10.
    json-compare/objects/simd-json
                            time:   [707.05 ms 708.57 ms 710.88 ms]
                            thrpt:  [211.52 MiB/s 212.21 MiB/s 212.66 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    json-compare/objects/sonic-rs
                            time:   [485.88 ms 486.62 ms 487.32 ms]
                            thrpt:  [308.55 MiB/s 308.99 MiB/s 309.47 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
    
    ┌─ corpus: nested
    │  size:          122.49 MiB  (128444451 bytes)
    │  structural:  29000001     (231.2 per KiB)
    │
    │      objects:   5000000      arrays:   1000001     strings:  13000000
    │      members:  10000000     scalars:         0       loose:         1
    │  typed: nums:   4000000       trues:   1000000      falses:         0     nulls:         0
    └─
    Benchmarking json-compare/nested/meon-structural: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 12.0s, or reduce sample count to 10.
    json-compare/nested/meon-structural
                            time:   [611.71 ms 615.67 ms 619.84 ms]
                            thrpt:  [197.62 MiB/s 198.96 MiB/s 200.25 MiB/s]
    Benchmarking json-compare/nested/meon-typed: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 15.0s, or reduce sample count to 10.
    json-compare/nested/meon-typed
                            time:   [752.44 ms 752.93 ms 753.45 ms]
                            thrpt:  [162.58 MiB/s 162.69 MiB/s 162.80 MiB/s]
    Benchmarking json-compare/nested/simd-json: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 16.2s, or reduce sample count to 10.
    json-compare/nested/simd-json
                            time:   [743.49 ms 744.03 ms 744.62 ms]
                            thrpt:  [164.51 MiB/s 164.64 MiB/s 164.76 MiB/s]
    Benchmarking json-compare/nested/sonic-rs: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 11.3s, or reduce sample count to 10.
    json-compare/nested/sonic-rs
                            time:   [561.74 ms 562.11 ms 562.54 ms]
                            thrpt:  [217.75 MiB/s 217.92 MiB/s 218.06 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    ```
    
</details>

---

## Scaling from small to big

How each parser holds up as the input grows past cache (median `thrpt`, MiB/s;
`simd-json` / `sonic-rs` from the stable run):

| Parser                      | `numbers`     | `objects`     | `nested`      |
|-----------------------------|---------------|---------------|---------------|
| `meon-structural` - stable  | 845 -> 914    | 404 -> 298    | 262 -> 192    |
| `meon-structural` - nightly | 926 -> 1030   | 432 -> 302    | 273 -> 199    |
| `meon-typed` - stable       | 382 -> 327    | 281 -> 211    | 197 -> 151    |
| `meon-typed` - nightly      | 430 -> 373    | 309 -> 226    | 212 -> 163    |
| `simd-json`                 | 328 -> 227    | 563 -> 209    | 439 -> 161    |
| `sonic-rs`                  | 701 -> 236    | 785 -> 309    | 497 -> 215    |

- **meon degrades little with scale.** `meon-structural` gains on `numbers`
  (845 -> 914 stable, 926 -> 1030 nightly) and loses about a quarter on
  `objects` / `nested` (-26% / -27% stable, -30% / -27% nightly). `meon-typed`
  loses 13-27%. The flat span table stays largely cache-resident.
- **The validating parsers lose more at big.** `simd-json` loses 63% on
  `objects`, 63% on `nested` and 31% on `numbers`. `sonic-rs` loses 57-66% on
  every corpus, most on `numbers`. Both materialise a tape / an owned `Value`,
  and that working set outgrows cache as the document grows.
- **The gap narrows with scale.** At small, `simd-json` and `sonic-rs` lead
  `meon-structural` on `objects` / `nested` by 1.4-1.9x. At big,
  `meon-structural` overtakes `simd-json` (298 vs 209 on `objects`, 192 vs 161
  on `nested`), while `sonic-rs` keeps a smaller lead (309 vs 298, 215 vs 192).
  On `numbers` meon leads at every scale, from 1.2x over `sonic-rs` at small to
  about 4x at big.
- **AVX2 helps meon-json slightly.** The nightly AVX2 rows follow the same
  scaling shape and are 1-13% faster than the stable SWAR rows; the largest gain
  is on `numbers` (9.6% at small, 12.7% at big).

---

## meon-json standalone extraction (no comparator equivalent)

`find_*` scans the raw source for **one** element kind only — e.g. every string
— with no cross-element context. `simd-json` and `sonic-rs` have no equivalent:
pulling just the strings from them means materialising the whole tape / owned
`Value` first. The numbers below are meon-only; they are here because
single-kind extraction is part of the architecture difference this document is
about.

Each line reports `full` vs `standalone` counts. For JSON the remaining
divergence comes from rules that do not see other rules: `find_objects` and
`find_arrays` track their own bracket nesting, but a bracket inside a string
is still a bracket to them, and `find_members` does not see surrounding
containers, so nested members are not found. `find_strings` is exact (string
content has no nesting), which is why it is the recommended single-sweep use.
Use the full `parse` or the context-aware finders when containment matters
(see
[`ARCHITECTURE.md §12`](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE.md#12-standalone-iterators)).
Shown for both `small` and `big`.

### stable - `cargo bench --bench meon-json_standalone`

**small:**

#### Standalone - `find_*`
| Function       | `numbers`                  | `objects`                  | `nested`                   |
|----------------|----------------------------|----------------------------|----------------------------|
| `find_objects` | [19.362 µs]=[95.695 GiB/s] | [403.18 µs]=[3.3648 GiB/s] | [713.58 µs]=[1.5459 GiB/s] |
| `find_arrays`  | [46.791 µs]=[39.597 GiB/s] | [34.652 µs]=[39.150 GiB/s] | [219.59 µs]=[5.0236 GiB/s] |
| `find_strings` | [20.583 µs]=[90.016 GiB/s] | [1.9378 ms]=[716.89 MiB/s] | [2.1000 ms]=[537.90 MiB/s] |
| `find_members` | [19.660 µs]=[94.241 GiB/s] | [2.1997 ms]=[631.52 MiB/s] | [1.3472 ms]=[838.47 MiB/s] |

#### Context-aware - `context()` + `find_context_*`
| Function               | `numbers`                  | `objects`                  | `nested`                   |
|------------------------|----------------------------|----------------------------|----------------------------|
| `context`              | [19.331 µs]=[95.847 GiB/s] | [1.8295 ms]=[759.34 MiB/s] | [1.9634 ms]=[575.31 MiB/s] |
| `find_context_objects` | [23.509 µs]=[78.811 GiB/s] | [502.94 µs]=[2.6974 GiB/s] | [880.24 µs]=[1.2532 GiB/s] |
| `find_context_arrays`  | [45.866 µs]=[40.396 GiB/s] | [92.255 µs]=[14.705 GiB/s] | [317.43 µs]=[3.4751 GiB/s] |

#### Cold context - `find_context_*_cold`
| Function                    | `numbers`                  | `objects`                  | `nested`                   |
|-----------------------------|----------------------------|----------------------------|----------------------------|
| `find_context_objects_cold` | [41.155 µs]=[45.021 GiB/s] | [2.3494 ms]=[591.30 MiB/s] | [2.8607 ms]=[394.86 MiB/s] |
| `find_context_arrays_cold`  | [69.032 µs]=[26.840 GiB/s] | [1.9295 ms]=[719.98 MiB/s] | [2.2933 ms]=[492.56 MiB/s] |

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
                            time:   [19.264 µs 19.362 µs 19.432 µs]
                            thrpt:  [95.346 GiB/s 95.695 GiB/s 96.178 GiB/s]
        find_arrays    full=        1  standalone=        1
    json-standalone/numbers/find_arrays
                            time:   [45.641 µs 46.791 µs 47.704 µs]
                            thrpt:  [38.840 GiB/s 39.597 GiB/s 40.595 GiB/s]
        find_strings   full=        0  standalone=        0
    json-standalone/numbers/find_strings
                            time:   [20.033 µs 20.583 µs 21.215 µs]
                            thrpt:  [87.334 GiB/s 90.016 GiB/s 92.487 GiB/s]
        find_members   full=        0  standalone=        0
    json-standalone/numbers/find_members
                            time:   [19.390 µs 19.660 µs 19.830 µs]
                            thrpt:  [93.433 GiB/s 94.241 GiB/s 95.554 GiB/s]
    │  context regions (strings): 0
    json-standalone/numbers/context
                            time:   [19.199 µs 19.331 µs 19.411 µs]
                            thrpt:  [95.451 GiB/s 95.847 GiB/s 96.504 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low severe
      1 (5.00%) high mild
    │  full-vs-context-aware counts:
        find_context_objects full=        0  context-aware=        0
    json-standalone/numbers/find_context_objects
                            time:   [23.034 µs 23.509 µs 24.198 µs]
                            thrpt:  [76.569 GiB/s 78.811 GiB/s 80.437 GiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      4 (20.00%) low mild
        find_context_arrays full=        1  context-aware=        1
    json-standalone/numbers/find_context_arrays
                            time:   [45.105 µs 45.866 µs 46.605 µs]
                            thrpt:  [39.755 GiB/s 40.396 GiB/s 41.077 GiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      1 (5.00%) high mild
      3 (15.00%) high severe
    json-standalone/numbers/find_context_objects_cold
                            time:   [40.766 µs 41.155 µs 41.629 µs]
                            thrpt:  [44.508 GiB/s 45.021 GiB/s 45.450 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
    json-standalone/numbers/find_context_arrays_cold
                            time:   [66.738 µs 69.032 µs 71.607 µs]
                            thrpt:  [25.875 GiB/s 26.840 GiB/s 27.763 GiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      4 (20.00%) low severe
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
                            time:   [402.52 µs 403.18 µs 404.07 µs]
                            thrpt:  [3.3574 GiB/s 3.3648 GiB/s 3.3704 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_arrays    full=        1  standalone=        1
    json-standalone/objects/find_arrays
                            time:   [34.354 µs 34.652 µs 34.880 µs]
                            thrpt:  [38.894 GiB/s 39.150 GiB/s 39.490 GiB/s]
        find_strings   full=   120000  standalone=   120000
    json-standalone/objects/find_strings
                            time:   [1.9369 ms 1.9378 ms 1.9389 ms]
                            thrpt:  [716.48 MiB/s 716.89 MiB/s 717.21 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_members   full=   100000  standalone=   100000
    json-standalone/objects/find_members
                            time:   [2.1993 ms 2.1997 ms 2.2003 ms]
                            thrpt:  [631.38 MiB/s 631.52 MiB/s 631.65 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    │  context regions (strings): 120000
    json-standalone/objects/context
                            time:   [1.8229 ms 1.8295 ms 1.8369 ms]
                            thrpt:  [756.26 MiB/s 759.34 MiB/s 762.07 MiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) high mild
      1 (5.00%) high severe
    │  full-vs-context-aware counts:
        find_context_objects full=    20000  context-aware=    20000
    json-standalone/objects/find_context_objects
                            time:   [500.62 µs 502.94 µs 505.98 µs]
                            thrpt:  [2.6812 GiB/s 2.6974 GiB/s 2.7099 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_context_arrays full=        1  context-aware=        1
    json-standalone/objects/find_context_arrays
                            time:   [91.634 µs 92.255 µs 92.840 µs]
                            thrpt:  [14.613 GiB/s 14.705 GiB/s 14.805 GiB/s]
    json-standalone/objects/find_context_objects_cold
                            time:   [2.3484 ms 2.3494 ms 2.3506 ms]
                            thrpt:  [591.00 MiB/s 591.30 MiB/s 591.54 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    json-standalone/objects/find_context_arrays_cold
                            time:   [1.9286 ms 1.9295 ms 1.9305 ms]
                            thrpt:  [719.60 MiB/s 719.98 MiB/s 720.31 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
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
        find_objects   full=    50000  standalone=    50000
    json-standalone/nested/find_objects
                            time:   [711.94 µs 713.58 µs 715.10 µs]
                            thrpt:  [1.5426 GiB/s 1.5459 GiB/s 1.5494 GiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      3 (15.00%) low mild
      1 (5.00%) high mild
        find_arrays    full=    10001  standalone=    10001
    json-standalone/nested/find_arrays
                            time:   [219.42 µs 219.59 µs 219.78 µs]
                            thrpt:  [5.0190 GiB/s 5.0236 GiB/s 5.0274 GiB/s]
        find_strings   full=   130000  standalone=   130000
    json-standalone/nested/find_strings
                            time:   [2.0988 ms 2.1000 ms 2.1020 ms]
                            thrpt:  [537.38 MiB/s 537.90 MiB/s 538.22 MiB/s]
        find_members   full=   100000  standalone=    60000
    json-standalone/nested/find_members
                            time:   [1.3467 ms 1.3472 ms 1.3479 ms]
                            thrpt:  [838.04 MiB/s 838.47 MiB/s 838.79 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    │  context regions (strings): 130000
    json-standalone/nested/context
                            time:   [1.9625 ms 1.9634 ms 1.9642 ms]
                            thrpt:  [575.10 MiB/s 575.31 MiB/s 575.57 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    │  full-vs-context-aware counts:
        find_context_objects full=    50000  context-aware=    50000
    json-standalone/nested/find_context_objects
                            time:   [879.67 µs 880.24 µs 880.99 µs]
                            thrpt:  [1.2521 GiB/s 1.2532 GiB/s 1.2540 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_context_arrays full=    10001  context-aware=    10001
    json-standalone/nested/find_context_arrays
                            time:   [316.76 µs 317.43 µs 318.28 µs]
                            thrpt:  [3.4658 GiB/s 3.4751 GiB/s 3.4824 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    json-standalone/nested/find_context_objects_cold
                            time:   [2.8595 ms 2.8607 ms 2.8627 ms]
                            thrpt:  [394.58 MiB/s 394.86 MiB/s 395.03 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    json-standalone/nested/find_context_arrays_cold
                            time:   [2.2924 ms 2.2933 ms 2.2943 ms]
                            thrpt:  [492.35 MiB/s 492.56 MiB/s 492.74 MiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      4 (20.00%) high mild
    ```
</details>

**big:**

#### Standalone - `find_*`
| Function       | `numbers`                  | `objects`                  | `nested`                   |
|----------------|----------------------------|----------------------------|----------------------------|
| `find_objects` | [9.2588 ms]=[23.029 GiB/s] | [47.257 ms]=[3.1072 GiB/s] | [74.403 ms]=[1.6078 GiB/s] |
| `find_arrays`  | [19.561 ms]=[10.900 GiB/s] | [13.320 ms]=[11.024 GiB/s] | [27.099 ms]=[4.4143 GiB/s] |
| `find_strings` | [9.3437 ms]=[22.820 GiB/s] | [194.12 ms]=[774.60 MiB/s] | [210.41 ms]=[582.18 MiB/s] |
| `find_members` | [9.3444 ms]=[22.818 GiB/s] | [221.38 ms]=[679.20 MiB/s] | [136.43 ms]=[897.87 MiB/s] |

#### Context-aware - `context()` + `find_context_*`
| Function               | `numbers`                  | `objects`                  | `nested`                   |
|------------------------|----------------------------|----------------------------|----------------------------|
| `context`              | [9.3496 ms]=[22.805 GiB/s] | [224.15 ms]=[670.81 MiB/s] | [242.91 ms]=[504.28 MiB/s] |
| `find_context_objects` | [9.2924 ms]=[22.946 GiB/s] | [57.394 ms]=[2.5584 GiB/s] | [92.039 ms]=[1.2997 GiB/s] |
| `find_context_arrays`  | [19.562 ms]=[10.900 GiB/s] | [20.188 ms]=[7.2736 GiB/s] | [35.689 ms]=[3.3519 GiB/s] |

#### Cold context - `find_context_*_cold`
| Function                    | `numbers`                  | `objects`                  | `nested`                   |
|-----------------------------|----------------------------|----------------------------|----------------------------|
| `find_context_objects_cold` | [18.526 ms]=[11.510 GiB/s] | [283.60 ms]=[530.19 MiB/s] | [336.53 ms]=[363.99 MiB/s] |
| `find_context_arrays_cold`  | [28.571 ms]=[7.4628 GiB/s] | [246.37 ms]=[610.32 MiB/s] | [280.37 ms]=[436.90 MiB/s] |

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
                            time:   [9.2315 ms 9.2588 ms 9.2910 ms]
                            thrpt:  [22.949 GiB/s 23.029 GiB/s 23.097 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_arrays    full=        1  standalone=        1
    json-standalone/numbers/find_arrays
                            time:   [19.531 ms 19.561 ms 19.589 ms]
                            thrpt:  [10.884 GiB/s 10.900 GiB/s 10.917 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_strings   full=        0  standalone=        0
    json-standalone/numbers/find_strings
                            time:   [9.3247 ms 9.3437 ms 9.3641 ms]
                            thrpt:  [22.770 GiB/s 22.820 GiB/s 22.866 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_members   full=        0  standalone=        0
    json-standalone/numbers/find_members
                            time:   [9.3244 ms 9.3444 ms 9.3694 ms]
                            thrpt:  [22.757 GiB/s 22.818 GiB/s 22.867 GiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      2 (10.00%) low mild
      1 (5.00%) high mild
      1 (5.00%) high severe
    │  context regions (strings): 0
    json-standalone/numbers/context
                            time:   [9.3218 ms 9.3496 ms 9.3719 ms]
                            thrpt:  [22.751 GiB/s 22.805 GiB/s 22.873 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    │  full-vs-context-aware counts:
        find_context_objects full=        0  context-aware=        0
    json-standalone/numbers/find_context_objects
                            time:   [9.2798 ms 9.2924 ms 9.3032 ms]
                            thrpt:  [22.919 GiB/s 22.946 GiB/s 22.977 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low mild
      1 (5.00%) high severe
        find_context_arrays full=        1  context-aware=        1
    json-standalone/numbers/find_context_arrays
                            time:   [19.481 ms 19.562 ms 19.662 ms]
                            thrpt:  [10.844 GiB/s 10.900 GiB/s 10.945 GiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      1 (5.00%) low mild
      1 (5.00%) high mild
      2 (10.00%) high severe
    json-standalone/numbers/find_context_objects_cold
                            time:   [18.489 ms 18.526 ms 18.564 ms]
                            thrpt:  [11.486 GiB/s 11.510 GiB/s 11.532 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    json-standalone/numbers/find_context_arrays_cold
                            time:   [28.512 ms 28.571 ms 28.629 ms]
                            thrpt:  [7.4478 GiB/s 7.4628 GiB/s 7.4783 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
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
                            time:   [47.212 ms 47.257 ms 47.305 ms]
                            thrpt:  [3.1041 GiB/s 3.1072 GiB/s 3.1102 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_arrays    full=        1  standalone=        1
    json-standalone/objects/find_arrays
                            time:   [13.285 ms 13.320 ms 13.377 ms]
                            thrpt:  [10.977 GiB/s 11.024 GiB/s 11.053 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_strings   full= 12000000  standalone= 12000000
    json-standalone/objects/find_strings
                            time:   [194.01 ms 194.12 ms 194.23 ms]
                            thrpt:  [774.14 MiB/s 774.60 MiB/s 775.03 MiB/s]
        find_members   full= 10000000  standalone= 10000000
    json-standalone/objects/find_members
                            time:   [221.09 ms 221.38 ms 221.71 ms]
                            thrpt:  [678.21 MiB/s 679.20 MiB/s 680.09 MiB/s]
    │  context regions (strings): 12000000
    json-standalone/objects/context
                            time:   [223.78 ms 224.15 ms 224.52 ms]
                            thrpt:  [669.70 MiB/s 670.81 MiB/s 671.93 MiB/s]
    │  full-vs-context-aware counts:
        find_context_objects full=  2000000  context-aware=  2000000
    Benchmarking json-standalone/objects/find_context_objects: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 12.1s, enable flat sampling, or reduce sample count to 10.
    json-standalone/objects/find_context_objects
                            time:   [57.290 ms 57.394 ms 57.539 ms]
                            thrpt:  [2.5520 GiB/s 2.5584 GiB/s 2.5631 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
        find_context_arrays full=        1  context-aware=        1
    json-standalone/objects/find_context_arrays
                            time:   [20.153 ms 20.188 ms 20.217 ms]
                            thrpt:  [7.2632 GiB/s 7.2736 GiB/s 7.2862 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    json-standalone/objects/find_context_objects_cold
                            time:   [282.86 ms 283.60 ms 284.64 ms]
                            thrpt:  [528.26 MiB/s 530.19 MiB/s 531.58 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    json-standalone/objects/find_context_arrays_cold
                            time:   [245.80 ms 246.37 ms 246.99 ms]
                            thrpt:  [608.77 MiB/s 610.32 MiB/s 611.72 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
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
        find_objects   full=  5000000  standalone=  5000000
    Benchmarking json-standalone/nested/find_objects: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 15.6s, enable flat sampling, or reduce sample count to 10.
    json-standalone/nested/find_objects
                            time:   [74.112 ms 74.403 ms 74.615 ms]
                            thrpt:  [1.6032 GiB/s 1.6078 GiB/s 1.6141 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) low mild
        find_arrays    full=  1000001  standalone=  1000001
    json-standalone/nested/find_arrays
                            time:   [27.069 ms 27.099 ms 27.137 ms]
                            thrpt:  [4.4081 GiB/s 4.4143 GiB/s 4.4192 GiB/s]
    Found 5 outliers among 20 measurements (25.00%)
      2 (10.00%) low mild
      1 (5.00%) high mild
      2 (10.00%) high severe
        find_strings   full= 13000000  standalone= 13000000
    json-standalone/nested/find_strings
                            time:   [210.28 ms 210.41 ms 210.55 ms]
                            thrpt:  [581.79 MiB/s 582.18 MiB/s 582.53 MiB/s]
        find_members   full= 10000000  standalone=  6000000
    json-standalone/nested/find_members
                            time:   [136.39 ms 136.43 ms 136.47 ms]
                            thrpt:  [897.61 MiB/s 897.87 MiB/s 898.10 MiB/s]
    │  context regions (strings): 13000000
    json-standalone/nested/context
                            time:   [242.49 ms 242.91 ms 243.45 ms]
                            thrpt:  [503.15 MiB/s 504.28 MiB/s 505.15 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    │  full-vs-context-aware counts:
        find_context_objects full=  5000000  context-aware=  5000000
    Benchmarking json-standalone/nested/find_context_objects: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 19.3s, enable flat sampling, or reduce sample count to 10.
    json-standalone/nested/find_context_objects
                            time:   [91.928 ms 92.039 ms 92.152 ms]
                            thrpt:  [1.2981 GiB/s 1.2997 GiB/s 1.3013 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
        find_context_arrays full=  1000001  context-aware=  1000001
    json-standalone/nested/find_context_arrays
                            time:   [35.609 ms 35.689 ms 35.818 ms]
                            thrpt:  [3.3397 GiB/s 3.3519 GiB/s 3.3594 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    json-standalone/nested/find_context_objects_cold
                            time:   [336.08 ms 336.53 ms 337.01 ms]
                            thrpt:  [363.47 MiB/s 363.99 MiB/s 364.48 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    json-standalone/nested/find_context_arrays_cold
                            time:   [279.96 ms 280.37 ms 280.83 ms]
                            thrpt:  [436.19 MiB/s 436.90 MiB/s 437.54 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    ```
</details>

### nightly - `RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-json_standalone --features avx2`

**small:**

#### Standalone - `find_*`
| Function       | `numbers`                  | `objects`                  | `nested`                   |
|----------------|----------------------------|----------------------------|----------------------------|
| `find_objects` | [18.883 µs]=[98.120 GiB/s] | [319.15 µs]=[4.2508 GiB/s] | [612.00 µs]=[1.8025 GiB/s] |
| `find_arrays`  | [42.616 µs]=[43.477 GiB/s] | [32.656 µs]=[41.544 GiB/s] | [187.22 µs]=[5.8922 GiB/s] |
| `find_strings` | [18.916 µs]=[97.950 GiB/s] | [1.3228 ms]=[1.0256 GiB/s] | [1.4272 ms]=[791.49 MiB/s] |
| `find_members` | [20.786 µs]=[89.139 GiB/s] | [1.7916 ms]=[775.39 MiB/s] | [1.1065 ms]=[1020.9 MiB/s] |

#### Context-aware - `context()` + `find_context_*`
| Function               | `numbers`                  | `objects`                  | `nested`                   |
|------------------------|----------------------------|----------------------------|----------------------------|
| `context`              | [20.722 µs]=[89.411 GiB/s] | [1.5127 ms]=[918.38 MiB/s] | [1.6341 ms]=[691.26 MiB/s] |
| `find_context_objects` | [21.941 µs]=[84.445 GiB/s] | [435.91 µs]=[3.1121 GiB/s] | [770.33 µs]=[1.4320 GiB/s] |
| `find_context_arrays`  | [48.661 µs]=[38.076 GiB/s] | [67.733 µs]=[20.029 GiB/s] | [273.28 µs]=[4.0365 GiB/s] |

#### Cold context - `find_context_*_cold`
| Function                    | `numbers`                  | `objects`                  | `nested`                   |
|-----------------------------|----------------------------|----------------------------|----------------------------|
| `find_context_objects_cold` | [43.601 µs]=[42.495 GiB/s] | [1.9401 ms]=[716.03 MiB/s] | [2.3844 ms]=[473.73 MiB/s] |
| `find_context_arrays_cold`  | [66.586 µs]=[27.826 GiB/s] | [1.5714 ms]=[884.07 MiB/s] | [1.9000 ms]=[594.53 MiB/s] |

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
                            time:   [18.693 µs 18.883 µs 19.017 µs]
                            thrpt:  [97.429 GiB/s 98.120 GiB/s 99.117 GiB/s]
        find_arrays    full=        1  standalone=        1
    json-standalone/numbers/find_arrays
                            time:   [42.521 µs 42.616 µs 42.706 µs]
                            thrpt:  [43.385 GiB/s 43.477 GiB/s 43.574 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_strings   full=        0  standalone=        0
    json-standalone/numbers/find_strings
                            time:   [18.853 µs 18.916 µs 19.002 µs]
                            thrpt:  [97.507 GiB/s 97.950 GiB/s 98.274 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_members   full=        0  standalone=        0
    json-standalone/numbers/find_members
                            time:   [20.194 µs 20.786 µs 21.200 µs]
                            thrpt:  [87.399 GiB/s 89.139 GiB/s 91.751 GiB/s]
    │  context regions (strings): 0
    json-standalone/numbers/context
                            time:   [20.612 µs 20.722 µs 20.814 µs]
                            thrpt:  [89.016 GiB/s 89.411 GiB/s 89.891 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) low mild
      1 (5.00%) high mild
    │  full-vs-context-aware counts:
        find_context_objects full=        0  context-aware=        0
    json-standalone/numbers/find_context_objects
                            time:   [21.837 µs 21.941 µs 22.072 µs]
                            thrpt:  [83.945 GiB/s 84.445 GiB/s 84.847 GiB/s]
        find_context_arrays full=        1  context-aware=        1
    json-standalone/numbers/find_context_arrays
                            time:   [46.716 µs 48.661 µs 50.596 µs]
                            thrpt:  [36.619 GiB/s 38.076 GiB/s 39.661 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) high mild
      2 (10.00%) high severe
    json-standalone/numbers/find_context_objects_cold
                            time:   [42.131 µs 43.601 µs 44.477 µs]
                            thrpt:  [41.658 GiB/s 42.495 GiB/s 43.977 GiB/s]
    json-standalone/numbers/find_context_arrays_cold
                            time:   [66.316 µs 66.586 µs 66.964 µs]
                            thrpt:  [27.669 GiB/s 27.826 GiB/s 27.939 GiB/s]
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
                            time:   [319.05 µs 319.15 µs 319.26 µs]
                            thrpt:  [4.2493 GiB/s 4.2508 GiB/s 4.2520 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low mild
      1 (5.00%) high mild
        find_arrays    full=        1  standalone=        1
    json-standalone/objects/find_arrays
                            time:   [32.571 µs 32.656 µs 32.759 µs]
                            thrpt:  [41.412 GiB/s 41.544 GiB/s 41.652 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low severe
      1 (5.00%) low mild
        find_strings   full=   120000  standalone=   120000
    json-standalone/objects/find_strings
                            time:   [1.3218 ms 1.3228 ms 1.3240 ms]
                            thrpt:  [1.0246 GiB/s 1.0256 GiB/s 1.0263 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_members   full=   100000  standalone=   100000
    json-standalone/objects/find_members
                            time:   [1.7894 ms 1.7916 ms 1.7936 ms]
                            thrpt:  [774.54 MiB/s 775.39 MiB/s 776.36 MiB/s]
    │  context regions (strings): 120000
    json-standalone/objects/context
                            time:   [1.5124 ms 1.5127 ms 1.5129 ms]
                            thrpt:  [918.20 MiB/s 918.38 MiB/s 918.52 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    │  full-vs-context-aware counts:
        find_context_objects full=    20000  context-aware=    20000
    json-standalone/objects/find_context_objects
                            time:   [434.56 µs 435.91 µs 437.98 µs]
                            thrpt:  [3.0975 GiB/s 3.1121 GiB/s 3.1218 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) high mild
      2 (10.00%) high severe
        find_context_arrays full=        1  context-aware=        1
    json-standalone/objects/find_context_arrays
                            time:   [67.587 µs 67.733 µs 67.837 µs]
                            thrpt:  [19.998 GiB/s 20.029 GiB/s 20.072 GiB/s]
    json-standalone/objects/find_context_objects_cold
                            time:   [1.9384 ms 1.9401 ms 1.9429 ms]
                            thrpt:  [714.99 MiB/s 716.03 MiB/s 716.66 MiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) high mild
      2 (10.00%) high severe
    json-standalone/objects/find_context_arrays_cold
                            time:   [1.5710 ms 1.5714 ms 1.5718 ms]
                            thrpt:  [883.81 MiB/s 884.07 MiB/s 884.26 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
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
        find_objects   full=    50000  standalone=    50000
    json-standalone/nested/find_objects
                            time:   [611.78 µs 612.00 µs 612.27 µs]
                            thrpt:  [1.8017 GiB/s 1.8025 GiB/s 1.8031 GiB/s]
        find_arrays    full=    10001  standalone=    10001
    json-standalone/nested/find_arrays
                            time:   [187.03 µs 187.22 µs 187.44 µs]
                            thrpt:  [5.8851 GiB/s 5.8922 GiB/s 5.8980 GiB/s]
        find_strings   full=   130000  standalone=   130000
    json-standalone/nested/find_strings
                            time:   [1.4267 ms 1.4272 ms 1.4277 ms]
                            thrpt:  [791.20 MiB/s 791.49 MiB/s 791.73 MiB/s]
        find_members   full=   100000  standalone=    60000
    json-standalone/nested/find_members
                            time:   [1.1045 ms 1.1065 ms 1.1089 ms]
                            thrpt:  [1018.7 MiB/s 1020.9 MiB/s 1022.7 MiB/s]
    │  context regions (strings): 130000
    json-standalone/nested/context
                            time:   [1.6334 ms 1.6341 ms 1.6350 ms]
                            thrpt:  [690.87 MiB/s 691.26 MiB/s 691.54 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    │  full-vs-context-aware counts:
        find_context_objects full=    50000  context-aware=    50000
    json-standalone/nested/find_context_objects
                            time:   [762.61 µs 770.33 µs 778.31 µs]
                            thrpt:  [1.4173 GiB/s 1.4320 GiB/s 1.4465 GiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      4 (20.00%) high severe
        find_context_arrays full=    10001  context-aware=    10001
    json-standalone/nested/find_context_arrays
                            time:   [273.04 µs 273.28 µs 273.62 µs]
                            thrpt:  [4.0316 GiB/s 4.0365 GiB/s 4.0400 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    json-standalone/nested/find_context_objects_cold
                            time:   [2.3835 ms 2.3844 ms 2.3853 ms]
                            thrpt:  [473.57 MiB/s 473.73 MiB/s 473.92 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    json-standalone/nested/find_context_arrays_cold
                            time:   [1.8993 ms 1.9000 ms 1.9005 ms]
                            thrpt:  [594.36 MiB/s 594.53 MiB/s 594.73 MiB/s]
    ```
</details>

**big:**

#### Standalone - `find_*`
| Function       | `numbers`                  | `objects`                  | `nested`                   |
|----------------|----------------------------|----------------------------|----------------------------|
| `find_objects` | [9.3633 ms]=[22.772 GiB/s] | [36.774 ms]=[3.9930 GiB/s] | [64.730 ms]=[1.8480 GiB/s] |
| `find_arrays`  | [19.449 ms]=[10.963 GiB/s] | [13.135 ms]=[11.180 GiB/s] | [23.719 ms]=[5.0434 GiB/s] |
| `find_strings` | [9.3449 ms]=[22.817 GiB/s] | [132.49 ms]=[1.1083 GiB/s] | [143.70 ms]=[852.44 MiB/s] |
| `find_members` | [9.3550 ms]=[22.792 GiB/s] | [179.89 ms]=[835.86 MiB/s] | [112.26 ms]=[1.0656 GiB/s] |

#### Context-aware - `context()` + `find_context_*`
| Function               | `numbers`                  | `objects`                  | `nested`                   |
|------------------------|----------------------------|----------------------------|----------------------------|
| `context`              | [9.3479 ms]=[22.810 GiB/s] | [194.03 ms]=[774.96 MiB/s] | [210.32 ms]=[582.42 MiB/s] |
| `find_context_objects` | [9.3033 ms]=[22.919 GiB/s] | [50.768 ms]=[2.8923 GiB/s] | [80.548 ms]=[1.4851 GiB/s] |
| `find_context_arrays`  | [19.672 ms]=[10.839 GiB/s] | [18.757 ms]=[7.8283 GiB/s] | [31.451 ms]=[3.8035 GiB/s] |

#### Cold context - `find_context_*_cold`
| Function                    | `numbers`                  | `objects`                  | `nested`                   |
|-----------------------------|----------------------------|----------------------------|----------------------------|
| `find_context_objects_cold` | [18.590 ms]=[11.470 GiB/s] | [244.77 ms]=[614.31 MiB/s] | [290.29 ms]=[421.97 MiB/s] |
| `find_context_arrays_cold`  | [29.032 ms]=[7.3445 GiB/s] | [213.88 ms]=[703.04 MiB/s] | [241.75 ms]=[506.69 MiB/s] |

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
                            time:   [9.2894 ms 9.3633 ms 9.4306 ms]
                            thrpt:  [22.610 GiB/s 22.772 GiB/s 22.953 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) high mild
      1 (5.00%) high severe
        find_arrays    full=        1  standalone=        1
    json-standalone/numbers/find_arrays
                            time:   [19.427 ms 19.449 ms 19.469 ms]
                            thrpt:  [10.952 GiB/s 10.963 GiB/s 10.976 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
        find_strings   full=        0  standalone=        0
    json-standalone/numbers/find_strings
                            time:   [9.3220 ms 9.3449 ms 9.3662 ms]
                            thrpt:  [22.765 GiB/s 22.817 GiB/s 22.873 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_members   full=        0  standalone=        0
    json-standalone/numbers/find_members
                            time:   [9.3234 ms 9.3550 ms 9.3837 ms]
                            thrpt:  [22.723 GiB/s 22.792 GiB/s 22.869 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    │  context regions (strings): 0
    json-standalone/numbers/context
                            time:   [9.3270 ms 9.3479 ms 9.3706 ms]
                            thrpt:  [22.754 GiB/s 22.810 GiB/s 22.861 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    │  full-vs-context-aware counts:
        find_context_objects full=        0  context-aware=        0
    json-standalone/numbers/find_context_objects
                            time:   [9.2854 ms 9.3033 ms 9.3176 ms]
                            thrpt:  [22.884 GiB/s 22.919 GiB/s 22.963 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_context_arrays full=        1  context-aware=        1
    json-standalone/numbers/find_context_arrays
                            time:   [19.626 ms 19.672 ms 19.712 ms]
                            thrpt:  [10.817 GiB/s 10.839 GiB/s 10.864 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    json-standalone/numbers/find_context_objects_cold
                            time:   [18.567 ms 18.590 ms 18.620 ms]
                            thrpt:  [11.451 GiB/s 11.470 GiB/s 11.484 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) high mild
      1 (5.00%) high severe
    json-standalone/numbers/find_context_arrays_cold
                            time:   [28.906 ms 29.032 ms 29.116 ms]
                            thrpt:  [7.3233 GiB/s 7.3445 GiB/s 7.3764 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
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
                            time:   [36.737 ms 36.774 ms 36.813 ms]
                            thrpt:  [3.9888 GiB/s 3.9930 GiB/s 3.9970 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_arrays    full=        1  standalone=        1
    json-standalone/objects/find_arrays
                            time:   [13.112 ms 13.135 ms 13.163 ms]
                            thrpt:  [11.155 GiB/s 11.180 GiB/s 11.199 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
        find_strings   full= 12000000  standalone= 12000000
    json-standalone/objects/find_strings
                            time:   [132.36 ms 132.49 ms 132.63 ms]
                            thrpt:  [1.1071 GiB/s 1.1083 GiB/s 1.1094 GiB/s]
        find_members   full= 10000000  standalone= 10000000
    json-standalone/objects/find_members
                            time:   [179.68 ms 179.89 ms 180.12 ms]
                            thrpt:  [834.80 MiB/s 835.86 MiB/s 836.84 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    │  context regions (strings): 12000000
    json-standalone/objects/context
                            time:   [193.79 ms 194.03 ms 194.24 ms]
                            thrpt:  [774.09 MiB/s 774.96 MiB/s 775.90 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) low mild
    │  full-vs-context-aware counts:
        find_context_objects full=  2000000  context-aware=  2000000
    Benchmarking json-standalone/objects/find_context_objects: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 10.5s, enable flat sampling, or reduce sample count to 10.
    json-standalone/objects/find_context_objects
                            time:   [50.325 ms 50.768 ms 51.334 ms]
                            thrpt:  [2.8605 GiB/s 2.8923 GiB/s 2.9178 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_context_arrays full=        1  context-aware=        1
    json-standalone/objects/find_context_arrays
                            time:   [18.676 ms 18.757 ms 18.825 ms]
                            thrpt:  [7.8003 GiB/s 7.8283 GiB/s 7.8623 GiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      1 (5.00%) high mild
      3 (15.00%) high severe
    json-standalone/objects/find_context_objects_cold
                            time:   [244.64 ms 244.77 ms 244.89 ms]
                            thrpt:  [613.99 MiB/s 614.31 MiB/s 614.63 MiB/s]
    json-standalone/objects/find_context_arrays_cold
                            time:   [213.48 ms 213.88 ms 214.39 ms]
                            thrpt:  [701.34 MiB/s 703.04 MiB/s 704.35 MiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) high mild
      1 (5.00%) high severe
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
        find_objects   full=  5000000  standalone=  5000000
    Benchmarking json-standalone/nested/find_objects: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 13.7s, enable flat sampling, or reduce sample count to 10.
    json-standalone/nested/find_objects
                            time:   [64.349 ms 64.730 ms 65.025 ms]
                            thrpt:  [1.8396 GiB/s 1.8480 GiB/s 1.8590 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_arrays    full=  1000001  standalone=  1000001
    json-standalone/nested/find_arrays
                            time:   [23.682 ms 23.719 ms 23.753 ms]
                            thrpt:  [5.0362 GiB/s 5.0434 GiB/s 5.0512 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_strings   full= 13000000  standalone= 13000000
    json-standalone/nested/find_strings
                            time:   [143.52 ms 143.70 ms 143.93 ms]
                            thrpt:  [851.10 MiB/s 852.44 MiB/s 853.51 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_members   full= 10000000  standalone=  6000000
    json-standalone/nested/find_members
                            time:   [112.14 ms 112.26 ms 112.39 ms]
                            thrpt:  [1.0643 GiB/s 1.0656 GiB/s 1.0668 GiB/s]
    │  context regions (strings): 13000000
    json-standalone/nested/context
                            time:   [210.12 ms 210.32 ms 210.55 ms]
                            thrpt:  [581.79 MiB/s 582.42 MiB/s 582.98 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    │  full-vs-context-aware counts:
        find_context_objects full=  5000000  context-aware=  5000000
    Benchmarking json-standalone/nested/find_context_objects: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 16.9s, enable flat sampling, or reduce sample count to 10.
    json-standalone/nested/find_context_objects
                            time:   [80.251 ms 80.548 ms 80.773 ms]
                            thrpt:  [1.4810 GiB/s 1.4851 GiB/s 1.4906 GiB/s]
        find_context_arrays full=  1000001  context-aware=  1000001
    json-standalone/nested/find_context_arrays
                            time:   [31.362 ms 31.451 ms 31.579 ms]
                            thrpt:  [3.7881 GiB/s 3.8035 GiB/s 3.8143 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) high mild
      1 (5.00%) high severe
    json-standalone/nested/find_context_objects_cold
                            time:   [290.01 ms 290.29 ms 290.59 ms]
                            thrpt:  [421.54 MiB/s 421.97 MiB/s 422.38 MiB/s]
    json-standalone/nested/find_context_arrays_cold
                            time:   [241.32 ms 241.75 ms 242.25 ms]
                            thrpt:  [505.66 MiB/s 506.69 MiB/s 507.59 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    ```
</details>

---

## meon-json context-aware extraction (`context()` + `find_context_*`)

For JSON the opaque rule is the string: `context(source)` maps every string
region in one streaming pass, and the `find_context_*` variants skip candidate
delimiters inside them — a `{` inside a string value no longer counts as an
object open. Strings themselves keep only their context-free `find_strings`
(they *are* the context source). The current context-aware object/array
finders also track their own nesting; their budget is not shared with member
values as it is in the full parse. The logs below use historical semantics
and are retained as measurements of that older revision (see
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
