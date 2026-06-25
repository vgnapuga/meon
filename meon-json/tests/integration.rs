//! Integration tests for `meon-json`.
//!
//! # Scope
//!
//! These exercise only the public `JsonParser::parse` API and the *structural*
//! `JsonContent` fields the engine itself produces:
//!
//! - `objects` / `arrays`   — one span per container (brackets included).
//! - `strings`              — one span per `"..."` run (content only; the raw
//!   span excludes the quotes — see the "empty string" boundary test).
//! - `members`              — one `Member { key, value }` per `key: value`
//!   pair, both fields raw spans (quotes/brackets included where relevant).
//! - `scalars`              — top-level inline fallback (bare text outside
//!   any container, or a value the engine could not otherwise place).
//! - `loose`                — block-level fallback (the accumulated
//!   non-blank "paragraph" run(s); for a single JSON document with no blank
//!   lines this is exactly one span covering the whole input).
//!
//! **No test here asserts anything about scalar *typing*** (`nums` / `trues` /
//! `falses` / `nulls`). That projection no longer exists in the engine at
//! all — it moved to a separate post-pass (`JsonContent::type_scalars` /
//! `type_field`) with its own test coverage. A consequence worth stating
//! plainly: the engine no longer tracks individual *array elements* as spans
//! at all (only the array's own outer span). Per-element introspection is
//! entirely a post-pass concern now; these tests check container/member
//! structure and raw text, never "how many elements".
//!
//! # Multi-line / pretty JSON IS covered here
//!
//! An earlier version of this suite stated multi-line JSON was unsupported.
//! That is no longer true: the engine's `inline` scan now runs over a single
//! accumulated multi-line run rather than per physical line, so the unified
//! stack (`key_value` frames, open containers) survives every `\n` inside a
//! document with no blank lines. A dedicated section below pins this down
//! with pretty-printed, indented, multi-line input.
//!
//! # Raw, untrimmed values
//!
//! Without the old scalar layer, **nothing trims a member value** anymore.
//! A value's span runs from right after its `:` (plus, at most, one
//! immediately-following space — see `allow_sep`) to its terminator,
//! verbatim: trailing spaces, tabs, `\r`, and embedded `\n` are all part of
//! the raw span. Several tests below assert on that whitespace explicitly,
//! on purpose, to lock the behaviour in rather than let it drift silently.

use meon_json::{JsonContent, JsonParser};

/// Collect a span field as owned `String`s in field order.
fn texts(c: &JsonContent<'_>, spans: &[meon::span::Span]) -> Vec<String> {
    spans
        .iter()
        .map(|s| c.str(*s).unwrap().to_string())
        .collect()
}

/// Same, sorted — for order-insensitive multiset assertions.
fn sorted(c: &JsonContent<'_>, spans: &[meon::span::Span]) -> Vec<String> {
    let mut v = texts(c, spans);
    v.sort();
    v
}

/// Collect `(key, value)` pairs as owned strings, in member order.
fn members(c: &JsonContent<'_>) -> Vec<(String, String)> {
    c.members
        .iter()
        .map(|m| {
            (
                c.str(m.key).unwrap().to_string(),
                c.str(m.value).unwrap().to_string(),
            )
        })
        .collect()
}

// ==========================================================================
// Empty / trivial containers
// ==========================================================================

// 01. An empty object is one container, no members.
#[test]
fn test_01_empty_object() {
    let c = JsonParser::parse(br#"{}"#);
    assert_eq!(c.objects.len(), 1);
    assert_eq!(c.arrays.len(), 0);
    assert_eq!(c.members.len(), 0);
}

// 02. An empty array is one container, no members.
#[test]
fn test_02_empty_array() {
    let c = JsonParser::parse(br#"[]"#);
    assert_eq!(c.arrays.len(), 1);
    assert_eq!(c.objects.len(), 0);
    assert_eq!(c.members.len(), 0);
}

// 03. Empty input does not panic and yields nothing.
#[test]
fn test_03_empty_input() {
    let c = JsonParser::parse(b"");
    assert_eq!(c.objects.len(), 0);
    assert_eq!(c.arrays.len(), 0);
    assert_eq!(c.members.len(), 0);
    assert_eq!(c.strings.len(), 0);
    assert_eq!(c.scalars.len(), 0);
}

// 04. Whitespace-only input does not panic and produces no containers.
#[test]
fn test_04_whitespace_only_input() {
    let c = JsonParser::parse(b"   ");
    assert_eq!(c.objects.len(), 0);
    assert_eq!(c.arrays.len(), 0);
    assert_eq!(c.members.len(), 0);
}

// 05. An object whose only member's value is an empty object.
#[test]
fn test_05_nested_empty_object() {
    let c = JsonParser::parse(br#"{"a":{}}"#);
    assert_eq!(c.objects.len(), 2);
    assert_eq!(c.members.len(), 1);
    assert_eq!(members(&c), vec![(r#""a""#.into(), "{}".into())]);
}

// 06. An object whose only member's value is an empty array.
#[test]
fn test_06_nested_empty_array() {
    let c = JsonParser::parse(br#"{"a":[]}"#);
    assert_eq!(c.objects.len(), 1);
    assert_eq!(c.arrays.len(), 1);
    assert_eq!(members(&c), vec![(r#""a""#.into(), "[]".into())]);
}

// 07. An array of three empty objects: three containers, no members.
#[test]
fn test_07_array_of_empty_objects() {
    let c = JsonParser::parse(br#"[{},{},{}]"#);
    assert_eq!(c.arrays.len(), 1);
    assert_eq!(c.objects.len(), 3);
    assert_eq!(c.members.len(), 0);
}

// ==========================================================================
// Basic single/multi member structure — raw text only, no type assertions
// ==========================================================================

// 08. A number value: captured verbatim as the member's raw value text.
#[test]
fn test_08_single_member_number_raw() {
    let c = JsonParser::parse(br#"{"a":1}"#);
    assert_eq!(c.objects.len(), 1);
    assert_eq!(members(&c), vec![(r#""a""#.into(), "1".into())]);
}

// 09. A string value: raw member value includes the quotes.
#[test]
fn test_09_single_member_string_raw() {
    let c = JsonParser::parse(br#"{"a":"b"}"#);
    assert_eq!(members(&c), vec![(r#""a""#.into(), r#""b""#.into())]);
}

// 10. `true` as a value: just raw text "true", no claim about its "type".
#[test]
fn test_10_single_member_true_raw() {
    let c = JsonParser::parse(br#"{"a":true}"#);
    assert_eq!(members(&c), vec![(r#""a""#.into(), "true".into())]);
}

// 11. `false` as a value: raw text only.
#[test]
fn test_11_single_member_false_raw() {
    let c = JsonParser::parse(br#"{"a":false}"#);
    assert_eq!(members(&c), vec![(r#""a""#.into(), "false".into())]);
}

// 12. `null` as a value: raw text only.
#[test]
fn test_12_single_member_null_raw() {
    let c = JsonParser::parse(br#"{"a":null}"#);
    assert_eq!(members(&c), vec![(r#""a""#.into(), "null".into())]);
}

// 13. An array value: the member's raw value is the whole bracketed array —
//     containment, not a per-element breakdown (the engine no longer tracks
//     elements at all).
#[test]
fn test_13_single_member_array_value_whole_span() {
    let c = JsonParser::parse(br#"{"a":[1,2,3]}"#);
    assert_eq!(c.arrays.len(), 1);
    assert_eq!(members(&c), vec![(r#""a""#.into(), "[1,2,3]".into())]);
}

// 14. An object value: the member's raw value is the whole nested object.
#[test]
fn test_14_single_member_object_value_whole_span() {
    let c = JsonParser::parse(br#"{"a":{"b":2}}"#);
    assert_eq!(c.objects.len(), 2);
    assert_eq!(c.members.len(), 2);
    let vals: Vec<String> = members(&c).into_iter().map(|(_, v)| v).collect();
    assert!(vals.contains(&r#"{"b":2}"#.to_string()));
}

// 15. Two members: first finalised at `,`, second at `}`.
#[test]
fn test_15_two_members_comma() {
    let c = JsonParser::parse(br#"{"a":1,"b":2}"#);
    assert_eq!(c.members.len(), 2);
    assert_eq!(
        members(&c),
        vec![(r#""a""#.into(), "1".into()), (r#""b""#.into(), "2".into())]
    );
}

// 16. Three members, declaration order preserved in `members`.
#[test]
fn test_16_three_members_order_preserved() {
    let c = JsonParser::parse(br#"{"a":1,"b":2,"c":3}"#);
    assert_eq!(
        members(&c),
        vec![
            (r#""a""#.into(), "1".into()),
            (r#""b""#.into(), "2".into()),
            (r#""c""#.into(), "3".into()),
        ]
    );
}

// 17. Twenty flat members: a plain count sanity check at moderate width.
#[test]
fn test_17_many_members_flat_count() {
    let mut s = String::from("{");
    for i in 0..20 {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!(r#""k{i}":{i}"#));
    }
    s.push('}');
    let c = JsonParser::parse(s.as_bytes());
    assert_eq!(c.objects.len(), 1);
    assert_eq!(c.members.len(), 20);
}

// 18. Duplicate keys are not deduplicated — both members are recorded, in
//     order. (Whether that is "valid" JSON is a question for a validator
//     layer, not this engine; the engine reports what it saw.)
#[test]
fn test_18_duplicate_keys_both_recorded() {
    let c = JsonParser::parse(br#"{"a":1,"a":2}"#);
    assert_eq!(c.members.len(), 2);
    assert_eq!(
        members(&c),
        vec![(r#""a""#.into(), "1".into()), (r#""a""#.into(), "2".into())]
    );
}

// ==========================================================================
// Strings & keys
// ==========================================================================

// 19. A string value and its key both land in `strings` (unquoted content).
#[test]
fn test_19_string_value_and_key_in_strings_field() {
    let c = JsonParser::parse(br#"{"name":"Alice"}"#);
    assert_eq!(sorted(&c, &c.strings), vec!["Alice", "name"]);
}

// 20. An empty string value `""` is a count-2 delimiter run that matches no
//     `1 => strings` arm, so it emits NO `strings` span. The content is still
//     recoverable from the raw member value (`""`).
#[test]
fn test_20_empty_string_value_no_strings_span() {
    let c = JsonParser::parse(br#"{"a":""}"#);
    assert_eq!(members(&c), vec![(r#""a""#.into(), r#""""#.into())]);
    // Only the key "a" lands in `strings`; the empty value does not.
    assert_eq!(sorted(&c, &c.strings), vec!["a"]);
}

// 21. Keys carry their quotes in `members`; the unquoted text lives in
//     `strings` alongside string values.
#[test]
fn test_21_key_quotes_included_in_member_key() {
    let c = JsonParser::parse(br#"{"age":30}"#);
    assert_eq!(members(&c), vec![(r#""age""#.into(), "30".into())]);
    assert_eq!(sorted(&c, &c.strings), vec!["age"]);
}

// 22. A string value containing an escaped quote: the escape-aware close
//     search is not fooled by `\"` into closing early.
#[test]
fn test_22_string_with_escaped_quote() {
    let c = JsonParser::parse(br#"{"a":"say \"hi\""}"#);
    assert_eq!(c.members.len(), 1);
    assert_eq!(c.str(c.members[0].value).unwrap(), r#""say \"hi\"""#);
    // The string content itself (between the outer quotes) is captured too.
    assert!(texts(&c, &c.strings).iter().any(|s| s == r#"say \"hi\""#));
}

// 23. A string value ending in an escaped backslash followed by the real
//     closing quote: an even run of backslashes before `"` does not escape
//     it.
#[test]
fn test_23_string_ending_escaped_backslash_then_real_quote() {
    let c = JsonParser::parse(br#"{"a":"x\\"}"#);
    assert_eq!(members(&c), vec![(r#""a""#.into(), r#""x\\""#.into())]);
}

// 24. A key containing an escaped quote: the key span still spans the whole
//     quoted run, escape included.
#[test]
fn test_24_key_with_escaped_quote_inside() {
    let c = JsonParser::parse(br#"{"a\"b":1}"#);
    assert_eq!(c.members.len(), 1);
    assert_eq!(c.str(c.members[0].key).unwrap(), r#""a\"b""#);
}

// 25. A string value containing `:` and `,` — neither breaks parsing; both
//     bytes are opaque content inside the string, not structure.
#[test]
fn test_25_string_containing_colon_and_comma_not_structural() {
    let c = JsonParser::parse(br#"{"a":"x:y,z","b":2}"#);
    assert_eq!(c.members.len(), 2);
    assert_eq!(
        members(&c),
        vec![
            (r#""a""#.into(), r#""x:y,z""#.into()),
            (r#""b""#.into(), "2".into()),
        ]
    );
}

// 26. A string value containing brackets — they stay opaque content, no
//     phantom container is opened inside the string.
#[test]
fn test_26_string_containing_brackets_not_structural() {
    let c = JsonParser::parse(br#"{"a":"x{y[z"}"#);
    assert_eq!(c.objects.len(), 1);
    assert_eq!(c.arrays.len(), 0);
    assert_eq!(members(&c), vec![(r#""a""#.into(), r#""x{y[z""#.into())]);
}

// ==========================================================================
// Arrays — structural only (the engine no longer tracks individual elements)
// ==========================================================================

// 27. A flat numeric array: one container. `arrays[i]` is content-only (the
//     engine's universal asymmetric-field convention — brackets excluded);
//     the bracket-inclusive raw form is the generated `arrays_raw()`
//     accessor.
#[test]
fn test_27_array_whole_span_text() {
    let c = JsonParser::parse(br#"[1,2,-3,42]"#);
    assert_eq!(c.arrays.len(), 1);
    assert_eq!(c.str(c.arrays[0]).unwrap(), "1,2,-3,42");
    assert_eq!(
        std::str::from_utf8(c.arrays_raw().next().unwrap()).unwrap(),
        "[1,2,-3,42]"
    );
    assert!(c.members.is_empty());
}

// 28. An array of strings: every element lands in `strings`.
#[test]
fn test_28_array_of_strings_in_strings_field() {
    let c = JsonParser::parse(br#"["x","y","z"]"#);
    assert_eq!(c.arrays.len(), 1);
    assert_eq!(sorted(&c, &c.strings), vec!["x", "y", "z"]);
}

// 29. Nested arrays: each level is its own container.
#[test]
fn test_29_nested_arrays_depth_count() {
    let c = JsonParser::parse(br#"[[1, 2], [3]]"#);
    assert_eq!(c.arrays.len(), 3); // one outer + two inner
}

// 30. An array of objects: containers and members counted on both axes.
#[test]
fn test_30_array_of_objects_counts() {
    let c = JsonParser::parse(br#"[{"a":1},{"b":2}]"#);
    assert_eq!(c.arrays.len(), 1);
    assert_eq!(c.objects.len(), 2);
    assert_eq!(
        members(&c),
        vec![(r#""a""#.into(), "1".into()), (r#""b""#.into(), "2".into())]
    );
}

// 31. Four levels of array nesting: four containers.
#[test]
fn test_31_deeply_nested_arrays_count() {
    let c = JsonParser::parse(br#"[[[[1]]]]"#);
    assert_eq!(c.arrays.len(), 4);
}

// 32. A double comma inside an array does not panic; the array still closes
//     and its content span (brackets excluded) matches; the `_raw()`
//     accessor recovers the bracket-inclusive form. (Element-level
//     robustness for the double comma is now a post-pass concern, not an
//     engine one.)
#[test]
fn test_32_array_double_comma_no_panic() {
    let c = JsonParser::parse(br#"[1,,2]"#);
    assert_eq!(c.arrays.len(), 1);
    assert_eq!(c.str(c.arrays[0]).unwrap(), "1,,2");
    assert_eq!(
        std::str::from_utf8(c.arrays_raw().next().unwrap()).unwrap(),
        "[1,,2]"
    );
}

// 33. An array value alongside a sibling scalar member: the array member's
//     raw value is the whole array; the sibling is unaffected.
#[test]
fn test_33_array_in_member_value_whole_span() {
    let c = JsonParser::parse(br#"{"a":[1,2],"b":3}"#);
    assert_eq!(c.members.len(), 2);
    assert_eq!(
        members(&c),
        vec![
            (r#""a""#.into(), "[1,2]".into()),
            (r#""b""#.into(), "3".into())
        ]
    );
}

// ==========================================================================
// Object nesting
// ==========================================================================

// 34. Three levels deep: three objects, three members.
#[test]
fn test_34_nested_objects_three_levels() {
    let c = JsonParser::parse(br#"{"a":{"b":{"c":1}}}"#);
    assert_eq!(c.objects.len(), 3);
    assert_eq!(c.members.len(), 3);
}

// 35. Mixed nesting (object -> array -> objects -> array): counts on every
//     axis, the deepest combination this suite exercises.
#[test]
fn test_35_mixed_nesting_array_object_array() {
    let c = JsonParser::parse(br#"{"a":[{"b":[1,2]},{"c":3}]}"#);
    // outer object, plus the two objects inside the array.
    assert_eq!(c.objects.len(), 3);
    // "a"'s array, plus "b"'s inner array.
    assert_eq!(c.arrays.len(), 2);
    // "a" (outer), "b" (inner), "c" (inner) — three key:value pairs total.
    assert_eq!(c.members.len(), 3);
}

// 36. Three sibling objects directly inside a top-level array.
#[test]
fn test_36_sibling_objects_in_top_level_array() {
    let c = JsonParser::parse(br#"[{"x":1},{"y":2},{"z":3}]"#);
    assert_eq!(c.arrays.len(), 1);
    assert_eq!(c.objects.len(), 3);
    assert_eq!(
        members(&c),
        vec![
            (r#""x""#.into(), "1".into()),
            (r#""y""#.into(), "2".into()),
            (r#""z""#.into(), "3".into()),
        ]
    );
}

// ==========================================================================
// Projection / containment sanity (still no scalar typing)
// ==========================================================================

// 37. The member value's raw bytes match the literal source slice exactly.
#[test]
fn test_37_member_value_matches_raw_text() {
    let c = JsonParser::parse(br#"{"a":7}"#);
    assert_eq!(c.bytes(c.members[0].value), b"7");
}

// 38. When a value is a container, the member's `value` span is raw and
//     bracket-inclusive — byte-equal to the matching `arrays_raw()` slice,
//     NOT to the bare `arrays` span (which is content-only; see the doc
//     comment at the top of this crate's `lib.rs`). Containment still holds,
//     just expressed against the `_raw()` form.
#[test]
fn test_38_member_value_container_containment_matches_array_span() {
    let c = JsonParser::parse(br#"{"a":[1,2,3]}"#);
    assert_eq!(c.members.len(), 1);
    assert_eq!(c.arrays.len(), 1);
    let v = c.bytes(c.members[0].value);
    let raw = c.arrays_raw().next().unwrap();
    assert_eq!(v, raw);
}

// 39. The bare `objects` field is content-only (braces excluded — the
//     engine's universal asymmetric-field convention); the generated
//     `objects_raw()` accessor recovers the brace-inclusive form.
#[test]
fn test_39_object_span_includes_braces() {
    let c = JsonParser::parse(br#"{"a":1}"#);
    let content = c.str(c.objects[0]).unwrap();
    assert_eq!(content, r#""a":1"#);
    assert!(!content.starts_with('{'));
    let raw = std::str::from_utf8(c.objects_raw().next().unwrap()).unwrap();
    assert!(raw.starts_with('{') && raw.ends_with('}'));
    assert_eq!(raw, r#"{"a":1}"#);
}

// 40. The bare `arrays` field is content-only (brackets excluded); the
//     generated `arrays_raw()` accessor recovers the bracket-inclusive form.
#[test]
fn test_40_array_span_includes_brackets() {
    let c = JsonParser::parse(br#"[1,2]"#);
    let content = c.str(c.arrays[0]).unwrap();
    assert_eq!(content, "1,2");
    assert!(!content.starts_with('['));
    let raw = std::str::from_utf8(c.arrays_raw().next().unwrap()).unwrap();
    assert!(raw.starts_with('[') && raw.ends_with(']'));
    assert_eq!(raw, "[1,2]");
}

// ==========================================================================
// Whitespace & raw-value semantics (no scalar trim layer anymore)
// ==========================================================================

// 41. Spaces around `:` and `,`: `allow_sep` skips exactly one leading space
//     right after `:`, but nothing trims the *trailing* side — the value's
//     raw span runs up to its terminator byte verbatim, trailing space
//     included. This is intentional now that the scalar trim layer is gone;
//     pinned down explicitly rather than left to silently drift.
#[test]
fn test_41_spaces_around_colon_and_comma() {
    let c = JsonParser::parse(br#"{ "a" : 1 , "b" : 2 }"#);
    assert_eq!(c.members.len(), 2);
    let (k0, v0) = &members(&c)[0];
    let (k1, v1) = &members(&c)[1];
    assert_eq!(k0, r#""a""#);
    assert_eq!(v0, "1 "); // trailing space before the comma, kept raw
    assert_eq!(k1, r#""b""#);
    assert_eq!(v1, "2 "); // trailing space before the closing brace, kept raw
}

// 42. A tab immediately after `:` is NOT skipped — `allow_sep` only skips a
//     single leading *space*, not a tab — so the raw value starts with the
//     tab itself.
#[test]
fn test_42_tab_after_colon_not_skipped_raw() {
    let c = JsonParser::parse(b"{\"a\":\t5}");
    assert_eq!(c.members.len(), 1);
    assert_eq!(c.str(c.members[0].value).unwrap(), "\t5");
}

// 43. A value spanning a newline: the raw span includes the embedded `\n`
//     (and any indentation on the following line) verbatim — nothing trims
//     it without the old scalar layer.
#[test]
fn test_43_value_spanning_newline_raw() {
    let c = JsonParser::parse(b"{\"a\":\n  5\n}");
    assert_eq!(c.members.len(), 1);
    assert_eq!(c.str(c.members[0].value).unwrap(), "\n  5\n");
}

// 44. A value followed by CRLF before the closing brace: `\r` is not a
//     recognised byte anywhere in this grammar (only `\n` is `eol`), so it is
//     ordinary content, captured raw as part of the value.
#[test]
fn test_44_value_with_crlf_raw() {
    let c = JsonParser::parse(b"{\"a\":1\r\n}");
    assert_eq!(c.members.len(), 1);
    assert_eq!(c.objects.len(), 1);
    assert_eq!(c.str(c.members[0].value).unwrap(), "1\r\n");
}

// ==========================================================================
// Multi-line / pretty-printed JSON — the new streaming capability
// ==========================================================================

// 45. A simple two-member object, pretty-printed across four lines: the
//     unified inline stack must survive each internal `\n` untouched.
#[test]
fn test_45_pretty_printed_simple_object() {
    let src = b"{\n  \"a\": 1,\n  \"b\": 2\n}";
    let c = JsonParser::parse(src);
    assert_eq!(c.objects.len(), 1);
    assert_eq!(c.members.len(), 2);
    let m = members(&c);
    assert_eq!(m[0].0, r#""a""#);
    assert_eq!(m[1].0, r#""b""#);
}

// 46. Three levels of nesting, each on its own indented line: object/member
//     counts must match the compact equivalent exactly.
#[test]
fn test_46_pretty_printed_nested_object() {
    let pretty = b"{\n  \"a\": {\n    \"b\": {\n      \"c\": 1\n    }\n  }\n}";
    let compact = br#"{"a":{"b":{"c":1}}}"#;
    let cp = JsonParser::parse(pretty);
    let cc = JsonParser::parse(compact);
    assert_eq!(cp.objects.len(), cc.objects.len());
    assert_eq!(cp.objects.len(), 3);
    assert_eq!(cp.members.len(), cc.members.len());
    assert_eq!(cp.members.len(), 3);
}

// 47. A typical pretty-printed "API response": an array of objects spread
//     across many lines.
#[test]
fn test_47_pretty_printed_array_of_objects() {
    let src = b"[\n  {\n    \"id\": 1,\n    \"name\": \"Alice\"\n  },\n  {\n    \"id\": 2,\n    \"name\": \"Bob\"\n  }\n]";
    let c = JsonParser::parse(src);
    assert_eq!(c.arrays.len(), 1);
    assert_eq!(c.objects.len(), 2);
    assert_eq!(c.members.len(), 4);
    assert_eq!(
        sorted(&c, &c.strings),
        vec!["Alice", "Bob", "id", "id", "name", "name"]
    );
}

// 48. Deep indentation across many lines: the stack still resolves correctly
//     at the far end of a long multi-line run, not just for short inputs.
#[test]
fn test_48_stack_survives_many_lines_deep_indent() {
    let src = b"{\n  \"a\": {\n    \"b\": {\n      \"c\": {\n        \"d\": {\n          \"e\": 1\n        }\n      }\n    }\n  }\n}";
    let c = JsonParser::parse(src);
    assert_eq!(c.objects.len(), 5);
    assert_eq!(c.members.len(), 5);
}

// ==========================================================================
// EOL-drain / unterminated input
// ==========================================================================

// 49. An unterminated object still finalises its open key_value pair at end
//     of input (the kv frame's own end-of-run drain): the member is
//     committed, but the unclosed container span itself is discarded
//     (`Vec::remove` on the never-closed placeholder). Nothing leaks into
//     `scalars` here because the kv drain explicitly advances `text_start`
//     past the committed value before the final flush runs.
#[test]
fn test_49_unterminated_object_member_committed_container_discarded() {
    let c = JsonParser::parse(br#"{"a":1"#);
    assert_eq!(c.objects.len(), 0);
    assert_eq!(members(&c), vec![(r#""a""#.into(), "1".into())]);
    assert!(c.scalars.is_empty());
}

// 50. An unterminated *bare* (unwrapped) array: the array placeholder is
//     discarded the same way, but — unlike the object case above — there is
//     no key_value frame here to advance `text_start` past the content. The
//     unconditional final flush therefore sweeps the array's untracked tail
//     content into `scalars`. This asymmetry is a direct, deterministic
//     consequence of the drain order, not a bug; pinned down here on purpose.
#[test]
fn test_50_unterminated_array_discarded_tail_leaks_to_scalars() {
    let c = JsonParser::parse(br#"[1,2"#);
    assert_eq!(c.arrays.len(), 0);
    assert_eq!(texts(&c, &c.scalars), vec!["1,2"]);
}

// 51. A top-level unterminated string: the legacy (off-stack) string search
//     never finds a close, so nothing is pushed to `strings`; the whole
//     input — orphan opening quote included — falls through to the
//     unconditional final flush as one `scalars` entry.
#[test]
fn test_51_unterminated_string_top_level_falls_to_scalars() {
    let c = JsonParser::parse(b"\"abc");
    assert!(c.strings.is_empty());
    assert_eq!(texts(&c, &c.scalars), vec!["\"abc"]);
}

// ==========================================================================
// Top-level scalar fallback
// ==========================================================================

// 52. A bare top-level value (no wrapping object or array) is not a
//     container, a member, or a string — it falls straight through to the
//     inline fallback field.
#[test]
fn test_52_bare_top_level_scalar_in_scalars_field() {
    let c = JsonParser::parse(br#"42"#);
    assert_eq!(c.objects.len(), 0);
    assert_eq!(c.arrays.len(), 0);
    assert_eq!(c.members.len(), 0);
    assert_eq!(texts(&c, &c.scalars), vec!["42"]);
}

// ==========================================================================
// Robustness (malformed input: no panic, sane partial output)
// ==========================================================================

// 53. A mismatched closing bracket (`]` where the open container is `{`)
//     does not panic. The byte is claimed by neither cascade branch (the
//     kv-drain-before-pop check requires the frame below to close on THIS
//     byte; the container-pop check requires the TOP frame to close on it;
//     neither matches `]` against an object's `}`), so `text_start` is never
//     advanced past it. The kv frame for "a" is still open at end-of-run, so
//     its value is finalised verbatim to `parse_end` — the stray `]`
//     included, consistent with the raw/untrimmed value semantics
//     established above. The object itself was never properly closed, so it
//     is discarded the same way any unterminated object is.
#[test]
fn test_53_mismatched_closing_bracket_no_panic() {
    let c = JsonParser::parse(br#"{"a":1]"#);
    assert_eq!(c.objects.len(), 0);
    assert_eq!(members(&c), vec![(r#""a""#.into(), "1]".into())]);
}

// 54. Trailing garbage after a complete, valid JSON value: the valid part is
//     still parsed correctly; the garbage tail is swept into `scalars`.
#[test]
fn test_54_trailing_garbage_after_valid_json() {
    let c = JsonParser::parse(br#"{"a":1}garbage"#);
    assert_eq!(c.objects.len(), 1);
    assert_eq!(members(&c), vec![(r#""a""#.into(), "1".into())]);
    assert_eq!(texts(&c, &c.scalars), vec!["garbage"]);
}

// 55. Multiple top-level siblings with no wrapping array: the engine's flat
//     model has no concept of "one JSON document" — it just reports every
//     container it sees, regardless of how many sit at the top level.
#[test]
fn test_55_multiple_top_level_siblings_no_wrapper() {
    let c = JsonParser::parse(br#"{}{}[]"#);
    assert_eq!(c.objects.len(), 2);
    assert_eq!(c.arrays.len(), 1);
}

// 56. Nesting close to (but under) the grammar's `max_nest = 64` cap: no
//     panic, no silent truncation at moderate depth.
#[test]
fn test_56_deep_nesting_near_max_no_panic() {
    const DEPTH: usize = 30;
    let mut s = String::new();
    for _ in 0..DEPTH {
        s.push_str(r#"{"a":"#);
    }
    s.push('1');
    for _ in 0..DEPTH {
        s.push('}');
    }
    let c = JsonParser::parse(s.as_bytes());
    assert_eq!(c.objects.len(), DEPTH);
    assert_eq!(c.members.len(), DEPTH);
}

// ==========================================================================
// Nesting depth — AT and BEYOND the max_nest = 64 cap
//
// test_56 only reached depth 30 (well under the cap). These pin the boundary
// itself, where the bounded-stack / overflow path actually engages.
//
// Frame budget matters and differs by shape:
//   - pure array nesting `[[[...]]]` costs ONE frame per level (one asymmetric
//     open), so 64 levels exactly fill the stack.
//   - object nesting `{"a":{...}}` costs TWO frames per level (the `{` open
//     AND the `:` key_value frame), so the effective cap is 32 levels = 64
//     frames. This is why test_56 chose 30 — it was already near that real
//     ceiling for the object shape.
// ==========================================================================

// 57. Pure array nesting at EXACTLY the cap: 64 levels, one frame each, fill
//     the stack precisely with no overflow. All 64 containers are tracked.
#[test]
fn test_57_array_nesting_at_max_nest_cap() {
    let depth = 64;
    let s = "[".repeat(depth) + &"]".repeat(depth);
    let c = JsonParser::parse(s.as_bytes());
    assert_eq!(c.arrays.len(), 64);
}

// 58. Pure array nesting BEYOND the cap: 70 levels. The 6 innermost opens
//     find the stack full and bump the one-shot overflow counter instead of
//     pushing a frame; their matching closes drain that counter symmetrically.
//     No kv frames interleave here, so the accounting is clean — exactly the
//     64 outermost containers are tracked, and nothing panics or overcounts.
#[test]
fn test_58_array_nesting_beyond_cap_clamped() {
    let depth = 70;
    let s = "[".repeat(depth) + &"]".repeat(depth);
    let c = JsonParser::parse(s.as_bytes());
    assert_eq!(c.arrays.len(), 64);
}

// 59. Object nesting at the object-shape's true cap: 32 levels of `{"a":...}`
//     consume 64 frames (one `{` + one `:` kv frame per level) — the stack
//     fills exactly, with no overflow. Every object and every member is
//     tracked. This is the boundary test_56 sat just under.
#[test]
fn test_59_object_nesting_at_frame_cap() {
    let depth = 32;
    let s = r#"{"a":"#.repeat(depth) + "1" + &"}".repeat(depth);
    let c = JsonParser::parse(s.as_bytes());
    assert_eq!(c.objects.len(), 32);
    assert_eq!(c.members.len(), 32);
}

// 60. Object nesting BEYOND the frame cap: 40 levels need 80 frames; only the
//     outer 32 fit. The over-cap levels engage the overflow counter, which
//     interleaves with the per-level key_value drains in an
//     implementation-defined way — so this test deliberately asserts only the
//     robust invariants (no panic, counts bounded by the cap, at least the
//     outermost level survives) rather than an exact over-cap count. If you
//     ever want exact numbers here, decide and document the
//     overflow-vs-kv-drain ordering first; today it is intentionally not a
//     contract.
#[test]
fn test_60_object_nesting_beyond_cap_no_panic_bounded() {
    let depth = 40;
    let s = r#"{"a":"#.repeat(depth) + "1" + &"}".repeat(depth);
    let c = JsonParser::parse(s.as_bytes());
    assert!(c.objects.len() <= 32);
    assert!(c.members.len() <= 32);
    assert!(!c.members.is_empty());
}

// ==========================================================================
// Blank lines — the multi-line run boundary
//
// The whole multi-line capability rests on "a document with NO blank lines":
// a blank line closes the accumulated run and drains the inline stack. These
// two pin both sides of that contract — the benign case and the cliff.
// ==========================================================================

// 61. A blank line BETWEEN two complete, self-contained documents is benign:
//     each run closes cleanly on its own, so both objects and both members
//     are recovered, and the block-level fallback records the two runs as two
//     separate `loose` spans.
#[test]
fn test_61_blank_line_between_complete_documents() {
    let c = JsonParser::parse(b"{\"a\":1}\n\n{\"b\":2}");
    assert_eq!(c.objects.len(), 2);
    assert_eq!(c.members.len(), 2);
    assert_eq!(c.loose.len(), 2);
}

// 62. A blank line INSIDE a container is the documented limitation, pinned
//     down as a characterization test. The `{` sits on its own run, which the
//     blank line closes before the object can ever reach its `}` — so the open
//     container frame is discarded at the run's end and the object is LOST
//     (`objects == 0`). The trailing `"a": 1` becomes a second run; its
//     key_value pair still finalises at end-of-run, so a member survives, but
//     with no enclosing object. No panic either way. If the run model ever
//     learns to span blank lines, this test will flag the behaviour change.
#[test]
fn test_62_blank_line_inside_container_breaks_it() {
    let c = JsonParser::parse(b"{\n\n  \"a\": 1\n}");
    assert_eq!(c.objects.len(), 0); // the outer object did not survive
    assert_eq!(c.members.len(), 1); // its member did, orphaned
    assert_eq!(c.loose.len(), 2); // two runs, split by the blank line
}

// ==========================================================================
// Non-ASCII / UTF-8 content
//
// Every prior test is ASCII; spans are byte offsets, so multibyte content is
// the natural place for an off-by-one to hide, and `str()`'s documented
// "None on invalid UTF-8, never panic" contract had zero coverage.
// ==========================================================================

// 63. A multibyte (Cyrillic) key and value resolve to the correct slices —
//     byte offsets land on char boundaries, `str()` succeeds.
#[test]
fn test_63_multibyte_key_and_value_resolve() {
    let c = JsonParser::parse("{\"ключ\":\"значение\"}".as_bytes());
    assert_eq!(c.members.len(), 1);
    assert_eq!(c.str(c.members[0].key).unwrap(), "\"ключ\"");
    assert_eq!(c.str(c.members[0].value).unwrap(), "\"значение\"");
    let s = texts(&c, &c.strings);
    assert!(s.contains(&"ключ".to_string()));
    assert!(s.contains(&"значение".to_string()));
}

// 64. A 4-byte UTF-8 scalar (emoji) as a string value: the value span covers
//     all four content bytes plus the quotes, and resolves cleanly.
#[test]
fn test_64_emoji_string_value_resolves() {
    let c = JsonParser::parse("{\"a\":\"😀\"}".as_bytes());
    assert_eq!(c.members.len(), 1);
    assert_eq!(c.str(c.members[0].value).unwrap(), "\"😀\"");
    assert!(texts(&c, &c.strings).contains(&"😀".to_string()));
}

// 65. Invalid UTF-8 inside a string value: the engine works on raw bytes, so
//     parsing still succeeds structurally — but `str()` returns `None` (the
//     documented contract) instead of panicking, while `bytes()` hands back
//     the raw span verbatim. Note this test cannot use the `members()` helper,
//     which unwraps `str()`.
#[test]
fn test_65_invalid_utf8_value_str_none_bytes_raw() {
    let c = JsonParser::parse(b"{\"a\":\"\xff\xfe\"}");
    assert_eq!(c.members.len(), 1);
    assert!(c.str(c.members[0].value).is_none()); // contract: None, not panic
    assert_eq!(c.bytes(c.members[0].value), b"\"\xff\xfe\"");
}

// ==========================================================================
// `loose` — block-level fallback (described in this file's header but, until
// now, never actually asserted on)
// ==========================================================================

// 66. For a single-line document, `loose` is exactly one span covering the
//     whole input — the engine has no blank lines to split the run on.
#[test]
fn test_66_loose_single_span_covers_whole_input() {
    let src = br#"{"a":1}"#;
    let c = JsonParser::parse(src);
    assert_eq!(c.loose.len(), 1);
    assert_eq!(c.bytes(c.loose[0]), src);
}

// 67. A pretty-printed, multi-line document with no blank lines is still ONE
//     `loose` span over the whole input: internal `\n`s are ordinary run
//     content, not run boundaries.
#[test]
fn test_67_loose_single_span_over_multiline_no_blank() {
    let src = b"{\n  \"a\": 1,\n  \"b\": 2\n}";
    let c = JsonParser::parse(src);
    assert_eq!(c.loose.len(), 1);
    assert_eq!(c.bytes(c.loose[0]), src);
}

// ==========================================================================
// Accessor coverage: `strings_raw()` / `strings_clean()`
//
// Prior tests exercised `arrays_raw()` / `objects_raw()` but never the
// generated accessors on the `strings` field.
// ==========================================================================

// 68. `strings_raw()` yields the quote-inclusive slice; `strings_clean()`
//     yields the bare content. (`strings` itself is the content-only form.)
#[test]
fn test_68_strings_raw_and_clean_accessors() {
    let c = JsonParser::parse(br#"{"a":"b"}"#);
    let raws: Vec<String> = c
        .strings_raw()
        .map(|b| std::str::from_utf8(b).unwrap().to_string())
        .collect();
    assert!(raws.contains(&r#""a""#.to_string()));
    assert!(raws.contains(&r#""b""#.to_string()));

    let cleans: Vec<String> = c
        .strings_clean()
        .map(|b| std::str::from_utf8(b).unwrap().to_string())
        .collect();
    assert!(cleans.contains(&"a".to_string()));
    assert!(cleans.contains(&"b".to_string()));
}

// ==========================================================================
// More malformed / edge shapes (no panic, sane partial output)
// ==========================================================================

// 69. A trailing comma in an object is tolerated: the `,` finalises the
//     preceding member, and the immediately-following `}` closes the object
//     with no phantom extra member.
#[test]
fn test_69_object_trailing_comma_tolerated() {
    let c = JsonParser::parse(br#"{"a":1,}"#);
    assert_eq!(c.objects.len(), 1);
    assert_eq!(members(&c), vec![(r#""a""#.into(), "1".into())]);
}

// 70. A trailing comma in an array does not panic; the array still closes and
//     its content span keeps the trailing comma verbatim (element-level
//     handling is a post-pass concern, not the engine's).
#[test]
fn test_70_array_trailing_comma_no_panic() {
    let c = JsonParser::parse(br#"[1,2,]"#);
    assert_eq!(c.arrays.len(), 1);
    assert_eq!(c.str(c.arrays[0]).unwrap(), "1,2,");
}

// 71. A comma INSIDE a string element is opaque content, not a structural
//     separator — the string is captured whole, comma and all. (The same
//     guarantee test_25 made for a member value, here for an array element.)
#[test]
fn test_71_comma_inside_array_element_string_is_opaque() {
    let c = JsonParser::parse(br#"["a,b","c"]"#);
    assert_eq!(c.arrays.len(), 1);
    let s = texts(&c, &c.strings);
    assert!(s.contains(&"a,b".to_string()));
    assert!(s.contains(&"c".to_string()));
}

// 72. A `\uXXXX` escape is NOT decoded — the engine treats the backslash run
//     as ordinary opaque bytes, so the raw value carries the literal
//     `\u0041`, not `A`. (Decoding, if wanted, is a caller concern.)
#[test]
fn test_72_unicode_escape_is_opaque_content() {
    let c = JsonParser::parse(br#"{"a":"\u0041"}"#);
    assert_eq!(c.members.len(), 1);
    assert_eq!(c.str(c.members[0].value).unwrap(), r#""\u0041""#);
    assert!(texts(&c, &c.strings).contains(&r#"\u0041"#.to_string()));
}

// 73. A missing value (`{"a":}`): the `:` opens a key_value frame whose value
//     is empty, and the `}` immediately closes it. The member is committed
//     with an empty value span; no panic.
#[test]
fn test_73_missing_value_empty_no_panic() {
    let c = JsonParser::parse(br#"{"a":}"#);
    assert_eq!(c.objects.len(), 1);
    assert_eq!(c.members.len(), 1);
    assert_eq!(c.str(c.members[0].key).unwrap(), r#""a""#);
    assert_eq!(c.str(c.members[0].value).unwrap(), "");
}

// 74. A key with no colon (`{"a"}`): no `:` means no key_value frame is ever
//     opened, so there is no member — just the object, with the bare string
//     `"a"` landing in `strings`. No panic.
#[test]
fn test_74_key_without_colon_no_member() {
    let c = JsonParser::parse(br#"{"a"}"#);
    assert_eq!(c.objects.len(), 1);
    assert_eq!(c.members.len(), 0);
    assert_eq!(sorted(&c, &c.strings), vec!["a"]);
}
