use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, parse_quote, Ident, ItemTrait, Result, ReturnType, Signature, Token,
    TraitItem, TraitItemFn,
};

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
    let attrs = parse_macro_input!(attr as Attrs);
    let item = parse_macro_input!(item as ItemTrait);

    let erased_trait = mk_erased_trait(&attrs, &item);

    quote! {
        #item

        #erased_trait
    }
    .into()
}

fn mk_erased_trait(attrs: &Attrs, item: &ItemTrait) -> TokenStream {
    let erased_trait = ItemTrait {
        ident: attrs.ident.clone(),
        items: item
            .items
            .iter()
            .map(|item| {
                if let TraitItem::Fn(
                    trait_item_fn @ TraitItemFn {
                        sig:
                            Signature {
                                asyncness: Some(..),
                                ..
                            },
                        ..
                    },
                ) = item
                {
                    let (ret_arrow, ret) = match &trait_item_fn.sig.output {
                        ReturnType::Default => (Token![->](Span::call_site()), quote!(())),
                        ReturnType::Type(arrow, ret) => (*arrow, quote!(#ret)),
                    };
                    TraitItem::Fn(TraitItemFn {
                        sig: Signature {
                            asyncness: None,
                            output: parse_quote! {
                                #ret_arrow ::core::pin::Pin<Box<
                                dyn ::core::future::Future<Output = #ret>>>
                            },
                            ..trait_item_fn.sig.clone()
                        },
                        ..trait_item_fn.clone()
                    })
                } else {
                    item.clone()
                }
            })
            .collect(),
        ..item.clone()
    };
    quote! { #erased_trait }
}
