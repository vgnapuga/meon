use meon::define_parser;

define_parser!(Demo {
    sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
    limits {
        max_block_span = 0;
        max_nest = 2;
        max_inline_span = 1_024;
    }
    inline { fallback => texts [10]; }
    blocks { fallback => paragraphs [10]; }
});

fn main() {}
