use crate::lifetime::{AddLifetimeToImplTrait, CollectLifetimes};
use crate::receiver::{has_self_in_sig, mut_pat};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use std::mem;
use syn::punctuated::Punctuated;
use syn::visit_mut::VisitMut;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, parse_quote, parse_quote_spanned, FnArg, GenericParam, Generics, Ident,
    ItemTrait, Lifetime, LifetimeParam, Pat, Result, ReturnType, Signature, Token, TraitItem,
    TraitItemFn, Type, WhereClause,
};

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
/// ```rust,ignore
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
    let _attrs = parse_macro_input!(attr as Attrs);
    let item = parse_macro_input!(item as ItemTrait);

    let erased_trait = mk_erased_trait(&item);

    quote! {
        #item

        #erased_trait
    }
    .into()
}

fn mk_erased_trait(item_trait: &ItemTrait) -> TokenStream {
    let erased_trait =
        ItemTrait {
            ident: Ident::new(&format!("Erased{}", item_trait.ident), Span::call_site()),
            items: item_trait
                .items
                .iter()
                .map(|item| {
                    if let TraitItem::Fn(
                        trait_item_fn @ TraitItemFn {
                            sig:
                                Signature {
                                    asyncness: Some(..),
                                    ..
                                },
                            ..
                        },
                    ) = item
                    {
                        let mut sig = trait_item_fn.sig.clone();

                        sig.fn_token.span = sig.asyncness.take().unwrap().span;

                        let (ret_arrow, ret) = match &sig.output {
                            ReturnType::Default => (Token![->](Span::call_site()), quote!(())),
                            ReturnType::Type(arrow, ret) => (*arrow, quote!(#ret)),
                        };

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

                        for param in used_lifetimes(&item_trait.generics, &lifetimes.explicit) {
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

                        if has_self_in_sig(&mut sig) {
                            where_clause_or_default(&mut sig.generics.where_clause)
                                .predicates
                                .push(parse_quote! {
                                    Self: 'dynosaur
                                });
                        }

                        for (i, arg) in sig.inputs.iter_mut().enumerate() {
                            if let FnArg::Typed(arg) = arg {
                                if match *arg.ty {
                                    Type::Reference(_) => false,
                                    _ => true,
                                } {
                                    match &*arg.pat {
                                        Pat::Ident(_) => {}
                                        _ => {
                                            let positional = positional_arg(i, &arg.pat);
                                            let m = mut_pat(&mut arg.pat);
                                            arg.pat = parse_quote!(#m #positional);
                                        }
                                    }
                                }
                                AddLifetimeToImplTrait.visit_type_mut(&mut arg.ty);
                            }
                        }

                        sig.output = parse_quote! {
                            #ret_arrow ::core::pin::Pin<Box<
                            dyn ::core::future::Future<Output = #ret> + 'dynosaur>>
                        };
                        TraitItem::Fn(TraitItemFn {
                            sig,
                            ..trait_item_fn.clone()
                        })
                    } else {
                        item.clone()
                    }
                })
                .collect(),
            ..item_trait.clone()
        };
    quote! { #erased_trait }
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

fn positional_arg(i: usize, pat: &Pat) -> Ident {
    let span = syn::spanned::Spanned::span(pat).resolved_at(Span::mixed_site());
    format_ident!("__arg{}", i, span = span)
}
