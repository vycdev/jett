//! Typed, non-destructive rendering for compiler-owned debug observations.
use super::{Expr, FileId, Interpreter, ReflectionFieldInfo, Span, TypeExpr, Value};

impl Interpreter {
    pub(super) fn set_inferred_debug_binding(
        &mut self,
        name: &super::Ident,
        value: Value,
        fallback: Option<&TypeExpr>,
    ) {
        let ty = self
            .checked_expression_type(name.span)
            .and_then(|name| Self::debug_type(name))
            .or_else(|| fallback.cloned());
        if let Some(ty) = ty {
            self.set_variable_with_type(&name.name, value, ty);
        } else {
            self.set_variable(&name.name, value);
        }
    }

    pub(super) fn debug_expression_type(&self, expression: &Expr) -> Option<TypeExpr> {
        match expression {
            Expr::Clone(inner, _)
            | Expr::View(inner, _)
            | Expr::Paren(inner, _)
            | Expr::Run(inner, _) => self.debug_expression_type(inner),
            Expr::Join(inner, span) => {
                let ty = self.debug_expression_type(inner)?;
                if matches!(self.inference_base_type(&ty), TypeExpr::Generic(name, _, _) if name.name == "result")
                {
                    Some(ty)
                } else {
                    Some(TypeExpr::Generic(
                        super::Ident {
                            name: "result".into(),
                            span: *span,
                        },
                        vec![
                            ty,
                            TypeExpr::Named(super::Ident {
                                name: "string".into(),
                                span: *span,
                            }),
                        ],
                        *span,
                    ))
                }
            }
            Expr::Call(callee, args, _) | Expr::GenericCall(callee, _, args, _) => {
                let type_args = match expression {
                    Expr::GenericCall(_, args, _, _) => args.as_slice(),
                    _ => &[],
                };
                let actual = args
                    .iter()
                    .map(|arg| self.debug_expression_type(&arg.value))
                    .collect::<Option<Vec<_>>>()
                    .and_then(|actual| {
                        let order = self
                            .source_function_argument_order(callee, args, false)
                            .ok()?;
                        Self::reorder_function_arguments(actual, order.as_deref()).ok()
                    });
                self.debug_call_result_type(callee, type_args, actual.as_deref().unwrap_or(&[]))
                    .or_else(|| self.call_argument_type(expression))
            }
            Expr::FieldAccess(owner, field, _) => {
                let ty = self.debug_expression_type(owner)?;
                self.debug_fields(&ty, None)
                    .into_iter()
                    .find(|(name, _)| name == &field.name)
                    .map(|(_, ty)| ty)
                    .or_else(|| self.call_argument_type(expression))
            }
            _ => self.call_argument_type(expression),
        }
    }

    fn debug_call_result_type(
        &self,
        callee: &Expr,
        type_args: &[TypeExpr],
        actual: &[TypeExpr],
    ) -> Option<TypeExpr> {
        let callee = Self::unparenthesized(callee);
        if self.is_value_call_target(callee) {
            if let Some(TypeExpr::Function(_, result, _)) = self.call_argument_type(callee) {
                return Some(*result);
            }
            return None;
        }
        let source_name = Self::dotted_expr_name(callee)?;
        let name = self.registry_name(&self.functions, &source_name)?;
        let registered = self.functions.get(&name)?;
        let definition = &registered.definition;
        let inferred = self.inferred_user_function_type_args_from_types(callee, type_args, actual);
        let args = inferred.as_deref().unwrap_or(type_args);
        if args.len() != definition.type_params.len() {
            return None;
        }
        let substitutions = definition
            .type_params
            .iter()
            .zip(args)
            .map(|(param, arg)| (param.name.clone(), self.substitute_type_expr(arg)))
            .collect();
        definition.return_type.as_ref().map(|ty| {
            self.substitute_type_expr_with_map_in_namespace(
                ty,
                &substitutions,
                registered.namespace.as_deref(),
            )
        })
    }

    pub(super) fn debug_pipeline_result_type(
        &self,
        step: &super::PipelineStep,
        input: Option<&TypeExpr>,
    ) -> Option<TypeExpr> {
        let (callee, type_args, args) = match &step.function {
            Expr::GenericCall(callee, types, args, _) => {
                (callee.as_ref(), types.as_slice(), args.as_slice())
            }
            Expr::Call(callee, args, _) => (callee.as_ref(), &[][..], args.as_slice()),
            callee => (callee, &[][..], step.extra_args.as_slice()),
        };
        let mut actual = input.cloned().into_iter().collect::<Vec<_>>();
        if let Some(extra) = args
            .iter()
            .map(|arg| self.debug_expression_type(&arg.value))
            .collect::<Option<Vec<_>>>()
        {
            actual.extend(extra);
        } else {
            actual.clear();
        }
        let order = self
            .source_function_argument_order(callee, args, true)
            .ok()?;
        let actual = Self::reorder_function_arguments(actual, order.as_deref()).ok()?;
        self.debug_call_result_type(callee, type_args, &actual)
    }

    pub(super) fn debug_expression_args(&self, expression: &Expr) -> Vec<TypeExpr> {
        self.debug_type_args(self.debug_expression_type(expression).as_ref())
    }

    pub(super) fn debug_type_args(&self, ty: Option<&TypeExpr>) -> Vec<TypeExpr> {
        match ty.map(|ty| self.inference_base_type(ty)) {
            Some(TypeExpr::Generic(_, args, _)) => args,
            Some(ty @ TypeExpr::Named(_)) => vec![ty],
            _ => Vec::new(),
        }
    }

    pub(super) fn debug_type(name: &str) -> Option<TypeExpr> {
        let span = Span::new(FileId::new(0), 0, 0);
        if let Some(inner) = name.strip_prefix("view ") {
            return Some(TypeExpr::View(Box::new(Self::debug_type(inner)?), span));
        }
        if let Some((owner, args)) = Self::split_generic_type_display(name) {
            let args = Self::split_type_display_args(args)
                .into_iter()
                .map(|arg| Self::debug_type(arg.trim()))
                .collect::<Option<Vec<_>>>()?;
            return Some(TypeExpr::Generic(
                super::Ident {
                    name: owner.to_owned(),
                    span,
                },
                args,
                span,
            ));
        }
        if let Some((owner, state)) = name.rsplit_once(" at ") {
            return Some(TypeExpr::StateQualified(
                Box::new(Self::debug_type(owner)?),
                super::Ident {
                    name: state.to_owned(),
                    span,
                },
                span,
            ));
        }
        Some(TypeExpr::Named(super::Ident {
            name: name.to_owned(),
            span,
        }))
    }

    pub(super) fn debug_fields(
        &self,
        ty: &TypeExpr,
        member: Option<(&str, bool)>,
    ) -> Vec<(String, TypeExpr)> {
        let checked: Option<&[ReflectionFieldInfo]> = match member {
            Some((name, true)) => self.checked_machine(ty).and_then(|machine| {
                machine
                    .states
                    .iter()
                    .find(|state| state.name == name)
                    .map(|state| state.fields.as_slice())
            }),
            Some((name, false)) => self.checked_type_variants(ty).and_then(|variants| {
                variants
                    .iter()
                    .find(|variant| variant.name == name)
                    .map(|variant| variant.fields.as_slice())
            }),
            None => self.checked_type_fields(ty),
        };
        if let Some(fields) = checked {
            return fields
                .iter()
                .filter_map(|field| {
                    Self::debug_type(&field.type_name).map(|ty| (field.name.clone(), ty))
                })
                .collect();
        }
        let fields = match member {
            Some((name, true)) => self
                .type_expr_machine(ty)
                .states
                .into_iter()
                .find(|state| state.name == name)
                .map(|state| state.fields)
                .unwrap_or_default(),
            Some((name, false)) => self
                .type_expr_variants(ty)
                .into_iter()
                .find(|variant| variant.name == name)
                .map(|variant| variant.fields)
                .unwrap_or_default(),
            None => self.type_expr_fields(ty),
        };
        fields
            .into_iter()
            .map(|field| (field.name, field.ty))
            .collect()
    }

    fn debug_nominal_owner(&self, declared: Option<TypeExpr>, concrete: &str) -> Option<TypeExpr> {
        fn name(ty: &TypeExpr) -> Option<&str> {
            match ty {
                TypeExpr::Named(name) | TypeExpr::Generic(name, _, _) => Some(&name.name),
                TypeExpr::StateQualified(inner, _, _) | TypeExpr::View(inner, _) => name(inner),
                _ => None,
            }
        }
        // Interface bindings retain their declared label, but fields belong to
        // the concrete runtime owner. Keep generic arguments when the declared
        // and concrete nominal owners agree.
        declared
            .filter(|ty| name(ty) == Some(concrete))
            .or_else(|| Self::debug_type(concrete))
    }

    pub(super) fn format_debug_value(&self, value: &Value, ty: Option<&TypeExpr>) -> String {
        let ty = ty.map(|ty| self.inference_base_type(ty));
        if let Some(TypeExpr::View(inner, _)) = &ty {
            return self.format_debug_value(value, Some(inner));
        }
        let args = match &ty {
            Some(TypeExpr::Generic(name, _, _)) if name.name == "secret" => {
                return "[redacted]".to_owned();
            }
            Some(TypeExpr::Generic(_, args, _)) => args.as_slice(),
            _ => &[],
        };
        match value {
            Value::Typed { type_name, value } => {
                let concrete = Self::debug_type(type_name);
                self.format_debug_value(value, concrete.as_ref().or(ty.as_ref()))
            }
            Value::Pending(inner) => {
                format!("pending({})", self.format_debug_value(inner, ty.as_ref()))
            }
            Value::List(items) | Value::Set(items) => {
                let name = if matches!(value, Value::List(_)) {
                    "list"
                } else {
                    "set"
                };
                let items = items
                    .iter()
                    .map(|item| self.format_debug_value(item, args.first()))
                    .collect::<Vec<_>>();
                format!("{name}({})", items.join(", "))
            }
            Value::Map(entries) => {
                let entries = entries
                    .iter()
                    .map(|(key, value)| {
                        format!(
                            "{}: {}",
                            self.format_debug_value(key, args.first()),
                            self.format_debug_value(value, args.get(1)),
                        )
                    })
                    .collect::<Vec<_>>();
                format!("map({})", entries.join(", "))
            }
            Value::OptionalSome(inner) => {
                format!("some({})", self.format_debug_value(inner, args.first()))
            }
            Value::ResultOk(inner) => {
                format!("ok({})", self.format_debug_value(inner, args.first()))
            }
            Value::ResultFail(inner) => {
                format!("fail({})", self.format_debug_value(inner, args.get(1)))
            }
            Value::Struct {
                concrete_type,
                type_name,
                fields,
            } => {
                let owner = concrete_type
                    .as_deref()
                    .and_then(Self::debug_type)
                    .or_else(|| self.debug_nominal_owner(ty, type_name));
                let types = owner
                    .as_ref()
                    .map(|ty| self.debug_fields(ty, None))
                    .unwrap_or_default();
                let fields = fields
                    .iter()
                    .map(|(name, value)| {
                        let ty = types
                            .iter()
                            .find(|(field, _)| field == name)
                            .map(|(_, ty)| ty);
                        format!("{name}: {}", self.format_debug_value(value, ty))
                    })
                    .collect::<Vec<_>>();
                format!("{type_name}({})", fields.join(", "))
            }
            Value::Enum {
                type_name,
                variant: member,
                fields,
            }
            | Value::Machine {
                type_name,
                state: member,
                fields,
            } => {
                let machine = matches!(value, Value::Machine { .. });
                let owner = self.debug_nominal_owner(ty, type_name);
                let types = owner
                    .as_ref()
                    .map(|ty| self.debug_fields(ty, Some((member, machine))))
                    .unwrap_or_default();
                let separator = if machine { "@" } else { "." };
                let mut result = format!("{type_name}{separator}{member}");
                if !fields.is_empty() {
                    let fields = fields
                        .iter()
                        .enumerate()
                        .map(|(index, value)| {
                            self.format_debug_value(value, types.get(index).map(|(_, ty)| ty))
                        })
                        .collect::<Vec<_>>();
                    result.push_str(&format!("({})", fields.join(", ")));
                }
                result
            }
            Value::TypeConstruction {
                type_name,
                variant,
                state,
                fields,
            } => {
                let mut owner = type_name.clone();
                if let Some(variant) = variant {
                    owner.push_str(&format!(".{variant}"));
                }
                if let Some(state) = state {
                    owner.push_str(&format!("@{state}"));
                }
                let fields = fields
                    .iter()
                    .map(|(_, name, type_name, value)| {
                        let ty = Self::debug_type(type_name);
                        format!("{name}: {}", self.format_debug_value(value, ty.as_ref()))
                    })
                    .collect::<Vec<_>>();
                format!("TypeConstruction[{owner}]({})", fields.join(", "))
            }
            _ => value.to_string(),
        }
    }
}
