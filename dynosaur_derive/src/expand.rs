use crate::lifetime::{AddLifetimeToImplTrait, CollectLifetimes};
use crate::receiver::has_self_in_sig;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use std::mem;
use syn::punctuated::Punctuated;
use syn::token::RArrow;
use syn::visit_mut::VisitMut;
use syn::{
    parse_quote, parse_quote_spanned, Error, FnArg, GenericParam, Generics, ItemTrait, Lifetime,
    LifetimeParam, ReturnType, Signature, Token, TraitItem, TraitItemFn, Type, TypeImplTrait,
    WhereClause,
};

/// Expands the signature of each function on the trait, converting async fn into fn with return
/// type Future and makes lifetimes explicit.
///
/// Converts:
///
/// ```rust
/// trait MyTrait {
///     async fn foo(&self) -> i32;
/// }
/// ```
///
/// into:
///
/// ```rust
/// trait ErasedMyTrait {
///     fn foo<'life0, 'dynosaur>(&'life0 self)
///     ->
///         ::core::pin::Pin<Box<dyn ::core::future::Future<Output = i32> +
///         'dynosaur>>
///     where
///     'life0: 'dynosaur,
///     Self: 'dynosaur;
/// }
/// ```
pub fn expand_trait_async_fns_to_dyn(item_trait: &ItemTrait) -> ItemTrait {
    let mut item_trait = item_trait.clone();

    for trait_item_fn in impl_trait_fns_iter(&mut item_trait.items) {
        expand_async_fn_input(&item_trait.generics, trait_item_fn);
        expand_async_fn_output(trait_item_fn);
        remove_asyncness_from_fn(&mut trait_item_fn.sig);
    }

    item_trait
}

fn impl_trait_fns_iter(
    item_trait_items: &mut Vec<TraitItem>,
) -> impl Iterator<Item = &mut TraitItemFn> {
    item_trait_items.iter_mut().filter_map(|item| match item {
        TraitItem::Fn(
            trait_item_fn @ TraitItemFn {
                sig: Signature {
                    asyncness: Some(_), ..
                },
                ..
            },
        ) => Some(trait_item_fn),
        TraitItem::Fn(TraitItemFn {
            sig:
                Signature {
                    asyncness: None,
                    output: ReturnType::Type(_, ret),
                    ..
                },
            ..
        }) => {
            if matches!(**ret, Type::ImplTrait(_)) {
                if let TraitItem::Fn(trait_item_fn) = item {
                    return Some(trait_item_fn);
                }
            }
            None
        }
        _ => None,
    })
}

pub fn remove_asyncness_from_fn(sig: &mut Signature) {
    if let Some(asyncness) = sig.asyncness.take() {
        sig.fn_token.span = asyncness.span;
    }
}

fn expand_async_fn_input(item_trait_generics: &Generics, trait_item_fn: &mut TraitItemFn) {
    let sig = &mut trait_item_fn.sig;

    let mut lifetimes = CollectLifetimes::new();
    for arg in &mut sig.inputs {
        match arg {
            FnArg::Receiver(arg) => lifetimes.visit_receiver_mut(arg),
            FnArg::Typed(arg) => lifetimes.visit_type_mut(&mut arg.ty),
        }
    }

    for param in &mut sig.generics.params {
        match param {
            GenericParam::Type(param) => {
                let param_name = &param.ident;
                let span = match param.colon_token.take() {
                    Some(colon_token) => colon_token.span,
                    None => param_name.span(),
                };
                let bounds = mem::replace(&mut param.bounds, Punctuated::new());
                where_clause_or_default(&mut sig.generics.where_clause)
                    .predicates
                    .push(parse_quote_spanned!(span=> #param_name: 'dynosaur + #bounds));
            }
            GenericParam::Lifetime(param) => {
                let param_name = &param.lifetime;
                let span = match param.colon_token.take() {
                    Some(colon_token) => colon_token.span,
                    None => param_name.span(),
                };
                let bounds = mem::replace(&mut param.bounds, Punctuated::new());
                where_clause_or_default(&mut sig.generics.where_clause)
                    .predicates
                    .push(parse_quote_spanned!(span=> #param: 'dynosaur + #bounds));
            }
            GenericParam::Const(_) => {}
        }
    }

    for param in used_lifetimes(&item_trait_generics, &lifetimes.explicit) {
        let param = &param.lifetime;
        let span = param.span();
        where_clause_or_default(&mut sig.generics.where_clause)
            .predicates
            .push(parse_quote_spanned!(span=> #param: 'dynosaur));
    }

    if sig.generics.lt_token.is_none() {
        sig.generics.lt_token = Some(Token![<](sig.ident.span()));
    }
    if sig.generics.gt_token.is_none() {
        sig.generics.gt_token = Some(Token![>](sig.paren_token.span.join()));
    }

    for elided in lifetimes.elided {
        sig.generics.params.push(parse_quote!(#elided));
        where_clause_or_default(&mut sig.generics.where_clause)
            .predicates
            .push(parse_quote_spanned!(elided.span()=> #elided: 'dynosaur));
    }

    sig.generics.params.push(parse_quote!('dynosaur));

    if has_self_in_sig(sig) {
        where_clause_or_default(&mut sig.generics.where_clause)
            .predicates
            .push(parse_quote! {
                Self: 'dynosaur
            });
    }

    for arg in &mut sig.inputs {
        if let FnArg::Typed(arg) = arg {
            AddLifetimeToImplTrait.visit_type_mut(&mut arg.ty);
        }
    }
}

fn expand_async_fn_output(trait_item_fn: &mut TraitItemFn) {
    let sig = &mut trait_item_fn.sig;

    let (ret_arrow, ret) = expand_async_ret_ty(sig);
    trait_item_fn.sig.output =
        parse_quote! { #ret_arrow ::core::pin::Pin<Box<dyn #ret + 'dynosaur>> };
}

pub fn expand_async_ret_ty(sig: &Signature) -> (RArrow, TokenStream) {
    let is_async = sig.asyncness.is_some();

    if is_async {
        let (ret_arrow, ret) = match &sig.output {
            ReturnType::Default => (Token![->](Span::call_site()), quote!(())),
            ReturnType::Type(arrow, ret) => (*arrow, quote!(#ret)),
        };

        (ret_arrow, quote! { ::core::future::Future<Output = #ret> })
    } else {
        if let ReturnType::Type(ret_arrow, ret) = &sig.output {
            match &**ret {
                Type::ImplTrait(TypeImplTrait { bounds, .. }) => (*ret_arrow, quote!(#bounds)),
                _ => (
                    Token![->](Span::call_site()),
                    Error::new_spanned(&sig.output, "wrong return type").to_compile_error(),
                ),
            }
        } else {
            (
                Token![->](Span::call_site()),
                Error::new_spanned(&sig.output, "wrong return type").to_compile_error(),
            )
        }
    }
}

fn used_lifetimes<'a>(
    generics: &'a Generics,
    used: &'a [Lifetime],
) -> impl Iterator<Item = &'a LifetimeParam> {
    generics.params.iter().filter_map(move |param| {
        if let GenericParam::Lifetime(param) = param {
            if used.contains(&param.lifetime) {
                return Some(param);
            }
        }
        None
    })
}

fn where_clause_or_default(clause: &mut Option<WhereClause>) -> &mut WhereClause {
    clause.get_or_insert_with(|| WhereClause {
        where_token: Default::default(),
        predicates: Punctuated::new(),
    })
}
