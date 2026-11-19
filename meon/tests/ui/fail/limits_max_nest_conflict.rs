use meon::define_parser;

define_parser!(Demo {
    sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\', max_nest = 2;
    limits {
        max_nest = 3;
    }
    inline { fallback => texts [10]; }
});

fn main() {}
