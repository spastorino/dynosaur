use crate::lifetime::{AddLifetimeToImplTrait, CollectLifetimes};
use crate::receiver::has_self_in_sig;
use crate::sig::{is_async, is_rpit};
use crate::where_clauses::{has_where_self_sized, where_clause_or_default};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use std::mem;
use syn::punctuated::Punctuated;
use syn::token::RArrow;
use syn::visit_mut::VisitMut;
use syn::{
    parse_quote, parse_quote_spanned, Error, FnArg, GenericParam, Generics, Ident, ItemTrait, Pat,
    PatIdent, PatType, ReturnType, Signature, Token, TraitItemFn, Type, TypeImplTrait,
    TypeParamBound,
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

    expand_arg_names(sig);

    if is_async(sig) {
        expand_fn_input(item_trait_generics, sig);
        expand_sig_ret_ty_to_pin_box(sig);
    } else if is_rpit(sig) {
        expand_fn_input(item_trait_generics, sig);
        expand_sig_ret_ty_to_box(sig);
    }

    // Remove default method if any for the erased trait
    trait_item_fn.default = None;
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

    for lifetime in item_trait_generics
        .params
        .iter()
        .filter_map(move |param| match param {
            GenericParam::Lifetime(param) => Some(&param.lifetime),
            _ => None,
        })
    {
        let span = lifetime.span();
        where_clause_or_default(&mut sig.generics.where_clause)
            .predicates
            .push(parse_quote_spanned!(span=> #lifetime: 'dynosaur));
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

pub(crate) fn expand_arg_names(sig: &mut Signature) {
    let mut wild_id = 1;

    for arg in &mut sig.inputs {
        if let FnArg::Typed(arg) = arg {
            if matches!(*arg.pat, Pat::Wild(_)) {
                arg.pat = Box::new(Pat::Ident(PatIdent {
                    attrs: Vec::new(),
                    by_ref: None,
                    mutability: None,
                    ident: Ident::new(&format!("__dynosaur_arg{}", wild_id), Span::call_site()),
                    subpat: None,
                }));

                wild_id += 1;
            }
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

pub(crate) fn expand_sig_ret_ty_to_box(sig: &mut Signature) {
    let (arrow, ret) = expand_arrow_ret_ty(sig);
    if let Some(asyncness) = sig.asyncness.take() {
        sig.fn_token.span = asyncness.span;
    }
    sig.output = parse_quote! { #arrow Box<dyn #ret + 'dynosaur> };
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

pub(crate) fn expand_blanket_impl_fn(
    item_trait: &ItemTrait,
    trait_item_fn: &mut TraitItemFn,
) -> TokenStream {
    let is_async = is_async(&trait_item_fn.sig);
    let is_rpit = is_rpit(&trait_item_fn.sig);

    expand_fn_sig(&item_trait.generics, trait_item_fn);
    let sig = &trait_item_fn.sig;

    let trait_ident = &item_trait.ident;
    let (_, trait_generics, _) = &item_trait.generics.split_for_impl();
    let ident = &sig.ident;
    let args = expand_invoke_args(sig, false);
    let value = quote! { <Self as #trait_ident #trait_generics>::#ident(#(#args),*) };

    let value = if is_async {
        quote! {
            Box::pin(#value)
        }
    } else if is_rpit {
        quote! {
            Box::new(#value)
        }
    } else {
        value
    };

    quote! {
        #sig {
            #value
        }
    }
}

pub(crate) fn expand_dyn_struct_fn(sig: &Signature) -> TokenStream {
    if has_where_self_sized(&sig) {
        quote! {
            #sig {
                unreachable!()
            }
        }
    } else {
        let ident = &sig.ident;

        let mut sig = sig.clone();
        expand_arg_names(&mut sig);
        let args = expand_invoke_args(&sig, true);

        if is_async(&sig) {
            let ret = expand_ret_ty(&sig);
            expand_sig_ret_ty_to_rpit(&mut sig);

            quote! {
                #sig {
                    let fut: ::core::pin::Pin<Box<dyn #ret + '_>> = self.ptr.#ident(#(#args),*);
                    let fut: ::core::pin::Pin<Box<dyn #ret + 'static>> = unsafe { ::core::mem::transmute(fut) };
                    fut
                }
            }
        } else if is_rpit(&sig) {
            let ret = expand_ret_ty(&sig);

            quote! {
                #sig {
                    let ret: Box<dyn #ret + '_> = self.ptr.#ident(#(#args),*);
                    let ret: Box<dyn #ret + '_> = unsafe { ::core::mem::transmute(ret) };
                    ret
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
                let mut ret_bounds: Punctuated<&TypeParamBound, Token![+]> = Punctuated::new();

                for bound in bounds {
                    if !matches!(bound, TypeParamBound::Lifetime(_)) {
                        ret_bounds.push(bound);
                    }
                }

                return (*arrow, quote! { #ret_bounds });
            }
        }
        _ => {}
    }

    (
        Token![->](Span::call_site()),
        Error::new_spanned(&sig.output, "unsupported return type").to_compile_error(),
    )
}
