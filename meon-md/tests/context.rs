//! Integration tests for [`meon::ParseContext`] and the generated
//! `find_context_*` standalone iterators on the Markdown grammar.
//!
//! The contract under test: a context-aware finder agrees with the full
//! parse where the context models the source. Candidates inside an opaque
//! region (fenced block, code span, autolink) or among a line's markers are
//! skipped, a pair stays inside one inline run, and a line-family item is
//! kept when the full parse gives its line to the item's rule. Enclosing
//! spans may still *contain* opaque regions.

use meon_md::MarkdownParser;
use meon_md::span::Span;

fn spans(it: impl Iterator<Item = Span>) -> Vec<(u32, u32)> {
    it.map(|s| (s.start, s.end)).collect()
}

// 01. Empty context on plain text: context-aware == context-free
#[test]
fn test_01_no_opaque_no_difference() {
    let src = b"plain **bold** and *italic* text";
    let ctx = MarkdownParser::context(src);
    assert!(ctx.spans().is_empty());
    assert_eq!(
        spans(MarkdownParser::find_context_bolds(src, &ctx)),
        spans(MarkdownParser::find_bolds(src)),
    );
}

// 02. Bold inside a fenced block is suppressed; outside is kept
#[test]
fn test_02_bold_inside_fence() {
    let src = b"**a**\n```\n**b**\n```\n**c**";
    let ctx = MarkdownParser::context(src);
    assert_eq!(spans(MarkdownParser::find_bolds(src)).len(), 3);
    assert_eq!(
        spans(MarkdownParser::find_context_bolds(src, &ctx)),
        vec![(2, 3), (22, 23)],
    );
}

// 03. Bold delimiters inside a code span are suppressed
#[test]
fn test_03_bold_inside_code_span() {
    let src = b"code `**x**` and **real**";
    let ctx = MarkdownParser::context(src);
    assert_eq!(
        spans(MarkdownParser::find_context_bolds(src, &ctx)),
        vec![(19, 23)],
    );
}

// 04. A bold span may contain a code span (context suppresses positions,
//     not enclosing spans) — matches the full-parse shape
#[test]
fn test_04_bold_containing_code_span() {
    let src = b"**a `code` b**";
    let ctx = MarkdownParser::context(src);
    let bolds = spans(MarkdownParser::find_context_bolds(src, &ctx));
    assert_eq!(bolds, vec![(2, 12)]);
    let full = MarkdownParser::parse(src);
    assert_eq!(
        bolds,
        full.bolds
            .iter()
            .map(|s| (s.start, s.end))
            .collect::<Vec<_>>(),
    );
}

// 05. Headings inside a fenced block are suppressed
#[test]
fn test_05_heading_inside_fence() {
    let src = b"# real\n```\n# fake\n```\n## real2";
    let ctx = MarkdownParser::context(src);
    let aware: Vec<_> = MarkdownParser::find_context_headings(src, &ctx).collect();
    assert_eq!(aware.len(), 2);
    assert_eq!(u8::from(aware[0].0.level), 1);
    assert_eq!(u8::from(aware[1].0.level), 2);
    // Context-free finder sees the fake one too.
    assert_eq!(MarkdownParser::find_headings(src).count(), 3);
}

// 06. Bullet items and ordered items inside a fence are suppressed
#[test]
fn test_06_list_items_inside_fence() {
    let src = b"- real\n```\n- fake\n1. fake\n```\n1. real";
    let ctx = MarkdownParser::context(src);
    assert_eq!(
        MarkdownParser::find_context_bullet_items(src, &ctx).count(),
        1
    );
    assert_eq!(
        MarkdownParser::find_context_ordered_items(src, &ctx).count(),
        1
    );
    assert_eq!(MarkdownParser::find_bullet_items(src).count(), 2);
    assert_eq!(MarkdownParser::find_ordered_items(src).count(), 2);
}

// 07. Thematic breaks inside a fence are suppressed
#[test]
fn test_07_thematic_break_inside_fence() {
    let src = b"---\n```\n---\n```\n";
    let ctx = MarkdownParser::context(src);
    assert_eq!(
        MarkdownParser::find_context_thematic_breaks(src, &ctx).count(),
        1
    );
    assert_eq!(MarkdownParser::find_thematic_breaks(src).count(), 2);
}

// 08. Blockquote markers inside a fence are suppressed
#[test]
fn test_08_blockquote_inside_fence() {
    let src = b"> real\n```\n> fake\n```\n";
    let ctx = MarkdownParser::context(src);
    assert_eq!(
        MarkdownParser::find_context_blockquotes(src, &ctx).count(),
        1
    );
    assert_eq!(MarkdownParser::find_blockquotes(src).count(), 2);
}

// 09. Unclosed fence covers to end of input
#[test]
fn test_09_unclosed_fence() {
    let src = b"**a**\n```\n**never** # nope - nope";
    let ctx = MarkdownParser::context(src);
    assert_eq!(
        spans(MarkdownParser::find_context_bolds(src, &ctx)),
        vec![(2, 3)],
    );
    assert_eq!(MarkdownParser::find_context_headings(src, &ctx).count(), 0);
}

// 10. Italic close candidate inside a code span is skipped; the true close
//     after the code span is found (full-parse shape)
#[test]
fn test_10_close_candidate_inside_code_span() {
    let src = b"*a `*` b*";
    let ctx = MarkdownParser::context(src);
    assert_eq!(
        spans(MarkdownParser::find_context_italics(src, &ctx)),
        vec![(1, 8)],
    );
}

// 11. Autolink contents are covered: emphasis-looking bytes in a URL
#[test]
fn test_11_autolink_covers_contents() {
    let src = b"<http://x/*y*z> and *real*";
    let ctx = MarkdownParser::context(src);
    assert_eq!(
        spans(MarkdownParser::find_context_italics(src, &ctx)),
        vec![(21, 25)],
    );
}

// 12. Context-aware finders agree with the full parse on a mixed document
//     where the context-free finders diverge
#[test]
fn test_12_agreement_with_full_parse() {
    let src = b"# t\n\npara **b** `c` *i*\n\n```\n**x** # h - l\n```\n\n> **q**\n";
    let ctx = MarkdownParser::context(src);
    let full = MarkdownParser::parse(src);

    let aware_bolds = spans(MarkdownParser::find_context_bolds(src, &ctx));
    let full_bolds: Vec<_> = full.bolds.iter().map(|s| (s.start, s.end)).collect();
    assert_eq!(aware_bolds, full_bolds);

    let aware_italics = spans(MarkdownParser::find_context_italics(src, &ctx));
    let full_italics: Vec<_> = full.italics.iter().map(|s| (s.start, s.end)).collect();
    assert_eq!(aware_italics, full_italics);

    assert_eq!(
        MarkdownParser::find_context_headings(src, &ctx).count(),
        full.headings.len(),
    );
    assert_eq!(
        MarkdownParser::find_context_bullet_items(src, &ctx).count(),
        full.bullet_items.len(),
    );
}

// ---- Agreement with the full parse on line structure ------------------- //

fn pairs<'a>(it: impl IntoIterator<Item = &'a Span>) -> Vec<(u32, u32)> {
    it.into_iter().map(|s| (s.start, s.end)).collect()
}

fn item_spans<T>(it: impl IntoIterator<Item = (T, Span)>) -> Vec<(u32, u32)> {
    it.into_iter().map(|(_, s)| (s.start, s.end)).collect()
}

/// Every context-aware finder agrees with the full parse on `src`.
fn assert_agrees(src: &[u8]) {
    let ctx = MarkdownParser::context(src);
    let full = MarkdownParser::parse(src);
    let text = String::from_utf8_lossy(src);
    assert_eq!(
        spans(MarkdownParser::find_context_italics(src, &ctx)),
        pairs(&full.italics),
        "italics in {text:?}"
    );
    assert_eq!(
        spans(MarkdownParser::find_context_bolds(src, &ctx)),
        pairs(&full.bolds),
        "bolds in {text:?}"
    );
    assert_eq!(
        spans(MarkdownParser::find_context_bold_italics(src, &ctx)),
        pairs(&full.bold_italics),
        "bold_italics in {text:?}"
    );
    assert_eq!(
        spans(MarkdownParser::find_context_blockquotes(src, &ctx)),
        pairs(&full.blockquotes),
        "blockquotes in {text:?}"
    );
    assert_eq!(
        item_spans(MarkdownParser::find_context_headings(src, &ctx)),
        item_spans(full.headings.iter().copied()),
        "headings in {text:?}"
    );
    assert_eq!(
        item_spans(MarkdownParser::find_context_thematic_breaks(src, &ctx)),
        item_spans(full.thematic_breaks.iter().copied()),
        "thematic_breaks in {text:?}"
    );
    assert_eq!(
        item_spans(MarkdownParser::find_context_bullet_items(src, &ctx)),
        item_spans(full.bullet_items.iter().copied()),
        "bullet_items in {text:?}"
    );
    assert_eq!(
        item_spans(MarkdownParser::find_context_ordered_items(src, &ctx)),
        item_spans(full.ordered_items.iter().copied()),
        "ordered_items in {text:?}"
    );
}

// 13. Headings and list items whose content opens with a code span are kept
#[test]
fn test_13_items_opening_with_code() {
    assert_agrees(b"## `fn parse()`\n");
    assert_agrees(b"- `--foo` enables foo\n");
    assert_agrees(b"1. `x` y\n");
}

// 14. A bullet's marker is no emphasis delimiter, and a bullet wins over a
//     thematic break made of the same bytes
#[test]
fn test_14_bullet_markers() {
    assert_agrees(b"* item with *emphasis*\n");
    assert_agrees(b"* * *\n- - -\n***\n");
}

// 15. A fenced block inside a blockquote is opaque
#[test]
fn test_15_fence_inside_blockquote() {
    assert_agrees(b"> ```\n> let p = *x * *y;\n> **not bold**\n> ```\n> *real*\n");
    assert_agrees(b"> ```\n> *x*\nplain *y*\n");
}

// 16. An unclosed backtick does not reach into the list item, heading or
//     blockquote on the next line
#[test]
fn test_16_unclosed_backtick_before_block_line() {
    assert_agrees(b"Options for `cmd:\n- `--foo` enables **foo**\n");
    assert_agrees(b"The `bar\n## Heading with `code`\n");
    assert_agrees(b"- run `make\n- then `make install`\n");
    assert_agrees(b"`x\n> y`\n> > z\n");
}

// 17. Emphasis does not pair across a line some rule claims, and on such a
//     line it stays on the line
#[test]
fn test_17_emphasis_runs() {
    assert_agrees(b"**a\n- b**\n");
    assert_agrees(b"*a\n# b*\n");
    assert_agrees(b"- **a\nb**\n");
    assert_agrees(b"**a `x\ny\nz` b**\n");
}

// 18. A mixed document
#[test]
fn test_18_mixed_document() {
    assert_agrees(
        b"# Title with `code`\n\
          \n\
          Intro *text* and **bold** with `ticks\n\
          - item `one`\n\
          - item *two*\n\
          \n\
          > quote **here**\n\
          > ```\n\
          > *code* in a quote\n\
          > ```\n\
          > > nested *quote*\n\
          \n\
          1. first\n\
          2) second `x`\n\
          \n\
          * * *\n\
          \n\
          ```\n\
          # not a heading\n\
          ```\n\
          ---\n",
    );
}

// 19. Blockquote frames come innermost-first, so each is looked up on its
//     own: a context whose region covers the outer frame's marker but ends
//     before the inner one drops the outer frame only
#[test]
fn test_19_blockquote_frames_any_order() {
    let src = b"`x\n> y`\n> > z\n";
    let ctx =
        meon::ParseContext::build(src, b'\\', meon::LineModel::default(), &[(b'`', 1)], &[], 0);
    assert_eq!(ctx.spans(), &[Span::new(0, 7)]);
    assert_eq!(
        spans(MarkdownParser::find_context_blockquotes(src, &ctx)),
        vec![(10, 14)],
    );
}
