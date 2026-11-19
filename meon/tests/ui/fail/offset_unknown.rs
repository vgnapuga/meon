use meon::define_parser;

define_parser!(Demo {
    sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
    limits { offset = u128; }
    inline { fallback => texts [10]; }
});

fn main() {}
