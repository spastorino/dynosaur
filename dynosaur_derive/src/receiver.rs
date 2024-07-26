use proc_macro2::{TokenStream, TokenTree};
use syn::visit_mut::{self, VisitMut};
use syn::{ExprPath, Item, Macro, Pat, PatIdent, Receiver, Signature, Token, TypePath};

pub fn has_self_in_sig(sig: &mut Signature) -> bool {
    let mut visitor = HasSelf(false);
    visitor.visit_signature_mut(sig);
    visitor.0
}

pub fn mut_pat(pat: &mut Pat) -> Option<Token![mut]> {
    let mut visitor = HasMutPat(None);
    visitor.visit_pat_mut(pat);
    visitor.0
}

struct HasSelf(bool);

impl VisitMut for HasSelf {
    fn visit_expr_path_mut(&mut self, expr: &mut ExprPath) {
        self.0 |= expr.path.segments[0].ident == "Self";
        visit_mut::visit_expr_path_mut(self, expr);
    }

    fn visit_type_path_mut(&mut self, ty: &mut TypePath) {
        self.0 |= ty.path.segments[0].ident == "Self";
        visit_mut::visit_type_path_mut(self, ty);
    }

    fn visit_receiver_mut(&mut self, _arg: &mut Receiver) {
        self.0 = true;
    }

    fn visit_item_mut(&mut self, _: &mut Item) {
        // Do not recurse into nested items.
    }

    fn visit_macro_mut(&mut self, mac: &mut Macro) {
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

impl VisitMut for HasMutPat {
    fn visit_pat_ident_mut(&mut self, i: &mut PatIdent) {
        if let Some(m) = i.mutability {
            self.0 = Some(m);
        } else {
            visit_mut::visit_pat_ident_mut(self, i);
        }
    }
}
