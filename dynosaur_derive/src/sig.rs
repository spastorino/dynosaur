use syn::{ReturnType, Signature, Type, TypeParamBound};

pub(crate) fn is_async(sig: &Signature) -> bool {
    match sig {
        Signature {
            asyncness: Some(_), ..
        } => true,
        Signature {
            asyncness: None,
            output: ReturnType::Type(_, ret),
            ..
        } => {
            if let Type::ImplTrait(type_impl_trait) = &**ret {
                type_impl_trait
                    .bounds
                    .iter()
                    .find(|bound| match bound {
                        TypeParamBound::Trait(trait_bound) => {
                            let segments = &trait_bound.path.segments;

                            segments.len() == 3
                                && (segments[0].ident == "core" || segments[0].ident == "std")
                                && segments[1].ident == "future"
                                && segments[2].ident == "Future"
                        }
                        _ => false,
                    })
                    .is_some()
            } else {
                false
            }
        }
        _ => false,
    }
}

pub(crate) fn is_rpit(sig: &Signature) -> bool {
    match sig {
        Signature {
            asyncness: Some(_), ..
        } => false,
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
