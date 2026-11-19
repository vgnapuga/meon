//! Intermediate representation exchanged between the front-end ([`crate::collect`])
//! and the back-end ([`crate::codegen`] / [`crate::methods`]), plus small shared
//! token helpers.

use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Ident, Literal, Span as PS, TokenStream as TS2};
use quote::quote;

/// Resolve the path to the runtime crate (the one exporting `span`,
/// `define_content!`, `define_standalone_fns!`, ...) as named in the dependent
/// crate's `Cargo.toml`.
///
/// Yields `crate` when expanded inside the runtime crate itself, otherwise the
/// imported crate identifier.
///
/// NOTE: the search string must match the runtime crate's `name` in
/// `Cargo.toml`. Keep it in sync if the package is renamed.
pub(crate) fn crate_path() -> TS2 {
    match crate_name("meon") {
        Ok(FoundCrate::Itself) => quote! { crate },
        Ok(FoundCrate::Name(name)) => {
            let i = Ident::new(&name, PS::call_site());
            quote! { #i }
        }
        Err(_) => quote! { crate },
    }
}

/// Build `<base><suffix>` as a new identifier, preserving `base`'s span so
/// diagnostics keep pointing at the user's field name.
pub(crate) fn append_ident(base: &Ident, suffix: &str) -> Ident {
    Ident::new(&format!("{}{}", base, suffix), base.span())
}

/// Return the destination field identifier carried by any [`StandaloneRule`].
pub(crate) fn standalone_field(rule: &StandaloneRule) -> &Ident {
    match rule {
        StandaloneRule::SymmetricExact { field, .. } => field,
        StandaloneRule::AsymmetricExact { field, .. } => field,
        StandaloneRule::Chained { field, .. } => field,
        StandaloneRule::KeyValue { field, .. } => field,
        StandaloneRule::LineMarker { field, .. } => field,
        StandaloneRule::LineUniform { field, .. } => field,
        StandaloneRule::Fence { field, .. } => field,
        StandaloneRule::Cont { field, .. } => field,
        StandaloneRule::BlockMarker { field, .. } => field,
        StandaloneRule::BlockNumbered { field, .. } => field,
    }
}

/// The settings of one `chained` component.
#[derive(Clone, Copy)]
pub(crate) struct ChainedFlags {
    pub(crate) parse_inside: bool,
    pub(crate) balanced: bool,
}

/// One standalone finder to generate.
///
/// Each variant mirrors a grammar construct and carries exactly the literals,
/// types and field identifiers needed to emit both its `find_*` iterator (via
/// `define_standalone_fns!`) and its `_clean` / `_raw` accessors.
pub(crate) enum StandaloneRule {
    SymmetricExact {
        field: Ident,
        byte: Literal,
        count: Literal,
        /// `true` when the grammar arm declares `parse_inside = false` —
        /// the rule is a context *source* and gets no `find_context_*` variant.
        opaque: bool,
        /// The field's `[cap]` divisor; opaque rules contribute it to the
        /// generated `context()`'s preallocation hint.
        cap: Literal,
    },
    AsymmetricExact {
        field: Ident,
        open: Literal,
        close: Literal,
        count: Literal,
        /// See [`StandaloneRule::SymmetricExact::opaque`].
        opaque: bool,
        /// The rule's `balanced` flag. With it or `parse_inside = true` each
        /// open byte is an open of its own, and `count` is `1`: on the full
        /// parse's stack when transparent, in one forward search that counts
        /// the rule's bytes when opaque.
        balanced: bool,
        /// See [`StandaloneRule::SymmetricExact::cap`].
        cap: Literal,
    },
    Chained {
        field: Ident,
        open1: Literal,
        close1: Literal,
        open2: Literal,
        close2: Literal,
        prefix: Literal,
        ty: TS2,
        pf: Ident,
        ff: Ident,
        sf: Ident,
        /// The `parse_inside` and `balanced` flags of the first component.
        /// With both components `parse_inside = false` the rule is a context
        /// source.
        first_flags: ChainedFlags,
        /// The `parse_inside` and `balanced` flags of the second component.
        second_flags: ChainedFlags,
        /// See [`StandaloneRule::SymmetricExact::cap`].
        cap: Literal,
    },
    KeyValue {
        field: Ident,
        eq: Literal,
        end: Literal,
        allow_sep: bool,
        ty: TS2,
        kf: Ident,
        vf: Ident,
    },
    LineMarker {
        field: Ident,
        byte: Literal,
        max: Literal,
        ty: TS2,
        var: Ident,
        body: TS2,
    },
    LineUniform {
        field: Ident,
        /// The byte pattern as written, e.g. `b'-' | b'*'`.
        pat: TS2,
        min: Literal,
        ty: TS2,
        var: Ident,
        body: TS2,
    },
    Fence {
        field: Ident,
        /// The byte pattern as written; a fence closes with the byte it
        /// opened with.
        pat: TS2,
        min: Literal,
        /// See [`StandaloneRule::SymmetricExact::cap`].
        cap: Literal,
    },
    Cont {
        field: Ident,
        byte: Literal,
    },
    BlockMarker {
        field: Ident,
        /// The marker byte pattern as written.
        pat: TS2,
        ty: TS2,
        var: Ident,
        body: TS2,
    },
    BlockNumbered {
        field: Ident,
        /// The digit and end byte patterns as written.
        digits: TS2,
        ends: TS2,
        ty: TS2,
        num_var: Ident,
        kind_var: Ident,
        body: TS2,
    },
}

/// Collected fields, accumulated while walking the grammar.
///
/// `collect_*` push into these vectors; the codegen/methods modules read them
/// back to emit the content struct, the parser body and the accessors.
#[derive(Default)]
pub(crate) struct CF {
    pub(crate) inline: Vec<(Ident, TS2, Literal)>,
    pub(crate) inline_simple: Vec<(Ident, Literal)>,
    pub(crate) line: Vec<(Ident, TS2, Literal)>,
    pub(crate) block: Vec<(Ident, TS2, Literal)>,
    pub(crate) block_simple: Vec<(Ident, Literal)>,
    pub(crate) standalone: Vec<StandaloneRule>,
}
