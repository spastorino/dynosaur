use crate::where_clauses::has_where_self_sized;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{punctuated::Punctuated, Ident, ItemTrait, Token, TraitItem, TraitItemType};

/// Remove Self: Sized fns
pub(crate) fn dyn_compatible_items(
    item_trait_items: &[TraitItem],
) -> impl Iterator<Item = &TraitItem> {
    item_trait_items
        .iter()
        .filter_map(|trait_item| match trait_item {
            TraitItem::Fn(trait_item_fn) if has_where_self_sized(&trait_item_fn.sig) => None,
            _ => Some(trait_item),
        })
}

pub(crate) fn trait_item_erased_name(trait_ident: &Ident) -> Ident {
    Ident::new(&format!("Erased{}", trait_ident), Span::call_site())
}

pub(crate) struct StructTraitParams {
    pub(crate) struct_params: TokenStream,
    pub(crate) struct_with_bounds_params: TokenStream,
    pub(crate) trait_params: TokenStream,
}

pub(crate) fn struct_trait_params(item_trait: &ItemTrait) -> StructTraitParams {
    let mut struct_params: Punctuated<_, Token![,]> = Punctuated::new();
    let mut struct_with_bounds_params: Punctuated<_, Token![,]> = Punctuated::new();
    let mut trait_params: Punctuated<_, Token![,]> = Punctuated::new();

    struct_params.push(quote! { 'dynosaur_struct });
    struct_with_bounds_params.push(quote! { 'dynosaur_struct });
    item_trait.generics.params.iter().for_each(|item| {
        struct_params.push(quote! { #item });
        struct_with_bounds_params.push(quote! { #item });
        trait_params.push(quote! { #item });
    });
    item_trait.items.iter().for_each(|item| match item {
        TraitItem::Type(TraitItemType { ident, bounds, .. }) => {
            struct_params.push(quote! { #ident });
            struct_with_bounds_params.push(quote! { #ident: #bounds });
            trait_params.push(quote! { #ident = #ident });
        }
        _ => {}
    });

    StructTraitParams {
        struct_params: quote! { <#struct_params> },
        struct_with_bounds_params: quote! { <#struct_with_bounds_params> },
        // only this can be empty as others struct params include 'dynosaur
        trait_params: if trait_params.is_empty() {
            quote! {}
        } else {
            quote! { <#trait_params> }
        },
    }
}
