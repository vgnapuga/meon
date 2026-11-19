use meon::define_parser;

mod unlimited {
    use super::*;

    define_parser!(Unlimited {
        sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
        limits {
            max_input_bytes = _;
        }
        inline { fallback => texts [10]; }
        blocks { fallback => paragraphs [10]; }
    });
}

mod maximum {
    use super::*;

    define_parser!(Maximum {
        sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
        limits {
            max_input_bytes = 0;
        }
        inline { fallback => texts [10]; }
        blocks { fallback => paragraphs [10]; }
    });
}

fn main() {}
