use crate::expand::expand_trait_async_fns_to_dyn;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, parse_quote,
    punctuated::Punctuated,
    Error, FnArg, GenericParam, Ident, ItemTrait, Pat, PatType, Result, Token, TraitItem,
    TraitItemConst, TraitItemFn, TraitItemType, TypeParam,
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
/// ```rust
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
    let item_trait = parse_macro_input!(item as ItemTrait);

    let expanded_trait_to_dyn = expand_trait_async_fns_to_dyn(&item_trait);
    let erased_trait = mk_erased_trait(&expanded_trait_to_dyn);
    let erased_trait_blanket_impl = mk_erased_trait_blanket_impl(&item_trait.ident, &erased_trait);
    let dyn_struct = mk_dyn_struct(&attrs.ident, &erased_trait);
    let dyn_struct_impl_item = mk_dyn_struct_impl_item(&attrs.ident, &item_trait);

    quote! {
        #item_trait

        #erased_trait
        #erased_trait_blanket_impl
        #dyn_struct
        #dyn_struct_impl_item
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
    let ref_ = quote! { <Self as #trait_ident #trait_generics>:: };
    let items = erased_trait.items.iter().map(|item| {
        impl_item(item, &ref_, &ref_, |ident, args| {
            quote! { Box::pin(<Self as #trait_ident #trait_generics>::#ident(#(#args),*)) }
        })
    });
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

fn impl_item(
    item: &TraitItem,
    type_ref: &TokenStream,
    const_ref: &TokenStream,
    fn_body: impl Fn(&Ident, Vec<TokenStream>) -> TokenStream,
) -> TokenStream {
    match item {
        TraitItem::Const(TraitItemConst {
            ident,
            generics,
            ty,
            ..
        }) => {
            quote! {
                const #ident #generics: #ty = #const_ref #ident;
            }
        }
        TraitItem::Fn(TraitItemFn { sig, .. }) => {
            let ident = &sig.ident;
            let args: Vec<_> = sig
                .inputs
                .iter()
                .map(|arg| match arg {
                    FnArg::Receiver(_) => quote! { self },
                    FnArg::Typed(PatType { pat, .. }) => match &**pat {
                        Pat::Ident(arg) => quote! { #arg },
                        _ => Error::new_spanned(pat, "patterns are not supported in arguments")
                            .to_compile_error(),
                    },
                })
                .collect();
            let fn_body = fn_body(ident, args);
            quote! {
                #sig {
                    #fn_body
                }
            }
        }
        TraitItem::Type(TraitItemType {
            ident, generics, ..
        }) => {
            let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
            quote! {
                type #ident #impl_generics = #type_ref #ident #ty_generics #where_clause;
            }
        }
        _ => Error::new_spanned(item, "unsupported item type").into_compile_error(),
    }
}

fn mk_dyn_struct(struct_ident: &Ident, erased_trait: &ItemTrait) -> TokenStream {
    let erased_trait_ident = &erased_trait.ident;
    let (struct_params, trait_params) = struct_trait_params(erased_trait);

    quote! {
        struct #struct_ident #struct_params {
            ptr: *mut (dyn #erased_trait_ident #trait_params + 'dynosaur_struct),
            owned: bool,
        }
    }
}

fn struct_trait_params(erased_trait: &ItemTrait) -> (TokenStream, TokenStream) {
    let mut struct_params: Punctuated<_, Token![,]> = Punctuated::new();
    let mut trait_params: Punctuated<_, Token![,]> = Punctuated::new();

    struct_params.push(quote! { 'dynosaur_struct });
    erased_trait.generics.params.iter().for_each(|item| {
        struct_params.push(quote! { #item });
        trait_params.push(quote! { #item });
    });
    erased_trait.items.iter().for_each(|item| match item {
        TraitItem::Type(TraitItemType { ident, .. }) => {
            struct_params.push(quote! { #ident });
            trait_params.push(quote! { #ident = #ident });
        }
        _ => {}
    });

    let trait_params = if trait_params.is_empty() {
        quote! {}
    } else {
        quote! { <#trait_params> }
    };

    (quote! { <#struct_params> }, trait_params)
}

fn mk_dyn_struct_impl_item(struct_ident: &Ident, erased_trait: &ItemTrait) -> TokenStream {
    let erased_trait_ident = &erased_trait.ident;
    let (_, trait_generics, where_clause) = &erased_trait.generics.split_for_impl();

    let type_ref = quote! {};
    let const_ref = quote! { <Self as #erased_trait_ident #trait_generics>:: };
    let items = erased_trait.items.iter().map(|item| {
        impl_item(item, &type_ref, &const_ref, |ident, args| {
            let args = match &args[..] {
                [arg, rest @ ..] => {
                    if "self" == arg.to_string() {
                        rest
                    } else {
                        &args
                    }
                }
                _ => &args,
            };

            quote! {
                unsafe { &*self.ptr }.#ident(#(#args),*).await
            }
        })
    });

    let (struct_params, _) = struct_trait_params(erased_trait);

    quote! {
        impl #struct_params #erased_trait_ident #trait_generics for #struct_ident #struct_params #where_clause
        {
            #(#items)*
        }
    }
}
