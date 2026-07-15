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

// ================================================================
// nesting
// ================================================================

// 01. A double blockquote marker opens two nested continuation frames
#[test]
fn nest_01_double_cont_opens_two_frames() {
    let src = b"> > text";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;
    run_block_nested!(stack, depth, src, 0, src.len(), 4);
    assert_eq!(depth, 2);
}

// 02. The stack correctly stores the marker byte for both nested continuation frames
#[test]
fn nest_02_double_cont_stack_bytes() {
    let src = b"> > text";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;
    run_block_nested!(stack, depth, src, 0, src.len(), 4);
    assert_eq!(stack[0].1, b'>');
    assert_eq!(stack[1].1, b'>');
}

// 03. Continuing a double blockquote maintains the nesting depth
#[test]
fn nest_03_double_cont_continue_maintains_depth() {
    let src = b"> > next";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    stack[0] = (1, b'>', 0, 0);
    stack[1] = (1, b'>', 0, 2);
    let mut depth = 2;
    run_block_nested!(stack, depth, src, 0, src.len(), 4);
    assert_eq!(depth, 2);
}

// 04. Dropping the inner blockquote marker closes the inner frame and pushes a span
#[test]
fn nest_04_double_cont_drop_inner_closes_inner() {
    let src = b"> text";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    stack[0] = (1, b'>', 0, 0);
    stack[1] = (1, b'>', 0, 2);
    let mut depth = 2;
    let (st, _) = run_block_nested!(stack, depth, src, 0, src.len(), 4);
    assert_eq!(depth, 1);
    assert_eq!(st.blockquotes.len(), 1);
}

// 05. Dropping all blockquote markers closes both frames and pushes two spans
#[test]
fn nest_05_double_cont_drop_all_closes_both() {
    let src = b"text";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    stack[0] = (1, b'>', 0, 0);
    stack[1] = (1, b'>', 0, 2);
    let mut depth = 2;
    let (st, _) = run_block_nested!(stack, depth, src, 0, src.len(), 4);
    assert_eq!(depth, 0);
    assert_eq!(st.blockquotes.len(), 2);
}

// 06. A bullet item can open inside a continuation block when max_nest > 1
#[test]
fn nest_06_bullet_inside_cont() {
    let src = b"> - item";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;
    let (st, _) = run_block_nested!(stack, depth, src, 0, src.len(), 4);
    assert_eq!(depth, 1);
    assert_eq!(st.bullet_items.len(), 1);
}

// 07. An ordered item can open inside a continuation block
#[test]
fn nest_07_ordered_inside_cont() {
    let src = b"> 1. item";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;
    let (st, _) = run_block_nested!(stack, depth, src, 0, src.len(), 4);
    assert_eq!(depth, 1);
    assert_eq!(st.ordered_items.len(), 1);
}

// 08. A fenced code block can open inside a continuation block
#[test]
fn nest_08_fence_inside_cont_open() {
    let src = b"> ```";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;
    run_block_nested!(stack, depth, src, 0, src.len(), 4);
    assert_eq!(depth, 2);
    assert_eq!(stack[1].0, 0u8);
}

// 09. Content inside a nested fence is consumed and does not close the outer continuation
#[test]
fn nest_09_fence_inside_cont_content() {
    let src = b"> code";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    stack[0] = (1, b'>', 0, 0);
    stack[1] = (0, b'`', 3, 2);
    let mut depth = 2;
    let (st, _) = run_block_nested!(stack, depth, src, 0, src.len(), 4);
    assert_eq!(depth, 2);
    assert!(st.blockquotes.is_empty());
}

// 10. A matching fence closes the inner fence but leaves the outer continuation open
#[test]
fn nest_10_fence_inside_cont_close() {
    let src = b"> ```";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    stack[0] = (1, b'>', 0, 0);
    stack[1] = (0, b'`', 3, 2);
    let mut depth = 2;
    let (st, _) = run_block_nested!(stack, depth, src, 0, src.len(), 4);
    assert_eq!(depth, 1);
    assert_eq!(st.fenced_codes.len(), 1);
}

// 11. A continuation marker inside an open fence does not open a new continuation frame
#[test]
fn nest_11_cont_inside_fence_no_open() {
    let src = b"> text";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    stack[0] = (0, b'`', 3, 0);
    let mut depth = 1;
    run_block_nested!(stack, depth, src, 0, src.len(), 4);
    assert_eq!(depth, 1);
}

// 12. A triple blockquote marker opens three nested continuation frames
#[test]
fn nest_12_triple_cont_opens_three_frames() {
    let src = b"> > > text";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;
    run_block_nested!(stack, depth, src, 0, src.len(), 4);
    assert_eq!(depth, 3);
}

// 13. Dropping the innermost marker of a triple blockquote closes only the innermost frame
#[test]
fn nest_13_triple_cont_drop_innermost() {
    let src = b"> > text";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    stack[0] = (1, b'>', 0, 0);
    stack[1] = (1, b'>', 0, 2);
    stack[2] = (1, b'>', 0, 4);
    let mut depth = 3;
    let (st, _) = run_block_nested!(stack, depth, src, 0, src.len(), 4);
    assert_eq!(depth, 2);
    assert_eq!(st.blockquotes.len(), 1);
}

// 14. max_nest limit prevents opening a third continuation frame when max_nest = 2
#[test]
fn nest_14_max_nest_limits_cont_depth() {
    let src = b"> > > text";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;
    run_block_nested!(stack, depth, src, 0, src.len(), 2);
    assert_eq!(depth, 2);
}

// 15. At max_nest = 1, a bullet does NOT open inside a freshly-opened continuation.
//     The `>` opens one cont frame (depth 1 == max_nest); the leaf is then gated
//     out and `- item` is left for inline. This is exactly the pre-nesting
//     behaviour and is what the `@open_block` depth gate guarantees.
#[test]
fn nest_15_no_bullet_inside_cont_max_nest_1() {
    let src = b"> - item";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;
    let (st, _) = run_block_nested!(stack, depth, src, 0, src.len(), 1);
    assert_eq!(depth, 1);
    assert!(st.bullet_items.is_empty());
}

// 16. An outer continuation closing forcibly closes an inner fenced code block
#[test]
fn nest_16_outer_cont_closes_inner_fence() {
    let src = b"text";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    stack[0] = (1, b'>', 0, 0);
    stack[1] = (0, b'`', 3, 2);
    let mut depth = 2;
    let (st, _) = run_block_nested!(stack, depth, src, 0, src.len(), 4);
    assert_eq!(depth, 0);
    assert_eq!(st.blockquotes.len(), 1);
    assert_eq!(st.fenced_codes.len(), 1);
}

// 17. An outer continuation closing forcibly closes an inner continuation block
#[test]
fn nest_17_outer_cont_closes_inner_cont() {
    let src = b"text";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    stack[0] = (1, b'>', 0, 0);
    stack[1] = (1, b'>', 0, 2);
    let mut depth = 2;
    let (st, _) = run_block_nested!(stack, depth, src, 0, src.len(), 4);
    assert_eq!(depth, 0);
    assert_eq!(st.blockquotes.len(), 2);
}

// 18. A fence closing does NOT close the outer continuation block
#[test]
fn nest_18_inner_fence_close_keeps_outer_cont() {
    let src = b"> ```";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    stack[0] = (1, b'>', 0, 0);
    stack[1] = (0, b'`', 3, 2);
    let mut depth = 2;
    let (st, _) = run_block_nested!(stack, depth, src, 0, src.len(), 4);
    assert_eq!(depth, 1);
    assert_eq!(st.fenced_codes.len(), 1);
    assert!(st.blockquotes.is_empty());
}

// 19. Multiple bullet items can sequentially open inside an open continuation block
#[test]
fn nest_19_multiple_bullets_inside_cont() {
    let src1 = b"> - item 1";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;
    let (st1, _) = run_block_nested!(stack, depth, src1, 0, src1.len(), 4);
    assert_eq!(st1.bullet_items.len(), 1);

    let src2 = b"> - item 2";
    let (st2, _) = run_block_nested!(stack, depth, src2, 0, src2.len(), 4);
    assert_eq!(depth, 1);
    assert_eq!(st2.bullet_items.len(), 1);
}

// 20. A continuation block correctly calculates the nested continuation skip offset
#[test]
fn nest_20_double_cont_skip_offset() {
    let src = b"> > text";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;
    let (_, result) = run_block_nested!(stack, depth, src, 0, src.len(), 4);
    assert_eq!(result.unwrap().1, 4);
}

// ================================================================
// nesting — boundaries and containment
// ================================================================

// 21. A double continuation creates two spans where the outer contains the inner (interval containment)
#[test]
fn nest_21_double_cont_outer_contains_inner() {
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    stack[0] = (1, b'>', 0, 0);
    stack[1] = (1, b'>', 0, 2);
    let mut depth = 2;
    let (st, _) = run_block_nested!(stack, depth, b"next", 0, 4, 4);
    assert_eq!(st.blockquotes.len(), 2);
    // Frames close innermost-first, so the entry with the larger start is the inner.
    let (outer, inner) = if st.blockquotes[0].start < st.blockquotes[1].start {
        (&st.blockquotes[0], &st.blockquotes[1])
    } else {
        (&st.blockquotes[1], &st.blockquotes[0])
    };
    assert!(outer.start <= inner.start);
    assert!(outer.end >= inner.end);
}

// 22. A triple continuation creates three spans whose starts are 0, 2, 4
#[test]
fn nest_22_triple_cont_spans_sorted_by_start() {
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    stack[0] = (1, b'>', 0, 0);
    stack[1] = (1, b'>', 0, 2);
    stack[2] = (1, b'>', 0, 4);
    let mut depth = 3;
    let (st, _) = run_block_nested!(stack, depth, b"text", 0, 4, 4);
    assert_eq!(st.blockquotes.len(), 3);
    let mut starts: Vec<u32> = st.blockquotes.iter().map(|s| s.start).collect();
    starts.sort();
    assert_eq!(starts, vec![0, 2, 4]);
}

// 23. A fence opened inside a continuation has its span start after the continuation markers
#[test]
fn nest_23_fence_inside_cont_span_start_after_cont_markers() {
    let src = b"> ```";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;
    run_block_nested!(stack, depth, src, 0, src.len(), 4);
    assert_eq!(stack[1].3, 2);
}

// 24. A fence closed inside a continuation has its span end at the closing fence line
#[test]
fn nest_24_fence_inside_cont_span_end_at_close_fence() {
    let src = b"> ```";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    stack[0] = (1, b'>', 0, 0);
    stack[1] = (0, b'`', 3, 2);
    let mut depth = 2;
    let (st, _) = run_block_nested!(stack, depth, src, 0, src.len(), 4);
    assert_eq!(st.fenced_codes[0].end, 5);
}

// ================================================================
// nesting — regression (max_nest = 1)
// ================================================================

// 25. At max_nest = 1, a double continuation marker opens only one frame (collapsing behavior)
#[test]
fn nest_25_max_nest_1_double_cont_opens_one_frame() {
    let src = b"> > text";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;
    run_block_nested!(stack, depth, src, 0, src.len(), 1);
    assert_eq!(depth, 1);
}

// 26. At max_nest = 1, a fence inside a continuation does NOT open (no nesting allowed)
#[test]
fn nest_26_max_nest_1_fence_inside_cont_no_open() {
    let src = b"> ```";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;
    run_block_nested!(stack, depth, src, 0, src.len(), 1);
    assert_eq!(depth, 1);
    assert_eq!(stack[0].0, 1u8);
}

// 27. At max_nest = 1, an ordered item does NOT open inside a freshly-opened continuation
//     (same gate as nest_15: the `>` fills the single depth slot, the leaf is left for inline).
#[test]
fn nest_27_max_nest_1_ordered_inside_cont_no_open() {
    let src = b"> 1. item";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;
    let (st, _) = run_block_nested!(stack, depth, src, 0, src.len(), 1);
    assert_eq!(depth, 1);
    assert!(st.ordered_items.is_empty());
}

// 28. At max_nest = 1, a standalone fence still opens correctly (regression)
#[test]
fn nest_28_max_nest_1_fence_still_opens() {
    let src = b"```";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;
    run_block_nested!(stack, depth, src, 0, src.len(), 1);
    assert_eq!(depth, 1);
    assert_eq!(stack[0].0, 0u8);
}

// ================================================================
// nesting — edge cases
// ================================================================

// 29. A stray continuation marker (extra >) beyond the current depth is treated as content
#[test]
fn nest_29_stray_cont_marker_beyond_depth() {
    let src = b"> > > text";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    stack[0] = (1, b'>', 0, 0);
    let mut depth = 1;
    let (st, _) = run_block_nested!(stack, depth, src, 0, src.len(), 2);
    assert_eq!(depth, 2);
    assert_eq!(st.blockquotes.len(), 0);
}

// 30. Multiple independent bullet groups inside a continuation each open correctly
#[test]
fn nest_30_multiple_independent_bullet_groups_inside_cont() {
    let src1 = b"> - item1";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;
    let (st1, _) = run_block_nested!(stack, depth, src1, 0, src1.len(), 4);

    let src2 = b"> - item2";
    let (st2, _) = run_block_nested!(stack, depth, src2, 0, src2.len(), 4);

    assert_eq!(st1.bullet_items.len(), 1);
    assert_eq!(st2.bullet_items.len(), 1);
    assert_eq!(depth, 1);
}

// 31. When actual nesting exceeds max_nest, the overflow frames are not tracked
#[test]
fn nest_31_overflow_beyond_max_nest_not_tracked() {
    let src = b"> > > > text";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;
    run_block_nested!(stack, depth, src, 0, src.len(), 2);
    assert_eq!(depth, 2);
}

// 32. An unclosed outer continuation with a closed inner fence: only the fence span is emitted
#[test]
fn nest_32_unclosed_outer_cont_with_closed_inner_fence() {
    let src = b"> ```";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    stack[0] = (1, b'>', 0, 0);
    stack[1] = (0, b'`', 3, 2);
    let mut depth = 2;
    let (st, _) = run_block_nested!(stack, depth, src, 0, src.len(), 4);
    assert_eq!(st.fenced_codes.len(), 1);
    assert!(st.blockquotes.is_empty());
}

// 33. A continuation with empty content (marker absent on an empty line) closes with an empty span
#[test]
fn nest_33_cont_with_empty_content_creates_span() {
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    stack[0] = (1, b'>', 0, 0);
    let mut depth = 1;
    let (st, _) = run_block_nested!(stack, depth, b"", 0, 0, 4);
    assert_eq!(st.blockquotes.len(), 1);
    assert_eq!(st.blockquotes[0].start, 0);
    assert_eq!(st.blockquotes[0].end, 0);
}

// 34. A leading stray continuation marker (before any open frame) does not corrupt subsequent parsing
#[test]
fn nest_34_leading_stray_cont_marker_no_corruption() {
    let src = b"> > text";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;
    let (st, _) = run_block_nested!(stack, depth, src, 0, src.len(), 4);
    assert_eq!(depth, 2);
    assert_eq!(st.blockquotes.len(), 0);
}

// 35. A bare fence line at the top level opens a fence frame (there is no active
//     fence to close, so it is an open, and an open pushes no span).
#[test]
fn nest_35_top_level_fence_line_opens_frame() {
    let src = b"```";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;
    let (st, _) = run_block_nested!(stack, depth, src, 0, src.len(), 4);
    assert_eq!(depth, 1);
    assert_eq!(stack[0].0, 0u8);
    assert!(st.fenced_codes.is_empty());
}

// ================================================================
// nesting — content correctness
// ================================================================

// 36. A bullet item opened inside a continuation has correct content span
#[test]
fn nest_36_bullet_inside_cont_content_correct() {
    let src = b"> - hello";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;
    let (st, _) = run_block_nested!(stack, depth, src, 0, src.len(), 4);
    assert_eq!(txt(src, st.bullet_items[0].1), "hello");
}

// 37. Content inside a fence opened inside a continuation is not parsed as inline
#[test]
fn nest_37_fence_inside_cont_content_not_parsed() {
    let src = b"> ```";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;
    run_block_nested!(stack, depth, src, 0, src.len(), 4);

    let content = b"> code";
    let (st, _) = run_block_nested!(stack, depth, content, 0, content.len(), 4);
    assert_eq!(depth, 2);
    assert!(st.bullet_items.is_empty());
    assert!(st.ordered_items.is_empty());
}

// 38. An ordered item opened inside a continuation has correct content span
#[test]
fn nest_38_ordered_inside_cont_content_correct() {
    let src = b"> 1. world";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;
    let (st, _) = run_block_nested!(stack, depth, src, 0, src.len(), 4);
    assert_eq!(txt(src, st.ordered_items[0].1), "world");
}

// 39. Text between nested continuation markers does not create separate paragraph spans
#[test]
fn nest_39_text_between_nested_cont_no_separate_para() {
    let src = b"> > text";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    stack[0] = (1, b'>', 0, 0);
    stack[1] = (1, b'>', 0, 2);
    let mut depth = 2;
    let (st, _) = run_block_nested!(stack, depth, src, 0, src.len(), 4);
    assert!(st.paragraphs.is_empty());
}

// 40. All frames unclosed at end: no spans emitted until explicitly closed
#[test]
fn nest_40_all_frames_unclosed_no_spans_until_closed() {
    let src = b"> > text";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;
    let (st, _) = run_block_nested!(stack, depth, src, 0, src.len(), 4);
    assert_eq!(depth, 2);
    // Frames remain open, no spans created yet
    assert!(st.blockquotes.is_empty());
    // Now close them by providing a line without markers (outermost absent ⇒ both close).
    let (st2, _) = run_block_nested!(stack, depth, b"end", 0, 3, 4);
    assert_eq!(depth, 0);
    assert_eq!(st2.blockquotes.len(), 2);
}

// ================================================================
// nesting — content verification
// ================================================================

// 41. Closing a double blockquote in one call pushes outer + inner with outer.start ≤ inner.start.
//     (Cross-call span *ends* don't compose — each call has its own ParseState and local offsets —
//     so containment is verified by starts within a single close call.)
#[test]
fn nest_41_double_cont_close_emits_outer_and_inner() {
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    stack[0] = (1, b'>', 0, 0);
    stack[1] = (1, b'>', 0, 2);
    let mut depth = 2;
    let (st, _) = run_block_nested!(stack, depth, b"text", 0, 4, 4);
    assert_eq!(depth, 0);
    assert_eq!(st.blockquotes.len(), 2);
    let mut starts: Vec<u32> = st.blockquotes.iter().map(|s| s.start).collect();
    starts.sort();
    assert_eq!(starts, vec![0, 2]);
}

// 42. A fence opened inside a continuation starts after the continuation markers,
//     with its info string still ahead of the run (single call, no trailing newline).
#[test]
fn nest_42_fence_inside_cont_span_excludes_cont_markers() {
    let src = b"> ```rust";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;
    run_block_nested!(stack, depth, src, 0, src.len(), 4);

    assert_eq!(depth, 2);
    assert_eq!(stack[1].0, 0u8); // fence discriminant
    assert_eq!(stack[1].3, 2); // start after "> "
}

// 43. Bullet list inside continuation: bullet content excludes both continuation and bullet markers
#[test]
fn nest_43_bullet_inside_cont_content_excludes_both_markers() {
    let src = b"> - item text";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;
    let (st, _) = run_block_nested!(stack, depth, src, 0, src.len(), 4);

    assert_eq!(st.bullet_items.len(), 1);
    // Content starts after "> - " (4 bytes)
    assert_eq!(st.bullet_items[0].1.start, 4);
    assert_eq!(txt(src, st.bullet_items[0].1), "item text");
}

// 44. Ordered list inside continuation: ordered content excludes all markers
#[test]
fn nest_44_ordered_inside_cont_content_excludes_all_markers() {
    let src = b"> 42. item text";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;
    let (st, _) = run_block_nested!(stack, depth, src, 0, src.len(), 4);

    assert_eq!(st.ordered_items.len(), 1);
    // Content starts after "> 42. " (6 bytes)
    assert_eq!(st.ordered_items[0].1.start, 6);
    assert_eq!(txt(src, st.ordered_items[0].1), "item text");
    assert_eq!(st.ordered_items[0].0.num, 42);
}

// 45. Triple blockquote: closing all three in one call yields starts 0, 2, 4
#[test]
fn nest_45_triple_cont_all_spans_correct_boundaries() {
    let src1 = b"> > > text";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;
    run_block_nested!(stack, depth, src1, 0, src1.len(), 4);

    let (st, _) = run_block_nested!(stack, depth, b"end", 0, 3, 4);
    assert_eq!(st.blockquotes.len(), 3);

    let mut starts: Vec<u32> = st.blockquotes.iter().map(|s| s.start).collect();
    starts.sort();
    assert_eq!(starts, vec![0, 2, 4]);
}

// 46. A fence closing inside a continuation spans from its opening marker (after "> ")
//     to the closing fence line end (single call, no trailing newline).
#[test]
fn nest_46_fence_inside_cont_close_span() {
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    stack[0] = (1, b'>', 0, 0);
    stack[1] = (0, b'`', 3, 2);
    let mut depth = 2;
    let (st, _) = run_block_nested!(stack, depth, b"> ```", 0, 5, 4);

    assert_eq!(depth, 1);
    assert_eq!(st.fenced_codes.len(), 1);
    assert_eq!(st.fenced_codes[0].start, 2);
    assert_eq!(st.fenced_codes[0].end, 5);
}

// 47. Multiple bullet items inside single continuation frame
#[test]
fn nest_47_multiple_bullets_inside_single_cont() {
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;

    let line1 = b"> - item1";
    let (st1, _) = run_block_nested!(stack, depth, line1, 0, line1.len(), 4);

    let line2 = b"> - item2";
    let (st2, _) = run_block_nested!(stack, depth, line2, 0, line2.len(), 4);

    let line3 = b"> - item3";
    let (st3, _) = run_block_nested!(stack, depth, line3, 0, line3.len(), 4);

    assert_eq!(st1.bullet_items.len(), 1);
    assert_eq!(st2.bullet_items.len(), 1);
    assert_eq!(st3.bullet_items.len(), 1);
    assert_eq!(depth, 1);

    assert_eq!(txt(line1, st1.bullet_items[0].1), "item1");
    assert_eq!(txt(line2, st2.bullet_items[0].1), "item2");
    assert_eq!(txt(line3, st3.bullet_items[0].1), "item3");
}

// 48. Continuation dropped mid-line: inner span ends at line start
#[test]
fn nest_48_cont_dropped_mid_line_inner_span_ends_at_line_start() {
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    stack[0] = (1, b'>', 0, 0);
    stack[1] = (1, b'>', 0, 2);
    let mut depth = 2;

    // Line without inner marker - inner closes at line start
    let src = b"> text";
    let (st, _) = run_block_nested!(stack, depth, src, 0, src.len(), 4);

    assert_eq!(depth, 1);
    assert_eq!(st.blockquotes.len(), 1);
    // Inner span ends at position 0 (line start where marker was missing)
    assert_eq!(st.blockquotes[0].end, 0);
}

// 49. Fence inside continuation: info string is part of fence span
#[test]
fn nest_49_fence_inside_cont_info_string_included() {
    let src = b"> ```javascript";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;
    run_block_nested!(stack, depth, src, 0, src.len(), 4);

    // Fence is now open at depth 2
    assert_eq!(depth, 2);
    assert_eq!(stack[1].0, 0u8); // fence discriminant
}

// 50. Nested continuation, level by level: each call's depth transition and per-call
//     span count is checked (each call has a fresh ParseState, so spans don't accumulate).
#[test]
fn nest_50_nested_cont_different_content_per_level() {
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;

    // Open outer, then inner.
    let line1 = b"> outer";
    run_block_nested!(stack, depth, line1, 0, line1.len(), 4);
    let line2 = b"> > inner";
    run_block_nested!(stack, depth, line2, 0, line2.len(), 4);
    assert_eq!(depth, 2);

    // Inner marker absent ⇒ inner frame closes (one span in this call's state).
    let line3 = b"> outer again";
    let (st3, _) = run_block_nested!(stack, depth, line3, 0, line3.len(), 4);
    assert_eq!(depth, 1);
    assert_eq!(st3.blockquotes.len(), 1);

    // Outer marker absent ⇒ outer frame closes (one span in this call's state).
    let (st4, _) = run_block_nested!(stack, depth, b"end", 0, 3, 4);
    assert_eq!(depth, 0);
    assert_eq!(st4.blockquotes.len(), 1);
}

// 51. Bullet with tab separator inside continuation
#[test]
fn nest_51_bullet_tab_sep_inside_cont() {
    let src = b"> -\titem";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;
    let (st, _) = run_block_nested!(stack, depth, src, 0, src.len(), 4);

    assert_eq!(st.bullet_items.len(), 1);
    assert_eq!(txt(src, st.bullet_items[0].1), "item");
}

// 52. Ordered item with parenthesis inside continuation
#[test]
fn nest_52_ordered_paren_inside_cont() {
    let src = b"> 1) item";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;
    let (st, _) = run_block_nested!(stack, depth, src, 0, src.len(), 4);

    assert_eq!(st.ordered_items.len(), 1);
    assert_eq!(st.ordered_items[0].0.kind, b')');
    assert_eq!(txt(src, st.ordered_items[0].1), "item");
}

// 53. Fence with backticks inside continuation with backticks in content
#[test]
fn nest_53_fence_inside_cont_backticks_in_content() {
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;

    let open = b"> ```";
    run_block_nested!(stack, depth, open, 0, open.len(), 4);

    // Content with backticks should not close fence
    let content = b"> `inline code`";
    run_block_nested!(stack, depth, content, 0, content.len(), 4);

    assert_eq!(depth, 2);

    let close = b"> ```";
    let (st, _) = run_block_nested!(stack, depth, close, 0, close.len(), 4);

    assert_eq!(st.fenced_codes.len(), 1);
}

// 54. A `cont` marker is matched at an exact position: it does NOT skip leading
//     whitespace the way a list marker does. So `>  >  text` (two spaces) opens
//     only ONE frame — the second `>` sits in the first frame's content.
#[test]
fn nest_54_cont_with_extra_spaces() {
    let src = b">  >  text";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;
    run_block_nested!(stack, depth, src, 0, src.len(), 4);
    assert_eq!(depth, 1);
}

// 55. Bullet item with different marker kinds inside same continuation
#[test]
fn nest_55_different_bullet_markers_inside_cont() {
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;

    let line1 = b"> - dash";
    let (st1, _) = run_block_nested!(stack, depth, line1, 0, line1.len(), 4);

    let line2 = b"> * star";
    let (st2, _) = run_block_nested!(stack, depth, line2, 0, line2.len(), 4);

    let line3 = b"> + plus";
    let (st3, _) = run_block_nested!(stack, depth, line3, 0, line3.len(), 4);

    assert_eq!(st1.bullet_items[0].0.kind, b'-');
    assert_eq!(st2.bullet_items[0].0.kind, b'*');
    assert_eq!(st3.bullet_items[0].0.kind, b'+');
}

// 56. Fence inside double continuation: fence span excludes both continuation markers
#[test]
fn nest_56_fence_inside_double_cont_excludes_both_markers() {
    let src = b"> > ```";
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;
    run_block_nested!(stack, depth, src, 0, src.len(), 4);

    // Fence should start at position 4 (after "> > ")
    assert_eq!(stack[2].3, 4);
}

// 57. Ordered items with increasing numbers inside continuation
#[test]
fn nest_57_ordered_increasing_nums_inside_cont() {
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;

    let line1 = b"> 1. first";
    let (st1, _) = run_block_nested!(stack, depth, line1, 0, line1.len(), 4);

    let line2 = b"> 2. second";
    let (st2, _) = run_block_nested!(stack, depth, line2, 0, line2.len(), 4);

    let line3 = b"> 3. third";
    let (st3, _) = run_block_nested!(stack, depth, line3, 0, line3.len(), 4);

    assert_eq!(st1.ordered_items[0].0.num, 1);
    assert_eq!(st2.ordered_items[0].0.num, 2);
    assert_eq!(st3.ordered_items[0].0.num, 3);
}

// 58. Continuation with only markers and no content creates empty span
#[test]
fn nest_58_cont_only_markers_empty_span() {
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    stack[0] = (1, b'>', 0, 0);
    let mut depth = 1;

    let (st, _) = run_block_nested!(stack, depth, b"", 0, 0, 4);

    assert_eq!(st.blockquotes.len(), 1);
    assert_eq!(st.blockquotes[0].start, st.blockquotes[0].end);
}

// 59. Mixed content: bullet then ordered inside continuation
#[test]
fn nest_59_mixed_content_inside_cont() {
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;

    let line1 = b"> - bullet";
    let (st1, _) = run_block_nested!(stack, depth, line1, 0, line1.len(), 4);

    let line2 = b"> 1. ordered";
    let (st2, _) = run_block_nested!(stack, depth, line2, 0, line2.len(), 4);

    assert_eq!(st1.bullet_items.len(), 1);
    assert_eq!(st2.ordered_items.len(), 1);
    assert_eq!(depth, 1);
}

// 60. Fence inside continuation: a content line keeps depth and pushes no fenced
//     span; the closing fence line drops depth by one and pushes exactly one span
//     (checked per call — each call has its own ParseState).
#[test]
fn nest_60_fence_close_inside_cont_span_includes_content() {
    let mut stack = [(0u8, 0u8, 0u8, 0u32); 4];
    let mut depth = 0;

    let open = b"> ```";
    run_block_nested!(stack, depth, open, 0, open.len(), 4);

    let content = b"> line1";
    let (st_content, _) = run_block_nested!(stack, depth, content, 0, content.len(), 4);
    assert_eq!(depth, 2);
    assert!(st_content.fenced_codes.is_empty());

    let close = b"> ```";
    let (st_close, _) = run_block_nested!(stack, depth, close, 0, close.len(), 4);
    assert_eq!(depth, 1);
    assert_eq!(st_close.fenced_codes.len(), 1);
}

// ================================================================
// fence line-remainder closures (peel-phase close and open info)
// ================================================================

// 17. A closing fence line with trailing separators and tabs still closes
//     (the remainder scan actually runs, unlike a bare fence line)
#[test]
fn fence_17_close_with_trailing_whitespace() {
    let src = b"``` \t ";
    let mut active = Some((0u8, b'`', 3u8, 0u32));
    let (st, _) = run_block!(&mut active, src, 0, src.len());
    assert!(active.is_none());
    assert_eq!(st.fenced_codes.len(), 1);
}

// 18. A close-line candidate with junk after the fence run does not close
#[test]
fn fence_18_close_with_junk_not_closed() {
    let src = b"``` x";
    let mut active = Some((0u8, b'`', 3u8, 0u32));
    let (st, _) = run_block!(&mut active, src, 0, src.len());
    assert!(active.is_some());
    assert!(st.fenced_codes.is_empty());
}

// 19. An opening fence with an info string opens (the info scan runs over a
//     non-empty remainder)
#[test]
fn fence_19_open_with_info_string() {
    let src = b"```rust";
    let mut active = None;
    let (_, res) = run_block!(&mut active, src, 0, src.len());
    assert!(matches!(res, Some((true, _))));
    assert!(active.is_some());
}

// 20. An opening fence whose info string contains the fence byte is rejected
#[test]
fn fence_20_open_rejected_by_fence_byte_in_info() {
    let src = b"``` a`b";
    let mut active = None;
    let (_, res) = run_block!(&mut active, src, 0, src.len());
    assert!(res.is_none());
    assert!(active.is_none());
}
