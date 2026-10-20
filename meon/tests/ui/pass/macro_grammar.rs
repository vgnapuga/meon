// A grammar written inside a `macro_rules!` macro: its settings, byte
// patterns and capacities arrive as `$x:literal` / `$x:pat` fragments, and a
// metadata body keeps an `$x:expr`'s precedence.
#![deny(warnings)]

#[derive(Debug, Clone, Copy)]
pub struct Item {
    pub kind: u8,
    pub weight: u8,
}

macro_rules! grammar {
    ($name:ident, $nest:literal, $em:literal, $pat:pat, $min:literal, $cap:literal, $w:expr) => {
        meon::define_parser!($name {
            sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\', max_nest = $nest;
            inline {
                on_trigger($em) {
                    symmetric $em { parse_inside = true; balanced = true; 1 => ems [$cap], }
                }
                fallback => texts [10];
            }
            blocks {
                block_simple { fence($pat, min = $min) => codes [40]; }
                block { ($pat) |b|: Item { kind: b, weight: $w * 2 } => items [40]; }
                fallback => paragraphs [80];
            }
        });
    };
}

grammar!(Wrapped, 4, b'*', b'~' | b'=', 3, 40, 1 + 1);

fn main() {
    let src = b"*a*\n~~~\nx\n~~~\n= item";
    let c = WrappedParser::parse(src);
    assert_eq!(c.ems.len(), 1);
    assert_eq!(c.codes.len(), 1);
    assert_eq!(c.items.len(), 1);
    assert_eq!(c.items[0].0.weight, 4);
    assert_eq!(WrappedParser::find_codes(src).count(), 1);
    assert_eq!(
        WrappedParser::find_items(src).next().map(|i| i.0.weight),
        Some(4)
    );
}
