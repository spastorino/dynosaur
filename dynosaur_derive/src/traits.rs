use crate::where_clauses::has_where_self_sized;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::visit::Visit;
use syn::{
    punctuated::Punctuated, Ident, ItemTrait, Receiver, Token, TraitItem, TraitItemType, Type,
};

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

pub(crate) fn is_pin(item_trait: &ItemTrait) -> IsPin {
    let mut visitor = PinVisitor(IsPin::Undefined);
    visitor.visit_item_trait(item_trait);
    visitor.0
}

#[derive(PartialEq, Eq)]
pub(crate) enum IsPin {
    PinOnly,
    PinAndNonPin,
    TypeNotPin,
    Undefined,
}

impl IsPin {
    fn mark_pin(&mut self) {
        if *self == IsPin::Undefined {
            *self = IsPin::PinOnly;
        }

        if *self == IsPin::TypeNotPin {
            *self = IsPin::PinAndNonPin;
        }
    }

    fn mark_as_type_not_pin(&mut self) {
        if *self == IsPin::Undefined {
            *self = IsPin::TypeNotPin;
        }

        if *self == IsPin::PinOnly {
            *self = IsPin::PinAndNonPin;
        }
    }
}

struct PinVisitor(IsPin);

impl Visit<'_> for PinVisitor {
    fn visit_receiver(&mut self, arg: &Receiver) {
        if let Type::Path(type_path) = &*arg.ty {
            let segments = &type_path.path.segments;

            if segments.len() == 3
                && (segments[0].ident == "core" || segments[0].ident == "std")
                && segments[1].ident == "pin"
                && segments[2].ident == "Pin"
                || segments.len() == 2 && segments[0].ident == "pin" && segments[1].ident == "Pin"
                || segments.len() == 1 && segments[0].ident == "Pin"
            {
                self.0.mark_pin();
                return;
            }
        }

        self.0.mark_as_type_not_pin();
    }
}
