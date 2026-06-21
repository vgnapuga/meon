//! Back-end: emit the inherent `impl` for the content struct — the `str` /
//! `bytes` helpers plus a `_clean` / `_raw` accessor pair per standalone rule,
//! and a `_clean` accessor for every simple field that has no standalone rule.

use std::collections::HashSet;

use proc_macro2::{Ident, TokenStream as TS2};
use quote::quote;

use crate::model::{CF, StandaloneRule, append_ident, crate_path, standalone_field};

/// Emit `impl<'a> Name<'a> { ... }` with all accessor methods.
pub(crate) fn build_content_methods(
    name: &Ident,
    cf: &CF,
    sep: &impl quote::ToTokens,
    _eol: &impl quote::ToTokens,
    tab: &impl quote::ToTokens,
    _esc: &impl quote::ToTokens,
) -> TS2 {
    let mc = crate_path();

    // Returns `None` on invalid UTF-8 instead of panicking. Panicking inside a
    // library on user-controlled input is unsound: callers that build spans from
    // outside sources (or hit a parser bug) would get an unrecoverable crash.
    let str_fn = quote! {
        #[inline]
        pub fn str(&self, span: #mc::span::Span) -> ::core::option::Option<&str> {
            ::core::str::from_utf8(
                &self.source[span.start as usize..span.end as usize]
            ).ok()
        }
    };

    let bytes_fn = quote! {
        #[inline]
        pub fn bytes(&self, span: #mc::span::Span) -> &[u8] {
            &self.source[span.start as usize..span.end as usize]
        }
    };

    let standalone_names: HashSet<String> = cf
        .standalone
        .iter()
        .map(|r| standalone_field(r).to_string())
        .collect();

    let standalone_methods = cf.standalone.iter().map(|rule| match rule {
        StandaloneRule::SymmetricExact { field, count, .. } => {
            let clean = append_ident(field, "_clean");
            let raw = append_ident(field, "_raw");
            quote! {
                pub fn #clean(&self) -> impl ::std::iter::Iterator<Item = &[u8]> + '_ {
                    let source = self.source;
                    self.#field.iter().map(move |s| {
                        &source[s.start as usize..s.end as usize]
                    })
                }
                /// Returns the inner slice including the surrounding delimiter
                /// characters. Uses `saturating_sub` so a span that starts at
                /// the very beginning of the buffer never underflows.
                pub fn #raw(&self) -> impl ::std::iter::Iterator<Item = &[u8]> + '_ {
                    let source = self.source;
                    self.#field.iter().map(move |s| {
                        let start = (s.start as usize).saturating_sub(#count as usize);
                        let end   = (s.end as usize + #count as usize).min(source.len());
                        &source[start..end]
                    })
                }
            }
        }

        StandaloneRule::AsymmetricExact { field, count, .. } => {
            let clean = append_ident(field, "_clean");
            let raw = append_ident(field, "_raw");
            quote! {
                pub fn #clean(&self) -> impl ::std::iter::Iterator<Item = &[u8]> + '_ {
                    let source = self.source;
                    self.#field.iter().map(move |s| {
                        &source[s.start as usize..s.end as usize]
                    })
                }
                pub fn #raw(&self) -> impl ::std::iter::Iterator<Item = &[u8]> + '_ {
                    let source = self.source;
                    self.#field.iter().map(move |s| {
                        let start = (s.start as usize).saturating_sub(#count as usize);
                        let end   = (s.end as usize + 1).min(source.len());
                        &source[start..end]
                    })
                }
            }
        }

        StandaloneRule::Chained {
            field, pf, ff, sf, ..
        } => {
            let clean = append_ident(field, "_clean");
            let raw = append_ident(field, "_raw");
            quote! {
                pub fn #clean(&self) -> impl ::std::iter::Iterator<Item = (&[u8], &[u8])> + '_ {
                    let source = self.source;
                    self.#field.iter().map(move |el| (
                        &source[el.#ff.start as usize..el.#ff.end as usize],
                        &source[el.#sf.start as usize..el.#sf.end as usize],
                    ))
                }
                pub fn #raw(&self) -> impl ::std::iter::Iterator<Item = &[u8]> + '_ {
                    let source = self.source;
                    self.#field.iter().map(move |el| {
                        // Subtract open-bracket (1) and optional prefix flag (0 or 1).
                        // saturating_sub prevents underflow when the match sits at offset 0.
                        let prefix_len = el.#pf as usize;
                        let start = (el.#ff.start as usize)
                            .saturating_sub(1)
                            .saturating_sub(prefix_len);
                        let end = (el.#sf.end as usize + 1).min(source.len());
                        &source[start..end]
                    })
                }
            }
        }

        StandaloneRule::KeyValue { field, kf, vf, .. } => {
            let clean = append_ident(field, "_clean");
            let raw = append_ident(field, "_raw");
            quote! {
                pub fn #clean(&self) -> impl ::std::iter::Iterator<Item = (&[u8], &[u8])> + '_ {
                    let source = self.source;
                    self.#field.iter().map(move |el| (
                        &source[el.#kf.start as usize..el.#kf.end as usize],
                        &source[el.#vf.start as usize..el.#vf.end as usize],
                    ))
                }
                pub fn #raw(&self) -> impl ::std::iter::Iterator<Item = &[u8]> + '_ {
                    let source = self.source;
                    self.#field.iter().map(move |el| {
                        &source[el.#kf.start as usize..el.#vf.end as usize]
                    })
                }
            }
        }

        StandaloneRule::LineMarker { field, byte, .. } => {
            let clean = append_ident(field, "_clean");
            let raw = append_ident(field, "_raw");
            quote! {
                pub fn #clean(&self) -> impl ::std::iter::Iterator<Item = &[u8]> + '_ {
                    let source = self.source;
                    self.#field.iter().map(move |(_, s)| {
                        &source[s.start as usize..s.end as usize]
                    })
                }
                pub fn #raw(&self) -> impl ::std::iter::Iterator<Item = &[u8]> + '_ {
                    let source = self.source;
                    self.#field.iter().map(move |(_, s)| {
                        let mut start = s.start as usize;
                        if start > 0 && source[start - 1] == #sep {
                            start -= 1;
                        }
                        while start > 0 && source[start - 1] == #byte {
                            start -= 1;
                        }
                        &source[start..s.end as usize]
                    })
                }
            }
        }

        StandaloneRule::LineUniform { field, .. } => {
            let clean = append_ident(field, "_clean");
            let raw = append_ident(field, "_raw");
            quote! {
                pub fn #clean(&self) -> impl ::std::iter::Iterator<Item = &[u8]> + '_ {
                    let source = self.source;
                    self.#field.iter().map(move |(_, s)| {
                        &source[s.start as usize..s.end as usize]
                    })
                }
                pub fn #raw(&self) -> impl ::std::iter::Iterator<Item = &[u8]> + '_ {
                    let source = self.source;
                    self.#field.iter().map(move |(_, s)| {
                        &source[s.start as usize..s.end as usize]
                    })
                }
            }
        }

        StandaloneRule::Fence { field, .. } => {
            let clean = append_ident(field, "_clean");
            let raw = append_ident(field, "_raw");
            quote! {
                pub fn #clean(&self) -> impl ::std::iter::Iterator<Item = &[u8]> + '_ {
                    let source = self.source;
                    self.#field.iter().map(move |s| {
                        &source[s.start as usize..s.end as usize]
                    })
                }
                pub fn #raw(&self) -> impl ::std::iter::Iterator<Item = &[u8]> + '_ {
                    let source = self.source;
                    self.#field.iter().map(move |s| {
                        &source[s.start as usize..s.end as usize]
                    })
                }
            }
        }

        StandaloneRule::Cont { field, .. } => {
            let clean = append_ident(field, "_clean");
            let raw = append_ident(field, "_raw");
            quote! {
                pub fn #clean(&self) -> impl ::std::iter::Iterator<Item = &[u8]> + '_ {
                    let source = self.source;
                    self.#field.iter().map(move |s| {
                        &source[s.start as usize..s.end as usize]
                    })
                }
                pub fn #raw(&self) -> impl ::std::iter::Iterator<Item = &[u8]> + '_ {
                    let source = self.source;
                    self.#field.iter().map(move |s| {
                        &source[s.start as usize..s.end as usize]
                    })
                }
            }
        }

        StandaloneRule::BlockMarker { field, .. } => {
            let clean = append_ident(field, "_clean");
            let raw = append_ident(field, "_raw");
            quote! {
                pub fn #clean(&self) -> impl ::std::iter::Iterator<Item = &[u8]> + '_ {
                    let source = self.source;
                    self.#field.iter().map(move |(_, s)| {
                        &source[s.start as usize..s.end as usize]
                    })
                }
                /// Raw span includes the marker byte and its separator.
                /// `saturating_sub(2)` prevents underflow on pathological spans.
                pub fn #raw(&self) -> impl ::std::iter::Iterator<Item = &[u8]> + '_ {
                    let source = self.source;
                    self.#field.iter().map(move |(_, s)| {
                        let start = (s.start as usize).saturating_sub(2);
                        &source[start..s.end as usize]
                    })
                }
            }
        }

        StandaloneRule::BlockNumbered { field, .. } => {
            let clean = append_ident(field, "_clean");
            let raw = append_ident(field, "_raw");
            quote! {
                pub fn #clean(&self) -> impl ::std::iter::Iterator<Item = &[u8]> + '_ {
                    let source = self.source;
                    self.#field.iter().map(move |(_, s)| {
                        &source[s.start as usize..s.end as usize]
                    })
                }
                pub fn #raw(&self) -> impl ::std::iter::Iterator<Item = &[u8]> + '_ {
                    let source = self.source;
                    self.#field.iter().map(move |(_, s)| {
                        let mut start = s.start as usize;
                        if start > 0
                            && (source[start - 1] == #sep || source[start - 1] == #tab)
                        {
                            start -= 1;
                        }
                        if start > 0 {
                            start -= 1;
                        }
                        while start > 0 && source[start - 1].is_ascii_digit() {
                            start -= 1;
                        }
                        &source[start..s.end as usize]
                    })
                }
            }
        }
    });

    let simple_methods = cf
        .inline_simple
        .iter()
        .chain(cf.block_simple.iter())
        .filter(|(f, _)| !standalone_names.contains(&f.to_string()))
        .map(|(f, _)| {
            let clean = append_ident(f, "_clean");
            quote! {
                pub fn #clean(&self) -> impl ::std::iter::Iterator<Item = &[u8]> + '_ {
                    let source = self.source;
                    self.#f.iter().map(move |s| &source[s.start as usize..s.end as usize])
                }
            }
        });

    quote! {
        impl<'a> #name<'a> {
            #str_fn
            #bytes_fn
            #(#standalone_methods)*
            #(#simple_methods)*
        }
    }
}
