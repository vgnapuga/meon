# meon — Fuzzing

EN | [**RU**](https://github.com/vgnapuga/meon/blob/main/fuzz/README_RU.md)

Coverage-guided fuzzing of the [`meon`](https://github.com/vgnapuga/meon/blob/main/meon/README.md)
engine with [`cargo-fuzz`](https://github.com/rust-fuzz/cargo-fuzz) (libFuzzer).
The single target drives the engine through a **fuzz-only grammar** built from
[`meon-md`](https://github.com/vgnapuga/meon/blob/main/meon-md/README.md)'s rule
set plus a `key_value` rule (see [Target](#target)). It is purely a
**panic / memory-safety test** — grammar *correctness* lives in the unit and
integration test suites, not here.

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
* ***FUZZING.md***    <--

---

## Target

A single target — `parse_text` (`fuzz/fuzz_targets/parse_text.rs`). It feeds
arbitrary bytes to a fuzz-only parser, `MdKvFuzzParser`, and holds the engine
to a **panic / in-bounds floor** across three separate code paths in one run.

```rust
fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT_LEN {
        return;
    }
    // Fuzz-only grammar: meon-md verbatim + one `key_value` rule sharing the
    // unified frames/fdepth stack (see "Why a fuzz-only grammar" below).
    let c = MdKvFuzzParser::parse(data);

    let check = |start: u32, end: u32| {
        assert!(start <= end);
        let _ = &c.source[start as usize..end as usize];  // panics on OOB
    };

    // (a) Full-parse span validity, every field — including `pairs`:
    //     texts, bolds, italics, bold_italics, codes, autolinks,
    //     hard_breaks (start == end), links, pairs, paragraphs, blockquotes,
    //     fenced_codes, headings, thematic_breaks, bullet_items, ordered_items

    // (b) Generated `_raw()` / `_clean()` accessors driven to completion —
    //     their delimiter arithmetic (`start - count` / `end + count`) is the
    //     panic-prone part — for codes/italics/bolds/bold_italics/autolinks.

    // (c) Standalone `find_*` iterators over `data` — a SEPARATE codegen path;
    //     no cross-comparison with the full parse (they diverge by design),
    //     only the same in-bounds floor.
});
```

### Why a fuzz-only grammar

The grammar is **not** `meon-md`'s published grammar. It is `meon-md`'s rule
set copied verbatim plus exactly one addition: a `key_value` rule inside the
same `on_trigger` block as `*` / `` ` `` / `<` / `[`, with `:` as its `eq`
trigger (the `,` `end` byte is auto-added by the engine).

The reason is coverage, not realism. `meon-md` declares no `key_value` rule, so
the most intricate new machinery in the engine — `key_value` frames sharing the
**unified `frames` / `fdepth` stack** with `balanced = true` symmetric (`*`)
and asymmetric (`<`,`>`) frames, the close cascade's kv-drain-before-pop, the
end-of-run drain — gets **zero** fuzz coverage from the production grammar. This
target exists to drive those engine mechanisms, by combining rules that all
land on the one shared stack. It is deliberately *not* shipped in `meon-md`:
`:` firing on ordinary prose (`Note:`, `3:00`, a URL after `http`) on every
real parse is not an acceptable cost for the production crate just to get this
coverage.

The grammar is bound to **engine mechanisms**, not to `meon-md` — it is valid as
long as it keeps a combination of rules that exercises the shared stack, and it
carries no obligation to track `meon-md`'s grammar over time.

### What the target checks

All three phases are the same floor — *no panic, every span in bounds* — and
none asserts anything about *what* the spans should be (that is the unit /
integration suites' job):

1. **Full parse** — every field of `MdKvFuzzContent`, `pairs` included; hard-break
   anchors additionally asserted zero-length (`start == end`).
2. **`_raw()` / `_clean()` accessors** — the generated delimiter arithmetic, a
   distinct concern from the bare span fields, exercised by draining each
   iterator. `_raw` widens a stored span outward by the delimiter run length;
   an underflowing `start - count` or an `end + count` past the buffer would
   panic here.
3. **Standalone `find_*` iterators** — emitted by a *different* arm of the macro
   than `parse`, scanned over the raw `data`. They are documented to diverge
   from the full parse on content, so no value comparison is made — only the
   in-bounds floor.

---

## Invariants checked

For every span produced — by the full parse, by a `_raw`/`_clean` accessor, or
by a `find_*` iterator:

- `start <= end` — spans are well-formed half-open ranges.
- `source[start..end]` does not panic — spans never point past the input.
- Hard-break anchors are zero-length: `start == end`.
- `key_value` pairs validate both their `key` and `value` spans independently.
- Inputs longer than `MAX_INPUT_LEN` (`u32::MAX`, 4 GiB) are skipped early,
  since `u32` offsets cannot represent them (see
  [`ARCHITECTURE.md §14`](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE.md#14-span-representation)).

The `check` closure uses a real slice index (`&c.source[start..end]`) rather
than a numeric comparison, so any out-of-bounds access causes an immediate
panic that libFuzzer catches and saves as a reproducing artifact.

This is a safety floor, not a correctness oracle. The target intentionally makes
**no** assertion about grammar logic — that two-member object resolves to two
members, that a pair's key precedes its value, that `find_bolds` agrees with the
full parse. Those are covered by `meon`'s unit and `meon-md` / `meon-json`'s
integration tests; here the only contract is "no input, however malformed, makes
the engine panic or read out of bounds."

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

 realese version | date       | toolchain          | total exec | cov  | ft   | corp       | exec/s | rss   |
-----------------|------------|--------------------|------------|------|------|------------|--------|-------|
 v0.1.0          | 2026-06-15 | nightly-2026-05-22 | ~104M      | 841  | 4766 | 1758/252Kb | ~35k   | 629Mb |
 v0.2.0          | 2026-06-21 | nightly-2026-05-22 | ~111M      | 1114 | 6853 | 2346/440Kb | ~32k   | 641Mb |

**Coverage saturation** at `cov: 1114 ft: 6853 corp: 1758/252Kb` means
libFuzzer exhausted reachable branches on random inputs without seeds. Adding
seed documents from real Markdown files or from the benchmark corpora will
push coverage higher by guiding the fuzzer into structured code paths. The
`key_value` / accessor / standalone paths added to the target widen the
reachable surface, so expect the saturation point to move on the next campaign.

---

## Notes

- The crate contains **no `unsafe` code** (`#![forbid(unsafe_code)]`), and the
  target exercises only the engine and its generated parser, so
  AddressSanitizer is redundant for memory-safety purposes and can be disabled
  with `--sanitizer none` for a 2–4x throughput boost.
- Standalone `find_*` iterators **are** now exercised by this target (phase (c)
  above) — a separate codegen path from `parse`, held to the same in-bounds
  floor. So are the generated `_raw()` / `_clean()` accessors (phase (b)).
- The target does not cross-check the full parse against the standalone
  iterators: they diverge by design (a delimiter inside a fence, an escaped
  close), so only each path's own safety floor is asserted.
- `avx512` was not tested during fuzzing — AVX-512 hardware was unavailable.
  The scalar and `avx2` paths are covered.
