//! Tests for the scalar-typing post-pass (`JsonContent::type_scalars` /
//! `type_field`) — separate from `tests/integration.rs`, which covers only
//! the engine's own structural output and deliberately asserts nothing about
//! typing.
//!
//! # Scope
//!
//! `type_scalars` classifies **three** sources by first byte, after
//! trimming `sep`/`tab`/`\n`/`\r` from both ends:
//!
//! - member values (`self.members[i].value`)
//! - array elements (recovered by splitting each `arrays` span on its own
//!   top-level commas)
//! - bare top-level values (`self.scalars` — a document with no enclosing
//!   object or array, e.g. just `42`)
//!
//! A first byte of `"`, `{`, or `[` (string or container) — or anything else
//! unrecognised — classifies to `None` and is silently skipped; only bare
//! scalar leaves (`true` / `false` / `null` / a number-shaped run) ever land
//! in one of the four output vectors.
//!
//! # What this is *not*
//!
//! This is first-byte classification, not JSON-number validation. A value
//! like `1abc` is typed as a number purely because it starts with a digit —
//! the classifier never checks that the rest of the run is a syntactically
//! valid number. That is a deliberate scope boundary (the same one the old
//! in-engine `scalar` rule had), pinned down explicitly below rather than
//! left to be discovered by surprise.

use meon::span::Span;
use meon_json::{JsonParser, ScalarKind, TypedScalars};

/// Collect a span list as owned `String`s, in order.
fn texts(src: &[u8], spans: &[Span]) -> Vec<String> {
    spans
        .iter()
        .map(|s| {
            std::str::from_utf8(&src[s.start as usize..s.end as usize])
                .unwrap()
                .to_string()
        })
        .collect()
}

/// Same, sorted — for order-insensitive multiset assertions.
fn sorted_texts(src: &[u8], spans: &[Span]) -> Vec<String> {
    let mut v = texts(src, spans);
    v.sort();
    v
}

/// `(start, end)` pairs, for comparing two `Vec<Span>` without depending on
/// `Span` itself implementing `PartialEq`/`Debug`.
fn bounds(spans: &[Span]) -> Vec<(u32, u32)> {
    spans.iter().map(|s| (s.start, s.end)).collect()
}

// ==========================================================================
// Member value typing
// ==========================================================================

// 01. A plain integer member value types as a number.
#[test]
fn test_01_single_number_member_typed() {
    let src = br#"{"a":1}"#;
    let c = JsonParser::parse(src);
    let t = c.type_scalars();
    assert_eq!(texts(src, &t.nums), vec!["1"]);
    assert!(t.trues.is_empty() && t.falses.is_empty() && t.nulls.is_empty());
}

// 02. A leading `-` routes to the same `nums` field as a leading digit.
#[test]
fn test_02_negative_number_member_typed() {
    let src = br#"{"a":-7}"#;
    let c = JsonParser::parse(src);
    assert_eq!(texts(src, &c.type_scalars().nums), vec!["-7"]);
}

// 03. `true` routes to `trues`.
#[test]
fn test_03_true_member_typed() {
    let src = br#"{"a":true}"#;
    let c = JsonParser::parse(src);
    assert_eq!(texts(src, &c.type_scalars().trues), vec!["true"]);
}

// 04. `false` routes to `falses`.
#[test]
fn test_04_false_member_typed() {
    let src = br#"{"a":false}"#;
    let c = JsonParser::parse(src);
    assert_eq!(texts(src, &c.type_scalars().falses), vec!["false"]);
}

// 05. `null` routes to `nulls`.
#[test]
fn test_05_null_member_typed() {
    let src = br#"{"a":null}"#;
    let c = JsonParser::parse(src);
    assert_eq!(texts(src, &c.type_scalars().nulls), vec!["null"]);
}

// 06. A string value's first byte is `"` — classifies to `None`, so it never
//     lands in any of the four vectors.
#[test]
fn test_06_string_member_not_typed() {
    let src = br#"{"a":"hello"}"#;
    let c = JsonParser::parse(src);
    let t = c.type_scalars();
    assert!(t.nums.is_empty() && t.trues.is_empty() && t.falses.is_empty() && t.nulls.is_empty());
}

// 07. An object value's first byte is `{` — classifies to `None` at the
//     member level (its own inner scalar, if any, is typed separately, by
//     its own member entry).
#[test]
fn test_07_object_member_not_typed_directly() {
    let src = br#"{"a":{"b":1}}"#;
    let c = JsonParser::parse(src);
    let t = c.type_scalars();
    // Only the inner "b":1 pair types; the outer "a":{...} value does not.
    assert_eq!(texts(src, &t.nums), vec!["1"]);
}

// 08. A flat object with one of every kind: each value lands in exactly the
//     field its first byte routes to, nothing cross-contaminates.
#[test]
fn test_08_mixed_members_all_kinds_typed_correctly() {
    let src = br#"{"i":0,"n":-100,"t":true,"f":false,"z":null,"s":"hi"}"#;
    let c = JsonParser::parse(src);
    let t = c.type_scalars();
    assert_eq!(sorted_texts(src, &t.nums), vec!["-100", "0"]);
    assert_eq!(texts(src, &t.trues), vec!["true"]);
    assert_eq!(texts(src, &t.falses), vec!["false"]);
    assert_eq!(texts(src, &t.nulls), vec!["null"]);
}

// ==========================================================================
// Array element typing
// ==========================================================================

// 09. A flat numeric array: every element typed into `nums`.
#[test]
fn test_09_flat_numeric_array_elements_typed() {
    let src = br#"[1, 2, -3, 42]"#;
    let c = JsonParser::parse(src);
    assert_eq!(
        sorted_texts(src, &c.type_scalars().nums),
        vec!["-3", "1", "2", "42"]
    );
}

// 10. A mixed array: each primitive routes to its own field; the string is
//     not mis-typed.
#[test]
fn test_10_mixed_array_elements_typed() {
    let src = br#"[true, false, null, "apple", 3.14]"#;
    let c = JsonParser::parse(src);
    let t = c.type_scalars();
    assert_eq!(texts(src, &t.trues), vec!["true"]);
    assert_eq!(texts(src, &t.falses), vec!["false"]);
    assert_eq!(texts(src, &t.nulls), vec!["null"]);
    assert_eq!(texts(src, &t.nums), vec!["3.14"]);
}

// 11. Nested arrays: the OUTER array's two elements are themselves arrays
//     (first byte `[`, classify to `None`) so the outer pass types nothing;
//     each INNER array contributes its own numbers via its own entry in
//     `self.arrays` — nesting is handled without any recursion in the
//     splitter itself, it falls out of `arrays` containing every container
//     flat, regardless of depth.
#[test]
fn test_11_nested_array_elements_typed_via_own_entry() {
    let src = br#"[[1, 2], [3, 4]]"#;
    let c = JsonParser::parse(src);
    assert_eq!(
        sorted_texts(src, &c.type_scalars().nums),
        vec!["1", "2", "3", "4"]
    );
}

// 12. An array value inside an object member: the member's own value is not
//     typed (first byte `[`), but the array's elements are, via the
//     `arrays` pass.
#[test]
fn test_12_array_inside_object_member_elements_typed() {
    let src = br#"{"a":[1,2,3]}"#;
    let c = JsonParser::parse(src);
    assert_eq!(
        sorted_texts(src, &c.type_scalars().nums),
        vec!["1", "2", "3"]
    );
}

// 13. A double comma yields an empty segment that is skipped (not a phantom
//     scalar); the two real elements still type.
#[test]
fn test_13_double_comma_array_no_phantom_scalar() {
    let src = br#"[1,,2]"#;
    let c = JsonParser::parse(src);
    assert_eq!(sorted_texts(src, &c.type_scalars().nums), vec!["1", "2"]);
}

// 14. An array of strings and objects emits no scalars at all — every
//     element's first byte is `"` or `{`.
#[test]
fn test_14_array_containing_strings_and_objects_not_misrouted() {
    let src = br#"["x", {"a":1}, "y"]"#;
    let c = JsonParser::parse(src);
    let t = c.type_scalars();
    // The only number here lives one level down, inside the object element.
    assert_eq!(texts(src, &t.nums), vec!["1"]);
    assert!(t.trues.is_empty() && t.falses.is_empty() && t.nulls.is_empty());
}

// ==========================================================================
// Bare top-level scalar typing
// ==========================================================================

// 15. A bare top-level number (no wrapping object/array) types correctly —
//     the completeness gap this layer originally had: `type_scalars` did
//     not examine `self.scalars` at all. Now it does.
#[test]
fn test_15_bare_top_level_number_typed() {
    let src = br#"42"#;
    let c = JsonParser::parse(src);
    assert_eq!(texts(src, &c.type_scalars().nums), vec!["42"]);
}

// 16. A bare top-level `true` types correctly.
#[test]
fn test_16_bare_top_level_true_typed() {
    let src = br#"true"#;
    let c = JsonParser::parse(src);
    assert_eq!(texts(src, &c.type_scalars().trues), vec!["true"]);
}

// 17. A bare top-level STRING is captured by the engine's own `strings`
//     field (the symmetric string rule fires regardless of nesting depth),
//     never reaching `self.scalars` at all — so there is nothing for
//     `type_scalars` to type here; all four vectors stay empty.
#[test]
fn test_17_bare_top_level_string_not_typed() {
    let src = br#""hello""#;
    let c = JsonParser::parse(src);
    assert!(!c.strings.is_empty());
    let t = c.type_scalars();
    assert!(t.nums.is_empty() && t.trues.is_empty() && t.falses.is_empty() && t.nulls.is_empty());
}

// 18. Bare top-level garbage (first byte matches no arm) does not panic and
//     types nothing.
#[test]
fn test_18_bare_top_level_garbage_not_typed() {
    let src = br#"xyz"#;
    let c = JsonParser::parse(src);
    let t = c.type_scalars();
    assert!(t.nums.is_empty() && t.trues.is_empty() && t.falses.is_empty() && t.nulls.is_empty());
}

// ==========================================================================
// `type_field` — single-kind extraction
// ==========================================================================

// 19. `type_field(Num)` returns exactly the same spans as `type_scalars().nums`
//     — same traversal order (members, then array elements, then bare
//     top-level scalars) — across a mix that exercises all three sources at
//     once.
#[test]
fn test_19_type_field_num_matches_type_scalars_nums() {
    let src = br#"{"a":1,"b":[2,3]}"#;
    let c = JsonParser::parse(src);
    let full = c.type_scalars();
    let only = c.type_field(ScalarKind::Num);
    assert_eq!(bounds(&full.nums), bounds(&only));
}

// 20. `type_field(True)` likewise matches `type_scalars().trues`.
#[test]
fn test_20_type_field_true_matches_type_scalars_trues() {
    let src = br#"[true, false, true]"#;
    let c = JsonParser::parse(src);
    assert_eq!(
        bounds(&c.type_scalars().trues),
        bounds(&c.type_field(ScalarKind::True))
    );
}

// 21. `type_field` excludes every other kind: asking for `Num` on a mixed
//     object returns ONLY numbers, none of the bools or nulls.
#[test]
fn test_21_type_field_excludes_other_kinds() {
    let src = br#"{"a":1,"b":true,"c":null,"d":false,"e":2}"#;
    let c = JsonParser::parse(src);
    let nums = c.type_field(ScalarKind::Num);
    assert_eq!(sorted_texts(src, &nums), vec!["1", "2"]);
}

// 22. `type_field` also sees bare top-level scalars, same as `type_scalars`.
#[test]
fn test_22_type_field_sees_top_level_scalars() {
    let src = br#"true"#;
    let c = JsonParser::parse(src);
    assert_eq!(texts(src, &c.type_field(ScalarKind::True)), vec!["true"]);
    assert!(c.type_field(ScalarKind::Num).is_empty());
}

// ==========================================================================
// Whitespace & multi-line — trimming before classification
// ==========================================================================

// 23. Spaces around `:`/`,` are trimmed away by the typing layer (unlike the
//     raw `members[i].value`, which keeps them — see `tests/integration.rs`
//     for that distinction).
#[test]
fn test_23_whitespace_trimmed_before_classification() {
    let src = br#"{ "a" : 1 , "b" : 2 }"#;
    let c = JsonParser::parse(src);
    // Raw member values keep the trailing space (locked down elsewhere);
    // the typed span does not.
    assert_eq!(c.str(c.members[0].value).unwrap(), "1 ");
    assert_eq!(sorted_texts(src, &c.type_scalars().nums), vec!["1", "2"]);
}

// 24. A tab right after `:` is not skipped by the engine's own `allow_sep`
//     (only a single leading space is), but the typing layer's `trim` strips
//     it before classification regardless.
#[test]
fn test_24_tab_after_colon_trimmed_for_typing() {
    let src = b"{\"a\":\t5}";
    let c = JsonParser::parse(src);
    assert_eq!(c.str(c.members[0].value).unwrap(), "\t5");
    assert_eq!(texts(src, &c.type_scalars().nums), vec!["5"]);
}

// 25. A value spanning a newline: the raw member value carries the `\n` and
//     surrounding indentation verbatim, but the typed span is the clean
//     token alone.
#[test]
fn test_25_value_spanning_newline_trimmed_for_typing() {
    let src = b"{\"a\":\n  5\n}";
    let c = JsonParser::parse(src);
    assert_eq!(c.str(c.members[0].value).unwrap(), "\n  5\n");
    assert_eq!(texts(src, &c.type_scalars().nums), vec!["5"]);
}

// 26. A pretty-printed, multi-line array: elements split correctly across
//     the embedded newlines and indentation, each trimmed before typing.
#[test]
fn test_26_pretty_printed_array_elements_typed_across_lines() {
    let src = b"[\n  1,\n  2,\n  3\n]";
    let c = JsonParser::parse(src);
    assert_eq!(c.arrays.len(), 1);
    assert_eq!(
        sorted_texts(src, &c.type_scalars().nums),
        vec!["1", "2", "3"]
    );
}

// ==========================================================================
// Known limitation: first-byte classification, not number validation
// ==========================================================================

// 27. A value that merely STARTS with a digit is typed as a number even
//     though the rest of it is not valid JSON-number syntax. This is the
//     classifier's actual contract (first byte only), pinned down here on
//     purpose rather than left for someone to discover as a surprise.
#[test]
fn test_27_malformed_number_first_byte_still_typed_as_num() {
    let src = br#"{"a":1abc}"#;
    let c = JsonParser::parse(src);
    assert_eq!(texts(src, &c.type_scalars().nums), vec!["1abc"]);
}

// 28. JSON keywords are lowercase by spec; the classifier matches lowercase
//     bytes only (`b't'`/`b'f'`/`b'n'`), so a capitalised keyword's first
//     byte matches no arm and is silently skipped — not an error, just
//     untyped.
#[test]
fn test_28_uppercase_keyword_not_typed() {
    let src = br#"{"a":True}"#;
    let c = JsonParser::parse(src);
    let t = c.type_scalars();
    assert!(t.trues.is_empty() && t.nums.is_empty() && t.falses.is_empty() && t.nulls.is_empty());
}

// 29. A variety of number shapes (int, float, exponent, signed) all type as
//     `Num` purely by first byte (digit or `-`) — breadth check, not a claim
//     of full numeric-grammar validation.
#[test]
fn test_29_various_number_forms_all_classify_as_num() {
    let src = br#"[0, 0.5, -0.5, 1e10, -1E-10]"#;
    let c = JsonParser::parse(src);
    assert_eq!(sorted_texts(src, &c.type_scalars().nums), {
        let mut v = vec!["-0.5", "-1E-10", "0", "0.5", "1e10"];
        v.sort();
        v
    });
}

// ==========================================================================
// Projection sanity
// ==========================================================================

// 30. The typed span's bytes, recovered via the source slice directly
//     (without going through `JsonContent::str`/`bytes`, since `TypedScalars`
//     is returned standalone and outlives no particular content borrow
//     beyond the slice itself), match the expected trimmed text exactly.
#[test]
fn test_30_typed_span_byte_equal_to_trimmed_source_slice() {
    let src = br#"{"a": 99 }"#;
    let c = JsonParser::parse(src);
    let t: TypedScalars = c.type_scalars();
    assert_eq!(t.nums.len(), 1);
    let span = t.nums[0];
    assert_eq!(&src[span.start as usize..span.end as usize], b"99");
}

// ==========================================================================
// Escape-awareness inside array-element strings (for_each_array_element's
// own string-skip, independent of the engine's own escape handling).
// ==========================================================================

// 31. An escaped quote inside a string that is itself an array element: the
//     string-skip must not treat `\"` as the real close, so the comma after
//     the string is still found at the right place and the sibling number
//     types correctly. The string itself still classifies to `None` (first
//     byte `"`), as always.
#[test]
fn test_31_escaped_quote_inside_array_element_string() {
    let src = br#"["a\"b", 1]"#;
    let c = JsonParser::parse(src);
    let t = c.type_scalars();
    assert_eq!(texts(src, &t.nums), vec!["1"]);
    assert!(t.trues.is_empty() && t.falses.is_empty() && t.nulls.is_empty());
}

// 32. An escaped backslash immediately followed by the real closing quote
//     (`\\"` — content `a\`, i.e. one literal backslash, then close). The
//     jump-by-two-on-backslash trick must consume the escaped backslash and
//     its pair together so the *next* byte (the real `"`) is seen fresh,
//     not mistaken for itself being escaped. Getting this wrong would run
//     the string-skip past the end of the array looking for a close that
//     was already there, swallowing the sibling element.
#[test]
fn test_32_escaped_backslash_then_real_close_inside_array_element() {
    let src = br#"["a\\", 1]"#;
    let c = JsonParser::parse(src);
    assert_eq!(texts(src, &c.type_scalars().nums), vec!["1"]);
}

// 33. Brace and bracket characters *inside* a string element must never
//     reach the depth counter at all — the string-skip's own inner loop
//     only ever looks for `\` and `"`, so a `{`/`}` here is just an
//     ordinary byte to it, never seen by `for_each_array_element`'s outer
//     `depth` tracking. Two such strings, with a real number sandwiched
//     between them, locks down that depth is never perturbed by them.
#[test]
fn test_33_brackets_inside_array_element_string_do_not_affect_depth() {
    let src = br#"["{not real}", 1, "}another{"]"#;
    let c = JsonParser::parse(src);
    let t = c.type_scalars();
    assert_eq!(texts(src, &t.nums), vec!["1"]);
    assert!(t.trues.is_empty() && t.falses.is_empty() && t.nulls.is_empty());
}

// ==========================================================================
// Multi-level depth within a single array's own top-level comma-split
// (distinct from test_11, where nesting never made the *outer* array's own
// depth exceed 1 — both its elements were containers from the very first
// byte, contributing nothing to type via the outer pass at all).
// ==========================================================================

// 34. A middle element that is a *doubly*-nested array (`[2,[3,4]]`) drives
//     this array's own `depth` from 0 up to 2 and back down to 0 before the
//     next top-level comma is recognised, with a real scalar sibling on
//     each side. Deliberately nested ARRAYS here, not objects: a nested
//     object's own `key_value` pairs are found by the engine independently
//     of array/object wrapping (the same reason test_07's inner `"b":1`
//     types via its own member entry, separate from the outer member it
//     sits inside) — they would type via `self.members` regardless of
//     whether this array's own depth-tracking were even correct, which
//     would defeat the point of this test. Nested arrays carry no such
//     independent path: every number here can *only* be found correctly if
//     each array's own comma-split — including this outer one's depth
//     bookkeeping around its doubly-nested middle element — is right.
#[test]
fn test_34_depth_reaches_two_and_returns_to_zero_within_one_array() {
    let src = br#"[1,[2,[3,4]],5]"#;
    let c = JsonParser::parse(src);
    assert_eq!(
        sorted_texts(src, &c.type_scalars().nums),
        vec!["1", "2", "3", "4", "5"]
    );
}

// 35. An array nested inside a member's value, itself containing another
//     nested array as one of its own elements: exercises the member-value
//     skip (`[` classifies to `None` so `"a"`'s value itself never types),
//     the outer array's own depth-tracking around its nested sibling array,
//     and that nested array's own, entirely separate `self.arrays` entry —
//     three mechanisms most other tests only exercise one or two of at a
//     time.
#[test]
fn test_35_array_nested_two_levels_inside_member_value() {
    let src = br#"{"a":[1,[2,3],4]}"#;
    let c = JsonParser::parse(src);
    assert_eq!(
        sorted_texts(src, &c.type_scalars().nums),
        vec!["1", "2", "3", "4"]
    );
}

// ==========================================================================
// Empty / degenerate array shapes.
// ==========================================================================

// 36. An empty array's content span has `start == end`; `for_each_array_element`
//     must take its `inner_start >= inner_end` early return and produce
//     nothing, not panic or emit a phantom empty-segment scalar.
#[test]
fn test_36_empty_array_types_nothing_no_panic() {
    let src = br#"{"a":[]}"#;
    let c = JsonParser::parse(src);
    let t = c.type_scalars();
    assert!(t.nums.is_empty() && t.trues.is_empty() && t.falses.is_empty() && t.nulls.is_empty());
}

// 37. An empty array as a bare top-level document, with no member or other
//     wrapping at all.
#[test]
fn test_37_bare_top_level_empty_array_types_nothing() {
    let src = br#"[]"#;
    let c = JsonParser::parse(src);
    let t = c.type_scalars();
    assert!(t.nums.is_empty() && t.trues.is_empty() && t.falses.is_empty() && t.nulls.is_empty());
}

// ==========================================================================
// `type_field` — the two kinds not yet covered by a dedicated test
// (Num and True already have one each; False and Null did not).
// ==========================================================================

// 38. `type_field(False)` matches `type_scalars().falses` exactly.
#[test]
fn test_38_type_field_false_matches_type_scalars_falses() {
    let src = br#"[true, false, false, true]"#;
    let c = JsonParser::parse(src);
    assert_eq!(
        bounds(&c.type_scalars().falses),
        bounds(&c.type_field(ScalarKind::False))
    );
}

// 39. `type_field(Null)` matches `type_scalars().nulls` exactly.
#[test]
fn test_39_type_field_null_matches_type_scalars_nulls() {
    let src = br#"{"a":null,"b":1,"c":null}"#;
    let c = JsonParser::parse(src);
    assert_eq!(
        bounds(&c.type_scalars().nulls),
        bounds(&c.type_field(ScalarKind::Null))
    );
}

// ==========================================================================
// `\r` — the fourth whitespace byte every trim loop in this file matches,
// exercised directly (test_24 covers `\t`, test_25/26 cover `\n`; none of
// the existing tests use a bare `\r`).
// ==========================================================================

// 40. A bare `\r` right after `:` is, like the tab in test_24, not skipped
//     by the engine's own `allow_sep` (which only skips a literal space) —
//     the raw member value carries it — but the typing layer's own
//     whitespace-skip strips it before classification regardless.
#[test]
fn test_40_carriage_return_after_colon_trimmed_for_typing() {
    let src = b"{\"a\":\r5}";
    let c = JsonParser::parse(src);
    assert_eq!(c.str(c.members[0].value).unwrap(), "\r5");
    assert_eq!(texts(src, &c.type_scalars().nums), vec!["5"]);
}

// ==========================================================================
// Array-element comma-split: a comma INSIDE a string element
//
// The string-skip in `for_each_array_element` exists precisely so the split
// delimiter (`,`) is not honoured while inside a `"..."`. Prior tests checked
// the skip against escaped quotes (31/32) and brackets (33) — but never
// against a bare comma, which is the one byte the splitter actually keys on.
// This is the most direct test of that guarantee, and it was missing.
// ==========================================================================

// 41. A comma inside a string element must not split it: `"x,y"` is one
//     element (first byte `"`, classifies to `None`), so only the real
//     sibling number types. Getting the skip wrong would split `"x,y"` into
//     `"x` and `y"`, and the second fragment (`y"`) would still classify to
//     `None` — so the give-away is whether the number is found at the right
//     boundary, which only holds if the comma was correctly skipped.
#[test]
fn test_41_comma_inside_array_element_string_not_split() {
    let src = br#"["x,y", 1]"#;
    let c = JsonParser::parse(src);
    let t = c.type_scalars();
    assert_eq!(texts(src, &t.nums), vec!["1"]);
    assert!(t.trues.is_empty() && t.falses.is_empty() && t.nulls.is_empty());
}

// ==========================================================================
// Classifier boundary — the first byte is the WHOLE contract
//
// test_27 pinned the number side (`1abc` -> Num). These pin the rest of the
// boundary: the keyword side, and the bytes that look number-ish to a human
// but are NOT in the classifier's set.
// ==========================================================================

// 42. The dual of test_27 for keywords: any lowercase `t`/`f`/`n`-led run is
//     routed by its first byte alone, regardless of what follows. `nan` is
//     Null, `txy` is True, `fzz` is False — none of them are real JSON
//     keywords, but the classifier never looks past byte zero. Pinned on
//     purpose, exactly as the number-prefix case is.
#[test]
fn test_42_lowercase_keyword_prefix_typed_by_first_byte() {
    let src = br#"{"a":nan,"b":txy,"c":fzz}"#;
    let c = JsonParser::parse(src);
    let t = c.type_scalars();
    assert_eq!(texts(src, &t.nulls), vec!["nan"]);
    assert_eq!(texts(src, &t.trues), vec!["txy"]);
    assert_eq!(texts(src, &t.falses), vec!["fzz"]);
    assert!(t.nums.is_empty());
}

// 43. A leading `+` or `.` is NOT a number start to the classifier — only a
//     digit or a leading `-` is. JSON forbids both `+5` and `.5` anyway, but
//     the point here is the classifier's own set: these route to `None` and
//     stay untyped rather than being leniently accepted as numbers.
#[test]
fn test_43_plus_and_dot_leading_not_classified_as_num() {
    let src = br#"{"a":+5,"b":.5}"#;
    let c = JsonParser::parse(src);
    let t = c.type_scalars();
    assert!(t.nums.is_empty() && t.trues.is_empty() && t.falses.is_empty() && t.nulls.is_empty());
}

// 44. A lone `-` (no digits after) is still a number to the classifier — the
//     first byte is `-`, and that is the entire decision. Degenerate, but it
//     is the documented first-byte contract, so it is locked in rather than
//     left ambiguous.
#[test]
fn test_44_lone_minus_typed_as_num() {
    let src = br#"{"a":-}"#;
    let c = JsonParser::parse(src);
    assert_eq!(texts(src, &c.type_scalars().nums), vec!["-"]);
}

// ==========================================================================
// Keys (and other non-value positions) are never a typing source
// ==========================================================================

// 45. A numeric-looking KEY is a quoted string — it lands in `strings`, never
//     in a typed vector. `type_scalars` only ever visits member *values*,
//     array elements, and bare top-level scalars; keys are not among its
//     three sources. So `{"1":2,"3":4}` types `2` and `4`, never `1` or `3`.
#[test]
fn test_45_numeric_looking_keys_never_typed() {
    let src = br#"{"1":2,"3":4}"#;
    let c = JsonParser::parse(src);
    let nums = sorted_texts(src, &c.type_scalars().nums);
    assert_eq!(nums, vec!["2", "4"]);
    assert!(!nums.contains(&"1".to_string()));
    assert!(!nums.contains(&"3".to_string()));
}

// ==========================================================================
// Three-source traversal ORDER
//
// test_19 noted the order (members, then array elements, then bare top-level
// scalars) but only exercised two of the three sources. This drives all three
// at once and asserts the un-sorted order, so a reordering of `for_each_scalar`
// would be caught.
// ==========================================================================

// 46. Members first (`"a":1`), then array elements (`[2,3]`), then the bare
//     trailing top-level scalar (`4`). The numbers must appear in exactly that
//     order, not sorted — `1, 2, 3, 4` here only because that is the source
//     order, not a coincidence of value.
#[test]
fn test_46_three_source_traversal_order() {
    let src = br#"{"a":1,"b":[2,3]}4"#;
    let c = JsonParser::parse(src);
    // un-sorted: order is the contract under test
    assert_eq!(texts(src, &c.type_scalars().nums), vec!["1", "2", "3", "4"]);
}

// ==========================================================================
// Degenerate / whitespace-only values
// ==========================================================================

// 47. A member value that is whitespace only: `process_scalar` trims it to an
//     empty span (`start == end`) and emits nothing — no panic, no phantom
//     scalar. (The raw member value, by contrast, keeps the spaces; that is a
//     structural concern, tested in `tests/integration.rs`.)
#[test]
fn test_47_whitespace_only_member_value_types_nothing() {
    let src = br#"{"a":   }"#;
    let c = JsonParser::parse(src);
    let t = c.type_scalars();
    assert!(t.nums.is_empty() && t.trues.is_empty() && t.falses.is_empty() && t.nulls.is_empty());
}

// 48. A trailing comma in an array leaves an empty final segment, which the
//     splitter skips — no phantom typed scalar. The two real elements still
//     type. (Structural tolerance of the trailing comma is locked down in
//     `tests/integration.rs`; this is its typing-side counterpart.)
#[test]
fn test_48_array_trailing_comma_no_phantom_typed_scalar() {
    let src = br#"[1,2,]"#;
    let c = JsonParser::parse(src);
    assert_eq!(sorted_texts(src, &c.type_scalars().nums), vec!["1", "2"]);
}

// ==========================================================================
// Bare top-level `false` / `null`
//
// test_15/16 covered bare top-level number and `true`; the other two keyword
// kinds had no bare-top-level test.
// ==========================================================================

// 49. A bare top-level `false` routes to `falses` via `self.scalars`.
#[test]
fn test_49_bare_top_level_false_typed() {
    let src = br#"false"#;
    let c = JsonParser::parse(src);
    assert_eq!(texts(src, &c.type_scalars().falses), vec!["false"]);
}

// 50. A bare top-level `null` routes to `nulls` via `self.scalars`.
#[test]
fn test_50_bare_top_level_null_typed() {
    let src = br#"null"#;
    let c = JsonParser::parse(src);
    assert_eq!(texts(src, &c.type_scalars().nulls), vec!["null"]);
}

// ==========================================================================
// trim_end — trailing whitespace that is NOT a space
//
// test_23/30 exercise trim_end against a trailing space; test_25 against a
// trailing newline. A trailing TAB (and the symmetry of trim_end matching all
// four bytes on the trailing side, not just leading) had no direct test.
// ==========================================================================

// 51. A trailing tab is stripped by `trim_end` before classification, even
//     though the engine's own `allow_sep` never touches it: the raw member
//     value keeps the `\t`, the typed span does not.
#[test]
fn test_51_trailing_tab_trimmed_before_classification() {
    let src = b"{\"a\":5\t}";
    let c = JsonParser::parse(src);
    assert_eq!(c.str(c.members[0].value).unwrap(), "5\t");
    assert_eq!(texts(src, &c.type_scalars().nums), vec!["5"]);
}

// ==========================================================================
// Empty / trivial inputs
// ==========================================================================

// 52. An empty object and empty input both type nothing — no members, no
//     arrays, no scalars to visit — and neither panics.
#[test]
fn test_52_empty_object_and_empty_input_type_nothing() {
    for src in [&b"{}"[..], &b""[..]] {
        let c = JsonParser::parse(src);
        let t = c.type_scalars();
        assert!(
            t.nums.is_empty() && t.trues.is_empty() && t.falses.is_empty() && t.nulls.is_empty()
        );
    }
}

// ==========================================================================
// Immutability of the post-pass
//
// The module doc promises `type_scalars` writes nothing back into
// `JsonContent` — it "stays an immutable record of exactly what the engine
// saw". This pins that: typing twice yields identical results, and the
// content's own structural fields are untouched by typing.
// ==========================================================================

// 53. `type_scalars` is repeatable and side-effect-free: two calls produce
//     byte-identical span sets, and the content's structural fields are
//     unchanged across them.
#[test]
fn test_53_type_scalars_is_repeatable_and_non_mutating() {
    let src = br#"{"a":1,"b":[2,3]}"#;
    let c = JsonParser::parse(src);

    let members_before = c.members.len();
    let arrays_before = c.arrays.len();

    let t1 = c.type_scalars();
    let t2 = c.type_scalars();
    assert_eq!(bounds(&t1.nums), bounds(&t2.nums));

    // The content struct is an immutable record — typing did not disturb it.
    assert_eq!(c.members.len(), members_before);
    assert_eq!(c.arrays.len(), arrays_before);
}
