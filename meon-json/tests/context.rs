//! Integration tests for [`meon::ParseContext`] on the JSON grammar: strings
//! are the (only) opaque construct, so `find_context_objects` /
//! `find_context_arrays` stop matching brackets that live inside string
//! content.

use meon_json::JsonParser;

// 01. The context covers exactly the string runs, quotes included
#[test]
fn test_01_context_is_strings() {
    let src = br#"{"a": "x", "b": 1}"#;
    let ctx = JsonParser::context(src);
    let got: Vec<(u32, u32)> = ctx.spans().iter().map(|s| (s.start, s.end)).collect();
    assert_eq!(got, vec![(1, 4), (6, 9), (11, 14)]);
}

// 02. Braces/brackets inside a string no longer corrupt object matching
#[test]
fn test_02_brace_inside_string() {
    let src = br#"{"note": "u{se} [it]"}"#;
    let ctx = JsonParser::context(src);
    // Context-free: the close search stops at the `}` INSIDE the string,
    // yielding one wrong span that ends mid-string.
    let cf: Vec<(u32, u32)> = JsonParser::find_objects(src)
        .map(|s| (s.start, s.end))
        .collect();
    assert_eq!(cf, vec![(1, 14)]);
    // Context-aware: the in-string `}` is skipped, the object closes at the
    // real brace — byte-identical to the full parse.
    let ca: Vec<(u32, u32)> = JsonParser::find_context_objects(src, &ctx)
        .map(|s| (s.start, s.end))
        .collect();
    assert_eq!(ca, vec![(1, 21)]);
    // The `[it]` inside the string is invisible to the context-aware array
    // finder.
    assert_eq!(JsonParser::find_context_arrays(src, &ctx).count(), 0);
    assert_eq!(JsonParser::find_arrays(src).count(), 1);
}

// 03. Escaped quotes inside strings do not end the context region early
#[test]
fn test_03_escaped_quote() {
    let src = br#"{"a": "say \"{hi}\" ok", "b": [1]}"#;
    let ctx = JsonParser::context(src);
    assert_eq!(JsonParser::find_context_objects(src, &ctx).count(), 1);
    assert_eq!(JsonParser::find_context_arrays(src, &ctx).count(), 1);
}

// 04. Strings are opaque rules: no context-aware variant is generated for
//     them, and the context-free finder still works
#[test]
fn test_04_strings_context_free_finder_kept() {
    let src = br#"{"a": "x"}"#;
    assert_eq!(JsonParser::find_strings(src).count(), 2);
}
