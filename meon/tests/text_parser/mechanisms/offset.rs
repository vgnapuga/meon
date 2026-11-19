use meon::define_parser;
use meon::span::Span;

mod u8_grammar {
    use super::*;

    define_parser!(U8 {
        sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
        limits {
            offset = u8;
            max_nest = 2;
        }
        inline {
            on_trigger(b'*') {
                symmetric b'*' {
                    parse_inside = true;
                    1 => italics [10];
                }
            }
            fallback => texts [10];
        }
        blocks { fallback => paragraphs [10]; }
    });

    #[test]
    fn parse_truncates_at_u8_max() {
        let src = vec![b'a'; 256];
        let c = U8Parser::parse(&src).unwrap();
        assert_eq!(c.texts.len(), 1);
        assert_eq!(c.texts[0].end, 255u8);
    }

    #[test]
    fn find_returns_u8_spans() {
        let src = b"*a*";
        let spans: Vec<Span<u8>> = U8Parser::find_italics(src).unwrap().collect();
        assert_eq!(spans, vec![Span::<u8>::from_parts(1, 2)]);
    }

    #[test]
    fn context_finders_return_u8_spans() {
        let src = b"*a*";
        let ctx = U8Parser::context(src).unwrap();
        let spans: Vec<Span<u8>> = U8Parser::find_context_italics(src, &ctx).unwrap().collect();
        assert_eq!(spans, vec![Span::<u8>::from_parts(1, 2)]);
    }

    #[test]
    fn unchecked_parse_truncates_too() {
        let src = vec![b'a'; 256];
        let c = U8Parser::parse_unchecked(&src);
        assert_eq!(c.texts[0].end, 255u8);
    }
}

mod u8_limit {
    use super::*;

    define_parser!(U8Limit {
        sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
        limits {
            offset = u8;
            max_input_bytes = 10;
        }
        inline { fallback => texts [10]; }
        blocks { fallback => paragraphs [10]; }
    });

    #[test]
    fn limit_is_checked_before_truncation() {
        let err = U8LimitParser::parse(b"01234567890").err().unwrap();
        assert_eq!(err.len, 11);
        assert_eq!(err.max, 10u8);
    }

    #[test]
    fn at_the_limit_is_ok() {
        assert!(U8LimitParser::parse(b"0123456789").is_ok());
    }
}

mod u8_zero {
    use super::*;

    define_parser!(U8Zero {
        sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
        limits {
            offset = u8;
            max_input_bytes = 0;
        }
        inline { fallback => texts [10]; }
        blocks { fallback => paragraphs [10]; }
    });

    #[test]
    fn zero_is_the_type_maximum() {
        let err = U8ZeroParser::parse(&vec![b'a'; 256]).err().unwrap();
        assert_eq!(err.max, u8::MAX);
    }
}

mod u16_grammar {
    use super::*;

    define_parser!(U16 {
        sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
        limits { offset = u16; }
        inline { fallback => texts [10]; }
        blocks { fallback => paragraphs [10]; }
    });

    #[test]
    fn parse_truncates_at_u16_max() {
        let src = vec![b'a'; 65_536];
        let c = U16Parser::parse(&src).unwrap();
        assert_eq!(c.texts[0].end, u16::MAX);
    }
}

mod u64_grammar {
    use super::*;

    define_parser!(U64 {
        sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
        limits { offset = u64; }
        inline {
            on_trigger(b'`') {
                symmetric b'`' { parse_inside = false; 1 => codes [10]; }
            }
            fallback => texts [10];
        }
        blocks { fallback => paragraphs [10]; }
    });

    #[test]
    fn spans_use_u64() {
        let spans: Vec<Span<u64>> = U64Parser::find_codes(b"`a`").unwrap().collect();
        assert_eq!(spans, vec![Span::<u64>::from_parts(1, 2)]);
    }
}

mod u64_limits {
    use super::*;

    define_parser!(U64Limits {
        sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
        limits {
            offset = u64;
            max_inline_span = 5_000_000_000;
            max_input_bytes = 5_000_000_000;
        }
        inline { fallback => texts [10]; }
        blocks { fallback => paragraphs [10]; }
    });
}
