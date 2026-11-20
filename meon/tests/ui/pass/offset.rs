use meon::define_parser;

mod u8_grammar {
    use super::*;

    define_parser!(U8 {
        sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
        limits { offset = u8; }
        inline { fallback => texts [10]; }
        blocks { fallback => paragraphs [10]; }
    });
}

mod u16_grammar {
    use super::*;

    define_parser!(U16 {
        sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
        limits { offset = u16; }
        inline { fallback => texts [10]; }
        blocks { fallback => paragraphs [10]; }
    });
}

mod u32_grammar {
    use super::*;

    define_parser!(U32 {
        sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
        limits { offset = u32; }
        inline { fallback => texts [10]; }
        blocks { fallback => paragraphs [10]; }
    });
}

mod u64_grammar {
    use super::*;

    define_parser!(U64 {
        sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
        limits { offset = u64; }
        inline { fallback => texts [10]; }
        blocks { fallback => paragraphs [10]; }
    });
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

fn main() {}
