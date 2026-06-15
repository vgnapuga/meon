//! Content struct generator — the `define_content!` macro.
//!
//! # Purpose
//!
//! `define_content!` is the bridge between the grammar DSL and the runtime
//! output. It generates two paired types from a single grammar description:
//!
//! - `<Name>State` — a mutable accumulator used *during* parsing. All fields
//!   are `pub(crate)` and pre-allocated with capacity hints derived from the
//!   source length divided by a per-field divisor.
//! - `<Name>` (the content struct) — the immutable output handed to the caller
//!   after parsing completes. All fields are `pub`.
//!
//! The two types share identical field names and element types; the state is
//! converted into content via `into_content(source)` at the end of the parse.
//!
//! # Field categories
//!
//! The macro accepts five named sections. Each section determines both the
//! storage layout and the parsing rules that populate it:
//!
//! ## `inline { field: Type [div] }`
//!
//! Stores `Vec<Type>` — a user-defined struct carrying multiple [`Span`] fields.
//! Used for inline constructs that have *more than one* span component, such as
//! a link (`text` span + `url` span) or a key-value pair (`key` span +
//! `value` span). The type must be defined by the grammar author.
//!
//! ## `inline_simple { field [div] }`
//!
//! Stores `Vec<Span>` — a plain byte-range with no additional metadata.
//! Used for single-span inline constructs such as bold, italic, code, autolinks,
//! and plain text runs. These fields support optional *merge* behaviour: adjacent
//! spans separated by at most one byte are coalesced into a single span via
//! `push_merge_<field>` (controlled by `merge_simple` in the grammar).
//!
//! ## `line { field: Type [div] }`
//!
//! Stores `Vec<(Type, Span)>` — metadata paired with a content span.
//! Used for whole-line constructs that carry per-element metadata in addition to
//! a content range. The distinction from `block` is purely in parsing rules:
//! line rules (`parse_line!`) match in a single pass at the start of a line
//! without any multi-line state. Examples: headings (level + content span),
//! thematic breaks (delimiter kind + line span).
//!
//! ## `block { field: Type [div] }`
//!
//! Stores `Vec<(Type, Span)>` — same layout as `line`.
//! Used for constructs where per-line metadata is needed but the parsing rule
//! lives in `parse_block!`. Block rules can interact with the active-block slot
//! and are tried after line rules. Examples: bullet list items (marker kind +
//! content span), ordered list items (number + delimiter kind + content span).
//!
//! ## `block_simple { field [div] }`
//!
//! Stores `Vec<Span>` — same layout as `inline_simple`.
//! Used for multi-line block constructs that need only a span with no per-line
//! metadata. Examples: fenced code blocks (entire block as one span),
//! blockquote runs (entire continuation as one span), paragraphs, hard breaks.
//!
//! # Capacity divisors
//!
//! Each field carries a `[div]` literal. The initial `Vec` capacity is
//! `source.len() / div`. This is a heuristic — a divisor of `10` means
//! "expect roughly one element per 10 bytes of source". Tune based on the
//! expected density of each element in real inputs. Over-allocating wastes
//! memory; under-allocating causes reallocation during parsing.
//!
//! # Generated API surface
//!
//! For every `inline_simple` field `f` the macro also emits:
//!
//! - `push_f(&mut self, Span)` — plain append.
//! - `push_merge_f(&mut self, Span)` — append with coalescing: if the last span
//!   is non-empty, the new span is non-empty, and they are adjacent (gap ≤ 1
//!   byte), the last span's end is extended instead of pushing a new entry.
//!
//! The merge variant is selected by `parse_text!` when `merge_simple = true` is
//! set in the grammar's `inline` section.
//!
//! # Example expansion
//!
//! Given:
//! ```text
//! define_content!(Demo {
//!     inline        { links: Link [100] }
//!     inline_simple { texts [10], bolds [40] }
//!     line          { headings: Heading [200] }
//!     block         { bullet_items: BulletItem [80] }
//!     block_simple  { paragraphs [80] }
//! });
//! ```
//!
//! The macro emits (schematically):
//! ```text
//! pub(crate) struct DemoState {
//!     pub(crate) links:        Vec<Link>,
//!     pub(crate) texts:        Vec<Span>,
//!     pub(crate) bolds:        Vec<Span>,
//!     pub(crate) headings:     Vec<(Heading, Span)>,
//!     pub(crate) bullet_items: Vec<(BulletItem, Span)>,
//!     pub(crate) paragraphs:   Vec<Span>,
//! }
//! pub struct Demo<'a> {
//!     pub source:       &'a [u8],
//!     pub links:        Vec<Link>,
//!     pub texts:        Vec<Span>,
//!     pub bolds:        Vec<Span>,
//!     pub headings:     Vec<(Heading, Span)>,
//!     pub bullet_items: Vec<(BulletItem, Span)>,
//!     pub paragraphs:   Vec<Span>,
//! }
//! ```
#[doc(hidden)]
#[macro_export]
macro_rules! define_content {
    ( $name:ident {
        inline {
            $( $inline_field:ident : $inline_ty:ty [$inline_div:literal] ),* $(,)?
        }
        inline_simple {
            $( $inline_simple_field:ident [$inline_simple_div:literal] ),* $(,)?
        }
        line {
            $( $line_field:ident : $line_ty:ty [$line_div:literal] ),* $(,)?
        }
        block {
            $( $block_field:ident : $block_ty:ty [$block_div:literal] ),* $(,)?
        }
        block_simple {
            $( $block_simple_field:ident [$simple_div:literal] ),* $(,)?
        }
    }) => {
        $crate::paste::paste! {
            pub(crate) struct [<$name State>] {
                $( pub(crate) $inline_field:        Vec<$inline_ty>, )*
                $( pub(crate) $inline_simple_field: Vec<$crate::span::Span>, )*
                $( pub(crate) $line_field:          Vec<($line_ty, $crate::span::Span)>, )*
                $( pub(crate) $block_field:         Vec<($block_ty, $crate::span::Span)>, )*
                $( pub(crate) $block_simple_field:  Vec<$crate::span::Span>, )*
            }

            impl [<$name State>] {
                pub(crate) fn new(n: usize) -> Self {
                    Self {
                        $( $inline_field:        Vec::with_capacity(n / $inline_div), )*
                        $( $inline_simple_field: Vec::with_capacity(n / $inline_simple_div), )*
                        $( $line_field:          Vec::with_capacity(n / $line_div), )*
                        $( $block_field:         Vec::with_capacity(n / $block_div), )*
                        $( $block_simple_field:  Vec::with_capacity(n / $simple_div), )*
                    }
                }

                $(
                    #[allow(dead_code)]
                    #[inline(always)]
                    pub(crate) fn [<push_ $inline_simple_field>](
                        &mut self, s: $crate::span::Span,
                    ) {
                        self.$inline_simple_field.push(s);
                    }

                    #[inline(always)]
                    pub(crate) fn [<push_merge_ $inline_simple_field>](
                        &mut self, s: $crate::span::Span,
                    ) {
                        let vec = &mut self.$inline_simple_field;
                        if let Some(last) = vec.last_mut() {
                            if last.start != last.end && s.start != s.end {
                                if s.start.saturating_sub(last.end) <= 1 {
                                    last.end = s.end;
                                    return;
                                }
                            }
                        }
                        vec.push(s);
                    }
                )*

                pub(crate) fn into_content<'a>(self, source: &'a [u8]) -> $name<'a> {
                    $name {
                        source,
                        $( $inline_field:        self.$inline_field, )*
                        $( $inline_simple_field: self.$inline_simple_field, )*
                        $( $line_field:          self.$line_field, )*
                        $( $block_field:         self.$block_field, )*
                        $( $block_simple_field:  self.$block_simple_field, )*
                    }
                }
            }

            pub(crate) type ParseState = [<$name State>];

            #[allow(missing_docs)]
            pub struct $name<'a> {
                pub source: &'a [u8],
                $( pub $inline_field:        Vec<$inline_ty>, )*
                $( pub $inline_simple_field: Vec<$crate::span::Span>, )*
                $( pub $line_field:          Vec<($line_ty, $crate::span::Span)>, )*
                $( pub $block_field:         Vec<($block_ty, $crate::span::Span)>, )*
                $( pub $block_simple_field:  Vec<$crate::span::Span>, )*
            }
        }
    };
}
