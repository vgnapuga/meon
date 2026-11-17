use meon::span::Span;

#[derive(Debug, Clone, Copy)]
pub struct Head {
    pub level: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct Item {
    pub kind: u8,
}

meon::define_parser!(Doc {
    sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\', max_nest = 4;
    inline {
        merge_simple = true;
        hard_break(b'\\', b' ', 2) => breaks [100];
        on_trigger(b'*', b'`', b'<', b'{', b'}') {
            symmetric b'`' { parse_inside = false; 1 => codes [80], }
            symmetric b'*' { parse_inside = true; 1 => ems [40], 2 => strongs [40], }
            asymmetric b'<', b'>' { balanced = false; parse_inside = false; 1 => angles [100], }
            asymmetric b'{', b'}' { balanced = true; parse_inside = false; 1 => rings [100], }
        }
        fallback => texts [10];
    }
    lines {
        line(b'#', max = 6) |n|: Head { level: n } => heads [200];
    }
    blocks {
        block_simple {
            fence(b'~', min = 3) => fences [400];
            cont(b'>') => quotes [200];
        }
        block {
            (b'-') |b|: Item { kind: b } => items [80];
        }
        fallback => paras [80];
    }
});

fn all(src: &[u8], v: &[Span]) -> Vec<String> {
    v.iter()
        .map(|s| String::from_utf8_lossy(&src[s.start as usize..s.end as usize]).into_owned())
        .collect()
}

// ================================================================
// a run of `cont` frames alone
// ================================================================
//
// Consecutive lines whose content follows the markers of the same `cont`
// frames, and opens nothing, are one inline run, as the lines of a paragraph
// are. The markers of every line after the first belong to no inline element.

// 01. A pair opened on one line closes on the next line of the same frames
#[test]
fn cont_run_01_pair_across_lines() {
    let src = b"> *a\n> b*\n";
    let c = DocParser::parse(src);
    assert_eq!(c.quotes.len(), 1);
    assert_eq!(all(src, &c.ems), vec!["a\n> b"]);
    assert!(c.texts.is_empty());
}

// 02. Plain text is flushed line by line: the markers are no text
#[test]
fn cont_run_02_text_per_line() {
    let src = b"> a\n> b\n> c\n";
    let c = DocParser::parse(src);
    assert_eq!(all(src, &c.texts), vec!["a", "b", "c"]);
}

// 03. A marker byte that is also a close byte closes nothing: forward
//     searches skip the markers, with or without the `sep` after them
#[test]
fn cont_run_03_markers_are_no_close_bytes() {
    let src = b"> <a\n> b>\n";
    let c = DocParser::parse(src);
    assert_eq!(all(src, &c.angles), vec!["a\n> b"]);
    let src = b"> {a\n>b}\n";
    let c = DocParser::parse(src);
    assert_eq!(all(src, &c.rings), vec!["a\n>b"]);
    let src = b"> `a\n> b`\n";
    let c = DocParser::parse(src);
    assert_eq!(all(src, &c.codes), vec!["a\n> b"]);
}

// 04. Nested frames run the same way
#[test]
fn cont_run_04_nested_frames() {
    let src = b"> > *a\n> > b*\n";
    let c = DocParser::parse(src);
    assert_eq!(c.quotes.len(), 2);
    assert_eq!(all(src, &c.ems), vec!["a\n> > b"]);
}

// 05. A line of other frames ends the run: deeper, shallower, or none
#[test]
fn cont_run_05_other_frames_end_the_run() {
    for src in [&b"> *a\n> > b*\n"[..], b"> > *a\n> b*\n", b"> *a\nb*\n"] {
        let c = DocParser::parse(src);
        assert!(c.ems.is_empty(), "{:?}", String::from_utf8_lossy(src));
    }
    let src = b"> *a\n> > b*\n";
    let c = DocParser::parse(src);
    assert_eq!(all(src, &c.texts), vec!["*a", "b*"]);
}

// 06. A block item ends the run, and is a line of its own
#[test]
fn cont_run_06_item_ends_the_run() {
    let src = b"> *a\n> - b*\n> c\n";
    let c = DocParser::parse(src);
    assert!(c.ems.is_empty());
    assert_eq!(c.items.len(), 1);
    assert_eq!(c.items[0].0.kind, b'-');
    assert_eq!(all(src, &c.texts), vec!["*a", "b*", "c"]);
}

// 07. Content of `sep` and `tab` bytes alone ends the run, as a blank line
//     ends a paragraph run, and is no text
#[test]
fn cont_run_07_blank_content_ends_the_run() {
    for src in [&b"> *a\n>\n> b*\n"[..], b"> *a\n>  \t\n> b*\n"] {
        let c = DocParser::parse(src);
        assert!(c.ems.is_empty(), "{:?}", String::from_utf8_lossy(src));
        assert_eq!(c.quotes.len(), 1);
        assert_eq!(all(src, &c.texts), vec!["*a", "b*"]);
    }
}

// 08. A hard break at the end of a line of the run is kept
#[test]
fn cont_run_08_hard_break() {
    let src = b"> a  \n> b\\\n> c\n";
    let c = DocParser::parse(src);
    assert_eq!(
        c.breaks
            .iter()
            .map(|s| (s.start, s.end))
            .collect::<Vec<_>>(),
        vec![(3, 3), (9, 9)]
    );
    assert_eq!(all(src, &c.texts), vec!["a", "b", "c"]);
}

// 09. A fence inside the frames ends the run, and its lines are no inline
//     content
#[test]
fn cont_run_09_fence_ends_the_run() {
    let src = b"> *a\n> ~~~\n> b*\n> ~~~\n> c*\n";
    let c = DocParser::parse(src);
    assert!(c.ems.is_empty());
    assert_eq!(c.fences.len(), 1);
    assert_eq!(all(src, &c.texts), vec!["*a", "c*"]);
}

// ================================================================
// lines of `sep` and `tab` bytes alone
// ================================================================

// 01. Such a line ends a paragraph run as an empty line does
#[test]
fn blank_01_ends_paragraph_run() {
    let src = b"*a\n  \t \nb*\n";
    let c = DocParser::parse(src);
    assert!(c.ems.is_empty());
    assert_eq!(all(src, &c.paras), vec!["*a\n", "b*\n"]);
    assert_eq!(all(src, &c.texts), vec!["*a", "b*"]);
}

// 02. It closes the open frames as an empty line does
#[test]
fn blank_02_closes_frames() {
    let src = b"> a\n \n> b\n";
    let c = DocParser::parse(src);
    assert_eq!(c.quotes.len(), 2);
}

// 03. Inside a fence it is content, as an empty line is
#[test]
fn blank_03_fence_content() {
    let src = b"~~~\n  \n~~~\nx\n";
    let c = DocParser::parse(src);
    assert_eq!(all(src, &c.fences), vec!["~~~\n  \n~~~\n"]);
    assert_eq!(all(src, &c.texts), vec!["x"]);
}

// 04. At the end of input it is no paragraph
#[test]
fn blank_04_at_end_of_input() {
    let src = b"a\n   ";
    let c = DocParser::parse(src);
    assert_eq!(all(src, &c.paras), vec!["a\n"]);
    assert_eq!(all(src, &c.texts), vec!["a"]);
}

// 10. The context finders read a run of `cont` frames as the full parse does
#[test]
fn cont_run_10_context_finders_agree() {
    let spans = |v: &[Span]| v.iter().map(|s| (s.start, s.end)).collect::<Vec<_>>();
    for src in [
        &b"> *a\n> b*\n"[..],
        b"> *a <x\n> y> b*\n",
        b"> *a `x\n> *` b*\n",
        b"> > **a\n> > b**\n> c\n",
        b"> *a\n> > b*\n",
        b"> *a\n> - b*\n",
        b"> *a\n>\n> b*\n",
        b"> *a\n>   \n> b*\n",
        b"*a\n \nb*\n",
        b"> *a {\n>b*}*\n",
    ] {
        let c = DocParser::parse(src);
        let ctx = DocParser::context(src);
        let what = String::from_utf8_lossy(src);
        let ems: Vec<Span> = DocParser::find_context_ems(src, &ctx).collect();
        let strongs: Vec<Span> = DocParser::find_context_strongs(src, &ctx).collect();
        assert_eq!(spans(&ems), spans(&c.ems), "ems in {what:?}");
        assert_eq!(spans(&strongs), spans(&c.strongs), "strongs in {what:?}");
    }
}

// 11. A line rule's line ends the run before it, and its content is a run of
//     its own
#[test]
fn cont_run_11_line_rule_ends_paragraph_run() {
    let src = b"*a\n## b*\nc*\n";
    let c = DocParser::parse(src);
    assert!(c.ems.is_empty());
    assert_eq!(c.heads.len(), 1);
    assert_eq!(c.heads[0].0.level, 2);
    assert_eq!(all(src, &c.texts), vec!["*a", "b*", "c*"]);
}

// ================================================================
// `crlf = true`: a `\r` right before `eol` is part of the line end
// ================================================================

#[derive(Debug, Clone, Copy)]
pub struct Pair {
    pub key: Span,
    pub value: Span,
}

mod crlf {
    use super::{Head, Item, Pair};
    meon::define_parser!(CrDoc {
        sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\', max_nest = 4, crlf = true;
        inline {
            merge_simple = true;
            hard_break(b'\\', b' ', 2) => breaks [100];
            on_trigger(b'*', b'`', b'<', b'=') {
                symmetric b'`' { parse_inside = false; 1 => codes [80], }
                symmetric b'*' { parse_inside = true; 1 => ems [40], }
                asymmetric b'<', b'>' { balanced = false; parse_inside = false; 1 => angles [100], }
                key_value: Pair {
                    eq = b'='; allow_sep = true; end = b'\n';
                    key => key, value => value,
                } => pairs [100]
            }
            fallback => texts [10];
        }
        lines {
            line(b'#', max = 6) |n|: Head { level: n } => heads [200];
            line_simple(b'=', min = 3) |b|: Item { kind: b } => rules [200];
        }
        blocks {
            block_simple {
                fence(b'~', min = 3) => fences [400];
                cont(b'>') => quotes [200];
            }
            block {
                (b'-') |b|: Item { kind: b } => items [80];
            }
            fallback => paras [80];
        }
    });
}
use crlf::CrDocParser;

fn pairs(v: &[Span]) -> Vec<(u32, u32)> {
    v.iter().map(|s| (s.start, s.end)).collect()
}

// 01. Text and elements end before the `\r`; a pair spans `\r\n`
#[test]
fn crlf_01_line_content_ends_before_cr() {
    let src = b"a *b\r\nc* d\r\n";
    let c = CrDocParser::parse(src);
    assert_eq!(all(src, &c.ems), vec!["b\r\nc"]);
    assert_eq!(all(src, &c.texts), vec!["a ", " d"]);
    assert_eq!(all(src, &c.paras), vec!["a *b\r\nc* d\r\n"]);
}

// 02. A lone `\r` is an ordinary byte
#[test]
fn crlf_02_lone_cr_is_content() {
    let src = b"a\rb\r\n";
    let c = CrDocParser::parse(src);
    assert_eq!(all(src, &c.texts), vec!["a\rb"]);
}

// 03. A hard break before `\r\n` is found
#[test]
fn crlf_03_hard_break() {
    let src = b"a\\\r\nb  \r\nc\\\r\n";
    let c = CrDocParser::parse(src);
    assert_eq!(pairs(&c.breaks), vec![(1, 1), (5, 5), (10, 10)]);
    assert_eq!(all(src, &c.texts), vec!["a", "b", "c"]);
}

// 04. Line and block rules read the line without its `\r`
#[test]
fn crlf_04_line_and_block_rules() {
    let src = b"# h\r\n===\r\n- i\r\n#\r\n";
    let c = CrDocParser::parse(src);
    assert_eq!(c.heads.len(), 2);
    assert_eq!(
        all(src, &c.heads.iter().map(|h| h.1).collect::<Vec<_>>()),
        vec!["h", ""]
    );
    assert_eq!(c.rules.len(), 1);
    assert_eq!(
        all(src, &c.rules.iter().map(|r| r.1).collect::<Vec<_>>()),
        vec!["==="]
    );
    assert_eq!(
        all(src, &c.items.iter().map(|r| r.1).collect::<Vec<_>>()),
        vec!["i"]
    );
}

// 05. A fence closes on a line ending in `\r\n`, and its span takes it
#[test]
fn crlf_05_fence_close() {
    let src = b"~~~\r\nx\r\n~~~ \r\ny\r\n";
    let c = CrDocParser::parse(src);
    assert_eq!(all(src, &c.fences), vec!["~~~\r\nx\r\n~~~ \r\n"]);
    assert_eq!(all(src, &c.texts), vec!["y"]);
}

// 06. A line of `\r` alone, or of `sep`/`tab` bytes and `\r`, is blank
#[test]
fn crlf_06_blank_lines() {
    for src in [&b"*a\r\n\r\nb*\r\n"[..], b"*a\r\n \t\r\nb*\r\n"] {
        let c = CrDocParser::parse(src);
        assert!(c.ems.is_empty(), "{:?}", String::from_utf8_lossy(src));
        assert_eq!(c.paras.len(), 2);
    }
}

// 07. A run of `cont` frames reads past `\r\n` and the next markers
#[test]
fn crlf_07_cont_run() {
    let src = b"> *a\r\n> b*\r\n";
    let c = CrDocParser::parse(src);
    assert_eq!(all(src, &c.ems), vec!["a\r\n> b"]);
    assert_eq!(all(src, &c.texts), vec![] as Vec<String>);
    let src = b"> a\r\n>\r\n> b\r\n";
    let c = CrDocParser::parse(src);
    assert_eq!(all(src, &c.texts), vec!["a", "b"]);
}

// 08. Without `crlf` the `\r` is content
#[test]
fn crlf_08_off_keeps_cr() {
    let src = b"a\r\n# h\r\n";
    let c = DocParser::parse(src);
    assert_eq!(all(src, &c.texts), vec!["a\r", "h\r"]);
}

// 09. The finders read lines the same way
#[test]
fn crlf_09_finders_agree() {
    for src in [
        &b"a *b\r\nc* d\r\n"[..],
        b"*a\r\n\r\nb*\r\n",
        b"*a\r\n  \r\nb* *c*\r\n",
        b"> *a\r\n> b*\r\n",
        b"<a\r\n\r\nb> <c\r\nd>\r\n",
        b"~~~\r\n*x*\r\n~~~\r\n*y*\r\n",
        b"# *h*\r\n- *i*\r\n===\r\n",
    ] {
        let c = CrDocParser::parse(src);
        let ctx = CrDocParser::context(src);
        let what = String::from_utf8_lossy(src);
        let ems: Vec<Span> = CrDocParser::find_context_ems(src, &ctx).collect();
        assert_eq!(pairs(&ems), pairs(&c.ems), "ems in {what:?}");
        let angles: Vec<Span> = CrDocParser::find_angles(src).collect();
        assert_eq!(pairs(&angles), pairs(&c.angles), "angles in {what:?}");
        let fences: Vec<Span> = CrDocParser::find_fences(src).collect();
        assert_eq!(pairs(&fences), pairs(&c.fences), "fences in {what:?}");
        let heads: Vec<Span> = CrDocParser::find_context_heads(src, &ctx)
            .map(|h| h.1)
            .collect();
        assert_eq!(
            pairs(&heads),
            pairs(&c.heads.iter().map(|h| h.1).collect::<Vec<_>>())
        );
        let rules: Vec<Span> = CrDocParser::find_context_rules(src, &ctx)
            .map(|h| h.1)
            .collect();
        assert_eq!(
            pairs(&rules),
            pairs(&c.rules.iter().map(|h| h.1).collect::<Vec<_>>())
        );
        let items: Vec<Span> = CrDocParser::find_context_items(src, &ctx)
            .map(|h| h.1)
            .collect();
        assert_eq!(
            pairs(&items),
            pairs(&c.items.iter().map(|h| h.1).collect::<Vec<_>>())
        );
    }
}

// 10. Lines of `sep` and `tab` bytes alone end paragraphs for the finders too
#[test]
fn blank_05_finders_agree() {
    for src in [&b"*a\n  \nb*\n"[..], b"<a\n\t\nb>\n", b"`a\n \n` `b`\n"] {
        let c = DocParser::parse(src);
        let ctx = DocParser::context(src);
        let what = String::from_utf8_lossy(src);
        let ems: Vec<Span> = DocParser::find_ems(src).collect();
        assert_eq!(pairs(&ems), pairs(&c.ems), "ems in {what:?}");
        let ems: Vec<Span> = DocParser::find_context_ems(src, &ctx).collect();
        assert_eq!(pairs(&ems), pairs(&c.ems), "context ems in {what:?}");
        let angles: Vec<Span> = DocParser::find_angles(src).collect();
        assert_eq!(pairs(&angles), pairs(&c.angles), "angles in {what:?}");
        let codes: Vec<Span> = DocParser::find_codes(src).collect();
        assert_eq!(pairs(&codes), pairs(&c.codes), "codes in {what:?}");
    }
}

// 11. A fence inside the frames, its lines ending in `\r\n`, leaves the
//     frames open past its close line
#[test]
fn crlf_10_fence_inside_cont() {
    let src = b"> ~~~\r\n> x\r\n> ~~~\r\n> y\r\n";
    let c = CrDocParser::parse(src);
    assert_eq!(c.quotes.len(), 1);
    assert_eq!(c.fences.len(), 1);
    assert_eq!(all(src, &c.texts), vec!["y"]);
}

// 12. A value that ends with its line ends before the `\r`
#[test]
fn crlf_11_key_value() {
    let src = b"k = v\r\nj=w\r\n";
    let c = CrDocParser::parse(src);
    let keys: Vec<Span> = c.pairs.iter().map(|p| p.key).collect();
    assert_eq!(all(src, &keys), vec!["k", "j"]);
    let values: Vec<Span> = c.pairs.iter().map(|p| p.value).collect();
    assert_eq!(all(src, &values), vec!["v", "w"]);
    let found: Vec<Span> = CrDocParser::find_pairs(src).map(|p| p.value).collect();
    assert_eq!(pairs(&found), pairs(&values));
}

// 12. EOF is not an eol byte: a trailing CR remains ordinary content.
#[test]
fn crlf_12_trailing_cr_is_content() {
    for src in [&b"\r"[..], b" \t\r", b"a\r", b"a\r\n\r"] {
        let c = CrDocParser::parse(src);
        assert_eq!(c.paras.len(), 1);
        assert_eq!(c.paras[0], Span::new(0, src.len() as u32));
        assert_eq!(c.texts.last().unwrap().end, src.len() as u32);
    }
}

// 13. Line and block content keep the final CR in full and selective scans.
#[test]
fn crlf_13_marker_content_at_eof() {
    let src = b"# x\r";
    let c = CrDocParser::parse(src);
    assert_eq!(c.heads[0].1, Span::new(2, 4));
    assert!(
        CrDocParser::find_heads(src)
            .map(|h| h.1)
            .eq([Span::new(2, 4)])
    );
    let ctx = CrDocParser::context(src);
    assert!(
        CrDocParser::find_context_heads(src, &ctx)
            .map(|h| h.1)
            .eq([Span::new(2, 4)])
    );
    let src = b"- x\r";
    let c = CrDocParser::parse(src);
    assert_eq!(c.items[0].1, Span::new(2, 4));
    assert!(
        CrDocParser::find_items(src)
            .map(|h| h.1)
            .eq([Span::new(2, 4)])
    );
    let src = b"> x\r";
    let c = CrDocParser::parse(src);
    assert_eq!(c.texts.as_slice(), &[Span::new(2, 4)]);
}

// 14. A bare CR invalidates a uniform line and a fence closing line.
#[test]
fn crlf_14_trailing_cr_is_not_a_line_terminator() {
    for src in [&b"===\r"[..], b"=== \r"] {
        let c = CrDocParser::parse(src);
        assert!(c.rules.is_empty());
        assert!(CrDocParser::find_rules(src).next().is_none());
        let ctx = CrDocParser::context(src);
        assert!(CrDocParser::find_context_rules(src, &ctx).next().is_none());
        assert_eq!(c.paras.as_slice(), &[Span::new(0, src.len() as u32)]);
    }
    let src = b"~~~\nx\n~~~\r";
    let c = CrDocParser::parse(src);
    assert_eq!(c.fences.len(), 1);
    assert_eq!(c.fences[0].end, src.len() as u32);
    assert!(CrDocParser::find_fences(src).eq(c.fences.iter().copied()));
    assert_eq!(
        CrDocParser::context(src).spans(),
        &[Span::new(0, src.len() as u32)]
    );
}

// 15. An actual CRLF still terminates content, including uniform lines.
#[test]
fn crlf_15_actual_crlf_at_eof() {
    let src = b"# x\r\n";
    let c = CrDocParser::parse(src);
    assert_eq!(c.heads[0].1, Span::new(2, 3));
    assert!(
        CrDocParser::find_heads(src)
            .map(|h| h.1)
            .eq([Span::new(2, 3)])
    );
    let src = b"===\r\n";
    assert_eq!(CrDocParser::parse(src).rules.len(), 1);
    assert_eq!(CrDocParser::find_rules(src).count(), 1);
}

// ================================================================
// a run of `cont` frames alone, with no `hard_break` and two `cont` rules
// ================================================================

mod plain {
    meon::define_parser!(Plain {
        sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\', max_nest = 2;
        inline {
            on_trigger(b'*', b'<') {
                symmetric b'*' { parse_inside = true; 1 => ems [40], }
                asymmetric b'<', b'>' { balanced = false; parse_inside = false; 1 => angles [100], }
            }
            fallback => texts [10];
        }
        blocks {
            block_simple {
                cont(b'>') => quotes [200];
                cont(b'|') => bars [200];
            }
            fallback => paras [80];
        }
    });
}
use plain::PlainParser;

// 01. Without `hard_break` the scan still steps past each line's markers
#[test]
fn cont_plain_01_markers_skipped() {
    let src = b"> *a\n> b*\n> <c\n>d>\n";
    let c = PlainParser::parse(src);
    assert_eq!(all(src, &c.ems), vec!["a\n> b"]);
    assert_eq!(all(src, &c.angles), vec!["c\n>d"]);
    let src = b"> a\n>b\n";
    let c = PlainParser::parse(src);
    assert_eq!(all(src, &c.texts), vec!["a", "b"]);
}

// 02. A line of another frame at the same depth ends the run
#[test]
fn cont_plain_02_other_frame_same_depth() {
    let src = b"> *a\n| b*\n";
    let c = PlainParser::parse(src);
    assert!(c.ems.is_empty());
    assert_eq!((c.quotes.len(), c.bars.len()), (1, 1));
    assert_eq!(all(src, &c.texts), vec!["*a", "b", "*"]);
    let ctx = PlainParser::context(src);
    assert!(PlainParser::find_context_ems(src, &ctx).next().is_none());
}

// 03. A blank line inside a fence inside the frames keeps them open, for the
//     context too
#[test]
fn cont_run_12_blank_line_in_nested_fence() {
    for src in [
        &b"> ~~~\n  \n> *x*\n> ~~~\n*y*\n"[..],
        b"> ~~~\n\n> *x*\n> ~~~\n*y*\n",
    ] {
        let c = DocParser::parse(src);
        assert_eq!(c.fences.len(), 1);
        assert_eq!(all(src, &c.ems), vec!["y"]);
        let ctx = DocParser::context(src);
        let ems: Vec<Span> = DocParser::find_context_ems(src, &ctx).collect();
        assert_eq!(
            pairs(&ems),
            pairs(&c.ems),
            "{:?}",
            String::from_utf8_lossy(src)
        );
    }
}

// 04. A run of one line reads as a run: whatever follows it — another
//     line, a deeper frame, a blank line or the end of input
#[test]
fn cont_run_13_one_line_runs() {
    let src = b"> *a*\nb\n";
    let c = DocParser::parse(src);
    assert_eq!(all(src, &c.ems), vec!["a"]);
    assert_eq!(all(src, &c.texts), vec!["b"]);
    let src = b"> *a\n> > b*\n";
    let c = DocParser::parse(src);
    assert!(c.ems.is_empty());
    assert_eq!(all(src, &c.texts), vec!["*a", "b*"]);
    for src in [&b"> <a> *b*\n\nc"[..], b"> <a> *b*"] {
        let c = DocParser::parse(src);
        assert_eq!(all(src, &c.angles), vec!["a"]);
        assert_eq!(all(src, &c.ems), vec!["b"]);
    }
}

// ================================================================
// lines a paragraph run goes on with
// ================================================================
//
// With no block open, a line whose first byte begins no block or line rule
// goes on the paragraph run; every other line takes the block and line
// rules.

// 01. A line beginning with a marker opens its rule after a paragraph line
#[test]
fn para_01_marker_lines_open_rules() {
    let c = DocParser::parse(b"a\n- b\n");
    assert_eq!((c.paras.len(), c.items.len()), (1, 1));
    let c = DocParser::parse(b"a\n# b\n");
    assert_eq!((c.paras.len(), c.heads.len()), (1, 1));
    let c = DocParser::parse(b"a\n> b\n");
    assert_eq!((c.paras.len(), c.quotes.len()), (1, 1));
    let c = DocParser::parse(b"a\n~~~\nb\n~~~\n");
    assert_eq!((c.paras.len(), c.fences.len()), (1, 1));
}

// 02. A `sep` or `tab` byte may stand before a block marker
#[test]
fn para_02_marker_after_sep_or_tab() {
    for src in [&b"a\n - b\n"[..], b"a\n\t- b\n"] {
        let c = DocParser::parse(src);
        assert_eq!(c.items.len(), 1, "{:?}", String::from_utf8_lossy(src));
        assert_eq!(c.paras.len(), 1);
    }
}

// 03. Other lines are one run with the paragraph's first
#[test]
fn para_03_text_lines_one_run() {
    let src = b"a *b\nc* d\ne\n";
    let c = DocParser::parse(src);
    assert_eq!(c.paras.len(), 1);
    assert_eq!(all(src, &c.ems), vec!["b\nc"]);
}

// ================================================================
// an opaque `chained` rule with no `balanced` component
// ================================================================

mod links {
    use meon::span::Span;

    #[derive(Debug, Clone, Copy)]
    pub struct Link {
        pub is_image: bool,
        pub text: Span,
        pub url: Span,
    }

    meon::define_parser!(Lk {
        sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\', max_nest = 2;
        inline {
            on_trigger(b'[') {
                chained: Link {
                    | b'[', b']' | { parse_inside = false; balanced = false; } => text,
                    | b'(', b')' | { parse_inside = false; balanced = false; } => url,
                    prefix | b'!' | => is_image,
                } => links [40]
            }
            fallback => texts [10];
        }
        blocks {
            block_simple { cont(b'>') => quotes [40]; }
            fallback => paras [40];
        }
    });
}
use links::LkParser;

// 01. An escaped close byte right before the close is content
#[test]
fn chained_plain_01_escaped_close_before_close() {
    let src = b"[a\\]](u\\)) ![b](c)";
    let c = LkParser::parse(src);
    let got: Vec<(bool, String, String)> = c
        .links
        .iter()
        .map(|l| {
            let text = all(src, &[l.text]).remove(0);
            (l.is_image, text, all(src, &[l.url]).remove(0))
        })
        .collect();
    assert_eq!(
        got,
        vec![
            (false, "a\\]".to_string(), "u\\)".to_string()),
            (true, "b".to_string(), "c".to_string())
        ]
    );
}

// 02. Its searches cross the lines of a run and skip their markers
#[test]
fn chained_plain_02_across_run_lines() {
    let src = b"> [a\n> b](c\n> d)\n";
    let c = LkParser::parse(src);
    assert_eq!(c.links.len(), 1);
    assert_eq!(all(src, &[c.links[0].text]), vec!["a\n> b"]);
    assert_eq!(all(src, &[c.links[0].url]), vec!["c\n> d"]);
}

// ================================================================
// balanced opaque `chained` searches across inline runs
// ================================================================

mod balanced_links {
    use super::links::Link;

    meon::define_parser!(Balanced {
        sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\', max_nest = 2;
        inline {
            on_trigger(b'[') {
                chained: Link {
                    | b'[', b']' | { parse_inside = false; balanced = true; } => text,
                    | b'(', b')' | { parse_inside = false; balanced = true; } => url,
                    prefix | b'!' | => is_image,
                } => links [40]
            }
            fallback => texts [10];
        }
        blocks {
            block_simple {
                cont(b'[') => brackets [40];
                cont(b'(') => parens [40];
            }
            fallback => paras [40];
        }
    });
}
use balanced_links::BalancedParser;

// 01. A continuation marker equal to the text opener does not nest in
//     the text component. The leading `x` ends block-marker dispatch.
#[test]
fn chained_balanced_01_text_marker_is_no_opener() {
    let src = b"[ x[a\n[ b](u)\n";
    let c = BalancedParser::parse(src);
    assert_eq!(c.links.len(), 1);
    assert!(!c.links[0].is_image);
    assert_eq!(c.links[0].text, Span::new(4, 9));
    assert_eq!(c.links[0].url, Span::new(11, 12));
    assert_eq!(BalancedParser::context(src).spans(), &[Span::new(3, 13)]);
}

// 02. A continuation marker equal to the url opener does not nest in
//     the url component either.
#[test]
fn chained_balanced_02_url_marker_is_no_opener() {
    let src = b"( x[a](u\n( v)\n";
    let c = BalancedParser::parse(src);
    assert_eq!(c.links.len(), 1);
    assert!(!c.links[0].is_image);
    assert_eq!(c.links[0].text, Span::new(4, 5));
    assert_eq!(c.links[0].url, Span::new(7, 12));
    assert_eq!(BalancedParser::context(src).spans(), &[Span::new(3, 13)]);
}

// 03. A failed balanced text search with a close in its paragraph does
//     not suppress a valid pair in the next paragraph.
#[test]
fn chained_balanced_03_text_failure_ends_at_paragraph() {
    let src = b"x[a [b]\n\nx[c](d)";
    let c = BalancedParser::parse(src);
    assert_eq!(c.links.len(), 1);
    assert!(!c.links[0].is_image);
    assert_eq!(c.links[0].text, Span::new(11, 12));
    assert_eq!(c.links[0].url, Span::new(14, 15));
    assert_eq!(BalancedParser::context(src).spans(), &[Span::new(10, 16)]);
}

// 04. A failed balanced url search leaves both the valid inner pair and
//     the pair in the following paragraph available.
#[test]
fn chained_balanced_04_url_failure_ends_at_paragraph() {
    let src = b"x[a](b [c](d)\n\nx[e](f)";
    let c = BalancedParser::parse(src);
    assert!(c.links.iter().map(|l| (l.is_image, l.text, l.url)).eq([
        (false, Span::new(8, 9), Span::new(11, 12)),
        (false, Span::new(17, 18), Span::new(20, 21)),
    ]));
    assert_eq!(
        BalancedParser::context(src).spans(),
        &[Span::new(7, 13), Span::new(16, 22)]
    );
}

// 05. After an outer failure builds the close index, continuation markers
//     remain absent from both components' nesting, just as in a direct scan.
#[test]
fn chained_balanced_05_index_skips_continuation_markers() {
    for (src, text, url) in [
        (
            &b"[ x[z [a\n[ b](u)\n"[..],
            Span::new(7, 12),
            Span::new(14, 15),
        ),
        (
            &b"( x[z [a](u\n( v)\n"[..],
            Span::new(7, 8),
            Span::new(10, 15),
        ),
    ] {
        let c = BalancedParser::parse(src);
        assert_eq!(c.links.len(), 1);
        assert_eq!(c.links[0].text, text);
        assert_eq!(c.links[0].url, url);
        assert_eq!(BalancedParser::context(src).spans(), &[Span::new(6, 16)]);
    }
}
