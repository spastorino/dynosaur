use expand::{expand_fn_sig, expand_ret_ty, expand_sig_ret_ty_to_rpit, is_async_or_rpit};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, parse_quote,
    punctuated::Punctuated,
    Error, FnArg, GenericParam, Ident, ItemTrait, Pat, PatType, Receiver, Result, Signature, Token,
    TraitItem, TraitItemConst, TraitItemFn, TraitItemType, TypeParam,
};
use where_clauses::has_where_self_sized;

mod expand;
mod lifetime;
mod receiver;
mod where_clauses;

struct Attrs {
    ident: Ident,
    target: Option<Target>,
}

struct Target {
    trait_name: Ident,
}

impl Parse for Attrs {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Attrs {
            ident: input.parse()?,
            target: if input.peek(Token![=]) {
                input.parse::<Token![=]>()?;
                input.parse::<Token![dyn]>()?;
                Some(Target {
                    trait_name: input.parse()?,
                })
            } else {
                None
            },
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
/// ## Use with `trait_variant`
///
/// You can use dynosaur with the trait_variant macro like this. The
/// trait_variant attribute must go first.
///
/// ```rust
/// # pub mod dynosaur { pub use dynosaur_derive::dynosaur; }
/// #[trait_variant::make(SendNext: Send)]
/// #[dynosaur::dynosaur(DynNext = dyn Next)]
/// #[dynosaur::dynosaur(DynSendNext = dyn SendNext)]
/// trait Next {
///     type Item;
///     async fn next(&mut self) -> Option<Self::Item>;
/// }
/// # // This is necessary to prevent weird scoping errors in the doctets:
/// # fn main() {}
/// ```
///
/// The `DynNext = dyn Next` is a more explicit form of the macro invocation
/// that allows you to select a particular trait.
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
    if let Some(target) = attrs.target {
        // This attribute is being applied to a different trait than the one
        // named (possibly one created by trait_variant).
        // TODO: Add checking in case a trait is missing or misspelled.
        if target.trait_name != item_trait.ident {
            return quote! { #item_trait }.into();
        }
    }

    let vis = &item_trait.vis;
    let struct_ident = &attrs.ident;

    let erased_trait = mk_erased_trait(&item_trait);
    let erased_trait_blanket_impl = mk_erased_trait_blanket_impl(&item_trait);
    let dyn_struct = mk_dyn_struct(&struct_ident, &item_trait);
    let dyn_struct_impl_item = mk_dyn_struct_impl_item(struct_ident, &item_trait);
    let struct_inherent_impl = mk_struct_inherent_impl(struct_ident, &item_trait);

    let dynosaur_mod = Ident::new(
        &format!(
            "_dynosaur_macro_{}",
            struct_ident.to_string().to_lowercase(),
        ),
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

        #vis use #dynosaur_mod::#struct_ident;
    }
    .into()
}

fn mk_erased_trait(item_trait: &ItemTrait) -> ItemTrait {
    let items: Vec<_> = dyn_compatible_items(&item_trait.items)
        .cloned()
        .map(|mut trait_item| {
            if let TraitItem::Fn(ref mut trait_item_fn) = trait_item {
                expand_fn_sig(&item_trait.generics, trait_item_fn);
            }

            trait_item
        })
        .collect();

    ItemTrait {
        ident: trait_item_erased_name(&item_trait.ident),
        items,
        ..item_trait.clone()
    }
}

/// Remove Self: Sized fns
fn dyn_compatible_items(item_trait_items: &[TraitItem]) -> impl Iterator<Item = &TraitItem> {
    item_trait_items
        .iter()
        .filter(|trait_item| match trait_item {
            TraitItem::Fn(trait_item_fn) if has_where_self_sized(&trait_item_fn.sig) => false,
            _ => true,
        })
}

fn trait_item_erased_name(trait_ident: &Ident) -> Ident {
    Ident::new(&format!("Erased{}", trait_ident), Span::call_site())
}

fn mk_erased_trait_blanket_impl(item_trait: &ItemTrait) -> TokenStream {
    let trait_ident = &item_trait.ident;
    let (_, trait_generics, _) = &item_trait.generics.split_for_impl();
    let items = dyn_compatible_items(&item_trait.items)
        .cloned()
        .map(|trait_item| {
            match trait_item {
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
                TraitItem::Fn(mut trait_item_fn) => {
                    let is_async_or_rpit = is_async_or_rpit(&trait_item_fn.sig);
                    expand_fn_sig(&item_trait.generics, &mut trait_item_fn);
                    let sig = &trait_item_fn.sig;
                    let (receiver, mut args) = invoke_fn_args(sig);
                    if receiver.is_some() {
                        args.insert(0, quote!(self));
                    }
                    let ident = &sig.ident;
                    if is_async_or_rpit {
                        quote! {
                            #sig {
                                Box::pin(<Self as #trait_ident #trait_generics>::#ident(#(#args),*))
                            }
                        }
                    } else {
                        quote! {
                            #sig {
                                <Self as #trait_ident #trait_generics>::#ident(#(#args),*)
                            }
                        }
                    }
                }
                TraitItem::Type(TraitItemType { ident, generics, .. }) => {
                    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
                    quote! {
                        type #ident #impl_generics = <Self as #trait_ident #trait_generics>:: #ident #ty_generics #where_clause;
                    }
                }
                _ => Error::new_spanned(trait_item, "unsupported item type").into_compile_error(),
            }
        });
    let blanket_bound: TypeParam = parse_quote!(DYNOSAUR: #trait_ident #trait_generics);
    let blanket = &blanket_bound.ident.clone();
    let erased_trait_ident = trait_item_erased_name(&trait_ident);
    let mut blanket_generics = item_trait.generics.clone();
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

fn invoke_fn_args(sig: &Signature) -> (Option<&Receiver>, Vec<TokenStream>) {
    let mut receiver = None;
    let mut args = Vec::new();

    for arg in &sig.inputs {
        match arg {
            FnArg::Receiver(fn_arg_receiver) => {
                receiver = Some(fn_arg_receiver);
            }
            FnArg::Typed(PatType { pat, .. }) => match &**pat {
                Pat::Ident(arg) => {
                    args.push(quote! { #arg });
                }
                _ => {
                    args.push(
                        Error::new_spanned(pat, "patterns are not supported in arguments")
                            .to_compile_error(),
                    );
                }
            },
        }
    }

    (receiver, args)
}

fn mk_dyn_struct(struct_ident: &Ident, item_trait: &ItemTrait) -> TokenStream {
    let erased_trait = mk_erased_trait(&item_trait);
    let erased_trait_ident = &erased_trait.ident;
    let StructTraitParams {
        struct_with_bounds_params,
        trait_params,
        ..
    } = struct_trait_params(&erased_trait);

    quote! {
        #[repr(transparent)]
        pub struct #struct_ident #struct_with_bounds_params {
            ptr: dyn #erased_trait_ident #trait_params + 'dynosaur_struct
        }
    }
}

struct StructTraitParams {
    struct_params: TokenStream,
    struct_with_bounds_params: TokenStream,
    trait_params: TokenStream,
}

fn struct_trait_params(item_trait: &ItemTrait) -> StructTraitParams {
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

    let trait_params = if trait_params.is_empty() {
        quote! {}
    } else {
        quote! { <#trait_params> }
    };

    StructTraitParams {
        struct_params: quote! { <#struct_params> },
        struct_with_bounds_params: quote! { <#struct_with_bounds_params> },
        trait_params,
    }
}

fn mk_dyn_struct_impl_item(struct_ident: &Ident, item_trait: &ItemTrait) -> TokenStream {
    let item_trait_ident = &item_trait.ident;
    let (_, trait_generics, where_clause) = &item_trait.generics.split_for_impl();

    let items = item_trait.items.iter().map(|item| {
        match item {
            TraitItem::Const(TraitItemConst {
                ident,
                generics,
                ty,
                ..
            }) => {
                quote! {
                    const #ident #generics: #ty = <Self as #item_trait_ident #trait_generics>::#ident;
                }
            }
            TraitItem::Fn(TraitItemFn { sig, .. }) => {
                if has_where_self_sized(&sig) {
                    quote! {
                        #sig {
                            unreachable!()
                        }
                    }
                } else {
                    let ident = &sig.ident;
                    let (_, args) = invoke_fn_args(&sig);

                    if is_async_or_rpit(&sig) {
                        let ret = expand_ret_ty(&sig);
                        let mut sig = sig.clone();
                        expand_sig_ret_ty_to_rpit(&mut sig);

                        quote! {
                            #sig {
                                let fut: ::core::pin::Pin<Box<dyn #ret + '_>> = self.ptr.#ident(#(#args),*);
                                let fut: ::core::pin::Pin<Box<dyn #ret + 'static>> = unsafe { ::core::mem::transmute(fut) };
                                fut
                            }
                        }
                    } else {
                        quote! {
                            #sig {
                                self.ptr.#ident(#(#args),*)
                            }
                        }
                    }
                }
            }
            TraitItem::Type(TraitItemType { ident, generics, .. }) => {
                let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
                quote! {
                    type #ident #impl_generics = #ident #ty_generics #where_clause;
                }
            }
            _ => Error::new_spanned(item, "unsupported item type").into_compile_error(),
        }
    });

    let StructTraitParams {
        struct_params,
        struct_with_bounds_params,
        ..
    } = struct_trait_params(item_trait);

    quote! {
        impl #struct_with_bounds_params #item_trait_ident #trait_generics for #struct_ident #struct_params #where_clause
        {
            #(#items)*
        }
    }
}

fn mk_struct_inherent_impl(struct_ident: &Ident, item_trait: &ItemTrait) -> TokenStream {
    let trait_ident = &item_trait.ident;
    let erased_trait = mk_erased_trait(item_trait);
    let StructTraitParams {
        struct_params,
        struct_with_bounds_params,
        trait_params,
    } = struct_trait_params(&erased_trait);
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
        impl #struct_with_bounds_params #struct_ident #struct_params
        {
            pub fn boxed(value: impl #trait_ident #trait_params + 'dynosaur_struct) -> Box<#struct_ident #struct_params> {
                let value = Box::new(value);
                let value: Box<dyn #erased_trait_ident #trait_params + 'dynosaur_struct> = value;
                unsafe { ::core::mem::transmute(value) }
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
