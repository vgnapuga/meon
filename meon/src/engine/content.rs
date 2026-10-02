//! Content struct generator - the `define_content!` macro.
//!
//! # Purpose
//!
//! `define_content!` is the bridge between the grammar DSL and the runtime
//! output. It generates two paired types from a single grammar description:
//!
//! - `<Name>State` - a mutable accumulator used *during* parsing. All fields
//!   are `pub(crate)`; each starts unallocated and reserves capacity derived
//!   from the source length and a per-field divisor on its first element.
//! - `<Name>` (the content struct) - the immutable output handed to the caller
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
//! Stores `Vec<Type>` - a user-defined struct carrying multiple [`Span`](crate::span::Span) fields.
//! Used for inline constructs that have *more than one* span component, such as
//! a link (`text` span + `url` span) or a key-value pair (`key` span +
//! `value` span). The type must be defined by the grammar author.
//!
//! ## `inline_simple { field [div] }`
//!
//! Stores `Vec<Span>` - a plain byte-range with no additional metadata.
//! Used for single-span inline constructs such as bold, italic, code, autolinks,
//! and plain text runs. These fields support optional *merge* behaviour: adjacent
//! spans separated by at most one byte are coalesced into a single span via
//! `push_merge_<field>` (controlled by `merge_simple` in the grammar).
//!
//! ## `line { field: Type [div] }`
//!
//! Stores `Vec<(Type, Span)>` - metadata paired with a content span.
//! Used for whole-line constructs that carry per-element metadata in addition to
//! a content range. The distinction from `block` is purely in parsing rules:
//! line rules (`parse_line!`) match in a single pass at the start of a line
//! without any multi-line state. Examples: headings (level + content span),
//! thematic breaks (delimiter kind + line span).
//!
//! ## `block { field: Type [div] }`
//!
//! Stores `Vec<(Type, Span)>` - same layout as `line`.
//! Used for constructs where per-line metadata is needed but the parsing rule
//! lives in `parse_block!`. Block rules can interact with the active-block slot
//! and are tried after line rules. Examples: bullet list items (marker kind +
//! content span), ordered list items (number + delimiter kind + content span).
//!
//! ## `block_simple { field [div] }`
//!
//! Stores `Vec<Span>` - same layout as `inline_simple`.
//! Used for multi-line block constructs that need only a span with no per-line
//! metadata. Examples: fenced code blocks (entire block as one span),
//! blockquote runs (entire continuation as one span), paragraphs, hard breaks.
//!
//! # Capacity divisors
//!
//! Each field carries a `[div]` literal. A field's `Vec` starts with no
//! allocation and reserves `source.len() / div` elements when its first
//! element arrives, so a rule that never matches costs nothing at all. The
//! divisor is a heuristic - `10` means "expect roughly one element per 10
//! bytes of source". Tune it to the expected density in real inputs:
//! over-allocating wastes memory, under-allocating causes reallocation
//! during parsing.
//!
//! # Generated API surface
//!
//! For every field `f` the macro emits `push_f` - the single append entry
//! point. Its parameter follows the storage layout: `Span` for the `*_simple`
//! categories, `Type` for `inline`, `(Type, Span)` for `line` and `block`.
//!
//! `push_f` makes the one test `Vec::push` makes anyway, `len == capacity`.
//! While there is room it pushes directly, and the compiler drops the
//! second, identical test inside `Vec::push`. A full vector - which includes
//! one that was never allocated - goes to `push_cold_f`, an out-of-line
//! `#[cold]` function that reserves the `[div]` hint when the capacity is
//! still zero and then pushes, letting `Vec` grow as usual otherwise. The
//! lazy reservation therefore adds no instruction to the path a push takes
//! almost every time.
//!
//! Every `inline_simple` field gets one more:
//!
//! - `push_merge_f(&mut self, Span)` - append with coalescing: if the last span
//!   is non-empty, the new span is non-empty, and they are adjacent (gap <= 1
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
                #[allow(dead_code)]
                src_len: usize,
                $( pub(crate) $inline_field:        Vec<$inline_ty>, )*
                $( pub(crate) $inline_simple_field: Vec<$crate::span::Span>, )*
                $( pub(crate) $line_field:          Vec<($line_ty, $crate::span::Span)>, )*
                $( pub(crate) $block_field:         Vec<($block_ty, $crate::span::Span)>, )*
                $( pub(crate) $block_simple_field:  Vec<$crate::span::Span>, )*
            }

            impl [<$name State>] {
                pub(crate) fn new(n: usize) -> Self {
                    Self {
                        src_len: n,
                        $( $inline_field:        Vec::new(), )*
                        $( $inline_simple_field: Vec::new(), )*
                        $( $line_field:          Vec::new(), )*
                        $( $block_field:         Vec::new(), )*
                        $( $block_simple_field:  Vec::new(), )*
                    }
                }

                $(
                    #[allow(dead_code)]
                    #[inline(always)]
                    pub(crate) fn [<push_ $inline_field>](&mut self, v: $inline_ty) {
                        if self.$inline_field.len() == self.$inline_field.capacity() {
                            self.[<push_cold_ $inline_field>](v);
                        } else {
                            self.$inline_field.push(v);
                        }
                    }

                    #[allow(dead_code)]
                    #[cold]
                    #[inline(never)]
                    fn [<push_cold_ $inline_field>](&mut self, v: $inline_ty) {
                        if self.$inline_field.capacity() == 0 {
                            self.$inline_field.reserve_exact(self.src_len / $inline_div);
                        }
                        self.$inline_field.push(v);
                    }
                )*

                $(
                    #[allow(dead_code)]
                    #[inline(always)]
                    pub(crate) fn [<push_ $line_field>](
                        &mut self, v: ($line_ty, $crate::span::Span),
                    ) {
                        if self.$line_field.len() == self.$line_field.capacity() {
                            self.[<push_cold_ $line_field>](v);
                        } else {
                            self.$line_field.push(v);
                        }
                    }

                    #[allow(dead_code)]
                    #[cold]
                    #[inline(never)]
                    fn [<push_cold_ $line_field>](
                        &mut self, v: ($line_ty, $crate::span::Span),
                    ) {
                        if self.$line_field.capacity() == 0 {
                            self.$line_field.reserve_exact(self.src_len / $line_div);
                        }
                        self.$line_field.push(v);
                    }
                )*

                $(
                    #[allow(dead_code)]
                    #[inline(always)]
                    pub(crate) fn [<push_ $block_field>](
                        &mut self, v: ($block_ty, $crate::span::Span),
                    ) {
                        if self.$block_field.len() == self.$block_field.capacity() {
                            self.[<push_cold_ $block_field>](v);
                        } else {
                            self.$block_field.push(v);
                        }
                    }

                    #[allow(dead_code)]
                    #[cold]
                    #[inline(never)]
                    fn [<push_cold_ $block_field>](
                        &mut self, v: ($block_ty, $crate::span::Span),
                    ) {
                        if self.$block_field.capacity() == 0 {
                            self.$block_field.reserve_exact(self.src_len / $block_div);
                        }
                        self.$block_field.push(v);
                    }
                )*

                $(
                    #[allow(dead_code)]
                    #[inline(always)]
                    pub(crate) fn [<push_ $block_simple_field>](
                        &mut self, s: $crate::span::Span,
                    ) {
                        if self.$block_simple_field.len()
                            == self.$block_simple_field.capacity()
                        {
                            self.[<push_cold_ $block_simple_field>](s);
                        } else {
                            self.$block_simple_field.push(s);
                        }
                    }

                    #[allow(dead_code)]
                    #[cold]
                    #[inline(never)]
                    fn [<push_cold_ $block_simple_field>](
                        &mut self, s: $crate::span::Span,
                    ) {
                        if self.$block_simple_field.capacity() == 0 {
                            self.$block_simple_field.reserve_exact(self.src_len / $simple_div);
                        }
                        self.$block_simple_field.push(s);
                    }
                )*

                $(
                    #[allow(dead_code)]
                    #[inline(always)]
                    pub(crate) fn [<push_ $inline_simple_field>](
                        &mut self, s: $crate::span::Span,
                    ) {
                        if self.$inline_simple_field.len()
                            == self.$inline_simple_field.capacity()
                        {
                            self.[<push_cold_ $inline_simple_field>](s);
                        } else {
                            self.$inline_simple_field.push(s);
                        }
                    }

                    #[allow(dead_code)]
                    #[cold]
                    #[inline(never)]
                    fn [<push_cold_ $inline_simple_field>](
                        &mut self, s: $crate::span::Span,
                    ) {
                        if self.$inline_simple_field.capacity() == 0 {
                            self.$inline_simple_field
                                .reserve_exact(self.src_len / $inline_simple_div);
                        }
                        self.$inline_simple_field.push(s);
                    }

                    #[inline(always)]
                    pub(crate) fn [<push_merge_ $inline_simple_field>](
                        &mut self, s: $crate::span::Span,
                    ) {
                        if let Some(last) = self.$inline_simple_field.last_mut() {
                            if last.start != last.end && s.start != s.end {
                                if s.start.saturating_sub(last.end) <= 1 {
                                    last.end = s.end;
                                    return;
                                }
                            }
                        }
                        self.[<push_ $inline_simple_field>](s);
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
