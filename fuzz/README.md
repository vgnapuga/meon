# meon — Fuzzing

EN | [**RU**](https://github.com/vgnapuga/meon/blob/main/fuzz/README_RU.md)

Coverage-guided fuzzing of the [`meon`](https://github.com/vgnapuga/meon/blob/main/meon/README.md)
engine through the [`meon-md`](https://github.com/vgnapuga/meon/blob/main/meon-md/README.md)
reference grammar, using [`cargo-fuzz`](https://github.com/rust-fuzz/cargo-fuzz)
(libFuzzer).

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
* [***BENCHMARKS.md***](https://github.com/vgnapuga/meon/blob/main/meon/benches/README.md)
* ***FUZZING.md***    <--

---

## Target

A single target — `parse_text` (`fuzz/fuzz_targets/parse_text.rs`): feeds
arbitrary bytes to `MarkdownParser::parse` and asserts the engine's core
**span-validity invariant** on every element kind of the resulting content
struct.

```rust
fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT_LEN {
        return;
    }
    let c = MarkdownParser::parse(data);
    let check = |start: u32, end: u32| {
        assert!(start <= end);
        let _ = &c.source[start as usize..end as usize];  // panics on OOB
    };
    // every element kind: texts, bolds, italics, bold_italics, codes,
    // autolinks, hard_breaks, links, paragraphs, blockquotes, fenced_codes,
    // headings, thematic_breaks, bullet_items, ordered_items
});
```

The grammar exercised is the full `meon-md` Markdown subset, so the fuzzer
drives every rule family (inline, line, block) of the engine in one pass.

---

## Invariants checked

For every span produced by a parse:

- `start <= end` — spans are well-formed half-open ranges.
- `source[start..end]` does not panic — spans never point past the input.
- Hard-break anchors are zero-length: `start == end`.
- Inputs longer than `MAX_INPUT_LEN` (`u32::MAX`, 4 GiB) are skipped early,
  since `u32` offsets cannot represent them (see
  [`ARCHITECTURE.md §14`](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE.md#14-span-representation)).

The `check` closure uses a real slice index (`&c.source[start..end]`) rather
than a numeric comparison, so any out-of-bounds access causes an immediate
panic that libFuzzer catches and saves as a reproducing artifact.

---

## Requirements

- A **nightly** toolchain (libFuzzer needs the sanitizer runtime).
- `cargo-fuzz`.

Both are provided by the dev shell:

```sh
nix develop
```

Outside Nix:

```sh
rustup toolchain install nightly
cargo install cargo-fuzz
```

---

## Running

```sh
# List targets:
cargo fuzz list

# Run until interrupted (Ctrl-C):
cargo fuzz run parse_text

# Time-boxed run:
cargo fuzz run parse_text -- -max_total_time=7200

# Without AddressSanitizer (2-4x faster; safe since the crate has no unsafe):
cargo fuzz run parse_text --sanitizer none -- -max_total_time=7200

# Seeded from the benchmark corpora (recommended — boosts coverage faster):
cargo fuzz run parse_text fuzz/corpus/parse_text
```

---

## Triage

```sh
# Re-run a saved crash:
cargo fuzz run parse_text fuzz/artifacts/parse_text/<crash-file>

# Minimise a crash to the smallest reproducing input:
cargo fuzz tmin parse_text fuzz/artifacts/parse_text/<crash-file>

# Re-run the corpus without fuzzing (regression check):
cargo fuzz run parse_text fuzz/corpus/parse_text -- -runs=0
```

`corpus/`, `artifacts/`, `coverage/` and `target/` are git-ignored
(`fuzz/.gitignore`).

---

## Campaign log

| date       | duration | toolchain          | exec/s | total exec | cov | crashes | notes                        |
|------------|----------|--------------------|--------|------------|-----|---------|------------------------------|
| 2026-06-15 | ~50 min  | nightly-2026-05-22 | ~35k   | ~104M      | 841 | 0       | default flags, ASan enabled, coverage saturated early, only REDUCE after ~7M |

**Coverage saturation** at `cov: 841 ft: 4762 corp: 1679/298Kb` means
libFuzzer exhausted reachable branches on random inputs without seeds. Adding
seed documents from real Markdown files or from the benchmark corpora will
push coverage higher by guiding the fuzzer into structured code paths.

---

## Notes

- The crate contains **no `unsafe` code** (`#![forbid(unsafe_code)]`).
  AddressSanitizer is therefore redundant for memory-safety purposes and can
  be disabled with `--sanitizer none` for a 2–4x throughput boost.
- Standalone `find_*` iterators are not exercised by this target — they scan
  raw bytes independently and their span bounds are covered by unit tests.
  A dedicated standalone fuzz target could be added if the `_raw` accessor
  arithmetic is extended.
- `avx512` was not tested during fuzzing — AVX-512 hardware was unavailable.
  The scalar and `avx2` paths are covered.
