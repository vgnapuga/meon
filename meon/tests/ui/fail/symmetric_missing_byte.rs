use meon::define_parser;

define_parser!(Demo {
    sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
    inline { on_trigger(b'*') { symmetric { 1 => bolds [40], } } }
});

fn main() {}
