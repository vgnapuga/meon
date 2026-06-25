#![no_main]
use libfuzzer_sys::fuzz_target;
use meon::span::MAX_INPUT_LEN;
use meon_md::MarkdownParser;

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT_LEN {
        return;
    }

    let c = MarkdownParser::parse(data);

    let check = |start: u32, end: u32| {
        assert!(start <= end);
        let _ = &c.source[start as usize..end as usize];
    };

    for &s in &c.texts {
        check(s.start, s.end);
    }
    for &s in &c.bolds {
        check(s.start, s.end);
    }
    for &s in &c.italics {
        check(s.start, s.end);
    }
    for &s in &c.bold_italics {
        check(s.start, s.end);
    }
    for &s in &c.codes {
        check(s.start, s.end);
    }
    for &s in &c.autolinks {
        check(s.start, s.end);
    }

    for &s in &c.hard_breaks {
        assert!(s.start == s.end);
        check(s.start, s.end);
    }

    for l in &c.links {
        check(l.text.start, l.text.end);
        check(l.url.start, l.url.end);
    }

    for &s in &c.paragraphs {
        check(s.start, s.end);
    }
    for &s in &c.blockquotes {
        check(s.start, s.end);
    }
    for &s in &c.fenced_codes {
        check(s.start, s.end);
    }

    for (_, s) in &c.headings {
        check(s.start, s.end);
    }
    for (_, s) in &c.thematic_breaks {
        check(s.start, s.end);
    }
    for (_, s) in &c.bullet_items {
        check(s.start, s.end);
    }
    for (_, s) in &c.ordered_items {
        check(s.start, s.end);
    }
});
