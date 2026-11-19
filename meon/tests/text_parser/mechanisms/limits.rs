use meon::span::Span;

fn txt(src: &[u8], span: Span) -> &str {
    std::str::from_utf8(&src[span.start as usize..span.end as usize]).unwrap()
}

mod limited {
    use super::txt;
    use meon::define_parser;
    use meon::span::Span;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Heading {
        pub level: u8,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Bullet {
        pub kind: u8,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Link {
        pub is_image: bool,
        pub text: Span,
        pub url: Span,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Pair {
        pub key: Span,
        pub value: Span,
    }

    define_parser!(Limited {
        sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
        limits {
            max_nest = 4;
            max_inline_span = 3;
            max_line_span = 2;
            max_block_span = 3;
        }
        inline {
            on_trigger(b'*', b'`', b'[', b'"') {
                symmetric b'*' { parse_inside = true; 1 => italics [40], }
                symmetric b'`' { parse_inside = false; 1 => codes [40], }
                symmetric b'"' { parse_inside = false; 1 => strs [40], }
                chained: Link {
                    | b'[', b']' | { parse_inside = false; balanced = false; } => text,
                    | b'(', b')' | { parse_inside = false; balanced = false; } => url,
                    prefix | b'!' | => is_image,
                } => links [40]
                key_value: Pair {
                    eq = b':'; allow_sep = true; end = b',';
                    key => key, value => value,
                } => pairs [40]
            }
            fallback => texts [10];
        }
        lines {
            line(b'#', max = 6) |n|: Heading { level: n } => headings [10];
        }
        blocks {
            block_simple {
                fence(b'`', min = 3) => fenced_codes [10];
                cont(b'>') => blockquotes [10];
            }
            block {
                (b'-') |b|: Bullet { kind: b } => bullets [10];
            }
            fallback => paragraphs [10];
        }
    });

    #[test]
    fn inline_transparent_over_limit_keeps_text() {
        let src = b"*abcd*";
        let c = LimitedParser::parse_unchecked(src);
        assert!(c.italics.is_empty());
        assert_eq!(c.texts.len(), 1);
        assert_eq!(txt(src, c.texts[0]), "*abcd*");
    }

    #[test]
    fn inline_transparent_under_limit_is_kept() {
        let src = b"*ab*";
        let c = LimitedParser::parse_unchecked(src);
        assert_eq!(c.italics.len(), 1);
        assert_eq!(txt(src, c.italics[0]), "ab");
        assert!(c.texts.is_empty());
    }

    #[test]
    fn opaque_over_limit_reparses_inside() {
        let src = b"`*ab*`";
        let c = LimitedParser::parse_unchecked(src);
        assert!(c.codes.is_empty());
        assert_eq!(c.italics.len(), 1);
        assert_eq!(txt(src, c.italics[0]), "ab");
    }

    #[test]
    fn opaque_under_limit_is_opaque() {
        let src = b"`*a*`";
        let c = LimitedParser::parse_unchecked(src);
        assert_eq!(c.codes.len(), 1);
        assert!(c.italics.is_empty());
    }

    #[test]
    fn chained_over_limit_drops_link_and_keeps_text() {
        let src = b"[a](u)";
        let c = LimitedParser::parse_unchecked(src);
        assert!(c.links.is_empty());
        let all: String = c.texts.iter().map(|s| txt(src, *s)).collect();
        assert_eq!(all, "[a](u)");
    }

    #[test]
    fn key_value_over_limit_drops_pair() {
        let src = b"\"a\":1";
        let c = LimitedParser::parse_unchecked(src);
        assert!(c.pairs.is_empty());
        assert_eq!(c.strs.len(), 1);
        assert_eq!(txt(src, c.strs[0]), "a");
    }

    #[test]
    fn line_over_limit_goes_to_block_fallback() {
        let src = b"# abc";
        let c = LimitedParser::parse_unchecked(src);
        assert!(c.headings.is_empty());
        assert_eq!(c.paragraphs.len(), 1);
        assert_eq!(txt(src, c.paragraphs[0]), "abc");
        assert_eq!(c.texts.len(), 1);
        assert_eq!(txt(src, c.texts[0]), "abc");
    }

    #[test]
    fn block_item_over_limit_goes_to_block_fallback() {
        let src = b"- item";
        let c = LimitedParser::parse_unchecked(src);
        assert!(c.bullets.is_empty());
        assert_eq!(c.paragraphs.len(), 1);
        assert_eq!(txt(src, c.paragraphs[0]), "item");
        assert_eq!(txt(src, c.texts[0]), "item");
    }

    #[test]
    fn cont_over_limit_goes_to_block_fallback() {
        let src = b"> quote\n";
        let c = LimitedParser::parse_unchecked(src);
        assert!(c.blockquotes.is_empty());
        assert_eq!(c.paragraphs.len(), 1);
        assert_eq!(txt(src, c.paragraphs[0]), "> quote\n");
        assert_eq!(txt(src, c.texts[0]), "quote");
    }

    #[test]
    fn fence_over_limit_reparses_content() {
        let src = b"```\ncode\n```\n";
        let c = LimitedParser::parse_unchecked(src);
        assert!(c.fenced_codes.is_empty());
        assert_eq!(c.paragraphs.len(), 1);
        assert_eq!(c.texts.len(), 1);
        assert_eq!(txt(src, c.texts[0]), "code");
    }

    #[test]
    fn standalone_finders_honor_limits() {
        assert!(
            LimitedParser::find_italics_unchecked(b"*abcd*")
                .next()
                .is_none()
        );
        assert_eq!(LimitedParser::find_italics_unchecked(b"*ab*").count(), 1);
        assert!(
            LimitedParser::find_headings_unchecked(b"# abc")
                .next()
                .is_none()
        );
        assert!(
            LimitedParser::find_bullets_unchecked(b"- item")
                .next()
                .is_none()
        );
        assert!(
            LimitedParser::find_fenced_codes_unchecked(b"```\ncode\n```\n")
                .next()
                .is_none()
        );
    }

    #[test]
    fn fence_inside_cont_skips_markers() {
        let src = b"> ```\n> *ab*\n> ```\n";
        let c = LimitedParser::parse_unchecked(src);
        assert_eq!(c.italics.len(), 1);
        let all: String = c.texts.iter().map(|s| txt(src, *s)).collect();
        assert_eq!(all, "");
    }

    #[test]
    fn fence_content_splits_at_blank_lines() {
        let src = b"```\na\n\nb\n```\n";
        let c = LimitedParser::parse_unchecked(src);
        let all: Vec<&str> = c.texts.iter().map(|s| txt(src, *s)).collect();
        assert_eq!(all, vec!["a", "b"]);
    }

    #[test]
    fn context_removes_over_limit_opaque_region() {
        let src = b"`*ab*`";
        let ctx = LimitedParser::context_unchecked(src);
        assert_eq!(
            LimitedParser::find_context_italics_unchecked(src, &ctx).count(),
            1
        );
    }

    #[test]
    fn context_rescans_over_limit_region() {
        let src = b"\"a `*b*` c\"";
        let ctx = LimitedParser::context_unchecked(src);
        assert_eq!(
            LimitedParser::find_context_italics_unchecked(src, &ctx).count(),
            0
        );
    }
    #[test]
    fn context_reparses_over_limit_fence() {
        let src = b"```\n*a*\n```\n";
        let ctx = LimitedParser::context_unchecked(src);
        assert_eq!(
            LimitedParser::find_context_italics_unchecked(src, &ctx).count(),
            1
        );
    }

    #[test]
    fn context_keeps_under_limit_opaque_region() {
        let src = b"`*a*`";
        let ctx = LimitedParser::context_unchecked(src);
        assert_eq!(
            LimitedParser::find_context_italics_unchecked(src, &ctx).count(),
            0
        );
    }
}

mod zero {
    use meon::define_parser;

    define_parser!(Zero {
        sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
        limits { max_nest = 2; max_inline_span = 0; }
        inline {
            on_trigger(b'*') {
                symmetric b'*' {
                    parse_inside = true;
                    1 => italics [10], 2 => bolds [10],
                }
            }
            fallback => texts [10];
        }
        blocks { fallback => paragraphs [10]; }
    });

    #[test]
    fn zero_limit_is_absent() {
        let src = b"*abcd*";
        let c = ZeroParser::parse_unchecked(src);
        assert_eq!(c.italics.len(), 1);
    }

    #[test]
    fn max_nest_in_limits_is_used() {
        let src = b"*a **b** c*";
        let c = ZeroParser::parse_unchecked(src);
        assert_eq!(c.italics.len(), 1);
        assert_eq!(c.bolds.len(), 1);
    }
}

mod input {
    use meon::InputBytesTooLarge;
    use meon::define_parser;

    define_parser!(Input {
        sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
        limits { max_input_bytes = 8; }
        inline {
            on_trigger(b'*', b'`') {
                symmetric b'*' { parse_inside = true; 1 => italics [10]; }
                symmetric b'`' { parse_inside = false; 1 => codes [10]; }
            }
            fallback => texts [10];
        }
        blocks { fallback => paragraphs [10]; }
    });

    #[test]
    fn parse_accepts_at_the_limit() {
        assert!(InputParser::parse(b"12345678").is_ok());
    }

    #[test]
    fn parse_rejects_over_the_limit() {
        assert_eq!(
            InputParser::parse(b"123456789").err().unwrap(),
            InputBytesTooLarge { len: 9, max: 8 }
        );
    }

    #[test]
    fn unchecked_parse_ignores_the_limit() {
        assert_eq!(InputParser::parse_unchecked(b"123456789").texts.len(), 1);
    }

    #[test]
    fn finders_check_the_limit() {
        assert!(InputParser::find_codes(b"123456789").is_err());
        assert_eq!(InputParser::find_codes(b"`a`").unwrap().count(), 1);
        assert_eq!(InputParser::find_codes_unchecked(b"123456789").count(), 0);
    }

    #[test]
    fn context_checks_the_limit() {
        assert!(InputParser::context(b"123456789").is_err());
        let ctx = InputParser::context(b"`a`").unwrap();
        assert_eq!(ctx.spans().len(), 1);
        assert_eq!(
            InputParser::context_unchecked(b"123456789").spans().len(),
            0
        );
    }

    #[test]
    fn context_finders_check_the_limit() {
        assert!(
            InputParser::find_context_italics(
                b"123456789",
                &InputParser::context_unchecked(b"123456789")
            )
            .is_err()
        );
        let src = b"*a*";
        let ctx = InputParser::context_unchecked(src);
        assert_eq!(
            InputParser::find_context_italics(src, &ctx)
                .unwrap()
                .count(),
            1
        );
        assert_eq!(
            InputParser::find_context_italics_unchecked(b"123456789", &ctx).count(),
            0
        );
    }

    #[test]
    fn error_display_names_both_lengths() {
        let err = InputParser::parse(b"123456789").err().unwrap();
        assert_eq!(
            err.to_string(),
            "input is 9 bytes, exceeding the grammar limit of 8 bytes"
        );
    }
}

mod unlimited {
    use meon::define_parser;

    define_parser!(Unlimited {
        sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
        limits { max_input_bytes = _; }
        inline {
            on_trigger(b'`') {
                symmetric b'`' { parse_inside = false; 1 => codes [10]; }
            }
            fallback => texts [10];
        }
        blocks { fallback => paragraphs [10]; }
    });

    #[test]
    fn underscore_means_no_check() {
        assert_eq!(UnlimitedParser::parse(b"123456789").unwrap().texts.len(), 1);
        assert!(UnlimitedParser::find_codes(b"123456789").is_ok());
        assert!(UnlimitedParser::context(b"123456789").is_ok());
    }
}
