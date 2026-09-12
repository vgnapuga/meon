use meon::define_parser;

pub struct Link {
    pub is_image: bool,
    pub text: meon::span::Span,
    pub url: meon::span::Span,
}

define_parser!(Demo {
    sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
    inline {
        on_trigger(b'[') {
            chained: Link {
                | b'[', b']' | { parse_inside = false; balanced = false; } => text,
                | b'(', b')' | { parse_inside = false; balanced = false; } => url,
            } => links [100]
        }
        fallback => texts [10];
    }
});

fn main() {}
