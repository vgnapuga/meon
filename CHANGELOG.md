# Changelog

All notable changes to this project are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
This repository is a Cargo workspace of three independently published
crates — `meon`, `meon-macros`, `meon-md` — released together; each entry
below is labelled with the crate(s) it applies to.

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
