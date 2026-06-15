use super::common::*;

// ================================================================
// bullet
// ================================================================

// 01. A dash marker is recognized as a bullet item kind
#[test]
fn bullet_01_dash_kind() {
    let src = b"- item";
    let mut active = None;
    let (st, _) = run_block!(&mut active, src, 0, src.len());
    assert_eq!(st.bullet_items[0].0.kind, b'-');
}

// 02. A star marker is recognized as a bullet item kind
#[test]
fn bullet_02_star_kind() {
    let src = b"* item";
    let mut active = None;
    let (st, _) = run_block!(&mut active, src, 0, src.len());
    assert_eq!(st.bullet_items[0].0.kind, b'*');
}

// 03. A plus marker is recognized as a bullet item kind
#[test]
fn bullet_03_plus_kind() {
    let src = b"+ item";
    let mut active = None;
    let (st, _) = run_block!(&mut active, src, 0, src.len());
    assert_eq!(st.bullet_items[0].0.kind, b'+');
}

// 04. A missing space after the marker produces no bullet item
#[test]
fn bullet_04_requires_space_after_marker() {
    let src = b"-noitem";
    let mut active = None;
    let (st, result) = run_block!(&mut active, src, 0, src.len());
    assert!(result.is_none());
    assert!(st.bullet_items.is_empty());
}

// 05. A tab character serves as a valid separator after the marker
#[test]
fn bullet_05_tab_after_marker_matches() {
    let src = b"-\titem";
    let mut active = None;
    let (st, _) = run_block!(&mut active, src, 0, src.len());
    assert_eq!(st.bullet_items.len(), 1);
}

// 06. The span start points right after the marker and separator
#[test]
fn bullet_06_span_start_after_marker_and_sep() {
    let src = b"- hello";
    let mut active = None;
    let (st, _) = run_block!(&mut active, src, 0, src.len());
    assert_eq!(st.bullet_items[0].1.start, 2);
}

// 07. The span end matches the line end
#[test]
fn bullet_07_span_end_is_le() {
    let src = b"- hello";
    let mut active = None;
    let (st, _) = run_block!(&mut active, src, 0, src.len());
    assert_eq!(st.bullet_items[0].1.end, src.len() as u32);
}

// 08. The bullet content excludes the marker and separator
#[test]
fn bullet_08_content() {
    let src = b"- hello";
    let mut active = None;
    let (st, _) = run_block!(&mut active, src, 0, src.len());
    assert_eq!(txt(src, st.bullet_items[0].1), "hello");
}

// 09. A successfully parsed bullet returns an opened state of true
#[test]
fn bullet_09_result_opened_true() {
    let src = b"- item";
    let mut active = None;
    let (_, result) = run_block!(&mut active, src, 0, src.len());
    assert_eq!(result, Some((true, 2)));
}

// 10. The continuation skip is two bytes when separated by a space
#[test]
fn bullet_10_result_cs_is_two() {
    let src = b"- x";
    let mut active = None;
    let (_, result) = run_block!(&mut active, src, 0, src.len());
    assert_eq!(result.unwrap().1, 2);
}

// 11. Bullet markers are recognized even when preceded by spaces
#[test]
fn bullet_11_pos_nonzero() {
    let src = b"  - item";
    let mut active = None;
    let (st, _) = run_block!(&mut active, src, 2, src.len());
    assert_eq!(st.bullet_items[0].0.kind, b'-');
}

// 12. Processing beyond the line end produces no match
#[test]
fn bullet_12_pos_ge_le_no_match() {
    let src = b"- item";
    let mut active = None;
    let (st, result) = run_block!(&mut active, src, src.len(), src.len());
    assert!(result.is_none());
    assert!(st.bullet_items.is_empty());
}

// ================================================================
// ordered
// ================================================================

// 01. A dot is recognized as an ordered item kind
#[test]
fn ordered_01_dot_kind() {
    let src = b"1. item";
    let mut active = None;
    let (st, _) = run_block!(&mut active, src, 0, src.len());
    assert_eq!(st.ordered_items[0].0.kind, b'.');
}

// 02. A parenthesis is recognized as an ordered item kind
#[test]
fn ordered_02_paren_kind() {
    let src = b"1) item";
    let mut active = None;
    let (st, _) = run_block!(&mut active, src, 0, src.len());
    assert_eq!(st.ordered_items[0].0.kind, b')');
}

// 03. Parses the numeric value correctly for single digits
#[test]
fn ordered_03_num_one() {
    let src = b"1. item";
    let mut active = None;
    let (st, _) = run_block!(&mut active, src, 0, src.len());
    assert_eq!(st.ordered_items[0].0.num, 1);
}

// 04. Zero is successfully parsed as a valid ordered item number
#[test]
fn ordered_04_num_zero() {
    let src = b"0. item";
    let mut active = None;
    let (st, _) = run_block!(&mut active, src, 0, src.len());
    assert_eq!(st.ordered_items[0].0.num, 0);
}

// 05. Parses multi-digit numbers correctly
#[test]
fn ordered_05_num_multidigit() {
    let src = b"42. item";
    let mut active = None;
    let (st, _) = run_block!(&mut active, src, 0, src.len());
    assert_eq!(st.ordered_items[0].0.num, 42);
}

// 06. Parses the maximum allowed limit of nine consecutive digits
#[test]
fn ordered_06_max_nine_digits() {
    let src = b"123456789. item";
    let mut active = None;
    let (st, _) = run_block!(&mut active, src, 0, src.len());
    assert_eq!(st.ordered_items[0].0.num, 123456789);
}

// 07. Rejects ordered lists lacking a space after the delimiter
#[test]
fn ordered_07_requires_space_after_end() {
    let src = b"1.item";
    let mut active = None;
    let (st, result) = run_block!(&mut active, src, 0, src.len());
    assert!(result.is_none());
    assert!(st.ordered_items.is_empty());
}

// 08. Accepts a tab character as a valid separator after the delimiter
#[test]
fn ordered_08_tab_after_end_matches() {
    let src = b"1.\titem";
    let mut active = None;
    let (st, _) = run_block!(&mut active, src, 0, src.len());
    assert_eq!(st.ordered_items.len(), 1);
}

// 09. The ordered content excludes the number, delimiter, and separator
#[test]
fn ordered_09_content() {
    let src = b"1. hello";
    let mut active = None;
    let (st, _) = run_block!(&mut active, src, 0, src.len());
    assert_eq!(txt(src, st.ordered_items[0].1), "hello");
}

// 10. A successfully parsed ordered item returns an opened state of true
#[test]
fn ordered_10_result_opened_true() {
    let src = b"1. item";
    let mut active = None;
    let (_, result) = run_block!(&mut active, src, 0, src.len());
    assert_eq!(result.map(|(o, _)| o), Some(true));
}

// 11. A delimiter without preceding digits produces no match
#[test]
fn ordered_11_no_digits_no_match() {
    let src = b". item";
    let mut active = None;
    let (st, result) = run_block!(&mut active, src, 0, src.len());
    assert!(result.is_none());
    assert!(st.ordered_items.is_empty());
}

// 12. The span end matches the line end
#[test]
fn ordered_12_span_end_is_le() {
    let src = b"1. hello";
    let mut active = None;
    let (st, _) = run_block!(&mut active, src, 0, src.len());
    assert_eq!(st.ordered_items[0].1.end, src.len() as u32);
}

// ================================================================
// fence
// ================================================================

// 01. Opening a fence with backticks sets the active state
#[test]
fn fence_01_open_backtick_sets_active() {
    let src = b"```";
    let mut active = None;
    run_block!(&mut active, src, 0, src.len());
    assert!(active.is_some());
    assert_eq!(active.unwrap().0, 0u8);
}

// 02. The active state stores the backtick as the marker byte
#[test]
fn fence_02_open_stores_byte_backtick() {
    let src = b"```";
    let mut active = None;
    run_block!(&mut active, src, 0, src.len());
    assert_eq!(active.unwrap().1, b'`');
}

// 03. The active state stores the correct consecutive marker count of three
#[test]
fn fence_03_open_stores_count_three() {
    let src = b"```";
    let mut active = None;
    run_block!(&mut active, src, 0, src.len());
    assert_eq!(active.unwrap().2, 3);
}

// 04. The active state stores the correct consecutive marker count of four
#[test]
fn fence_04_open_stores_count_four() {
    let src = b"````";
    let mut active = None;
    run_block!(&mut active, src, 0, src.len());
    assert_eq!(active.unwrap().2, 4);
}

// 05. A successfully opened fence returns an opened state of true
#[test]
fn fence_05_open_result_opened_true() {
    let src = b"```";
    let mut active = None;
    let (_, result) = run_block!(&mut active, src, 0, src.len());
    assert_eq!(result.map(|(o, _)| o), Some(true));
}

// 06. Two backticks are insufficient to open a fence block
#[test]
fn fence_06_two_backticks_no_open() {
    let src = b"``";
    let mut active = None;
    run_block!(&mut active, src, 0, src.len());
    assert!(active.is_none());
}

// 07. A backtick inside the info string prevents the fence from opening
#[test]
fn fence_07_open_backtick_with_backtick_in_info_no_open() {
    let src = b"``` `x`";
    let mut active = None;
    run_block!(&mut active, src, 0, src.len());
    assert!(active.is_none());
}

// 08. The active state records the starting position of the fence
#[test]
fn fence_08_open_stores_start_pos() {
    let src = b"```";
    let mut active = None;
    run_block!(&mut active, src, 0, src.len());
    assert_eq!(active.unwrap().3, 0u32);
}

// 09. A closing fence matching the active parameters pushes a new span
#[test]
fn fence_09_close_matching_pushes_span() {
    let mut active = Some((0u8, b'`', 3u8, 0u32));
    let src = b"```";
    let (st, _) = run_block!(&mut active, src, 0, src.len());
    assert_eq!(st.fenced_codes.len(), 1);
    assert!(active.is_none());
}

// 10. A closing fence with more markers than the opening successfully closes the block
#[test]
fn fence_10_close_longer_count_closes() {
    let mut active = Some((0u8, b'`', 3u8, 0u32));
    let src = b"````";
    let (st, _) = run_block!(&mut active, src, 0, src.len());
    assert_eq!(st.fenced_codes.len(), 1);
}

// 11. A closing fence with fewer markers than the opening is ignored
#[test]
fn fence_11_close_shorter_count_no_close() {
    let mut active = Some((0u8, b'`', 4u8, 0u32));
    let src = b"```";
    let (st, _) = run_block!(&mut active, src, 0, src.len());
    assert!(st.fenced_codes.is_empty());
    assert!(active.is_some());
}

// 12. A closing fence with a different marker byte is ignored
#[test]
fn fence_12_close_wrong_byte_no_close() {
    let mut active = Some((0u8, b'~', 3u8, 0u32));
    let src = b"```";
    let (st, _) = run_block!(&mut active, src, 0, src.len());
    assert!(st.fenced_codes.is_empty());
}

// 13. The resulting span starts at the original opening position recorded in the active state
#[test]
fn fence_13_close_span_start_is_active_start() {
    let mut active = Some((0u8, b'`', 3u8, 5u32));
    let src = b"```";
    let (st, _) = run_block!(&mut active, src, 0, src.len());
    assert_eq!(st.fenced_codes[0].start, 5);
}

// 14. The active state is cleared upon successfully closing a fence
#[test]
fn fence_14_close_active_cleared() {
    let mut active = Some((0u8, b'`', 3u8, 0u32));
    let src = b"```";
    run_block!(&mut active, src, 0, src.len());
    assert!(active.is_none());
}

// 15. Closing a fence returns an opened state of false
#[test]
fn fence_15_close_result_opened_false() {
    let mut active = Some((0u8, b'`', 3u8, 0u32));
    let src = b"```";
    let (_, result) = run_block!(&mut active, src, 0, src.len());
    assert_eq!(result.map(|(o, _)| o), Some(false));
}

// 16. Processing a normal content line leaves the active state unchanged
#[test]
fn fence_16_content_line_active_unchanged() {
    let mut active = Some((0u8, b'`', 3u8, 0u32));
    let src = b"some code";
    let (st, result) = run_block!(&mut active, src, 0, src.len());
    assert!(st.fenced_codes.is_empty());
    assert!(active.is_some());
    assert_eq!(result.map(|(o, _)| o), Some(false));
}

// ================================================================
// cont
// ================================================================

// 01. Opening a continuation sets the active discriminator and marker byte
#[test]
fn cont_01_open_sets_active_disc_and_byte() {
    let src = b"> text";
    let mut active = None;
    run_block!(&mut active, src, 0, src.len());
    let (disc, byte, _, _) = active.unwrap();
    assert_eq!(disc, 1u8);
    assert_eq!(byte, b'>');
}

// 02. Opening a continuation returns an opened state of true
#[test]
fn cont_02_open_result_opened_true() {
    let src = b"> text";
    let mut active = None;
    let (_, result) = run_block!(&mut active, src, 0, src.len());
    assert_eq!(result.map(|(o, _)| o), Some(true));
}

// 03. The continuation skip is two bytes when followed by a space
#[test]
fn cont_03_open_with_space_cs_is_two() {
    let src = b"> text";
    let mut active = None;
    let (_, result) = run_block!(&mut active, src, 0, src.len());
    assert_eq!(result.unwrap().1, 2);
}

// 04. The continuation skip is one byte when immediately followed by text
#[test]
fn cont_04_open_without_space_cs_is_one() {
    let src = b">text";
    let mut active = None;
    let (_, result) = run_block!(&mut active, src, 0, src.len());
    assert_eq!(result.unwrap().1, 1);
}

// 05. The active state stores the correct starting position
#[test]
fn cont_05_open_stores_start_pos() {
    let src = b"> text";
    let mut active = None;
    run_block!(&mut active, src, 0, src.len());
    assert_eq!(active.unwrap().3, 0u32);
}

// 06. Continuing a block with the same marker does not immediately produce a span
#[test]
fn cont_06_continue_same_byte_no_span_yet() {
    let mut active = Some((1u8, b'>', 0u8, 0u32));
    let src = b"> next";
    let (st, result) = run_block!(&mut active, src, 0, src.len());
    assert!(st.blockquotes.is_empty());
    assert_eq!(result.map(|(o, _)| o), Some(false));
}

// 07. Continuing a block calculates the continuation skip correctly with spaces
#[test]
fn cont_07_continue_cs_with_space() {
    let mut active = Some((1u8, b'>', 0u8, 0u32));
    let src = b"> next";
    let (_, result) = run_block!(&mut active, src, 0, src.len());
    assert_eq!(result.unwrap().1, 2);
}

// 08. Closing a continuation pushes a span when the marker is absent
#[test]
fn cont_08_close_non_gt_pushes_span() {
    let mut active = Some((1u8, b'>', 0u8, 0u32));
    let src = b"text";
    let (st, _) = run_block!(&mut active, src, 0, src.len());
    assert_eq!(st.blockquotes.len(), 1);
}

// 09. The active state is cleared upon closing a continuation block
#[test]
fn cont_09_close_active_cleared() {
    let mut active = Some((1u8, b'>', 0u8, 0u32));
    let src = b"text";
    run_block!(&mut active, src, 0, src.len());
    assert!(active.is_none());
}

// 10. Closing a continuation returns no result
#[test]
fn cont_10_close_result_is_none() {
    let mut active = Some((1u8, b'>', 0u8, 0u32));
    let src = b"text";
    let (_, result) = run_block!(&mut active, src, 0, src.len());
    assert!(result.is_none());
}

// 11. The generated span spans from the original active start to the current line end
#[test]
fn cont_11_close_span_start_is_active_start() {
    let src = b"    text";
    let mut active = Some((1u8, b'>', 0u8, 0u32));
    let (st, _) = run_block!(&mut active, src, 4, src.len());
    assert_eq!(st.blockquotes[0].start, 0);
    assert_eq!(st.blockquotes[0].end, 4);
}
