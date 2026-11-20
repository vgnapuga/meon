use meon::define_parser;

define_parser!(Demo {
    sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
    limits {
        offset = u8;
        max_inline_span = 300;
    }
    inline { fallback => texts [10]; }
});

fn main() {}
