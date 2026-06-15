use super::common::*;

// ================================================================
// heading
// ================================================================

// 01. A single hash produces a level 1 heading
#[test]
fn heading_01_h1_level() {
    let src = b"# h";
    let (st, _) = run_line!(src, 0, src.len());
    assert_eq!(st.headings[0].0.level.get(), 1);
}

// 02. Two hashes produce a level 2 heading
#[test]
fn heading_02_h2_level() {
    let src = b"## h";
    let (st, _) = run_line!(src, 0, src.len());
    assert_eq!(st.headings[0].0.level.get(), 2);
}

// 03. Three hashes produce a level 3 heading
#[test]
fn heading_03_h3_level() {
    let src = b"### h";
    let (st, _) = run_line!(src, 0, src.len());
    assert_eq!(st.headings[0].0.level.get(), 3);
}

// 04. Four hashes produce a level 4 heading
#[test]
fn heading_04_h4_level() {
    let src = b"#### h";
    let (st, _) = run_line!(src, 0, src.len());
    assert_eq!(st.headings[0].0.level.get(), 4);
}

// 05. Five hashes produce a level 5 heading
#[test]
fn heading_05_h5_level() {
    let src = b"##### h";
    let (st, _) = run_line!(src, 0, src.len());
    assert_eq!(st.headings[0].0.level.get(), 5);
}

// 06. Six hashes produce a level 6 heading
#[test]
fn heading_06_h6_level() {
    let src = b"###### h";
    let (st, _) = run_line!(src, 0, src.len());
    assert_eq!(st.headings[0].0.level.get(), 6);
}

// 07. Seven hashes do not match a heading
#[test]
fn heading_07_seven_hashes_no_match() {
    let src = b"####### h";
    let (st, result) = run_line!(src, 0, src.len());
    assert!(result.is_none());
    assert!(st.headings.is_empty());
}

// 08. A valid heading returns Some with the continuation skip
#[test]
fn heading_08_returns_some_on_match() {
    let src = b"# h";
    let (_, result) = run_line!(src, 0, src.len());
    assert!(result.is_some());
}

// 09. Plain text without hashes returns None
#[test]
fn heading_09_returns_none_no_hash() {
    let src = b"text";
    let (_, result) = run_line!(src, 0, src.len());
    assert!(result.is_none());
}

// 10. A hash without a following space produces no match
#[test]
fn heading_10_no_space_after_hash_no_match() {
    let src = b"#nospace";
    let (st, result) = run_line!(src, 0, src.len());
    assert!(result.is_none());
    assert!(st.headings.is_empty());
}

// 11. The heading content span excludes the hashes and the space
#[test]
fn heading_11_span_excludes_hashes_and_space() {
    let src = b"# hello";
    let (st, _) = run_line!(src, 0, src.len());
    assert_eq!(txt(src, st.headings[0].1), "hello");
}

// 12. A level 1 heading span starts at index 2
#[test]
fn heading_12_span_start_h1() {
    let src = b"# hello";
    let (st, _) = run_line!(src, 0, src.len());
    assert_eq!(st.headings[0].1.start, 2);
}

// 13. A level 2 heading span starts at index 3
#[test]
fn heading_13_span_start_h2() {
    let src = b"## hello";
    let (st, _) = run_line!(src, 0, src.len());
    assert_eq!(st.headings[0].1.start, 3);
}

// 14. A level 6 heading span starts at index 7
#[test]
fn heading_14_span_start_h6() {
    let src = b"###### hello";
    let (st, _) = run_line!(src, 0, src.len());
    assert_eq!(st.headings[0].1.start, 7);
}

// 15. The heading span end matches the line end
#[test]
fn heading_15_span_end_equals_le() {
    let src = b"# hello";
    let (st, _) = run_line!(src, 0, src.len());
    assert_eq!(st.headings[0].1.end, src.len() as u32);
}

// 16. The continuation skip for a level 1 heading is 2
#[test]
fn heading_16_result_cs_h1() {
    let src = b"# hello";
    let (_, result) = run_line!(src, 0, src.len());
    assert_eq!(result, Some(2));
}

// 17. The continuation skip for a level 2 heading is 3
#[test]
fn heading_17_result_cs_h2() {
    let src = b"## hello";
    let (_, result) = run_line!(src, 0, src.len());
    assert_eq!(result, Some(3));
}

// 18. The continuation skip for a level 6 heading is 7
#[test]
fn heading_18_result_cs_h6() {
    let src = b"###### h";
    let (_, result) = run_line!(src, 0, src.len());
    assert_eq!(result, Some(7));
}

// 19. A hash without content or trailing space produces an empty span
#[test]
fn heading_19_hash_only_empty_content() {
    let src = b"#";
    let (st, result) = run_line!(src, 0, src.len());
    assert!(result.is_some());
    assert_eq!(st.headings[0].1.start, st.headings[0].1.end);
}

// 20. A hash with a space but no content produces an empty span
#[test]
fn heading_20_hash_space_empty_content() {
    let src = b"# ";
    let (st, result) = run_line!(src, 0, src.len());
    assert_eq!(result, Some(2));
    assert_eq!(st.headings[0].1.start, st.headings[0].1.end);
}

// 21. Hashes at the end of the line without content produce an empty span
#[test]
fn heading_21_hashes_without_content_at_eol() {
    let src = b"###";
    let (st, result) = run_line!(src, 0, src.len());
    assert!(result.is_some());
    assert_eq!(st.headings[0].0.level.get(), 3);
    assert_eq!(st.headings[0].1.start, st.headings[0].1.end);
}

// 22. A heading is successfully parsed when starting from a non-zero offset
#[test]
fn heading_22_pos_nonzero() {
    let src = b"text # hello";
    let (st, _) = run_line!(src, 5, src.len());
    assert_eq!(st.headings[0].0.level.get(), 1);
    assert_eq!(txt(src, st.headings[0].1), "hello");
}

// 23. Processing at or beyond the line end produces no match
#[test]
fn heading_23_pos_ge_le_no_match() {
    let src = b"# h";
    let (st, result) = run_line!(src, src.len(), src.len());
    assert!(result.is_none());
    assert!(st.headings.is_empty());
}

// 24. A valid heading does not push a thematic break
#[test]
fn heading_24_does_not_push_thematic_break() {
    let src = b"# h";
    let (st, _) = run_line!(src, 0, src.len());
    assert!(st.thematic_breaks.is_empty());
}

// 25. The heading span satisfies start <= end
#[test]
fn heading_25_span_start_le_end() {
    let src = b"# hello";
    let (st, _) = run_line!(src, 0, src.len());
    assert!(st.headings[0].1.start <= st.headings[0].1.end);
}

// 26. All heading levels from 1 to 6 are parsed correctly via the macro
#[test]
fn heading_26_all_levels_correct() {
    use std::num::NonZeroU8;
    use {Heading, ThematicBreak};
    for lvl in 1u8..=6 {
        let s = format!("{} h", "#".repeat(lvl as usize));
        let src = s.as_bytes();
        let mut st = ParseState::new(src.len());
        meon::parse_line!(
            st, src, 0, src.len(), sep = b' ';
            line(b'#', max = 6) |n|:
                Heading { level: NonZeroU8::new(n).unwrap_or(NonZeroU8::MIN) }
                => headings;
            line_simple(b'-' | b'*' | b'_', min = 3) |b|:
                ThematicBreak { kind: b }
                => thematic_breaks;
        );
        assert_eq!(st.headings[0].0.level.get(), lvl, "level {}", lvl);
    }
}

// ================================================================
// thematic_breaks
// ================================================================

// 01. Three dashes are recognized as a thematic break kind
#[test]
fn tb_01_dash_kind() {
    let src = b"---";
    let (st, _) = run_line!(src, 0, src.len());
    assert_eq!(st.thematic_breaks[0].0.kind, b'-');
}

// 02. Three stars are recognized as a thematic break kind
#[test]
fn tb_02_star_kind() {
    let src = b"***";
    let (st, _) = run_line!(src, 0, src.len());
    assert_eq!(st.thematic_breaks[0].0.kind, b'*');
}

// 03. Three underscores are recognized as a thematic break kind
#[test]
fn tb_03_underscore_kind() {
    let src = b"___";
    let (st, _) = run_line!(src, 0, src.len());
    assert_eq!(st.thematic_breaks[0].0.kind, b'_');
}

// 04. Exactly three marker characters produce a thematic break
#[test]
fn tb_04_exactly_three_match() {
    let src = b"---";
    let (st, _) = run_line!(src, 0, src.len());
    assert_eq!(st.thematic_breaks.len(), 1);
}

// 05. Two dashes do not produce a thematic break
#[test]
fn tb_05_two_dashes_no_match() {
    let src = b"--";
    let (st, result) = run_line!(src, 0, src.len());
    assert!(result.is_none());
    assert!(st.thematic_breaks.is_empty());
}

// 06. Two stars do not produce a thematic break
#[test]
fn tb_06_two_stars_no_match() {
    let src = b"**";
    let (st, result) = run_line!(src, 0, src.len());
    assert!(result.is_none());
    assert!(st.thematic_breaks.is_empty());
}

// 07. Four dashes successfully match as a thematic break
#[test]
fn tb_07_four_dashes_match() {
    let src = b"----";
    let (st, _) = run_line!(src, 0, src.len());
    assert_eq!(st.thematic_breaks.len(), 1);
}

// 08. Spaced dashes successfully match as a thematic break
#[test]
fn tb_08_spaced_dashes() {
    let src = b"- - -";
    let (st, _) = run_line!(src, 0, src.len());
    assert_eq!(st.thematic_breaks[0].0.kind, b'-');
}

// 09. Spaced stars successfully match as a thematic break
#[test]
fn tb_09_spaced_stars() {
    let src = b"* * *";
    let (st, _) = run_line!(src, 0, src.len());
    assert_eq!(st.thematic_breaks[0].0.kind, b'*');
}

// 10. Spaced underscores successfully match as a thematic break
#[test]
fn tb_10_spaced_underscores() {
    let src = b"_ _ _";
    let (st, _) = run_line!(src, 0, src.len());
    assert_eq!(st.thematic_breaks[0].0.kind, b'_');
}

// 11. Mixed dashes and underscores do not match
#[test]
fn tb_11_mixed_dash_underscore_no_match() {
    let src = b"-_-";
    let (st, result) = run_line!(src, 0, src.len());
    assert!(result.is_none());
    assert!(st.thematic_breaks.is_empty());
}

// 12. Mixed stars and dashes do not match
#[test]
fn tb_12_mixed_star_dash_no_match() {
    let src = b"-*-";
    let (st, result) = run_line!(src, 0, src.len());
    assert!(result.is_none());
    assert!(st.thematic_breaks.is_empty());
}

// 13. The thematic break span start matches the initial position
#[test]
fn tb_13_span_start_equals_pos() {
    let src = b"---";
    let (st, _) = run_line!(src, 0, src.len());
    assert_eq!(st.thematic_breaks[0].1.start, 0);
}

// 14. The thematic break span end matches the line end
#[test]
fn tb_14_span_end_equals_le() {
    let src = b"---";
    let (st, _) = run_line!(src, 0, src.len());
    assert_eq!(st.thematic_breaks[0].1.end, src.len() as u32);
}

// 15. A thematic break returns the line end as the continuation skip
#[test]
fn tb_15_result_is_le() {
    let src = b"---";
    let (_, result) = run_line!(src, 0, src.len());
    assert_eq!(result, Some(src.len()));
}

// 16. The thematic break kind is determined by its first byte
#[test]
fn tb_16_kind_equals_first_byte() {
    let src = b"***";
    let (st, _) = run_line!(src, 0, src.len());
    assert_eq!(st.thematic_breaks[0].0.kind, src[0]);
}

// 17. A thematic break does not push a heading span
#[test]
fn tb_17_does_not_push_heading() {
    let src = b"---";
    let (st, _) = run_line!(src, 0, src.len());
    assert!(st.headings.is_empty());
}

// 18. A heading structure takes priority over a thematic break
#[test]
fn tb_18_heading_takes_priority() {
    let src = b"### h";
    let (st, _) = run_line!(src, 0, src.len());
    assert!(st.thematic_breaks.is_empty());
    assert_eq!(st.headings.len(), 1);
}

// 19. A line with only spaces produces no match
#[test]
fn tb_19_spaces_only_no_match() {
    let src = b"   ";
    let (st, result) = run_line!(src, 0, src.len());
    assert!(result.is_none());
    assert!(st.thematic_breaks.is_empty());
}

// 20. Processing beyond the line end produces no match
#[test]
fn tb_20_pos_ge_le_no_match() {
    let src = b"---";
    let (st, result) = run_line!(src, 3, 3);
    assert!(result.is_none());
    assert!(st.thematic_breaks.is_empty());
}

// 21. A thematic break is successfully parsed when starting from a non-zero offset
#[test]
fn tb_21_pos_nonzero() {
    let src = b"  ---";
    let (st, _) = run_line!(src, 2, src.len());
    assert_eq!(st.thematic_breaks[0].0.kind, b'-');
}

// 22. The span start matches the non-zero initial position
#[test]
fn tb_22_pos_nonzero_span_start_equals_pos() {
    let src = b"  ---";
    let (st, _) = run_line!(src, 2, src.len());
    assert_eq!(st.thematic_breaks[0].1.start, 2);
}

// 23. Five spaced dashes match as a thematic break
#[test]
fn tb_23_five_dashes_with_spaces() {
    let src = b"- - - - -";
    let (st, _) = run_line!(src, 0, src.len());
    assert_eq!(st.thematic_breaks.len(), 1);
}

// 24. Double spaces between markers are permitted
#[test]
fn tb_24_double_spaces_between() {
    let src = b"-  -  -";
    let (st, _) = run_line!(src, 0, src.len());
    assert_eq!(st.thematic_breaks.len(), 1);
}

// 25. The returned result points exactly to the line end without an offset
#[test]
fn tb_25_result_is_le_not_le_plus_one() {
    let src = b"- - -";
    let (_, result) = run_line!(src, 0, src.len());
    assert_eq!(result, Some(src.len()));
    assert_ne!(result, Some(src.len() + 1));
}
