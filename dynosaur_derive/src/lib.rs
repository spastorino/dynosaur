use crate::expand::expand_trait;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Ident, ItemTrait, Result,
};

mod expand;
mod lifetime;
mod receiver;

struct Attrs {
    ident: Ident,
}

impl Parse for Attrs {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Attrs {
            ident: input.parse()?,
        })
    }
}

/// Given a trait like
///
/// ```rust,ignore
/// use dynosaur::dynosaur;
///
/// #[dynosaur(DynMyTrait)]
/// trait MyTrait {
///     type Item;
///     async fn foo(&self) -> Self::Item;
/// }
/// ```
///
/// The above example causes the trait to be rewritten as:
///
/// ```rust,ignore
/// trait DynMyTrait {
///     type Item;
///     fn foo(&self) -> ::core::pin::Pin<Box<dyn ::core::future::Future<Output = Self::Item>>>;
/// }
/// ```
#[proc_macro_attribute]
pub fn dynosaur(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let _attrs = parse_macro_input!(attr as Attrs);
    let item_trait = parse_macro_input!(item as ItemTrait);

    let expanded_trait = expand_trait(&item_trait);
    let erased_trait = mk_erased_trait(&expanded_trait);

    quote! {
        #item_trait

        #erased_trait
    }
    .into()
}

fn mk_erased_trait(item_trait: &ItemTrait) -> ItemTrait {
    ItemTrait {
        ident: Ident::new(&format!("Erased{}", item_trait.ident), Span::call_site()),
        ..item_trait.clone()
    }
}
