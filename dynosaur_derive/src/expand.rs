use crate::lifetime::{used_lifetimes, AddLifetimeToImplTrait, CollectLifetimes};
use crate::receiver::has_self_in_sig;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use std::mem;
use syn::punctuated::Punctuated;
use syn::token::RArrow;
use syn::visit_mut::VisitMut;
use syn::{
    parse_quote, parse_quote_spanned, Error, FnArg, GenericParam, Generics, Pat, PatType,
    ReturnType, Signature, Token, TraitItemFn, Type, TypeImplTrait, WhereClause,
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
pub(crate) fn expand_fn_sig(item_trait_generics: &Generics, trait_item_fn: &mut TraitItemFn) {
    let sig = &mut trait_item_fn.sig;

    if is_async_or_rpit(sig) {
        expand_fn_input(item_trait_generics, sig);
        expand_sig_ret_ty_to_pin_box(sig);
    }

    // Remove default method if any for the erased trait
    trait_item_fn.default = None;
}

pub(crate) fn is_async_or_rpit(sig: &Signature) -> bool {
    match sig {
        Signature {
            asyncness: Some(_), ..
        } => true,
        Signature {
            asyncness: None,
            output: ReturnType::Type(_, ret),
            ..
        } => {
            matches!(**ret, Type::ImplTrait(_))
        }
        _ => false,
    }
}

fn expand_fn_input(item_trait_generics: &Generics, sig: &mut Signature) {
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

pub(crate) fn expand_sig_ret_ty_to_pin_box(sig: &mut Signature) {
    let (arrow, ret) = expand_arrow_ret_ty(sig);
    if let Some(asyncness) = sig.asyncness.take() {
        sig.fn_token.span = asyncness.span;
    }
    sig.output = parse_quote! { #arrow ::core::pin::Pin<Box<dyn #ret + 'dynosaur>> };
}

pub(crate) fn expand_sig_ret_ty_to_rpit(sig: &mut Signature) {
    let (arrow, ret) = expand_arrow_ret_ty(sig);
    if let Some(asyncness) = sig.asyncness.take() {
        sig.fn_token.span = asyncness.span;
    }
    sig.output = parse_quote! { #arrow impl #ret };
}

pub(crate) fn expand_ret_ty(sig: &Signature) -> TokenStream {
    expand_arrow_ret_ty(sig).1
}

pub(crate) fn expand_invoke_args(sig: &Signature, ufc: bool) -> Vec<TokenStream> {
    let mut args = Vec::new();

    for arg in &sig.inputs {
        match arg {
            FnArg::Receiver(_) => {
                if !ufc {
                    // Do not need & or &mut as this is at calling site
                    args.push(quote! { self });
                }
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

    args
}

fn expand_arrow_ret_ty(sig: &Signature) -> (RArrow, TokenStream) {
    match (sig.asyncness.is_some(), &sig.output) {
        (true, ReturnType::Default) => {
            return (
                Token![->](Span::call_site()),
                quote! { ::core::future::Future<Output = ()> },
            );
        }
        (true, ReturnType::Type(arrow, ret)) => {
            return (*arrow, quote! { ::core::future::Future<Output = #ret> });
        }
        (false, ReturnType::Type(arrow, ret)) => {
            if let Type::ImplTrait(TypeImplTrait { bounds, .. }) = &**ret {
                return (*arrow, quote!(#bounds));
            }
        }
        _ => {}
    }

    (
        Token![->](Span::call_site()),
        Error::new_spanned(&sig.output, "unsupported return type").to_compile_error(),
    )
}

fn where_clause_or_default(clause: &mut Option<WhereClause>) -> &mut WhereClause {
    clause.get_or_insert_with(|| WhereClause {
        where_token: Default::default(),
        predicates: Punctuated::new(),
    })
}
