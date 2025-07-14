use expand::{expand_blanket_impl_fn, expand_dyn_struct_fn, expand_fn_sig, InvokeArgsMode};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    parenthesized,
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
    target: Target,
    bridge: Option<Bridge>,
}

struct Target {
    trait_name: Ident,
}

enum Bridge {
    None,
    Blanket,
    Dyn,
}

impl Parse for Attrs {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Attrs {
            vis: input.parse()?,
            ident: input.parse()?,
            target: {
                if input.parse::<Token![=]>().is_err() {
                    return Err(
                        input.error("expected `= dyn(box) TraitName`; dynosaur 0.3 requires this")
                    );
                }
                let dyn_token = input.parse::<Token![dyn]>()?;
                let Ok(strategy) = (|| {
                    let strategy;
                    _ = parenthesized!(strategy in input);
                    Ok(strategy)
                })() else {
                    return Err(syn::Error::new(dyn_token.span, "expected `dyn(box)`"));
                };
                strategy.parse::<Token![box]>()?;
                Target {
                    trait_name: input
                        .parse()
                        .map_err(|_| input.error("expected trait name after `dyn(box)`"))?,
                }
            },
            bridge: if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
                let blanket: Ident = input.parse()?;
                if blanket != "bridge" {
                    Err(input.error("unknown option"))?
                }
                let bridge;
                _ = parenthesized!(bridge in input);
                Some(bridge.parse()?)
            } else {
                None
            },
        })
    }
}

impl Parse for Bridge {
    fn parse(input: ParseStream) -> Result<Self> {
        if input.peek(Token![dyn]) {
            input.parse::<Token![dyn]>()?;
            Ok(Bridge::Dyn)
        } else {
            let opt: Ident = input.parse()?;
            if opt == "blanket" {
                Ok(Bridge::Blanket)
            } else if opt == "none" {
                Ok(Bridge::None)
            } else {
                Err(input.error("unknown option"))
            }
        }
    }
}

/// Create a struct that takes the place of `dyn Trait` for a trait, supporting
/// both `async` and `-> impl Trait` methods.
///
/// ```
/// # mod dynosaur { pub use dynosaur_derive::dynosaur; }
/// use dynosaur::dynosaur;
///
/// #[dynosaur(pub DynNext = dyn(box) Next)]
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
/// # use dynosaur::dynosaur;
/// # #[dynosaur(DynNext = dyn(box) Next)]
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
/// ```
/// # struct DynTrait<'a>(&'a i32);
/// # trait Trait {}
/// impl<'a> DynTrait<'a> {
///     fn new_box(from: impl Trait) -> Box<Self> { todo!() }
///     fn new_arc(from: impl Trait) -> std::sync::Arc<Self> { todo!() }
///     fn new_rc(from: impl Trait) -> std::rc::Rc<Self> { todo!() }
///
///     fn from_box(from: Box<impl Trait + 'a>) -> Box<Self> { todo!() }
///     fn from_ref(from: &'a impl Trait) -> &'a Self { todo!() }
///     fn from_mut(from: &'a mut impl Trait) -> &'a mut Self { todo!() }
/// }
/// ```
///
/// Normally a concrete type behind a pointer coerces to `dyn Trait` implicitly.
/// When using the `Dyn` struct created by this macro, such conversions must be
/// done explicitly with the provided constructors.
///
/// ## Bridge impls
///
/// Dynosaur writes the following *bridge implementations* for your trait:
///
/// ```
/// # trait Trait {}
/// // Always:
/// impl<T: Trait> Trait for Box<T> { }
/// // If all method receivers are `&mut self` or `&self`:
/// impl<T: Trait> Trait for &mut T { }
/// // If all method receivers are `&self`:
/// impl<T: Trait> Trait for &T { }
/// ```
///
/// This can be controlled with the `bridge` option.
///
/// ```
/// # mod dynosaur { pub use dynosaur_derive::dynosaur; }
/// # fn main() {}
/// # use dynosaur::dynosaur;
/// #[dynosaur(pub DynNext = dyn(box) Next, bridge(none))]
/// pub trait Next {
///     type Item;
///     async fn next(&self) -> Option<Self::Item>;
/// }
///
/// // Bridge impls are disabled above, so this does not conflict.
/// impl<T: Next + Clone> Next for Box<T> {
///     type Item = T::Item;
///     async fn next(&self) -> Option<Self::Item> {
///         T::next(&self.clone()).await
///     }
/// }
/// ```
///
/// The following options are supported for `bridge`:
///
/// * `bridge(blanket)`: The default impls listed above.
/// * `bridge(dyn)`: The impls listed above, except `T` is replaced with
///   `DynTrait`. This can prevent some cases of overlapping impls.
/// * `bridge(none)`: No impls.
///
/// ## Use with `trait_variant`
///
/// You can use dynosaur with the [trait_variant] macro like this:
///
/// ```rust
/// # pub mod dynosaur { pub use dynosaur_derive::dynosaur; }
/// # fn main() {}
/// # use dynosaur::dynosaur;
/// #[trait_variant::make(SendNext: Send)]
/// #[dynosaur(DynNext = dyn(box) Next, bridge(dyn))]
/// #[dynosaur(DynSendNext = dyn(box) SendNext, bridge(dyn))]
/// trait Next {
///     type Item;
///     async fn next(&mut self) -> Option<Self::Item>;
/// }
/// ```
///
/// The `#[trait_variant::make]` attribute must go first, and `bridge(dyn)` is
/// necessary to prevent compiler errors.
///
/// [trait_variant]: https://docs.rs/trait-variant/latest/trait_variant/
///
/// ## Argument-position `impl Trait` support
///
/// Dynosaur has basic support for dyn-compatible `impl Trait` in argument
/// position. Traits that use dynosaur themselves are not yet supported (see
/// [this issue][#63]).
///
/// In order to use this, the trait used in argument position needs a bridge
/// impl such that `Box<dyn MyTrait>: MyTrait`, and your trait must use bare
/// `impl MyTrait` by value in argument position.
///
/// ```rust
/// # mod dynosaur { pub use dynosaur_derive::dynosaur; }
/// # fn main() {}
/// # use dynosaur::dynosaur;
/// trait Foo {}
///
/// impl Foo for Box<dyn Foo + '_> {}
///
/// #[dynosaur(DynMyTrait = dyn(box) MyTrait)]
/// trait MyTrait {
///     fn foo(&self, _: impl Foo) -> i32;
/// }
/// ```
///
/// [#63]: https://github.com/spastorino/dynosaur/issues/63
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
    if attrs.target.trait_name != item_trait.ident {
        // This attribute is being applied to a different trait than the one
        // named (possibly one created by trait_variant).
        // TODO: Add checking in case a trait is missing or misspelled.
        return quote! { #item_trait }.into();
    }

    let vis = &attrs.vis;
    let struct_ident = &attrs.ident;

    let erased_trait = mk_erased_trait(&item_trait);
    let erased_trait_blanket_impl = mk_erased_trait_blanket_impl(&item_trait);
    let dyn_struct = mk_dyn_struct(&struct_ident, &item_trait);
    let dyn_struct_impl_item = mk_dyn_struct_impl_item(struct_ident, &item_trait);
    let struct_inherent_impl = mk_struct_inherent_impl(struct_ident, &item_trait);
    let box_blanket_impl = match attrs.bridge {
        Some(Bridge::None) => quote!(),
        Some(Bridge::Blanket) | None => {
            mk_box_blanket_impl(&struct_ident, &item_trait, Bridge::Blanket)
        }
        Some(Bridge::Dyn) => mk_box_blanket_impl(&struct_ident, &item_trait, Bridge::Dyn),
    };

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
            expand_dyn_struct_fn(sig, &InvokeArgsMode::MaybeBoxedNonUfcs)
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

            pub const fn from_box(value: Box<impl #trait_ident #trait_params + 'dynosaur_struct>) -> Box<#struct_ident #struct_params> {
                let value: Box<dyn #erased_trait_ident #trait_params + 'dynosaur_struct> = value;
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

fn mk_box_blanket_impl(
    struct_ident: &Ident,
    item_trait: &ItemTrait,
    blanket: Bridge,
) -> TokenStream {
    let item_trait_ident = &item_trait.ident;
    let (_, trait_generics, _) = &item_trait.generics.split_for_impl();

    let items = item_trait.items.iter().map(|item| match item {
        TraitItem::Const(_) => Error::new_spanned(item, "consts make the trait not dyn compatible")
            .into_compile_error(),
        TraitItem::Fn(TraitItemFn { sig, .. }) => {
            let self_ = match blanket {
                Bridge::Blanket => {
                    quote! {
                        <DYNOSAUR as #item_trait_ident #trait_generics>
                    }
                }
                Bridge::Dyn => {
                    let StructTraitParams { struct_params, .. } = struct_trait_params(item_trait);
                    quote! {
                        <#struct_ident #struct_params as #item_trait_ident #trait_generics>
                    }
                }
                Bridge::None => quote!(),
            };
            expand_dyn_struct_fn(sig, &InvokeArgsMode::DirectUfcs(self_))
        }
        TraitItem::Type(TraitItemType {
            ident, generics, ..
        }) => {
            let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

            let prefix = match blanket {
                Bridge::Blanket => {
                    quote! {
                        <DYNOSAUR as #item_trait_ident #trait_generics>::
                    }
                }
                Bridge::Dyn | Bridge::None => quote!(),
            };

            quote! {
                type #ident #impl_generics = #prefix #ident #ty_generics #where_clause;
            }
        }
        _ => Error::new_spanned(item, "unsupported item type").into_compile_error(),
    });

    let (
        blanket_generics,
        item_trait_ident,
        trait_generics,
        blanket,
        blanket_params,
        blanket_impl_generics,
        where_bounds,
    ) = match blanket {
        Bridge::None => return quote!(),
        Bridge::Blanket => {
            let blanket_bound: TypeParam =
                parse_quote!(DYNOSAUR: #item_trait_ident #trait_generics);
            let blanket = blanket_bound.ident.clone();
            let mut blanket_generics = item_trait.generics.clone();
            blanket_generics
                .params
                .push(GenericParam::Type(blanket_bound));
            let (blanket_impl_generics, _, _) = blanket_generics.split_for_impl();

            let mut where_bounds: Punctuated<_, Token![,]> = Punctuated::new();
            where_bounds.push(quote! { DYNOSAUR: ?Sized });

            for supertrait in &item_trait.supertraits {
                where_bounds.push(quote! { Self: #supertrait });
            }

            (
                blanket_generics.clone(),
                item_trait_ident,
                trait_generics,
                blanket,
                quote!(),
                quote! { #blanket_impl_generics },
                quote! { where #where_bounds },
            )
        }
        Bridge::Dyn => {
            let StructTraitParams {
                struct_params,
                struct_with_bounds_params,
                ..
            } = struct_trait_params(item_trait);

            let mut where_bounds: Punctuated<_, Token![,]> = Punctuated::new();

            for supertrait in &item_trait.supertraits {
                where_bounds.push(quote! { Self: #supertrait });
            }

            let where_bounds = if where_bounds.is_empty() {
                quote!()
            } else {
                quote! { where #where_bounds }
            };

            (
                item_trait.generics.clone(),
                item_trait_ident,
                trait_generics,
                struct_ident.clone(),
                struct_params,
                struct_with_bounds_params,
                where_bounds,
            )
        }
    };

    let (_, _, blanket_where_clause) = blanket_generics.split_for_impl();

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

    if self_receiver.should_gen_ref() {
        let items = items.clone();

        result.extend(
            quote! {
                impl #blanket_impl_generics #item_trait_ident #trait_generics for & #blanket #blanket_params #blanket_where_clause #where_bounds {
                    #(#items)*
                }
            }
        );
    }

    if self_receiver.should_gen_mut_ref() {
        let items = items.clone();

        result.extend(
            quote! {
                impl #blanket_impl_generics #item_trait_ident #trait_generics for &mut #blanket #blanket_params #blanket_where_clause #where_bounds {
                    #(#items)*
                }
            }
        );
    }

    if self_receiver.should_gen_box_self() {
        result.extend(
            quote! {
                impl #blanket_impl_generics #item_trait_ident #trait_generics for Box<#blanket #blanket_params #blanket_where_clause> #where_bounds {
                    #(#items)*
                }
            }
        );
    }

    result
}
