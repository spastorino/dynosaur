use proc_macro2::{TokenStream, TokenTree};
use syn::visit::{self, Visit};
use syn::visit_mut::{self, VisitMut};
use syn::{ExprPath, Item, Macro, Pat, PatIdent, Receiver, Signature, Token, TypePath};

pub fn has_self_in_sig(sig: &Signature) -> bool {
    let mut visitor = HasSelf(false);
    visitor.visit_signature(sig);
    visitor.0
}

pub fn mut_pat(pat: &Pat) -> Option<Token![mut]> {
    let mut visitor = HasMutPat(None);
    visitor.visit_pat(pat);
    visitor.0
}

struct HasSelf(bool);

impl Visit<'_> for HasSelf {
    fn visit_expr_path(&mut self, expr: &ExprPath) {
        self.0 |= expr.path.segments[0].ident == "Self";
        visit::visit_expr_path(self, expr);
    }

    fn visit_type_path(&mut self, ty: &TypePath) {
        self.0 |= ty.path.segments[0].ident == "Self";
        visit::visit_type_path(self, ty);
    }

    fn visit_receiver(&mut self, _arg: &Receiver) {
        self.0 = true;
    }

    fn visit_item(&mut self, _: &Item) {
        // Do not recurse into nested items.
    }

    fn visit_macro(&mut self, mac: &Macro) {
        if !contains_fn(mac.tokens.clone()) {
            self.0 |= has_self_in_token_stream(mac.tokens.clone());
        }
    }
}

fn contains_fn(tokens: TokenStream) -> bool {
    tokens.into_iter().any(|tt| match tt {
        TokenTree::Ident(ident) => ident == "fn",
        TokenTree::Group(group) => contains_fn(group.stream()),
        _ => false,
    })
}

fn has_self_in_token_stream(tokens: TokenStream) -> bool {
    tokens.into_iter().any(|tt| match tt {
        TokenTree::Ident(ident) => ident == "Self",
        TokenTree::Group(group) => has_self_in_token_stream(group.stream()),
        _ => false,
    })
}

struct HasMutPat(Option<Token![mut]>);

impl Visit<'_> for HasMutPat {
    fn visit_pat_ident(&mut self, i: &PatIdent) {
        if let Some(m) = i.mutability {
            self.0 = Some(m);
        } else {
            visit::visit_pat_ident(self, i);
        }
    }
}
