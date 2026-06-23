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
