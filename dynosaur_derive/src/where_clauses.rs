use syn::punctuated::Punctuated;
use syn::visit::Visit;
use syn::{PredicateType, Signature, Type, TypeParamBound, WhereClause, WherePredicate};

pub(crate) fn where_clause_or_default(clause: &mut Option<WhereClause>) -> &mut WhereClause {
    clause.get_or_insert_with(|| WhereClause {
        where_token: Default::default(),
        predicates: Punctuated::new(),
    })
}

pub(crate) fn has_where_self_sized(sig: &Signature) -> bool {
    let mut visitor = SelfSized(false);
    visitor.visit_signature(sig);
    visitor.0
}

struct SelfSized(bool);

impl Visit<'_> for SelfSized {
    fn visit_where_clause(&mut self, clause: &WhereClause) {
        self.0 |= clause
            .predicates
            .iter()
            .find(|predicate| match predicate {
                WherePredicate::Type(PredicateType {
                    bounded_ty: Type::Path(type_path),
                    bounds,
                    ..
                }) => {
                    type_path.path.get_ident().map(|ident| ident.to_string())
                        == Some(String::from("Self"))
                        && bounds
                            .iter()
                            .find(|bound| match bound {
                                TypeParamBound::Trait(trait_bound)
                                    if trait_bound
                                        .path
                                        .get_ident()
                                        .map(|ident| ident.to_string())
                                        == Some(String::from("Sized")) =>
                                {
                                    true
                                }
                                _ => false,
                            })
                            .is_some()
                }

                _ => false,
            })
            .is_some();
    }
}
