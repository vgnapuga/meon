# meon — Benchmarks

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

* [***FAQ.md***](https://github.com/vgnapuga/meon/blob/main/FAQ.md)
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

- **`max_nest` costs stack space, not per-line work.** The block-level
  active-block stack and the inline engine's bounded stacks are `[T; max_nest]`
  arrays initialised once per call rather than once per line, so per-line cost
  is flat in the cap and throughput follows the nesting a document actually
  uses. `meon-md` uses `4`, `meon-json` uses `64`. A very large cap still
  enlarges the stack frame, so keep it to what the grammar needs.
- **Output vectors allocate on first use.** Each field reserves
  `source.len() / div` elements when its rule first matches and stays
  unallocated otherwise, so the timed region pays for the element kinds the
  document contains rather than for every kind the grammar declares.
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

         10 159,29 msec task-clock:u                     #    1,000 CPUs utilized               ( +-  0,04% )
    43 264 463 783      cycles:u                         #    4,259 GHz                         ( +-  0,07% )  (83,35%)
   218 594 362 736      instructions:u                   #    5,05  insn per cycle              ( +-  0,07% )  (83,35%)
    34 020 712 611      branches:u                       #    3,349 G/sec                       ( +-  0,06% )  (83,33%)
        35 635 825      branch-misses:u                  #    0,10% of all branches             ( +-  0,08% )  (83,32%)
     1 880 142 537      cache-references:u               #  185,066 M/sec                       ( +-  0,06% )  (83,35%)
        28 214 445      cache-misses:u                   #    1,50% of all cache refs           ( +-  1,19% )  (83,33%)

          10,16244 +- 0,00413 seconds time elapsed  ( +-  0,04% )


 Performance counter stats for 'cargo bench --bench meon-md_parse -- hot/full --profile-time 10' (10 runs):

         10 180,13 msec task-clock:u                     #    1,000 CPUs utilized               ( +-  0,05% )
    43 330 876 527      cycles:u                         #    4,256 GHz                         ( +-  0,06% )  (83,33%)
   199 103 802 420      instructions:u                   #    4,59  insn per cycle              ( +-  0,05% )  (83,36%)
    40 355 490 829      branches:u                       #    3,964 G/sec                       ( +-  0,05% )  (83,36%)
        31 818 168      branch-misses:u                  #    0,08% of all branches             ( +-  0,14% )  (83,34%)
       771 867 251      cache-references:u               #   75,821 M/sec                       ( +-  0,10% )  (83,33%)
        31 139 456      cache-misses:u                   #    4,03% of all cache refs           ( +-  1,63% )  (83,31%)

          10,18336 +- 0,00475 seconds time elapsed  ( +-  0,05% )


 Performance counter stats for 'cargo bench --bench meon-md_parse -- heavy/full --profile-time 10' (10 runs):

         10 185,68 msec task-clock:u                     #    0,999 CPUs utilized               ( +-  0,02% )
    43 394 650 499      cycles:u                         #    4,260 GHz                         ( +-  0,04% )  (83,31%)
   177 936 216 759      instructions:u                   #    4,10  insn per cycle              ( +-  0,06% )  (83,35%)
    36 857 702 085      branches:u                       #    3,619 G/sec                       ( +-  0,06% )  (83,34%)
        37 668 617      branch-misses:u                  #    0,10% of all branches             ( +-  0,07% )  (83,33%)
       666 666 978      cache-references:u               #   65,451 M/sec                       ( +-  0,19% )  (83,35%)
        40 337 326      cache-misses:u                   #    6,05% of all cache refs           ( +-  1,94% )  (83,33%)

          10,19303 +- 0,00501 seconds time elapsed  ( +-  0,05% )
```

</details>

<details>
<summary>big</summary>

```
 Performance counter stats for 'cargo bench --bench meon-md_parse -- plain/full --profile-time 10' (10 runs):

         10 884,77 msec task-clock:u                     #    1,000 CPUs utilized               ( +-  0,07% )
    45 012 693 533      cycles:u                         #    4,135 GHz                         ( +-  0,07% )  (83,36%)
   221 459 271 552      instructions:u                   #    4,92  insn per cycle              ( +-  0,13% )  (83,33%)
    34 666 532 531      branches:u                       #    3,185 G/sec                       ( +-  0,13% )  (83,33%)
        31 592 758      branch-misses:u                  #    0,09% of all branches             ( +-  0,13% )  (83,32%)
     1 927 288 769      cache-references:u               #  177,063 M/sec                       ( +-  0,14% )  (83,35%)
        29 491 657      cache-misses:u                   #    1,53% of all cache refs           ( +-  0,79% )  (83,33%)

          10,88755 +- 0,00803 seconds time elapsed  ( +-  0,07% )


 Performance counter stats for 'cargo bench --bench meon-md_parse -- hot/full --profile-time 10' (10 runs):

         10 876,84 msec task-clock:u                     #    1,000 CPUs utilized               ( +-  0,16% )
    32 806 757 908      cycles:u                         #    3,016 GHz                         ( +-  0,12% )  (83,34%)
   139 551 699 389      instructions:u                   #    4,25  insn per cycle              ( +-  0,12% )  (83,34%)
    28 186 238 995      branches:u                       #    2,591 G/sec                       ( +-  0,13% )  (83,34%)
        25 475 333      branch-misses:u                  #    0,09% of all branches             ( +-  0,16% )  (83,33%)
       675 845 164      cache-references:u               #   62,136 M/sec                       ( +-  0,16% )  (83,34%)
        24 629 452      cache-misses:u                   #    3,64% of all cache refs           ( +-  1,79% )  (83,33%)

           10,8816 +- 0,0190 seconds time elapsed  ( +-  0,17% )


 Performance counter stats for 'cargo bench --bench meon-md_parse -- heavy/full --profile-time 10' (10 runs):

         10 796,42 msec task-clock:u                     #    1,000 CPUs utilized               ( +-  0,29% )
    33 353 325 730      cycles:u                         #    3,089 GHz                         ( +-  0,32% )  (83,34%)
   128 189 219 133      instructions:u                   #    3,84  insn per cycle              ( +-  0,32% )  (83,34%)
    26 426 568 356      branches:u                       #    2,448 G/sec                       ( +-  0,32% )  (83,33%)
        30 807 557      branch-misses:u                  #    0,12% of all branches             ( +-  0,32% )  (83,34%)
       842 155 265      cache-references:u               #   78,003 M/sec                       ( +-  0,31% )  (83,34%)
        27 096 943      cache-misses:u                   #    3,22% of all cache refs           ( +-  0,71% )  (83,34%)

           10,7993 +- 0,0313 seconds time elapsed  ( +-  0,29% )
```

</details>
