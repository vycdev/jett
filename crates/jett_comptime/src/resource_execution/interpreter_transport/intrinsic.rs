//! Exact checked intrinsic metadata, without a mutable ambient reflection map.
use super::*;
use crate::resource_execution::PreparedIntrinsicArguments;

impl Interpreter {
    pub(super) fn call_resource_intrinsic(
        &mut self,
        invocation: &CheckedInvocation,
        prepared: &PreparedIntrinsicArguments,
        args: Vec<Value>,
    ) -> Result<Value, String> {
        let checked = &self
            .resource_transport
            .as_ref()
            .ok_or("missing checked Resource transport")?
            .checked;
        prepared
            .validate(checked, invocation)
            .map_err(|error| error.to_string())?;
        let intrinsic = prepared.intrinsic();
        let name = intrinsic.canonical_name();
        let info = prepared.reflections().first();
        match intrinsic {
            IntrinsicId::TypeName
            | IntrinsicId::TypeKind
            | IntrinsicId::TypeKindTag
            | IntrinsicId::TypePrimitiveTag
            | IntrinsicId::TypeHasSecret
            | IntrinsicId::TypeInfo => {
                if let Some(result) = check_args(name, 0, &args) {
                    return result;
                }
                let info = info.ok_or("checked intrinsic has no exact reflection argument")?;
                Ok(match intrinsic {
                    IntrinsicId::TypeName => Value::String(info.type_name.clone()),
                    IntrinsicId::TypeKind => Value::String(info.kind.clone()),
                    IntrinsicId::TypeKindTag => Self::type_kind_tag_value(&info.kind),
                    IntrinsicId::TypePrimitiveTag => {
                        Self::primitive_tag_value(info.primitive_tag.as_deref())
                    }
                    IntrinsicId::TypeHasSecret => Value::Bool(info.has_secret),
                    IntrinsicId::TypeInfo => Self::reflection_type_info_value(info),
                    _ => return Err("invalid checked metadata operation".to_string()),
                })
            }
            IntrinsicId::TypeArg => {
                if let Some(result) = check_args(name, 1, &args) {
                    return result;
                }
                let info = info.ok_or("checked intrinsic has no exact reflection argument")?;
                let index = match args.first() {
                    Some(Value::Int64(index)) if *index >= 0 => *index as usize,
                    Some(other) => {
                        return Err(format!(
                            "type.arg expects a non-negative int64 index, got {other}"
                        ));
                    }
                    None => return Err("checked type.arg has no index".to_string()),
                };
                info.args
                    .get(index)
                    .map(Self::reflection_type_info_value)
                    .ok_or_else(|| {
                        format!(
                            "type.arg index {index} is out of range for type '{}'",
                            info.type_name
                        )
                    })
            }
            _ => {
                let resolved = prepared
                    .types()
                    .iter()
                    .map(|&ty| {
                        Self::debug_type(&checked.program().checked().interner.type_name(ty))
                            .ok_or_else(|| {
                                "checked intrinsic argument has no canonical interpreter type"
                                    .to_string()
                            })
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                self.call_function_from_resolved_source(name, &resolved, args, None)
            }
        }
    }
}
