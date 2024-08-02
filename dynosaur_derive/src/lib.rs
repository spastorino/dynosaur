use crate::expand::expand_trait;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, parse_quote, Error, FnArg, GenericParam, Ident, ItemTrait, Pat, PatType,
    Result, TraitItem, TraitItemConst, TraitItemFn, TraitItemType, TypeGenerics, TypeParam,
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
    let erased_trait_blanket_impl = mk_erased_trait_blanket_impl(&item_trait.ident, &erased_trait);

    quote! {
        #item_trait

        #erased_trait
        #erased_trait_blanket_impl
    }
    .into()
}

fn mk_erased_trait(item_trait: &ItemTrait) -> ItemTrait {
    ItemTrait {
        ident: Ident::new(&format!("Erased{}", item_trait.ident), Span::call_site()),
        ..item_trait.clone()
    }
}

fn mk_erased_trait_blanket_impl(trait_ident: &Ident, erased_trait: &ItemTrait) -> TokenStream {
    let erased_trait_ident = &erased_trait.ident;
    let (_, trait_generics, _) = &erased_trait.generics.split_for_impl();
    let items = erased_trait
        .items
        .iter()
        .map(|item| blanket_impl_item(item, trait_ident, trait_generics));
    let blanket_bound: TypeParam = parse_quote!(DYNOSAUR: #trait_ident #trait_generics);
    let blanket = &blanket_bound.ident.clone();
    let mut blanket_generics = erased_trait.generics.clone();
    blanket_generics
        .params
        .push(GenericParam::Type(blanket_bound));
    let (blanket_impl_generics, _, blanket_where_clause) = &blanket_generics.split_for_impl();
    quote! {
        impl #blanket_impl_generics #erased_trait_ident #trait_generics for #blanket #blanket_where_clause
        {
            #(#items)*
        }
    }
}

fn blanket_impl_item(
    item: &TraitItem,
    trait_ident: &Ident,
    trait_generics: &TypeGenerics<'_>,
) -> TokenStream {
    match item {
        TraitItem::Const(TraitItemConst {
            ident,
            generics,
            ty,
            ..
        }) => {
            quote! {
                const #ident #generics: #ty = <Self as #trait_ident #trait_generics>::#ident;
            }
        }
        TraitItem::Fn(TraitItemFn { sig, .. }) => {
            let ident = &sig.ident;
            let args = sig.inputs.iter().map(|arg| match arg {
                FnArg::Receiver(_) => quote! { self },
                FnArg::Typed(PatType { pat, .. }) => match &**pat {
                    Pat::Ident(arg) => quote! { #arg },
                    _ => Error::new_spanned(pat, "patterns are not supported in arguments")
                        .to_compile_error(),
                },
            });
            quote! {
                #sig {
                    Box::pin(<Self as #trait_ident #trait_generics>::#ident(#(#args),*))
                }
            }
        }
        TraitItem::Type(TraitItemType {
            ident, generics, ..
        }) => {
            let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
            quote! {
                type #ident #impl_generics = <Self as #trait_ident #trait_generics>::#ident #ty_generics #where_clause;
            }
        }
        _ => Error::new_spanned(item, "unsupported item type").into_compile_error(),
    }
}
