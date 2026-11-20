# Changelog

All notable changes to this project are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
This repository is a Cargo workspace of independently published crates —
`meon`, `meon-macros`, `meon-md`, and `meon-json` — each versioned on its own;
every entry below is labelled with the crate(s) it applies to.

## [Unreleased]

### Added

- **`meon`, `meon-macros`** — optional `limits { ... }` grammar section:
  `offset`, `max_nest`, `max_inline_span`, `max_line_span`,
  `max_block_span` and `max_input_bytes`. `max_nest` is still accepted in
  the header for existing grammars; when both places are present they must
  agree.
- **`meon`, `meon-macros`** — `limits { offset = u8 | u16 | u32 | u64; }`
  selects the span offset type (default `u32`). `Span<T>` and `SpanIndex` are
  public; generated content, finders and context use the selected type.
- **`meon`, `meon-macros`** — optional `max_input_bytes` in `limits`. `0` and
  the offset type's maximum mean that maximum; `_` or an absent key means no
  check and the parser reads only the prefix that fits. The checked methods
  reject an over-limit source with `InputBytesTooLarge` before allocating any
  parser state; when no limit is set, no check code is generated.
- **`meon`** — span caps are measured on the clean span, `end - start`
  without delimiters. `0` and the offset type's maximum mean absent; a value
  above the offset maximum is a compile error. When a cap is absent, no
  comparison, fallback or reparse code is generated for that level.
- **`meon`** — an over-limit span is not emitted: a transparent inline span
  keeps its text and inner spans; an opaque inline span and a `chained`
  element are reparsed inside; a `key_value` pair is dropped without reparse;
  a line or block item span goes to `blocks { fallback => ... }`; an over-long
  fence is reparsed as paragraph runs, skipping the `cont` markers of the
  frames that held it.
- **`meon`** — `find_*` and `find_context_*` apply the caps. `ParseContext`
  drops an over-limit opaque region and rescans its interior for nested
  regions.

### Changed

- **`meon`** — `Span` is generic over its offset type. `Span` without a
  parameter is `Span<u32>`; `Span::new` keeps the `u32` constructor and
  `from_usize`/`from_parts` cover the other types. `MAX_INPUT_LEN` remains
  `u32::MAX`; `Span<T>::MAX_INPUT_LEN` gives the selected type's bound.
- **`meon`** — `InputBytesTooLarge<T = u32>` and `ParseContext<T = u32>` are
  generic. The default `u32` keeps existing type paths valid.
- **`meon`** — `parse`, `find_*`, `find_context_*` and `context` return
  `Result<_, InputBytesTooLarge<O>>`. `parse_unchecked`, `find_*_unchecked`,
  `find_context_*_unchecked` and `context_unchecked` keep the previous
  infallible behaviour.
- **`meon`** — `max_inline_span`, `max_line_span`, `max_block_span` and
  `max_input_bytes` are bounded by the selected offset type. `u8` and `u16`
  grammars read only the prefix that fits when no `max_input_bytes` is set.
- **`meon-md`, `meon-json`** — grammars leave `max_input_bytes` unset and use
  the default `offset = u32`, so their checked methods never fail on length.
- **`meon`** — `parse_text!`, `parse_inline!`, `parse_line!` and
  `parse_block!` accept optional `[]`/`[N]` cap tokens and an `off` type
  token; direct callers default to `u32`.
- **`meon-md`, `meon-json`** — grammars now declare `max_nest` in `limits`,
  with `max_inline_span = 0`, `max_line_span = 0` and `max_block_span = 0`.
  The header form is still accepted.

### Fixed

- **`meon`** — a `chained` over-limit match keeps its delimiters in source
  order.
- **`meon`** — an opaque key keeps its full span through the key-value parser.
- **`meon`** — context caps are measured on the clean span, not on the
  delimiter-inclusive region.
- **`meon`** — chained and key-value close offsets are stored in the selected
  span type.
- **`meon`** — the no-limit inline scan keeps its state by value instead of a
  redundant mutable borrow.
- **`meon`** — full-parse span construction keeps positions in `usize`/the
  selected offset type, so `offset = u64` no longer truncates inline, line or
  block spans above the `u32` range.
- **`meon-macros`** — the front-end rejects a non-integer header `max_nest`,
  an unknown top-level section, invalid `merge_simple` values, malformed
  `hard_break`/`on_trigger` argument lists, a non-`max` second `line`
  argument, and a stacked `asymmetric` rule with no `1 => field` arm.

### Documentation

- `README.md`, crate `README.md`s, `ARCHITECTURE.md` and rustdoc describe
  `offset`, `Span<T>`, the per-type input bound, the checked/unchecked API,
  block and inline internals, accessor/finder/context coverage and the DSL
  requirements.
- `benches/README.md`, `MD_COMPARE.md` and `JSON_COMPARE.md` document the
  `small`/`big` Cargo features, the isolated (`_parse`) and warmed
  (`_compare`) scenarios, and the current benchmark targets.
- `FAQ.md` and `fuzz/README.md` are updated to the current behavior.

### Benchmarks

- **`benches`** — `small` and `big` Cargo features select the working-set
  size; `big` is 100× `small`. Benchmarks call the checked parser APIs with
  `unwrap`. New `meon-md_offsets` and `meon-json_offsets` targets parse their
  respective small corpora with `Span<u16>`, `Span<u32>` and `Span<u64>`
  offsets, and 32 MiB corpora with `Span<u32>` and `Span<u64>`; the offset
  benchmarks also report output heap.

Median full-parse throughput from the comparison runs, `small → big`:

| Corpus                     | Stable            | Nightly + AVX2          |
|----------------------------|-------------------|-------------------------|
| Markdown `plain`           | 3.07 → 2.96 GiB/s | 4.41 → 4.18 GiB/s       |
| Markdown `hot`             | 981 → 976 MiB/s   | 1.228 → 1.229 GiB/s     |
| Markdown `heavy`           | 806 → 808 MiB/s   | 930 → 950 MiB/s         |
| JSON `numbers`, structural | 854 → 904 MiB/s   | 962 MiB/s → 1.043 GiB/s |
| JSON `objects`, structural | 401 → 303 MiB/s   | 424 → 315 MiB/s         |
| JSON `nested`, structural  | 248 → 193 MiB/s   | 269 → 202 MiB/s         |

The Markdown composition report agrees with pulldown-cmark on the three
generated corpora under its mapping of element counts and plain-text byte
coverage. On the large JSON `nested` corpus, the structural pass reaches
193 MiB/s on stable and the parse with scalar typing reaches 161 MiB/s.
The corresponding nightly + AVX2 figures are 202 and 160 MiB/s. On the
small stable corpora, Markdown `find_links` reaches 5.77 GiB/s on `heavy`,
and JSON `find_objects` reaches 1.56 GiB/s on `nested`. The JSON
`find_context_objects` pass reaches 1.22 GiB/s with a prebuilt context and
130 MiB/s when building the context is included.

### Testing

- **Fuzzing (`v1.0.0` campaign, 2026-11-20).** The `parse_text` target ran
  ~220M executions on nightly-2026-05-22 at ~6k exec/s and 808 MB RSS. The
  campaign log reports `cov` 4980 → 5115, `ft` 30787 → 31657 and corpus
  6775/2134 KB → 7175/1728 KB compared with the `v0.7.0` entry.
- Unit, UI (`trybuild`) and mechanism tests cover the `limits` section,
  `max_input_bytes`, and `offset = u8/u16/u32/u64`; the full workspace test
  suite passes.

### CI

- The lint job runs formatting, clippy, rustdoc with warnings denied, and
  package checks with locked dependencies.
- Tests run on Linux, macOS and Windows, plus MSRV and 32-bit checks; nightly
  covers all features and both bench feature sets.
- Security covers advisories and dependency review; Dependabot keeps
  dependencies current, and a coverage job publishes an lcov artifact.

## [0.7.0] - 2026-11-19

### Fixed

- **`meon`, `meon-md`** — **unmatched inline delimiters stay in the text.**
  With `*` as a transparent symmetric rule, `*a\nb\n` now leaves `*a\nb`
  in fallback, including the opener. Refused opens and runs of undeclared
  length stay literal. Removing an unclosed placeholder preserves completed
  inner elements, so ``**a `c` b`` keeps the code span and the two surrounding
  text runs. Successful chained and key/value matches roll back the fallback
  bytes they cover, including across intervening frame closes.
- **`meon`** — **inline runs keep their own boundaries.** Fallback merging
  stays within one run and skips bytes consumed by block and line rules.
  The final line terminator stays outside the inline scan. A hard break
  removes its marker only. With CRLF enabled, a bare CR at EOF remains
  content in the full parse, standalone iterators and context builder.
- **`meon`** — **chained and key/value delimiters are read at the byte that
  closes them.** The rest of a delimiter run can still be read by the
  scanner. A transparent chain keeps its own phases, and a failed chain
  leaves literal text available. Keys stop at line and opaque-span
  boundaries, a rejected pair at full stack depth remains text, and
  separator padding before a value does not prematurely close it when the
  separator is also the terminator.
- **`meon`** — **opaque balanced close searches reuse their work.**
  Balanced asymmetric searches remember which openers cannot be closed.
  For an opaque chain, a failure involving a balanced component builds an
  index of the remaining inline run. Later balanced queries take constant
  time, including when nested first components move second starts backwards.
  The full parse, `find_*` and the context builder share the chain search.
  The index uses one lazy reusable buffer, four bytes per indexed byte per
  balanced component. Successful chains before a failure need no search
  buffer. Unbalanced searches reuse known closes or absent suffixes.
- **`meon`** — **selective scans use the grammar's rule and line semantics.**
  Standalone iterators honour escapes, delimiter runs, balancing and
  nesting for the rule they scan. Context construction follows paragraph,
  block-line and same-marker continuation boundaries, including fences in
  continuations. It picks opaque symmetric and exact asymmetric arms by
  opening byte and count, and adds fully opaque chained matches to the map.
  Raw finders can still see delimiters inside another rule's opaque regions.
- **`meon-macros`** — numbered-block `_raw()` uses the declared digit
  pattern. A grammar that accepts `AB: x` now returns the whole fragment
  instead of starting at `:`. The clean span and number are unchanged.
- **`meon-json`** — scalar typing uses checked slices for member values,
  array elements and top-level scalars. The crate forbids unsafe code.
  Out-of-bounds spans supplied by a caller can panic through checked
  indexing.

### Changed

- **`meon`** — `0.6.0` → `0.7.0`. **Line and block rules use the declared
  byte patterns.** Uniform lines, fences, marker blocks and both arguments
  of `num` preserve Rust byte patterns through the full parse, standalone
  generation and context classification. A fence closes with its opening
  byte, even when its pattern accepts several bytes. The optional `crlf`
  setting recognises CR before the declared `eol`. Lines made only of
  `sep` and `tab` are blank.
- **`meon-macros`** — `0.4.0` → `0.5.0`. Transparent symmetric rules use
  the shared bounded stack and take `parse_inside` and count arms. An
  asymmetric rule with `balanced = true` or `parse_inside = true` has one
  `1 => field` arm. Forwarded literal and pattern fragments from
  `macro_rules!` reach the runtime grammar, and metadata expressions keep
  their original grouping.
- **`meon-md`** — `0.5.0` → `0.6.0`. Enables CRLF and balances the
  opaque components of links and images. `[a [b] c](url)` and
  `[t](https://x.y/a_(b))` each produce one link. Inline matches can cross
  same-marker blockquote lines without counting continuation markers as
  delimiters.
- **`meon-json`** — `0.4.0` → `0.5.0`. Recompiled against the `0.7.0`
  engine. The standalone object and array finders now nest their own
  brackets. Their context-aware variants also skip brackets inside strings.
  Nested members still require the full parse to see the surrounding
  containers.

### Removed

- **`meon-macros`** — the `balanced` setting on `symmetric`. Remove it
  from existing symmetric declarations. Asymmetric rules and chained
  components still take the setting.

### Documentation

- `ARCHITECTURE.md` and crate `README.md` files describe the grammar changes,
  inline-run boundaries, opaque close searches and context-aware extraction.
  The grammar examples use the current DSL.
- The Markdown benchmark report checks composition against pulldown-cmark
  outside the timed loop. The synthetic corpora use URL autolinks and
  backtick fences, and separate the ordered list from following prose.

### Benchmarks

Median full-parse throughput from the comparison runs, `small → big`:

| Corpus                     | Stable            | Nightly + AVX2          |
|----------------------------|-------------------|-------------------------|
| Markdown `plain`           | 2.86 → 2.70 GiB/s | 4.76 → 4.24 GiB/s       |
| Markdown `hot`             | 946 → 925 MiB/s   | 1.217 → 1.214 GiB/s     |
| Markdown `heavy`           | 817 → 827 MiB/s   | 948 → 949 MiB/s         |
| JSON `numbers`, structural | 845 → 914 MiB/s   | 926 MiB/s → 1.006 GiB/s |
| JSON `objects`, structural | 404 → 298 MiB/s   | 432 → 302 MiB/s         |
| JSON `nested`, structural  | 262 → 192 MiB/s   | 273 → 199 MiB/s         |

The Markdown composition report agrees with pulldown-cmark on the three
generated corpora under its mapping of element counts and plain-text byte
coverage. On the large JSON `nested` corpus, the structural pass reaches
192 MiB/s on stable and the parse with scalar typing reaches 151 MiB/s.
The corresponding nightly + AVX2 figures are 199 and 163 MiB/s. On the
small stable corpora, Markdown `find_links` reaches 5.56 GiB/s on `heavy`,
and JSON `find_objects` reaches 1.55 GiB/s on `nested`. The JSON
`find_context_objects` pass reaches 1.25 GiB/s with a prebuilt context and
395 MiB/s when building the context is included.

### Testing

- **Fuzzing (`v0.7.0` campaign, 2026-11-17).** The `parse_text` target ran
  ~30M executions on nightly-2026-05-22 at ~5k exec/s and 639 MB RSS. The
  campaign log reports `cov` 4497 → 4980, `ft` 28312 → 30787 and corpus
  7282/1793 KB → 6775/2134 KB compared with the `v0.6.0` entry.
- New engine regressions cover fallback rollback and unclosed frames,
  continuation markers, CRLF and bare CR at EOF, opaque arm selection and
  numbered raw spans. Balanced chained cases count bytes scanned across
  failed candidates and compare indexed results with a forward oracle.
- Standalone and context tests cover rule isolation, balanced and
  transparent matching, and line/block classification. `trybuild` checks
  forwarded macro fragments and the diagnostics for symmetric `balanced`
  and unsupported asymmetric arms.

## [0.6.0] - 2026-10-04

### Fixed

- **`meon`, `meon-md`** — **plain text of an inline construct that never closes
  is no longer dropped.** While a frame was open on the unified inline stack,
  plain-text flushes were gated on `fdepth == 0`, so a run that ended with the
  frame still open discarded not only the unmatched delimiter but every text
  byte it enclosed: `*a\nb\n` produced no `italics` span **and** no `texts`
  span. Text runs are now appended as they are scanned, and each push site
  snapshots the fallback vector so that a normal close rolls the held text back
  — output for constructs that do close is unchanged. `*a\nb\n` now yields
  `texts = ["a\nb"]`, and an element that closed inside an unclosed container
  keeps its own span without being duplicated as text (``**a `c` b`` yields
  `codes = ["c"]` and `texts = ["a ", " b"]`). The unmatched delimiter bytes
  themselves are still discarded. `meon-json`'s output is unaffected.
- **`meon`** — **quadratic close searches in unbalanced runs.** The off-stack
  forward searches — legacy `asymmetric`, and `chained` components declared
  `balanced = false` — rescanned to the end of the run for every opener, so *k*
  unmatched openers in one run cost *O(k·n)*; a document of repeated `[a` took
  tens of seconds. A per-run 256-bit set now records the close bytes whose
  search has already come back empty, and a later opener of the same kind is
  rejected without scanning, which makes the whole run *O(n)*. Depth-counting
  (`balanced = true`) searches are excluded, since their failure is not
  monotone. Output is unchanged.

### Changed

- **`meon`** — `0.5.0` → `0.6.0`. **Output vectors allocate on first use.**
  `NameState` fields start unallocated and reserve `source.len() / div`
  elements when the field receives its first element; every push in the runtime
  macros goes through a generated `push_<field>` method that carries the
  reservation. A rule that never matches a given document allocates nothing, so
  peak output size follows the document rather than the grammar — a grammar
  with many element kinds no longer reserves a slice of the source length for
  each of them up front. `meon-md`'s output went from 233% of the input size to
  90% on markup-free prose, and from 239% to 180% on a mixed corpus. The
  reservation rides on the `len == capacity` test `Vec::push` makes anyway: a
  push with room left is a plain store, and only a full vector — the first
  element included — leaves for an out-of-line `#[cold]` path that reserves
  the hint and pushes. `MarkdownParser::parse` compiles to 34 296 bytes,
  against 37 508 when every field was reserved up front. Parser output is
  unchanged.
- **`meon-macros`** — `0.3.1` → `0.4.0`. **The front-end is the single
  authority on grammar shape.** A canonicalisation pass (`normalize.rs`)
  rewrites the accepted `inline` section into the one token shape the runtime
  `macro_rules!` patterns match, so sub-rules and settings may be written in
  any order, with or without trailing `;` / `,`, and still compile — spellings
  the front-end accepted but the runtime rejected with `no rules expected this
  token` are now working grammars. In exchange the front-end rejects earlier
  and with a location: an unknown keyword names its section and the keywords
  that section accepts; `parse_inside` and `balanced` are required in
  `symmetric`, `asymmetric` and `chained` component bodies and must be `true`
  or `false`; an incomplete `chained` or `key_value` is reported at the rule
  instead of as a pattern mismatch inside a runtime macro. Every grammar that
  compiled before still compiles.
- **`meon`** — the seven copies of the paragraph-bounded close search in the
  standalone iterators are replaced by one `next_in_paragraph` primitive in
  `standalone/common.rs`, with the per-caller decision moved into an
  `enter_line` callback. The pending slot of `symmetric`
  (`parse_inside = true, balanced = false`) drops an unreachable `balanced`
  branch and a tuple field that was always zero, and the unreachable
  `parse_text!` arm without `max_nest` is removed. Behaviour is unchanged.
- **`meon-md`** — `0.4.0` → `0.5.0`, **`meon-json`** — `0.3.0` → `0.4.0`.
  Recompiled against the `0.6.0` engine. `meon-md` picks up the retained text
  of unclosed emphasis; `meon-json`'s output is identical to `0.3.0`.

### Removed

- The `memchr` alias in the `inline` DSL. It named no runtime construct and
  never compiled past the front-end.
- Russian documentation (`*_RU.md`). Documentation is English-only from this
  release; the Russian files are no longer maintained.

### Documentation

- `ARCHITECTURE.md` — §4 documents the canonicalisation stage and the
  order-independence it buys, §8 the lazy output allocation, §9 the held text
  of unclosed frames and the dead-close-byte set, §12 the shared
  paragraph-bounded close search, and §17 gains the `on_trigger` close-byte
  requirement that §9 refers to.
- Crate `README.md`s — capacity divisors are described as reserve-on-first-push,
  `meon-macros` documents what the front-end rejects, and `meon-md` no longer
  claims that emphasis spanning multiple lines goes undetected (it has been
  matched within a paragraph since `0.3.0`).
- `benches/README.md` — the `max_nest` performance note no longer describes the
  per-line stack re-initialisation that `0.3.0` removed.
- In-code comments across the workspace rewritten to describe current behaviour
  instead of change history; stale rustdoc statements fixed. `cargo doc` now
  builds the workspace without warnings.

### Benchmarks

Throughput is unchanged from `0.5.0` on every corpus, within run-to-run noise:
the held text of unclosed frames, the dead-close-byte set and the lazy output
allocation cost nothing measurable on ordinary documents. What they change is
the edge of the input space — a run of unmatched openers is linear instead of
quadratic (see Fixed), and a document no longer reserves output for element
kinds it does not contain (see Changed).

Hardware counters for the full `meon-md_parse` pass (`perf stat`, 10 runs,
user-space counters, stable build, `--profile-time 10`), `small → big`:

| Corpus  | insn/cycle   | branch-misses  | cache-misses   |
|---------|--------------|----------------|----------------|
| `plain` | 5.05 → 4.92  | 0.10% → 0.09%  | 1.50% → 1.53%  |
| `hot`   | 4.59 → 4.25  | 0.08% → 0.09%  | 4.03% → 3.64%  |
| `heavy` | 4.10 → 3.84  | 0.10% → 0.12%  | 6.05% → 3.22%  |

IPC holds at 3.8–5.1 with branch-misses at 0.08–0.12%, and the cache-miss rate
stays flat or falls as the input scales from `small` to `big`. Every IPC figure
is within 0.11 of its `0.5.0` value; the largest move is `plain`, up by ~0.1 on
both sizes.

### Testing

- **Fuzzing (`v0.6.0` campaign).** The `parse_text` target and its fuzz-only
  grammar are unchanged; through the same four phases it now also reaches the
  engine's new paths — the held text of unclosed frames, the dead-close-byte
  set and the cold push path. Campaign: `cov` 4196 → 4497, `ft` 26488 → 28312,
  corpus 6641/1610 KB → 7282/1793 KB, ~200M execs, no crashes. Full log in
  `FUZZING.md`.
- New unit tests in `meon` for the held text of unclosed frames (symmetric and
  asymmetric frames, nested frames, an element that closes inside an unclosed
  one, merge roll-back, hard breaks, and a run with no trailing newline), the
  dead-close-byte set, the pending-slot contract, lazy allocation (an unused
  field holds nothing, the hint is used up before any growth, growth past it
  keeps every element, a merge into a full vector extends in place), and
  `next_in_paragraph`.
- New `meon-macros` unit tests for the canonicalisation pass and for the new
  front-end rejections, plus `trybuild` cases: a grammar declared entirely in
  the "wrong" order must compile and parse (`ui/pass/reordered_grammar.rs`),
  and an unknown inline keyword, a missing flag, and an incomplete `chained`
  must each fail with a located error.
- New `meon-md` integration tests for unclosed emphasis, for the linear-time
  behaviour of unbalanced runs, and for which output fields a document
  actually allocates.

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
