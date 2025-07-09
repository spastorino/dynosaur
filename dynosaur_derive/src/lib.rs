use expand::{expand_blanket_impl_fn, expand_dyn_struct_fn, expand_fn_sig, InvokeArgsMode};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, parse_quote,
    punctuated::Punctuated,
    Error, GenericParam, Ident, ItemTrait, Result, Token, TraitItem, TraitItemFn, TraitItemType,
    TypeParam, Visibility,
};
use traits::{
    dyn_compatible_items, self_receiver, struct_trait_params, trait_item_erased_name,
    StructTraitParams,
};
use where_clauses::has_where_self_sized;

mod expand;
mod lifetime;
mod receiver;
mod sig;
mod traits;
mod where_clauses;

struct Attrs {
    vis: Visibility,
    ident: Ident,
    target: Option<Target>,
}

struct Target {
    trait_name: Ident,
}

impl Parse for Attrs {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Attrs {
            vis: input.parse()?,
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
/// #[dynosaur::dynosaur(pub DynNext)]
/// pub trait Next {
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
///     fn new_box(from: impl Trait) -> Box<Self> { ... }
///     fn new_arc(from: impl Trait) -> std::sync::Arc<Self> { ... }
///     fn new_rc(from: impl Trait) -> std::rc::Rc<Self> { ... }
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
/// // #[dynosaur::dynosaur(DynNext = dyn Next)]
/// // #[dynosaur::dynosaur(DynSendNext = dyn SendNext)]
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

    let vis = &attrs.vis;
    let struct_ident = &attrs.ident;

    let erased_trait = mk_erased_trait(&item_trait);
    let erased_trait_blanket_impl = mk_erased_trait_blanket_impl(&item_trait);
    let dyn_struct = mk_dyn_struct(&struct_ident, &item_trait);
    let dyn_struct_impl_item = mk_dyn_struct_impl_item(struct_ident, &item_trait);
    let struct_inherent_impl = mk_struct_inherent_impl(struct_ident, &item_trait);
    let box_blanket_impl = mk_box_blanket_impl(&item_trait);

    let dynosaur_mod = Ident::new(
        &format!(
            "__dynosaur_macro_{}",
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
            #box_blanket_impl
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
                // Remove default method if any for the erased trait
                trait_item_fn.default = None;

                expand_fn_sig(&item_trait.generics, &mut trait_item_fn.sig);
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

fn mk_erased_trait_blanket_impl(item_trait: &ItemTrait) -> TokenStream {
    // Check for where_self_sized and error, we need to remove this and properly handle where
    // Self: Sized
    for trait_item in &item_trait.items {
        match trait_item {
            TraitItem::Fn(trait_item_fn) if has_where_self_sized(&trait_item_fn.sig) => {
                return Error::new_spanned(trait_item_fn, "where Self: Sized is unsupported")
                    .into_compile_error()
            }
            _ => {}
        }
    }

    let trait_ident = &item_trait.ident;
    let erased_trait_ident = trait_item_erased_name(&trait_ident);
    let (_, trait_generics, _) = &item_trait.generics.split_for_impl();

    let items = dyn_compatible_items(&item_trait.items)
        .cloned()
        .map(|trait_item| {
            match trait_item {
                TraitItem::Const(_) => Error::new_spanned(trait_item, "consts make the trait not dyn compatible").into_compile_error(),
                TraitItem::Fn(mut trait_item_fn) => {
                    expand_blanket_impl_fn(item_trait, &mut trait_item_fn.sig)
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
    let blanket = blanket_bound.ident.clone();
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

fn mk_dyn_struct_impl_item(struct_ident: &Ident, item_trait: &ItemTrait) -> TokenStream {
    let item_trait_ident = &item_trait.ident;
    let (_, trait_generics, where_clause) = &item_trait.generics.split_for_impl();

    let items = item_trait.items.iter().map(|item| match item {
        TraitItem::Const(_) => Error::new_spanned(item, "consts make the trait not dyn compatible")
            .into_compile_error(),
        TraitItem::Fn(TraitItemFn { sig, .. }) => {
            expand_dyn_struct_fn(sig, InvokeArgsMode::DynamicUfc)
        }
        TraitItem::Type(TraitItemType {
            ident, generics, ..
        }) => {
            let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
            quote! {
                type #ident #impl_generics = #ident #ty_generics #where_clause;
            }
        }
        _ => Error::new_spanned(item, "unsupported item type").into_compile_error(),
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
            pub fn new_box(value: impl #trait_ident #trait_params + 'dynosaur_struct) -> Box<#struct_ident #struct_params> {
                let value = Box::new(value);
                let value: Box<dyn #erased_trait_ident #trait_params + 'dynosaur_struct> = value;
                unsafe { ::core::mem::transmute(value) }
            }

            pub fn new_arc(value: impl #trait_ident #trait_params + 'dynosaur_struct) -> std::sync::Arc<#struct_ident #struct_params> {
                let value = std::sync::Arc::new(value);
                let value: std::sync::Arc<dyn #erased_trait_ident #trait_params + 'dynosaur_struct> = value;
                unsafe { ::core::mem::transmute(value) }
            }

            pub fn new_rc(value: impl #trait_ident #trait_params + 'dynosaur_struct) -> std::rc::Rc<#struct_ident #struct_params> {
                let value = std::rc::Rc::new(value);
                let value: std::rc::Rc<dyn #erased_trait_ident #trait_params + 'dynosaur_struct> = value;
                unsafe { ::core::mem::transmute(value) }
            }

            pub const fn from_ref(value: &(impl #trait_ident #trait_params + 'dynosaur_struct)) -> & #struct_ident #struct_params {
                let value: &(dyn #erased_trait_ident #trait_params + 'dynosaur_struct) = &*value;
                unsafe { ::core::mem::transmute(value) }
            }

            pub const fn from_mut(value: &mut (impl #trait_ident #trait_params + 'dynosaur_struct)) -> &mut #struct_ident #struct_params {
                let value: &mut (dyn #erased_trait_ident #trait_params + 'dynosaur_struct) = &mut *value;
                unsafe { ::core::mem::transmute(value) }
            }
        }
    }
}

fn mk_box_blanket_impl(item_trait: &ItemTrait) -> TokenStream {
    let item_trait_ident = &item_trait.ident;
    let (_, trait_generics, _) = &item_trait.generics.split_for_impl();

    let items = item_trait.items.iter().map(|item| match item {
        TraitItem::Const(_) => Error::new_spanned(item, "consts make the trait not dyn compatible")
            .into_compile_error(),
        TraitItem::Fn(TraitItemFn { sig, .. }) => {
            expand_dyn_struct_fn(sig, InvokeArgsMode::OnlyExpandSelf)
        }
        TraitItem::Type(TraitItemType {
            ident, generics, ..
        }) => {
            let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
            quote! {
                type #ident #impl_generics = DYNOSAUR::#ident #ty_generics #where_clause;
            }
        }
        _ => Error::new_spanned(item, "unsupported item type").into_compile_error(),
    });

    let blanket_bound: TypeParam = parse_quote!(DYNOSAUR: #item_trait_ident #trait_generics);
    let blanket = blanket_bound.ident.clone();
    let mut blanket_generics = item_trait.generics.clone();
    blanket_generics
        .params
        .push(GenericParam::Type(blanket_bound));
    let (blanket_impl_generics, _, blanket_where_clause) = &blanket_generics.split_for_impl();

    let self_receiver = self_receiver(item_trait);

    let mut result = TokenStream::new();

    if let Some(arg) = &self_receiver.other {
        result.extend(Error::new_spanned(arg, "unsupported self type").into_compile_error());
    }

    if let Some(arg) = &self_receiver.owned {
        result
            .extend(Error::new_spanned(arg, "By value Self is not supported").into_compile_error());
    }

    if let Some(arg) = &self_receiver.box_self {
        result.extend(Error::new_spanned(arg, "Box<Self> is not supported").into_compile_error());
    }

    let mut where_bounds: Punctuated<_, Token![,]> = Punctuated::new();
    where_bounds.push(quote! { DYNOSAUR: ?Sized });

    for supertrait in &item_trait.supertraits {
        where_bounds.push(quote! { Self: #supertrait });
    }

    if self_receiver.should_gen_ref() {
        let items = items.clone();

        result.extend(
            quote! {
                impl #blanket_impl_generics #item_trait_ident #trait_generics for & #blanket #blanket_where_clause where #where_bounds {
                    #(#items)*
                }
            }
        );
    }

    if self_receiver.should_gen_mut_ref() {
        let items = items.clone();

        result.extend(
            quote! {
                impl #blanket_impl_generics #item_trait_ident #trait_generics for &mut #blanket #blanket_where_clause where #where_bounds {
                    #(#items)*
                }
            }
        );
    }

    if self_receiver.should_gen_box_self() {
        result.extend(
            quote! {
                impl #blanket_impl_generics #item_trait_ident #trait_generics for Box<#blanket #blanket_where_clause> where #where_bounds {
                    #(#items)*
                }
            }
        );
    }

    result
}
