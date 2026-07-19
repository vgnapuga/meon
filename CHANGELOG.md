# Changelog

All notable changes to this project are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
This repository is a Cargo workspace of independently published crates —
`meon`, `meon-macros`, `meon-md`, and `meon-json` — each versioned on its own;
every entry below is labelled with the crate(s) it applies to.

## [0.5.0] - 2026-07-19

### Added

- **`meon`, `meon-macros`** — **context-aware standalone extraction.** Every
  generated parser gains two new method families:
  - `context(source: &[u8]) -> ParseContext` — the opaque-region map: fenced
    blocks plus every `parse_inside = false` inline rule, resolved in **one
    streaming pass**. Fence bytes and opaque triggers share a single
    deduplicated needle set — one search per iteration (`memchr` / `memchr2` /
    `memchr3` up to three distinct bytes, SWAR `find_any` beyond) — with the
    full parser's leftmost-wins semantics: a matched construct covers its full
    extent, delimiters included, and the scan resumes after it. Close searches
    are escape-aware and paragraph-bounded (a pair may span one line break; an
    empty line or a fence-opening line aborts the pending opener). The result
    is a sorted, non-overlapping span vector, preallocated from the grammar's
    own `[cap]` divisors.
  - `find_context_*(source, &ctx)` — one per rule that is not itself opaque:
    the same streaming matcher as the context-free `find_*`, with candidate
    delimiters inside opaque regions skipped. A covered candidate jumps the
    whole region in one step through a monotone, amortised-O(1) cursor, so a
    fenced block is stepped over without walking its content. The context
    suppresses **trigger positions**, not enclosing spans — a bold span may
    still legally contain a code span, exactly as in the full parse.
    Line-family rules (`line`, `line_simple`, `cont`, `block`, `num`) reuse
    the context-free iterator and drop items whose span start is covered —
    candidate-exact for whole-line constructs. Opaque sources and `chained` /
    `key_value` rules get no variant by construction.

  One map is built per source and shared by any number of context-aware
  iterators over it. This closes the documented opacity divergence of the
  context-free `find_*`: in `meon-md`, emphasis markers inside a fenced block
  or a code span are no longer matched; in `meon-json`, a `{` inside a string
  value no longer counts as an object open.

- **`meon`** — standalone same-type block nesting. `cont` rules self-nest in
  the standalone scan exactly as in the full parse: `find_blockquotes` sees
  `> >` as two nested, correctly-bounded spans, up to the grammar's
  `max_nest`.

- Nix dev shell — `cargo-llvm-cov`.

### Changed

- **`meon`** — `0.4.0` → `0.5.0`. Standalone `find_*` iterators are reworked
  from per-line scanning into a **byte-stream scan**: one `memchr`-family
  search per candidate, so stretches of input without marker bytes are never
  walked at all. Matching semantics are now explicit — inline pairs are
  paragraph-bounded (a pair may cross a single line break; an empty line
  aborts the pending opener), and `symmetric` / `asymmetric` standalone rules
  match exactly the declared delimiter count, ignoring `balanced`. Standalone
  output can change for inputs that relied on the old per-line behaviour.
- **`meon-macros`** — `0.3.0` → `0.3.1`. The compile pipeline additionally
  emits the `context` / `find_context_*` methods; purely additive, no changes
  to the grammar DSL or to the existing expansion.
- **`meon-md`** — `0.3.0` → `0.4.0`, **`meon-json`** — `0.2.0` → `0.3.0`.
  Recompiled against the `0.5.0` engine: their `find_*` output follows the
  streaming rework above, and both gain the context-aware surface. In
  `meon-md` the context sources are code spans, autolinks and fenced blocks;
  every other element kind gets a `find_context_*` variant. In `meon-json`
  the opaque rule is the string — `find_context_objects` / `find_context_arrays`
  skip candidates inside strings, while `find_strings` itself stays
  context-free (it *is* the context source). The context closes the
  string-opacity divergence, not the nesting-insensitivity: context-aware
  scans still match literal delimiters without tracking depth.
- Documentation overhaul (EN + RU): `ARCHITECTURE.md` §12 rewritten around
  the streaming scan and the context map, all crate `README.md`s describe the
  `context` / `find_context_*` surface, the benchmark docs gain context-aware
  extraction result sections (`small` + `big`, stable + AVX2), `BENCHMARKS.md`
  gains a microarchitecture section with `perf stat` counters, and the
  cross-parser docs are reframed around the two different jobs — span vectors
  on one side, an event stream / AST / tape / owned value on the other.

### Benchmarks

The per-line → byte-stream rework is the headline (stable build, `small`
corpora, `find_codes` as the representative single-kind scan):

| Corpus  | `0.4.0`      | `0.5.0`      |
|---------|--------------|--------------|
| `plain` | 6.2202 GiB/s | 91.398 GiB/s |
| `hot`   | 2.9992 GiB/s | 9.0383 GiB/s |
| `heavy` | 2.5367 GiB/s | 7.2167 GiB/s |

Marker-free input is now scanned at memory speed (~15× on `plain`), and dense
corpora gain ~3× from skipping unmarked stretches. The context machinery
prices out as: build the map once (162.94 µs on `hot`, shared by all eight
context-aware iterators), pay ~10% per-candidate overhead on a warm
`find_context_*` scan (143.73 µs vs 130.86 µs context-free `find_italics`),
or pay build + scan in one call in the cold single-shot case (300.92 µs).
Full tables — `small` + `big`, stable + AVX2, both grammars — are in
`MD_COMPARE.md` and `JSON_COMPARE.md`; the new `BENCHMARKS.md`
microarchitecture section records IPC 3.9–4.9, branch-misses ≈0.1%, and a
cache-miss rate that stays flat as the input grows ~100× from `small` to
`big`.

### Testing

- **Fuzzing — target extended (`v0.5.0` campaign).** The `parse_text` target
  now also drives (d) the `context()` opaque-region map plus the
  `find_context_*` iterators — yet another codegen path: the map's own region
  spans are bounds-checked first, then every context-aware scanner is drained,
  held to the same no-panic / in-bounds floor as the other three phases.
  Campaign: `cov` 3529 → 4196, `ft` 22591 → 26488, corpus unchanged at
  6641/1610 KB, ~120M execs, no crashes. Full log in `FUZZING.md`.
- New unit tests across the engine: `ParseContext` construction
  (leftmost-wins, escaped delimiters, paragraph bounds, fence open/close
  edges, wide needle sets and needle overflow), the context-aware iterators,
  the streaming standalone paths, `parse_block!`, and the SWAR layer —
  coverage of the streaming standalones and `parse_block!` raised to ~97%.
- New integration tests for context-aware standalone parsing in `meon-md`
  and `meon-json`.

## [0.4.0] - 2026-07-09

### Changed

- **`meon`, `meon-macros`, `meon-md`, `meon-json`** — **relicensed** from
  `AGPL-3.0-only OR LicenseRef-meon-commercial` to `MIT OR Apache-2.0`
  (at the user's option). Versions published before this release remain
  available under their original license. No code changes — versions are
  bumped so the new license metadata lands on crates.io:
  `meon` 0.3.0 → 0.4.0, `meon-macros` 0.2.0 → 0.3.0,
  `meon-md` 0.2.0 → 0.3.0, `meon-json` 0.1.1 → 0.2.0.
- Version mentions across `*.md` files updated to the actual crate versions.

### Added

- `CONTRIBUTING.md` — contributions are accepted under the project's dual
  license (inbound = outbound, per Apache-2.0 §5).
- `PULL_REQUEST_TEMPLATE.md`.
- `LICENSE-MIT` and `LICENSE-APACHE` at the workspace root.

### Removed

- `CLA.md` — a contributor license agreement is unnecessary under a
  permissive license.
- `COMMERCIAL.md` — the commercial licensing option is discontinued.

## [0.3.1] - 2026-07-05

### Changed
- removed doubled bullets from docs.
- added additional info about `time` in `standalone` benchmarks.

## [0.3.0] - 2026-06-26

### Added

- **`meon-json`** (new crate, `0.1.0`) — a structural flat JSON reader built on
  the engine. It emits one flat `Vec` per element kind (`objects`, `arrays`,
  `strings`, `members`, plus the `scalars` / `loose` fallbacks) and recovers
  document structure by interval containment rather than building a tree. Scalar
  typing (`nums` / `trues` / `falses` / `nulls`) is an opt-in post-pass
  (`type_scalars` / `type_field`), not part of the hot loop — a caller that never
  types pays nothing. Grammar sets `max_nest = 64`. It is a **structural reader,
  not a validator**: malformed input never errors, it yields sane partial output.
  The only `unsafe` in the workspace lives in `meon-json`'s typing layer; the
  engine itself remains `#![forbid(unsafe_code)]`.
- **`meon`** — streaming multi-line inline processing. The inline scan now runs
  over one accumulated multi-line run instead of restarting per physical line, so
  the nesting stack survives every internal `\n` in a document with no blank
  lines. This is what lets pretty-printed JSON parse like its single-line form,
  and it is the prerequisite for the unified stack below.
- **Benchmarks** — cross-parser comparison benches alongside the existing
  intra-engine ones:
  - `meon-md_compare` — `meon-md` vs `pulldown-cmark` (parse-only event stream)
    and `comrak` (full CommonMark AST).
  - `meon-json_parse` / `meon-json_standalone` / `meon-json_compare` —
    `meon-json` structural and typed, vs `simd-json` (`to_tape`) and `sonic-rs`
    (`from_slice::<Value>`).
  - New comparison docs `MD_COMPARE.md` and `JSON_COMPARE.md` (EN + RU). The
    general `BENCHMARKS.md` is now a shared overview of the markdown and JSON
    benches rather than markdown-only.

### Changed

- **`meon`** — `0.2.0` → `0.3.0`. The three separate bounded stacks of `0.2.0`
  (the block-level active-block stack, the inline `symmetric` stack, and the
  inline `asymmetric` pending slot) are unified into a **single
  `frames` / `fdepth` stack** shared by `symmetric`, `asymmetric`, and
  `key_value` frames. A frame's kind is recovered from its stored open byte, so
  no per-frame kind tag is carried; `key_value` uses a parallel `kv_pending`
  slot and asymmetric overflow is counted rather than stored. One stack now backs
  all inline nesting in place of three independent mechanisms.
- **`meon-macros`, `meon-md`** — remain at `0.2.0`, recompiled against the
  `0.3.0` engine. `meon-md`'s grammar (`max_nest = 4`) and observable output are
  unchanged.
- Documentation overhaul (EN + RU): `ARCHITECTURE.md` §9 rewritten around the
  unified stack, all crate `README.md`s, `FUZZING.md`, and the benchmark docs.

### Fixed

- **`meon`** — non-linear cost in `max_nest`. In `0.2.0` the bounded-stack arrays
  were zero-initialised on every per-line parse call, so throughput scaled with
  the configured cap rather than with the nesting actually used — the
  **−54% / −50%** hit on `hot` / `heavy` at `max_nest = 255` documented in the
  `0.2.0` Benchmarks section. The unified streaming stack removes this per-line
  re-initialisation: per-line cost is now ~flat in `max_nest`, and throughput
  scales linearly with input size past cache. This is what makes `meon-json`'s
  `max_nest = 64` affordable.

### Benchmarks

The scalability fix shows up most clearly in how throughput holds when the input
grows past cache. `meon-json` structural vs the validating parsers (stable build,
median `thrpt` in MiB/s, `small` → `big`):

| Parser            | `numbers`  | `objects`  | `nested`   |
|-------------------|------------|------------|------------|
| `meon-structural` | 815 → 881  | 357 → 287  | 243 → 192  |
| `simd-json`       | 232 → 233  | 708 → 221  | 522 → 172  |
| `sonic-rs`        | 686 → 243  | 792 → 322  | 498 → 227  |

`meon-structural` loses ≤20% from `small` to `big` (and gains on `numbers`),
while the validating parsers lose ~55–69% on the structured corpora as their
materialised tape / owned value blows cache — so the small-input ranking inverts
at scale (on `objects`, meon overtakes `simd-json`, 287 vs 221). Full tables,
AVX2 numbers, and the markdown comparison are in `JSON_COMPARE.md` and
`MD_COMPARE.md`.

### Testing

- **Fuzzing — strategy extended (`v0.3.0` campaign).** The fuzz-only grammar is
  now `meon-md`'s rule set plus a `key_value` rule sharing the unified
  `frames` / `fdepth` stack, so the engine's most intricate new machinery
  (`key_value` frames alongside `balanced` symmetric/asymmetric frames, the close
  cascade's kv-drain-before-pop, the end-of-run drain) gets fuzz coverage the
  production grammar can never provide. The `parse_text` target now also drives
  (b) the generated `_raw()` / `_clean()` accessor delimiter arithmetic and
  (c) the standalone `find_*` iterators — both separate codegen paths — held to
  the same no-panic / in-bounds floor. Campaign: `cov` 1114 → 3529,
  `ft` 6853 → 22591, corpus 2346/440 KB → 6641/1610 KB, ~100M execs, no crashes.
  Full log in `FUZZING.md`.
- New `meon-json` unit tests (structural + typing) and integration tests.

## [0.2.0] - 2026-06-21

### Added

- **`meon`, `meon-macros`** — Optional `max_nest` grammar setting: a
  compile-time-bounded nesting-depth cap shared by the block-level
  active-block stack and the inline engine's bounded `symmetric` /
  `asymmetric` stacks. Defaults to `1`, which reproduces the original,
  pre-nesting behaviour exactly — existing grammars are unaffected unless
  they opt in.
  - **Block level.** `cont` / `fence` rules can now self-nest: `> > text`
    opens two distinct, correctly-bounded blockquote spans instead of
    one; a fenced code block can open on a continuation line inside a
    blockquote without the outer continuation's state being lost.
  - **Inline level, `symmetric`.** With `parse_inside = true, balanced =
    true`, a different-count occurrence of the same delimiter now opens
    its own tracked frame on a bounded stack instead of overwriting the
    single pending slot — `**bold *italic* still-bold**` now resolves
    both levels instead of losing the outer pair.
  - **Inline level, `asymmetric`.** With `balanced = true` and/or
    `parse_inside = true`, multiple different bracket types declared in
    the same `on_trigger` block (e.g. `{`/`}` and `[`/`]`) now nest
    validly into each other, bounded by `max_nest`.
- **`meon-md`** — Grammar now sets `max_nest = 4`. Nested blockquotes,
  fenced code blocks inside blockquotes, and nested emphasis now resolve
  correctly instead of hitting the limitations documented in `0.1.x`.

### Fixed

- **`meon`** — Asymmetric close-byte dispatch could cascade-close two
  distinct frames on a single input byte when two `asymmetric` rules in
  the same `on_trigger` block shared a close byte but had different open
  bytes (e.g. `(`/`)` and `[`/`)`). Closing is now a single unified pass
  that dispatches by the frame's own recorded open byte, rather than by
  which rule's independent block happened to run first.
- **`meon`** — Three internal, opaque forward searches had no
  escape-awareness at all when searching for their own closing
  delimiter, so a backslash-escaped closing delimiter (e.g. `` \` ``
  inside a code span) was incorrectly accepted as the real close:
  - `symmetric` with `parse_inside = false` (e.g. code spans).
  - The legacy `asymmetric` memchr/depth search (e.g. autolinks).
  - The legacy `chained` two-phase search (e.g. `[text](url)` links).

  All three now skip an escaped candidate and continue searching,
  independent of and orthogonal to the `parse_inside` opacity setting —
  content can stay fully opaque to other rules while the closing search
  still correctly respects escaping.

### Changed

- **`meon`, `meon-macros`, `meon-md`** — `0.1.x` → `0.2.0` across all
  three crates as a coordinated release. `meon` and `meon-macros` gain
  new, backward-compatible API surface via `max_nest`; `meon-md`'s
  observable output changes for inputs that previously hit the nesting
  limitations above.
- Documentation overhaul across `ARCHITECTURE.md`, all three crates'
  `README.md`, `FUZZING.md`, and `BENCHMARKS.md` (EN + RU) to describe
  the bounded-stack mechanism accurately, replacing stale references to
  the pre-`max_nest` single-active-block / single-pending-slot design.
- `BENCHMARKS.md` — `heavy` corpus now includes nested-blockquote and
  nested-emphasis constructs, so it exercises the bounded-stack code
  paths and not only flat element density.

### Benchmarks

Stable build, `small` corpora (fits in cache), `meon-md_parse`, grammar's
default `max_nest = 4`:

| Corpus  | Throughput   |
|---------|--------------|
| `plain` | 2.5484 GiB/s |
| `hot`   | 1.0636 GiB/s |
| `heavy` | 964.89 MiB/s |

Cost of `max_nest` itself — `meon-md` rebuilt with `max_nest = 255`
instead of `4`, same build and corpora:

| Corpus  | `max_nest = 4`  | `max_nest = 255`    | Δ            |
|---------|-----------------|---------------------|--------------|
| `plain` | 2.5484 GiB/s    | 2.5758 GiB/s        | ~0% (noise)  |
| `hot`   | ~1089 MiB/s     | 500.60 MiB/s        | **−54%**     |
| `heavy` | 964.89 MiB/s    | 482.16 MiB/s        | **−50%**     |

The bounded-stack arrays sized by `max_nest` are zero-initialised on every
`parse_block!` / `parse_inline!` call regardless of whether that specific
line nests anything — cost scales with the configured cap, not with the
nesting depth actually used in the input. `plain` is unaffected because it
contains no inline trigger bytes at all. Full breakdown, AVX2 numbers, and
cache-exceeding `big`-corpus results are in `BENCHMARKS.md`.

### Testing

- Fuzzing (`cargo-fuzz`, `parse_text` target, `meon-md` grammar):
  `cov` 841 → 1114, `ft` 4766 → 6853, corpus 1758/252 KB → 2346/440 KB,
  no crashes across the campaign. Full log in `FUZZING.md`.
- New unit tests covering the close-byte-sharing fix and the
  escape-awareness fix, across `symmetric` (both `balanced` settings),
  the legacy `asymmetric` path, and the legacy `chained` path (both
  components, both `balanced` settings).
- New integration tests in `meon-md` for nested blockquotes, fenced code
  inside blockquotes, and nested emphasis.
