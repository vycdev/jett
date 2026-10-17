//! Exact source-level backing for viewed iteration bindings.
//! These are logical views, separate from ABI parameters and backend storage.
use jett_common::Span;
use jett_types::{Type, TypeId, TypeInterner};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IterationPart {
    Element,
    Key,
    Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewIterationBinding {
    loop_span: Span,
    iterable_type: TypeId,
    binder_type: TypeId,
    part: IterationPart,
}

impl ViewIterationBinding {
    pub fn loop_span(self) -> Span {
        self.loop_span
    }
    pub fn iterable_type(self) -> TypeId {
        self.iterable_type
    }
    pub fn binder_type(self) -> TypeId {
        self.binder_type
    }
    pub fn part(self) -> IterationPart {
        self.part
    }
    pub(crate) fn matches_binding(self, ty: TypeId, declaration: Span) -> bool {
        self.binder_type == ty
            && declaration.file == self.loop_span.file
            && declaration.start >= self.loop_span.start
            && declaration.end <= self.loop_span.end
    }
}

/// Prove one endpoint of the actual checked loop. The caller separately proves
/// its exact binder LocalId, original For/ForEach association and body scope.
pub fn checked_view_iteration_binding(
    types: &TypeInterner,
    loop_span: Span,
    iterable_type: TypeId,
    binder_type: TypeId,
    part: IterationPart,
) -> Result<ViewIterationBinding, String> {
    if iterable_type == TypeInterner::ERROR
        || binder_type == TypeInterner::ERROR
        || iterable_type.index() as usize >= types.len()
        || binder_type.index() as usize >= types.len()
    {
        return Err("viewed iteration references an invalid endpoint type".into());
    }
    let endpoint = match (types.resolve(iterable_type), part) {
        (Type::List(element) | Type::Set(element), IterationPart::Element) => *element,
        (Type::String, IterationPart::Element) => TypeInterner::STRING,
        (Type::Map(key, _), IterationPart::Key) => *key,
        (Type::Map(_, value), IterationPart::Value) => *value,
        _ => return Err("viewed iteration binding role differs from its exact iterable".into()),
    };
    if endpoint != binder_type {
        return Err("viewed iteration binding type differs from its exact endpoint".into());
    }
    Ok(ViewIterationBinding {
        loop_span,
        iterable_type,
        binder_type,
        part,
    })
}
