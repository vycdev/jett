//! One original checked PipelineStep, distinct from an Expr::Call packet.
use jett_parser::ast::{PipelineStep, TypeExpr};
use jett_typecheck::{CheckedCallerEffect, CheckedCallerOrigin, CheckedCallerSyntax};

use super::*;

pub(crate) struct PreparedPipelineStep<'source> {
    original: &'source PipelineStep,
    initial: Option<&'source Expr>,
    callee: &'source Expr,
    type_arguments: &'source [TypeExpr],
    extra_arguments: &'source [CallArg],
    invocation: CheckedInvocation,
    input_type: TypeId,
    raw_call_type: TypeId,
    output_type: TypeId,
    initial_borrow: Option<DefId>,
    constructor: Option<(String, String, TypeId)>,
}

impl<'source> PreparedPipelineStep<'source> {
    pub(crate) fn original(&self) -> &'source PipelineStep {
        self.original
    }
    pub(crate) fn initial(&self) -> Option<&'source Expr> {
        self.initial
    }
    pub(crate) fn callee(&self) -> &'source Expr {
        self.callee
    }
    pub(crate) fn type_arguments(&self) -> &'source [TypeExpr] {
        self.type_arguments
    }
    pub(crate) fn extra_arguments(&self) -> &'source [CallArg] {
        self.extra_arguments
    }
    pub(crate) fn invocation(&self) -> &CheckedInvocation {
        &self.invocation
    }
    pub(crate) fn input_type(&self) -> TypeId {
        self.input_type
    }
    pub(crate) fn raw_call_type(&self) -> TypeId {
        self.raw_call_type
    }
    pub(crate) fn output_type(&self) -> TypeId {
        self.output_type
    }
    pub(crate) fn initial_borrow(&self) -> Option<DefId> {
        self.initial_borrow
    }
    pub(crate) fn constructor(&self) -> Option<(String, String, TypeId)> {
        self.constructor.clone()
    }
}

impl CheckedExecution {
    pub(crate) fn prepare_pipeline_step<'source>(
        &self,
        pipeline: &'source Expr,
        index: usize,
    ) -> Result<PreparedPipelineStep<'source>, ResourceExecutionError> {
        self.validate_executable_cursor(&self.body)?;
        let reference = CheckedBodyReference {
            cursor: self.body.clone(),
        };
        let mut matches = 0usize;
        walk_region(
            reference.region()?,
            &mut |_| {},
            &mut |original| {
                if matches!(original, Expr::Pipeline(..)) && std::ptr::eq(original, pipeline) {
                    matches += 1;
                }
            },
            &mut |_| {},
        );
        if matches != 1 {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        let Expr::Pipeline(initial, steps, _) = pipeline else {
            return Err(ResourceExecutionError::MissingCheckedBody);
        };
        let step = steps
            .get(index)
            .ok_or(ResourceExecutionError::MissingCheckedInvocation)?;
        let input_span = if index == 0 {
            initial.span()
        } else {
            steps
                .get(index - 1)
                .ok_or(ResourceExecutionError::MissingCheckedInvocation)?
                .span
        };
        let (callee, type_arguments, extra_arguments, step_view) = pipeline_parts(step);
        let invocation = self.invocation(step.span)?;
        if invocation.arguments().len() != extra_arguments.len() + 1 {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        let source0 = &invocation.arguments()[0];
        let source_view = step_view || (index == 0 && written_view(initial));
        if source0.source_span != input_span
            || source0.source_index != 0
            || source0.parameter_index != 0
            || (source0.syntax == CheckedCallerSyntax::WrittenView) != source_view
            || (index != 0 && source0.origin != CheckedCallerOrigin::OwnedExpression)
        {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        for (argument, fact) in extra_arguments.iter().zip(&invocation.arguments()[1..]) {
            if argument.value.span() != fact.source_span
                || (fact.syntax == CheckedCallerSyntax::WrittenView)
                    != written_view(&argument.value)
            {
                return Err(ResourceExecutionError::InvalidInvocation);
            }
        }
        let facts = self.facts(&self.body)?;
        let input_type = *facts
            .pipeline_inputs
            .get(&step.span)
            .ok_or(ResourceExecutionError::MissingCheckedBody)?;
        let raw_call_type = *facts
            .pipeline_calls
            .get(&step.span)
            .ok_or(ResourceExecutionError::MissingCheckedBody)?;
        let output_type = *facts
            .source_types
            .get(&step.span)
            .ok_or(ResourceExecutionError::MissingCheckedBody)?;
        for ty in [input_type, raw_call_type, output_type] {
            if ty.index() as usize >= self.program.checked().interner.len() {
                return Err(ResourceExecutionError::InvalidCheckedProgram);
            }
        }
        // A Function signature is its declaration result, independently of
        // checked secret propagation at this actual invocation occurrence.
        if let CheckedInvocationShape::Intrinsic { result_type, .. } = &invocation.packet().shape {
            if raw_call_type != *result_type {
                return Err(ResourceExecutionError::InvalidInvocation);
            }
        }
        let initial_borrow =
            if index == 0 && source_view && source0.effect == CheckedCallerEffect::RetainBorrow {
                let mut bare = initial.as_ref();
                while let Expr::Paren(inner, _) | Expr::View(inner, _) = bare {
                    bare = inner;
                }
                if let (Expr::Ident(identifier), CheckedCallerOrigin::Binding(binding)) =
                    (bare, &source0.origin)
                {
                    if self.resolved_definition(identifier.span)? != binding.definition
                        || facts.bindings.get(&binding.declaration_span) != Some(binding)
                    {
                        return Err(ResourceExecutionError::InvalidInvocation);
                    }
                    Some(binding.definition)
                } else {
                    None
                }
            } else {
                None
            };
        // Keep root's closed enum preparation before evaluating even source0.
        let constructor = self.enum_constructor(callee, &invocation)?;
        Ok(PreparedPipelineStep {
            original: step,
            initial: (index == 0).then_some(initial.as_ref()),
            callee,
            type_arguments,
            extra_arguments,
            invocation,
            input_type,
            raw_call_type,
            output_type,
            initial_borrow,
            constructor,
        })
    }
}

fn written_view(mut expression: &Expr) -> bool {
    while let Expr::Paren(inner, _) | Expr::Coarsen(inner, _) | Expr::Declassify(inner, _) =
        expression
    {
        expression = inner;
    }
    matches!(expression, Expr::View(..))
}

fn pipeline_parts(step: &PipelineStep) -> (&Expr, &[TypeExpr], &[CallArg], bool) {
    let (function, view) = match &step.function {
        Expr::View(inner, _) => (inner.as_ref(), true),
        other => (other, false),
    };
    let (callee, types, arguments) = match function {
        Expr::Call(callee, arguments, _) => (callee.as_ref(), &[][..], arguments.as_slice()),
        Expr::GenericCall(callee, types, arguments, _) => {
            (callee.as_ref(), types.as_slice(), arguments.as_slice())
        }
        other => (other, &[][..], step.extra_args.as_slice()),
    };
    let mut callee = callee;
    while let Expr::Paren(inner, _) = callee {
        callee = inner;
    }
    (callee, types, arguments, view)
}

#[cfg(test)]
#[path = "pipeline/tests.rs"]
mod tests;
