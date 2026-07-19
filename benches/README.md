# meon — Benchmarks

EN | [**RU**](https://github.com/vgnapuga/meon/blob/main/benches/README_RU.md)

Throughput benchmarks for the [`meon-md`](https://github.com/vgnapuga/meon/blob/main/meon-md/README.md)
and [`meon-json`](https://github.com/vgnapuga/meon/blob/main/meon-json/README.md)
reference grammars, built on the [`meon`](https://github.com/vgnapuga/meon/blob/main/meon/README.md)
engine. They exist to track engine performance across changes and feature
flags, and to set its flat span vectors next to parsers that build other
output shapes (see [Scope & fairness](#scope--fairness)).

| Bench                 | Measures                                                             |
|-----------------------|----------------------------------------------------------------------|
| `meon-md_parse`       | `MarkdownParser::parse` — full single-pass parse.                    |
| `meon-md_standalone`  | `find_*` iterators — one element kind, no context; plus the `context()` map build and the context-aware `find_context_*` variants (warm and cold). |
| `meon-md_compare`     | meon-md vs `pulldown-cmark` / `comrak` — cross-parser throughput.    |
| `meon-json_parse`     | `JsonParser::parse` (+ `type_scalars`) — structural / typed parse.   |
| `meon-json_compare`   | meon-json vs `simd-json` / `sonic-rs` — cross-parser throughput.     |

Per-corpus composition reports, full result tables, and the cross-parser
numbers live in their own documents — this file is the overview, how-to-run,
fairness frame and test hardware. For the cross-parser comparisons see
[***MD_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/MD_COMPARE.md)
(Markdown) and
[***JSON_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/JSON_COMPARE.md)
(JSON).

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
* ***BENCHMARKS.md***    <--
  * [***MD_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/MD_COMPARE.md)
  * [***JSON_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/JSON_COMPARE.md)
* [***FUZZING.md***](https://github.com/vgnapuga/meon/blob/main/fuzz/README.md)

---

## Corpora

### Markdown corpora (`meon-md_*`)

Each base document is tiled `REPEAT_COUNT` times (default `10`) so the working
set comfortably exceeds cache.

| Corpus  | Shape                                                                  | Stresses                                                          |
|---------|------------------------------------------------------------------------|-------------------------------------------------------------------|
| `plain` | Prose only, no markup.                                                 | Fallback/text path, line loop. Ceiling case (near-pure scanning). |
| `hot`   | Light, evenly spread markup (~one of each common inline per paragraph).| Typical real-world document.                                      |
| `heavy` | Dense: headings, rules, quotes, fences, lists, nested inline.          | Every rule family at once, including nesting. Stress case.        |

> **Synthetic data notice.** These corpora are generated programmatically with
> uniform, predictable structure. Real-world documents typically have **lower
> element density** than `hot` or `heavy` — and lower density means less
> per-element work, so real throughput usually sits **at or above** the
> `hot`/`heavy` numbers. Read `hot`/`heavy` as a conservative lower bound, with
> `plain` (markup-free) as the ceiling, and your workload somewhere in between.

Exact per-corpus element counts are in
[***MD_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/MD_COMPARE.md).

### JSON corpora (`meon-json_*`)

Each corpus is one valid top-level JSON array, scaled by `COUNT`
(`benches/benches/docs_json.rs`); the `small` and `big` runs differ only in
`COUNT`.

| Corpus    | Shape                                                                  | Stresses                                                                 |
|-----------|------------------------------------------------------------------------|--------------------------------------------------------------------------|
| `numbers` | Flat array of numbers / bools / nulls.                                 | Scalar scanning. meon emits span vectors; validating parsers parse every number. |
| `objects` | Array of flat objects with mixed-typed fields.                         | Members, keys, typed scalars. A typical API payload.                     |
| `nested`  | Array of moderately nested objects (objects-in-objects, small arrays). | The unified nesting stack and the string rule.                           |

> **Synthetic data notice.** These corpora are generated programmatically with
> uniform structure; real JSON is less regular. Treat the figures as a
> demonstration of the architectural difference (a structural reader vs a
> validating parser), not as expected production throughput.

Exact per-corpus composition is in
[***JSON_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/JSON_COMPARE.md).

---

## Running

Inside `nix develop`:

```sh
# Stable, scalar SWAR path:
cargo bench --bench meon-md_parse
cargo bench --bench meon-md_standalone
cargo bench --bench meon-md_compare
cargo bench --bench meon-json_parse
cargo bench --bench meon-json_compare

# Nightly, AVX2 SIMD path, tuned for the host CPU:
RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_parse        --features avx2
RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_standalone   --features avx2
RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_compare      --features avx2
RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-json_parse      --features avx2
RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-json_compare    --features avx2
```

Criterion knobs (`SAMPLE_SIZE`, `SAMPLE_TIME`, `WARMUP_TIME`) live in
`benches/benches/docs_md.rs` and `benches/benches/docs_json.rs`. Defaults favour
a quick local run; raise them for publication-grade numbers.

---

## Scope & fairness

- **Intra-engine first.** `meon-md_parse` / `meon-md_standalone` /
  `meon-json_parse` measure *this* engine over *these* corpora — "did my
  change regress?" and "how much does AVX2 help?".
- **Cross-parser comparisons are architectural.** `meon-md` emits flat span
  vectors for a Markdown *subset* (no AST, reference-link resolution, or
  rendering); `meon-json` is a *structural reader* (no validation, number
  parsing, or string unescaping). The comparisons — against `pulldown-cmark` /
  `comrak`
  ([***MD_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/MD_COMPARE.md))
  and against `simd-json` / `sonic-rs`
  ([***JSON_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/JSON_COMPARE.md))
  — are framed there as different jobs with different output shapes: span
  vectors on one side, an event stream, AST, tape or owned value on the other.
- **End-to-end cost.** The timed region includes the internal `Vec`
  allocations, because that is what a real caller pays. Input and output are
  `black_box`-ed; document generation is outside the timed region.

---

## Performance notes

- **`max_nest` has a flat per-line cost.** The block-level active-block stack
  and the inline engine's bounded stacks are `[T; max_nest]` arrays
  zero-initialised on *every* `parse_block!` / `parse_inline!` call, regardless
  of whether that line actually nests. Set `max_nest` to the smallest value
  your grammar needs (`meon-md` uses `4`, `meon-json` uses `64`); a larger cap
  costs throughput on every inline-bearing line whether or not anything nests.
- **AVX-512 is implemented but not benchmarked.** The `avx512` feature exists
  (see [`swar.rs`](https://github.com/vgnapuga/meon/blob/main/meon/src/swar.rs))
  but no AVX-512 hardware was available during development. Contributions with
  real numbers are welcome.

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

## Microarchitecture

Hardware counters for the full `meon-md_parse` pass over each corpus, taken on
the hardware above (`perf stat`, 10 runs, user-space counters, stable build,
`--profile-time 10`). Each cell reads `small -> big`:

| Corpus  | insn/cycle   | branch-misses  | cache-misses   |
|---------|--------------|----------------|----------------|
| `plain` | 4.94 -> 4.82 | 0.11% -> 0.09% | 1.61% -> 1.58% |
| `hot`   | 4.55 -> 4.27 | 0.08% -> 0.09% | 4.20% -> 3.12% |
| `heavy` | 4.11 -> 3.85 | 0.11% -> 0.12% | 6.74% -> 2.88% |

IPC holds at 3.9-4.9 with branch-misses near 0.1%, and the cache-miss rate
does not grow as the input scales ~100x from `small` to `big` — the flat
span-vector working set stays cache-resident.

<details>
<summary>small</summary>

```
 Performance counter stats for 'cargo bench --bench meon-md_parse -- plain/full --profile-time 10' (10 runs):

         10 188,03 msec task-clock:u                     #    0,999 CPUs utilized               ( +-  0,08% )
    41 703 907 951      cycles:u                         #    4,093 GHz                         ( +-  0,09% )  (83,32%)
   206 061 678 743      instructions:u                   #    4,94  insn per cycle              ( +-  0,10% )  (83,34%)
    31 784 289 849      branches:u                       #    3,120 G/sec                       ( +-  0,10% )  (83,36%)
        34 402 750      branch-misses:u                  #    0,11% of all branches             ( +-  0,09% )  (83,35%)
     1 772 186 919      cache-references:u               #  173,948 M/sec                       ( +-  0,11% )  (83,33%)
        28 472 216      cache-misses:u                   #    1,61% of all cache refs           ( +-  2,26% )  (83,32%)

           10,1941 +- 0,0103 seconds time elapsed  ( +-  0,10% )

 Performance counter stats for 'cargo bench --bench meon-md_parse -- hot/full --profile-time 10' (10 runs):

         10 197,21 msec task-clock:u                     #    0,999 CPUs utilized               ( +-  0,09% )
    43 358 916 187      cycles:u                         #    4,252 GHz                         ( +-  0,07% )  (83,31%)
   197 334 384 022      instructions:u                   #    4,55  insn per cycle              ( +-  0,07% )  (83,35%)
    38 453 951 651      branches:u                       #    3,771 G/sec                       ( +-  0,07% )  (83,35%)
        31 655 793      branch-misses:u                  #    0,08% of all branches             ( +-  0,15% )  (83,35%)
       771 926 269      cache-references:u               #   75,700 M/sec                       ( +-  0,07% )  (83,34%)
        32 419 197      cache-misses:u                   #    4,20% of all cache refs           ( +-  0,75% )  (83,33%)

          10,20385 +- 0,00828 seconds time elapsed  ( +-  0,08% )

 Performance counter stats for 'cargo bench --bench meon-md_parse -- heavy/full --profile-time 10' (10 runs):

         10 200,95 msec task-clock:u                     #    0,999 CPUs utilized               ( +-  0,10% )
    43 380 829 468      cycles:u                         #    4,253 GHz                         ( +-  0,08% )  (83,31%)
   178 171 172 323      instructions:u                   #    4,11  insn per cycle              ( +-  0,10% )  (83,36%)
    36 240 505 684      branches:u                       #    3,553 G/sec                       ( +-  0,10% )  (83,35%)
        38 582 525      branch-misses:u                  #    0,11% of all branches             ( +-  0,21% )  (83,33%)
       692 203 903      cache-references:u               #   67,857 M/sec                       ( +-  0,09% )  (83,34%)
        46 647 503      cache-misses:u                   #    6,74% of all cache refs           ( +-  1,26% )  (83,34%)

           10,2061 +- 0,0101 seconds time elapsed  ( +-  0,10% )
```

</details>

<details>
<summary>big</summary>

```
 Performance counter stats for 'cargo bench --bench meon-md_parse -- plain/full --profile-time 10' (10 runs):

         10 839,57 msec task-clock:u                     #    0,999 CPUs utilized               ( +-  0,07% )
    44 748 388 452      cycles:u                         #    4,128 GHz                         ( +-  0,07% )  (83,33%)
   215 463 976 982      instructions:u                   #    4,82  insn per cycle              ( +-  0,30% )  (83,34%)
    33 406 712 411      branches:u                       #    3,082 G/sec                       ( +-  0,29% )  (83,35%)
        30 402 477      branch-misses:u                  #    0,09% of all branches             ( +-  0,34% )  (83,34%)
     1 856 418 129      cache-references:u               #  171,263 M/sec                       ( +-  0,35% )  (83,33%)
        29 363 581      cache-misses:u                   #    1,58% of all cache refs           ( +-  2,97% )  (83,34%)

          10,84521 +- 0,00770 seconds time elapsed  ( +-  0,07% )

 Performance counter stats for 'cargo bench --bench meon-md_parse -- hot/full --profile-time 10' (10 runs):

         10 831,93 msec task-clock:u                     #    0,999 CPUs utilized               ( +-  0,28% )
    32 349 636 825      cycles:u                         #    2,987 GHz                         ( +-  0,28% )  (83,34%)
   138 012 462 194      instructions:u                   #    4,27  insn per cycle              ( +-  0,27% )  (83,33%)
    26 822 349 966      branches:u                       #    2,476 G/sec                       ( +-  0,27% )  (83,35%)
        25 188 136      branch-misses:u                  #    0,09% of all branches             ( +-  0,26% )  (83,34%)
       673 691 545      cache-references:u               #   62,195 M/sec                       ( +-  0,30% )  (83,33%)
        21 031 825      cache-misses:u                   #    3,12% of all cache refs           ( +-  2,04% )  (83,33%)

           10,8402 +- 0,0289 seconds time elapsed  ( +-  0,27% )

 Performance counter stats for 'cargo bench --bench meon-md_parse -- heavy/full --profile-time 10' (10 runs):

         10 852,56 msec task-clock:u                     #    0,999 CPUs utilized               ( +-  0,09% )
    33 022 729 384      cycles:u                         #    3,043 GHz                         ( +-  0,05% )  (83,35%)
   127 002 621 975      instructions:u                   #    3,85  insn per cycle              ( +-  0,01% )  (83,34%)
    25 708 883 847      branches:u                       #    2,369 G/sec                       ( +-  0,01% )  (83,36%)
        31 236 788      branch-misses:u                  #    0,12% of all branches             ( +-  0,10% )  (83,33%)
       844 217 588      cache-references:u               #   77,790 M/sec                       ( +-  0,09% )  (83,32%)
        24 276 598      cache-misses:u                   #    2,88% of all cache refs           ( +-  1,57% )  (83,32%)

          10,86128 +- 0,00872 seconds time elapsed  ( +-  0,08% )
```

</details>
