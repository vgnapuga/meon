use super::common::*;

// ================================================================
// texts
// ================================================================

// 01. Empty source produces no text spans
#[test]
fn text_01_empty_src() {
    let src = b"";
    let (st, _) = run_inline!(src);
    assert!(st.texts.is_empty());
}

// 02. A plain word produces a single text span covering the whole input
#[test]
fn text_02_plain_single_word() {
    let src = b"hello";
    let (st, _) = run_inline!(src);
    assert_eq!(st.texts.len(), 1);
    assert_eq!(txt(src, st.texts[0]), "hello");
}

// 03. Text span starts at byte offset zero
#[test]
fn text_03_span_start_zero() {
    let src = b"abc";
    let (st, _) = run_inline!(src);
    assert_eq!(st.texts[0].start, 0);
}

// 04. Text span end equals the source length
#[test]
fn text_04_span_end_equals_len() {
    let src = b"abc";
    let (st, _) = run_inline!(src);
    assert_eq!(st.texts[0].end, 3);
}

// 05. Text before a bold span is captured separately
#[test]
fn text_05_before_bold() {
    let src = b"hi **b**";
    let (st, _) = run_inline!(src);
    assert_eq!(txt(src, st.texts[0]), "hi ");
}

// 06. Text after a bold span is captured separately
#[test]
fn text_06_after_bold() {
    let src = b"**b** end";
    let (st, _) = run_inline!(src);
    assert_eq!(txt(src, st.texts[0]), " end");
}

// 07. Text between two bold spans is captured as a single span
#[test]
fn text_07_between_two_bolds() {
    let src = b"**a** mid **b**";
    let (st, _) = run_inline!(src);
    assert_eq!(st.texts.len(), 1);
    assert_eq!(txt(src, st.texts[0]), " mid ");
}

// 08. An unclosed star is treated as plain text
#[test]
fn text_08_unclosed_star_included() {
    let src = b"*no close";
    let (st, _) = run_inline!(src);
    assert_eq!(st.texts.len(), 1);
    assert!(txt(src, st.texts[0]).contains('*'));
}

// 09. Input with no delimiters produces exactly one text span
#[test]
fn text_09_no_delimiter_single_span() {
    let src = b"just words here";
    let (st, _) = run_inline!(src);
    assert_eq!(st.texts.len(), 1);
    assert_eq!(txt(src, st.texts[0]), "just words here");
}

// 10. All text spans satisfy start <= end
#[test]
fn text_10_all_spans_start_le_end() {
    let src = b"a **b** c *d* `e`";
    let (st, _) = run_inline!(src);
    for s in &st.texts {
        assert!(s.start <= s.end);
    }
}

// 11. The consumed byte count equals the source length
#[test]
fn text_11_consumed_equals_len() {
    let src = b"hello";
    let (_, consumed) = run_inline!(src);
    assert_eq!(consumed, src.len());
}

// 12. Text before an italic span is captured separately
#[test]
fn text_12_before_italic() {
    let src = b"prefix *i*";
    let (st, _) = run_inline!(src);
    assert_eq!(txt(src, st.texts[0]), "prefix ");
}

// 13. Text after an italic span is captured separately
#[test]
fn text_13_after_italic() {
    let src = b"*i* suffix";
    let (st, _) = run_inline!(src);
    assert_eq!(txt(src, st.texts[0]), " suffix");
}

// 14. Text before a code span is captured separately
#[test]
fn text_14_before_code() {
    let src = b"pre `c`";
    let (st, _) = run_inline!(src);
    assert_eq!(txt(src, st.texts[0]), "pre ");
}

// 15. Text after a code span is captured separately
#[test]
fn text_15_after_code() {
    let src = b"`c` post";
    let (st, _) = run_inline!(src);
    assert_eq!(txt(src, st.texts[0]), " post");
}

// 16. Text on both sides of a bold span produces exactly two text spans
#[test]
fn text_16_two_spans_around_bold() {
    let src = b"a **x** b";
    let (st, _) = run_inline!(src);
    assert_eq!(st.texts.len(), 2);
    assert_eq!(txt(src, st.texts[0]), "a ");
    assert_eq!(txt(src, st.texts[1]), " b");
}

// 17. An unclosed backtick is treated as plain text
#[test]
fn text_17_unclosed_backtick_is_text() {
    let src = b"`unclosed";
    let (st, _) = run_inline!(src);
    assert_eq!(st.texts.len(), 1);
    assert!(txt(src, st.texts[0]).contains('`'));
}

// 18. A lone star between words does not produce an italic span
#[test]
fn text_18_star_between_words_no_italic() {
    let src = b"a * b";
    let (st, _) = run_inline!(src);
    assert!(st.italics.is_empty());
    let all: String = st.texts.iter().map(|&s| txt(src, s)).collect();
    assert!(all.contains('*'));
}

// 19. Numbers and punctuation pass through as plain text
#[test]
fn text_19_numbers_and_punctuation() {
    let src = b"abc 123 !@#$%^&()";
    let (st, _) = run_inline!(src);
    assert_eq!(st.texts.len(), 1);
    assert_eq!(txt(src, st.texts[0]), "abc 123 !@#$%^&()");
}

// 20. A fully bold input produces no text spans
#[test]
fn text_20_no_text_when_fully_bold() {
    let src = b"**bold**";
    let (st, _) = run_inline!(src);
    assert!(st.texts.is_empty());
}

// 21. A fully code input produces no text spans
#[test]
fn text_21_no_text_when_fully_code() {
    let src = b"`code`";
    let (st, _) = run_inline!(src);
    assert!(st.texts.is_empty());
}

// 22. A pending symmetric open closes on matching count from the left
#[test]
fn text_22_star_closes_pending_with_same_count() {
    let src = b"*a *b";
    let (st, _) = run_inline!(src);
    assert_eq!(st.italics.len(), 1);
    assert_eq!(txt(src, st.italics[0]), "a ");
}

// 23. Hard-break trailing spaces are excluded from the text span
#[test]
fn text_23_hard_break_excludes_trailing_spaces() {
    let src = b"word  ";
    let (st, _) = run_inline!(src);
    assert_eq!(txt(src, st.texts[0]), "word");
}

// 24. Hard-break backslash is excluded from the text span
#[test]
fn text_24_hard_break_backslash_excludes_backslash() {
    let src = b"word\\";
    let (st, _) = run_inline!(src);
    assert_eq!(txt(src, st.texts[0]), "word");
}

// 25. Every text span contains valid UTF-8
#[test]
fn text_25_spans_valid_utf8() {
    let src = b"hello world";
    let (st, _) = run_inline!(src);
    for s in &st.texts {
        assert!(std::str::from_utf8(&src[s.start as usize..s.end as usize]).is_ok());
    }
}

// ================================================================
// bold
// ================================================================

// 01. Basic double-star bold produces one span with the inner content
#[test]
fn bold_01_basic() {
    let src = b"**bold**";
    let (st, _) = run_inline!(src);
    assert_eq!(st.bolds.len(), 1);
    assert_eq!(txt(src, st.bolds[0]), "bold");
}

// 02. Bold span excludes the surrounding marker bytes
#[test]
fn bold_02_span_excludes_markers() {
    let src = b"**x**";
    let (st, _) = run_inline!(src);
    assert_eq!(
        &src[st.bolds[0].start as usize..st.bolds[0].end as usize],
        b"x"
    );
}

// 03. Bold span start points to the first content byte
#[test]
fn bold_03_span_start_points_to_content() {
    let src = b"**hi**";
    let (st, _) = run_inline!(src);
    assert_eq!(st.bolds[0].start, 2);
}

// 04. Bold span end points one past the last content byte
#[test]
fn bold_04_span_end_points_to_content_end() {
    let src = b"**hi**";
    let (st, _) = run_inline!(src);
    assert_eq!(st.bolds[0].end, 4);
}

// 05. Two adjacent bold spans are captured independently
#[test]
fn bold_05_two_on_same_input() {
    let src = b"**a** **b**";
    let (st, _) = run_inline!(src);
    assert_eq!(st.bolds.len(), 2);
    assert_eq!(txt(src, st.bolds[0]), "a");
    assert_eq!(txt(src, st.bolds[1]), "b");
}

// 06. Three consecutive bold spans are all captured
#[test]
fn bold_06_three_consecutive() {
    let src = b"**a** **b** **c**";
    let (st, _) = run_inline!(src);
    assert_eq!(st.bolds.len(), 3);
}

// 07. An unclosed double-star produces no bold span
#[test]
fn bold_07_unclosed_no_bold() {
    let src = b"**unclosed";
    let (st, _) = run_inline!(src);
    assert!(st.bolds.is_empty());
}

// 08. A mismatched single-star close produces no bold span
#[test]
fn bold_08_mismatched_one_star_close_no_bold() {
    let src = b"**word*";
    let (st, _) = run_inline!(src);
    assert!(st.bolds.is_empty());
}

// 09. Triple-star is bold-italic, not bold
#[test]
fn bold_09_three_stars_is_bold_italic_not_bold() {
    let src = b"***x***";
    let (st, _) = run_inline!(src);
    assert!(st.bolds.is_empty());
    assert_eq!(st.bold_italics.len(), 1);
}

// 10. Multi-word bold content is captured in full
#[test]
fn bold_10_multiword() {
    let src = b"**hello world**";
    let (st, _) = run_inline!(src);
    assert_eq!(txt(src, st.bolds[0]), "hello world");
}

// 11. Bold span satisfies start <= end
#[test]
fn bold_11_start_le_end() {
    let src = b"**x**";
    let (st, _) = run_inline!(src);
    assert!(st.bolds[0].start <= st.bolds[0].end);
}

// 12. A pending bold open closes on the first matching count from the left
#[test]
fn bold_12_pending_closes_on_same_count() {
    let src = b"**a **b**";
    let (st, _) = run_inline!(src);
    assert_eq!(st.bolds.len(), 1);
    assert_eq!(txt(src, st.bolds[0]), "a ");
}

// 13. Plain text produces no bold spans
#[test]
fn bold_13_no_bold_plain_text() {
    let src = b"just text";
    let (st, _) = run_inline!(src);
    assert!(st.bolds.is_empty());
}

// 14. Bold and italic can appear adjacently without interfering
#[test]
fn bold_14_adjacent_to_italic() {
    let src = b"**b** *i*";
    let (st, _) = run_inline!(src);
    assert_eq!(st.bolds.len(), 1);
    assert_eq!(st.italics.len(), 1);
}

// 15. Bold and code can appear adjacently without interfering
#[test]
fn bold_15_adjacent_to_code() {
    let src = b"**b** `c`";
    let (st, _) = run_inline!(src);
    assert_eq!(st.bolds.len(), 1);
    assert_eq!(st.codes.len(), 1);
}

// 16. Four stars produce bold-italic, not bold
#[test]
fn bold_16_four_stars_is_bold_italic() {
    let src = b"****x****";
    let (st, _) = run_inline!(src);
    assert!(st.bolds.is_empty());
    assert_eq!(st.bold_italics.len(), 1);
}

// 17. An escaped opening star suppresses bold recognition
#[test]
fn bold_17_escaped_open_no_bold() {
    let src = br"\**not bold**";
    let (st, _) = run_inline!(src);
    assert!(st.bolds.is_empty());
}

// ================================================================
// italic
// ================================================================

// 01. Basic single-star italic produces one span with the inner content
#[test]
fn italic_01_basic() {
    let src = b"*italic*";
    let (st, _) = run_inline!(src);
    assert_eq!(st.italics.len(), 1);
    assert_eq!(txt(src, st.italics[0]), "italic");
}

// 02. Italic span excludes the surrounding marker bytes
#[test]
fn italic_02_span_excludes_markers() {
    let src = b"*x*";
    let (st, _) = run_inline!(src);
    assert_eq!(
        &src[st.italics[0].start as usize..st.italics[0].end as usize],
        b"x"
    );
}

// 03. Italic span start points to the first content byte
#[test]
fn italic_03_span_start_points_to_content() {
    let src = b"*hi*";
    let (st, _) = run_inline!(src);
    assert_eq!(st.italics[0].start, 1);
}

// 04. Italic span end points one past the last content byte
#[test]
fn italic_04_span_end_points_to_content_end() {
    let src = b"*hi*";
    let (st, _) = run_inline!(src);
    assert_eq!(st.italics[0].end, 3);
}

// 05. An unclosed single-star produces no italic span
#[test]
fn italic_05_unclosed_no_italic() {
    let src = b"*unclosed";
    let (st, _) = run_inline!(src);
    assert!(st.italics.is_empty());
}

// 06. Two adjacent italic spans are captured independently
#[test]
fn italic_06_two_on_same_input() {
    let src = b"*a* *b*";
    let (st, _) = run_inline!(src);
    assert_eq!(st.italics.len(), 2);
    assert_eq!(txt(src, st.italics[0]), "a");
    assert_eq!(txt(src, st.italics[1]), "b");
}

// 07. Three consecutive italic spans are all captured
#[test]
fn italic_07_three_consecutive() {
    let src = b"*a* *b* *c*";
    let (st, _) = run_inline!(src);
    assert_eq!(st.italics.len(), 3);
}

// 08. A mismatched double-star close produces no italic span
#[test]
fn italic_08_mismatched_two_stars_close_no_italic() {
    let src = b"*word**";
    let (st, _) = run_inline!(src);
    assert!(st.italics.is_empty());
}

// 09. A pending italic open closes on the first matching count from the left
#[test]
fn italic_09_pending_closes_on_same_count() {
    let src = b"*a *b*";
    let (st, _) = run_inline!(src);
    assert_eq!(st.italics.len(), 1);
    assert_eq!(txt(src, st.italics[0]), "a ");
}

// 10. Italic span satisfies start <= end
#[test]
fn italic_10_start_le_end() {
    let src = b"*x*";
    let (st, _) = run_inline!(src);
    assert!(st.italics[0].start <= st.italics[0].end);
}

// 11. Plain text produces no italic spans
#[test]
fn italic_11_no_italic_plain_text() {
    let src = b"plain text";
    let (st, _) = run_inline!(src);
    assert!(st.italics.is_empty());
}

// 12. Italic and bold can appear adjacently without interfering
#[test]
fn italic_12_adjacent_to_bold() {
    let src = b"*i* **b**";
    let (st, _) = run_inline!(src);
    assert_eq!(st.italics.len(), 1);
    assert_eq!(st.bolds.len(), 1);
}

// 13. Triple-star is bold-italic, not italic
#[test]
fn italic_13_three_stars_is_bold_italic() {
    let src = b"***x***";
    let (st, _) = run_inline!(src);
    assert!(st.italics.is_empty());
    assert_eq!(st.bold_italics.len(), 1);
}

// 14. An escaped opening star suppresses italic recognition
#[test]
fn italic_14_escaped_open_no_italic() {
    let src = br"\*not italic\*";
    let (st, _) = run_inline!(src);
    assert!(st.italics.is_empty());
}

// 15. Multi-word italic content is captured in full
#[test]
fn italic_15_multiword() {
    let src = b"*hello world*";
    let (st, _) = run_inline!(src);
    assert_eq!(txt(src, st.italics[0]), "hello world");
}

// ================================================================
// bold_italic
// ================================================================

// 01. Basic triple-star bold-italic produces one span with the inner content
#[test]
fn bold_italic_01_basic() {
    let src = b"***bi***";
    let (st, _) = run_inline!(src);
    assert_eq!(st.bold_italics.len(), 1);
    assert_eq!(txt(src, st.bold_italics[0]), "bi");
}

// 02. Bold-italic span excludes the surrounding marker bytes
#[test]
fn bold_italic_02_span_excludes_markers() {
    let src = b"***x***";
    let (st, _) = run_inline!(src);
    assert_eq!(txt(src, st.bold_italics[0]), "x");
}

// 03. Four stars also produce bold-italic
#[test]
fn bold_italic_03_four_stars() {
    let src = b"****x****";
    let (st, _) = run_inline!(src);
    assert_eq!(st.bold_italics.len(), 1);
    assert_eq!(txt(src, st.bold_italics[0]), "x");
}

// 04. Five stars produce bold-italic
#[test]
fn bold_italic_04_five_stars() {
    let src = b"*****x*****";
    let (st, _) = run_inline!(src);
    assert_eq!(st.bold_italics.len(), 1);
}

// 05. An unclosed triple-star produces no bold-italic span
#[test]
fn bold_italic_05_unclosed_no_span() {
    let src = b"***unclosed";
    let (st, _) = run_inline!(src);
    assert!(st.bold_italics.is_empty());
}

// 06. Two adjacent bold-italic spans are captured independently
#[test]
fn bold_italic_06_two_on_same_input() {
    let src = b"***a*** ***b***";
    let (st, _) = run_inline!(src);
    assert_eq!(st.bold_italics.len(), 2);
    assert_eq!(txt(src, st.bold_italics[0]), "a");
    assert_eq!(txt(src, st.bold_italics[1]), "b");
}

// 07. A pending bold-italic open closes on the first matching count
#[test]
fn bold_italic_07_pending_closes_on_same_count() {
    let src = b"***a ***b***";
    let (st, _) = run_inline!(src);
    assert_eq!(st.bold_italics.len(), 1);
    assert_eq!(txt(src, st.bold_italics[0]), "a ");
}

// 08. Bold-italic span satisfies start <= end
#[test]
fn bold_italic_08_start_le_end() {
    let src = b"***x***";
    let (st, _) = run_inline!(src);
    assert!(st.bold_italics[0].start <= st.bold_italics[0].end);
}

// 09. Bold-italic, bold, and italic can coexist without interfering
#[test]
fn bold_italic_09_mixed_with_bold_and_italic() {
    let src = b"***bi*** **b** *i*";
    let (st, _) = run_inline!(src);
    assert_eq!(st.bold_italics.len(), 1);
    assert_eq!(st.bolds.len(), 1);
    assert_eq!(st.italics.len(), 1);
}

// 10. A mismatched double-star close produces no bold-italic span
#[test]
fn bold_italic_10_mismatched_close_no_span() {
    let src = b"***word**";
    let (st, _) = run_inline!(src);
    assert!(st.bold_italics.is_empty());
}

// ================================================================
// code
// ================================================================

// 01. A single backtick pair produces one code span
#[test]
fn code_01_single_backtick() {
    let src = b"`code`";
    let (st, _) = run_inline!(src);
    assert_eq!(st.codes.len(), 1);
    assert_eq!(txt(src, st.codes[0]), "code");
}

// 02. A double backtick pair produces one code span
#[test]
fn code_02_double_backtick() {
    let src = b"``code``";
    let (st, _) = run_inline!(src);
    assert_eq!(st.codes.len(), 1);
    assert_eq!(txt(src, st.codes[0]), "code");
}

// 03. A triple backtick inline pair produces one code span
#[test]
fn code_03_triple_backtick_inline() {
    let src = b"```code```";
    let (st, _) = run_inline!(src);
    assert_eq!(st.codes.len(), 1);
    assert_eq!(txt(src, st.codes[0]), "code");
}

// 04. Code span excludes the surrounding backtick markers
#[test]
fn code_04_span_excludes_backticks() {
    let src = b"`x`";
    let (st, _) = run_inline!(src);
    assert_eq!(
        &src[st.codes[0].start as usize..st.codes[0].end as usize],
        b"x"
    );
}

// 05. Code span start points to the first content byte
#[test]
fn code_05_span_start_points_to_content() {
    let src = b"`hi`";
    let (st, _) = run_inline!(src);
    assert_eq!(st.codes[0].start, 1);
}

// 06. Code span end points one past the last content byte
#[test]
fn code_06_span_end_points_to_content_end() {
    let src = b"`hi`";
    let (st, _) = run_inline!(src);
    assert_eq!(st.codes[0].end, 3);
}

// 07. Stars inside a code span are not parsed as emphasis
#[test]
fn code_07_verbatim_stars_inside() {
    let src = b"`**not bold**`";
    let (st, _) = run_inline!(src);
    assert!(st.bolds.is_empty());
    assert_eq!(txt(src, st.codes[0]), "**not bold**");
}

// 08. A single backtick inside double backticks is verbatim content
#[test]
fn code_08_verbatim_single_backtick_in_double() {
    let src = b"`` a`b ``";
    let (st, _) = run_inline!(src);
    assert_eq!(st.codes.len(), 1);
    assert_eq!(txt(src, st.codes[0]), " a`b ");
}

// 09. An unclosed backtick produces no code span
#[test]
fn code_09_unclosed_no_span() {
    let src = b"`unclosed";
    let (st, _) = run_inline!(src);
    assert!(st.codes.is_empty());
}

// 10. Mismatched backtick count produces no code span
#[test]
fn code_10_mismatched_count_no_span() {
    let src = b"``no close`";
    let (st, _) = run_inline!(src);
    assert!(st.codes.is_empty());
}

// 11. Two code spans on the same line are captured independently
#[test]
fn code_11_two_on_same_input() {
    let src = b"`a` `b`";
    let (st, _) = run_inline!(src);
    assert_eq!(st.codes.len(), 2);
    assert_eq!(txt(src, st.codes[0]), "a");
    assert_eq!(txt(src, st.codes[1]), "b");
}

// 12. Code and bold can appear adjacently without interfering
#[test]
fn code_12_after_bold() {
    let src = b"**b** `c`";
    let (st, _) = run_inline!(src);
    assert_eq!(st.bolds.len(), 1);
    assert_eq!(st.codes.len(), 1);
}

// 13. Code span satisfies start <= end
#[test]
fn code_13_start_le_end() {
    let src = b"`x`";
    let (st, _) = run_inline!(src);
    assert!(st.codes[0].start <= st.codes[0].end);
}

// 14. A backslash inside a code span is treated as verbatim content
#[test]
fn code_14_verbatim_ignores_escape() {
    let src = b"`\\*still stars\\*`";
    let (st, _) = run_inline!(src);
    assert_eq!(st.codes.len(), 1);
    assert!(st.italics.is_empty());
    assert!(txt(src, st.codes[0]).contains('\\'));
}

// 15. A four-backtick pair matches a four-backtick close
#[test]
fn code_15_count_four_matches_count_four() {
    let src = b"````x````";
    let (st, _) = run_inline!(src);
    assert_eq!(st.codes.len(), 1);
    assert_eq!(txt(src, st.codes[0]), "x");
}

// ================================================================
// escape
// ================================================================

// 01. A backslash before a star suppresses italic recognition
#[test]
fn escape_01_star_suppresses_italic() {
    let src = br"\*word\*";
    let (st, _) = run_inline!(src);
    assert!(st.italics.is_empty());
}

// 02. A backslash before a backtick suppresses code recognition
#[test]
fn escape_02_backtick_suppresses_code() {
    let src = br"\`word\`";
    let (st, _) = run_inline!(src);
    assert!(st.codes.is_empty());
}

// 03. Two backslashes before a star allow italic recognition
#[test]
fn escape_03_double_backslash_allows_italic() {
    let src = br"\\*italic*";
    let (st, _) = run_inline!(src);
    assert_eq!(st.italics.len(), 1);
}

// 04. An odd count of backslashes suppresses the following delimiter
#[test]
fn escape_04_odd_count_suppresses() {
    let src = br"\\\*no italic";
    let (st, _) = run_inline!(src);
    assert!(st.italics.is_empty());
}

// 05. An even count of backslashes allows the following delimiter
#[test]
fn escape_05_even_count_allows() {
    let src = br"\\\\*italic*";
    let (st, _) = run_inline!(src);
    assert_eq!(st.italics.len(), 1);
}

// 06. A backslash before a non-delimiter byte is treated as plain text
#[test]
fn escape_06_before_non_delimiter_is_text() {
    let src = br"\a word";
    let (st, _) = run_inline!(src);
    assert_eq!(st.texts.len(), 1);
    assert_eq!(txt(src, st.texts[0]), r"\a word");
}

// 07. Escaping the first star of a double-star leaves one opening star
#[test]
fn escape_07_first_star_escaped_second_opens() {
    let src = br"\**italic*";
    let (st, _) = run_inline!(src);
    assert_eq!(st.italics.len(), 1);
    assert_eq!(txt(src, st.italics[0]), "italic");
}

// 08. An escaped star in the middle of text produces no italic
#[test]
fn escape_08_star_in_middle_no_italic() {
    let src = br"hello \* world";
    let (st, _) = run_inline!(src);
    assert!(st.italics.is_empty());
}

// ================================================================
// hard_break
// ================================================================

// 01. Two trailing spaces trigger a hard break
#[test]
fn hard_break_01_two_spaces() {
    let src = b"word  ";
    let (st, _) = run_inline!(src);
    assert_eq!(st.hard_breaks.len(), 1);
}

// 02. Three trailing spaces also trigger a hard break
#[test]
fn hard_break_02_three_spaces() {
    let src = b"word   ";
    let (st, _) = run_inline!(src);
    assert_eq!(st.hard_breaks.len(), 1);
}

// 03. A trailing backslash triggers a hard break
#[test]
fn hard_break_03_backslash() {
    let src = b"word\\";
    let (st, _) = run_inline!(src);
    assert_eq!(st.hard_breaks.len(), 1);
}

// 04. A single trailing space does not trigger a hard break
#[test]
fn hard_break_04_one_space_no_break() {
    let src = b"word ";
    let (st, _) = run_inline!(src);
    assert!(st.hard_breaks.is_empty());
}

// 05. The hard break span is zero-length (start == end)
#[test]
fn hard_break_05_zero_length_span() {
    let src = b"word  ";
    let (st, _) = run_inline!(src);
    let s = st.hard_breaks[0];
    assert_eq!(s.start, s.end);
}

// 06. Trailing spaces are excluded from the preceding text span
#[test]
fn hard_break_06_text_excludes_trailing_spaces() {
    let src = b"word  ";
    let (st, _) = run_inline!(src);
    assert_eq!(txt(src, st.texts[0]), "word");
}

// 07. The hard break position equals the end of the preceding text span
#[test]
fn hard_break_07_position_equals_text_end() {
    let src = b"word  ";
    let (st, _) = run_inline!(src);
    assert_eq!(st.texts[0].end, st.hard_breaks[0].start);
}

// 08. A hard break can follow a bold span
#[test]
fn hard_break_08_after_bold() {
    let src = b"**x**  ";
    let (st, _) = run_inline!(src);
    assert_eq!(st.bolds.len(), 1);
    assert_eq!(st.hard_breaks.len(), 1);
}

// 09. A hard break can follow a code span
#[test]
fn hard_break_09_after_code() {
    let src = b"`x`  ";
    let (st, _) = run_inline!(src);
    assert_eq!(st.codes.len(), 1);
    assert_eq!(st.hard_breaks.len(), 1);
}

// 10. Empty input produces no hard break
#[test]
fn hard_break_10_no_break_empty_input() {
    let src = b"";
    let (st, _) = run_inline!(src);
    assert!(st.hard_breaks.is_empty());
}

// 11. A hard break can follow an italic span
#[test]
fn hard_break_11_after_italic() {
    let src = b"*i*  ";
    let (st, _) = run_inline!(src);
    assert_eq!(st.italics.len(), 1);
    assert_eq!(st.hard_breaks.len(), 1);
}

// 12. A hard break can follow a link span
#[test]
fn hard_break_12_after_link() {
    let src = b"[text](url)  ";
    let (st, _) = run_inline!(src);
    assert_eq!(st.links.len(), 1);
    assert!(!st.links[0].is_image);
    assert_eq!(st.hard_breaks.len(), 1);
}

// 13. Trailing spaces in the middle of a line do not trigger a hard break
#[test]
fn hard_break_13_not_at_end_of_line() {
    let src = b"word  more";
    let (st, _) = run_inline!(src);
    assert!(st.hard_breaks.is_empty());
}

// 14. A hard break can follow a bold-italic span
#[test]
fn hard_break_14_after_bold_italic() {
    let src = b"***bi***  ";
    let (st, _) = run_inline!(src);
    assert_eq!(st.bold_italics.len(), 1);
    assert_eq!(st.hard_breaks.len(), 1);
}

// 15. A hard break can follow an autolink span
#[test]
fn hard_break_15_after_autolink() {
    let src = b"<https://example.com>  ";
    let (st, _) = run_inline!(src);
    assert_eq!(st.autolinks.len(), 1);
    assert_eq!(st.hard_breaks.len(), 1);
}

// 16. A hard break can follow an image span
#[test]
fn hard_break_16_after_image() {
    let src = b"![alt](url)  ";
    let (st, _) = run_inline!(src);
    assert_eq!(st.links.len(), 1);
    assert!(st.links[0].is_image);
    assert_eq!(st.hard_breaks.len(), 1);
}

// ================================================================
// autolinks
// ================================================================

// 01. A basic angle-bracket URL produces one autolink span
#[test]
fn autolink_01_basic() {
    let src = b"<https://example.com>";
    let (st, _) = run_inline!(src);
    assert_eq!(st.autolinks.len(), 1);
    assert_eq!(txt(src, st.autolinks[0]), "https://example.com");
}

// 02. Autolink span excludes the angle bracket markers
#[test]
fn autolink_02_span_excludes_angle_brackets() {
    let src = b"<url>";
    let (st, _) = run_inline!(src);
    assert_eq!(st.autolinks[0].start, 1);
    assert_eq!(st.autolinks[0].end, 4);
}

// 03. An unclosed angle bracket produces no autolink span
#[test]
fn autolink_03_unclosed_no_span() {
    let src = b"<unclosed";
    let (st, _) = run_inline!(src);
    assert!(st.autolinks.is_empty());
}

// 04. Text before and after an autolink is captured as separate text spans
#[test]
fn autolink_04_between_text() {
    let src = b"see <https://example.com> here";
    let (st, _) = run_inline!(src);
    assert_eq!(st.autolinks.len(), 1);
    assert_eq!(st.texts.len(), 2);
    assert_eq!(txt(src, st.texts[0]), "see ");
    assert_eq!(txt(src, st.texts[1]), " here");
}

// 05. Two autolinks on the same line are captured independently
#[test]
fn autolink_05_multiple() {
    let src = b"<a> and <b>";
    let (st, _) = run_inline!(src);
    assert_eq!(st.autolinks.len(), 2);
    assert_eq!(txt(src, st.autolinks[0]), "a");
    assert_eq!(txt(src, st.autolinks[1]), "b");
}

// 06. Autolink span satisfies start <= end
#[test]
fn autolink_06_span_start_le_end() {
    let src = b"<https://example.com>";
    let (st, _) = run_inline!(src);
    assert!(st.autolinks[0].start <= st.autolinks[0].end);
}

// ================================================================
// links
// ================================================================

// 01. A basic link produces one span and is not an image
#[test]
fn link_01_basic() {
    let src = b"[text](url)";
    let (st, _) = run_inline!(src);
    assert_eq!(st.links.len(), 1);
    assert!(!st.links[0].is_image);
}

// 02. The link text span contains the bracketed content
#[test]
fn link_02_text_span() {
    let src = b"[hello](url)";
    let (st, _) = run_inline!(src);
    assert_eq!(txt(src, st.links[0].text), "hello");
}

// 03. The link url span contains the parenthesised content
#[test]
fn link_03_url_span() {
    let src = b"[text](https://example.com)";
    let (st, _) = run_inline!(src);
    assert_eq!(txt(src, st.links[0].url), "https://example.com");
}

// 04. An empty text bracket produces an empty text span
#[test]
fn link_04_empty_text() {
    let src = b"[](url)";
    let (st, _) = run_inline!(src);
    assert_eq!(st.links.len(), 1);
    assert_eq!(txt(src, st.links[0].text), "");
}

// 05. An empty url parenthesis produces an empty url span
#[test]
fn link_05_empty_url() {
    let src = b"[text]()";
    let (st, _) = run_inline!(src);
    assert_eq!(st.links.len(), 1);
    assert_eq!(txt(src, st.links[0].url), "");
}

// 06. Both text and url can be empty simultaneously
#[test]
fn link_06_empty_both() {
    let src = b"[]()";
    let (st, _) = run_inline!(src);
    assert_eq!(st.links.len(), 1);
    assert_eq!(txt(src, st.links[0].text), "");
    assert_eq!(txt(src, st.links[0].url), "");
}

// 07. A link without a bang prefix is not an image
#[test]
fn link_07_is_not_image() {
    let src = b"[text](url)";
    let (st, _) = run_inline!(src);
    assert!(!st.links[0].is_image);
}

// 08. Two links on the same line are captured independently
#[test]
fn link_08_two() {
    let src = b"[a](x) [b](y)";
    let (st, _) = run_inline!(src);
    assert_eq!(st.links.len(), 2);
    assert_eq!(txt(src, st.links[0].text), "a");
    assert_eq!(txt(src, st.links[1].text), "b");
}

// 09. Text before a link is captured as a separate text span
#[test]
fn link_09_text_before() {
    let src = b"hello [text](url)";
    let (st, _) = run_inline!(src);
    assert_eq!(st.links.len(), 1);
    assert_eq!(txt(src, st.texts[0]), "hello ");
}

// 10. Text after a link is captured as a separate text span
#[test]
fn link_10_text_after() {
    let src = b"[text](url) world";
    let (st, _) = run_inline!(src);
    assert_eq!(st.links.len(), 1);
    assert_eq!(txt(src, st.texts[0]), " world");
}

// 11. A bracket without a following parenthesis produces no link
#[test]
fn link_11_no_paren_no_match() {
    let src = b"[text]";
    let (st, _) = run_inline!(src);
    assert!(st.links.is_empty());
}

// 12. An unclosed bracket produces no link
#[test]
fn link_12_no_close_bracket_no_match() {
    let src = b"[text(url)";
    let (st, _) = run_inline!(src);
    assert!(st.links.is_empty());
}

// 13. A space between the bracket and parenthesis produces no link
#[test]
fn link_13_space_between_no_match() {
    let src = b"[text] (url)";
    let (st, _) = run_inline!(src);
    assert!(st.links.is_empty());
}

// 14. An escaped opening bracket produces no link
#[test]
fn link_14_escaped_bracket_no_match() {
    let src = br"\[text](url)";
    let (st, _) = run_inline!(src);
    assert!(st.links.is_empty());
}

// 15. Link text span satisfies start <= end
#[test]
fn link_15_text_span_start_le_end() {
    let src = b"[text](url)";
    let (st, _) = run_inline!(src);
    assert!(st.links[0].text.start <= st.links[0].text.end);
}

// 16. Link url span satisfies start <= end
#[test]
fn link_16_url_span_start_le_end() {
    let src = b"[text](url)";
    let (st, _) = run_inline!(src);
    assert!(st.links[0].url.start <= st.links[0].url.end);
}

// 17. A link and a bold span can appear adjacently without interfering
#[test]
fn link_17_adjacent_to_bold() {
    let src = b"**bold** [text](url)";
    let (st, _) = run_inline!(src);
    assert_eq!(st.bolds.len(), 1);
    assert_eq!(st.links.len(), 1);
}

// 18. A link and a code span can appear adjacently without interfering
#[test]
fn link_18_adjacent_to_code() {
    let src = b"`code` [text](url)";
    let (st, _) = run_inline!(src);
    assert_eq!(st.codes.len(), 1);
    assert_eq!(st.links.len(), 1);
}

// ================================================================
// images
// ================================================================

// 01. A bang-prefixed link produces one image span
#[test]
fn image_01_basic() {
    let src = b"![alt](url)";
    let (st, _) = run_inline!(src);
    assert_eq!(st.links.len(), 1);
    assert!(st.links[0].is_image);
}

// 02. The image alt span contains the bracketed content
#[test]
fn image_02_alt_span() {
    let src = b"![hello](url)";
    let (st, _) = run_inline!(src);
    assert_eq!(txt(src, st.links[0].text), "hello");
}

// 03. The image url span contains the parenthesised content
#[test]
fn image_03_url_span() {
    let src = b"![alt](https://example.com/img.png)";
    let (st, _) = run_inline!(src);
    assert_eq!(txt(src, st.links[0].url), "https://example.com/img.png");
}

// 04. An empty alt bracket produces an empty text span
#[test]
fn image_04_empty_alt() {
    let src = b"![](url)";
    let (st, _) = run_inline!(src);
    assert_eq!(st.links.len(), 1);
    assert_eq!(txt(src, st.links[0].text), "");
}

// 05. An empty url parenthesis produces an empty url span
#[test]
fn image_05_empty_url() {
    let src = b"![alt]()";
    let (st, _) = run_inline!(src);
    assert_eq!(st.links.len(), 1);
    assert_eq!(txt(src, st.links[0].url), "");
}

// 06. An image span has is_image set to true
#[test]
fn image_06_is_image_true() {
    let src = b"![alt](url)";
    let (st, _) = run_inline!(src);
    assert!(st.links[0].is_image);
}

// 07. Text before an image is captured as a separate text span
#[test]
fn image_07_text_before() {
    let src = b"see ![alt](url) here";
    let (st, _) = run_inline!(src);
    assert_eq!(st.links.len(), 1);
    assert_eq!(txt(src, st.texts[0]), "see ");
}

// 08. Text after an image is captured as a separate text span
#[test]
fn image_08_text_after() {
    let src = b"![alt](url) caption";
    let (st, _) = run_inline!(src);
    assert_eq!(st.links.len(), 1);
    assert_eq!(txt(src, st.texts[0]), " caption");
}

// 09. Two images on the same line are captured independently
#[test]
fn image_09_two() {
    let src = b"![a](x) ![b](y)";
    let (st, _) = run_inline!(src);
    assert_eq!(st.links.len(), 2);
    assert!(st.links[0].is_image);
    assert!(st.links[1].is_image);
}

// 10. An image and a link can appear together; only the image has is_image set
#[test]
fn image_10_link_mixed() {
    let src = b"![img](url1) [link](url2)";
    let (st, _) = run_inline!(src);
    assert_eq!(st.links.len(), 2);
    assert!(st.links[0].is_image);
    assert!(!st.links[1].is_image);
}

// 11. A bracket without a bang prefix is a link, not an image
#[test]
fn image_11_no_bang_is_link() {
    let src = b"[alt](url)";
    let (st, _) = run_inline!(src);
    assert!(!st.links[0].is_image);
}

// 12. An escaped bang prefix produces a link, not an image
#[test]
fn image_12_escaped_bang_no_image() {
    let src = br"\![alt](url)";
    let (st, _) = run_inline!(src);
    assert_eq!(st.links.len(), 1);
    assert!(!st.links[0].is_image);
}

// 13. Image text and url spans both satisfy start <= end
#[test]
fn image_13_span_start_le_end() {
    let src = b"![alt](url)";
    let (st, _) = run_inline!(src);
    assert!(st.links[0].text.start <= st.links[0].text.end);
    assert!(st.links[0].url.start <= st.links[0].url.end);
}

// 14. An image and a bold span can appear adjacently without interfering
#[test]
fn image_14_adjacent_to_bold() {
    let src = b"**bold** ![alt](url)";
    let (st, _) = run_inline!(src);
    assert_eq!(st.bolds.len(), 1);
    assert_eq!(st.links.len(), 1);
    assert!(st.links[0].is_image);
}

// ================================================================
// key_value
// ================================================================

// 01. A simple key=value pair on one line is captured correctly
#[test]
fn kv_01_simple() {
    let src = b"key=value";
    let (st, _) = run_inline!(src);
    assert_eq!(st.key_values.len(), 1);
    assert_eq!(txt(src, st.key_values[0].key), "key");
    assert_eq!(txt(src, st.key_values[0].value), "value");
    assert!(st.texts.is_empty());
}

// 02. A newline terminates the value; text after the newline becomes a text span
#[test]
fn kv_02_with_newline_and_rest() {
    let src = b"key=value\nrest";
    let (st, _) = run_inline!(src);
    assert_eq!(st.key_values.len(), 1);
    assert_eq!(txt(src, st.key_values[0].key), "key");
    assert_eq!(txt(src, st.key_values[0].value), "value");
    assert_eq!(st.texts.len(), 1);
    assert_eq!(txt(src, st.texts[0]), "rest");
}

// 03. Spaces around the equals sign are trimmed from key and value
#[test]
fn kv_03_spaces_around_eq() {
    let src = b"key  =  value";
    let (st, _) = run_inline!(src);
    assert_eq!(st.key_values.len(), 1);
    assert_eq!(txt(src, st.key_values[0].key), "key");
    assert_eq!(txt(src, st.key_values[0].value), "value");
}

// 04. Text before the key is captured as a separate text span
#[test]
fn kv_04_text_before_kv() {
    let src = b"hello key=value";
    let (st, _) = run_inline!(src);
    assert_eq!(st.key_values.len(), 1);
    assert_eq!(st.texts.len(), 1);
    assert_eq!(txt(src, st.texts[0]), "hello ");
    assert_eq!(txt(src, st.key_values[0].key), "key");
    assert_eq!(txt(src, st.key_values[0].value), "value");
}

// 05. Without a newline the value consumes the rest of the input including further pairs
#[test]
fn kv_05_no_newline_eats_rest() {
    let src = b"a=1 b=2";
    let (st, _) = run_inline!(src);
    assert_eq!(st.key_values.len(), 1);
    assert_eq!(txt(src, st.key_values[0].key), "a");
    assert_eq!(txt(src, st.key_values[0].value), "1 b=2");
}

// 06. Two key=value pairs on separate lines are captured independently
#[test]
fn kv_06_split_by_newline() {
    let src = b"a=1\nb=2";
    let (st, _) = run_inline!(src);
    assert_eq!(st.key_values.len(), 2);
    assert_eq!(txt(src, st.key_values[0].key), "a");
    assert_eq!(txt(src, st.key_values[0].value), "1");
    assert_eq!(txt(src, st.key_values[1].key), "b");
    assert_eq!(txt(src, st.key_values[1].value), "2");
}

// 07. A key with no value after the equals sign produces an empty value span
#[test]
fn kv_07_empty_value() {
    let src = b"key=\n";
    let (st, _) = run_inline!(src);
    assert_eq!(st.key_values.len(), 1);
    assert_eq!(txt(src, st.key_values[0].key), "key");
    assert_eq!(txt(src, st.key_values[0].value), "");
}

// 08. A value containing spaces is captured in full up to the newline
#[test]
fn kv_08_value_with_spaces() {
    let src = b"key=hello world\n";
    let (st, _) = run_inline!(src);
    assert_eq!(st.key_values.len(), 1);
    assert_eq!(txt(src, st.key_values[0].key), "key");
    assert_eq!(txt(src, st.key_values[0].value), "hello world");
}

// 09. A space-separated prefix before the key becomes a text span
#[test]
fn kv_09_space_in_key_prefix() {
    let src = b"my key=value\n";
    let (st, _) = run_inline!(src);
    assert_eq!(st.key_values.len(), 1);
    assert_eq!(st.texts.len(), 1);
    assert_eq!(txt(src, st.texts[0]), "my ");
    assert_eq!(txt(src, st.key_values[0].key), "key");
    assert_eq!(txt(src, st.key_values[0].value), "value");
}

// 10. A key followed immediately by equals with no value produces an empty value span
#[test]
fn kv_10_only_key_and_eq() {
    let src = b"key=";
    let (st, _) = run_inline!(src);
    assert_eq!(st.key_values.len(), 1);
    assert_eq!(txt(src, st.key_values[0].key), "key");
    assert_eq!(txt(src, st.key_values[0].value), "");
}

// ================================================================
// balanced asymmetric (objects)
// ================================================================

// 01. A simple brace pair produces one object span
#[test]
fn balanced_01_simple() {
    let src = b"{hello}";
    let (st, _) = run_inline_balanced!(src);
    assert_eq!(st.objects.len(), 1);
    assert_eq!(txt(src, st.objects[0]), "hello");
}

// 02. An empty brace pair produces one empty object span
#[test]
fn balanced_02_empty() {
    let src = b"{}";
    let (st, _) = run_inline_balanced!(src);
    assert_eq!(st.objects.len(), 1);
    assert_eq!(txt(src, st.objects[0]), "");
}

// 03. A nested brace is included in the outer span when balanced=true
#[test]
fn balanced_03_nested_one_level() {
    let src = b"{a {b} c}";
    let (st, _) = run_inline_balanced!(src);
    assert_eq!(st.objects.len(), 1);
    assert_eq!(txt(src, st.objects[0]), "a {b} c");
}

// 04. Two levels of nesting are both included in the outer span
#[test]
fn balanced_04_nested_two_levels() {
    let src = b"{a {b {c} d} e}";
    let (st, _) = run_inline_balanced!(src);
    assert_eq!(st.objects.len(), 1);
    assert_eq!(txt(src, st.objects[0]), "a {b {c} d} e");
}

// 05. Three independent brace pairs produce three object spans
#[test]
fn balanced_05_multiple_top_level() {
    let src = b"{a} {b} {c}";
    let (st, _) = run_inline_balanced!(src);
    assert_eq!(st.objects.len(), 3);
    assert_eq!(txt(src, st.objects[0]), "a");
    assert_eq!(txt(src, st.objects[1]), "b");
    assert_eq!(txt(src, st.objects[2]), "c");
}

// 06. Two adjacent brace pairs produce two object spans
#[test]
fn balanced_06_adjacent() {
    let src = b"{a}{b}";
    let (st, _) = run_inline_balanced!(src);
    assert_eq!(st.objects.len(), 2);
    assert_eq!(txt(src, st.objects[0]), "a");
    assert_eq!(txt(src, st.objects[1]), "b");
}

// 07. Text before and after a brace pair is captured as separate text spans
#[test]
fn balanced_07_text_before_and_after() {
    let src = b"before {x} after";
    let (st, _) = run_inline_balanced!(src);
    assert_eq!(st.objects.len(), 1);
    assert_eq!(txt(src, st.objects[0]), "x");
    assert_eq!(st.texts.len(), 2);
    assert_eq!(txt(src, st.texts[0]), "before ");
    assert_eq!(txt(src, st.texts[1]), " after");
}

// 08. An unclosed brace produces no object span
#[test]
fn balanced_08_unclosed_no_span() {
    let src = b"{hello";
    let (st, _) = run_inline_balanced!(src);
    assert_eq!(st.objects.len(), 0);
}

// 09. Object span start and end point to the inner content, not the braces
#[test]
fn balanced_09_span_bounds() {
    let src = b"aa{bb}cc";
    let (st, _) = run_inline_balanced!(src);
    assert_eq!(st.objects.len(), 1);
    assert_eq!(st.objects[0].start, 3);
    assert_eq!(st.objects[0].end, 5);
}

// ================================================================
// symmetric balanced
// ================================================================

// 01. A basic double-quote pair produces one code span
#[test]
fn sym_bal_01_basic() {
    let src = b"\"hello\"";
    let st = run_sym_balanced!(src);
    assert_eq!(st.codes.len(), 1);
    assert_eq!(txt(src, st.codes[0]), "hello");
}

// 02. A doubled inner quote is treated as balanced content, not a closer
#[test]
fn sym_bal_02_doubled_is_escape() {
    let src = b"\"hello \"\"world\"\" end\"";
    let st = run_sym_balanced!(src);
    assert_eq!(st.codes.len(), 1);
    assert_eq!(txt(src, st.codes[0]), "hello \"\"world\"\" end");
}

// 03. Two independent quoted spans are captured separately
#[test]
fn sym_bal_03_two_spans() {
    let src = b"\"a\" \"b\"";
    let st = run_sym_balanced!(src);
    assert_eq!(st.codes.len(), 2);
    assert_eq!(txt(src, st.codes[0]), "a");
    assert_eq!(txt(src, st.codes[1]), "b");
}

// 04. An unclosed quote produces no code span
#[test]
fn sym_bal_04_unclosed_no_span() {
    let src = b"\"unclosed";
    let st = run_sym_balanced!(src);
    assert!(st.codes.is_empty());
}

// 05. A double-quote pair inside a triple-quote is captured as the inner content
#[test]
fn sym_bal_05_double_quote_paired_independently() {
    let src = b"\"hello \"\"world\"\"";
    let st = run_sym_balanced!(src);
    assert_eq!(st.codes.len(), 1);
    assert_eq!(txt(src, st.codes[0]), "world");
}

// 06. Two adjacent double-quotes form a count-2 opener that finds no closer
#[test]
fn sym_bal_06_adjacent_quotes_is_count2_opener() {
    let src = b"\"\"";
    let st = run_sym_balanced!(src);
    assert_eq!(st.codes.len(), 0);
}

// 07. Text before and after a quoted span is captured as separate text spans
#[test]
fn sym_bal_07_text_before_and_after() {
    let src = b"before \"inside\" after";
    let st = run_sym_balanced!(src);
    assert_eq!(st.codes.len(), 1);
    assert_eq!(txt(src, st.codes[0]), "inside");
    assert_eq!(st.texts.len(), 2);
}

// 08. A count-2 opener matches a count-2 closer to capture the inner content
#[test]
fn sym_bal_08_count2_opener_and_closer() {
    let src = b"\"\"hello\"\"";
    let st = run_sym_balanced!(src);
    assert_eq!(st.codes.len(), 1);
    assert_eq!(txt(src, st.codes[0]), "hello");
}

// ================================================================
// chained balanced — text component (tbal)
// ================================================================

// 01. A nested bracket in the text component is included when tbal=true
#[test]
fn chained_tbal_01_nested_brackets() {
    let src = b"[a [b] c](url)";
    let st = run_chained_balanced!(src, true, false);
    assert_eq!(st.links.len(), 1);
    assert_eq!(txt(src, st.links[0].text), "a [b] c");
    assert_eq!(txt(src, st.links[0].url), "url");
}

// 02. A nested bracket in the text component stops the match when tbal=false
#[test]
fn chained_tbal_02_false_nested_no_match() {
    let src = b"[a [b] c](url)";
    let st = run_chained_balanced!(src, false, false);
    assert_eq!(st.links.len(), 0);
}

// 03. Without nesting, tbal=true and tbal=false produce the same result
#[test]
fn chained_tbal_03_no_nesting_same_both_modes() {
    let src = b"[text](url)";
    let st_t = run_chained_balanced!(src, true, false);
    let st_f = run_chained_balanced!(src, false, false);
    assert_eq!(st_t.links.len(), 1);
    assert_eq!(st_f.links.len(), 1);
    assert_eq!(txt(src, st_t.links[0].text), "text");
    assert_eq!(txt(src, st_f.links[0].text), "text");
}

// 04. Two levels of bracket nesting are included in the text span when tbal=true
#[test]
fn chained_tbal_04_deep_nesting() {
    let src = b"[a [b [c] d] e](url)";
    let st = run_chained_balanced!(src, true, false);
    assert_eq!(st.links.len(), 1);
    assert_eq!(txt(src, st.links[0].text), "a [b [c] d] e");
}

// ================================================================
// chained balanced — url component (ubal)
// ================================================================

// 05. A nested parenthesis in the url component is included when ubal=true
#[test]
fn chained_ubal_05_nested_parens() {
    let src = b"[text](url(nested))";
    let st = run_chained_balanced!(src, false, true);
    assert_eq!(st.links.len(), 1);
    assert_eq!(txt(src, st.links[0].url), "url(nested)");
}

// 06. A nested parenthesis in the url component stops the match when ubal=false
#[test]
fn chained_ubal_06_false_stops_at_first_paren() {
    let src = b"[text](url(nested))";
    let st = run_chained_balanced!(src, false, false);
    assert_eq!(st.links.len(), 1);
    assert_eq!(txt(src, st.links[0].url), "url(nested");
}

// 07. Without nesting, ubal=true and ubal=false produce the same result
#[test]
fn chained_ubal_07_no_nesting_same_both_modes() {
    let src = b"[text](simple)";
    let st_t = run_chained_balanced!(src, false, true);
    let st_f = run_chained_balanced!(src, false, false);
    assert_eq!(st_t.links.len(), 1);
    assert_eq!(st_f.links.len(), 1);
    assert_eq!(txt(src, st_t.links[0].url), "simple");
    assert_eq!(txt(src, st_f.links[0].url), "simple");
}

// 08. Two levels of parenthesis nesting are included in the url span when ubal=true
#[test]
fn chained_ubal_08_deep_nesting() {
    let src = b"[text](a(b(c)))";
    let st = run_chained_balanced!(src, false, true);
    assert_eq!(st.links.len(), 1);
    assert_eq!(txt(src, st.links[0].url), "a(b(c))");
}

// ================================================================
// chained both balanced
// ================================================================

// 09. Both text and url components can be balanced independently
#[test]
fn chained_both_bal_09_basic() {
    let src = b"[a [b] c](url(nested))";
    let st = run_chained_balanced!(src, true, true);
    assert_eq!(st.links.len(), 1);
    assert_eq!(txt(src, st.links[0].text), "a [b] c");
    assert_eq!(txt(src, st.links[0].url), "url(nested)");
}

// 10. All span bounds satisfy start <= end <= source length when both are balanced
#[test]
fn chained_both_bal_10_span_bounds() {
    let src = b"[a [b] c](url(nested))";
    let st = run_chained_balanced!(src, true, true);
    let len = src.len() as u32;
    assert!(st.links[0].text.start <= st.links[0].text.end);
    assert!(st.links[0].text.end <= len);
    assert!(st.links[0].url.start <= st.links[0].url.end);
    assert!(st.links[0].url.end <= len);
}

// 11. An image with parentheses in the url is captured correctly when ubal=true
#[test]
fn chained_ubal_11_image_with_parens_in_url() {
    let src = b"![alt](img/photo(1).png)";
    let st = run_chained_balanced!(src, false, true);
    assert_eq!(st.links.len(), 1);
    assert!(st.links[0].is_image);
    assert_eq!(txt(src, st.links[0].text), "alt");
    assert_eq!(txt(src, st.links[0].url), "img/photo(1).png");
}

// 12. Multiple links with nested text brackets are captured independently
#[test]
fn chained_tbal_12_multiple_links_sequence() {
    let src = b"[a [b] c](url1) [d [e] f](url2)";
    let st = run_chained_balanced!(src, true, false);
    assert_eq!(st.links.len(), 2);
    assert_eq!(txt(src, st.links[0].text), "a [b] c");
    assert_eq!(txt(src, st.links[0].url), "url1");
    assert_eq!(txt(src, st.links[1].text), "d [e] f");
    assert_eq!(txt(src, st.links[1].url), "url2");
}

// ================================================================
// balanced asymmetric — multi-level (max_nest > 1)
// ================================================================
//
// All of these use `run_inline_balanced_nested!`, the same grammar as
// `run_inline_balanced!` above with a caller-supplied `max_nest`. The
// `balanced_*` tests above already cover `max_nest = 1` (the collapsing,
// pre-nesting-equivalent behaviour) — these specifically exercise depth > 1.

// 01. At depth 2, a single level of nesting is split into two spans,
// sorted by start: outer first, inner second.
#[test]
fn balanced_nested_01_depth2_two_spans() {
    let src = b"{a {b} c}";
    let (st, _) = run_inline_balanced_nested!(src, 2);
    assert_eq!(st.objects.len(), 2);
    assert!(st.objects[0].start < st.objects[1].start);
    assert_eq!(txt(src, st.objects[0]), "a {b} c");
    assert_eq!(txt(src, st.objects[1]), "b");
}

// 02. The outer span contains the inner span (interval containment —
// this is how a consumer reconstructs the tree, no parent field needed).
#[test]
fn balanced_nested_02_outer_contains_inner() {
    let src = b"{a {b} c}";
    let (st, _) = run_inline_balanced_nested!(src, 2);
    assert!(st.objects[0].start <= st.objects[1].start);
    assert!(st.objects[0].end >= st.objects[1].end);
}

// 03. At depth 3, three nested levels each get their own span.
#[test]
fn balanced_nested_03_depth3_three_spans() {
    let src = b"{a {b {c} d} e}";
    let (st, _) = run_inline_balanced_nested!(src, 3);
    assert_eq!(st.objects.len(), 3);
    assert!(st.objects[0].start < st.objects[1].start);
    assert!(st.objects[1].start < st.objects[2].start);
    assert_eq!(txt(src, st.objects[0]), "a {b {c} d} e");
    assert_eq!(txt(src, st.objects[1]), "b {c} d");
    assert_eq!(txt(src, st.objects[2]), "c");
}

// 04. A cap below the actual nesting depth in the input still resolves
// correctly: the untracked innermost pair is skipped via the overflow
// counter, so the capped (middle) level's close lands on the right brace,
// not the first one the scanner sees.
#[test]
fn balanced_nested_04_cap_below_actual_depth() {
    let src = b"{a {b {c} d} e}";
    let (st, _) = run_inline_balanced_nested!(src, 2);
    assert_eq!(st.objects.len(), 2);
    assert_eq!(txt(src, st.objects[0]), "a {b {c} d} e");
    assert_eq!(txt(src, st.objects[1]), "b {c} d");
}

// 05. A cap of 1 on a multi-level input reproduces the exact collapsing
// behaviour of `run_inline_balanced!` — the explicit depth-1 boundary,
// not just its default.
#[test]
fn balanced_nested_05_depth1_collapses() {
    let src = b"{a {b} c}";
    let (st, _) = run_inline_balanced_nested!(src, 1);
    assert_eq!(st.objects.len(), 1);
    assert_eq!(txt(src, st.objects[0]), "a {b} c");
}

// 06. A frame that closes properly (inner) survives even though an
// enclosing frame (outer) never finds its close before line end — this is
// the exact scenario that requires `Vec::remove` rather than `truncate` in
// the discard step: the inner entry sits at a *higher* index than the
// still-open outer one.
#[test]
fn balanced_nested_06_unclosed_outer_keeps_closed_inner() {
    let src = b"{a {b} c";
    let (st, _) = run_inline_balanced_nested!(src, 4);
    assert_eq!(st.objects.len(), 1);
    assert_eq!(txt(src, st.objects[0]), "b");
}

// 07. Both frames unclosed: nothing survives.
#[test]
fn balanced_nested_07_both_unclosed_nothing_survives() {
    let src = b"{a {b c";
    let (st, _) = run_inline_balanced_nested!(src, 4);
    assert_eq!(st.objects.len(), 0);
}

// 08. A close byte encountered with an empty stack is literal, not a
// structural event — no panic, no underflow, no spurious span.
#[test]
fn balanced_nested_08_stray_close_on_empty_stack_is_literal() {
    let src = b"{a} b} c";
    let (st, _) = run_inline_balanced_nested!(src, 2);
    assert_eq!(st.objects.len(), 1);
    assert_eq!(txt(src, st.objects[0]), "a");
}

// 09. Three independent (non-nested) top-level pairs still each get their
// own span at higher max_nest, exactly as at max_nest = 1 — depth only
// matters when pairs are actually nested.
#[test]
fn balanced_nested_09_independent_pairs_unaffected_by_depth() {
    let src = b"{a} {b} {c}";
    let (st, _) = run_inline_balanced_nested!(src, 4);
    assert_eq!(st.objects.len(), 3);
    assert_eq!(txt(src, st.objects[0]), "a");
    assert_eq!(txt(src, st.objects[1]), "b");
    assert_eq!(txt(src, st.objects[2]), "c");
}

// ================================================================
// symmetric balanced (parse_inside = true) — different-key nesting fix
// ================================================================
//
// All of these use `run_inline_sym_nested!`, a `symmetric { parse_inside =
// true; balanced = true; ... }` fixture with its own `n_italics` / `n_bolds`
// fields, separate from the pre-existing `italics` / `bolds` fixtures
// above (which stay `balanced = false`, exercising the original single
// pending-slot mechanism, untouched).

// 01. The bug this fixes: with a single pending slot, a different-count
// occurrence of the same byte used to silently overwrite the still-pending
// outer delimiter, so the outer pair never closed. With the bounded stack
// (depth >= 2), both levels resolve.
#[test]
fn sym_nested_01_different_key_nesting_fix() {
    let src = b"**bold *italic* still-bold**";
    let (st, _) = run_inline_sym_nested!(src, 2);
    assert_eq!(st.n_bolds.len(), 1);
    assert_eq!(st.n_italics.len(), 1);
    assert_eq!(txt(src, st.n_bolds[0]), "bold *italic* still-bold");
    assert_eq!(txt(src, st.n_italics[0]), "italic");
}

// 02. At depth 1 (the boundary, not just the default), the different-key
// case is deliberately *not* fixed: there is no room to track the inner
// frame, so it is left as literal content inside the (still correctly
// closing, thanks to the stack rather than an overwritable slot) outer
// span.
#[test]
fn sym_nested_02_depth1_outer_still_closes_inner_untracked() {
    let src = b"**bold *italic* still-bold**";
    let (st, _) = run_inline_sym_nested!(src, 1);
    assert_eq!(st.n_bolds.len(), 1);
    assert_eq!(st.n_italics.len(), 0);
    assert_eq!(txt(src, st.n_bolds[0]), "bold *italic* still-bold");
}

// 03. An identical (byte, count) pair cannot self-nest — open and close
// look the same for a symmetric delimiter, so there is no signal to tell
// "nested open" apart from "close". Two adjacent runs result, with the
// middle text left as plain content rather than nested.
#[test]
fn sym_nested_03_identical_key_toggles_not_nests() {
    let src = b"**a **b** c**";
    let (st, _) = run_inline_sym_nested!(src, 4);
    assert_eq!(st.n_bolds.len(), 2);
    assert_eq!(txt(src, st.n_bolds[0]), "a ");
    assert_eq!(txt(src, st.n_bolds[1]), " c");
}

// 04. A frame still pending at line end is discarded — neither delimiter
// closes within the line, so neither produces a span.
#[test]
fn sym_nested_04_unclosed_both_discarded() {
    let src = b"**bold *italic still open";
    let (st, _) = run_inline_sym_nested!(src, 3);
    assert_eq!(st.n_bolds.len(), 0);
    assert_eq!(st.n_italics.len(), 0);
}

// 05. Two different italic occurrences inside one bold, both resolved when
// there is room for them: each closes and reopens its own field
// independently, sequentially, after the previous one closed.
#[test]
fn sym_nested_05_two_different_keys_in_sequence() {
    let src = b"**bold *i1* mid *i2* end**";
    let (st, _) = run_inline_sym_nested!(src, 2);
    assert_eq!(st.n_bolds.len(), 1);
    assert_eq!(st.n_italics.len(), 2);
    assert_eq!(txt(src, st.n_italics[0]), "i1");
    assert_eq!(txt(src, st.n_italics[1]), "i2");
    assert_eq!(txt(src, st.n_bolds[0]), "bold *i1* mid *i2* end");
}

// 06. Plain text with no delimiters at all produces neither field.
#[test]
fn sym_nested_06_plain_text_no_spans() {
    let src = b"just plain text";
    let (st, _) = run_inline_sym_nested!(src, 4);
    assert!(st.n_bolds.is_empty());
    assert!(st.n_italics.is_empty());
}

// ================================================================
// balanced asymmetric — multi-level, continued (run edge cases)
// ================================================================

// 10. A triple-character open run with max_nest = 3 opens three real
// levels from one run — each byte of `{{{` is its own event.
#[test]
fn balanced_nested_10_triple_run_depth3_three_levels() {
    let src = b"{{{x}}}";
    let (st, _) = run_inline_balanced_nested!(src, 3);
    assert_eq!(st.objects.len(), 3);
    assert_eq!(txt(src, st.objects[0]), "{{x}}");
    assert_eq!(txt(src, st.objects[1]), "{x}");
    assert_eq!(txt(src, st.objects[2]), "x");
}

// 11. The same triple-character run with max_nest = 2: the innermost
// pair is skipped via the overflow counter, the outer two still resolve.
#[test]
fn balanced_nested_11_triple_run_depth2_overflow_skips_innermost() {
    let src = b"{{{x}}}";
    let (st, _) = run_inline_balanced_nested!(src, 2);
    assert_eq!(st.objects.len(), 2);
    assert_eq!(txt(src, st.objects[0]), "{{x}}");
    assert_eq!(txt(src, st.objects[1]), "{x}");
}

// 12. The same triple-character run with max_nest = 1 fully collapses to
// one span — multi-character runs behave the same as single-character
// ones at the depth-1 boundary.
#[test]
fn balanced_nested_12_triple_run_depth1_collapses() {
    let src = b"{{{x}}}";
    let (st, _) = run_inline_balanced_nested!(src, 1);
    assert_eq!(st.objects.len(), 1);
    assert_eq!(txt(src, st.objects[0]), "{{x}}");
}

// 13. A close run longer than what's actually needed: the real close
// consumes one byte, the remaining bytes of that same run are literal
// text — not lost, not mistaken for anything else.
#[test]
fn balanced_nested_13_excess_close_bytes_become_text_not_lost() {
    let src = b"{x}}}";
    let (st, _) = run_inline_balanced_nested!(src, 2);
    assert_eq!(st.objects.len(), 1);
    assert_eq!(txt(src, st.objects[0]), "x");
    assert_eq!(st.texts.len(), 1);
    assert_eq!(txt(src, st.texts[0]), "}}");
}

// 14. An open run longer than what later closes: the outermost level
// never finds its close and is discarded, while the inner levels that
// did close survive — and survive at the right indices, since closing
// uses `Vec::remove`, not `truncate`.
#[test]
fn balanced_nested_14_unclosed_outer_discarded_inner_survive() {
    let src = b"{{{x}}";
    let (st, _) = run_inline_balanced_nested!(src, 3);
    assert_eq!(st.objects.len(), 2);
    assert_eq!(txt(src, st.objects[0]), "{x}");
    assert_eq!(txt(src, st.objects[1]), "x");
}

// 15. At max_nest = 1, three opens followed by only two closes leaves
// nothing: both available closes are consumed decrementing the overflow
// counter (for the two untracked extra opens), so the one tracked frame
// never receives its real close and is discarded entirely.
#[test]
fn balanced_nested_15_depth1_overflow_consumes_both_closes_nothing_survives() {
    let src = b"{{{x}}";
    let (st, _) = run_inline_balanced_nested!(src, 1);
    assert_eq!(st.objects.len(), 0);
}

// 16. Two independent multi-level groups on the same line don't share or
// corrupt each other's bookkeeping — each gets its own pair of levels.
#[test]
fn balanced_nested_16_two_independent_multilevel_groups() {
    let src = b"{{x}} {{y}}";
    let (st, _) = run_inline_balanced_nested!(src, 2);
    assert_eq!(st.objects.len(), 4);
    assert_eq!(txt(src, st.objects[0]), "{x}");
    assert_eq!(txt(src, st.objects[1]), "x");
    assert_eq!(txt(src, st.objects[2]), "{y}");
    assert_eq!(txt(src, st.objects[3]), "y");
}

// 17. A stray close byte with nothing open yet (right at the start of the
// line) is literal text and does not corrupt a properly nested group
// that follows it.
#[test]
fn balanced_nested_17_leading_stray_close_then_valid_group() {
    let src = b"}{{a}}";
    let (st, _) = run_inline_balanced_nested!(src, 2);
    assert_eq!(st.texts.len(), 1);
    assert_eq!(txt(src, st.texts[0]), "}");
    assert_eq!(st.objects.len(), 2);
    assert_eq!(txt(src, st.objects[0]), "{a}");
    assert_eq!(txt(src, st.objects[1]), "a");
}

// 18. Zero-length content at the innermost nested level is captured
// correctly as an empty span, not skipped or merged with its parent.
#[test]
fn balanced_nested_18_innermost_empty_content() {
    let src = b"{{}}";
    let (st, _) = run_inline_balanced_nested!(src, 2);
    assert_eq!(st.objects.len(), 2);
    assert_eq!(txt(src, st.objects[0]), "{}");
    assert_eq!(txt(src, st.objects[1]), "");
}

// 19. Text between an outer pair's open and a same-type inner pair's open
// is not a separate top-level text span — same principle as the
// symmetric tests above, applied to asymmetric. Confirms the existing
// `balanced_nested_01` shape (`objects` content is unchanged) while also
// locking in that `texts` stays empty.
#[test]
fn balanced_nested_19_filler_between_nested_opens_not_a_separate_text_span() {
    let src = b"{a {b} c}";
    let (st, _) = run_inline_balanced_nested!(src, 2);
    assert_eq!(st.objects.len(), 2);
    assert_eq!(txt(src, st.objects[0]), "a {b} c");
    assert_eq!(txt(src, st.objects[1]), "b");
    assert!(st.texts.is_empty());
}

// ================================================================
// symmetric balanced (parse_inside = true) — continued (edge cases)
// ================================================================

// 07. An occurrence whose count matches no declared arm (only 1 and 2
// are declared; this is a run of 3) is literal — it doesn't open a frame,
// doesn't move the stack, and doesn't prevent the enclosing pair that's
// already open from later closing correctly around it.
#[test]
fn sym_nested_07_unmatched_count_inside_open_pair_is_literal() {
    let src = b"**bold ***literal*** still-bold**";
    let (st, _) = run_inline_sym_nested!(src, 2);
    assert_eq!(st.n_bolds.len(), 1);
    assert_eq!(txt(src, st.n_bolds[0]), "bold ***literal*** still-bold");
}

// 08. A different-key occurrence arriving once the stack is already at
// its cap is literal — it doesn't corrupt the frame already open, and a
// later, properly-nested occurrence of that same different key still
// resolves once the cap frees up again. Nothing here ends up in `texts`:
// every byte not claimed by `n_bolds`/`n_italics` is still content of the
// open bold, not standalone top-level text.
#[test]
fn sym_nested_08_beyond_cap_different_key_is_literal_then_recovers() {
    let src = b"**a *b**c* d**";
    let (st, _) = run_inline_sym_nested!(src, 2);
    assert_eq!(st.n_bolds.len(), 1);
    assert_eq!(txt(src, st.n_bolds[0]), "a *b**c* d");
    assert_eq!(st.n_italics.len(), 1);
    assert_eq!(txt(src, st.n_italics[0]), "b**c");
    assert!(st.texts.is_empty());
}

// 09. Text between an outer pair's open and an inner, different-key
// pair's open is not a separate top-level text span — it stays inside the
// outer pair's own content, exactly like the bytes between the inner
// pair's close and the outer pair's close.
#[test]
fn sym_nested_09_filler_between_open_pairs_not_a_separate_text_span() {
    let src = b"**bold *italic* still-bold**";
    let (st, _) = run_inline_sym_nested!(src, 2);
    assert_eq!(st.n_bolds.len(), 1);
    assert_eq!(st.n_italics.len(), 1);
    assert!(st.texts.is_empty());
}

// 10. An outer frame that never closes (discarded at line end) doesn't
// affect an inner, different-field frame that already closed properly —
// the two live in separate Vecs, so discarding one is independent of the
// other surviving.
#[test]
fn sym_nested_10_unclosed_outer_does_not_affect_closed_inner_different_field() {
    let src = b"**bold *italic* unclosed";
    let (st, _) = run_inline_sym_nested!(src, 2);
    assert!(st.n_bolds.is_empty());
    assert_eq!(st.n_italics.len(), 1);
    assert_eq!(txt(src, st.n_italics[0]), "italic");
}

// ---------------------------------------------------------------
// max_nest = 0 (degenerate, but legal — front-end doesn't reject it)
// ---------------------------------------------------------------

// 01. At max_nest = 0, a balanced asymmetric pair never opens at all — the
//     cap is 0, so `(asym_depth as usize) < _cap` is false on the very
//     first occurrence, and the open byte is left as literal text rather
//     than panicking on a zero-sized array.
#[test]
fn maxnest_zero_01_balanced_asymmetric_never_opens() {
    let src = b"{hello}";
    let (st, _) = run_inline_balanced_nested!(src, 0);
    assert_eq!(st.objects.len(), 0);
}

// 02. At max_nest = 0, the open and close bytes both surface as plain text
//     (no panic, no lost bytes — the overflow counter never engages for
//     `balanced = true` outside the open path, so the close byte falls
//     through to ordinary text accumulation).
#[test]
fn maxnest_zero_02_bytes_preserved_as_text() {
    let src = b"{x}";
    let (st, _) = run_inline_balanced_nested!(src, 0);
    assert!(st.objects.is_empty());
    let all: String = st.texts.iter().map(|&s| txt(src, s)).collect();
    assert_eq!(all, "{x}");
}

// 03. At max_nest = 0, a symmetric balanced rule (parse_inside = true,
//     balanced = true) likewise never opens a frame — same cap-is-zero
//     reasoning as the asymmetric case, exercised on the other stack.
#[test]
fn maxnest_zero_03_symmetric_balanced_never_opens() {
    let src = b"*italic*";
    let (st, _) = run_inline_sym_nested!(src, 0);
    assert!(st.n_italics.is_empty());
    assert!(st.n_bolds.is_empty());
}

// ---------------------------------------------------------------
// asymmetric rules sharing a close byte (documented caveat, not rejected)
// ---------------------------------------------------------------
//
// Fixture: two asymmetric rules in one on_trigger block, `(`,`)` and `[`,`)`
// — chosen so the OPEN bytes differ (so the dispatcher can tell which rule
// opened a given frame) but the CLOSE byte (`)`) is identical between them.
// This is the exact shape flagged in inline.rs's doc-comment: the frame on
// the stack still resolves by its own recorded close byte, but the
// `match _rc { $an => … }` arm that actually receives the close is whichever
// rule's `1 => field` happens to be reached for count `1` in declaration
// order inside that match — which, since both rules declare their exact
// arm as `1`, are not actually ambiguous at the match level (each rule's
// `$an => $af` pair is distinct token-wise), but the open byte is what
// decides which frame (and therefore which field) is on the stack in the
// first place. These tests lock in that the OPEN byte — not just "any
// occurrence of the shared close byte" — determines routing.

macro_rules! run_inline_shared_close_nested {
    ($src:expr, $maxn:literal) => {{
        let src: &[u8] = $src;
        let le = src.len();
        let mut st = ParseState::new(le);
        let consumed = meon::parse_inline!(
            st, src, 0, le, texts, false, b'\\', b' ', b'\t', b'n', $maxn;
            on_trigger(b'(', b')', b'[') {
                asymmetric b'(', b')' {
                    balanced     = true;
                    parse_inside = false;
                    1 => objects
                }
                asymmetric b'[', b')' {
                    balanced     = true;
                    parse_inside = false;
                    1 => n_italics
                }
            }
        );
        (st, consumed)
    }};
}

// 04. A `(...)` pair routes to its own rule's field (`objects`), matching
//     close byte `)` against the frame's own recorded `$ac` — not against
//     whichever rule happens to be declared first.
#[test]
fn shared_close_04_paren_pair_routes_to_objects() {
    let src = b"(hello)";
    let (st, _) = run_inline_shared_close_nested!(src, 2);
    assert_eq!(st.objects.len(), 1);
    assert_eq!(txt(src, st.objects[0]), "hello");
    assert!(st.n_italics.is_empty());
}

// 05. A `[...)` pair (mismatched-looking but matching this fixture's second
//     rule, which declares open `[` / close `)`) routes to that rule's own
//     field (`n_italics`), not to `objects` — confirming routing keys off
//     the *open* byte recorded on the frame, not just "first rule whose
//     close byte matches".
#[test]
fn shared_close_05_bracket_paren_pair_routes_to_n_italics() {
    let src = b"[hello)";
    let (st, _) = run_inline_shared_close_nested!(src, 2);
    assert_eq!(st.n_italics.len(), 1);
    assert_eq!(txt(src, st.n_italics[0]), "hello");
    assert!(st.objects.is_empty());
}

// 06. Two independent pairs, one of each rule, on the same line: each
//     resolves to its own field without cross-contamination, even though
//     both close on the identical byte `)`.
#[test]
fn shared_close_06_both_rules_independent_on_same_line() {
    let src = b"(a) [b)";
    let (st, _) = run_inline_shared_close_nested!(src, 2);
    assert_eq!(st.objects.len(), 1);
    assert_eq!(txt(src, st.objects[0]), "a");
    assert_eq!(st.n_italics.len(), 1);
    assert_eq!(txt(src, st.n_italics[0]), "b");
}

// 07. A `(` opened while a `[` frame is already on top of the stack nests
//     correctly using each frame's own recorded close byte: the inner `(`
//     closes on the first `)`, and because the outer `[`'s recorded close
//     byte is also `)`, the *second* `)` correctly closes the outer frame
//     rather than being mistaken for stray input.
#[test]
fn shared_close_07_nested_different_open_bytes_shared_close() {
    let src = b"[a (b) c)";
    let (st, _) = run_inline_shared_close_nested!(src, 2);
    assert_eq!(st.objects.len(), 1);
    assert_eq!(txt(src, st.objects[0]), "b");
    assert_eq!(st.n_italics.len(), 1);
    assert_eq!(txt(src, st.n_italics[0]), "a (b) c");
}

// ================================================================
// escaped closing delimiters — internal forward-search fix
// ================================================================
//
// These exercise the `@is_escaped` check added to every *internal*
// forward search that previously bypassed the outer dispatch loop's
// escape-awareness entirely:
//
//   - symmetric greedy mode (`parse_inside = false`), both `balanced`
//     settings — used for code spans and balanced quote-like rules;
//   - the legacy asymmetric memchr search (`balanced = false,
//     parse_inside = false`) — used for autolinks;
//   - the legacy chained two-phase search (both components
//     `parse_inside = false`) — used for `[text](url)`-style links.
//
// Opacity (`parse_inside`) is unaffected in every case below — none of
// these rules scan their own content for other rules' triggers, before or
// after this fix. What changed is purely whether the *closing* delimiter
// itself is correctly distinguished from a literal, backslash-escaped
// occurrence of the same byte. The existing `sym_bal_02_doubled_is_escape`
// test (unescaped doubled-quote content) already covers the regression
// case that this fix must not disturb — content with no backslash involved
// at all — so it isn't duplicated here.

// 01. An escaped closing backtick inside an open code span does not close
//     it; the search continues and finds the real, unescaped closing run.
//     Before this fix, the first (escaped) backtick closed immediately,
//     leaving "b`" stray outside the span.
#[test]
fn escape_close_01_code_span_escaped_backtick_skipped() {
    let src = br"`a\`b`";
    let (st, _) = run_inline!(src);
    assert_eq!(st.codes.len(), 1);
    assert_eq!(txt(src, st.codes[0]), r"a\`b");
}

// 02. Two consecutively, individually escaped backticks are both skipped
//     in turn before the real, unescaped close is found.
#[test]
fn escape_close_02_code_span_two_escaped_backticks() {
    let src = br"`a\`\`b`";
    let (st, _) = run_inline!(src);
    assert_eq!(st.codes.len(), 1);
    assert_eq!(txt(src, st.codes[0]), r"a\`\`b");
}

// 03. With nothing between the escaped backtick and the real close, the
//     span still resolves correctly. Before this fix, this exact input
//     produced no code span at all: the two adjacent backticks right after
//     the escaped one formed a count-2 run that never matched the
//     count-1 opener, and the search found nothing else to close on.
#[test]
fn escape_close_03_code_span_escaped_then_immediate_close() {
    let src = br"`\``";
    let (st, _) = run_inline!(src);
    assert_eq!(st.codes.len(), 1);
    assert_eq!(txt(src, st.codes[0]), r"\`");
}

// 04. An escaped quote inside a balanced-greedy symmetric match (the same
//     mechanism that treats an unescaped doubled quote as literal content)
//     is treated as literal content too, not mistaken for the close.
#[test]
fn escape_close_04_balanced_symmetric_escaped_quote() {
    let src = b"\"a\\\"b\"";
    let st = run_sym_balanced!(src);
    assert_eq!(st.codes.len(), 1);
    assert_eq!(txt(src, st.codes[0]), "a\\\"b");
}

// 05. An escaped closing `>` on an autolink (the legacy asymmetric memchr
//     path) is skipped; the real, unescaped `>` closes it instead.
#[test]
fn escape_close_05_autolink_escaped_close() {
    let src = br"<http://example.com\>more>";
    let (st, _) = run_inline!(src);
    assert_eq!(st.autolinks.len(), 1);
    assert_eq!(txt(src, st.autolinks[0]), r"http://example.com\>more");
}

// 06. A chained link's text bracket, non-balanced (tbal = false): an
//     escaped `]` does not close the text component.
#[test]
fn escape_close_06_chained_text_escaped_bracket_no_nesting() {
    let src = br"[a\]b](url)";
    let st = run_chained_balanced!(src, false, false);
    assert_eq!(st.links.len(), 1);
    assert_eq!(txt(src, st.links[0].text), r"a\]b");
    assert_eq!(txt(src, st.links[0].url), "url");
}

// 07. A chained link's text bracket, balanced (tbal = true): an escaped
//     `]` inside genuine nested brackets is neither mistaken for the close
//     nor for a depth-decrementing event — it doesn't disrupt the depth
//     count for the real, unescaped nested pair.
#[test]
fn escape_close_07_chained_text_escaped_bracket_with_real_nesting() {
    let src = br"[a [b\]c] d](url)";
    let st = run_chained_balanced!(src, true, false);
    assert_eq!(st.links.len(), 1);
    assert_eq!(txt(src, st.links[0].text), r"a [b\]c] d");
    assert_eq!(txt(src, st.links[0].url), "url");
}

// 08. A chained link's url paren, non-balanced (ubal = false): an escaped
//     `)` does not close the url component.
#[test]
fn escape_close_08_chained_url_escaped_paren_no_nesting() {
    let src = br"[text](a\)b)";
    let st = run_chained_balanced!(src, false, false);
    assert_eq!(st.links.len(), 1);
    assert_eq!(txt(src, st.links[0].url), r"a\)b");
}

// 09. A chained link's url paren, balanced (ubal = true): an escaped `)`
//     inside genuine nested parens neither closes early nor disrupts the
//     depth count for the real, unescaped nested pair.
#[test]
fn escape_close_09_chained_url_escaped_paren_with_real_nesting() {
    let src = br"[text](a(b\)c) d)";
    let st = run_chained_balanced!(src, false, true);
    assert_eq!(st.links.len(), 1);
    assert_eq!(txt(src, st.links[0].url), r"a(b\)c) d");
}

// 10. Span bounds stay within source length across every path touched by
//     this fix — a basic sanity invariant, not specific to escaping itself.
#[test]
fn escape_close_10_span_bounds_sane_across_fixed_paths() {
    let len_check = |src: &[u8], spans: &[meon::span::Span]| {
        let len = src.len() as u32;
        for s in spans {
            assert!(s.start <= s.end && s.end <= len);
        }
    };

    let src1 = br"`a\`b`";
    let (st1, _) = run_inline!(src1);
    len_check(src1, &st1.codes);

    let src2: &[u8] = br"<http://example.com\>more>";
    let (st2, _) = run_inline!(src2);
    len_check(src2, &st2.autolinks);

    let src3: &[u8] = br"[a [b\]c] d](url)";
    let st3 = run_chained_balanced!(src3, true, false);
    let link_spans: Vec<meon::span::Span> =
        st3.links.iter().flat_map(|l| [l.text, l.url]).collect();
    len_check(src3, &link_spans);
}

// 01. quoted key + scalar value; key span includes quotes, object content excludes braces
#[test]
fn kvn_01_simple_pair() {
    let src = br#"{"a":1}"#;
    let (st, _) = run_inline_kv_json!(src, 8);
    assert_eq!(st.key_values.len(), 1);
    assert_eq!(txt(src, st.key_values[0].key), r#""a""#);
    assert_eq!(txt(src, st.key_values[0].value), "1");
    assert_eq!(txt(src, st.objects[0]), r#""a":1"#);
}

// 02. two pairs at one level, separated by the auto-trigger comma
#[test]
fn kvn_02_two_pairs() {
    let src = br#"{"a":1,"b":2}"#;
    let (st, _) = run_inline_kv_json!(src, 8);
    assert_eq!(st.key_values.len(), 2);
    assert_eq!(txt(src, st.key_values[0].key), r#""a""#);
    assert_eq!(txt(src, st.key_values[0].value), "1");
    assert_eq!(txt(src, st.key_values[1].key), r#""b""#);
    assert_eq!(txt(src, st.key_values[1].value), "2");
}

// 03. value is an array: value span includes brackets, array span is its content (containment)
#[test]
fn kvn_03_array_value_containment() {
    let src = br#"{"a":[1,2]}"#;
    let (st, _) = run_inline_kv_json!(src, 8);
    assert_eq!(st.key_values.len(), 1);
    assert_eq!(txt(src, st.key_values[0].value), "[1,2]");
    assert_eq!(st.autolinks.len(), 1);
    assert_eq!(txt(src, st.autolinks[0]), "1,2");
    let v = st.key_values[0].value;
    let a = st.autolinks[0];
    assert!(v.start <= a.start && a.end <= v.end); // array interval inside value interval
}

// 04. comma inside the array does NOT terminate the outer value (depth-aware)
#[test]
fn kvn_04_inner_comma_not_terminator() {
    let src = br#"{"a":[1,2],"b":3}"#;
    let (st, _) = run_inline_kv_json!(src, 8);
    assert_eq!(st.key_values.len(), 2);
    assert_eq!(txt(src, st.key_values[0].value), "[1,2]");
    assert_eq!(txt(src, st.key_values[1].value), "3");
}

// 05. nested object value; LIFO cascade on `}}` finalises inner then outer
#[test]
fn kvn_05_nested_object_value() {
    let src = br#"{"a":{"b":1}}"#;
    let (st, _) = run_inline_kv_json!(src, 8);
    assert_eq!(st.key_values.len(), 2);
    // inner pushed later, popped first on the first '}'
    assert_eq!(txt(src, st.key_values[0].key), r#""b""#);
    assert_eq!(txt(src, st.key_values[0].value), "1");
    assert_eq!(txt(src, st.key_values[1].key), r#""a""#);
    assert_eq!(txt(src, st.key_values[1].value), r#"{"b":1}"#);
}

// 06. string value with a `:` and `,` inside is opaque — not seen as eq/end
#[test]
fn kvn_06_string_value_opaque() {
    let src = br#"{"a":"x:y,z"}"#;
    let (st, _) = run_inline_kv_json!(src, 8);
    assert_eq!(st.key_values.len(), 1);
    assert_eq!(txt(src, st.key_values[0].value), r#""x:y,z""#);
}

// 07. key anchor after a comma resolves the next quoted key correctly
#[test]
fn kvn_07_key_anchor_after_comma() {
    let src = br#"{"aa":1,"bb":2}"#;
    let (st, _) = run_inline_kv_json!(src, 8);
    assert_eq!(txt(src, st.key_values[1].key), r#""bb""#);
}

// 08. spaces around ':' trimmed from key and value
#[test]
fn kvn_08_spaces_trimmed() {
    let src = br#"{ "a" : 1 ,"b":2}"#;
    let (st, _) = run_inline_kv_json!(src, 8);
    assert_eq!(txt(src, st.key_values[0].key), r#""a""#);
    assert_eq!(txt(src, st.key_values[0].value), "1 "); // trailing space before ',' kept (legacy semantics)
}

// 09. unclosed object: kv finalises its value to line end, object emits no span
#[test]
fn kvn_09_unclosed_object_lenient() {
    let src = br#"{"a":1"#;
    let (st, _) = run_inline_kv_json!(src, 8);
    assert_eq!(st.key_values.len(), 1);
    assert_eq!(txt(src, st.key_values[0].value), "1");
    assert!(st.objects.is_empty());
}

// 10. depth budget: a pair nested one object deep needs room for obj+kv+obj+kv
#[test]
fn kvn_10_depth_budget() {
    let src = br#"{"a":{"b":1}}"#;
    let (st_lo, _) = run_inline_kv_json!(src, 3); // too shallow: inner pair untracked
    let (st_hi, _) = run_inline_kv_json!(src, 8);
    assert_eq!(st_hi.key_values.len(), 2);
    assert!(st_lo.key_values.len() < 2); // documents the shared-budget consequence
}

// 11. empty value before a comma
#[test]
fn kvn_11_empty_value() {
    let src = br#"{"a":,"b":2}"#;
    let (st, _) = run_inline_kv_json!(src, 8);
    assert_eq!(txt(src, st.key_values[0].value), "");
}

// 12. multiple independent objects on one line
#[test]
fn kvn_12_two_objects() {
    let src = br#"{"a":1} {"b":2}"#;
    let (st, _) = run_inline_kv_json!(src, 8);
    assert_eq!(st.key_values.len(), 2);
    assert_eq!(st.objects.len(), 2);
}
