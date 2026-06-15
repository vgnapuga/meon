use meon::define_parser;

define_parser!(Demo {
    sap = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
});

fn main() {}
