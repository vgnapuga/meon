use meon::define_parser;

define_parser!(Demo {
    sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
    limits {
        max_spans = 1;
    }
    inline { fallback => texts [10]; }
});

fn main() {}
