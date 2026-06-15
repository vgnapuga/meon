// Cross-crate smoke test: define_parser! must work with only `use meon::define_parser`
// in scope — no additional imports of parse_text!, define_content!, etc.
// If this compiles, all macro emissions are correctly qualified and the
// future meon-md split will be purely mechanical.
use meon::define_parser;

define_parser! {
    Mini {
        sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
        inline {
            fallback => texts [10];
        }
        blocks {
            fallback => paragraphs [80];
        }
    }
}

fn main() {
    let _ = MiniParser::parse(b"hello world\n");
}
