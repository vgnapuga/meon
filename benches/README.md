# meon — Benchmarks

Throughput benchmarks for the [`meon-md`](https://github.com/vgnapuga/meon/blob/main/meon-md/README.md)
and [`meon-json`](https://github.com/vgnapuga/meon/blob/main/meon-json/README.md)
reference grammars, built on the [`meon`](https://github.com/vgnapuga/meon/blob/main/meon/README.md)
engine. They exist to track engine performance across changes and feature
flags, and to set its flat span vectors next to parsers that build other
output shapes (see [Scope & fairness](#scope--fairness)).

| Bench                 | Measures                                                                                                                                           |
|-----------------------|----------------------------------------------------------------------------------------------------------------------------------------------------|
| `meon-md_parse`       | `MarkdownParser::parse` — full single-pass parse.                                                                                                  |
| `meon-md_standalone`  | `find_*` iterators — one element kind, no context; plus the `context()` map build and the context-aware `find_context_*` variants (warm and cold). |
| `meon-md_compare`     | meon-md vs `pulldown-cmark` / `comrak` — cross-parser throughput.                                                                                  |
| `meon-md_offsets`     | small corpora with `Span<u16/u32/u64>`; 32 MiB corpora with `u32/u64`.                                                                             |
| `meon-json_parse`     | `JsonParser::parse` (+ `type_scalars`) — structural / typed parse.                                                                                 |
| `meon-json_standalone`| `find_*` iterators, the `context()` map and `find_context_*` variants.                                                                             |
| `meon-json_offsets`   | small corpora with `Span<u16/u32/u64>`; 32 MiB corpora with `u32/u64`.                                                                             |
| `meon-json_compare`   | meon-json vs `simd-json` / `sonic-rs` — cross-parser throughput.                                                                                   |

### Cold and warmed scenarios

- `meon-md_parse` and `meon-json_parse` — **isolated**: only the engine parser
  is timed; no cross-parser sanity pass or other parser's timed benchmark runs
  before it.
- `meon-md_compare` and `meon-json_compare` — **warmed**: all compared parsers
  run in one process after a one-time sanity/validation pass, so the allocator,
  pages and caches are shared and already warm. `pulldown-cmark`, `comrak`,
  `simd-json` and `sonic-rs` are measured under the same conditions as the
  corresponding meon parser.
- Absolute numbers from `_parse` and `_compare` answer different questions and
  are not interchangeable.

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

Each base document is tiled `REPEAT_COUNT` times so the working set
comfortably exceeds cache. The `small` feature keeps the default tiling;
`big` multiplies it by 100. With neither feature set, `small` is used.

| Corpus  | Shape                                                                  | Stresses                                                          |
|---------|------------------------------------------------------------------------|-------------------------------------------------------------------|
| `plain` | Prose only, no markup.                                                 | Fallback/text path, line loop. Ceiling case (near-pure scanning). |
| `hot`   | Light, evenly spread markup (~one of each common inline per paragraph).| Typical real-world document.                                      |
| `heavy` | Dense: headings, rules, quotes, fences, lists, nested inline.          | Every rule family at once, including nesting. Stress case.        |

> **Synthetic data notice.** Regular generated corpora do not establish a
> lower bound for production throughput. Real inputs differ in delimiter
> distribution, nesting, matched/unmatched rules and size; measure the workload
> you intend to parse.

Current Markdown reports include a pulldown-cmark composition cross-check
before the timed loops. Counts exclude container paragraphs and collapse
direct emphasis/strong wrappers to bold-italic; text is compared by byte
coverage excluding LF. A check mark means agreement under this mapping, not
CommonMark compliance or equality of all spans.

Current composition and historical measurements are in
[***MD_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/MD_COMPARE.md).

### JSON corpora (`meon-json_*`)

Each corpus is one valid top-level JSON array, scaled by `REPEAT_COUNT`
(`benches/benches/docs_json.rs`); the `small` and `big` runs differ only in
that factor, selected by the corresponding Cargo feature.

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
cargo bench --bench meon-md_offsets
cargo bench --bench meon-json_parse
cargo bench --bench meon-json_standalone
cargo bench --bench meon-json_offsets
cargo bench --bench meon-json_compare

# Nightly, AVX2 SIMD path, tuned for the host CPU:
RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_parse        --features meon/avx2
RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_standalone   --features meon/avx2
RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_compare      --features meon/avx2
RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_offsets      --features meon/avx2
RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-json_parse      --features meon/avx2
RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-json_standalone --features meon/avx2
RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-json_offsets    --features meon/avx2
RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-json_compare    --features meon/avx2

# Working-set size: `small` (default) or `big` (100x larger).
cargo bench --bench meon-md_parse        --features benches/small
cargo bench --bench meon-json_parse      --features benches/big
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
OS:              NixOS 26.05
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
| `plain` | 5.09 -> 4.73 | 0.07% -> 0.06% | 1.42% -> 1.72% |
| `hot`   | 4.32 -> 3.94 | 0.08% -> 0.09% | 4.58% -> 3.09% |
| `heavy` | 3.86 -> 3.61 | 0.13% -> 0.11% | 6.79% -> 2.62% |

IPC holds at 3.6-5.1 with branch-misses near 0.1%, and the cache-miss rate
stays low as the input scales ~100x from `small` to `big` — the flat
span-vector working set stays largely cache-resident.

<details>
    <summary>small</summary>
    
    ```
    Performance counter stats for 'cargo bench --bench meon-md_parse --features benches/small -- plain/full --profile-time 10' (10 runs):
    
             10 229,55 msec task-clock:u                    #    0,999 CPUs utilized               ( +-  0,09% )
        42 940 004 250      cycles:u                        #    4,198 GHz                         ( +-  0,09% )  (83,36%)
       218 764 234 353      instructions:u                  #    5,09  insn per cycle              ( +-  0,10% )  (83,33%)
        35 086 851 687      branches:u                      #    3,430 G/sec                       ( +-  0,10% )  (83,32%)
            25 611 871      branch-misses:u                 #    0,07% of all branches             ( +-  0,56% )  (83,34%)
         2 032 549 469      cache-references:u              #  198,694 M/sec                       ( +-  0,10% )  (83,35%)
            28 890 576      cache-misses:u                  #    1,42% of all cache refs           ( +-  0,69% )  (83,32%)
    
              10,23617 +- 0,00881 seconds time elapsed  ( +-  0,09% )
    
    
    Performance counter stats for 'cargo bench --bench meon-md_parse --features benches/small -- hot/full --profile-time 10' (10 runs):
    
            10 232,54 msec task-clock:u                     #    0,998 CPUs utilized               ( +-  0,08% )
       43 086 199 662      cycles:u                         #    4,211 GHz                         ( +-  0,11% )  (83,36%)
      186 214 408 966      instructions:u                   #    4,32  insn per cycle              ( +-  0,15% )  (83,34%)
       35 219 716 781      branches:u                       #    3,442 G/sec                       ( +-  0,15% )  (83,25%)
           28 610 837      branch-misses:u                  #    0,08% of all branches             ( +-  0,13% )  (83,34%)
          700 157 070      cache-references:u               #   68,425 M/sec                       ( +-  0,20% )  (83,36%)
           32 053 100      cache-misses:u                   #    4,58% of all cache refs           ( +-  1,35% )  (83,38%)
    
              10,25184 +- 0,00836 seconds time elapsed  ( +-  0,08% )
    
    
    Performance counter stats for 'cargo bench --bench meon-md_parse --features benches/small -- heavy/full --profile-time 10' (10 runs):
    
            10 240,16 msec task-clock:u                     #    0,999 CPUs utilized               ( +-  0,09% )
       43 205 511 924      cycles:u                         #    4,219 GHz                         ( +-  0,11% )  (83,33%)
      166 772 735 749      instructions:u                   #    3,86  insn per cycle              ( +-  0,13% )  (83,32%)
       34 276 261 321      branches:u                       #    3,347 G/sec                       ( +-  0,14% )  (83,33%)
           44 907 899      branch-misses:u                  #    0,13% of all branches             ( +-  0,70% )  (83,34%)
          724 374 118      cache-references:u               #   70,739 M/sec                       ( +-  0,35% )  (83,36%)
           49 163 554      cache-misses:u                   #    6,79% of all cache refs           ( +-  2,11% )  (83,35%)
    
              10,25073 +- 0,00908 seconds time elapsed  ( +-  0,09% )
    ```

</details>

<details>
    <summary>big</summary>
    
    ```
    Performance counter stats for 'cargo bench --bench meon-md_parse --features benches/big -- plain/full --profile-time 10' (10 runs):
   
            15 766,90 msec task-clock:u                     #    0,999 CPUs utilized               ( +-  0,13% )
       57 415 448 091      cycles:u                         #    3,642 GHz                         ( +-  0,12% )  (83,34%)
      271 480 901 406      instructions:u                   #    4,73  insn per cycle              ( +-  0,21% )  (83,35%)
       46 181 629 676      branches:u                       #    2,929 G/sec                       ( +-  0,20% )  (83,33%)
           27 370 876      branch-misses:u                  #    0,06% of all branches             ( +-  3,81% )  (83,33%)
        2 646 265 051      cache-references:u               #  167,837 M/sec                       ( +-  0,21% )  (83,33%)
           45 505 010      cache-misses:u                   #    1,72% of all cache refs           ( +-  0,48% )  (83,33%)
   
              15,7794 +- 0,0188 seconds time elapsed  ( +-  0,12% )
    
    
     Performance counter stats for 'cargo bench --bench meon-md_parse --features benches/big -- hot/full --profile-time 10' (10 runs):
    
             15 704,54 msec task-clock:u                     #    0,999 CPUs utilized               ( +-  0,10% )
        45 345 606 824      cycles:u                         #    2,887 GHz                         ( +-  0,14% )  (83,35%)
       178 657 494 273      instructions:u                   #    3,94  insn per cycle              ( +-  0,19% )  (83,34%)
        35 051 476 785      branches:u                       #    2,232 G/sec                       ( +-  0,19% )  (83,34%)
            33 024 256      branch-misses:u                  #    0,09% of all branches             ( +-  0,44% )  (83,33%)
         1 213 788 136      cache-references:u               #   77,289 M/sec                       ( +-  0,17% )  (83,32%)
            37 526 901      cache-misses:u                   #    3,09% of all cache refs           ( +-  0,68% )  (83,34%)
    
               15,7154 +- 0,0158 seconds time elapsed  ( +-  0,10% )
    
    
     Performance counter stats for 'cargo bench --bench meon-md_parse --features benches/big -- heavy/full --profile-time 10' (10 runs):
    
             15 703,29 msec task-clock:u                     #    0,999 CPUs utilized               ( +-  0,12% )
        46 411 408 196      cycles:u                         #    2,956 GHz                         ( +-  0,17% )  (83,35%)
       167 358 470 651      instructions:u                   #    3,61  insn per cycle              ( +-  0,18% )  (83,35%)
        34 841 370 726      branches:u                       #    2,219 G/sec                       ( +-  0,18% )  (83,33%)
            39 704 223      branch-misses:u                  #    0,11% of all branches             ( +-  0,45% )  (83,32%)
         1 363 939 130      cache-references:u               #   86,857 M/sec                       ( +-  0,13% )  (83,31%)
            35 731 776      cache-misses:u                   #    2,62% of all cache refs           ( +-  0,45% )  (83,36%)
    
               15,7176 +- 0,0194 seconds time elapsed  ( +-  0,12% )
    ```

</details>
