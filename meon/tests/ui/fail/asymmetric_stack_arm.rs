use meon::define_parser;

define_parser!(Demo {
    sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
    inline {
        on_trigger(b'{', b'}') {
            asymmetric b'{', b'}' { balanced = true; parse_inside = true; 2 => pairs [40], }
        }
        fallback => texts [10];
    }
});

fn main() {}
