//! Integration tests for the generated context-free finders of `meon-json`:
//! on input where only the finder's own rule is at work, `find_*` yields
//! what `JsonParser::parse` puts in the matching field.

use meon::span::Span;
use meon_json::JsonParser;

fn spans(it: impl Iterator<Item = Span>) -> Vec<(u32, u32)> {
    it.map(|s| (s.start, s.end)).collect()
}

/// A member's key and value spans.
type Member = (Span, Span);

/// The members of the full parse and those `find_members` yields.
fn members(src: &[u8]) -> (Vec<Member>, Vec<Member>) {
    let full = JsonParser::parse(src)
        .members
        .iter()
        .map(|m| (m.key, m.value))
        .collect();
    let find = JsonParser::find_members(src)
        .map(|m| (m.key, m.value))
        .collect();
    (full, find)
}

// 01. Containers nest, an escaped close byte closes nothing, and the spans
//     come in open order
#[test]
fn test_01_containers() {
    for src in [&b"{a{b}c}"[..], b"{a\\}b}", b"[[1],[2,[3]]]", b"{a {b}"] {
        let full = JsonParser::parse(src);
        assert_eq!(
            spans(JsonParser::find_objects(src)),
            spans(full.objects.iter().copied())
        );
        assert_eq!(
            spans(JsonParser::find_arrays(src)),
            spans(full.arrays.iter().copied())
        );
    }
    assert_eq!(spans(JsonParser::find_objects(b"{a\\}b}")), vec![(1, 5)]);
}

// 02. An escaped `:` opens no member and an escaped `,` ends none; the next
//     key starts past a whole `,` run; a value goes on across a line break
#[test]
fn test_02_members() {
    for src in [
        &b"a\\:b,c:d"[..],
        b"a:x\\,b:1",
        b"a:1,,b:2",
        b"a: 1\nb: 2",
        b"a:1\n\nb:2",
    ] {
        let (full, find) = members(src);
        assert_eq!(find, full, "{:?}", String::from_utf8_lossy(src));
    }
    let (_, find) = members(b"a\\:b,c:d");
    assert_eq!(find, vec![(Span::new(0, 6), Span::new(7, 8))]);
}
