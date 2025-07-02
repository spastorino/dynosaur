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

pub(crate) fn self_receiver(item_trait: &ItemTrait) -> SelfReceiver {
    let mut visitor = ReceiverVisitor(SelfReceiver {
        shared_ref: false,
        mut_ref: false,
        owned: false,
        box_self: false,
        other: false,
    });
    visitor.visit_item_trait(item_trait);
    visitor.0
}

#[derive(PartialEq, Eq)]
pub(crate) struct SelfReceiver {
    pub(crate) shared_ref: bool,
    pub(crate) mut_ref: bool,
    pub(crate) owned: bool,
    pub(crate) box_self: bool,
    pub(crate) other: bool,
}

impl SelfReceiver {
    pub(crate) fn should_gen_ref(&self) -> bool {
        matches!(
            *self,
            SelfReceiver {
                shared_ref: _,
                mut_ref: false,
                owned: false,
                box_self: false,
                other: false,
            }
        )
    }

    pub(crate) fn should_gen_mut_ref(&self) -> bool {
        matches!(
            *self,
            SelfReceiver {
                shared_ref: _,
                mut_ref: _,
                owned: false,
                box_self: false,
                other: false,
            }
        )
    }

    pub(crate) fn should_gen_box_self(&self) -> bool {
        matches!(
            *self,
            SelfReceiver {
                shared_ref: _,
                mut_ref: _,
                owned: _,
                box_self: _,
                other: false,
            }
        )
    }
}

struct ReceiverVisitor(SelfReceiver);

impl Visit<'_> for ReceiverVisitor {
    fn visit_receiver(&mut self, arg: &Receiver) {
        // TODO: this doesn't handle self: &&Self or self: &mut Box<Self> for example.
        match &*arg.ty {
            Type::Reference(type_reference) => {
                if type_reference.mutability.is_none() {
                    self.0.shared_ref = true;
                } else {
                    self.0.mut_ref = true;
                }
            }

            Type::Path(type_path) => {
                let segments = &type_path.path.segments;

                if segments.len() == 1 {
                    if segments[0].ident == "Box" {
                        self.0.box_self = true;
                    } else if segments[0].ident == "Self" {
                        self.0.owned = true;
                    } else {
                        self.0.other = true;
                    }
                }
            }

            _ => {}
        }
    }
}
