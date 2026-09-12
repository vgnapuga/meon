use meon::define_parser;

define_parser!(Demo {
    sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
    inline {
        on_trigger(b'*') {
            symmetric b'*' { parse_inside = true; 1 => italics [40], }
        }
        fallback => texts [10];
    }
});

fn main() {}
