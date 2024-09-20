use expand::{expand_async_ret_ty, expand_trait_async_fns_to_dyn, remove_asyncness_from_fn};
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

/// Create a struct that takes the place of `dyn Trait` for a trait, supporting
/// both `async` and `-> impl Trait` methods.
///
/// ```
/// # mod dynosaur { pub use dynosaur_derive::dynosaur; }
/// #[dynosaur::dynosaur(DynNext)]
/// trait Next {
///     type Item;
///     async fn next(&self) -> Option<Self::Item>;
/// }
/// # // This is necessary to prevent weird scoping errors in the doctets:
/// # fn main() {}
/// ```
///
/// Here, the struct is named `DynNext`. It can be used like this:
///
/// ```
/// # mod dynosaur { pub use dynosaur_derive::dynosaur; }
/// # #[dynosaur::dynosaur(DynNext)]
/// # trait Next {
/// #     type Item;
/// #     async fn next(&self) -> Option<Self::Item>;
/// # }
/// #
/// # fn from_iter<T: IntoIterator>(v: T) -> impl Next<Item = i32> {
/// #     Never
/// # }
/// # struct Never;
/// # impl Next for Never {
/// #     type Item = i32;
/// #     async fn next(&self) -> Option<Self::Item> { None }
/// # }
/// #
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() {
/// #
/// async fn dyn_dispatch(iter: &mut DynNext<'_, i32>) {
///     while let Some(item) = iter.next().await {
///         println!("- {item}");
///     }
/// }
///
/// let a = [1, 2, 3];
/// dyn_dispatch(DynNext::from_mut(&mut from_iter(a))).await;
/// # }
/// ```
///
/// ## Interface
///
/// The `Dyn` struct produced by this macro has the following constructors:
///
/// ```ignore
/// impl<'a> DynTrait<'a> {
///     fn new(from: Box<impl Trait>) -> Box<Self> { ... }
///     fn boxed(from: impl Trait) -> Box<Self> { ... }
///     fn from_ref(from: &'a impl Trait) -> &'a Self { ... }
///     fn from_mut(from: &'a mut impl Trait) -> &'a mut Self { ... }
/// }
/// ```
///
/// Normally a concrete type behind a pointer coerces to `dyn Trait` implicitly.
/// When using the `Dyn` struct created by this macro, such conversions must be
/// done explicitly with the provided constructors.
///
/// ## Performance
///
/// In addition to the normal overhead of dynamic dispatch, calling `async` and
/// `-> impl Trait` methods on a `Dyn` struct will box the returned value so it
/// has a known size.
///
/// There is no performance cost to using this macro when the trait is used with
/// static dispatch.
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
        ident: erased_trait_ident(&item_trait.ident),
        ..item_trait.clone()
    }
}

fn erased_trait_ident(item_trait_ident: &Ident) -> Ident {
    Ident::new(&format!("Erased{}", item_trait_ident), Span::call_site())
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
            |TraitItemFn {
                 sig,
                 ..
             }| {
                 let args = invoke_fn_args(sig);
                 let ident = &sig.ident;
                 quote! {
                     #sig {
                         Box::pin(<Self as #trait_ident #trait_generics>::#ident(#(#args),*))
                     }
                 }
            },
            |TraitItemType {
                ident, generics, .. }| {
                    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
                    quote! {
                        type #ident #impl_generics = <Self as #trait_ident #trait_generics>:: #ident #ty_generics #where_clause;
                    }
                })
    });
    let blanket_bound: TypeParam = parse_quote!(DYNOSAUR: #trait_ident #trait_generics + ?Sized);
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
    item_fn_fn: impl Fn(&TraitItemFn) -> TokenStream,
    item_type_fn: impl Fn(&TraitItemType) -> TokenStream,
) -> TokenStream {
    match item {
        TraitItem::Const(trait_item_const) => item_const_fn(trait_item_const),
        TraitItem::Fn(trait_item_fn) => item_fn_fn(trait_item_fn),
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
        #[repr(transparent)]
        pub struct #struct_ident #struct_params {
            ptr: dyn #erased_trait_ident #trait_params + 'dynosaur_struct
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
    let (struct_params, _) = struct_trait_params(item_trait);

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
            |TraitItemFn {
                 sig,
                 ..
             }| {
                 let erased_trait_ident = erased_trait_ident(item_trait_ident);
                 let ident = &sig.ident;
                 let mut sig = sig.clone();
                 let args = invoke_fn_args(&sig);
                 let (ret_arrow, ret) = expand_async_ret_ty(&sig);
                 sig.output = parse_quote! { #ret_arrow impl #ret  };
                 remove_asyncness_from_fn(&mut sig);

                 quote! {
                     #sig {
                         let fut: ::core::pin::Pin<Box<dyn #ret + '_>> = <#struct_ident #struct_params as #erased_trait_ident #trait_generics>::#ident(#(#args),*);
                         let fut: ::core::pin::Pin<Box<dyn #ret + 'static>> = unsafe { ::core::mem::transmute(fut) };
                         fut
                     }
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
            pub fn new(value: Box<impl #trait_ident #trait_params + 'dynosaur_struct>) -> Box<#struct_ident #struct_params> {
                let value: Box<dyn #erased_trait_ident #trait_params + 'dynosaur_struct> = value;
                unsafe { ::core::mem::transmute(value) }
            }

            pub fn boxed(value: impl #trait_ident #trait_params + 'dynosaur_struct) -> Box<#struct_ident #struct_params> {
                Self::new(Box::new(value))
            }

            pub fn from_ref(value: &(impl #trait_ident #trait_params + 'dynosaur_struct)) -> & #struct_ident #struct_params {
                let value: &(dyn #erased_trait_ident #trait_params + 'dynosaur_struct) = &*value;
                unsafe { ::core::mem::transmute(value) }
            }

            pub fn from_mut(value: &mut (impl #trait_ident #trait_params + 'dynosaur_struct)) -> &mut #struct_ident #struct_params {
                let value: &mut (dyn #erased_trait_ident #trait_params + 'dynosaur_struct) = &mut *value;
                unsafe { ::core::mem::transmute(value) }
            }
        }
    }
}
