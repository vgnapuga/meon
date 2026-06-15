use meon_md::MarkdownParser;

fn txt(src: &[u8], s: meon::span::Span) -> &str {
    std::str::from_utf8(&src[s.start as usize..s.end as usize]).unwrap()
}

// ================================================================
// paragraphs
// ================================================================

// 01. A single line forms a single paragraph
#[test]
fn integ_para_01_single() {
    let src = b"hello world\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.paragraphs.len(), 1);
}

// 02. Two text blocks separated by an empty line form two paragraphs
#[test]
fn integ_para_02_two_paragraphs() {
    let src = b"first\n\nsecond\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.paragraphs.len(), 2);
}

// 03. Three distinct text blocks form three separate paragraphs
#[test]
fn integ_para_03_three_paragraphs() {
    let src = b"one\n\ntwo\n\nthree\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.paragraphs.len(), 3);
}

// 04. Inline formatting elements like bold are resolved within a paragraph
#[test]
fn integ_para_04_with_inline() {
    let src = b"text **bold** text\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.paragraphs.len(), 1);
    assert_eq!(c.bolds.len(), 1);
}

// 05. A source containing only blank newlines produces no paragraphs
#[test]
fn integ_para_05_empty_lines_only() {
    let src = b"\n\n\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.paragraphs.len(), 0);
}

// ================================================================
// headings
// ================================================================

// 01. A single hash line produces a level 1 heading
#[test]
fn integ_heading_01_h1() {
    let src = b"# Hello\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.headings.len(), 1);
    assert_eq!(c.headings[0].0.level.get(), 1);
}

// 02. A double hash line produces a level 2 heading
#[test]
fn integ_heading_02_h2() {
    let src = b"## Hello\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.headings[0].0.level.get(), 2);
}

// 03. A six hash line produces a level 6 heading
#[test]
fn integ_heading_03_h6() {
    let src = b"###### Hello\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.headings[0].0.level.get(), 6);
}

// 04. Multiple sequential headings are parsed and collected correctly
#[test]
fn integ_heading_04_multiple() {
    let src = b"# H1\n## H2\n### H3\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.headings.len(), 3);
}

// 05. Headings successfully process nested inline styles like bold
#[test]
fn integ_heading_05_with_inline() {
    let src = b"# Title **bold**\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.headings.len(), 1);
    assert_eq!(c.bolds.len(), 1);
}

// 06. A heading followed by a paragraph parses both structures correctly
#[test]
fn integ_heading_06_then_paragraph() {
    let src = b"# Title\nSome text\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.headings.len(), 1);
    assert_eq!(c.paragraphs.len(), 1);
}

// ================================================================
// thematic breaks
// ================================================================

// 01. Three sequential dashes produce a thematic break
#[test]
fn integ_tb_01_dash() {
    let src = b"---\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.thematic_breaks.len(), 1);
}

// 02. Three sequential stars produce a thematic break
#[test]
fn integ_tb_02_star() {
    let src = b"***\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.thematic_breaks.len(), 1);
}

// 03. A heading followed directly by a thematic break is parsed cleanly
#[test]
fn integ_tb_03_heading_then_tb() {
    let src = b"# Title\n---\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.headings.len(), 1);
    assert_eq!(c.thematic_breaks.len(), 1);
}

// 04. A thematic break properly divides two regular paragraph blocks
#[test]
fn integ_tb_04_between_paragraphs() {
    let src = b"before\n\n---\n\nafter\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.thematic_breaks.len(), 1);
    assert_eq!(c.paragraphs.len(), 2);
}

// ================================================================
// bold / italic / bold_italic
// ================================================================

// 01. Double asterisks form a clean bold inline block
#[test]
fn integ_inline_01_bold() {
    let src = b"**bold**\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.bolds.len(), 1);
    assert_eq!(txt(src, c.bolds[0]), "bold");
}

// 02. Single asterisks form an italic inline block
#[test]
fn integ_inline_02_italic() {
    let src = b"*italic*\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.italics.len(), 1);
    assert_eq!(txt(src, c.italics[0]), "italic");
}

// 03. Triple asterisks parse as a bold italic token combination
#[test]
fn integ_inline_03_bold_italic() {
    let src = b"***bi***\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.bold_italics.len(), 1);
}

// 04. Bold and italic spans can reside simultaneously on one text line
#[test]
fn integ_inline_04_bold_and_italic() {
    let src = b"**bold** and *italic*\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.bolds.len(), 1);
    assert_eq!(c.italics.len(), 1);
}

// 05. Multiple independent bold spans are collected sequentially
#[test]
fn integ_inline_05_multiple_bolds() {
    let src = b"**a** **b** **c**\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.bolds.len(), 3);
}

// 06. Bold, italic, and code spans can coexist inside a single paragraph
#[test]
fn integ_inline_06_all_in_paragraph() {
    let src = b"Text **bold** and *italic* and `code`\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.bolds.len(), 1);
    assert_eq!(c.italics.len(), 1);
    assert_eq!(c.codes.len(), 1);
}

// ================================================================
// code spans
// ================================================================

// 01. Single backticks extract an inline code block segment
#[test]
fn integ_code_01_span() {
    let src = b"`code`\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.codes.len(), 1);
    assert_eq!(txt(src, c.codes[0]), "code");
}

// 02. Double backticks without proper matching or context yield no valid codes
#[test]
fn integ_code_02_double_backtick() {
    let src = b"``code``\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.codes.len(), 0);
}

// 03. Regular inline styling is treated as verbatim text inside backticks
#[test]
fn integ_code_03_verbatim_stars() {
    let src = b"`**not bold**`\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.codes.len(), 1);
    assert_eq!(c.bolds.len(), 0);
}

// ================================================================
// links and images
// ================================================================

// 01. Standard markdown syntax creates a valid hypertext link
#[test]
fn integ_link_01_standard() {
    let src = b"[text](url)\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.links.len(), 1);
    assert!(!c.links[0].is_image);
    assert_eq!(txt(src, c.links[0].text), "text");
    assert_eq!(txt(src, c.links[0].url), "url");
}

// 02. A leading exclamation mark correctly signifies an image element
#[test]
fn integ_link_02_image() {
    let src = b"![alt](url)\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.links.len(), 1);
    assert!(c.links[0].is_image);
}

// 03. A text stream can hold a standard link and an image link concurrently
#[test]
fn integ_link_03_link_and_image() {
    let src = b"[link](url1) ![img](url2)\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.links.len(), 2);
    assert!(!c.links[0].is_image);
    assert!(c.links[1].is_image);
}

// 04. Angle brackets extract an explicit URI as an autolink component
#[test]
fn integ_link_04_autolink() {
    let src = b"<https://example.com>\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.autolinks.len(), 1);
}

// 05. Link anchor description text can wrap internal inline elements like bold
#[test]
fn integ_link_05_with_bold_text() {
    let src = b"[**bold**](url)\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.links.len(), 1);
}

// ================================================================
// hard breaks
// ================================================================

// 01. Two trailing whitespaces at the end of a line trigger a hard break
#[test]
fn integ_hb_01_spaces() {
    let src = b"line  \n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.hard_breaks.len(), 1);
}

// 02. A trailing backslash character right before a newline triggers a hard break
#[test]
fn integ_hb_02_backslash() {
    let src = b"line\\\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.hard_breaks.len(), 1);
}

// ================================================================
// fenced code blocks
// ================================================================

// 01. Triple backticks enclose a block level fenced code block structure
#[test]
fn integ_fenced_01_backtick() {
    let src = b"```\ncode\n```\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.fenced_codes.len(), 1);
}

// 02. A language syntax identifier string can follow the opening fence
#[test]
fn integ_fenced_02_with_lang() {
    let src = b"```rust\nfn main() {}\n```\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.fenced_codes.len(), 1);
}

// 03. All inline markdown tags are ignored and remain verbatim inside block code
#[test]
fn integ_fenced_03_no_inline_inside() {
    let src = b"```\n**not bold**\n```\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.fenced_codes.len(), 1);
    assert_eq!(c.bolds.len(), 0);
}

// 04. Multiple independent fenced code structures parse safely sequentially
#[test]
fn integ_fenced_04_two_blocks() {
    let src = b"```\nfirst\n```\n\n```\nsecond\n```\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.fenced_codes.len(), 2);
}

// 05. A standard text paragraph followed directly by a fenced block parses well
#[test]
fn integ_fenced_05_paragraph_then_code() {
    let src = b"text\n\n```\ncode\n```\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.paragraphs.len(), 1);
    assert_eq!(c.fenced_codes.len(), 1);
}

// ================================================================
// blockquotes
// ================================================================

// 01. A standard right angle bracket prefix builds a blockquote wrapper
#[test]
fn integ_quote_01_single() {
    let src = b"> quote\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.blockquotes.len(), 1);
}

// 02. Sequential lines sharing blockquote markers are unified into one quote block
#[test]
fn integ_quote_02_multiline() {
    let src = b"> line one\n> line two\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.blockquotes.len(), 1);
}

// 03. Blockquote blocks correctly process and nest structural inlines like bold
#[test]
fn integ_quote_03_with_inline() {
    let src = b"> **bold** text\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.blockquotes.len(), 1);
    assert_eq!(c.bolds.len(), 1);
}

// 04. Two blockquotes separated by empty vertical space are treated as distinct
#[test]
fn integ_quote_04_two_blockquotes() {
    let src = b"> first\n\n> second\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.blockquotes.len(), 2);
}

// 05. A blockquote block transitioning into a standard paragraph element
#[test]
fn integ_quote_05_then_paragraph() {
    let src = b"> quote\n\nparagraph\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.blockquotes.len(), 1);
    assert_eq!(c.paragraphs.len(), 1);
}

// 06. A blockquote block followed immediately by a bullet item line switch
#[test]
fn integ_quote_06_then_bullet() {
    let src = b"> quote\n- bullet\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.blockquotes.len(), 1);
    assert_eq!(c.bullet_items.len(), 1);
}

// ================================================================
// bullet lists
// ================================================================

// 01. A single dash marker initializes a bullet list item node
#[test]
fn integ_bullet_01_single() {
    let src = b"- item\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.bullet_items.len(), 1);
}

// 02. Multiple distinct lines with bullet prefixes register individual items
#[test]
fn integ_bullet_02_three_bullets() {
    let src = b"- a\n- b\n- c\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.bullet_items.len(), 3);
}

// 03. An asterisk character acts as a valid bullet list marker variant
#[test]
fn integ_bullet_03_star() {
    let src = b"* item\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.bullet_items.len(), 1);
    assert_eq!(c.bullet_items[0].0.kind, b'*');
}

// 04. A plus sign character acts as a valid bullet list marker variant
#[test]
fn integ_bullet_04_plus() {
    let src = b"+ item\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.bullet_items.len(), 1);
    assert_eq!(c.bullet_items[0].0.kind, b'+');
}

// 05. Bullet item contents seamlessly handle embedded inlines like bold
#[test]
fn integ_bullet_05_with_inline() {
    let src = b"- **bold** item\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.bullet_items.len(), 1);
    assert_eq!(c.bolds.len(), 1);
}

// 06. Indented items are registered as nested levels in list hierarchy
#[test]
fn integ_bullet_06_nested() {
    let src = b"- parent\n  - child\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.bullet_items.len(), 2);
}

// 07. Multi-stage spacing layout creates deeply nested bullet items
#[test]
fn integ_bullet_07_deeply_nested_bullets() {
    let src = b"- a\n  - b\n    - c\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.bullet_items.len(), 3);
}

// ================================================================
// ordered lists
// ================================================================

// 01. A line starting with numbers and a period initiates an ordered item
#[test]
fn integ_ordered_01_single() {
    let src = b"1. item\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.ordered_items.len(), 1);
    assert_eq!(c.ordered_items[0].0.num, 1);
}

// 02. Multiple numeric list elements are accurately extracted in sequence
#[test]
fn integ_ordered_02_three_ordered() {
    let src = b"1. a\n2. b\n3. c\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.ordered_items.len(), 3);
}

// 03. A closing parenthesis marker forms an ordered item alternative design
#[test]
fn integ_ordered_03_paren() {
    let src = b"1) item\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.ordered_items.len(), 1);
    assert_eq!(c.ordered_items[0].0.kind, b')');
}

// 04. Ordered block items safely resolve inner inline modifications like bold
#[test]
fn integ_ordered_04_with_inline() {
    let src = b"1. **bold** item\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.ordered_items.len(), 1);
    assert_eq!(c.bolds.len(), 1);
}

// 05. Spaced lines inside numerical blocks set up nested child lists
#[test]
fn integ_ordered_05_nested() {
    let src = b"1. parent\n   1. child\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.ordered_items.len(), 2);
}

// ================================================================
// combinations
// ================================================================

// 01. Headings, text blocks, and fenced structures process layout properly together
#[test]
fn integ_combo_01_heading_paragraph_code() {
    let src = b"# Title\n\nSome text\n\n```\ncode\n```\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.headings.len(), 1);
    assert_eq!(c.paragraphs.len(), 1);
    assert_eq!(c.fenced_codes.len(), 1);
}

// 02. Verifies structural compilation over a complete composite markdown stream
#[test]
fn integ_combo_02_full_document_structure() {
    let src = b"# Title\n---\nParagraph\n> Quote\n- Bullet\n1. Ordered\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.headings.len(), 1);
    assert_eq!(c.thematic_breaks.len(), 1);
    assert_eq!(c.paragraphs.len(), 1);
    assert_eq!(c.blockquotes.len(), 1);
    assert_eq!(c.bullet_items.len(), 1);
    assert_eq!(c.ordered_items.len(), 1);
}

// 03. A blockquote context followed cleanly by an isolated block code structure
#[test]
fn integ_combo_03_blockquote_then_fenced_code() {
    let src = b"> quote\n> second\n```rust\ncode\n```\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.blockquotes.len(), 1);
    assert_eq!(c.fenced_codes.len(), 1);
}

// 04. A sequence changing instantly from bullet layouts into numerical lists
#[test]
fn integ_combo_04_bullets_then_ordered() {
    let src = b"- bullet\n- bullet2\n1. ordered\n2. ordered2\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.bullet_items.len(), 2);
    assert_eq!(c.ordered_items.len(), 2);
}

// 05. A heavy paragraph block containing all supported inline modifier layouts
#[test]
fn integ_combo_05_inline_all_types() {
    let src = b"**bold** *italic* ***bi*** `code` [link](url) ![img](url) <auto>\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.bolds.len(), 1);
    assert_eq!(c.italics.len(), 1);
    assert_eq!(c.bold_italics.len(), 1);
    assert_eq!(c.codes.len(), 1);
    assert_eq!(c.links.len(), 2);
    assert_eq!(c.autolinks.len(), 1);
}

// 06. A header structure built using all major active inline markers
#[test]
fn integ_combo_06_heading_with_all_inline() {
    let src = b"# **bold** *italic* `code`\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.headings.len(), 1);
    assert_eq!(c.bolds.len(), 1);
    assert_eq!(c.italics.len(), 1);
    assert_eq!(c.codes.len(), 1);
}

// 07. A blockquote environment text utilizing multiple distinct inline elements
#[test]
fn integ_combo_07_blockquote_with_all_inline() {
    let src = b"> **bold** *italic* `code` [link](url)\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.blockquotes.len(), 1);
    assert_eq!(c.bolds.len(), 1);
    assert_eq!(c.italics.len(), 1);
    assert_eq!(c.codes.len(), 1);
    assert_eq!(c.links.len(), 1);
}

// 08. An isolated bullet element housing all common types of inline syntax
#[test]
fn integ_combo_08_bullet_with_all_inline() {
    let src = b"- **bold** *italic* `code` [link](url)\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.bullet_items.len(), 1);
    assert_eq!(c.bolds.len(), 1);
    assert_eq!(c.italics.len(), 1);
    assert_eq!(c.codes.len(), 1);
    assert_eq!(c.links.len(), 1);
}

// 09. An ordered line block holding an exhaustive collection of inline variables
#[test]
fn integ_combo_09_ordered_with_all_inline() {
    let src = b"1. **bold** *italic* `code` [link](url)\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.ordered_items.len(), 1);
    assert_eq!(c.bolds.len(), 1);
    assert_eq!(c.italics.len(), 1);
    assert_eq!(c.codes.len(), 1);
    assert_eq!(c.links.len(), 1);
}

// 10. Parses an intense multi-line complex document section smoothly
#[test]
fn integ_combo_10_doc_heavy_chunk() {
    let src = b"# Title 0\n---\nText with **bold** and *italic* [text](url).\n> Blockquote with **bold**.\n> Second line.\n```rust\ncode\n```\n- Bullet **bold**\n- Second bullet\n- Third bullet\n1. Ordered\n2. Second\n\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.headings.len(), 1);
    assert_eq!(c.thematic_breaks.len(), 1);
    assert_eq!(c.blockquotes.len(), 1);
    assert_eq!(c.fenced_codes.len(), 1);
    assert_eq!(c.bullet_items.len(), 3);
    assert_eq!(c.ordered_items.len(), 2);
}

// 11. Alternating structures between headers and normal text segments
#[test]
fn integ_combo_11_repeated_headings_paragraphs() {
    let src = b"# H1\npara1\n\n## H2\npara2\n\n### H3\npara3\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.headings.len(), 3);
    assert_eq!(c.paragraphs.len(), 3);
}

// 12. Multiple list bullet kinds mixed alongside structured numerical inputs
#[test]
fn integ_combo_12_mixed_lists() {
    let src = b"- bullet\n* bullet2\n+ bullet3\n1. ordered\n2. ordered2\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.bullet_items.len(), 3);
    assert_eq!(c.ordered_items.len(), 2);
}

// 13. Verifies that escaped characters neutralize italic markers in text flow
#[test]
fn integ_combo_13_escape_in_paragraph() {
    let src = b"\\*not italic\\*\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.italics.len(), 0);
    assert_eq!(c.paragraphs.len(), 1);
}

// 14. An escaped hash token suppresses heading creation on text lines
#[test]
fn integ_combo_14_escape_heading() {
    let src = b"\\# not heading\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.headings.len(), 0);
    assert_eq!(c.paragraphs.len(), 1);
}

// 15. A code block layout positioned cleanly immediately below a bullet listing
#[test]
fn integ_combo_15_fenced_code_after_bullets() {
    let src = b"- item1\n- item2\n```\ncode\n```\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.bullet_items.len(), 2);
    assert_eq!(c.fenced_codes.len(), 1);
}

// 16. A block level code enclosure generated straight after quote boundaries
#[test]
fn integ_combo_16_fenced_code_after_blockquote() {
    let src = b"> quote\n```\ncode\n```\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.blockquotes.len(), 1);
    assert_eq!(c.fenced_codes.len(), 1);
}

// 17. Distinct quote segments containing bold and italic decorations individually
#[test]
fn integ_combo_17_multiple_blockquotes_with_inline() {
    let src = b"> **bold** quote\n\n> *italic* quote\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.blockquotes.len(), 2);
    assert_eq!(c.bolds.len(), 1);
    assert_eq!(c.italics.len(), 1);
}

// 18. A continuous block paragraph reading across multiple line divisions
#[test]
fn integ_combo_18_paragraph_multiline() {
    let src = b"line one\nline two\nline three\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.paragraphs.len(), 1);
}

// 19. Multiline paragraph strings carrying centered inline highlights like bold
#[test]
fn integ_combo_19_paragraph_multiline_with_inline() {
    let src = b"line one\nline **bold** two\nline three\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.paragraphs.len(), 1);
    assert_eq!(c.bolds.len(), 1);
}

// 20. Ordered lists transitioning properly immediately below active headers
#[test]
fn integ_combo_20_ordered_after_heading() {
    let src = b"# Title\n1. first\n2. second\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.headings.len(), 1);
    assert_eq!(c.ordered_items.len(), 2);
}

// 21. Bullet points initiating structure safely straight following headers
#[test]
fn integ_combo_21_bullet_after_heading() {
    let src = b"# Title\n- first\n- second\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.headings.len(), 1);
    assert_eq!(c.bullet_items.len(), 2);
}

// 22. A hypertext reference anchor encapsulated in a flat list line structure
#[test]
fn integ_combo_22_link_in_bullet() {
    let src = b"- [link](url) text\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.bullet_items.len(), 1);
    assert_eq!(c.links.len(), 1);
}

// 23. An inline graphic reference parsed cleanly inside a quote component block
#[test]
fn integ_combo_23_image_in_blockquote() {
    let src = b"> ![alt](url)\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.blockquotes.len(), 1);
    assert_eq!(c.links.len(), 1);
    assert!(c.links[0].is_image);
}

// 24. A clean automated web address macro nested safely inside list items
#[test]
fn integ_combo_24_autolink_in_bullet() {
    let src = b"- <https://example.com>\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.bullet_items.len(), 1);
    assert_eq!(c.autolinks.len(), 1);
}

// 25. An inline code segment located right inside a header block title
#[test]
fn integ_combo_25_code_in_heading() {
    let src = b"# Title `code`\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.headings.len(), 1);
    assert_eq!(c.codes.len(), 1);
}

// 26. Structural span offsets obey rigorous alignment boundaries against total source size
#[test]
fn integ_combo_26_spans_valid_bounds() {
    let src = b"# Title\n**bold** *italic* `code` [link](url)\n> quote\n- bullet\n1. ordered\n```\ncode\n```\n";
    let c = MarkdownParser::parse(src);
    let len = src.len() as u32;
    for s in &c.texts {
        assert!(s.start <= s.end && s.end <= len);
    }
    for s in &c.bolds {
        assert!(s.start <= s.end && s.end <= len);
    }
    for s in &c.italics {
        assert!(s.start <= s.end && s.end <= len);
    }
    for s in &c.codes {
        assert!(s.start <= s.end && s.end <= len);
    }
    for l in &c.links {
        assert!(l.text.start <= l.text.end && l.text.end <= len);
        assert!(l.url.start <= l.url.end && l.url.end <= len);
    }
    for s in &c.paragraphs {
        assert!(s.start <= s.end && s.end <= len);
    }
    for s in &c.blockquotes {
        assert!(s.start <= s.end && s.end <= len);
    }
    for s in &c.fenced_codes {
        assert!(s.start <= s.end && s.end <= len);
    }
    for (_, s) in &c.headings {
        assert!(s.start <= s.end && s.end <= len);
    }
    for (_, s) in &c.bullet_items {
        assert!(s.start <= s.end && s.end <= len);
    }
    for (_, s) in &c.ordered_items {
        assert!(s.start <= s.end && s.end <= len);
    }
}

// 27. An empty document raw text scenario produces zero elements across allocations
#[test]
fn integ_combo_27_empty_source() {
    let src = b"";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.paragraphs.len(), 0);
    assert_eq!(c.headings.len(), 0);
    assert_eq!(c.texts.len(), 0);
}

// 28. A single isolated trailing newline creates no dangling block components
#[test]
fn integ_combo_28_single_newline() {
    let src = b"\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.paragraphs.len(), 0);
}

// 29. A plain input without a trailing newline successfully resolves to a paragraph
#[test]
fn integ_combo_29_no_trailing_newline() {
    let src = b"hello";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.paragraphs.len(), 1);
}

// 30. A markdown header structure missing a trailing newline executes cleanly
#[test]
fn integ_combo_30_heading_no_trailing_newline() {
    let src = b"# Title";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.headings.len(), 1);
}

// 31. An open fenced structure terminating abruptly safely wraps block generation
#[test]
fn integ_combo_31_fenced_code_no_trailing_newline() {
    let src = b"```\ncode\n```";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.fenced_codes.len(), 1);
}

// 32. Multi-paragraph environments track bold sections relative to native block offsets
#[test]
fn integ_combo_32_bold_across_multiple_paragraphs() {
    let src = b"**bold1**\n\n**bold2**\n\n**bold3**\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.bolds.len(), 3);
    assert_eq!(c.paragraphs.len(), 3);
}

// 33. Validates a diverse mixture of rich markdown inline items on complex segments
#[test]
fn integ_combo_33_doc_hot_chunk() {
    let src = b"## Section\nPlain text with **bold** and *italic* words.\nAnother line with [text](url) and `code` and <link>.\n\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.headings.len(), 1);
    assert_eq!(c.bolds.len(), 1);
    assert_eq!(c.italics.len(), 1);
    assert_eq!(c.links.len(), 1);
    assert_eq!(c.codes.len(), 1);
    assert_eq!(c.autolinks.len(), 1);
}

// 34. A horizontal line separator instantly leading into structural quote text
#[test]
fn integ_combo_34_thematic_break_then_blockquote() {
    let src = b"---\n> quote\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.thematic_breaks.len(), 1);
    assert_eq!(c.blockquotes.len(), 1);
}

// 35. A thematic break followed immediately by a standard list bullet node
#[test]
fn integ_combo_35_thematic_break_then_bullet() {
    let src = b"---\n- item\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.thematic_breaks.len(), 1);
    assert_eq!(c.bullet_items.len(), 1);
}

// 36. A thematic break followed immediately by an ordered item sequence line
#[test]
fn integ_combo_36_thematic_break_then_ordered() {
    let src = b"---\n1. item\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.thematic_breaks.len(), 1);
    assert_eq!(c.ordered_items.len(), 1);
}

// 37. A multiline blockquote containing a dense pack of varying inline syntax markers
#[test]
fn integ_combo_37_blockquote_multiline_with_inline() {
    let src = b"> **bold** line\n> *italic* line\n> `code` line\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.blockquotes.len(), 1);
    assert_eq!(c.bolds.len(), 1);
    assert_eq!(c.italics.len(), 1);
    assert_eq!(c.codes.len(), 1);
}

// 38. Numeric list markers parsing integers composed of multiple digits correctly
#[test]
fn integ_combo_38_ordered_multidigit() {
    let src = b"10. item\n11. item\n12. item\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.ordered_items.len(), 3);
    assert_eq!(c.ordered_items[0].0.num, 10);
}

// 39. Paragraph layout checking balanced tracking of mixed inline text sections
#[test]
fn integ_combo_39_mixed_inline_in_paragraph() {
    let src = b"Text **bold** more *italic* and `code` end\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.paragraphs.len(), 1);
    assert_eq!(c.bolds.len(), 1);
    assert_eq!(c.italics.len(), 1);
    assert_eq!(c.codes.len(), 1);
    assert!(!c.texts.is_empty());
}

// 40. A structural fenced block sitting inside separate outer bullet sections
#[test]
fn integ_combo_40_fenced_code_between_lists() {
    let src = b"- before\n```\ncode\n```\n- after\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.fenced_codes.len(), 1);
    assert_eq!(c.bullet_items.len(), 2);
}

// 41. Sequential loop tracking verifying matching header level numbers from 1 to 6
#[test]
fn integ_combo_41_heading_counts() {
    let src = b"# H1\n## H2\n### H3\n#### H4\n##### H5\n###### H6\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.headings.len(), 6);
    for (i, (h, _)) in c.headings.iter().enumerate() {
        assert_eq!(h.level.get() as usize, i + 1);
    }
}

// ================================================================
// additional edge cases
// ================================================================

// 01. A multiline paragraph block including an active internal hard break signature
#[test]
fn integ_edge_01_hard_break_in_multiline_paragraph() {
    let src = b"line one  \nline two\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.paragraphs.len(), 1);
    assert_eq!(c.hard_breaks.len(), 1);
}

// 02. An autolink element verified cleanly inside standard paragraph text flows
#[test]
fn integ_edge_02_autolink_in_paragraph() {
    let src = b"Visit <https://example.com> today\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.paragraphs.len(), 1);
    assert_eq!(c.autolinks.len(), 1);
}

// 03. An image link asset reference identified correctly inside a text paragraph
#[test]
fn integ_edge_03_image_in_paragraph() {
    let src = b"See ![alt](url) here\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.paragraphs.len(), 1);
    assert_eq!(c.links.len(), 1);
    assert!(c.links[0].is_image);
}

// 04. An active list component separated cleanly from trailing independent text paragraphs
#[test]
fn integ_edge_04_list_followed_by_paragraph() {
    let src = b"- item\n\nparagraph\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.bullet_items.len(), 1);
    assert_eq!(c.paragraphs.len(), 1);
}

// 05. Transition context cleanly maintaining block status from ordered directly to bullets
#[test]
fn integ_edge_05_ordered_followed_by_bullet() {
    let src = b"1. first\n- second\n";
    let c = MarkdownParser::parse(src);
    assert_eq!(c.ordered_items.len(), 1);
    assert_eq!(c.bullet_items.len(), 1);
}
