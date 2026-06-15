use meon::define_parser;

define_parser!(Demo {
    sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
    blocks { block_simple { fence(b'`') => fenced_codes [400]; } }
});

fn main() {}
