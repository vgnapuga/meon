use meon::define_parser;

define_parser!(Demo {
    sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';
    lines { line(b'#') |n|: Heading { level: n } => headings [200]; }
});

fn main() {}
