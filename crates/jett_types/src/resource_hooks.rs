use crate::{Type, TypeId, TypeInterner};

/// Closed backend-neutral role of a checked private resource declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceHookKind {
    Construct,
    BorrowOperation,
    Close,
}

/// Compiler catalog signature recipes, not source names or public provider APIs.
///
/// No production catalog is installed yet. These three recipes establish the
/// checked identity substrate using deterministic compiler test declarations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceKernelRecipe {
    NetworkFactory,
    NetworkBorrow,
    Finalize,
}

impl ResourceKernelRecipe {
    pub const fn kind(self) -> ResourceHookKind {
        match self {
            Self::NetworkFactory => ResourceHookKind::Construct,
            Self::NetworkBorrow => ResourceHookKind::BorrowOperation,
            Self::Finalize => ResourceHookKind::Close,
        }
    }

    pub fn parameter_names(self) -> &'static [&'static str] {
        match self {
            Self::NetworkFactory => &["net", "label"],
            Self::NetworkBorrow => &["net", "resource"],
            Self::Finalize => &["resource"],
        }
    }

    /// Derive a signature entirely in the associated checker's interner.
    pub fn intern_signature(self, types: &mut TypeInterner, resource: TypeId) -> Option<TypeId> {
        if resource.index() as usize >= types.len()
            || !matches!(types.resolve(resource), Type::Resource(_))
        {
            return None;
        }
        let (params, view_params, return_type) = match self {
            Self::NetworkFactory => (
                vec![TypeInterner::NETWORK, TypeInterner::INT64],
                vec![true, false],
                types.intern(Type::Result(resource, TypeInterner::STRING)),
            ),
            Self::NetworkBorrow => (
                vec![TypeInterner::NETWORK, resource],
                vec![true, true],
                types.intern(Type::Result(TypeInterner::INT64, TypeInterner::STRING)),
            ),
            Self::Finalize => (vec![resource], vec![false], TypeInterner::NOTHING),
        };
        Some(types.intern(Type::Function {
            params,
            view_params,
            return_type,
        }))
    }

    /// Exact shape/mode check; carrier equality or a matching name is insufficient.
    pub fn matches_signature(
        self,
        types: &TypeInterner,
        resource: TypeId,
        function: TypeId,
    ) -> bool {
        if resource.index() as usize >= types.len()
            || function.index() as usize >= types.len()
            || !matches!(types.resolve(resource), Type::Resource(_))
        {
            return false;
        }
        let Type::Function {
            params,
            view_params,
            return_type,
        } = types.resolve(function)
        else {
            return false;
        };
        if return_type.index() as usize >= types.len() {
            return false;
        }
        match self {
            Self::NetworkFactory => {
                params.as_slice() == [TypeInterner::NETWORK, TypeInterner::INT64]
                    && view_params.as_slice() == [true, false]
                    && matches!(types.resolve(*return_type), Type::Result(value, error)
                        if *value == resource && *error == TypeInterner::STRING)
            }
            Self::NetworkBorrow => {
                params.as_slice() == [TypeInterner::NETWORK, resource]
                    && view_params.as_slice() == [true, true]
                    && matches!(types.resolve(*return_type), Type::Result(value, error)
                        if *value == TypeInterner::INT64 && *error == TypeInterner::STRING)
            }
            Self::Finalize => {
                params.as_slice() == [resource]
                    && view_params.as_slice() == [false]
                    && *return_type == TypeInterner::NOTHING
            }
        }
    }
}
