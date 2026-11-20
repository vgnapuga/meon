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

| Corpus    | Shape                                                                                     | Stresses                                                                         |
|-----------|-------------------------------------------------------------------------------------------|----------------------------------------------------------------------------------|
| `numbers` | Flat array of numbers / bools / nulls.                                                    | Scalar scanning. meon emits span vectors; validating parsers parse every number. |
| `objects` | Array of flat objects with mixed-typed fields (`id`/`name`/`active`/...).                 | Members, keys, typed scalars. A typical API payload.                             |
| `nested`  | Array of moderately nested objects (objects-in-objects, small arrays, an escaped string). | The unified nesting stack and the string rule.                                   |

> **Synthetic data notice.** All three corpora are generated programmatically
> with uniform, predictable structure. Real-world JSON typically has less
> regular structure than these — treat the figures as a demonstration of the
> architectural difference, not as expected production throughput.

### Corpus composition

**small (`--features benches/small`):**

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

**big (`--features benches/big`):**

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

**small (`--features benches/small`):**

| Corpus    | `meon-structural`          | `meon-typed`               | `simd-json`                | `sonic-rs`                 |
|-----------|----------------------------|----------------------------|----------------------------|----------------------------|
| `numbers` | [2.2209 ms]=[854.27 MiB/s] | [4.3972 ms]=[431.48 MiB/s] | [5.4437 ms]=[348.53 MiB/s] | [2.6341 ms]=[720.28 MiB/s] |
| `objects` | [3.4646 ms]=[400.97 MiB/s] | [4.5952 ms]=[302.31 MiB/s] | [2.4420 ms]=[568.86 MiB/s] | [1.7494 ms]=[794.11 MiB/s] |
| `nested`  | [4.5622 ms]=[247.59 MiB/s] | [5.7245 ms]=[197.32 MiB/s] | [2.5204 ms]=[448.17 MiB/s] | [2.2899 ms]=[493.29 MiB/s] |

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
                            time:   [2.2207 ms 2.2209 ms 2.2212 ms]
                            thrpt:  [854.18 MiB/s 854.27 MiB/s 854.36 MiB/s]
    json-compare/numbers/meon-typed
                            time:   [4.3942 ms 4.3972 ms 4.4012 ms]
                            thrpt:  [431.08 MiB/s 431.48 MiB/s 431.77 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    json-compare/numbers/simd-json
                            time:   [5.1720 ms 5.4437 ms 5.8957 ms]
                            thrpt:  [321.81 MiB/s 348.53 MiB/s 366.84 MiB/s]
    json-compare/numbers/sonic-rs
                            time:   [2.6270 ms 2.6341 ms 2.6408 ms]
                            thrpt:  [718.44 MiB/s 720.28 MiB/s 722.23 MiB/s]
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
                            time:   [3.4602 ms 3.4646 ms 3.4722 ms]
                            thrpt:  [400.09 MiB/s 400.97 MiB/s 401.48 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    json-compare/objects/meon-typed
                            time:   [4.5937 ms 4.5952 ms 4.5965 ms]
                            thrpt:  [302.23 MiB/s 302.31 MiB/s 302.41 MiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      1 (5.00%) low severe
      2 (10.00%) high mild
      1 (5.00%) high severe
    json-compare/objects/simd-json
                            time:   [2.4050 ms 2.4420 ms 2.5080 ms]
                            thrpt:  [553.90 MiB/s 568.86 MiB/s 577.64 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    json-compare/objects/sonic-rs
                            time:   [1.7459 ms 1.7494 ms 1.7512 ms]
                            thrpt:  [793.27 MiB/s 794.11 MiB/s 795.67 MiB/s]
    
    ┌─ corpus: nested
    │  size:            1.13 MiB  (1184451 bytes)
    │  structural:    290001     (250.7 per KiB)
    │
    │      objects:     50000      arrays:     10001     strings:    130000
    │      members:    100000     scalars:         0       loose:         1
    │  typed: nums:     40000       trues:     10000      falses:         0     nulls:         0
    └─
    json-compare/nested/meon-structural
                            time:   [4.5228 ms 4.5622 ms 4.5827 ms]
                            thrpt:  [246.49 MiB/s 247.59 MiB/s 249.75 MiB/s]
    json-compare/nested/meon-typed
                            time:   [5.6818 ms 5.7245 ms 5.7474 ms]
                            thrpt:  [196.54 MiB/s 197.32 MiB/s 198.81 MiB/s]
    json-compare/nested/simd-json
                            time:   [2.4867 ms 2.5204 ms 2.5793 ms]
                            thrpt:  [437.93 MiB/s 448.17 MiB/s 454.25 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    json-compare/nested/sonic-rs
                            time:   [2.2889 ms 2.2899 ms 2.2912 ms]
                            thrpt:  [493.02 MiB/s 493.29 MiB/s 493.51 MiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) high mild
      1 (5.00%) high severe
    ```

</details>

**big (`--features benches/big`):**

| Corpus    | `meon-structural`          | `meon-typed`               | `simd-json`                | `sonic-rs`                 |
|-----------|----------------------------|----------------------------|----------------------------|----------------------------|
| `numbers` | [241.46 ms]=[904.23 MiB/s] | [608.86 ms]=[358.60 MiB/s] | [938.82 ms]=[232.57 MiB/s] | [898.22 ms]=[243.08 MiB/s] |
| `objects` | [495.85 ms]=[303.24 MiB/s] | [666.93 ms]=[225.46 MiB/s] | [685.36 ms]=[219.39 MiB/s] | [467.23 ms]=[321.81 MiB/s] |
| `nested`  | [634.45 ms]=[193.07 MiB/s] | [761.42 ms]=[160.88 MiB/s] | [722.58 ms]=[169.52 MiB/s] | [538.06 ms]=[227.66 MiB/s] |

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
                            time:   [240.62 ms 241.46 ms 242.50 ms]
                            thrpt:  [900.37 MiB/s 904.23 MiB/s 907.40 MiB/s]
                     change:
                            time:   [+10737% +10776% +10820%] (p = 0.00 < 0.05)
                            thrpt:  [-99.084% -99.081% -99.077%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    Benchmarking json-compare/numbers/meon-typed: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 12.1s, or reduce sample count to 10.
    json-compare/numbers/meon-typed
                            time:   [605.88 ms 608.86 ms 611.90 ms]
                            thrpt:  [356.82 MiB/s 358.60 MiB/s 360.37 MiB/s]
                     change:
                            time:   [+13542% +13622% +13705%] (p = 0.00 < 0.05)
                            thrpt:  [-99.276% -99.271% -99.267%]
                            Performance has regressed.
    Benchmarking json-compare/numbers/simd-json: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 21.8s, or reduce sample count to 10.
    json-compare/numbers/simd-json
                            time:   [937.87 ms 938.82 ms 939.75 ms]
                            thrpt:  [232.34 MiB/s 232.57 MiB/s 232.80 MiB/s]
                     change:
                            time:   [+13701% +14984% +16463%] (p = 0.00 < 0.05)
                            thrpt:  [-99.396% -99.337% -99.275%]
                            Performance has regressed.
    Benchmarking json-compare/numbers/sonic-rs: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 17.9s, or reduce sample count to 10.
    json-compare/numbers/sonic-rs
                            time:   [896.66 ms 898.22 ms 900.03 ms]
                            thrpt:  [242.59 MiB/s 243.08 MiB/s 243.50 MiB/s]
                     change:
                            time:   [+32915% +33208% +33425%] (p = 0.00 < 0.05)
                            thrpt:  [-99.702% -99.700% -99.697%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    
    ┌─ corpus: objects
    │  size:          150.36 MiB  (157666671 bytes)
    │  structural:  24000001     (155.9 per KiB)
    │
    │      objects:   2000000      arrays:         1     strings:  12000000
    │      members:  10000000     scalars:         0       loose:         1
    │  typed: nums:   4000000       trues:   1000000      falses:   1000000     nulls:   2000000
    └─
    json-compare/objects/meon-structural
                            time:   [495.44 ms 495.85 ms 496.33 ms]
                            thrpt:  [302.95 MiB/s 303.24 MiB/s 303.49 MiB/s]
                     change:
                            time:   [+14279% +14303% +14328%] (p = 0.00 < 0.05)
                            thrpt:  [-99.307% -99.306% -99.305%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    Benchmarking json-compare/objects/meon-typed: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 13.3s, or reduce sample count to 10.
    json-compare/objects/meon-typed
                            time:   [664.63 ms 666.93 ms 669.68 ms]
                            thrpt:  [224.53 MiB/s 225.46 MiB/s 226.24 MiB/s]
                     change:
                            time:   [+14432% +14480% +14545%] (p = 0.00 < 0.05)
                            thrpt:  [-99.317% -99.314% -99.312%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    Benchmarking json-compare/objects/simd-json: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 15.4s, or reduce sample count to 10.
    json-compare/objects/simd-json
                            time:   [684.31 ms 685.36 ms 686.34 ms]
                            thrpt:  [219.08 MiB/s 219.39 MiB/s 219.73 MiB/s]
                     change:
                            time:   [+24749% +26276% +27752%] (p = 0.00 < 0.05)
                            thrpt:  [-99.641% -99.621% -99.598%]
                            Performance has regressed.
    json-compare/objects/sonic-rs
                            time:   [466.66 ms 467.23 ms 467.86 ms]
                            thrpt:  [321.38 MiB/s 321.81 MiB/s 322.21 MiB/s]
                     change:
                            time:   [+26630% +26665% +26699%] (p = 0.00 < 0.05)
                            thrpt:  [-99.627% -99.626% -99.626%]
                            Performance has regressed.
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
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 12.3s, or reduce sample count to 10.
    json-compare/nested/meon-structural
                            time:   [631.22 ms 634.45 ms 637.20 ms]
                            thrpt:  [192.24 MiB/s 193.07 MiB/s 194.06 MiB/s]
                     change:
                            time:   [+14525% +14612% +14685%] (p = 0.00 < 0.05)
                            thrpt:  [-99.324% -99.320% -99.316%]
                            Performance has regressed.
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) low severe
      1 (5.00%) low mild
    Benchmarking json-compare/nested/meon-typed: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 15.4s, or reduce sample count to 10.
    json-compare/nested/meon-typed
                            time:   [758.31 ms 761.42 ms 764.79 ms]
                            thrpt:  [160.17 MiB/s 160.88 MiB/s 161.53 MiB/s]
                     change:
                            time:   [+13706% +13763% +13831%] (p = 0.00 < 0.05)
                            thrpt:  [-99.282% -99.279% -99.276%]
                            Performance has regressed.
    Benchmarking json-compare/nested/simd-json: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 15.9s, or reduce sample count to 10.
    json-compare/nested/simd-json
                            time:   [721.78 ms 722.58 ms 723.38 ms]
                            thrpt:  [169.34 MiB/s 169.52 MiB/s 169.71 MiB/s]
                     change:
                            time:   [+25389% +26745% +28028%] (p = 0.00 < 0.05)
                            thrpt:  [-99.644% -99.627% -99.608%]
                            Performance has regressed.
    Benchmarking json-compare/nested/sonic-rs: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 10.8s, or reduce sample count to 10.
    json-compare/nested/sonic-rs
                            time:   [536.52 ms 538.06 ms 539.98 ms]
                            thrpt:  [226.85 MiB/s 227.66 MiB/s 228.31 MiB/s]
                     change:
                            time:   [+23400% +23496% +23599%] (p = 0.00 < 0.05)
                            thrpt:  [-99.578% -99.576% -99.574%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    ```

</details>

### nightly - `RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-json_compare --features meon/avx2`

> meon on AVX2; `simd-json` / `sonic-rs` on their own runtime SIMD.

**small (`--features benches/small`):**

| Corpus    | `meon-structural`          | `meon-typed`               | `simd-json`                | `sonic-rs`                 |
|-----------|----------------------------|----------------------------|----------------------------|----------------------------|
| `numbers` | [1.9720 ms]=[962.13 MiB/s] | [4.5561 ms]=[416.42 MiB/s] | [5.0258 ms]=[377.51 MiB/s] | [2.7981 ms]=[678.05 MiB/s] |
| `objects` | [3.2739 ms]=[424.32 MiB/s] | [4.7426 ms]=[292.92 MiB/s] | [2.2699 ms]=[612.00 MiB/s] | [1.6397 ms]=[847.24 MiB/s] |
| `nested`  | [4.1966 ms]=[269.17 MiB/s] | [5.7542 ms]=[196.30 MiB/s] | [2.3799 ms]=[474.64 MiB/s] | [2.1529 ms]=[524.69 MiB/s] |

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
                            time:   [1.9706 ms 1.9720 ms 1.9733 ms]
                            thrpt:  [961.49 MiB/s 962.13 MiB/s 962.78 MiB/s]
    json-compare/numbers/meon-typed
                            time:   [4.5537 ms 4.5561 ms 4.5599 ms]
                            thrpt:  [416.08 MiB/s 416.42 MiB/s 416.64 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    json-compare/numbers/simd-json
                            time:   [4.7699 ms 5.0258 ms 5.4668 ms]
                            thrpt:  [347.05 MiB/s 377.51 MiB/s 397.76 MiB/s]
    json-compare/numbers/sonic-rs
                            time:   [2.7927 ms 2.7981 ms 2.8050 ms]
                            thrpt:  [676.40 MiB/s 678.05 MiB/s 679.37 MiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      1 (5.00%) low mild
      2 (10.00%) high mild
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
                            time:   [3.2717 ms 3.2739 ms 3.2764 ms]
                            thrpt:  [424.00 MiB/s 424.32 MiB/s 424.61 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    json-compare/objects/meon-typed
                            time:   [4.7406 ms 4.7426 ms 4.7446 ms]
                            thrpt:  [292.80 MiB/s 292.92 MiB/s 293.04 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    json-compare/objects/simd-json
                            time:   [2.2347 ms 2.2699 ms 2.3307 ms]
                            thrpt:  [596.04 MiB/s 612.00 MiB/s 621.65 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    json-compare/objects/sonic-rs
                            time:   [1.6386 ms 1.6397 ms 1.6408 ms]
                            thrpt:  [846.68 MiB/s 847.24 MiB/s 847.77 MiB/s]
    
    ┌─ corpus: nested
    │  size:            1.13 MiB  (1184451 bytes)
    │  structural:    290001     (250.7 per KiB)
    │
    │      objects:     50000      arrays:     10001     strings:    130000
    │      members:    100000     scalars:         0       loose:         1
    │  typed: nums:     40000       trues:     10000      falses:         0     nulls:         0
    └─
    json-compare/nested/meon-structural
                            time:   [4.1695 ms 4.1966 ms 4.2217 ms]
                            thrpt:  [267.57 MiB/s 269.17 MiB/s 270.92 MiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      1 (5.00%) low severe
      2 (10.00%) high mild
      1 (5.00%) high severe
    json-compare/nested/meon-typed
                            time:   [5.7329 ms 5.7542 ms 5.7695 ms]
                            thrpt:  [195.78 MiB/s 196.30 MiB/s 197.04 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
    json-compare/nested/simd-json
                            time:   [2.3538 ms 2.3799 ms 2.4294 ms]
                            thrpt:  [464.96 MiB/s 474.64 MiB/s 479.90 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    json-compare/nested/sonic-rs
                            time:   [2.1515 ms 2.1529 ms 2.1547 ms]
                            thrpt:  [524.24 MiB/s 524.69 MiB/s 525.03 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    ```

</details>

**big (`--features benches/big`):**

| Corpus    | `meon-structural`          | `meon-typed`               | `simd-json`                | `sonic-rs`                 |
|-----------|----------------------------|----------------------------|----------------------------|----------------------------|
| `numbers` | [204.51 ms]=[1.0426 GiB/s] | [616.94 ms]=[353.90 MiB/s] | [920.43 ms]=[237.21 MiB/s] | [905.85 ms]=[241.03 MiB/s] |
| `objects` | [478.00 ms]=[314.56 MiB/s] | [680.86 ms]=[220.84 MiB/s] | [672.10 ms]=[223.72 MiB/s] | [462.36 ms]=[325.21 MiB/s] |
| `nested`  | [606.23 ms]=[202.06 MiB/s] | [766.03 ms]=[159.91 MiB/s] | [712.91 ms]=[171.82 MiB/s] | [529.37 ms]=[231.40 MiB/s] |

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
                            time:   [204.43 ms 204.51 ms 204.59 ms]
                            thrpt:  [1.0422 GiB/s 1.0426 GiB/s 1.0430 GiB/s]
                     change:
                            time:   [+10267% +10273% +10279%] (p = 0.00 < 0.05)
                            thrpt:  [-99.036% -99.036% -99.035%]
                            Performance has regressed.
    Benchmarking json-compare/numbers/meon-typed: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 12.3s, or reduce sample count to 10.
    json-compare/numbers/meon-typed
                            time:   [616.11 ms 616.94 ms 617.73 ms]
                            thrpt:  [353.46 MiB/s 353.90 MiB/s 354.38 MiB/s]
                     change:
                            time:   [+13395% +13423% +13448%] (p = 0.00 < 0.05)
                            thrpt:  [-99.262% -99.261% -99.259%]
                            Performance has regressed.
    Benchmarking json-compare/numbers/simd-json: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 21.0s, or reduce sample count to 10.
    json-compare/numbers/simd-json
                            time:   [918.87 ms 920.43 ms 921.95 ms]
                            thrpt:  [236.82 MiB/s 237.21 MiB/s 237.62 MiB/s]
                     change:
                            time:   [+13535% +14899% +16512%] (p = 0.00 < 0.05)
                            thrpt:  [-99.398% -99.333% -99.267%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
    Benchmarking json-compare/numbers/sonic-rs: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 18.2s, or reduce sample count to 10.
    json-compare/numbers/sonic-rs
                            time:   [904.80 ms 905.85 ms 906.86 ms]
                            thrpt:  [240.76 MiB/s 241.03 MiB/s 241.31 MiB/s]
                     change:
                            time:   [+32184% +32264% +32338%] (p = 0.00 < 0.05)
                            thrpt:  [-99.692% -99.691% -99.690%]
                            Performance has regressed.
    
    ┌─ corpus: objects
    │  size:          150.36 MiB  (157666671 bytes)
    │  structural:  24000001     (155.9 per KiB)
    │
    │      objects:   2000000      arrays:         1     strings:  12000000
    │      members:  10000000     scalars:         0       loose:         1
    │  typed: nums:   4000000       trues:   1000000      falses:   1000000     nulls:   2000000
    └─
    json-compare/objects/meon-structural
                            time:   [477.12 ms 478.00 ms 478.88 ms]
                            thrpt:  [313.99 MiB/s 314.56 MiB/s 315.15 MiB/s]
                     change:
                            time:   [+14476% +14505% +14534%] (p = 0.00 < 0.05)
                            thrpt:  [-99.317% -99.315% -99.314%]
                            Performance has regressed.
    Benchmarking json-compare/objects/meon-typed: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 13.6s, or reduce sample count to 10.
    json-compare/objects/meon-typed
                            time:   [679.85 ms 680.86 ms 681.96 ms]
                            thrpt:  [220.49 MiB/s 220.84 MiB/s 221.17 MiB/s]
                     change:
                            time:   [+14233% +14256% +14281%] (p = 0.00 < 0.05)
                            thrpt:  [-99.305% -99.303% -99.302%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    Benchmarking json-compare/objects/simd-json: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 15.2s, or reduce sample count to 10.
    json-compare/objects/simd-json
                            time:   [671.46 ms 672.10 ms 672.73 ms]
                            thrpt:  [223.51 MiB/s 223.72 MiB/s 223.93 MiB/s]
                     change:
                            time:   [+25787% +27401% +28957%] (p = 0.00 < 0.05)
                            thrpt:  [-99.656% -99.636% -99.614%]
                            Performance has regressed.
    json-compare/objects/sonic-rs
                            time:   [461.25 ms 462.36 ms 463.37 ms]
                            thrpt:  [324.50 MiB/s 325.21 MiB/s 325.99 MiB/s]
                     change:
                            time:   [+28021% +28100% +28160%] (p = 0.00 < 0.05)
                            thrpt:  [-99.646% -99.645% -99.644%]
                            Performance has regressed.
    
    ┌─ corpus: nested
    │  size:          122.49 MiB  (128444451 bytes)
    │  structural:  29000001     (231.2 per KiB)
    │
    │      objects:   5000000      arrays:   1000001     strings:  13000000
    │      members:  10000000     scalars:         0       loose:         1
    │  typed: nums:   4000000       trues:   1000000      falses:         0     nulls:         0
    └─
    Benchmarking json-compare/nested/meon-structural: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 11.7s, or reduce sample count to 10.
    json-compare/nested/meon-structural
                            time:   [604.47 ms 606.23 ms 608.03 ms]
                            thrpt:  [201.46 MiB/s 202.06 MiB/s 202.65 MiB/s]
                     change:
                            time:   [+14349% +14414% +14482%] (p = 0.00 < 0.05)
                            thrpt:  [-99.314% -99.311% -99.308%]
                            Performance has regressed.
    Benchmarking json-compare/nested/meon-typed: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 15.4s, or reduce sample count to 10.
    json-compare/nested/meon-typed
                            time:   [762.15 ms 766.03 ms 769.44 ms]
                            thrpt:  [159.20 MiB/s 159.91 MiB/s 160.72 MiB/s]
                     change:
                            time:   [+13149% +13219% +13282%] (p = 0.00 < 0.05)
                            thrpt:  [-99.253% -99.249% -99.245%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low severe
      1 (5.00%) low mild
    Benchmarking json-compare/nested/simd-json: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 15.7s, or reduce sample count to 10.
    json-compare/nested/simd-json
                            time:   [711.35 ms 712.91 ms 714.44 ms]
                            thrpt:  [171.45 MiB/s 171.82 MiB/s 172.20 MiB/s]
                     change:
                            time:   [+26441% +27924% +29315%] (p = 0.00 < 0.05)
                            thrpt:  [-99.660% -99.643% -99.623%]
                            Performance has regressed.
    Benchmarking json-compare/nested/sonic-rs: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 10.7s, or reduce sample count to 10.
    json-compare/nested/sonic-rs
                            time:   [527.75 ms 529.37 ms 531.25 ms]
                            thrpt:  [230.58 MiB/s 231.40 MiB/s 232.11 MiB/s]
                     change:
                            time:   [+24413% +24488% +24577%] (p = 0.00 < 0.05)
                            thrpt:  [-99.595% -99.593% -99.592%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    ```
    
</details>

---

## Scaling from small to big

How each parser holds up as the input grows past cache (median `thrpt`, MiB/s;
`simd-json` / `sonic-rs` from the stable run):

| Parser                      | `numbers`     | `objects`     | `nested`      |
|-----------------------------|---------------|---------------|---------------|
| `meon-structural` - stable  | 854 -> 904    | 401 -> 303    | 248 -> 193    |
| `meon-structural` - nightly | 962 -> 1068   | 424 -> 315    | 269 -> 202    |
| `meon-typed` - stable       | 431 -> 359    | 302 -> 225    | 197 -> 161    |
| `meon-typed` - nightly      | 416 -> 354    | 293 -> 221    | 196 -> 160    |
| `simd-json`                 | 349 -> 233    | 569 -> 219    | 448 -> 170    |
| `sonic-rs`                  | 720 -> 243    | 794 -> 322    | 493 -> 228    |

- **meon degrades little with scale.** `meon-structural` gains on `numbers`
  (854 -> 904 stable, 962 -> 1068 nightly) and loses about a quarter on
  `objects` / `nested` (-24% / -22% stable, -26% / -25% nightly). `meon-typed`
  loses 15-26%. The flat span table stays largely cache-resident.
- **The validating parsers lose more at big.** `simd-json` loses 62% on
  `objects`, 62% on `nested` and 33% on `numbers`. `sonic-rs` loses 54-66% on
  every corpus, most on `numbers`. Both materialise a tape / an owned `Value`,
  and that working set outgrows cache as the document grows.
- **The gap narrows with scale.** At small, `simd-json` and `sonic-rs` lead
  `meon-structural` on `objects` / `nested` by 1.4-2.0x. At big,
  `meon-structural` overtakes `simd-json` (303 vs 219 on `objects`, 193 vs 170
  on `nested`), while `sonic-rs` keeps a smaller lead (322 vs 303, 228 vs 193).
  On `numbers` meon leads at every scale, from 1.2x over `sonic-rs` at small to
  about 4x at big.
- **AVX2 helps meon-json slightly.** The nightly AVX2 rows follow the same
  scaling shape and are 4-18% faster than the stable SWAR rows; the largest gain
  is on `numbers` (12.6% at small, 18.1% at big).

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

**small (`--features benches/small`):**

#### Standalone - `find_*`
| Function       | `numbers`                  | `objects`                  | `nested`                   |
|----------------|----------------------------|----------------------------|----------------------------|
| `find_objects` | [19.568 µs]=[94.684 GiB/s] | [401.60 µs]=[3.3780 GiB/s] | [707.19 µs]=[1.5599 GiB/s] |
| `find_arrays`  | [42.895 µs]=[43.194 GiB/s] | [34.091 µs]=[39.794 GiB/s] | [219.66 µs]=[5.0220 GiB/s] |
| `find_strings` | [19.710 µs]=[94.002 GiB/s] | [1.9970 ms]=[695.64 MiB/s] | [2.1609 ms]=[522.74 MiB/s] |
| `find_members` | [19.954 µs]=[92.852 GiB/s] | [2.2074 ms]=[629.34 MiB/s] | [1.3464 ms]=[838.97 MiB/s] |

#### Context-aware - `context()` + `find_context_*`
| Function               | `numbers`                  | `objects`                  | `nested`                   |
|------------------------|----------------------------|----------------------------|----------------------------|
| `context`              | [20.494 µs]=[90.408 GiB/s] | [6.2727 ms]=[221.47 MiB/s] | [7.6615 ms]=[147.44 MiB/s] |
| `find_context_objects` | [20.000 µs]=[92.643 GiB/s] | [507.07 µs]=[2.6754 GiB/s] | [904.27 µs]=[1.2199 GiB/s] |
| `find_context_arrays`  | [44.130 µs]=[41.985 GiB/s] | [78.811 µs]=[17.214 GiB/s] | [324.65 µs]=[3.3979 GiB/s] |

#### Cold context - `find_context_*_cold`
| Function                    | `numbers`                  | `objects`                  | `nested`                   |
|-----------------------------|----------------------------|----------------------------|----------------------------|
| `find_context_objects_cold` | [45.069 µs]=[41.111 GiB/s] | [6.8848 ms]=[201.78 MiB/s] | [8.6794 ms]=[130.15 MiB/s] |
| `find_context_arrays_cold`  | [67.042 µs]=[27.637 GiB/s] | [6.4312 ms]=[216.01 MiB/s] | [8.1821 ms]=[138.06 MiB/s] |

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
                            time:   [19.492 µs 19.568 µs 19.662 µs]
                            thrpt:  [94.232 GiB/s 94.684 GiB/s 95.055 GiB/s]
        find_arrays    full=        1  standalone=        1
    json-standalone/numbers/find_arrays
                            time:   [42.854 µs 42.895 µs 42.932 µs]
                            thrpt:  [43.156 GiB/s 43.194 GiB/s 43.235 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_strings   full=        0  standalone=        0
    json-standalone/numbers/find_strings
                            time:   [19.422 µs 19.710 µs 20.011 µs]
                            thrpt:  [92.587 GiB/s 94.002 GiB/s 95.397 GiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      2 (10.00%) high mild
      2 (10.00%) high severe
        find_members   full=        0  standalone=        0
    json-standalone/numbers/find_members
                            time:   [19.658 µs 19.954 µs 20.187 µs]
                            thrpt:  [91.782 GiB/s 92.852 GiB/s 94.250 GiB/s]
    │  context regions (strings): 0
    json-standalone/numbers/context
                            time:   [20.009 µs 20.494 µs 20.930 µs]
                            thrpt:  [88.523 GiB/s 90.408 GiB/s 92.598 GiB/s]
    │  full-vs-context-aware counts:
        find_context_objects full=        0  context-aware=        0
    json-standalone/numbers/find_context_objects
                            time:   [19.310 µs 20.000 µs 20.558 µs]
                            thrpt:  [90.127 GiB/s 92.643 GiB/s 95.953 GiB/s]
        find_context_arrays full=        1  context-aware=        1
    json-standalone/numbers/find_context_arrays
                            time:   [43.350 µs 44.130 µs 45.056 µs]
                            thrpt:  [41.122 GiB/s 41.985 GiB/s 42.741 GiB/s]
    json-standalone/numbers/find_context_objects_cold
                            time:   [43.796 µs 45.069 µs 46.010 µs]
                            thrpt:  [40.270 GiB/s 41.111 GiB/s 42.305 GiB/s]
    json-standalone/numbers/find_context_arrays_cold
                            time:   [65.376 µs 67.042 µs 68.202 µs]
                            thrpt:  [27.167 GiB/s 27.637 GiB/s 28.341 GiB/s]
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
                            time:   [401.46 µs 401.60 µs 401.77 µs]
                            thrpt:  [3.3766 GiB/s 3.3780 GiB/s 3.3792 GiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      1 (5.00%) low severe
      2 (10.00%) low mild
      1 (5.00%) high mild
        find_arrays    full=        1  standalone=        1
    json-standalone/objects/find_arrays
                            time:   [33.642 µs 34.091 µs 34.645 µs]
                            thrpt:  [39.158 GiB/s 39.794 GiB/s 40.326 GiB/s]
        find_strings   full=   120000  standalone=   120000
    json-standalone/objects/find_strings
                            time:   [1.9954 ms 1.9970 ms 1.9997 ms]
                            thrpt:  [694.68 MiB/s 695.64 MiB/s 696.20 MiB/s]
        find_members   full=   100000  standalone=   100000
    json-standalone/objects/find_members
                            time:   [2.2003 ms 2.2074 ms 2.2131 ms]
                            thrpt:  [627.71 MiB/s 629.34 MiB/s 631.36 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    │  context regions (strings): 120000
    json-standalone/objects/context
                            time:   [6.2549 ms 6.2727 ms 6.2898 ms]
                            thrpt:  [220.87 MiB/s 221.47 MiB/s 222.10 MiB/s]
    │  full-vs-context-aware counts:
        find_context_objects full=    20000  context-aware=    20000
    json-standalone/objects/find_context_objects
                            time:   [506.29 µs 507.07 µs 507.81 µs]
                            thrpt:  [2.6715 GiB/s 2.6754 GiB/s 2.6796 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_context_arrays full=        1  context-aware=        1
    json-standalone/objects/find_context_arrays
                            time:   [77.894 µs 78.811 µs 79.506 µs]
                            thrpt:  [17.063 GiB/s 17.214 GiB/s 17.416 GiB/s]
    json-standalone/objects/find_context_objects_cold
                            time:   [6.8759 ms 6.8848 ms 6.8924 ms]
                            thrpt:  [201.55 MiB/s 201.78 MiB/s 202.04 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    json-standalone/objects/find_context_arrays_cold
                            time:   [6.4161 ms 6.4312 ms 6.4438 ms]
                            thrpt:  [215.58 MiB/s 216.01 MiB/s 216.52 MiB/s]
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
                            time:   [706.11 µs 707.19 µs 708.72 µs]
                            thrpt:  [1.5565 GiB/s 1.5599 GiB/s 1.5622 GiB/s]
        find_arrays    full=    10001  standalone=    10001
    json-standalone/nested/find_arrays
                            time:   [219.49 µs 219.66 µs 219.83 µs]
                            thrpt:  [5.0180 GiB/s 5.0220 GiB/s 5.0259 GiB/s]
        find_strings   full=   130000  standalone=   130000
    json-standalone/nested/find_strings
                            time:   [2.1596 ms 2.1609 ms 2.1627 ms]
                            thrpt:  [522.29 MiB/s 522.74 MiB/s 523.06 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
        find_members   full=   100000  standalone=    60000
    json-standalone/nested/find_members
                            time:   [1.3461 ms 1.3464 ms 1.3468 ms]
                            thrpt:  [838.71 MiB/s 838.97 MiB/s 839.16 MiB/s]
    │  context regions (strings): 130000
    json-standalone/nested/context
                            time:   [7.6133 ms 7.6615 ms 7.7260 ms]
                            thrpt:  [146.20 MiB/s 147.44 MiB/s 148.37 MiB/s]
    │  full-vs-context-aware counts:
        find_context_objects full=    50000  context-aware=    50000
    json-standalone/nested/find_context_objects
                            time:   [902.38 µs 904.27 µs 906.08 µs]
                            thrpt:  [1.2174 GiB/s 1.2199 GiB/s 1.2224 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) high mild
      1 (5.00%) high severe
        find_context_arrays full=    10001  context-aware=    10001
    json-standalone/nested/find_context_arrays
                            time:   [324.08 µs 324.65 µs 325.32 µs]
                            thrpt:  [3.3909 GiB/s 3.3979 GiB/s 3.4038 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      3 (15.00%) high mild
    json-standalone/nested/find_context_objects_cold
                            time:   [8.6536 ms 8.6794 ms 8.7072 ms]
                            thrpt:  [129.73 MiB/s 130.15 MiB/s 130.53 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    json-standalone/nested/find_context_arrays_cold
                            time:   [8.1455 ms 8.1821 ms 8.2343 ms]
                            thrpt:  [137.18 MiB/s 138.06 MiB/s 138.68 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    ```
</details>

**big (`--features benches/big`):**

#### Standalone - `find_*`
| Function       | `numbers`                  | `objects`                  | `nested`                   |
|----------------|----------------------------|----------------------------|----------------------------|
| `find_objects` | [9.2373 ms]=[23.083 GiB/s] | [47.321 ms]=[3.1030 GiB/s] | [74.729 ms]=[1.6008 GiB/s] |
| `find_arrays`  | [19.073 ms]=[11.179 GiB/s] | [13.084 ms]=[11.223 GiB/s] | [26.796 ms]=[4.4643 GiB/s] |
| `find_strings` | [9.2006 ms]=[23.175 GiB/s] | [199.51 ms]=[753.67 MiB/s] | [216.07 ms]=[566.91 MiB/s] |
| `find_members` | [9.2060 ms]=[23.161 GiB/s] | [220.17 ms]=[682.94 MiB/s] | [135.91 ms]=[901.30 MiB/s] |

#### Context-aware - `context()` + `find_context_*`
| Function               | `numbers`                  | `objects`                  | `nested`                   |
|------------------------|----------------------------|----------------------------|----------------------------|
| `context`              | [9.5094 ms]=[22.422 GiB/s] | [786.90 ms]=[191.08 MiB/s] | [859.26 ms]=[142.56 MiB/s] |
| `find_context_objects` | [9.1844 ms]=[23.215 GiB/s] | [57.755 ms]=[2.5425 GiB/s] | [93.890 ms]=[1.2741 GiB/s] |
| `find_context_arrays`  | [19.250 ms]=[11.076 GiB/s] | [19.495 ms]=[7.5320 GiB/s] | [36.008 ms]=[3.3222 GiB/s] |

#### Cold context - `find_context_*_cold`
| Function                    | `numbers`                  | `objects`                  | `nested`                   |
|-----------------------------|----------------------------|----------------------------|----------------------------|
| `find_context_objects_cold` | [18.543 ms]=[11.499 GiB/s] | [851.09 ms]=[176.67 MiB/s] | [958.46 ms]=[127.80 MiB/s] |
| `find_context_arrays_cold`  | [28.758 ms]=[7.4145 GiB/s] | [808.80 ms]=[185.91 MiB/s] | [903.05 ms]=[135.65 MiB/s] |

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
                            time:   [9.2158 ms 9.2373 ms 9.2727 ms]
                            thrpt:  [22.995 GiB/s 23.083 GiB/s 23.136 GiB/s]
                     change:
                            time:   [+47078% +47387% +47775%] (p = 0.00 < 0.05)
                            thrpt:  [-99.791% -99.789% -99.788%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_arrays    full=        1  standalone=        1
    json-standalone/numbers/find_arrays
                            time:   [19.027 ms 19.073 ms 19.123 ms]
                            thrpt:  [11.150 GiB/s 11.179 GiB/s 11.206 GiB/s]
                     change:
                            time:   [+44153% +44264% +44367%] (p = 0.00 < 0.05)
                            thrpt:  [-99.775% -99.775% -99.774%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_strings   full=        0  standalone=        0
    json-standalone/numbers/find_strings
                            time:   [9.1905 ms 9.2006 ms 9.2127 ms]
                            thrpt:  [23.144 GiB/s 23.175 GiB/s 23.200 GiB/s]
                     change:
                            time:   [+46456% +46880% +47259%] (p = 0.00 < 0.05)
                            thrpt:  [-99.789% -99.787% -99.785%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_members   full=        0  standalone=        0
    json-standalone/numbers/find_members
                            time:   [9.1791 ms 9.2060 ms 9.2454 ms]
                            thrpt:  [23.062 GiB/s 23.161 GiB/s 23.229 GiB/s]
                     change:
                            time:   [+46234% +46815% +47385%] (p = 0.00 < 0.05)
                            thrpt:  [-99.789% -99.787% -99.784%]
                            Performance has regressed.
    Found 3 outliers among 20 measurements (15.00%)
      2 (10.00%) high mild
      1 (5.00%) high severe
    │  context regions (strings): 0
    json-standalone/numbers/context
                            time:   [9.4453 ms 9.5094 ms 9.5740 ms]
                            thrpt:  [22.271 GiB/s 22.422 GiB/s 22.574 GiB/s]
                     change:
                            time:   [+43062% +44971% +46865%] (p = 0.00 < 0.05)
                            thrpt:  [-99.787% -99.778% -99.768%]
                            Performance has regressed.
    │  full-vs-context-aware counts:
        find_context_objects full=        0  context-aware=        0
    json-standalone/numbers/find_context_objects
                            time:   [9.1710 ms 9.1844 ms 9.1959 ms]
                            thrpt:  [23.187 GiB/s 23.215 GiB/s 23.250 GiB/s]
                     change:
                            time:   [+45568% +46523% +47442%] (p = 0.00 < 0.05)
                            thrpt:  [-99.790% -99.786% -99.781%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
        find_context_arrays full=        1  context-aware=        1
    json-standalone/numbers/find_context_arrays
                            time:   [19.194 ms 19.250 ms 19.301 ms]
                            thrpt:  [11.047 GiB/s 11.076 GiB/s 11.109 GiB/s]
                     change:
                            time:   [+42090% +42877% +43646%] (p = 0.00 < 0.05)
                            thrpt:  [-99.771% -99.767% -99.763%]
                            Performance has regressed.
    json-standalone/numbers/find_context_objects_cold
                            time:   [18.444 ms 18.543 ms 18.644 ms]
                            thrpt:  [11.437 GiB/s 11.499 GiB/s 11.561 GiB/s]
                     change:
                            time:   [+41822% +43266% +44798%] (p = 0.00 < 0.05)
                            thrpt:  [-99.777% -99.769% -99.761%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    json-standalone/numbers/find_context_arrays_cold
                            time:   [28.713 ms 28.758 ms 28.813 ms]
                            thrpt:  [7.4002 GiB/s 7.4145 GiB/s 7.4259 GiB/s]
                     change:
                            time:   [+43084% +43961% +44869%] (p = 0.00 < 0.05)
                            thrpt:  [-99.778% -99.773% -99.768%]
                            Performance has regressed.
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
    Benchmarking json-standalone/objects/find_objects: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 10.0s, enable flat sampling, or reduce sample count to 10.
    json-standalone/objects/find_objects
                            time:   [47.091 ms 47.321 ms 47.596 ms]
                            thrpt:  [3.0851 GiB/s 3.1030 GiB/s 3.1182 GiB/s]
                     change:
                            time:   [+11709% +11766% +11816%] (p = 0.00 < 0.05)
                            thrpt:  [-99.161% -99.157% -99.153%]
                            Performance has regressed.
        find_arrays    full=        1  standalone=        1
    json-standalone/objects/find_arrays
                            time:   [13.066 ms 13.084 ms 13.103 ms]
                            thrpt:  [11.207 GiB/s 11.223 GiB/s 11.238 GiB/s]
                     change:
                            time:   [+37479% +37902% +38355%] (p = 0.00 < 0.05)
                            thrpt:  [-99.740% -99.737% -99.734%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_strings   full= 12000000  standalone= 12000000
    json-standalone/objects/find_strings
                            time:   [199.44 ms 199.51 ms 199.60 ms]
                            thrpt:  [753.32 MiB/s 753.67 MiB/s 753.94 MiB/s]
                     change:
                            time:   [+9836.7% +9857.0% +9876.5%] (p = 0.00 < 0.05)
                            thrpt:  [-98.998% -98.996% -98.994%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_members   full= 10000000  standalone= 10000000
    json-standalone/objects/find_members
                            time:   [220.13 ms 220.17 ms 220.21 ms]
                            thrpt:  [682.83 MiB/s 682.94 MiB/s 683.06 MiB/s]
                     change:
                            time:   [+9868.8% +9883.5% +9896.7%] (p = 0.00 < 0.05)
                            thrpt:  [-99.000% -98.998% -98.997%]
                            Performance has regressed.
    │  context regions (strings): 12000000
    Benchmarking json-standalone/objects/context: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 15.5s, or reduce sample count to 10.
    json-standalone/objects/context
                            time:   [784.96 ms 786.90 ms 789.32 ms]
                            thrpt:  [190.50 MiB/s 191.08 MiB/s 191.55 MiB/s]
                     change:
                            time:   [+12434% +12488% +12544%] (p = 0.00 < 0.05)
                            thrpt:  [-99.209% -99.206% -99.202%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    │  full-vs-context-aware counts:
        find_context_objects full=  2000000  context-aware=  2000000
    Benchmarking json-standalone/objects/find_context_objects: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 12.1s, enable flat sampling, or reduce sample count to 10.
    json-standalone/objects/find_context_objects
                            time:   [57.646 ms 57.755 ms 57.905 ms]
                            thrpt:  [2.5359 GiB/s 2.5425 GiB/s 2.5472 GiB/s]
                     change:
                            time:   [+11265% +11293% +11323%] (p = 0.00 < 0.05)
                            thrpt:  [-99.125% -99.122% -99.120%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
        find_context_arrays full=        1  context-aware=        1
    json-standalone/objects/find_context_arrays
                            time:   [19.381 ms 19.495 ms 19.626 ms]
                            thrpt:  [7.4819 GiB/s 7.5320 GiB/s 7.5764 GiB/s]
                     change:
                            time:   [+24714% +24962% +25208%] (p = 0.00 < 0.05)
                            thrpt:  [-99.605% -99.601% -99.597%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    Benchmarking json-standalone/objects/find_context_objects_cold: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 17.0s, or reduce sample count to 10.
    json-standalone/objects/find_context_objects_cold
                            time:   [849.22 ms 851.09 ms 852.78 ms]
                            thrpt:  [176.32 MiB/s 176.67 MiB/s 177.06 MiB/s]
                     change:
                            time:   [+12225% +12262% +12297%] (p = 0.00 < 0.05)
                            thrpt:  [-99.193% -99.191% -99.189%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
    Benchmarking json-standalone/objects/find_context_arrays_cold: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 16.4s, or reduce sample count to 10.
    json-standalone/objects/find_context_arrays_cold
                            time:   [807.97 ms 808.80 ms 809.71 ms]
                            thrpt:  [185.70 MiB/s 185.91 MiB/s 186.10 MiB/s]
                     change:
                            time:   [+12450% +12473% +12497%] (p = 0.00 < 0.05)
                            thrpt:  [-99.206% -99.205% -99.203%]
                            Performance has regressed.
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
                            time:   [74.096 ms 74.729 ms 75.604 ms]
                            thrpt:  [1.5822 GiB/s 1.6008 GiB/s 1.6144 GiB/s]
                     change:
                            time:   [+10385% +10465% +10548%] (p = 0.00 < 0.05)
                            thrpt:  [-99.061% -99.053% -99.046%]
                            Performance has regressed.
        find_arrays    full=  1000001  standalone=  1000001
    json-standalone/nested/find_arrays
                            time:   [26.771 ms 26.796 ms 26.822 ms]
                            thrpt:  [4.4598 GiB/s 4.4643 GiB/s 4.4684 GiB/s]
                     change:
                            time:   [+12096% +12128% +12164%] (p = 0.00 < 0.05)
                            thrpt:  [-99.185% -99.182% -99.180%]
                            Performance has regressed.
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) high mild
      2 (10.00%) high severe
        find_strings   full= 13000000  standalone= 13000000
    json-standalone/nested/find_strings
                            time:   [216.04 ms 216.07 ms 216.11 ms]
                            thrpt:  [566.82 MiB/s 566.91 MiB/s 567.00 MiB/s]
                     change:
                            time:   [+9890.1% +9897.2% +9903.0%] (p = 0.00 < 0.05)
                            thrpt:  [-99.000% -99.000% -98.999%]
                            Performance has regressed.
        find_members   full= 10000000  standalone=  6000000
    json-standalone/nested/find_members
                            time:   [135.75 ms 135.91 ms 136.13 ms]
                            thrpt:  [899.84 MiB/s 901.30 MiB/s 902.32 MiB/s]
                     change:
                            time:   [+9977.2% +9989.5% +10006%] (p = 0.00 < 0.05)
                            thrpt:  [-99.010% -99.009% -99.008%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high severe
    │  context regions (strings): 13000000
    Benchmarking json-standalone/nested/context: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 17.1s, or reduce sample count to 10.
    json-standalone/nested/context
                            time:   [856.85 ms 859.26 ms 861.96 ms]
                            thrpt:  [142.11 MiB/s 142.56 MiB/s 142.96 MiB/s]
                     change:
                            time:   [+11044% +11110% +11180%] (p = 0.00 < 0.05)
                            thrpt:  [-99.113% -99.108% -99.103%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    │  full-vs-context-aware counts:
        find_context_objects full=  5000000  context-aware=  5000000
    Benchmarking json-standalone/nested/find_context_objects: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 19.8s, enable flat sampling, or reduce sample count to 10.
    json-standalone/nested/find_context_objects
                            time:   [93.859 ms 93.890 ms 93.921 ms]
                            thrpt:  [1.2737 GiB/s 1.2741 GiB/s 1.2745 GiB/s]
                     change:
                            time:   [+10279% +10299% +10317%] (p = 0.00 < 0.05)
                            thrpt:  [-99.040% -99.038% -99.037%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low mild
      1 (5.00%) high severe
        find_context_arrays full=  1000001  context-aware=  1000001
    json-standalone/nested/find_context_arrays
                            time:   [35.978 ms 36.008 ms 36.036 ms]
                            thrpt:  [3.3195 GiB/s 3.3222 GiB/s 3.3249 GiB/s]
                     change:
                            time:   [+10985% +11004% +11022%] (p = 0.00 < 0.05)
                            thrpt:  [-99.101% -99.099% -99.098%]
                            Performance has regressed.
    Benchmarking json-standalone/nested/find_context_objects_cold: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 19.1s, or reduce sample count to 10.
    json-standalone/nested/find_context_objects_cold
                            time:   [956.74 ms 958.46 ms 960.16 ms]
                            thrpt:  [127.58 MiB/s 127.80 MiB/s 128.03 MiB/s]
                     change:
                            time:   [+10855% +10904% +10947%] (p = 0.00 < 0.05)
                            thrpt:  [-99.095% -99.091% -99.087%]
                            Performance has regressed.
    Benchmarking json-standalone/nested/find_context_arrays_cold: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 18.0s, or reduce sample count to 10.
    json-standalone/nested/find_context_arrays_cold
                            time:   [899.43 ms 903.05 ms 908.41 ms]
                            thrpt:  [134.84 MiB/s 135.65 MiB/s 136.19 MiB/s]
                     change:
                            time:   [+10785% +10865% +10954%] (p = 0.00 < 0.05)
                            thrpt:  [-99.095% -99.088% -99.081%]
                            Performance has regressed.
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) high mild
      2 (10.00%) high severe
    ```
</details>

### nightly - `RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-json_standalone --features meon/avx2`

**small (`--features benches/small`):**

#### Standalone - `find_*`
| Function       | `numbers`                  | `objects`                  | `nested`                   |
|----------------|----------------------------|----------------------------|----------------------------|
| `find_objects` | [20.623 µs]=[89.841 GiB/s] | [399.27 µs]=[3.3978 GiB/s] | [708.77 µs]=[1.5564 GiB/s] |
| `find_arrays`  | [44.753 µs]=[41.401 GiB/s] | [35.348 µs]=[38.379 GiB/s] | [219.27 µs]=[5.0308 GiB/s] |
| `find_strings` | [20.780 µs]=[89.162 GiB/s] | [2.0028 ms]=[693.62 MiB/s] | [2.1666 ms]=[521.36 MiB/s] |
| `find_members` | [20.981 µs]=[88.311 GiB/s] | [2.2076 ms]=[629.28 MiB/s] | [1.3443 ms]=[840.26 MiB/s] |

#### Context-aware - `context()` + `find_context_*`
| Function               | `numbers`                  | `objects`                  | `nested`                   |
|------------------------|----------------------------|----------------------------|----------------------------|
| `context`              | [20.007 µs]=[92.609 GiB/s] | [6.2626 ms]=[221.82 MiB/s] | [7.6794 ms]=[147.09 MiB/s] |
| `find_context_objects` | [20.534 µs]=[90.230 GiB/s] | [503.17 µs]=[2.6962 GiB/s] | [901.90 µs]=[1.2231 GiB/s] |
| `find_context_arrays`  | [44.508 µs]=[41.628 GiB/s] | [80.372 µs]=[16.879 GiB/s] | [320.62 µs]=[3.4405 GiB/s] |

#### Cold context - `find_context_*_cold`
| Function                    | `numbers`                  | `objects`                  | `nested`                   |
|-----------------------------|----------------------------|----------------------------|----------------------------|
| `find_context_objects_cold` | [39.044 µs]=[47.455 GiB/s] | [6.8536 ms]=[202.70 MiB/s] | [8.6324 ms]=[130.85 MiB/s] |
| `find_context_arrays_cold`  | [62.581 µs]=[29.607 GiB/s] | [6.4755 ms]=[214.53 MiB/s] | [8.0746 ms]=[139.89 MiB/s] |

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
                            time:   [20.238 µs 20.623 µs 21.173 µs]
                            thrpt:  [87.506 GiB/s 89.841 GiB/s 91.552 GiB/s]
        find_arrays    full=        1  standalone=        1
    json-standalone/numbers/find_arrays
                            time:   [44.505 µs 44.753 µs 45.051 µs]
                            thrpt:  [41.127 GiB/s 41.401 GiB/s 41.631 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_strings   full=        0  standalone=        0
    json-standalone/numbers/find_strings
                            time:   [20.639 µs 20.780 µs 20.874 µs]
                            thrpt:  [88.760 GiB/s 89.162 GiB/s 89.772 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low severe
        find_members   full=        0  standalone=        0
    json-standalone/numbers/find_members
                            time:   [20.249 µs 20.981 µs 21.452 µs]
                            thrpt:  [86.371 GiB/s 88.311 GiB/s 91.499 GiB/s]
    │  context regions (strings): 0
    json-standalone/numbers/context
                            time:   [19.660 µs 20.007 µs 20.536 µs]
                            thrpt:  [90.222 GiB/s 92.609 GiB/s 94.244 GiB/s]
    │  full-vs-context-aware counts:
        find_context_objects full=        0  context-aware=        0
    json-standalone/numbers/find_context_objects
                            time:   [20.360 µs 20.534 µs 20.643 µs]
                            thrpt:  [89.756 GiB/s 90.230 GiB/s 91.003 GiB/s]
        find_context_arrays full=        1  context-aware=        1
    json-standalone/numbers/find_context_arrays
                            time:   [44.270 µs 44.508 µs 44.787 µs]
                            thrpt:  [41.369 GiB/s 41.628 GiB/s 41.853 GiB/s]
    json-standalone/numbers/find_context_objects_cold
                            time:   [38.616 µs 39.044 µs 39.556 µs]
                            thrpt:  [46.840 GiB/s 47.455 GiB/s 47.981 GiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    json-standalone/numbers/find_context_arrays_cold
                            time:   [62.046 µs 62.581 µs 63.192 µs]
                            thrpt:  [29.320 GiB/s 29.607 GiB/s 29.862 GiB/s]
    Found 6 outliers among 20 measurements (30.00%)
      2 (10.00%) low mild
      4 (20.00%) high mild
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
                            time:   [399.16 µs 399.27 µs 399.44 µs]
                            thrpt:  [3.3963 GiB/s 3.3978 GiB/s 3.3987 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_arrays    full=        1  standalone=        1
    json-standalone/objects/find_arrays
                            time:   [35.293 µs 35.348 µs 35.408 µs]
                            thrpt:  [38.314 GiB/s 38.379 GiB/s 38.439 GiB/s]
        find_strings   full=   120000  standalone=   120000
    json-standalone/objects/find_strings
                            time:   [2.0019 ms 2.0028 ms 2.0039 ms]
                            thrpt:  [693.24 MiB/s 693.62 MiB/s 693.94 MiB/s]
        find_members   full=   100000  standalone=   100000
    json-standalone/objects/find_members
                            time:   [2.2054 ms 2.2076 ms 2.2106 ms]
                            thrpt:  [628.42 MiB/s 629.28 MiB/s 629.91 MiB/s]
    │  context regions (strings): 120000
    json-standalone/objects/context
                            time:   [6.2507 ms 6.2626 ms 6.2760 ms]
                            thrpt:  [221.35 MiB/s 221.82 MiB/s 222.24 MiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      4 (20.00%) high mild
    │  full-vs-context-aware counts:
        find_context_objects full=    20000  context-aware=    20000
    json-standalone/objects/find_context_objects
                            time:   [502.82 µs 503.17 µs 503.52 µs]
                            thrpt:  [2.6943 GiB/s 2.6962 GiB/s 2.6980 GiB/s]
        find_context_arrays full=        1  context-aware=        1
    json-standalone/objects/find_context_arrays
                            time:   [79.330 µs 80.372 µs 81.036 µs]
                            thrpt:  [16.741 GiB/s 16.879 GiB/s 17.101 GiB/s]
    json-standalone/objects/find_context_objects_cold
                            time:   [6.8402 ms 6.8536 ms 6.8635 ms]
                            thrpt:  [202.40 MiB/s 202.70 MiB/s 203.09 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) low mild
      1 (5.00%) high severe
    json-standalone/objects/find_context_arrays_cold
                            time:   [6.4649 ms 6.4755 ms 6.4869 ms]
                            thrpt:  [214.15 MiB/s 214.53 MiB/s 214.88 MiB/s]
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
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
                            time:   [706.83 µs 708.77 µs 711.84 µs]
                            thrpt:  [1.5496 GiB/s 1.5564 GiB/s 1.5606 GiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      3 (15.00%) high severe
        find_arrays    full=    10001  standalone=    10001
    json-standalone/nested/find_arrays
                            time:   [219.13 µs 219.27 µs 219.42 µs]
                            thrpt:  [5.0274 GiB/s 5.0308 GiB/s 5.0340 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_strings   full=   130000  standalone=   130000
    json-standalone/nested/find_strings
                            time:   [2.1638 ms 2.1666 ms 2.1708 ms]
                            thrpt:  [520.35 MiB/s 521.36 MiB/s 522.03 MiB/s]
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) high mild
      2 (10.00%) high severe
        find_members   full=   100000  standalone=    60000
    json-standalone/nested/find_members
                            time:   [1.3439 ms 1.3443 ms 1.3447 ms]
                            thrpt:  [839.99 MiB/s 840.26 MiB/s 840.53 MiB/s]
    │  context regions (strings): 130000
    json-standalone/nested/context
                            time:   [7.6583 ms 7.6794 ms 7.7012 ms]
                            thrpt:  [146.68 MiB/s 147.09 MiB/s 147.50 MiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
    │  full-vs-context-aware counts:
        find_context_objects full=    50000  context-aware=    50000
    json-standalone/nested/find_context_objects
                            time:   [901.57 µs 901.90 µs 902.21 µs]
                            thrpt:  [1.2227 GiB/s 1.2231 GiB/s 1.2235 GiB/s]
    Found 4 outliers among 20 measurements (20.00%)
      2 (10.00%) low mild
      2 (10.00%) high mild
        find_context_arrays full=    10001  context-aware=    10001
    json-standalone/nested/find_context_arrays
                            time:   [320.33 µs 320.62 µs 320.91 µs]
                            thrpt:  [3.4374 GiB/s 3.4405 GiB/s 3.4437 GiB/s]
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    json-standalone/nested/find_context_objects_cold
                            time:   [8.6151 ms 8.6324 ms 8.6518 ms]
                            thrpt:  [130.56 MiB/s 130.85 MiB/s 131.12 MiB/s]
    json-standalone/nested/find_context_arrays_cold
                            time:   [8.0414 ms 8.0746 ms 8.1100 ms]
                            thrpt:  [139.28 MiB/s 139.89 MiB/s 140.47 MiB/s]
    ```
</details>

**big (`--features benches/big`):**

#### Standalone - `find_*`
| Function       | `numbers`                  | `objects`                  | `nested`                   |
|----------------|----------------------------|----------------------------|----------------------------|
| `find_objects` | [9.3066 ms]=[22.911 GiB/s] | [47.227 ms]=[3.1092 GiB/s] | [75.172 ms]=[1.5913 GiB/s] |
| `find_arrays`  | [19.266 ms]=[11.067 GiB/s] | [13.004 ms]=[11.292 GiB/s] | [26.581 ms]=[4.5003 GiB/s] |
| `find_strings` | [9.2159 ms]=[23.136 GiB/s] | [200.20 ms]=[751.07 MiB/s] | [217.12 ms]=[564.19 MiB/s] |
| `find_members` | [9.2156 ms]=[23.137 GiB/s] | [221.06 ms]=[680.20 MiB/s] | [136.24 ms]=[899.13 MiB/s] |

#### Context-aware - `context()` + `find_context_*`
| Function               | `numbers`                  | `objects`                  | `nested`                   |
|------------------------|----------------------------|----------------------------|----------------------------|
| `context`              | [9.3452 ms]=[22.816 GiB/s] | [786.33 ms]=[191.22 MiB/s] | [869.23 ms]=[140.92 MiB/s] |
| `find_context_objects` | [9.3051 ms]=[22.915 GiB/s] | [57.840 ms]=[2.5387 GiB/s] | [94.031 ms]=[1.2722 GiB/s] |
| `find_context_arrays`  | [19.316 ms]=[11.039 GiB/s] | [19.371 ms]=[7.5802 GiB/s] | [35.942 ms]=[3.3282 GiB/s] |

#### Cold context - `find_context_*_cold`
| Function                    | `numbers`                  | `objects`                  | `nested`                   |
|-----------------------------|----------------------------|----------------------------|----------------------------|
| `find_context_objects_cold` | [18.626 ms]=[11.447 GiB/s] | [852.33 ms]=[176.41 MiB/s] | [962.08 ms]=[127.32 MiB/s] |
| `find_context_arrays_cold`  | [28.759 ms]=[7.4141 GiB/s] | [811.52 ms]=[185.28 MiB/s] | [906.91 ms]=[135.07 MiB/s] |

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
                            time:   [9.2891 ms 9.3066 ms 9.3242 ms]
                            thrpt:  [22.867 GiB/s 22.911 GiB/s 22.954 GiB/s]
                     change:
                            time:   [+43508% +44273% +45060%] (p = 0.00 < 0.05)
                            thrpt:  [-99.779% -99.775% -99.771%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_arrays    full=        1  standalone=        1
    json-standalone/numbers/find_arrays
                            time:   [19.240 ms 19.266 ms 19.300 ms]
                            thrpt:  [11.048 GiB/s 11.067 GiB/s 11.082 GiB/s]
                     change:
                            time:   [+42998% +43184% +43372%] (p = 0.00 < 0.05)
                            thrpt:  [-99.770% -99.769% -99.768%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_strings   full=        0  standalone=        0
    json-standalone/numbers/find_strings
                            time:   [9.1827 ms 9.2159 ms 9.2593 ms]
                            thrpt:  [23.028 GiB/s 23.136 GiB/s 23.220 GiB/s]
                     change:
                            time:   [+44475% +44736% +45020%] (p = 0.00 < 0.05)
                            thrpt:  [-99.778% -99.777% -99.776%]
                            Performance has regressed.
    Found 5 outliers among 20 measurements (25.00%)
      4 (20.00%) low mild
      1 (5.00%) high mild
        find_members   full=        0  standalone=        0
    json-standalone/numbers/find_members
                            time:   [9.1900 ms 9.2156 ms 9.2371 ms]
                            thrpt:  [23.083 GiB/s 23.137 GiB/s 23.202 GiB/s]
                     change:
                            time:   [+44486% +45589% +46654%] (p = 0.00 < 0.05)
                            thrpt:  [-99.786% -99.781% -99.776%]
                            Performance has regressed.
    │  context regions (strings): 0
    json-standalone/numbers/context
                            time:   [9.3313 ms 9.3452 ms 9.3582 ms]
                            thrpt:  [22.784 GiB/s 22.816 GiB/s 22.850 GiB/s]
                     change:
                            time:   [+44330% +45109% +45986%] (p = 0.00 < 0.05)
                            thrpt:  [-99.783% -99.779% -99.775%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
    │  full-vs-context-aware counts:
        find_context_objects full=        0  context-aware=        0
    json-standalone/numbers/find_context_objects
                            time:   [9.2882 ms 9.3051 ms 9.3187 ms]
                            thrpt:  [22.881 GiB/s 22.915 GiB/s 22.956 GiB/s]
                     change:
                            time:   [+45300% +45750% +46242%] (p = 0.00 < 0.05)
                            thrpt:  [-99.784% -99.782% -99.780%]
                            Performance has regressed.
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low mild
      1 (5.00%) high mild
      1 (5.00%) high severe
        find_context_arrays full=        1  context-aware=        1
    json-standalone/numbers/find_context_arrays
                            time:   [19.272 ms 19.316 ms 19.369 ms]
                            thrpt:  [11.008 GiB/s 11.039 GiB/s 11.064 GiB/s]
                     change:
                            time:   [+43350% +43624% +43910%] (p = 0.00 < 0.05)
                            thrpt:  [-99.773% -99.771% -99.770%]
                            Performance has regressed.
    json-standalone/numbers/find_context_objects_cold
                            time:   [18.547 ms 18.626 ms 18.696 ms]
                            thrpt:  [11.404 GiB/s 11.447 GiB/s 11.496 GiB/s]
                     change:
                            time:   [+47411% +47862% +48298%] (p = 0.00 < 0.05)
                            thrpt:  [-99.793% -99.791% -99.790%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) low mild
    json-standalone/numbers/find_context_arrays_cold
                            time:   [28.702 ms 28.759 ms 28.819 ms]
                            thrpt:  [7.3987 GiB/s 7.4141 GiB/s 7.4288 GiB/s]
                     change:
                            time:   [+45516% +45802% +46100%] (p = 0.00 < 0.05)
                            thrpt:  [-99.784% -99.782% -99.781%]
                            Performance has regressed.
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
                            time:   [47.074 ms 47.227 ms 47.440 ms]
                            thrpt:  [3.0952 GiB/s 3.1092 GiB/s 3.1193 GiB/s]
                     change:
                            time:   [+11692% +11726% +11763%] (p = 0.00 < 0.05)
                            thrpt:  [-99.157% -99.154% -99.152%]
                            Performance has regressed.
        find_arrays    full=        1  standalone=        1
    json-standalone/objects/find_arrays
                            time:   [12.978 ms 13.004 ms 13.035 ms]
                            thrpt:  [11.265 GiB/s 11.292 GiB/s 11.314 GiB/s]
                     change:
                            time:   [+36607% +36731% +36879%] (p = 0.00 < 0.05)
                            thrpt:  [-99.730% -99.728% -99.728%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_strings   full= 12000000  standalone= 12000000
    json-standalone/objects/find_strings
                            time:   [200.08 ms 200.20 ms 200.35 ms]
                            thrpt:  [750.51 MiB/s 751.07 MiB/s 751.53 MiB/s]
                     change:
                            time:   [+9886.7% +9893.7% +9901.7%] (p = 0.00 < 0.05)
                            thrpt:  [-99.000% -98.999% -98.999%]
                            Performance has regressed.
    Found 3 outliers among 20 measurements (15.00%)
      3 (15.00%) high severe
        find_members   full= 10000000  standalone= 10000000
    json-standalone/objects/find_members
                            time:   [220.97 ms 221.06 ms 221.16 ms]
                            thrpt:  [679.90 MiB/s 680.20 MiB/s 680.46 MiB/s]
                     change:
                            time:   [+9895.5% +9905.6% +9915.2%] (p = 0.00 < 0.05)
                            thrpt:  [-99.002% -99.001% -99.000%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    │  context regions (strings): 12000000
    Benchmarking json-standalone/objects/context: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 15.5s, or reduce sample count to 10.
    json-standalone/objects/context
                            time:   [785.11 ms 786.33 ms 787.66 ms]
                            thrpt:  [190.90 MiB/s 191.22 MiB/s 191.52 MiB/s]
                     change:
                            time:   [+12385% +12430% +12472%] (p = 0.00 < 0.05)
                            thrpt:  [-99.205% -99.202% -99.199%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    │  full-vs-context-aware counts:
        find_context_objects full=  2000000  context-aware=  2000000
    Benchmarking json-standalone/objects/find_context_objects: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 12.1s, enable flat sampling, or reduce sample count to 10.
    json-standalone/objects/find_context_objects
                            time:   [57.766 ms 57.840 ms 57.922 ms]
                            thrpt:  [2.5351 GiB/s 2.5387 GiB/s 2.5420 GiB/s]
                     change:
                            time:   [+11381% +11398% +11422%] (p = 0.00 < 0.05)
                            thrpt:  [-99.132% -99.130% -99.129%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_context_arrays full=        1  context-aware=        1
    json-standalone/objects/find_context_arrays
                            time:   [19.340 ms 19.371 ms 19.416 ms]
                            thrpt:  [7.5627 GiB/s 7.5802 GiB/s 7.5926 GiB/s]
                     change:
                            time:   [+24274% +24563% +24854%] (p = 0.00 < 0.05)
                            thrpt:  [-99.599% -99.595% -99.590%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
    Benchmarking json-standalone/objects/find_context_objects_cold: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 17.0s, or reduce sample count to 10.
    json-standalone/objects/find_context_objects_cold
                            time:   [851.25 ms 852.33 ms 853.38 ms]
                            thrpt:  [176.20 MiB/s 176.41 MiB/s 176.64 MiB/s]
                     change:
                            time:   [+12210% +12301% +12359%] (p = 0.00 < 0.05)
                            thrpt:  [-99.197% -99.194% -99.188%]
                            Performance has regressed.
    Benchmarking json-standalone/objects/find_context_arrays_cold: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 16.2s, or reduce sample count to 10.
    json-standalone/objects/find_context_arrays_cold
                            time:   [810.70 ms 811.52 ms 812.51 ms]
                            thrpt:  [185.06 MiB/s 185.28 MiB/s 185.47 MiB/s]
                     change:
                            time:   [+12407% +12431% +12455%] (p = 0.00 < 0.05)
                            thrpt:  [-99.203% -99.202% -99.200%]
                            Performance has regressed.
    Found 3 outliers among 20 measurements (15.00%)
      1 (5.00%) low mild
      2 (10.00%) high severe
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
                            time:   [74.331 ms 75.172 ms 75.952 ms]
                            thrpt:  [1.5750 GiB/s 1.5913 GiB/s 1.6093 GiB/s]
                     change:
                            time:   [+10467% +10578% +10679%] (p = 0.00 < 0.05)
                            thrpt:  [-99.072% -99.063% -99.054%]
                            Performance has regressed.
        find_arrays    full=  1000001  standalone=  1000001
    json-standalone/nested/find_arrays
                            time:   [26.558 ms 26.581 ms 26.601 ms]
                            thrpt:  [4.4969 GiB/s 4.5003 GiB/s 4.5043 GiB/s]
                     change:
                            time:   [+12007% +12022% +12039%] (p = 0.00 < 0.05)
                            thrpt:  [-99.176% -99.175% -99.174%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high mild
        find_strings   full= 13000000  standalone= 13000000
    json-standalone/nested/find_strings
                            time:   [217.04 ms 217.12 ms 217.21 ms]
                            thrpt:  [563.94 MiB/s 564.19 MiB/s 564.39 MiB/s]
                     change:
                            time:   [+9903.4% +9918.9% +9930.8%] (p = 0.00 < 0.05)
                            thrpt:  [-99.003% -99.002% -99.000%]
                            Performance has regressed.
    Found 4 outliers among 20 measurements (20.00%)
      1 (5.00%) high mild
      3 (15.00%) high severe
        find_members   full= 10000000  standalone=  6000000
    json-standalone/nested/find_members
                            time:   [136.18 ms 136.24 ms 136.30 ms]
                            thrpt:  [898.69 MiB/s 899.13 MiB/s 899.48 MiB/s]
                     change:
                            time:   [+10030% +10035% +10041%] (p = 0.00 < 0.05)
                            thrpt:  [-99.014% -99.013% -99.013%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      1 (5.00%) high mild
      1 (5.00%) high severe
    │  context regions (strings): 13000000
    Benchmarking json-standalone/nested/context: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 17.2s, or reduce sample count to 10.
    json-standalone/nested/context
                            time:   [866.72 ms 869.23 ms 871.98 ms]
                            thrpt:  [140.48 MiB/s 140.92 MiB/s 141.33 MiB/s]
                     change:
                            time:   [+11200% +11251% +11306%] (p = 0.00 < 0.05)
                            thrpt:  [-99.123% -99.119% -99.115%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    │  full-vs-context-aware counts:
        find_context_objects full=  5000000  context-aware=  5000000
    Benchmarking json-standalone/nested/find_context_objects: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 19.8s, enable flat sampling, or reduce sample count to 10.
    json-standalone/nested/find_context_objects
                            time:   [93.977 ms 94.031 ms 94.081 ms]
                            thrpt:  [1.2715 GiB/s 1.2722 GiB/s 1.2729 GiB/s]
                     change:
                            time:   [+10319% +10326% +10334%] (p = 0.00 < 0.05)
                            thrpt:  [-99.042% -99.041% -99.040%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) high severe
        find_context_arrays full=  1000001  context-aware=  1000001
    json-standalone/nested/find_context_arrays
                            time:   [35.923 ms 35.942 ms 35.965 ms]
                            thrpt:  [3.3261 GiB/s 3.3282 GiB/s 3.3300 GiB/s]
                     change:
                            time:   [+11113% +11125% +11137%] (p = 0.00 < 0.05)
                            thrpt:  [-99.110% -99.109% -99.108%]
                            Performance has regressed.
    Found 2 outliers among 20 measurements (10.00%)
      2 (10.00%) high mild
    Benchmarking json-standalone/nested/find_context_objects_cold: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 19.2s, or reduce sample count to 10.
    json-standalone/nested/find_context_objects_cold
                            time:   [960.16 ms 962.08 ms 963.75 ms]
                            thrpt:  [127.10 MiB/s 127.32 MiB/s 127.58 MiB/s]
                     change:
                            time:   [+10983% +11019% +11052%] (p = 0.00 < 0.05)
                            thrpt:  [-99.103% -99.101% -99.098%]
                            Performance has regressed.
    Found 1 outliers among 20 measurements (5.00%)
      1 (5.00%) low mild
    Benchmarking json-standalone/nested/find_context_arrays_cold: Warming up for 3.0000 s
    Warning: Unable to complete 20 samples in 10.0s. You may wish to increase target time to 18.0s, or reduce sample count to 10.
    json-standalone/nested/find_context_arrays_cold
                            time:   [905.08 ms 906.91 ms 908.94 ms]
                            thrpt:  [134.77 MiB/s 135.07 MiB/s 135.34 MiB/s]
                     change:
                            time:   [+11083% +11127% +11170%] (p = 0.00 < 0.05)
                            thrpt:  [-99.113% -99.109% -99.106%]
                            Performance has regressed.
    Found 3 outliers among 20 measurements (15.00%)
      3 (15.00%) high mild
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
