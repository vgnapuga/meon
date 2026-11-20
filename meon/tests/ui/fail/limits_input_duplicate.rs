use meon::define_parser;

define_parser!(Demo {
    sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
    limits {
        max_input_bytes = 1;
        max_input_bytes = _;
    }
    inline { fallback => texts [10]; }
});

fn main() {}
