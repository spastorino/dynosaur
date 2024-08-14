use crate::expand::expand_trait_async_fns_to_dyn;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, parse_quote,
    punctuated::Punctuated,
    Error, FnArg, GenericParam, Ident, ItemTrait, Pat, PatType, Result, Signature, Token,
    TraitItem, TraitItemConst, TraitItemFn, TraitItemType, TypeParam,
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

    let struct_ident = &attrs.ident;
    let expanded_trait_to_dyn = expand_trait_async_fns_to_dyn(&item_trait);
    let erased_trait = mk_erased_trait(&expanded_trait_to_dyn);
    let erased_trait_blanket_impl = mk_erased_trait_blanket_impl(&item_trait.ident, &erased_trait);
    let dyn_struct = mk_dyn_struct(&attrs.ident, &erased_trait);
    let dyn_struct_impl_item = mk_dyn_struct_impl_item(struct_ident, &item_trait);
    let struct_inherent_impl =
        mk_struct_inherent_impl(&attrs.ident, &item_trait.ident, &erased_trait);
    let dynosaur_mod = Ident::new(
        &format!("_dynosaur_macro_{}", struct_ident),
        Span::call_site(),
    );

    quote! {
        #item_trait

        mod #dynosaur_mod {
            use super::*;
            #erased_trait
            #erased_trait_blanket_impl
            #dyn_struct
            #dyn_struct_impl_item
            #struct_inherent_impl
        }

        use #dynosaur_mod::#struct_ident;
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
    let items = erased_trait.items.iter().map(|item| {
        impl_item(
            item,
            |TraitItemConst {
                 ident,
                 generics,
                 ty,
                 ..
             }| {
                quote! {
                    const #ident #generics: #ty = <Self as #trait_ident #trait_generics>::#ident;
                }
            },
            |ident, args| {
                quote! { Box::pin(<Self as #trait_ident #trait_generics>::#ident(#(#args),*)) }
            },
            |TraitItemType {
                ident, generics, .. }| {
                    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
                    quote! {
                        type #ident #impl_generics = <Self as #trait_ident #trait_generics>:: #ident #ty_generics #where_clause;
                    }
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
    item_const_fn: impl Fn(&TraitItemConst) -> TokenStream,
    fn_body: impl Fn(&Ident, Vec<TokenStream>) -> TokenStream,
    item_type_fn: impl Fn(&TraitItemType) -> TokenStream,
) -> TokenStream {
    match item {
        TraitItem::Const(trait_item_const) => item_const_fn(trait_item_const),
        TraitItem::Fn(TraitItemFn { sig, .. }) => {
            let args = invoke_fn_args(sig);
            let fn_body = fn_body(&sig.ident, args);
            quote! {
                #sig {
                    #fn_body
                }
            }
        }
        TraitItem::Type(trait_item_type) => item_type_fn(trait_item_type),
        _ => Error::new_spanned(item, "unsupported item type").into_compile_error(),
    }
}

fn invoke_fn_args(sig: &Signature) -> Vec<TokenStream> {
    sig.inputs
        .iter()
        .map(|arg| match arg {
            FnArg::Receiver(_) => quote! { self },
            FnArg::Typed(PatType { pat, .. }) => match &**pat {
                Pat::Ident(arg) => quote! { #arg },
                _ => Error::new_spanned(pat, "patterns are not supported in arguments")
                    .to_compile_error(),
            },
        })
        .collect()
}

fn mk_dyn_struct(struct_ident: &Ident, erased_trait: &ItemTrait) -> TokenStream {
    let erased_trait_ident = &erased_trait.ident;
    let (struct_params, trait_params) = struct_trait_params(erased_trait);

    quote! {
        pub struct #struct_ident #struct_params {
            ptr: *mut (dyn #erased_trait_ident #trait_params + 'dynosaur_struct),
            owned: bool,
        }
    }
}

fn struct_trait_params(item_trait: &ItemTrait) -> (TokenStream, TokenStream) {
    let mut struct_params: Punctuated<_, Token![,]> = Punctuated::new();
    let mut trait_params: Punctuated<_, Token![,]> = Punctuated::new();

    struct_params.push(quote! { 'dynosaur_struct });
    item_trait.generics.params.iter().for_each(|item| {
        struct_params.push(quote! { #item });
        trait_params.push(quote! { #item });
    });
    item_trait.items.iter().for_each(|item| match item {
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

fn mk_dyn_struct_impl_item(struct_ident: &Ident, item_trait: &ItemTrait) -> TokenStream {
    let item_trait_ident = &item_trait.ident;
    let (_, trait_generics, where_clause) = &item_trait.generics.split_for_impl();

    let items = item_trait.items.iter().map(|item| {
        impl_item(item,
            |TraitItemConst {
                ident,
                generics,
                ty,
                ..
            }| {
                quote! {
                    const #ident #generics: #ty = <Self as #item_trait_ident #trait_generics>::#ident;
                }
            },
            |ident, args| {
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
            },
            |TraitItemType {
                ident, generics, .. }| {
                    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
                    quote! {
                        type #ident #impl_generics = #ident #ty_generics #where_clause;
                    }
                }
        )
    });

    let (struct_params, _) = struct_trait_params(item_trait);

    quote! {
        impl #struct_params #item_trait_ident #trait_generics for #struct_ident #struct_params #where_clause
        {
            #(#items)*
        }
    }
}

fn mk_struct_inherent_impl(
    struct_ident: &Ident,
    trait_ident: &Ident,
    erased_trait: &ItemTrait,
) -> TokenStream {
    let (struct_params, trait_params) = struct_trait_params(erased_trait);
    let erased_trait_ident = &erased_trait.ident;

    let mut where_bounds: Punctuated<_, Token![,]> = Punctuated::new();
    erased_trait
        .generics
        .type_params()
        .map(|param| &param.ident)
        .for_each(|param| {
            where_bounds.push(quote! {
                #param: 'dynosaur_struct
            });
        });
    erased_trait.items.iter().for_each(|item| match item {
        TraitItem::Type(TraitItemType { ident, .. }) => {
            where_bounds.push(quote! {
                #ident: 'dynosaur_struct,
            });
        }
        _ => {}
    });

    quote! {
        impl #struct_params #struct_ident #struct_params
        {
            pub fn new<DYNOSAUR>(value: DYNOSAUR) -> Self
            where
                DYNOSAUR: #trait_ident #trait_params + 'dynosaur_struct,
                #where_bounds
            {
                let value = Box::new(value);
                Self {
                    ptr: Box::into_raw(value
                             as Box<dyn #erased_trait_ident #trait_params + 'dynosaur_struct>)
                        as *mut (dyn #erased_trait_ident #trait_params + 'dynosaur_struct),
                        owned: true,
                }
            }

            pub fn from_ref<DYNOSAUR>(value: &'dynosaur_struct DYNOSAUR) -> ::dynosaur::macro_lib::Ref<Self>
            where
                DYNOSAUR: #trait_ident #trait_params + 'dynosaur_struct,
                #where_bounds
            {
                let this = Self {
                    ptr: value
                        as &'dynosaur_struct (dyn #erased_trait_ident #trait_params + 'dynosaur_struct)
                        as *const (dyn #erased_trait_ident #trait_params + 'dynosaur_struct)
                        as *mut (dyn #erased_trait_ident #trait_params + 'dynosaur_struct),
                        owned: false,
                };
                unsafe { ::dynosaur::macro_lib::Ref::new(this) }
            }

            pub fn from_mut<DYNOSAUR>(value: &'dynosaur_struct mut DYNOSAUR) -> ::dynosaur::macro_lib::RefMut<Self>
            where
                DYNOSAUR: #trait_ident #trait_params + 'dynosaur_struct,
                #where_bounds
            {
                let this = Self {
                    ptr: value
                        as &'dynosaur_struct mut (dyn #erased_trait_ident #trait_params + 'dynosaur_struct)
                        as *mut (dyn #erased_trait_ident #trait_params + 'dynosaur_struct),
                        owned: false,
                };
                unsafe { ::dynosaur::macro_lib::RefMut::new(this) }
            }
        }
    }
}
